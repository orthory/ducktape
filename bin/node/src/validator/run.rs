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

use super::announce::{CapabilityAnnouncer, ReadinessSignaller};
use crate::constants::{DRAIN_TICK, MAX_PROTOCOL_VERSION};
use crate::host_reads::read_upgrade_version_fields;
use crate::rpc::{JoinRequestRecord, RpcJob, spawn_rpc_listener};
use crate::sync::serve::SyncStateRequest;
use crate::util::{participant_bytes, resident_bytes};
use crate::{oracle_pool, relay_runtime, voice_plane};

pub(super) type ValidatorNode = node::OrderedNode<
    consensus::SimplexOrderer,
    recovery::Recovery<commonware_runtime::tokio::Context>,
>;

/// a join gate held open awaiting its `Redeem` frame's consensus fate (ADR
/// §3.2). the member submitted the redemption and holds the joiner's
/// `Admitted`/`Rejected` reply keyed by the frame id until `on_drain` resolves
/// it — the settle-then-answer seam, mirroring `pending_relays`.
struct GatePending {
    /// the lobby-channel connection the reply goes back to.
    peer: ed25519::PublicKey,
    /// the joiner key, to clear the `gating` in-flight index on resolution.
    joiner: Vec<u8>,
    /// the packed coord cap to deliver on `Admitted` (private coordination).
    cap: Option<Vec<u8>>,
    /// answer `Busy` (non-terminal) once past this instant (§3.2 timeout).
    deadline: std::time::Instant,
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
    pub(super) lobby_tx: super::MeshSender,
    pub(super) relay_tx: super::MeshSender,
    pub(super) sync_state_rx: futures::channel::mpsc::Receiver<SyncStateRequest>,
    pub(super) lobby_ingress: futures::channel::mpsc::Receiver<(ed25519::PublicKey, Vec<u8>)>,
    pub(super) relay_ingress: futures::channel::mpsc::Receiver<(ed25519::PublicKey, Vec<u8>)>,
    pub(super) next_seq: u64,
    pub(super) prev_ckpt: (Option<u64>, u64),
    pub(super) signer: ed25519::PrivateKey,
    pub(super) label: String,
    pub(super) namespace: Vec<u8>,
    pub(super) peers: Vec<ed25519::PublicKey>,
    pub(super) validators: Vec<ed25519::PublicKey>,
    pub(super) dev_demo: bool,
    pub(super) checkpoint_blocks: u64,
    pub(super) announce_capabilities: bool,
    pub(super) sandbox: capability_host::SandboxBackend,
    pub(super) sandbox_capacity: std::collections::BTreeMap<String, u64>,
    pub(super) rpc_listener: Option<std::net::TcpListener>,
    pub(super) http_cmds: futures::channel::mpsc::Receiver<noded::NodeCommand>,
    pub(super) stream_hub: noded::StreamHub,
    pub(super) index: std::sync::Arc<indexer::IndexStore>,
    pub(super) blobs: noded::blobs::BlobHandle,
    pub(super) agent_provisioner: dispatch_oracle::SharedProvisioner,
    pub(super) agent_dirs: capability_host::AgentDirs,
    pub(super) metrics: noded::NodeMetrics,
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
    lobby_tx: super::MeshSender,
    relay_tx: super::MeshSender,
    next_seq: u64,
    prev_ckpt: (Option<u64>, u64),
    signer: ed25519::PrivateKey,
    label: String,
    namespace: Vec<u8>,
    peers: Vec<ed25519::PublicKey>,
    validators: Vec<ed25519::PublicKey>,
    dev_demo: bool,
    checkpoint_blocks: u64,
    announce_capabilities: bool,
    stream_hub: noded::StreamHub,
    index: std::sync::Arc<indexer::IndexStore>,
    blobs: noded::blobs::BlobHandle,
    metrics: noded::NodeMetrics,
    status_public_key: String,
    coordination: crate::config::Coordination,
    expected: usize,
    applied: usize,
    converged: bool,
    pending_submits: std::collections::HashMap<
        node::FrameId,
        (
            futures::channel::oneshot::Sender<Result<noded::BlockSummary, String>>,
            std::time::Instant,
        ),
    >,
    pending_relays:
        std::collections::HashMap<node::FrameId, (ed25519::PublicKey, std::time::Instant)>,
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
    last_flush: std::time::Instant,
    pending_retarget: Option<reachability::MeshEpochEvent>,
    heartbeat_disabled: bool,
    last_crank: std::time::Instant,
    last_nudge: std::time::Instant,
    workers: Vec<Box<dyn host::worker::Worker>>,
    signaller: ReadinessSignaller,
    code_signaller: super::code_announce::CodeReadinessSignaller,
    /// completed pending-swap code fetches, reaped at each readiness pump so
    /// a failed fetch retries next tick (the sender rides in each task).
    fetch_done_tx: tokio::sync::mpsc::UnboundedSender<[u8; 32]>,
    fetch_done_rx: tokio::sync::mpsc::UnboundedReceiver<[u8; 32]>,
    announcer: CapabilityAnnouncer,
    upgrade_armed_latch: Option<(String, u32)>,
    upgrade_pending_seen: Option<String>,
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
        lobby_tx,
        relay_tx,
        mut sync_state_rx,
        mut lobby_ingress,
        mut relay_ingress,
        next_seq,
        prev_ckpt,
        signer,
        label,
        namespace,
        peers,
        validators,
        dev_demo,
        checkpoint_blocks,
        announce_capabilities,
        sandbox,
        sandbox_capacity,
        rpc_listener,
        http_cmds,
        stream_hub,
        index,
        blobs,
        agent_provisioner,
        agent_dirs,
        metrics,
        status_public_key,
        coordination,
    } = state;
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
            std::time::Instant,
        ),
    > = std::collections::HashMap::new();
    // relayed submits held for a wire answer, keyed like pending_submits by
    // the frame's content address: resolved by the SAME drain that resolves
    // local holds, expired on the same SUBMIT_HOLD budget. the peer is where
    // the Reply goes.
    let pending_relays: std::collections::HashMap<
        node::FrameId,
        (ed25519::PublicKey, std::time::Instant),
    > = std::collections::HashMap::new();
    // join gates held open awaiting their Redeem frame's consensus fate, and
    // the joiner→frame in-flight index that dedups a re-Request while settling.
    let pending_gates: std::collections::HashMap<node::FrameId, GatePending> =
        std::collections::HashMap::new();
    let gating: std::collections::HashMap<Vec<u8>, node::FrameId> =
        std::collections::HashMap::new();
    let validator_relay = relay_runtime::ValidatorRelay::new(blobs.clone());
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
    // the per-block-time flush cadence: packs the window's enqueued frames
    // (real ops and/or an idle nop) into one batch block. see the flush loop.
    let last_flush = std::time::Instant::now();
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
    let last_crank = std::time::Instant::now();
    // throttle for the dispatch delivery-nudge pump below.
    let last_nudge = std::time::Instant::now();
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
    let providers = capability_host::discover(
        signer.public_key().as_ref(),
        agent_dirs.clone(),
        Some(stream_hub.run_output().output_sink()),
        // the operator's `node.toml sandbox` choice: Direct (default) or a
        // Podman container that enforces this node's announced capacity.
        sandbox,
        // headless: no forced private netns (honors DUCKTAPE_SANDBOX_PRIVATE_NET).
        false,
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
    let (oracle_worker, _oracle_control, mut oracle_results) = oracle_pool::build(
        context,
        providers,
        signer.public_key().as_ref().to_vec(),
        agent_provisioner.clone(),
        // the announced capacity IS the pool's ledger — one source, so the
        // scheduler never promises what this node can't seat.
        sandbox_capacity.clone(),
    );
    let workers: Vec<Box<dyn host::worker::Worker>> = vec![oracle_worker];
    // the readiness self-signaller: polls COMMITTED upgrade state between drains
    // and emits ONE truthful validator-origin `SignalReady` per pending upgrade
    // this binary can execute. survives restart/late-join (state-driven, not a
    // one-shot effect). inert before the module is registered.
    let native_v1_compat = node.host().state_schema_fingerprint()
        == crate::constants::native_v1_state_schema_fingerprint();
    let signaller = ReadinessSignaller::new(
        if native_v1_compat {
            1
        } else {
            MAX_PROTOCOL_VERSION
        },
        signer.public_key().as_ref().to_vec(),
    );
    // the CODE readiness self-signaller: the byte-receipt twin of the
    // above for pending modreg swaps — verifies (or fetches) the committed
    // component bytes and emits one truthful `SignalReady` per swap.
    let code_signaller =
        super::code_announce::CodeReadinessSignaller::new(signer.public_key().as_ref().to_vec());
    let code_signaller = if native_v1_compat {
        code_signaller.fetch_only()
    } else {
        code_signaller
    };
    let (fetch_done_tx, fetch_done_rx) = tokio::sync::mpsc::unbounded_channel();
    // the capability self-announcer: publishes this node's discovered
    // provider set into the capability registry once (state-driven,
    // idempotent). inert when this host installed no executor CLIs.
    let announcer = CapabilityAnnouncer::new(
        signer.public_key().as_ref().to_vec(),
        my_capabilities,
        sandbox_capacity,
    );
    // one-shot upgrade transition markers keyed off COMMITTED upgrade state,
    // modeled on the `converged` latch: `upgrade armed …` fires when readiness
    // first reaches R==n (every current boundary member signaled) for the
    // pending upgrade — the pre-boundary observable the e2e keys on; `upgrade
    // cleared …` fires when a previously-observed pending clears (the boundary
    // `Advance` reconciliation at H, on ARM or ABORT). the boundary crossing
    // itself prints the `upgrade activated …` / `upgrade aborted …` verdict.
    let upgrade_armed_latch: Option<(String, u32)> = None;
    let upgrade_pending_seen: Option<String> = None;

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
        lobby_tx,
        relay_tx,
        next_seq,
        prev_ckpt,
        signer,
        label,
        namespace,
        peers,
        validators,
        dev_demo,
        checkpoint_blocks,
        announce_capabilities,
        stream_hub,
        index,
        blobs,
        metrics,
        status_public_key,
        coordination,
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
        last_crank,
        last_nudge,
        workers,
        signaller,
        code_signaller,
        fetch_done_tx,
        fetch_done_rx,
        announcer,
        upgrade_armed_latch,
        upgrade_pending_seen,
        next_drain: context.current() + DRAIN_TICK,
    };

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
            announce = lobby_ingress.next() => {
                if let Some((peer, bytes)) = announce {
                    runtime.on_lobby(peer, bytes).await;
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
    }
}

impl ValidatorRuntime<'_> {
    async fn on_signal(&mut self) -> ! {
        println!(
            "[node {}] SIGTERM/SIGINT — graceful checkpoint then exit",
            self.label
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
        let (cv, pu) = read_upgrade_version_fields(node.host()).await;
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
            cv,
            pu,
            pos,
            next_seq,
        ) {
            let _ = node.sink_mut().write_manifest(&manifest).await;
        }
    }
    let _ = node.sink_mut().sync().await;
}
