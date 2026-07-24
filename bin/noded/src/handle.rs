//! the node-actor command lane ([`NodeCommand`]) and the router's shared
//! state ([`NodeHandle`]): every http handler talks to whichever actor owns
//! the non-Send `host::Host` exclusively through this seam.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::http::StatusCode;
use axum::response::Response;
use futures::SinkExt as _;
use futures::channel::{mpsc, oneshot};

use crate::call::CallLane;
use crate::gateway_http::{BrowserGateway, GatewayLane};
use crate::gateway_ws_token::WsTokenStore;
use crate::metrics::NodeMetrics;
use crate::stream::{LogRing, StreamHub};
use crate::{BlockSummary, NodeStatus, OperationalStatus, error_response};

/// inbound command backlog before submit/query callers see backpressure.
pub(crate) const COMMAND_BUFFER: usize = 64;
/// internal block wakeups buffered per lagging websocket subscriber.
pub(crate) const EVENT_BUFFER: usize = 64;

/// a request to the actor that owns the host. replies cross the channel as
/// wire-ready types so the http layer stays free of sdk conversions.
pub enum NodeCommand {
    Submit {
        target: String,
        payload: Vec<u8>,
        /// `Origin::External` bytes for this block (see [`SubmitRequest::origin`]).
        origin: Vec<u8>,
        reply: oneshot::Sender<Result<BlockSummary, String>>,
    },
    /// take custody of an ALREADY-SIGNED op frame (`POST /v1/submit/frame`).
    /// carries the RAW frame bytes: the origin rides INSIDE them as the
    /// signature's verified signer, so no lane consults a caller string and no
    /// lane may re-sign — a validator that re-framed this with its own node key
    /// would destroy the exact authorship the lane exists to carry (an agent's
    /// session key). the bytes are verified before they reach any actor, and
    /// every actor verifies again where it must.
    SubmitFrame {
        frame: Vec<u8>,
        reply: oneshot::Sender<Result<BlockSummary, String>>,
    },
    Query {
        target: String,
        req: Vec<u8>,
        reply: oneshot::Sender<Result<Vec<u8>, String>>,
    },
}

/// the committed facts a `/v1/peers` sample needs from the actor: valset
/// standing (hex key sets), the served height, and the epoch. published
/// beside the status snapshot at the same boundaries; the peer/traffic
/// counters themselves are parsed LIVE from the wired exposition source.
#[derive(Clone, Default)]
pub struct PeersStanding {
    pub validators: std::collections::BTreeSet<String>,
    pub residents: std::collections::BTreeSet<String>,
    pub height: u64,
    pub epoch: Option<u64>,
}

/// the observability snapshot cell: the actor that owns the host PUBLISHES
/// its projections at each boundary it settles — the complete [`NodeStatus`]
/// and the peers standing — and the http handlers read the last ones
/// published without ever crossing the command lane. that read-side
/// independence is the point: a sync/catch-up stage keeps the pump away from
/// its command queue for whole stages, and the observability surface
/// (status, peers, /metrics) must keep answering through exactly that state.
#[derive(Clone, Default)]
pub struct StatusCell {
    inner: Arc<StatusCellInner>,
}

#[derive(Default)]
struct StatusCellInner {
    /// the last-published snapshot. publish swaps the WHOLE struct under one
    /// write, so a read reflects exactly one boundary — never a torn one.
    snapshot: std::sync::RwLock<NodeStatus>,
    /// the last-published peers standing (same whole-struct-swap contract).
    standing: std::sync::RwLock<PeersStanding>,
    /// the live operations source — the metrics' shared projection, wired
    /// once at boot by daemons that register [`NodeMetrics`]. a read overlays
    /// it so phase and sync progress stay live BETWEEN boundary publishes
    /// (they move mid-stage, exactly when no boundary publish can happen).
    /// unwired (simnode), the published operations serve as-is.
    operations: std::sync::OnceLock<Arc<std::sync::RwLock<OperationalStatus>>>,
    /// the live OpenMetrics exposition source — a registry encoder wired once
    /// at boot (the commonware context's `encode`). `/metrics`, `/v1/peers`,
    /// and the ws metrics topic all read it directly; the registry is shared
    /// state, so encoding it never needs the actor.
    exposition: std::sync::OnceLock<Arc<dyn Fn() -> String + Send + Sync>>,
}

