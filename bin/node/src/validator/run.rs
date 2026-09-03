//! The validator consensus pump.
//!
//! The biased selector stays in one small coordinator so ordering is explicit.
//! Each event is handled by a focused method over one loop-owned runtime state.

mod drain;
mod ingress;
pub(crate) mod sync;

use commonware_cryptography::{Signer, ed25519};
use commonware_runtime::Clock;
use futures::{FutureExt as _, StreamExt as _};

use recovery::Manifest;

use crate::constants::DRAIN_TICK;
use crate::reachability_plane::GateOutcomes;
use crate::rpc::{JoinRequestRecord, RpcJob};
use crate::sync::serve::SyncStateRequest;
use crate::util::{participant_bytes, resident_bytes};
use crate::{join_gate, overlay_book, relay_runtime};

pub(super) type ValidatorNode = node::OrderedNode<
    consensus::SimplexOrderer,
    recovery::Recovery<commonware_runtime::tokio::Context>,
>;

/// a join gate held open awaiting its `Redeem` frame's consensus fate. the
/// member submitted the redemption and holds the joiner's outcome
/// keyed by the frame id until `on_drain` resolves it into `gate_outcomes` —
/// the settle-then-answer seam, mirroring `pending_relays`. no mesh `peer`
/// rides here any more: the answer goes back over the tunnel
/// doorbell, read from the shared map on the joiner's next retransmit.
struct GatePending {
    /// the joiner key: clears the `gating` in-flight index and keys the
    /// outcome write on resolution.
    joiner: Vec<u8>,
    /// the packed coord cap to deliver on `Admitted` (private coordination).
    cap: Option<Vec<u8>>,
    /// answer `Busy` (non-terminal) once past this instant (the settle timeout).
    deadline: std::time::SystemTime,
}

/// write a resolved gate outcome where the intro doorbell reads it — the
/// shared map the joiner's next retransmit is answered from.
fn settle_gate(outcomes: &GateOutcomes, joiner: Vec<u8>, reply: join_gate::IntroReply) {
    outcomes
        .lock()
        .expect("gate outcomes lock")
        .insert(joiner, reply);
}

/// assemble this node's boundary facts and publish them into the shared
/// `/v1/status` cell — the http route reads the cell directly, never the
/// command lane, so this is the ONE place a validator's status becomes
/// visible (reached via [`ValidatorRuntime::publish_status`] at startup and
/// after every drain turn). pure assembly over the node's committed state;
/// the operations section is stamped from the shared metrics projection
/// (and overlaid live on the read side anyway).
pub(super) fn publish_boundary_status(
    status: &noded::StatusCell,
    node: &ValidatorNode,
    metrics: &noded::NodeMetrics,
    status_public_key: &str,
) {
    let modules = crate::constants::MODULE_IDS
        .iter()
        .map(|m| noded::ModuleStatus {
            id: (*m).into(),
            root: node
                .host()
                .module_root(m)
                .map(|r| crate::util::hex(&r))
                .unwrap_or_default(),
            category: noded::ModuleCategory::of(m),
        })
        .collect();
    status.publish(noded::NodeStatus {
        version: crate::build_version(),
        root_hash: crate::util::hex(&node.root_hash()),
        height: node.finalized().map(|f| f.height).unwrap_or(0),
        modules,
        public_key: status_public_key.into(),
        // the cell overlays the boot-wired chain id on every read.
        chain_id: String::new(),
        operations: metrics.operational_status(),
    });
}

