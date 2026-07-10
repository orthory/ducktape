//! The validator consensus pump.
//!
//! This file intentionally keeps the single `select_biased!` event loop whole:
//! its drain, ingress, checkpoint, and cutover arms share one ordered state
//! machine and are reviewed as one responsibility.

use commonware_codec::DecodeExt as _;
use commonware_cryptography::{Signer, ed25519};
use commonware_p2p::{Manager, Recipients, Sender as P2pSender};
use commonware_runtime::{Clock, IoBuf, Metrics};
use commonware_utils::ordered::Set;
use futures::{FutureExt as _, StreamExt as _};

use consensus::ContentStore;
use directory::{DirQuery, DirReply, decode_reply, encode_query};
use recovery::Manifest;
use sdk::{Msg, StateRoot};

use super::announce::{
    CapabilityAnnouncer, ReadinessSignaller, dispatch_pending_deliveries, saga_next_expiry,
};
use crate::config::{hex_bytes, unhex};
use crate::constants::{DRAIN_TICK, MAX_PROTOCOL_VERSION, MODULE_IDS, NOP_TARGET, SUBMIT_HOLD};
use crate::explorer::{explorer_root_op, ship_index_blobs};
use crate::host_reads::{
    read_members_from_host, read_redemptions_from_host, read_upgrade_state,
    read_upgrade_status_raw, read_upgrade_version_fields, read_valset_members,
    read_valset_residents,
};
use crate::host_state::run_output_sink;
use crate::rpc::{
    JoinRequestRecord, JoinRequestView, RpcJob, RpcReply, RpcRequest, RpcStatus, spawn_rpc_listener,
};
use crate::sync::serve::{SyncBoundary, SyncStateRequest};
use crate::util::{hex, participant_bytes, resident_bytes, unix_ms};
use crate::{
    blob_fetch, config, lobby, oracle_pool, relay, relay_runtime, statesync_plane, voice_plane,
};

