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

/// materialize the duckfs source at `dir` (plus W6 skill ro mounts under the
/// sibling `ro_root`) and hand back the live workspace. mount names arrive
/// PRE-VALIDATED by the dispatching provisioner (`mount_dir_name` + dedup).
/// `node_url` is this node's http base (`None` = no http surface), handed to
/// the run as `DUCKTAPE_NODE` so its tool plane can dial back.
#[allow(clippy::too_many_arguments)]
pub(super) async fn provision(
    handle: NodeHandle,
    dir: PathBuf,
    ro_root: PathBuf,
    prefix: String,
    snapshot: Option<String>,
    node_url: Option<String>,
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
    // W6 skill ro mounts land at a SUFFIXED SIBLING of the rw checkout
    // root (`<slug>-ro/<name>`): `commit` scans only under `dir`, so a
    // skill tree beside it can never leak into the output snapshot.
    let ro_dir = if spec.ro_mounts.is_empty() {
        None
    } else {
        let mount_handle = handle.clone();
        let mounts = spec.ro_mounts.clone();
        let checkout_ro = ro_root.clone();
        let checkout_rw = dir.clone();
        tokio::task::spawn_blocking(move || {
            super::checkout_ro_mounts(&mount_handle, &checkout_ro, &mounts).inspect_err(|_| {
                // W5 again: the run never gets a workspace handle on a
                // failed provision, so the already-materialized rw checkout
                // goes too (the mount helper removed its own partial tree).
                let _ = std::fs::remove_dir_all(&checkout_rw);
            })
        })
        .await
        .map_err(|_| "skill mount checkout task panicked".to_string())??;
        Some(ro_root)
    };
    // the workspace EXISTS now, so ask consensus to bind the run's agent session
    // — never before: a bind for a run that failed to materialize would spend an
    // op on a run that never starts.
    let session = super::session::open(&handle, spec).await;
    let env = super::run_env(
        &dir,
        ro_dir.as_deref(),
        node_url.as_deref(),
        spec,
        session.as_ref(),
    );
    Ok(Box::new(NodedWorkspace {
        handle,
        dir,
        ro_dir,
        source: spec.source.clone(),
        env,
    }))
}

/// one live materialized workspace: its on-disk dir, the source the receipt
/// echoes, and the actor handle its commit rides.
struct NodedWorkspace {
    handle: NodeHandle,
    dir: PathBuf,
    /// the W6 skill ro root (`<slug>-ro`), `Some` iff the run had mounts —
    /// tracked ONLY so cleanup can remove it; commit never looks at it.
    ro_dir: Option<PathBuf>,
    source: WorkspaceSource,
    env: BTreeMap<String, String>,
}

impl NodedWorkspace {
    /// a receipt-only spec: `commit`/`no_changes` read only the source coords,
    /// so the run ids/tools/mount are irrelevant here.
    fn receipt_spec(&self) -> WorkspaceSpec {
        WorkspaceSpec {
            run_id: String::new(),
            consensus_run_id: None,
            agent_id: None,
            agent_display_name: None,
            source: self.source.clone(),
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
        super::tool_path_entries()
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
        // other error is swallowed — cleanup must never fail the run. the
        // skill ro root is the run's debris too.
        let dir = self.dir.clone();
        let ro_dir = self.ro_dir.clone();
        let _ = tokio::task::spawn_blocking(move || {
            let _ = std::fs::remove_dir_all(&dir);
            if let Some(ro) = &ro_dir {
                let _ = std::fs::remove_dir_all(ro);
            }
        })
        .await;
    }
}
