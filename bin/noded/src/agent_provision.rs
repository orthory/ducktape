//! the REAL [`WorkspaceProvisioner`] for portable (v3) agent runs: one
//! per-run workspace under a D7-validated root, materialized from whichever
//! source the run's envelope pinned.
//!
//! two lanes, dispatched on [`WorkspaceSource`]:
//! - **duckfs** ([`duckfs`]): checkout / commit a duckfs subtree over the
//!   in-daemon actor lane — the original lane, moved verbatim.
//! - **forge** ([`forge`]): a git WORKTREE of a node-local forge repo at the
//!   run's pinned commit, committed with agent authorship and pushed back
//!   through this node's own loopback smart-HTTP lane so the branch move
//!   settles through consensus (wire contract §4). configured per binary via
//!   [`NodedProvisioner::with_forge`]; unconfigured or unusable (no repo
//!   base, no http surface, no worktree-capable host `git`) the lane fails
//!   each forge attempt LOUDLY while duckfs runs are untouched.
//!
//! this lives in the noded LIB crate — the only place `duckfs-client` (the
//! checkout/commit engine), the actor-lane `NodeApi`, and the node handle's
//! forge repo base are all reachable, the reachability wall dispatch-oracle
//! cannot cross.
//!
//! D7 (isolation floor): the per-run dir is minted under [`agent_runs_root`],
//! a root VALIDATED at boot to be OUTSIDE `<storage>` — so a `..` from a
//! checkout can NOT reach `user.key`, the node keys, qmdb, the blobstore, or
//! forge's git substrate. the managed `/v1/fs/workspaces` root stays under
//! `<storage>`; this is a distinct, relocated root for live agent runs.

use std::path::{Path, PathBuf};

use dispatch_oracle::{
    ProvisionedWorkspace, WorkspaceProvisioner, WorkspaceSource, WorkspaceSpec,
};

use crate::NodeHandle;

mod duckfs;
mod forge;

pub use forge::forge_push_base;

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

/// the real provisioner: mints per-run workspaces under `root`, driving the
/// duckfs engine over `handle`'s actor lane and (when [`Self::with_forge`]
/// configured a usable lane) the forge worktree engine over host `git`.
pub struct NodedProvisioner {
    handle: NodeHandle,
    root: PathBuf,
    /// the forge lane: `Ok` when this node can provision forge worktrees,
    /// `Err(reason)` — decided ONCE at construction, permanent and loud —
    /// when it can't. the duckfs lane is unaffected either way.
    forge: Result<forge::ForgeLane, String>,
}

impl NodedProvisioner {
    pub fn new(handle: NodeHandle, root: impl Into<PathBuf>) -> Self {
        Self {
            handle,
            root: root.into(),
            forge: Err("this provisioner was built without a forge lane \
                        (with_forge was never called)"
                .into()),
        }
    }

    /// configure the forge worktree lane: `push_base` is the loopback
    /// smart-HTTP base URL ([`forge_push_base`] derives it from the node's
    /// http listen address; `None` = this node serves no http surface) and
    /// `committer_name` is this node's stable identity — the COMMITTER on
    /// every run commit (D2: author is the agent, committer is the node).
    /// the repo base is read off the handle's forge repo (the same base the
    /// forge module materializes into). host `git` is probed ONCE here —
    /// a probe failure makes the lane permanently unavailable, loudly.
    pub fn with_forge(
        self,
        push_base: Option<String>,
        committer_name: impl Into<String>,
    ) -> Self {
        self.with_forge_probed(push_base, committer_name, forge::probe_host_git)
    }

    /// [`Self::with_forge`] with the construction-time probe injected — the
    /// seam that lets tests exercise a probe failure without uninstalling git.
    fn with_forge_probed(
        mut self,
        push_base: Option<String>,
        committer_name: impl Into<String>,
        probe: impl FnOnce() -> Result<(), String>,
    ) -> Self {
        self.forge =
            forge::ForgeLane::configure(&self.handle, push_base, committer_name.into(), probe);
        if let Err(reason) = &self.forge {
            eprintln!("[oracle] forge workspace provisioning unavailable on this node: {reason}");
        }
        self
    }
}

#[async_trait::async_trait]
impl WorkspaceProvisioner for NodedProvisioner {
    async fn provision(
        &self,
        spec: &WorkspaceSpec,
    ) -> Result<Box<dyn ProvisionedWorkspace>, String> {
        let run_dir = self.root.join(run_slug(&spec.run_id));
        match &spec.source {
            WorkspaceSource::Duckfs {
                source_prefix,
                source_snapshot,
            } => {
                duckfs::provision(
                    self.handle.clone(),
                    run_dir,
                    source_prefix.clone(),
                    source_snapshot.clone(),
                    spec,
                )
                .await
            }
            WorkspaceSource::Forge { repo, .. } => match &self.forge {
                Ok(lane) => forge::provision(lane, run_dir, spec).await,
                // a loud attempt failure BEFORE any on-disk debris — the saga
                // settles the attempt (liveness is its job, not ours).
                Err(reason) => Err(format!(
                    "forge workspace provisioning for repo {repo:?} is unavailable on this \
                     node: {reason}"
                )),
            },
        }
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
