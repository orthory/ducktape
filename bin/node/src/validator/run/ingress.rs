//! Local RPC and network/app ingress handlers.

use commonware_codec::DecodeExt as _;
use commonware_cryptography::{Signer as _, ed25519};
use commonware_runtime::{Clock as _, Metrics as _};

use sdk::Msg;

use super::{ValidatorRuntime, graceful_checkpoint};
use crate::config::{hex_bytes, unhex};
use crate::constants::{GATE_SETTLE_TIMEOUT, MODULE_IDS, SUBMIT_HOLD};
use crate::host_reads::{
    read_clients, read_redemptions_from_host, read_valset_members, read_valset_residents,
};
use crate::rpc::{
    JoinRequestRecord, JoinRequestView, JoinStateView, RpcJob, RpcReply, RpcRequest, RpcStatus,
};
use crate::util::{hex, unix_ms};
use crate::{config, lobby, relay, relay_runtime};

impl ValidatorRuntime<'_> {
    async fn refresh_operations(&self, exposition: &str) {
        let members = self.orchestrator.current_members();
        let me = self.signer.public_key();
        let reachable = reachable_validators(exposition, members, &me);
        self.metrics.update_consensus(
            self.orchestrator.epoch(),
            self.node.finalized_view().unwrap_or(0),
            members.len() as u64,
            reachable,
            (self.node.pending_batch_len() + self.node.orderer().pending_len()) as u64,
        );
        self.metrics.update_storage(
            self.prev_ckpt.0.unwrap_or_default(),
            self.index.is_poisoned(),
            MODULE_IDS.iter().map(|module| {
                (
                    (*module).to_string(),
                    self.index.applied_height(module).unwrap_or_default(),
                )
            }),
        );
    }

    /// one direct-peer sample: the exposition parse plus valset standing,
    /// stamped with this validator's own chain position.
    async fn peers_sample(&self) -> noded::peers::PeersView {
        let hex_set = |keys: Vec<Vec<u8>>| keys.iter().map(|k| hex_bytes(k)).collect();
        let validators = hex_set(read_valset_members(self.node.host()).await);
        let residents = hex_set(read_valset_residents(self.node.host()).await);
        let height = self.node.finalized().map(|f| f.height).unwrap_or(0);
        let epoch = Some(self.orchestrator.epoch());
        noded::peers::peers_from_exposition(&self.context.encode(), unix_ms(), height, epoch)
            .with_roles(&validators, &residents)
    }

    pub(super) async fn on_rpc(&mut self, (req, reply): RpcJob) {
        let Self {
            node,
            orchestrator,
            next_seq,
            signer,
            label,
            join_requests,
            ..
        } = self;

        let resp = match req {
            RpcRequest::Submit {
                target,
                payload_hex,
            } => match unhex(&payload_hex) {
                Ok(payload) => {
                    let seq = *next_seq;
                    *next_seq += 1;
                    match node.submit(signer, seq, Msg { target, payload }).await {
                        Ok(_) => RpcReply::ok(),
                        Err(e) => RpcReply::err(format!("submit failed: {e}")),
                    }
                }
                Err(e) => RpcReply::err(format!("bad payload_hex: {e}")),
            },
            RpcRequest::Query { target, req_hex } => match unhex(&req_hex) {
                Ok(req_bytes) => match node.host().query(&target, &req_bytes).await {
                    Ok(bytes) => RpcReply {
                        reply_hex: Some(hex_bytes(&bytes)),
                        ..RpcReply::ok()
                    },
                    Err(e) => RpcReply::err(format!("query failed: {e}")),
                },
                Err(e) => RpcReply::err(format!("bad req_hex: {e}")),
            },
            RpcRequest::Status => {
                let mut modules = std::collections::BTreeMap::new();
                for &m in MODULE_IDS {
                    if let Some(root) = node.host().module_root(m) {
                        modules.insert(m.to_string(), hex(&root));
                    }
                }
                RpcReply {
                    status: Some(RpcStatus {
                        height: node.finalized().map(|f| f.height),
                        app_hash: hex(&node.app_hash()),
                        modules,
                    }),
                    ..RpcReply::ok()
                }
            }
            RpcRequest::JoinRequests => {
                // read-time hygiene: an approved joiner holds
                // STANDING now (resident or already validator) —
                // its request is settled, drop it.
                let members = read_valset_members(node.host()).await;
                let residents_now = read_valset_residents(node.host()).await;
                join_requests.retain(|joiner, _| {
                    !members.contains(joiner) && !residents_now.contains(joiner)
                });
                let views = join_requests
                    .iter()
                    .map(|(joiner, r)| JoinRequestView {
                        joiner: hex_bytes(joiner),
                        issuer: hex_bytes(&r.issuer),
                        first_seen_ms: r.first_seen_ms,
                        last_seen_ms: r.last_seen_ms,
                    })
                    .collect();
                RpcReply {
                    join_requests: Some(views),
                    ..RpcReply::ok()
                }
            }
            RpcRequest::JoinState => {
                // a validator is a full member — the terminal join state. the
                // node-owned source (ADR §6): no log-marker parsing.
                RpcReply {
                    join_state: Some(JoinStateView {
                        phase: "promoted".into(),
                        detail: "validator".into(),
                        height: node.finalized().map(|f| f.height),
                    }),
                    ..RpcReply::ok()
                }
            }
            RpcRequest::Peers => RpcReply {
                peers: Some(self.peers_sample().await),
                ..RpcReply::ok()
            },
            RpcRequest::Shutdown => {
                // best-effort final checkpoint + journal barrier so
                // the restart replays a minimal suffix; a failure
                // here is just the crash path, which also recovers.
                // SAME sequence as the signal arm (shared macro).
                graceful_checkpoint(node, orchestrator, *next_seq).await;
                let _ = reply.send(RpcReply::ok());
                tracing::info!(
                    target: "ducktape::node",
                    node = %label,
                    "shutdown requested via rpc; exiting"
                );
                std::process::exit(0);
            }
        };
        let _ = reply.send(resp);
    }

    pub(super) async fn on_oracle_result(&mut self, msg: Msg) {
        let Self {
            node,
            next_seq,
            signer,
            label,
            ..
        } = self;

        // a completed off-loop provider run: its OracleResult op
        // re-enters the ordered lane as an ordinary signed
        // submit — the oracle-as-op, unchanged; only WHERE the
        // provider ran moved.
        let seq = *next_seq;
        *next_seq += 1;
        if let Err(e) = node.submit(signer, seq, msg).await {
            tracing::warn!(
                target: "ducktape::saga",
                node = %label,
                error = %e,
                reason = "oracle_result_submit_failed",
                "oracle result dropped"
            );
        }
    }

    /// the join gate (join ADR §4), arriving over the WireGuard-tunnel
    /// doorbell: the reachability plane already OPENED and VERIFIED the sealed
    /// intro (V1–V5 crypto, V4 expiry, V8 role) and installed the tunnel —
    /// what reaches this loop is a verified request. this runs the
    /// COMMITTED-STATE checks (V6/V7/V9), then — on pass — submits `Redeem`
    /// and HOLDS the joiner's outcome against that frame until the drain
    /// reports its consensus fate (`pending_gates`, resolved in `on_drain`
    /// into `gate_outcomes`, where the doorbell answers the joiner's next
    /// retransmit). the outcome is authoritative: `Admitted` means standing
    /// is COMMITTED, `Rejected{terminal}` means stop.
    pub(super) async fn on_gate_forward(&mut self, fwd: lobby::GateForward) {
        let Self {
            context,
            node,
            gate_outcomes,
            next_seq,
            signer,
            label,
            validators,
            coordination,
            join_requests,
            gating,
            pending_gates,
            ..
        } = self;

        let joiner_bytes = fwd.joiner.clone();
        let issuer_bytes = fwd.issuer.clone();
        // V6: nonce unspent in committed redemptions. a nonce redeemed by
        // ANOTHER key can never redeem again — terminal Spent. (redeemed by
        // the SAME key = this joiner already has standing; V9 handles it.)
        let redemptions = read_redemptions_from_host(node.host()).await;
        if let Some(spent) = redemptions
            .iter()
            .find(|r| r.nonce == fwd.nonce && r.joiner != joiner_bytes)
        {
            tracing::warn!(
                target: "ducktape::join",
                node = %label,
                peer = %hex_bytes(&joiner_bytes[..4.min(joiner_bytes.len())]),
                spent_by = %hex_bytes(&spent.joiner[..4.min(spent.joiner.len())]),
                height = spent.height,
                reason = "invite_already_redeemed",
                "gate: peer presented an ALREADY-REDEEMED invite; refusing permanently"
            );
            super::settle_gate(
                gate_outcomes,
                joiner_bytes,
                lobby::IntroReply::Rejected {
                    code: lobby::RejectCode::Spent,
                    detail: "invite already redeemed — an invite admits exactly one person; \
                             ask the inviter for a fresh invite"
                        .into(),
                    terminal: true,
                },
            );
            return;
        }
        let members = read_valset_members(node.host()).await;
        let residents_now = read_valset_residents(node.host()).await;
        // V7: issuer in committed valset. NON-TERMINAL — this member's local
        // view cannot distinguish a REMOVED issuer (invite dead) from a
        // just-admitted one it has not applied yet; a terminal answer here
        // would let one lagging validator kill a healthy join. the joiner
        // fails over to another member (§3.1 V7, PR #538 ruling).
        if !members.contains(&issuer_bytes) {
            super::settle_gate(
                gate_outcomes,
                joiner_bytes,
                lobby::IntroReply::Rejected {
                    code: lobby::RejectCode::IssuerUnknown,
                    detail: "the inviting member is not in this member's current view — if it \
                             was removed, this invite is dead (ask a current member for a fresh \
                             one); if it was just admitted, another member will redeem shortly"
                        .into(),
                    terminal: false,
                },
            );
            return;
        }
        // V9: already holding standing (validator OR resident) → idempotent
        // SUCCESS. a re-gated joiner is not an error — answer Admitted at the
        // current committed height.
        if members.contains(&joiner_bytes) || residents_now.contains(&joiner_bytes) {
            let height = node.finalized().map(|f| f.height).unwrap_or(0);
            super::settle_gate(
                gate_outcomes,
                joiner_bytes,
                lobby::IntroReply::Admitted { height, cap: None },
            );
            return;
        }

        // V6/V7/V9 pass. ONE in-flight gate per joiner key (§3.2): a duplicate
        // forward while settling (the joiner's retransmit cadence outpacing
        // consensus) re-arms nothing — no double-submit (the nonce set would
        // collapse racing submits to one grant anyway, but a second
        // pending_gate could mis-map the committed grant's sibling reject onto
        // this joiner, so dedup is load-bearing).
        if gating.contains_key(&joiner_bytes) {
            return;
        }

        // MINT the coordinator capability for the joiner (private coordination
        // only, and only a GENESIS validator's cap is trusted by the
        // coordinator). additive, side-effect-free (a pure ed25519 sign). the
        // cap cannot ride the invite (the joiner's key did not exist at
        // invite-mint time), so the sealed `Admitted` ack is its only delivery
        // channel. delivered when the gate settles.
        let minted_cap = if *coordination == config::Coordination::Private
            && validators.contains(&signer.public_key())
        {
            let Ok(subj) = <[u8; 32]>::try_from(joiner_bytes.as_slice()) else {
                // `verify_intro` decoded this key upstream — a non-32-byte
                // joiner cannot reach the loop; refuse rather than panic.
                return;
            };
            let cap = nat_traversal::mint_coord_cap(
                signer,
                nat_traversal::NodeKey(subj),
                nat_traversal::now_secs() + nat_traversal::COORD_CAP_TTL_SECS,
            );
            Some(config::pack_coord_cap(&cap))
        } else {
            None
        };

        // SETTLE-THEN-ANSWER (§3.2): submit the Redeem and hold the joiner's
        // outcome against the frame id. `submit` returns the FrameId; the drain
        // reports its consensus fate on `pending_gates` (Applied → Admitted,
        // Rejected → mapped code, timeout → Busy) — this handler never blocks.
        let lobby::GateForward {
            issuer,
            nonce,
            token_sig,
            joiner,
            proof,
            role,
            expires_unix_secs,
        } = fwd;
        let redeem = governance::GovMsg::Redeem {
            issuer,
            nonce,
            token_sig,
            joiner,
            proof,
            role,
            expires_unix_secs,
        };
        let seq = *next_seq;
        *next_seq += 1;
        match node
            .submit(
                signer,
                seq,
                Msg {
                    target: "governance".into(),
                    payload: governance::encode_msg(&redeem),
                },
            )
            .await
        {
            Ok(frame_id) => {
                tracing::info!(
                    target: "ducktape::join",
                    node = %label,
                    peer = %hex_bytes(&joiner_bytes[..4.min(joiner_bytes.len())]),
                    issuer = %hex_bytes(&issuer_bytes[..4.min(issuer_bytes.len())]),
                    frame = %hex_bytes(&frame_id),
                    "gate: redemption submitted for {}; awaiting consensus before answering \
                     Admitted",
                    hex_bytes(&joiner_bytes[..4.min(joiner_bytes.len())])
                );
                let now = unix_ms();
                join_requests
                    .entry(joiner_bytes.clone())
                    .or_insert(JoinRequestRecord {
                        issuer: issuer_bytes,
                        first_seen_ms: now,
                        last_seen_ms: now,
                    });
                gating.insert(joiner_bytes.clone(), frame_id);
                pending_gates.insert(
                    frame_id,
                    super::GatePending {
                        joiner: joiner_bytes,
                        cap: minted_cap,
                        deadline: context.current() + GATE_SETTLE_TIMEOUT,
                    },
                );
            }
            Err(e) => {
                // submit failure is transient (§3.2): the joiner tries another
                // member rather than exiting.
                super::settle_gate(
                    gate_outcomes,
                    joiner_bytes,
                    lobby::IntroReply::Rejected {
                        code: lobby::RejectCode::Busy,
                        detail: format!("could not submit redemption: {e}"),
                        terminal: false,
                    },
                );
            }
        }
    }

    pub(super) async fn on_relay(&mut self, peer: ed25519::PublicKey, bytes: Vec<u8>) {
        let Self {
            context,
            node,
            relay_tx,
            pending_submits,
            pending_relays,
            validator_relay,
            ..
        } = self;
        let now = context.current();

        let Ok(msg) = relay::decode_msg(&bytes) else {
            return;
        };
        let needs_standing = matches!(
            msg,
            relay::RelayMsg::BlobOffer { .. } | relay::RelayMsg::Submit { .. }
        );
        let (members_now, residents_now, clients_now) = if needs_standing {
            (
                read_valset_members(node.host()).await,
                read_valset_residents(node.host()).await,
                read_clients(node.host()).await,
            )
        } else {
            (Vec::new(), Vec::new(), Vec::new())
        };
        let Some(action) = validator_relay.on_message(
            now,
            peer,
            msg,
            &members_now,
            &residents_now,
            &clients_now,
            relay_tx,
        ) else {
            return;
        };
        match action {
            relay_runtime::ValidatorAction::SubmitResident {
                frame_id,
                frame,
                peer,
            } => match node.submit_frame(frame).await {
                Ok(id) => {
                    debug_assert_eq!(id, frame_id);
                    pending_relays.insert(id, (peer, now + SUBMIT_HOLD));
                }
                Err(e) => relay_runtime::send_reply(
                    relay_tx,
                    &peer,
                    frame_id,
                    relay::RelayOutcome::Refused {
                        detail: format!("submit failed: {e}"),
                    },
                ),
            },
            relay_runtime::ValidatorAction::SubmitLocal {
                frame_id,
                frame,
                reply,
                deadline,
            } => match node.submit_frame(frame).await {
                Ok(id) => {
                    debug_assert_eq!(id, frame_id);
                    pending_submits.insert(id, (reply, deadline));
                }
                Err(e) => {
                    let _ = reply.send(Err(format!("submit failed: {e}")));
                }
            },
        }
    }

    /// take custody of an already-framed op on THIS validator: fan a forge pack
    /// out to the peers that need it, then pin + propose the frame and hold the
    /// caller's reply against the frame id until the drain answers it.
    ///
    /// the frame arrives SIGNED and is submitted verbatim — nothing here looks
    /// at, replaces, or re-derives its origin. that is what makes it usable by
    /// both callers: the frameless lane, whose frame this node just signed with
    /// its own key, and the signed-frame lane, whose frame some OTHER key signed
    /// (an agent's per-run session key). a re-sign here would silently convert
    /// the second into the first — the exact defect the frame lane closes.
    async fn submit_local_frame(
        &mut self,
        frame: Vec<u8>,
        reply: futures::channel::oneshot::Sender<Result<noded::BlockSummary, String>>,
    ) {
        let Self {
            context,
            node,
            relay_tx,
            signer,
            pending_submits,
            validator_relay,
            ..
        } = self;
        let now = context.current();

        let peers: Vec<ed25519::PublicKey> = if relay::required_blob_digest(&frame).is_some() {
            read_valset_members(node.host())
                .await
                .iter()
                .filter_map(|raw| ed25519::PublicKey::decode(raw.as_slice()).ok())
                .filter(|key| key != &signer.public_key())
                .collect()
        } else {
            Vec::new()
        };
        match validator_relay.prepare_local(now, frame, reply, peers, relay_tx) {
            Ok(Some(relay_runtime::ValidatorAction::SubmitLocal {
                frame_id,
                frame,
                reply,
                deadline,
            })) => match node.submit_frame(frame).await {
                Ok(id) => {
                    debug_assert_eq!(id, frame_id);
                    pending_submits.insert(id, (reply, deadline));
                }
                Err(e) => {
                    let _ = reply.send(Err(format!("submit failed: {e}")));
                }
            },
            Ok(Some(relay_runtime::ValidatorAction::SubmitResident { .. })) => {
                unreachable!("local preparation returns a local action")
            }
            Ok(None) => {}
            Err((reply, detail)) => {
                let _ = reply.send(Err(detail));
            }
        }
    }

    pub(super) async fn on_http(&mut self, cmd: noded::NodeCommand) {
        match cmd {
            // `origin` is the caller's CLAIMED submitter identity —
            // meaningful on the embedded daemon, but this lane signs
            // frames, and the signed origin IS this node's pubkey
            // (authenticated authorship that governance relies on).
            // a claimed origin cannot ride a signed frame without
            // making authorship forgeable, so it is ignored here;
            // display names resolve via the name registry instead. a
            // caller that needs to be its OWN author signs a frame and
            // takes the SubmitFrame arm below.
            noded::NodeCommand::Submit {
                target,
                payload,
                origin: _,
                reply,
            } => {
                let seq = self.next_seq;
                self.next_seq += 1;
                let frame = node::encode_frame(&self.signer, seq, &Msg { target, payload }, None);
                self.submit_local_frame(frame, reply).await;
            }
            // an ALREADY-SIGNED frame: submitted VERBATIM, never re-signed and
            // never re-originated. `OrderedNode::submit_frame` verifies the
            // signature before anything is pinned, so a forged or tampered frame
            // is a rejection here, not junk in the store — and the origin the
            // block carries is the frame's own proven signer, which is the only
            // authorship a module can trust.
            noded::NodeCommand::SubmitFrame { frame, reply } => {
                self.submit_local_frame(frame, reply).await;
            }
            noded::NodeCommand::Query { target, req, reply } => {
                let result = self
                    .node
                    .host()
                    .query(&target, &req)
                    .await
                    .map_err(|e| e.to_string());
                let _ = reply.send(result);
            }
            noded::NodeCommand::Status { reply } => {
                let exposition = self.context.encode();
                self.refresh_operations(&exposition).await;
                let modules = MODULE_IDS
                    .iter()
                    .map(|m| noded::ModuleStatus {
                        id: (*m).into(),
                        root: self
                            .node
                            .host()
                            .module_root(m)
                            .map(|r| hex(&r))
                            .unwrap_or_default(),
                        category: noded::ModuleCategory::of(m),
                    })
                    .collect();
                let _ = reply.send(noded::NodeStatus {
                    version: env!("CARGO_PKG_VERSION").into(),
                    app_hash: hex(&self.node.app_hash()),
                    height: self.node.finalized().map(|f| f.height).unwrap_or(0),
                    modules,
                    public_key: self.status_public_key.clone(),
                    operations: self.metrics.operational_status(),
                });
            }
            noded::NodeCommand::Peers { reply } => {
                let _ = reply.send(self.peers_sample().await);
            }
            noded::NodeCommand::Metrics { reply } => {
                // one registry: commonware's runtime series plus the
                // `ducktape_*` block series the drain loop records.
                let exposition = self.context.encode();
                self.refresh_operations(&exposition).await;
                let _ = reply.send(self.context.encode());
            }
        }
    }
}

