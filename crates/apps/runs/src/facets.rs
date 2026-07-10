use super::response::RUNNER_RESULT_VERSION;
use super::{
    ACTION_TASKS_CREATE, ACTION_TASKS_UPDATE_STATUS, AgentAction, AgentResponse, CapRequest, Ctx,
    Deserialize, JOB_FINALIZE_PAYLOAD_BYTES, MAX_ACTIONS_PER_RUN, Msg, PendingState, RunsModule,
    Serialize,
};

/// the R5 typed-data facet ceiling — data larger than this degrades to null.
const MAX_DATA_BYTES: usize = 32 * 1024;
/// the faceted job-finalize payload envelope version (the `ducktape_delivery`
/// wrapper every run's finalize carries).
const DELIVERY_RECEIPT_VERSION: u32 = 1;

/// the wrapper a portable (`v3`) provider returns instead of the bare response
/// text: the model prose plus the host-assembled facets — message
/// (`response_text`) / data / effects / artifact (`workspace_receipt`) / sink /
/// status. the five facet fields are ADDITIVE `#[serde(default)]`s (this struct
/// is deliberately NOT `deny_unknown_fields`), so a minimal
/// `{ducktape_runner_result, response_text, workspace_receipt}` wrapper still
/// decodes and a bytes-with-no-marker result becomes a message-only result. the
/// single delivery path ([`RunsModule::deliver_run_result`]) applies whatever
/// facets are present; a plain (message-only) result carries none.
#[derive(Deserialize, Debug)]
pub(super) struct RunnerResult {
    pub(super) ducktape_runner_result: u32,
    pub(super) response_text: String,
    pub(super) workspace_receipt: WorkspaceReceipt,
    /// R5 typed-data facet: an already-serialized JSON text or `None`.
    #[serde(default)]
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
pub(super) struct WorkspaceReceipt {
    source_prefix: String,
    #[allow(dead_code, reason = "audit metadata retained in dispatch history")]
    source_snapshot: Option<String>,
    output_snapshot: Option<String>,
    commit_height: Option<u64>,
    rebased: bool,
    no_changes: bool,
    /// `Some` iff the executor's workspace commit failed — the writes were not
    /// captured (paired with a `degraded` status by the wrapper). additive:
    /// absent on healthy receipts.
    #[serde(default)]
    #[allow(dead_code, reason = "audit metadata retained in dispatch history")]
    commit_error: Option<String>,
}

/// one host-assembled declarative effect (R2). `kind` is a run-effect wire name
/// (`tasks.create` / `tasks.update_status`); the remaining fields carry the
/// action's payload. mapped to an [`AgentAction`] by [`effects_to_actions`],
/// where an unknown `kind` fails the run deterministically (R4).
#[derive(Deserialize, Debug)]
pub(super) struct WireEffect {
    kind: String,
    #[serde(default)]
    task_id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    status: String,
}

/// the O1/O2 output sink. internally tagged on `mode`; a MISSING sink field
/// defaults to `Chain` via the `#[serde(default)]` on [`RunnerResult::sink`], and
/// a present `{"mode":"pr",...}` decodes to `Pr`. `Merge` is DEFINED-BUT-INERT in
/// v1 (validated on the wire, treated like `Chain` with a breadcrumb).
#[derive(Deserialize, Default, Debug)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub(super) enum WireSink {
    #[default]
    Chain,
    Pr {
        #[serde(default)]
        repo: String,
        source_branch: String,
        #[serde(default)]
        target_branch: String,
        title: String,
        #[serde(default)]
        body: String,
    },
    Merge {
        #[serde(default)]
        repo: String,
        number: u64,
        #[allow(
            dead_code,
            reason = "merge sink wire is forward-compatible but inert in v1"
        )]
        prev_target_oid: String,
        #[allow(
            dead_code,
            reason = "merge sink wire is forward-compatible but inert in v1"
        )]
        expected_source_oid: String,
        #[allow(
            dead_code,
            reason = "merge sink wire is forward-compatible but inert in v1"
        )]
        merge_oid: String,
        #[allow(
            dead_code,
            reason = "merge sink wire is forward-compatible but inert in v1"
        )]
        pack_digest: String,
    },
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
/// wrapper that claims the marker but is malformed or an unsupported version
/// fails the run. bytes with NO marker — every legacy raw-text result — become a
/// message-only result (response_text = the lossy-decoded bytes, no facets).
pub(super) fn decode_run_result_v1(bytes: &[u8]) -> Result<RunnerResult, String> {
    match serde_json::from_slice::<serde_json::Value>(bytes) {
        Ok(serde_json::Value::Object(map)) if map.contains_key("ducktape_runner_result") => {
            let result: RunnerResult = serde_json::from_value(serde_json::Value::Object(map))
                .map_err(|e| format!("runner result is malformed: {e}"))?;
            if result.ducktape_runner_result != RUNNER_RESULT_VERSION {
                return Err(format!(
                    "runner result version {} is not supported (understands {RUNNER_RESULT_VERSION})",
                    result.ducktape_runner_result
                ));
            }
            Ok(result)
        }
        _ => Ok(RunnerResult {
            ducktape_runner_result: RUNNER_RESULT_VERSION,
            response_text: String::from_utf8_lossy(bytes).into_owned(),
            workspace_receipt: WorkspaceReceipt::default(),
            data: None,
            effects: Vec::new(),
            sink: WireSink::Chain,
            status: WireStatus::Ok,
        }),
    }
}

