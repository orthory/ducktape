//! deterministic user-defined automations over chat hooks and memory watches.
//!
//! an operator registers rules — a [`Trigger`] (chat post filters or memory
//! publish filters) plus an [`Action`] (post a chat message, create a task, or
//! deliver an inbox notification). when chat fans a post out to its hooks, or
//! memory fans a publish out to its watches, this module evaluates every enabled
//! rule and emits the matching actions as follow-up [`sdk::Msg`]s in the SAME
//! block as the event (P2).
//!
//! ## Origin-gated intake (spoof-proofing)
//!
//! dispatch is routed by the HOST-ASSIGNED origin, exactly like agent v2:
//! - `Origin::Module("chat")` → the payload is a raw `chat::ChatEvent`
//!   (chat's generic hook fan-out delivers the event bytes verbatim, unwrapped),
//!   decoded in the NO-FAIL hook arm.
//! - `Origin::Module("memory")` → the payload is a raw
//!   `memory::MemoryEvent`, decoded in the same NO-FAIL arm style.
//! - every other origin → an [`AutomationsMsg`] admin op (rule CRUD). an
//!   [`AutomationsMsg::HookEvent`] from a non-chat origin is rejected — only
//!   chat's own follow-ups ever wear `Origin::Module("chat")`, so a submitter
//!   cannot forge a hook event.
//!
//! ## Loop prevention
//!
//! a rule fires ONLY when the event author is `AuthorRef::User(_)`. posts authored
//! by modules or agents — including this module's own `PostMessage` follow-ups —
//! never trigger rules, so an automation posting into a hooked channel cannot
//! cascade. this mirrors the agent module's user-author-only decision.
//! memory publish events whose author equals this module id are also skipped as
//! a defense-in-depth loop guard (there is no memory-writing action today).
//!
//! ## No-fail hook arm, probes, and atomicity (P2)
//!
//! the hook arm runs in the user's posting block. an `Err` here would abort the
//! post itself (and every other hook subscriber's delivery), so an undecodable
//! event, a failed message-text fetch, or an action that is structurally
//! impossible to build (e.g. a template that substitutes to an empty
//! message/title, or a composed id over the cap) is a staged no-op recorded as a
//! [`RunRecord`] with `action_ok = false` — never a block failure.
//!
//! on top of that, chat/task actions are PROBED before they are emitted (agent
//! v2's no-fail-arm pattern applied to follow-ups): host-routed queries against
//! the target module's staged-or-committed state — deterministic on every
//! validator — verify that a `PostMessage` target channel exists and its
//! deterministic message id is unused (a user could pre-post the composed id to
//! wedge the rule — id squatting), and that a `CreateTask` id is unused. a probe
//! rejection downgrades to a `RunRecord`, protecting the posting user's block
//! from every structurally-KNOWABLE follow-up failure.
//!
//! `DeliverInbox` is different: member/body caps are checked before emit, and
//! inbox delivery is otherwise no-op tolerant. the one accepted residual abort
//! path is inbox at [`inbox::MAX_MEMBERS`] rejecting a brand-new member;
//! by P2 that aborts the whole block. this is rare and accepted by design.
//!
//! probes cannot catch everything: two rules composing the same id within one
//! event emit past each other's probes, and any other post-probe follow-up
//! failure still aborts the whole block, leaving no trace. that is correct
//! platform behavior — the rule's effect and the triggering event commit or
//! abort as one atomic unit (P2).
//!
//! ## Hook registration is a separate operator op
//!
//! registering a rule does NOT subscribe this module to any channel. the operator
//! separately submits `ChatMsg::RegisterHook { channel_id, module_id: "automations" }`
//! to chat for each channel whose posts should reach these rules.
//! memory rules likewise need a separate
//! `MemoryMsg::RegisterWatch { prefix, module_id: "automations" }` op for each
//! memory subtree whose publishes should reach these rules.
//! memory-triggered [`RunRecord::channel_id`] values carry the memory path.
//!
//! ## State model
//!
//! committed state is `BTreeMap<rule_id, Rule>` plus a bounded run-history ring
//! (`VecDeque`, capped at [`MAX_RUN_HISTORY`], oldest dropped — deterministic).
//! writes stage during `execute` and publish at `commit_block`; `root()` is a
//! sha256 over the canonical committed encoding, which is byte-identical to the
//! `snapshot()` preimage. every size cap is enforced at execute with rejection,
//! so oversized bytes never enter the root preimage (the repo's poison-value
//! lesson).

// the wire surface: this module's shared types, flattened at the crate root.
mod interface;
pub use interface::*;

use std::collections::{BTreeMap, VecDeque};
use std::fmt::Write as _;

use chat::{
    AuthorRef, Block, ChatEvent, ChatMsg, ChatQuery, ChatReply, decode_event as chat_decode_event,
    decode_reply as chat_decode_reply, encode_msg as chat_encode_msg,
    encode_query as chat_encode_query,
};
use inbox::{
    InboxMsg, MAX_BODY_BYTES as INBOX_MAX_BODY_BYTES, MAX_KIND_BYTES, MAX_MEMBER_BYTES,
    encode_msg as inbox_encode_msg,
};
use memory::{
    MAX_PATH_BYTES as MEMORY_MAX_PATH_BYTES, MAX_SEGMENT_BYTES as MEMORY_MAX_SEGMENT_BYTES,
    META_KIND, MemoryEvent, decode_event as memory_decode_event,
};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot};
use sha2::{Digest, Sha256};
use tasks::{
    TaskMsg, TaskQuery, TaskReply, decode_reply as tasks_decode_reply,
    encode_msg as tasks_encode_msg, encode_query as tasks_encode_query,
};

/// max rules retained. registering beyond this is rejected at execute.
pub const MAX_RULES: usize = 1024;
/// `rule_id` byte bound (also the `channel_id`/`task_id_prefix` bound).
pub const MAX_ID_BYTES: usize = 256;
/// trigger filter (`mention`, `text_contains`) byte bound.
pub const MAX_FILTER_BYTES: usize = 256;
/// action template byte bound.
pub const MAX_TEMPLATE_BYTES: usize = 4096;
/// run-history ring capacity; the oldest record is dropped past this.
pub const MAX_RUN_HISTORY: usize = 1024;
/// actions emitted per incoming event; matching rules past this are recorded as
/// skipped (`action_ok = false`, `detail = "action budget exceeded"`).
pub const MAX_ACTIONS_PER_EVENT: usize = 8;

pub struct Automations {
    id: ModuleId,
    /// the chat module id — both the trusted hook origin and the `PostMessage`
    /// follow-up target.
    chat: ModuleId,
    /// the tasks module id — the `CreateTask` follow-up target.
    tasks: ModuleId,
    /// the inbox module id — the `DeliverInbox` follow-up target.
    inbox: ModuleId,
    /// the memory module id — the trusted memory-watch origin.
    memory: ModuleId,
    // committed state (the root preimage).
    rules: BTreeMap<String, Rule>,
    history: VecDeque<RunRecord>,
    // staged overlay, published at commit_block. `Some` upserts a rule, `None`
    // tombstones it; run-history appends collect here in emit order.
    pending_rules: BTreeMap<String, Option<Rule>>,
    pending_history: Vec<RunRecord>,
}

impl Automations {
    pub fn new(
        id: impl Into<ModuleId>,
        chat: impl Into<ModuleId>,
        tasks: impl Into<ModuleId>,
        inbox: impl Into<ModuleId>,
        memory: impl Into<ModuleId>,
    ) -> Self {
        Self {
            id: id.into(),
            chat: chat.into(),
            tasks: tasks.into(),
            inbox: inbox.into(),
            memory: memory.into(),
            rules: BTreeMap::new(),
            history: VecDeque::new(),
            pending_rules: BTreeMap::new(),
            pending_history: Vec::new(),
        }
    }

    // ---- validation ---------------------------------------------------------

    fn validate_non_empty(field: &str, value: &str) -> Result<(), Error> {
        if value.is_empty() {
            return Err(Error::Module(format!("{field} must not be empty")));
        }
        Ok(())
    }

    fn validate_len(field: &str, value: &str, max: usize) -> Result<(), Error> {
        if value.len() > max {
            return Err(Error::Module(format!(
                "{field} exceeds {max} bytes ({} given)",
                value.len()
            )));
        }
        Ok(())
    }

    fn validate_trigger(trigger: &Trigger) -> Result<(), Error> {
        match trigger {
            Trigger::MessagePosted {
                channel_id,
                mention,
                text_contains,
            } => {
                if let Some(channel_id) = channel_id {
                    Self::validate_non_empty("trigger channel_id", channel_id)?;
                    Self::validate_len("trigger channel_id", channel_id, MAX_ID_BYTES)?;
                }
                if let Some(mention) = mention {
                    Self::validate_len("trigger mention", mention, MAX_FILTER_BYTES)?;
                }
                if let Some(text_contains) = text_contains {
                    Self::validate_len("trigger text_contains", text_contains, MAX_FILTER_BYTES)?;
                }
            }
            Trigger::MemoryPublished {
                prefix,
                meta_kind,
                author_contains,
            } => {
                if let Some(prefix) = prefix {
                    validate_memory_prefix(prefix)?;
                }
                if let Some(meta_kind) = meta_kind {
                    Self::validate_len("trigger meta_kind", meta_kind, MAX_FILTER_BYTES)?;
                }
                if let Some(author_contains) = author_contains {
                    Self::validate_len(
                        "trigger author_contains",
                        author_contains,
                        MAX_FILTER_BYTES,
                    )?;
                }
            }
        }
        Ok(())
    }

    fn validate_action(action: &Action) -> Result<(), Error> {
        match action {
            Action::PostMessage {
                channel_id,
                template,
            } => {
                Self::validate_non_empty("action channel_id", channel_id)?;
                Self::validate_len("action channel_id", channel_id, MAX_ID_BYTES)?;
                Self::validate_len("action template", template, MAX_TEMPLATE_BYTES)?;
            }
            Action::CreateTask {
                task_id_prefix,
                title_template,
            } => {
                Self::validate_non_empty("action task_id_prefix", task_id_prefix)?;
                Self::validate_len("action task_id_prefix", task_id_prefix, MAX_ID_BYTES)?;
                Self::validate_len("action title_template", title_template, MAX_TEMPLATE_BYTES)?;
            }
            Action::DeliverInbox {
                member_template,
                kind,
                body_template,
            } => {
                Self::validate_len(
                    "action member_template",
                    member_template,
                    MAX_TEMPLATE_BYTES,
                )?;
                Self::validate_non_empty("action kind", kind)?;
                Self::validate_len("action kind", kind, MAX_KIND_BYTES)?;
                Self::validate_len("action body_template", body_template, MAX_TEMPLATE_BYTES)?;
            }
        }
        Ok(())
    }

    // ---- staged-overlay reads -----------------------------------------------

    /// the effective rule under the staged overlay: a staged upsert wins, a
    /// staged tombstone hides the committed rule, otherwise the committed rule.
    fn rule(&self, rule_id: &str) -> Option<Rule> {
        match self.pending_rules.get(rule_id) {
            Some(staged) => staged.clone(),
            None => self.rules.get(rule_id).cloned(),
        }
    }

    /// committed rules merged with the staged overlay (upserts win, tombstones
    /// remove), in `rule_id` order.
    fn effective_rules(&self) -> BTreeMap<String, Rule> {
        let mut merged = self.rules.clone();
        for (id, staged) in &self.pending_rules {
            match staged {
                Some(rule) => {
                    merged.insert(id.clone(), rule.clone());
                }
                None => {
                    merged.remove(id);
                }
            }
        }
        merged
    }

    /// committed history followed by staged appends (read-your-writes in-block).
    fn effective_history(&self) -> impl Iterator<Item = &RunRecord> {
        self.history.iter().chain(self.pending_history.iter())
    }

    // ---- admin ops ----------------------------------------------------------

