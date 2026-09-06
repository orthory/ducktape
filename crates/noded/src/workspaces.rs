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
use std::path::{Path as FsPath, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use duckfs_client::checkout::{CheckoutOptions, checkout_with};
use duckfs_client::commit::{CommitError, commit};
use duckfs_client::index::DUCKFS_DIR;
use serde::Deserialize;

use crate::actor_api::ActorNodeApi;
use crate::{NodeHandle, error_response};

const MANAGED_WORKSPACE_ROOT: &str = "/shared/workspaces";

/// the sidecar next to `.duckfs/index.json` that records who may commit this
/// checkout — a noded-only fact, not a duckfs one, so it lives beside the
/// index rather than growing [`duckfs_client::index::Index`]'s shared schema
/// for a field only this RPC reads.
const OWNER_FILE: &str = "owner";

fn owner_path(dir: &FsPath) -> PathBuf {
    dir.join(DUCKFS_DIR).join(OWNER_FILE)
}

/// stamp `dir` as created by `origin` — called once, right after the checkout
/// that materializes `.duckfs`. best-effort is not an option here: a checkout
/// with no recorded owner would let the FIRST committer after it silently
/// claim it, so a write failure fails the whole create.
fn write_owner(dir: &FsPath, origin: &[u8]) -> std::io::Result<()> {
    std::fs::write(owner_path(dir), origin)
}

/// who created this workspace, or `None` if no owner was ever recorded (a
/// workspace this build always writes one for — treated as unowned, not
/// legacy-tolerated, so a missing file refuses rather than guesses).
fn read_owner(dir: &FsPath) -> Option<Vec<u8>> {
    std::fs::read(owner_path(dir)).ok()
}

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
    let owner = origin.clone();
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
        // lane, never a stored http base. the owner is stamped right after,
        // so a checkout that fails leaves no owner file behind either.
        let opts = CheckoutOptions::default();
        match checkout_with(&api, &dir, &prefix, snapshot.as_deref(), &opts) {
            Ok(index) => match write_owner(&dir, &owner) {
                Ok(()) => Ok(index),
                Err(e) => Err(e.to_string()),
            },
            Err(e) => Err(e.to_string()),
        }
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
        Ok(Err(err)) => error_response(StatusCode::BAD_REQUEST, &err),
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
    // a lock entry is minted per id below — only mint one for an id that
    // names a real, currently-materialized checkout, or any signed member
    // repeatedly posting fresh random ids grows WORKSPACE_LOCKS forever
    // (delete_workspace, the only pruning path, is Operator-only).
    if !dir.exists() {
        return error_response(StatusCode::NOT_FOUND, "workspace not found");
    }
    // `signed` is `None` when the request presented the OPERATOR credential
    // (the admin-token header) instead of a user signature — the Acting
    // lane's gate has no other way past it with no `SignedBy` at all. But the
    // gate ALSO inserts `SignedBy` for a signature by the operator's OWN key
    // (`operator_key_matches`, the PoP `ducktape node log-filter` etc. use),
    // so "is this the operator" is not just "is `signed` absent" — a signed
    // caller must be either the workspace's creator or the operator, the same
    // pair DELETE already admits.
    if let Some(crate::SignedBy(acting)) = signed.as_deref() {
        let is_owner = read_owner(&dir).is_some_and(|owner| owner == *acting);
        let is_operator = crate::signed_req::operator_key_matches(&handle.admin, acting);
        if !is_owner && !is_operator {
            return error_response(StatusCode::FORBIDDEN, "workspace_not_owner");
        }
    }
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
///
/// AUTH: node-level (`signed_req`, `Authority::Operator`), unlike the
/// create and commit above. It takes no acting identity and `remove_dir_all`s
/// any valid-slug dir under the managed root, so a key that proved only
/// possession would be able to wipe another run's checkout.
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

#[cfg(test)]
mod tests {
    use axum::Json;
    use axum::extract::{Path as AxPath, State};

    use super::*;

    /// a commit against an id nobody ever created must 404 WITHOUT minting a
    /// `WORKSPACE_LOCKS` entry — the only pruning path (`delete_workspace`) is
    /// Operator-only, so a lock entry per unchecked id is unbounded growth any
    /// signed member can trigger.
    #[tokio::test]
    async fn commit_on_a_nonexistent_workspace_404s_and_mints_no_lock() {
        let (handle, _cmd_rx, _hub) = crate::NodeHandle::channel();
        let root = tempfile::tempdir().unwrap();
        let handle = handle.with_duckfs_workspaces(root.path());
        let id = "deadbeefcafef00d0000000000000001".to_string();

        let resp = commit_workspace(
            State(handle),
            None,
            AxPath(id.clone()),
            Json(CommitWsBody {
                message: String::new(),
            }),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        assert!(
            !WORKSPACE_LOCKS
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .contains_key(&id),
            "a nonexistent workspace must never mint a lock entry"
        );
    }

    /// a manually materialized checkout: a real `.duckfs` dir with an owner
    /// stamped, but no `index.json` — enough to exercise the ownership gate
    /// without a full checkout/actor round trip (the gate must run BEFORE the
    /// engine ever reads the index).
    fn stamp_owned_dir(root: &std::path::Path, id: &str, owner: &[u8]) -> std::path::PathBuf {
        let dir = root.join(id);
        std::fs::create_dir_all(dir.join(DUCKFS_DIR)).unwrap();
        write_owner(&dir, owner).unwrap();
        dir
    }

    fn commit_as(
        handle: crate::NodeHandle,
        id: &str,
        signed: Option<Vec<u8>>,
    ) -> impl std::future::Future<Output = Response> {
        commit_workspace(
            State(handle),
            signed.map(|key| axum::Extension(crate::SignedBy(key))),
            AxPath(id.to_string()),
            Json(CommitWsBody {
                message: String::new(),
            }),
        )
    }

    /// run B's key must not commit run A's checkout: ids are not secret (the
    /// managed root is world-readable), so possession of some other key on a
    /// real id must still be refused — the exact hole #1810 found on COMMIT
    /// after DELETE was already closed.
    #[tokio::test]
    async fn a_non_owner_signed_key_is_refused() {
        let (handle, _cmd_rx, _hub) = crate::NodeHandle::channel();
        let root = tempfile::tempdir().unwrap();
        let handle = handle.with_duckfs_workspaces(root.path());
        let id = "aaaa000000000000000000000000000a";
        let owner = b"run-a-key".to_vec();
        stamp_owned_dir(root.path(), id, &owner);

        let resp = commit_as(handle, id, Some(b"run-b-key".to_vec())).await;

        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["error"], "workspace_not_owner");
    }

    /// a workspace with no recorded owner refuses every signed caller — fails
    /// closed rather than treating an unstamped checkout as up for grabs.
    #[tokio::test]
    async fn a_missing_owner_file_refuses_every_signed_caller() {
        let (handle, _cmd_rx, _hub) = crate::NodeHandle::channel();
        let root = tempfile::tempdir().unwrap();
        let handle = handle.with_duckfs_workspaces(root.path());
        let id = "bbbb000000000000000000000000000b";
        std::fs::create_dir_all(root.path().join(id).join(DUCKFS_DIR)).unwrap();

        let resp = commit_as(handle, id, Some(b"anybody".to_vec())).await;

        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    /// the creator's own key clears the ownership gate — whatever it fails on
    /// past that point (no real checkout was ever made here) is NOT 403.
    #[tokio::test]
    async fn the_owner_clears_the_gate() {
        let (handle, _cmd_rx, _hub) = crate::NodeHandle::channel();
        let root = tempfile::tempdir().unwrap();
        let handle = handle.with_duckfs_workspaces(root.path());
        let id = "cccc000000000000000000000000000c";
        let owner = b"run-a-key".to_vec();
        stamp_owned_dir(root.path(), id, &owner);

        let resp = commit_as(handle, id, Some(owner)).await;

        assert_ne!(resp.status(), StatusCode::FORBIDDEN);
    }

    /// the operator credential presents no `SignedBy` at all (the gate bypasses
    /// the signature check for it) — the same bypass DELETE already gives the
    /// operator, so it may commit ANY workspace regardless of who created it.
    #[tokio::test]
    async fn the_operator_credential_bypasses_the_ownership_gate() {
        let (handle, _cmd_rx, _hub) = crate::NodeHandle::channel();
        let root = tempfile::tempdir().unwrap();
        let handle = handle.with_duckfs_workspaces(root.path());
        let id = "dddd000000000000000000000000000d";
        stamp_owned_dir(root.path(), id, b"run-a-key");

        let resp = commit_as(handle, id, None).await;

        assert_ne!(resp.status(), StatusCode::FORBIDDEN);
    }

    /// the operator's OWN key, signed rather than the admin-token header,
    /// also clears the gate on a workspace it did not create: the gate
    /// inserts `SignedBy` for THAT signature too (`operator_key_matches`), so
    /// "is `signed` absent" is not the same test as "is this the operator".
    #[tokio::test]
    async fn an_operator_signed_key_bypasses_the_ownership_gate_too() {
        let operator = commonware_cryptography::ed25519::PrivateKey::from_seed(4242);
        use commonware_cryptography::Signer as _;
        let (handle, _cmd_rx, _hub) = crate::NodeHandle::channel();
        let handle = handle.with_admin(crate::AdminConfig {
            owner_key: Some(operator.public_key().as_ref().to_vec()),
            ..Default::default()
        });
        let root = tempfile::tempdir().unwrap();
        let handle = handle.with_duckfs_workspaces(root.path());
        let id = "eeee000000000000000000000000000e";
        stamp_owned_dir(root.path(), id, b"run-a-key");

        let resp = commit_as(handle, id, Some(operator.public_key().as_ref().to_vec())).await;

        assert_ne!(resp.status(), StatusCode::FORBIDDEN);
    }
}
