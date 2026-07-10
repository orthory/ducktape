use super::facets::{
    WireStatus, decode_run_result_v1, effects_to_actions, encode_delivery_receipt, valid_data,
};
use super::{
    ACTION_CHAT_POST, AgentAction, AgentRecord, AgentResponse, BTreeSet, Block, ChatMsg, ChatQuery,
    ChatReply, Ctx, Error, MAX_ACTIONS_BYTES, MAX_ACTIONS_PER_RUN, MAX_REPLY_BLOCKS_BYTES,
    MAX_THREAD_REPLIES, Msg, Origin, PendingState, ReplyBlock, ResultEvent, RunsModule, SagaOrigin,
    TaskMsg, TaskQuery, TaskReply, TaskStatus, chat_decode_reply, chat_encode_msg,
    chat_encode_query, decode_result_event, envelope, reply_message_id, tasks_decode_reply,
    tasks_encode_msg, tasks_encode_query,
};

// ---- response normalization ---------------------------------------------------------
// the dispatch-plane oracle returns the model's RAW text (opinion-free, Text
// contract); shaping it into an [`AgentResponse`] is deterministic string
// processing and therefore consensus work, done here in the result intake.

/// the reply-block kinds normalization keeps — the closed vocabulary the
/// strict-output instruction names.
const REPLY_KIND_PARAGRAPH: &str = "paragraph";
const REPLY_KIND_HEADING: &str = "heading";
const REPLY_KIND_CODE: &str = "code";
pub(super) const RUNNER_RESULT_VERSION: u32 = envelope::RUNNER_RESULT_VERSION;

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