/// The long-lived state owned by the validator event loop.
///
/// A single owner lets the handler modules share ordered state without locks or
/// cross-task message passing.
pub(super) struct ValidatorLoopState<'a> {
    pub(super) context: &'a commonware_runtime::tokio::Context,
    pub(super) node: ValidatorNode,
    pub(super) orchestrator: consensus::ValsetOrchestrator<ed25519::PublicKey>,
    pub(super) epoch_spawner: super::engine::EpochSpawner<'a>,
    pub(super) last_cert_height: Option<u64>,
    pub(super) latest_floor: Option<recovery::FloorCert>,
    pub(super) mesh_oracle: commonware_p2p::authenticated::lookup::Oracle<ed25519::PublicKey>,
    pub(super) mesh_window: crate::mesh_window::MeshWindowTracker,
    pub(super) mesh_book: std::sync::Arc<crate::mesh_book::MeshAddressBook>,
    pub(super) gateway_book: Option<std::sync::Arc<crate::gateway_plane::OverlayBook>>,
    pub(super) media_peers: Option<std::sync::Arc<overlay_book::OverlayPeers>>,
    pub(super) blob_peers: std::sync::Arc<std::sync::RwLock<Vec<ed25519::PublicKey>>>,
    pub(super) blob_client: crate::blob_fetch::ServeLaneBlobClient<super::MeshSender>,
    pub(super) reach_cmd: Option<tokio::sync::mpsc::Sender<reachability::ReachabilityCommand>>,
    pub(super) relay_tx: super::MeshSender,
    pub(super) sync_state_rx: futures::channel::mpsc::Receiver<SyncStateRequest>,
    /// the join GATE's forward lane from the intro doorbell:
    /// verified gate requests the reachability plane rang through.
    pub(super) gate_fwd_rx: tokio::sync::mpsc::Receiver<join_gate::GateForward>,
    /// a never-sending clone of the forward lane's sender, held so the select
    /// arm stays PENDING (instead of None-spinning) when no reachability
    /// plane was wired to ring the doorbell.
    pub(super) gate_fwd_keepalive: tokio::sync::mpsc::Sender<join_gate::GateForward>,
    /// where the drain writes each settled gate outcome; the doorbell reads
    /// it on the joiner's next retransmit.
    pub(super) gate_outcomes: GateOutcomes,
    pub(super) relay_ingress: futures::channel::mpsc::Receiver<(ed25519::PublicKey, Vec<u8>)>,
    pub(super) next_seq: u64,
    pub(super) prev_ckpt: (Option<u64>, u64),
    pub(super) signer: ed25519::PrivateKey,
    pub(super) label: String,
    pub(super) validators: Vec<ed25519::PublicKey>,
    pub(super) dev_demo: bool,
    pub(super) checkpoint_blocks: u64,
    pub(super) cadence: consensus::Cadence,
    /// sync retention lease (unix secs of the last served state-sync request)
    /// — the drain defers oplog pruning while it is fresh.
    pub(super) sync_lease: std::sync::Arc<std::sync::atomic::AtomicU64>,
    /// the local rpc bridge's parsed-request queue — the caller owns the
    /// listener spawn (a promoted node's listener pump carries over from
    /// its parked life; a fresh boot spawns one), so both entries feed the
    /// same seam.
    pub(super) rpc_ingress: futures::channel::mpsc::Receiver<RpcJob>,
    pub(super) http_cmds: futures::channel::mpsc::Receiver<noded::NodeCommand>,
    pub(super) stream_hub: noded::StreamHub,
    pub(super) index: std::sync::Arc<indexer::IndexStore>,
    pub(super) blobs: noded::blobs::BlobHandle,
    /// the volatile service-signaling catalog: the live half of the capability
    /// announce (`grant ∩ hello`). Shared with the http surface's handle, so a
    /// daemon's hello is visible to the announce pump the moment it lands.
    pub(super) metrics: noded::NodeMetrics,
    pub(super) status: noded::StatusCell,
    pub(super) status_public_key: String,
    pub(super) coordination: crate::config::Coordination,
    /// where a SIGUSR1 task dump lands (`<workspace>/tasks.txt`).
    pub(super) workspace: std::path::PathBuf,
}