impl StatusCell {
    /// publish a complete snapshot — one whole-struct swap.
    pub fn publish(&self, status: NodeStatus) {
        *self
            .inner
            .snapshot
            .write()
            .expect("status snapshot lock poisoned") = status;
    }

    /// publish the peers standing — one whole-struct swap, same contract as
    /// the status snapshot.
    pub fn publish_peers(&self, standing: PeersStanding) {
        *self
            .inner
            .standing
            .write()
            .expect("peers standing lock poisoned") = standing;
    }

    /// the last-published peers standing (zeroed before the first publish —
    /// an empty sample with no roles, the honest pre-boundary answer).
    pub fn peers_standing(&self) -> PeersStanding {
        self.inner
            .standing
            .read()
            .expect("peers standing lock poisoned")
            .clone()
    }

    /// wire the live operations overlay to the metrics' shared projection.
    /// once per process; a second wiring is a programming error.
    pub fn wire_metrics(&self, metrics: &NodeMetrics) {
        self.inner
            .operations
            .set(metrics.operations_handle())
            .expect("status cell operations source wired twice");
    }

    /// wire the live OpenMetrics exposition source (the registry encoder).
    /// once per process; a second wiring is a programming error.
    pub fn wire_exposition(&self, encode: impl Fn() -> String + Send + Sync + 'static) {
        if self.inner.exposition.set(Arc::new(encode)).is_err() {
            panic!("status cell exposition source wired twice");
        }
    }

    /// one live exposition sample, or `None` when no source is wired (a
    /// handle whose embedder registers no metrics — the routes answer 503).
    pub fn exposition(&self) -> Option<String> {
        self.inner.exposition.get().map(|encode| encode())
    }

    /// the current status: the last-published boundary facts, with live
    /// operations overlaid when a metrics source is wired.
    pub fn current(&self) -> NodeStatus {
        let mut status = self
            .inner
            .snapshot
            .read()
            .expect("status snapshot lock poisoned")
            .clone();
        if let Some(operations) = self.inner.operations.get() {
            status.operations = operations
                .read()
                .expect("operations lock poisoned")
                .clone();
        }
        status
    }
}

