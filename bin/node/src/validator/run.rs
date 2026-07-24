//! The validator consensus pump.
//!
//! The biased selector stays in one small coordinator so ordering is explicit.
//! Each event is handled by a focused method over one loop-owned runtime state.

mod drain;
mod ingress;
mod sync;

use commonware_cryptography::{Signer, ed25519};
use commonware_runtime::Clock;
use futures::{FutureExt as _, StreamExt as _};

use recovery::Manifest;

use super::announce::CapabilityAnnouncer;
use crate::constants::DRAIN_TICK;
use crate::reachability_plane::GateOutcomes;
use crate::rpc::{JoinRequestRecord, RpcJob, spawn_rpc_listener};
use crate::sync::serve::SyncStateRequest;
use crate::util::{participant_bytes, resident_bytes};
use crate::{lobby, oracle_pool, relay_runtime, voice_plane};

pub(super) type ValidatorNode = node::OrderedNode<
    consensus::SimplexOrderer,
    recovery::Recovery<commonware_runtime::tokio::Context>,
>;

/// a join gate held open awaiting its `Redeem` frame's consensus fate (ADR
/// §3.2). the member submitted the redemption and holds the joiner's outcome
/// keyed by the frame id until `on_drain` resolves it into `gate_outcomes` —
/// the settle-then-answer seam, mirroring `pending_relays`. no mesh `peer`
/// rides here any more (join ADR §4): the answer goes back over the tunnel
/// doorbell, read from the shared map on the joiner's next retransmit.
struct GatePending {
    /// the joiner key: clears the `gating` in-flight index and keys the
    /// outcome write on resolution.
    joiner: Vec<u8>,
    /// the packed coord cap to deliver on `Admitted` (private coordination).
    cap: Option<Vec<u8>>,
    /// answer `Busy` (non-terminal) once past this instant (§3.2 timeout).
    deadline: std::time::SystemTime,
}

/// write a resolved gate outcome where the intro doorbell reads it — the
/// shared map the joiner's next retransmit is answered from (join ADR §4).
fn settle_gate(outcomes: &GateOutcomes, joiner: Vec<u8>, reply: lobby::IntroReply) {
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
        version: env!("CARGO_PKG_VERSION").into(),
        root_hash: crate::util::hex(&node.root_hash()),
        height: node.finalized().map(|f| f.height).unwrap_or(0),
        modules,
        public_key: status_public_key.into(),
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
    pub(super) mesh_oracle: commonware_p2p::authenticated::discovery::Oracle<ed25519::PublicKey>,
    pub(super) gateway_book: Option<std::sync::Arc<crate::gateway_plane::OverlayBook>>,
    pub(super) media_peers: Option<std::sync::Arc<voice_plane::MediaPeers>>,
    pub(super) blob_peers: std::sync::Arc<std::sync::RwLock<Vec<ed25519::PublicKey>>>,
    pub(super) blob_client: crate::blob_fetch::ServeLaneBlobClient<super::MeshSender>,
    pub(super) reach_cmd: Option<tokio::sync::mpsc::Sender<reachability::ReachabilityCommand>>,
    pub(super) relay_tx: super::MeshSender,
    pub(super) sync_state_rx: futures::channel::mpsc::Receiver<SyncStateRequest>,
    /// the join GATE's forward lane from the intro doorbell (join ADR §4):
    /// verified gate requests the reachability plane rang through.
    pub(super) gate_fwd_rx: tokio::sync::mpsc::Receiver<lobby::GateForward>,
    /// a never-sending clone of the forward lane's sender, held so the select
    /// arm stays PENDING (instead of None-spinning) when no reachability
    /// plane was wired to ring the doorbell.
    pub(super) gate_fwd_keepalive: tokio::sync::mpsc::Sender<lobby::GateForward>,
    /// where the drain writes each settled gate outcome; the doorbell reads
    /// it on the joiner's next retransmit.
    pub(super) gate_outcomes: GateOutcomes,
    pub(super) relay_ingress: futures::channel::mpsc::Receiver<(ed25519::PublicKey, Vec<u8>)>,
    pub(super) next_seq: u64,
    pub(super) prev_ckpt: (Option<u64>, u64),
    pub(super) signer: ed25519::PrivateKey,
    pub(super) label: String,
    pub(super) peers: Vec<ed25519::PublicKey>,
    pub(super) validators: Vec<ed25519::PublicKey>,
    pub(super) dev_demo: bool,
    pub(super) checkpoint_blocks: u64,
    /// sync retention lease (unix secs of the last served state-sync request)
    /// — the drain defers oplog pruning while it is fresh.
    pub(super) sync_lease: std::sync::Arc<std::sync::atomic::AtomicU64>,
    pub(super) announce_capabilities: bool,
    pub(super) sandbox: Option<capability_host::SandboxBackend>,
    pub(super) sandbox_capacity: std::collections::BTreeMap<String, u64>,
    pub(super) rpc_listener: Option<std::net::TcpListener>,
    pub(super) http_cmds: futures::channel::mpsc::Receiver<noded::NodeCommand>,
    pub(super) stream_hub: noded::StreamHub,
    pub(super) index: std::sync::Arc<indexer::IndexStore>,
    pub(super) blobs: noded::blobs::BlobHandle,
    pub(super) agent_provisioner: dispatch_host::SharedProvisioner,
    pub(super) cred_resolver: dispatch_host::SharedCredentialResolver,
    pub(super) agent_dirs: capability_host::AgentDirs,
    pub(super) metrics: noded::NodeMetrics,
    pub(super) status: noded::StatusCell,
    pub(super) status_public_key: String,
    pub(super) coordination: crate::config::Coordination,
}

