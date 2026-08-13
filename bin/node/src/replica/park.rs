//! phases 6b–6d of the joiner/replica role: build the serve state (sync
//! client + announce closures + resident relay/announcers/dispatch/oracle
//! pool + optional replica-restart recovery-by-replay), then the park
//! `loop` itself (serve window, drain pass, detection lane, ascension), and
//! finally the promotion checkpoint + [`reboot_self`]. one function on
//! purpose (decision 2 in the plan): the join gate phase (ADR §3.3), the
//! loop's `not_serving` closure, and its mountain of loop-scoped state never
//! leave it, so splitting sub-phases into separate functions would just turn
//! them back into a carrier struct with more steps.

use commonware_codec::DecodeExt as _;
use commonware_consensus::simplex::scheme::ed25519 as simplex_ed25519;
use commonware_cryptography::{Signer as _, ed25519};
use commonware_p2p::authenticated::discovery;
use commonware_p2p::{Manager, Receiver as P2pReceiver};
use commonware_runtime::{Clock, Metrics, Spawner, Supervisor};
use futures::{FutureExt as _, StreamExt as _};
use recovery::{Manifest, Recovery};

use crate::config::{hex_bytes, unhex};
use crate::constants::*;
use crate::drain_actions::{CutoverTrigger, EpochActions};
use crate::explorer::{boundary_block_row, heal_and_backfill_index, heal_index};
use crate::host_reads::{joiner_epoch_mesh, read_valset_members, read_valset_residents};
use crate::host_state::{SyncSubstrates, restore_host, sync_all_modules};
use crate::relay;
use crate::relay_runtime;
use crate::replica;
use crate::rpc::{JoinStateView, RpcJob, RpcReply, RpcRequest, RpcStatus, spawn_rpc_listener};
use crate::sync::catchup::{SuffixCatchupError, catch_up_suffix_frames};
use crate::sync::serve::{
    ServedSeal, reopen_preflight_synced_host, reopen_recovery, replica_backfill,
    replica_orchestrator_at, replica_verifier, verify_manifest_floor, write_boundary_checkpoint,
};
use crate::util::{fatal, hex};
use noded::projection::{BlockProjection, project_block};

use super::promotion::{PromotionBoundary, choose_promotion_boundary, joiner_manifest_fetch_retry};
use super::wiring::ReplicaChannels;
use crate::validator::PromotionBaton;

use sdk::StateRoot;
use statesync::p2p::P2pSyncClient;
use statesync::{fetch_manifest, fetch_tip_coords};
use std::time::Duration;

/// one direct-peer sample off this lane's registry: the exposition parse
/// plus whatever standing the lane can attest — the serving host's valset
/// when one exists, else the announce-target member set alone (a parked
/// joiner has no queryable valset yet, but it knows who the members are).
/// `height` is the served boundary (0 pre-first-sync, like status); a parked
/// lane runs no consensus, so the epoch stays absent.
async fn peers_sample(
    exposition: String,
    host: Option<&host::Host>,
    announce_targets: &[ed25519::PublicKey],
    height: u64,
) -> noded::peers::PeersView {
    let (validators, residents) = replica_roles(host, announce_targets).await;
    noded::peers::peers_from_exposition(&exposition, crate::util::unix_ms(), height, None)
        .with_roles(&validators, &residents)
}

/// the replica's valset standing, hex-keyed: the serving host's committed
/// valset when one exists, else the announce-target member set alone (a
/// parked joiner has no queryable valset yet, but it knows who the members
/// are — and can attest no residents).
async fn replica_roles(
    host: Option<&host::Host>,
    announce_targets: &[ed25519::PublicKey],
) -> (
    std::collections::BTreeSet<String>,
    std::collections::BTreeSet<String>,
) {
    match host {
        Some(host) => (
            read_valset_members(host)
                .await
                .iter()
                .map(|k| hex_bytes(k))
                .collect(),
            read_valset_residents(host)
                .await
                .iter()
                .map(|k| hex_bytes(k))
                .collect(),
        ),
        None => (
            announce_targets.iter().map(|k| hex_bytes(k.as_ref())).collect(),
            std::collections::BTreeSet::new(),
        ),
    }
}

/// the activation cutover's coordinates, lifted OWNED off the orchestrator's
/// respawn plan the moment the fold observes a plan that seats this key —
/// the drain pass stashes these and the loop body seats the baton once the
/// `serving` borrow ends.
struct SeatCoords {
    epoch: u64,
    view_base: u64,
    participants: Vec<Vec<u8>>,
    residents: Vec<Vec<u8>>,
}

/// orderly standby-plane shutdown, then reclaim the reachability lane for
/// the promoted validator's member plane. bounded exactly like the retired
/// pre-exec teardown: a wedged plane (queue full, thread stuck) keeps its
/// lane and the seat proceeds without a member plane rather than hanging
/// the promotion — `None` says so.
async fn shutdown_reach_plane(
    context: &commonware_runtime::tokio::Context,
    label: &str,
    reach_cmd: &Option<tokio::sync::mpsc::Sender<reachability::ReachabilityCommand>>,
    reach_reclaim: Option<crate::reachability_plane::ReachLaneHandback>,
) -> Option<crate::validator::MeshChannel> {
    let Some(cmd) = reach_cmd else { return None };
    let _ = cmd.try_send(reachability::ReachabilityCommand::Shutdown);
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while !cmd.is_closed() && std::time::Instant::now() < deadline {
        context.sleep(Duration::from_millis(20)).await;
    }
    let (tx_handback, rx_handback) = reach_reclaim?;
    // the pumps hand their halves back as they observe the dead plane;
    // the same 2s grace bounds the wait.
    let lanes = futures::future::join(tx_handback, rx_handback).fuse();
    let grace = context.sleep(Duration::from_secs(2)).fuse();
    futures::pin_mut!(lanes, grace);
    futures::select_biased! {
        halves = lanes => match halves {
            (Ok(tx), Ok(rx)) => Some((tx, rx)),
            _ => None,
        },
        _ = grace => {
            tracing::warn!(
                target: "ducktape::reachability",
                node = %label,
                reason = "reach_lane_reclaim_timeout",
                "standby plane kept its lane past shutdown; promoting without a member plane"
            );
            None
        }
    }
}

