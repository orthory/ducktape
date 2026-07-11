use super::envelope::RUNNER_RESULT_VERSION;
use super::{
    ACTION_PAGES_COMMENT, ACTION_PAGES_SET_CHECKED, ACTION_TASKS_CREATE,
    ACTION_TASKS_UPDATE_STATUS, AgentAction, AgentResponse, Deserialize,
    JOB_FINALIZE_PAYLOAD_BYTES, MAX_ACTIONS_PER_RUN, Serialize,
};

/// the R5 typed-data facet ceiling — data larger than this degrades to null.
const MAX_DATA_BYTES: usize = 32 * 1024;
/// the faceted job-finalize payload envelope version (the `ducktape_delivery`
/// wrapper every run's finalize carries).
const DELIVERY_RECEIPT_VERSION: u32 = 1;

/// the wrapper the oracle's provisioning path returns instead of the bare
/// response text: the model prose plus the host-assembled facets — message
/// (`response_text`) / data / effects / artifact (`workspace_receipt`) / sink /
/// status. `deny_unknown_fields`: the assembled shape is this crate's own
/// contract with dispatch-oracle, and an unrecognized key is drift, not
/// forward compat. the facet fields keep `#[serde(default)]` because the
/// oracle SKIP-SERIALIZES empty/default facets (the minimal
/// `{ducktape_runner_result, response_text, workspace_receipt}` shape) —
/// load-bearing, not forward-compat. the single delivery path
/// ([`RunsModule::deliver_run_result`]) applies whatever facets are present.
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub(super) struct RunnerResult {
    pub(super) ducktape_runner_result: u32,
    pub(super) response_text: String,
    pub(super) workspace_receipt: WorkspaceReceipt,
    /// R5 typed-data facet: an already-serialized JSON text or `None`.
    pub(super) data: Option<String>,
    /// R2 declarative effects, host-assembled (lifted from the model's actions).
    #[serde(default)]
    pub(super) effects: Vec<WireEffect>,
    /// O1/O2 output sink; default [`WireSink::Chain`].
    #[serde(default)]
    pub(super) sink: WireSink,
    /// the host's terminal observation; default [`WireStatus::Ok`].
    #[serde(default)]
    pub(super) status: WireStatus,
}

#[derive(Deserialize, Default, Debug)]
pub(crate) struct WorkspaceReceipt {
    pub(crate) source_prefix: String,
    pub(crate) output_snapshot: Option<String>,
    pub(crate) commit_height: Option<u64>,
    pub(crate) rebased: bool,
    pub(crate) no_changes: bool,
    /// `Some` iff the executor's workspace commit failed — the writes were not
    /// captured (paired with a `degraded` status by the wrapper). the oracle
    /// skip-serializes it on healthy receipts (a missing Option key is None).
    pub(crate) commit_error: Option<String>,
    /// forge (§5): the pushed work branch — `Some` on a pushed receipt, and
    /// the ATTEMPTED branch on a commit-failed one; absent on every duckfs
    /// receipt.
    pub(crate) branch: Option<String>,
    /// forge (§5): the new commit oid — the forge output_ref is
    /// `branch@output_commit`. `Some` only when a push landed.
    pub(crate) output_commit: Option<String>,
}

/// the receipt's durable output reference for the delivered-runs ring: the
/// forge `branch@output_commit` when a push landed, else the duckfs output
/// snapshot, else `None` (nothing moved this run).
pub(super) fn output_ref_of(receipt: &WorkspaceReceipt) -> Option<String> {
    match (&receipt.branch, &receipt.output_commit) {
        (Some(branch), Some(oid)) => Some(format!("{branch}@{oid}")),
        _ => receipt.output_snapshot.clone(),
    }
}

/// one host-assembled declarative effect (R2). `kind` is a run-effect wire name
/// (`tasks.create` / `tasks.update_status` / `pages.comment` /
/// `pages.set_checked`); the remaining fields carry the action's payload.
/// mapped to an [`AgentAction`] by [`effects_to_actions`], where an unknown
/// `kind` fails the run deterministically (R4).
#[derive(Deserialize, Debug)]
pub(super) struct WireEffect {
    kind: String,
    #[serde(default)]
    task_id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    status: String,
    /// `pages.comment`: the page or block id the comment anchors to.
    #[serde(default)]
    target: String,
    /// `pages.comment`: the comment text.
    #[serde(default)]
    body: String,
    /// `pages.set_checked`: the todo block id.
    #[serde(default)]
    block: String,
    /// `pages.set_checked`: the desired checked state.
    #[serde(default)]
    checked: bool,
}

