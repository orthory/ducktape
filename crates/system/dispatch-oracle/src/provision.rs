//! the host-side provisioning seam: an injected [`WorkspaceProvisioner`] that
//! materializes a per-run duckfs workspace OUTSIDE storage, the plain-data
//! vocabulary the pool passes across the reachability wall, and the SINGLE
//! owner of the host-assembled [`WorkspaceReceipt`] + `RunnerResult` bytes.
//!
//! dispatch-oracle CANNOT depend on duckfs-client (the reachability wall — a
//! kernel/system crate must never touch the OS-side checkout engine), so this
//! module speaks only plain data: the concrete `checkout_with`/`commit` calls
//! live in the node binary's provisioner impl. the pool brackets a portable
//! run with provision → bind → run → commit → assemble → cleanup ONLY when
//! both a v3 plan AND a wired provisioner exist; otherwise the run is
//! byte-identical to today (see [`crate::pool`]). the whole path is dormant
//! pre-flip: the runs composer is still held at v2, so no v3 envelope is ever
//! emitted and [`crate::envelope::prepare`] returns `workspace: None`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use capability_host::RunContext;
use serde::Serialize;

/// the pinned portable plan [`crate::envelope::prepare`] surfaces out of a v3
/// envelope. `Some` only for a v3 run; the pool turns it into a
/// [`WorkspaceSpec`] iff a provisioner is wired, else it is inert (dormant).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortablePlan {
    pub source_prefix: String,
    pub source_snapshot: Option<String>,
    pub mount_path: String,
    pub base_tools: Vec<BaseTool>,
}

/// what the pool hands the provisioner for one run.
#[derive(Debug, Clone)]
pub struct WorkspaceSpec {
    /// `"{saga_id}:{attempt}"` — idempotency key + per-run dir naming.
    pub run_id: String,
    pub agent_id: Option<String>,
    /// the rw source duckfs subtree (envelope `workspace.source_prefix`).
    pub source_prefix: String,
    /// the pinned source snapshot id (W2); `None` = committed head.
    pub source_snapshot: Option<String>,
    /// advisory only — NEVER used as a real host path (W1); the provisioner
    /// mints its own writable mount OUTSIDE storage.
    pub mount_path: String,
    pub base_tools: Vec<BaseTool>,
    /// W6 skill/instruction ro subtrees — EMPTY in phase 2, carried so phase 4
    /// is purely additive.
    pub ro_mounts: Vec<RoMount>,
}

/// one base-tool manifest entry (validated at accept; bindings wired later).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseTool {
    pub name: String,
    pub version: String,
    pub exposure: String,
}

/// a read-only mount the provisioner materializes beside the rw source (W6).
/// carried but never populated in phase 2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoMount {
    pub source_prefix: String,
    pub source_snapshot: Option<String>,
    pub mount_subpath: String,
}

/// the host-assembled receipt embedded in the `RunnerResult`. field-for-field
/// with `runs::WorkspaceReceipt` so the assembled bytes round-trip through
/// `runs::response_text_from_dispatch_bytes` — a rename in either crate must
/// fail the cross-crate wire test, never production.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspaceReceipt {
    pub source_prefix: String,
    pub source_snapshot: Option<String>,
    pub output_snapshot: Option<String>,
    pub commit_height: Option<u64>,
    pub rebased: bool,
    pub no_changes: bool,
}

impl WorkspaceReceipt {
    /// the agent committed a new snapshot (the `output_ref`).
    pub fn committed(spec: &WorkspaceSpec, snapshot: String, height: u64, rebased: bool) -> Self {
        Self {
            source_prefix: spec.source_prefix.clone(),
            source_snapshot: spec.source_snapshot.clone(),
            output_snapshot: Some(snapshot),
            commit_height: Some(height),
            rebased,
            no_changes: false,
        }
    }

    /// the agent wrote nothing — a clean working copy (R2: any facet may be
    /// empty). no `output_ref` is produced.
    pub fn no_changes(spec: &WorkspaceSpec) -> Self {
        Self {
            source_prefix: spec.source_prefix.clone(),
            source_snapshot: spec.source_snapshot.clone(),
            output_snapshot: None,
            commit_height: None,
            rebased: false,
            no_changes: true,
        }
    }
}

