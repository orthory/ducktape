//! the host-side provisioning seam: an injected [`WorkspaceProvisioner`] that
//! materializes a per-run duckfs workspace OUTSIDE storage, the plain-data
//! vocabulary the pool passes across the reachability wall, and the SINGLE
//! owner of the host-assembled [`WorkspaceReceipt`] + `RunnerResult` bytes.
//!
//! dispatch-oracle CANNOT depend on duckfs-client (the reachability wall — a
//! kernel/system crate must never touch the OS-side checkout engine), so this
//! module speaks only plain data: the concrete `checkout_with`/`commit` calls
//! live in the node binary's provisioner impl. the pool brackets a run with
//! provision → bind → run → commit → assemble → cleanup ONLY when a
//! provisioner is wired; an embedder without one keeps the accept-only
//! degrade (raw-text delivery, no workspace). this path is LIVE in both node
//! binaries.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use capability_host::RunContext;
use serde::{Deserialize, Serialize};

use crate::workspace_source::WorkspaceSource;

/// the pinned portable plan carried by [`crate::Prepared`]. the pool turns it
/// into a [`WorkspaceSpec`] iff a provisioner is wired, else it is inert
/// (dormant).
///
/// carries NO `mount_path`: the composer emits SOURCE coordinates only (D7),
/// and the provisioner mints its own writable host cwd. `skills` are the C4
/// read-only mounts, surfaced straight into [`WorkspaceSpec::ro_mounts`];
/// `sink` is the REQUESTED output sink (`result_contract.sink`, absent ⇒
/// [`Sink::Chain`]) the pool echoes onto the assembled `RunnerResult`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortablePlan {
    pub source: WorkspaceSource,
    /// the run's CONSENSUS id, verbatim from the envelope — see
    /// [`WorkspaceSpec::consensus_run_id`]. `None` on an envelope composed
    /// before the field existed.
    pub consensus_run_id: Option<String>,
    pub sink: Sink,
    pub skills: Vec<RoMount>,
    /// committed registry name, carried to the Forge commit boundary.
    pub agent_display_name: String,
    /// whether the agent's `duckfs_read` caps cover the global skill library —
    /// see [`WorkspaceSpec::library_readable`]. `false` on an envelope composed
    /// before the field existed: the conservative default, since the paragraph it
    /// gates is only useful to an agent that can act on it.
    pub library_readable: bool,
}

/// what the pool hands the provisioner for one run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSpec {
    /// `"{saga_id}:{attempt}"` — idempotency key + per-run dir naming.
    ///
    /// NOT the id `runs` resolves a run by: it is a HOST-local key, and it
    /// carries the attempt on purpose (a re-lease spawns a new attempt while
    /// the old one may still be running, and the two must never share a
    /// checkout dir). hashing it resolves nothing in consensus — the id a run
    /// is named by there is [`Self::consensus_run_id`].
    pub run_id: String,
    /// the id `runs` resolves the run by — the key of its pending map, and the
    /// run the agent session lane binds to. carried from the composer through
    /// the envelope because the host cannot derive it (see [`Self::run_id`]).
    /// `None` = a pre-field envelope, or an embedder that composes its own: the
    /// run then opens NO agent session and executes on the read-only tool plane
    /// — the pre-session behaviour, never an error.
    pub consensus_run_id: Option<String>,
    pub agent_id: Option<String>,
    pub agent_display_name: Option<String>,
    /// the pinned source the provisioner materializes — a duckfs subtree or a
    /// forge repo@commit on a work branch, verbatim from the plan.
    pub source: WorkspaceSource,
    /// W6 skill/instruction ro subtrees — the plan's C4 skill mounts,
    /// verbatim.
    pub ro_mounts: Vec<RoMount>,
    /// whether the agent may READ the global skill library
    /// (`agent::SKILL_LIBRARY_PREFIX`): a plain-data echo of the committed
    /// `duckfs_read` grant, decided in consensus by the composer and carried
    /// across the reachability wall like every other plan field.
    ///
    /// the provisioner hands it to [`crate::assemble_context_doc`], which emits
    /// the library paragraph only when it is `true` — an agent without the grant
    /// is never pointed at a prefix the MCP tool plane would refuse it.
    pub library_readable: bool,
}

