//! Finalized-block drain, checkpoint, and epoch-cutover handling.

use commonware_codec::DecodeExt as _;
use commonware_cryptography::{Signer as _, ed25519};
use commonware_p2p::{Manager as _, Recipients, Sender as _};
use commonware_runtime::{Clock as _, IoBuf};
use commonware_utils::ordered::Set;

use consensus::ContentStore;
use directory::{DirQuery, DirReply, decode_reply, encode_query};
use recovery::Manifest;
use sdk::Msg;

use super::super::announce::{dispatch_pending_deliveries, saga_next_expiry};
use super::ValidatorRuntime;
use crate::constants::{DRAIN_TICK, NOP_TARGET};
use crate::drain_actions::{CutoverTrigger, EpochActions};
use noded::projection::{BlockProjection, project_block};
use crate::host_reads::{
    read_valset_members, read_valset_residents,
};
use crate::{lobby, relay};
use crate::util::{fatal, hex, participant_bytes, resident_bytes};

impl ValidatorRuntime<'_> {
    pub(super) async fn on_drain(&mut self) {

        let Self {
            context,
            node,
            orchestrator,
            epoch_spawner,
            last_cert_height,
            latest_floor,
            mesh_oracle,
            gateway_book,
            media_peers,
            blob_peers,
            reach_cmd,
            relay_tx,
            gate_outcomes,
            next_seq,
            prev_ckpt,
            signer,
            label,
            peers,
            checkpoint_blocks,
            sync_lease,
            stream_hub,
            index,
            blobs,
            metrics,
            applied,
            pending_submits,
            pending_relays,
            pending_gates,
            gating,
            validator_relay,
            last_published,
            blocks_since_checkpoint,
            last_reach_view,
            pending_retarget,
            next_drain,
            ..
        } = self;
        let context = *context;
        let checkpoint_blocks = *checkpoint_blocks;

        *next_drain = context.current() + DRAIN_TICK;
        // FAIL-STOP: a drain error is a node-local block-boundary
        // fault — this node's state is indeterminate relative to its
        // peers, so applying even one more finalized op could
        // silently fork it. exit loudly; an operator (or supervisor)
        // restarts the node, which then re-joins via state sync.
        let drained_count = match node.drain_delivered().await {
            Ok(n) => n,
            Err(e) => {
                fatal!(label, "{e} — halting");
            }
        };
        *applied += drained_count;
        // durabilize the tip seal when the chain goes idle. a seal is a
        // plain journal append made durable only by the NEXT block's
        // pre-apply sync; on an idle chain the tip block's seal can sit
        // un-synced for a whole block-time, and a crash there loses it,
        // turning the tip into a TRAILING block. that is fine for most
        // ops, but a trailing SELF-READING op — a files CAS commit whose
        // re-execution reads the claimant's already-durable post-state —
        // cannot be selective-replayed and would brick a SOLO node (no
        // peer to re-sync from). syncing on the idle transition closes
        // the window; a busy chain amortizes durability against the next
        // pre-apply and needs no extra sync here.
        if drained_count > 0
            && node.pending_batch_len() == 0
            && node.orderer().pending_len() == 0
            && let Err(e) = node.sink_mut().sync().await
        {
            // the idle-transition durability sync: losing it turns the tip into a
            // TRAILING block, which on a SOLO node can brick a self-reading op.
            tracing::warn!(
                target: "ducktape::consensus",
                node = %label,
                error = %e,
                "tip-seal sync failed — the tip block may not be durable"
            );
        }
        // resolve held app-surface submits against what this
        // drain finished with; every disposition is deterministic,
        // so the reply faithfully reports the op's consensus fate.
        let drained = node.take_drained();
        // sealed = journaled: one seal per BLOCK (height), whatever a
        // batch's member count. count DISTINCT sealed heights so the
        // checkpoint cadence stays per-block; applied and rejected
        // members both seal, discarded frames never sealed a height.
        *blocks_since_checkpoint += drained
            .iter()
            .filter(|d| d.disposition != node::Disposition::Discarded)
            .map(|d| d.height)
            .collect::<std::collections::BTreeSet<u64>>()
            .len() as u64;
        // The orderer-independent projection keeps member/System order,
        // explorer rows, and discard handling identical to the replica.
        for projection in project_block(&drained, node.take_system_dispatches(), blobs) {
            let BlockProjection {
                height,
                dispatches,
                record,
                applied,
                latency_us,
                applied_ops,
                rejected_ops,
                ..
            } = projection;
            // one block per height: an APPLIED block records fully
            // (count, this node's summed apply latency, per-module
            // dispatch counters); an all-rejected block (the idle nop
            // lands here) only follows the height gauge. ops_total
            // counts the aggregated member ops.
            if applied {
                metrics.record_block(height, latency_us, &dispatches);
            } else {
                metrics.record_height(height);
            }
            metrics.record_op_outcomes(applied_ops, rejected_ops);
            // this lane's agreed clock IS the height: the drain stamps
            // BlockContext { consensus_time: height } for every block. the
            // shared index-fold epilogue owns the STALE-index error log.
            noded::projection::apply_block_to_index(index, height, height, record, &dispatches);
        }
        for d in drained {
            // a DISCARD is not this hold's outcome: the cutover
            // carries the frame into the new epoch under the SAME
            // FrameId, so the hold stays open until the carried
            // frame finalizes there (or SUBMIT_HOLD expires into
            // the truthful re-query reply).
            if d.disposition == node::Disposition::Discarded {
                continue;
            }
            // resolve a HELD JOIN GATE (ADR §3.2 / join ADR §4): the joiner's
            // outcome was held against this Redeem frame — now the drain knows
            // its consensus fate. Applied ⇒ the AUTHORITATIVE Admitted at the
            // committed height (carrying the coord cap); Rejected ⇒ map the
            // module reason to a code + terminal bit (chiefly a spent-nonce
            // race the pre-filter missed). the outcome goes into the shared
            // `gate_outcomes` map, where the intro doorbell answers the
            // joiner's next tunnel retransmit — this loop owns no doorbell
            // socket. a gate frame is EXCLUSIVELY a gate, so resolve and move
            // on.
            if let Some(gate) = pending_gates.remove(&d.id) {
                gating.remove(&gate.joiner);
                let reply = match d.disposition {
                    node::Disposition::Applied => lobby::IntroReply::Admitted {
                        height: d.height,
                        cap: gate.cap,
                    },
                    node::Disposition::Rejected => {
                        // settle-race guard: this member's Redeem lost to a
                        // SIBLING member's grant for the same joiner (a slow
                        // settle swept us to Busy, the joiner failed over, and
                        // both Redeems batched — governance answers "already
                        // redeemed" to the loser). the joiner IS admitted; a
                        // terminal Spent here would `exit(1)` a granted join.
                        // if the joiner now holds resident standing, answer
                        // Admitted, not Spent.
                        let admitted = read_valset_residents(node.host())
                            .await
                            .iter()
                            .any(|r| r.as_slice() == gate.joiner.as_slice());
                        if admitted {
                            lobby::IntroReply::Admitted {
                                height: d.height,
                                cap: gate.cap,
                            }
                        } else {
                            let (code, terminal) =
                                lobby::redeem_reject_outcome(d.reason.as_deref());
                            lobby::IntroReply::Rejected {
                                code,
                                detail: d.reason.clone().unwrap_or_else(|| {
                                    "invite redemption rejected in consensus".into()
                                }),
                                terminal,
                            }
                        }
                    }
                    node::Disposition::Discarded => unreachable!("filtered at the loop top"),
                };
                // on a GRANT, re-track the just-admitted resident onto the
                // mesh oracle IMMEDIATELY (blessed decision #1): its real key
                // must complete the discovery handshake to dial statesync,
                // and the epoch cutover that formally re-tracks it lands a
                // few views later — a gap the joiner would spend bounced at
                // the door. every validator resolves this same Applied block
                // in its own drain, so the widened set converges within a
                // beat (the same transient the reboot-inside-cutover window
                // already tolerates).
                if let lobby::IntroReply::Admitted { .. } = &reply
                    && let Ok(joiner_pk) = ed25519::PublicKey::decode(gate.joiner.as_slice())
                {
                    let mut transport: std::collections::BTreeSet<ed25519::PublicKey> =
                        orchestrator
                            .current_members()
                            .iter()
                            .chain(orchestrator.current_residents())
                            .cloned()
                            .collect();
                    transport.insert(joiner_pk);
                    mesh_oracle.track(
                        orchestrator.epoch(),
                        super::super::wiring::mesh_at(peers, &transport),
                    );
                }
                super::settle_gate(gate_outcomes, gate.joiner, reply);
                continue;
            }
            // resolve a relayed hold FIRST: a relayed frame has no
            // local pending_submits entry, so this must precede the
            // `else { continue }` below or the wire Reply is lost.
            if let Some((peer, _)) = pending_relays.remove(&d.id) {
                let outcome = match d.disposition {
                    node::Disposition::Applied => relay::RelayOutcome::Applied {
                        height: d.height,
                        app_hash: hex(&d.app_hash),
                    },
                    node::Disposition::Rejected => relay::RelayOutcome::Rejected {
                        // carry the module's VERBATIM reason (node-
                        // local observability off the DrainedFrame)
                        // so the resident forwards it to its caller
                        // — the duckfs-client engine keys on the
                        // "files: conflict:" prefix. generic wording
                        // only when the drain captured no reason.
                        detail: d.reason.clone().unwrap_or_else(|| {
                            "op finalized but rejected (deterministic no-op)".into()
                        }),
                    },
                    node::Disposition::Discarded => unreachable!("filtered at the loop top"),
                };
                let msg = relay::RelayMsg::Reply {
                    frame_id: d.id,
                    outcome,
                };
                let _ = relay_tx.send(
                    Recipients::One(peer),
                    IoBuf::from(relay::encode_msg(&msg)),
                    false,
                );
            }
            // BEFORE the pending_submits lookup, deliberately. An op rejected in
            // consensus produced no record ANYWHERE: the submitter's own log says
            // SUCCESS (the submit was accepted) while the state machine says NO.
            //
            // and the internal submits — oracle results, capability announces,
            // upgrade readiness, code-ready signals — are fire-and-forget and never
            // enter `pending_submits` at all, so the `continue` below swallows their
            // rejection whole. That is exactly how an announcer that latches on
            // submit-Ok wedges FOREVER: silently out of every rendezvous pool, the
            // upgrade stuck at R<n, and nothing anywhere saying why.
            let module = d.op.as_ref().map_or("system", |op| op.target.as_str());
            // the idle-chain NOP filler is rejected BY DESIGN — it targets a module
            // that deliberately does not exist. warning on it would fire every block
            // forever on an idle chain, evicting the whole 4096-line ring in ~68
            // minutes and drowning the very evidence someone came to read.
            if d.disposition == node::Disposition::Rejected && module != NOP_TARGET {
                tracing::warn!(
                    target: "ducktape::submit",
                    node = %label,
                    frame = %noded::hex_bytes(&d.id),
                    height = d.height,
                    module,
                    reason = %d.reason.as_deref().unwrap_or("deterministic_no_op"),
                    "op rejected in consensus"
                );
            }
            let Some((reply, _)) = pending_submits.remove(&d.id) else {
                continue;
            };
            let _ = reply.send(match d.disposition {
                node::Disposition::Applied => Ok(noded::BlockSummary {
                    height: d.height,
                    // the PER-BLOCK boundary this frame settled at
                    // (not the end-of-drain hash — a drain can
                    // apply several blocks).
                    app_hash: hex(&d.app_hash),
                }),
                node::Disposition::Rejected => Err(d.reason.clone().unwrap_or_else(|| {
                    // the module's VERBATIM reason when the drain
                    // captured one (duckfs-client keys on the
                    // "files: conflict:" prefix); generic wording
                    // otherwise.
                    "op finalized but rejected (deterministic no-op)".into()
                })),
                // unreachable — filtered at the loop top — but
                // stay total rather than panic.
                node::Disposition::Discarded => continue,
            });
        }
        validator_relay.expire(context.current(), relay_tx);
        // expire holds the mesh never finalized in time. the op may
        // still land later — clients re-query on block events.
        if !pending_submits.is_empty() {
            let now = context.current();
            let expired: Vec<node::FrameId> = pending_submits
                .iter()
                .filter(|(_, (_, deadline))| *deadline <= now)
                .map(|(k, _)| *k)
                .collect();
            for k in expired {
                if let Some((reply, _)) = pending_submits.remove(&k) {
                    let _ = reply.send(Err(
                        "timed out awaiting finalization — re-query on the next block".into(),
                    ));
                }
            }
        }
        // the same expiry contract for relayed holds: the mesh never
        // finalized in time, so answer the resident truthfully — the
        // op may still land, it re-queries on the next block.
        if !pending_relays.is_empty() {
            let now = context.current();
            let expired: Vec<node::FrameId> = pending_relays
                .iter()
                .filter(|(_, (_, deadline))| *deadline <= now)
                .map(|(k, _)| *k)
                .collect();
            for k in expired {
                if let Some((peer, _)) = pending_relays.remove(&k) {
                    let msg = relay::RelayMsg::Reply {
                        frame_id: k,
                        outcome: relay::RelayOutcome::Refused {
                            detail: "timed out awaiting finalization — re-query on the next block"
                                .into(),
                        },
                    };
                    let _ = relay_tx.send(
                        Recipients::One(peer),
                        IoBuf::from(relay::encode_msg(&msg)),
                        false,
                    );
                }
            }
        }
        // held join gates that never settled within GATE_SETTLE_TIMEOUT: write
        // Busy (NON-terminal, §3.2) into the outcome map so the joiner's next
        // retransmit reads it and fails over to another member rather than
        // exiting. the Redeem may still land later — a re-forward then hits
        // the V9 idempotent Admitted.
        if !pending_gates.is_empty() {
            let now = context.current();
            let expired: Vec<node::FrameId> = pending_gates
                .iter()
                .filter(|(_, g)| g.deadline <= now)
                .map(|(k, _)| *k)
                .collect();
            for k in expired {
                if let Some(gate) = pending_gates.remove(&k) {
                    gating.remove(&gate.joiner);
                    super::settle_gate(
                        gate_outcomes,
                        gate.joiner,
                        lobby::IntroReply::Rejected {
                            code: lobby::RejectCode::Busy,
                            detail: "the gate could not settle in time — trying another member"
                                .into(),
                            terminal: false,
                        },
                    );
                }
            }
        }
        // publish each newly-applied boundary to ws subscribers
        // (send only errs when nobody is subscribed — fine). the
        // drain loop above already folded each block into the
        // metrics series; this tip seam carries the ws block
        // summary only — it fires once per drain.
        if let Some(f) = node.finalized()
            && *last_published != Some(f.height)
        {
            stream_hub.publish_block(f.height, hex(&f.app_hash));
            *last_published = Some(f.height);
        }

        // persist the finalization floor for the newest certificate
        // whose view has fully drained. read the certificate FIRST,
        // the release point second: releases happen only on this
        // thread, so a certificate strictly below every slot still
        // pending at the later read is fully applied — a floor ahead
        // of app state would suppress replay of finalized ops a
        // restart still needs. matched to the SEALED tip view (not
        // just the newest certificate) and deliberately NOT gated on
        // a momentarily empty inbox: on a busy chain a fresh
        // finalization is (nearly) always in flight, and the empty-
        // inbox gate starved — the floor stopped tracking the tip and
        // the statesync boundary serve (floor == tip, exactly)
        // refused every joiner for as long as the load lasted.
        if let Some(tip_view) = node.finalized_view()
            && let Some((view, cert)) = node.orderer().finalization_at_or_below(tip_view)
            && view != 0
            && node
                .orderer()
                .min_unreleased_view()
                .is_none_or(|pending| pending > view)
        {
            let height = orchestrator.app_height(view);
            if last_cert_height.is_none_or(|h| height > h) {
                let fc = recovery::FloorCert {
                    epoch: orchestrator.epoch(),
                    height,
                    cert,
                };
                match node.sink_mut().write_floor_cert(&fc).await {
                    Ok(()) => {
                        *last_cert_height = Some(height);
                        *latest_floor = Some(fc);
                    }
                    Err(e) => tracing::warn!(
                        target: "ducktape::recovery",
                        node = %label,
                        height,
                        error = %e,
                        "floor cert write failed; retrying"
                    ),
                }
            }
        }

        // periodic checkpoint: snapshot the in-memory cohort and
        // prune the op journal below the PREVIOUS checkpoint once
        // the persisted floor has passed it (pruned frames must
        // never be needed to resolve a re-reported finalization).
        if *blocks_since_checkpoint >= checkpoint_blocks
            && let Some(f) = node.finalized()
        {
            let pos = node.sink_mut().oplog_pos().await;
            let captured = Manifest::capture(
                node.host(),
                Some(f.height),
                orchestrator.epoch(),
                orchestrator.epoch_base(),
                participant_bytes(orchestrator),
                resident_bytes(orchestrator),
                orchestrator.pending_cutover().map(|c| c.cutover_view()),
                pos,
                *next_seq,
            );
            match captured {
                Ok(m) => match node.sink_mut().write_manifest(&m).await {
                    Ok(()) => {
                        *blocks_since_checkpoint = 0;
                        let floor_passed = matches!(
                            node.sink_mut().floor_cert(),
                            Ok(Some(fc))
                                if prev_ckpt.0.is_none_or(|h| fc.height >= h)
                        );
                        let lease_active = crate::sync::serve::sync_lease_active(sync_lease);
                        if floor_passed && lease_active {
                            // a syncer is actively pulling from this node:
                            // pruning now would yank its boundary away and put
                            // it on the rebootstrap treadmill. defer — the
                            // next checkpoint prunes once the lease lapses.
                            tracing::debug!(
                                target: "ducktape::statesync",
                                node = %label,
                                reason = "sync_lease_active",
                                "oplog prune deferred"
                            );
                        } else if floor_passed
                            && let Err(e) = node.sink_mut().prune_oplog(prev_ckpt.1).await
                        {
                            tracing::warn!(
                                target: "ducktape::recovery",
                                node = %label,
                                error = %e,
                                "oplog prune failed"
                            );
                        }
                        *prev_ckpt = (m.height, pos);
                        tracing::info!(
                            target: "ducktape::recovery",
                            event = "node_checkpoint_written",
                            node = %label,
                            height = m.height.unwrap_or_default()
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            event = "node_checkpoint_failed",
                            node = %label,
                            stage = "write",
                            error = %e
                        );
                    }
                },
                Err(e) => {
                    tracing::warn!(
                        target: "ducktape::recovery",
                        event = "node_checkpoint_failed",
                        node = %label,
                        stage = "capture",
                        error = %e
                    );
                }
            }
        }

        // the VALSET ORCHESTRATION step: observe the finalized
        // membership projection; a change schedules a deterministic
        // cutover (arming the discard ceiling), and crossing the
        // cutover view tears the engine down and respawns it over
        // the set read AT the boundary. the observation barrier
        // guarantees this tick's last view IS the changing block's
        // view when membership moved.
        if let Some(engine_view) = node.last_engine_view() {
            // tick the reachability plane's freshness clock.
            // engine views are EPOCH-LOCAL (they reset at every
            // cutover), so convert to the absolute app-height
            // clock (`epoch_base + view`) — the regime the boot
            // Retarget's `view_base` put the plane's advert and
            // handshake expiries in.
            if let Some(cmd) = &reach_cmd {
                let absolute_view = orchestrator.app_height(engine_view);
                if last_reach_view.is_none_or(|v| v < absolute_view) {
                    // NON-BLOCKING: the plane is not consensus. a
                    // full command queue (a wedged or slow plane)
                    // sheds this tick — the next drain beat carries
                    // a fresher one — instead of stalling the loop
                    // behind an actor that may never drain.
                    let _ =
                        cmd.try_send(reachability::ReachabilityCommand::ViewTick(absolute_view));
                    *last_reach_view = Some(absolute_view);
                }
                // flush a staged cutover Retarget (see
                // `pending_retarget`) — MUST eventually land, so
                // it retries every beat rather than being shed.
                if let Some(event) = pending_retarget.take()
                    && let Err(tokio::sync::mpsc::error::TrySendError::Full(
                        reachability::ReachabilityCommand::Retarget(event),
                    )) = cmd.try_send(reachability::ReachabilityCommand::Retarget(event))
                {
                    *pending_retarget = Some(event);
                }
            }
            let members_raw = read_valset_members(node.host()).await;
            let mut observed: Vec<ed25519::PublicKey> = Vec::new();
            for key in &members_raw {
                if let Ok(pk) = ed25519::PublicKey::decode(key.as_slice()) {
                    observed.push(pk);
                }
            }
            // the RESIDENT projection, read at the same frozen
            // point: a grant/revoke arms the same single cutover
            // slot (mesh admission is epoch-scoped).
            let residents_raw = read_valset_residents(node.host()).await;
            let mut observed_residents: Vec<ed25519::PublicKey> = Vec::new();
            for key in &residents_raw {
                if let Ok(pk) = ed25519::PublicKey::decode(key.as_slice()) {
                    observed_residents.push(pk);
                }
            }
            let mut actions =
                EpochActions::new(orchestrator, engine_view, observed, observed_residents);
            if let Some(CutoverTrigger::Membership(cutover)) = actions.observe_members() {
                tracing::info!(
                    target: "ducktape::consensus",
                    node = %label,
                    observed_view = cutover.observed_view(),
                    next_epoch = cutover.next_epoch(),
                    cutover_view = cutover.cutover_view(),
                    "membership change observed"
                );
                node.set_view_ceiling(cutover.cutover_view());
            }
            if let Some(plan) = actions.respawn() {
                let members = plan.valset().consensus_members();
                let member_bytes: Vec<Vec<u8>> =
                    members.iter().map(|k| k.as_ref().to_vec()).collect();
                let plan_residents: Vec<ed25519::PublicKey> = plan
                    .valset()
                    .transport_members()
                    .difference(members)
                    .cloned()
                    .collect();
                let plan_resident_bytes: Vec<Vec<u8>> =
                    plan_residents.iter().map(|k| k.as_ref().to_vec()).collect();
                // transport FIRST: the new epoch's mesh must admit
                // its members (a fresh joiner — or a granted
                // resident — above all) before anything is
                // expected of them. the mesh tracks the TRANSPORT
                // union; the engine below gets validators only.
                // index = epoch, strictly increasing across
                // cutovers.
                mesh_oracle.track(
                    plan.epoch(),
                    super::super::wiring::mesh_at(peers, plan.valset().transport_members()),
                );
                // the gateway plane serves (and admits) exactly
                // who the mesh tracks — follow the re-track.
                if let Some(book) = &gateway_book {
                    book.set_peers(plan.valset().transport_members().iter());
                }
                // the media planes authenticate inbound by the same
                // tracked set — follow the re-track too, so a
                // just-added member's huddle media is admitted.
                if let Some(peers) = &media_peers {
                    peers.set_peers(plan.valset().transport_members().iter());
                }
                // the blob code lane's peer book follows the same
                // cutover — a fetch after a membership change asks
                // the members that actually exist.
                *blob_peers.write().expect("blob peers lock") =
                    plan.valset().transport_members().iter().cloned().collect();
                // the reachability plane retunnels for the new
                // member set the moment transport admits it —
                // with the epoch's resident tier as the pre-warm
                // standbys, so a registered joiner's tunnels
                // assemble ahead of its activation cutover.
                // cutover_app_height IS the new epoch's absolute
                // view at engine view 0 — the raw engine_view
                // here would be epoch-local, a different clock
                // than the ViewTicks above and the boot
                // Retarget's view_base.
                if reach_cmd.is_some() {
                    // STAGED, not sent inline: the flush below
                    // (every drain beat) try_sends it, so a plane
                    // whose queue is full delays retunneling by
                    // beats — it can never stall the cutover or
                    // the loop.
                    *pending_retarget = Some(reachability::MeshEpochEvent {
                        epoch: plan.epoch(),
                        members: members.iter().cloned().collect(),
                        standbys: plan_residents.clone(),
                        current_view: plan.cutover_app_height(),
                    });
                }
                if !members.contains(&signer.public_key()) {
                    tracing::info!(
                        target: "ducktape::consensus",
                        node = %label,
                        epoch = plan.epoch(),
                        "demoted from the validator set; halting"
                    );
                    std::process::exit(0);
                }
                let participants: Set<ed25519::PublicKey> =
                    Set::try_from(members.iter().cloned().collect::<Vec<_>>())
                        .expect("orchestrator membership has no duplicates");
                // a fresh epoch: new store (pins of the torn-down
                // epoch die with it), genesis floor.
                let orderer =
                    epoch_spawner.spawn(plan.epoch(), participants, ContentStore::new(), None);
                match node
                    .cutover(
                        orderer,
                        plan.epoch(),
                        plan.cutover_app_height(),
                        &member_bytes,
                        &plan_resident_bytes,
                    )
                    .await
                {
                    // the accept contract crossing the boundary:
                    // every locally-accepted op the old epoch
                    // never resolved was re-proposed into the
                    // new engine.
                    Ok(carried) if carried > 0 => tracing::info!(
                        target: "ducktape::consensus",
                        node = %label,
                        carried,
                        epoch = plan.epoch(),
                        "accepted ops carried across the cutover"
                    ),
                    Ok(_) => {}
                    Err(e) => {
                        fatal!(label, "{e} — halting");
                    }
                }
                // checkpoint IMMEDIATELY: the manifest must record
                // the new epoch's participant set (the journal's
                // cutover record alone covers only the crash
                // window until this write lands).
                let pos = node.sink_mut().oplog_pos().await;
                let captured = Manifest::capture(
                    node.host(),
                    node.finalized().map(|f| f.height),
                    orchestrator.epoch(),
                    orchestrator.epoch_base(),
                    participant_bytes(orchestrator),
                    resident_bytes(orchestrator),
                    None,
                    pos,
                    *next_seq,
                );
                match captured {
                    Ok(m) => match node.sink_mut().write_manifest(&m).await {
                        Ok(()) => {
                            *blocks_since_checkpoint = 0;
                            *prev_ckpt = (m.height, pos);
                        }
                        Err(e) => tracing::warn!(
                            target: "ducktape::recovery",
                            node = %label,
                            error = %e,
                            "post-cutover checkpoint write failed; the cutover journal record \
                             covers a restart"
                        ),
                    },
                    Err(e) => tracing::warn!(
                        target: "ducktape::recovery",
                        node = %label,
                        error = %e,
                        "post-cutover checkpoint capture failed; the cutover journal record \
                         covers a restart"
                    ),
                }
                tracing::info!(
                    target: "ducktape::consensus",
                    node = %label,
                    epoch = plan.epoch(),
                    validators = members.len(),
                    base_height = plan.cutover_app_height(),
                    "cutover complete: epoch {} with {} validators",
                    plan.epoch(),
                    members.len()
                );
            }
        }

        // the state-driven pumps, each its own method below: block
        // cadence/heartbeat, code readiness, capability announce,
        // saga crank, dispatch delivery nudge.
        self.pump_heartbeat().await;
        self.pump_code_readiness().await;
        self.pump_capability_announce().await;
        self.pump_saga_crank().await;
        self.pump_dispatch_nudge().await;

        let Self {
            node,
            next_seq,
            signer,
            label,
            dev_demo,
            expected,
            applied,
            converged,
            workers,
            ..
        } = self;
        let dev_demo = *dev_demo;
        let expected = *expected;

        // the reactor seam: offer each finalized block's events to
        // the host-owned workers; a claiming worker's follow-up op
        // re-enters through the ordered lane as its own block (the
        // oracle-as-op). events no worker claims are the plain
        // observability stream — only decodable-but-unhandled worker
        // requests would indicate a saga stuck Pending.
        // one drain can apply MANY blocks; the events accumulated across all of
        // them, so stamp them with the drain's finalized tip.
        let height = node.finalized().map_or(0, |f| f.height);
        // the same worker routing the noded submit lane and the sim run — but
        // each claimed follow-up re-enters the ORDERED lane as its own batch
        // (not an inline block), so this lane SUBMITS the follows rather than
        // inline-draining them: the continuous block cadence carries them.
        let host::worker::Offered { follows, unclaimed } =
            host::worker::offer(workers.as_slice(), node.take_events()).await;
        for follow in follows {
            let seq = *next_seq;
            *next_seq += 1;
            if let Err(e) = node.submit(signer, seq, follow).await {
                tracing::warn!(
                    target: "ducktape::modules",
                    node = %label,
                    height,
                    error = %e,
                    "worker follow-up submit failed"
                );
            }
        }
        // an unclaimed event is the module's ONLY diagnostic channel (a wasm
        // guest cannot log) — unless it decodes as a worker request, which
        // means a saga is stuck Pending.
        let mut notes = noded::log::ModuleNotes::new(height);
        for eff in &unclaimed {
            notes.unclaimed(eff);
        }
        notes.finish();
        if dev_demo && !*converged && *applied >= expected {
            let h = node.app_hash();
            tracing::info!(
                target: "ducktape::consensus",
                "node={label} converged app_hash={}", hex(&h)
            );
            // dump every directory key so the demo can eyeball the ops
            // (each node ends holding the op it originated AND the peer's).
            for k in 0..expected {
                let reply = node
                    .host()
                    .query(
                        "directory",
                        &encode_query(&DirQuery::Get {
                            key: format!("k{k}"),
                        }),
                    )
                    .await
                    .expect("directory query");
                if let Ok(DirReply::Value(v)) = decode_reply(&reply) {
                    tracing::debug!(
                        target: "ducktape::modules",
                        node = %label,
                        key = %format_args!("k{k}"),
                        value = ?v,
                        "demo directory value"
                    );
                }
            }
            *converged = true;
        }
    }

    // BLOCK CADENCE + heartbeat, unified. `submit`/`submit_frame` ENQUEUE into
    // the node's `pending_batch`; this arm is the one place that FLUSHES the
    // window — packing every frame it holds (real ops and/or an idle nop) into
    // ONE batch super-frame proposed as a single block — on TWO cadences,
    // decided purely in [`heartbeat_action`]:
    //
    // - BUSY: a window holding real ops flushes on THIS drain tick. the
    //   block-interval floor is `DRAIN_TICK` (the pump never runs this arm
    //   faster), so agreement runs at max speed while everything that arrived
    //   within one tick still aggregates into one block — the 1-tx-1-block
    //   regime stays dead.
    // - IDLE: an empty window beats one nop per `BLOCK_TIME`. finalized views
    //   only advance with a proposed frame, so an idle network would freeze
    //   (its height never ticks and a pending cutover, which crosses only when
    //   finalized views REACH it, would park forever). the nop targets an
    //   unregistered module: it rejects identically everywhere and leaves no
    //   state. a window with real ops needs no nop — the ops ARE the block.
    //
    // GATE the idle nop on an empty orderer FIFO too: a nop pushed
    // while a batch still awaits finalization only piles behind a
    // finalization stall (a flapping quorum peer would stack idle
    // blocks). real ops are never gated — they must not wait.
    async fn pump_heartbeat(&mut self) {
        let now = self.context.current();
        let heartbeat_due = now.duration_since(self.last_flush).unwrap_or_default()
            >= consensus::BLOCK_TIME;
        let ops_pending = self.node.pending_batch_len() > 0;
        let orderer_idle = self.node.orderer().pending_len() == 0;
        match heartbeat_action(self.heartbeat_disabled, ops_pending, heartbeat_due, orderer_idle) {
            HeartbeatAction::Idle => {}
            HeartbeatAction::Flush => self.flush_window(now).await,
            HeartbeatAction::BeatNop => self.beat_nop(now).await,
        }
    }

    /// submit one heartbeat nop, then flush it as the idle beat's block.
    async fn beat_nop(&mut self, now: std::time::SystemTime) {
        let Self {
            node,
            next_seq,
            signer,
            label,
            ..
        } = self;
        let seq = *next_seq;
        *next_seq += 1;
        if let Err(e) = node
            .submit(
                signer,
                seq,
                Msg {
                    target: NOP_TARGET.into(),
                    payload: Vec::new(),
                },
            )
            .await
        {
            tracing::debug!(
                target: "ducktape::submit",
                node = %label,
                error = %e,
                "heartbeat nop submit failed"
            );
        }
        self.flush_window(now).await;
    }

    /// restart the beat grid, then flush the window: no-op when
    /// `pending_batch` is empty (idle with a batch already in flight — wait
    /// for it).
    async fn flush_window(&mut self, now: std::time::SystemTime) {
        self.last_flush = now;
        if let Err(e) = self.node.flush_batch().await {
            tracing::debug!(
                target: "ducktape::submit",
                node = %self.label,
                error = %e,
                "batch flush failed"
            );
        }
    }

    // CODE READINESS: the byte-receipt half of a pending modreg swap.
    // a current boundary member checks the committed pending swaps against
    // its LOCAL blob store: verified-resident bytes earn one truthful
    // validator-origin `SignalReady` (the covering signal latches the swap
    // `ready` in consensus); missing bytes spawn one ranged mesh fetch
    // (the custodian's data-plane push normally lands first — this heals a
    // node the push missed). state-driven and idempotent; inert while
    // nothing is pending.
    async fn pump_code_readiness(&mut self) {
        let Self {
            node,
            orchestrator,
            next_seq,
            signer,
            label,
            code_signaller,
            blob_client,
            blobs,
            fetch_done_tx,
            fetch_done_rx,
            ..
        } = self;
        // reap finished fetch tasks first, so a failed fetch retries.
        while let Ok(digest) = fetch_done_rx.try_recv() {
            code_signaller.fetching.remove(&digest);
        }
        if !orchestrator
            .current_members()
            .contains(&signer.public_key())
        {
            return;
        }
        let req = lifecycle::encode_query(&lifecycle::LifecycleQuery::ModuleStatus);
        let Ok(bytes) = node.host().query(host::LIFECYCLE_MODULE_ID, &req).await else {
            return; // registry absent: byte-identical drain on a baseline net.
        };
        let Ok(lifecycle::LifecycleReply::ModuleStatus { modules }) = lifecycle::decode_reply(&bytes)
        else {
            return;
        };
        // residency is a VERIFYING read (content re-hashed on the disk path):
        // signing ready must mean sha256(local bytes) == committed hash.
        let actions = code_signaller.decide(&modules, |digest| blobs.has_chunk(digest));
        for digest in actions.fetches {
            let client = blob_client.clone();
            let blobs = blobs.clone();
            let done = fetch_done_tx.clone();
            let label = label.clone();
            tokio::spawn(async move {
                if let Err(e) = crate::blob_fetch::fetch_blob(
                    &client,
                    &blobs,
                    &digest,
                    crate::constants::MAX_MODULE_CODE_BYTES,
                    crate::constants::BLOB_FETCH_ATTEMPTS,
                )
                .await
                {
                    tracing::warn!(
                        target: "ducktape::modules",
                        node = %label,
                        digest = %crate::config::hex_bytes(&digest),
                        error = %e,
                        "pending-swap code fetch failed"
                    );
                }
                let _ = done.send(digest);
            });
        }
        for (key, msg) in actions.signals {
            let seq = *next_seq;
            *next_seq += 1;
            match node.submit(signer, seq, msg).await {
                Ok(_) => tracing::info!(
                    target: "ducktape::modules",
                    node = %label,
                    module = %key.0,
                    swap = key.1,
                    "code-ready signaled"
                ),
                Err(e) => {
                    // un-latch so a transient submit failure retries next tick.
                    code_signaller.unlatch(&key);
                    tracing::debug!(
                        target: "ducktape::modules",
                        node = %label,
                        error = %e,
                        "code readiness submit failed; retrying"
                    );
                }
            }
        }
    }

    // CAPABILITY ANNOUNCE: a current member whose discovered
    // provider set differs from the committed registry
    // self-submits ONE declarative `Announce`. member-gated (the
    // module rejects non-members) and idempotent (committed-read
    // + local latch). inert on a host with no executor CLIs, and
    // suppressed entirely under `announce_capabilities = false`
    // (the accept-lane-only provider: this node still executes
    // what it can, but only by claiming unassigned announcements
    // — it never enters a tag's rendezvous pool).
    async fn pump_capability_announce(&mut self) {
        let Self {
            node,
            orchestrator,
            next_seq,
            signer,
            label,
            announce_capabilities,
            announcer,
            ..
        } = self;
        if *announce_capabilities
            && orchestrator
                .current_members()
                .contains(&signer.public_key())
            && let Some(msg) = announcer.maybe_announce(node.host()).await
        {
            let seq = *next_seq;
            *next_seq += 1;
            match node.submit(signer, seq, msg).await {
                Ok(_) => tracing::info!(
                    target: "ducktape::modules",
                    node = %label,
                    capabilities = ?announcer.capabilities,
                    "capabilities announced"
                ),
                Err(e) => {
                    // un-latch so a transient submit failure retries.
                    announcer.announced = None;
                    tracing::debug!(
                        target: "ducktape::modules",
                        node = %label,
                        error = %e,
                        "capability announce submit failed; retrying"
                    );
                }
            }
        }
    }

    // SAGA CRANK (P7 liveness, host side): nothing else ever
    // submits `SagaMsg::Crank`, and under strict leases a
    // saga whose assignee went dark advances ONLY via a crank
    // (lease re-lease or deadline timeout). state-driven:
    // when the committed next expiry is at or past the latest
    // finalized height, push one permissionless crank —
    // throttled like the heartbeat, since a backlog wider
    // than CRANK_BUDGET legitimately needs several. duplicate
    // cranks from other nodes are deterministic no-ops.
    async fn pump_saga_crank(&mut self) {
        let Self {
            context,
            node,
            next_seq,
            signer,
            label,
            last_crank,
            ..
        } = self;
        let now = context.current();
        let crank_due = now.duration_since(*last_crank).unwrap_or_default() >= consensus::BLOCK_TIME;
        if crank_due
            && let Some(finalized_height) = node.finalized().map(|f| f.height)
            && let Some(expiry) = saga_next_expiry(node.host()).await
            && expiry <= finalized_height
        {
            *last_crank = now;
            let seq = *next_seq;
            *next_seq += 1;
            if let Err(e) = node
                .submit(
                    signer,
                    seq,
                    Msg {
                        target: "saga".into(),
                        payload: saga::encode_msg(&saga::SagaMsg::Crank {}),
                    },
                )
                .await
            {
                tracing::debug!(
                    target: "ducktape::saga",
                    node = %label,
                    error = %e,
                    "saga crank submit failed"
                );
            } else {
                tracing::debug!(
                    target: "ducktape::saga",
                    node = %label,
                    next_expiry = expiry,
                    finalized_height,
                    "saga crank submitted"
                );
            }
        }
    }

    // DISPATCH DELIVERY NUDGE (never-pop-stack liveness): a
    // result committed into the dispatch mailbox delivers via
    // the drain's DeliverPending injection in the NEXT
    // successful block — and heartbeat nops are rejected
    // frames that never apply, so a quiet chain would sit on
    // its mailbox. state-driven: while the committed mailbox
    // is non-empty, push one permissionless Nudge — a no-op
    // whose block carries the injection. duplicate nudges
    // from other nodes are free.
    async fn pump_dispatch_nudge(&mut self) {
        let Self {
            context,
            node,
            next_seq,
            signer,
            label,
            last_nudge,
            ..
        } = self;
        let now = context.current();
        let nudge_due = now.duration_since(*last_nudge).unwrap_or_default() >= consensus::BLOCK_TIME;
        if nudge_due && dispatch_pending_deliveries(node.host()).await > 0 {
            *last_nudge = now;
            let seq = *next_seq;
            *next_seq += 1;
            if let Err(e) = node
                .submit(
                    signer,
                    seq,
                    Msg {
                        target: "dispatch".into(),
                        payload: dispatch::encode_msg(&dispatch::DispatchMsg::Nudge {}),
                    },
                )
                .await
            {
                tracing::debug!(
                    target: "ducktape::saga",
                    node = %label,
                    error = %e,
                    "dispatch delivery nudge submit failed"
                );
            } else {
                tracing::debug!(
                    target: "ducktape::saga",
                    node = %label,
                    "dispatch delivery nudge submitted"
                );
            }
        }
    }
}