/// map host-assembled declarative effects into the validated [`AgentAction`]
/// vocabulary. v1 vocab == today's two task verbs (chat.post is the message
/// facet, not an effect). an UNKNOWN kind fails the run deterministically (R4)
/// — this is the concrete gate for any verb beyond the 3-verb set.
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
/// can set `workspace.source = prior.output_snapshot`.
#[derive(Serialize, Clone)]
struct OutputRef<'a> {
    source_prefix: &'a str,
    output_snapshot: &'a str,
    commit_height: Option<u64>,
    rebased: bool,
    no_changes: bool,
}

pub(super) fn encode_delivery_receipt(
    response: &AgentResponse,
    data: Option<&str>,
    receipt: &WorkspaceReceipt,
    status: WireStatus,
) -> String {
    let output_ref = receipt.output_snapshot.as_deref().map(|snap| OutputRef {
        source_prefix: &receipt.source_prefix,
        output_snapshot: snap,
        commit_height: receipt.commit_height,
        rebased: receipt.rebased,
        no_changes: receipt.no_changes,
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

// ---- forge sink wire (local mirrors) -----------------------------------------
// runs does NOT take a production dependency on the heavy `forge` crate (it
// pulls vendored libgit2). instead it mirrors the exact JSON shape forge decodes
// for the sink op it emits, and a dev-only conformance test pins the mirror
// against `forge::decode_msg` so the wire can't silently drift.

/// the exact `ForgeMsg::OpenPr` JSON the forge module decodes. only the PR sink
/// is emitted in v1 (the merge sink is inert), so only `OpenPr` is mirrored.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum ForgeSinkMsg<'a> {
    OpenPr {
        repo: &'a str,
        title: &'a str,
        body: &'a str,
        source_branch: &'a str,
        target_branch: &'a str,
    },
}

pub(super) fn forge_open_pr_bytes(
    repo: &str,
    title: &str,
    body: &str,
    src: &str,
    tgt: &str,
) -> Vec<u8> {
    serde_json::to_vec(&ForgeSinkMsg::OpenPr {
        repo,
        title,
        body,
        source_branch: src,
        target_branch: tgt,
    })
    .expect("forge sink msg serializes")
}

/// the `ForgeQuery::ListRefs` mirror the branch-born probe encodes.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum ForgeSinkQuery<'a> {
    ListRefs { repo: &'a str },
}

impl RunsModule {
    /// apply the O1/O2 sink. Chain is a breadcrumb/no-op in v1 (durable
    /// output_ref chaining is future work — the receipt already carries the
    /// output_ref for a downstream consumer). Pr emits a forge `OpenPr` gated on
    /// the agent's D3 `ForgePush` cap (Phase 4's `permits`, NOT a KNOWN_ACTIONS
    /// grant) and a committed-state branch-born probe (the no-fail rule: an
    /// OpenPr for an unborn branch would abort the block). Merge is inert in v1.
    /// any missing precondition degrades to a breadcrumb — the sink NEVER aborts
    /// the delivery block.
    pub(super) async fn emit_sink(
        &self,
        ctx: &mut dyn Ctx,
        run_id: &str,
        entry: &PendingState,
        sink: &WireSink,
    ) {
        match sink {
            WireSink::Chain => {}
            WireSink::Pr {
                repo,
                source_branch,
                target_branch,
                title,
                body,
            } => {
                // malformed pr sinks degrade to a breadcrumb.
                if repo.is_empty() || source_branch.is_empty() || target_branch.is_empty() {
                    return self.note(
                        ctx,
                        format!(
                            "run {run_id} pr sink skipped: incomplete pr sink (repo/source_branch/target_branch required)"
                        ),
                    );
                }
                let Some(forge) = self.forge.clone() else {
                    return self.note(
                        ctx,
                        format!("run {run_id} pr sink skipped: no forge module wired"),
                    );
                };
                let agent = match self.agent_record(&*ctx, &entry.agent_id).await {
                    Ok(Some(a)) => a,
                    _ => {
                        return self.note(
                            ctx,
                            format!("run {run_id} pr sink skipped: agent not registered"),
                        );
                    }
                };
                if !agent.permits(&CapRequest::ForgePush(repo.as_str())) {
                    return self.note(
                        ctx,
                        format!("run {run_id} pr sink skipped: agent lacks forge_push for {repo}"),
                    );
                }
                match self
                    .forge_branch_born(&*ctx, &forge, repo, source_branch)
                    .await
                {
                    Ok(true) => {}
                    Ok(false) => {
                        return self.note(
                            ctx,
                            format!("run {run_id} pr sink skipped: source branch not present"),
                        );
                    }
                    Err(why) => {
                        return self.note(ctx, format!("run {run_id} pr sink skipped: {why}"));
                    }
                }
                ctx.emit_msg(Msg {
                    target: forge,
                    payload: forge_open_pr_bytes(repo, title, body, source_branch, target_branch),
                });
            }
            WireSink::Merge { repo, number, .. } => {
                // v1: the merge sink needs a host-computed merge pack (a phase-2
                // wrapper responsibility). validate the wire, breadcrumb, and
                // fall through like Chain — never emit a MergePr yet.
                self.note(
                    ctx,
                    format!("run {run_id} merge sink for {repo}#{number} is inert in v1 (treated as chain)"),
                );
            }
        }
    }

    /// deterministic committed-state probe: is `branch` a born ref of `repo`?
    /// reads COMMITTED forge state via a query (never node-local pending), so it
    /// is uniform across validators. decoded via `serde_json::Value` to avoid a
    /// production dependency on the forge crate.
    async fn forge_branch_born(
        &self,
        ctx: &dyn Ctx,
        forge: &str,
        repo: &str,
        branch: &str,
    ) -> Result<bool, String> {
        let reply = ctx
            .query(
                forge,
                &serde_json::to_vec(&ForgeSinkQuery::ListRefs { repo }).expect("query serializes"),
            )
            .await
            .map_err(|e| format!("forge refs lookup failed: {e}"))?;
        let value: serde_json::Value =
            serde_json::from_slice(&reply).map_err(|e| format!("undecodable forge reply: {e}"))?;
        let Some(refs) = value.get("refs").and_then(|r| r.as_array()) else {
            return Err("unexpected forge reply for a refs listing".into());
        };
        Ok(refs
            .iter()
            .any(|r| r.get("name").and_then(|n| n.as_str()) == Some(branch)))
    }
}
