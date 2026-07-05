//! the daemon's client-facing surface: json wire types, the node-actor command
//! channel, and the axum router.
//!
//! the split matters: `host::Host` is deliberately non-Send (single-threaded by
//! design), so http handlers never touch it. they send a [`NodeCommand`] over a
//! runtime-agnostic futures mpsc channel to whichever actor owns the host — the
//! real one lives in `main.rs` on a commonware tokio runner; router tests drive
//! a fake actor on plain tokio. payloads stay opaque json: a submit/query body
//! carries the module's own `*Msg`/`*Query` enum as a json value, encoded to the
//! exact bytes the `*-interface` crates' `encode_*` helpers would produce
//! (`serde_json::to_vec`), so the daemon needs no per-module knowledge —
//! with ONE deliberate exception: the files blob lane. chunk bytes must never
//! transit consensus (no op carries them), so POST `/v1/files/blob` and GET
//! `/v1/files/blob/{digest}` bypass the actor entirely and talk straight to
//! the node-local [`files::BlobHandle`] the registered files module shares.
//!
//! lifecycle is part of the surface: `/v1/status` carries the daemon's build
//! version (so a newer app can spot a stale orphan), and POST `/v1/shutdown`
//! asks the process to exit gracefully — the managing app has no pid, only
//! this port.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::body::Bytes;
use axum::extract::rejection::BytesRejection;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures::SinkExt as _;
use futures::channel::{mpsc, oneshot};
use sdk::StateRoot;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;

/// inbound command backlog before submit/query callers see backpressure.
pub const COMMAND_BUFFER: usize = 64;
/// block events buffered per lagging websocket subscriber before it skips ahead.
pub const EVENT_BUFFER: usize = 64;

/// one finalized block, as reported to clients (http response + ws frame).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockSummary {
    pub height: u64,
    pub app_hash: String,
}

/// the `/v1/submit` reply: the block that INCLUDED the caller's op, plus the
/// op's content address — sha256 of the exact payload bytes the host committed.
/// the bytes are staged in the node-local blob store under that digest, so
/// `GET /v1/files/blob/{op_hash}` serves them back: the hash is addressable,
/// not just informational.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitReceipt {
    pub height: u64,
    pub app_hash: String,
    pub op_hash: String,
}

/// how many recent telemetry frames the daemon retains for `GET /v1/telemetry`.
/// a client connecting mid-stream pulls this backfill, then follows the ws.
pub const TELEMETRY_RING_CAP: usize = 256;

/// the observability record for one finalized block: the host's DETERMINISTIC
/// dispatch trace decorated with this node's WALL-CLOCK apply latency. rides the
/// ws stream (`WsFrame::Telemetry`) and is buffered in the [`TelemetryRing`] for
/// `GET /v1/telemetry`. keyed by `(height, source)` — the same space the future
/// on-consensus telemetry module will use, so the two planes correlate.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryFrame {
    pub height: u64,
    /// the block's AGREED logical clock (from the block context) — identical on
    /// every node. NOT this node's wall clock.
    pub consensus_time: u64,
    /// node-local cost of applying this block, in microseconds. the ONE
    /// non-deterministic field: it differs per node and never enters consensus.
    pub latency_us: u64,
    /// one entry per module dispatched this block, in drain (causal) order.
    pub dispatches: Vec<DispatchInfo>,
    /// observability events modules emitted during the block, in dispatch order.
    pub events: Vec<TelemetryEvent>,
}

/// one dispatch in a block's drain — the wire twin of `host::DispatchRecord`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchInfo {
    /// the module dispatched this step.
    pub module: String,
    /// what triggered it: `"external"`, `"external:<name>"`, `"system"`, or
    /// `"module:<id>"` for a follow-up.
    pub origin: String,
    /// follow-up `Msg`s this dispatch emitted (its causal fan-out).
    pub emitted_msgs: usize,
    /// observability `Event`s this dispatch emitted.
    pub emitted_events: usize,
}

/// one observability event a module emitted (`Ctx::emit_event`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryEvent {
    /// the module that emitted it.
    pub source: String,
    /// best-effort UTF-8 preview of the (module-defined) payload, capped. binary
    /// payloads render lossily; no module emits events yet, so this is forward
    /// wiring for when they do.
    pub payload: String,
}

/// a bounded, shared ring of the most recent telemetry frames. the node actor
/// pushes from its own thread; the http layer reads from the server runtime, so
/// it is an `Arc<Mutex>` — frames are plain data, cheap to clone out under the
/// lock. drops oldest at [`TELEMETRY_RING_CAP`].
#[derive(Clone, Default)]
pub struct TelemetryRing(Arc<Mutex<VecDeque<TelemetryFrame>>>);

impl TelemetryRing {
    /// append a frame, evicting the oldest once the cap is reached.
    pub fn push(&self, frame: TelemetryFrame) {
        let mut ring = self.0.lock().expect("telemetry ring poisoned");
        if ring.len() == TELEMETRY_RING_CAP {
            ring.pop_front();
        }
        ring.push_back(frame);
    }

    /// the most recent `limit` frames, oldest-first (`None` → all buffered).
    pub fn recent(&self, limit: Option<usize>) -> Vec<TelemetryFrame> {
        let ring = self.0.lock().expect("telemetry ring poisoned");
        let take = limit.map_or(ring.len(), |n| n.min(ring.len()));
        ring.iter().skip(ring.len() - take).cloned().collect()
    }
}

/// how many recent blocks `GET /v1/blocks` serves when the caller names no
/// `limit` — a bounded default page over the durable block index.
pub const BLOCKS_DEFAULT_LIMIT: usize = 256;

/// cap on the payload-preview characters a block record carries.
pub const PAYLOAD_PREVIEW_MAX: usize = 1024;

/// how a block's op landed, as the explorer surface reports it. only
/// journaled outcomes appear (a frame discarded at an epoch cutover is
/// dropped before decoding, so there are no contents to show): an applied op
/// mutated state; a rejected op finalized but rolled back — a failed tx.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BlockDisposition {
    Applied,
    Rejected,
}

/// one non-empty finalized block, as the explorer reads it: the block's
/// consensus coordinates (height, frame content hash, post-block app-hash),
/// its authenticated proposer, and the op it carried with the deterministic
/// dispatch trace. stored as the block's row in the index store's blocks
/// database ([`indexer::BlockOps::record`]) and served by `GET /v1/blocks`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockRecord {
    pub height: u64,
    /// hex of the frame's content address (sha256 over the exact bytes the
    /// orderer carried) — the block's hash on this surface. empty on the
    /// embedded daemon's lane: nothing is framed or signed there, so the
    /// field stays honest rather than carrying a fabricated digest.
    pub hash: String,
    /// hex of the composed app-hash after this block settled — the commit.
    pub commit_hash: String,
    /// hex of the proposing validator's ed25519 public key — the frame's
    /// VERIFIED signer, not a claimed identity. on the embedded daemon's
    /// frameless lane this is the SUBMITTER's origin bytes instead
    /// (unverified — that lane authenticates nothing).
    pub proposer: String,
    pub disposition: BlockDisposition,
    /// the root op's target module.
    pub target: String,
    /// the dispatch trace, in drain order — the "transactions" inside the
    /// block. empty for a rejected op (a deterministic no-op leaves no trace).
    pub operations: Vec<DispatchInfo>,
    /// best-effort utf-8 preview of the root op's payload (module `*Msg` json
    /// on this lane), capped at [`PAYLOAD_PREVIEW_MAX`] chars.
    pub payload: String,
    /// hex of the root op's content address — sha256 of the exact payload
    /// bytes the host committed, staged in the node-local blob store as the
    /// record is ringed, so `GET /v1/files/blob/{op_hash}` serves the full
    /// bytes back. same semantics as [`SubmitReceipt::op_hash`]; this is the
    /// only place the NETWORKED surface exposes it (its submit reply carries
    /// height only until the noded→ordered-node convergence).
    pub op_hash: String,
}

