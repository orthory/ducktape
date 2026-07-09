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
//! byte-identical to the legacy path (see [`crate::pool`]). this path is LIVE
//! in both node binaries: they wire the files module unconditionally, so the
//! runs composer emits v3 for every agent run (the de-versioned activation —
//! no flag day, pre-production re-genesis). only embedders without a files
//! module (dev tools, tests) still compose v2.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use capability_host::RunContext;
use serde::Serialize;

/// the pinned portable plan [`crate::envelope::prepare`] surfaces out of a v3
/// envelope. `Some` only for a v3 run; the pool turns it into a
/// [`WorkspaceSpec`] iff a provisioner is wired, else it is inert (dormant).
///
/// carries NO `mount_path`: the phase-5 composer emits SOURCE coordinates only
/// (D7), and the provisioner mints its own writable host cwd. `skills` are the
/// C4 read-only mounts, surfaced straight into [`WorkspaceSpec::ro_mounts`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortablePlan {
    pub source_prefix: String,
    pub source_snapshot: Option<String>,
    pub base_tools: Vec<BaseTool>,
    pub skills: Vec<RoMount>,
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
/// `runs::decode_run_result_v1` — a rename in either crate must fail the
/// cross-crate wire test, never production.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspaceReceipt {
    pub source_prefix: String,
    pub source_snapshot: Option<String>,
    pub output_snapshot: Option<String>,
    pub commit_height: Option<u64>,
    pub rebased: bool,
    pub no_changes: bool,
    /// `Some` iff the commit MECHANISM failed (conflict, transport, rejection)
    /// — the agent's writes were NOT captured and this is not a clean tree.
    /// distinct from `no_changes` (the agent genuinely wrote nothing) so a
    /// failed capture can never masquerade as one; skip-serialized so the
    /// healthy wire shape is unchanged.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_error: Option<String>,
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
            commit_error: None,
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
            commit_error: None,
        }
    }

    /// the commit MECHANISM failed — the agent's writes were not captured. the
    /// truth is "capture failed", never "clean tree": `no_changes` stays false
    /// and the error rides the receipt into the audit lane (I4), while the
    /// run's answer still delivers (R4) under a `Degraded` status.
    pub fn commit_failed(spec: &WorkspaceSpec, error: String) -> Self {
        Self {
            source_prefix: spec.source_prefix.clone(),
            source_snapshot: spec.source_snapshot.clone(),
            output_snapshot: None,
            commit_height: None,
            rebased: false,
            no_changes: false,
            commit_error: Some(error),
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

// ---- faceted receipt (v1 wire) ------------------------------------------------
// the receipt grew from message-only to the six ADR facets — message
// (`response_text`) / data / effects / artifact (`workspace_receipt`) / sink /
// status. the extra five are ADDITIVE and skip-serialized when empty/default, so
// a plain run still emits the minimal `{ducktape_runner_result, response_text,
// workspace_receipt}` shape that `runs` decodes as a message-only result. the
// single `runs` delivery path applies whatever facets are present. the
// marker/version stay `ducktape_runner_result` / 1.

/// the wire name a [`RunEffect`] carries for a task create.
const EFFECT_TASKS_CREATE: &str = "tasks.create";
/// the wire name a [`RunEffect`] carries for a task status move.
const EFFECT_TASKS_UPDATE_STATUS: &str = "tasks.update_status";

/// one declarative, host-assembled effect the model's answer requested. lifted
/// out of `response_text` by [`effects_from_response_text`] so at v4 runs applies
/// the HOST's observation (R1) rather than re-parsing prose. idempotent by
/// run_id (applied once at the delivery boundary).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunEffect {
    pub kind: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub task_id: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub status: String,
}

/// the O1/O2 output sink: `Chain` (default — the next run reads this run's
/// output_ref) / `Pr` (open a forge PR) / `Merge` (merge a forge PR). the
/// concrete routing is runs' concern; the wrapper only names the intent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum Sink {
    #[default]
    Chain,
    Pr {
        repo: String,
        source_branch: String,
        target_branch: String,
        title: String,
        body: String,
    },
    Merge {
        repo: String,
        number: u64,
        prev_target_oid: String,
        expected_source_oid: String,
        merge_oid: String,
        pack_digest: String,
    },
}

impl Sink {
    fn is_chain(&self) -> bool {
        matches!(self, Sink::Chain)
    }
}

/// the host's observation of the run's terminal state. `Ok` is the default; a
/// `Failed` status makes runs fail the run even with a present message facet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    #[default]
    Ok,
    Degraded,
    Failed,
}

impl Status {
    fn is_ok(&self) -> bool {
        matches!(self, Status::Ok)
    }
}

/// the serialized receipt shape. the three core fields lead (unchanged
/// positions); the five facets follow and skip-serialize when empty/default.
#[derive(Serialize)]
struct RunnerResultWire<'a> {
    ducktape_runner_result: u64,
    response_text: &'a str,
    workspace_receipt: &'a WorkspaceReceipt,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    effects: Vec<RunEffect>,
    #[serde(skip_serializing_if = "Sink::is_chain")]
    sink: Sink,
    #[serde(skip_serializing_if = "Status::is_ok")]
    status: Status,
}

/// the winning attempt's delivered bytes for a portable run: the model prose,
/// the host-assembled receipt, and the four other facets, under marker
/// `ducktape_runner_result` (R1, host-assembled). the version is
/// [`crate::envelope::RUNNER_RESULT_VERSION`] — the SINGLE owner, never a second
/// const; `runs` reads it back as `u32 == 1` and unwraps `response_text`
/// deterministically on every node. empty/default facets skip-serialize, so a
/// plain run's bytes stay byte-compatible with the pre-v4 minimal shape.
pub fn assemble_runner_result(
    response_text: &str,
    receipt: &WorkspaceReceipt,
    data: Option<String>,
    effects: Vec<RunEffect>,
    sink: Sink,
    status: Status,
) -> Vec<u8> {
    serde_json::to_vec(&RunnerResultWire {
        ducktape_runner_result: crate::envelope::RUNNER_RESULT_VERSION,
        response_text,
        workspace_receipt: receipt,
        data,
        effects,
        sink,
        status,
    })
    .expect("runner result serializes")
}

