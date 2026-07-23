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
use crate::stream::{LogRing, StreamHub};
use crate::{BlockSummary, NodeStatus, error_response};

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
    Status {
        reply: oneshot::Sender<NodeStatus>,
    },
    /// sample the direct-peer projection (`GET /v1/peers`): the actor owns
    /// the metrics registry the sample is parsed from, so this read crosses
    /// the command lane like every other.
    Peers {
        reply: oneshot::Sender<crate::peers::PeersView>,
    },
    /// encode the runtime's Prometheus registry (commonware runtime metrics plus
    /// the daemon's own `ducktape_*` series) to the OpenMetrics text exposition.
    /// the actor owns the commonware context that holds the registry, so this,
    /// like every other read, crosses the command lane.
    Metrics {
        reply: oneshot::Sender<String>,
    },
}

/// the router's shared state: a command lane into the node actor, the
/// stream hub for websocket subscribers, the shutdown signal, and the
/// node-local blob store the files module shares.
#[derive(Clone)]
pub struct NodeHandle {
    pub(crate) cmds: mpsc::Sender<NodeCommand>,
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
