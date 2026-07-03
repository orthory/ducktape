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
use std::sync::{Arc, Mutex};

use axum::body::Bytes;
use axum::extract::rejection::BytesRejection;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{StatusCode, header};
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
        };
        (handle, cmd_rx, event_tx)
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

fn hex_bytes(bytes: &[u8]) -> String {
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
            payload,
            origin,
            reply,
        })
        .await
    {
        return resp;
    }
    match rx.await {
        Ok(Ok(block)) => Json(block).into_response(),
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
}
