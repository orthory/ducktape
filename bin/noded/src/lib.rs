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
//! version (so a newer app can spot a stale orphan), and POST `/v1/admin/shutdown`
//! asks the process to exit gracefully — the managing app has no pid, only
//! this port.

// the owner-gated control namespace (ADR A2/A5): `/v1/admin/*` on the same
// listener, PoP-gated to the node owner. shutdown + module-code moved here off
// the unauthenticated public surface.
pub mod admin;
pub use admin::{AdminConfig, AdminExposure};

pub mod blobs;
pub mod log;
pub mod stream;
pub use stream::{
    ClientMsg, LogRing, RunOutputEvent, RunOutputRegistry, RunStream, ServerFrame, StreamErrorCode,
    StreamHub, StreamOpRow, StreamOrigin, StreamOriginKind, TailItem,
};
// the duckfs product surface lives in its own module; re-exported flat so the
// router keeps its bare handler names and the public param structs stay at
// `noded::CommitBody` &c.
mod files_http;
pub use files_http::*;
// the workspace RPC (`/v1/fs/workspaces`) and its actor-lane `NodeApi` adapter.
// crate-internal: the router registers the handlers and the adapter is used only
// by the workspace handlers — nothing outside the crate touches either.
mod actor_api;
mod workspaces;
// the REAL portable-agent-run provisioner (NodedProvisioner + agent_runs_root,
// the D7 root). public so BOTH node binaries can build one and wire it into
// their DispatchPool constructor.
pub mod agent_provision;
// realtime overlay websocket lanes: huddle and Pages-presence session/control types.
mod call;
pub use call::{
    CallClientControl, CallControlIn, CallControlOut, CallLane, CallParams, CallServerControl,
    CallSession, CallSessionRequest, PageCursor, PresenceClientControl, PresenceControlIn,
    PresenceControlOut, PresenceParams, PresenceServerControl, PresenceSession,
    PresenceSessionRequest, RealtimeSessionRequest,
};
// the gateway lane: signed-route proxying + the isolated browser-gateway
// origin (`gateway_http` because the `gateway` crate is a dependency).
mod gateway_http;
pub mod gateway_ws_token;
pub mod origin_guard;
pub use gateway_http::{
    GatewayFailure, GatewayJob, GatewayLane, GatewayProxyReply, GatewayProxyRequest,
    GatewayResponse, GatewayWsMsg, gateway_browser_router, serve_browser_gateway,
};
// git smart-HTTP: forge as a full push+fetch remote over /forge/{repo}/….
mod git_http;
pub use git_http::InfoRefsParams;
// the node-actor command lane and the router's shared state handle.
mod handle;
pub use handle::{NodeCommand, NodeHandle};

mod module_code;
pub use module_code::{CODE_KIND_MODULE, CodePeerReceipt, CodeStageLane, CodeStageRequest};
// the derived-index tier: store construction, rebuilds, /v1/index/* + /v1/blocks.
mod index;
pub use index::{
    BlocksParams, IndexScanParams, index_block_ops, index_origin, open_index_store,
    rebuild_stale_modules,
};
// the ducktape_* Prometheus series + GET /metrics.
mod metrics;
pub use metrics::NodeMetrics;

use axum::body::Bytes;
use axum::extract::rejection::BytesRejection;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use duckfs_core::CHUNK_SIZE;
use futures::channel::oneshot;
use sdk::StateRoot;
use serde::{Deserialize, Serialize};

use crate::call::{call_ws, presence_ws};
use crate::gateway_http::{gateway_browser_base, gateway_proxy};
use crate::git_http::{GIT_PACK_BODY_LIMIT, git_info_refs, git_receive_pack, git_upload_pack};
use crate::index::{blocks, index_ops, index_scan, index_status, index_view};
use crate::metrics::metrics;

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

/// cap on the payload-preview characters a block record carries.
pub(crate) const PAYLOAD_PREVIEW_MAX: usize = 1024;

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
    /// Node-owned operational state. This is the stable, role-aware facade for
    /// operators; dependency-specific consensus and transport metrics remain
    /// available on `/metrics` for deeper diagnosis.
    #[serde(default)]
    pub operations: OperationalStatus,
}

/// The job this process is currently performing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeRole {
    /// Single-writer embedded daemon; no mesh or consensus participation.
    Local,
    Validator,
    Resident,
    SyncOnly,
    /// Used only until the full node has selected its role during boot.
    #[default]
    Unknown,
}