/// the explorer's wire rendering of one dispatch. `Origin::External` renders
/// as plain `"external"`: the block-level `proposer` field already carries
/// the key, and raw ed25519 bytes are not utf-8 (telemetry's
/// `external:<name>` convention assumes the embedded daemon's readable
/// names).
impl From<&host::DispatchRecord> for DispatchInfo {
    fn from(record: &host::DispatchRecord) -> Self {
        DispatchInfo {
            module: record.module.clone(),
            origin: match &record.origin {
                sdk::Origin::External(_) => "external".to_string(),
                sdk::Origin::Module(id) => format!("module:{id}"),
                sdk::Origin::System => "system".to_string(),
            },
            emitted_msgs: record.emitted_msgs,
            emitted_events: record.emitted_events,
        }
    }
}

/// best-effort utf-8 preview of an op payload, capped at
/// [`PAYLOAD_PREVIEW_MAX`] chars. binary bytes render lossily — payloads on
/// this lane are module `*Msg` json, so the common case is readable.
pub fn payload_preview(payload: &[u8]) -> String {
    let text = String::from_utf8_lossy(payload);
    match text.char_indices().nth(PAYLOAD_PREVIEW_MAX) {
        Some((cut, _)) => format!("{}…", &text[..cut]),
        None => text.into_owned(),
    }
}

/// encode a [`BlockRecord`] as its stored index row ([`indexer::BlockOps::record`]).
/// both binaries feed rows through this one seam so `GET /v1/blocks` reads a
/// single shape regardless of which lane wrote it.
pub fn block_row(record: &BlockRecord) -> Vec<u8> {
    serde_json::to_vec(record).expect("a plain record struct serializes")
}

/// the status projection: daemon build version, global app-hash, and each
/// registered module's root.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeStatus {
    pub version: String,
    pub app_hash: String,
    pub height: u64,
    pub modules: Vec<ModuleStatus>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleStatus {
    pub id: String,
    pub root: String,
    pub category: ModuleCategory,
}

/// A module's presentation category — how the app's Modules view groups the
/// registered set. This is catalog metadata the status projection attaches by
/// id; it is not part of a module's consensus identity (that stays `id` +
/// `root`) and never enters the app-hash.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModuleCategory {
    Workspace,
    Developer,
    Automation,
    System,
}

