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
//! (`serde_json::to_vec`), so the daemon needs no per-module knowledge.
//!
//! lifecycle is part of the surface: `/v1/status` carries the daemon's build
//! version (so a newer app can spot a stale orphan), and POST `/v1/shutdown`
//! asks the process to exit gracefully — the managing app has no pid, only
//! this port.

use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::StatusCode;
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
/// breaking subscribers.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum WsFrame {
    Block(BlockSummary),
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
}

/// the router's shared state: a command lane into the node actor, the
/// block-event fan-out for websocket subscribers, and the shutdown signal.
#[derive(Clone)]
pub struct NodeHandle {
    cmds: mpsc::Sender<NodeCommand>,
    events: broadcast::Sender<BlockSummary>,
    shutdown: std::sync::Arc<tokio::sync::Notify>,
}

impl NodeHandle {
    /// build the handle plus the actor-side ends: the command receiver the
    /// actor drains and the event sender it publishes finalized blocks on.
    pub fn channel() -> (Self, mpsc::Receiver<NodeCommand>, broadcast::Sender<BlockSummary>) {
        let (cmd_tx, cmd_rx) = mpsc::channel(COMMAND_BUFFER);
        let (event_tx, _) = broadcast::channel(EVENT_BUFFER);
        let handle = Self {
            cmds: cmd_tx,
            events: event_tx.clone(),
            shutdown: std::sync::Arc::new(tokio::sync::Notify::new()),
        };
        (handle, cmd_rx, event_tx)
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
    root.0.iter().map(|b| format!("{b:02x}")).collect()
}

fn error_response(status: StatusCode, message: &str) -> Response {
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}

/// the actor dropped the reply oneshot — it panicked or shut down mid-request.
fn actor_gone() -> Response {
    error_response(StatusCode::SERVICE_UNAVAILABLE, "node actor dropped the reply")
}

pub fn router(handle: NodeHandle) -> Router {
    Router::new()
        .route("/v1/submit", post(submit))
        .route("/v1/query", post(query))
        .route("/v1/status", get(status))
        .route("/v1/shutdown", post(shutdown))
        .route("/v1/ws", get(ws))
        // the web app is served from a different origin than the node.
        .layer(CorsLayer::permissive())
        .with_state(handle)
}

/// the fallback submitter identity when a client sends no `origin`.
pub const DEFAULT_ORIGIN: &str = "noded";

async fn submit(State(handle): State<NodeHandle>, Json(req): Json<SubmitRequest>) -> Response {
    let payload =
        serde_json::to_vec(&req.payload).expect("a decoded json value re-serializes");
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

async fn shutdown(State(handle): State<NodeHandle>) -> Response {
    // reply first, then signal — the connection closes before the process does.
    handle.shutdown.notify_one();
    Json(serde_json::json!({ "ok": true })).into_response()
}

/// serve the client surface on `listener` until a shutdown request lands
/// (POST /v1/shutdown). the caller owns the runtime this runs on; the host
/// actor lives elsewhere and is reachable only through `handle`'s command
/// lane — which is what lets ANY binary that owns a host (the embedded daemon,
/// the p2p validator) stand up the identical surface.
pub async fn serve(
    listener: tokio::net::TcpListener,
    handle: NodeHandle,
) -> std::io::Result<()> {
    let shutdown = handle.clone();
    axum::serve(listener, router(handle))
        .with_graceful_shutdown(async move { shutdown.shutdown_requested().await })
        .await
}

async fn ws(State(handle): State<NodeHandle>, upgrade: WebSocketUpgrade) -> Response {
    let events = handle.events.subscribe();
    upgrade.on_upgrade(move |socket| stream_blocks(socket, events))
}

async fn stream_blocks(mut socket: WebSocket, mut events: broadcast::Receiver<BlockSummary>) {
    loop {
        match events.recv().await {
            Ok(block) => {
                let frame = serde_json::to_string(&WsFrame::Block(block))
                    .expect("ws frame serializes");
                if socket.send(Message::Text(frame.into())).await.is_err() {
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