    fn stage_create_rule(
        &mut self,
        rule_id: String,
        trigger: Trigger,
        action: Action,
        consensus_time: u64,
    ) -> Result<(), Error> {
        Self::validate_non_empty("rule_id", &rule_id)?;
        Self::validate_len("rule_id", &rule_id, MAX_ID_BYTES)?;
        Self::validate_trigger(&trigger)?;
        Self::validate_action(&action)?;
        if self.rule(&rule_id).is_some() {
            return Err(Error::Module(format!("rule already exists: {rule_id}")));
        }
        if self.effective_rules().len() >= MAX_RULES {
            return Err(Error::Module(format!("rule cap reached ({MAX_RULES})")));
        }
        self.pending_rules.insert(
            rule_id.clone(),
            Some(Rule {
                rule_id,
                enabled: true,
                trigger,
                action,
                created_at: consensus_time,
                fire_count: 0,
            }),
        );
        Ok(())
    }

    fn stage_set_enabled(&mut self, rule_id: String, enabled: bool) -> Result<(), Error> {
        Self::validate_non_empty("rule_id", &rule_id)?;
        let mut rule = self
            .rule(&rule_id)
            .ok_or_else(|| Error::Module(format!("unknown rule: {rule_id}")))?;
        if rule.enabled == enabled {
            // idempotent: staging nothing keeps the committed root byte-identical.
            return Ok(());
        }
        rule.enabled = enabled;
        self.pending_rules.insert(rule_id, Some(rule));
        Ok(())
    }

    fn stage_delete_rule(&mut self, rule_id: String) -> Result<(), Error> {
        Self::validate_non_empty("rule_id", &rule_id)?;
        if self.rule(&rule_id).is_none() {
            return Err(Error::Module(format!("unknown rule: {rule_id}")));
        }
        self.pending_rules.insert(rule_id, None);
        Ok(())
    }

    // ---- the chat hook intake (NO-FAIL arm) ---------------------------------

    async fn on_chat_event(&mut self, ctx: &mut dyn Ctx, payload: &[u8]) -> Result<(), Error> {
        let Ok(event) = chat_decode_event(payload) else {
            // an undecodable event must not abort the posting block.
            return Ok(());
        };
        let ChatEvent::MessagePosted {
            channel_id,
            seq,
            thread_root: _,
            author,
            mentions,
        } = event;

        // LOOP PREVENTION: only user-authored posts fire rules. module/agent
        // posts (including our own PostMessage follow-ups) never re-trigger.
        if !matches!(author, AuthorRef::User(_)) {
            return Ok(());
        }
        let height = ctx.env().height;
        let effective = self.effective_rules();

        // fetch the post's text ONCE, and only if some rule that already matches
        // on channel + mention needs it (a `text_contains` filter, or a `{text}`
        // placeholder). `None` = the fetch FAILED (query error / message absent),
        // which is distinct from a legitimately empty body (`Some("")`): rules
        // that need text record a failure on `None` instead of silently
        // matching against emptiness.
        let needs_text = effective.values().any(|rule| {
            rule.enabled
                && Self::matches_channel_and_mention(rule, &channel_id, &mentions)
                && Self::rule_wants_text(rule)
        });
        let text: Option<String> = if needs_text {
            self.fetch_text(&*ctx, &channel_id, seq).await
        } else {
            Some(String::new())
        };
        let author_display = display_author(&author);
        let mention_display = mentions
            .first()
            .map(display_author)
            .unwrap_or_else(String::new);

        // evaluate in deterministic rule_id order (BTreeMap iteration).
        let mut budget = 0usize;
        let mut fired: Vec<(String, Rule)> = Vec::new();
        let mut records: Vec<RunRecord> = Vec::new();
        for (rule_id, rule) in &effective {
            if !rule.enabled || !Self::matches_channel_and_mention(rule, &channel_id, &mentions) {
                continue;
            }
            let record = |action_ok: bool, detail: String| RunRecord {
                rule_id: rule_id.clone(),
                channel_id: channel_id.clone(),
                seq,
                height,
                action_ok,
                detail,
            };
            // a rule that needs text cannot be evaluated (or substituted) when
            // the fetch failed: a recorded failure, never empty-text guessing.
            let text = match (&text, Self::rule_wants_text(rule)) {
                (Some(text), _) => text.as_str(),
                (None, false) => "",
                (None, true) => {
                    records.push(record(false, "text fetch failed".into()));
                    continue;
                }
            };
            if let Trigger::MessagePosted {
                text_contains: Some(want),
                ..
            } = &rule.trigger
            {
                if !text.contains(want.as_str()) {
                    continue;
                }
            }
            if budget >= MAX_ACTIONS_PER_EVENT {
                records.push(record(false, "action budget exceeded".into()));
                continue;
            }
            let vars = TemplateVars {
                channel: &channel_id,
                seq: Some(seq),
                author: &author_display,
                text,
                mention: &mention_display,
                path: "",
                generation: None,
            };
            match self
                .build_and_emit(ctx, rule, &channel_id, seq, &vars)
                .await
            {
                Ok(detail) => {
                    budget += 1;
                    let mut updated = rule.clone();
                    updated.fire_count = updated.fire_count.saturating_add(1);
                    fired.push((rule_id.clone(), updated));
                    records.push(record(true, detail));
                }
                Err(detail) => {
                    // a structurally impossible or probe-rejected action is
                    // recorded, not emitted, and never bumps fire_count or
                    // consumes the action budget.
                    records.push(record(false, detail));
                }
            }
        }
        for (rule_id, updated) in fired {
            self.pending_rules.insert(rule_id, Some(updated));
        }
        self.pending_history.extend(records);
        Ok(())
    }

    async fn on_memory_event(&mut self, ctx: &mut dyn Ctx, payload: &[u8]) -> Result<(), Error> {
        let Ok(MemoryEvent::Published {
            path,
            generation,
            meta,
            author,
        }) = memory_decode_event(payload)
        else {
            // an undecodable event must not abort the publishing block.
            return Ok(());
        };
        if author == self.id {
            return Ok(());
        }
        let height = ctx.env().height;
        let effective = self.effective_rules();
        let vars = TemplateVars {
            channel: "",
            seq: None,
            author: &author,
            text: "",
            mention: "",
            path: &path,
            generation: Some(generation),
        };

        let mut budget = 0usize;
        let mut fired: Vec<(String, Rule)> = Vec::new();
        let mut records: Vec<RunRecord> = Vec::new();
        for (rule_id, rule) in &effective {
            if !rule.enabled || !Self::matches_memory(rule, &path, &meta, &author) {
                continue;
            }
            let record = |action_ok: bool, detail: String| RunRecord {
                rule_id: rule_id.clone(),
                channel_id: path.clone(),
                seq: generation,
                height,
                action_ok,
                detail,
            };
            if budget >= MAX_ACTIONS_PER_EVENT {
                records.push(record(false, "action budget exceeded".into()));
                continue;
            }
            match self
                .build_and_emit(ctx, rule, &path, generation, &vars)
                .await
            {
                Ok(detail) => {
                    budget += 1;
                    let mut updated = rule.clone();
                    updated.fire_count = updated.fire_count.saturating_add(1);
                    fired.push((rule_id.clone(), updated));
                    records.push(record(true, detail));
                }
                Err(detail) => {
                    records.push(record(false, detail));
                }
            }
        }
        for (rule_id, updated) in fired {
            self.pending_rules.insert(rule_id, Some(updated));
        }
        self.pending_history.extend(records);
        Ok(())
    }

    /// build the action for a firing rule, PROBE its target, and emit it as a
    /// follow-up. returns the success `detail` on emit, or an error `detail`
    /// when the action is structurally impossible or a probe rejects it
    /// (recorded, not a block failure).
    ///
    /// the probe layer (agent v2's no-fail-arm pattern applied to follow-ups):
    /// every structurally-KNOWABLE follow-up failure is checked here via
    /// host-routed queries against the target's staged-or-committed state —
    /// deterministic on every validator — so a missing channel, a squatted
    /// deterministic id, or a task-id collision downgrades to a RunRecord
    /// instead of aborting the posting user's block. probes cannot catch
    /// everything (e.g. two rules composing the same id in one event emit past
    /// each other's probes); a post-probe follow-up failure still aborts the
    /// block by P2 design.
    async fn build_and_emit(
        &self,
        ctx: &mut dyn Ctx,
        rule: &Rule,
        event_channel: &str,
        seq: u64,
        vars: &TemplateVars<'_>,
    ) -> Result<String, String> {
        match &rule.action {
            Action::PostMessage {
                channel_id,
                template,
            } => {
                let body = substitute_vars(template, vars);
                if body.is_empty() {
                    return Err("post template produced an empty message".into());
                }
                // deterministic, collision-free per (rule, message).
                let message_id = format!("auto-{}-{}-{}", rule.rule_id, event_channel, seq);
                // composed-id guard BEFORE the probes: event channel ids are
                // unbounded, so the composition can exceed this module's id cap.
                if message_id.len() > MAX_ID_BYTES {
                    return Err("composed id exceeds cap".into());
                }
                // probe 1: the target channel must exist — chat would reject
                // the post and abort the block otherwise.
                let req = chat_encode_query(&ChatQuery::Channel {
                    channel_id: channel_id.clone(),
                });
                match ctx.query(&self.chat, &req).await {
                    Err(e) => return Err(format!("chat probe failed: {e}")),
                    Ok(bytes) => match chat_decode_reply(&bytes) {
                        Ok(ChatReply::Channel(Some(_))) => {}
                        Ok(ChatReply::Channel(None)) => {
                            return Err(format!("target channel does not exist: {channel_id}"));
                        }
                        _ => return Err("chat probe returned an unexpected reply".into()),
                    },
                }
                // probe 2: the deterministic message id must be unused — ids
                // are caller-supplied at chat, so a user could pre-post the
                // composed id to wedge this rule's next fire (id squatting).
                let req = chat_encode_query(&ChatQuery::Message {
                    message_id: message_id.clone(),
                });
                match ctx.query(&self.chat, &req).await {
                    Err(e) => return Err(format!("chat probe failed: {e}")),
                    Ok(bytes) => match chat_decode_reply(&bytes) {
                        Ok(ChatReply::Message(None)) => {}
                        Ok(ChatReply::Message(Some(_))) => {
                            return Err(format!("message id already taken: {message_id}"));
                        }
                        _ => return Err("chat probe returned an unexpected reply".into()),
                    },
                }
                ctx.emit_msg(Msg {
                    target: self.chat.clone(),
                    payload: chat_encode_msg(&ChatMsg::PostMessage {
                        channel_id: channel_id.clone(),
                        message_id,
                        blocks: vec![Block::paragraph(body)],
                        thread: None,
                        as_agent: None,
                    }),
                });
                Ok(format!("posted to {channel_id}"))
            }
            Action::CreateTask {
                task_id_prefix,
                title_template,
            } => {
                let title = substitute_vars(title_template, vars);
                if title.is_empty() {
                    return Err("task template produced an empty title".into());
                }
                // deterministic, collision-free per (prefix, message).
                let task_id = format!("{task_id_prefix}-{event_channel}-{seq}");
                // composed-id guard BEFORE the probe (see PostMessage).
                if task_id.len() > MAX_ID_BYTES {
                    return Err("composed id exceeds cap".into());
                }
                // probe: the composed task id must be unused — tasks rejects
                // duplicates, which would abort the block. tasks-interface only
                // exposes List today, so this is an O(n) scan; switch to a Get
                // query when the interface grows one.
                let req = tasks_encode_query(&TaskQuery::List);
                match ctx.query(&self.tasks, &req).await {
                    Err(e) => return Err(format!("tasks probe failed: {e}")),
                    Ok(bytes) => match tasks_decode_reply(&bytes) {
                        Ok(TaskReply::Tasks(tasks)) => {
                            if tasks.iter().any(|task| task.id == task_id) {
                                return Err(format!("task id already exists: {task_id}"));
                            }
                        }
                        Err(_) => return Err("tasks probe returned an unexpected reply".into()),
                    },
                }
                ctx.emit_msg(Msg {
                    target: self.tasks.clone(),
                    payload: tasks_encode_msg(&TaskMsg::CreateTask {
                        task_id: task_id.clone(),
                        title,
                    }),
                });
                Ok(format!("created task {task_id}"))
            }
            Action::DeliverInbox {
                member_template,
                kind,
                body_template,
            } => {
                let member = substitute_vars(member_template, vars);
                if member.is_empty() {
                    return Err("inbox member is empty".into());
                }
                if member.len() > MAX_MEMBER_BYTES {
                    return Err("inbox member exceeds cap".into());
                }
                let body = substitute_vars(body_template, vars);
                if body.len() > INBOX_MAX_BODY_BYTES {
                    return Err("inbox body exceeds cap".into());
                }
                ctx.emit_msg(Msg {
                    target: self.inbox.clone(),
                    payload: inbox_encode_msg(&InboxMsg::Deliver {
                        member: member.clone(),
                        kind: kind.clone(),
                        body,
                    }),
                });
                Ok(format!("delivered inbox {kind} to {member}"))
            }
        }
    }