/// what one heartbeat tick does — the pure decision behind
/// [`ValidatorRuntime::pump_heartbeat`], separate so the two block cadences
/// stay unit-testable without a runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeartbeatAction {
    /// nothing due this tick.
    Idle,
    /// restart the beat grid and flush the window now: real ops are pending
    /// (the busy cadence — every drain tick), or the idle beat is due while a
    /// batch is still in flight (the flush itself is then a no-op).
    Flush,
    /// the idle beat is due with a clear orderer: submit one nop, then flush.
    BeatNop,
}

fn heartbeat_action(
    disabled: bool,
    ops_pending: bool,
    heartbeat_due: bool,
    orderer_idle: bool,
) -> HeartbeatAction {
    if disabled {
        return HeartbeatAction::Idle;
    }
    // the busy cadence: pending real ops flush on THIS tick — the block
    // interval floors at the drain tick, never waiting out `BLOCK_TIME`.
    if ops_pending {
        return HeartbeatAction::Flush;
    }
    if !heartbeat_due {
        return HeartbeatAction::Idle;
    }
    // the idle beat: one nop per `BLOCK_TIME`, and only while nothing is
    // already in flight — a nop behind a finalization stall would stack
    // idle blocks.
    if orderer_idle {
        HeartbeatAction::BeatNop
    } else {
        HeartbeatAction::Flush
    }
}