#[allow(clippy::too_many_arguments)]
pub(super) async fn run_loop<'a>(
    context: &'a commonware_runtime::tokio::Context,
    mut node: node::OrderedNode<
        consensus::SimplexOrderer,
        recovery::Recovery<commonware_runtime::tokio::Context>,
    >,
    mut orchestrator: consensus::ValsetOrchestrator<ed25519::PublicKey>,
    mut epoch_spawner: super::engine::EpochSpawner<'a>,
    mut last_cert_height: Option<u64>,
    mut latest_floor: Option<recovery::FloorCert>,
    mut mesh_oracle: commonware_p2p::authenticated::discovery::Oracle<ed25519::PublicKey>,
    sync_plane_book: Option<std::sync::Arc<statesync_plane::OverlayBook>>,
    media_peers: Option<std::sync::Arc<voice_plane::MediaPeers>>,
    blob_peers: std::sync::Arc<std::sync::RwLock<Vec<ed25519::PublicKey>>>,
    blob_fetcher: blob_fetch::BlobFetchFn,
    reach_cmd: Option<tokio::sync::mpsc::Sender<reachability::ReachabilityCommand>>,
    mut lobby_tx: super::MeshSender,
    mut relay_tx: super::MeshSender,
    mut sync_state_rx: futures::channel::mpsc::Receiver<SyncStateRequest>,
    mut lobby_ingress: futures::channel::mpsc::Receiver<(ed25519::PublicKey, Vec<u8>)>,
    mut relay_ingress: futures::channel::mpsc::Receiver<(ed25519::PublicKey, Vec<u8>)>,
    mut next_seq: u64,
    mut prev_ckpt: (Option<u64>, u64),
    signer: ed25519::PrivateKey,
    label: String,
    namespace: Vec<u8>,
    peers: Vec<ed25519::PublicKey>,
    validators: Vec<ed25519::PublicKey>,
    dev_demo: bool,
    checkpoint_blocks: u64,
    announce_capabilities: bool,
    rpc_listener: Option<std::net::TcpListener>,
    http_cmds: futures::channel::mpsc::Receiver<noded::NodeCommand>,
    stream_hub: noded::StreamHub,
    index: std::sync::Arc<indexer::IndexStore>,
    blobs: noded::blobs::BlobHandle,
    agent_provisioner: Option<dispatch_oracle::SharedProvisioner>,
    agent_dirs: capability_host::AgentDirs,
    metrics: noded::NodeMetrics,
    status_public_key: String,
    coordination: crate::config::Coordination,
) {
    // the local rpc bridge: blocking listener threads push parsed requests
    // into this bounded queue; the pump answers between drains.
    let (rpc_tx, mut rpc_ingress) = futures::channel::mpsc::channel::<RpcJob>(64);
    if let Some(listener) = rpc_listener {
        println!(
            "[node {label}] rpc listening on {}",
            listener
                .local_addr()
                .map(|a| a.to_string())
                .unwrap_or_default()
        );
        spawn_rpc_listener(listener, rpc_tx);
    } else {
        drop(rpc_tx); // rpc off: the branch below just stays pending forever.
    }

    // the ordered lane SIGNS every frame. rpc submits are signed by THIS
    // node's identity (the node is the local caller's custodian until user
    // keys reach the console); `next_seq` was set at boot — 1 on a fresh
    // genesis (after the demo op's 0), or past every recovered frame.

    // pump: drain finalized frames on an interval, apply them in agreed
    // (ascending-view) order, serve statesync rpcs, answer local rpc, and
    // drive the reactor seam between drains (every response reflects a
    // block boundary — never a torn mid-drain view). print `converged` ONCE
    // this node has applied every VALIDATOR's op. this infinite loop IS the
    // "run forever" park (keeps the mesh + sync service alive for joiners);
    // rpc `shutdown` is the graceful exit.
    let expected = validators.len();
    let mut applied = 0usize;
    let mut converged = false;
    // the app-surface lane: held submit replies keyed by the submitted
    // frame's content address, resolved when the frame drains (or expired
    // after SUBMIT_HOLD), plus the last block height published to ws
    // subscribers.
    let mut http_ingress = http_cmds;
    let mut pending_submits: std::collections::HashMap<
        node::FrameId,
        (
            futures::channel::oneshot::Sender<Result<noded::BlockSummary, String>>,
            std::time::Instant,
        ),
    > = std::collections::HashMap::new();
    // relayed submits held for a wire answer, keyed like pending_submits by
    // the frame's content address: resolved by the SAME drain that resolves
    // local holds, expired on the same SUBMIT_HOLD budget. the peer is where
    // the Reply goes.
    let mut pending_relays: std::collections::HashMap<
        node::FrameId,
        (ed25519::PublicKey, std::time::Instant),
    > = std::collections::HashMap::new();
    let mut validator_relay = relay_runtime::ValidatorRelay::new(blobs.clone());
    let mut last_published: Option<u64> = None;
    // verified-but-unapproved join requests, keyed by joiner key. NODE-
    // LOCAL and in-memory by design: this is a doorbell, not state — the
    // parked joiner re-announces every few seconds, so a restart loses
    // nothing durable. read by the `join-requests` rpc; entries whose key
    // has since become a member are dropped at read time.
    let mut join_requests: std::collections::BTreeMap<Vec<u8>, JoinRequestRecord> =
        std::collections::BTreeMap::new();
    // recovery cadence: sealed blocks since the last checkpoint manifest.
    let mut blocks_since_checkpoint: u64 = 0;
    // the last absolute view ticked to the reachability plane — one
    // ViewTick per actual advance, not one per 100ms drain pass.
    let mut last_reach_view: Option<u64> = None;
    // the per-block-time flush cadence: packs the window's enqueued frames
    // (real ops and/or an idle nop) into one batch block. see the flush loop.
    let mut last_flush = std::time::Instant::now();
    // a cutover Retarget the plane's command queue could not take yet
    // (NON-BLOCKING sends: the plane is not consensus, so the loop never
    // waits on it). retried every drain beat until it lands; a newer
    // epoch's Retarget supersedes an undelivered older one.
    let mut pending_retarget: Option<reachability::MeshEpochEvent> = None;
    // dev override (`make dev` sets DUCKTAPE_DISABLE_HEARTBEAT): keep an idle
    // dev chain quiet — no nop blocks — so every committed block is real
    // activity and the journal/logs carry no idle churn. NEVER set this on a
    // multi-node or upgrade-driving network: the heartbeat is what ticks an
    // idle chain across a pending cutover and keeps the console height
    // visibly live.
    let heartbeat_disabled = std::env::var_os("DUCKTAPE_DISABLE_HEARTBEAT").is_some();
    // throttle for the saga crank pump below.
    let mut last_crank = std::time::Instant::now();
    // throttle for the dispatch delivery-nudge pump below.
    let mut last_nudge = std::time::Instant::now();
    // the host-owned worker set (reactor seam): effects of finalized
    // blocks are offered here, and claimed follow-ups re-enter the ordered
    // lane as their own blocks.
    // load capability specs and discover this host's installed executor
    // CLIs (BYO — no credential handling here). the discovered tag set is
    // BOTH what the oracle worker can run and what this node announces to
    // the capability registry, so an announce can never claim more than
    // the host provides (`announce_capabilities = false` narrows the
    // announced set to nothing — never the reverse). routing and
    // default models live in the specs (docs/records/specs/capability-spec.md); a broken
    // operator spec is a boot error, not a silently dropped executor.
    let providers = capability_host::discover_with_dirs_and_output_sink(
        agent_dirs.clone(),
        run_output_sink(stream_hub.run_output()),
    )
    .unwrap_or_else(|e| panic!("capability specs failed to load: {e}"));
    let my_capabilities = providers.capabilities();
    // OFF-LOOP execution: the pool gates effects inline (lease check —
    // WorkerRequests leased to another node's key are skipped, not
    // double-run — under this node's submit key) but runs the provider
    // CLI on spawned background tasks; completed results come back over
    // `oracle_results` (an ingress arm below) and re-enter the ordered
    // lane as ordinary signed submits, so a minutes-long run never
    // stalls the drain/rpc/heartbeat arms of this loop.
    let (oracle_worker, mut oracle_results) = oracle_pool::build(
        context,
        providers,
        signer.public_key().as_ref().to_vec(),
        blobs.clone(),
        agent_provisioner.clone(),
        // fetch-on-miss over the mesh: a prompt pin staged on another
        // node's blob store resolves here instead of failing the run.
        Some(blob_fetcher),
    );
    let workers: Vec<Box<dyn reactor::Worker>> = vec![oracle_worker];
    // the readiness self-signaller: polls COMMITTED upgrade state between drains
    // and emits ONE truthful validator-origin `SignalReady` per pending upgrade
    // this binary can execute. survives restart/late-join (state-driven, not a
    // one-shot effect). inert before the module is registered.
    let mut signaller =
        ReadinessSignaller::new(MAX_PROTOCOL_VERSION, signer.public_key().as_ref().to_vec());
    // the capability self-announcer: publishes this node's discovered
    // provider set into the capability registry once (state-driven,
    // idempotent). inert when this host installed no executor CLIs.
    let mut announcer =
        CapabilityAnnouncer::new(signer.public_key().as_ref().to_vec(), my_capabilities);
    // one-shot upgrade transition markers keyed off COMMITTED upgrade state,
    // modeled on the `converged` latch: `upgrade armed …` fires when readiness
    // first reaches R==n (every current boundary member signaled) for the
    // pending upgrade — the pre-boundary observable the e2e keys on; `upgrade
    // cleared …` fires when a previously-observed pending clears (the boundary
    // `Advance` reconciliation at H, on ARM or ABORT). the boundary crossing
    // itself prints the `upgrade activated …` / `upgrade aborted …` verdict.
    let mut upgrade_armed_latch: Option<(String, u32)> = None;
    let mut upgrade_pending_seen: Option<String> = None;

    // graceful checkpoint on process signals (SIGTERM/SIGINT): the desktop
    // shell SIGTERMs the daemon on quit, so it must take the SAME safe path
    // as an rpc `Shutdown` — a best-effort final manifest + journal barrier
    // — instead of tearing down mid-block and leaving the disk ahead of the
    // last in-memory checkpoint (the recovery brick). the streams are made
    // INSIDE the tokio async context so the signal driver is live; a
    // failure to install them is non-fatal: log and carry on WITHOUT the
    // graceful-quit arm rather than aborting daemon boot — a hard SIGKILL /
    // power loss already lands on the same WAL-forward recovery, so the
    // worst case of a missing handler is the pre-fix behavior, not a brick.
    let mut sigterm = match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
    {
        Ok(s) => Some(s),
        Err(e) => {
            eprintln!(
                "[node {label}] WARN: SIGTERM handler install failed ({e}); \
                     graceful-quit checkpoint disabled (a hard kill still recovers)"
            );
            None
        }
    };
    let mut sigint = match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
    {
        Ok(s) => Some(s),
        Err(e) => {
            eprintln!(
                "[node {label}] WARN: SIGINT handler install failed ({e}); \
                     graceful-quit checkpoint disabled (a hard kill still recovers)"
            );
            None
        }
    };

    // the graceful checkpoint sequence, shared by the rpc `Shutdown` arm and
    // the signal arm so the two can never drift. a macro (not a fn) because
    // it borrows `node` mutably while reading `orchestrator`/`next_seq` and
    // `node`'s type is a large generic — it runs on the SAME single-threaded
    // select loop, so it can never race the periodic checkpoint below.
    // captures the committed upgrade version fields the same way the periodic
    // checkpoint does, so a graceful-quit manifest is byte-identical to one.
    macro_rules! graceful_checkpoint {
        () => {{
            if let Some(f) = node.finalized() {
                let pos = node.sink_mut().oplog_pos().await;
                let (cv, pu) = read_upgrade_version_fields(node.host()).await;
                if let Ok(m) = Manifest::capture(
                    node.host(),
                    Some(f.height),
                    orchestrator.epoch(),
                    orchestrator.epoch_base(),
                    participant_bytes(&orchestrator),
                    resident_bytes(&orchestrator),
                    orchestrator.pending_cutover().map(|c| c.cutover_view()),
                    cv,
                    pu,
                    pos,
                    next_seq,
                ) {
                    let _ = node.sink_mut().write_manifest(&m).await;
                }
            }
            let _ = node.sink_mut().sync().await;
        }};
    }
    // the drain deadline (see the drain arm): ABSOLUTE, so the
    // per-iteration select rebuild cannot reset it under ingress load.
    let mut next_drain = context.current() + DRAIN_TICK;
    loop {
        // resolve on whichever signal stream installed; if neither did,
        // this arm simply never fires (pending forever) and the loop runs
        // exactly as before the fix.
        let signalled = async {
            match (sigterm.as_mut(), sigint.as_mut()) {
                (Some(t), Some(i)) => {
                    let t = t.recv();
                    let i = i.recv();
                    futures::pin_mut!(t, i);
                    futures::future::select(t, i).await;
                }
                (Some(t), None) => {
                    t.recv().await;
                }
                (None, Some(i)) => {
                    i.recv().await;
                }
                (None, None) => futures::future::pending::<()>().await,
            }
        }
        .fuse();
        futures::pin_mut!(signalled);
        futures::select_biased! {
            _ = signalled => {
                println!(
                    "[node {label}] SIGTERM/SIGINT — graceful checkpoint then exit"
                );
                graceful_checkpoint!();
                std::process::exit(0);
            }
            // DRAIN CADENCE — an ABSOLUTE deadline, hoisted ABOVE the ingress
            // arms. this select is rebuilt every loop iteration, so an
            // arm-local `sleep(100ms)` restarts from zero whenever any other
            // arm completes first — a saturating rpc-submit stream (requests
            // landing well inside 100ms) then resets the timer forever and
            // the drain NEVER runs: heights and status freeze, held submit
            // replies starve, and the epoch cutover (`respawn_if_due` below
            // is drain-driven) stalls for exactly as long as the flood lasts
            // while the armed boundary's discard window swallows every
            // accepted op. an absolute deadline survives the select rebuild,
            // and sitting above the ingress arms makes `select_biased!` take
            // it the moment it is due — load can delay one drain by one
            // request's service time, never starve it.
            _ = context.sleep_until(next_drain).fuse() => {
                next_drain = context.current() + DRAIN_TICK;
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
                applied += drained_count;
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
                // the once-per-block System-injection traces (upgrade
                // Advance, mailbox DeliverPending follow-ups) ride beside
                // the member frames; each height's entry indexes AFTER
                // that height's member dispatches, matching the replay
                // paths' row order exactly.
                let mut system_dispatches: std::collections::BTreeMap<
                    u64,
                    Vec<host::DispatchRecord>,
                > = node.take_system_dispatches().into_iter().collect();
                // sealed = journaled: one seal per BLOCK (height), whatever a
                // batch's member count. count DISTINCT sealed heights so the
                // checkpoint cadence stays per-block; applied and rejected
                // members both seal, discarded frames never sealed a height.
                blocks_since_checkpoint += drained
                    .iter()
                    .filter(|d| d.disposition != node::Disposition::Discarded)
                    .map(|d| d.height)
                    .collect::<std::collections::BTreeSet<u64>>()
                    .len() as u64;
                // fold every SEALED frame into the derived per-module
                // index: an applied frame contributes its dispatch trace,
                // a rejected one folds EMPTY (it still consumed its
                // height, and every module's watermark must track the
                // sealed tip or restart staleness checks would rebuild
                // spuriously). discarded frames never sealed a height.
                // a frame the explorer shows — a decoded op that isn't
                // the heartbeat nop (the deliberately-empty block that
                // only ticks an idle chain) — additionally carries its
                // explorer row, so GET /v1/blocks survives restarts.
                // canonical state committed above, so an index failure
                // degrades read models only — the store poisons itself
                // and stays loud until rebuilt.
                // fold each BLOCK once: a batch delivers N DrainedFrames at
                // ONE height (its members, contiguous in agreed order). the
                // per-module index and the `ducktape_*` metrics series are
                // per-BLOCK — folding per frame would over-count blocks as ops
                // AND lose every member after the first to the index's
                // idempotent same-height skip. group the run of same-height
                // frames, concatenate their dispatch traces under one running
                // seq (so `op_key(height, seq)` stays unique across members),
                // and fold once. canonical state committed above, so an index
                // failure degrades read models only — it stays loud.
                let mut gi = 0;
                while gi < drained.len() {
                    let height = drained[gi].height;
                    let mut block_dispatches: Vec<host::DispatchRecord> = Vec::new();
                    let mut block_latency = 0u64;
                    let mut any_applied = false;
                    // the block record carries a RootOp for EVERY non-nop
                    // member (agreed order); the block hash is the first
                    // member's frame id and the commit is the members' shared
                    // app-hash. a pure nop/idle block shows no ops.
                    let mut block_ops: Vec<noded::RootOp> = Vec::new();
                    let mut block_hash: Option<node::FrameId> = None;
                    let mut block_app_hash: Option<StateRoot> = None;
                    while gi < drained.len() && drained[gi].height == height {
                        let d = &drained[gi];
                        gi += 1;
                        // a DISCARD never sealed this height (it is carried, not
                        // applied) — it contributes nothing to the fold.
                        if d.disposition == node::Disposition::Discarded {
                            continue;
                        }
                        if let (node::Disposition::Applied, Some(op)) =
                            (&d.disposition, &d.op)
                        {
                            any_applied = true;
                            block_latency = block_latency.saturating_add(op.latency_us);
                            block_dispatches.extend(op.dispatches.iter().cloned());
                        }
                        if let Some(op) = &d.op
                            && op.target != NOP_TARGET
                        {
                            let disposition = match d.disposition {
                                node::Disposition::Applied => noded::BlockDisposition::Applied,
                                node::Disposition::Rejected => noded::BlockDisposition::Rejected,
                                // unreachable: Discarded is filtered at the top
                                // of the inner loop; kept for match exhaustiveness.
                                node::Disposition::Discarded => continue,
                            };
                            if block_hash.is_none() {
                                block_hash = Some(d.id);
                                block_app_hash = Some(d.app_hash);
                            }
                            block_ops.push(explorer_root_op(
                                &blobs,
                                &op.origin,
                                &op.target,
                                &op.payload,
                                &op.dispatches,
                                disposition,
                            ));
                        }
                    }
                    // the block's System-injection dispatches index AFTER
                    // every member's (the replay paths' merge order) — an
                    // agent reply delivered via the mailbox injection is
                    // an op row here like anywhere else.
                    if let Some(sys) = system_dispatches.remove(&height) {
                        block_dispatches.extend(sys);
                    }
                    // one block per height: an APPLIED block records fully
                    // (count, this node's summed apply latency, per-module
                    // dispatch counters); an all-rejected block (the idle nop
                    // lands here) only follows the height gauge. ops_total
                    // counts the aggregated member ops.
                    if any_applied {
                        metrics.record_block(height, block_latency, &block_dispatches);
                    } else {
                        metrics.record_height(height);
                    }
                    metrics.record_ops(block_ops.len());
                    let record = (!block_ops.is_empty()).then(|| {
                        noded::block_row(&noded::BlockRecord {
                            height,
                            hash: block_hash.map(|h| noded::hex_bytes(&h)).unwrap_or_default(),
                            commit_hash: block_app_hash.map(|h| hex(&h)).unwrap_or_default(),
                            ops: block_ops,
                        })
                    });
                    // this lane's agreed clock IS the height: the drain stamps
                    // BlockContext { consensus_time: height } for every block.
                    let ops = indexer::BlockOps {
                        record,
                        ..noded::index_block_ops(height, height, &block_dispatches)
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
                        let msg = relay::RelayMsg::Reply { frame_id: d.id, outcome };
                        let _ = relay_tx.send(
                            Recipients::One(peer),
                            IoBuf::from(relay::encode_msg(&msg)),
                            false,
                        );
                    }
                    let Some((reply, _)) = pending_submits.remove(&d.id) else { continue };
                    let _ = reply.send(match d.disposition {
                        node::Disposition::Applied => Ok(noded::BlockSummary {
                            height: d.height,
                            // the PER-BLOCK boundary this frame settled at
                            // (not the end-of-drain hash — a drain can
                            // apply several blocks).
                            app_hash: hex(&d.app_hash),
                        }),
                        node::Disposition::Rejected => Err(d.reason.clone().unwrap_or_else(
                            || {
                                // the module's VERBATIM reason when the drain
                                // captured one (duckfs-client keys on the
                                // "files: conflict:" prefix); generic wording
                                // otherwise.
                                "op finalized but rejected (deterministic no-op)".into()
                            },
                        )),
                        // unreachable — filtered at the loop top — but
                        // stay total rather than panic.
                        node::Disposition::Discarded => continue,
                    });
                }
                validator_relay.expire(std::time::Instant::now(), &mut relay_tx);
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
                                "timed out awaiting finalization — re-query on the next block"
                                    .into(),
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
                                    detail: "timed out awaiting finalization — re-query on the next block".into(),
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
                // publish each newly-applied boundary to ws subscribers
                // (send only errs when nobody is subscribed — fine). the
                // drain loop above already folded each block into the
                // metrics series; this tip seam carries the ws block
                // summary only — it fires once per drain.
                if let Some(f) = node.finalized()
                    && last_published != Some(f.height)
                {
                    stream_hub.publish_block(f.height, hex(&f.app_hash));
                    last_published = Some(f.height);
                }

                // persist the finalization floor once everything at or
                // below it has drained. read the certificate FIRST, the
                // gate second: releases happen only on this thread, so a
                // zero gate proves the cert's view is fully applied — a
                // floor ahead of app state would suppress replay of
                // finalized ops a restart still needs.
                if let Some((view, cert)) = node.orderer().latest_finalization()
                    && view != 0
                    && node.orderer().unreleased_len() == 0
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
                                last_cert_height = Some(height);
                                latest_floor = Some(fc);
                            }
                            Err(e) => eprintln!(
                                "[node {label}] floor cert write failed (will retry): {e}"
                            ),
                        }
                    }
                }

                // periodic checkpoint: snapshot the in-memory cohort and
                // prune the op journal below the PREVIOUS checkpoint once
                // the persisted floor has passed it (pruned frames must
                // never be needed to resolve a re-reported finalization).
                if blocks_since_checkpoint >= checkpoint_blocks
                    && let Some(f) = node.finalized()
                {
                    let pos = node.sink_mut().oplog_pos().await;
                    let (cv, pu) = read_upgrade_version_fields(node.host()).await;
                    let captured = Manifest::capture(
                        node.host(),
                        Some(f.height),
                        orchestrator.epoch(),
                        orchestrator.epoch_base(),
                        participant_bytes(&orchestrator),
                        resident_bytes(&orchestrator),
                        orchestrator.pending_cutover().map(|c| c.cutover_view()),
                        cv,
                        pu,
                        pos,
                        next_seq,
                    );
                    match captured {
                        Ok(m) => match node.sink_mut().write_manifest(&m).await {
                            Ok(()) => {
                                blocks_since_checkpoint = 0;
                                let floor_passed = matches!(
                                    node.sink_mut().floor_cert(),
                                    Ok(Some(fc))
                                        if prev_ckpt.0.is_none_or(|h| fc.height >= h)
                                );
                                if floor_passed
                                    && let Err(e) =
                                        node.sink_mut().prune_oplog(prev_ckpt.1).await
                                {
                                    eprintln!("[node {label}] oplog prune failed: {e}");
                                }
                                prev_ckpt = (m.height, pos);
                            }
                            Err(e) => eprintln!(
                                "[node {label}] checkpoint write failed (will retry): {e}"
                            ),
                        },
                        Err(e) => eprintln!(
                            "[node {label}] checkpoint capture failed (will retry): {e}"
                        ),
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
                            let _ = cmd.try_send(
                                reachability::ReachabilityCommand::ViewTick(absolute_view),
                            );
                            last_reach_view = Some(absolute_view);
                        }
                        // flush a staged cutover Retarget (see
                        // `pending_retarget`) — MUST eventually land, so
                        // it retries every beat rather than being shed.
                        if let Some(event) = pending_retarget.take()
                            && let Err(tokio::sync::mpsc::error::TrySendError::Full(
                                reachability::ReachabilityCommand::Retarget(event),
                            )) = cmd.try_send(reachability::ReachabilityCommand::Retarget(
                                event,
                            ))
                        {
                            pending_retarget = Some(event);
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
                    if let consensus::ObservationOutcome::Scheduled(cutover) =
                        orchestrator.observe_members(
                            engine_view,
                            observed.iter().cloned(),
                            observed_residents.iter().cloned(),
                        )
                    {
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
                    if let Some(pending) = &boundary_upgrade.pending
                        && let consensus::ObservationOutcome::Scheduled(cutover) =
                            orchestrator.observe_upgrade(engine_view, pending.activation_height)
                    {
                        println!(
                            "[node {label}] upgrade '{}' armed — cutover to epoch {} at view {} (activation height {})",
                            pending.name,
                            cutover.next_epoch(),
                            cutover.cutover_view(),
                            pending.activation_height
                        );
                        node.set_view_ceiling(cutover.cutover_view());
                    }
                    if let Some(plan) = orchestrator.respawn_if_due(
                        engine_view,
                        observed,
                        observed_residents,
                        boundary_upgrade,
                    ) {
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
                        // transport FIRST: the new epoch's mesh must admit
                        // its members (a fresh joiner — or a granted
                        // resident — above all) before anything is
                        // expected of them. the mesh tracks the TRANSPORT
                        // union; the engine below gets validators only.
                        // index = epoch, strictly increasing across
                        // cutovers.
                        mesh_oracle.track(
                            plan.epoch(),
                            super::wiring::mesh_at(&peers, plan.valset().transport_members()),
                        );
                        // the statesync plane serves (and admits) exactly
                        // who the mesh tracks — follow the re-track.
                        if let Some(book) = &sync_plane_book {
                            book.set_peers(plan.valset().transport_members().iter());
                        }
                        // the media planes authenticate inbound by the same
                        // tracked set — follow the re-track too, so a
                        // just-added member's huddle media is admitted.
                        if let Some(peers) = &media_peers {
                            peers.set_peers(plan.valset().transport_members().iter());
                        }
                        // the blob fetch-on-miss lane fans out to the same
                        // tracked set — follow the re-track.
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
                            pending_retarget = Some(reachability::MeshEpochEvent {
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
                        let participants: Set<ed25519::PublicKey> = Set::try_from(
                            members.iter().cloned().collect::<Vec<_>>(),
                        )
                        .expect("orchestrator membership has no duplicates");
                        // a fresh epoch: new store (pins of the torn-down
                        // epoch die with it), genesis floor.
                        let orderer = epoch_spawner.spawn(
                            plan.epoch(),
                            participants,
                            ContentStore::new(),
                            None,
                        );
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
                            participant_bytes(&orchestrator),
                            resident_bytes(&orchestrator),
                            None,
                            cv,
                            pu,
                            pos,
                            next_seq,
                        );
                        match captured {
                            Ok(m) => match node.sink_mut().write_manifest(&m).await {
                                Ok(()) => {
                                    blocks_since_checkpoint = 0;
                                    prev_ckpt = (m.height, pos);
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
                if !heartbeat_disabled && last_flush.elapsed() >= consensus::BLOCK_TIME {
                    last_flush = std::time::Instant::now();
                    if node.pending_batch_len() == 0 && node.orderer().pending_len() == 0 {
                        let seq = next_seq;
                        next_seq += 1;
                        if let Err(e) = node
                            .submit(
                                &signer,
                                seq,
                                Msg { target: NOP_TARGET.into(), payload: Vec::new() },
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

                // READINESS SIGNAL (design §3 / plan Task 7.1): a current
                // boundary member whose binary can execute the pending upgrade
                // self-submits ONE `SignalReady`. gated to a current member (the
                // R = n readiness denominator); the signaller's own committed
                // read + local latch keep it idempotent. inert on a baseline net.
                if orchestrator
                    .current_members()
                    .contains(&signer.public_key())
                    && let Some((msg, name, to_version)) =
                        signaller.maybe_signal(node.host()).await
                {
                    let seq = next_seq;
                    next_seq += 1;
                    match node.submit(&signer, seq, msg).await {
                        Ok(_) => println!(
                            "[node {label}] signaled ready name={name} to_version={to_version}"
                        ),
                        Err(e) => {
                            // un-latch so a transient submit failure retries on
                            // the next tick (the module stays idempotent).
                            signaller.signaled = None;
                            eprintln!("[node {label}] readiness signal submit failed: {e}");
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
                if announce_capabilities
                    && orchestrator
                        .current_members()
                        .contains(&signer.public_key())
                    && let Some(msg) = announcer.maybe_announce(node.host()).await
                {
                    let seq = next_seq;
                    next_seq += 1;
                    match node.submit(&signer, seq, msg).await {
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

                // SAGA CRANK (P7 liveness, host side): nothing else ever
                // submits `SagaMsg::Crank`, and under strict leases a
                // saga whose assignee went dark advances ONLY via a crank
                // (lease re-lease or deadline timeout). state-driven:
                // when the committed next expiry is at or past the latest
                // finalized height, push one permissionless crank —
                // throttled like the heartbeat, since a backlog wider
                // than CRANK_BUDGET legitimately needs several. duplicate
                // cranks from other nodes are deterministic no-ops.
                if last_crank.elapsed() >= consensus::BLOCK_TIME
                    && let Some(finalized_height) = node.finalized().map(|f| f.height)
                    && let Some(expiry) = saga_next_expiry(node.host()).await
                    && expiry <= finalized_height
                {
                    last_crank = std::time::Instant::now();
                    let seq = next_seq;
                    next_seq += 1;
                    if let Err(e) = node
                        .submit(
                            &signer,
                            seq,
                            Msg {
                                target: "saga".into(),
                                payload: saga::encode_msg(
                                    &saga::SagaMsg::Crank {},
                                ),
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

                // DISPATCH DELIVERY NUDGE (never-pop-stack liveness): a
                // result committed into the dispatch mailbox delivers via
                // the drain's DeliverPending injection in the NEXT
                // successful block — and heartbeat nops are rejected
                // frames that never apply, so a quiet chain would sit on
                // its mailbox. state-driven: while the committed mailbox
                // is non-empty, push one permissionless Nudge — a no-op
                // whose block carries the injection. duplicate nudges
                // from other nodes are free.
                if last_nudge.elapsed() >= consensus::BLOCK_TIME
                    && dispatch_pending_deliveries(node.host()).await > 0
                {
                    last_nudge = std::time::Instant::now();
                    let seq = next_seq;
                    next_seq += 1;
                    if let Err(e) = node
                        .submit(
                            &signer,
                            seq,
                            Msg {
                                target: "dispatch".into(),
                                payload: dispatch::encode_msg(
                                    &dispatch::DispatchMsg::Nudge {},
                                ),
                            },
                        )
                        .await
                    {
                        eprintln!("[node {label}] dispatch nudge submit failed: {e}");
                    } else {
                        println!("[node {label}] dispatch delivery nudge submitted");
                    }
                }

                // UPGRADE TRANSITION MARKERS (one-shot, committed-state driven):
                // the greppable proof surface the e2e keys on. `armed` is the
                // module's own R==n verdict (pending set, boundary non-empty,
                // every current member signaled), so this fires exactly when
                // readiness first reaches the full set — before H is crossed.
                if let Some(st) = read_upgrade_status_raw(node.host()).await {
                    match &st.pending {
                        Some(up) => {
                            upgrade_pending_seen = Some(up.name.clone());
                            let key = (up.name.clone(), up.to_version);
                            if st.armed && upgrade_armed_latch.as_ref() != Some(&key) {
                                println!(
                                    "[node {label}] upgrade armed name={} to_version={} height={}",
                                    up.name, up.to_version, up.activation_height
                                );
                                upgrade_armed_latch = Some(key);
                            }
                        }
                        None => {
                            if let Some(name) = upgrade_pending_seen.take() {
                                // the boundary Advance reconciled the pending
                                // (ARM flip or ABORT clear) — the slot is free.
                                println!("[node {label}] upgrade cleared name={name}");
                                upgrade_armed_latch = None;
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
                    for w in &workers {
                        match w.run(&eff).await {
                            Ok(reactor::WorkOutcome::Handled(Some(follow))) => {
                                let seq = next_seq;
                                next_seq += 1;
                                if let Err(e) =
                                    node.submit(&signer, seq, follow).await
                                {
                                    eprintln!("[node {label}] worker follow-up submit failed: {e}");
                                }
                                claimed = true;
                                break;
                            }
                            // a deliberate skip (e.g. leased to another
                            // node): claimed, nothing to submit.
                            Ok(reactor::WorkOutcome::Handled(None)) => {
                                claimed = true;
                                break;
                            }
                            Ok(reactor::WorkOutcome::NotMine) => {}
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
                if dev_demo && !converged && applied >= expected {
                    let h = node.app_hash();
                    println!("[node {label}] converged app_hash={}", hex(&h));
                    // dump every directory key so the demo can eyeball the ops
                    // (each node ends holding the op it originated AND the peer's).
                    for k in 0..expected {
                        let reply = node
                            .host()
                            .query("directory", &encode_query(&DirQuery::Get { key: format!("k{k}") }))
                            .await
                            .expect("directory query");
                        if let Ok(DirReply::Value(v)) = decode_reply(&reply) {
                            println!("[node {label}]   directory k{k}={v:?}");
                        }
                    }
                    converged = true;
                }
            }
            job = rpc_ingress.next() => {
                let Some((req, reply)) = job else { continue };
                let resp = match req {
                    RpcRequest::Submit { target, payload_hex } => {
                        match unhex(&payload_hex) {
                            Ok(payload) => {
                                let seq = next_seq;
                                next_seq += 1;
                                match node
                                    .submit(&signer, seq, Msg { target, payload })
                                    .await
                                {
                                    Ok(_) => RpcReply::ok(),
                                    Err(e) => RpcReply::err(format!("submit failed: {e}")),
                                }
                            }
                            Err(e) => RpcReply::err(format!("bad payload_hex: {e}")),
                        }
                    }
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
                        let members = read_members_from_host(node.host()).await;
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
                        graceful_checkpoint!();
                        let _ = reply.send(RpcReply::ok());
                        println!("[node {label}] shutdown requested via rpc — exiting");
                        std::process::exit(0);
                    }
                };
                let _ = reply.send(resp);
            }
            result = oracle_results.next() => {
                // a completed off-loop provider run: its OracleResult op
                // re-enters the ordered lane as an ordinary signed
                // submit — the oracle-as-op, unchanged; only WHERE the
                // provider ran moved.
                let Some(msg) = result else { continue };
                let seq = next_seq;
                next_seq += 1;
                if let Err(e) = node.submit(&signer, seq, msg).await {
                    eprintln!("[node {label}] oracle result submit failed: {e}");
                }
            }
            announce = lobby_ingress.next() => {
                let Some((peer, bytes)) = announce else { continue };
                // `fatal: true` marks the refusal PERMANENT for this
                // invite — the joiner stops re-announcing instead of
                // spinning on a token that can never redeem.
                let mut send_reply = |recorded: bool, detail: String, cap: Option<Vec<u8>>, fatal: bool| {
                    let msg = lobby::LobbyMsg::JoinReply { recorded, detail, cap, fatal };
                    let _ = lobby_tx.send(
                        Recipients::One(peer.clone()),
                        IoBuf::from(lobby::encode_msg(&msg)),
                        false,
                    );
                };
                let msg = match lobby::decode_msg(&bytes) {
                    Ok(m) => m,
                    Err(_) => continue, // junk on the doorbell — drop.
                };
                // crypto first (pure, cheap): the token must verify for
                // THIS network and the announced key must prove itself.
                let verified = match lobby::verify_join_request(&msg, &namespace) {
                    Ok(v) => v,
                    Err(e) => {
                        send_reply(false, e, None, false);
                        continue;
                    }
                };
                // then membership: the issuer must still be a member (a
                // removed member's outstanding invites die with it), and a
                // joiner that already holds standing — VALIDATOR or
                // RESIDENT — has nothing pending.
                let members = read_members_from_host(node.host()).await;
                let residents_now = read_valset_residents(node.host()).await;
                let joiner_bytes = verified.joiner.as_ref().to_vec();
                if members.contains(&joiner_bytes) {
                    send_reply(false, "already a validator".into(), None, false);
                    continue;
                }
                if residents_now.contains(&joiner_bytes) {
                    send_reply(
                        false,
                        "already a resident — a member promotes it into the quorum".into(),
                        None,
                        false,
                    );
                    continue;
                }
                if !members.contains(&verified.issuer.as_ref().to_vec()) {
                    send_reply(
                        false,
                        "the inviting member is no longer part of this network".into(),
                        None,
                        false,
                    );
                    continue;
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
                    continue;
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
                let minted_cap = if coordination == config::Coordination::Private
                    && validators.contains(&signer.public_key())
                {
                    let mut subj = [0u8; 32];
                    subj.copy_from_slice(verified.joiner.as_ref());
                    let cap = nat_traversal::mint_coord_cap(
                        &signer,
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
                    continue;
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
                let seq = next_seq;
                next_seq += 1;
                match node
                    .submit(
                        &signer,
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
            relayed = relay_ingress.next() => {
                let Some((peer, bytes)) = relayed else { continue };
                let Ok(msg) = relay::decode_msg(&bytes) else { continue };
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
                let Some(action) = validator_relay.on_message(
                    peer,
                    msg,
                    &members_now,
                    &residents_now,
                    &mut relay_tx,
                ) else {
                    continue;
                };
                match action {
                    relay_runtime::ValidatorAction::SubmitResident {
                        frame_id,
                        frame,
                        peer,
                    } => match node.submit_frame(frame).await {
                        Ok(id) => {
                            debug_assert_eq!(id, frame_id);
                            pending_relays.insert(
                                id,
                                (peer, std::time::Instant::now() + SUBMIT_HOLD),
                            );
                        }
                        Err(e) => relay_runtime::send_reply(
                            &mut relay_tx,
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
            cmd = http_ingress.next() => {
                let Some(cmd) = cmd else { continue };
                match cmd {
                    // `origin` is the caller's CLAIMED submitter identity —
                    // meaningful on the embedded daemon, but this lane signs
                    // frames, and the signed origin IS this node's pubkey
                    // (authenticated authorship that governance relies on).
                    // a claimed origin cannot ride a signed frame without
                    // making authorship forgeable, so it is ignored here;
                    // display names resolve via the name registry instead.
                    noded::NodeCommand::Submit { target, payload, origin: _, reply } => {
                        let seq = next_seq;
                        next_seq += 1;
                        let frame = node::encode_frame(&signer, seq, &Msg { target, payload });
                        let peers: Vec<ed25519::PublicKey> =
                            if relay::required_blob_digest(&frame).is_some() {
                                read_valset_members(node.host())
                                    .await
                                    .iter()
                                    .filter_map(|raw| {
                                        ed25519::PublicKey::decode(raw.as_slice()).ok()
                                    })
                                    .filter(|key| key != &signer.public_key())
                                    .collect()
                            } else {
                                Vec::new()
                            };
                        match validator_relay.prepare_local(
                            frame,
                            reply,
                            peers,
                            &mut relay_tx,
                        ) {
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
                    noded::NodeCommand::Query { target, req, reply } => {
                        let result = node
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
                                root: node
                                    .host()
                                    .module_root(m)
                                    .map(|r| hex(&r))
                                    .unwrap_or_default(),
                                category: noded::ModuleCategory::of(m),
                            })
                            .collect();
                        let _ = reply.send(noded::NodeStatus {
                            version: env!("CARGO_PKG_VERSION").into(),
                            app_hash: hex(&node.app_hash()),
                            height: node.finalized().map(|f| f.height).unwrap_or(0),
                            modules,
                            public_key: status_public_key.clone(),
                        });
                    }
                    noded::NodeCommand::Metrics { reply } => {
                        // one registry: commonware's runtime series plus the
                        // `ducktape_*` block series the drain loop records.
                        let _ = reply.send(context.encode());
                    }
                }
            }
            req = sync_state_rx.next() => {
                // the statesync serve task's state touches (the
                // [`SyncStateRequest`] seam): each is one bounded read
                // against loop-owned state — the heavy serving (decode,
                // captures, slicing, replies) lives on the serve task.
                let Some(req) = req else {
                    // the serve task ended (network shutdown) — nothing
                    // left to answer; keep draining consensus regardless.
                    continue;
                };
                match req {
                    SyncStateRequest::Boundary { known, reply } => {
                        // the boundary's consensus coordinates ride the manifest.
                        // the floor certificate is served only when it certifies
                        // exactly the current boundary — a cert behind the
                        // boundary would make a joiner skip history it needs.
                        // stamp the served boundary's committed version fields from
                        // live upgrade state (like epoch/view_base). a joiner installs
                        // its dual-path modules at `current_version` and preflights
                        // against `required_min_version` — both derived from these.
                        let (bc_current, bc_pending) =
                            read_upgrade_version_fields(node.host()).await;
                        let coords = statesync::BoundaryCoords {
                            epoch: orchestrator.epoch(),
                            view_base: orchestrator.epoch_base(),
                            participants: participant_bytes(&orchestrator),
                            residents: resident_bytes(&orchestrator),
                            current_version: bc_current,
                            pending_upgrade: bc_pending,
                            floor_cert: latest_floor
                                .as_ref()
                                .filter(|fc| fc.epoch == orchestrator.epoch())
                                .filter(|fc| {
                                    node.finalized().is_some_and(|f| f.height == fc.height)
                                })
                                .map(|fc| fc.cert.clone()),
                        };
                        let finalized_for_sync = node.finalized().filter(|f| {
                            f.height <= coords.view_base || coords.floor_cert.is_some()
                        });
                        let answer = match finalized_for_sync {
                            // two refusals, named apart: no boundary at
                            // all (pre-first-block), vs the per-block
                            // window where the tip advanced but its
                            // finalization certificate has not persisted
                            // yet — a retry lands once they align.
                            None => Err(match node.finalized() {
                                Some(f) => format!(
                                    "boundary {} awaiting its finalization certificate — \
                                     retry",
                                    f.height
                                ),
                                None => "no finalized boundary to serve yet".to_string(),
                            }),
                            Some(finalized) => {
                                let id = statesync::BoundaryId {
                                    height: finalized.height,
                                    app_hash: finalized.app_hash,
                                };
                                if known.contains(&id) {
                                    // the serve task holds this boundary's
                                    // payload — coordinates only.
                                    Ok(SyncBoundary { id, coords, data: None })
                                } else {
                                    statesync::capture_boundary(
                                        node.host(),
                                        finalized,
                                        &coords,
                                    )
                                    .await
                                    .map(|(id, data)| SyncBoundary {
                                        id,
                                        coords,
                                        data: Some(data),
                                    })
                                }
                            }
                        };
                        let _ = reply.send(answer);
                    }
                    SyncStateRequest::ModuleServe { module_id, body, reply } => {
                        let served = node
                            .host()
                            .serve_sync(&module_id, &body)
                            .await
                            .map_err(|e| format!("module {module_id} serve_sync: {e}"));
                        let _ = reply.send(served);
                    }
                    SyncStateRequest::Frames { after_height, up_to_height, reply } => {
                        let read = node
                            .sink_mut()
                            .read_finalized_frames(after_height, up_to_height)
                            .await;
                        let _ = reply.send(read);
                    }
                    SyncStateRequest::IndexCut { reply } => {
                        let _ = reply.send(ship_index_blobs(&index, &label));
                    }
                    SyncStateRequest::TipCoords { reply } => {
                        // the detection lane: everything here is already
                        // loop-owned state — no capture, and deliberately
                        // no floor-cert alignment gate. that gate protects
                        // a JOINER from syncing a boundary whose history
                        // it would skip; a detection reply carries a
                        // presence bit, never certificate bytes, and every
                        // action taken on it (ascension, promotion)
                        // re-fetches a full manifest through the gated
                        // Boundary path.
                        let answer = match node.finalized() {
                            None => Err("no finalized boundary to serve yet".to_string()),
                            Some(f) => Ok(statesync::TipCoords {
                                height: f.height,
                                app_hash: f.app_hash,
                                epoch: orchestrator.epoch(),
                                view_base: orchestrator.epoch_base(),
                                participants: participant_bytes(&orchestrator),
                                residents: resident_bytes(&orchestrator),
                                has_floor: latest_floor
                                    .as_ref()
                                    .filter(|fc| fc.epoch == orchestrator.epoch())
                                    .is_some_and(|fc| fc.height == f.height),
                            }),
                        };
                        let _ = reply.send(answer);
                    }
                }
            }
        }
    }
}