    /// single-message fetch by sequence. `Some(text)` is the message's
    /// concatenated text blocks (possibly legitimately empty); `None` means the
    /// FETCH failed — a query error, an undecodable reply, or an absent
    /// message — which callers record instead of treating as empty text.
    async fn fetch_text(&self, ctx: &dyn Ctx, channel_id: &str, seq: u64) -> Option<String> {
        let req = chat_encode_query(&ChatQuery::MessagesRange {
            channel_id: channel_id.to_string(),
            from_seq: seq,
            limit: 1,
        });
        let bytes = ctx.query(&self.chat, &req).await.ok()?;
        let Ok(ChatReply::Messages(views)) = chat_decode_reply(&bytes) else {
            return None;
        };
        views
            .into_iter()
            .find(|view| view.seq == seq)
            .map(|view| blocks_text(&view.head.blocks))
    }

    fn matches_channel_and_mention(rule: &Rule, channel_id: &str, mentions: &[AuthorRef]) -> bool {
        let Trigger::MessagePosted {
            channel_id: trig_channel,
            mention,
            ..
        } = &rule.trigger
        else {
            return false;
        };
        if let Some(want) = trig_channel {
            if want != channel_id {
                return false;
            }
        }
        if let Some(want) = mention {
            if !mentions
                .iter()
                .any(|author| display_author(author).contains(want.as_str()))
            {
                return false;
            }
        }
        true
    }

    fn matches_memory(
        rule: &Rule,
        path: &str,
        meta: &std::collections::BTreeMap<String, String>,
        author: &str,
    ) -> bool {
        let Trigger::MemoryPublished {
            prefix,
            meta_kind,
            author_contains,
        } = &rule.trigger
        else {
            return false;
        };
        if let Some(prefix) = prefix {
            if !memory_prefix_matches(prefix, path) {
                return false;
            }
        }
        if let Some(kind) = meta_kind {
            if meta.get(META_KIND) != Some(kind) {
                return false;
            }
        }
        if let Some(needle) = author_contains {
            if !author.contains(needle.as_str()) {
                return false;
            }
        }
        true
    }

    fn rule_wants_text(rule: &Rule) -> bool {
        if matches!(
            &rule.trigger,
            Trigger::MessagePosted {
                text_contains: Some(_),
                ..
            }
        ) {
            return true;
        }
        match &rule.action {
            Action::PostMessage { template, .. } => template.contains("{text}"),
            Action::CreateTask { title_template, .. } => title_template.contains("{text}"),
            Action::DeliverInbox {
                member_template,
                body_template,
                ..
            } => member_template.contains("{text}") || body_template.contains("{text}"),
        }
    }

    // ---- state-sync ---------------------------------------------------------

    fn root_of(rules: &BTreeMap<String, Rule>, history: &VecDeque<RunRecord>) -> StateRoot {
        let mut h = Sha256::new();
        h.update(encode_state(rules, history));
        StateRoot(h.finalize().into())
    }

    /// the canonical committed encoding — byte-identical to the `root()` preimage.
    pub fn snapshot(&self) -> Vec<u8> {
        encode_state(&self.rules, &self.history)
    }

    /// adopt a peer snapshot only after re-deriving `expected` via the exact
    /// `root()` algorithm. all-or-nothing: on any error this module (and its
    /// root) is byte-identical to before the call.
    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        let (rules, history) = decode_state(bytes)?;
        if Self::root_of(&rules, &history) != expected {
            return Err(Error::Module("snapshot root mismatch".into()));
        }
        self.rules = rules;
        self.history = history;
        self.pending_rules.clear();
        self.pending_history.clear();
        Ok(())
    }
}

// ---- deterministic author display -------------------------------------------

struct TemplateVars<'a> {
    channel: &'a str,
    seq: Option<u64>,
    author: &'a str,
    text: &'a str,
    mention: &'a str,
    path: &'a str,
    generation: Option<u64>,
}

/// the deterministic display form matched by `mention` filters and substituted
/// for `{author}`. users render as their hex pubkey; agents as `module/agent_id`
/// so a `mention` filter of the agent id matches.
fn display_author(author: &AuthorRef) -> String {
    match author {
        AuthorRef::User(bytes) => format!("user:{}", hex(bytes)),
        AuthorRef::Agent { module, agent_id } => format!("{module}/{agent_id}"),
        AuthorRef::Module(module) => module.clone(),
        AuthorRef::System => "system".into(),
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}

/// concatenate a message's text blocks: spans within paragraph/quote blocks are
/// joined directly, code blocks contribute their text, dividers nothing, and
/// blocks are joined by newlines. deterministic.
fn blocks_text(blocks: &[Block]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for block in blocks {
        match block {
            Block::Paragraph(spans) | Block::Quote(spans) => {
                parts.push(spans.iter().map(|span| span.text.as_str()).collect());
            }
            Block::Code { text, .. } => parts.push(text.clone()),
            Block::Divider => {}
        }
    }
    parts.join("\n")
}

/// single-pass placeholder substitution (deterministic, no regex). the known
/// placeholders are replaced from the triggering event; any other `{...}` token
/// is left literal. a single pass means substituted values are never re-scanned
/// for placeholders.
#[cfg(test)]
fn substitute(template: &str, channel: &str, seq: u64, author: &str, text: &str) -> String {
    let vars = TemplateVars {
        channel,
        seq: Some(seq),
        author,
        text,
        mention: "",
        path: "",
        generation: None,
    };
    substitute_vars(template, &vars)
}

fn substitute_vars(template: &str, vars: &TemplateVars<'_>) -> String {
    let seq_str;
    let seq = match vars.seq {
        Some(seq) => {
            seq_str = seq.to_string();
            seq_str.as_str()
        }
        None => "",
    };
    let generation_str;
    let generation = match vars.generation {
        Some(generation) => {
            generation_str = generation.to_string();
            generation_str.as_str()
        }
        None => "",
    };
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        rest = &rest[open..];
        let Some(close) = rest.find('}') else {
            // no closing brace: the remainder is literal.
            break;
        };
        let replacement = match &rest[..=close] {
            "{channel}" => vars.channel,
            "{seq}" => seq,
            "{author}" => vars.author,
            "{text}" => vars.text,
            "{mention}" => vars.mention,
            "{path}" => vars.path,
            "{generation}" => generation,
            _ => {
                // unknown token: emit the '{' literally, rescan after it.
                out.push('{');
                rest = &rest[1..];
                continue;
            }
        };
        out.push_str(replacement);
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    out
}

fn validate_memory_prefix(prefix: &str) -> Result<(), Error> {
    normalize_memory_segments(prefix).map(|_| ())
}

fn normalize_memory_segments(path: &str) -> Result<Vec<&str>, Error> {
    if path.is_empty() {
        return Err(Error::Module("memory prefix must not be empty".into()));
    }
    if path.len() > MEMORY_MAX_PATH_BYTES {
        return Err(Error::Module("memory prefix exceeds byte cap".into()));
    }
    let Some(rest) = path.strip_prefix('/') else {
        return Err(Error::Module(
            "memory prefix must be absolute (start with '/')".into(),
        ));
    };
    if rest.is_empty() {
        return Ok(Vec::new());
    }
    if rest.ends_with('/') {
        return Err(Error::Module(
            "memory prefix must not have a trailing slash".into(),
        ));
    }
    let mut segments = Vec::new();
    for seg in rest.split('/') {
        if seg.is_empty() {
            return Err(Error::Module(
                "memory prefix must not contain empty segments".into(),
            ));
        }
        if seg == "." || seg == ".." {
            return Err(Error::Module(
                "memory prefix must not contain '.' or '..'".into(),
            ));
        }
        if seg.len() > MEMORY_MAX_SEGMENT_BYTES {
            return Err(Error::Module(
                "memory prefix segment exceeds byte cap".into(),
            ));
        }
        segments.push(seg);
    }
    Ok(segments)
}

fn memory_prefix_matches(prefix: &str, path: &str) -> bool {
    prefix == "/"
        || path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}

// ---- canonical byte codec (root preimage) -----------------------------------

fn encode_state(rules: &BTreeMap<String, Rule>, history: &VecDeque<RunRecord>) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(rules.len() as u64).to_le_bytes());
    for rule in rules.values() {
        push_string(&mut out, &rule.rule_id);
        out.push(rule.enabled as u8);
        push_trigger(&mut out, &rule.trigger);
        push_action(&mut out, &rule.action);
        out.extend_from_slice(&rule.created_at.to_le_bytes());
        out.extend_from_slice(&rule.fire_count.to_le_bytes());
    }
    out.extend_from_slice(&(history.len() as u64).to_le_bytes());
    for record in history {
        push_string(&mut out, &record.rule_id);
        push_string(&mut out, &record.channel_id);
        out.extend_from_slice(&record.seq.to_le_bytes());
        out.extend_from_slice(&record.height.to_le_bytes());
        out.push(record.action_ok as u8);
        push_string(&mut out, &record.detail);
    }
    out
}

fn push_string(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u64).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}

fn push_opt_string(out: &mut Vec<u8>, value: &Option<String>) {
    match value {
        Some(value) => {
            out.push(1);
            push_string(out, value);
        }
        None => out.push(0),
    }
}

fn push_trigger(out: &mut Vec<u8>, trigger: &Trigger) {
    match trigger {
        Trigger::MessagePosted {
            channel_id,
            mention,
            text_contains,
        } => {
            out.push(0);
            push_opt_string(out, channel_id);
            push_opt_string(out, mention);
            push_opt_string(out, text_contains);
        }
        Trigger::MemoryPublished {
            prefix,
            meta_kind,
            author_contains,
        } => {
            out.push(1);
            push_opt_string(out, prefix);
            push_opt_string(out, meta_kind);
            push_opt_string(out, author_contains);
        }
    }
}

fn push_action(out: &mut Vec<u8>, action: &Action) {
    match action {
        Action::PostMessage {
            channel_id,
            template,
        } => {
            out.push(0);
            push_string(out, channel_id);
            push_string(out, template);
        }
        Action::CreateTask {
            task_id_prefix,
            title_template,
        } => {
            out.push(1);
            push_string(out, task_id_prefix);
            push_string(out, title_template);
        }
        Action::DeliverInbox {
            member_template,
            kind,
            body_template,
        } => {
            out.push(2);
            push_string(out, member_template);
            push_string(out, kind);
            push_string(out, body_template);
        }
    }
}