/// a read-only mount the provisioner materializes beside the rw source (W6) —
/// populated from the composed skills.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoMount {
    pub source_prefix: String,
    pub source_snapshot: Option<String>,
    pub mount_subpath: String,
    /// the committed LOAD MODE (`SkillRef.load`): this skill's full body is
    /// inlined into the run's context document ([`crate::assemble_context_doc`])
    /// rather than merely indexed there. plain data crossing the reachability
    /// wall — the provisioner reads the materialized `SKILL.md` and assembles;
    /// this crate never touches duckfs.
    pub always: bool,
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
    /// forge only (contract §5, additive): the work branch — `Some` when a
    /// push landed, and on a forge FAILURE receipt (`commit_failed`), where
    /// the ATTEMPTED branch is known and rides the audit lane (task-2 review
    /// call). skip-serialized so duckfs receipt bytes are unchanged.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub branch: Option<String>,
    /// forge only (contract §5, additive): the new commit oid — the forge
    /// `output_ref` is `branch@output_commit`.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub output_commit: Option<String>,
}

impl WorkspaceReceipt {
    /// the base every constructor extends: source coordinates from the spec
    /// (duckfs prefix/pin verbatim; forge `forge:<repo>` + pinned commit, §5),
    /// everything else empty.
    fn base(spec: &WorkspaceSpec) -> Self {
        let (source_prefix, source_snapshot) = spec.source.receipt_coords();
        Self {
            source_prefix,
            source_snapshot,
            output_snapshot: None,
            commit_height: None,
            rebased: false,
            no_changes: false,
            commit_error: None,
            branch: None,
            output_commit: None,
        }
    }

    /// the agent committed a new duckfs snapshot (the `output_ref`).
    pub fn committed(spec: &WorkspaceSpec, snapshot: String, height: u64, rebased: bool) -> Self {
        Self {
            output_snapshot: Some(snapshot),
            commit_height: Some(height),
            rebased,
            ..Self::base(spec)
        }
    }

    /// forge: the agent's commit was PUSHED to the work branch — the forge
    /// `output_ref` is `<branch>@<output_commit>` (§5). duckfs's
    /// snapshot/height stay `None`: the artifact lives in the git substrate,
    /// not a duckfs snapshot. forge-ONLY: a duckfs success is `committed()`.
    pub fn pushed(spec: &WorkspaceSpec, output_commit: String) -> Self {
        // loud in debug so a mixed-up caller can never mint a branchless
        // "pushed" receipt for a duckfs spec (flagged in the task-2 review).
        debug_assert!(
            matches!(spec.source, WorkspaceSource::Forge { .. }),
            "WorkspaceReceipt::pushed is forge-only (duckfs success is committed())"
        );
        Self {
            branch: spec.source.forge_branch(),
            output_commit: Some(output_commit),
            ..Self::base(spec)
        }
    }

    /// the agent wrote nothing — a clean working copy (R2: any facet may be
    /// empty). no `output_ref` is produced (and for forge, no push happens).
    pub fn no_changes(spec: &WorkspaceSpec) -> Self {
        Self {
            no_changes: true,
            ..Self::base(spec)
        }
    }

