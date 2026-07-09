//! the REAL [`WorkspaceProvisioner`]: materialize / commit / clean a per-run
//! duckfs workspace over the in-daemon actor lane, for portable (v3) agent
//! runs.
//!
//! this lives in the noded LIB crate — the only place `duckfs-client` (the
//! checkout/commit engine) and [`ActorNodeApi`] (the actor-lane `NodeApi`,
//! `pub(crate)`) are both reachable, the reachability wall dispatch-oracle
//! cannot cross. it runs the exact `checkout_with`/`commit` primitives the
//! `/v1/fs/workspaces` RPC does ([`crate::workspaces`]), on `spawn_blocking`
//! (the engine is sync std::fs + `block_on` of the actor — NEVER an axum/tokio
//! worker), and drives them through [`ActorNodeApi`] so there is no self-dial.
//!
//! D7 (isolation floor): the per-run dir is minted under [`agent_runs_root`],
//! a root VALIDATED at boot to be OUTSIDE `<storage>` — so a `..` from a
//! checkout can NOT reach `user.key`, the node keys, qmdb, the blobstore, or
//! forge's git substrate. the managed `/v1/fs/workspaces` root stays under
//! `<storage>`; this is a distinct, relocated root for live agent runs.
//!
//! LIVE, not dormant: this branch de-versioned the ADR's phased rollout
//! (pre-production — no committed history, no mixed-binary set). both binaries
//! wire the files module unconditionally, so the runs composer emits v3 for
//! every agent run and the pool takes the full provision → bind → run →
//! commit → cleanup bracket through this provisioner. the v2/scratch path
//! survives only for embedders that never wire a files module (dev tools,
//! tests).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use dispatch_oracle::{ProvisionedWorkspace, WorkspaceProvisioner, WorkspaceReceipt, WorkspaceSpec};
use duckfs_client::checkout::{CheckoutOptions, checkout_with};
use duckfs_client::commit::{CommitError, commit};

use crate::NodeHandle;
use crate::actor_api::ActorNodeApi;

