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
use crate::drain_actions::{BlockAction, CutoverTrigger, EpochActions, block_actions};
use crate::host_reads::{
    read_upgrade_state, read_upgrade_status_raw, read_upgrade_version_fields, read_valset_members,
    read_valset_residents,
};
use crate::{lobby, relay};
use crate::util::{hex, participant_bytes, resident_bytes};

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
            reach_cmd,
            lobby_tx,
            relay_tx,
            next_seq,
            prev_ckpt,
            signer,
            label,
            peers,
            checkpoint_blocks,
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
                eprintln!("[node {label}] FATAL: {e} — halting");
                std::process::exit(1);
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
            eprintln!("[node {label}] tip-seal sync failed: {e}");
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
        for action in block_actions(&drained, node.take_system_dispatches(), blobs) {
            let BlockAction {
                height,
                dispatches,
                record,
                applied,
                latency_us,
                op_count,
                ..
            } = action;
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
            metrics.record_ops(op_count);
            // this lane's agreed clock IS the height: the drain stamps
            // BlockContext { consensus_time: height } for every block.
            let ops = indexer::BlockOps {
                record,
                ..noded::index_block_ops(height, height, &dispatches)
            };
            if let Err(err) = index.apply_block(&ops) {
                eprintln!(
                    "[node {label}] module index apply failed at height {height}: {err} \
                             — wipe <storage>/index to rebuild"
                );
            }
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
            // resolve a HELD JOIN GATE (ADR §3.2): the joiner's Admitted/
            // Rejected reply was held against this Redeem frame — now the drain
            // knows its consensus fate. Applied ⇒ the AUTHORITATIVE Admitted at
            // the committed height (carrying the coord cap); Rejected ⇒ map the
            // module reason to a code + terminal bit (chiefly a spent-nonce
            // race the pre-filter missed). a gate frame is EXCLUSIVELY a gate,
            // so resolve and move on.
            if let Some(gate) = pending_gates.remove(&d.id) {
                gating.remove(&gate.joiner);
                let reply = match d.disposition {
                    node::Disposition::Applied => lobby::GateMsg::Admitted {
                        height: d.height,
                        cap: gate.cap,
                    },
                    node::Disposition::Rejected => {
                        let (code, terminal) = lobby::redeem_reject_outcome(d.reason.as_deref());
                        lobby::GateMsg::Rejected {
                            code,
                            detail: d.reason.clone().unwrap_or_else(|| {
                                "invite redemption rejected in consensus".into()
                            }),
                            terminal,
                        }
                    }
                    node::Disposition::Discarded => unreachable!("filtered at the loop top"),
                };
                let _ = lobby_tx.send(
                    Recipients::One(gate.peer),
                    IoBuf::from(lobby::encode_msg(&reply)),
                    false,
                );
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
        validator_relay.expire(std::time::Instant::now(), relay_tx);
        // expire holds the mesh never finalized in time. the op may
        // still land later — clients re-query on block events.
        if !pending_submits.is_empty() {
            let now = std::time::Instant::now();
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
            let now = std::time::Instant::now();
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
        // held join gates that never settled within GATE_SETTLE_TIMEOUT: answer
        // Busy (NON-terminal, §3.2) so the joiner fails over to another member
        // rather than exiting. the Redeem may still land later — a re-Request
        // then hits the V9 idempotent Admitted.
        if !pending_gates.is_empty() {
            let now = std::time::Instant::now();
            let expired: Vec<node::FrameId> = pending_gates
                .iter()
                .filter(|(_, g)| g.deadline <= now)
                .map(|(k, _)| *k)
                .collect();
            for k in expired {
                if let Some(gate) = pending_gates.remove(&k) {
                    gating.remove(&gate.joiner);
                    let msg = lobby::GateMsg::Rejected {
                        code: lobby::RejectCode::Busy,
                        detail: "the gate could not settle in time — trying another member".into(),
                        terminal: false,
                    };
                    let _ = lobby_tx.send(
                        Recipients::One(gate.peer),
                        IoBuf::from(lobby::encode_msg(&msg)),
                        false,
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
                    Err(e) => eprintln!("[node {label}] floor cert write failed (will retry): {e}"),
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
            let (cv, pu) = read_upgrade_version_fields(node.host()).await;
            let captured = Manifest::capture(
                node.host(),
                Some(f.height),
                orchestrator.epoch(),
                orchestrator.epoch_base(),
                participant_bytes(orchestrator),
                resident_bytes(orchestrator),
                orchestrator.pending_cutover().map(|c| c.cutover_view()),
                cv,
                pu,
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
                        if floor_passed
                            && let Err(e) = node.sink_mut().prune_oplog(prev_ckpt.1).await
                        {
                            eprintln!("[node {label}] oplog prune failed: {e}");
                        }
                        *prev_ckpt = (m.height, pos);
                    }
                    Err(e) => eprintln!("[node {label}] checkpoint write failed (will retry): {e}"),
                },
                Err(e) => eprintln!("[node {label}] checkpoint capture failed (will retry): {e}"),
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
                println!(
                    "[node {label}] membership change observed at view {} — cutover to epoch {} at view {}",
                    cutover.observed_view(),
                    cutover.next_epoch(),
                    cutover.cutover_view()
                );
                node.set_view_ceiling(cutover.cutover_view());
            }
            // a pending upgrade arms the SAME single cutover slot at its
            // activation height (design §"One boundary carries both
            // concerns") — never a competing arm: when a membership
            // cutover already holds the slot `observe_upgrade` returns
            // Pending and the version flip rides that boundary via the
            // boundary read in `respawn_if_due`. inert until the module is
            // registered (`read_upgrade_state` returns baseline/no-pending).
            let boundary_upgrade = read_upgrade_state(node.host()).await;
            if let Some(CutoverTrigger::Upgrade {
                cutover,
                name,
                activation_height,
            }) = actions.observe_upgrade(&boundary_upgrade)
            {
                println!(
                    "[node {label}] upgrade '{name}' armed — cutover to epoch {} at view {} (activation height {activation_height})",
                    cutover.next_epoch(),
                    cutover.cutover_view()
                );
                node.set_view_ceiling(cutover.cutover_view());
            }
            if let Some(plan) = actions.respawn(boundary_upgrade) {
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
                    println!(
                        "[node {label}] demoted from the validator set at epoch {} — halting (restart to serve as sync/resident)",
                        plan.epoch()
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
                    Ok(carried) if carried > 0 => println!(
                        "[node {label}] carried {carried} accepted ops across the cutover into epoch {}",
                        plan.epoch()
                    ),
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("[node {label}] FATAL: {e} — halting");
                        std::process::exit(1);
                    }
                }
                // ACTIVATION (design §4): realize the agreed boundary
                // protocol version into every dual-path module's
                // active_version (branch selector) at H. driven ONLY by
                // the agreed `plan.boundary_version()` — deterministic,
                // non-hashed. the upgrade module's OWN committed
                // reconciliation (current_version flip + pending clear on
                // ARM, clear-only on ABORT) is NOT done here: it rides the
                // single in-block System `Advance` the host drain injects
                // at the same finalized view (Task 6.3), so both concerns
                // land at ONE boundary and every node agrees. do NOT branch
                // a separate abort-only follow-up — the one Advance owns both.
                node.host_mut().set_active_version(plan.boundary_version());
                match plan.upgrade_verdict() {
                    consensus::UpgradeVerdict::Armed { name, to_version } => println!(
                        "[node {label}] upgrade activated name={name} version={to_version} at height {}",
                        plan.cutover_app_height()
                    ),
                    consensus::UpgradeVerdict::Abort { name } => println!(
                        "[node {label}] upgrade aborted name={name} (unmet readiness) at height {} — network continues on version {}",
                        plan.cutover_app_height(),
                        plan.boundary_version()
                    ),
                    consensus::UpgradeVerdict::None => {}
                }
                // checkpoint IMMEDIATELY: the manifest must record
                // the new epoch's participant set (the journal's
                // cutover record alone covers only the crash
                // window until this write lands).
                let pos = node.sink_mut().oplog_pos().await;
                // post-boundary committed version fields: after an armed
                // Advance the module holds `current_version = to_version`
                // + no pending, so this checkpoint stamps the new baseline.
                let (cv, pu) = read_upgrade_version_fields(node.host()).await;
                let captured = Manifest::capture(
                    node.host(),
                    node.finalized().map(|f| f.height),
                    orchestrator.epoch(),
                    orchestrator.epoch_base(),
                    participant_bytes(orchestrator),
                    resident_bytes(orchestrator),
                    None,
                    cv,
                    pu,
                    pos,
                    *next_seq,
                );
                match captured {
                    Ok(m) => match node.sink_mut().write_manifest(&m).await {
                        Ok(()) => {
                            *blocks_since_checkpoint = 0;
                            *prev_ckpt = (m.height, pos);
                        }
                        Err(e) => eprintln!(
                            "[node {label}] post-cutover checkpoint write failed \
                                     (the journal's cutover record covers a restart): {e}"
                        ),
                    },
                    Err(e) => eprintln!(
                        "[node {label}] post-cutover checkpoint capture failed \
                                 (the journal's cutover record covers a restart): {e}"
                    ),
                }
                println!(
                    "[node {label}] cutover complete: epoch {} with {} validators (app height base {})",
                    plan.epoch(),
                    members.len(),
                    plan.cutover_app_height()
                );
            }
        }

        // the state-driven pumps, each its own method below: block
        // cadence/heartbeat, upgrade readiness, capability announce,
        // saga crank, dispatch delivery nudge.
        self.pump_heartbeat().await;
        self.pump_readiness_signal().await;
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
            upgrade_armed_latch,
            upgrade_pending_seen,
            ..
        } = self;
        let dev_demo = *dev_demo;
        let expected = *expected;

        // UPGRADE TRANSITION MARKERS (one-shot, committed-state driven):
        // the greppable proof surface the e2e keys on. `armed` is the
        // module's own R==n verdict (pending set, boundary non-empty,
        // every current member signaled), so this fires exactly when
        // readiness first reaches the full set — before H is crossed.
        if let Some(st) = read_upgrade_status_raw(node.host()).await {
            match &st.pending {
                Some(up) => {
                    *upgrade_pending_seen = Some(up.name.clone());
                    let key = (up.name.clone(), up.to_version);
                    if st.armed && upgrade_armed_latch.as_ref() != Some(&key) {
                        println!(
                            "[node {label}] upgrade armed name={} to_version={} height={}",
                            up.name, up.to_version, up.activation_height
                        );
                        *upgrade_armed_latch = Some(key);
                    }
                }
                None => {
                    if let Some(name) = upgrade_pending_seen.take() {
                        // the boundary Advance reconciled the pending
                        // (ARM flip or ABORT clear) — the slot is free.
                        println!("[node {label}] upgrade cleared name={name}");
                        *upgrade_armed_latch = None;
                    }
                }
            }
        }

        // the reactor seam: offer each finalized block's effects to
        // the host-owned workers; a claiming worker's follow-up op
        // re-enters through the ordered lane as its own block (the
        // oracle-as-op). unclaimed effects are logged, not silently
        // dropped — a saga stuck Pending should be visible.
        for eff in node.take_effects() {
            let mut claimed = false;
            for w in workers.iter() {
                match w.run(&eff).await {
                    Ok(host::worker::WorkOutcome::Handled(Some(follow))) => {
                        let seq = *next_seq;
                        *next_seq += 1;
                        if let Err(e) = node.submit(signer, seq, follow).await {
                            eprintln!("[node {label}] worker follow-up submit failed: {e}");
                        }
                        claimed = true;
                        break;
                    }
                    // a deliberate skip (e.g. leased to another
                    // node): claimed, nothing to submit.
                    Ok(host::worker::WorkOutcome::Handled(None)) => {
                        claimed = true;
                        break;
                    }
                    Ok(host::worker::WorkOutcome::NotMine) => {}
                    Err(e) => {
                        eprintln!("[node {label}] worker error: {e}");
                        claimed = true; // errored ≠ unclaimed; don't double-log
                        break;
                    }
                }
            }
            if !claimed {
                println!(
                    "[node {label}] effect with no worker ({} bytes) — dropped",
                    eff.0.len()
                );
            }
        }
        if dev_demo && !*converged && *applied >= expected {
            let h = node.app_hash();
            println!("[node {label}] converged app_hash={}", hex(&h));
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
                    println!("[node {label}]   directory k{k}={v:?}");
                }
            }
            *converged = true;
        }
    }

    // BLOCK CADENCE + heartbeat, unified. `submit`/`submit_frame`
    // now ENQUEUE into the node's `pending_batch`; this is the one
    // place per block-time that FLUSHES the window — packing every
    // frame that arrived in it (real ops and/or an idle nop) into
    // ONE batch super-frame and proposing it as a single block.
    // that is the aggregation: at most one block per BLOCK_TIME,
    // carrying all the window's txs, never 1-tx-1-block.
    //
    // the idle nop still exists: finalized views only advance with
    // a proposed frame, so an idle network would freeze (its height
    // never ticks and a pending cutover, which crosses only when
    // finalized views REACH it, would park forever). so on an EMPTY
    // window inject one deterministically-rejected nop (unknown
    // module target: rejects identically everywhere, leaves no
    // state) and flush that. a window with real ops needs no nop —
    // the ops ARE the block.
    //
    // GATE the idle nop on an empty orderer FIFO too: a nop pushed
    // while a batch still awaits finalization only piles behind a
    // finalization stall (a flapping quorum peer would stack idle
    // blocks). real ops are never gated — they must not wait.
    async fn pump_heartbeat(&mut self) {
        let Self {
            node,
            next_seq,
            signer,
            label,
            last_flush,
            heartbeat_disabled,
            ..
        } = self;
        if !*heartbeat_disabled && last_flush.elapsed() >= consensus::BLOCK_TIME {
            *last_flush = std::time::Instant::now();
            if node.pending_batch_len() == 0 && node.orderer().pending_len() == 0 {
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
                    eprintln!("[node {label}] heartbeat nop submit failed: {e}");
                }
            }
            // flush the window: no-op when `pending_batch` is empty
            // (idle with a batch already in flight — wait for it).
            if let Err(e) = node.flush_batch().await {
                eprintln!("[node {label}] batch flush failed: {e}");
            }
        }
    }

    // READINESS SIGNAL (design §3 / plan Task 7.1): a current
    // boundary member whose binary can execute the pending upgrade
    // self-submits ONE `SignalReady`. gated to a current member (the
    // R = n readiness denominator); the signaller's own committed
    // read + local latch keep it idempotent. inert on a baseline net.
    async fn pump_readiness_signal(&mut self) {
        let Self {
            node,
            orchestrator,
            next_seq,
            signer,
            label,
            signaller,
            ..
        } = self;
        if orchestrator
            .current_members()
            .contains(&signer.public_key())
            && let Some((msg, name, to_version)) = signaller.maybe_signal(node.host()).await
        {
            let seq = *next_seq;
            *next_seq += 1;
            match node.submit(signer, seq, msg).await {
                Ok(_) => {
                    println!("[node {label}] signaled ready name={name} to_version={to_version}")
                }
                Err(e) => {
                    // un-latch so a transient submit failure retries on
                    // the next tick (the module stays idempotent).
                    signaller.signaled = None;
                    eprintln!("[node {label}] readiness signal submit failed: {e}");
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
                Ok(_) => println!(
                    "[node {label}] announced capabilities {:?}",
                    announcer.capabilities
                ),
                Err(e) => {
                    // un-latch so a transient submit failure retries.
                    announcer.announced = None;
                    eprintln!("[node {label}] capability announce submit failed: {e}");
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
            node,
            next_seq,
            signer,
            label,
            last_crank,
            ..
        } = self;
        if last_crank.elapsed() >= consensus::BLOCK_TIME
            && let Some(finalized_height) = node.finalized().map(|f| f.height)
            && let Some(expiry) = saga_next_expiry(node.host()).await
            && expiry <= finalized_height
        {
            *last_crank = std::time::Instant::now();
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
                eprintln!("[node {label}] saga crank submit failed: {e}");
            } else {
                println!(
                    "[node {label}] saga crank submitted \
                             (next expiry {expiry} <= height {finalized_height})"
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
            node,
            next_seq,
            signer,
            label,
            last_nudge,
            ..
        } = self;
        if last_nudge.elapsed() >= consensus::BLOCK_TIME
            && dispatch_pending_deliveries(node.host()).await > 0
        {
            *last_nudge = std::time::Instant::now();
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
                eprintln!("[node {label}] dispatch nudge submit failed: {e}");
            } else {
                println!("[node {label}] dispatch delivery nudge submitted");
            }
        }
    }
}