impl ModuleCategory {
    /// The category a module id belongs to. Ids not listed here —
    /// infrastructure and internal modules (files, memory, saga, profiles, kv,
    /// valset, governance, vaults, directory, …) — fall to `System`, so a new
    /// or unknown module always groups sensibly rather than breaking the view.
    pub fn of(id: &str) -> Self {
        match id {
            "chat" | "tasks" | "inbox" | "document" | "pages" => Self::Workspace,
            "forge" | "agent" => Self::Developer,
            "automations" | "jobs" => Self::Automation,
            _ => Self::System,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitRequest {
    pub target: String,
    /// the module's `*Msg` enum as a json value — encoded verbatim into `Msg.payload`.
    pub payload: serde_json::Value,
    /// the submitter identity stamped into `Origin::External` — modules that
    /// derive authorship from origin (chat's `AuthorRef::User`) see these
    /// bytes. optional; the daemon's own name is the fallback. this is a
    /// TRUSTED-CLIENT convention, not authentication: anything that can reach
    /// the port can claim any origin. fine for a local daemon; a public
    /// deployment needs real submitter auth here.
    #[serde(default)]
    pub origin: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryRequest {
    pub target: String,
    /// the module's `*Query` enum as a json value.
    pub query: serde_json::Value,
}

/// a ws frame. tagged so the stream can grow beyond block events without
/// breaking subscribers — clients switch on `type` and ignore unknown kinds.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum WsFrame {
    Block(BlockSummary),
    Telemetry(TelemetryFrame),
}

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
    Query {
        target: String,
        req: Vec<u8>,
        reply: oneshot::Sender<Result<Vec<u8>, String>>,
    },
    Status {
        reply: oneshot::Sender<NodeStatus>,
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
/// block-event fan-out for websocket subscribers, the shutdown signal, and the
/// node-local blob store the files module shares.
#[derive(Clone)]
pub struct NodeHandle {
    cmds: mpsc::Sender<NodeCommand>,
    events: broadcast::Sender<WsFrame>,
    shutdown: std::sync::Arc<tokio::sync::Notify>,
    /// the files blob lane. NOT a command into the actor: chunk bytes stay
    /// node-local by design (never consensus state, never an op), so the http
    /// handlers read/write this store directly.
    blobs: files::BlobHandle,
    /// recent per-block telemetry. like `blobs`, this is node-local and never
    /// crosses the actor command lane: the actor pushes into it as blocks
    /// commit, `GET /v1/telemetry` reads it directly.
    telemetry: TelemetryRing,
    /// the forge module's on-disk repo base dir (`<storage>/<forge subdir>`);
    /// each named repo lives at `<forge_repo>/<name>` as a real libgit2 repo.
    /// threaded in so the git upload-pack (clone/fetch) handler can open a repo
    /// READ-ONLY and serve its objects — the ONE route that reads forge's git
    /// substrate directly instead of over the actor lane. `None` on a handle
    /// that never serves the git lane (the router tests' fake actor), which
    /// makes upload-pack a clean 500 there rather than a panic.
    forge_repo: Option<PathBuf>,
    /// the per-module derived index (fluent31-backed read models). node-local
    /// like `blobs`/`telemetry`: the actor is the one WRITER as blocks commit;
    /// the `/v1/index/*` handlers read it directly through MVCC snapshots, so
    /// an index scan never crosses the actor command lane. `None` on a handle
    /// whose embedder configured no index (the router tests' fake actor) —
    /// index routes answer 503 there.
    index: Option<Arc<indexer::IndexStore>>,
}

impl NodeHandle {
    /// build the handle plus the actor-side ends: the command receiver the
    /// actor drains and the event sender it publishes finalized blocks on.
    /// the blob store is born here — BEFORE genesis — so the embedding daemon
    /// can register its files module over [`Self::blob_handle`].
    pub fn channel() -> (
        Self,
        mpsc::Receiver<NodeCommand>,
        broadcast::Sender<WsFrame>,
    ) {
        let (cmd_tx, cmd_rx) = mpsc::channel(COMMAND_BUFFER);
        let (event_tx, _) = broadcast::channel(EVENT_BUFFER);
        let handle = Self {
            cmds: cmd_tx,
            events: event_tx.clone(),
            shutdown: std::sync::Arc::new(tokio::sync::Notify::new()),
            blobs: files::BlobHandle::default(),
            telemetry: TelemetryRing::default(),
            forge_repo: None,
            index: None,
        };
        (handle, cmd_rx, event_tx)
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

    /// the blob store this surface serves. the daemon constructs its files
    /// module over a clone (`Files::with_blobs`) so http uploads land exactly
    /// where the module's `serve_sync` reads.
    pub fn blob_handle(&self) -> files::BlobHandle {
        self.blobs.clone()
    }

    /// the telemetry ring this surface serves. the daemon actor pushes a frame
    /// into a clone of it as each block commits; `GET /v1/telemetry` reads here.
    pub fn telemetry_ring(&self) -> TelemetryRing {
        self.telemetry.clone()
    }

    /// resolves once a client asked the daemon to exit (POST /v1/shutdown).
    /// `Notify` stores the permit, so a request that lands before anyone awaits
    /// is not lost.
    pub async fn shutdown_requested(&self) {
        self.shutdown.notified().await;
    }

    async fn send(&self, cmd: NodeCommand) -> Result<(), Response> {
        let mut cmds = self.cmds.clone();
        cmds.send(cmd)
            .await
            .map_err(|_| error_response(StatusCode::SERVICE_UNAVAILABLE, "node actor is gone"))
    }
}

/// hex-encode a state root for the wire (stable, greppable, json-friendly).
pub fn hex_root(root: &StateRoot) -> String {
    hex_bytes(&root.0)
}

/// hex-encode arbitrary wire bytes (frame content hashes, proposer keys).
pub fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn error_response(status: StatusCode, message: &str) -> Response {
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}

/// the actor dropped the reply oneshot — it panicked or shut down mid-request.
fn actor_gone() -> Response {
    error_response(
        StatusCode::SERVICE_UNAVAILABLE,
        "node actor dropped the reply",
    )
}

pub fn router(handle: NodeHandle) -> Router {
    Router::new()
        .route("/v1/submit", post(submit))
        .route("/v1/query", post(query))
        .route("/v1/status", get(status))
        .route("/v1/telemetry", get(telemetry))
        .route("/v1/blocks", get(blocks))
        // the derived read-model tier: snapshot reads of the per-module
        // fluent31 indexes the actor materializes as blocks commit.
        .route("/v1/index/status", get(index_status))
        .route("/v1/index/{module}/ops", get(index_ops))
        .route("/v1/index/{module}/scan", get(index_scan))
        // the module's OWN endpoint on the derived tier: an opaque
        // module-defined view request, answered by its registered mapper.
        .route("/v1/index/{module}/view", post(index_view))
        // Prometheus scrape convention: root `/metrics`, not under `/v1`.
        .route("/metrics", get(metrics))
        .route("/v1/shutdown", post(shutdown))
        .route("/v1/ws", get(ws))
        .route(
            "/v1/files/blob",
            // one chunk per request, so the body cap IS the chunk cap. the
            // json routes keep axum's (smaller) default limit.
            post(put_blob).layer(DefaultBodyLimit::max(
                files_interface::MAX_CHUNK_SIZE as usize,
            )),
        )
        .route("/v1/files/blob/{digest}", get(get_blob))
        .route("/forge/{repo}/info/refs", get(git_info_refs))
        // git smart-HTTP: forge is a full push+fetch remote over one route pair.
        //   `git push  http://<node>/forge/<repo> main` — receive-pack (push)
        //   `git clone http://<node>/forge/<repo>`      — upload-pack (fetch)
        // the info/refs advertisement is tiny; both packfile POSTs carry a whole-
        // repo pack, so their body caps are lifted far above the json/chunk
        // defaults.
        .route(
            "/forge/{repo}/git-receive-pack",
            post(git_receive_pack).layer(DefaultBodyLimit::max(GIT_PACK_BODY_LIMIT)),
        )
        .route(
            "/forge/{repo}/git-upload-pack",
            post(git_upload_pack).layer(DefaultBodyLimit::max(GIT_PACK_BODY_LIMIT)),
        )
        // the web app is served from a different origin than the node.
        .layer(CorsLayer::permissive())
        .with_state(handle)
}

/// the fallback submitter identity when a client sends no `origin`.
pub const DEFAULT_ORIGIN: &str = "noded";

async fn submit(State(handle): State<NodeHandle>, Json(req): Json<SubmitRequest>) -> Response {
    let payload = serde_json::to_vec(&req.payload).expect("a decoded json value re-serializes");
    // empty string falls back too — chat rejects empty external authors
    let origin = req
        .origin
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| DEFAULT_ORIGIN.to_string())
        .into_bytes();
    let (reply, rx) = oneshot::channel();
    if let Err(resp) = handle
        .send(NodeCommand::Submit {
            target: req.target,
            payload: payload.clone(),
            origin,
            reply,
        })
        .await
    {
        return resp;
    }
    match rx.await {
        Ok(Ok(block)) => {
            // stage the op's bytes only AFTER the commit so a rejected op leaves
            // nothing behind. put_chunk keys by sha256, so the digest IS the
            // op's content address (fetchable via /v1/files/blob/{op_hash}).
            let op_hash = hex_bytes(&handle.blobs.put_chunk(payload));
            Json(SubmitReceipt {
                height: block.height,
                app_hash: block.app_hash,
                op_hash,
            })
            .into_response()
        }
        Ok(Err(err)) => error_response(StatusCode::BAD_REQUEST, &err),
        Err(_) => actor_gone(),
    }
}

async fn query(State(handle): State<NodeHandle>, Json(req): Json<QueryRequest>) -> Response {
    let req_bytes = serde_json::to_vec(&req.query).expect("a decoded json value re-serializes");
    let (reply, rx) = oneshot::channel();
    if let Err(resp) = handle
        .send(NodeCommand::Query {
            target: req.target,
            req: req_bytes,
            reply,
        })
        .await
    {
        return resp;
    }
    match rx.await {
        Ok(Ok(bytes)) => match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(value) => Json(value).into_response(),
            Err(_) => error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "module reply was not json",
            ),
        },
        Ok(Err(err)) => error_response(StatusCode::BAD_REQUEST, &err),
        Err(_) => actor_gone(),
    }
}

async fn status(State(handle): State<NodeHandle>) -> Response {
    let (reply, rx) = oneshot::channel();
    if let Err(resp) = handle.send(NodeCommand::Status { reply }).await {
        return resp;
    }
    match rx.await {
        Ok(status) => Json(status).into_response(),
        Err(_) => actor_gone(),
    }
}

/// query params for `GET /v1/telemetry`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryParams {
    /// cap the response to the most recent N frames (default: all buffered).
    pub limit: Option<usize>,
}

/// GET /v1/telemetry — recent per-block telemetry, oldest-first: `{"frames":[…]}`.
///
/// reads the node-local ring directly (no actor round-trip — the actor never
/// blocks on an http read). a client connecting mid-stream backfills here, then
/// follows `WsFrame::Telemetry` on `/v1/ws` for live frames.
async fn telemetry(
    State(handle): State<NodeHandle>,
    Query(params): Query<TelemetryParams>,
) -> Response {
    let frames = handle.telemetry.recent(params.limit);
    Json(serde_json::json!({ "frames": frames })).into_response()
}

// ---------------------------------------------------------------------------
// the derived-index tier: shared construction + from-state rebuild triggers.
// noded and the consensus validator (bin/node) both reuse this router, so
// they share the store setup too — one construction site, identical
// /v1/index/* behavior, each binary passing its own genesis module list.
// ---------------------------------------------------------------------------

/// open the per-module index store under `<storage>/index` with every view
/// mapper registered. an open failure is fatal-with-remedy for the caller:
/// the tier is rebuildable, so the fix is always "delete the directory".
pub fn open_index_store<S: AsRef<str>>(
    storage: &std::path::Path,
    module_ids: &[S],
) -> Result<Arc<indexer::IndexStore>, String> {
    let index_dir = storage.join("index");
    indexer::IndexStore::open(&index_dir, module_ids)
        .map(|store| {
            Arc::new(
                store
                    .with_indexer(Box::new(chat_index::ChatIndex::new("chat")))
                    .with_indexer(Box::new(tasks_index::TasksIndex::new("tasks")))
                    .with_indexer(Box::new(document_index::DocumentIndex::new("document")))
                    .with_indexer(Box::new(pages_index::PagesIndex::new("pages"))),
            )
        })
        .map_err(|err| {
            format!(
                "open module index at {}: {err} (derived tier — delete the directory to rebuild)",
                index_dir.display()
            )
        })
}

/// flatten a dispatch origin into the index's plain origin tag: external
/// submitter identities render lossily as utf-8, exactly what the mappers'
/// author rendering assumes — on BOTH lanes. the validator's key-byte
/// identities render the same way, because a mapper's from-state rebuild
/// re-renders authors from canonical state with this exact convention
/// (see chat-index `author_from_ref`): folded and rebuilt rows must match
/// byte-for-byte. hex-keyed identity belongs to the explorer row's
/// `proposer`, not the index op rows.
pub fn index_origin(origin: &sdk::Origin) -> indexer::OriginTag {
    match origin {
        sdk::Origin::External(id) => indexer::OriginTag::external(String::from_utf8_lossy(id)),
        sdk::Origin::Module(id) => indexer::OriginTag::module(id.clone()),
        sdk::Origin::System => indexer::OriginTag::system(),
    }
}

/// one sealed block's dispatch trace as the indexer's fold input. `time` is
/// the block's consensus time — noded passes its submit context's clock, the
/// consensus validator stamps `consensus_time = height`. an empty trace is a
/// real block (a rejected frame consumed its height): folding it advances
/// every module's watermark so staleness checks stay exact.
///
/// `record` starts [`None`]: a caller holding a block the explorer shows
/// grafts its [`block_row`] on via struct update. the live drain builds it
/// from the decoded frame; the validator's boot folds (journal replay, frame
/// catch-up) rebuild the SAME row from the sealed frame bytes riding the
/// replay observer — the row is not reproducible from the dispatch trace
/// alone, so a fold without frame content leaves it `None`.
pub fn index_block_ops(
    height: u64,
    time: u64,
    dispatches: &[host::DispatchRecord],
) -> indexer::BlockOps {
    indexer::BlockOps {
        height,
        time,
        ops: dispatches
            .iter()
            .map(|d| indexer::AppliedOp {
                origin: index_origin(&d.origin),
                module: d.module.clone(),
                payload: d.payload.clone(),
            })
            .collect(),
        record: None,
    }
}

/// one module's canonical state as the indexer's [`indexer::StateReader`]:
/// [`host::Host::query`] adapted onto the bytes-in/bytes-out rebuild surface,
/// module errors mapped into [`indexer::Error::State`].
pub struct HostStateReader<'a> {
    pub host: &'a host::Host,
    pub module: &'a str,
}

