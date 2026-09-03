//! the workspace RPC — the jobs/sandbox seam: daemon-managed duckfs checkouts
//! under an injected root.
//!
//! greenfield in phase 3 (no module consumes it yet): a caller creates a managed
//! checkout, edits the files ON DISK at the returned path, then commits or
//! deletes over http. state lives entirely on disk (`<root>/<id>/.duckfs`), so
//! the daemon holds no in-memory registry a restart would lose — only a
//! per-workspace lock map so two concurrent commits on ONE workspace can't
//! interleave scans. every engine call is disk I/O plus actor round-trips, so
//! the handlers offload to `spawn_blocking` (never blocking an axum worker) and
//! drive the engine through [`ActorNodeApi`] over the actor lane — no self-dial.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use duckfs_client::checkout::{CheckoutOptions, checkout_with};
use duckfs_client::commit::{CommitError, commit};
use serde::Deserialize;

use crate::actor_api::ActorNodeApi;
use crate::{NodeHandle, error_response};

const MANAGED_WORKSPACE_ROOT: &str = "/shared/workspaces";

/// per-workspace commit serialization. keyed by id, each value a mutex two
/// commits on the same workspace contend on; disjoint workspaces never wait.
/// state is on disk, so this map is the ONLY in-memory workspace state — a
/// restart drops it (and no commit is mid-flight across a restart anyway).
static WORKSPACE_LOCKS: LazyLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn workspace_lock(id: &str) -> Arc<Mutex<()>> {
    let mut map = WORKSPACE_LOCKS.lock().unwrap_or_else(|e| e.into_inner());
    map.entry(id.to_string())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

/// mint a fresh 16-hex workspace slug. monotonic within a process (an atomic
/// counter) and time-seeded across restarts — unique without pulling in a
/// randomness dep, and confined to `[0-9a-f]` so the id is never a traversal.
fn new_slug() -> String {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let mixed = nanos.rotate_left(17) ^ seq.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    format!("{mixed:016x}")
}

/// a slug is safe iff it is a bounded `[a-z0-9]` string — no `.`, no `/`, so a
/// path param can never escape the workspace root.
fn valid_slug(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

fn managed_prefix(id: &str, suffix: &[String]) -> String {
    let mut prefix = format!("{MANAGED_WORKSPACE_ROOT}/{id}");
    for seg in suffix {
        prefix.push('/');
        prefix.push_str(seg);
    }
    prefix
}

/// resolve the caller's workspace vocabulary into the duckfs namespace recorded
/// in `.duckfs/index.json`. `/workspace` is intentionally local to this RPC: it
/// maps to an id-scoped managed prefix, so a job can edit its returned disk dir
/// without knowing the module's writable roots.
fn checkout_prefix(id: &str, requested: &str) -> Result<String, String> {
    let trimmed = requested.trim().trim_end_matches('/');
    if trimmed.is_empty() || trimmed == "/" {
        return Ok(managed_prefix(id, &[]));
    }

    let absolute = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    };
    let segs = duckfs_core::paths::canonical(&absolute)
        .map_err(|e| format!("duckfs workspace prefix is invalid: {e}"))?;
    match segs.first().map(String::as_str) {
        Some("workspace") => Ok(managed_prefix(id, &segs[1..])),
        _ => Err("duckfs workspace prefix must be /workspace".to_string()),
    }
}

/// the 503 a node without a configured workspace root answers with (the router
/// tests' fake handle, and any daemon that never injected the root).
fn unconfigured() -> Response {
    error_response(
        StatusCode::SERVICE_UNAVAILABLE,
        "duckfs workspaces are not configured on this node",
    )
}

/// the POST /v1/fs/workspaces body.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateBody {
    /// the duckfs subtree to check out. omitted/empty/`/workspace` means the
    /// daemon chooses an id-scoped managed namespace for this local workspace.
    #[serde(default)]
    pub prefix: Option<String>,
    /// an explicit snapshot to check out at; omitted/`null` = the committed head
    /// (`None` head = an empty checkout).
    #[serde(default)]
    pub snapshot: Option<String>,
}