/// LIFT the model's `tasks.create` / `tasks.update_status` actions out of its
/// strict-response prose into declarative [`RunEffect`]s (R1, activation
/// correctness — critic #4). at v4 runs applies these host-assembled effects
/// rather than re-parsing the prose; an empty result lets runs fall back to the
/// response-parsed actions so nothing is silently dropped. the parse mirrors
/// `runs::parse_strict_response`: bare JSON, then a `` ```json `` fence, then the
/// outermost `{…}` span. the action shape is `AgentAction`'s snake_case tags.
pub fn effects_from_response_text(text: &str) -> Vec<RunEffect> {
    let Some(value) = parse_response_value(text) else {
        return Vec::new();
    };
    let Some(actions) = value.get("actions").and_then(|a| a.as_array()) else {
        return Vec::new();
    };
    actions
        .iter()
        .filter_map(|action| {
            let obj = action.as_object()?;
            let str_field = |m: &serde_json::Map<String, serde_json::Value>, k: &str| {
                m.get(k)
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string()
            };
            if let Some(create) = obj.get("create_task").and_then(|v| v.as_object()) {
                return Some(RunEffect {
                    kind: EFFECT_TASKS_CREATE.to_string(),
                    task_id: str_field(create, "task_id"),
                    title: str_field(create, "title"),
                    status: String::new(),
                });
            }
            obj.get("update_task_status")
                .and_then(|v| v.as_object())
                .map(|update| RunEffect {
                    kind: EFFECT_TASKS_UPDATE_STATUS.to_string(),
                    task_id: str_field(update, "task_id"),
                    title: String::new(),
                    status: str_field(update, "status"),
                })
        })
        .collect()
}

/// tolerantly parse the model's answer into a JSON object (bare / de-fenced /
/// outermost span), matching runs' `parse_strict_response`.
fn parse_response_value(text: &str) -> Option<serde_json::Value> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    [
        Some(trimmed),
        strip_code_fence(trimmed),
        outermost_json_object(trimmed),
    ]
    .into_iter()
    .flatten()
    .find_map(|candidate| {
        serde_json::from_str::<serde_json::Value>(candidate.trim())
            .ok()
            .filter(serde_json::Value::is_object)
    })
}

/// strip a single surrounding markdown code fence (mirrors runs).
fn strip_code_fence(text: &str) -> Option<&str> {
    let body = text.strip_prefix("```")?.split_once('\n').map(|(_, b)| b)?;
    let body = body.trim();
    Some(body.strip_suffix("```").unwrap_or(body).trim())
}

/// the span from the first `{` to the last `}` (mirrors runs).
fn outermost_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    (start < end).then(|| &text[start..=end])
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
        let bytes = assemble_runner_result("the answer", &r, None, Vec::new(), Sink::Chain, Status::Ok);
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["ducktape_runner_result"], 1);
        assert_eq!(v["response_text"], "the answer");
        assert_eq!(v["workspace_receipt"]["output_snapshot"], "cc".repeat(32));
        assert_eq!(v["workspace_receipt"]["rebased"], true);
        assert_eq!(v["workspace_receipt"]["no_changes"], false);
    }

    #[test]
    fn empty_facets_skip_serialize_to_the_minimal_shape() {
        // a plain run (no data/effects, chain sink, ok status) must emit ONLY the
        // three core fields — the pre-v4 wrapper shape — so the untouched
        // response_text extraction stays byte-compatible.
        let r = WorkspaceReceipt::no_changes(&spec());
        let bytes = assemble_runner_result("hi", &r, None, Vec::new(), Sink::Chain, Status::Ok);
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let obj = v.as_object().unwrap();
        assert!(obj.contains_key("ducktape_runner_result"));
        assert!(obj.contains_key("response_text"));
        assert!(obj.contains_key("workspace_receipt"));
        assert!(!obj.contains_key("data"), "empty data must skip-serialize");
        assert!(!obj.contains_key("effects"), "empty effects must skip-serialize");
        assert!(!obj.contains_key("sink"), "chain sink must skip-serialize");
        assert!(!obj.contains_key("status"), "ok status must skip-serialize");
    }

    #[test]
    fn effects_from_response_text_lifts_a_tasks_create() {
        // the exact strict-response shape a model emits — AgentAction's
        // snake_case tags, fenced like an agentic CLI reply.
        let text = "```json\n{\"reply_blocks\":[{\"kind\":\"paragraph\",\"text\":\"done\"}],\
            \"actions\":[{\"create_task\":{\"task_id\":\"t1\",\"title\":\"ship it\"}},\
            {\"update_task_status\":{\"task_id\":\"t1\",\"status\":\"done\"}}]}\n```";
        let effects = effects_from_response_text(text);
        assert_eq!(
            effects,
            vec![
                RunEffect {
                    kind: "tasks.create".into(),
                    task_id: "t1".into(),
                    title: "ship it".into(),
                    status: String::new(),
                },
                RunEffect {
                    kind: "tasks.update_status".into(),
                    task_id: "t1".into(),
                    title: String::new(),
                    status: "done".into(),
                },
            ]
        );
        // prose with no actions lifts nothing (runs then falls back cleanly).
        assert!(effects_from_response_text("just prose, no json").is_empty());
        assert!(effects_from_response_text("").is_empty());
    }
}