impl NodeRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Validator => "validator",
            Self::Resident => "resident",
            Self::SyncOnly => "sync_only",
            Self::Unknown => "unknown",
        }
    }
}

/// The role-independent lifecycle phase. A role says what the node is; a
/// phase says what it is doing now.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodePhase {
    #[default]
    Starting,
    Recovering,
    Joining,
    Syncing,
    Validating,
    Serving,
    Draining,
    Halted,
}

impl NodePhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Recovering => "recovering",
            Self::Joining => "joining",
            Self::Syncing => "syncing",
            Self::Validating => "validating",
            Self::Serving => "serving",
            Self::Draining => "draining",
            Self::Halted => "halted",
        }
    }
}

/// Stable operational projection shared by `/v1/status` and the
/// `ducktape_*` metrics. Optional sections are absent when they do not apply to
/// the selected role, rather than being filled with misleading zeroes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationalStatus {
    pub role: NodeRole,
    pub phase: NodePhase,
    /// Unix seconds when `phase` last changed.
    pub phase_since: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_finalized_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consensus: Option<ConsensusOperationalStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync: Option<SyncOperationalStatus>,
    pub storage: StorageOperationalStatus,
    pub upgrade: UpgradeOperationalStatus,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsensusOperationalStatus {
    pub epoch: u64,
    pub view: u64,
    pub validators: u64,
    pub quorum: u64,
    /// Current validators this node can use, including itself when it is a
    /// member. This makes the number directly comparable with `quorum`.
    pub reachable_validators: u64,
    pub pending_ops: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncOperationalStatus {
    /// Source identity is useful in status and logs but intentionally not a
    /// metric label (peer identities are unbounded cardinality).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub target_height: u64,
    pub applied_height: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_progress_at: Option<u64>,
    pub retries: u64,
    pub failures: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageOperationalStatus {
    pub checkpoint_height: u64,
    pub index_poisoned: bool,
    pub indexes: Vec<IndexOperationalStatus>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexOperationalStatus {
    pub module: String,
    pub applied_height: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpgradeOperationalStatus {
    pub current_version: u64,
    /// Highest protocol version this running binary can execute.
    pub max_supported_version: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_version: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activation_height: Option<u64>,
    pub ready_validators: u64,
    pub required_validators: u64,
    /// Present only when this node is a validator in the boundary set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locally_ready: Option<bool>,
    pub armed: bool,
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

/// hex-encode a state root for the wire (stable, greppable, json-friendly).
pub fn hex_root(root: &StateRoot) -> String {
    hex_bytes(&root.0)
}

/// hex-encode arbitrary wire bytes (frame content hashes, proposer keys).
/// a thin wrapper over [`duckfs_core::to_hex`] — one hex implementation; the
/// pub name stays because simnode and bin/node consume it.
pub fn hex_bytes(bytes: &[u8]) -> String {
    duckfs_core::to_hex(bytes)
}

/// the ONE funnel every HTTP rejection flows through — 403 origin-guard, 413 body
/// cap, 409 conflict, 503 no-mesh, every module 400. three lines light up the whole
/// surface.
///
/// 4xx is `debug` ON PURPOSE: `gateway_http.rs`'s duck:// browse fallback proxies
/// UNTRUSTED pages' fetches through this same funnel, so an unconditional `warn!`
/// here is a log-ring DoS any page could drive. Turn it on when you care:
///     curl -XPOST localhost:$PORT/v1/log-filter -d 'info,ducktape::http=debug'
///
/// 5xx is LATCHED for the same reason, and it is not hypothetical: the gateway
/// browse proxy maps a slow/dead publisher to a 502 (`gateway_failure_response`),
/// so a page whose script re-fetches a failing subresource in a loop mints one
/// line per request — enough to evict the whole 4096-line ring. First occurrence,
/// then every 50th, carrying `occurrences`; a real outage is still visible on the
/// first line, and the counter is what says "still broken" rather than "flapped".
///
/// NEVER log the URI: `/.duck/ws/{token}` carries a capability token IN THE PATH,
/// and the ring is streamed to the webview.
pub(crate) fn error_response(status: StatusCode, message: &str) -> Response {
    if status.is_server_error() {
        static SERVER_ERRORS: crate::log::Latch = crate::log::Latch::new(50);
        // keyed by class, not by message: an attacker-supplied path can vary the
        // message, and a per-message key would let them mint an unbounded number of
        // "first occurrences" — re-opening the exact hole this latch closes.
        if let Some(occurrences) = SERVER_ERRORS.hit("server_error") {
            tracing::warn!(
                target: "ducktape::http",
                status = status.as_u16(),
                message,
                occurrences,
                "request failed"
            );
        }
    } else {
        tracing::debug!(target: "ducktape::http", status = status.as_u16(), message, "request refused");
    }
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
    // the PUBLIC (data) surface — query/submit + reads, any account with
    // standing. control ops (shutdown, module-code staging) are NOT here: they
    // live on the owner-gated `/v1/admin/*` namespace merged below (ADR A2).
    let public = Router::new()
        .route("/v1/submit", post(submit))
        // the AUTHENTICATED submit lane: raw signed frame bytes in, the same
        // receipt out. distinct from `/v1/submit` above, whose `origin` is a
        // caller-supplied string.
        .route("/v1/submit/frame", post(submit_frame))
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
        .route("/v1/log-filter", post(log_filter))
        .route("/v1/ws", get(ws))
        .route("/v1/call/ws", get(call_ws))
        .route("/v1/presence/ws", get(presence_ws))
        .route(
            "/v1/gateway/proxy",
            post(gateway_proxy).layer(DefaultBodyLimit::max(
                gateway::MAX_REQUEST_BODY_BYTES as usize * 2 + gateway::MAX_PROXY_HEAD_BYTES,
            )),
        )
        .route("/v1/gateway/browser", get(gateway_browser_base))
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
        // the S3-shaped object facade: one url = one object. PUT is a
        // single-change commit (stage + put), GET streams the whole file,
        // DELETE is a single-change rm; LIST is the existing /v1/files/ls.
        .route(
            "/v1/files/object/{*path}",
            put(object_put)
                .get(object_get)
                .delete(object_delete)
                .layer(DefaultBodyLimit::max(MAX_OBJECT_BYTES)),
        )
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
        ;
    // the owner-gated `/v1/admin/*` namespace — merged only when exposure is
    // enabled, so `Disabled` leaves the control surface simply ABSENT (a 404),
    // not a gated-but-present route. its own PoP middleware is baked in.
    let app = if handle.admin.exposure.enabled() {
        public.merge(admin::admin_router(handle.clone()))
    } else {
        public
    };
    app
        // The public data plane forges account-signed consensus ops, reads all
        // state, writes the filesystem and pushes git. The trusted console is the
        // ONLY web page allowed to reach it: the guard refuses any other browser
        // origin, and the matching CORS allowlist stops a page reading a response
        // even on a request that carries no Origin at all. See `origin_guard`.
        // (The admin namespace inherits this outer origin guard AND its own PoP
        // gate — defense in depth.)
        .layer(axum::middleware::from_fn(origin_guard::guard))
        .layer(origin_guard::cors())
        .with_state(handle)
}

/// the fallback submitter identity when a client sends no `origin`.
pub const DEFAULT_ORIGIN: &str = "noded";

/// the EMBEDDED daemon's executing identity: the origin its dispatch pool
/// claims a run under, so an Accept claim records THIS key as the run's
/// assignee and every op the daemon submits on the run's behalf (the oracle
/// result, the agent session it opens) matches the lease-holder consensus
/// committed. the real node has a keypair for this and signs; the embedded
/// daemon has only its trusted-client origin string, so it must be ONE string
/// — the binary's actor loop, its oracle pool, and the provisioner all name it
/// here rather than each inventing a spelling.
pub const ORACLE_ORIGIN: &[u8] = b"oracle";

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

/// POST /v1/submit/frame — an ALREADY-SIGNED op frame (`application/octet-stream`,
/// the exact bytes [`node::encode_frame`] produced), answered with the same
/// [`SubmitReceipt`] `/v1/submit` returns.
///
/// this lane exists because `/v1/submit`'s `origin` is a caller-supplied STRING:
/// the embedded daemon honours it, `bin/node` throws it away and signs with its
/// own node key, so nothing submitted there can carry authorship consensus is
/// able to check. a frame can — its origin IS its verified signer, bound to
/// `(seq, target, payload)` under `FRAME_NS`, which every honest validator
/// re-verifies identically. that is what lets an agent's ephemeral session key
/// act for itself instead of borrowing the node's identity.
///
/// the signature is verified HERE, before the frame reaches any actor: a frame
/// that does not parse, or whose signature does not bind, is a 400 carrying the
/// codec's verbatim reason and never enters a store or an orderer. the actors
/// verify again where it matters (the validator's `submit_frame` re-checks
/// before it pins) — this is the cheap gate, not the only one.
async fn submit_frame(
    State(handle): State<NodeHandle>,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    let frame = match body {
        Ok(bytes) => bytes,
        Err(rejection) => return error_response(rejection.status(), &rejection.body_text()),
    };
    let payload = match node::decode_frame(&frame) {
        // the origin is DELIBERATELY dropped here: the http layer never tells an
        // actor who signed — the actor re-derives that from the bytes (or, on
        // the validator, `submit_frame` does). one authority on authorship.
        Ok((_origin, msg)) => msg.payload,
        Err(err) => return error_response(StatusCode::BAD_REQUEST, &err.to_string()),
    };
    let (reply, rx) = oneshot::channel();
    if let Err(resp) = handle
        .send(NodeCommand::SubmitFrame {
            frame: frame.to_vec(),
            reply,
        })
        .await
    {
        return resp;
    }
    match rx.await {
        Ok(Ok(block)) => {
            // the receipt's op_hash addresses the op PAYLOAD — the module's own
            // bytes, exactly as the frameless lane stages them. the frame
            // envelope is transport (signature, seq), not content.
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

/// POST /v1/admin/shutdown — ask the process to exit gracefully. lives on the
/// owner-gated admin namespace (ADR A2): it was on the unauthenticated public
/// surface, reachable by anything that could dial the port.
pub(crate) async fn shutdown(State(handle): State<NodeHandle>) -> Response {
    // reply first, then signal — the connection closes before the process does.
    handle.request_shutdown();
    Json(serde_json::json!({ "ok": true })).into_response()
}

/// POST /v1/log-filter — retune the log level of a RUNNING node.
///
///     curl -XPOST localhost:$PORT/v1/log-filter -d 'info,ducktape::join=debug'
///
/// RUST_LOG is read once at boot, so without this route every `debug!` in the
/// tree is unreachable without a restart — and restarting a wedged node destroys
/// the state you restarted it to look at. NOTE: unlike /v1/admin/shutdown this stays on the public surface (see log_filter caveat below).
async fn log_filter(body: String) -> Response {
    match crate::log::set_filter(body.trim()) {
        Ok(()) => (StatusCode::OK, body).into_response(),
        Err(err) => error_response(StatusCode::BAD_REQUEST, &err),
    }
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

/// serve the client surface on `listener` until a shutdown request lands
/// (POST /v1/admin/shutdown). the caller owns the runtime this runs on; the host
/// actor lives elsewhere and is reachable only through `handle`'s command
/// lane — which is what lets ANY binary that owns a host (the embedded daemon,
/// the p2p validator) stand up the identical surface.
pub async fn serve(listener: tokio::net::TcpListener, handle: NodeHandle) -> std::io::Result<()> {
    let shutdown = handle.clone();
    // connect-info is threaded so the admin namespace's guard can read the peer
    // address (the `Loopback` exposure refuses non-loopback peers). every other
    // route ignores it.
    axum::serve(
        listener,
        router(handle).into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async move { shutdown.shutdown_requested().await })
    .await
}

async fn ws(State(handle): State<NodeHandle>, upgrade: WebSocketUpgrade) -> Response {
    upgrade.on_upgrade(move |socket| stream::stream_session(socket, handle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_from_an_older_node_defaults_operational_state() {
        let status: NodeStatus = serde_json::from_value(serde_json::json!({
            "version": "0.1.0",
            "appHash": "",
            "height": 0,
            "modules": []
        }))
        .expect("the additive status field stays backward-compatible");

        assert_eq!(status.operations.role, NodeRole::Unknown);
        assert_eq!(status.operations.phase, NodePhase::Starting);
    }

    #[tokio::test]
    async fn shutdown_wakes_every_surface_and_remains_sticky() {
        let (handle, _commands, _hub) = NodeHandle::channel();
        let first = handle.clone();
        let second = handle.clone();
        let waiters = async move {
            tokio::join!(first.shutdown_requested(), second.shutdown_requested());
        };
        let trigger = async {
            tokio::task::yield_now().await;
            handle.request_shutdown();
        };

        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            tokio::join!(waiters, trigger);
        })
        .await
        .expect("every registered surface wakes");
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            handle.shutdown_requested(),
        )
        .await
        .expect("shutdown remains visible to a late surface");
    }

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