fn decode_state(bytes: &[u8]) -> Result<(BTreeMap<String, Rule>, VecDeque<RunRecord>), Error> {
    let mut off = 0usize;
    let rule_count = read_u64(bytes, &mut off)?;
    // each rule costs at least one byte, so this bounds the loop against a
    // truncated header claiming a huge count.
    if rule_count > (bytes.len() - off) as u64 {
        return Err(Error::Module("snapshot truncated".into()));
    }
    let mut rules: BTreeMap<String, Rule> = BTreeMap::new();
    for _ in 0..rule_count {
        let rule_id = read_string(bytes, &mut off)?;
        Automations::validate_non_empty("rule_id", &rule_id)?;
        Automations::validate_len("rule_id", &rule_id, MAX_ID_BYTES)?;
        let enabled = read_bool(bytes, &mut off)?;
        let trigger = read_trigger(bytes, &mut off)?;
        let action = read_action(bytes, &mut off)?;
        let created_at = read_u64(bytes, &mut off)?;
        let fire_count = read_u64(bytes, &mut off)?;
        if rules
            .last_key_value()
            .is_some_and(|(last, _)| last.as_str() >= rule_id.as_str())
        {
            return Err(Error::Module(
                "snapshot rule ids not strictly ascending".into(),
            ));
        }
        rules.insert(
            rule_id.clone(),
            Rule {
                rule_id,
                enabled,
                trigger,
                action,
                created_at,
                fire_count,
            },
        );
    }

    let history_count = read_u64(bytes, &mut off)?;
    if history_count > MAX_RUN_HISTORY as u64 {
        return Err(Error::Module("snapshot run history exceeds cap".into()));
    }
    if history_count > (bytes.len() - off) as u64 {
        return Err(Error::Module("snapshot truncated".into()));
    }
    let mut history: VecDeque<RunRecord> = VecDeque::with_capacity(history_count as usize);
    for _ in 0..history_count {
        // rule_id is always a registered rule's id, so the execute-time caps
        // hold for it. channel_id and detail derive from the chat EVENT (chat
        // does not bound channel-id length), so no length checks on them:
        // install must accept every execute-reachable state — the root
        // comparison is the integrity check (the poison-value lesson).
        let rule_id = read_string(bytes, &mut off)?;
        Automations::validate_non_empty("run record rule_id", &rule_id)?;
        Automations::validate_len("run record rule_id", &rule_id, MAX_ID_BYTES)?;
        let channel_id = read_string(bytes, &mut off)?;
        let seq = read_u64(bytes, &mut off)?;
        let height = read_u64(bytes, &mut off)?;
        let action_ok = read_bool(bytes, &mut off)?;
        let detail = read_string(bytes, &mut off)?;
        history.push_back(RunRecord {
            rule_id,
            channel_id,
            seq,
            height,
            action_ok,
            detail,
        });
    }

    if off != bytes.len() {
        return Err(Error::Module("snapshot has trailing bytes".into()));
    }
    Ok((rules, history))
}

fn read_bool(bytes: &[u8], off: &mut usize) -> Result<bool, Error> {
    match read_u8(bytes, off)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(Error::Module("snapshot has invalid bool byte".into())),
    }
}

fn read_opt_string(bytes: &[u8], off: &mut usize) -> Result<Option<String>, Error> {
    match read_u8(bytes, off)? {
        0 => Ok(None),
        1 => Ok(Some(read_string(bytes, off)?)),
        _ => Err(Error::Module("snapshot has invalid option flag".into())),
    }
}

fn read_trigger(bytes: &[u8], off: &mut usize) -> Result<Trigger, Error> {
    match read_u8(bytes, off)? {
        0 => {
            let channel_id = read_opt_string(bytes, off)?;
            if let Some(channel_id) = &channel_id {
                Automations::validate_non_empty("trigger channel_id", channel_id)?;
                Automations::validate_len("trigger channel_id", channel_id, MAX_ID_BYTES)?;
            }
            let mention = read_opt_string(bytes, off)?;
            if let Some(mention) = &mention {
                Automations::validate_len("trigger mention", mention, MAX_FILTER_BYTES)?;
            }
            let text_contains = read_opt_string(bytes, off)?;
            if let Some(text_contains) = &text_contains {
                Automations::validate_len(
                    "trigger text_contains",
                    text_contains,
                    MAX_FILTER_BYTES,
                )?;
            }
            Ok(Trigger::MessagePosted {
                channel_id,
                mention,
                text_contains,
            })
        }
        1 => {
            let prefix = read_opt_string(bytes, off)?;
            if let Some(prefix) = &prefix {
                validate_memory_prefix(prefix)?;
            }
            let meta_kind = read_opt_string(bytes, off)?;
            if let Some(meta_kind) = &meta_kind {
                Automations::validate_len("trigger meta_kind", meta_kind, MAX_FILTER_BYTES)?;
            }
            let author_contains = read_opt_string(bytes, off)?;
            if let Some(author_contains) = &author_contains {
                Automations::validate_len(
                    "trigger author_contains",
                    author_contains,
                    MAX_FILTER_BYTES,
                )?;
            }
            Ok(Trigger::MemoryPublished {
                prefix,
                meta_kind,
                author_contains,
            })
        }
        other => Err(Error::Module(format!(
            "snapshot has unknown trigger discriminant {other}"
        ))),
    }
}

fn read_action(bytes: &[u8], off: &mut usize) -> Result<Action, Error> {
    match read_u8(bytes, off)? {
        0 => {
            let channel_id = read_string(bytes, off)?;
            let template = read_string(bytes, off)?;
            let action = Action::PostMessage {
                channel_id,
                template,
            };
            Automations::validate_action(&action)?;
            Ok(action)
        }
        1 => {
            let task_id_prefix = read_string(bytes, off)?;
            let title_template = read_string(bytes, off)?;
            let action = Action::CreateTask {
                task_id_prefix,
                title_template,
            };
            Automations::validate_action(&action)?;
            Ok(action)
        }
        2 => {
            let member_template = read_string(bytes, off)?;
            let kind = read_string(bytes, off)?;
            let body_template = read_string(bytes, off)?;
            let action = Action::DeliverInbox {
                member_template,
                kind,
                body_template,
            };
            Automations::validate_action(&action)?;
            Ok(action)
        }
        other => Err(Error::Module(format!(
            "snapshot has unknown action discriminant {other}"
        ))),
    }
}

fn read_u8(bytes: &[u8], off: &mut usize) -> Result<u8, Error> {
    let end = off
        .checked_add(1)
        .filter(|&end| end <= bytes.len())
        .ok_or_else(|| Error::Module("snapshot truncated".into()))?;
    let value = bytes[*off];
    *off = end;
    Ok(value)
}

fn read_u64(bytes: &[u8], off: &mut usize) -> Result<u64, Error> {
    let end = off
        .checked_add(8)
        .filter(|&end| end <= bytes.len())
        .ok_or_else(|| Error::Module("snapshot truncated".into()))?;
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&bytes[*off..end]);
    *off = end;
    Ok(u64::from_le_bytes(buf))
}

fn read_string(bytes: &[u8], off: &mut usize) -> Result<String, Error> {
    let len = read_u64(bytes, off)?;
    let len = usize::try_from(len).map_err(|_| Error::Module("snapshot truncated".into()))?;
    if len > bytes.len() - *off {
        return Err(Error::Module("snapshot truncated".into()));
    }
    let value = std::str::from_utf8(&bytes[*off..*off + len])
        .map_err(|_| Error::Module("snapshot string is not utf-8".into()))?;
    *off += len;
    Ok(value.to_owned())
}