/// the router's shared state: a command lane into the node actor, the
/// stream hub for websocket subscribers, the shutdown signal, and the
/// node-local blob store the files module shares.
#[derive(Clone)]
pub struct NodeHandle {
    pub(crate) cmds: mpsc::Sender<NodeCommand>,
    /// the `/v1/status` snapshot the owning actor publishes into; the status
    /// route reads it directly (the one read that never crosses `cmds`).
    pub(crate) status: StatusCell,
    pub(crate) hub: StreamHub,
    pub(crate) shutdown: tokio::sync::watch::Sender<bool>,
    /// the files blob lane. NOT a command into the actor: chunk bytes stay
    /// node-local by design (never consensus state, never an op), so the http
    /// handlers read/write this store directly.
    pub(crate) blobs: crate::blobs::BlobHandle,
    /// the forge module's on-disk repo base dir (`<storage>/<forge subdir>`);
    /// each named repo lives at `<forge_repo>/<name>` as a real libgit2 repo.
    /// threaded in so the git upload-pack (clone/fetch) handler can open a repo
    /// READ-ONLY and serve its objects — the ONE route that reads forge's git
    /// substrate directly instead of over the actor lane. `None` on a handle
    /// that never serves the git lane (the router tests' fake actor), which
    /// makes upload-pack a clean 500 there rather than a panic.
    pub(crate) forge_repo: Option<PathBuf>,
    /// the per-module derived index (fluent31-backed read models). node-local
    /// like `blobs`: the actor is the one WRITER as blocks commit;
    /// the `/v1/index/*` handlers read it directly through MVCC snapshots, so
    /// an index scan never crosses the actor command lane. `None` on a handle
    /// whose embedder configured no index (the router tests' fake actor) —
    /// index routes answer 503 there.
    pub(crate) index: Option<Arc<indexer::IndexStore>>,
    /// the call hub's session-request lane. `None` on daemons without a mesh
    /// (the embedded daemon, router tests) — `/v1/call/ws` answers 503 there.
    pub(crate) call: Option<CallLane>,
    /// Purpose-specific gateway request lane. No raw peer, filesystem, or
    /// arbitrary socket proxy is exposed through the client surface.
    pub(crate) gateway: Option<GatewayLane>,
    /// Dedicated least-privilege browser origin for gateway rendering. It is
    /// a separate loopback listener, never the node API origin.
    pub(crate) browser_gateway: Option<BrowserGateway>,
    /// the root dir the duckfs workspace RPC materializes managed checkouts
    /// under (`<storage>/duckfs-workspaces`). node-local disk state, threaded in
    /// like `forge_repo`; `None` on a handle that never serves the seam (the
    /// router tests' fake handle), which makes `/v1/fs/workspaces` a clean 503.
    pub(crate) duckfs_workspaces: Option<PathBuf>,
    /// the node's code-plane stage lane (module-code fan-out). `None` on a
    /// daemon without a mesh — the admin stage route answers 503 there.
    pub(crate) code_stage: Option<crate::module_code::CodeStageLane>,
    /// the owner-gated control namespace's exposure + ownership config (ADR
    /// A2/A5). the default (`Loopback`, no node key) is the embedded daemon's
    /// loopback-trust surface; the full node overrides it via [`Self::with_admin`].
    pub(crate) admin: crate::admin::AdminConfig,
    /// the node-local interactive terminal-session manager. `None` on a handle
    /// that never wires one (router tests, an embedder that omits it) — the
    /// `/v1/term/*` routes answer 503 there and ws `TermInput`/`TermResize` are
    /// no-ops. off-chain, node-local: never consensus state.
    pub(crate) terminals: Option<crate::term::TerminalSessions>,
    /// the guest-side remote-session request lane into the overlay client half
    /// (mirrors [`Self::gateway`]). `None` on a handle without a mesh — a cross-
    /// node create answers 503 there. off-chain, like the gateway lane.
    pub(crate) session_lane: Option<crate::term_remote::SessionLane>,
    /// the guest-side session-id → host-node registry. Always present (Default):
    /// a remote create remembers its host here; the ws input/resize handlers read
    /// it to pick the forward lane over the absent local session.
    pub(crate) remote_sessions: crate::term_remote::RemoteSessions,
}

impl NodeHandle {
    /// build the handle plus the actor-side ends: the command receiver the
    /// actor drains and the stream hub it publishes finalized blocks on.
    /// the blob store is born here — BEFORE genesis — so the embedding daemon
    /// can hand [`Self::blob_handle`] clones to forge and its block loop.
    pub fn channel() -> (Self, mpsc::Receiver<NodeCommand>, StreamHub) {
        Self::channel_with_log_ring(LogRing::default())
    }