struct ValidatorRuntime<'a> {
    context: &'a commonware_runtime::tokio::Context,
    node: ValidatorNode,
    orchestrator: consensus::ValsetOrchestrator<ed25519::PublicKey>,
    epoch_spawner: super::engine::EpochSpawner<'a>,
    last_cert_height: Option<u64>,
    latest_floor: Option<recovery::FloorCert>,
    mesh_oracle: commonware_p2p::authenticated::lookup::Oracle<ed25519::PublicKey>,
    mesh_window: crate::mesh_window::MeshWindowTracker,
    mesh_book: std::sync::Arc<crate::mesh_book::MeshAddressBook>,
    gateway_book: Option<std::sync::Arc<crate::gateway_plane::OverlayBook>>,
    media_peers: Option<std::sync::Arc<overlay_book::OverlayPeers>>,
    blob_peers: std::sync::Arc<std::sync::RwLock<Vec<ed25519::PublicKey>>>,
    blob_client: crate::blob_fetch::ServeLaneBlobClient<super::MeshSender>,
    reach_cmd: Option<tokio::sync::mpsc::Sender<reachability::ReachabilityCommand>>,
    relay_tx: super::MeshSender,
    gate_outcomes: GateOutcomes,
    _gate_fwd_keepalive: tokio::sync::mpsc::Sender<join_gate::GateForward>,
    next_seq: u64,
    prev_ckpt: (Option<u64>, u64),
    signer: ed25519::PrivateKey,
    label: String,
    validators: Vec<ed25519::PublicKey>,
    dev_demo: bool,
    checkpoint_blocks: u64,
    cadence: consensus::Cadence,
    sync_lease: std::sync::Arc<std::sync::atomic::AtomicU64>,
    stream_hub: noded::StreamHub,
    index: std::sync::Arc<indexer::IndexStore>,
    blobs: noded::blobs::BlobHandle,
    metrics: noded::NodeMetrics,
    status: noded::StatusCell,
    status_public_key: String,
    coordination: crate::config::Coordination,
    /// read only by the SIGUSR1 task-dump arm, which exists on Linux alone.
    #[cfg_attr(
        not(all(
            tokio_unstable,
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        )),
        allow(dead_code)
    )]
    workspace: std::path::PathBuf,
    /// the earliest instant the NEXT `refresh_operations` may run — the
    /// exposition parse is the pricey part of a status publish, so it is
    /// paced here instead of riding every drained boundary.
    next_ops_refresh: std::time::SystemTime,
    expected: usize,
    applied: usize,
    converged: bool,
    pending_submits: std::collections::HashMap<
        node::FrameId,
        (
            futures::channel::oneshot::Sender<Result<noded::BlockSummary, String>>,
            std::time::SystemTime,
        ),
    >,
    pending_relays:
        std::collections::HashMap<node::FrameId, (ed25519::PublicKey, std::time::SystemTime)>,
    /// join gates held open awaiting their `Redeem` frame's consensus fate,
    /// keyed by frame id (the settle-then-answer seam, resolved in `on_drain`).
    pending_gates: std::collections::HashMap<node::FrameId, GatePending>,
    /// the in-flight-gate index (joiner key → its frame id): one gate per
    /// joiner, so a duplicate Request re-arms rather than double-submits.
    gating: std::collections::HashMap<Vec<u8>, node::FrameId>,
    validator_relay: relay_runtime::ValidatorRelay,
    last_published: Option<u64>,
    join_requests: std::collections::BTreeMap<Vec<u8>, JoinRequestRecord>,
    blocks_since_checkpoint: u64,
    /// the earliest a checkpoint may START again — see `cooldown_until`.
    /// blocks alone cannot express what a checkpoint COSTS the loop, and
    /// the loop is what answers `/v1/query` and SIGTERM (#1018).
    checkpoint_not_before: std::time::SystemTime,
    /// the composed root-hash recorded by the last manifest THIS loop wrote —
    /// the change gate for the periodic checkpoint. `None` until the first
    /// write, so a fresh boot re-anchors on its first cadence hit. See
    /// `checkpoint_due`: an idle chain's nop blocks must not buy a full
    /// re-encode of the manifest already on disk (#1308).
    last_written_root: Option<sdk::StateRoot>,
    last_reach_view: Option<u64>,
    last_flush: std::time::SystemTime,
    pending_retarget: Option<reachability::MeshEpochEvent>,
    heartbeat_disabled: bool,
    /// the sender half of the finalization delivery wake — re-installed on
    /// every epoch cutover's fresh orderer so event-driven draining survives
    /// the engine respawn.
    delivery_wake_tx: tokio::sync::mpsc::UnboundedSender<()>,
    /// whether our un-finalized proposals include REAL ops (a parked idle nop
    /// never sets this) — the condition for the leader-nudge escort. set at
    /// eager flush and cutover carry, cleared when the orderer FIFO drains.
    real_work_parked: bool,
    /// the last locally-estimated view a leader nudge was sent for — one
    /// nudge per view; every finalized block moves the estimate and re-arms.
    last_nudged_view: Option<u64>,
    last_crank: std::time::SystemTime,
    last_nudge: std::time::SystemTime,
    workers: Vec<Box<dyn host::worker::Worker>>,
    code_signaller: super::code_announce::CodeReadinessSignaller,
    /// completed pending-swap code fetches, reaped at each readiness pump so
    /// a failed fetch retries next tick (the sender rides in each task).
    fetch_done_tx: tokio::sync::mpsc::UnboundedSender<[u8; 32]>,
    fetch_done_rx: tokio::sync::mpsc::UnboundedReceiver<[u8; 32]>,
    next_drain: std::time::SystemTime,
}

