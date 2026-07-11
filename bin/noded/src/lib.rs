//! the daemon's client-facing surface: json wire types, the node-actor command
//! channel, and the axum router.
//!
//! the split matters: `host::Host` is deliberately non-Send (single-threaded by
//! design), so http handlers never touch it. they send a [`NodeCommand`] over a
//! runtime-agnostic futures mpsc channel to whichever actor owns the host — the
//! real one lives in `main.rs` on a commonware tokio runner; router tests drive
//! a fake actor on plain tokio. payloads stay opaque json: a submit/query body
//! carries the module's own `*Msg`/`*Query` enum as a json value, encoded to the
//! exact bytes the modules' crate-root `encode_*` helpers would produce
//! (`serde_json::to_vec`), so the daemon needs no per-module knowledge —
//! with ONE deliberate exception: the op-receipt blob lane. receipt bytes
//! never transit consensus (no op carries them), so POST `/v1/files/blob` and
//! GET `/v1/files/blob/{digest}` bypass the actor entirely and talk straight
//! to the node-local [`crate::blobs::BlobHandle`] forge and the block loop share.
//!
//! lifecycle is part of the surface: `/v1/status` carries the daemon's build
//! version (so a newer app can spot a stale orphan), and POST `/v1/shutdown`
//! asks the process to exit gracefully — the managing app has no pid, only
//! this port.

pub mod blobs;
pub mod stream;
pub use stream::{
    ClientMsg, LogRing, RunOutputRegistry, RunStream, ServerFrame, StreamErrorCode, StreamHub,
    StreamOpRow, StreamOrigin, StreamOriginKind, TailItem,
};
// the duckfs product surface lives in its own module (lib.rs is over the size
// cap); re-exported flat so the router keeps its bare handler names and the
// public param structs stay at `noded::CommitBody` &c.
mod files_http;
// The WebSocket side-door token store (consumed by the duck:// WS door in the
// viewer campaign). Public so node-bin's gateway plane can share one store.
pub mod gateway_ws_token;
pub use files_http::*;
// the workspace RPC (`/v1/fs/workspaces`) and its actor-lane `NodeApi` adapter.
// crate-internal: the router registers the handlers and the adapter is used only
// by the workspace handlers — nothing outside the crate touches either.
mod actor_api;
mod workspaces;
// the REAL portable-agent-run provisioner (NodedProvisioner + agent_runs_root,
// the D7 root). public so BOTH node binaries can build one and wire it into
// their DispatchPool via `with_provisioner`.
pub mod agent_provision;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::{Body, Bytes};
use axum::extract::rejection::BytesRejection;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{HeaderMap, Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use commonware_runtime::telemetry::metrics::{EncodeLabelSet, MetricsExt as _, Registered, raw};
use duckfs_core::CHUNK_SIZE;
use futures::SinkExt as _;
use futures::channel::{mpsc, oneshot};
use rand::RngCore as _;
use sdk::StateRoot;
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;

/// inbound command backlog before submit/query callers see backpressure.
pub const COMMAND_BUFFER: usize = 64;
/// internal block-note wakeups buffered per lagging websocket subscriber.
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
#[serde(rename_all = "snake_case")]
pub enum BlockDisposition {
    Applied,
    Rejected,
}

/// one member op inside a finalized block — the per-op detail the explorer
/// fans out over. a block now AGGREGATES the txs from its window, so it carries
/// a vector of these in agreed (applied) order.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootOp {
    /// hex of this op's authenticated author origin (the frame's VERIFIED
    /// signer), or `"system"` / `"module:<id>"` for a non-external origin. on
    /// the embedded daemon's frameless lane this is the SUBMITTER's origin
    /// bytes instead (unverified — that lane authenticates nothing).
    pub proposer: String,
    /// how this op landed: `applied` mutated state; `rejected` finalized but
    /// rolled back (a failed tx). deterministic on every validator.
    pub disposition: BlockDisposition,
    /// this op's target module.
    pub target: String,
    /// this op's dispatch trace, in drain order (empty for a rejected op).
    pub operations: Vec<DispatchInfo>,
    /// best-effort utf-8 preview of this op's payload (module `*Msg` json on
    /// this lane), capped at [`PAYLOAD_PREVIEW_MAX`] chars.
    pub payload: String,
    /// hex of this op's payload content address — sha256 of the exact bytes the
    /// host committed, staged in the node-local blob store so
    /// `GET /v1/files/blob/{op_hash}` serves the full bytes back.
    pub op_hash: String,
}

/// one non-empty finalized block, as the explorer reads it: the block's
/// consensus coordinates (height, frame content hash, post-block app-hash) and
/// the member ops it AGGREGATED, each with its deterministic dispatch trace.
/// stored as the block's row in the index store's blocks database
/// ([`indexer::BlockOps::record`]) and served by `GET /v1/blocks`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockRecord {
    pub height: u64,
    /// hex of the block's content address — the block's hash on this surface.
    /// empty on the embedded daemon's lane: nothing is framed or signed there,
    /// so the field stays honest rather than carrying a fabricated digest.
    pub hash: String,
    /// hex of the composed app-hash after this block settled — the commit.
    pub commit_hash: String,
    /// the member ops this block aggregated, in agreed (applied) order. empty
    /// for an idle/nop block (nothing but the heartbeat filler).
    pub ops: Vec<RootOp>,
}

/// the explorer's wire rendering of one dispatch. `Origin::External` renders
/// as plain `"external"`: the block-level `proposer` field already carries
/// the key, and raw ed25519 bytes are not utf-8 (the `external:<name>`
/// convention assumes the embedded daemon's readable names).
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

// ---------------------------------------------------------------------------
// node metrics: the `ducktape_*` Prometheus series behind GET /metrics.
// shared by every binary serving this surface — the embedded daemon folds a
// block in at submit, the consensus validator at drain — so one Grafana board
// reads them all.
// ---------------------------------------------------------------------------

/// histogram buckets for block apply latency, in SECONDS (Prometheus
/// convention). ~100µs to ~1s — the range one local block apply falls in.
const LATENCY_BUCKETS: [f64; 13] = [
    0.0001, 0.00025, 0.0005, 0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0,
];

/// labels for the per-dispatch counter. kept LOW-CARDINALITY: `module` is the
/// bounded registered set; `origin` is the trigger KIND only — never the
/// specific submitter name or emitter id.
#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct DispatchLabels {
    module: String,
    origin: String,
}

/// the low-cardinality trigger KIND of a dispatch origin — the metrics label.
fn origin_kind(origin: &sdk::Origin) -> &'static str {
    match origin {
        sdk::Origin::External(_) => "external",
        sdk::Origin::Module(_) => "module",
        sdk::Origin::System => "system",
    }
}