#[async_trait::async_trait(?Send)]
impl Module for Automations {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        Self::root_of(&self.rules, &self.history)
    }

    /// advertise the snapshot lane: [`Automations::snapshot`] is the exact
    /// preimage of `root()`, and [`Automations::install`] verifies before
    /// adopting — so a joiner can rebuild this module against the agreed root.
    fn state_sync_handle(&self) -> Result<sdk::StateSyncHandle, Error> {
        Ok(sdk::StateSyncHandle::SnapshotBytes(self.snapshot()))
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        // route by the HOST-ASSIGNED origin (spoof-proof): only chat's own
        // follow-ups reach the hook arm; everything else is an admin op.
        let origin = ctx.env().origin.clone();
        match origin {
            Origin::Module(module) if module == self.chat => {
                self.on_chat_event(ctx, &msg.payload).await
            }
            Origin::Module(module) if module == self.memory => {
                self.on_memory_event(ctx, &msg.payload).await
            }
            _ => match decode_msg(&msg.payload).map_err(Error::Module)? {
                AutomationsMsg::CreateRule {
                    rule_id,
                    trigger,
                    action,
                } => self.stage_create_rule(rule_id, trigger, action, ctx.env().consensus_time),
                AutomationsMsg::SetEnabled { rule_id, enabled } => {
                    self.stage_set_enabled(rule_id, enabled)
                }
                AutomationsMsg::DeleteRule { rule_id } => self.stage_delete_rule(rule_id),
                AutomationsMsg::HookEvent(_) => Err(Error::Module(
                    "hook events must originate from the chat module".into(),
                )),
            },
        }
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            AutomationsQuery::ListRules => Ok(encode_reply(&AutomationsReply::Rules(
                self.effective_rules().into_values().collect(),
            ))),
            AutomationsQuery::GetRule { rule_id } => {
                Ok(encode_reply(&AutomationsReply::Rule(self.rule(&rule_id))))
            }
            AutomationsQuery::RunHistory { rule_id, limit } => {
                let limit = usize::try_from(limit)
                    .unwrap_or(usize::MAX)
                    .min(MAX_RUN_HISTORY);
                let mut matched: Vec<RunRecord> = self
                    .effective_history()
                    .filter(|record| record.rule_id == rule_id)
                    .cloned()
                    .collect();
                if matched.len() > limit {
                    matched = matched.split_off(matched.len() - limit);
                }
                Ok(encode_reply(&AutomationsReply::History(matched)))
            }
        }
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        for (id, staged) in std::mem::take(&mut self.pending_rules) {
            match staged {
                Some(rule) => {
                    self.rules.insert(id, rule);
                }
                None => {
                    self.rules.remove(&id);
                }
            }
        }
        for record in std::mem::take(&mut self.pending_history) {
            self.history.push_back(record);
            while self.history.len() > MAX_RUN_HISTORY {
                self.history.pop_front();
            }
        }
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending_rules.clear();
        self.pending_history.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    use crate::{AutomationsReply, decode_reply, encode_msg, encode_query};
    use chat::{
        Block, Channel, Mark, MessageHead, MessageView, PostPolicy, Span,
        decode_msg as chat_decode_msg, decode_query as chat_decode_query,
        encode_event as chat_encode_event, encode_reply as chat_encode_reply,
    };
    use futures::executor::block_on;
    use inbox::{InboxMsg, decode_msg as inbox_decode_msg};
    use memory::{MemoryEvent, encode_event as memory_encode_event};
    use sdk::{Effect, Env, Event};
    use tasks::{
        Task, TaskStatus, decode_msg as tasks_decode_msg, encode_reply as tasks_encode_reply,
    };

    const CHAT: &str = "chat";
    const TASKS: &str = "tasks";
    const INBOX: &str = "inbox";
    const MEMORY: &str = "memory";
    const ME: &str = "automations";

    /// a minimal `Ctx` capturing emitted msgs and serving canned chat
    /// transcripts / channels and a task list — enough to unit-test `execute`
    /// (including the pre-emit probes) in isolation.
    struct CaptureCtx {
        env: Env,
        /// channel -> messages with contiguous seqs starting at 1. transcript
        /// channels also count as existing for the channel probe.
        transcripts: BTreeMap<String, Vec<MessageView>>,
        /// channels the chat probe reports as existing.
        channels: BTreeSet<String>,
        /// the task list served to the tasks probe.
        tasks: Vec<Task>,
        msgs: Vec<Msg>,
        /// when set, every query returns an error.
        fail_query: bool,
    }

    impl CaptureCtx {
        fn new() -> Self {
            Self {
                env: Env { protocol_version: 0,
                    height: 7,
                    consensus_time: 42,
                    origin: Origin::System,
                    me: ME.into(),
                },
                transcripts: BTreeMap::new(),
                channels: BTreeSet::new(),
                tasks: Vec::new(),
                msgs: Vec::new(),
                fail_query: false,
            }
        }
        fn from_origin(mut self, origin: Origin) -> Self {
            self.env.origin = origin;
            self
        }
        fn from_chat(self) -> Self {
            self.from_origin(Origin::Module(CHAT.into()))
        }
        fn with_transcript(mut self, channel: &str, messages: Vec<MessageView>) -> Self {
            self.channels.insert(channel.into());
            self.transcripts.insert(channel.into(), messages);
            self
        }
        fn with_channel(mut self, channel: &str) -> Self {
            self.channels.insert(channel.into());
            self
        }
        fn with_task(mut self, task_id: &str) -> Self {
            self.tasks.push(Task {
                id: task_id.into(),
                title: task_id.into(),
                status: TaskStatus::Open,
                created_at: 0,
                updated_at: 0,
            });
            self
        }
        fn failing_query(mut self) -> Self {
            self.fail_query = true;
            self
        }
        fn chat_msgs(&self) -> Vec<ChatMsg> {
            self.msgs
                .iter()
                .filter(|m| m.target == CHAT)
                .map(|m| chat_decode_msg(&m.payload).expect("chat msg"))
                .collect()
        }
        fn task_msgs(&self) -> Vec<TaskMsg> {
            self.msgs
                .iter()
                .filter(|m| m.target == TASKS)
                .map(|m| tasks_decode_msg(&m.payload).expect("task msg"))
                .collect()
        }
        fn inbox_msgs(&self) -> Vec<InboxMsg> {
            self.msgs
                .iter()
                .filter(|m| m.target == INBOX)
                .map(|m| inbox_decode_msg(&m.payload).expect("inbox msg"))
                .collect()
        }
    }

    #[async_trait::async_trait(?Send)]
    impl Ctx for CaptureCtx {
        fn env(&self) -> &Env {
            &self.env
        }
        fn module_root(&self, _target: &str) -> Option<StateRoot> {
            None
        }
        async fn query(&self, target: &str, req: &[u8]) -> Result<Vec<u8>, Error> {
            if self.fail_query {
                return Err(Error::Module("query failed".into()));
            }
            match target {
                CHAT => match chat_decode_query(req).map_err(Error::Module)? {
                    ChatQuery::MessagesRange {
                        channel_id,
                        from_seq,
                        limit,
                    } => {
                        let transcript = self.transcripts.get(&channel_id).ok_or_else(|| {
                            Error::Module(format!("unknown channel: {channel_id}"))
                        })?;
                        let head = transcript.len() as u64;
                        let from = from_seq.max(1);
                        let mut window = Vec::new();
                        if limit > 0 && from <= head {
                            let to = head.min(from + limit - 1);
                            window = transcript[(from - 1) as usize..to as usize].to_vec();
                        }
                        Ok(chat_encode_reply(&ChatReply::Messages(window)))
                    }
                    ChatQuery::Channel { channel_id } => {
                        let channel = self.channels.contains(&channel_id).then(|| Channel {
                            id: channel_id.clone(),
                            name: channel_id,
                            created_at: 0,
                            head_seq: 0,
                            post_policy: PostPolicy::Open,
                            hooks: Vec::new(),
                            pinned: Vec::new(),
                        });
                        Ok(chat_encode_reply(&ChatReply::Channel(channel)))
                    }
                    ChatQuery::Message { message_id } => {
                        Ok(chat_encode_reply(&ChatReply::Message(
                            self.transcripts
                                .values()
                                .flatten()
                                .find(|view| view.head.message_id == message_id)
                                .cloned(),
                        )))
                    }
                    _ => Err(Error::QueryUnsupported),
                },
                TASKS => Ok(tasks_encode_reply(&TaskReply::Tasks(self.tasks.clone()))),
                other => Err(Error::UnknownModule(other.into())),
            }
        }
        fn emit_msg(&mut self, msg: Msg) {
            self.msgs.push(msg);
        }
        fn emit_event(&mut self, _ev: Event) {}
        fn request_effect(&mut self, _eff: Effect) {}
    }

    // ---- fixtures -----------------------------------------------------------

    fn module() -> Automations {
        Automations::new(ME, CHAT, TASKS, INBOX, MEMORY)
    }

    fn user(byte: u8) -> AuthorRef {
        AuthorRef::User(vec![byte; 4])
    }

    fn post_trigger(channel: Option<&str>, text_contains: Option<&str>) -> Trigger {
        Trigger::MessagePosted {
            channel_id: channel.map(Into::into),
            mention: None,
            text_contains: text_contains.map(Into::into),
        }
    }

    fn post_action(channel: &str, template: &str) -> Action {
        Action::PostMessage {
            channel_id: channel.into(),
            template: template.into(),
        }
    }

    fn task_action(prefix: &str, title: &str) -> Action {
        Action::CreateTask {
            task_id_prefix: prefix.into(),
            title_template: title.into(),
        }
    }

    fn inbox_action(member: &str, kind: &str, body: &str) -> Action {
        Action::DeliverInbox {
            member_template: member.into(),
            kind: kind.into(),
            body_template: body.into(),
        }
    }

    fn memory_trigger(prefix: Option<&str>, meta_kind: Option<&str>) -> Trigger {
        Trigger::MemoryPublished {
            prefix: prefix.map(Into::into),
            meta_kind: meta_kind.map(Into::into),
            author_contains: None,
        }
    }

    fn admin(m: &AutomationsMsg) -> Msg {
        Msg {
            target: ME.into(),
            payload: encode_msg(m),
        }
    }

    fn create(rule_id: &str, trigger: Trigger, action: Action) -> Msg {
        admin(&AutomationsMsg::CreateRule {
            rule_id: rule_id.into(),
            trigger,
            action,
        })
    }

    /// a hook event as chat delivers it: raw ChatEvent bytes.
    fn posted(channel: &str, seq: u64, author: AuthorRef, mentions: Vec<AuthorRef>) -> Msg {
        Msg {
            target: ME.into(),
            payload: chat_encode_event(&ChatEvent::MessagePosted {
                channel_id: channel.into(),
                seq,
                thread_root: None,
                author,
                mentions,
            }),
        }
    }

    fn memory_published(path: &str, generation: u64, meta: &[(&str, &str)], author: &str) -> Msg {
        Msg {
            target: ME.into(),
            payload: memory_encode_event(&MemoryEvent::Published {
                path: path.into(),
                generation,
                meta: meta
                    .iter()
                    .map(|(key, value)| ((*key).into(), (*value).into()))
                    .collect(),
                author: author.into(),
            }),
        }
    }

    fn message(channel: &str, seq: u64, author: AuthorRef, blocks: Vec<Block>) -> MessageView {
        MessageView {
            channel_id: channel.into(),
            seq,
            head: MessageHead {
                message_id: format!("{channel}-m{seq}"),
                author,
                blocks,
                created_at: 0,
                rev: 0,
                edited_at: None,
                base_rev: None,
                deleted: false,
                thread: None,
                reply_count: 0,
                last_reply_seq: None,
            },
            reactions: Vec::new(),
            channel_head_seq: seq,
        }
    }

    fn list_rules(m: &Automations) -> Vec<Rule> {
        match decode_reply(
            &block_on(m.query(&encode_query(&AutomationsQuery::ListRules))).expect("query"),
        )
        .expect("reply")
        {
            AutomationsReply::Rules(rules) => rules,
            other => panic!("expected Rules, got {other:?}"),
        }
    }

    fn get_rule(m: &Automations, rule_id: &str) -> Option<Rule> {
        match decode_reply(
            &block_on(m.query(&encode_query(&AutomationsQuery::GetRule {
                rule_id: rule_id.into(),
            })))
            .expect("query"),
        )
        .expect("reply")
        {
            AutomationsReply::Rule(rule) => rule,
            other => panic!("expected Rule, got {other:?}"),
        }
    }

    fn history(m: &Automations, rule_id: &str, limit: u64) -> Vec<RunRecord> {
        match decode_reply(
            &block_on(m.query(&encode_query(&AutomationsQuery::RunHistory {
                rule_id: rule_id.into(),
                limit,
            })))
            .expect("query"),
        )
        .expect("reply")
        {
            AutomationsReply::History(records) => records,
            other => panic!("expected History, got {other:?}"),
        }
    }

    fn exec(m: &mut Automations, ctx: &mut CaptureCtx, msg: &Msg) -> Result<(), Error> {
        block_on(m.execute(ctx, msg))
    }

    // ---- rule CRUD ----------------------------------------------------------

    #[test]
    fn create_list_and_commit_rules() {
        let mut m = module();
        let mut ctx = CaptureCtx::new();
        exec(
            &mut m,
            &mut ctx,
            &create("r-b", post_trigger(None, None), task_action("t", "T")),
        )
        .expect("create b");
        exec(
            &mut m,
            &mut ctx,
            &create("r-a", post_trigger(None, None), task_action("t", "T")),
        )
        .expect("create a");

        // staged reads see both, in rule_id order; committed root has not moved.
        let root0 = m.root();
        let ids: Vec<String> = list_rules(&m).into_iter().map(|r| r.rule_id).collect();
        assert_eq!(ids, ["r-a", "r-b"], "list order is deterministic");
        assert_eq!(
            m.root(),
            root0,
            "staged writes do not move the committed root"
        );

        block_on(m.commit_block()).expect("commit");
        assert_ne!(m.root(), root0, "commit moves the root");
        assert!(get_rule(&m, "r-a").expect("r-a").enabled);
        assert_eq!(get_rule(&m, "r-a").expect("r-a").created_at, 42);
    }

    #[test]
    fn duplicate_rule_id_is_rejected() {
        let mut m = module();
        let mut ctx = CaptureCtx::new();
        exec(
            &mut m,
            &mut ctx,
            &create("dup", post_trigger(None, None), task_action("t", "T")),
        )
        .expect("create");
        block_on(m.commit_block()).expect("commit");
        let err = exec(
            &mut m,
            &mut ctx,
            &create("dup", post_trigger(None, None), task_action("t", "T")),
        )
        .expect_err("duplicate must reject");
        assert!(matches!(err, Error::Module(msg) if msg.contains("already exists")));
    }

    #[test]
    fn set_enabled_and_delete() {
        let mut m = module();
        let mut ctx = CaptureCtx::new();
        exec(
            &mut m,
            &mut ctx,
            &create("r", post_trigger(None, None), task_action("t", "T")),
        )
        .expect("create");
        block_on(m.commit_block()).expect("commit");

        exec(
            &mut m,
            &mut ctx,
            &admin(&AutomationsMsg::SetEnabled {
                rule_id: "r".into(),
                enabled: false,
            }),
        )
        .expect("disable");
        block_on(m.commit_block()).expect("commit");
        assert!(!get_rule(&m, "r").expect("r").enabled);

        exec(
            &mut m,
            &mut ctx,
            &admin(&AutomationsMsg::DeleteRule {
                rule_id: "r".into(),
            }),
        )
        .expect("delete");
        block_on(m.commit_block()).expect("commit");
        assert!(get_rule(&m, "r").is_none());
        assert!(list_rules(&m).is_empty());
    }

    #[test]
    fn set_enabled_unknown_rule_rejected() {
        let mut m = module();
        let mut ctx = CaptureCtx::new();
        let err = exec(
            &mut m,
            &mut ctx,
            &admin(&AutomationsMsg::SetEnabled {
                rule_id: "ghost".into(),
                enabled: true,
            }),
        )
        .expect_err("unknown rule");
        assert!(matches!(err, Error::Module(msg) if msg.contains("unknown rule")));
    }

    #[test]
    fn caps_are_enforced_at_execute() {
        let mut m = module();
        let mut ctx = CaptureCtx::new();

        // empty rule_id.
        assert!(
            exec(
                &mut m,
                &mut ctx,
                &create("", post_trigger(None, None), task_action("t", "T")),
            )
            .is_err()
        );

        // oversize template.
        let huge = "x".repeat(MAX_TEMPLATE_BYTES + 1);
        assert!(
            exec(
                &mut m,
                &mut ctx,
                &create("big", post_trigger(None, None), post_action("c", &huge)),
            )
            .is_err()
        );

        // oversize rule_id.
        let long_id = "r".repeat(MAX_ID_BYTES + 1);
        assert!(
            exec(
                &mut m,
                &mut ctx,
                &create(&long_id, post_trigger(None, None), task_action("t", "T")),
            )
            .is_err()
        );

        // empty action channel_id.
        assert!(
            exec(
                &mut m,
                &mut ctx,
                &create("x", post_trigger(None, None), post_action("", "hi")),
            )
            .is_err()
        );

        // empty inbox kind.
        assert!(
            exec(
                &mut m,
                &mut ctx,
                &create(
                    "inbox-empty-kind",
                    post_trigger(None, None),
                    inbox_action("alice", "", "body")
                ),
            )
            .is_err()
        );

        // oversized inbox kind.
        assert!(
            exec(
                &mut m,
                &mut ctx,
                &create(
                    "inbox-big-kind",
                    post_trigger(None, None),
                    inbox_action("alice", &"k".repeat(65), "body")
                ),
            )
            .is_err()
        );

        // invalid memory prefix.
        assert!(
            exec(
                &mut m,
                &mut ctx,
                &create(
                    "bad-memory-prefix",
                    memory_trigger(Some("relative"), None),
                    inbox_action("alice", "memory", "body")
                ),
            )
            .is_err()
        );

        // oversized memory prefix.
        assert!(
            exec(
                &mut m,
                &mut ctx,
                &create(
                    "big-memory-prefix",
                    memory_trigger(Some(&format!("/{}", "p".repeat(512))), None),
                    inbox_action("alice", "memory", "body")
                ),
            )
            .is_err()
        );

        assert!(list_rules(&m).is_empty(), "no rejected rule was staged");
    }

    // ---- origin + author gating --------------------------------------------

    #[test]
    fn hook_event_from_non_chat_origin_is_rejected() {
        let mut m = module();
        // an explicit HookEvent wrapper from an external submitter is a spoof.
        let mut ext = CaptureCtx::new().from_origin(Origin::External(vec![9; 4]));
        let err = exec(
            &mut m,
            &mut ext,
            &Msg {
                target: ME.into(),
                payload: encode_msg(&AutomationsMsg::HookEvent(vec![1, 2, 3])),
            },
        )
        .expect_err("hook from non-chat origin must reject");
        assert!(matches!(err, Error::Module(msg) if msg.contains("chat module")));

        // raw ChatEvent bytes from a non-chat origin fail to decode as an
        // AutomationsMsg — also rejected.
        assert!(exec(&mut m, &mut ext, &posted("general", 1, user(1), Vec::new()),).is_err());
    }

    #[test]
    fn only_user_authored_posts_fire() {
        let mut m = module();
        let mut ctx = CaptureCtx::new();
        exec(
            &mut m,
            &mut ctx,
            &create("r", post_trigger(None, None), task_action("t", "T")),
        )
        .expect("create");
        block_on(m.commit_block()).expect("commit");

        // a module-authored post (e.g. our own follow-up) never fires.
        let mut chat_ctx = CaptureCtx::new().from_chat();
        exec(
            &mut m,
            &mut chat_ctx,
            &posted(
                "general",
                1,
                AuthorRef::Module("automations".into()),
                Vec::new(),
            ),
        )
        .expect("no-fail arm");
        assert!(chat_ctx.msgs.is_empty(), "module posts must not trigger");
        block_on(m.commit_block()).expect("commit");
        assert_eq!(get_rule(&m, "r").expect("r").fire_count, 0);
        assert!(history(&m, "r", 16).is_empty());
    }

    #[test]
    fn memory_events_only_from_memory_origin_and_self_author_is_loop_guarded() {
        let mut m = module();
        let mut ctx = CaptureCtx::new();
        exec(
            &mut m,
            &mut ctx,
            &create(
                "mem",
                memory_trigger(Some("/docs"), None),
                inbox_action("alice", "memory", "published {path}"),
            ),
        )
        .expect("create");
        block_on(m.commit_block()).expect("commit");

        // A raw MemoryEvent from chat origin is decoded as a chat event and must
        // be a staged no-op, not a memory fire.
        let mut wrong_origin = CaptureCtx::new().from_chat();
        exec(
            &mut m,
            &mut wrong_origin,
            &memory_published("/docs/a", 1, &[], "writer"),
        )
        .expect("wrong-origin raw event is no-op");
        assert!(wrong_origin.msgs.is_empty());

        // A memory event authored by this module is inert as a defense-in-depth
        // loop guard even though automations has no memory-writing action.
        let mut memory_ctx = CaptureCtx::new().from_origin(Origin::Module(MEMORY.into()));
        exec(
            &mut m,
            &mut memory_ctx,
            &memory_published("/docs/a", 1, &[], ME),
        )
        .expect("self-authored memory event is no-op");
        assert!(memory_ctx.msgs.is_empty());
        block_on(m.commit_block()).expect("commit");
        assert_eq!(get_rule(&m, "mem").expect("mem").fire_count, 0);
        assert!(history(&m, "mem", 16).is_empty());
    }

    // ---- matching + action emission ----------------------------------------

    #[test]
    fn create_task_action_emits_deterministic_task_id() {
        let mut m = module();
        let mut ctx = CaptureCtx::new();
        exec(
            &mut m,
            &mut ctx,
            &create(
                "r",
                post_trigger(Some("general"), None),
                task_action("todo", "from {channel} #{seq}"),
            ),
        )
        .expect("create");
        block_on(m.commit_block()).expect("commit");

        let mut chat_ctx = CaptureCtx::new().from_chat();
        exec(
            &mut m,
            &mut chat_ctx,
            &posted("general", 5, user(1), Vec::new()),
        )
        .expect("fire");
        let tasks = chat_ctx.task_msgs();
        assert_eq!(tasks.len(), 1);
        let TaskMsg::CreateTask { task_id, title } = &tasks[0] else {
            panic!("expected CreateTask");
        };
        assert_eq!(task_id, "todo-general-5", "deterministic task id");
        assert_eq!(title, "from general #5", "substituted title");

        block_on(m.commit_block()).expect("commit");
        assert_eq!(get_rule(&m, "r").expect("r").fire_count, 1);
        let recs = history(&m, "r", 16);
        assert_eq!(recs.len(), 1);
        assert!(recs[0].action_ok);
        assert_eq!(recs[0].seq, 5);
        assert_eq!(recs[0].height, 7);
    }

    #[test]
    fn post_message_action_emits_deterministic_message_id() {
        let mut m = module();
        let mut ctx = CaptureCtx::new();
        exec(
            &mut m,
            &mut ctx,
            &create(
                "greet",
                post_trigger(Some("general"), None),
                post_action("announce", "welcome from {channel}/{seq}"),
            ),
        )
        .expect("create");
        block_on(m.commit_block()).expect("commit");

        let mut chat_ctx = CaptureCtx::new().from_chat().with_channel("announce");
        exec(
            &mut m,
            &mut chat_ctx,
            &posted("general", 3, user(2), Vec::new()),
        )
        .expect("fire");
        let posts = chat_ctx.chat_msgs();
        assert_eq!(posts.len(), 1);
        let ChatMsg::PostMessage {
            channel_id,
            message_id,
            blocks,
            ..
        } = &posts[0]
        else {
            panic!("expected PostMessage");
        };
        assert_eq!(channel_id, "announce", "posts to the action channel");
        assert_eq!(
            message_id, "auto-greet-general-3",
            "deterministic message id"
        );
        assert_eq!(blocks, &vec![Block::paragraph("welcome from general/3")]);
    }

    #[test]
    fn channel_filter_gates_matching() {
        let mut m = module();
        let mut ctx = CaptureCtx::new();
        exec(
            &mut m,
            &mut ctx,
            &create("r", post_trigger(Some("ops"), None), task_action("t", "T")),
        )
        .expect("create");
        block_on(m.commit_block()).expect("commit");

        // wrong channel: no fire.
        let mut wrong = CaptureCtx::new().from_chat();
        exec(
            &mut m,
            &mut wrong,
            &posted("general", 1, user(1), Vec::new()),
        )
        .expect("no fire");
        assert!(wrong.msgs.is_empty());

        // right channel: fires.
        let mut right = CaptureCtx::new().from_chat();
        exec(&mut m, &mut right, &posted("ops", 1, user(1), Vec::new())).expect("fire");
        assert_eq!(right.task_msgs().len(), 1);
    }

    #[test]
    fn mention_filter_matches_agent_display() {
        let mut m = module();
        let mut ctx = CaptureCtx::new();
        exec(
            &mut m,
            &mut ctx,
            &create(
                "r",
                Trigger::MessagePosted {
                    channel_id: None,
                    mention: Some("helper".into()),
                    text_contains: None,
                },
                task_action("t", "T"),
            ),
        )
        .expect("create");
        block_on(m.commit_block()).expect("commit");

        let helper = AuthorRef::Agent {
            module: "agent".into(),
            agent_id: "helper".into(),
        };
        // mention present -> fire.
        let mut hit = CaptureCtx::new().from_chat();
        exec(
            &mut m,
            &mut hit,
            &posted("general", 1, user(1), vec![helper.clone()]),
        )
        .expect("fire");
        assert_eq!(hit.task_msgs().len(), 1);

        // no matching mention -> no fire.
        let mut miss = CaptureCtx::new().from_chat();
        exec(
            &mut m,
            &mut miss,
            &posted("general", 2, user(1), Vec::new()),
        )
        .expect("no fire");
        assert!(miss.msgs.is_empty());
    }

    #[test]
    fn text_contains_filter_fetches_message_once() {
        let mut m = module();
        let mut ctx = CaptureCtx::new();
        exec(
            &mut m,
            &mut ctx,
            &create(
                "r",
                post_trigger(Some("general"), Some("deploy")),
                task_action("t", "seen: {text}"),
            ),
        )
        .expect("create");
        block_on(m.commit_block()).expect("commit");

        let msg_hit = message(
            "general",
            1,
            user(1),
            vec![Block::paragraph("please deploy now")],
        );
        let mut hit = CaptureCtx::new()
            .from_chat()
            .with_transcript("general", vec![msg_hit]);
        exec(&mut m, &mut hit, &posted("general", 1, user(1), Vec::new())).expect("fire");
        let tasks = hit.task_msgs();
        assert_eq!(tasks.len(), 1);
        let TaskMsg::CreateTask { title, .. } = &tasks[0] else {
            panic!("expected CreateTask");
        };
        assert_eq!(title, "seen: please deploy now", "{{text}} substituted");

        // text without the substring -> no fire.
        let msg_miss = message(
            "general",
            1,
            user(1),
            vec![Block::paragraph("just chatting")],
        );
        let mut miss = CaptureCtx::new()
            .from_chat()
            .with_transcript("general", vec![msg_miss]);
        exec(
            &mut m,
            &mut miss,
            &posted("general", 1, user(1), Vec::new()),
        )
        .expect("no fire");
        assert!(miss.msgs.is_empty());
    }

    #[test]
    fn chat_trigger_can_deliver_inbox_with_chat_placeholders() {
        let mut m = module();
        let mut ctx = CaptureCtx::new();
        exec(
            &mut m,
            &mut ctx,
            &create(
                "notify-chat",
                post_trigger(Some("general"), None),
                inbox_action(
                    "{mention}",
                    "chat",
                    "channel={channel} seq={seq} author={author} text={text} mention={mention} path=[{path}] generation=[{generation}]",
                ),
            ),
        )
        .expect("create");
        block_on(m.commit_block()).expect("commit");

        let mentioned = AuthorRef::Agent {
            module: "agent".into(),
            agent_id: "helper".into(),
        };
        let msg = message(
            "general",
            1,
            user(3),
            vec![Block::paragraph("please review")],
        );
        let mut chat_ctx = CaptureCtx::new()
            .from_chat()
            .with_transcript("general", vec![msg]);
        exec(
            &mut m,
            &mut chat_ctx,
            &posted("general", 1, user(3), vec![mentioned]),
        )
        .expect("fire");

        let delivered = chat_ctx.inbox_msgs();
        assert_eq!(delivered.len(), 1);
        let InboxMsg::Deliver { member, kind, body } = &delivered[0] else {
            panic!("expected Deliver");
        };
        assert_eq!(member, "agent/helper", "member uses first mention display");
        assert_eq!(kind, "chat");
        assert_eq!(
            body,
            "channel=general seq=1 author=user:03030303 text=please review mention=agent/helper path=[] generation=[]"
        );
    }

    #[test]
    fn memory_trigger_matches_and_delivers_inbox_with_memory_placeholders() {
        let mut m = module();
        let mut ctx = CaptureCtx::new();
        exec(
            &mut m,
            &mut ctx,
            &create(
                "notify-memory",
                Trigger::MemoryPublished {
                    prefix: Some("/docs".into()),
                    meta_kind: Some("decision".into()),
                    author_contains: Some("writer".into()),
                },
                inbox_action(
                    "{author}",
                    "memory",
                    "channel=[{channel}] seq=[{seq}] author={author} text=[{text}] mention=[{mention}] path={path} generation={generation}",
                ),
            ),
        )
        .expect("create");
        block_on(m.commit_block()).expect("commit");

        // non-subtree prefix match must miss: /docs does not match /docs2.
        let mut miss = CaptureCtx::new().from_origin(Origin::Module(MEMORY.into()));
        exec(
            &mut m,
            &mut miss,
            &memory_published("/docs2/a", 1, &[("kind", "decision")], "writer-1"),
        )
        .expect("prefix miss");
        assert!(miss.msgs.is_empty());

        let mut hit = CaptureCtx::new().from_origin(Origin::Module(MEMORY.into()));
        exec(
            &mut m,
            &mut hit,
            &memory_published("/docs/a", 3, &[("kind", "decision")], "writer-1"),
        )
        .expect("fire");
        let delivered = hit.inbox_msgs();
        assert_eq!(delivered.len(), 1);
        let InboxMsg::Deliver { member, kind, body } = &delivered[0] else {
            panic!("expected Deliver");
        };
        assert_eq!(member, "writer-1");
        assert_eq!(kind, "memory");
        assert_eq!(
            body,
            "channel=[] seq=[] author=writer-1 text=[] mention=[] path=/docs/a generation=3"
        );

        block_on(m.commit_block()).expect("commit");
        assert_eq!(get_rule(&m, "notify-memory").expect("rule").fire_count, 1);
        let recs = history(&m, "notify-memory", 16);
        assert_eq!(recs.len(), 1);
        assert!(recs[0].action_ok);
        assert_eq!(recs[0].channel_id, "/docs/a");
        assert_eq!(recs[0].seq, 3);
    }

    #[test]
    fn failed_text_fetch_is_recorded_not_guessed_empty() {
        let mut m = module();
        let mut ctx = CaptureCtx::new();
        exec(
            &mut m,
            &mut ctx,
            &create(
                "r",
                post_trigger(Some("general"), Some("deploy")),
                task_action("t", "T"),
            ),
        )
        .expect("create");
        block_on(m.commit_block()).expect("commit");

        // the fetch fails -> the text-needing rule cannot be evaluated: a
        // recorded failure (never empty-text guessing), no emit, and crucially
        // the block is NOT aborted.
        let mut ctx = CaptureCtx::new().failing_query().from_chat();
        exec(&mut m, &mut ctx, &posted("general", 1, user(1), Vec::new()))
            .expect("no-fail arm survives a failed fetch");
        assert!(ctx.msgs.is_empty());
        block_on(m.commit_block()).expect("commit");
        assert_eq!(get_rule(&m, "r").expect("r").fire_count, 0);
        let recs = history(&m, "r", 4);
        assert_eq!(recs.len(), 1);
        assert!(!recs[0].action_ok);
        assert_eq!(recs[0].detail, "text fetch failed");
    }

    #[test]
    fn legitimately_empty_body_is_valid_text() {
        let mut m = module();
        let mut ctx = CaptureCtx::new();
        // an empty text_contains filter matches an empty body; {text}
        // substitutes as empty. only a FAILED fetch is a failure.
        exec(
            &mut m,
            &mut ctx,
            &create(
                "r",
                post_trigger(Some("general"), Some("")),
                task_action("t", "seen [{text}]"),
            ),
        )
        .expect("create");
        block_on(m.commit_block()).expect("commit");

        let empty_body = message("general", 1, user(1), vec![Block::paragraph("")]);
        let mut chat_ctx = CaptureCtx::new()
            .from_chat()
            .with_transcript("general", vec![empty_body]);
        exec(
            &mut m,
            &mut chat_ctx,
            &posted("general", 1, user(1), Vec::new()),
        )
        .expect("fire");
        let tasks = chat_ctx.task_msgs();
        assert_eq!(tasks.len(), 1, "an empty body is valid content");
        let TaskMsg::CreateTask { title, .. } = &tasks[0] else {
            panic!("expected CreateTask");
        };
        assert_eq!(title, "seen []");
    }

    // ---- malformed action + budget -----------------------------------------

    #[test]
    fn empty_template_records_action_ok_false_without_failing() {
        let mut m = module();
        let mut ctx = CaptureCtx::new();
        exec(
            &mut m,
            &mut ctx,
            &create("r", post_trigger(None, None), post_action("c", "")),
        )
        .expect("create");
        block_on(m.commit_block()).expect("commit");

        let mut chat_ctx = CaptureCtx::new().from_chat();
        exec(
            &mut m,
            &mut chat_ctx,
            &posted("general", 1, user(1), Vec::new()),
        )
        .expect("no block failure");
        assert!(chat_ctx.msgs.is_empty(), "no action emitted");

        block_on(m.commit_block()).expect("commit");
        assert_eq!(
            get_rule(&m, "r").expect("r").fire_count,
            0,
            "no successful fire"
        );
        let recs = history(&m, "r", 16);
        assert_eq!(recs.len(), 1);
        assert!(!recs[0].action_ok);
        assert!(recs[0].detail.contains("empty message"));
    }

    #[test]
    fn inbox_member_over_cap_records_action_ok_false_without_emitting() {
        let mut m = module();
        let mut ctx = CaptureCtx::new();
        exec(
            &mut m,
            &mut ctx,
            &create(
                "member-cap",
                memory_trigger(Some("/docs"), None),
                inbox_action("{path}", "memory", "body"),
            ),
        )
        .expect("create");
        block_on(m.commit_block()).expect("commit");

        let long_path = format!("/docs/{}", "x".repeat(300));
        let mut memory_ctx = CaptureCtx::new().from_origin(Origin::Module(MEMORY.into()));
        exec(
            &mut m,
            &mut memory_ctx,
            &memory_published(&long_path, 1, &[], "writer"),
        )
        .expect("fire records failure");
        assert!(memory_ctx.msgs.is_empty());
        block_on(m.commit_block()).expect("commit");

        let recs = history(&m, "member-cap", 16);
        assert_eq!(recs.len(), 1);
        assert!(!recs[0].action_ok);
        assert!(recs[0].detail.contains("inbox member exceeds cap"));
        assert_eq!(get_rule(&m, "member-cap").expect("rule").fire_count, 0);
    }

    #[test]
    fn inbox_body_over_cap_records_action_ok_false_without_emitting() {
        let mut m = module();
        let mut ctx = CaptureCtx::new();
        exec(
            &mut m,
            &mut ctx,
            &create(
                "body-cap",
                memory_trigger(Some("/docs"), None),
                inbox_action("alice", "memory", "{author}"),
            ),
        )
        .expect("create");
        block_on(m.commit_block()).expect("commit");

        let long_author = "a".repeat(16 * 1024 + 1);
        let mut memory_ctx = CaptureCtx::new().from_origin(Origin::Module(MEMORY.into()));
        exec(
            &mut m,
            &mut memory_ctx,
            &memory_published("/docs/a", 1, &[], &long_author),
        )
        .expect("fire records failure");
        assert!(memory_ctx.msgs.is_empty());
        block_on(m.commit_block()).expect("commit");

        let recs = history(&m, "body-cap", 16);
        assert_eq!(recs.len(), 1);
        assert!(!recs[0].action_ok);
        assert!(recs[0].detail.contains("inbox body exceeds cap"));
        assert_eq!(get_rule(&m, "body-cap").expect("rule").fire_count, 0);
    }

    #[test]
    fn action_budget_caps_emissions_per_event() {
        let mut m = module();
        let mut ctx = CaptureCtx::new();
        // create more matching rules than the budget allows.
        let n = MAX_ACTIONS_PER_EVENT + 3;
        for i in 0..n {
            exec(
                &mut m,
                &mut ctx,
                &create(
                    &format!("r{i:02}"),
                    post_trigger(None, None),
                    task_action("t", "T"),
                ),
            )
            .expect("create");
        }
        block_on(m.commit_block()).expect("commit");

        let mut chat_ctx = CaptureCtx::new().from_chat();
        exec(
            &mut m,
            &mut chat_ctx,
            &posted("general", 1, user(1), Vec::new()),
        )
        .expect("fire");
        assert_eq!(
            chat_ctx.task_msgs().len(),
            MAX_ACTIONS_PER_EVENT,
            "exactly the budget is emitted"
        );
        block_on(m.commit_block()).expect("commit");

        // the first MAX rules (ascending) fired; the rest are budget-skipped.
        let fired = (0..MAX_ACTIONS_PER_EVENT)
            .filter(|i| get_rule(&m, &format!("r{i:02}")).expect("rule").fire_count == 1)
            .count();
        assert_eq!(fired, MAX_ACTIONS_PER_EVENT);
        let skipped = get_rule(&m, &format!("r{:02}", n - 1)).expect("last rule");
        assert_eq!(skipped.fire_count, 0);
        let recs = history(&m, &format!("r{:02}", n - 1), 4);
        assert_eq!(recs.len(), 1);
        assert!(!recs[0].action_ok);
        assert_eq!(recs[0].detail, "action budget exceeded");
    }

    #[test]
    fn disabled_rules_do_not_fire() {
        let mut m = module();
        let mut ctx = CaptureCtx::new();
        exec(
            &mut m,
            &mut ctx,
            &create("r", post_trigger(None, None), task_action("t", "T")),
        )
        .expect("create");
        exec(
            &mut m,
            &mut ctx,
            &admin(&AutomationsMsg::SetEnabled {
                rule_id: "r".into(),
                enabled: false,
            }),
        )
        .expect("disable");
        block_on(m.commit_block()).expect("commit");

        let mut chat_ctx = CaptureCtx::new().from_chat();
        exec(
            &mut m,
            &mut chat_ctx,
            &posted("general", 1, user(1), Vec::new()),
        )
        .expect("no fire");
        assert!(chat_ctx.msgs.is_empty());
    }

    // ---- pre-emit probes + guards -------------------------------------------

    #[test]
    fn missing_target_channel_is_recorded_not_emitted() {
        let mut m = module();
        let mut ctx = CaptureCtx::new();
        exec(
            &mut m,
            &mut ctx,
            &create("r", post_trigger(None, None), post_action("ghost", "hi")),
        )
        .expect("create");
        block_on(m.commit_block()).expect("commit");

        // "ghost" is not a known channel: the probe records, never emits.
        let mut chat_ctx = CaptureCtx::new().from_chat();
        exec(
            &mut m,
            &mut chat_ctx,
            &posted("general", 1, user(1), Vec::new()),
        )
        .expect("no-fail arm");
        assert!(chat_ctx.msgs.is_empty(), "no post to a missing channel");
        block_on(m.commit_block()).expect("commit");
        let recs = history(&m, "r", 4);
        assert_eq!(recs.len(), 1);
        assert!(!recs[0].action_ok);
        assert!(recs[0].detail.contains("does not exist"));
        assert_eq!(get_rule(&m, "r").expect("r").fire_count, 0);
    }

    #[test]
    fn squatted_message_id_is_caught_by_probe() {
        let mut m = module();
        let mut ctx = CaptureCtx::new();
        exec(
            &mut m,
            &mut ctx,
            &create("r", post_trigger(None, None), post_action("general", "hi")),
        )
        .expect("create");
        block_on(m.commit_block()).expect("commit");

        // the deterministic id for the seq-1 fire is already taken: a user
        // pre-posted it (id squatting). the probe records, never emits — the
        // emit would abort the posting block at chat's duplicate-id check.
        let mut squatted = message("general", 1, user(9), vec![Block::paragraph("squat")]);
        squatted.head.message_id = "auto-r-general-1".into();
        let mut chat_ctx = CaptureCtx::new()
            .from_chat()
            .with_transcript("general", vec![squatted]);
        exec(
            &mut m,
            &mut chat_ctx,
            &posted("general", 1, user(1), Vec::new()),
        )
        .expect("no-fail arm");
        assert!(chat_ctx.msgs.is_empty(), "no emit against a squatted id");
        block_on(m.commit_block()).expect("commit");
        let recs = history(&m, "r", 4);
        assert_eq!(recs.len(), 1);
        assert!(!recs[0].action_ok);
        assert!(recs[0].detail.contains("already taken"));
    }

    #[test]
    fn task_id_collision_is_caught_by_probe() {
        let mut m = module();
        let mut ctx = CaptureCtx::new();
        exec(
            &mut m,
            &mut ctx,
            &create("r", post_trigger(None, None), task_action("auto", "T")),
        )
        .expect("create");
        block_on(m.commit_block()).expect("commit");

        let mut chat_ctx = CaptureCtx::new().from_chat().with_task("auto-general-5");
        exec(
            &mut m,
            &mut chat_ctx,
            &posted("general", 5, user(1), Vec::new()),
        )
        .expect("no-fail arm");
        assert!(chat_ctx.msgs.is_empty(), "no emit against a taken task id");
        block_on(m.commit_block()).expect("commit");
        let recs = history(&m, "r", 4);
        assert_eq!(recs.len(), 1);
        assert!(!recs[0].action_ok);
        assert!(recs[0].detail.contains("already exists"));
    }

    #[test]
    fn oversized_composed_id_is_recorded_not_emitted() {
        let mut m = module();
        let mut ctx = CaptureCtx::new();
        exec(
            &mut m,
            &mut ctx,
            &create("r", post_trigger(None, None), task_action("auto", "T")),
        )
        .expect("create");
        block_on(m.commit_block()).expect("commit");

        // an event channel long enough to push the composed id over the cap.
        let long_channel = "c".repeat(MAX_ID_BYTES);
        let mut chat_ctx = CaptureCtx::new().from_chat();
        exec(
            &mut m,
            &mut chat_ctx,
            &posted(&long_channel, 1, user(1), Vec::new()),
        )
        .expect("no-fail arm");
        assert!(chat_ctx.msgs.is_empty(), "no emit with an oversized id");
        block_on(m.commit_block()).expect("commit");
        let recs = history(&m, "r", 4);
        assert_eq!(recs.len(), 1);
        assert!(!recs[0].action_ok);
        assert_eq!(recs[0].detail, "composed id exceeds cap");
    }

    #[test]
    fn fire_count_saturates_at_u64_max() {
        // craft a committed state whose rule already sits at u64::MAX via the
        // canonical codec (install verifies it against its own root), then
        // fire: the count must saturate, not wrap.
        let mut rules: BTreeMap<String, Rule> = BTreeMap::new();
        rules.insert(
            "r".into(),
            Rule {
                rule_id: "r".into(),
                enabled: true,
                trigger: Trigger::MessagePosted {
                    channel_id: None,
                    mention: None,
                    text_contains: None,
                },
                action: Action::CreateTask {
                    task_id_prefix: "auto".into(),
                    title_template: "T".into(),
                },
                created_at: 0,
                fire_count: u64::MAX,
            },
        );
        let history_ring: VecDeque<RunRecord> = VecDeque::new();
        let bytes = encode_state(&rules, &history_ring);
        let root = Automations::root_of(&rules, &history_ring);

        let mut m = module();
        m.install(&bytes, root).expect("install crafted state");

        let mut chat_ctx = CaptureCtx::new().from_chat();
        exec(
            &mut m,
            &mut chat_ctx,
            &posted("general", 1, user(1), Vec::new()),
        )
        .expect("fire");
        assert_eq!(chat_ctx.task_msgs().len(), 1, "the action still emits");
        block_on(m.commit_block()).expect("commit");
        assert_eq!(
            get_rule(&m, "r").expect("r").fire_count,
            u64::MAX,
            "fire_count saturates instead of wrapping"
        );
    }

    // ---- substitution -------------------------------------------------------

    #[test]
    fn substitution_covers_all_placeholders_single_pass() {
        // {author} for a user renders the hex pubkey; unknown tokens stay literal.
        let author = display_author(&AuthorRef::User(vec![0xab, 0xcd]));
        let out = substitute(
            "c={channel} s={seq} a={author} t={text} u={unknown}",
            "general",
            9,
            &author,
            "hello",
        );
        assert_eq!(
            out,
            format!("c=general s=9 a={author} t=hello u={{unknown}}")
        );
        assert_eq!(author, "user:abcd");
    }

    #[test]
    fn substitution_does_not_rescan_substituted_values() {
        // a channel value that itself contains a placeholder token is not
        // re-substituted (single pass).
        let out = substitute("{channel}-{seq}", "{seq}", 4, "a", "t");
        assert_eq!(out, "{seq}-4");
    }

    #[test]
    fn blocks_text_concatenates_text_bearing_blocks() {
        let blocks = vec![
            Block::Paragraph(vec![
                Span::plain("hello "),
                Span {
                    text: "world".into(),
                    marks: vec![Mark::Bold],
                },
            ]),
            Block::Divider,
            Block::Code {
                lang: None,
                text: "code line".into(),
            },
        ];
        assert_eq!(blocks_text(&blocks), "hello world\ncode line");
    }

    // ---- staging semantics --------------------------------------------------

    #[test]
    fn abort_discards_staged_rules_and_history() {
        let mut m = module();
        let mut ctx = CaptureCtx::new();
        exec(
            &mut m,
            &mut ctx,
            &create("r", post_trigger(None, None), task_action("t", "T")),
        )
        .expect("create");
        block_on(m.commit_block()).expect("commit");
        let root1 = m.root();

        // stage a fire then abort.
        let mut chat_ctx = CaptureCtx::new().from_chat();
        exec(
            &mut m,
            &mut chat_ctx,
            &posted("general", 1, user(1), Vec::new()),
        )
        .expect("fire");
        assert_eq!(
            m.root(),
            root1,
            "staged fire does not move the committed root"
        );
        block_on(m.abort_block()).expect("abort");
        assert_eq!(m.root(), root1, "abort is byte-identical");
        assert_eq!(get_rule(&m, "r").expect("r").fire_count, 0);
        assert!(history(&m, "r", 16).is_empty());
    }

    // ---- snapshot / install -------------------------------------------------

    #[test]
    fn snapshot_install_round_trip_and_root_stability() {
        let mut source = module();
        let mut ctx = CaptureCtx::new();
        exec(
            &mut source,
            &mut ctx,
            &create(
                "r1",
                post_trigger(Some("general"), None),
                post_action("general", "hi {seq}"),
            ),
        )
        .expect("create r1");
        exec(
            &mut source,
            &mut ctx,
            &create(
                "r2",
                post_trigger(None, Some("x")),
                task_action("p", "t {text}"),
            ),
        )
        .expect("create r2");
        exec(
            &mut source,
            &mut ctx,
            &create(
                "r3",
                Trigger::MemoryPublished {
                    prefix: Some("/docs".into()),
                    meta_kind: Some("note".into()),
                    author_contains: Some("writer".into()),
                },
                inbox_action("{author}", "memory", "published {path}@{generation}"),
            ),
        )
        .expect("create r3");
        block_on(source.commit_block()).expect("commit rules");

        // fire r1 to populate the run-history ring (the transcript provides the
        // channel for the probe and text for r2's filter, which does not match).
        let mut chat_ctx = CaptureCtx::new().from_chat().with_transcript(
            "general",
            vec![message(
                "general",
                1,
                user(1),
                vec![Block::paragraph("hello")],
            )],
        );
        exec(
            &mut source,
            &mut chat_ctx,
            &posted("general", 1, user(1), Vec::new()),
        )
        .expect("fire");
        block_on(source.commit_block()).expect("commit fire");

        let expected = source.root();
        let handle = source.state_sync_handle().expect("handle");
        let bytes = match handle {
            sdk::StateSyncHandle::SnapshotBytes(bytes) => bytes,
            other => panic!("expected SnapshotBytes, got {other:?}"),
        };
        assert_eq!(
            bytes,
            source.snapshot(),
            "handle carries the snapshot preimage"
        );

        let mut target = module();
        target.install(&bytes, expected).expect("install");
        assert_eq!(target.root(), expected, "root matches after install");
        assert_eq!(list_rules(&target), list_rules(&source));
        assert_eq!(history(&target, "r1", 16), history(&source, "r1", 16));
    }

    #[test]
    fn install_rejects_wrong_root() {
        let mut source = module();
        let mut ctx = CaptureCtx::new();
        exec(
            &mut source,
            &mut ctx,
            &create("r", post_trigger(None, None), task_action("t", "T")),
        )
        .expect("create");
        block_on(source.commit_block()).expect("commit");
        let bytes = source.snapshot();

        let mut target = module();
        let err = target
            .install(&bytes, StateRoot([9u8; sdk::ROOT_LEN]))
            .expect_err("wrong root must reject");
        assert!(matches!(err, Error::Module(msg) if msg.contains("root mismatch")));
        assert_eq!(
            target.root(),
            module().root(),
            "rejected install left state untouched"
        );
    }

    #[test]
    fn install_accepts_run_records_with_oversized_event_fields() {
        // chat does not bound channel-id length, so a matching rule can commit
        // a run record whose channel_id exceeds this module's own id caps
        // (here via the composed-id guard record). install must accept every
        // execute-reachable state — the root comparison is the integrity check.
        let mut source = module();
        let mut ctx = CaptureCtx::new();
        exec(
            &mut source,
            &mut ctx,
            &create("r", post_trigger(None, None), task_action("t", "T")),
        )
        .expect("create");
        block_on(source.commit_block()).expect("commit rule");

        let long_channel = "c".repeat(MAX_ID_BYTES * 2);
        let mut chat_ctx = CaptureCtx::new().from_chat();
        exec(
            &mut source,
            &mut chat_ctx,
            &posted(&long_channel, 1, user(1), Vec::new()),
        )
        .expect("fire");
        block_on(source.commit_block()).expect("commit fire");
        assert_eq!(history(&source, "r", 4).len(), 1, "the match was recorded");

        let mut target = module();
        target
            .install(&source.snapshot(), source.root())
            .expect("install must accept execute-reachable records");
        assert_eq!(target.root(), source.root());
        assert_eq!(history(&target, "r", 4), history(&source, "r", 4));
    }
}
