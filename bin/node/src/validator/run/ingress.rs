//! Local RPC and network/app ingress handlers.

use commonware_codec::DecodeExt as _;
use commonware_cryptography::{Signer as _, ed25519};
use commonware_p2p::{Recipients, Sender as _};
use commonware_runtime::{IoBuf, Metrics as _};

use sdk::Msg;

use super::{ValidatorRuntime, graceful_checkpoint};
use crate::config::{hex_bytes, unhex};
use crate::constants::{MODULE_IDS, SUBMIT_HOLD};
use crate::host_reads::{read_redemptions_from_host, read_valset_members, read_valset_residents};
use crate::rpc::{JoinRequestRecord, JoinRequestView, RpcJob, RpcReply, RpcRequest, RpcStatus};
use crate::util::{hex, unix_ms};
use crate::{config, lobby, relay, relay_runtime};

impl ValidatorRuntime<'_> {
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
                for m in MODULE_IDS {
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
            RpcRequest::Shutdown => {
                // best-effort final checkpoint + journal barrier so
                // the restart replays a minimal suffix; a failure
                // here is just the crash path, which also recovers.
                // SAME sequence as the signal arm (shared macro).
                graceful_checkpoint(node, orchestrator, *next_seq).await;
                let _ = reply.send(RpcReply::ok());
                println!("[node {label}] shutdown requested via rpc — exiting");
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
            eprintln!("[node {label}] oracle result submit failed: {e}");
        }
    }

    pub(super) async fn on_lobby(&mut self, peer: ed25519::PublicKey, bytes: Vec<u8>) {
        let Self {
            node,
            lobby_tx,
            next_seq,
            signer,
            label,
            namespace,
            validators,
            coordination,
            join_requests,
            ..
        } = self;

        // `fatal: true` marks the refusal PERMANENT for this
        // invite — the joiner stops re-announcing instead of
        // spinning on a token that can never redeem.
        let mut send_reply = |recorded: bool, detail: String, cap: Option<Vec<u8>>, fatal: bool| {
            let msg = lobby::LobbyMsg::JoinReply {
                recorded,
                detail,
                cap,
                fatal,
            };
            let _ = lobby_tx.send(
                Recipients::One(peer.clone()),
                IoBuf::from(lobby::encode_msg(&msg)),
                false,
            );
        };
        let msg = match lobby::decode_msg(&bytes) {
            Ok(m) => m,
            Err(_) => return, // junk on the doorbell — drop.
        };
        // crypto first (pure, cheap): the token must verify for
        // THIS network and the announced key must prove itself. a
        // verify failure is PERMANENT for this blob (tampered,
        // foreign, or malformed) — fail it loudly.
        let verified = match lobby::verify_join_request(&msg, namespace) {
            Ok(v) => v,
            Err(e) => {
                send_reply(false, e, None, true);
                return;
            }
        };
        // then membership: the issuer must still be a member (a
        // removed member's outstanding invites die with it), and a
        // joiner that already holds standing — VALIDATOR or
        // RESIDENT — has nothing pending.
        let members = read_valset_members(node.host()).await;
        let residents_now = read_valset_residents(node.host()).await;
        let joiner_bytes = verified.joiner.as_ref().to_vec();
        if members.contains(&joiner_bytes) {
            send_reply(false, "already a validator".into(), None, false);
            return;
        }
        if residents_now.contains(&joiner_bytes) {
            send_reply(
                false,
                "already a resident — a member promotes it into the quorum".into(),
                None,
                false,
            );
            return;
        }
        if !members.contains(&verified.issuer.as_ref().to_vec()) {
            // a removed member's invites are dead FOREVER — permanent
            // reject, same loud contract as the spent-nonce path.
            send_reply(
                false,
                "the inviting member is no longer part of this network — this \
                 invite is permanently dead; ask a current member for a fresh one"
                    .into(),
                None,
                true,
            );
            return;
        }
        // SPENT-INVITE check: the token's nonce is the
        // exactly-once key (governance's Redeem handler). a nonce
        // already redeemed by ANOTHER key can never redeem again —
        // resubmitting the op is pointless and the joiner would
        // spin on "redemption not landed yet" forever. fail it
        // loudly and permanently on both ends instead. (redeemed
        // by the SAME key = standing already granted; the
        // validator/resident checks above answered that.)
        let redemptions = read_redemptions_from_host(node.host()).await;
        if let Some(spent) = redemptions
            .iter()
            .find(|r| r.nonce == verified.nonce.as_slice() && r.joiner != joiner_bytes)
        {
            println!(
                "[node {label}] lobby: {} presented an ALREADY-REDEEMED invite \
                         (spent by {} at height {}) — refusing permanently; an invite \
                         admits exactly one person, mint a fresh one per joiner",
                hex_bytes(&joiner_bytes[..4]),
                hex_bytes(&spent.joiner[..4.min(spent.joiner.len())]),
                spent.height,
            );
            send_reply(
                false,
                "invite already redeemed — an invite admits exactly one person; \
                         ask the inviter for a fresh invite"
                    .into(),
                None,
                true,
            );
            return;
        }
        // AUTO-REDEMPTION: minting the invite WAS the approval, so
        // a verified announce submits the governance Redeem op on
        // the joiner's behalf — no human step. every validator
        // re-verifies the token in-consensus and the nonce set
        // makes it single-use, so racing members (the joiner
        // round-robins its announce) collapse to one grant and
        // deterministic rejects. the in-memory map only throttles
        // re-submits across the joiner's ~3s re-announces.
        let now = unix_ms();
        let fresh = !join_requests.contains_key(&joiner_bytes);
        let record = join_requests
            .entry(joiner_bytes)
            .or_insert(JoinRequestRecord {
                issuer: verified.issuer.as_ref().to_vec(),
                first_seen_ms: now,
                last_seen_ms: 0,
            });
        // MINT the coordinator capability for the joiner, additive
        // and side-effect-free (a pure ed25519 sign — no consensus,
        // no valset change). Gated: only a GENESIS validator on a
        // PRIVATE network issues one — its key is in the
        // coordinator's pinned genesis set, so the cap it signs
        // actually admits. A public network needs no cap; a
        // non-genesis member cannot mint one the coordinator trusts.
        // The cap cannot ride the invite (the joiner's key did not
        // exist at invite-mint time), so the JoinReply is its only
        // delivery channel — re-delivered on every re-announce in
        // case a reply was lost. Rotation is DEFERRED — the cap is
        // long-lived (COORD_CAP_TTL_SECS).
        let minted_cap = if *coordination == config::Coordination::Private
            && validators.contains(&signer.public_key())
        {
            let mut subj = [0u8; 32];
            subj.copy_from_slice(verified.joiner.as_ref());
            let cap = nat_traversal::mint_coord_cap(
                signer,
                nat_traversal::NodeKey(subj),
                nat_traversal::now_secs() + nat_traversal::COORD_CAP_TTL_SECS,
            );
            Some(config::pack_coord_cap(&cap))
        } else {
            None
        };
        const REDEEM_RESUBMIT_MS: u64 = 30_000;
        if !fresh && now.saturating_sub(record.last_seen_ms) < REDEEM_RESUBMIT_MS {
            send_reply(
                true,
                "redemption in flight — standing lands shortly".into(),
                minted_cap,
                false,
            );
            return;
        }
        record.last_seen_ms = now;
        let redeem = governance::GovMsg::Redeem {
            issuer: verified.issuer.as_ref().to_vec(),
            nonce: verified.nonce.to_vec(),
            token_sig: match &msg {
                lobby::LobbyMsg::JoinRequest { token_sig, .. } => token_sig.clone(),
                _ => unreachable!("verified above"),
            },
            joiner: verified.joiner.as_ref().to_vec(),
            proof: match &msg {
                lobby::LobbyMsg::JoinRequest { proof, .. } => proof.clone(),
                _ => unreachable!("verified above"),
            },
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
            Ok(_) => {
                println!(
                    "[node {label}] invite redemption submitted: {} (invited by {})",
                    hex_bytes(verified.joiner.as_ref()),
                    hex_bytes(verified.issuer.as_ref())
                );
                send_reply(
                    true,
                    "invite verified — redemption submitted, resident standing \
                             lands at the next block"
                        .into(),
                    minted_cap,
                    false,
                );
            }
            Err(e) => {
                send_reply(false, format!("redemption submit failed: {e}"), None, false);
            }
        }
    }

    pub(super) async fn on_relay(&mut self, peer: ed25519::PublicKey, bytes: Vec<u8>) {
        let Self {
            node,
            relay_tx,
            pending_submits,
            pending_relays,
            validator_relay,
            ..
        } = self;

        let Ok(msg) = relay::decode_msg(&bytes) else {
            return;
        };
        let needs_standing = matches!(
            msg,
            relay::RelayMsg::BlobOffer { .. } | relay::RelayMsg::Submit { .. }
        );
        let (members_now, residents_now) = if needs_standing {
            (
                read_valset_members(node.host()).await,
                read_valset_residents(node.host()).await,
            )
        } else {
            (Vec::new(), Vec::new())
        };
        let Some(action) =
            validator_relay.on_message(peer, msg, &members_now, &residents_now, relay_tx)
        else {
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
                    pending_relays.insert(id, (peer, std::time::Instant::now() + SUBMIT_HOLD));
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
            node,
            relay_tx,
            signer,
            pending_submits,
            validator_relay,
            ..
        } = self;

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
        match validator_relay.prepare_local(frame, reply, peers, relay_tx) {
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
                let frame = node::encode_frame(&self.signer, seq, &Msg { target, payload });
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
                });
            }
            noded::NodeCommand::Metrics { reply } => {
                // one registry: commonware's runtime series plus the
                // `ducktape_*` block series the drain loop records.
                let _ = reply.send(self.context.encode());
            }
        }
    }
}