#[async_trait::async_trait(?Send)]
impl indexer::StateReader for HostStateReader<'_> {
    async fn query(&self, req: &[u8]) -> indexer::Result<Vec<u8>> {
        self.host
            .query(self.module, req)
            .await
            .map_err(|err| indexer::Error::State(err.to_string()))
    }
}

/// heal every module whose watermark trails `boundary` from VERIFIED
/// canonical state: a mapper that declares a from-state rebuild re-derives
/// its read model; a module without one is stamped backfilled, its content
/// visibly beginning at the boundary. call wherever canonical state advanced
/// without the op stream — after state-sync installs a boundary, after
/// recovery skipped re-executing durable blocks, or over a wiped index
/// directory. returns `(module, rows)` per re-derived view.
pub async fn rebuild_stale_modules(
    index: &indexer::IndexStore,
    host: &host::Host,
    boundary: indexer::RebuildMeta,
) -> Result<Vec<(String, u64)>, indexer::Error> {
    let modules: Vec<String> = index.module_ids().map(str::to_string).collect();
    let mut rebuilt = Vec::new();
    for module in modules {
        if index.applied_height(&module)? >= boundary.height {
            continue;
        }
        let state = HostStateReader {
            host,
            module: &module,
        };
        match index.rebuild_module(&module, &state, boundary).await {
            Ok(rows) => rebuilt.push((module, rows)),
            Err(indexer::Error::RebuildUnsupported) => index.mark_backfilled(&module, boundary)?,
            Err(err) => return Err(err),
        }
    }
    Ok(rebuilt)
}

// ---------------------------------------------------------------------------
// the derived-index read lane. like the blob and telemetry lanes these
// handlers never cross the actor: the store is `Send + Sync` and every read
// runs at its own MVCC snapshot, concurrent with the actor's block writes.
// ---------------------------------------------------------------------------

/// query params for `GET /v1/index/{module}/scan` and `…/ops`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexScanParams {
    /// key prefix to scan under. ignored by `…/ops` (pinned to the op log).
    pub prefix: Option<String>,
    /// opaque page cursor: the `nextAfter` of the previous page.
    pub after: Option<String>,
    /// page size; the store clamps oversized asks.
    pub limit: Option<usize>,
}

/// default page size when a client sends no `limit`.
const INDEX_DEFAULT_LIMIT: usize = 100;

/// one scanned entry. values written by this tier are json (`value`); a
/// derived value that is not valid json ships as `valueHex` instead.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IndexEntry {
    key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<Box<serde_json::value::RawValue>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    value_hex: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IndexScanResponse {
    entries: Vec<IndexEntry>,
    has_more: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_after: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IndexOpsResponse {
    /// stored op-row envelopes, verbatim (height/seq/time/origin + payload).
    ops: Vec<Box<serde_json::value::RawValue>>,
    has_more: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_after: Option<String>,
}

fn index_store(handle: &NodeHandle) -> Result<&Arc<indexer::IndexStore>, Response> {
    handle
        .index
        .as_ref()
        .ok_or_else(|| error_response(StatusCode::SERVICE_UNAVAILABLE, "no index store configured"))
}

fn index_error(err: indexer::Error) -> Response {
    let status = match err {
        indexer::Error::UnknownModule(_) | indexer::Error::ViewUnsupported => StatusCode::NOT_FOUND,
        indexer::Error::View(_) => StatusCode::BAD_REQUEST,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    error_response(status, &err.to_string())
}

/// GET /v1/index/status — each module's applied watermark plus the poison
/// flag. a poisoned index keeps serving (stale but consistent) reads; the
/// remedy is a rebuild, which this surface makes visible. modules re-derived
/// from canonical state also report their backfill floor: content below it
/// was never folded from ops (heights are boundary-stamped, the op log
/// starts above it) — the gap stays visible instead of papered over.
async fn index_status(State(handle): State<NodeHandle>) -> Response {
    let store = match index_store(&handle) {
        Ok(store) => store,
        Err(resp) => return resp,
    };
    let mut modules = serde_json::Map::new();
    let mut backfilled = serde_json::Map::new();
    for id in store.module_ids() {
        match store.applied_height(id) {
            Ok(height) => {
                modules.insert(id.to_string(), height.into());
            }
            Err(err) => return index_error(err),
        }
        match store.backfill_height(id) {
            Ok(Some(floor)) => {
                backfilled.insert(id.to_string(), floor.into());
            }
            Ok(None) => {}
            Err(err) => return index_error(err),
        }
    }
    Json(serde_json::json!({
        "poisoned": store.is_poisoned(),
        "modules": modules,
        "backfilled": backfilled,
    }))
    .into_response()
}

/// GET /v1/index/{module}/ops?after=&limit= — one page of the module's op
/// log, oldest-first. rows are the stored envelopes verbatim; page forward by
/// echoing `nextAfter` as the next call's `after`.
async fn index_ops(
    State(handle): State<NodeHandle>,
    Path(module): Path<String>,
    Query(params): Query<IndexScanParams>,
) -> Response {
    let store = match index_store(&handle) {
        Ok(store) => store,
        Err(resp) => return resp,
    };
    let page = match store.scan(
        &module,
        indexer::OP_PREFIX.as_bytes(),
        params.after.as_deref().map(str::as_bytes),
        params.limit.unwrap_or(INDEX_DEFAULT_LIMIT),
    ) {
        Ok(page) => page,
        Err(err) => return index_error(err),
    };
    let mut ops = Vec::with_capacity(page.entries.len());
    for (_key, value) in &page.entries {
        match serde_json::from_slice(value) {
            Ok(row) => ops.push(row),
            // rows are written as json by construction; failing one means the
            // store is damaged — say so instead of silently dropping it.
            Err(_) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "stored op row was not json — rebuild the index",
                );
            }
        }
    }
    Json(IndexOpsResponse {
        ops,
        has_more: page.has_more,
        next_after: page.next_after,
    })
    .into_response()
}

/// POST /v1/index/{module}/view — the module's materialized view, served by
/// its registered mapper. request body and reply are module-defined json
/// (chat: `{"search": {…}}` → `{"hits": […]}`), exactly as opaque to the
/// daemon as `/v1/query` payloads are. modules with no view answer 404 —
/// some never will (forge's substrate is already a queryable git repo).
async fn index_view(
    State(handle): State<NodeHandle>,
    Path(module): Path<String>,
    Json(req): Json<serde_json::Value>,
) -> Response {
    let store = match index_store(&handle) {
        Ok(store) => store,
        Err(resp) => return resp,
    };
    let req_bytes = serde_json::to_vec(&req).expect("a decoded json value re-serializes");
    match store.view(&module, &req_bytes) {
        Ok(bytes) => match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(value) => Json(value).into_response(),
            Err(_) => error_response(StatusCode::INTERNAL_SERVER_ERROR, "view reply was not json"),
        },
        Err(err) => index_error(err),
    }
}

/// GET /v1/index/{module}/scan?prefix=&after=&limit= — one page of raw index
/// keys, for a module's derived read models (everything a registered
/// `ModuleIndexer` materialized outside the reserved op/meta spaces).
async fn index_scan(
    State(handle): State<NodeHandle>,
    Path(module): Path<String>,
    Query(params): Query<IndexScanParams>,
) -> Response {
    let store = match index_store(&handle) {
        Ok(store) => store,
        Err(resp) => return resp,
    };
    let prefix = params.prefix.unwrap_or_default();
    let page = match store.scan(
        &module,
        prefix.as_bytes(),
        params.after.as_deref().map(str::as_bytes),
        params.limit.unwrap_or(INDEX_DEFAULT_LIMIT),
    ) {
        Ok(page) => page,
        Err(err) => return index_error(err),
    };
    let entries = page
        .entries
        .iter()
        .map(|(key, value)| {
            let json: Option<Box<serde_json::value::RawValue>> = serde_json::from_slice(value).ok();
            IndexEntry {
                key: String::from_utf8_lossy(key).into_owned(),
                value_hex: json.is_none().then(|| hex_bytes(value)),
                value: json,
            }
        })
        .collect();
    Json(IndexScanResponse {
        entries,
        has_more: page.has_more,
        next_after: page.next_after,
    })
    .into_response()
}