/// the node's own Prometheus series, registered INTO commonware's runtime
/// registry so one `context.encode()` (GET /metrics) serves runtime + app
/// metrics together. each `Registered` handle is retained for the process life;
/// updates go through its `Deref` to the underlying metric.
pub struct NodeMetrics {
    block_height: Registered<raw::Gauge>,
    blocks_total: Registered<raw::Counter>,
    ops_total: Registered<raw::Counter>,
    apply_latency: Registered<raw::Histogram>,
    dispatch_total: Registered<raw::Family<DispatchLabels, raw::Counter>>,
}

impl NodeMetrics {
    /// register the `ducktape_*` series on the runtime context (root context, so
    /// names carry no child prefix).
    pub fn register<C: commonware_runtime::Metrics>(context: &C) -> Self {
        Self {
            block_height: context.gauge(
                "ducktape_block_height",
                "latest committed local block height",
            ),
            // NB: the registry appends the OpenMetrics `_total` suffix to a
            // counter, so the exposed names are `ducktape_blocks_total` and
            // `ducktape_dispatch_total{…}` — DON'T put `_total` in the name here
            // or it doubles.
            blocks_total: context.counter(
                "ducktape_blocks",
                "committed local blocks since daemon start",
            ),
            // one BLOCK now aggregates N member ops; `ducktape_blocks_total`
            // counts blocks, `ducktape_ops_total` counts the aggregated ops, so
            // ops/blocks is the average batch size.
            ops_total: context.counter(
                "ducktape_ops",
                "member ops aggregated into committed local blocks since daemon start",
            ),
            apply_latency: context.histogram(
                "ducktape_block_apply_latency_seconds",
                "node-local wall-clock cost of applying one block",
                LATENCY_BUCKETS,
            ),
            dispatch_total: context.family(
                "ducktape_dispatch",
                "module dispatches, by module and trigger-origin kind",
            ),
        }
    }

    /// fold one applied block into the series: height, count, this node's
    /// wall-clock apply latency, and the per-module dispatch counters.
    pub fn record_block(&self, height: u64, latency_us: u64, dispatches: &[host::DispatchRecord]) {
        self.block_height.set(height as i64);
        self.blocks_total.inc();
        // microseconds → seconds for the Prometheus convention.
        self.apply_latency.observe(latency_us as f64 / 1_000_000.0);
        for d in dispatches {
            self.dispatch_total
                .get_or_create(&DispatchLabels {
                    module: d.module.clone(),
                    origin: origin_kind(&d.origin).to_string(),
                })
                .inc();
        }
    }

    /// count the member ops an applied block aggregated (`ducktape_ops_total`).
    /// called once per applied block alongside [`record_block`](Self::record_block).
    pub fn record_ops(&self, ops: usize) {
        for _ in 0..ops {
            self.ops_total.inc();
        }
    }