/// the O1/O2 output sink. internally tagged on `mode`; a MISSING sink field
/// defaults to `Chain` via the `#[serde(default)]` on [`RunnerResult::sink`],
/// and a present `{"mode":"pr",...}` decodes to `Pr`.
///
/// ONE shape, both directions (M1): the same type also SERIALIZES as the
/// envelope's REQUESTED sink (`result_contract.sink`). a requested `Pr`
/// carries empty title/body — the keys skip-serialize, and delivery derives
/// them from the message facet — and `Chain` composes as an absent field
/// (see [`WireSink::is_chain`]), matching the oracle's own skip.
#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub(crate) enum WireSink {
    #[default]
    Chain,
    Pr {
        #[serde(default)]
        repo: String,
        source_branch: String,
        #[serde(default)]
        target_branch: String,
        #[serde(skip_serializing_if = "String::is_empty")]
        title: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        body: String,
    },
}

impl WireSink {
    /// the composed `result_contract` omits the sink key entirely for `Chain`
    /// — the serde skip mirroring dispatch-oracle's `is_chain`.
    pub(crate) fn is_chain(&self) -> bool {
        matches!(self, WireSink::Chain)
    }
}

/// the host's terminal observation of a run. `Failed` fails the run even with a
/// present message facet; `Degraded` still delivers (surfaced in the receipt).
#[derive(Deserialize, Default, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub(super) enum WireStatus {
    #[default]
    Ok,
    Degraded,
    Failed,
}

// ---- faceted delivery --------------------------------------------------------
// the single delivery path decodes the runner result below and applies whatever
// facets it carries; a plain (message-only) result carries none.

/// decode the full faceted [`RunnerResult`]. marker + version strict (R4): a
/// result that is not a well-formed `ducktape_runner_result` wrapper of the
/// understood version FAILS THE RUN, deterministically — the flat-string
/// passthrough for marker-less bytes is gone (flag day; the oracle always
/// wraps).
pub(super) fn decode_run_result_v1(bytes: &[u8]) -> Result<RunnerResult, String> {
    let result: RunnerResult = serde_json::from_slice(bytes)
        .map_err(|e| format!("runner result is malformed (no flat-payload tolerance): {e}"))?;
    if result.ducktape_runner_result != RUNNER_RESULT_VERSION {
        return Err(format!(
            "runner result version {} is not supported (understands {RUNNER_RESULT_VERSION})",
            result.ducktape_runner_result
        ));
    }
    Ok(result)
}

/// map host-assembled declarative effects into the validated [`AgentAction`]
/// vocabulary: the two task verbs plus the two pages verbs (chat.post is the
/// message facet, not an effect). an UNKNOWN kind fails the run
/// deterministically (R4) — this is the concrete gate for any verb beyond the
/// known set. payload validity is NOT checked here: task payloads are the
/// strict validator's job, pages payloads degrade per-action at apply.
pub(super) fn effects_to_actions(effects: &[WireEffect]) -> Result<Vec<AgentAction>, String> {
    if effects.len() > MAX_ACTIONS_PER_RUN {
        return Err(format!(
            "{} effects exceed the cap of {MAX_ACTIONS_PER_RUN}",
            effects.len()
        ));
    }
    effects
        .iter()
        .map(|e| match e.kind.as_str() {
            ACTION_TASKS_CREATE => Ok(AgentAction::CreateTask {
                task_id: e.task_id.clone(),
                title: e.title.clone(),
            }),
            ACTION_TASKS_UPDATE_STATUS => Ok(AgentAction::UpdateTaskStatus {
                task_id: e.task_id.clone(),
                status: e.status.clone(),
            }),
            ACTION_PAGES_COMMENT => Ok(AgentAction::AddPageComment {
                target: e.target.clone(),
                body: e.body.clone(),
            }),
            ACTION_PAGES_SET_CHECKED => Ok(AgentAction::SetPageChecked {
                block: e.block.clone(),
                checked: e.checked,
            }),
            other => Err(format!("unknown effect kind: {other}")),
        })
        .collect()
}