    /// same as [`Self::channel`], but uses a caller-created log ring so a
    /// tracing layer can feed the same ring before the handle is fully wired.
    pub fn channel_with_log_ring(logs: LogRing) -> (Self, mpsc::Receiver<NodeCommand>, StreamHub) {
        let (cmd_tx, cmd_rx) = mpsc::channel(COMMAND_BUFFER);
        let hub = StreamHub::with_log_ring(EVENT_BUFFER, logs);
        let handle = Self {
            cmds: cmd_tx,
            status: StatusCell::default(),
            hub: hub.clone(),
            shutdown: tokio::sync::watch::channel(false).0,
            blobs: crate::blobs::BlobHandle::default(),
            forge_repo: None,
            index: None,
            call: None,
            gateway: None,
            browser_gateway: None,
            duckfs_workspaces: None,
            code_stage: None,
            admin: crate::admin::AdminConfig::default(),
            terminals: None,
            session_lane: None,
            remote_sessions: crate::term_remote::RemoteSessions::default(),
        };
        (handle, cmd_rx, hub)
    }

    /// swap the blob store for a persistent one rooted at `root` (write-
    /// through to `<root>/<sha256-hex>`, disk fallback on a memory miss) so
    /// node-local blobs — an agent's registered prompt above all — survive a
    /// daemon restart. still never consensus state, never in any root. must
    /// run BEFORE any [`Self::blob_handle`] clone is handed out (the daemons
    /// chain it right after [`Self::channel`]); an unusable root is a loud
    /// startup error, not a silently-forgetful store.
    pub fn with_blob_root(mut self, root: impl Into<PathBuf>) -> std::io::Result<Self> {
        self.blobs = crate::blobs::BlobHandle::persistent(root)?;
        Ok(self)
    }

    /// point this handle at the forge module's on-disk repo base dir so the git
    /// upload-pack (clone/fetch) handler can open `<forge_repo>/<name>` and serve
    /// its objects. the daemon passes the SAME base it hands `Forge::with_blobs`,
    /// so the http fetch lane reads exactly the repos consensus materializes.
    pub fn with_forge_repo(mut self, base: impl Into<PathBuf>) -> Self {
        self.forge_repo = Some(base.into());
        self
    }

    /// point this handle at the per-module derived index so the `/v1/index/*`
    /// routes can serve snapshot reads. the daemon passes the SAME store its
    /// actor feeds block-by-block.
    pub fn with_index_store(mut self, index: Arc<indexer::IndexStore>) -> Self {
        self.index = Some(index);
        self
    }

    /// point this handle at a call hub's session-request lane so
    /// `/v1/call/ws` can open huddle sessions. only the p2p validator
    /// wires one — it owns the mesh the audio/video rides.
    pub fn with_call(mut self, call: CallLane) -> Self {
        self.call = Some(call);
        self
    }

    /// point this handle at the node's code-plane stage lane so the
    /// module-code admin route can fan staged artifacts out to members.
    /// only the p2p validator wires one — it owns the overlay the plane rides.
    pub fn with_code_stage(mut self, lane: crate::module_code::CodeStageLane) -> Self {
        self.code_stage = Some(lane);
        self
    }

    /// configure the owner-gated control namespace (ADR A2/A5). the full node
    /// passes its own consensus key (the `BindNode` subject ownership resolves
    /// against) and the exposure the operator chose; a daemon that leaves this
    /// at the default serves the loopback-trust surface.
    pub fn with_admin(mut self, admin: crate::admin::AdminConfig) -> Self {
        self.admin = admin;
        self
    }

    /// wire the node-local interactive terminal-session manager so the
    /// `/v1/term/*` routes and the ws `TermInput`/`TermResize` handlers can
    /// reach it. only the daemon wires one; a handle without it 503s the
    /// routes.
    pub fn with_terminals(mut self, terminals: crate::term::TerminalSessions) -> Self {
        self.terminals = Some(terminals);
        self
    }

    /// the terminal-session manager, if one is wired.
    pub(crate) fn terminals(&self) -> Option<&crate::term::TerminalSessions> {
        self.terminals.as_ref()
    }

    /// wire the guest-side remote-session request lane so a cross-node create/
    /// close/input can reach the overlay client half. only the daemon that owns a
    /// mesh wires one; a handle without it 503s a cross-node create.
    pub fn with_session_lane(mut self, lane: crate::term_remote::SessionLane) -> Self {
        self.session_lane = Some(lane);
        self
    }