#[cfg(test)]
mod heartbeat_action_tests {
    use super::{HeartbeatAction, heartbeat_action};

    #[test]
    fn pending_ops_flush_on_this_tick_not_the_beat() {
        // the busy cadence: the beat interval has NOT elapsed, yet pending
        // real ops flush anyway — the block interval floors at the drain tick.
        assert_eq!(
            heartbeat_action(false, true, false, true),
            HeartbeatAction::Flush
        );
        assert_eq!(
            heartbeat_action(false, true, false, false),
            HeartbeatAction::Flush
        );
    }

    #[test]
    fn pending_ops_never_beat_a_nop() {
        // a window with real ops needs no nop — the ops ARE the block.
        assert_eq!(
            heartbeat_action(false, true, true, true),
            HeartbeatAction::Flush
        );
    }

    #[test]
    fn idle_beat_waits_for_block_time() {
        assert_eq!(
            heartbeat_action(false, false, false, true),
            HeartbeatAction::Idle
        );
        assert_eq!(
            heartbeat_action(false, false, true, true),
            HeartbeatAction::BeatNop
        );
    }

    #[test]
    fn idle_beat_never_stacks_behind_a_stall() {
        // a batch is still in flight: restart the grid without piling a nop
        // behind the stall.
        assert_eq!(
            heartbeat_action(false, false, true, false),
            HeartbeatAction::Flush
        );
    }

    #[test]
    fn disabled_heartbeat_flushes_nothing() {
        for ops_pending in [false, true] {
            for heartbeat_due in [false, true] {
                for orderer_idle in [false, true] {
                    assert_eq!(
                        heartbeat_action(true, ops_pending, heartbeat_due, orderer_idle),
                        HeartbeatAction::Idle
                    );
                }
            }
        }
    }
}