pub(super) async fn run(state: ValidatorLoopState<'_>) {
    let ValidatorLoopState {
        context,
        node,
        orchestrator,
        epoch_spawner,
        last_cert_height,
        latest_floor,
        mesh_oracle,
        mesh_window,
        mesh_book,
        gateway_book,
        media_peers,
        blob_peers,
        blob_client,
        reach_cmd,
        relay_tx,
        mut sync_state_rx,
        mut gate_fwd_rx,
        gate_fwd_keepalive,
        gate_outcomes,
        mut relay_ingress,
        next_seq,
        prev_ckpt,
        signer,
        label,
        validators,
        dev_demo,
        checkpoint_blocks,
        cadence,
        sync_lease,
        rpc_ingress,
        http_cmds,
        stream_hub,
        index,
        blobs,
        metrics,
        status,
        status_public_key,
        coordination,
        workspace,
    } = state;
    let mut rpc_ingress = rpc_ingress;

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
    let applied = 0usize;
    let converged = false;
    // the app-surface lane: held submit replies keyed by the submitted
    // frame's content address, resolved when the frame drains (or expired
    // after SUBMIT_HOLD), plus the last block height published to ws
    // subscribers.
    let mut http_ingress = http_cmds;
    let pending_submits: std::collections::HashMap<
        node::FrameId,
        (
            futures::channel::oneshot::Sender<Result<noded::BlockSummary, String>>,
            std::time::SystemTime,
        ),
    > = std::collections::HashMap::new();
    // relayed submits held for a wire answer, keyed like pending_submits by
    // the frame's content address: resolved by the SAME drain that resolves
    // local holds, expired on the same SUBMIT_HOLD budget. the peer is where
    // the Reply goes.
    let pending_relays: std::collections::HashMap<
        node::FrameId,
        (ed25519::PublicKey, std::time::SystemTime),
    > = std::collections::HashMap::new();
    // join gates held open awaiting their Redeem frame's consensus fate, and
    // the joiner→frame in-flight index that dedups a re-Request while settling.
    let pending_gates: std::collections::HashMap<node::FrameId, GatePending> =
        std::collections::HashMap::new();
    let gating: std::collections::HashMap<Vec<u8>, node::FrameId> =
        std::collections::HashMap::new();
    let validator_relay = relay_runtime::ValidatorRelay::new(std::sync::Arc::new(blobs.clone()));
    let last_published: Option<u64> = None;
    // verified-but-unapproved join requests, keyed by joiner key. NODE-
    // LOCAL and in-memory by design: this is a doorbell, not state — the
    // parked joiner re-announces every few seconds, so a restart loses
    // nothing durable. read by the `join-requests` rpc; entries whose key
    // has since become a member are dropped at read time.
    let join_requests: std::collections::BTreeMap<Vec<u8>, JoinRequestRecord> =
        std::collections::BTreeMap::new();
    // recovery cadence: sealed blocks since the last checkpoint manifest.
    let blocks_since_checkpoint: u64 = 0;
    // no cooldown owed at boot: the first checkpoint's own cost is the
    // estimate every later one is held off by.
    let checkpoint_not_before = context.current();
    // the last absolute view ticked to the reachability plane — one
    // ViewTick per actual advance, not one per 100ms drain pass.
    let last_reach_view: Option<u64> = None;
    // the IDLE beat grid: when the last flush (or restamp) happened, pacing
    // the one-nop-per-block-time idle heartbeat. busy flushing is event-driven
    // and merely restamps this. see `pump_heartbeat` / `pump_eager_flush`.
    let last_flush = context.current();
    // a cutover Retarget the plane's command queue could not take yet
    // (NON-BLOCKING sends: the plane is not consensus, so the loop never
    // waits on it). retried every drain beat until it lands; a newer
    // epoch's Retarget supersedes an undelivered older one.
    let pending_retarget: Option<reachability::MeshEpochEvent> = None;
    // dev override (`make dev` sets DUCKTAPE_DISABLE_HEARTBEAT): keep an idle
    // dev chain quiet — no nop blocks — so every committed block is real
    // activity and the journal/logs carry no idle churn. NEVER set this on a
    // multi-node or upgrade-driving network: the heartbeat is what ticks an
    // idle chain across a pending cutover and keeps the console height
    // visibly live.
    let heartbeat_disabled = std::env::var_os("DUCKTAPE_DISABLE_HEARTBEAT").is_some();
    // throttle for the saga crank pump below.
    let last_crank = context.current();
    // throttle for the dispatch delivery-nudge pump below.
    let last_nudge = context.current();
    // the host-owned worker set (reactor seam): effects of finalized blocks are
    // offered here. It is EMPTY — dispatch work is executed by the standalone
    // compute daemon (`ducktape service run compute`), which discovers its
    // assignments from committed state and submits results over this node's own
    // /v1 surface. The node constructs no provider set, no dispatch pool and no
    // resource ledger; an unclaimed effect still surfaces through the drain's
    // module notes, which is the honest diagnostic on a node whose daemon is
    // not running.
    //
    // THE SANDBOX IS NOT THE NODE'S ANY MORE. Both planes that used to need it
    // are out of process: compute serves dispatch work and agent serves
    // interactive ptys. Each run's VMM is a child of the daemon that booted it,
    // so the daemons share no sandbox state at all — a restart of one cannot
    // reach the other's runs, which is what makes them separate failure
    // domains without any per-service root to keep apart.
    let workers: Vec<Box<dyn host::worker::Worker>> = Vec::new();
    // the CODE readiness self-signaller for pending code swaps — verifies (or
    // fetches) the committed component bytes and emits one truthful
    // `SignalReady` per swap.
    let code_signaller =
        super::code_announce::CodeReadinessSignaller::new(signer.public_key().as_ref().to_vec());
    let (fetch_done_tx, fetch_done_rx) = tokio::sync::mpsc::unbounded_channel();
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
            tracing::warn!(
                target: "ducktape::node",
                node = %label,
                signal = "SIGTERM",
                error = %e,
                reason = "signal_handler_install_failed",
                "graceful-quit checkpoint disabled"
            );
            None
        }
    };
    let mut sigint = match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
    {
        Ok(s) => Some(s),
        Err(e) => {
            tracing::warn!(
                target: "ducktape::node",
                node = %label,
                signal = "SIGINT",
                error = %e,
                reason = "signal_handler_install_failed",
                "graceful-quit checkpoint disabled"
            );
            None
        }
    };
    // the diagnostic task dump (#1386): SIGUSR1 never checkpoints or exits —
    // it just writes tokio's taskdump to `<workspace>/tasks.txt` so a wedged
    // node can say which task it is parked on. only where tokio's unstable
    // taskdump API exists; elsewhere the handler is not installed at all.
    #[cfg(all(
        tokio_unstable,
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    let mut sigusr1 =
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::user_defined1()) {
            Ok(s) => Some(s),
            Err(e) => {
                tracing::warn!(
                    target: "ducktape::node",
                    node = %label,
                    signal = "SIGUSR1",
                    error = %e,
                    reason = "signal_handler_install_failed",
                    "task dump on SIGUSR1 disabled"
                );
                None
            }
        };
    #[cfg(not(all(
        tokio_unstable,
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )))]
    crate::task_dump::log_unsupported(&label);

    // the finalization delivery wake: the engine's reporter pings it the
    // moment a finalized block becomes drainable, and the loop below drains
    // event-driven on it — the periodic drain tick stays only as the backstop.
    let (delivery_wake_tx, mut delivery_wake) = tokio::sync::mpsc::unbounded_channel::<()>();
    node.orderer().set_delivery_wake(delivery_wake_tx.clone());

    let mut runtime = ValidatorRuntime {
        context,
        node,
        orchestrator,
        epoch_spawner,
        last_cert_height,
        latest_floor,
        mesh_oracle,
        mesh_window,
        mesh_book,
        gateway_book,
        media_peers,
        blob_peers,
        blob_client,
        reach_cmd,
        relay_tx,
        gate_outcomes,
        _gate_fwd_keepalive: gate_fwd_keepalive,
        next_seq,
        prev_ckpt,
        signer,
        label,
        validators,
        dev_demo,
        checkpoint_blocks,
        cadence,
        sync_lease,
        stream_hub,
        index,
        blobs,
        metrics,
        status,
        status_public_key,
        coordination,
        workspace,
        next_ops_refresh: context.current(),
        expected,
        applied,
        converged,
        pending_submits,
        pending_relays,
        pending_gates,
        gating,
        validator_relay,
        last_published,
        join_requests,
        blocks_since_checkpoint,
        checkpoint_not_before,
        last_written_root: None,
        last_reach_view,
        last_flush,
        pending_retarget,
        heartbeat_disabled,
        delivery_wake_tx,
        real_work_parked: false,
        last_nudged_view: None,
        last_crank,
        last_nudge,
        workers,
        code_signaller,
        fetch_done_tx,
        fetch_done_rx,
        next_drain: context.current() + DRAIN_TICK,
    };
    // the startup snapshot: the RECOVERED boundary serves on /v1/status the
    // moment the loop exists, not after the first drain.
    runtime.publish_status().await;

    loop {
        // Resolve on whichever signal stream installed. If neither did,
        // this arm remains pending forever.
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

        // the SIGUSR1 task dump: a separate arm from `signalled` above —
        // unlike SIGTERM/SIGINT it never checkpoints or exits, so it must
        // not share `on_signal`'s terminal path.
        #[cfg(all(
            tokio_unstable,
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ))]
        let dumped = async {
            match sigusr1.as_mut() {
                Some(u) => {
                    u.recv().await;
                }
                None => futures::future::pending::<()>().await,
            }
        }
        .fuse();
        #[cfg(all(
            tokio_unstable,
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ))]
        futures::pin_mut!(dumped);
        #[cfg(not(all(
            tokio_unstable,
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        )))]
        let mut dumped = futures::future::pending::<()>().fuse();

        // Keep selection order in one place: signal and the absolute drain
        // deadline must outrank every ingress lane.
        let next_drain = runtime.next_drain;
        futures::select_biased! {
            _ = signalled => runtime.on_signal().await,
            _ = dumped => {
                #[cfg(all(
                    tokio_unstable,
                    target_os = "linux",
                    any(target_arch = "x86_64", target_arch = "aarch64")
                ))]
                crate::task_dump::dump_tasks(&runtime.workspace, &runtime.label).await;
            }
            _ = context.sleep_until(next_drain).fuse() => runtime.on_drain().await,
            wake = delivery_wake.recv().fuse() => {
                // a finalized block is drainable NOW — drain event-driven
                // instead of waiting out the tick. coalesce a finalization
                // burst into one pass; `None` (all senders dropped) cannot
                // happen while `runtime.delivery_wake_tx` lives.
                if wake.is_some() {
                    while delivery_wake.try_recv().is_ok() {}
                    runtime.on_drain().await;
                }
            }
            job = rpc_ingress.next() => {
                if let Some(job) = job {
                    runtime.on_rpc(job).await;
                }
            }
            fwd = gate_fwd_rx.recv().fuse() => {
                // the intro doorbell rang the GATE through the tunnel.
                // `None` cannot spin here: `gate_fwd_keepalive` holds the
                // channel open even when no plane was wired.
                if let Some(fwd) = fwd {
                    runtime.on_gate_forward(fwd).await;
                }
            }
            relayed = relay_ingress.next() => {
                if let Some((peer, bytes)) = relayed {
                    runtime.on_relay(peer, bytes).await;
                }
            }
            cmd = http_ingress.next() => {
                if let Some(cmd) = cmd {
                    runtime.on_http(cmd).await;
                }
            }
            req = sync_state_rx.next() => {
                if let Some(req) = req {
                    runtime.on_sync(req).await;
                }
            }
        }
        // the BUSY block path, event-driven: whatever this turn enqueued
        // (an ingress submit, a relay frame, a drain-arm system op) flushes
        // into a proposed block NOW — unless a batch of ours is already in
        // flight, in which case it aggregates until that batch clears and
        // the delivery wake turns the loop again. no interval anywhere: the
        // network's own agreement speed is the pacer.
        runtime.pump_eager_flush().await;
        // ...and while that real work waits on OUR leadership turn, escort it:
        // nudge the current view's leader to close its idle view now, so
        // rotation reaches us at network speed instead of the 1s idle beat.
        runtime.pump_leader_nudge().await;
    }
}