    /// follow the committed height WITHOUT recording a block apply — the
    /// validator lane calls this for rejected frames (a deterministic no-op
    /// advances the height but is not a sample worth the block series; the
    /// idle heartbeat nop lands here, so it never pollutes the histogram).
    pub fn record_height(&self, height: u64) {
        self.block_height.set(height as i64);
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
    /// this node's mesh identity (hex ed25519 key) — what a client stamps
    /// into ops that route peer traffic to it (chat's `JoinHuddle.node`).
    /// empty on daemons with no mesh identity (the embedded local daemon).
    #[serde(default)]
    pub public_key: String,
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
#[serde(rename_all = "snake_case")]
pub enum ModuleCategory {
    Workspace,
    Developer,
    Automation,
    System,
}

impl ModuleCategory {
    /// The category a module id belongs to. Ids not listed here —
    /// infrastructure and internal modules (files, saga, identity, kv,
    /// valset, governance, vaults, directory, …) — fall to `System`, so a new
    /// or unknown module always groups sensibly rather than breaking the view.
    pub fn of(id: &str) -> Self {
        match id {
            "chat" | "tasks" | "inbox" | "pages" => Self::Workspace,
            "forge" | "agent" | "evm" => Self::Developer,
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

// ---- the call lane ----------------------------------------------------------
// the webview end of a huddle: GET /v1/call/ws?channel=<id> upgrades to a
// typed socket that carries the huddle's audio, camera video, and call control
// together. the handler asks the node's call hub for a session over the request
// lane below; a daemon without a hub answers 503, and every refusal path says
// WHY as one text frame before closing.
//
// the binary frame layout (tag byte, header fields, BE headers/LE pcm payload
// per D1) and its encode/decode functions live in `chat::call_wire` — the
// single definition site this handler ports onto below. text frames are json
// control: client→server `CallClientControl` (recipients / beacon /
// keyframeRequest), server→client `CallServerControl` (keyframeRequest /
// peerBeacon / rateHint).
//
// the hub side lives with the mesh (only the p2p validator runs one): it
// fragments/reassembles the video ends over `Service::Video` and routes control
// (keyframe kicks, presence beacons, rate hints — see `chat::video`).

/// call-control the WEBVIEW asks the hub to act on (webview → hub).
pub enum CallControlIn {
    /// our local presence/state, pushed immediately AND repeated at 1 Hz as
    /// this session's beacon to every recipient. `sharing` = the video lane is a
    /// screen share rather than the camera.
    Beacon {
        muted: bool,
        camera_on: bool,
        sharing: bool,
    },
    /// our decoder lost `peer`'s stream — ask `peer` for a fresh keyframe.
    KeyframeRequest { peer: [u8; 32] },
}

/// call-control the hub surfaces to the WEBVIEW (hub → webview).
pub enum CallControlOut {
    /// a peer's receiver asked us to send it a fresh keyframe — the webview
    /// tells its encoder to emit one (rate-limited to ≤1 Hz by the hub).
    KeyframeRequest,
    /// a peer's 1 Hz presence beacon — drives the tile's mute/camera badges +
    /// the screen-share treatment.
    PeerBeacon {
        peer: [u8; 32],
        muted: bool,
        camera_on: bool,
        sharing: bool,
    },
    /// the effective outbound bitrate cap (min of every peer's hint) — the
    /// webview retargets its encoder. emitted only when the value changes.
    RateHint { max_kbps: u32 },
}

/// one live huddle session's channel ends, hub ↔ websocket handler / gateway.
pub struct CallSession {
    /// captured mic frames, exactly [`chat::voice::FRAME_SAMPLES`] samples each.
    pub pcm_in: tokio::sync::mpsc::Sender<Vec<i16>>,
    /// mixed playout frames at the 20 ms tick, same shape.
    pub mixed_out: tokio::sync::mpsc::Receiver<Vec<i16>>,
    /// where this session's frames fan out: the huddle roster's node keys
    /// (raw ed25519 bytes), steered by the client as consensus state changes.
    pub recipients: tokio::sync::watch::Sender<Vec<[u8; 32]>>,
    /// captured camera frames webview → hub (fragmented onto `Service::Video`).
    pub video_in: tokio::sync::mpsc::Sender<chat::call_wire::CapturedFrame>,
    /// reassembled peer camera frames hub → webview.
    pub video_out: tokio::sync::mpsc::Receiver<chat::call_wire::PeerFrame>,
    /// call-control webview → hub (local beacon, keyframe asks).
    pub control_in: tokio::sync::mpsc::Sender<CallControlIn>,
    /// call-control hub → webview (peer beacons, keyframe kicks, rate hints).
    pub control_out: tokio::sync::mpsc::Receiver<CallControlOut>,
}

/// a websocket handler's ask: open (or replace) the call session for a
/// channel. the hub replies with the session's ends or a refusal string.
pub struct CallSessionRequest {
    pub channel_id: String,
    pub reply: tokio::sync::oneshot::Sender<Result<CallSession, String>>,
}

/// the request lane into the call hub.
pub type CallLane = tokio::sync::mpsc::Sender<CallSessionRequest>;

/// client → server control messages on the call socket (text frames).
#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CallClientControl {
    /// replace the fan-out set with these hex node keys (self excluded —
    /// the client tracks the consensus huddle roster).
    Recipients { peers: Vec<String> },
    /// this client's ephemeral state; the hub beacons it to peers at 1 Hz.
    /// `sharing` defaults false so a pre-share client (no field) still parses.
    Beacon {
        muted: bool,
        camera_on: bool,
        #[serde(default)]
        sharing: bool,
    },
    /// the decoder lost sync with `peer` — ask it for a keyframe.
    KeyframeRequest { peer: String },
}

/// server → client control messages on the call socket (text frames).
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CallServerControl {
    /// a peer lost sync with US: encode the next frame as a keyframe.
    KeyframeRequest,
    /// a peer's 1 Hz beacon (ephemeral presence/state — never consensus).
    PeerBeacon {
        peer: String,
        muted: bool,
        camera_on: bool,
        sharing: bool,
    },
    /// send at no more than this (min across peers' loss reports).
    RateHint {
        #[cfg_attr(test, ts(type = "number"))]
        max_kbps: u32,
    },
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

/// One bounded invocation through a globally signed gateway route. The
/// full node drains this lane through `Service::Gateway`; the embedded daemon
/// leaves it unwired because it has no authenticated network transport.
pub struct GatewayJob {
    /// Derived from the locally finalized RouteRecord, never client input.
    pub publisher_node: [u8; 32],
    pub max_response_bytes: u64,
    pub head: gateway::ProxyRequestHead,
    pub body: Vec<u8>,
    pub reply: oneshot::Sender<Result<GatewayResponse, GatewayFailure>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayResponse {
    pub head: gateway::ProxyResponseHead,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatewayFailure {
    Invalid(String),
    Forbidden(String),
    NotFound(String),
    Conflict(String),
    Unavailable(String),
}

pub type GatewayLane = tokio::sync::mpsc::Sender<GatewayJob>;

const GATEWAY_SESSION_IDLE: Duration = Duration::from_secs(10 * 60);
const MAX_GATEWAY_SESSIONS: usize = 64;

#[derive(Clone)]
struct GatewaySession {
    account_id: Vec<u8>,
    name: gateway::RouteName,
    revision: u64,
    last_used: Instant,
}

#[derive(Clone)]
struct BrowserGateway {
    listen: SocketAddr,
    sessions: Arc<tokio::sync::Mutex<HashMap<String, GatewaySession>>>,
}

/// the router's shared state: a command lane into the node actor, the
/// stream hub for websocket subscribers, the shutdown signal, and the
/// node-local blob store the files module shares.
#[derive(Clone)]
pub struct NodeHandle {
    cmds: mpsc::Sender<NodeCommand>,
    hub: StreamHub,
    shutdown: std::sync::Arc<tokio::sync::Notify>,
    /// the files blob lane. NOT a command into the actor: chunk bytes stay
    /// node-local by design (never consensus state, never an op), so the http
    /// handlers read/write this store directly.
    blobs: crate::blobs::BlobHandle,
    /// the forge module's on-disk repo base dir (`<storage>/<forge subdir>`);
    /// each named repo lives at `<forge_repo>/<name>` as a real libgit2 repo.
    /// threaded in so the git upload-pack (clone/fetch) handler can open a repo
    /// READ-ONLY and serve its objects — the ONE route that reads forge's git
    /// substrate directly instead of over the actor lane. `None` on a handle
    /// that never serves the git lane (the router tests' fake actor), which
    /// makes upload-pack a clean 500 there rather than a panic.
    forge_repo: Option<PathBuf>,
    /// the per-module derived index (fluent31-backed read models). node-local
    /// like `blobs`: the actor is the one WRITER as blocks commit;
    /// the `/v1/index/*` handlers read it directly through MVCC snapshots, so
    /// an index scan never crosses the actor command lane. `None` on a handle
    /// whose embedder configured no index (the router tests' fake actor) —
    /// index routes answer 503 there.
    index: Option<Arc<indexer::IndexStore>>,
    /// the call hub's session-request lane. `None` on daemons without a mesh
    /// (the embedded daemon, router tests) — `/v1/call/ws` answers 503 there.
    call: Option<CallLane>,
    /// Purpose-specific gateway request lane. No raw peer, filesystem, or
    /// arbitrary socket proxy is exposed through the client surface.
    gateway: Option<GatewayLane>,
    /// Dedicated least-privilege browser origin for gateway rendering. It is
    /// a separate loopback listener, never the node API origin.
    browser_gateway: Option<BrowserGateway>,
    /// the root dir the duckfs workspace RPC materializes managed checkouts
    /// under (`<storage>/duckfs-workspaces`). node-local disk state, threaded in
    /// like `forge_repo`; `None` on a handle that never serves the seam (the
    /// router tests' fake handle), which makes `/v1/fs/workspaces` a clean 503.
    duckfs_workspaces: Option<PathBuf>,
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
            shutdown: std::sync::Arc::new(tokio::sync::Notify::new()),
            blobs: crate::blobs::BlobHandle::default(),
            forge_repo: None,
            index: None,
            call: None,
            gateway: None,
            browser_gateway: None,
            duckfs_workspaces: None,
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

    /// Point gateway requests at the full node's authenticated overlay
    /// stream. `net.duck` remains a local network-content read.
    pub fn with_gateway(mut self, lane: GatewayLane) -> Self {
        self.gateway = Some(lane);
        self
    }

    /// Enable gateway browsing on a separately bound loopback listener. The
    /// caller binds first so port 0 becomes an actual session-returned port.
    pub fn with_browser_gateway(mut self, listen: SocketAddr) -> Self {
        self.browser_gateway = Some(BrowserGateway {
            listen,
            sessions: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
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
        .route("/v1/call/ws", get(call_ws))
        .route(
            "/v1/gateway/proxy",
            post(gateway_proxy).layer(DefaultBodyLimit::max(
                gateway::MAX_REQUEST_BODY_BYTES as usize * 2 + gateway::MAX_PROXY_HEAD_BYTES,
            )),
        )
        .route("/v1/gateway/session", post(gateway_session))
        .route(
            "/v1/files/blob",
            // one receipt per request; the json routes keep axum's (smaller)
            // default limit.
            post(put_blob).layer(DefaultBodyLimit::max(MAX_BLOB_BODY_BYTES)),
        )
        .route("/v1/files/blob/{digest}", get(get_blob))
        // ---- duckfs product surface ----
        // thin convenience wrappers over the files module's ops/queries: each
        // encodes the duckfs wire server-side and threads it through the SAME
        // submit/query actor seam /v1/submit and /v1/query use — no new
        // consensus path. distinct plane from the op-receipt /v1/files/blob lane
        // above (the node-local blobstore), which these never touch.
        .route(
            "/v1/files/stage",
            // one duckfs chunk per request, so the body cap IS the single-chunk
            // cap (CHUNK_SIZE, the module's own putblob ceiling); a larger body
            // could never be a valid staged chunk, so the layer rejects it 413.
            post(files_stage).layer(DefaultBodyLimit::max(CHUNK_SIZE as usize)),
        )
        .route("/v1/files/commit", post(files_commit))
        .route("/v1/files/pin", post(files_pin))
        .route("/v1/files/watch", post(files_watch))
        .route("/v1/files/stat", get(files_stat))
        .route("/v1/files/ls", get(files_ls))
        .route("/v1/files/read", get(files_read))
        .route("/v1/files/find", get(files_find))
        .route("/v1/files/grep", get(files_grep))
        .route("/v1/files/history", get(files_history))
        // the read/probe surface the checkout/commit engine drives.
        .route("/v1/files/refs", get(files_refs))
        .route("/v1/files/diff", get(files_diff))
        .route("/v1/files/has-chunks", get(files_has_chunks))
        // ---- duckfs workspace RPC (the jobs/sandbox seam) ----
        // managed checkouts under the injected root: create, commit (409 on a
        // structured conflict), delete. `None` root → 503.
        .route("/v1/fs/workspaces", post(workspaces::create_workspace))
        .route(
            "/v1/fs/workspaces/{id}/commit",
            post(workspaces::commit_workspace),
        )
        .route(
            "/v1/fs/workspaces/{id}",
            delete(workspaces::delete_workspace),
        )
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GatewayProxyRequest {
    pub head: gateway::ProxyRequestHead,
    pub body_b64: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProxyReply {
    pub head: gateway::ProxyResponseHead,
    pub body_b64: String,
}

/// The node surface predates gateway and intentionally has permissive CORS for
/// the web console. Gateway is a network pivot, so its two API entries add a
/// narrower browser boundary: native clients omit Origin, while only the
/// bundled Tauri console origins may call from a WebView. Publisher sessions
/// and arbitrary websites fail before route resolution or overlay work.
fn gateway_api_origin_allowed(headers: &HeaderMap) -> bool {
    let mut origins = headers.get_all(header::ORIGIN).iter();
    let first = origins.next();
    if origins.next().is_some() {
        return false;
    }
    match first {
        Some(value) => {
            let Ok(origin) = value.to_str() else {
                return false;
            };
            matches!(
                origin,
                "tauri://localhost" | "http://tauri.localhost" | "https://tauri.localhost"
            ) || (cfg!(debug_assertions) && origin == "http://localhost:1430")
        }
        // A real native client has neither header. Browser requests without an
        // Origin still carry Fetch Metadata, so do not let that omission turn
        // into a bypass (for example from a sandboxed publisher document).
        None => !headers.contains_key("sec-fetch-site"),
    }
}

fn gateway_api_origin_guard(headers: &HeaderMap) -> Option<Response> {
    (!gateway_api_origin_allowed(headers)).then(|| {
        error_response(
            StatusCode::FORBIDDEN,
            "gateway API is limited to the trusted Ducktape console and native clients",
        )
    })
}

async fn current_route(
    handle: &NodeHandle,
    account_id: &[u8],
    name: &gateway::RouteName,
) -> Result<gateway::RouteRecord, GatewayFailure> {
    gateway::validate_account_id(account_id).map_err(GatewayFailure::Invalid)?;
    name.validate().map_err(GatewayFailure::Invalid)?;
    let (reply, rx) = oneshot::channel();
    let mut commands = handle.cmds.clone();
    commands
        .send(NodeCommand::Query {
            target: "gateway".into(),
            req: gateway::encode_query(&gateway::GatewayQuery::Get {
                account_id: account_id.to_vec(),
                name: name.clone(),
            }),
            reply,
        })
        .await
        .map_err(|_| GatewayFailure::Unavailable("node actor is gone".into()))?;
    let bytes = tokio::time::timeout(Duration::from_secs(5), rx)
        .await
        .map_err(|_| GatewayFailure::Unavailable("gateway route query timed out".into()))?
        .map_err(|_| GatewayFailure::Unavailable("node actor dropped the query".into()))?
        .map_err(GatewayFailure::Unavailable)?;
    match gateway::decode_reply(&bytes) {
        Ok(gateway::GatewayReply::Route(route)) => match *route {
            Some(record) if record.statement.route.is_some() => Ok(record),
            _ => Err(GatewayFailure::NotFound(
                "gateway route is not published".into(),
            )),
        },
        Ok(gateway::GatewayReply::Routes(_)) => Err(GatewayFailure::Unavailable(
            "gateway returned an unexpected route-list reply".into(),
        )),
        Err(error) => Err(GatewayFailure::Unavailable(error)),
    }
}

async fn proxy_current(
    handle: &NodeHandle,
    head: gateway::ProxyRequestHead,
    body: Vec<u8>,
) -> Result<GatewayResponse, GatewayFailure> {
    gateway::validate_proxy_request_head(&head).map_err(GatewayFailure::Invalid)?;
    if body.len() as u64 != head.body_len {
        return Err(GatewayFailure::Invalid(
            "gateway body length does not match its request head".into(),
        ));
    }
    let record = current_route(handle, &head.account_id, &head.name).await?;
    if record.statement.revision != head.revision {
        return Err(GatewayFailure::Conflict(
            "gateway route changed; resolve the name again".into(),
        ));
    }
    if !gateway::request_matches_record(&head, &record) {
        return Err(GatewayFailure::Forbidden(
            "gateway request is outside the current signed policy".into(),
        ));
    }
    let publisher_node: [u8; 32] = record
        .statement
        .publisher_node
        .as_slice()
        .try_into()
        .map_err(|_| GatewayFailure::Unavailable("invalid publisher in route state".into()))?;
    let max_response_bytes = record
        .statement
        .route
        .as_ref()
        .expect("current_route rejects tombstones")
        .policy
        .max_response_bytes;
    let Some(lane) = handle.gateway.clone() else {
        return Err(GatewayFailure::Unavailable(
            "gateway request requires an active network overlay".into(),
        ));
    };
    let (reply, rx) = oneshot::channel();
    lane.send(GatewayJob {
        publisher_node,
        max_response_bytes,
        head,
        body,
        reply,
    })
    .await
    .map_err(|_| GatewayFailure::Unavailable("gateway plane is not available".into()))?;
    let response = tokio::time::timeout(Duration::from_secs(15), rx)
        .await
        .map_err(|_| GatewayFailure::Unavailable("gateway publisher timed out".into()))?
        .map_err(|_| GatewayFailure::Unavailable("gateway plane dropped the request".into()))??;
    gateway::validate_response_head(&response.head).map_err(GatewayFailure::Unavailable)?;
    if response.body.len() as u64 > max_response_bytes {
        return Err(GatewayFailure::Unavailable(
            "publisher exceeded the route response cap".into(),
        ));
    }
    Ok(response)
}

async fn gateway_proxy(
    State(handle): State<NodeHandle>,
    headers: HeaderMap,
    Json(request): Json<GatewayProxyRequest>,
) -> Response {
    use base64::Engine as _;
    if let Some(response) = gateway_api_origin_guard(&headers) {
        return response;
    }
    let body = match base64::engine::general_purpose::STANDARD.decode(request.body_b64) {
        Ok(body) => body,
        Err(error) => {
            return error_response(StatusCode::BAD_REQUEST, &format!("body_b64: {error}"));
        }
    };
    match proxy_current(&handle, request.head, body).await {
        Ok(response) => Json(GatewayProxyReply {
            head: response.head,
            body_b64: base64::engine::general_purpose::STANDARD.encode(response.body),
        })
        .into_response(),
        Err(failure) => gateway_failure_response(failure),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GatewaySessionRequest {
    pub account_id: Vec<u8>,
    pub name: gateway::RouteName,
    pub revision: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GatewaySessionReply {
    url: String,
}

async fn gateway_session(
    State(handle): State<NodeHandle>,
    headers: HeaderMap,
    Json(request): Json<GatewaySessionRequest>,
) -> Response {
    if let Some(response) = gateway_api_origin_guard(&headers) {
        return response;
    }
    let Some(gateway) = handle.browser_gateway.clone() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "gateway browser gateway is not configured",
        );
    };
    if handle.gateway.is_none() {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "gateway browsing requires an active network overlay",
        );
    }
    let record = match current_route(&handle, &request.account_id, &request.name).await {
        Ok(record) => record,
        Err(failure) => return gateway_failure_response(failure),
    };
    if record.statement.revision != request.revision {
        return error_response(
            StatusCode::CONFLICT,
            "gateway route changed; resolve the name again",
        );
    }

    let now = Instant::now();
    let mut sessions = gateway.sessions.lock().await;
    sessions.retain(|_, session| now.duration_since(session.last_used) < GATEWAY_SESSION_IDLE);
    if sessions.len() >= MAX_GATEWAY_SESSIONS
        && let Some(oldest) = sessions
            .iter()
            .min_by_key(|(_, session)| session.last_used)
            .map(|(token, _)| token.clone())
    {
        sessions.remove(&oldest);
    }
    let token = loop {
        let mut random = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut random);
        let candidate = hex_bytes(&random);
        if !sessions.contains_key(&candidate) {
            break candidate;
        }
    };
    sessions.insert(
        token.clone(),
        GatewaySession {
            account_id: request.account_id,
            name: request.name,
            revision: request.revision,
            last_used: now,
        },
    );
    Json(GatewaySessionReply {
        url: format!("http://{token}.localhost:{}/", gateway.listen.port()),
    })
    .into_response()
}

/// Dedicated gateway-rendering router: no node API, no permissive CORS, and
/// no route that can address another `.duck` route with ambient user power.
pub fn gateway_browser_router(handle: NodeHandle) -> Router {
    Router::new()
        .fallback(gateway_browser_proxy)
        .layer(DefaultBodyLimit::max(
            gateway::MAX_REQUEST_BODY_BYTES as usize,
        ))
        .with_state(handle)
}

fn gateway_method(method: &Method) -> Option<gateway::RouteMethod> {
    match *method {
        Method::GET => Some(gateway::RouteMethod::Get),
        Method::HEAD => Some(gateway::RouteMethod::Head),
        Method::POST => Some(gateway::RouteMethod::Post),
        Method::PUT => Some(gateway::RouteMethod::Put),
        Method::PATCH => Some(gateway::RouteMethod::Patch),
        Method::DELETE => Some(gateway::RouteMethod::Delete),
        _ => None,
    }
}

fn gateway_request_headers(headers: &HeaderMap) -> Result<Vec<gateway::ProxyHeader>, String> {
    // Cookie now flows end to end; only hop-by-hop / forwarding / identity
    // headers (and any x-duck-* spoof) are stripped, via the shared denylist.
    let mut forwarded: Vec<gateway::ProxyHeader> = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for (name, value) in headers {
        let name = name.as_str().to_ascii_lowercase();
        if !gateway::header_forwardable(&name) {
            continue;
        }
        if !seen.insert(name.clone()) {
            return Err(format!("gateway browser rejects duplicate {name} headers"));
        }
        forwarded.push(gateway::ProxyHeader {
            name: name.clone(),
            value: value
                .to_str()
                .map_err(|_| format!("gateway browser received non-ASCII {name}"))?
                .to_string(),
        });
    }
    forwarded.sort_by(|left, right| left.name.cmp(&right.name));
    gateway::validate_headers(&forwarded, "request")?;
    Ok(forwarded)
}

async fn gateway_browser_proxy(
    State(handle): State<NodeHandle>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Some(gateway) = handle.browser_gateway.clone() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "gateway browser is disabled",
        );
    };
    let Some(method) = gateway_method(&method) else {
        return (
            StatusCode::METHOD_NOT_ALLOWED,
            [(header::ALLOW, "GET, HEAD, POST, PUT, PATCH, DELETE")],
            "method is not part of the gateway protocol",
        )
            .into_response();
    };
    let expected_suffix = format!(".localhost:{}", gateway.listen.port());
    let Some(host) = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
    else {
        return error_response(
            StatusCode::MISDIRECTED_REQUEST,
            "invalid gateway session origin",
        );
    };
    let Some(token) = host.strip_suffix(&expected_suffix).filter(|token| {
        token.len() == 32
            && token
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }) else {
        return error_response(
            StatusCode::MISDIRECTED_REQUEST,
            "invalid gateway session origin",
        );
    };
    let session_origin = format!("http://{host}");
    if headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|origin| origin != session_origin)
    {
        return error_response(StatusCode::FORBIDDEN, "cross-origin gateway call denied");
    }
    if headers
        .get("sec-fetch-site")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|site| site != "same-origin" && site != "none")
    {
        return error_response(StatusCode::FORBIDDEN, "cross-site gateway call denied");
    }
    let now = Instant::now();
    let session = {
        let mut sessions = gateway.sessions.lock().await;
        sessions.retain(|_, session| now.duration_since(session.last_used) < GATEWAY_SESSION_IDLE);
        let Some(session) = sessions.get_mut(token) else {
            return error_response(StatusCode::GONE, "gateway session expired");
        };
        session.last_used = now;
        session.clone()
    };
    let forwarded = match gateway_request_headers(&headers) {
        Ok(headers) => headers,
        Err(error) => return error_response(StatusCode::BAD_REQUEST, &error),
    };
    let head = gateway::ProxyRequestHead {
        account_id: session.account_id,
        name: session.name,
        revision: session.revision,
        method,
        path_and_query: uri
            .path_and_query()
            .map(|value| value.as_str())
            .unwrap_or("/")
            .to_string(),
        headers: forwarded,
        body_len: body.len() as u64,
        upgrade: false,
    };
    let response = match proxy_current(&handle, head, body.to_vec()).await {
        Ok(response) => response,
        Err(failure) => return gateway_failure_response(failure),
    };
    let status = StatusCode::from_u16(response.head.status).unwrap_or(StatusCode::BAD_GATEWAY);
    if status.is_informational() || status == StatusCode::SWITCHING_PROTOCOLS {
        return error_response(
            StatusCode::BAD_GATEWAY,
            "publisher returned an invalid status",
        );
    }
    let content_security_policy = format!(
        "default-src 'none'; script-src 'unsafe-inline' {session_origin}; style-src 'unsafe-inline' {session_origin}; img-src {session_origin} data: blob:; connect-src {session_origin}; font-src {session_origin} data:; media-src 'none'; frame-src 'none'; child-src 'none'; worker-src 'none'; manifest-src 'none'; object-src 'none'; form-action {session_origin}; base-uri 'none'; frame-ancestors 'none'; sandbox allow-scripts allow-same-origin allow-forms; webrtc 'block'"
    );
    let mut builder = Response::builder()
        .status(status)
        .header(header::CACHE_CONTROL, "no-store")
        .header("x-content-type-options", "nosniff")
        .header("x-dns-prefetch-control", "off")
        .header("referrer-policy", "no-referrer")
        .header("cross-origin-resource-policy", "same-origin")
        .header("cross-origin-opener-policy", "same-origin")
        .header("origin-agent-cluster", "?1")
        .header("access-control-allow-origin", &session_origin)
        .header(header::VARY, "Origin")
        .header(
            "permissions-policy",
            "accelerometer=(), camera=(), clipboard-read=(), clipboard-write=(), display-capture=(), encrypted-media=(), fullscreen=(), geolocation=(), gyroscope=(), hid=(), idle-detection=(), local-fonts=(), magnetometer=(), microphone=(), midi=(), payment=(), picture-in-picture=(), publickey-credentials-create=(), publickey-credentials-get=(), screen-wake-lock=(), serial=(), storage-access=(), usb=(), window-management=()",
        )
        .header(header::CONTENT_SECURITY_POLICY, content_security_policy);
    builder = builder.header(
        header::CONTENT_TYPE,
        gateway::header_value(&response.head.headers, "content-type")
            .unwrap_or("application/octet-stream"),
    );
    for name in ["etag", "last-modified", "location", "retry-after"] {
        if let Some(value) = gateway::header_value(&response.head.headers, name) {
            builder = builder.header(name, value);
        }
    }
    let body = if method == gateway::RouteMethod::Head
        || status == StatusCode::NO_CONTENT
        || status == StatusCode::NOT_MODIFIED
    {
        Body::empty()
    } else {
        Body::from(response.body)
    };
    builder
        .body(body)
        .unwrap_or_else(|_| error_response(StatusCode::BAD_GATEWAY, "invalid publisher response"))
}

fn gateway_failure_response(failure: GatewayFailure) -> Response {
    match failure {
        GatewayFailure::Invalid(detail) => error_response(StatusCode::BAD_REQUEST, &detail),
        GatewayFailure::Forbidden(detail) => error_response(StatusCode::FORBIDDEN, &detail),
        GatewayFailure::NotFound(detail) => error_response(StatusCode::NOT_FOUND, &detail),
        GatewayFailure::Conflict(detail) => error_response(StatusCode::CONFLICT, &detail),
        GatewayFailure::Unavailable(detail) => error_response(StatusCode::BAD_GATEWAY, &detail),
    }
}

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
                    .with_indexer(Box::new(chat::index::ChatIndex::new("chat")))
                    .with_indexer(Box::new(tasks::index::TasksIndex::new("tasks")))
                    .with_indexer(Box::new(pages::index::PagesIndex::new("pages")))
                    .with_indexer(Box::new(saga::index::UsageIndex::new("saga")))
                    .with_indexer(Box::new(evm::index::EvmIndex::new("evm"))),
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
/// submitter identities render via [`indexer::user_handle`] — printable claimed
/// names pass through, raw ed25519 pubkeys become hex (never the lossy `�`
/// boxes a plain utf-8 decode leaves). the same convention drives a mapper's
/// from-state rebuild (see `chat::index` `author_from_ref`), so folded and
/// rebuilt rows match byte-for-byte on BOTH lanes. hex-keyed identity belongs
/// to the explorer row's `proposer`, not the index op rows.
pub fn index_origin(origin: &sdk::Origin) -> indexer::OriginTag {
    match origin {
        sdk::Origin::External(id) => indexer::OriginTag::external(indexer::user_handle(id)),
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
// the derived-index read lane. like the blob lane these
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

fn index_store(handle: &NodeHandle) -> Option<&Arc<indexer::IndexStore>> {
    handle.index.as_ref()
}

fn no_index_store_response() -> Response {
    error_response(StatusCode::SERVICE_UNAVAILABLE, "no index store configured")
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
    let Some(store) = index_store(&handle) else {
        return no_index_store_response();
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
    let Some(store) = index_store(&handle) else {
        return no_index_store_response();
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
    let Some(store) = index_store(&handle) else {
        return no_index_store_response();
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
    let Some(store) = index_store(&handle) else {
        return no_index_store_response();
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

/// body cap for the op-receipt blob lane. a receipt-lane bound only —
/// unrelated to duckfs chunking, which rides the op stream.
const MAX_BLOB_BODY_BYTES: usize = 4 * 1024 * 1024;

/// POST /v1/files/blob — raw receipt bytes in, `{"digest":"<64-hex>"}` out.
///
/// bytes go straight into the node-local blob store; NOTHING reaches the node
/// actor and no op is submitted. the route's body limit is
/// `MAX_BLOB_BODY_BYTES`, and an oversized body is a 413 in the daemon's json
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
    let Some(raw) = duckfs_core::from_hex_32(&digest) else {
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
/// the ref namespace pushes may touch: any branch. a command outside
/// `refs/heads/*` (tags, notes) is refused with a per-ref `ng`.
const GIT_HEADS_PREFIX: &str = "refs/heads/";
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

/// query the forge module for a repo's committed branches (`[]` == unborn).
/// errors surface as an http `Response` so callers can early-return them.
async fn forge_refs(handle: &NodeHandle, repo: &str) -> Result<Vec<forge::RefHead>, Response> {
    let req = forge::encode_query(&forge::ForgeQuery::ListRefs {
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
    match forge::decode_reply(&bytes) {
        Ok(forge::ForgeReply::Refs(refs)) => Ok(refs),
        Ok(_) => Err(error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "unexpected forge reply to ListRefs",
        )),
        Err(err) => Err(error_response(StatusCode::INTERNAL_SERVER_ERROR, &err)),
    }
}

/// build a receive-pack `report-status` body: `unpack ok`, one status line per
/// ref, then a flush. each entry is `(full refname, None == ok | Some(reason)
/// == ng)`. forge's PushRefs is ATOMIC, so callers report one shared fate for
/// every ref of a push. the pack is always received by the time we answer, so
/// `unpack ok` is unconditional (we don't verify closure here).
fn git_report_status(results: &[(String, Option<String>)]) -> Response {
    let mut body = Vec::new();
    body.extend_from_slice(&pkt_line(b"unpack ok\n"));
    for (refname, err) in results {
        let status_line = match err {
            None => format!("ok {refname}\n"),
            Some(reason) => format!("ng {refname} {reason}\n"),
        };
        body.extend_from_slice(&pkt_line(status_line.as_bytes()));
    }
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
/// — a clone then reports an empty repository. a born repo advertises EVERY
/// committed branch; a fetch advertisement leads with a `HEAD` line at main's
/// oid so `git clone` resolves the default branch to check out. capabilities
/// ride the first emitted line after a NUL, per the v0 protocol.
async fn git_advertise_refs(handle: &NodeHandle, repo: &str, service: GitService) -> Response {
    let refs = match forge_refs(handle, repo).await {
        Ok(refs) => refs,
        Err(resp) => return resp,
    };
    let caps = service.caps();

    let mut body = Vec::new();
    body.extend_from_slice(&pkt_line(
        format!("# service={}\n", service.name()).as_bytes(),
    ));
    body.extend_from_slice(GIT_FLUSH_PKT);
    if refs.is_empty() {
        body.extend_from_slice(&pkt_line(
            format!("{GIT_ZERO_OID} capabilities^{{}}\0{caps}\n").as_bytes(),
        ));
    } else {
        let mut lines: Vec<String> = Vec::new();
        if matches!(service, GitService::Upload)
            && let Some(main) = refs.iter().find(|r| r.name == "main")
        {
            lines.push(format!("{} HEAD", main.head));
        }
        for r in &refs {
            lines.push(format!("{} {GIT_HEADS_PREFIX}{}", r.head, r.name));
        }
        for (i, line) in lines.iter().enumerate() {
            if i == 0 {
                body.extend_from_slice(&pkt_line(format!("{line}\0{caps}\n").as_bytes()));
            } else {
                body.extend_from_slice(&pkt_line(format!("{line}\n").as_bytes()));
            }
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
/// and CAS every branch through ONE atomic forge `PushRefs` op (one submit ==
/// one block). branch deletions (`:feature`) ride the same op pack-free. the
/// response is a git `report-status` reflecting the push's shared fate.
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
    if commands.is_empty() {
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
    }

    // each command line is `<old> <new> <refname>`, with capabilities after a
    // NUL on the FIRST line. parse every command — one push may update several
    // branches, and forge applies them ATOMICALLY.
    let mut cmds: Vec<(String, String, String)> = Vec::new();
    for raw in &commands {
        let nul = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
        let line = std::str::from_utf8(&raw[..nul])
            .map(str::trim_end)
            .unwrap_or("");
        let mut parts = line.split(' ');
        let (Some(old), Some(new), Some(refname)) = (parts.next(), parts.next(), parts.next())
        else {
            return error_response(StatusCode::BAD_REQUEST, "malformed ref-update command");
        };
        cmds.push((old.to_string(), new.to_string(), refname.to_string()));
    }

    // only branches are pushable (no tags/notes). consume-and-refuse: the pack
    // was fully received; reporting `ng` (not an http error) lets git print a
    // clean per-ref reason.
    if cmds
        .iter()
        .any(|(_, _, r)| !r.starts_with(GIT_HEADS_PREFIX))
    {
        let results: Vec<(String, Option<String>)> = cmds
            .into_iter()
            .map(|(_, _, r)| (r, Some(format!("only {GIT_HEADS_PREFIX}* is supported"))))
            .collect();
        return git_report_status(&results);
    }

    // old/new == the null oid mean "create" (prev_oid None) / "delete" (new_oid
    // None); otherwise 40-hex oids the forge per-branch CAS must match.
    let mut updates = Vec::new();
    for (old, new, refname) in &cmds {
        let prev_oid = if old == GIT_ZERO_OID {
            None
        } else {
            match hex_to_bytes(old).filter(|b| b.len() == GIT_OID_RAW_LEN) {
                Some(bytes) => Some(bytes),
                None => return error_response(StatusCode::BAD_REQUEST, "malformed old oid"),
            }
        };
        let new_oid = if new == GIT_ZERO_OID {
            None
        } else {
            match hex_to_bytes(new).filter(|b| b.len() == GIT_OID_RAW_LEN) {
                Some(bytes) => Some(bytes),
                None => return error_response(StatusCode::BAD_REQUEST, "malformed new oid"),
            }
        };
        updates.push(forge::RefUpdate {
            ref_name: refname[GIT_HEADS_PREFIX.len()..].to_string(),
            prev_oid,
            new_oid,
        });
    }

    // stash the WHOLE packfile as one node-local blob, keyed by its sha256;
    // forge materializes it by this digest (the bytes never cross consensus).
    // a delete-only push carries no objects, so nothing is stashed.
    let pack_digest = if updates.iter().any(|u| u.new_oid.is_some()) {
        Some(handle.blobs.put_chunk(pack.to_vec()).to_vec())
    } else {
        None
    };

    // CAS every branch through ONE atomic PushRefs op and await the block.
    let payload = forge::encode_msg(&forge::ForgeMsg::PushRefs {
        repo,
        updates,
        pack_digest,
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
    let refnames: Vec<String> = cmds.into_iter().map(|(_, _, r)| r).collect();
    match rx.await {
        Ok(Ok(_block)) => {
            let results: Vec<(String, Option<String>)> =
                refnames.into_iter().map(|r| (r, None)).collect();
            git_report_status(&results)
        }
        Ok(Err(reason)) => {
            // a CAS mismatch's rejection carries "non-fast-forward" — surface
            // exactly that token so git prints its standard "fetch first" hint.
            // any other rejection passes through as a single-line reason. the
            // op is atomic, so every ref shares the fate.
            let reason = if reason.contains("non-fast-forward") {
                "non-fast-forward".to_string()
            } else {
                reason.replace('\n', " ")
            };
            let results: Vec<(String, Option<String>)> = refnames
                .into_iter()
                .map(|r| (r, Some(reason.clone())))
                .collect();
            git_report_status(&results)
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

/// Serve only isolated gateway-rendering traffic on the pre-bound loopback
/// listener. The router contains no node API.
pub async fn serve_browser_gateway(
    listener: tokio::net::TcpListener,
    handle: NodeHandle,
) -> std::io::Result<()> {
    let shutdown = handle.clone();
    axum::serve(listener, gateway_browser_router(handle))
        .with_graceful_shutdown(async move { shutdown.shutdown_requested().await })
        .await
}

async fn ws(State(handle): State<NodeHandle>, upgrade: WebSocketUpgrade) -> Response {
    upgrade.on_upgrade(move |socket| stream::stream_session(socket, handle))
}

#[derive(Debug, Deserialize)]
pub struct CallParams {
    channel: String,
}

async fn call_ws(
    State(handle): State<NodeHandle>,
    Query(params): Query<CallParams>,
    upgrade: WebSocketUpgrade,
) -> Response {
    let Some(call) = handle.call.clone() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "calls are not available on this node (no mesh call hub)",
        );
    };
    if params.channel.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "channel must not be empty");
    }
    upgrade.on_upgrade(move |socket| call_session(socket, call, params.channel))
}

/// pump one huddle's audio, camera video, and call control between the webview
/// websocket and the hub session. binary client frames are decoded via
/// `chat::call_wire` and tag-dispatched: audio → `pcm_in`, captured video →
/// `video_in`; the hub's `mixed_out`/`video_out`/`control_out` ends flow back
/// as `chat::call_wire`-encoded binary + json text. text client frames steer
/// fan-out and carry beacons/keyframe asks. either side closing ends the
/// session — dropping the ends is the teardown signal the hub watches.
async fn call_session(mut socket: WebSocket, call: CallLane, channel_id: String) {
    let (reply, opened) = tokio::sync::oneshot::channel();
    let request = CallSessionRequest { channel_id, reply };
    // every refusal path says WHY as a text frame before closing — the client
    // surfaces it as a session error instead of a silent no-op.
    const NO_HUB: &str = "calls are not available on this node (no live call hub)";
    let session = match call.send(request).await {
        Ok(()) => match opened.await {
            Ok(Ok(session)) => session,
            Ok(Err(refusal)) => {
                let _ = socket.send(Message::Text(refusal.into())).await;
                return;
            }
            Err(_) => {
                // hub dropped the reply — shutting down.
                let _ = socket.send(Message::Text(NO_HUB.into())).await;
                return;
            }
        },
        Err(_) => {
            // the request lane is closed: a mode that never runs a hub
            // (parked joiner, sync-only observer) or a dead hub thread.
            let _ = socket.send(Message::Text(NO_HUB.into())).await;
            return;
        }
    };
    let CallSession {
        pcm_in,
        mut mixed_out,
        recipients,
        video_in,
        mut video_out,
        control_in,
        mut control_out,
    } = session;
    loop {
        tokio::select! {
            inbound = socket.recv() => match inbound {
                Some(Ok(Message::Binary(bytes))) => {
                    if let Some(frame) = chat::call_wire::decode_audio(&bytes) {
                        // full lane = the hub is behind; late audio is dead
                        // audio, so drop the frame rather than backpressure.
                        let _ = pcm_in.try_send(frame);
                    } else if let Some(frame) = chat::call_wire::decode_captured(&bytes) {
                        let _ = video_in.try_send(frame);
                    } // unknown/short frame — drop, stay alive
                }
                Some(Ok(Message::Text(text))) => {
                    match serde_json::from_str::<CallClientControl>(&text) {
                        Ok(CallClientControl::Recipients { peers }) => {
                            let keys: Vec<[u8; 32]> = peers
                                .iter()
                                .filter_map(|hex| duckfs_core::from_hex_32(hex))
                                .collect();
                            let _ = recipients.send(keys);
                        }
                        Ok(CallClientControl::Beacon {
                            muted,
                            camera_on,
                            sharing,
                        }) => {
                            let _ = control_in.try_send(CallControlIn::Beacon {
                                muted,
                                camera_on,
                                sharing,
                            });
                        }
                        Ok(CallClientControl::KeyframeRequest { peer }) => {
                            if let Some(key) = duckfs_core::from_hex_32(&peer) {
                                let _ = control_in
                                    .try_send(CallControlIn::KeyframeRequest { peer: key });
                            }
                        }
                        Err(_) => {} // unknown control — ignore, stay alive
                    }
                }
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
            mixed = mixed_out.recv() => match mixed {
                Some(frame) => {
                    let bytes = chat::call_wire::encode_audio(&frame);
                    if socket.send(Message::Binary(bytes.into())).await.is_err() {
                        break;
                    }
                }
                None => break, // the hub ended the session (replaced by a newer join).
            },
            video = video_out.recv() => match video {
                Some(frame) => {
                    let bytes = chat::call_wire::encode_peer(&frame);
                    if socket.send(Message::Binary(bytes.into())).await.is_err() {
                        break;
                    }
                }
                None => break,
            },
            control = control_out.recv() => match control {
                Some(out) => {
                    let message = match out {
                        CallControlOut::KeyframeRequest => CallServerControl::KeyframeRequest,
                        CallControlOut::PeerBeacon { peer, muted, camera_on, sharing } => {
                            CallServerControl::PeerBeacon {
                                peer: hex_bytes(&peer),
                                muted,
                                camera_on,
                                sharing,
                            }
                        }
                        CallControlOut::RateHint { max_kbps } => {
                            CallServerControl::RateHint { max_kbps }
                        }
                    };
                    let text = serde_json::to_string(&message).expect("serializable control");
                    if socket.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                }
                None => break,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// the explorer row round-trips through its stored index encoding — the
    /// one seam both binaries write and `GET /v1/blocks` reads.
    #[test]
    fn block_row_round_trips() {
        let record = BlockRecord {
            height: 7,
            hash: String::new(),
            commit_hash: "aa".repeat(32),
            ops: vec![
                RootOp {
                    proposer: "bb".repeat(32),
                    disposition: BlockDisposition::Applied,
                    target: "directory".into(),
                    operations: Vec::new(),
                    payload: "{}".into(),
                    op_hash: "ee".repeat(32),
                },
                RootOp {
                    proposer: "cc".repeat(32),
                    disposition: BlockDisposition::Rejected,
                    target: "chat".into(),
                    operations: Vec::new(),
                    payload: "{\"m\":1}".into(),
                    op_hash: "ff".repeat(32),
                },
            ],
        };
        let row = block_row(&record);
        let back: BlockRecord = serde_json::from_slice(&row).expect("row is json");
        assert_eq!(back.height, 7);
        assert_eq!(back.hash, "", "frameless lanes keep an honest empty hash");
        assert_eq!(back.ops.len(), 2, "the block aggregated two member ops");
        assert_eq!(back.ops[0].proposer, "bb".repeat(32));
        assert_eq!(back.ops[1].op_hash, "ff".repeat(32));
        // the wire keys stay camelCase — the app reads these fields verbatim.
        let json: serde_json::Value = serde_json::from_slice(&row).unwrap();
        assert!(json.get("commitHash").is_some());
        assert!(json["ops"][0].get("opHash").is_some());
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
        for id in ["chat", "tasks", "inbox", "pages"] {
            assert_eq!(ModuleCategory::of(id), Workspace, "{id}");
        }
        for id in ["forge", "agent", "evm"] {
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
            "saga",
            "identity",
            "duckdns",
            "gateway",
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
