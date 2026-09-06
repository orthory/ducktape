use std::collections::BTreeMap;

use crate::{CapRequest, MAX_DUCKFS_WRITE_TEXT_BYTES};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use files::paths::canonical as canonical_duckfs_path;

use super::facets::{WireStatus, decode_run_result, encode_delivery_receipt, output_ref_of};
use super::{
    ACTION_CHAT_POST, ACTION_PAGES_COMMENT, AgentAction, AgentResponse, BTreeSet, Block, ChatMsg,
    ChatQuery, ChatReply, Ctx, DelegationResult, DelegationState, DelegationStatus, DispatchMsg,
    EntryInfo, Error, FilesChange, FilesContent, FilesMsg, FilesQuery, FilesReply,
    MAX_ACTIONS_BYTES, MAX_ACTIONS_PER_RUN, MAX_REPLY_BLOCKS_BYTES, MAX_THREAD_REPLIES,
    ModelRecord, Msg, Origin, PendingState, ReplyBlock, ResultEvent, RunOrigin, RunsModule,
    TaskMsg, TaskQuery, TaskReply, TaskStatus, chat_decode_reply, chat_encode_msg,
    chat_encode_query, dispatch_encode_msg, dispatch_id_for, files_decode_reply, files_encode_msg,
    files_encode_query, page_thread_id, reply_message_id, tasks_decode_reply, tasks_encode_msg,
    tasks_encode_query,
};
use super::{Lane, RunOutcome, RunRecord, post_message_id, sink};

// ---- response normalization ---------------------------------------------------------
// the dispatch-plane oracle returns the model's RAW text (opinion-free, Text
// contract); shaping it into an [`AgentResponse`] is deterministic string
// processing and therefore consensus work, done here in the result intake.

/// the reply-block kinds normalization keeps — the closed vocabulary the
/// strict-output instruction names.
const REPLY_KIND_PARAGRAPH: &str = "paragraph";
const REPLY_KIND_HEADING: &str = "heading";
pub(crate) const REPLY_KIND_CODE: &str = "code";

/// the model's raw answer as a NORMALIZED [`AgentResponse`]: the wire shape
/// when it parses (unknown kinds and empty texts drop), a plain paragraph
/// reply as the fallback for prose. job runs never carry reply blocks — there
/// is no channel to deliver them to.
pub(super) fn agent_response_from_text(text: &str, job_run: bool) -> AgentResponse {
    let parsed = parse_strict_response(text).unwrap_or_else(|| AgentResponse {
        reply_blocks: if job_run {
            Vec::new()
        } else {
            vec![paragraph_block(non_empty_text(text))]
        },
        actions: Vec::new(),
        commit_message: None,
    });
    normalize_response(parsed, text, job_run)
}

/// decode the strict-output contract's [`AgentResponse`] from a provider's
/// final message. the contract asks for a bare JSON object, but LLMs routinely
/// wrap it in a ```` ```json ```` markdown fence (agentic multi-turn CLIs
/// especially) or pad it with a line of prose — so parse tolerantly: bare
/// first, then de-fenced, then the outermost `{…}` span. without this a
/// perfectly well-formed reply reaches chat as a raw ```` ```json ```` code
/// block instead of the prose the model actually wrote.
pub(super) fn parse_strict_response(text: &str) -> Option<AgentResponse> {
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
    .find_map(|candidate| serde_json::from_str::<AgentResponse>(candidate.trim()).ok())
}

/// strip a single surrounding markdown code fence, returning the inner body.
/// tolerant of an info string (```` ```json ````) and of a missing close.
fn strip_code_fence(text: &str) -> Option<&str> {
    // the opening fence's info string runs to the first newline (```json\n…).
    let body = text.strip_prefix("```")?.split_once('\n').map(|(_, b)| b)?;
    let body = body.trim();
    Some(body.strip_suffix("```").unwrap_or(body).trim())
}

/// the span from the first `{` to the last `}` — JSON the model buried in
/// prose. required fields keep a non-object span from parsing; an object with
/// no known fields decodes empty and degrades to the raw-text paragraph, so
/// over-matching is harmless.
fn outermost_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    // lazily — a `}` before the first `{` gives start > end, and slicing that
    // range panics; `then` must not evaluate the slice unless the range holds.
    (start < end).then(|| &text[start..=end])
}

fn paragraph_block(text: String) -> ReplyBlock {
    ReplyBlock {
        kind: REPLY_KIND_PARAGRAPH.into(),
        text,
        lang: None,
    }
}

/// map a NORMALIZED response's reply blocks into chat blocks — the only place
/// the response vocabulary meets chat's. normalization guarantees only known
/// kinds and non-empty texts remain.
fn to_chat_blocks(blocks: &[ReplyBlock]) -> Vec<Block> {
    blocks
        .iter()
        .map(|block| match block.kind.as_str() {
            REPLY_KIND_CODE => Block::Code {
                lang: block.lang.clone().filter(|l| !l.is_empty()),
                text: block.text.clone(),
            },
            _ => Block::paragraph(block.text.clone()),
        })
        .collect()
}