impl ValidatorRuntime<'_> {
    async fn on_signal(&mut self) -> ! {
        tracing::info!(
            target: "ducktape::node",
            node = %self.label,
            "SIGTERM/SIGINT — graceful checkpoint then exit"
        );
        self.graceful_checkpoint().await;
        std::process::exit(0);
    }

    async fn graceful_checkpoint(&mut self) {
        graceful_checkpoint(&mut self.node, &self.orchestrator, self.next_seq).await;
    }
}

async fn graceful_checkpoint(
    node: &mut ValidatorNode,
    orchestrator: &consensus::ValsetOrchestrator<ed25519::PublicKey>,
    next_seq: u64,
) {
    if let Some(f) = node.finalized() {
        let pos = node.sink_mut().oplog_pos().await;
        if let Ok(manifest) = Manifest::capture(
            node.host(),
            Some(f.height),
            orchestrator.epoch(),
            orchestrator.epoch_base(),
            participant_bytes(orchestrator),
            resident_bytes(orchestrator),
            orchestrator
                .pending_cutover()
                .map(|cutover| cutover.cutover_view()),
            pos,
            next_seq,
        ) {
            let _ = node.sink_mut().write_manifest(&manifest).await;
        }
    }
    // no trailing sync: every record the sink writes — pin, pre_apply, seal,
    // cutover — fsyncs where it is written, and `write_manifest` syncs the
    // journal before it puts. there is nothing buffered left to barrier.
}
