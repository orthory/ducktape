//! the duckfs lane: materialize / commit / clean a per-run duckfs checkout
//! over the in-daemon actor lane — moved VERBATIM from the pre-split
//! `agent_provision.rs` (behavior-identical; only the source match moved up
//! to the dispatching provisioner).
//!
//! it runs the exact `checkout_with`/`commit` primitives the
//! `/v1/fs/workspaces` RPC does ([`crate::workspaces`]), on `spawn_blocking`
//! (the engine is sync std::fs + `block_on` of the actor — NEVER an
//! axum/tokio worker), and drives them through [`ActorNodeApi`] so there is
//! no self-dial.
//!
//! LIVE, not dormant: this branch de-versioned the ADR's phased rollout
//! (pre-production — no committed history, no mixed-binary set). both binaries
//! wire the files module unconditionally, so the runs composer emits v3 for
//! every agent run and the pool takes the full provision → bind → run →
//! commit → cleanup bracket through this provisioner. the v2/scratch path
//! survives only for embedders that never wire a files module (dev tools,
//! tests).

use std::collections::BTreeMap;
use std::path::PathBuf;

use dispatch_oracle::{
    ProvisionedWorkspace, WorkspaceReceipt, WorkspaceSource, WorkspaceSpec,
};
use duckfs_client::checkout::{CheckoutOptions, checkout_with};
use duckfs_client::commit::{CommitError, commit};

use crate::NodeHandle;
use crate::actor_api::ActorNodeApi;

/// materialize the duckfs source at `dir` and hand back the live workspace.
pub(super) async fn provision(
    handle: NodeHandle,
    dir: PathBuf,
    prefix: String,
    snapshot: Option<String>,
    spec: &WorkspaceSpec,
) -> Result<Box<dyn ProvisionedWorkspace>, String> {
    let api = ActorNodeApi::new(handle.clone());
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
        handle,
        dir,
        source: spec.source.clone(),
        env,
    }))
}

/// one live materialized workspace: its on-disk dir, the source the receipt
/// echoes, and the actor handle its commit rides.
struct NodedWorkspace {
    handle: NodeHandle,
    dir: PathBuf,
    source: WorkspaceSource,
    env: BTreeMap<String, String>,
}

impl NodedWorkspace {
    /// a receipt-only spec: `commit`/`no_changes` read only the source coords,
    /// so the run_id/tools/mount are irrelevant here.
    fn receipt_spec(&self) -> WorkspaceSpec {
        WorkspaceSpec {
            run_id: String::new(),
            agent_id: None,
            source: self.source.clone(),
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