/// query params for `GET /v1/blocks`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlocksParams {
    /// cap the response to the most recent N blocks (default:
    /// [`BLOCKS_DEFAULT_LIMIT`]).
    pub limit: Option<usize>,
}

/// GET /v1/blocks — recent non-empty blocks, oldest-first: `{"blocks":[…]}`.
///
/// reads the index store's durable blocks database directly (no actor
/// round-trip), the same discipline as the other `/v1/index/*` reads — so
/// history survives a restart. heartbeat nops never get a row, so an empty
/// reply means no real ops have finalized, not an idle chain. a handle with
/// no index store configured serves the same "no blocks yet" shape.
async fn blocks(State(handle): State<NodeHandle>, Query(params): Query<BlocksParams>) -> Response {
    let Some(store) = handle.index.as_ref() else {
        return Json(serde_json::json!({ "blocks": [] })).into_response();
    };
    let rows = match store.recent_block_rows(params.limit.unwrap_or(BLOCKS_DEFAULT_LIMIT)) {
        Ok(rows) => rows,
        Err(err) => return index_error(err),
    };
    let mut blocks: Vec<Box<serde_json::value::RawValue>> = Vec::with_capacity(rows.len());
    for row in &rows {
        match serde_json::from_slice(row) {
            Ok(block) => blocks.push(block),
            // rows are written as json by construction; failing one means the
            // store is damaged — say so instead of silently dropping it.
            Err(_) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "stored block row was not json — rebuild the index",
                );
            }
        }
    }
    Json(serde_json::json!({ "blocks": blocks })).into_response()
}

/// the OpenMetrics content type a Prometheus scraper negotiates for `/metrics`.
const OPENMETRICS_CONTENT_TYPE: &str = "application/openmetrics-text; version=1.0.0; charset=utf-8";

/// GET /metrics — the Prometheus scrape surface. the actor encodes the
/// commonware runtime registry (which the daemon's `ducktape_*` series are
/// registered into) to OpenMetrics text and hands it back over the command lane.
async fn metrics(State(handle): State<NodeHandle>) -> Response {
    let (reply, rx) = oneshot::channel();
    if let Err(resp) = handle.send(NodeCommand::Metrics { reply }).await {
        return resp;
    }
    match rx.await {
        Ok(body) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, OPENMETRICS_CONTENT_TYPE)],
            body,
        )
            .into_response(),
        Err(_) => actor_gone(),
    }
}

async fn shutdown(State(handle): State<NodeHandle>) -> Response {
    // reply first, then signal — the connection closes before the process does.
    handle.shutdown.notify_one();
    Json(serde_json::json!({ "ok": true })).into_response()
}

/// POST /v1/files/blob — raw chunk bytes in, `{"digest":"<64-hex>"}` out.
///
/// bytes go straight into the node-local blob store; NOTHING reaches the node
/// actor and no op is submitted — committing a manifest that references the
/// digest is a separate, explicit `/v1/submit`. the route's body limit is
/// `MAX_CHUNK_SIZE` (a bigger chunk could never be referenced by a valid
/// manifest anyway), and an oversized body is a 413 in the daemon's json
/// error envelope.
async fn put_blob(
    State(handle): State<NodeHandle>,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    let bytes = match body {
        Ok(bytes) => bytes,
        // the DefaultBodyLimit layer stops reading past the cap and the
        // extractor rejects with 413 — re-wrap it in the json envelope.
        Err(rejection) => return error_response(rejection.status(), &rejection.body_text()),
    };
    let digest = handle.blobs.put_chunk(bytes.to_vec());
    Json(serde_json::json!({ "digest": hex_bytes(&digest) })).into_response()
}

/// GET /v1/files/blob/{digest} — chunk bytes back out of the node-local store.
/// a malformed digest (anything but 64 lowercase hex chars) is a 400; a
/// well-formed digest this node holds no bytes for is a 404.
async fn get_blob(State(handle): State<NodeHandle>, Path(digest): Path<String>) -> Response {
    let Some(raw) = files::from_hex_32(&digest) else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "digest must be 64 characters of lowercase hex",
        );
    };
    match handle.blobs.get_chunk(&raw) {
        Some(bytes) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/octet-stream")],
            bytes,
        )
            .into_response(),
        None => error_response(StatusCode::NOT_FOUND, "no chunk with that digest"),
    }
}

// ============================================================================
// git smart-HTTP: forge is a full push+fetch remote.
//
// this is the ONE forge-specific corner of the surface (every other route is
// module-agnostic opaque json). it speaks the git smart-HTTP protocol on both
// sides so a stock `git` clones, pulls, and pushes `http://<node>/forge/<repo>`:
//   GET  /forge/{repo}/info/refs?service=git-receive-pack — advertise for push
//   GET  /forge/{repo}/info/refs?service=git-upload-pack  — advertise for fetch
//   POST /forge/{repo}/git-receive-pack                   — receive a push
//   POST /forge/{repo}/git-upload-pack                    — serve a fetch/clone
//
// PUSH bridges to forge's consensus `Push` op: the packfile bytes land in the
// node-local blob store (never consensus); only the (prev_oid, new_oid,
// pack_digest) CAS crosses into a block, and forge's in-module `materialize`
// verifies the pack against the repo's objects.
//
// FETCH reads forge's git substrate DIRECTLY — the one route that opens the
// on-disk repo (`<forge_repo>/<name>`, threaded onto the handle) instead of
// talking to the actor. it builds a packfile of the wanted oids' full closure
// and streams it back on side-band-64k. the MVP ignores `have`s and always
// serves a full closure — always correct, just larger than an incremental
// fetch; `git pull` works, it just refetches.
// ============================================================================

/// the capabilities forge's receive-pack advertises. deliberately NO
/// `side-band-64k`, so the client sends the report-status back as plain
/// pkt-lines (not muxed onto a side channel) — the minimal wire this bridge
/// needs to read.
const GIT_RECEIVE_PACK_CAPS: &str =
    "report-status report-status-v2 delete-refs ofs-delta agent=ducktape-forge/0.1";
/// the capabilities forge's upload-pack (fetch/clone) advertises. `side-band-64k`
/// muxes the packfile onto band 1 of the reply — git clients request it by
/// default; `multi_ack_detailed` is the modern negotiation, `thin-pack`/
/// `ofs-delta` are standard pack encodings. no fetch-side extras (shallow /
/// filter): the MVP serves a full closure.
const GIT_UPLOAD_PACK_CAPS: &str =
    "multi_ack_detailed side-band-64k thin-pack ofs-delta agent=ducktape-forge/0.1";
/// the body cap for a git packfile POST — push (whole-repo pack) and fetch
/// (want/have negotiation). lifted far above the json/chunk defaults.
const GIT_PACK_BODY_LIMIT: usize = 512 * 1024 * 1024;
/// max PACK bytes per side-band-64k data pkt-line: prefixed with the 1-byte band
/// id, plus the 4-byte pkt length header, this yields a 65520-byte line — git's
/// `LARGE_PACKET_MAX`, the ceiling a side-band-64k client accepts.
const GIT_SIDE_BAND_CHUNK: usize = 65515;
/// the only ref this MVP applies a push to; multi-branch is future work. both
/// `git push <remote> main` and `git push <remote> HEAD:main` send this ref.
const GIT_MAIN_REF: &str = "refs/heads/main";
/// 40 ascii zeros: git's "null" oid — the old value of a ref being created, and
/// the head advertised for an unborn repo.
const GIT_ZERO_OID: &str = "0000000000000000000000000000000000000000";
/// raw sha1 oid length in bytes. git's wire oids are 40 hex chars == 20 bytes;
/// forge's `Push` op wants exactly these raw bytes (it re-length-checks too).
const GIT_OID_RAW_LEN: usize = 20;
/// the flush-pkt: a zero-length pkt that ends a pkt-line stream or section.
const GIT_FLUSH_PKT: &[u8] = b"0000";

