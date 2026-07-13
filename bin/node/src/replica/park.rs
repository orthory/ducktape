//! phases 6b–6d of the joiner/replica role: build the serve state (sync
//! client + announce closures + resident relay/announcers/dispatch/oracle
//! pool + optional replica-restart recovery-by-replay), then the park
//! `loop` itself (serve window, drain pass, detection lane, ascension), and
//! finally the promotion checkpoint + [`reboot_self`]. one function on
//! purpose (decision 2 in the plan): the loop's `send_announce`/
//! `not_serving` closures and its mountain of loop-scoped state never leave
//! it, so splitting sub-phases into separate functions would just turn them
//! back into a carrier struct with more steps.

use commonware_codec::DecodeExt as _;
use commonware_consensus::simplex::scheme::ed25519 as simplex_ed25519;
use commonware_cryptography::{ed25519, Signer as _};
use commonware_p2p::authenticated::discovery;
use commonware_p2p::{Manager, Recipients, Receiver as P2pReceiver, Sender as P2pSender};
use commonware_runtime::{Clock, IoBuf, Metrics, Spawner, Supervisor};
use futures::{FutureExt as _, StreamExt as _};
use recovery::{Manifest, Recovery};

use crate::blob_fetch;
use crate::config::{self, hex_bytes, unhex};
use crate::constants::*;
use crate::drain_actions::{BlockAction, CutoverTrigger, EpochActions, block_actions};
use crate::explorer::{boundary_block_row, heal_index, stage_shipped_index};
use crate::host_reads::{
    joiner_epoch_mesh, read_upgrade_state, read_upgrade_version_fields, read_valset_members,
    read_valset_residents,
};
use crate::host_state::{
    NetworkBindings, SyncSubstrates, restore_host, run_output_sink,
    sync_all_modules,
};
use crate::lobby;
use crate::oracle_pool;
use crate::relay;
use crate::relay_runtime;
use crate::replica;
use crate::resident_announce;
use crate::resident_dispatch;
use crate::rpc::{RpcJob, RpcReply, RpcRequest, RpcStatus, spawn_rpc_listener};
use crate::sync::catchup::{catch_up_post_reboot_frames, PostRebootCatchupError};
use crate::sync::serve::{
    reopen_preflight_synced_host, reopen_recovery, replica_backfill, replica_orchestrator_at,
    replica_verifier, verify_manifest_floor, write_boundary_checkpoint, ServedSeal,
};
use crate::util::{diag_log, hex};

use super::promotion::{choose_promotion_boundary, joiner_manifest_fetch_retry, PromotionBoundary};
use super::reboot_self;
use super::wiring::ReplicaChannels;

use sdk::StateRoot;
use statesync::p2p::P2pSyncClient;
use statesync::{fetch_manifest, fetch_tip_coords};
use std::time::Duration;