fn to_page_comment_text(blocks: &[ReplyBlock]) -> String {
    blocks
        .iter()
        .map(|block| {
            if block.kind == REPLY_KIND_CODE {
                format!(
                    "```{}\n{}\n```",
                    block.lang.as_deref().unwrap_or(""),
                    block.text
                )
            } else {
                block.text.clone()
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn page_reply_comment_id(run_id: &str) -> String {
    format!("agent/{}/reply", crate::dispatch_id_for(run_id))
}

fn normalize_response(mut response: AgentResponse, raw_text: &str, job_run: bool) -> AgentResponse {
    // The host consumed this from the raw provider response before assembling
    // the runner result. It is not a consensus-delivery facet, and retaining
    // it here could needlessly inflate a job's bounded finalize payload.
    response.commit_message = None;
    // NOTHING is dropped from the action set here. normalization SHAPES a
    // response (unknown block kinds, empty texts); the caps are the validator's
    // to enforce, loudly — `validate_response` refuses an over-cap set by name
    // and the run fails with that sentence in its reply. this used to
    // `truncate(MAX_ACTIONS_PER_RUN)`, which silently dropped the tail (an
    // agent closing a run with twelve actions lost four, with no line in the
    // log, the run output or the reply) and made the validator's own count
    // check unreachable from the provider path.
    response.reply_blocks = response
        .reply_blocks
        .into_iter()
        .filter_map(|block| {
            let text = block.text.trim().to_string();
            if text.is_empty() {
                return None;
            }
            match block.kind.as_str() {
                REPLY_KIND_PARAGRAPH | REPLY_KIND_HEADING => Some(paragraph_block(text)),
                REPLY_KIND_CODE => Some(ReplyBlock {
                    kind: REPLY_KIND_CODE.into(),
                    text,
                    lang: block.lang.filter(|l| !l.is_empty()),
                }),
                _ => None,
            }
        })
        .collect();
    if job_run {
        response.reply_blocks.clear();
        return response;
    }
    if response.reply_blocks.is_empty() {
        response
            .reply_blocks
            .push(paragraph_block(non_empty_text(raw_text)));
    }
    let bytes =
        serde_json::to_vec(&to_chat_blocks(&response.reply_blocks)).expect("blocks serialize");
    if bytes.len() > MAX_REPLY_BLOCKS_BYTES {
        response.reply_blocks = vec![paragraph_block(crate::truncate_on_boundary(
            &non_empty_text(raw_text),
            MAX_REPLY_BLOCKS_BYTES / 4,
            "…",
        ))];
    }
    response
}

fn non_empty_text(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        "Done.".into()
    } else {
        trimmed.into()
    }
}

/// byte bound on the error excerpt a failure reply carries — same order as
/// the host's diagnostic excerpts (the provider bounds stderr to 400).
pub(super) const FAILURE_EXCERPT_BYTES: usize = 400;

/// a failed run's error as ONE bounded chat line: whitespace runs (newlines
/// included) collapse to single spaces, then the excerpt bound applies.
pub(super) fn failure_excerpt(reason: &str) -> String {
    let line = reason.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.is_empty() {
        return "no error detail".into();
    }
    crate::truncate_on_boundary(&line, FAILURE_EXCERPT_BYTES, "…")
}

/// the canonical state form of a dispatch origin (see [`RunOrigin`]). a
/// program account has no run standing of its own, so its origin is refused
/// rather than mapped.
pub(super) fn canonical_origin(origin: &Origin) -> Result<RunOrigin, Error> {
    match origin {
        Origin::External(key) => Ok(RunOrigin::External(key.clone())),
        Origin::Module(module) => Ok(RunOrigin::Module(module.clone())),
        Origin::Program(account) => Ok(RunOrigin::Program(*account)),
        Origin::System => Ok(RunOrigin::System),
    }
}

/// the wire name of a task status an [`AgentAction::UpdateTaskStatus`] carries.
fn task_status(name: &str) -> Option<TaskStatus> {
    match name {
        "open" => Some(TaskStatus::Open),
        "in_progress" => Some(TaskStatus::InProgress),
        "done" => Some(TaskStatus::Done),
        _ => None,
    }
}

/// whether the registry granted this agent an action name.
pub(super) fn allows(agent: &ModelRecord, action: &str) -> bool {
    agent.allowed_actions.iter().any(|a| a == action)
}

impl RunsModule {
    // ---- the result intake (origin == dispatch) ------------------------------------

    /// NO-FAIL ARM. the dispatch plane delivers a run's outcome here inside
    /// its delivery block; an `Err` would abort that block, the committed
    /// mailbox would re-inject next block, and every block after would abort
    /// (the permanent-abort loop). unknown dispatch ids are staged no-ops;
    /// the model's raw text is normalized deterministically, and a response
    /// that fails validation FAILS THE RUN — breadcrumb + pruned entry —
    /// never the block. the entry prunes on EVERY matched delivery: the
    /// dispatch module is the lifecycle ledger, this map is only the
    /// correlation for work still in flight.
    pub(super) async fn on_result_event(
        &mut self,
        ctx: &mut dyn Ctx,
        event: ResultEvent,
    ) -> Result<(), Error> {
        let Some(entry) = self.pending_entry(&event.dispatch_id).cloned() else {
            self.note(
                ctx,
                format!("dropped result for unknown dispatch {}", event.dispatch_id),
            );
            return Ok(());
        };
        let run_id = entry.run_id();
        let ResultEvent {
            dispatch_id,
            outcome,
            ..
        } = event;
        self.pending_overlay.insert(dispatch_id, None);
        // a session may NEVER outlive its run. this is the only place a pending
        // entry prunes — delivery, worker failure, timeout, and cancellation all
        // arrive as the one `ResultEvent` (cancel routes through the dispatch
        // plane, whose Err("cancelled") delivery lands right here) — so pruning
        // the session beside it is the whole close-out. an agent's key stops
        // being an authority in the same block its run stops existing.
        self.pending_sessions.insert(run_id.clone(), None);
        self.close_delegations_for_run(ctx, &run_id, &entry);

        match outcome {
            // THE single delivery path: decode the runner result and apply
            // whatever facets it carries. a plain (message-only) result carries
            // none — it delivers exactly the model prose + its parsed actions.
            Ok(bytes) if entry.delegation_id.is_some() => {
                self.deliver_delegated_result(ctx, &run_id, &entry, &bytes)
                    .await
            }
            Ok(bytes) => self.deliver_run_result(ctx, &run_id, &entry, &bytes).await,
            Err(reason) if entry.delegation_id.is_some() => {
                self.fail_delegated_run(ctx, &run_id, &entry, reason).await
            }
            Err(reason) => self.fail_run(ctx, &run_id, &entry, reason).await,
        }
        Ok(())
    }

    /// Cancel unfinished descendants when their caller exits. A root exit
    /// removes the complete ephemeral result tree; no ModelRecord relation is
    /// left behind.
    fn close_delegations_for_run(&mut self, ctx: &mut dyn Ctx, run_id: &str, entry: &PendingState) {
        let root_exit = entry.delegation_id.is_none();
        let ids = self.delegation_ids();
        let mut scoped = BTreeSet::new();
        if root_exit {
            for id in &ids {
                if self
                    .delegation(id)
                    .is_some_and(|state| state.view.root_run_id == run_id)
                {
                    scoped.insert(id.clone());
                }
            }
        } else {
            let mut exiting = BTreeSet::from([run_id.to_string()]);
            loop {
                let mut changed = false;
                for id in &ids {
                    let Some(state) = self.delegation(id) else {
                        continue;
                    };
                    if exiting.contains(&state.view.caller_run_id) && scoped.insert(id.clone()) {
                        exiting.insert(state.view.callee_run_id.clone());
                        changed = true;
                    }
                }
                if !changed {
                    break;
                }
            }
        }
        for id in scoped {
            let Some(mut state) = self.delegation(&id).cloned() else {
                continue;
            };
            if state.view.status == DelegationStatus::Pending {
                ctx.emit_msg(Msg {
                    target: self.dispatch.clone(),
                    payload: dispatch_encode_msg(&DispatchMsg::CancelDispatch {
                        dispatch_id: dispatch_id_for(&state.view.callee_run_id),
                    }),
                });
                self.pending_overlay
                    .insert(dispatch_id_for(&state.view.callee_run_id), None);
                self.pending_sessions
                    .insert(state.view.callee_run_id.clone(), None);
                state.view.status = DelegationStatus::Cancelled;
                state.view.completed_at = Some(ctx.env().consensus_time);
                state.view.result = Some(DelegationResult {
                    reply_blocks: Vec::new(),
                    output_ref: None,
                    error: Some("caller run exited before the callee settled".into()),
                });
                self.pending_delegations.insert(id.clone(), Some(state));
            }
            if root_exit {
                self.pending_delegations.insert(id, None);
            }
        }
    }

    async fn deliver_delegated_result(
        &mut self,
        ctx: &mut dyn Ctx,
        run_id: &str,
        entry: &PendingState,
        bytes: &[u8],
    ) {
        let result = match decode_run_result(bytes) {
            Ok(result) => result,
            Err(reason) => return self.fail_delegated_run(ctx, run_id, entry, reason).await,
        };
        if result.status == WireStatus::Failed {
            return self
                .fail_delegated_run(ctx, run_id, entry, "run reported a failed status".into())
                .await;
        }
        let response = agent_response_from_text(&result.response_text, false);
        let response = match self
            .validate_response(&*ctx, run_id, entry, Lane::DelegatedSettle, response)
            .await
        {
            Ok(response) => response,
            Err(reason) => return self.fail_delegated_run(ctx, run_id, entry, reason).await,
        };
        let reply_blocks = response.reply_blocks.clone();
        self.emit_pages_effects(ctx, run_id, entry, Lane::DelegatedSettle, &response.actions)
            .await;
        self.emit_duckfs_effects(ctx, run_id, entry, &response.actions)
            .await;
        self.emit_response(
            ctx,
            run_id,
            entry,
            Lane::DelegatedSettle,
            AgentResponse {
                reply_blocks: Vec::new(),
                ..response
            },
        )
        .await;
        let output_ref = output_ref_of(&result.workspace_receipt);
        self.complete_delegation(
            entry,
            DelegationStatus::Delivered,
            DelegationResult {
                reply_blocks,
                output_ref: output_ref.clone(),
                error: None,
            },
            ctx.env().consensus_time,
        );
        self.pending_history.push(RunRecord {
            run_id: run_id.to_string(),
            agent_id: entry.agent_id.clone(),
            channel_id: entry.channel_id.clone(),
            anchor_seq: entry.anchor_seq,
            outcome: RunOutcome::Delivered,
            degraded: result.status == WireStatus::Degraded,
            created_at: entry.created_at,
            delivered_at: ctx.env().consensus_time,
            executing_node: self.executing_node(&*ctx, run_id).await,
            output_ref,
            pr_number: None,
        });
    }

    async fn fail_delegated_run(
        &mut self,
        ctx: &mut dyn Ctx,
        run_id: &str,
        entry: &PendingState,
        reason: String,
    ) {
        let reason = failure_excerpt(&reason);
        self.note(ctx, format!("delegated run {run_id} failed: {reason}"));
        self.complete_delegation(
            entry,
            DelegationStatus::Failed,
            DelegationResult {
                reply_blocks: Vec::new(),
                output_ref: None,
                error: Some(reason),
            },
            ctx.env().consensus_time,
        );
        self.pending_history.push(RunRecord {
            run_id: run_id.to_string(),
            agent_id: entry.agent_id.clone(),
            channel_id: entry.channel_id.clone(),
            anchor_seq: entry.anchor_seq,
            outcome: RunOutcome::Failed,
            degraded: false,
            created_at: entry.created_at,
            delivered_at: ctx.env().consensus_time,
            executing_node: self.executing_node(&*ctx, run_id).await,
            output_ref: None,
            pr_number: None,
        });
    }

    fn complete_delegation(
        &mut self,
        entry: &PendingState,
        status: DelegationStatus,
        result: DelegationResult,
        completed_at: u64,
    ) {
        let Some(id) = entry.delegation_id.as_deref() else {
            return;
        };
        let Some(mut state): Option<DelegationState> = self.delegation(id).cloned() else {
            return;
        };
        // A caller-exit cancellation won the race; the late dispatch result may
        // prune its PendingRun but cannot resurrect a result nobody can collect.
        if state.view.status == DelegationStatus::Cancelled {
            return;
        }
        state.view.status = status;
        state.view.result = Some(result);
        state.view.completed_at = Some(completed_at);
        self.pending_delegations.insert(id.to_string(), Some(state));
    }

    /// the failure triple (breadcrumb note + threaded failure reply + job
    /// finalize false) — unchanged behavior, was inlined three times. also
    /// records the terminal run into the delivered-runs ring (derived state,
    /// observation only: nothing emitted changes).
    async fn fail_run(
        &mut self,
        ctx: &mut dyn Ctx,
        run_id: &str,
        entry: &PendingState,
        reason: String,
    ) {
        self.note(ctx, format!("run {run_id} failed: {reason}"));
        self.emit_failure_reply(ctx, run_id, entry, &reason).await;
        let executing_node = self.executing_node(&*ctx, run_id).await;
        self.pending_history.push(RunRecord {
            run_id: run_id.to_string(),
            agent_id: entry.agent_id.clone(),
            channel_id: entry.channel_id.clone(),
            anchor_seq: entry.anchor_seq,
            outcome: RunOutcome::Failed,
            degraded: false,
            created_at: entry.created_at,
            delivered_at: ctx.env().consensus_time,
            executing_node,
            output_ref: None,
            pr_number: None,
        });
        self.emit_job_finalize_if_current_claimant(ctx, entry, false, reason)
            .await;
    }

    /// THE single delivery path. the model prose normalizes into one
    /// [`AgentResponse`] (validate/emit reused); the sink is applied (cap-gated,
    /// probe-guarded, degrades to a breadcrumb, never aborts); artifact (O1) +
    /// status fold into the faceted finalize payload. a plain (message-only)
    /// wrapper — prose, no facets — delivers exactly the model prose + its
    /// parsed actions; marker-less bytes FAIL the run (flag day, the flat
    /// tolerance is gone). idempotent by run_id — every effect applies once,
    /// here, from the winning attempt (X2); nothing is emitted mid-run.
    async fn deliver_run_result(
        &mut self,
        ctx: &mut dyn Ctx,
        run_id: &str,
        entry: &PendingState,
        bytes: &[u8],
    ) {
        let result = match decode_run_result(bytes) {
            Ok(r) => r,
            Err(reason) => return self.fail_run(ctx, run_id, entry, reason).await,
        };
        // the host observation overrides a present message facet (R4).
        if result.status == WireStatus::Failed {
            return self
                .fail_run(ctx, run_id, entry, "run reported a failed status".into())
                .await;
        }
        let response = agent_response_from_text(&result.response_text, entry.job_id.is_some());
        let response = match self
            .validate_response(&*ctx, run_id, entry, Lane::Settle, response)
            .await
        {
            Ok(r) => r,
            Err(reason) => return self.fail_run(ctx, run_id, entry, reason).await,
        };
        // build the faceted finalize payload — and render the message facet
        // the PR sink derives its title/body from — BEFORE moving `response`
        // into emit_response; emission order is response → sink → finalize.
        let payload = encode_delivery_receipt(&response, &result.workspace_receipt, result.status);
        let message = sink::message_facet_text(&response.reply_blocks);
        // the run's durable executor attribution (sink.rs's saga lookup) —
        // computed once here, shared by the PR-body breadcrumb and the ring.
        let executing_node = self.executing_node(&*ctx, run_id).await;
        // the pages effects lane: applied here at the run boundary like every
        // other effect, but probe-guarded and cap-gated per action — a bad
        // pages action degrades to a breadcrumb, the run still delivers. the
        // duckfs write lane is the same shape: a stale per-path base degrades
        // alone, it never costs the response its reply or its other actions.
        self.emit_pages_effects(ctx, run_id, entry, Lane::Settle, &response.actions)
            .await;
        self.emit_duckfs_effects(ctx, run_id, entry, &response.actions)
            .await;
        self.emit_response(ctx, run_id, entry, Lane::Settle, response)
            .await;
        let pr_number = self
            .emit_sink(
                ctx,
                run_id,
                entry,
                &result.sink,
                &message,
                &result.workspace_receipt,
                &executing_node,
            )
            .await;
        // record the delivery into the ring AFTER the sink so the record can
        // carry the PR number the sink opened/updated. observation only —
        // every emitted op above is byte-identical with or without it.
        self.pending_history.push(RunRecord {
            run_id: run_id.to_string(),
            agent_id: entry.agent_id.clone(),
            channel_id: entry.channel_id.clone(),
            anchor_seq: entry.anchor_seq,
            outcome: RunOutcome::Delivered,
            degraded: result.status == WireStatus::Degraded,
            created_at: entry.created_at,
            delivered_at: ctx.env().consensus_time,
            executing_node,
            output_ref: output_ref_of(&result.workspace_receipt),
            pr_number,
        });
        self.emit_job_finalize_if_current_claimant(ctx, entry, true, payload)
            .await;
    }

    /// deterministic response validation — THE safety boundary (design §5).
    /// the response is data until every check here passes; only then do its
    /// follow-ups exist. beyond grants and caps, this probes everything the
    /// emitted follow-ups could make chat or tasks REJECT (which would abort
    /// the delivery block — the no-fail rule): a squatted reply message id, a
    /// full thread, a duplicate, over-cap, or unknown task id.
    ///
    /// every probe reads COMMITTED state, so a check on a SHARED, countable
    /// resource must also count what this same response already staged — the
    /// duplicate-task-id set below, and the thread-reply counter (the peer of
    /// the pages lane's `already_staged`). without that, two posts into a
    /// thread one reply short of the cap both pass the committed probe and the
    /// SECOND is rejected at apply — aborting the delivery block forever.
    ///
    /// THE ONE definition of what an agent may do. the session lane
    /// ([`super::sessions`]) validates each mid-run action by calling this with
    /// a one-action response under [`Lane::Session`], so the tool plane can
    /// never become a second, wider permission vocabulary: if these two ever
    /// disagreed, the disagreement WOULD be the hole this design exists to
    /// close.
    pub(super) async fn validate_response(
        &self,
        ctx: &dyn Ctx,
        run_id: &str,
        entry: &PendingState,
        lane: Lane,
        response: AgentResponse,
    ) -> Result<AgentResponse, String> {
        let agent = self
            .agent_for_run(ctx, entry)
            .await?
            .ok_or_else(|| format!("agent is not registered: {}", entry.agent_id))?;
        if response.reply_blocks.is_empty() && response.actions.is_empty() {
            return Err("response carries neither reply blocks nor actions".into());
        }
        if response.actions.len() > MAX_ACTIONS_PER_RUN {
            return Err(format!(
                "{} actions exceed the cap of {MAX_ACTIONS_PER_RUN}",
                response.actions.len()
            ));
        }
        // the byte peer of the count cap: action payloads are unbounded strings,
        // and the finalize payload embeds the validated response — prove the size
        // BEFORE emitting, exactly like the reply-blocks cap below.
        let actions_bytes = serde_json::to_vec(&response.actions)
            .expect("actions are serializable")
            .len();
        if actions_bytes > MAX_ACTIONS_BYTES {
            return Err(format!(
                "actions are {actions_bytes} bytes; the cap is {MAX_ACTIONS_BYTES}"
            ));
        }

        // the thread posts THIS response has already staged, per (channel,
        // root) — chat's reply cap is the one countable resource a single
        // response can now consume TWICE (the reply plus a `chat.post_message`,
        // or two of them). the emitted follow-ups all land in ONE delivery
        // block, but each probe below reads committed state only, so without
        // this counter both posts pass at 4095 replies, chat REJECTS the second
        // at apply, and the block aborts — forever (the mailbox re-injects it).
        // counted in EMISSION order: the run's own reply first, then the
        // actions by index, exactly as `emit_response` emits them.
        let mut staged_replies: BTreeMap<(String, u64), u64> = BTreeMap::new();
        // the requester's post standing per channel a `chat.post_message`
        // action names — see `requester_may_post`. one response may carry many
        // posts into the same channel and the answer cannot change mid-pass,
        // so each channel is asked exactly once.
        let mut post_standing: BTreeMap<String, bool> = BTreeMap::new();

        if !response.reply_blocks.is_empty() {
            if matches!(lane, Lane::DelegatedSettle) {
                let reply_bytes = serde_json::to_vec(&response.reply_blocks)
                    .expect("reply blocks are serializable");
                if reply_bytes.len() > MAX_REPLY_BLOCKS_BYTES {
                    return Err(format!(
                        "delegated result is {} bytes; the cap is {MAX_REPLY_BLOCKS_BYTES}",
                        reply_bytes.len()
                    ));
                }
            } else {
                let page_run = page_thread_id(&entry.channel_id).is_some();
                let action = if page_run {
                    ACTION_PAGES_COMMENT
                } else {
                    ACTION_CHAT_POST
                };
                if !allows(&agent, action) {
                    return Err(format!(
                        "agent {} is not allowed to {action}",
                        entry.agent_id
                    ));
                }
                let reply_bytes = if page_run {
                    to_page_comment_text(&response.reply_blocks).into_bytes()
                } else {
                    serde_json::to_vec(&to_chat_blocks(&response.reply_blocks))
                        .expect("blocks are serializable")
                };
                if reply_bytes.len() > MAX_REPLY_BLOCKS_BYTES {
                    return Err(format!(
                        "reply blocks are {} bytes; the cap is {MAX_REPLY_BLOCKS_BYTES}",
                        reply_bytes.len()
                    ));
                }
                if page_run {
                    self.page_reply_msg(ctx, run_id, entry, &response.reply_blocks)
                        .await?;
                } else {
                    self.probe_reply_postable(ctx, run_id, entry).await?;
                    if let Some(root) = entry.thread_root {
                        *staged_replies
                            .entry((entry.channel_id.clone(), root))
                            .or_default() += 1;
                    }
                }
            }
        }

        // the strict all-or-nothing lane covers the CHAT-POST and TASK verbs.
        // the pages actions and the duckfs write are deliberately NOT validated
        // here: they gate and validate at apply (`emit_pages_effects`,
        // `emit_duckfs_effects`), where a bad one degrades ALONE with a
        // breadcrumb instead of failing the whole run — a stale write base is a
        // fact about a shared, concurrently-written filesystem, not a defect in
        // this response, so it must never cost the response its reply.
        //
        // the tasks arms probe BY ID (`task_exists`), so a response carrying no
        // task action never touches the tasks module at all.
        let mut created: BTreeSet<&str> = BTreeSet::new();
        for (index, action) in response.actions.iter().enumerate() {
            if super::pages_effects::is_pages_action(action) || is_duckfs_action(action) {
                continue;
            }
            let name = action.vocabulary_name();
            if !allows(&agent, name) {
                return Err(format!("agent {} is not allowed to {name}", entry.agent_id));
            }
            match action {
                // an agent SPEAKING — its own channel, its own moment — as
                // opposed to reply_blocks, which only answer where the agent was
                // engaged. that is the wider power, so it rides its own grant
                // (`chat.post_message`): holding `chat.post` must NEVER widen
                // into it, or every already-registered agent would have been
                // silently handed the wider one.
                AgentAction::PostMessage {
                    channel_id,
                    text,
                    thread,
                } => {
                    if text.trim().is_empty() {
                        return Err("chat.post_message requires a non-empty text".into());
                    }
                    // the whole actions vec is already bounded by
                    // MAX_ACTIONS_BYTES above, and every post writes its OWN
                    // message record, so no number of posts can push one head
                    // past chat's MAX_MESSAGE_HEAD_BYTES.
                    self.probe_channel_exists(ctx, channel_id).await?;
                    if !self
                        .requester_may_post(ctx, entry, channel_id, &mut post_standing)
                        .await?
                    {
                        return Err(format!(
                            "the run's requester may not post to channel: {channel_id}"
                        ));
                    }
                    // the thread cap is the one check a sibling post can move
                    // out from under: fold in what this response already staged
                    // into the same thread, then count this post as staged.
                    let thread_key = thread.map(|root| (channel_id.clone(), root));
                    let already_staged = thread_key
                        .as_ref()
                        .and_then(|key| staged_replies.get(key))
                        .copied()
                        .unwrap_or(0);
                    self.probe_post_lands(
                        ctx,
                        channel_id,
                        &post_message_id(run_id, &lane.slot(index)),
                        *thread,
                        already_staged,
                    )
                    .await?;
                    if let Some(key) = thread_key {
                        *staged_replies.entry(key).or_default() += 1;
                    }
                }
                AgentAction::CreateTask { task_id, title } => {
                    if task_id.is_empty() || title.is_empty() {
                        return Err("task actions require a non-empty task_id and title".into());
                    }
                    // tasks' OWN admission rule for an id, applied with tasks'
                    // OWN call so the two can never drift: at most
                    // MAX_TASK_ID bytes, and free of the reserved KEY_SEP. a
                    // model-authored id is bounded only by MAX_ACTIONS_BYTES,
                    // so without this an id tasks REJECTS at apply aborts the
                    // whole settle op instead of failing the run.
                    sdk::validate_id("task_id", task_id, tasks::MAX_TASK_ID)
                        .map_err(|e| e.to_string())?;
                    // duplicates — committed or earlier in this very
                    // response — would make tasks reject the follow-up.
                    let on_board = self.task_exists(ctx, task_id).await?;
                    if on_board || !created.insert(task_id) {
                        return Err(format!("task already exists: {task_id}"));
                    }
                }
                AgentAction::UpdateTaskStatus { task_id, status } => {
                    if task_status(status).is_none() {
                        return Err(format!("unknown task status: {status}"));
                    }
                    let staged_here = created.contains(task_id.as_str());
                    if !staged_here && !self.task_exists(ctx, task_id).await? {
                        return Err(format!("unknown task: {task_id}"));
                    }
                }
                AgentAction::DuckfsWriteText { .. } => {
                    unreachable!("duckfs actions are skipped above")
                }
                AgentAction::AddPageComment { .. } | AgentAction::SetPageChecked { .. } => {
                    unreachable!("pages actions are skipped above")
                }
            }
        }

        Ok(response)
    }

    /// prove a reply under the run's message id could land in chat RIGHT NOW
    /// — the no-fail rule again: an emitted post must be valid by
    /// construction, so anything chat would reject is probed first. the run's
    /// own channel is where its anchor came from, so only the post itself needs
    /// probing.
    ///
    /// the reply is always the FIRST post a response stages (`emit_response`
    /// emits it before any action, and a failure reply is the only post its run
    /// makes), so it never has a sibling to account for: it probes at zero
    /// already-staged, and its caller counts it for the actions that follow.
    async fn probe_reply_postable(
        &self,
        ctx: &dyn Ctx,
        run_id: &str,
        entry: &PendingState,
    ) -> Result<(), String> {
        self.probe_post_lands(
            ctx,
            &entry.channel_id,
            &reply_message_id(run_id),
            entry.thread_root,
            0,
        )
        .await
    }

    /// THE chat-post probe, shared by the run's reply and by a
    /// `chat.post_message` action: everything chat would REJECT about the post
    /// we are about to emit, checked against committed state first — a squatted
    /// message id (ids are client-chosen, so anyone can take one), a thread root
    /// that does not exist, one that is itself a reply (chat forbids
    /// subthreads), and a full thread.
    ///
    /// a `chat.post_message` additionally names its OWN channel (the reply's is
    /// the run's), which is probed by the caller's `require_channel` peer below.
    ///
    /// `already_staged` is how many posts this same response has already staged
    /// into THIS thread — the same-block thread-cap accounting, because chat's
    /// `reply_count` here is the COMMITTED one and the siblings staged in this
    /// block are invisible to it (the pages lane's `already_staged` peer). the
    /// message id needs no such counter: it is minted per lane SLOT
    /// ([`post_message_id`]) and the reply's is distinct again, so a response's
    /// own posts can never collide on one — only a squatter can, which the
    /// committed lookup already catches.
    async fn probe_post_lands(
        &self,
        ctx: &dyn Ctx,
        channel_id: &str,
        message_id: &str,
        thread: Option<u64>,
        already_staged: u64,
    ) -> Result<(), String> {
        if channel_id.is_empty() {
            return Err("chat posts require a non-empty channel_id".into());
        }
        let reply = ctx
            .query(
                &self.chat,
                &chat_encode_query(&ChatQuery::Message {
                    message_id: message_id.to_string(),
                }),
            )
            .await
            .map_err(|e| format!("chat message lookup failed: {e}"))?;
        match chat_decode_reply(&reply) {
            Ok(ChatReply::Message(None)) => {}
            Ok(ChatReply::Message(Some(_))) => {
                return Err(format!("reply message id already taken: {message_id}"));
            }
            _ => return Err("unexpected chat reply for a message lookup".into()),
        }
        // a threaded post must anchor on a real ROOT and still fit under chat's
        // thread cap.
        if let Some(root_seq) = thread {
            let reply = ctx
                .query(
                    &self.chat,
                    &chat_encode_query(&ChatQuery::MessagesRange {
                        channel_id: channel_id.to_string(),
                        from_seq: root_seq,
                        limit: 1,
                    }),
                )
                .await
                .map_err(|e| format!("chat thread lookup failed: {e}"))?;
            let Ok(ChatReply::Messages(views)) = chat_decode_reply(&reply) else {
                return Err("unexpected chat reply for a thread lookup".into());
            };
            let root = views
                .first()
                .filter(|v| v.seq == root_seq)
                .ok_or_else(|| format!("thread root does not exist: {root_seq}"))?;
            if root.head.thread.is_some() {
                return Err(format!(
                    "thread replies cannot start subthreads: {channel_id}/{root_seq}"
                ));
            }
            if root.head.reply_count + already_staged >= MAX_THREAD_REPLIES as u64 {
                return Err(format!("thread reply cap reached: {channel_id}/{root_seq}"));
            }
        }
        Ok(())
    }

    /// Build a Pages reply only after proving every rejection surface: thread
    /// and target still exist, reply id is free, thread has room, and the
    /// agent holds pages_write for the owning page.
    async fn page_reply_msg(
        &self,
        ctx: &dyn Ctx,
        run_id: &str,
        entry: &PendingState,
        blocks: &[ReplyBlock],
    ) -> Result<Msg, String> {
        let pages = self
            .pages
            .as_deref()
            .ok_or_else(|| "pages module is not configured".to_string())?;
        let thread_id = page_thread_id(&entry.channel_id)
            .ok_or_else(|| "run is not a pages comment run".to_string())?;
        let reply = ctx
            .query(
                pages,
                &pages::encode_query(&pages::PageQuery::CommentThread {
                    thread_id: thread_id.to_string(),
                }),
            )
            .await
            .map_err(|e| format!("pages thread lookup failed: {e}"))?;
        let view = match pages::decode_reply(&reply) {
            Ok(pages::PageReply::CommentThread(Some(view))) => view,
            _ => return Err(format!("pages thread is missing: {thread_id}")),
        };
        if view.thread.comment_ids.len() >= pages::MAX_COMMENTS_PER_THREAD {
            return Err(format!("pages comment thread is full: {thread_id}"));
        }
        let target_reply = ctx
            .query(
                pages,
                &pages::encode_query(&pages::PageQuery::GetBlock {
                    block_id: view.thread.target.clone(),
                }),
            )
            .await
            .map_err(|e| format!("pages target lookup failed: {e}"))?;
        let target = match pages::decode_reply(&target_reply) {
            Ok(pages::PageReply::Block(Some(block))) => block,
            _ => return Err(format!("pages target is missing: {}", view.thread.target)),
        };
        let agent = self
            .agent_for_run(ctx, entry)
            .await?
            .ok_or_else(|| format!("agent is not registered: {}", entry.agent_id))?;
        self.check_pages_write(&agent, &target.page)?;
        let comment_id = page_reply_comment_id(run_id);
        if comment_id.len() > pages::MAX_COMMENT_ID_BYTES || !pages::id_is_index_safe(&comment_id) {
            return Err("derived pages reply id is invalid".into());
        }
        let existing = ctx
            .query(
                pages,
                &pages::encode_query(&pages::PageQuery::GetComment {
                    comment_id: comment_id.clone(),
                }),
            )
            .await
            .map_err(|e| format!("pages comment lookup failed: {e}"))?;
        match pages::decode_reply(&existing) {
            Ok(pages::PageReply::Comment(None)) => {}
            Ok(pages::PageReply::Comment(Some(_))) => {
                return Err(format!("pages reply id already taken: {comment_id}"));
            }
            _ => return Err("unexpected pages reply for a comment lookup".into()),
        }
        let text = to_page_comment_text(blocks);
        if text.len() > pages::MAX_COMMENT_TEXT_BYTES {
            return Err(format!(
                "pages reply is {} bytes; the cap is {}",
                text.len(),
                pages::MAX_COMMENT_TEXT_BYTES
            ));
        }
        Ok(Msg {
            target: pages.to_string(),
            payload: pages::encode_msg(&pages::PageMsg::AddComment {
                thread_id: thread_id.to_string(),
                comment_id,
                target: view.thread.target,
                text,
                anchor: None,
                mentions: Vec::new(),
            }),
        })
    }

    /// Both the model user and an explicit external requester must retain
    /// posting access to a tool action's destination. Targets independently
    /// apply their normal Program account gate when the action actually runs.
    async fn requester_may_post(
        &self,
        ctx: &dyn Ctx,
        entry: &PendingState,
        channel_id: &str,
        known: &mut BTreeMap<String, bool>,
    ) -> Result<bool, String> {
        if let Some(answer) = known.get(channel_id) {
            return Ok(*answer);
        }
        let bytes = ctx
            .query(
                &self.chat,
                &chat_encode_query(&ChatQuery::Access {
                    channel_id: channel_id.into(),
                    party: chat::Party::Account(entry.account),
                }),
            )
            .await
            .map_err(|error| error.to_string())?;
        let ChatReply::Access(access) =
            chat_decode_reply(&bytes).map_err(|error| error.to_string())?
        else {
            return Err("unexpected chat access reply".into());
        };
        let requester_allowed = match &entry.requester {
            RunOrigin::External(key) => {
                !key.is_empty() && self.may_post(ctx, key, channel_id).await?
            }
            RunOrigin::Program(_) | RunOrigin::Module(_) | RunOrigin::System => true,
        };
        let may_post = access.may_post && requester_allowed;
        known.insert(channel_id.to_string(), may_post);
        Ok(may_post)
    }

    /// prove a channel EXISTS before an agent speaks into it — chat rejects a
    /// post to an unknown channel, and on the settle path that rejection would
    /// abort the delivery block. existence ONLY: chat admits a module/agent
    /// author into any channel it holds, which is exactly why the standing gate
    /// ([`RunsModule::requester_may_post`]) has to run beside this probe.
    async fn probe_channel_exists(&self, ctx: &dyn Ctx, channel_id: &str) -> Result<(), String> {
        let reply = ctx
            .query(
                &self.chat,
                &chat_encode_query(&ChatQuery::Channel {
                    channel_id: channel_id.to_string(),
                }),
            )
            .await
            .map_err(|e| format!("chat channel lookup failed: {e}"))?;
        match chat_decode_reply(&reply) {
            Ok(ChatReply::Channel(Some(_))) => Ok(()),
            Ok(ChatReply::Channel(None)) => Err(format!("unknown channel: {channel_id}")),
            _ => Err("unexpected chat reply for a channel lookup".into()),
        }
    }

    /// resolve the committed entry at `path` under one snapshot: `None` is
    /// files' OWN meaning for "no snapshot" everywhere else it appears as a
    /// query argument — the CURRENT head, not the empty tree (that inversion
    /// is `FilesMsg::Commit`'s `base_snapshot`-only special case, handled by
    /// the caller, never here).
    async fn duckfs_stat(
        &self,
        ctx: &dyn Ctx,
        files: &str,
        path: &str,
        snapshot: Option<String>,
    ) -> Result<Option<EntryInfo>, String> {
        let reply = ctx
            .query(
                files,
                &files_encode_query(&FilesQuery::Stat {
                    path: path.to_string(),
                    snapshot,
                }),
            )
            .await
            .map_err(|e| format!("files stat query failed: {e}"))?;
        match files_decode_reply(&reply) {
            Ok(FilesReply::Stat(info)) => Ok(info),
            Ok(_) => Err("unexpected files reply for a stat query".into()),
            Err(e) => Err(format!("files stat reply failed to decode: {e}")),
        }
    }

    /// predict whether files' own per-path CAS (`fs.rs` step 7: `entry_at(base)
    /// != entry_at(head)` -> "changed since base") would accept this write —
    /// the ONE base rule; there is no global-head check here on purpose, since
    /// files never applies one and a global check refuses disjoint-path writes
    /// files itself would take. `base_snapshot: None` is files' create-only
    /// sense (the empty tree, matching `FilesMsg::Commit`'s own doc comment):
    /// the path must not already exist. `Some(snapshot)` resolves that
    /// snapshot's entry at `path` and requires it match the current one.
    async fn probe_duckfs_write_base(
        &self,
        ctx: &dyn Ctx,
        files: &str,
        path: &str,
        base_snapshot: &Option<String>,
    ) -> Result<(), String> {
        let base_entry = match base_snapshot {
            None => None,
            Some(snapshot) => {
                self.duckfs_stat(ctx, files, path, Some(snapshot.clone()))
                    .await?
            }
        };
        let current_entry = self.duckfs_stat(ctx, files, path, None).await?;
        if base_entry != current_entry {
            return Err(format!(
                "duckfs.write_text base snapshot is stale for {path}"
            ));
        }
        Ok(())
    }

    /// ONE duckfs.write_text action as an emit-ready follow-up, or the reason
    /// it must not be emitted — the same shape as `pages_action_msg`: gate
    /// order grant -> shape/cap/permission -> the per-path base probe. `Err`
    /// degrades to a breadcrumb on the settle path and returns to the
    /// submitter on the session lane; same verdict, two failure policies.
    pub(super) async fn duckfs_write_msg(
        &self,
        ctx: &dyn Ctx,
        agent: &ModelRecord,
        action: &AgentAction,
    ) -> Result<Msg, String> {
        let AgentAction::DuckfsWriteText {
            path,
            text,
            base_snapshot,
        } = action
        else {
            unreachable!("only the duckfs action reaches this lane");
        };
        let name = action.vocabulary_name();
        if !allows(agent, name) {
            return Err(format!("agent {} is not allowed to {name}", agent.agent_id));
        }
        validate_duckfs_text_write(agent, path, text)?;
        let files = self
            .files
            .as_ref()
            .ok_or_else(|| "no files module is configured".to_string())?;
        self.probe_duckfs_write_base(ctx, files, path, base_snapshot)
            .await?;
        Ok(Msg {
            target: files.clone(),
            payload: files_encode_msg(&FilesMsg::Commit {
                base_snapshot: base_snapshot.clone(),
                message: "agent duckfs.write_text".into(),
                changes: vec![FilesChange::Put {
                    path: path.clone(),
                    exec: false,
                    meta: BTreeMap::new(),
                    content: FilesContent::Inline {
                        b64: STANDARD.encode(text.as_bytes()),
                    },
                }],
            }),
        })
    }

    /// apply the duckfs.write_text actions of a validated response — its own
    /// lane, mirroring `emit_pages_effects`: each action either emits its files
    /// follow-up or degrades to a breadcrumb. never errors, never fails the
    /// run — a base gone stale under a concurrent commit is expected traffic
    /// on a shared filesystem, not a reason to discard the reply and every
    /// other effect this response staged.
    pub(super) async fn emit_duckfs_effects(
        &self,
        ctx: &mut dyn Ctx,
        run_id: &str,
        entry: &PendingState,
        actions: &[AgentAction],
    ) {
        if !actions.iter().any(is_duckfs_action) {
            return;
        }
        let skip = |what: &str| format!("run {run_id} duckfs.write_text action skipped: {what}");
        let agent = match self.agent_for_run(&*ctx, entry).await {
            Ok(Some(a)) => a,
            _ => {
                self.note(ctx, skip("agent not registered"));
                return;
            }
        };
        for (index, action) in actions.iter().enumerate() {
            if !is_duckfs_action(action) {
                continue;
            }
            match self.duckfs_write_msg(&*ctx, &agent, action).await {
                Ok(msg) => ctx.emit_msg(msg),
                Err(why) => self.note(
                    ctx,
                    format!("run {run_id} duckfs.write_text action {index} skipped: {why}"),
                ),
            }
        }
    }

    /// surface a failed CHAT run as a threaded reply authored by the agent —
    /// same message id as a success reply would use, so the one-reply-per-run
    /// dedup holds and a redelivered result (entry already pruned) can never
    /// double-post. anything that keeps the post from being valid by
    /// construction (job run, unregistered agent, missing chat.post grant,
    /// squatted id, full thread) degrades to the pre-existing breadcrumb-only
    /// silence — never an error on this no-fail arm.
    async fn emit_failure_reply(
        &self,
        ctx: &mut dyn Ctx,
        run_id: &str,
        entry: &PendingState,
        reason: &str,
    ) {
        if entry.job_id.is_some() {
            // job runs have no channel; the finalize payload carries the error.
            return;
        }
        match self.failure_reply(&*ctx, run_id, entry, reason).await {
            Ok(msg) => ctx.emit_msg(msg),
            Err(why) => self.note(ctx, format!("run {run_id} failure not surfaced: {why}")),
        }
    }

    /// the failure post, or the reason it must stay unposted.
    async fn failure_reply(
        &self,
        ctx: &dyn Ctx,
        run_id: &str,
        entry: &PendingState,
        reason: &str,
    ) -> Result<Msg, String> {
        let agent = self
            .agent_for_run(ctx, entry)
            .await?
            .ok_or_else(|| format!("agent is not registered: {}", entry.agent_id))?;
        let page_run = page_thread_id(&entry.channel_id).is_some();
        // posting the failure is a reply like any success — ungranted
        // agents keep the old silent-fail.
        let action = if page_run {
            ACTION_PAGES_COMMENT
        } else {
            ACTION_CHAT_POST
        };
        if !allows(&agent, action) {
            return Err(format!(
                "agent {} is not allowed to {action}",
                entry.agent_id
            ));
        }
        let name = if agent.display_name.is_empty() {
            agent.agent_id.as_str()
        } else {
            agent.display_name.as_str()
        };
        let text = format!("⚠ {name} failed: {}", failure_excerpt(reason));
        if page_run {
            return self
                .page_reply_msg(
                    ctx,
                    run_id,
                    entry,
                    &[ReplyBlock {
                        kind: REPLY_KIND_PARAGRAPH.into(),
                        text,
                        lang: None,
                    }],
                )
                .await;
        }
        self.probe_reply_postable(ctx, run_id, entry).await?;
        Ok(Msg {
            target: self.chat.clone(),
            payload: chat_encode_msg(&ChatMsg::PostMessage {
                channel_id: entry.channel_id.clone(),
                message_id: reply_message_id(run_id),
                blocks: vec![Block::paragraph(text)],
                thread: entry.thread_root,
            }),
        })
    }

    /// does `task_id` name a live task RIGHT NOW — this block's staged creates
    /// included (the tasks board answers both its reads through its overlay)?
    ///
    /// a BY-ID read, never the board. a validated response carries at most
    /// [`MAX_ACTIONS_PER_RUN`] actions, so a settle spends at most that many
    /// point reads here; the whole-board walk this replaced cost one store read
    /// PER TASK, and blew the wasm host's 4096-read budget for good once the
    /// board outgrew it — every settle failing from then on, with no delete op
    /// to shrink the board back.
    async fn task_exists(&self, ctx: &dyn Ctx, task_id: &str) -> Result<bool, String> {
        let tasks = self
            .tasks
            .clone()
            .ok_or_else(|| "no tasks module is configured".to_string())?;
        let req = tasks_encode_query(&TaskQuery::Get {
            task_id: task_id.to_string(),
        });
        let reply = ctx
            .query(&tasks, &req)
            .await
            .map_err(|e| format!("tasks lookup failed: {e}"))?;
        match tasks_decode_reply(&reply) {
            Ok(TaskReply::Task(task)) => Ok(task.is_some()),
            Ok(TaskReply::Tasks(_) | TaskReply::OwnerOpenCount(_)) => {
                Err("tasks answered a page, not a task".into())
            }
            Err(e) => Err(format!("undecodable tasks reply: {e}")),
        }
    }

    /// Prepare validated chat, page and task intents. The result/session
    /// boundary records them for the account's program to execute.
    pub(super) async fn emit_response(
        &self,
        ctx: &mut dyn Ctx,
        run_id: &str,
        entry: &PendingState,
        lane: Lane,
        response: AgentResponse,
    ) {
        if !response.reply_blocks.is_empty() {
            if page_thread_id(&entry.channel_id).is_some() {
                match self
                    .page_reply_msg(&*ctx, run_id, entry, &response.reply_blocks)
                    .await
                {
                    Ok(msg) => ctx.emit_msg(msg),
                    Err(why) => self.note(ctx, format!("run {run_id} page reply skipped: {why}")),
                }
            } else {
                ctx.emit_msg(Msg {
                    target: self.chat.clone(),
                    payload: chat_encode_msg(&ChatMsg::PostMessage {
                        channel_id: entry.channel_id.clone(),
                        message_id: reply_message_id(run_id),
                        blocks: to_chat_blocks(&response.reply_blocks),
                        thread: entry.thread_root,
                    }),
                });
            }
        }
        for (index, action) in response.actions.into_iter().enumerate() {
            let msg = match action {
                AgentAction::PostMessage {
                    channel_id,
                    text,
                    thread,
                } => Msg {
                    target: self.chat.clone(),
                    payload: chat_encode_msg(&ChatMsg::PostMessage {
                        channel_id,
                        message_id: post_message_id(run_id, &lane.slot(index)),
                        blocks: vec![Block::paragraph(text)],
                        thread,
                    }),
                },
                AgentAction::CreateTask { task_id, title } => Msg {
                    target: self.task_target(),
                    // runs' own task creation stays owned by this module's
                    // id ("runs"), unchanged — see #1740's scope note: the
                    // requester identity plumbing would ripple through every
                    // caller of `emit_response`'s test expectations, so it is
                    // left alone here (only automations' rule-owner
                    // attribution was in scope).
                    payload: tasks_encode_msg(&TaskMsg::CreateTask {
                        task_id,
                        title,
                        owner: None,
                    }),
                },
                AgentAction::UpdateTaskStatus { task_id, status } => Msg {
                    target: self.task_target(),
                    payload: tasks_encode_msg(&TaskMsg::UpdateStatus {
                        task_id,
                        status: task_status(&status).expect("status was validated"),
                    }),
                },
                // duckfs and pages actions were already applied by
                // `emit_duckfs_effects` / `emit_pages_effects` (their own
                // lanes: probes, cap gate, per-action degrade).
                AgentAction::DuckfsWriteText { .. }
                | AgentAction::AddPageComment { .. }
                | AgentAction::SetPageChecked { .. } => continue,
            };
            ctx.emit_msg(msg);
        }
    }

    fn task_target(&self) -> super::ModuleId {
        self.tasks
            .clone()
            .expect("task actions were validated against a configured tasks module")
    }
}

/// whether an action belongs to the duckfs-write lane (and is therefore
/// skipped by the strict task-action validator and the generic emitter) — the
/// peer of `pages_effects::is_pages_action`.
pub(super) fn is_duckfs_action(action: &AgentAction) -> bool {
    matches!(action, AgentAction::DuckfsWriteText { .. })
}

fn validate_duckfs_text_write(agent: &ModelRecord, path: &str, text: &str) -> Result<(), String> {
    canonical_duckfs_path(path)?;
    if text.len() > MAX_DUCKFS_WRITE_TEXT_BYTES {
        return Err(format!(
            "duckfs.write_text content is {} bytes; the cap is {MAX_DUCKFS_WRITE_TEXT_BYTES}",
            text.len()
        ));
    }
    if !agent.permits(&CapRequest::DuckfsWrite(path)) {
        return Err(format!(
            "agent {} may not write duckfs path {path}",
            agent.agent_id
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ModelStatus, ResourceCaps};

    fn agent_with_write(prefix: &str) -> ModelRecord {
        ModelRecord {
            account: 2,
            agent_id: "bot".into(),
            owner: RunOrigin::System,
            display_name: "Bot".into(),
            capability: "codex".into(),
            allowed_actions: vec![crate::ACTION_DUCKFS_WRITE_TEXT.into()],
            status: ModelStatus::Active,
            role: crate::ModelRole::General,
            created_at: 0,
            updated_at: 0,
            recipe_hash: Vec::new(),
            caps: ResourceCaps {
                duckfs_write: vec![prefix.into()],
                ..Default::default()
            },
            skills: Vec::new(),
        }
    }

    #[test]
    fn duckfs_text_write_validation_is_path_size_and_cap_gated() {
        let agent = agent_with_write("/shared/agents/qa-fixer");
        validate_duckfs_text_write(
            &agent,
            "/shared/agents/qa-fixer/self-improvement/SKILL.md",
            "lesson",
        )
        .expect("granted path");

        let sibling =
            validate_duckfs_text_write(&agent, "/shared/agents/qa-fixer-policy/SKILL.md", "lesson")
                .unwrap_err();
        assert!(sibling.contains("may not write duckfs path"), "{sibling}");

        let relative = validate_duckfs_text_write(&agent, "shared/out.txt", "lesson").unwrap_err();
        assert!(relative.contains("path must be absolute"), "{relative}");

        let too_large = "x".repeat(MAX_DUCKFS_WRITE_TEXT_BYTES + 1);
        let oversized = validate_duckfs_text_write(
            &agent,
            "/shared/agents/qa-fixer/self-improvement/SKILL.md",
            &too_large,
        )
        .unwrap_err();
        assert!(oversized.contains("the cap is"), "{oversized}");
    }
}