    /// the commit MECHANISM failed — the agent's writes were not captured. the
    /// truth is "capture failed", never "clean tree": `no_changes` stays false
    /// and the error rides the receipt into the audit lane (I4), while the
    /// run's answer still delivers (R4) under a `Degraded` status. the forge
    /// push-CAS reject rides THIS lane too (§5).
    pub fn commit_failed(spec: &WorkspaceSpec, error: String) -> Self {
        Self {
            commit_error: Some(error),
            // forge: the ATTEMPTED work branch is known at failure time and
            // rides the audit lane too (task-2 review call). duckfs specs
            // carry no branch, so their receipt bytes are unchanged.
            branch: spec.source.forge_branch(),
            ..Self::base(spec)
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
    /// the run's SOUL: its curated skills assembled into one document
    /// ([`crate::assemble_context_doc`]) by whoever materialized the ro mounts
    /// — the only layer that can read them. `None` = the agent curated no
    /// skills. defaulted for embedders whose workspace has no skill plane; the
    /// production provisioner always answers.
    fn context_doc(&self) -> Option<String> {
        None
    }
    /// commit ONLY the rw source; a clean working copy → a `no_changes`
    /// receipt (never an error).
    async fn commit(&self, message: &str) -> Result<WorkspaceReceipt, String>;
    /// W5 cleanup: idempotent, best-effort, never fails the run.
    async fn cleanup(&self);
}

/// the shared handle the pool holds — injected like the blob resolver.
pub type SharedProvisioner = Arc<dyn WorkspaceProvisioner>;

/// the ONE place a materialized workspace is bound onto the run context: the
/// mount becomes the child's cwd, its env is layered additively, its tool bin
/// dirs feed `PATH`, and its assembled soul rides into the run — capability-host
/// decides the door (the executor's auto-load path, or the stdin prompt).
pub fn bind_workspace(ws: &dyn ProvisionedWorkspace, ctx: &mut RunContext) {
    ctx.workdir_override = Some(ws.workdir());
    ctx.env.extend(ws.env());
    ctx.path_entries = ws.path_entries();
    ctx.context_doc = ws.context_doc();
}

// ---- faceted runner result (v1 wire) --------------------------------------------
// the runner result grew from message-only to the six ADR facets — message
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
/// the wire name a [`RunEffect`] carries for a page comment (M2).
const EFFECT_PAGES_COMMENT: &str = "pages.comment";
/// the wire name a [`RunEffect`] carries for a todo check flip (M2).
const EFFECT_PAGES_SET_CHECKED: &str = "pages.set_checked";

/// one declarative, host-assembled effect the model's answer requested. lifted
/// out of `response_text` by [`effects_from_response_text`] so at v4 runs applies
/// the HOST's observation (R1) rather than re-parsing prose. idempotent by
/// run_id (applied once at the delivery boundary).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct RunEffect {
    pub kind: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub task_id: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub status: String,
    /// `pages.comment` (M2): the page/block anchor and the comment text.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub target: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub body: String,
    /// `pages.set_checked` (M2): the todo block and the desired state. a
    /// skipped `checked` decodes false on the runs side — same value.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub block: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub checked: bool,
}

/// the O1/O2 output sink: `Chain` (default — the next run reads this run's
/// output_ref) / `Pr` (open a forge PR). the concrete routing is runs'
/// concern; the wrapper only names the intent.
///
/// Deserialize decodes the composer's REQUESTED sink
/// (`result_contract.sink`, contract §1): the requested-Pr shape carries NO
/// title/body keys, so those default EMPTY on decode. Serialize keeps them
/// PRESENT (no skip) — runs' decode keeps `title` required, so the echoed
/// `RunnerResult` must always state them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum Sink {
    #[default]
    Chain,
    Pr {
        repo: String,
        source_branch: String,
        target_branch: String,
        #[serde(default)]
        title: String,
        #[serde(default)]
        body: String,
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
    /// not constructed host-side today — kept so the wire vocabulary states
    /// all three states runs decodes.
    #[allow(dead_code)]
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
///
/// the assembled bytes are delivered as the saga's Ok payload, and the saga
/// ABORTS any Ok larger than [`saga::MAX_RESULT_BYTES`] at the block — the
/// attempt could then never land and the run would wedge until its deadline.
/// so the assembly is capped HERE: an oversized result gets its PROSE
/// truncated (char-boundary, with a note naming the original size) while the
/// receipt/effects/sink survive; in the pathological case where even a
/// note-only wrapper is over, the effects and data facets are dropped too.
pub fn assemble_runner_result(
    response_text: &str,
    receipt: &WorkspaceReceipt,
    data: Option<String>,
    effects: Vec<RunEffect>,
    sink: Sink,
    status: Status,
) -> Vec<u8> {
    let encode = |text: &str, data: Option<String>, effects: Vec<RunEffect>| {
        serde_json::to_vec(&RunnerResultWire {
            ducktape_runner_result: crate::envelope::RUNNER_RESULT_VERSION,
            response_text: text,
            workspace_receipt: receipt,
            data,
            effects,
            sink: sink.clone(),
            status,
        })
        .expect("runner result serializes")
    };
    let full = encode(response_text, data.clone(), effects.clone());
    if full.len() <= saga::MAX_RESULT_BYTES {
        return full;
    }
    // removing N prose bytes shrinks the JSON by AT LEAST N (escaping only
    // grows a byte), so cutting the overage plus the note's own length (with
    // slack for the note's escaped newline) always fits — one re-serialize,
    // no loop.
    let note = format!("\n[output truncated ({} bytes)]", response_text.len());
    let mut keep = response_text
        .len()
        .saturating_sub(full.len() - saga::MAX_RESULT_BYTES + note.len() + 16);
    while keep > 0 && !response_text.is_char_boundary(keep) {
        keep -= 1;
    }
    let truncated = encode(
        &format!("{}{note}", &response_text[..keep]),
        data,
        effects,
    );
    if truncated.len() <= saga::MAX_RESULT_BYTES {
        return truncated;
    }
    // the non-prose facets alone exceed the cap (a pathological effects/data
    // payload): drop them — a degraded-but-landing result beats a wedged saga.
    let bare = encode(&note, None, Vec::new());
    if bare.len() <= saga::MAX_RESULT_BYTES {
        return bare;
    }
    // last rung: even the receipt+sink alone exceed the cap (an unbounded
    // commit_error or sink title/body). replace the receipt's free-form
    // strings with short notes and drop the sink payload — the result MUST
    // land under the cap or the saga wedges until deadline (the exact
    // failure this cap exists to prevent).
    let stub = WorkspaceReceipt {
        commit_error: receipt
            .commit_error
            .as_ref()
            .map(|e| format!("[commit error truncated ({} bytes)]", e.len())),
        branch: receipt.branch.clone(),
        output_commit: receipt.output_commit.clone(),
        ..receipt_stub(receipt)
    };
    serde_json::to_vec(&RunnerResultWire {
        ducktape_runner_result: crate::envelope::RUNNER_RESULT_VERSION,
        response_text: &note,
        workspace_receipt: &stub,
        data: None,
        effects: Vec::new(),
        sink: Sink::Chain,
        status,
    })
    .expect("runner result serializes")
}

/// the bounded core of a receipt: everything except the free-form strings
/// the last degrade rung replaces.
fn receipt_stub(r: &WorkspaceReceipt) -> WorkspaceReceipt {
    WorkspaceReceipt {
        source_prefix: r.source_prefix.clone(),
        output_snapshot: r.output_snapshot.clone(),
        source_snapshot: r.source_snapshot.clone(),
        commit_height: r.commit_height,
        rebased: r.rebased,
        no_changes: r.no_changes,
        commit_error: None,
        branch: None,
        output_commit: None,
    }
}

/// LIFT the model's `tasks.create` / `tasks.update_status` /
/// `pages.comment` / `pages.set_checked` actions out of its
/// strict-response prose into declarative [`RunEffect`]s (R1, activation
/// correctness — critic #4). at v4 runs applies these host-assembled effects
/// rather than re-parsing the prose; an empty result lets runs fall back to the
/// response-parsed actions so nothing is silently dropped — which is also why
/// EVERY known action kind must lift: a partial lift would make the effects
/// facet override the prose actions and drop the unlifted ones. the parse
/// mirrors `runs::parse_strict_response`: bare JSON, then a `` ```json ``
/// fence, then the outermost `{…}` span. the action shape is `AgentAction`'s
/// snake_case tags.
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
                    ..RunEffect::default()
                });
            }
            if let Some(update) = obj.get("update_task_status").and_then(|v| v.as_object()) {
                return Some(RunEffect {
                    kind: EFFECT_TASKS_UPDATE_STATUS.to_string(),
                    task_id: str_field(update, "task_id"),
                    status: str_field(update, "status"),
                    ..RunEffect::default()
                });
            }
            if let Some(comment) = obj.get("add_page_comment").and_then(|v| v.as_object()) {
                return Some(RunEffect {
                    kind: EFFECT_PAGES_COMMENT.to_string(),
                    target: str_field(comment, "target"),
                    body: str_field(comment, "body"),
                    ..RunEffect::default()
                });
            }
            obj.get("set_page_checked")
                .and_then(|v| v.as_object())
                .map(|flip| RunEffect {
                    kind: EFFECT_PAGES_SET_CHECKED.to_string(),
                    block: str_field(flip, "block"),
                    checked: flip.get("checked").and_then(|v| v.as_bool()).unwrap_or(false),
                    ..RunEffect::default()
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
    #[test]
    fn a_pathological_receipt_still_fits_the_saga_cap() {
        // even when the receipt's free-form strings alone exceed the cap
        // (an unbounded commit_error), the last degrade rung must land the
        // result under saga::MAX_RESULT_BYTES — an oversized Ok wedges the
        // saga until deadline.
        let receipt = super::WorkspaceReceipt {
            source_prefix: "p".into(),
            source_snapshot: None,
            output_snapshot: None,
            commit_height: None,
            rebased: false,
            no_changes: false,
            commit_error: Some("x".repeat(saga::MAX_RESULT_BYTES + 4096)),
            branch: None,
            output_commit: None,
        };
        let out = super::assemble_runner_result(
            "answer",
            &receipt,
            None,
            Vec::new(),
            super::Sink::Chain,
            super::Status::Ok,
        );
        assert!(
            out.len() <= saga::MAX_RESULT_BYTES,
            "degraded result must fit the cap, got {} bytes",
            out.len()
        );
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert!(
            v["workspace_receipt"]["commit_error"]
                .as_str()
                .unwrap()
                .contains("truncated"),
            "the oversized commit_error is replaced with a bounded note"
        );
    }

    use super::*;

    /// a CONSENSUS run id in the shape `runs` mints (`chat\x1f<channel>\x1f<seq>\x1f<agent>`).
    /// written out rather than imported: this crate must not depend on an app
    /// module. the cross-crate proof that composer and provisioner agree on the
    /// id space lives where both are reachable — `bin/noded`'s session-boundary
    /// test.
    const CONSENSUS_RUN_ID: &str = "chat\u{1f}general\u{1f}1\u{1f}bot";

    fn spec() -> WorkspaceSpec {
        WorkspaceSpec {
            run_id: "s1:0".into(),
            consensus_run_id: Some(CONSENSUS_RUN_ID.into()),
            agent_id: Some("bot".into()),
            agent_display_name: Some("Bot".into()),
            source: WorkspaceSource::Duckfs {
                source_prefix: "/shared/agent-workspaces/bot".into(),
                source_snapshot: Some("aa".repeat(32)),
            },
            ro_mounts: Vec::new(),
            library_readable: false,
        }
    }

    fn forge_spec() -> WorkspaceSpec {
        WorkspaceSpec {
            run_id: "s1:0".into(),
            consensus_run_id: Some(CONSENSUS_RUN_ID.into()),
            agent_id: Some("bot".into()),
            agent_display_name: Some("Bot".into()),
            source: WorkspaceSource::Forge {
                repo: "app".into(),
                commit: "d0".repeat(20),
                branch: "agent/item-7".into(),
                branch_born: false,
            },
            ro_mounts: Vec::new(),
            library_readable: false,
        }
    }

    #[test]
    fn forge_receipts_carry_repo_coords_and_the_pinned_commit() {
        // contract §5: source_prefix = "forge:<repo>", source_snapshot =
        // Some(pinned commit) — on EVERY constructor, success or not.
        let r = WorkspaceReceipt::no_changes(&forge_spec());
        assert_eq!(r.source_prefix, "forge:app");
        assert_eq!(r.source_snapshot.as_deref(), Some("d0".repeat(20).as_str()));
        assert_eq!(r.branch, None, "no push landed — no pushed branch");
        assert_eq!(r.output_commit, None);

        // a FAILURE receipt still names the ATTEMPTED branch (the audit lane
        // knows where the push aimed; task-2 review call) — but never mints
        // an output_commit.
        let r = WorkspaceReceipt::commit_failed(&forge_spec(), "push CAS-rejected".into());
        assert_eq!(r.source_prefix, "forge:app");
        assert_eq!(r.source_snapshot.as_deref(), Some("d0".repeat(20).as_str()));
        assert_eq!(r.branch.as_deref(), Some("agent/item-7"));
        assert_eq!(r.output_commit, None);
        assert_eq!(r.commit_error.as_deref(), Some("push CAS-rejected"));
    }

    #[test]
    fn a_pushed_receipt_is_the_forge_output_ref() {
        // the forge success shape task 3's provisioner emits: the output_ref
        // is `branch@output_commit`; duckfs's snapshot/height stay None (the
        // artifact lives in the git substrate, not a duckfs snapshot).
        let r = WorkspaceReceipt::pushed(&forge_spec(), "e1".repeat(20));
        assert_eq!(r.source_prefix, "forge:app");
        assert_eq!(r.source_snapshot.as_deref(), Some("d0".repeat(20).as_str()));
        assert_eq!(r.branch.as_deref(), Some("agent/item-7"));
        assert_eq!(r.output_commit.as_deref(), Some("e1".repeat(20).as_str()));
        assert_eq!(r.output_snapshot, None);
        assert_eq!(r.commit_height, None);
        assert!(!r.no_changes && !r.rebased && r.commit_error.is_none());
    }

    #[test]
    fn the_new_receipt_fields_are_additive_on_the_wire() {
        // None ⇒ ABSENT keys: a duckfs receipt's bytes are unchanged by the
        // forge fields (the additive half of contract §5).
        let duckfs = WorkspaceReceipt::committed(&spec(), "cc".repeat(32), 9, false);
        let v = serde_json::to_value(&duckfs).unwrap();
        let obj = v.as_object().unwrap();
        assert!(!obj.contains_key("branch"), "None branch must skip-serialize");
        assert!(
            !obj.contains_key("output_commit"),
            "None output_commit must skip-serialize"
        );
        // Some ⇒ PRESENT keys with the §5 values.
        let pushed = WorkspaceReceipt::pushed(&forge_spec(), "e1".repeat(20));
        let v = serde_json::to_value(&pushed).unwrap();
        assert_eq!(v["branch"], "agent/item-7");
        assert_eq!(v["output_commit"], "e1".repeat(20));
        assert_eq!(v["source_prefix"], "forge:app");
        assert_eq!(v["source_snapshot"], "d0".repeat(20));
    }

    #[test]
    fn a_requested_pr_sink_without_title_or_body_decodes_default_empty() {
        // the composer's requested-sink bytes (contract §1) carry NO
        // title/body keys — delivery derives them later; this worker decodes
        // them default-empty into the existing enum.
        let sink: Sink = serde_json::from_str(
            r#"{"mode":"pr","repo":"app","source_branch":"agent/item-7","target_branch":"main"}"#,
        )
        .unwrap();
        assert_eq!(
            sink,
            Sink::Pr {
                repo: "app".into(),
                source_branch: "agent/item-7".into(),
                target_branch: "main".into(),
                title: String::new(),
                body: String::new(),
            }
        );
        // present title/body keys still decode verbatim.
        let sink: Sink = serde_json::from_str(
            r#"{"mode":"pr","repo":"app","source_branch":"b","target_branch":"main","title":"T","body":"B"}"#,
        )
        .unwrap();
        assert!(
            matches!(&sink, Sink::Pr { title, body, .. } if title == "T" && body == "B"),
            "got {sink:?}"
        );
        // an unknown mode is a loud decode failure, never a guessed sink.
        assert!(serde_json::from_str::<Sink>(r#"{"mode":"carrier_pigeon"}"#).is_err());
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
        let bytes =
            assemble_runner_result("the answer", &r, None, Vec::new(), Sink::Chain, Status::Ok);
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
        assert!(
            !obj.contains_key("effects"),
            "empty effects must skip-serialize"
        );
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
                    ..RunEffect::default()
                },
                RunEffect {
                    kind: "tasks.update_status".into(),
                    task_id: "t1".into(),
                    status: "done".into(),
                    ..RunEffect::default()
                },
            ]
        );
        // prose with no actions lifts nothing (runs then falls back cleanly).
        assert!(effects_from_response_text("just prose, no json").is_empty());
        assert!(effects_from_response_text("").is_empty());
    }

    #[test]
    fn effects_from_response_text_lifts_the_pages_actions() {
        // M2: the pages verbs must lift alongside the task verbs — a partial
        // lift would override the prose actions and silently drop these.
        let text = "{\"actions\":[\
            {\"add_page_comment\":{\"target\":\"b1\",\"body\":\"nice\"}},\
            {\"set_page_checked\":{\"block\":\"b2\",\"checked\":true}}]}";
        let effects = effects_from_response_text(text);
        assert_eq!(
            effects,
            vec![
                RunEffect {
                    kind: "pages.comment".into(),
                    target: "b1".into(),
                    body: "nice".into(),
                    ..RunEffect::default()
                },
                RunEffect {
                    kind: "pages.set_checked".into(),
                    block: "b2".into(),
                    checked: true,
                    ..RunEffect::default()
                },
            ]
        );
        // the wire keys skip-serialize: a set_checked carries no task keys.
        let json = serde_json::to_value(&effects[1]).unwrap();
        let obj = json.as_object().unwrap();
        assert!(!obj.contains_key("task_id"));
        assert!(!obj.contains_key("target"));
    }
}