struct ValidatorRuntime<'a> {
    context: &'a commonware_runtime::tokio::Context,
    node: ValidatorNode,
    orchestrator: consensus::ValsetOrchestrator<ed25519::PublicKey>,
    epoch_spawner: super::engine::EpochSpawner<'a>,
    last_cert_height: Option<u64>,
    latest_floor: Option<recovery::FloorCert>,
    mesh_oracle: commonware_p2p::authenticated::discovery::Oracle<ed25519::PublicKey>,
    gateway_book: Option<std::sync::Arc<crate::gateway_plane::OverlayBook>>,
    media_peers: Option<std::sync::Arc<voice_plane::MediaPeers>>,
    blob_peers: std::sync::Arc<std::sync::RwLock<Vec<ed25519::PublicKey>>>,
    blob_client: crate::blob_fetch::ServeLaneBlobClient<super::MeshSender>,
    reach_cmd: Option<tokio::sync::mpsc::Sender<reachability::ReachabilityCommand>>,
    relay_tx: super::MeshSender,
    gate_outcomes: GateOutcomes,
    _gate_fwd_keepalive: tokio::sync::mpsc::Sender<lobby::GateForward>,
    next_seq: u64,
    prev_ckpt: (Option<u64>, u64),
    signer: ed25519::PrivateKey,
    label: String,
    peers: Vec<ed25519::PublicKey>,
    validators: Vec<ed25519::PublicKey>,
    dev_demo: bool,
    checkpoint_blocks: u64,
    sync_lease: std::sync::Arc<std::sync::atomic::AtomicU64>,
    announce_capabilities: bool,
    stream_hub: noded::StreamHub,
    index: std::sync::Arc<indexer::IndexStore>,
    blobs: noded::blobs::BlobHandle,
    metrics: noded::NodeMetrics,
    status: noded::StatusCell,
    status_public_key: String,
    coordination: crate::config::Coordination,
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
    announcer: CapabilityAnnouncer,
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
        peers,
        validators,
        dev_demo,
        checkpoint_blocks,
        sync_lease,
        announce_capabilities,
        sandbox,
        sandbox_capacity,
        rpc_listener,
        http_cmds,
        stream_hub,
        index,
        blobs,
        agent_provisioner,
        cred_resolver,
        agent_dirs,
        metrics,
        status,
        status_public_key,
        coordination,
    } = state;
    // the local rpc bridge: blocking listener threads push parsed requests
    // into this bounded queue; the pump answers between drains.
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
    // the last absolute view ticked to the reachability plane — one
    // ViewTick per actual advance, not one per 100ms drain pass.
    let last_reach_view: Option<u64> = None;
    // the IDLE beat grid: when the last flush (or restamp) happened, pacing
    // the one-nop-per-BLOCK_TIME idle heartbeat. busy flushing is event-driven
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
    // no `node.toml [sandbox]` = no compute plane: nothing is discovered or
    // announced and the pool below has nothing it could ever spawn — a bare
    // host spawn is unrepresentable.
    let providers = match sandbox {
        Some(backend) => capability_host::discover(
            signer.public_key().as_ref(),
            agent_dirs.clone(),
            Some(stream_hub.run_output().output_sink()),
            backend,
            // headless: no forced private netns (honors DUCKTAPE_SANDBOX_PRIVATE_NET).
            false,
        )
        .unwrap_or_else(|e| panic!("capability specs failed to load: {e}")),
        None => capability_host::ProviderSet::empty(),
    };
    let my_capabilities = providers.capabilities();
    // OFF-LOOP execution: the pool gates effects inline (lease check —
    // WorkerRequests leased to another node's key are skipped, not
    // double-run — under this node's submit key) but runs the provider
    // CLI on spawned background tasks; completed results come back over
    // `oracle_results` (an ingress arm below) and re-enter the ordered
    // lane as ordinary signed submits, so a minutes-long run never
    // stalls the drain/rpc/heartbeat arms of this loop.
    let (oracle_worker, _oracle_control, mut oracle_results) = oracle_pool::build(
        context,
        providers,
        signer.public_key().as_ref().to_vec(),
        agent_provisioner.clone(),
        // the announced capacity IS the pool's ledger — one source, so the
        // scheduler never promises what this node can't seat.
        sandbox_capacity.clone(),
        Some(cred_resolver.clone()),
    );
    let workers: Vec<Box<dyn host::worker::Worker>> = vec![oracle_worker];
    // the CODE readiness self-signaller for pending code swaps — verifies (or
    // fetches) the committed component bytes and emits one truthful
    // `SignalReady` per swap.
    let code_signaller =
        super::code_announce::CodeReadinessSignaller::new(signer.public_key().as_ref().to_vec());
    let (fetch_done_tx, fetch_done_rx) = tokio::sync::mpsc::unbounded_channel();
    // the capability self-announcer: publishes this node's discovered
    // provider set into the capability registry once (state-driven,
    // idempotent). inert when this host installed no executor CLIs.
    let announcer = CapabilityAnnouncer::new(
        signer.public_key().as_ref().to_vec(),
        my_capabilities,
        sandbox_capacity,
    );
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
        peers,
        validators,
        dev_demo,
        checkpoint_blocks,
        sync_lease,
        announce_capabilities,
        stream_hub,
        index,
        blobs,
        metrics,
        status,
        status_public_key,
        coordination,
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
        announcer,
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

        // Keep selection order in one place: signal and the absolute drain
        // deadline must outrank every ingress lane.
        let next_drain = runtime.next_drain;
        futures::select_biased! {
            _ = signalled => runtime.on_signal().await,
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
            result = oracle_results.next() => {
                if let Some(msg) = result {
                    runtime.on_oracle_result(msg).await;
                }
            }
            fwd = gate_fwd_rx.recv().fuse() => {
                // the intro doorbell rang the GATE through the tunnel (§4).
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
    let _ = node.sink_mut().sync().await;
}
