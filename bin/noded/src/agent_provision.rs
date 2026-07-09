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
//! a root OUTSIDE `<storage>` and OUTSIDE `~/.ducktape` — so a `..` from a
//! checkout can NOT reach `user.key`, the node keys, qmdb, the blobstore, or
//! forge's git substrate. the managed `/v1/fs/workspaces` root stays under
//! `<storage>`; this is a distinct, relocated root for live agent runs.
//!
//! DORMANT in phase 2: the runs composer is held at v2, so no v3 envelope is
//! composed and the pool never reaches this provisioner on a live run. it is
//! wired now so the coordinated flip (Phase 5) activates it without another
//! deploy.

use std::collections::BTreeMap;
use std::path::PathBuf;

use dispatch_oracle::{ProvisionedWorkspace, WorkspaceProvisioner, WorkspaceReceipt, WorkspaceSpec};
use duckfs_client::checkout::{CheckoutOptions, checkout_with};
use duckfs_client::commit::{CommitError, commit};

use crate::NodeHandle;
use crate::actor_api::ActorNodeApi;

/// the D7 relocation lever: the root per-run agent workspaces are minted under.
/// MUST be outside `<storage>` and outside `~/.ducktape` (see the module doc).
/// `DUCKTAPE_AGENT_WORKSPACES` overrides it (operators point it at an isolated
/// volume); the default is the system temp tree, the same safe scratch tree
/// `CliProvider`'s fallback workdir already uses.
pub fn agent_runs_root() -> PathBuf {
    std::env::var_os("DUCKTAPE_AGENT_WORKSPACES")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("ducktape-agent-runs"))
}

/// a slug is a bounded `[a-z0-9]` string — a saga_id is NEVER trusted as a raw
/// path component (no `.`, no `/`, so a per-run dir can never escape the root).
fn run_slug(run_id: &str) -> String {
    let mut s: String = run_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .take(48)
        .collect();
    if s.is_empty() {
        s.push('r');
    }
    s
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
        })
        .await
        .map_err(|_| "workspace checkout task panicked".to_string())?
        .map_err(|e| e.to_string())?;
        // phase 2 wires the rw source only; spec.ro_mounts is empty (W6 skill
        // trees = phase 4).
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
    fn agent_runs_root_honors_the_env_override() {
        // no env mutation (parallel tests share the process): assert the
        // default shape only.
        let root = agent_runs_root();
        assert!(
            root.ends_with("ducktape-agent-runs")
                || std::env::var_os("DUCKTAPE_AGENT_WORKSPACES").is_some(),
            "default root lives under the temp tree, outside <storage>"
        );
    }

    #[test]
    fn run_slug_is_a_bounded_lowercase_alnum_path_component() {
        assert_eq!(run_slug("s1:0"), "s10");
        assert_eq!(run_slug("../../etc/passwd"), "etcpasswd");
        assert_eq!(run_slug(""), "r");
        assert_eq!(run_slug("A/B.C-D"), "abcd");
        let long = run_slug(&"z".repeat(200));
        assert_eq!(long.len(), 48, "slug is length-bounded");
        assert!(
            run_slug("saga:99").chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()),
            "no path-traversal metacharacters survive"
        );
    }
}