/// materialize a per-run workspace (rw source + ro mounts) at a WRITABLE path
/// OUTSIDE `<storage>` (D7), with zero external network (W2). injected by the
/// node binary, where duckfs-client + the actor lane are reachable. an
/// embedder that never wires one keeps today's accept-only behavior.
#[async_trait::async_trait]
pub trait WorkspaceProvisioner: Send + Sync {
    async fn provision(
        &self,
        spec: &WorkspaceSpec,
    ) -> Result<Box<dyn ProvisionedWorkspace>, String>;
}

/// a live materialized workspace the pool binds onto a [`RunContext`],
/// commits, and cleans up.
#[async_trait::async_trait]
pub trait ProvisionedWorkspace: Send + Sync {
    /// the rw mount root → `ctx.workdir_override`.
    fn workdir(&self) -> PathBuf;
    /// run-scoped tool/workspace env vars → additive `ctx.env`.
    fn env(&self) -> BTreeMap<String, String>;
    /// tool bin dirs prepended to `PATH` (populated in phase 4).
    fn path_entries(&self) -> Vec<PathBuf>;
    /// commit ONLY the rw source; a clean working copy → a `no_changes`
    /// receipt (never an error).
    async fn commit(&self, message: &str) -> Result<WorkspaceReceipt, String>;
    /// W5 cleanup: idempotent, best-effort, never fails the run.
    async fn cleanup(&self);
}

/// the shared handle the pool holds — injected like the blob resolver.
pub type SharedProvisioner = Arc<dyn WorkspaceProvisioner>;

/// the ONE place a materialized workspace is bound onto the run context: the
/// mount becomes the child's cwd, its env is layered additively, and its tool
/// bin dirs feed `PATH`.
pub fn bind_workspace(ws: &dyn ProvisionedWorkspace, ctx: &mut RunContext) {
    ctx.workdir_override = Some(ws.workdir());
    ctx.env.extend(ws.env());
    ctx.path_entries = ws.path_entries();
}

/// the winning attempt's delivered bytes for a portable run: the model prose
/// plus the host-assembled receipt under marker `ducktape_runner_result` (R1,
/// host-assembled). the version is [`crate::envelope::RUNNER_RESULT_VERSION`]
/// — the SINGLE owner, never a second const; `runs` reads it back as
/// `u32 == 1` and unwraps `response_text` deterministically on every node.
pub fn assemble_runner_result(response_text: &str, receipt: &WorkspaceReceipt) -> Vec<u8> {
    serde_json::json!({
        "ducktape_runner_result": crate::envelope::RUNNER_RESULT_VERSION,
        "response_text": response_text,
        "workspace_receipt": receipt,
    })
    .to_string()
    .into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> WorkspaceSpec {
        WorkspaceSpec {
            run_id: "s1:0".into(),
            agent_id: Some("bot".into()),
            source_prefix: "/shared/agent-workspaces/bot".into(),
            source_snapshot: Some("aa".repeat(32)),
            mount_path: "/tmp/ducktape-workspace".into(),
            base_tools: vec![BaseTool {
                name: "ducktape-files".into(),
                version: "1".into(),
                exposure: "cli".into(),
            }],
            ro_mounts: Vec::new(),
        }
    }

    #[test]
    fn committed_receipt_carries_the_output_ref() {
        let r = WorkspaceReceipt::committed(&spec(), "cc".repeat(32), 9, false);
        assert_eq!(r.output_snapshot.as_deref(), Some("cc".repeat(32).as_str()));
        assert_eq!(r.commit_height, Some(9));
        assert!(!r.no_changes);
        assert_eq!(r.source_prefix, "/shared/agent-workspaces/bot");
        assert_eq!(r.source_snapshot.as_deref(), Some("aa".repeat(32).as_str()));
    }

    #[test]
    fn no_changes_receipt_has_no_output_ref() {
        let r = WorkspaceReceipt::no_changes(&spec());
        assert!(r.no_changes);
        assert_eq!(r.output_snapshot, None);
        assert_eq!(r.commit_height, None);
    }

    #[test]
    fn assembled_runner_result_carries_marker_text_and_receipt() {
        let r = WorkspaceReceipt::committed(&spec(), "cc".repeat(32), 9, true);
        let bytes = assemble_runner_result("the answer", &r);
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["ducktape_runner_result"], 1);
        assert_eq!(v["response_text"], "the answer");
        assert_eq!(v["workspace_receipt"]["output_snapshot"], "cc".repeat(32));
        assert_eq!(v["workspace_receipt"]["rebased"], true);
        assert_eq!(v["workspace_receipt"]["no_changes"], false);
    }
}