/// encode one git pkt-line: a 4-hex length (INCLUDING the 4 length bytes)
/// followed by the payload. every line this bridge emits is tiny, well under
/// the 65516-byte payload cap, so no splitting is needed.
fn pkt_line(payload: &[u8]) -> Vec<u8> {
    let len = payload.len() + 4;
    let mut out = format!("{len:04x}").into_bytes();
    out.extend_from_slice(payload);
    out
}

/// split a leading pkt-line section off `buf`: parse length-framed lines until a
/// flush-pkt (`0000`), returning each payload (WITHOUT its 4-byte length header)
/// and the bytes AFTER the flush (for receive-pack, the raw packfile). a
/// truncated or malformed length is a clean error, never a panic — a corrupt
/// body becomes a 400.
fn parse_pkt_lines(buf: &[u8]) -> Result<(Vec<Vec<u8>>, &[u8]), String> {
    let mut lines = Vec::new();
    let mut rest = buf;
    loop {
        if rest.len() < 4 {
            return Err("truncated pkt-line length header".into());
        }
        let hdr =
            std::str::from_utf8(&rest[..4]).map_err(|_| "non-ascii pkt-line length".to_string())?;
        let len = usize::from_str_radix(hdr, 16)
            .map_err(|_| "invalid pkt-line length hex".to_string())?;
        if len == 0 {
            // flush-pkt terminates the command section; the rest is the pack.
            return Ok((lines, &rest[4..]));
        }
        if len < 4 || len > rest.len() {
            return Err("pkt-line length out of range".into());
        }
        lines.push(rest[4..len].to_vec());
        rest = &rest[len..];
    }
}

/// decode an even-length hex string to raw bytes; `None` on an odd length or any
/// non-hex nibble. turns a git pkt-line oid (40 hex) into raw sha1 bytes.
fn hex_to_bytes(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok())
        .collect()
}

/// validate a `{repo}` path segment the SAME way forge's `norm_repo` does (empty
/// -> the default repo; otherwise 1..=64 bytes of `[a-z0-9._-]` and never
/// `.`/`..`). returns the normalized slug, or `None` for an invalid name (a 404
/// at the route). keeping this in lockstep means an accepted URL always names a
/// repo the module will also accept.
fn norm_repo(repo: &str) -> Option<String> {
    if repo.is_empty() {
        return Some("default".to_string());
    }
    if repo.len() > 64 || repo == "." || repo == ".." {
        return None;
    }
    repo.bytes()
        .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'-'))
        .then(|| repo.to_string())
}

/// query the forge module for a repo's committed HEAD oid hex (`None` == unborn).
/// errors surface as an http `Response` so callers can early-return them.
async fn forge_head(handle: &NodeHandle, repo: &str) -> Result<Option<String>, Response> {
    let req = forge_interface::encode_query(&forge_interface::ForgeQuery::HeadOf {
        repo: repo.to_string(),
    });
    let (reply, rx) = oneshot::channel();
    handle
        .send(NodeCommand::Query {
            target: "forge".into(),
            req,
            reply,
        })
        .await?;
    let bytes = rx
        .await
        .map_err(|_| actor_gone())?
        .map_err(|err| error_response(StatusCode::INTERNAL_SERVER_ERROR, &err))?;
    match forge_interface::decode_reply(&bytes) {
        Ok(forge_interface::ForgeReply::Head(head)) => Ok(head),
        Ok(_) => Err(error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "unexpected forge reply to HeadOf",
        )),
        Err(err) => Err(error_response(StatusCode::INTERNAL_SERVER_ERROR, &err)),
    }
}

/// build a receive-pack `report-status` body: `unpack ok`, one ref status line,
/// then a flush. `err` is `None` for success (`ok <ref>`) or `Some(reason)` for
/// a rejection (`ng <ref> <reason>`). the pack is always received by the time we
/// answer, so `unpack ok` is unconditional (we don't verify closure here).
fn git_report_status(refname: &str, err: Option<&str>) -> Response {
    let mut body = Vec::new();
    body.extend_from_slice(&pkt_line(b"unpack ok\n"));
    let status_line = match err {
        None => format!("ok {refname}\n"),
        Some(reason) => format!("ng {refname} {reason}\n"),
    };
    body.extend_from_slice(&pkt_line(status_line.as_bytes()));
    body.extend_from_slice(GIT_FLUSH_PKT);
    (
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                "application/x-git-receive-pack-result",
            ),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        body,
    )
        .into_response()
}

/// query params for the ref advertisement; git always sends `service=`.
#[derive(Debug, Deserialize)]
pub struct InfoRefsParams {
    pub service: Option<String>,
}

/// which smart-HTTP service an info/refs advertisement is for — push
/// (receive-pack) or fetch (upload-pack). the two differ only in the banner,
/// the capability set, the content-type, and whether a `HEAD` line rides along.
#[derive(Clone, Copy)]
enum GitService {
    Receive,
    Upload,
}

impl GitService {
    fn name(self) -> &'static str {
        match self {
            Self::Receive => "git-receive-pack",
            Self::Upload => "git-upload-pack",
        }
    }
    fn caps(self) -> &'static str {
        match self {
            Self::Receive => GIT_RECEIVE_PACK_CAPS,
            Self::Upload => GIT_UPLOAD_PACK_CAPS,
        }
    }
    fn advertisement_content_type(self) -> &'static str {
        match self {
            Self::Receive => "application/x-git-receive-pack-advertisement",
            Self::Upload => "application/x-git-upload-pack-advertisement",
        }
    }
}

/// GET /forge/{repo}/info/refs?service=… — the smart-HTTP ref advertisement a
/// `git push`/`git clone` fetches FIRST to learn the remote's current head. the
/// advertised head is forge's COMMITTED head (from the actor lane, so it matches
/// consensus); the fetch POST then serves the matching objects off disk. both
/// receive-pack (push) and upload-pack (fetch) are served — the v0 banner we
/// send makes git speak the classic protocol for the follow-up POST even when it
/// probed with `Git-Protocol: version=2`.
async fn git_info_refs(
    State(handle): State<NodeHandle>,
    Path(repo): Path<String>,
    Query(params): Query<InfoRefsParams>,
) -> Response {
    let Some(repo) = norm_repo(&repo) else {
        return error_response(StatusCode::NOT_FOUND, "no such repo");
    };
    let service = match params.service.as_deref() {
        Some("git-receive-pack") => GitService::Receive,
        Some("git-upload-pack") => GitService::Upload,
        _ => {
            return error_response(
                StatusCode::FORBIDDEN,
                "only git-receive-pack and git-upload-pack are served",
            );
        }
    };
    git_advertise_refs(&handle, &repo, service).await
}

/// build the smart-HTTP ref advertisement for `service`: the service banner, a
/// flush, the ref line(s), then a flush. an unborn repo advertises the null oid
/// against the magic `capabilities^{}` ref (so caps ride along with no real ref)
/// — a clone then reports an empty repository. a born repo advertises its head
/// against refs/heads/main with caps after a NUL; a fetch advertisement ALSO
/// emits a `HEAD` line at the same oid so `git clone` resolves the branch to
/// check out (git matches HEAD's oid to refs/heads/main).
async fn git_advertise_refs(handle: &NodeHandle, repo: &str, service: GitService) -> Response {
    let head = match forge_head(handle, repo).await {
        Ok(head) => head,
        Err(resp) => return resp,
    };
    let caps = service.caps();

    let mut body = Vec::new();
    body.extend_from_slice(&pkt_line(
        format!("# service={}\n", service.name()).as_bytes(),
    ));
    body.extend_from_slice(GIT_FLUSH_PKT);
    match head {
        Some(oid) => {
            body.extend_from_slice(&pkt_line(
                format!("{oid} {GIT_MAIN_REF}\0{caps}\n").as_bytes(),
            ));
            if matches!(service, GitService::Upload) {
                body.extend_from_slice(&pkt_line(format!("{oid} HEAD\n").as_bytes()));
            }
        }
        None => {
            body.extend_from_slice(&pkt_line(
                format!("{GIT_ZERO_OID} capabilities^{{}}\0{caps}\n").as_bytes(),
            ));
        }
    }
    body.extend_from_slice(GIT_FLUSH_PKT);

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, service.advertisement_content_type()),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        body,
    )
        .into_response()
}