/// Commonware owns the detailed peer series. This bounded adapter counts only
/// current validators and includes self, insulating the stable Ducktape facade
/// from dashboard knowledge of dependency-specific metric names.
fn reachable_validators(
    exposition: &str,
    validators: &std::collections::BTreeSet<ed25519::PublicKey>,
    me: &ed25519::PublicKey,
) -> u64 {
    // ponytail: O(validators × exposition lines); replace with a connection
    // snapshot API if validator sets become large enough for scrape cost to matter.
    validators
        .iter()
        .filter(|validator| {
            if *validator == me {
                return true;
            }
            let prefix = format!(
                "network_tracker_directory_connected{{peer=\"{}\"}} ",
                validator
            );
            exposition.lines().any(|line| {
                line.strip_prefix(&prefix)
                    .and_then(|value| value.split_whitespace().next())
                    .and_then(|value| value.parse::<f64>().ok())
                    .is_some_and(|value| value > 0.0)
            })
        })
        .count() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_reachable_count_includes_self_and_connected_members_only() {
        let me = ed25519::PrivateKey::from_seed(1).public_key();
        let connected = ed25519::PrivateKey::from_seed(2).public_key();
        let disconnected = ed25519::PrivateKey::from_seed(3).public_key();
        let outsider = ed25519::PrivateKey::from_seed(4).public_key();
        let validators = [me.clone(), connected.clone(), disconnected]
            .into_iter()
            .collect();
        let exposition = format!(
            "network_tracker_directory_connected{{peer=\"{connected}\"}} 1720000000\n\
             network_tracker_directory_connected{{peer=\"{outsider}\"}} 1720000000\n"
        );

        assert_eq!(reachable_validators(&exposition, &validators, &me), 2);
    }
}