#[allow(clippy::too_many_arguments)]
pub(super) async fn park(
    channels: ReplicaChannels,
    oracle: &mut discovery::Oracle<ed25519::PublicKey>,
    signer: ed25519::PrivateKey,
    label: String,
    namespace: Vec<u8>,
    identity_chain_id: String,
    peers: Vec<ed25519::PublicKey>,
    validators: Vec<ed25519::PublicKey>,
    wireguard_listen: Option<std::net::SocketAddr>,
    wireguard_effect: config::WireGuardEffectKind,
    invite_token: &Option<config::InviteToken>,
    checkpoint_blocks: u64,
    sync_index: bool,
    announce_capabilities: bool,
    sandbox: capability_host::SandboxBackend,
    sandbox_capacity: std::collections::BTreeMap<String, u64>,
    sync_sources: Vec<ed25519::PublicKey>,
    sync_source: Option<ed25519::PublicKey>,
    status_public_key: String,
    rpc_listener: Option<std::net::TcpListener>,
    http_cmds: futures::channel::mpsc::Receiver<noded::NodeCommand>,
    gateway_requests: Option<tokio::sync::mpsc::Receiver<noded::GatewayJob>>,
    gateway_commands: futures::channel::mpsc::Sender<noded::NodeCommand>,
    stream_hub: &noded::StreamHub,
    index: std::sync::Arc<indexer::IndexStore>,
    blobs: noded::blobs::BlobHandle,
    agent_provisioner: &Option<dispatch_oracle::SharedProvisioner>,
    agent_dirs: &capability_host::AgentDirs,
    overlay_slot: overlay_net::userspace::StackSlot,
    bulk_pacer: data_plane::BulkPacer,
    planes: data_plane::PlaneMonitor,
    workspace: std::path::PathBuf,
    storage_for_sync: std::path::PathBuf,
    forge_repo: std::path::PathBuf,
    duckfs_dir: std::path::PathBuf,
    manifest: &Option<Manifest>,
    recovery: Recovery<commonware_runtime::tokio::Context>,
) -> ! {
    let ReplicaChannels {
        context,
        replica_store,
        mut head_wake,
        mut cert_bridge,
        sync_tx,
        sync_rx,
        reach_cmd,
        mut relay_tx,
        relay_rx,
        mut lobby_tx,
    } = channels;
    let agent_peers = (wireguard_listen.is_some()
        && !matches!(wireguard_effect, config::WireGuardEffectKind::Fake))
    .then(|| {
        let tracked = crate::voice_plane::MediaPeers::new(
            String::from_utf8(namespace.clone()).expect("namespace is utf-8"),
        );
        tracked.set_peers(peers.iter());
        let me: [u8; 32] = signer
            .public_key()
            .as_ref()
            .try_into()
            .expect("ed25519 keys are 32 bytes");
        crate::agent_plane::spawn(
            label.clone(),
            crate::overlay_book::socket_factory(wireguard_effect, &overlay_slot),
            std::sync::Arc::clone(&tracked),
            me,
            bulk_pacer.clone(),
            planes.clone(),
            stream_hub.run_output(),
        );
        tracked
    });
    let gateway_book = gateway_requests.map(|requests| {
        let book = crate::gateway_plane::OverlayBook::new(
            String::from_utf8(namespace.clone()).expect("namespace is utf-8"),
        );
        book.set_peers(peers.iter());
        crate::gateway_plane::spawn(
            crate::gateway_plane::SpawnConfig {
                label: label.clone(),
                book: std::sync::Arc::clone(&book),
                me: signer.public_key(),
                factory: crate::overlay_book::socket_factory(wireguard_effect, &overlay_slot),
                pacer: bulk_pacer.clone(),
                planes: planes.clone(),
                commands: gateway_commands,
                workspace,
            },
            requests,
        );
        book
    });
    if sync_source.is_none() {
        eprintln!(
            "[node {label}] no statesync source: no validator other than this node \
             is available to serve (only validators answer the statesync channel)"
        );
        std::process::exit(1);
    }
    // the resident's mesh blob fetch-on-miss lane (the #298 cross-node
    // gap, resident side): the oracle pool's resolver asks current peers
    // for a digest its own store lacks, over this same statesync channel.
    // the park loop's sync client owns the channel receiver, so OUR fetch
    // answers route back through its unmatched-frame hook below — blob
    // rpc ids are top-bit-set random, disjoint from the client's small
    // sequential ids by construction. residents deliberately run no serve
    // loop (only validators answer this channel): fetch/client side only.
    let blob_pending: blob_fetch::PendingMap = Default::default();
    let blob_peers: std::sync::Arc<std::sync::RwLock<Vec<ed25519::PublicKey>>> =
        std::sync::Arc::new(std::sync::RwLock::new(peers.clone()));
    // the joiner's sync client: the mesh path, ROTATING across every
    // validator that can serve.
    let client = P2pSyncClient::with_sources(
        context.child("sync_client"),
        sync_tx,
        sync_rx,
        sync_sources.clone(),
        // classify_mesh_frame consumes OUR blob answers into their
        // oneshot waiters; anything else (a stray, junk) drops — a
        // resident serves nothing.
        Some(std::sync::Arc::new(move |id, body: &[u8]| {
            let _ = blob_fetch::classify_mesh_frame(&blob_pending, id, body);
        })),
    );

    // the announce, built once: this key + the invite token + the
    // proof-of-possession binding them. re-sent (round-robin over the
    // known members) until the manifest shows this key admitted —
    // members keep the request queue in memory, so a member restart
    // just gets the next re-announce.
    let announce_frame = invite_token
        .as_ref()
        .map(|t| IoBuf::from(lobby::encode_msg(&lobby::join_request(&signer, &namespace, t))));
    let mut announce_targets: Vec<ed25519::PublicKey> = validators.clone();

    let me_bytes = signer.public_key().as_ref().to_vec();
    let mut last_tracked = PEER_SET;
    // the epoch the reachability plane last retargeted to (standby
    // role) — one Retarget per observed epoch.
    let mut last_plane_epoch: Option<u64> = None;
    let mut attempt = 0usize;
    let mut announce_round = 0usize;
    // once resident standing is seen, parking is the STEADY state
    // (awaiting a deliberate promote) — the not-admitted bail below
    // must never fire.
    let mut resident_standing = false;
    let mut send_announce = |targets: &[ed25519::PublicKey], attempt: usize| {
        let Some(frame) = &announce_frame else { return };
        if attempt % LOBBY_ANNOUNCE_EVERY != 1 || targets.is_empty() {
            return;
        }
        let target = targets[announce_round % targets.len()].clone();
        announce_round += 1;
        let attempted = lobby_tx.send(Recipients::One(target.clone()), frame.clone(), false);
        if !attempted.is_empty() {
            println!(
                "[node {label}] invite announce sent to member {} — redemption follows",
                hex_bytes(&target.as_ref()[..4])
            );
        }
    };

    // ---- the RESIDENT's serving lanes ------------------------------
    //
    // the same two local surfaces a validator exposes, pumped by the
    // park loop's serve window below: a resident answers reads from
    // its last pre-synced boundary; a still-parked joiner answers
    // with a clear not-admitted error instead of a dead port. writes
    // are refused — ops enter the chain through validators only.
    // promotion re-execs this process (`reboot_self`), which closes
    // these listeners (CLOEXEC) and re-binds them on the validator
    // path.
    let (rpc_tx, mut rpc_ingress) = futures::channel::mpsc::channel::<RpcJob>(64);
    if let Some(listener) = rpc_listener {
        println!(
            "[node {label}] rpc listening on {}",
            listener.local_addr().map(|a| a.to_string()).unwrap_or_default()
        );
        spawn_rpc_listener(listener, rpc_tx);
    } else {
        drop(rpc_tx); // rpc off: the ingress arm stays terminated.
    }
    let mut http_ingress = http_cmds;
    // the last pre-synced boundary this resident serves reads from:
    // (boundary height, the composed host). exactly ONE live host may
    // exist — the sync path reopens the same on-disk partitions, so
    // this is dropped before every re-sync.
    // the REPLICA node: the same OrderedNode a validator drains, a
    // FollowerOrderer in the engine's seat, this node's real recovery
    // journal as the sink. None while knocking / bootstrapping; Some
    // from ascension on. reads serve from `.1.host()` through the
    // serve window; the fold driver feeds `.1.orderer_mut()`.
    let mut serving: Option<(
        u64,
        node::OrderedNode<
            consensus::FollowerOrderer,
            Recovery<commonware_runtime::tokio::Context>,
        >,
    )> = None;
    // the joiner's recovery journal, slot-shaped: ascension moves it
    // into the replica node (it IS the node's block sink); a descend
    // (epoch cutover / promotion) reopens a fresh handle after the
    // node drops. every path out of this branch diverges (reboot),
    // so the validator path below never observes the move.
    // the blob-plane code source every recovery/fold instance in this loop
    // realizes code-registry swaps through: local store first, then a ranged
    // verified fetch through the park loop's own sync client — a resident
    // whose binary trails a committed component heals instead of halting.
    let code_source: std::sync::Arc<dyn host::CodeSource> =
        std::sync::Arc::new(crate::blob_fetch::FetchingCodeSource::new(
            blobs.clone(),
            client.clone(),
            crate::constants::MAX_MODULE_CODE_BYTES,
            crate::constants::BLOB_FETCH_ATTEMPTS,
        ));
    let mut recovery_slot = Some(recovery);
    let mut recovery_reopens = 0u32;
    // fold-driver state, all epoch-scoped and reset at (re)ascension:
    // the verifier for the CURRENT epoch's certificates, the view
    // coordinates, and the admitted-view watermark plan_fold plans
    // against (main-side twin of the follower's internal guard).
    let mut replica_scheme: Option<simplex_ed25519::Scheme> = None;
    let mut replica_epoch: u64 = 0;
    let mut replica_view_base: u64 = 0;
    let mut replica_watermark: Option<u64> = None;
    // served seals awaiting the post-fold cross-check: a BACKFILLED
    // frame's trust is the served seal, verified against what OUR
    // fold produced (height -> served (disposition, app_hash)).
    let mut pending_seal_checks: std::collections::HashMap<u64, ServedSeal> =
        std::collections::HashMap::new();
    let mut blocks_since_checkpoint: u64 = 0;
    let mut last_cert_height: Option<u64> = None;
    // the serving replica's manifest-fetch pacer (see the gate at the
    // fetch site). absolute, so per-cert window closes can't starve it.
    let mut next_manifest_fetch = std::time::Instant::now();
    // the replica's valset orchestrator — Some exactly when serving.
    // observe/ceiling/cutover mirror the validator drain; the SWAP
    // exchanges the follower orderer where a validator respawns an
    // engine.
    let mut replica_orchestrator: Option<
        consensus::ValsetOrchestrator<ed25519::PublicKey>,
    > = None;
    // the last checkpoint's (height, oplog position) — the prune
    // anchor: the journal below it drops once the floor passes it.
    let mut replica_prev_ckpt: (Option<u64>, u64) = (None, 0);
    // the app-hash of the last boundary the derived tier followed:
    // the index feed (heal + explorer row + ws event) fires only when
    // the verified app-hash MOVED. an unchanged hash is an idle
    // stride — state is byte-identical, the read models are already
    // exact, and the explorer stays as quiet as the validator's nop
    // gate keeps it. in-memory on purpose: after a restart the first
    // boundary re-fires and every write below is idempotent.
    let mut last_indexed_root: Option<StateRoot> = None;
    // ---- REPLICA RESTART: recover by journal replay --------------
    //
    // a checkpoint that routed us here (it names this key a resident,
    // not a participant) is a real recovery base: replay the journal
    // exactly as a validator restart would — restore the checkpoint
    // host, fold the retained suffix, verify the recomposed app-hash
    // — and enter the park loop ALREADY serving at the recovered tip.
    // no re-bootstrap: the fold driver closes any offline gap over
    // the Frames lane the moment the first certificate's parent
    // linkage names it.
    if let Some(ckpt) = manifest.as_ref() {
        if let Err(e) = ckpt.preflight(MAX_PROTOCOL_VERSION) {
            eprintln!(
                "[node {label}] FATAL: cannot recover — {e} (recovered boundary needs \
                 protocol v{}, this binary supports up to v{MAX_PROTOCOL_VERSION})",
                ckpt.required_min_version
            );
            std::process::exit(1);
        }
        if let Err(e) = crate::host_state::preflight_recovery_schema(ckpt) {
            eprintln!("[node {label}] FATAL: cannot recover — {e}");
            std::process::exit(1);
        }
        let restored = restore_host(
            &context,
            &forge_repo,
            &duckfs_dir,
            ckpt,
            NetworkBindings {
                invite: &namespace,
                identity_chain_id: &identity_chain_id,
            },
            blobs.clone(),
        )
        .await;
        let mut host = match restored {
            Ok(h) => h,
            Err(e) => {
                eprintln!("[node {label}] FATAL: replica checkpoint restore: {e}");
                std::process::exit(1);
            }
        };
        // heal the derived index against the CHECKPOINT boundary
        // before replay, so the suffix folds land contiguously.
        if let Some(ckpt_height) = ckpt.height {
            heal_index(&index, &host, ckpt_height, &label).await;
        }
        let mut recovery = recovery_slot
            .take()
            .expect("the journal slot is filled before the first ascension");
        let rec = match recovery.recover_with_sink(&mut host, ckpt, None).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!(
                    "[node {label}] FATAL: {e}\n\
                     [node {label}] replica state cannot be locally recovered. wipe \
                     the app-state partitions and re-join — but ALWAYS keep the \
                     consensus journal partitions: they are the anti-equivocation \
                     record for this key."
                );
                std::process::exit(1);
            }
        };
        // seed the shared store with every retained frame so a
        // re-observed certificate resolves locally instead of
        // wedging the gate awaiting a fetch nobody owes us.
        for frame in &rec.frames {
            replica_store.pin(frame.clone());
        }
        let tip = rec.height.unwrap_or(rec.view_base);
        let root = rec.app_hash;
        let follower = consensus::FollowerOrderer::new(replica_store.clone());
        // the replica fold realizes code-registry swaps through the SAME source
        // recovery replay used (wired at Recovery::open).
        let code_source = recovery.code_source();
        let mut node_r = node::OrderedNode::resume(
            host,
            follower,
            recovery,
            rec.height.map(|height| host::FinalizedBlock {
                height,
                app_hash: root,
            }),
            rec.view_base,
        );
        node_r.set_code_source(code_source);
        replica_scheme = Some(replica_verifier(&namespace, &rec.participants));
        replica_orchestrator = Some(replica_orchestrator_at(
            rec.epoch,
            rec.view_base,
            &rec.participants,
            &rec.residents,
        ));
        replica_prev_ckpt = (ckpt.height, ckpt.oplog_pos);
        replica_epoch = rec.epoch;
        replica_view_base = rec.view_base;
        replica_watermark = Some(tip.saturating_sub(rec.view_base));
        resident_standing = rec
            .residents
            .iter()
            .any(|k| k.as_slice() == me_bytes.as_slice());
        println!(
            "[node {label}] replica: restart replayed the journal to {} \
             (epoch {}, replayed {}, already-on-disk {}{}, app_hash={})",
            tip,
            rec.epoch,
            rec.applied,
            rec.skipped,
            if rec.rolled_forward {
                ", rolled 1 forward"
            } else {
                ""
            },
            hex(&root)
        );
        // the e2e / operator serve marker, truthful here too: the
        // node serves a verified boundary — the recovered tip.
        println!(
            "[node {label}] resident: pre-synced boundary {tip} app_hash={}",
            hex(&root)
        );
        heal_index(&index, node_r.host(), tip, &label).await;
        last_indexed_root = Some(root);
        serving = Some((tip, node_r));
    }
    let not_serving = |standing: bool| -> String {
        if standing {
            "resident: no boundary pre-synced yet — retry shortly".into()
        } else {
            "joining: redemption not landed yet — no state to serve".into()
        }
    };
    // The relay runtime owns caller holds, Forge pack fanout, and the
    // persisted resident sequence. This loop only supplies current
    // validator targets and consumes unclaimed pump replies.
    let mut resident_relay = relay_runtime::ResidentRelay::new(
        storage_for_sync.join("relay-submit-seq"),
        blobs.clone(),
    );
    // bridge the relay lane ONCE, before the park loop: the serve
    // window's select is torn down every 2s tick, and dropping the p2p
    // receiver's actor-backed `recv()` mid-flight could eat a delivered
    // reply. a bounded drop-on-full mpsc survives the tick losslessly;
    // a dropped reply degrades to the caller's honest SUBMIT_HOLD sweep.
    let (relay_bridge_tx, mut relay_ingress) =
        futures::channel::mpsc::channel::<(ed25519::PublicKey, Vec<u8>)>(64);
    context.child("relay_replies").spawn(move |_ctx| {
        let mut receiver = relay_rx;
        let mut bridge_tx = relay_bridge_tx;
        async move {
            loop {
                match receiver.recv().await {
                    Ok((peer, msg)) => {
                        let bytes: Vec<u8> = msg.into();
                        let _ = bridge_tx.try_send((peer, bytes));
                    }
                    Err(_) => return, // network shutdown — nothing to serve.
                }
            }
        }
    });
    // ---- the RESIDENT-tier pumps -----------------------------------
    //
    // the state-driven twins of the validator loop's announce pump and
    // reactor seam, adapted to a node that installs boundaries instead
    // of executing blocks (see resident_announce.rs /
    // resident_dispatch.rs). discovery here mirrors the validator
    // boot: the discovered tag set is BOTH what the worker can run and
    // what this node announces, so a resident announce can never claim
    // more than the host provides; a broken operator spec is a boot
    // error, not a silently dropped executor. execution is OFF-LOOP —
    // the same DispatchPool wiring the validator runs: the gate is
    // inline, the provider CLI runs on spawned children, completed
    // results come back over `resident_oracle_results` and are
    // drained by the park loop's pump pass, so a minutes-long run
    // never stalls the serve window, boundary follow, or promotion
    // detection.
    let resident_provider_set = capability_host::discover(
        agent_dirs.clone(),
        Some(run_output_sink(stream_hub.run_output())),
        // the operator's `node.toml sandbox` choice (Direct or Podman), same
        // as the validator boot — a resident sandboxes its runs identically.
        sandbox,
    )
    .unwrap_or_else(|e| panic!("capability specs failed to load: {e}"));
    let resident_capabilities = resident_provider_set.capabilities();
    let mut resident_announcer = resident_announce::ResidentAnnouncer::new(
        me_bytes.clone(),
        resident_capabilities,
        sandbox_capacity.clone(),
    );
    let (resident_pool, mut resident_oracle_results) = oracle_pool::build(
        &context,
        resident_provider_set,
        me_bytes.clone(),
        agent_provisioner.clone(),
        // the announced capacity IS the pool's ledger (one source), same as
        // the validator path.
        sandbox_capacity,
    );
    let mut resident_dispatch =
        resident_dispatch::ResidentDispatch::new(resident_pool, me_bytes.clone());
    let (boundary, host, floor) = loop {
        attempt += 1;
        if attempt > 900 && !resident_standing {
            // ~30 minutes of 2s retries: parking forever is operator
            // guidance territory, not a silent spin. (a RESIDENT
            // holds standing indefinitely — that bail is gated off.)
            eprintln!(
                "[node {label}] FATAL: still no standing after {attempt} attempts — \
                 the invite may be spent or expired, or no member is reachable; \
                 ask for a fresh invite (manual fallback: `ducktape-node \
                 invite-accept {}`)",
                hex_bytes(&me_bytes)
            );
            std::process::exit(1);
        }
        // the serve window: between manifest polls, pump the local
        // read surfaces from the last pre-synced boundary. the window
        // closes on EITHER a head wake (cert-lane traffic — a boundary
        // just sealed, fetch now) or the fallback tick; a knocking or
        // not-yet-serving joiner keeps the fast tick, a serving
        // resident stretches it since wakes carry the follow. (a sync
        // in flight below queues jobs here — bounded by the rpc
        // bridge's buffer and the listener's reply timeout — so every
        // answer reflects a whole boundary, never a torn one.)
        {
            let fallback = if resident_standing && serving.is_some() {
                RESIDENT_FALLBACK_POLL
            } else {
                JOINER_POLL
            };
            let tick = context.sleep(fallback).fuse();
            futures::pin_mut!(tick);
            loop {
                futures::select_biased! {
                    job = rpc_ingress.next() => {
                        let Some((req, reply)) = job else { continue };
                        let resp = match req {
                            // WITH standing AND a pre-synced boundary, a
                            // write leaves here: sign it, relay to a
                            // validator, HOLD this caller's reply keyed by
                            // the frame id (answered on the relay Reply arm
                            // or the sweep). the refusal stays for the
                            // un-standing / not-yet-serving cases.
                            RpcRequest::Submit { target, payload_hex } => {
                                if !resident_standing || serving.is_none() {
                                    RpcReply::err(not_serving(resident_standing))
                                } else {
                                    match unhex(&payload_hex) {
                                        Ok(payload) => match resident_relay.submit(
                                            &signer,
                                            &announce_targets,
                                            &mut relay_tx,
                                            target,
                                            payload,
                                            relay_runtime::ResidentHold::Rpc(reply.clone()),
                                        ) {
                                            Ok(_) => {
                                                continue;
                                            }
                                            Err((_hold, e)) => RpcReply::err(e),
                                        },
                                        Err(e) => {
                                            RpcReply::err(format!("bad payload_hex: {e}"))
                                        }
                                    }
                                }
                            }
                            RpcRequest::Query { target, req_hex } => match &serving {
                                Some((_, node_r)) => match unhex(&req_hex) {
                                    Ok(req_bytes) => {
                                        match node_r.host().query(&target, &req_bytes).await
                                        {
                                            Ok(bytes) => RpcReply {
                                                reply_hex: Some(hex_bytes(&bytes)),
                                                ..RpcReply::ok()
                                            },
                                            Err(e) => RpcReply::err(format!(
                                                "query failed: {e}"
                                            )),
                                        }
                                    }
                                    Err(e) => RpcReply::err(format!("bad req_hex: {e}")),
                                },
                                None => RpcReply::err(not_serving(resident_standing)),
                            },
                            RpcRequest::Status => match &serving {
                                Some((height, node_r)) => {
                                    let mut modules = std::collections::BTreeMap::new();
                                    for m in MODULE_IDS {
                                        if let Some(root) = node_r.host().module_root(m) {
                                            modules.insert(m.to_string(), hex(&root));
                                        }
                                    }
                                    RpcReply {
                                        status: Some(RpcStatus {
                                            height: Some(*height),
                                            app_hash: hex(&node_r.host().app_hash()),
                                            modules,
                                        }),
                                        ..RpcReply::ok()
                                    }
                                }
                                None => RpcReply::err(not_serving(resident_standing)),
                            },
                            RpcRequest::JoinRequests => RpcReply::err(
                                "this node is not a member — join requests queue on \
                                 validators",
                            ),
                            RpcRequest::Shutdown => {
                                // a resident writes no checkpoint — nothing to
                                // flush; a restart parks straight back here.
                                let _ = reply.send(RpcReply::ok());
                                println!(
                                    "[node {label}] shutdown requested via rpc — exiting"
                                );
                                std::process::exit(0);
                            }
                        };
                        let _ = reply.send(resp);
                    }
                    cmd = http_ingress.next() => {
                        let Some(cmd) = cmd else { continue };
                        match cmd {
                            // `origin` is the caller's CLAIMED submitter — but
                            // this lane signs frames with THIS node's identity
                            // (authorship = status.publicKey), so it is ignored.
                            // WITH standing AND a boundary, relay and HOLD the
                            // oneshot keyed by the frame id; otherwise refuse.
                            noded::NodeCommand::Submit {
                                target,
                                payload,
                                origin: _,
                                reply,
                            } => {
                                if !resident_standing || serving.is_none() {
                                    let _ =
                                        reply.send(Err(not_serving(resident_standing)));
                                } else {
                                    match resident_relay.submit(
                                        &signer,
                                        &announce_targets,
                                        &mut relay_tx,
                                        target,
                                        payload,
                                        relay_runtime::ResidentHold::Http(reply),
                                    ) {
                                        Ok(_) => {}
                                        Err((hold, e)) => hold.fail(e),
                                    }
                                }
                            }
                            // an ALREADY-SIGNED frame (an agent's session key,
                            // not this node's): relayed VERBATIM — the resident
                            // is the courier, never the author, so it neither
                            // re-signs nor spends its own seq. the custodian
                            // validator verifies the signature before it pins,
                            // exactly as it does for a frame the resident signed
                            // itself. same standing rule as above: no standing,
                            // no boundary, no relay.
                            noded::NodeCommand::SubmitFrame { frame, reply } => {
                                if !resident_standing || serving.is_none() {
                                    let _ =
                                        reply.send(Err(not_serving(resident_standing)));
                                } else {
                                    match resident_relay.submit_frame(
                                        frame,
                                        &announce_targets,
                                        &mut relay_tx,
                                        relay_runtime::ResidentHold::Http(reply),
                                    ) {
                                        Ok(_) => {}
                                        Err((hold, e)) => hold.fail(e),
                                    }
                                }
                            }
                            noded::NodeCommand::Query { target, req, reply } => {
                                let result = match &serving {
                                    Some((_, node_r)) => node_r
                                        .host()
                                        .query(&target, &req)
                                        .await
                                        .map_err(|e| e.to_string()),
                                    None => Err(not_serving(resident_standing)),
                                };
                                let _ = reply.send(result);
                            }
                            noded::NodeCommand::Status { reply } => {
                                // pre-first-sync the surface still answers (the
                                // app's liveness heartbeat): a zeroed status is
                                // honest — no boundary is served yet.
                                let (height, app_hash, modules) = match &serving {
                                    Some((height, node_r)) => (
                                        *height,
                                        hex(&node_r.host().app_hash()),
                                        MODULE_IDS
                                            .iter()
                                            .map(|m| noded::ModuleStatus {
                                                id: (*m).into(),
                                                root: node_r
                                                    .host()
                                                    .module_root(m)
                                                    .map(|r| hex(&r))
                                                    .unwrap_or_default(),
                                                category: noded::ModuleCategory::of(m),
                                            })
                                            .collect(),
                                    ),
                                    None => (0, String::new(), Vec::new()),
                                };
                                let _ = reply.send(noded::NodeStatus {
                                    version: env!("CARGO_PKG_VERSION").into(),
                                    app_hash,
                                    height,
                                    modules,
                                    public_key: status_public_key.clone(),
                                });
                            }
                            noded::NodeCommand::Metrics { reply } => {
                                let _ = reply.send(context.encode());
                            }
                        }
                    }
                    // a validator's answer for a frame we relayed: match it
                    // to the held caller by frame id and release the reply.
                    // an unknown id (already swept, or a stray) drops.
                    answer = relay_ingress.next() => {
                        let Some((peer, bytes)) = answer else { continue };
                        let Ok(msg) = relay::decode_msg(&bytes) else { continue };
                        let Some((frame_id, outcome)) =
                            resident_relay.on_message(peer, msg, &mut relay_tx)
                        else {
                            continue;
                        };
                        // Unclaimed final replies belong to the
                        // resident-owned capability/dispatch pumps.
                        let applied =
                            matches!(outcome, relay::RelayOutcome::Applied { .. });
                        if let Some(ok) = resident_announcer.on_reply(&frame_id, applied) {
                            if ok {
                                println!(
                                    "[node {label}] resident: announced capabilities {:?}",
                                    resident_announcer.capabilities()
                                );
                            } else {
                                eprintln!(
                                    "[node {label}] resident: capability announce did not \
                                     apply ({outcome:?}) - will retry"
                                );
                            }
                        } else if let Some((saga_id, attempt)) =
                            resident_dispatch.on_reply(&frame_id, applied)
                        {
                            if applied {
                                println!(
                                    "[node {label}] resident: dispatch result for saga \
                                     {saga_id} attempt {attempt} applied"
                                );
                            } else {
                                eprintln!(
                                    "[node {label}] resident: dispatch result for saga \
                                     {saga_id} attempt {attempt} did not apply \
                                     ({outcome:?}) - will retry while leased"
                                );
                            }
                        }
                    }
                    // a raw certificate arrived. FOLDING replica:
                    // decode, plan against the watermark, admit
                    // through the verified follower gate (backfilling
                    // any parent-linkage gap over the Frames lane
                    // first), then close the window so the post-
                    // window pass drains the fold. NOT yet folding:
                    // fall through — the coalesced wake below carries
                    // the old poll-now semantics.
                    cert = cert_bridge.next() => {
                        let Some(raw) = cert else { continue };
                        let (Some((_, node_r)), Some(scheme)) =
                            (serving.as_mut(), replica_scheme.as_ref())
                        else {
                            continue;
                        };
                        let Some(anchor) = replica::anchor_from_cert_msg(scheme, &raw)
                        else {
                            continue;
                        };
                        if anchor.epoch != replica_epoch {
                            // another epoch's certificate: our epoch
                            // ended. the manifest fallback observes
                            // the new epoch and descends/re-ascends.
                            break;
                        }
                        if let replica::FoldStep::Stale =
                            replica::plan_fold(replica_watermark, &anchor)
                        {
                            continue;
                        }
                        if let replica::FoldStep::BackfillThenObserve {
                            after_view,
                            up_to_view,
                        } = replica::plan_fold(replica_watermark, &anchor)
                            && let Err(e) = replica_backfill(
                                &client,
                                node_r,
                                replica_view_base,
                                (after_view, up_to_view),
                                &mut replica_watermark,
                                &mut pending_seal_checks,
                                &label,
                            )
                            .await
                        {
                            println!(
                                "[node {label}] replica: backfill ({after_view}, \
                                 {up_to_view}] unavailable: {e} — retrying on the \
                                 next certificate"
                            );
                            break;
                        }
                        match node_r.orderer_mut().observe_finalization(
                            &mut rand::rngs::OsRng,
                            scheme,
                            &anchor.finalization,
                        ) {
                            Ok(consensus::Observed::Admitted(view)) => {
                                replica_watermark = Some(view);
                                // fold in the post-window drain pass.
                                break;
                            }
                            Ok(consensus::Observed::Stale(_)) => continue,
                            Ok(consensus::Observed::Unresolvable(view)) => {
                                // payload gossip missed this block's
                                // bytes and the follower runs without
                                // a resolver: fetch the frame itself
                                // over the Frames lane (seal
                                // cross-checked post-fold), which
                                // also admits it.
                                if let Err(e) = replica_backfill(
                                    &client,
                                    node_r,
                                    replica_view_base,
                                    (replica_watermark.unwrap_or(0), view),
                                    &mut replica_watermark,
                                    &mut pending_seal_checks,
                                    &label,
                                )
                                .await
                                {
                                    println!(
                                        "[node {label}] replica: unresolvable view \
                                         {view} backfill failed: {e} — retrying on \
                                         the next certificate"
                                    );
                                }
                                break;
                            }
                            Err(e) => {
                                // quorum verification failed: a lying
                                // certificate source. drop it loudly.
                                eprintln!(
                                    "[node {label}] replica: certificate refused: {e}"
                                );
                                continue;
                            }
                        }
                    },
                    // a sealed boundary's certificate arrived: stop
                    // serving the window and go fetch the manifest.
                    // (None — every drain gone — only happens at mesh
                    // shutdown; fall through to the tick's exit.)
                    wake = head_wake.next() => if wake.is_some() { break },
                    _ = tick => break,
                }
            }
        }
        // ---- the replica drain pass ------------------------------
        //
        // fold whatever the gate released, then the validator drain's
        // per-block side effects, minus its validator-only concerns
        // (submit holds, engine orchestration): the seal cross-check
        // for backfilled heights, the per-block derived-index fold
        // (no more healing), the explorer row, the ws block event,
        // the finalization floor, and the checkpoint cadence.
        if let Some((served_height, node_r)) = serving.as_mut() {
            if let Err(e) = node_r.drain_delivered().await {
                eprintln!("[node {label}] FATAL: replica fold: {e}");
                std::process::exit(1);
            }
            let drained = node_r.take_drained();
            // The same projection the validator consumes; this loop retains
            // replica-only seal verification, streaming, and checkpoints.
            for action in block_actions(&drained, node_r.take_system_dispatches(), &blobs) {
                let BlockAction {
                    height,
                    dispatches,
                    record,
                    sealed_hash,
                    ..
                } = action;
                // a BACKFILLED height's trust is the served seal:
                // what our fold produced must match it exactly, or
                // this replica has diverged from the quorum's fold.
                if let Some((_, served_hash, served_roots)) =
                    pending_seal_checks.remove(&height)
                    && sealed_hash.is_some_and(|h| h != served_hash)
                {
                    // name the diverging module(s) — the one lead an
                    // operator (or the next debugger) needs first.
                    for (module, served_root) in &served_roots {
                        let ours = node_r.host().module_root(module);
                        if ours.as_ref() != Some(served_root) {
                            eprintln!(
                                "[node {label}] replica: diverged module={module} \
                                 served={} ours={}",
                                hex(served_root),
                                ours.map(|r| hex(&r)).unwrap_or_else(|| "none".into())
                            );
                        }
                    }
                    eprintln!(
                        "[node {label}] FATAL: backfilled height {height} folded to \
                         {} but the quorum sealed {} — state diverged",
                        hex(&sealed_hash.expect("checked above")),
                        hex(&served_hash)
                    );
                    std::process::exit(1);
                }
                let ops = indexer::BlockOps {
                    record,
                    ..noded::index_block_ops(height, height, &dispatches)
                };
                if let Err(err) = index.apply_block(&ops) {
                    eprintln!(
                        "[node {label}] replica index apply failed at height \
                         {height}: {err} — wipe <storage>/index to rebuild"
                    );
                }
                if let Some(root) = sealed_hash {
                    stream_hub.publish_block(height, hex(&root));
                    last_indexed_root = Some(root);
                }
                *served_height = height;
                blocks_since_checkpoint += 1;
            }
            // ---- valset orchestration (the replica mirror) --------
            //
            // observe → ceiling → cutover, exactly the validator
            // drain's discipline. the CEILING is correctness, not
            // bookkeeping: a frame finalized before the cutover but
            // landing after it is DISCARDED by every validator, and
            // a replica without the ceiling would apply it — silent
            // divergence. the cutover SWAPS the follower orderer
            // (journaling Record::Cutover) where a validator
            // respawns an engine; the manifest-epoch descend remains
            // the safety net for anything this mirror missed.
            if !drained.is_empty()
                && let Some(orch) = replica_orchestrator.as_mut()
            {
                let folded_view = served_height.saturating_sub(replica_view_base);
                let members_raw = read_valset_members(node_r.host()).await;
                let observed: Vec<ed25519::PublicKey> = members_raw
                    .iter()
                    .filter_map(|k| ed25519::PublicKey::decode(k.as_slice()).ok())
                    .collect();
                let residents_raw = read_valset_residents(node_r.host()).await;
                let observed_residents: Vec<ed25519::PublicKey> = residents_raw
                    .iter()
                    .filter_map(|k| ed25519::PublicKey::decode(k.as_slice()).ok())
                    .collect();
                let mut actions =
                    EpochActions::new(orch, folded_view, observed, observed_residents);
                if let Some(CutoverTrigger::Membership(cutover)) = actions.observe_members() {
                    println!(
                        "[node {label}] replica: membership change observed at view {} \
                         — cutover to epoch {} at view {}",
                        cutover.observed_view(),
                        cutover.next_epoch(),
                        cutover.cutover_view()
                    );
                    node_r.set_view_ceiling(cutover.cutover_view());
                }
                let boundary_upgrade = read_upgrade_state(node_r.host()).await;
                if let Some(CutoverTrigger::Upgrade {
                    cutover,
                    name,
                    activation_height,
                }) = actions.observe_upgrade(&boundary_upgrade)
                {
                    println!(
                        "[node {label}] replica: upgrade '{name}' armed — cutover to epoch \
                         {} at view {} (activation height {activation_height})",
                        cutover.next_epoch(),
                        cutover.cutover_view()
                    );
                    node_r.set_view_ceiling(cutover.cutover_view());
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
                    let plan_resident_bytes: Vec<Vec<u8>> = plan_residents
                        .iter()
                        .map(|k| k.as_ref().to_vec())
                        .collect();
                    // transport first, exactly like the validator:
                    // the new epoch's mesh must admit its members.
                    let mesh =
                        joiner_epoch_mesh(&peers, &member_bytes, &plan_resident_bytes);
                    // the blob fetch-on-miss lane fans out to the same
                    // tracked set — follow the re-track (the validator
                    // drain's exact discipline).
                    *blob_peers.write().expect("blob peers lock") =
                        mesh.iter().cloned().collect();
                    oracle.track(plan.epoch(), mesh);
                    if let Some(book) = &gateway_book {
                        book.set_peers(plan.valset().transport_members().iter());
                    }
                    if let Some(peers) = &agent_peers {
                        peers.set_peers(plan.valset().transport_members().iter());
                    }
                    last_tracked = plan.epoch();
                    // the follower swap: same OrderedNode, fresh
                    // orderer, cutover journaled — the epoch-local
                    // view clock restarts with the new base.
                    let follower =
                        consensus::FollowerOrderer::new(replica_store.clone());
                    if let Err(e) = node_r
                        .cutover(
                            follower,
                            plan.epoch(),
                            plan.cutover_app_height(),
                            &member_bytes,
                            &plan_resident_bytes,
                        )
                        .await
                    {
                        eprintln!(
                            "[node {label}] FATAL: replica cutover journal write: {e}"
                        );
                        std::process::exit(1);
                    }
                    node_r.host_mut().set_active_version(plan.boundary_version());
                    replica_scheme =
                        Some(replica_verifier(&namespace, &member_bytes));
                    replica_epoch = plan.epoch();
                    replica_view_base = plan.cutover_app_height();
                    replica_watermark = None;
                    pending_seal_checks.clear();
                    // force a checkpoint on the next pass — the
                    // validator writes one immediately post-cutover
                    // for the same restart-boundary reason.
                    blocks_since_checkpoint = checkpoint_blocks;
                    println!(
                        "[node {label}] replica: epoch cutover to {} at base {} — \
                         follower swapped in-loop",
                        plan.epoch(),
                        plan.cutover_app_height()
                    );
                }
            }
            // persist the finalization floor for the newest certificate
            // whose view has fully drained — cert first, release point
            // second, same ordering proof (and the same busy-chain
            // starvation fix) as the validator drain.
            if let Some(tip_view) = node_r.finalized_view()
                && let Some((view, cert)) = node_r.orderer().finalization_at_or_below(tip_view)
                && view != 0
                && node_r
                    .orderer()
                    .min_unreleased_view()
                    .is_none_or(|pending| pending > view)
            {
                let height = replica_view_base + view;
                if last_cert_height.is_none_or(|h| height > h) {
                    let fc = recovery::FloorCert {
                        epoch: replica_epoch,
                        height,
                        cert,
                    };
                    match node_r.sink_mut().write_floor_cert(&fc).await {
                        Ok(()) => last_cert_height = Some(height),
                        Err(e) => eprintln!(
                            "[node {label}] replica floor cert write failed \
                             (will retry): {e}"
                        ),
                    }
                }
            }
            // periodic checkpoint at the folded tip: a restart
            // recovers here and replays only the suffix — exactly a
            // validator restart. participants/residents read from the
            // FOLDED state (the same projection the checkpoint's
            // epoch coordinates describe). journal pruning stays the
            // validator's concern for now (a replica's journal prunes
            // at its next ascension checkpoint).
            if blocks_since_checkpoint >= checkpoint_blocks
                && let Some(f) = node_r.finalized()
            {
                let pos = node_r.sink_mut().oplog_pos().await;
                let (cv, pu) = read_upgrade_version_fields(node_r.host()).await;
                let members = read_valset_members(node_r.host()).await;
                let residents = read_valset_residents(node_r.host()).await;
                let captured = Manifest::capture(
                    node_r.host(),
                    Some(f.height),
                    replica_epoch,
                    replica_view_base,
                    members,
                    residents,
                    None,
                    cv,
                    pu,
                    pos,
                    1,
                );
                match captured {
                    Ok(ckpt) => match node_r.sink_mut().write_manifest(&ckpt).await {
                        Ok(()) => {
                            // prune the journal below the PREVIOUS
                            // checkpoint once the persisted floor
                            // passed it — the validator's exact
                            // prune discipline. without this a
                            // long-lived replica's journal grows
                            // without bound (pruned frames must
                            // never be needed to resolve a
                            // re-reported finalization; the floor
                            // gate guarantees it).
                            let floor_passed = matches!(
                                node_r.sink_mut().floor_cert(),
                                Ok(Some(fc))
                                    if replica_prev_ckpt
                                        .0
                                        .is_none_or(|h| fc.height >= h)
                            );
                            if floor_passed
                                && let Err(e) = node_r
                                    .sink_mut()
                                    .prune_oplog(replica_prev_ckpt.1)
                                    .await
                            {
                                eprintln!(
                                    "[node {label}] replica oplog prune failed: {e}"
                                );
                            }
                            replica_prev_ckpt = (ckpt.height, pos);
                            blocks_since_checkpoint = 0;
                        }
                        Err(e) => eprintln!(
                            "[node {label}] replica checkpoint write failed \
                             (will retry): {e}"
                        ),
                    },
                    Err(e) => eprintln!(
                        "[node {label}] replica checkpoint capture failed \
                         (will retry): {e}"
                    ),
                }
            }
        }
        resident_relay.expire(std::time::Instant::now());
        // a FOLDING replica's window closes per certificate; this
        // poll is only the fallback DETECTION lane now (standing
        // detection pre-ascension; promotion, cutover, and revocation
        // detection after). it reads tip COORDINATES — membership,
        // epoch, height — which the server answers from loop-owned
        // state with no capture, no lease, and no floor-cert gate;
        // the transitions that consume an actual boundary (ascension,
        // promotion) fetch a full manifest inside their branch. pace
        // it on an ABSOLUTE deadline — the window's own tick restarts
        // per close and would never fire under steady cert traffic —
        // so a fleet of replicas doesn't besiege the serve window per
        // block, yet detection stays bounded by the fallback cadence.
        if serving.is_some() && std::time::Instant::now() < next_manifest_fetch {
            continue;
        }
        next_manifest_fetch = std::time::Instant::now() + RESIDENT_FALLBACK_POLL;
        let tip = match fetch_tip_coords(&client).await {
            Ok(tip) => tip,
            Err(e) => {
                let retry = joiner_manifest_fetch_retry(&label, resident_standing, &e);
                println!("{}", retry.log_line);
                if retry.announce {
                    send_announce(&announce_targets, attempt);
                }
                continue;
            }
        };
        // follow the mesh rotation while parked. the participant
        // list is an unverified serving hint — the union with the
        // descriptor mesh keeps the real members reachable, and
        // promotion re-derives everything from verified state.
        if tip.epoch > last_tracked {
            if tip.epoch >= EPOCH_CHANNEL_BANK {
                println!(
                    "[node {label}] warning: the network is at epoch {} — beyond this \
                     process's pre-registered channel bank ({EPOCH_CHANNEL_BANK}); \
                     expect reconnect churn while parked",
                    tip.epoch
                );
            }
            let mesh = joiner_epoch_mesh(&peers, &tip.participants, &tip.residents);
            // the blob fetch-on-miss lane fans out to the same tracked
            // set — follow the re-track.
            *blob_peers.write().expect("blob peers lock") = mesh.iter().cloned().collect();
            oracle.track(tip.epoch, mesh);
            if gateway_book.is_some() || agent_peers.is_some() {
                let transport: Vec<ed25519::PublicKey> = tip
                    .participants
                    .iter()
                    .chain(tip.residents.iter())
                    .filter_map(|key| ed25519::PublicKey::decode(key.as_slice()).ok())
                    .collect();
                if let Some(book) = &gateway_book {
                    book.set_peers(transport.iter());
                }
                if let Some(peers) = &agent_peers {
                    peers.set_peers(transport.iter());
                }
            }
            last_tracked = tip.epoch;
        }
        // drive the reachability plane's standby role off the
        // manifest: membership and resident standing come from the
        // synced boundary, whose height doubles as the plane's
        // freshness clock (the same app-height regime the members'
        // ViewTicks run — within the advert TTL's generous window).
        // Nothing is sent before standing: no member would admit the
        // gossip yet.
        if let Some(cmd) = &reach_cmd
            && tip.residents.iter().any(|k| k == &me_bytes)
        {
            // NON-BLOCKING sends throughout: the plane is not this
            // loop's dependency. a shed ViewTick is one beat of
            // advert staleness (the next poll carries a fresher one);
            // a refused Retarget retries naturally — the epoch latch
            // below only advances when the send is taken.
            let clock = tip.view_base.max(tip.height);
            let _ = cmd.try_send(reachability::ReachabilityCommand::ViewTick(clock));
            if last_plane_epoch != Some(tip.epoch) {
                let members: Vec<ed25519::PublicKey> = tip
                    .participants
                    .iter()
                    .filter_map(|k| ed25519::PublicKey::decode(k.as_slice()).ok())
                    .collect();
                let standbys: Vec<ed25519::PublicKey> = tip
                    .residents
                    .iter()
                    .filter_map(|k| ed25519::PublicKey::decode(k.as_slice()).ok())
                    .collect();
                if cmd
                    .try_send(reachability::ReachabilityCommand::Retarget(
                        reachability::MeshEpochEvent {
                            epoch: tip.epoch,
                            members,
                            standbys,
                            current_view: clock,
                        },
                    ))
                    .is_ok()
                {
                    last_plane_epoch = Some(tip.epoch);
                }
            }
        }
        if !tip.participants.iter().any(|k| k == &me_bytes) {
            // the tip names the CURRENT members — better announce
            // targets than the genesis descriptor's list.
            let current: Vec<ed25519::PublicKey> = tip
                .participants
                .iter()
                .filter_map(|k| ed25519::PublicKey::decode(k.as_slice()).ok())
                .collect();
            if !current.is_empty() {
                announce_targets = current;
            }
            if serving.is_some() && tip.epoch > replica_epoch {
                // the network cut over to a new epoch: our follower's
                // verifier and fetch lane are the old epoch's, so its
                // certs stopped verifying here. DESCEND — drop the
                // node (journal checkpointed on cadence), reopen the
                // journal handle — and re-ascend at the new epoch's
                // boundary below. the in-loop follower swap
                // (node.cutover, no re-bootstrap) is the promotion
                // collapse's concern (phase 3).
                println!(
                    "[node {label}] replica: epoch cutover {} -> {} — re-ascending",
                    replica_epoch, tip.epoch
                );
                serving = None;
                replica_scheme = None;
                replica_orchestrator = None;
                recovery_slot =
                    Some(reopen_recovery(&context, &mut recovery_reopens, &label, code_source.clone()).await);
            }
            if tip.residents.iter().any(|k| k == &me_bytes) {
                if !resident_standing {
                    resident_standing = true;
                    println!(
                        "[node {label}] resident: standing granted — following \
                         boundaries and serving local reads"
                    );
                }
                // RESIDENT standing (staged admission): granted, so
                // stop knocking — and ASCEND to the replica pipeline
                // (unified-node phase 2): bootstrap ONE boundary,
                // journal it as this node's recovery-boot base, fold
                // the frame suffix to the live tip through that same
                // journal, then follow the head by folding finalized
                // frames exactly like a validator — the boundary
                // re-install loop is gone. reads serve from the
                // node's host through the serve window above, and
                // `promote` finds a node already at head.
                if serving.is_none() {
                    // ascension consumes the BOUNDARY itself — module
                    // entries to sync and the floor certificate to
                    // verify — so this transition (and only this
                    // transition) rides the full Manifest lane.
                    let m = match fetch_manifest(&client).await {
                        Ok(m) => m,
                        Err(e) => {
                            let retry = joiner_manifest_fetch_retry(
                                &label,
                                resident_standing,
                                &e,
                            );
                            println!("{}", retry.log_line);
                            continue;
                        }
                    };
                    if let Err(e) = m.preflight(MAX_PROTOCOL_VERSION) {
                        eprintln!(
                            "[node {label}] FATAL: cannot observe this network — {e}"
                        );
                        std::process::exit(1);
                    }
                    println!(
                        "[node {label}] replica: bootstrapping at boundary {} ({} modules)",
                        m.height,
                        m.entries.len()
                    );
                    match sync_all_modules(
                        &context,
                        &client,
                        &m,
                        NetworkBindings {
                            invite: &namespace,
                            identity_chain_id: &identity_chain_id,
                        },
                        SyncSubstrates {
                            forge_repo: &forge_repo,
                            duckfs_dir: &duckfs_dir,
                            blobs: blobs.clone(),
                        },
                        attempt,
                    )
                    .await
                    {
                        Ok(mut host) => {
                            // the boundary's floor must verify (real
                            // quorum signatures) before it becomes
                            // this journal's genesis — the same gate
                            // promotion runs.
                            let floor = match verify_manifest_floor(&namespace, &m) {
                                Ok(cert) => cert.map(|cert| recovery::FloorCert {
                                    epoch: m.epoch,
                                    height: m.height,
                                    cert,
                                }),
                                Err(e) => {
                                    println!(
                                        "[node {label}] replica: boundary {} floor \
                                         refused ({e}) — retrying",
                                        m.height
                                    );
                                    continue;
                                }
                            };
                            let mut recovery = recovery_slot
                                .take()
                                .expect("the journal slot is filled whenever serving is None");
                            let ckpt_pos = write_boundary_checkpoint(
                                &mut recovery,
                                &host,
                                &m,
                                &floor,
                                &label,
                                "replica_checkpoint",
                            )
                            .await;
                            replica_prev_ckpt = (Some(m.height), ckpt_pos);
                            // close the boundary -> live-tip gap
                            // through the SAME journal a validator
                            // restart would replay; every served
                            // frame is seal-verified inside.
                            let caught = match catch_up_post_reboot_frames(
                                &client,
                                &mut recovery,
                                &mut host,
                                None,
                                m.height,
                                POST_REBOOT_CATCHUP_MAX_ITERS,
                            )
                            .await
                            {
                                Ok(c) => c,
                                Err(PostRebootCatchupError::Fatal(e)) => {
                                    eprintln!(
                                        "[node {label}] FATAL: replica suffix fold: {e}"
                                    );
                                    std::process::exit(1);
                                }
                                Err(e) => {
                                    println!(
                                        "[node {label}] replica: suffix fold at \
                                         boundary {} unavailable ({e:?}) — re-bootstrapping",
                                        m.height
                                    );
                                    recovery_slot = Some(recovery);
                                    continue;
                                }
                            };
                            let tip = caught.to_height.max(m.height);
                            // seed the shared store with the folded
                            // suffix: peers' resolvers can fetch these
                            // from us, and a re-reported cert for a
                            // just-folded height resolves locally.
                            for bytes in &caught.frame_bytes {
                                replica_store.put(bytes.clone());
                            }
                            let root = host.app_hash();
                            // the fold pipeline: the follower orderer
                            // in the engine's seat of the SAME
                            // OrderedNode a validator drains, this
                            // journal as its sink. resolver-less by
                            // design (see the lane wiring above): a
                            // store miss surfaces as Unresolvable and
                            // the driver backfills over the Frames
                            // lane.
                            let follower =
                                consensus::FollowerOrderer::new(replica_store.clone());
                            let code_source = recovery.code_source();
                            let mut node_r = node::OrderedNode::resume(
                                host,
                                follower,
                                recovery,
                                Some(host::FinalizedBlock {
                                    height: tip,
                                    app_hash: root,
                                }),
                                m.view_base,
                            );
                            node_r.set_code_source(code_source);
                            replica_scheme =
                                Some(replica_verifier(&namespace, &m.participants));
                            replica_orchestrator = Some(replica_orchestrator_at(
                                m.epoch,
                                m.view_base,
                                &m.participants,
                                &m.residents,
                            ));
                            replica_epoch = m.epoch;
                            replica_view_base = m.view_base;
                            replica_watermark = Some(tip.saturating_sub(m.view_base));
                            blocks_since_checkpoint = 0;
                            pending_seal_checks.clear();
                            // the stable serve marker: "this node now
                            // serves a verified boundary" — the line
                            // the e2e suite (and operators) key on,
                            // truthful under both the old re-install
                            // model and the fold pipeline.
                            println!(
                                "[node {label}] resident: pre-synced boundary {} \
                                 app_hash={}",
                                tip,
                                hex(&root)
                            );
                            println!(
                                "[node {label}] replica: following the head from {} \
                                 (epoch {}, app_hash={})",
                                tip,
                                m.epoch,
                                hex(&root)
                            );
                            // the derived tier starts exact at the
                            // ascension tip; per-block folds keep it
                            // current from here (no more healing).
                            if last_indexed_root.as_ref() != Some(&root) {
                                heal_index(&index, node_r.host(), tip, &label).await;
                                if let Err(err) = index.apply_block_record(
                                    tip,
                                    boundary_block_row(tip, &root),
                                ) {
                                    eprintln!(
                                        "[node {label}] replica: explorer row at \
                                         ascension tip {tip} refused: {err}"
                                    );
                                }
                                stream_hub.publish_block(tip, hex(&root));
                                last_indexed_root = Some(root);
                            }
                            serving = Some((tip, node_r));
                        }
                        Err(e) => println!(
                            "[node {label}] replica bootstrap at boundary {} failed: {e}",
                            m.height
                        ),
                    }
                }
                // ---- the resident-tier pumps, one pass per poll ----
                //
                // both read the served boundary (committed state) and
                // write through the relay lane — the resident's only
                // write path. state-driven and idempotent like their
                // validator-loop twins: quiet once committed state
                // matches, deadline-based retry over the lossy lane.
                if let Some((_, node_r)) = &serving {
                    let host = node_r.host();
                    let now = std::time::Instant::now();
                    // CAPABILITY ANNOUNCE (resident tier): mirrors the
                    // validator pump, including the config gate — an
                    // `announce_capabilities = false` resident stays an
                    // accept-lane-only provider and never enters a
                    // tag's rendezvous pool.
                    if announce_capabilities
                        && let Some(msg) =
                            resident_announcer.maybe_announce(host, now).await
                    {
                        match resident_relay.submit_unheld(
                            &signer,
                            &announce_targets,
                            &mut relay_tx,
                            msg.target,
                            msg.payload,
                        ) {
                            Ok(id) => {
                                resident_announcer.sent(id, now);
                                println!(
                                    "[node {label}] resident: capability announce \
                                     relayed ({:?})",
                                    resident_announcer.capabilities()
                                );
                            }
                            Err(e) => {
                                resident_announcer.send_failed();
                                eprintln!(
                                    "[node {label}] resident: capability announce \
                                     relay failed: {e}"
                                );
                            }
                        }
                    }
                    // DISPATCH EXECUTION (resident tier): serve the
                    // saga attempts leased to this key, so an announced
                    // resident never stalls an assignment. completed
                    // off-loop runs are drained FIRST (they become due
                    // relay sends in this same pass); the tick itself
                    // only gates and spawns — it never awaits a
                    // provider.
                    while let Ok(msg) = resident_oracle_results.try_recv() {
                        resident_dispatch.completed(msg);
                    }
                    for (key, msg) in resident_dispatch.tick(host, now).await {
                        match resident_relay.submit_unheld(
                            &signer,
                            &announce_targets,
                            &mut relay_tx,
                            msg.target,
                            msg.payload,
                        ) {
                            Ok(id) => {
                                resident_dispatch.sent(&key, id, now);
                                println!(
                                    "[node {label}] resident: dispatch result for \
                                     saga {} attempt {} relayed",
                                    key.0, key.1
                                );
                            }
                            Err(e) => eprintln!(
                                "[node {label}] resident: dispatch result relay \
                                 failed for saga {} attempt {}: {e}",
                                key.0, key.1
                            ),
                        }
                    }
                }
                continue;
            }
            println!(
                "[node {label}] joining: awaiting redemption (epoch {} has {} validators)",
                tip.epoch,
                tip.participants.len()
            );
            send_announce(&announce_targets, attempt);
            continue;
        }
        // in the epoch set: PROMOTION consumes the boundary itself —
        // module entries and the real floor certificate — so it rides
        // the full Manifest lane from here.
        let m = match fetch_manifest(&client).await {
            Ok(m) => m,
            Err(e) => {
                let retry = joiner_manifest_fetch_retry(&label, resident_standing, &e);
                println!("{}", retry.log_line);
                continue;
            }
        };
        // a boundary PAST the epoch base needs its
        // finalization floor served alongside, or the respawned
        // engine would re-deliver history the synced state already
        // contains — retry until the source's floor catches up.
        if m.height > m.view_base && m.floor_cert.is_none() {
            println!(
                "[node {label}] admitted; boundary {} lacks its finalization floor \
                 yet — retrying",
                m.height
            );
            continue;
        }
        println!(
            "[node {label}] admitted at epoch {} boundary {} — syncing {} modules",
            m.epoch,
            m.height,
            m.entries.len()
        );
        // BOOT PREFLIGHT (design §5 / plan Task 7.3): refuse an
        // under-versioned binary against the served boundary before
        // install/replay — a clear early refusal, not a post-sync app-hash
        // mismatch. inert on a baseline manifest.
        if let Err(e) = m.preflight(MAX_PROTOCOL_VERSION) {
            eprintln!("[node {label}] FATAL: cannot promote — {e}");
            std::process::exit(1);
        }
        // THE PROMOTION COLLAPSE for a FOLDING replica: it is already
        // at head with a journal that proved every block it folded —
        // checkpoint OUR OWN state as the validator boot base and
        // reboot. no re-sync against the source, no boundary wait: a
        // quorum-widening cutover HALTS the source awaiting this very
        // node's votes, so any wait-for-the-source flow deadlocks —
        // the freshest member seats itself from its own state.
        if serving.is_some() {
            let (folded_tip, mut node_r) =
                serving.take().expect("checked serving above");
            let mut base = m.clone();
            base.height = folded_tip;
            base.app_hash = node_r.host().app_hash();
            // a boundary at/below its epoch base needs no floor (the
            // fresh epoch starts from its genesis floor — exactly the
            // halted-cutover promotion); past the base, OUR persisted
            // floor cert anchors the replay window.
            let floor = if folded_tip <= base.view_base {
                None
            } else {
                match node_r.sink_mut().floor_cert() {
                    Ok(fc) => fc.filter(|fc| fc.height <= folded_tip),
                    Err(e) => {
                        eprintln!(
                            "[node {label}] FATAL: replica promotion floor read: {e}"
                        );
                        std::process::exit(1);
                    }
                }
            };
            let (sink, folded_host) = node_r.sink_and_host();
            write_boundary_checkpoint(
                sink,
                folded_host,
                &base,
                &floor,
                &label,
                "replica_promotion_checkpoint",
            )
            .await;
            println!(
                "[node {label}] promoted: validator at epoch {} boundary {} — rebooting",
                base.epoch, base.height
            );
            if let Some(cmd) = &reach_cmd {
                let _ = cmd.try_send(reachability::ReachabilityCommand::Shutdown);
                let deadline = std::time::Instant::now() + Duration::from_secs(2);
                while !cmd.is_closed() && std::time::Instant::now() < deadline {
                    context.sleep(Duration::from_millis(20)).await;
                }
            }
            reboot_self();
        }
        // pre-ascension promotion (direct, un-staged admission): the
        // node never folded, so the classic flow stands — sync the
        // served boundary, fabricate its checkpoint, reboot.
        if recovery_slot.is_none() {
            recovery_slot =
                Some(reopen_recovery(&context, &mut recovery_reopens, &label, code_source.clone()).await);
        }
        match sync_all_modules(
            &context,
            &client,
            &m,
            NetworkBindings {
                invite: &namespace,
                identity_chain_id: &identity_chain_id,
            },
            SyncSubstrates {
                forge_repo: &forge_repo,
                duckfs_dir: &duckfs_dir,
                blobs: blobs.clone(),
            },
            attempt,
        )
        .await
        {
            Ok(host) => {
                let latest = match fetch_manifest(&client).await {
                    Ok(latest) => latest,
                    Err(e) => {
                        println!(
                            "[node {label}] synced boundary {} but could not revalidate \
                             latest manifest ({e}); retrying",
                            m.height
                        );
                        continue;
                    }
                };
                let host_hash = host.app_hash();
                diag_log(format!(
                    "DIAG admission_revalidate synced_height={} synced_hash={} \
                     latest_height={} latest_hash={} host_hash={} latest_matches_host={} \
                     latest_floor_present={}",
                    m.height,
                    hex(&m.app_hash),
                    latest.height,
                    hex(&latest.app_hash),
                    hex(&host_hash),
                    latest.app_hash == host_hash,
                    latest.floor_cert.is_some()
                ));
                if let Err(e) = reopen_preflight_synced_host(&host, m.app_hash) {
                    eprintln!("[node {label}] FATAL: promotion preflight failed: {e}");
                    std::process::exit(1);
                }
                match choose_promotion_boundary(host_hash, &latest, &me_bytes) {
                    PromotionBoundary::Promote { boundary, source } => {
                        diag_log(format!(
                            "DIAG promotion_boundary chosen_height={} chosen_hash={} \
                             chosen_floor_present={} source={}",
                            boundary.height,
                            hex(&boundary.app_hash),
                            boundary.floor_cert.is_some(),
                            source.as_str()
                        ));
                        let boundary = boundary.clone();
                        let boundary_floor =
                            match verify_manifest_floor(&namespace, &boundary) {
                                Ok(floor) => floor,
                                Err(e) => {
                                    eprintln!(
                                        "[node {label}] FATAL: promotion floor verify: {e}"
                                    );
                                    std::process::exit(1);
                                }
                            };
                        diag_log(format!(
                            "DIAG suffix_install from={} to={} frames=0",
                            boundary.height, boundary.height
                        ));
                        let floor = boundary_floor.map(|cert| recovery::FloorCert {
                            epoch: boundary.epoch,
                            height: boundary.height,
                            cert,
                        });
                        break (boundary, host, floor);
                    }
                    PromotionBoundary::Retry => {}
                }
                println!(
                    "[node {label}] boundary {} drifted during sync ({} -> latest {}); \
                     discarding scratch and retrying",
                    m.height,
                    hex(&m.app_hash),
                    hex(&latest.app_hash)
                );
            }
            Err(e) => println!("[node {label}] sync at boundary {} failed: {e}", m.height),
        }
    };
    println!("[node {label}] synced app_hash={}", hex(&host.app_hash()));

    // the optional shipped-index warm start rides the same sync
    // connection, staged BEFORE the promotion checkpoint lands: a
    // crash mid-fetch reboots back into joiner mode and refetches,
    // and a torn staging directory is discarded at adoption. the
    // promoted reboot's IndexStore::open adopts what committed here.
    if sync_index {
        stage_shipped_index(&client, boundary.boundary_id(), &storage_for_sync, &label)
            .await;
    }

    // fabricate the checkpoint a restart would have left; the normal
    // recovery boot turns it into a live validator. (a REJOINING key
    // that later resubmits a byte-identical (seq, payload) pair could
    // be dropped by a peer's in-process digest gate; accepted edge
    // until submit sequences ride app state.)
    let mut recovery = recovery_slot
        .take()
        .expect("the journal slot is filled whenever the loop breaks to promote");
    write_boundary_checkpoint(
        &mut recovery,
        &host,
        &boundary,
        &floor,
        &label,
        "promotion_checkpoint",
    )
    .await;
    // tear the pre-warm interface down cleanly before the exec: the
    // in-process boringtun device dies with the process either way,
    // but only an orderly Shutdown unlinks its UAPI socket path —
    // a stale one would fail the rebooted validator's restore-time
    // create. Bounded: the reboot must not hang on a wedged plane —
    // try_send (a plane whose queue is full would never process the
    // Shutdown anyway), then a 2s grace for the orderly unlink.
    if let Some(cmd) = &reach_cmd {
        let _ = cmd.try_send(reachability::ReachabilityCommand::Shutdown);
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while !cmd.is_closed() && std::time::Instant::now() < deadline {
            context.sleep(Duration::from_millis(20)).await;
        }
    }
    println!(
        "[node {label}] promoted: validator at epoch {} boundary {} — rebooting",
        boundary.epoch, boundary.height
    );
    reboot_self();
}