/// return the request body, gzip-inflated if `Content-Encoding: gzip`. git may
/// compress a receive-pack request; any other encoding is passed through.
fn decode_git_body(headers: &HeaderMap, body: &[u8]) -> Result<Vec<u8>, String> {
    let gzip = headers
        .get(header::CONTENT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("gzip"));
    if !gzip {
        return Ok(body.to_vec());
    }
    use std::io::Read as _;
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(body)
        .read_to_end(&mut out)
        .map_err(|e| format!("gzip inflate failed: {e}"))?;
    Ok(out)
}

/// POST /forge/{repo}/git-receive-pack — receive a push: parse the ref-update
/// command list + packfile, stash the whole pack in the node-local blob store,
/// and CAS the repo head through forge's `Push` op (one submit == one block).
/// the response is a git `report-status` reflecting whether the CAS committed.
async fn git_receive_pack(
    State(handle): State<NodeHandle>,
    Path(repo): Path<String>,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    let Some(repo) = norm_repo(&repo) else {
        return error_response(StatusCode::NOT_FOUND, "no such repo");
    };
    let body = match body {
        Ok(bytes) => bytes,
        // the DefaultBodyLimit layer rejects an oversized pack with 413.
        Err(rejection) => return error_response(rejection.status(), &rejection.body_text()),
    };
    let body = match decode_git_body(&headers, &body) {
        Ok(bytes) => bytes,
        Err(msg) => return error_response(StatusCode::BAD_REQUEST, &msg),
    };

    // the body is a pkt-line command list, a flush-pkt, then the raw packfile.
    let (commands, pack) = match parse_pkt_lines(&body) {
        Ok(parsed) => parsed,
        Err(msg) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                &format!("malformed git command stream: {msg}"),
            );
        }
    };
    let Some(first) = commands.first() else {
        // a push whose pack exceeds git's `http.postBuffer` (1 MiB default) is
        // preceded by a flush-only PROBE POST (Content-Length: 4, body `0000`,
        // zero commands) before git streams the real chunked request. an empty
        // command list is a valid no-op: answer 200 with an empty result so the
        // probe succeeds and git proceeds with the actual push. 400 here aborts
        // every push larger than the post buffer.
        return (
            StatusCode::OK,
            [
                (
                    header::CONTENT_TYPE,
                    "application/x-git-receive-pack-result",
                ),
                (header::CACHE_CONTROL, "no-cache"),
            ],
            GIT_FLUSH_PKT.to_vec(),
        )
            .into_response();
    };

    // the first command carries the ref update, with capabilities after a NUL.
    // strip the caps (from the first NUL on) and any trailing newline.
    let nul = first.iter().position(|&b| b == 0).unwrap_or(first.len());
    let line = std::str::from_utf8(&first[..nul])
        .map(str::trim_end)
        .unwrap_or("");
    let mut parts = line.split(' ');
    let (Some(old), Some(new), Some(refname)) = (parts.next(), parts.next(), parts.next()) else {
        return error_response(StatusCode::BAD_REQUEST, "malformed ref-update command");
    };

    if refname != GIT_MAIN_REF {
        // consume-and-refuse: the pack was fully received; we just don't apply
        // it. reporting `ng` (not an http error) lets git print a clean reason.
        return git_report_status(refname, Some(&format!("only {GIT_MAIN_REF} is supported")));
    }

    // old == the null oid means "create" (unborn -> prev_oid None); otherwise it
    // is the 40-hex prev the forge CAS must match. new is always a real oid.
    let prev_oid = if old == GIT_ZERO_OID {
        None
    } else {
        match hex_to_bytes(old).filter(|b| b.len() == GIT_OID_RAW_LEN) {
            Some(bytes) => Some(bytes),
            None => return error_response(StatusCode::BAD_REQUEST, "malformed old oid"),
        }
    };
    let Some(new_oid) = hex_to_bytes(new).filter(|b| b.len() == GIT_OID_RAW_LEN) else {
        return error_response(StatusCode::BAD_REQUEST, "malformed new oid");
    };

    // stash the WHOLE packfile as one node-local blob, keyed by its sha256; forge
    // materializes it by this digest. the bytes never cross consensus.
    let pack_digest = handle.blobs.put_chunk(pack.to_vec());

    // CAS the head through a forge Push op and await the block result.
    let payload = forge_interface::encode_msg(&forge_interface::ForgeMsg::Push {
        repo,
        prev_oid,
        new_oid,
        pack_digest: pack_digest.to_vec(),
    });
    let (reply, rx) = oneshot::channel();
    if let Err(resp) = handle
        .send(NodeCommand::Submit {
            target: "forge".into(),
            payload,
            origin: DEFAULT_ORIGIN.as_bytes().to_vec(),
            reply,
        })
        .await
    {
        return resp;
    }
    match rx.await {
        Ok(Ok(_block)) => git_report_status(GIT_MAIN_REF, None),
        Ok(Err(reason)) => {
            // a CAS mismatch's rejection carries "non-fast-forward" — surface
            // exactly that token so git prints its standard "fetch first" hint.
            // any other rejection passes through as a single-line reason.
            let reason = if reason.contains("non-fast-forward") {
                "non-fast-forward".to_string()
            } else {
                reason.replace('\n', " ")
            };
            git_report_status(GIT_MAIN_REF, Some(&reason))
        }
        Err(_) => actor_gone(),
    }
}

/// POST /forge/{repo}/git-upload-pack — serve a fetch/clone. parse the pkt-line
/// negotiation (`want <oid>` lines, capabilities on the FIRST; `have`/`done`
/// lines are read but IGNORED — the MVP serves a full closure), open
/// `<forge_repo>/{repo}` READ-ONLY, build a packfile of the wanted oids' closure,
/// and reply `NAK` then the pack muxed on side-band-64k band 1. incremental
/// (`have`-aware) fetch is future work: a full pack is always correct, just
/// larger, and `git pull` still works (it refetches).
async fn git_upload_pack(
    State(handle): State<NodeHandle>,
    Path(repo): Path<String>,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    let Some(repo) = norm_repo(&repo) else {
        return error_response(StatusCode::NOT_FOUND, "no such repo");
    };
    let Some(forge_repo) = handle.forge_repo.clone() else {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "forge repo path not configured on this node",
        );
    };
    let body = match body {
        Ok(bytes) => bytes,
        // the DefaultBodyLimit layer rejects an oversized request with 413.
        Err(rejection) => return error_response(rejection.status(), &rejection.body_text()),
    };
    let body = match decode_git_body(&headers, &body) {
        Ok(bytes) => bytes,
        Err(msg) => return error_response(StatusCode::BAD_REQUEST, &msg),
    };

    // the request opens with the `want` list (caps on the first want), then a
    // flush-pkt, then have/done lines. parse_pkt_lines returns exactly the want
    // section; the have/done tail after the flush is deliberately ignored (the
    // full-closure MVP negotiates no common base).
    let (lines, _rest) = match parse_pkt_lines(&body) {
        Ok(parsed) => parsed,
        Err(msg) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                &format!("malformed git-upload-pack request: {msg}"),
            );
        }
    };

    let mut wants: Vec<String> = Vec::new();
    let mut side_band = false;
    let mut first_want = true;
    for line in &lines {
        let text = std::str::from_utf8(line).map(str::trim_end).unwrap_or("");
        let Some(rest) = text.strip_prefix("want ") else {
            continue;
        };
        // `want <oid>[ <cap> <cap> …]` — the oid then space-separated caps on the
        // first want line only.
        let mut toks = rest.split(' ');
        let Some(oid) = toks.next().filter(|s| !s.is_empty()) else {
            continue;
        };
        wants.push(oid.to_string());
        if first_want {
            side_band = toks.any(|c| c == "side-band-64k");
            first_want = false;
        }
    }
    if wants.is_empty() {
        return error_response(
            StatusCode::BAD_REQUEST,
            "git-upload-pack request carried no want lines",
        );
    }

    // the pack build is blocking git2 IO over a non-Send `Repository`; run it off
    // the async worker, moving only Send data (the dir + hex oids) across.
    let repo_dir = forge_repo.join(&repo);
    let pack = match tokio::task::spawn_blocking(move || build_upload_pack(&repo_dir, &wants)).await
    {
        Ok(Ok(pack)) => pack,
        Ok(Err(msg)) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &msg),
        Err(_) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "git pack builder task panicked",
            );
        }
    };

    let mut out = Vec::new();
    // no common base was negotiated (haves ignored), so a single NAK precedes the
    // pack — sent as a PLAIN pkt-line, BEFORE any side-band framing begins.
    out.extend_from_slice(&pkt_line(b"NAK\n"));
    if side_band {
        // band 1 = pack data, chunked to the side-band-64k ceiling.
        for chunk in pack.chunks(GIT_SIDE_BAND_CHUNK) {
            let mut framed = Vec::with_capacity(chunk.len() + 1);
            framed.push(0x01);
            framed.extend_from_slice(chunk);
            out.extend_from_slice(&pkt_line(&framed));
        }
        out.extend_from_slice(GIT_FLUSH_PKT);
    } else {
        // the client didn't request side-band: the raw pack follows NAK directly
        // (no band framing, no trailing flush — the pack trailer ends the stream).
        out.extend_from_slice(&pack);
    }

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/x-git-upload-pack-result"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        out,
    )
        .into_response()
}