fn normalize_response(mut response: AgentResponse, raw_text: &str, job_run: bool) -> AgentResponse {
    response.actions.truncate(MAX_ACTIONS_PER_RUN);
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
        response.reply_blocks = vec![paragraph_block(truncate_utf8(
            &non_empty_text(raw_text),
            MAX_REPLY_BLOCKS_BYTES / 4,
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

fn truncate_utf8(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let mut keep = max;
    while keep > 0 && !text.is_char_boundary(keep) {
        keep -= 1;
    }
    format!("{}…", &text[..keep])
}

/// byte bound on the error excerpt a failure reply carries — same order as
/// the host's diagnostic excerpts (capability-host bounds stderr to 400).
pub(super) const FAILURE_EXCERPT_BYTES: usize = 400;

/// a failed run's error as ONE bounded chat line: whitespace runs (newlines
/// included) collapse to single spaces, then the excerpt bound applies.
pub(super) fn failure_excerpt(reason: &str) -> String {
    let line = reason.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.is_empty() {
        return "no error detail".into();
    }
    truncate_utf8(&line, FAILURE_EXCERPT_BYTES)
}

/// the canonical state form of a dispatch origin (see [`SagaOrigin`]).
pub(super) fn canonical_origin(origin: &Origin) -> SagaOrigin {
    match origin {
        Origin::External(key) => SagaOrigin::External(key.clone()),
        Origin::Module(module) => SagaOrigin::Module(module.clone()),
        Origin::System => SagaOrigin::System,
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
fn allows(agent: &AgentRecord, action: &str) -> bool {
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
        payload: &[u8],
    ) -> Result<(), Error> {
        let Ok(event) = decode_result_event(payload) else {
            self.note(ctx, "dropped undecodable dispatch result event".into());
            return Ok(());
        };
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

        match outcome {
            // THE single delivery path: decode the runner result and apply
            // whatever facets it carries. a plain (message-only) result carries
            // none — it delivers exactly the model prose + its parsed actions.
            Ok(bytes) => self.deliver_run_result(ctx, &run_id, &entry, &bytes).await,
            Err(reason) => self.fail_run(ctx, &run_id, &entry, reason).await,
        }
        Ok(())
    }

    /// the failure triple (breadcrumb note + threaded failure reply + job
    /// finalize false) — unchanged behavior, was inlined three times.
    async fn fail_run(
        &mut self,
        ctx: &mut dyn Ctx,
        run_id: &str,
        entry: &PendingState,
        reason: String,
    ) {
        self.note(ctx, format!("run {run_id} failed: {reason}"));
        self.emit_failure_reply(ctx, run_id, entry, &reason).await;
        self.emit_job_finalize_if_current_claimant(ctx, entry, false, reason)
            .await;
    }

    /// THE single delivery path. message facet + host-assembled effects → one
    /// [`AgentResponse`] (validate/emit reused); the sink is applied (cap-gated,
    /// probe-guarded, degrades to a breadcrumb, never aborts); data (R5) +
    /// artifact (O1) + status fold into the faceted finalize payload. a plain
    /// (message-only) result — raw text or an `AgentResponse` with no runner
    /// marker — decodes to a facet-free [`RunnerResult`] (Chain sink, Ok status,
    /// empty effects), so it delivers exactly the model prose + its parsed
    /// actions. idempotent by run_id — every effect applies once, here, from the
    /// winning attempt (X2); nothing is emitted mid-run.
    async fn deliver_run_result(
        &mut self,
        ctx: &mut dyn Ctx,
        run_id: &str,
        entry: &PendingState,
        bytes: &[u8],
    ) {
        let result = match decode_run_result_v1(bytes) {
            Ok(r) => r,
            Err(reason) => return self.fail_run(ctx, run_id, entry, reason).await,
        };
        // the host observation overrides a present message facet (R4).
        if result.status == WireStatus::Failed {
            return self
                .fail_run(ctx, run_id, entry, "run reported a failed status".into())
                .await;
        }
        let mut response = agent_response_from_text(&result.response_text, entry.job_id.is_some());
        // R1: host-assembled effects are authoritative. FALLBACK: only override
        // the response-parsed actions when the effects facet is non-empty, so a
        // model that emitted actions only in prose (an oracle that didn't lift
        // them) still gets them applied — never a silent drop. a message-only
        // result has empty effects, so it keeps its prose-parsed actions.
        if !result.effects.is_empty() {
            response.actions = match effects_to_actions(&result.effects) {
                Ok(actions) => actions,
                Err(reason) => return self.fail_run(ctx, run_id, entry, reason).await,
            };
        }
        let response = match self.validate_response(&*ctx, run_id, entry, response).await {
            Ok(r) => r,
            Err(reason) => return self.fail_run(ctx, run_id, entry, reason).await,
        };
        // build the faceted finalize payload BEFORE moving `response` into
        // emit_response; emission order is response → sink → finalize.
        let payload = encode_delivery_receipt(
            &response,
            valid_data(&result.data),
            &result.workspace_receipt,
            result.status,
        );
        self.emit_response(ctx, run_id, entry, response);
        self.emit_sink(ctx, run_id, entry, &result.sink).await;
        self.emit_job_finalize_if_current_claimant(ctx, entry, true, payload)
            .await;
    }

    /// deterministic response validation — THE safety boundary (design §5).
    /// the response is data until every check here passes; only then do its
    /// follow-ups exist. beyond grants and caps, this probes everything the
    /// emitted follow-ups could make chat or tasks REJECT (which would abort
    /// the delivery block — the no-fail rule): a squatted reply message id, a
    /// full thread, a duplicate or unknown task id.
    async fn validate_response(
        &self,
        ctx: &dyn Ctx,
        run_id: &str,
        entry: &PendingState,
        response: AgentResponse,
    ) -> Result<AgentResponse, String> {
        let agent = self
            .agent_record(ctx, &entry.agent_id)
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

        if !response.reply_blocks.is_empty() {
            if !allows(&agent, ACTION_CHAT_POST) {
                return Err(format!(
                    "agent {} is not allowed to {ACTION_CHAT_POST}",
                    entry.agent_id
                ));
            }
            let reply_bytes = serde_json::to_vec(&to_chat_blocks(&response.reply_blocks))
                .expect("blocks are serializable");
            if reply_bytes.len() > MAX_REPLY_BLOCKS_BYTES {
                return Err(format!(
                    "reply blocks are {} bytes; the cap is {MAX_REPLY_BLOCKS_BYTES}",
                    reply_bytes.len()
                ));
            }
            self.probe_reply_postable(ctx, run_id, entry).await?;
        }

        if !response.actions.is_empty() {
            let Some(tasks) = self.tasks.clone() else {
                return Err("no tasks module is configured".into());
            };
            let existing = self.task_ids(ctx, &tasks).await?;
            let mut created: BTreeSet<&str> = BTreeSet::new();
            for action in &response.actions {
                let name = action.vocabulary_name();
                if !allows(&agent, name) {
                    return Err(format!("agent {} is not allowed to {name}", entry.agent_id));
                }
                match action {
                    AgentAction::CreateTask { task_id, title } => {
                        if task_id.is_empty() || title.is_empty() {
                            return Err("task actions require a non-empty task_id and title".into());
                        }
                        // duplicates — committed or earlier in this very
                        // response — would make tasks reject the follow-up.
                        if existing.contains(task_id) || !created.insert(task_id) {
                            return Err(format!("task already exists: {task_id}"));
                        }
                    }
                    AgentAction::UpdateTaskStatus { task_id, status } => {
                        if task_status(status).is_none() {
                            return Err(format!("unknown task status: {status}"));
                        }
                        if !existing.contains(task_id) && !created.contains(task_id.as_str()) {
                            return Err(format!("unknown task: {task_id}"));
                        }
                    }
                }
            }
        }

        Ok(response)
    }

    /// prove a reply under the run's message id could land in chat RIGHT NOW
    /// — the no-fail rule again: an emitted post must be valid by
    /// construction, so anything chat would reject is probed first.
    async fn probe_reply_postable(
        &self,
        ctx: &dyn Ctx,
        run_id: &str,
        entry: &PendingState,
    ) -> Result<(), String> {
        // message ids are client-chosen, so anyone could squat the reply
        // id before the result lands; chat would reject the duplicate and
        // abort the block. fail the run instead.
        let message_id = reply_message_id(run_id);
        let reply = ctx
            .query(
                &self.chat,
                &chat_encode_query(&ChatQuery::Message {
                    message_id: message_id.clone(),
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
        // a threaded reply must still fit under chat's thread cap.
        if let Some(root_seq) = entry.thread_root {
            let reply = ctx
                .query(
                    &self.chat,
                    &chat_encode_query(&ChatQuery::MessagesRange {
                        channel_id: entry.channel_id.clone(),
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
            if root.head.reply_count >= MAX_THREAD_REPLIES as u64 {
                return Err(format!(
                    "thread reply cap reached: {}/{root_seq}",
                    entry.channel_id
                ));
            }
        }
        Ok(())
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
            .agent_record(ctx, &entry.agent_id)
            .await?
            .ok_or_else(|| format!("agent is not registered: {}", entry.agent_id))?;
        // posting the failure is a chat post like any reply — ungranted
        // agents keep the old silent-fail.
        if !allows(&agent, ACTION_CHAT_POST) {
            return Err(format!(
                "agent {} is not allowed to {ACTION_CHAT_POST}",
                entry.agent_id
            ));
        }
        self.probe_reply_postable(ctx, run_id, entry).await?;
        let name = if agent.display_name.is_empty() {
            agent.agent_id.as_str()
        } else {
            agent.display_name.as_str()
        };
        let text = format!("⚠ {name} failed: {}", failure_excerpt(reason));
        Ok(Msg {
            target: self.chat.clone(),
            payload: chat_encode_msg(&ChatMsg::PostMessage {
                channel_id: entry.channel_id.clone(),
                message_id: reply_message_id(run_id),
                blocks: vec![Block::paragraph(text)],
                thread: entry.thread_root,
                as_agent: Some(entry.agent_id.clone()),
            }),
        })
    }

    async fn task_ids(&self, ctx: &dyn Ctx, tasks: &str) -> Result<BTreeSet<String>, String> {
        let reply = ctx
            .query(tasks, &tasks_encode_query(&TaskQuery::List))
            .await
            .map_err(|e| format!("tasks lookup failed: {e}"))?;
        match tasks_decode_reply(&reply) {
            Ok(TaskReply::Tasks(list)) => Ok(list.into_iter().map(|t| t.id).collect()),
            Err(e) => Err(format!("undecodable tasks reply: {e}")),
        }
    }

    /// hand a VALIDATED response its follow-ups: the chat reply (authored as
    /// the agent, threaded like its anchor) and the task writes — all drained
    /// in this same delivery block (P2, P6).
    fn emit_response(
        &self,
        ctx: &mut dyn Ctx,
        run_id: &str,
        entry: &PendingState,
        response: AgentResponse,
    ) {
        if !response.reply_blocks.is_empty() {
            ctx.emit_msg(Msg {
                target: self.chat.clone(),
                payload: chat_encode_msg(&ChatMsg::PostMessage {
                    channel_id: entry.channel_id.clone(),
                    message_id: reply_message_id(run_id),
                    blocks: to_chat_blocks(&response.reply_blocks),
                    thread: entry.thread_root,
                    as_agent: Some(entry.agent_id.clone()),
                }),
            });
        }
        for action in response.actions {
            let target = self
                .tasks
                .clone()
                .expect("actions were validated against a configured tasks module");
            let payload = match action {
                AgentAction::CreateTask { task_id, title } => {
                    tasks_encode_msg(&TaskMsg::CreateTask { task_id, title })
                }
                AgentAction::UpdateTaskStatus { task_id, status } => {
                    tasks_encode_msg(&TaskMsg::UpdateStatus {
                        task_id,
                        status: task_status(&status).expect("status was validated"),
                    })
                }
            };
            ctx.emit_msg(Msg { target, payload });
        }
    }
}