    /// the guest-side remote-session request lane, if one is wired.
    pub(crate) fn session_lane(&self) -> Option<&crate::term_remote::SessionLane> {
        self.session_lane.as_ref()
    }

    /// the guest-side session-id → host-node registry (always present).
    pub(crate) fn remote_sessions(&self) -> &crate::term_remote::RemoteSessions {
        &self.remote_sessions
    }

    /// Point gateway requests at the full node's authenticated overlay
    /// stream. `net.duck` remains a local network-content read.
    pub fn with_gateway(mut self, lane: GatewayLane) -> Self {
        self.gateway = Some(lane);
        self
    }

    /// Enable gateway browsing on a separately bound loopback listener. The
    /// caller binds first so port 0 becomes an actual reportable port.
    pub fn with_browser_gateway(mut self, listen: SocketAddr) -> Self {
        self.browser_gateway = Some(BrowserGateway {
            listen,
            ws_tokens: Arc::new(WsTokenStore::new()),
        });
        self
    }

    /// point this handle at the root dir the duckfs workspace RPC manages
    /// checkouts under. the daemon passes `<storage>/duckfs-workspaces`; an
    /// unset root makes `/v1/fs/workspaces` answer 503.
    pub fn with_duckfs_workspaces(mut self, root: impl Into<PathBuf>) -> Self {
        self.duckfs_workspaces = Some(root.into());
        self
    }

    /// the blob store this surface serves. the daemon hands clones to forge
    /// (push packfiles) and its block loop (op receipts) so http uploads land
    /// exactly where those consumers read.
    pub fn blob_handle(&self) -> crate::blobs::BlobHandle {
        self.blobs.clone()
    }

    /// a clone of the command lane's sender, for embedder-side producers
    /// that inject commands exactly as the http layer does — the oracle
    /// pool's completed provider runs re-enter as `Submit` commands here.
    /// the loopback base URL of this node's browser gateway (`http://<addr>`),
    /// or `None` when none is wired. It is the `via` a per-run airlock config
    /// routes credential traffic through onto the overlay gateway plane; a node
    /// without it cannot host a lent-credential run.
    pub fn browser_gateway_url(&self) -> Option<String> {
        self.browser_gateway
            .as_ref()
            .map(|gw| format!("http://{}", gw.listen))
    }

    pub fn command_sender(&self) -> mpsc::Sender<NodeCommand> {
        self.cmds.clone()
    }

    /// the `/v1/status` snapshot cell — the owning actor keeps a clone and
    /// publishes into it at every boundary it settles.
    pub fn status_cell(&self) -> StatusCell {
        self.status.clone()
    }

    /// the multiplexed stream hub backing `/v1/ws`.
    pub fn stream_hub(&self) -> StreamHub {
        self.hub.clone()
    }

    pub(crate) fn stream_index(&self) -> Option<Arc<indexer::IndexStore>> {
        self.index.clone()
    }

    /// Publish a durable shutdown state to every current and future surface.
    /// Request graceful shutdown; embedders (simnode lib) call this for teardown.
    pub fn request_shutdown(&self) {
        self.shutdown.send_replace(true);
    }

    /// resolves once a client asked the daemon to exit (POST /v1/admin/shutdown).
    pub async fn shutdown_requested(&self) {
        let mut shutdown = self.shutdown.subscribe();
        if *shutdown.borrow() {
            return;
        }
        let _ = shutdown.changed().await;
    }

    pub(crate) async fn send(&self, cmd: NodeCommand) -> Result<(), Response> {
        let mut cmds = self.cmds.clone();
        cmds.send(cmd)
            .await
            .map_err(|_| error_response(StatusCode::SERVICE_UNAVAILABLE, "node actor is gone"))
    }
}