/// build a self-contained packfile of the FULL object closure of `want_hexes`
/// from the repo at `repo_dir`, opened READ-ONLY. mirrors forge's
/// `git::pack_closure`: a revwalk seeded with every wanted oid, each reachable
/// commit inserted (the `PackBuilder` pulls its tree/blobs and dedups objects
/// shared between commits). single-threaded so the bytes are a pure function of
/// the closure. any git2 failure — a missing repo dir, an oid absent from the
/// odb, a pack-write error — is returned as a message the handler surfaces.
fn build_upload_pack(repo_dir: &std::path::Path, want_hexes: &[String]) -> Result<Vec<u8>, String> {
    let repo = git2::Repository::open(repo_dir).map_err(|e| format!("open forge repo: {e}"))?;
    let mut pb = repo
        .packbuilder()
        .map_err(|e| format!("packbuilder: {e}"))?;
    pb.set_threads(1);
    let mut walk = repo.revwalk().map_err(|e| format!("revwalk: {e}"))?;
    for hex in want_hexes {
        let oid = git2::Oid::from_str(hex).map_err(|e| format!("bad want oid {hex}: {e}"))?;
        walk.push(oid)
            .map_err(|e| format!("wanted oid {hex} not present: {e}"))?;
    }
    for oid in walk {
        let oid = oid.map_err(|e| format!("revwalk step: {e}"))?;
        pb.insert_commit(oid)
            .map_err(|e| format!("insert commit {oid}: {e}"))?;
    }
    let mut buf = git2::Buf::new();
    pb.write_buf(&mut buf)
        .map_err(|e| format!("write pack: {e}"))?;
    Ok(buf.to_vec())
}

/// serve the client surface on `listener` until a shutdown request lands
/// (POST /v1/shutdown). the caller owns the runtime this runs on; the host
/// actor lives elsewhere and is reachable only through `handle`'s command
/// lane — which is what lets ANY binary that owns a host (the embedded daemon,
/// the p2p validator) stand up the identical surface.
pub async fn serve(listener: tokio::net::TcpListener, handle: NodeHandle) -> std::io::Result<()> {
    let shutdown = handle.clone();
    axum::serve(listener, router(handle))
        .with_graceful_shutdown(async move { shutdown.shutdown_requested().await })
        .await
}

async fn ws(State(handle): State<NodeHandle>, upgrade: WebSocketUpgrade) -> Response {
    let frames = handle.events.subscribe();
    upgrade.on_upgrade(move |socket| stream_frames(socket, frames))
}

async fn stream_frames(mut socket: WebSocket, mut frames: broadcast::Receiver<WsFrame>) {
    loop {
        match frames.recv().await {
            Ok(frame) => {
                let text = serde_json::to_string(&frame).expect("ws frame serializes");
                if socket.send(Message::Text(text.into())).await.is_err() {
                    return; // client hung up
                }
            }
            // this subscriber fell behind the buffer; skip ahead — clients
            // re-query on every block anyway, missing one is harmless.
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => return,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(height: u64) -> TelemetryFrame {
        TelemetryFrame {
            height,
            consensus_time: 0,
            latency_us: 0,
            dispatches: Vec::new(),
            events: Vec::new(),
        }
    }

    #[test]
    fn ring_evicts_oldest_at_cap_and_recent_returns_newest_tail() {
        let ring = TelemetryRing::default();
        // fill two past the cap: the oldest two fall out, the newest survive.
        for h in 0..(TELEMETRY_RING_CAP as u64 + 2) {
            ring.push(frame(h));
        }
        let all = ring.recent(None);
        assert_eq!(
            all.len(),
            TELEMETRY_RING_CAP,
            "buffer holds exactly the cap"
        );
        assert_eq!(all.first().unwrap().height, 2, "oldest two evicted");
        assert_eq!(
            all.last().unwrap().height,
            TELEMETRY_RING_CAP as u64 + 1,
            "newest retained, oldest-first ordering"
        );

        // recent(limit) returns the newest `limit`, still oldest-first.
        let tail: Vec<u64> = ring.recent(Some(3)).iter().map(|f| f.height).collect();
        assert_eq!(
            tail,
            vec![
                TELEMETRY_RING_CAP as u64 - 1,
                TELEMETRY_RING_CAP as u64,
                TELEMETRY_RING_CAP as u64 + 1,
            ],
        );

        // a limit past the buffer size just returns everything.
        assert_eq!(
            ring.recent(Some(TELEMETRY_RING_CAP + 100)).len(),
            TELEMETRY_RING_CAP,
        );
    }

    /// the explorer row round-trips through its stored index encoding — the
    /// one seam both binaries write and `GET /v1/blocks` reads.
    #[test]
    fn block_row_round_trips() {
        let record = BlockRecord {
            height: 7,
            hash: String::new(),
            commit_hash: "aa".repeat(32),
            proposer: "bb".repeat(32),
            disposition: BlockDisposition::Applied,
            target: "directory".into(),
            operations: Vec::new(),
            payload: "{}".into(),
            op_hash: "ee".repeat(32),
        };
        let row = block_row(&record);
        let back: BlockRecord = serde_json::from_slice(&row).expect("row is json");
        assert_eq!(back.height, 7);
        assert_eq!(back.hash, "", "frameless lanes keep an honest empty hash");
        assert_eq!(back.proposer, "bb".repeat(32));
        assert_eq!(back.op_hash, "ee".repeat(32));
        // the wire keys stay camelCase — the app reads these fields verbatim.
        let json: serde_json::Value = serde_json::from_slice(&row).unwrap();
        assert!(json.get("commitHash").is_some());
        assert!(json.get("opHash").is_some());
    }

    #[test]
    fn payload_preview_caps_and_marks_truncation() {
        assert_eq!(payload_preview(b"{\"k\":\"v\"}"), "{\"k\":\"v\"}");
        let long = "x".repeat(PAYLOAD_PREVIEW_MAX + 10);
        let preview = payload_preview(long.as_bytes());
        assert_eq!(preview.chars().count(), PAYLOAD_PREVIEW_MAX + 1);
        assert!(preview.ends_with('…'), "truncation is visible");
        // invalid utf-8 renders lossily rather than erroring.
        assert_eq!(payload_preview(&[0xff, 0xfe]), "\u{fffd}\u{fffd}");
    }

    #[test]
    fn module_categories_group_the_genesis_set() {
        use ModuleCategory::*;
        for id in ["chat", "tasks", "inbox", "document", "pages"] {
            assert_eq!(ModuleCategory::of(id), Workspace, "{id}");
        }
        for id in ["forge", "agent"] {
            assert_eq!(ModuleCategory::of(id), Developer, "{id}");
        }
        for id in ["automations", "jobs"] {
            assert_eq!(ModuleCategory::of(id), Automation, "{id}");
        }
        // infra + internals fall to the System bucket — including ids only the
        // full `node` binary registers (kv/valset/governance/vaults/directory)
        // and anything unknown, so the view never breaks on a new module.
        for id in [
            "files",
            "memory",
            "saga",
            "profiles",
            "kv",
            "valset",
            "governance",
            "vaults",
            "directory",
            "totally-unknown",
        ] {
            assert_eq!(ModuleCategory::of(id), System, "{id}");
        }
    }
}