/// POST /v1/fs/workspaces — materialize a managed checkout under the injected
/// root, returning `{id, path, snapshot}`. the `path` is where the caller edits
/// files before committing over the id.
pub(crate) async fn create_workspace(
    State(handle): State<NodeHandle>,
    signed: Option<axum::Extension<crate::SignedBy>>,
    Json(body): Json<CreateBody>,
) -> Response {
    let origin = crate::signed_req::acting_origin(signed.as_deref());
    let Some(root) = handle.duckfs_workspaces.clone() else {
        return unconfigured();
    };
    let id = new_slug();
    let dir = root.join(&id);
    let path_str = dir.display().to_string();
    let api = ActorNodeApi::new(handle.clone(), origin);
    let prefix = match checkout_prefix(&id, body.prefix.as_deref().unwrap_or("/workspace")) {
        Ok(prefix) => prefix,
        Err(err) => return error_response(StatusCode::BAD_REQUEST, &err),
    };
    let snapshot = body.snapshot;
    let result = tokio::task::spawn_blocking(move || {
        // a managed checkout records no node url — its commits ride the actor
        // lane, never a stored http base.
        let opts = CheckoutOptions::default();
        checkout_with(&api, &dir, &prefix, snapshot.as_deref(), &opts)
    })
    .await;
    match result {
        Ok(Ok(index)) => Json(serde_json::json!({
            "id": id,
            "path": path_str,
            "prefix": index.prefix,
            "snapshot": index.base_snapshot,
        }))
        .into_response(),
        Ok(Err(err)) => error_response(StatusCode::BAD_REQUEST, &err.to_string()),
        Err(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "workspace checkout task panicked",
        ),
    }
}

/// the POST /v1/fs/workspaces/{id}/commit body.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitWsBody {
    #[serde(default)]
    pub message: String,
}

/// POST /v1/fs/workspaces/{id}/commit — commit the workspace's on-disk edits.
/// success is `{snapshot, height, rebased}`; a structured conflict is a **409**
/// carrying the serialized `ConflictReport` (clashing paths and a remedy).
pub(crate) async fn commit_workspace(
    State(handle): State<NodeHandle>,
    signed: Option<axum::Extension<crate::SignedBy>>,
    Path(id): Path<String>,
    Json(body): Json<CommitWsBody>,
) -> Response {
    let origin = crate::signed_req::acting_origin(signed.as_deref());
    let Some(root) = handle.duckfs_workspaces.clone() else {
        return unconfigured();
    };
    if !valid_slug(&id) {
        return error_response(StatusCode::BAD_REQUEST, "invalid workspace id");
    }
    let dir = root.join(&id);
    let api = ActorNodeApi::new(handle.clone(), origin);
    let lock = workspace_lock(&id);
    let message = body.message;
    let result = tokio::task::spawn_blocking(move || {
        // hold the per-workspace lock across the whole scan+commit so two
        // concurrent commits on ONE workspace never interleave.
        let _guard = lock.lock().unwrap_or_else(|e| e.into_inner());
        commit(&api, &dir, &message)
    })
    .await;
    match result {
        Ok(Ok(summary)) => Json(serde_json::json!({
            "snapshot": summary.snapshot,
            "height": summary.height,
            "rebased": summary.rebased,
        }))
        .into_response(),
        Ok(Err(CommitError::Conflict(report))) => {
            (StatusCode::CONFLICT, Json(*report)).into_response()
        }
        Ok(Err(err)) => error_response(StatusCode::BAD_REQUEST, &err.to_string()),
        Err(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "workspace commit task panicked",
        ),
    }
}

/// DELETE /v1/fs/workspaces/{id} — remove the managed checkout dir. idempotent:
/// deleting an already-gone workspace is still `{ok:true}`.
pub(crate) async fn delete_workspace(
    State(handle): State<NodeHandle>,
    Path(id): Path<String>,
) -> Response {
    let Some(root) = handle.duckfs_workspaces.clone() else {
        return unconfigured();
    };
    if !valid_slug(&id) {
        return error_response(StatusCode::BAD_REQUEST, "invalid workspace id");
    }
    let dir = root.join(&id);
    let result = tokio::task::spawn_blocking(move || match std::fs::remove_dir_all(&dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    })
    .await;
    match result {
        Ok(Ok(())) => {
            // drop the lock-map entry too, so the map never grows without bound.
            WORKSPACE_LOCKS
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&id);
            Json(serde_json::json!({ "ok": true })).into_response()
        }
        Ok(Err(err)) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &err),
        Err(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "workspace delete task panicked",
        ),
    }
}