/// the R5 data facet, valid only when it is within the size ceiling AND parses
/// as JSON; anything else degrades to null (never fails the run).
pub(super) fn valid_data(data: &Option<String>) -> Option<&str> {
    data.as_deref().filter(|s| {
        s.len() <= MAX_DATA_BYTES && serde_json::from_str::<serde_json::Value>(s).is_ok()
    })
}

/// the faceted job-finalize payload: the validated response plus the data
/// facet, the derived output_ref (O1), and the status. deterministic — fixed
/// serde field order, data embedded verbatim as already-validated JSON.
#[derive(Serialize)]
struct DeliveryReceipt<'a> {
    ducktape_delivery: u32,
    response: &'a AgentResponse,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_ref: Option<OutputRef<'a>>,
    status: &'static str,
}

/// the artifact facet distilled to a chainable reference (O1): a downstream run
/// can set `workspace.source = prior.output_snapshot`; a forge run's output is
/// the git coordinates `branch@output_commit` instead (the snapshot key stays a
/// stated `null`, the forge keys skip-serialize on duckfs receipts — pre-forge
/// bytes are unchanged).
#[derive(Serialize, Clone)]
struct OutputRef<'a> {
    source_prefix: &'a str,
    output_snapshot: Option<&'a str>,
    commit_height: Option<u64>,
    rebased: bool,
    no_changes: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    branch: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_commit: Option<&'a str>,
}

pub(super) fn encode_delivery_receipt(
    response: &AgentResponse,
    data: Option<&str>,
    receipt: &WorkspaceReceipt,
    status: WireStatus,
) -> String {
    // an output exists when the run produced a duckfs snapshot OR pushed a
    // forge commit; both distill into the one output_ref shape.
    let output_ref = (receipt.output_snapshot.is_some() || receipt.output_commit.is_some())
        .then(|| OutputRef {
            source_prefix: &receipt.source_prefix,
            output_snapshot: receipt.output_snapshot.as_deref(),
            commit_height: receipt.commit_height,
            rebased: receipt.rebased,
            no_changes: receipt.no_changes,
            branch: receipt.branch.as_deref(),
            output_commit: receipt.output_commit.as_deref(),
        });
    let status = match status {
        WireStatus::Ok => "ok",
        WireStatus::Degraded => "degraded",
        WireStatus::Failed => "failed",
    };
    let encode = |data: Option<&str>| {
        serde_json::to_string(&DeliveryReceipt {
            ducktape_delivery: DELIVERY_RECEIPT_VERSION,
            response,
            data,
            output_ref: output_ref.clone(),
            status,
        })
        .expect("delivery receipt serializes")
    };
    // the finalize payload MUST stay valid JSON within the jobs cap: the naive
    // byte-truncation the jobs board applies would corrupt it. the response is
    // capped by validation (MAX_REPLY_BLOCKS_BYTES + MAX_ACTIONS_BYTES) and
    // output_ref/status are tiny, so a no-data receipt always fits; the
    // optional `data` facet is embedded only if the whole receipt still fits,
    // else DROPPED here (the full data facet stays in the dispatch-history
    // audit lane, R6, so nothing durable is lost). the ladder re-checks its own
    // fallback — never hand the jobs board something it would byte-truncate.
    let full = encode(data);
    if full.len() <= JOB_FINALIZE_PAYLOAD_BYTES {
        return full;
    }
    let without_data = encode(None);
    if without_data.len() <= JOB_FINALIZE_PAYLOAD_BYTES {
        return without_data;
    }
    // unreachable while the validation caps hold (32Ki blocks + 8Ki actions
    // << 64Ki cap); a deterministic stub keeps the payload valid JSON with the
    // O1 output_ref intact even if a cap regresses.
    serde_json::to_string(&DeliveryReceipt {
        ducktape_delivery: DELIVERY_RECEIPT_VERSION,
        response: &AgentResponse::default(),
        data: None,
        output_ref,
        status: "degraded",
    })
    .expect("delivery receipt serializes")
}