/// the D7 relocation lever: the root per-run agent workspaces are minted
/// under. MUST be outside `<storage>` — VALIDATED here at boot, never trusted.
/// `DUCKTAPE_AGENT_RUNS_ROOT` overrides the base (operators point it at an
/// isolated volume); deliberately NOT `DUCKTAPE_AGENT_WORKSPACES`, which
/// already means the legacy persistent per-agent root in `capability-host` —
/// one knob must not govern two unrelated trees. the default is the system
/// temp tree, the same safe scratch tree `CliProvider`'s fallback workdir
/// already uses.
///
/// the returned root is salted with a hash of THIS node's storage path, so
/// co-located nodes (fleet tiles, multi-node test boxes) never share a
/// run-dir tree — one node's W5 cleanup must never be able to delete a
/// sibling process's in-flight checkout.
pub fn agent_runs_root(storage: &Path) -> Result<PathBuf, String> {
    let base = std::env::var_os("DUCKTAPE_AGENT_RUNS_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("ducktape-agent-runs"));
    runs_root_under(base, storage)
}

/// the testable core of [`agent_runs_root`]: salt `base` per-storage, create
/// it, and REFUSE a root inside `<storage>` (D7 is a MUST, not a convention).
fn runs_root_under(base: PathBuf, storage: &Path) -> Result<PathBuf, String> {
    let digest = duckfs_core::objects::object_id(
        duckfs_core::objects::Kind::Chunk,
        storage.to_string_lossy().as_bytes(),
    );
    let salt: String = duckfs_core::to_hex(&digest).chars().take(16).collect();
    let root = base.join(salt);
    std::fs::create_dir_all(&root).map_err(|e| {
        format!(
            "agent runs root {} could not be created: {e}",
            root.display()
        )
    })?;
    // D7 (MUST): the run tree may never live under <storage> — a `..` from a
    // checkout would reach user.key/node keys/qmdb/blobstore. canonicalize
    // both sides so symlinks/relative paths cannot dodge the check.
    let canon_root = root
        .canonicalize()
        .map_err(|e| format!("agent runs root {}: {e}", root.display()))?;
    let canon_storage = storage
        .canonicalize()
        .unwrap_or_else(|_| storage.to_path_buf());
    if canon_root.starts_with(&canon_storage) {
        return Err(format!(
            "agent runs root {} is inside the node storage tree {} — D7 forbids \
             this; set DUCKTAPE_AGENT_RUNS_ROOT to a directory outside it",
            canon_root.display(),
            canon_storage.display()
        ));
    }
    Ok(root)
}

/// a bounded, collision-free `[a-z0-9]` dir name derived from the FULL run_id
/// (`"{saga_id}:{attempt}"`). the SHA-256 tail keys the dir on the ENTIRE
/// run_id — INCLUDING the attempt — so distinct attempts of one saga never
/// share a checkout dir. this matters because a re-lease spawns a NEW attempt
/// WITHOUT cancelling the still-running prior one (agent runs are minutes-long,
/// lease windows shorter), so two attempts can execute concurrently; distinct
/// dirs keep them from interleaving writes / racing commits / cleaning up each
/// other's tree. a readable alnum prefix aids debugging but is NEVER the
/// discriminator, and the id is never trusted as a raw path component (no `.`,
/// no `/`, so a per-run dir can never escape the root).
fn run_slug(run_id: &str) -> String {
    // reuse duckfs's content-address hash (no new dep): a domain-separated
    // sha-256 over the FULL run_id → a stable 24-hex tail keyed on the entire
    // id, attempt included.
    let digest = duckfs_core::objects::object_id(
        duckfs_core::objects::Kind::Chunk,
        run_id.as_bytes(),
    );
    let hash: String = duckfs_core::to_hex(&digest).chars().take(24).collect();
    let prefix: String = run_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .take(24)
        .collect();
    format!("{prefix}{hash}")
}

/// the real provisioner: mints per-run checkouts under `root`, driving the
/// duckfs engine over `handle`'s actor lane.
pub struct NodedProvisioner {
    handle: NodeHandle,
    root: PathBuf,
}

impl NodedProvisioner {
    pub fn new(handle: NodeHandle, root: impl Into<PathBuf>) -> Self {
        Self {
            handle,
            root: root.into(),
        }
    }
}

#[async_trait::async_trait]
impl WorkspaceProvisioner for NodedProvisioner {
    async fn provision(
        &self,
        spec: &WorkspaceSpec,
    ) -> Result<Box<dyn ProvisionedWorkspace>, String> {
        let dir = self.root.join(run_slug(&spec.run_id));
        let api = ActorNodeApi::new(self.handle.clone());
        let prefix = spec.source_prefix.clone();
        let snapshot = spec.source_snapshot.clone();
        let checkout_dir = dir.clone();
        // the engine call is blocking std::fs + block_on(actor) — MUST be
        // spawn_blocking (never an async worker), exactly like
        // workspaces.rs::create_workspace. a managed checkout records no node
        // url (its commits ride the actor lane).
        tokio::task::spawn_blocking(move || {
            checkout_with(
                &api,
                &checkout_dir,
                &prefix,
                snapshot.as_deref(),
                &CheckoutOptions::default(),
            )
            .inspect_err(|_| {
                // a checkout can fail PARTWAY (transport mid-read, verify
                // mismatch) after materializing some of the tree — the run
                // never gets a workspace handle to clean up, so the error
                // path must remove its own debris (W5 applies here too).
                let _ = std::fs::remove_dir_all(&checkout_dir);
            })
        })
        .await
        .map_err(|_| "workspace checkout task panicked".to_string())?
        .map_err(|e| e.to_string())?;
        // this provisioner wires the rw source only: W6 skill/instruction ro
        // mounts are NOT materialized yet. an agent with skills still runs —
        // minus its skill trees — and the gap is LOUD, never silent: the
        // envelope pinned those refs on consensus, so dropping them without a
        // trace would break the composer's contract invisibly.
        if !spec.ro_mounts.is_empty() {
            eprintln!(
                "[oracle] run {} requests {} skill mount(s) this provisioner \
                 does not materialize yet — running without them",
                spec.run_id,
                spec.ro_mounts.len()
            );
        }
        let mut env = BTreeMap::new();
        env.insert("DUCKTAPE_RUN_WORKSPACE".into(), dir.display().to_string());
        Ok(Box::new(NodedWorkspace {
            handle: self.handle.clone(),
            dir,
            source_prefix: spec.source_prefix.clone(),
            source_snapshot: spec.source_snapshot.clone(),
            env,
        }))
    }
}

/// one live materialized workspace: its on-disk dir, the source coords the
/// receipt echoes, and the actor handle its commit rides.
struct NodedWorkspace {
    handle: NodeHandle,
    dir: PathBuf,
    source_prefix: String,
    source_snapshot: Option<String>,
    env: BTreeMap<String, String>,
}

impl NodedWorkspace {
    /// a receipt-only spec: `commit`/`no_changes` read only the source coords,
    /// so the run_id/tools/mount are irrelevant here.
    fn receipt_spec(&self) -> WorkspaceSpec {
        WorkspaceSpec {
            run_id: String::new(),
            agent_id: None,
            source_prefix: self.source_prefix.clone(),
            source_snapshot: self.source_snapshot.clone(),
            mount_path: String::new(),
            base_tools: Vec::new(),
            ro_mounts: Vec::new(),
        }
    }
}

#[async_trait::async_trait]
impl ProvisionedWorkspace for NodedWorkspace {
    fn workdir(&self) -> PathBuf {
        self.dir.clone()
    }

    fn env(&self) -> BTreeMap<String, String> {
        self.env.clone()
    }

    fn path_entries(&self) -> Vec<PathBuf> {
        Vec::new()
    }

    async fn commit(&self, message: &str) -> Result<WorkspaceReceipt, String> {
        let api = ActorNodeApi::new(self.handle.clone());
        let dir = self.dir.clone();
        let message = message.to_string();
        let result = tokio::task::spawn_blocking(move || commit(&api, &dir, &message))
            .await
            .map_err(|_| "workspace commit task panicked".to_string())?;
        let spec = self.receipt_spec();
        match result {
            Ok(summary) => Ok(WorkspaceReceipt::committed(
                &spec,
                summary.snapshot,
                summary.height,
                summary.rebased,
            )),
            // the agent wrote nothing — a clean working copy (R2 empty facet).
            Err(CommitError::Nothing) => Ok(WorkspaceReceipt::no_changes(&spec)),
            // a conflict / rejection / transport failure is loud.
            Err(e) => Err(e.to_string()),
        }
    }

    async fn cleanup(&self) {
        // W5: idempotent, best-effort. an already-gone dir is success; any
        // other error is swallowed — cleanup must never fail the run.
        let dir = self.dir.clone();
        let _ = tokio::task::spawn_blocking(move || match std::fs::remove_dir_all(&dir) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        })
        .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_root_is_salted_per_storage_and_refuses_a_root_inside_storage() {
        // no env mutation (parallel tests share the process): exercise the
        // testable core directly.
        let scratch = std::env::temp_dir().join("ducktape-runs-root-test");
        let storage_a = scratch.join("storage-a");
        let storage_b = scratch.join("storage-b");
        std::fs::create_dir_all(&storage_a).unwrap();
        std::fs::create_dir_all(&storage_b).unwrap();
        let base = scratch.join("runs-base");

        // co-located nodes (distinct storage) get DISJOINT roots — one node's
        // W5 cleanup can never touch a sibling's in-flight checkout.
        let a = runs_root_under(base.clone(), &storage_a).unwrap();
        let b = runs_root_under(base.clone(), &storage_b).unwrap();
        assert_ne!(a, b, "the storage-path salt separates co-located nodes");
        assert!(a.starts_with(&base) && b.starts_with(&base));
        // deterministic per storage: a restart reuses the same root.
        assert_eq!(a, runs_root_under(base.clone(), &storage_a).unwrap());

        // D7 is ENFORCED, not advisory: a base inside <storage> is refused.
        let err = runs_root_under(storage_a.join("agent-runs"), &storage_a).unwrap_err();
        assert!(
            err.contains("D7 forbids") && err.contains("DUCKTAPE_AGENT_RUNS_ROOT"),
            "the refusal names the invariant and the remedy: {err}"
        );
    }

    #[test]
    fn run_slug_is_bounded_alnum_and_collision_free_per_attempt() {
        // pure [a-z0-9], bounded, never empty, no traversal metacharacter survives.
        for id in ["s1:0", "../../etc/passwd", "", "A/B.C-D", &"z".repeat(200)] {
            let s = run_slug(id);
            assert!(!s.is_empty() && s.len() <= 48, "slug {s:?} bounded+non-empty");
            assert!(
                s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()),
                "slug {s:?} is [a-z0-9] — no path-traversal metacharacter"
            );
        }
        // THE bug this guards: distinct attempts of one saga (id differs only in
        // the ":{attempt}" tail) must map to DISTINCT dirs so overlapping
        // attempts never corrupt one checkout.
        let a0 = run_slug("dispatch\u{1f}r\u{1f}deadbeefdeadbeef:0");
        let a1 = run_slug("dispatch\u{1f}r\u{1f}deadbeefdeadbeef:1");
        assert_ne!(a0, a1, "attempt 0 and 1 get distinct dirs");
        // deterministic per run_id (idempotent provision + cleanup).
        assert_eq!(run_slug("saga:2"), run_slug("saga:2"));
    }
}