/// assemble + publish the replica's `/v1/status` snapshot into the shared
/// cell: the served boundary when one exists, the zeroed answer otherwise —
/// pre-first-sync the surface still answers (the app's liveness heartbeat),
/// and a zeroed status is honest: no boundary is served yet. the storage
/// section rides along so index watermarks stay current with the boundary,
/// and the peers standing rides too (the off-lane /v1/peers composition
/// reads it beside the live exposition).
async fn publish_replica_status(
    status: &noded::StatusCell,
    metrics: &noded::NodeMetrics,
    index: &indexer::IndexStore,
    ckpt_height: Option<u64>,
    status_public_key: &str,
    announce_targets: &[ed25519::PublicKey],
    serving: Option<(u64, &host::Host)>,
) {
    metrics.update_storage(
        ckpt_height.unwrap_or_default(),
        index.is_poisoned(),
        MODULE_IDS.iter().map(|module| {
            (
                (*module).to_string(),
                index.applied_height(module).unwrap_or_default(),
            )
        }),
    );
    let (height, root_hash, modules) = match serving {
        Some((height, host)) => (
            height,
            hex(&host.root_hash()),
            MODULE_IDS
                .iter()
                .map(|m| noded::ModuleStatus {
                    id: (*m).into(),
                    root: host.module_root(m).map(|r| hex(&r)).unwrap_or_default(),
                    category: noded::ModuleCategory::of(m),
                })
                .collect(),
        ),
        None => (0, String::new(), Vec::new()),
    };
    status.publish(noded::NodeStatus {
        version: env!("CARGO_PKG_VERSION").into(),
        root_hash,
        height,
        modules,
        public_key: status_public_key.into(),
        operations: metrics.operational_status(),
    });
    // the same standing the retired per-request sample attested; the epoch
    // stays absent — the replica lane runs no consensus.
    let (validators, residents) =
        replica_roles(serving.map(|(_, host)| host), announce_targets).await;
    status.publish_peers(noded::PeersStanding {
        validators,
        residents,
        height,
        epoch: None,
    });
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn park(
    channels: ReplicaChannels,
    oracle: &mut discovery::Oracle<ed25519::PublicKey>,
    signer: ed25519::PrivateKey,
    label: String,
    namespace: Vec<u8>,
    peers: Vec<ed25519::PublicKey>,
    validators: Vec<ed25519::PublicKey>,
    wireguard_listen: Option<std::net::SocketAddr>,
    checkpoint_blocks: u64,
    // what this node ANNOUNCES it can seat. The capacity is the node's to
    // publish; the sandbox that would honour it belongs to the service daemons.
    sync_sources: Vec<ed25519::PublicKey>,
    sync_source: Option<ed25519::PublicKey>,
    status_public_key: String,
    rpc_listener: Option<std::net::TcpListener>,
    http_cmds: futures::channel::mpsc::Receiver<noded::NodeCommand>,
    gateway_requests: Option<tokio::sync::mpsc::Receiver<noded::GatewayJob>>,
    gateway_commands: futures::channel::mpsc::Sender<noded::NodeCommand>,
    session_manager: Option<noded::TerminalSessions>,
    session_requests: tokio::sync::mpsc::Receiver<noded::SessionJob>,
    local_gateway_via: String,
    stream_hub: &noded::StreamHub,
    index: std::sync::Arc<indexer::IndexStore>,
    metrics: noded::NodeMetrics,
    status: noded::StatusCell,
    blobs: noded::blobs::BlobHandle,
    overlay_slot: overlay_net::userspace::StackSlot,
    bulk_pacer: data_plane::BulkPacer,
    planes: data_plane::PlaneMonitor,
    workspace: std::path::PathBuf,
    storage_for_sync: std::path::PathBuf,
    forge_repo: std::path::PathBuf,
    duckfs_dir: std::path::PathBuf,
    manifest: &Option<Manifest>,
    recovery: Recovery<commonware_runtime::tokio::Context>,
) -> crate::validator::PromotionBaton {
    let ReplicaChannels {
        context,
        replica_store,
        lane_bank,
        mut head_wake,
        mut cert_bridge,
        sync_tx,
        sync_rx,
        reach_cmd,
        reach_reclaim,
        mut relay_tx,
        relay_rx,
        admitted,
        voice_requests,
    } = channels;
    metrics.set_role_phase(noded::NodeRole::Resident, noded::NodePhase::Joining);
    tracing::info!(
        target: "ducktape::join",
        event = "node_phase_transition",
        role = "resident",
        phase = "joining",
        node = %label,
        "joining: awaiting redemption"
    );
    let media_peers = if wireguard_listen.is_some() {
        let tracked = crate::voice_plane::MediaPeers::new(
            String::from_utf8(namespace.clone()).expect("namespace is utf-8"),
        );
        tracked.set_peers(peers.iter());
        let me: [u8; 32] = signer
            .public_key()
            .as_ref()
            .try_into()
            .expect("ed25519 keys are 32 bytes");
        crate::voice::spawn_hub(
            voice_requests,
            crate::overlay_book::socket_factory(wireguard_listen.is_some(), &overlay_slot),
            std::sync::Arc::clone(&tracked),
            me,
            planes.clone(),
        );
        crate::agent_plane::spawn(
            label.clone(),
            crate::overlay_book::socket_factory(wireguard_listen.is_some(), &overlay_slot),
            std::sync::Arc::clone(&tracked),
            me,
            bulk_pacer.clone(),
            planes.clone(),
            stream_hub.run_output(),
        );
        // the terminal-session plane: forwards a session's output ring and
        // ordered command log to peers, hosts the directed create/close +
        // creator-gated input control lanes, and drains the guest-side session
        // lane (the client half).
        crate::term_plane::spawn(
            label.clone(),
            crate::overlay_book::socket_factory(wireguard_listen.is_some(), &overlay_slot),
            std::sync::Arc::clone(&tracked),
            me,
            bulk_pacer.clone(),
            planes.clone(),
            stream_hub.terminals(),
            stream_hub.term_commands(),
            session_manager,
            gateway_commands.clone(),
            local_gateway_via,
            workspace.clone(),
            session_requests,
        );
        Some(tracked)
    } else {
        tracing::warn!(
            target: "ducktape::voice",
            node = %label,
            reason = "overlay_unavailable",
            "realtime sessions disabled"
        );
        drop(voice_requests);
        None
    };
    // the announce pump re-reads the grant from this path per tick; the
    // gateway closure below takes ownership of the original.
    // handed to the seat if this node is promoted, so the announce keeps its
    // read-through grant across the transition.
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
                factory: crate::overlay_book::socket_factory(
                    wireguard_listen.is_some(),
                    &overlay_slot,
                ),
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
        let error = "no validator state-sync source is configured";
        metrics.record_sync_failure(error);
        metrics.set_role_phase(noded::NodeRole::Resident, noded::NodePhase::Halted);
        tracing::error!(
            target: "ducktape::statesync",
            event = "node_sync_failed",
            role = "resident",
            node = %label,
            error,
            "FATAL: no validator state-sync source is available"
        );
        std::process::exit(1);
    }
    // the joiner's sync client: the mesh path, ROTATING across every
    // validator that can serve. no unmatched-frame hook: drop-on-miss
    // (the blob fetch lane that consumed those frames is retired). the
    // real-key standing proof (ADR §5.1) is signed ONCE here with the same
    // `signer` the join proof uses: pre-admission the server refuses it (key
    // not in standing), once admitted (key in residents) it serves — the
    // client is oblivious to its own standing; the server decides.
    let (sync_requester, sync_proof) = statesync::sign_sync_proof(&signer, &namespace);
    // the promotion seam: firing `sync_stop` revokes the client's dispatch
    // task, which hands the sync receiver back over `sync_handback` — the
    // promoted validator's serve loop takes the lane over in-process.
    let (sync_stop, sync_handback, sync_reclaim) = statesync::p2p::LaneReclaim::arm();
    let client = P2pSyncClient::with_sources(
        context.child("sync_client"),
        sync_tx,
        sync_rx,
        sync_sources.clone(),
        None,
        sync_requester,
        sync_proof,
        Some(sync_reclaim),
    );

    // the CANDIDATE members the join gate (ADR §3.3) targets — the descriptor's
    // validators (inviter ∪ fronts, as discovered). also the resident relay's
    // targets once standing lands; the manifest poll refreshes it to the tip's
    // current members.
    let mut announce_targets: Vec<ed25519::PublicKey> = validators.clone();

    let me_bytes = signer.public_key().as_ref().to_vec();
    let mut last_tracked = PEER_SET;
    // the epoch the reachability plane last retargeted to (standby
    // role) — one Retarget per observed epoch.
    let mut last_plane_epoch: Option<u64> = None;
    let mut attempt = 0usize;
    // once resident standing is seen, parking is the STEADY state
    // (awaiting a deliberate promote) — the not-admitted bail below
    // must never fire.
    let mut resident_standing = false;

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
        let listen = listener
            .local_addr()
            .map(|a| a.to_string())
            .unwrap_or_default();
        tracing::info!(
            target: "ducktape::node",
            node = %label,
            %listen,
            "rpc listening on {listen}"
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
        node::OrderedNode<consensus::FollowerOrderer, Recovery<commonware_runtime::tokio::Context>>,
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
    // the forge pack sweep: a resident is never a submit-time fanout target, so
    // it is the node most likely to hold a committed head whose objects never
    // arrived. it pulls them through the SAME verified lane as `code_source`
    // above, out of band — see `blob_fetch::sweep_forge_packs`.
    tokio::spawn(crate::blob_fetch::sweep_forge_packs(
        client.clone(),
        blobs.clone(),
        forge_repo.clone(),
        label.clone(),
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
    // fold produced (height -> served (disposition, root_hash)).
    let mut pending_seal_checks: std::collections::HashMap<u64, ServedSeal> =
        std::collections::HashMap::new();
    let mut blocks_since_checkpoint: u64 = 0;
    // the earliest a checkpoint may START again. The replica checkpoints on
    // ITS loop too — the same loop that answers `NodeCommand::Query` — so it
    // owes the same duty bound the validator does (#1018).
    let mut checkpoint_not_before = context.current();
    let mut last_cert_height: Option<u64> = None;
    // the serving replica's manifest-fetch pacer (see the gate at the
    // fetch site). absolute, so per-cert window closes can't starve it.
    let mut next_manifest_fetch = std::time::Instant::now();
    // the replica's valset orchestrator — Some exactly when serving.
    // observe/ceiling/cutover mirror the validator drain; the SWAP
    // exchanges the follower orderer where a validator respawns an
    // engine.
    let mut replica_orchestrator: Option<consensus::ValsetOrchestrator<ed25519::PublicKey>> = None;
    // the last checkpoint's (height, oplog position) — the prune
    // anchor: the journal below it drops once the floor passes it.
    let mut replica_prev_ckpt: (Option<u64>, u64) = (None, 0);
    // the root-hash of the last boundary the derived tier followed:
    // the index feed (heal + explorer row + ws event) fires only when
    // the verified root-hash MOVED. an unchanged hash is an idle
    // stride — state is byte-identical, the read models are already
    // exact, and the explorer stays as quiet as the validator's nop
    // gate keeps it. in-memory on purpose: after a restart the first
    // boundary re-fires and every write below is idempotent.
    let mut last_indexed_root: Option<StateRoot> = None;
    // ---- REPLICA RESTART: recover by journal replay --------------
    //
    // A resident checkpoint is a real recovery base: replay the journal
    // exactly as a validator restart would — restore the checkpoint
    // host, fold the retained suffix, verify the recomposed root-hash
    // — and enter the park loop ALREADY serving at the recovered tip.
    // no re-bootstrap: the fold driver closes any offline gap over
    // the Frames lane the moment the first certificate's parent
    // linkage names it. A checkpoint that seats this key as a validator
    // cannot choose the current role: the key may have been removed and
    // re-granted resident standing while offline. Leave that checkpoint cold
    // so the manifest poll below resolves the latest role before ascending.
    let resident_checkpoint = manifest.as_ref().filter(|ckpt| {
        !ckpt
            .participants
            .iter()
            .any(|key| key.as_slice() == me_bytes.as_slice())
    });
    if let Some(ckpt) = resident_checkpoint {
        let restored = restore_host(
            &context,
            &forge_repo,
            &duckfs_dir,
            ckpt,
            blobs.clone(),
        )
        .await;
        let mut host = match restored {
            Ok(h) => h,
            Err(e) => {
                fatal!(label, "replica checkpoint restore: {e}");
            }
        };
        // heal the derived index against the CHECKPOINT boundary
        // before replay, so the suffix folds land contiguously.
        if let Some(ckpt_height) = ckpt.height {
            heal_index(&index, ckpt_height, &label);
        }
        let mut recovery = recovery_slot
            .take()
            .expect("the journal slot is filled before the first ascension");
        let rec = match recovery.recover_with_sink(&mut host, ckpt, None).await {
            Ok(r) => r,
            Err(e) => {
                fatal!(
                    label,
                    "{e}\n\
                     [node {label}] replica state cannot be locally recovered. wipe \
                     the app-state partitions and re-join — but ALWAYS keep the \
                     consensus journal partitions: they are the anti-equivocation \
                     record for this key."
                );
            }
        };
        // seed the shared store with every retained frame so a
        // re-observed certificate resolves locally instead of
        // wedging the gate awaiting a fetch nobody owes us.
        for frame in &rec.frames {
            replica_store.pin(frame.clone());
        }
        let tip = rec.height.unwrap_or(rec.view_base);
        let root = rec.root_hash;
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
                root_hash: root,
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
        tracing::info!(
            target: "ducktape::recovery",
            node = %label,
            height = tip,
            epoch = rec.epoch,
            replayed = rec.applied,
            already_on_disk = rec.skipped,
            rolled_forward = rec.rolled_forward,
            root_hash = %hex(&root),
            "replica: restart replayed the journal"
        );
        // the e2e / operator serve marker, truthful here too: the
        // node serves a verified boundary — the recovered tip.
        tracing::info!(
            target: "ducktape::statesync",
            node = %label,
            height = tip,
            root_hash = %hex(&root),
            "resident: pre-synced boundary {tip} root_hash={}", hex(&root)
        );
        heal_index(&index, tip, &label);
        last_indexed_root = Some(root);
        serving = Some((tip, node_r));
        metrics.set_role_phase(noded::NodeRole::Resident, noded::NodePhase::Serving);
        publish_replica_status(
            &status,
            &metrics,
            &index,
            replica_prev_ckpt.0,
            &status_public_key,
            &announce_targets,
            serving.as_ref().map(|(h, node_r)| (*h, node_r.host())),
        ).await;
        tracing::info!(
            event = "node_phase_transition",
            role = "resident",
            phase = "serving",
            node = %label,
            height = tip,
            source = "recovery"
        );
        // #1104: the reachability plane's boot Retarget comes from the
        // RECOVERED state, not the manifest poll — the poll needs the p2p
        // mesh, the mesh dials through the tunnels restore() would bring
        // up, and restore() runs only on the first Retarget; without this
        // the restarted resident deadlocks in that cycle. Same standing
        // gate, freshness clock, and non-blocking discipline as the poll
        // below: a shed Retarget is retried there (the epoch latch only
        // advances when the send is taken).
        if let Some(cmd) = &reach_cmd
            && resident_standing
        {
            let clock = rec.view_base.max(tip);
            let members: Vec<ed25519::PublicKey> = rec
                .participants
                .iter()
                .filter_map(|k| ed25519::PublicKey::decode(k.as_slice()).ok())
                .collect();
            let standbys: Vec<ed25519::PublicKey> = rec
                .residents
                .iter()
                .filter_map(|k| ed25519::PublicKey::decode(k.as_slice()).ok())
                .collect();
            let _ = cmd.try_send(reachability::ReachabilityCommand::ViewTick(clock));
            if cmd
                .try_send(reachability::ReachabilityCommand::Retarget(
                    reachability::MeshEpochEvent {
                        epoch: rec.epoch,
                        members,
                        standbys,
                        current_view: clock,
                    },
                ))
                .is_ok()
            {
                last_plane_epoch = Some(rec.epoch);
            }
        }
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
        std::sync::Arc::new(blobs.clone()),
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
    // the SAME announce pump the validator loop drives, adapted to a node that
    // installs boundaries instead of executing blocks: it decides from committed
    // state, latches the submitted frame, and un-latches on a non-applied fate —
    // one copy, so a wedge fixed on one tier cannot survive on the other. There
    // is no resident dispatch pump any more: the
    // compute daemon serves this node's assigned work and its announcements
    // over /v1, on both tiers alike.
    //
    // No podman service here either: each service daemon starts its own under
    // its own root (see the validator boot for the rationale).
    // A resident discovers nothing and executes nothing: the compute daemon
    // does both, and reaches consensus through this node's own /v1 surface —
    // which serves a resident's committed queries and relays its submits
    // exactly as it does for any other local client. The announce is not here
    // either: `service enable`/`disable` submit it, and the liveness watcher
    // (`crate::announce`) retracts it, both over that same /v1 surface. A
    // resident therefore runs no announce pump at all.

    // ── THE JOIN GATE rides first contact now (join ADR §4) ────────────────
    // a fresh TOKENED joiner's sealed intro IS its gate request: the wiring
    // phase's first-contact race announces it to every candidate member's
    // doorbell, and the settled outcome comes back over the same tunnel —
    // `Admitted` sets the shared `admitted` flag (cap persisted, token
    // deleted, by that task), a terminal `Rejected` exits there (R2). the
    // loop below just picks the flag up; the RESTORE path (persisted
    // standing) and the token-less MANUAL path (out-of-band pubkey, admitted
    // by `node resident accept`/`node member promote`) keep their existing detection.
    let (boundary, host, floor) = loop {
        attempt += 1;
        if !resident_standing && admitted.load(std::sync::atomic::Ordering::Acquire) {
            tracing::info!(
                target: "ducktape::join",
                node = %label,
                "admission confirmed by first contact; syncing the boundary"
            );
            resident_standing = true;
        }
        if attempt > 900 && !resident_standing {
            // ~30 minutes of 2s retries: parking forever is operator
            // guidance territory, not a silent spin. (a RESIDENT
            // holds standing indefinitely — that bail is gated off.)
            fatal!(
                label,
                "still no standing after {attempt} attempts — \
                 the invite may be spent or expired, or no member is reachable; \
                 ask for a fresh invite (manual fallback: `ducktape node \
                 resident accept {}`)",
                hex_bytes(&me_bytes)
            );
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
                                    for &m in MODULE_IDS {
                                        if let Some(root) = node_r.host().module_root(m) {
                                            modules.insert(m.to_string(), hex(&root));
                                        }
                                    }
                                    RpcReply {
                                        status: Some(RpcStatus {
                                            height: Some(*height),
                                            root_hash: hex(&node_r.host().root_hash()),
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
                            // the node-owned join state (ADR §6): derived from
                            // the gate outcome + committed chain progress this
                            // loop already holds — never a scattered guess. a
                            // TERMINAL reject exits the process before the loop,
                            // so this arm only ever answers the live states.
                            RpcRequest::JoinState => {
                                let (phase, detail, height) = match &serving {
                                    Some((h, _)) if resident_standing => (
                                        "synced",
                                        "serving reads from a pre-synced boundary",
                                        Some(*h),
                                    ),
                                    _ if resident_standing => {
                                        ("admitted", "standing granted — syncing the boundary", None)
                                    }
                                    _ => ("parked", "awaiting admission through the join gate", None),
                                };
                                RpcReply {
                                    join_state: Some(JoinStateView {
                                        phase: phase.into(),
                                        detail: detail.into(),
                                        height,
                                    }),
                                    ..RpcReply::ok()
                                }
                            }
                            RpcRequest::Peers => RpcReply {
                                peers: Some(
                                    peers_sample(
                                        context.encode(),
                                        serving.as_ref().map(|(_, node_r)| node_r.host()),
                                        &announce_targets,
                                        serving.as_ref().map(|(h, _)| *h).unwrap_or(0),
                                    )
                                    .await,
                                ),
                                ..RpcReply::ok()
                            },
                            RpcRequest::Shutdown => {
                                // a resident writes no checkpoint — nothing to
                                // flush; a restart parks straight back here.
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
                    cmd = http_ingress.next() => {
                        let Some(cmd) = cmd else { continue };
                        match cmd {
                            // `origin` is the caller's CLAIMED submitter — but
                            // this lane signs frames with THIS node's identity
                            // (authorship = status.public_key), so it is ignored.
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
                        }
                    }
                    // a validator's answer for a frame we relayed: match it
                    // to the held caller by frame id and release the reply.
                    // an unknown id (already swept, or a stray) drops.
                    answer = relay_ingress.next() => {
                        let Some((peer, bytes)) = answer else { continue };
                        let Ok(msg) = relay::decode_msg(&bytes) else { continue };
                        // the reply is routed for its SIDE EFFECT — it
                        // releases whatever held caller was waiting on this
                        // frame. Nothing here consumes the outcome any more:
                        // the announce was the only resident-tier pump that
                        // did, and it no longer travels this lane.
                        resident_relay.on_message(peer, msg, &mut relay_tx);
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
                        {
                            metrics.begin_sync(
                                Some(client.current_source().to_string()),
                                replica_view_base.saturating_add(up_to_view),
                            );
                            metrics.set_role_phase(
                                noded::NodeRole::Resident,
                                noded::NodePhase::Syncing,
                            );
                            if let Err(e) = replica_backfill(
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
                                if e.permanent {
                                    // the source pruned the gap: no certificate
                                    // will ever make this range servable again
                                    // (the slept-laptop shape — the chain outran
                                    // the retention window while we were
                                    // suspended). DESCEND, exactly like the
                                    // epoch-cutover branch, so the fallback poll
                                    // re-ascends at a fresh boundary instead of
                                    // retrying the impossible range forever.
                                    tracing::warn!(
                                        target: "ducktape::statesync",
                                        node = %label,
                                        after_view,
                                        up_to_view,
                                        error = %e.detail,
                                        reason = "range_pruned",
                                        "replica backfill pruned; re-syncing at a fresh boundary"
                                    );
                                    serving = None;
                                    publish_replica_status(
                                        &status,
                                        &metrics,
                                        &index,
                                        replica_prev_ckpt.0,
                                        &status_public_key,
                                        &announce_targets,
                                        None,
                                    ).await;
                                    metrics.record_sync_failure(e.detail.clone());
                                    replica_scheme = None;
                                    replica_orchestrator = None;
                                    recovery_slot = Some(
                                        reopen_recovery(
                                            &context,
                                            &mut recovery_reopens,
                                            &label,
                                            code_source.clone(),
                                        )
                                        .await,
                                    );
                                    break;
                                }
                                metrics.record_sync_retry(e.detail.clone());
                                metrics.set_role_phase(
                                    noded::NodeRole::Resident,
                                    noded::NodePhase::Serving,
                                );
                                tracing::debug!(
                                    target: "ducktape::statesync",
                                    node = %label,
                                    after_view,
                                    up_to_view,
                                    error = %e.detail,
                                    "replica backfill unavailable; retrying on the next certificate"
                                );
                                break;
                            }
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
                                metrics.begin_sync(
                                    Some(client.current_source().to_string()),
                                    replica_view_base.saturating_add(view),
                                );
                                metrics.set_role_phase(
                                    noded::NodeRole::Resident,
                                    noded::NodePhase::Syncing,
                                );
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
                                    if e.permanent {
                                        // same escalation as the gap branch
                                        // above: a pruned range cannot heal
                                        // by waiting.
                                        tracing::warn!(
                                            target: "ducktape::statesync",
                                            node = %label,
                                            view,
                                            error = %e.detail,
                                            reason = "range_pruned",
                                            "unresolvable replica view pruned; re-syncing at a \
                                             fresh boundary"
                                        );
                                        serving = None;
                                        publish_replica_status(
                                            &status,
                                            &metrics,
                                            &index,
                                            replica_prev_ckpt.0,
                                            &status_public_key,
                                            &announce_targets,
                                            None,
                                        ).await;
                                        metrics.record_sync_failure(e.detail.clone());
                                        metrics.set_role_phase(
                                            noded::NodeRole::Resident,
                                            noded::NodePhase::Syncing,
                                        );
                                        replica_scheme = None;
                                        replica_orchestrator = None;
                                        recovery_slot = Some(
                                            reopen_recovery(
                                                &context,
                                                &mut recovery_reopens,
                                                &label,
                                                code_source.clone(),
                                            )
                                            .await,
                                        );
                                        break;
                                    }
                                    metrics.record_sync_retry(e.detail.clone());
                                    metrics.set_role_phase(
                                        noded::NodeRole::Resident,
                                        noded::NodePhase::Serving,
                                    );
                                    tracing::debug!(
                                        target: "ducktape::statesync",
                                        node = %label,
                                        view,
                                        error = %e.detail,
                                        "unresolvable replica view backfill failed; retrying on \
                                         the next certificate"
                                    );
                                }
                                break;
                            }
                            Err(e) => {
                                // quorum verification failed: a lying
                                // certificate source. drop it loudly.
                                tracing::warn!(
                                    target: "ducktape::consensus",
                                    node = %label,
                                    error = %e,
                                    reason = "certificate_invalid",
                                    "replica certificate refused"
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
        // the activation cutover's seat coords, stashed by the drain pass
        // below when the fold observes a respawn plan that seats this key;
        // the promotion block after the pass consumes them.
        let mut seat_plan: Option<SeatCoords> = None;
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
                fatal!(label, "replica fold: {e}");
            }
            let drained = node_r.take_drained();
            // The same projection the validator consumes; this loop retains
            // replica-only seal verification, streaming, and checkpoints.
            for projection in project_block(&drained, node_r.take_system_dispatches(), &blobs) {
                let BlockProjection {
                    height,
                    dispatches,
                    record,
                    sealed_hash,
                    applied,
                    latency_us,
                    applied_ops,
                    rejected_ops,
                    ..
                } = projection;
                if applied {
                    metrics.record_block(height, latency_us, &dispatches);
                } else {
                    metrics.record_height(height);
                }
                metrics.record_op_outcomes(applied_ops, rejected_ops);
                // a BACKFILLED height's trust is the served seal:
                // what our fold produced must match it exactly, or
                // this replica has diverged from the quorum's fold.
                if let Some((_, served_hash, served_roots)) = pending_seal_checks.remove(&height)
                    && sealed_hash.is_some_and(|h| h != served_hash)
                {
                    // name the diverging module(s) — the one lead an
                    // operator (or the next debugger) needs first.
                    for (module, served_root) in &served_roots {
                        let ours = node_r.host().module_root(module);
                        if ours.as_ref() != Some(served_root) {
                            tracing::error!(
                                target: "ducktape::consensus",
                                node = %label,
                                module,
                                served = %hex(served_root),
                                ours = %ours.map(|r| hex(&r)).unwrap_or_else(|| "none".into()),
                                "replica module diverged"
                            );
                        }
                    }
                    fatal!(
                        label,
                        "backfilled height {height} folded to \
                         {} but the quorum sealed {} — state diverged",
                        hex(&sealed_hash.expect("checked above")),
                        hex(&served_hash)
                    );
                }
                let ops = indexer::BlockOps {
                    record,
                    ..noded::index_block_ops(height, height, &dispatches)
                };
                if let Err(err) = index.apply_block(&ops) {
                    tracing::error!(
                        target: "ducktape::modules",
                        event = "node_index_poisoned",
                        node = %label,
                        role = "resident",
                        height,
                        error = %err,
                        "replica index apply failed; wipe <storage>/index to rebuild"
                    );
                }
                if let Some(root) = sealed_hash {
                    stream_hub.publish_block(
                        height,
                        hex(&root),
                        noded::BlockWake::from_dispatches(&dispatches),
                    );
                    last_indexed_root = Some(root);
                }
                *served_height = height;
                blocks_since_checkpoint += 1;
            }
            if !drained.is_empty()
                && metrics.operational_status().phase == noded::NodePhase::Syncing
            {
                metrics.record_sync_progress(*served_height);
                metrics.set_role_phase(noded::NodeRole::Resident, noded::NodePhase::Serving);
            }
            // the boundary this pass folded is visible NOW on /v1/status.
            if !drained.is_empty() {
                publish_replica_status(
                    &status,
                    &metrics,
                    &index,
                    replica_prev_ckpt.0,
                    &status_public_key,
                    &announce_targets,
                    Some((*served_height, node_r.host())),
                ).await;
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
                    tracing::info!(
                        target: "ducktape::consensus",
                        node = %label,
                        observed_view = cutover.observed_view(),
                        next_epoch = cutover.next_epoch(),
                        cutover_view = cutover.cutover_view(),
                        "replica membership change observed"
                    );
                    node_r.set_view_ceiling(cutover.cutover_view());
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
                    let plan_resident_bytes: Vec<Vec<u8>> = plan_residents
                        .iter()
                        .map(|k| k.as_ref().to_vec())
                        .collect();
                    // THE ACTIVATION CUTOVER THAT SEATS THIS KEY: promotion
                    // is decided HERE, off this node's own folded state —
                    // never by polling members, who may already be halted
                    // awaiting this very node's votes. stash the coords and
                    // let the promotion block after the drain pass take the
                    // node once this borrow ends.
                    let seat_is_mine = members.contains(&signer.public_key());
                    if seat_is_mine {
                        seat_plan = Some(SeatCoords {
                            epoch: plan.epoch(),
                            view_base: plan.cutover_app_height(),
                            participants: member_bytes,
                            residents: plan_resident_bytes,
                        });
                    } else {
                        // transport first, exactly like the validator:
                        // the new epoch's mesh must admit its members.
                        let mesh =
                            joiner_epoch_mesh(&peers, &member_bytes, &plan_resident_bytes);
                        oracle.track(plan.epoch(), mesh);
                        if let Some(book) = &gateway_book {
                            book.set_peers(plan.valset().transport_members().iter());
                        }
                        if let Some(peers) = &media_peers {
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
                            fatal!(label, "replica cutover journal write: {e}");
                        }
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
                        // ...and CLEAR the duty cooldown with it. The force
                        // says "a restart must land on the new boundary", and
                        // an unpaid cooldown from the previous checkpoint would
                        // otherwise hold this one off for minutes.
                        checkpoint_not_before = context.current();
                        tracing::info!(
                            target: "ducktape::consensus",
                            node = %label,
                            epoch = plan.epoch(),
                            base_height = plan.cutover_app_height(),
                            "replica epoch cutover; follower swapped in-loop"
                        );
                    }
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
                        Err(e) => tracing::warn!(
                            target: "ducktape::recovery",
                            node = %label,
                            height,
                            error = %e,
                            "replica floor cert write failed; retrying"
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
            if crate::drain_actions::checkpoint_due(
                blocks_since_checkpoint,
                checkpoint_blocks,
                context.current(),
                checkpoint_not_before,
            ) && let Some(f) = node_r.finalized()
            {
                let pos = node_r.sink_mut().oplog_pos().await;
                let checkpoint_started = context.current();
                let members = read_valset_members(node_r.host()).await;
                let residents = read_valset_residents(node_r.host()).await;
                // the capture's OWN window: the two valset reads above are host
                // queries that run module execution, and charging them to
                // `capture_ms` would put time in the stage that the per-module
                // breakdown cannot account for. `checkpoint_started` still spans
                // them for the cooldown — they block the loop too.
                let capture_started = context.current();
                // TIMED, exactly like the validator's periodic checkpoint: this
                // capture blocks the replica's own select loop, so its per-module
                // cost is the same diagnosis (#1018) and must not be visible in
                // only one of the two roles.
                let captured = Manifest::capture_timed(
                    node_r.host(),
                    Some(f.height),
                    replica_epoch,
                    replica_view_base,
                    members,
                    residents,
                    None,
                    pos,
                    1,
                    || {
                        context
                            .current()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                    },
                );
                let captured_at = context.current();
                match captured {
                    Ok((ckpt, capture_cost)) => match node_r.sink_mut().write_manifest(&ckpt).await
                    {
                        Ok(()) => {
                            let written_at = context.current();
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
                                && let Err(e) =
                                    node_r.sink_mut().prune_oplog(replica_prev_ckpt.1).await
                            {
                                tracing::warn!(
                                    target: "ducktape::recovery",
                                    node = %label,
                                    error = %e,
                                    "replica oplog prune failed"
                                );
                            }
                            replica_prev_ckpt = (ckpt.height, pos);
                            blocks_since_checkpoint = 0;
                            let since = |a: std::time::SystemTime, b: std::time::SystemTime| {
                                b.duration_since(a).unwrap_or_default().as_millis()
                            };
                            let done_at = context.current();
                            tracing::info!(
                                target: "ducktape::recovery",
                                event = "node_checkpoint_written",
                                node = %label,
                                height = ckpt.height.unwrap_or_default(),
                                capture_ms = since(capture_started, captured_at),
                                write_ms = since(captured_at, written_at),
                                prune_ms = since(written_at, done_at),
                                capture_modules = %crate::drain_actions::capture_breakdown(&capture_cost)
                            );
                        }
                        Err(e) => tracing::warn!(
                            target: "ducktape::recovery",
                            node = %label,
                            error = %e,
                            "replica checkpoint write failed; retrying"
                        ),
                    },
                    Err(e) => tracing::warn!(
                        target: "ducktape::recovery",
                        node = %label,
                        error = %e,
                        "replica checkpoint capture failed; retrying"
                    ),
                }
                // OUTSIDE THE MATCH: a capture that fails costs this loop
                // everything a successful one does, and neither failure arm
                // resets `blocks_since_checkpoint` — so without the cooldown
                // the retry is immediate and the node re-pays the full cost on
                // every pass, forever.
                let attempt = context
                    .current()
                    .duration_since(checkpoint_started)
                    .unwrap_or_default();
                checkpoint_not_before =
                    crate::drain_actions::cooldown_until(context.current(), attempt);
            }
        }
        // ---- THE PROMOTION SEAT (in-process) ---------------------
        //
        // the fold observed the activation cutover that seats this key:
        // journal the cutover, checkpoint the folded state as the validator
        // boot base, reclaim the lanes the parked role owned, and hand the
        // baton up — `run_node` continues on the validator path inside this
        // same process. a quorum-widening cutover HALTS the members awaiting
        // this very node's votes, so nothing here may fetch from them.
        if let Some(seat) = seat_plan {
            let (folded_tip, mut node_r) =
                serving.take().expect("a seat plan only forms while serving");
            metrics.set_role_phase(noded::NodeRole::Resident, noded::NodePhase::Draining);
            tracing::info!(
                event = "node_phase_transition",
                role = "resident",
                phase = "draining",
                node = %label,
                height = folded_tip,
                reason = "promotion"
            );
            // the cutover journal record first (the crash window), exactly
            // the follower swap's write — the engine seat replaces this
            // placeholder orderer at the validator entry, so it is never
            // polled.
            let follower = consensus::FollowerOrderer::new(replica_store.clone());
            if let Err(e) = node_r
                .cutover(
                    follower,
                    seat.epoch,
                    seat.view_base,
                    &seat.participants,
                    &seat.residents,
                )
                .await
            {
                fatal!(label, "promotion cutover journal write: {e}");
            }
            let root_hash = node_r.host().root_hash();
            let (host, mut recovery) = node_r.into_parts();
            // the promotion checkpoint: the manifest seats this key, so a
            // crash from here re-enters role resolution with a valid state base.
            // (next_seq stays 1 — the fabricated-checkpoint rejoin edge,
            // accepted until submit sequences ride app state.)
            let pos = recovery.oplog_pos().await;
            let ckpt = match Manifest::capture(
                &host,
                Some(folded_tip),
                seat.epoch,
                seat.view_base,
                seat.participants.clone(),
                seat.residents.clone(),
                None,
                pos,
                1,
            ) {
                Ok(ckpt) => ckpt,
                Err(e) => {
                    fatal!(label, "promotion checkpoint capture: {e}");
                }
            };
            if let Err(e) = recovery.write_manifest(&ckpt).await {
                fatal!(label, "promotion checkpoint write: {e}");
            }
            // the parked role's lanes: swap the journal's code source off
            // the sync client (the local blob store covers the gap until
            // the validator wiring installs its serve-lane source), revoke
            // the client's dispatch task and take the sync lane back, and
            // shut the standby reachability plane down for its lane — the
            // member plane rewires over it.
            recovery.set_code_source(std::sync::Arc::new(crate::host_state::BlobCodeSource(
                std::sync::Arc::new(blobs.clone()),
            )));
            let _ = sync_stop.send(());
            let sync_rx = sync_handback
                .await
                .expect("the revoked sync dispatch task hands its lane back");
            let sync_tx = client.lane_sender();
            let reach_lane =
                shutdown_reach_plane(&context, &label, &reach_cmd, reach_reclaim).await;
            tracing::info!(
                target: "ducktape::join",
                node = %label,
                epoch = seat.epoch,
                height = folded_tip,
                "promoted: validator at epoch {} boundary {}; seating in-process",
                seat.epoch,
                folded_tip
            );
            return PromotionBaton {
                context,
                host,
                recovery,
                epoch: seat.epoch,
                view_base: seat.view_base,
                height: folded_tip,
                root_hash,
                participants: seat.participants,
                residents: seat.residents,
                // the seat boundary IS the fresh epoch's base — its floor
                // is the epoch genesis floor, exactly a validator cutover.
                floor: None,
                lane_bank,
                sync_tx,
                sync_rx,
                relay_tx,
                relay_ingress,
                reach_lane,
                media_peers,
                gateway_book,
                rpc_ingress,
                http_ingress,
                prev_ckpt: (Some(folded_tip), pos),
            };
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
                // this poll runs POST-admission (the gate already granted
                // standing, or this is the manual/restore path); a fetch miss
                // just retries on the next tick — no re-announce, the gate is
                // done.
                let retry = joiner_manifest_fetch_retry(&label, resident_standing, &e);
                metrics.record_sync_retry(e.to_string());
                tracing::debug!(
                    target: "ducktape::statesync",
                    attempts = attempt,
                    announce = retry.announce,
                    "{}", retry.log_line
                );
                continue;
            }
        };
        // follow the mesh rotation while parked. the participant
        // list is an unverified serving hint — the union with the
        // descriptor mesh keeps the real members reachable, and
        // promotion re-derives everything from verified state.
        if tip.epoch > last_tracked {
            if !lane_bank.covers(tip.epoch) {
                tracing::warn!(
                    target: "ducktape::reachability",
                    node = %label,
                    epoch = tip.epoch,
                    channel_bank = EPOCH_CHANNEL_BANK,
                    reason = "epoch_outside_channel_bank",
                    "expect reconnect churn while parked"
                );
            }
            let mesh = joiner_epoch_mesh(&peers, &tip.participants, &tip.residents);
            oracle.track(tip.epoch, mesh);
            if gateway_book.is_some() || media_peers.is_some() {
                let transport: Vec<ed25519::PublicKey> = tip
                    .participants
                    .iter()
                    .chain(tip.residents.iter())
                    .filter_map(|key| ed25519::PublicKey::decode(key.as_slice()).ok())
                    .collect();
                if let Some(book) = &gateway_book {
                    book.set_peers(transport.iter());
                }
                if let Some(peers) = &media_peers {
                    peers.set_peers(transport.iter());
                }
            }
            last_tracked = tip.epoch;
        }
        // drive the reachability plane's standby role off the
        // manifest: membership and resident standing come from the
        // synced boundary, whose height doubles as the plane's
        // freshness clock (the same app-height regime the members'
        // ViewTicks run — it bounds handshake-message freshness;
        // records themselves carry no TTL).
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
        let member_in_tip = tip.participants.iter().any(|k| k == &me_bytes);
        if serving.is_some() && tip.epoch > replica_epoch {
            // the network cut over past our folded epoch while we serve:
            // the follower's verifier and fetch lane are the old epoch's,
            // so its certs stopped verifying here. DESCEND — drop the node
            // (journal checkpointed on cadence), reopen the journal handle
            // — and re-ascend at the new epoch's boundary below. a MEMBER
            // here means the fold missed the seat cutover's own tail (a
            // shed final certificate has no successor to re-anchor it on a
            // halted chain): it descends the same way and the cold
            // admission sync below seats it from the frozen boundary.
            tracing::info!(
                target: "ducktape::consensus",
                node = %label,
                from_epoch = replica_epoch,
                to_epoch = tip.epoch,
                "replica epoch cutover; re-ascending"
            );
            serving = None;
            publish_replica_status(
                &status,
                &metrics,
                &index,
                replica_prev_ckpt.0,
                &status_public_key,
                &announce_targets,
                None,
            ).await;
            metrics.set_role_phase(noded::NodeRole::Resident, noded::NodePhase::Syncing);
            replica_scheme = None;
            replica_orchestrator = None;
            recovery_slot =
                Some(reopen_recovery(&context, &mut recovery_reopens, &label, code_source.clone()).await);
        }
        if !member_in_tip {
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
            if tip.residents.iter().any(|k| k == &me_bytes) {
                if !resident_standing {
                    resident_standing = true;
                    tracing::info!(
                        target: "ducktape::join",
                        node = %label,
                        "resident: standing granted; following boundaries and serving local \
                         reads"
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
                            metrics.record_sync_retry(e.to_string());
                            let retry = joiner_manifest_fetch_retry(&label, resident_standing, &e);
                            tracing::debug!(
                                target: "ducktape::statesync",
                                attempts = attempt,
                                announce = retry.announce,
                                "{}", retry.log_line
                            );
                            continue;
                        }
                    };
                    tracing::info!(
                        target: "ducktape::statesync",
                        node = %label,
                        height = m.height,
                        modules = m.entries.len(),
                        "replica: bootstrapping at boundary {} ({} modules)",
                        m.height,
                        m.entries.len()
                    );
                    metrics.begin_sync(Some(client.current_source().to_string()), m.height);
                    metrics.set_role_phase(noded::NodeRole::Resident, noded::NodePhase::Syncing);
                    tracing::info!(
                        event = "node_phase_transition",
                        role = "resident",
                        phase = "syncing",
                        node = %label,
                        target_height = m.height
                    );
                    match sync_all_modules(
                        &context,
                        &client,
                        &m,
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
                                    metrics.record_sync_retry(e.to_string());
                                    tracing::debug!(
                                        target: "ducktape::statesync",
                                        node = %label,
                                        height = m.height,
                                        error = %e,
                                        "replica boundary floor refused; retrying"
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
                            let caught = match catch_up_suffix_frames(
                                &client,
                                &mut recovery,
                                &mut host,
                                None,
                                m.height,
                                SUFFIX_CATCHUP_MAX_ITERS,
                            )
                            .await
                            {
                                Ok(c) => c,
                                Err(SuffixCatchupError::Fatal(e)) => {
                                    metrics.record_sync_failure(e.clone());
                                    metrics.set_role_phase(
                                        noded::NodeRole::Resident,
                                        noded::NodePhase::Halted,
                                    );
                                    tracing::error!(
                                        event = "node_sync_failed",
                                        role = "resident",
                                        node = %label,
                                        stage = "suffix_fold",
                                        error = %e
                                    );
                                    fatal!(label, "replica suffix fold: {e}");
                                }
                                Err(SuffixCatchupError::Retry(e)) => {
                                    metrics.record_sync_retry(e.clone());
                                    tracing::warn!(
                                        target: "ducktape::statesync",
                                        node = %label,
                                        height = m.height,
                                        error = %e,
                                        "replica suffix fold unavailable; re-bootstrapping"
                                    );
                                    recovery_slot = Some(recovery);
                                    continue;
                                }
                            };
                            let tip = caught.to_height.max(m.height);
                            metrics.begin_sync(Some(client.current_source().to_string()), tip);
                            metrics.record_sync_progress(tip);
                            // seed the shared store with the folded
                            // suffix: peers' resolvers can fetch these
                            // from us, and a re-reported cert for a
                            // just-folded height resolves locally.
                            for bytes in &caught.frame_bytes {
                                replica_store.put(bytes.clone());
                            }
                            let root = host.root_hash();
                            // the fold pipeline: the follower orderer
                            // in the engine's seat of the SAME
                            // OrderedNode a validator drains, this
                            // journal as its sink. resolver-less by
                            // design (see the lane wiring above): a
                            // store miss surfaces as Unresolvable and
                            // the driver backfills over the Frames
                            // lane.
                            let follower = consensus::FollowerOrderer::new(replica_store.clone());
                            let code_source = recovery.code_source();
                            let mut node_r = node::OrderedNode::resume(
                                host,
                                follower,
                                recovery,
                                Some(host::FinalizedBlock {
                                    height: tip,
                                    root_hash: root,
                                }),
                                m.view_base,
                            );
                            node_r.set_code_source(code_source);
                            replica_scheme = Some(replica_verifier(&namespace, &m.participants));
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
                            tracing::info!(
                                target: "ducktape::statesync",
                                node = %label,
                                height = tip,
                                root_hash = %hex(&root),
                                "resident: pre-synced boundary {tip} root_hash={}", hex(&root)
                            );
                            tracing::info!(
                                target: "ducktape::consensus",
                                node = %label,
                                height = tip,
                                epoch = m.epoch,
                                root_hash = %hex(&root),
                                "replica: following the head from {tip}"
                            );
                            // the derived tier starts exact at the
                            // ascension tip; per-block folds keep it
                            // current from here (no more healing).
                            if last_indexed_root.as_ref() != Some(&root) {
                                // the stamp, then the SOURCE'S OWN op rows
                                // below it — inline, while this node is not
                                // yet serving and not yet folding live
                                // blocks. that window is the whole
                                // correctness argument for writing straight
                                // into the feed (see
                                // `heal_and_backfill_index`), and it closes
                                // at `serving = Some(..)` below.
                                heal_and_backfill_index(&index, &client, tip, &label).await;
                                if let Err(err) = index.apply_block_record(
                                    tip,
                                    boundary_block_row(tip, &root),
                                ) {
                                    tracing::warn!(
                                        target: "ducktape::modules",
                                        node = %label,
                                        height = tip,
                                        error = %err,
                                        "replica explorer row refused"
                                    );
                                }
                                // THE HEAL REWOUND FLOORS, so this wake must
                                // reach the index topics even though it carries
                                // no dispatches of its own: `heal_index` above
                                // wiped module dbs and stamped backfill floors,
                                // and the `lagged` frame a subscriber is owed
                                // comes only from the scan this wakes.
                                stream_hub.publish_block(
                                    tip,
                                    hex(&root),
                                    noded::BlockWake::IndexChanged,
                                );
                                last_indexed_root = Some(root);
                            }
                            serving = Some((tip, node_r));
                            metrics.set_role_phase(
                                noded::NodeRole::Resident,
                                noded::NodePhase::Serving,
                            );
                            publish_replica_status(
                                &status,
                                &metrics,
                                &index,
                                replica_prev_ckpt.0,
                                &status_public_key,
                                &announce_targets,
                                serving.as_ref().map(|(h, node_r)| (*h, node_r.host())),
                            ).await;
                            tracing::info!(
                                event = "node_phase_transition",
                                role = "resident",
                                phase = "serving",
                                node = %label,
                                height = tip
                            );
                        }
                        Err(e) => {
                            metrics.record_sync_failure(e.to_string());
                            tracing::warn!(
                                target: "ducktape::statesync",
                                event = "node_sync_failed",
                                role = "resident",
                                node = %label,
                                target_height = m.height,
                                error = %e,
                                "replica bootstrap failed"
                            );
                        }
                    }
                }
                continue;
            }
            // the token-less MANUAL / restore path polling for an out-of-band
            // grant (`node resident accept`/`node member promote`): the tokened join gate already
            // ran to completion above, so there is nothing to re-announce here
            // — just keep polling for the grant to land.
            metrics.set_role_phase(noded::NodeRole::Resident, noded::NodePhase::Joining);
            continue;
        }
        // in the epoch set while STILL FOLDING toward the seat cutover:
        // the drain pass seats this node from its own folded state the
        // moment its plan lands — nothing to fetch from members who may
        // already be halted awaiting these very votes. (a fold wedged
        // BEHIND the cutover descends above and re-enters cold.)
        if serving.is_some() {
            continue;
        }
        // in the epoch set, COLD (direct, un-staged admission — the node
        // never folded): promotion consumes the boundary itself — module
        // entries and the real floor certificate — so it rides the full
        // Manifest lane from here.
        let m = match fetch_manifest(&client).await {
            Ok(m) => m,
            Err(e) => {
                metrics.record_sync_retry(e.to_string());
                let retry = joiner_manifest_fetch_retry(&label, resident_standing, &e);
                tracing::debug!(
                    target: "ducktape::statesync",
                    attempts = attempt,
                    announce = retry.announce,
                    "{}", retry.log_line
                );
                continue;
            }
        };
        // a boundary PAST the epoch base needs its
        // finalization floor served alongside, or the respawned
        // engine would re-deliver history the synced state already
        // contains — retry until the source's floor catches up.
        if m.height > m.view_base && m.floor_cert.is_none() {
            tracing::debug!(
                target: "ducktape::statesync",
                node = %label,
                height = m.height,
                "admitted boundary lacks its finalization floor; retrying"
            );
            continue;
        }
        tracing::info!(
            target: "ducktape::join",
            node = %label,
            epoch = m.epoch,
            height = m.height,
            modules = m.entries.len(),
            "admitted at epoch {} boundary {}; syncing {} modules",
            m.epoch,
            m.height,
            m.entries.len()
        );
        // the classic cold flow: sync the served (frozen) boundary,
        // fabricate its checkpoint, and seat from it.
        if recovery_slot.is_none() {
            recovery_slot = Some(
                reopen_recovery(&context, &mut recovery_reopens, &label, code_source.clone()).await,
            );
        }
        metrics.begin_sync(Some(client.current_source().to_string()), m.height);
        metrics.set_role_phase(noded::NodeRole::Resident, noded::NodePhase::Syncing);
        tracing::info!(
            event = "node_phase_transition",
            role = "resident",
            phase = "syncing",
            node = %label,
            target_height = m.height,
            reason = "promotion"
        );
        match sync_all_modules(
            &context,
            &client,
            &m,
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
                metrics.begin_sync(Some(client.current_source().to_string()), m.height);
                metrics.record_sync_progress(m.height);
                let latest = match fetch_manifest(&client).await {
                    Ok(latest) => latest,
                    Err(e) => {
                        metrics.record_sync_retry(e.to_string());
                        tracing::debug!(
                            target: "ducktape::join",
                            node = %label,
                            height = m.height,
                            error = %e,
                            "synced boundary could not revalidate the latest manifest; retrying"
                        );
                        continue;
                    }
                };
                let host_hash = host.root_hash();
                tracing::debug!(
                    target: "ducktape::join",
                    synced_height = m.height,
                    synced_hash = %hex(&m.root_hash),
                    latest_height = latest.height,
                    latest_hash = %hex(&latest.root_hash),
                    host_hash = %hex(&host_hash),
                    latest_matches_host = latest.root_hash == host_hash,
                    latest_floor_present = latest.floor_cert.is_some(),
                    "admission revalidate"
                );
                if let Err(e) = reopen_preflight_synced_host(&host, m.root_hash) {
                    fatal!(label, "promotion preflight failed: {e}");
                }
                match choose_promotion_boundary(host_hash, &latest, &me_bytes) {
                    PromotionBoundary::Promote { boundary, source } => {
                        tracing::debug!(
                            target: "ducktape::join",
                            chosen_height = boundary.height,
                            chosen_hash = %hex(&boundary.root_hash),
                            chosen_floor_present = boundary.floor_cert.is_some(),
                            source = %source.as_str(),
                            "promotion boundary chosen"
                        );
                        let boundary = boundary.clone();
                        let boundary_floor = match verify_manifest_floor(&namespace, &boundary) {
                                Ok(floor) => floor,
                                Err(e) => {
                                    fatal!(label, "promotion floor verify: {e}");
                                }
                            };
                        tracing::debug!(
                            target: "ducktape::join",
                            from = boundary.height,
                            to = boundary.height,
                            frames = 0,
                            "suffix install"
                        );
                        let floor = boundary_floor.map(|cert| recovery::FloorCert {
                            epoch: boundary.epoch,
                            height: boundary.height,
                            cert,
                        });
                        // THE LAST MOMENT A SYNC CLIENT EXISTS: `run_promoted`
                        // seats from the baton and never sees one, so the
                        // op-row backfill has to run here. it stamps first
                        // (the wipe would eat anything written before it) at
                        // exactly the boundary `run_promoted` heals against,
                        // which makes that later heal the no-op it should be.
                        heal_and_backfill_index(&index, &client, boundary.height, &label).await;
                        break (boundary, host, floor);
                    }
                    PromotionBoundary::Retry => {}
                }
                tracing::debug!(
                    target: "ducktape::join",
                    node = %label,
                    height = m.height,
                    synced_hash = %hex(&m.root_hash),
                    latest_hash = %hex(&latest.root_hash),
                    "boundary drifted during sync; discarding scratch and retrying"
                );
            }
            Err(e) => {
                metrics.record_sync_failure(e.to_string());
                tracing::warn!(
                    target: "ducktape::statesync",
                    event = "node_sync_failed",
                    role = "resident",
                    node = %label,
                    target_height = m.height,
                    error = %e,
                    "resident boundary sync failed"
                );
            }
        }
    };
    tracing::info!(
        target: "ducktape::statesync",
        "node={label} synced root_hash={}", hex(&host.root_hash())
    );

    // fabricate the checkpoint a restart would have left; a crash from
    // here re-enters role resolution with a valid state base. (a REJOINING key
    // that later resubmits a byte-identical (seq, payload) pair could
    // be dropped by a peer's in-process digest gate; accepted edge
    // until submit sequences ride app state.)
    let mut recovery = recovery_slot
        .take()
        .expect("the journal slot is filled whenever the loop breaks to promote");
    let pos = write_boundary_checkpoint(
        &mut recovery,
        &host,
        &boundary,
        &floor,
        &label,
        "promotion_checkpoint",
    )
    .await;
    metrics.set_role_phase(noded::NodeRole::Resident, noded::NodePhase::Draining);
    tracing::info!(
        event = "node_phase_transition",
        role = "resident",
        phase = "draining",
        node = %label,
        height = boundary.height,
        reason = "promotion"
    );
    // the parked role's lanes, exactly the warm seat's teardown: local
    // blob store covers code-registry reads until the validator wiring
    // installs its serve-lane source; the sync dispatch task hands the
    // sync lane back; the standby reachability plane shuts down for its
    // lane — the member plane rewires over the persisted mesh, so the
    // seat starts connected instead of assembling.
    recovery.set_code_source(std::sync::Arc::new(crate::host_state::BlobCodeSource(
        std::sync::Arc::new(blobs.clone()),
    )));
    let _ = sync_stop.send(());
    let sync_rx = sync_handback
        .await
        .expect("the revoked sync dispatch task hands its lane back");
    let sync_tx = client.lane_sender();
    let reach_lane = shutdown_reach_plane(&context, &label, &reach_cmd, reach_reclaim).await;
    tracing::info!(
        target: "ducktape::join",
        node = %label,
        epoch = boundary.epoch,
        height = boundary.height,
        "promoted: validator at epoch {} boundary {}; seating in-process",
        boundary.epoch,
        boundary.height
    );
    let root_hash = host.root_hash();
    PromotionBaton {
        context,
        host,
        recovery,
        epoch: boundary.epoch,
        view_base: boundary.view_base,
        height: boundary.height,
        root_hash,
        participants: boundary.participants.clone(),
        residents: boundary.residents.clone(),
        floor,
        lane_bank,
        sync_tx,
        sync_rx,
        relay_tx,
        relay_ingress,
        reach_lane,
        media_peers,
        gateway_book,
        rpc_ingress,
        http_ingress,
        prev_ckpt: (Some(boundary.height), pos),
    }
}
