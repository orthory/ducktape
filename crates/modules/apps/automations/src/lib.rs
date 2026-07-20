//! deterministic user-defined automations over chat hooks.
//!
//! an operator registers rules — a [`Trigger`] (chat post filters) plus an
//! [`Action`] (post a chat message, create a task, or deliver an inbox
//! notification). when chat fans a post out to its hooks, this module evaluates
//! every enabled rule and emits the matching actions as follow-up [`sdk::Msg`]s
//! in the SAME block as the event (P2).
//!
//! [`Trigger`] is a chat message-posted filter (chat is the only event source
//! today); the snapshot codec keeps a trigger-kind byte so a future
//! filesystem-change (duckfs) trigger can join without a state break.
//!
//! ## Origin-gated intake (spoof-proofing)
//!
//! dispatch is routed by the HOST-ASSIGNED origin, exactly like agent v2:
//! - `Origin::Module("chat")` → the payload is a raw `chat::ChatEvent`
//!   (chat's generic hook fan-out delivers the event bytes verbatim, unwrapped),
//!   decoded in the NO-FAIL hook arm.
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
use sdk::codec::{self, Cursor};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot, require_non_empty};
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
    ) -> Self {
        Self {
            id: id.into(),
            chat: chat.into(),
            tasks: tasks.into(),
            inbox: inbox.into(),
            rules: BTreeMap::new(),
            history: VecDeque::new(),
            pending_rules: BTreeMap::new(),
            pending_history: Vec::new(),
        }
    }

    // ---- validation ---------------------------------------------------------

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
        if let Some(channel_id) = &trigger.channel_id {
            require_non_empty("trigger channel_id", channel_id)?;
            Self::validate_len("trigger channel_id", channel_id, MAX_ID_BYTES)?;
        }
        if let Some(mention) = &trigger.mention {
            Self::validate_len("trigger mention", mention, MAX_FILTER_BYTES)?;
        }
        if let Some(text_contains) = &trigger.text_contains {
            Self::validate_len("trigger text_contains", text_contains, MAX_FILTER_BYTES)?;
        }
        Ok(())
    }

    fn validate_action(action: &Action) -> Result<(), Error> {
        match action {
            Action::PostMessage {
                channel_id,
                template,
            } => {
                require_non_empty("action channel_id", channel_id)?;
                Self::validate_len("action channel_id", channel_id, MAX_ID_BYTES)?;
                Self::validate_len("action template", template, MAX_TEMPLATE_BYTES)?;
            }
            Action::CreateTask {
                task_id_prefix,
                title_template,
            } => {
                require_non_empty("action task_id_prefix", task_id_prefix)?;
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
                require_non_empty("action kind", kind)?;
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
        require_non_empty("rule_id", &rule_id)?;
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
        require_non_empty("rule_id", &rule_id)?;
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
        require_non_empty("rule_id", &rule_id)?;
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
            if let Some(want) = &rule.trigger.text_contains
                && !text.contains(want.as_str())
            {
                continue;
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
                // duplicates, which would abort the block. the tasks wire surface only
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
        if let Some(want) = &rule.trigger.channel_id
            && want != channel_id
        {
            return false;
        }
        if let Some(want) = &rule.trigger.mention
            && !mentions
                .iter()
                .any(|author| display_author(author).contains(want.as_str()))
        {
            return false;
        }
        true
    }

    fn rule_wants_text(rule: &Rule) -> bool {
        if rule.trigger.text_contains.is_some() {
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

// ---- canonical byte codec (root preimage) -----------------------------------

fn encode_state(rules: &BTreeMap<String, Rule>, history: &VecDeque<RunRecord>) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(rules.len() as u64).to_le_bytes());
    for rule in rules.values() {
        codec::push_str(&mut out, &rule.rule_id);
        out.push(rule.enabled as u8);
        push_trigger(&mut out, &rule.trigger);
        push_action(&mut out, &rule.action);
        out.extend_from_slice(&rule.created_at.to_le_bytes());
        out.extend_from_slice(&rule.fire_count.to_le_bytes());
    }
    out.extend_from_slice(&(history.len() as u64).to_le_bytes());
    for record in history {
        codec::push_str(&mut out, &record.rule_id);
        codec::push_str(&mut out, &record.channel_id);
        out.extend_from_slice(&record.seq.to_le_bytes());
        out.extend_from_slice(&record.height.to_le_bytes());
        out.push(record.action_ok as u8);
        codec::push_str(&mut out, &record.detail);
    }
    out
}

fn push_trigger(out: &mut Vec<u8>, trigger: &Trigger) {
    // the leading 0 is the legacy single-variant trigger-kind byte, kept so
    // the committed root preimage stays byte-identical; a future non-chat
    // trigger claims the next discriminant.
    out.push(0);
    codec::push_opt_str(out, trigger.channel_id.as_deref());
    codec::push_opt_str(out, trigger.mention.as_deref());
    codec::push_opt_str(out, trigger.text_contains.as_deref());
}

fn push_action(out: &mut Vec<u8>, action: &Action) {
    match action {
        Action::PostMessage {
            channel_id,
            template,
        } => {
            out.push(0);
            codec::push_str(out, channel_id);
            codec::push_str(out, template);
        }
        Action::CreateTask {
            task_id_prefix,
            title_template,
        } => {
            out.push(1);
            codec::push_str(out, task_id_prefix);
            codec::push_str(out, title_template);
        }
        Action::DeliverInbox {
            member_template,
            kind,
            body_template,
        } => {
            out.push(2);
            codec::push_str(out, member_template);
            codec::push_str(out, kind);
            codec::push_str(out, body_template);
        }
    }
}

fn decode_state(bytes: &[u8]) -> Result<(BTreeMap<String, Rule>, VecDeque<RunRecord>), Error> {
    let mut c = Cursor::new(bytes);
    let rule_count = c.u64("snapshot rule count")?;
    // each rule costs at least one byte, bounding a forged count.
    c.bound(rule_count, 1, "snapshot rule count")?;
    let mut rules: BTreeMap<String, Rule> = BTreeMap::new();
    for _ in 0..rule_count {
        let rule_id = c.string("snapshot rule_id")?;
        require_non_empty("rule_id", &rule_id)?;
        Automations::validate_len("rule_id", &rule_id, MAX_ID_BYTES)?;
        let enabled = c.bool("snapshot rule enabled")?;
        let trigger = read_trigger(&mut c)?;
        let action = read_action(&mut c)?;
        let created_at = c.u64("snapshot rule created_at")?;
        let fire_count = c.u64("snapshot rule fire_count")?;
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

    let history_count = c.u64("snapshot history count")?;
    if history_count > MAX_RUN_HISTORY as u64 {
        return Err(Error::Module("snapshot run history exceeds cap".into()));
    }
    c.bound(history_count, 1, "snapshot history count")?;
    let mut history: VecDeque<RunRecord> = VecDeque::with_capacity(history_count as usize);
    for _ in 0..history_count {
        // rule_id is always a registered rule's id, so the execute-time caps
        // hold for it. channel_id and detail derive from the chat EVENT (chat
        // does not bound channel-id length), so no length checks on them:
        // install must accept every execute-reachable state — the root
        // comparison is the integrity check (the poison-value lesson).
        let rule_id = c.string("snapshot record rule_id")?;
        require_non_empty("run record rule_id", &rule_id)?;
        Automations::validate_len("run record rule_id", &rule_id, MAX_ID_BYTES)?;
        let channel_id = c.string("snapshot record channel_id")?;
        let seq = c.u64("snapshot record seq")?;
        let height = c.u64("snapshot record height")?;
        let action_ok = c.bool("snapshot record action_ok")?;
        let detail = c.string("snapshot record detail")?;
        history.push_back(RunRecord {
            rule_id,
            channel_id,
            seq,
            height,
            action_ok,
            detail,
        });
    }

    c.finish("automations snapshot")?;
    Ok((rules, history))
}

/// a PLAIN optional string (flag byte + length-prefixed string). deliberately
/// not [`Cursor::opt_str`]: its non-empty rule would reject the
/// execute-reachable `Some("")` mention/text filters.
fn read_opt_string(c: &mut Cursor<'_>, what: &str) -> Result<Option<String>, Error> {
    Ok(if c.bool(what)? {
        Some(c.string(what)?)
    } else {
        None
    })
}

fn read_trigger(c: &mut Cursor<'_>) -> Result<Trigger, Error> {
    match c.byte("snapshot trigger kind")? {
        0 => {
            let channel_id = read_opt_string(c, "trigger channel_id")?;
            if let Some(channel_id) = &channel_id {
                require_non_empty("trigger channel_id", channel_id)?;
                Automations::validate_len("trigger channel_id", channel_id, MAX_ID_BYTES)?;
            }
            let mention = read_opt_string(c, "trigger mention")?;
            if let Some(mention) = &mention {
                Automations::validate_len("trigger mention", mention, MAX_FILTER_BYTES)?;
            }
            let text_contains = read_opt_string(c, "trigger text_contains")?;
            if let Some(text_contains) = &text_contains {
                Automations::validate_len(
                    "trigger text_contains",
                    text_contains,
                    MAX_FILTER_BYTES,
                )?;
            }
            Ok(Trigger {
                channel_id,
                mention,
                text_contains,
            })
        }
        other => Err(Error::Module(format!(
            "snapshot has unknown trigger discriminant {other}"
        ))),
    }
}

fn read_action(c: &mut Cursor<'_>) -> Result<Action, Error> {
    let action = match c.byte("snapshot action kind")? {
        0 => Action::PostMessage {
            channel_id: c.string("action channel_id")?,
            template: c.string("action template")?,
        },
        1 => Action::CreateTask {
            task_id_prefix: c.string("action task_id_prefix")?,
            title_template: c.string("action title_template")?,
        },
        2 => Action::DeliverInbox {
            member_template: c.string("action member_template")?,
            kind: c.string("action kind")?,
            body_template: c.string("action body_template")?,
        },
        other => {
            return Err(Error::Module(format!(
                "snapshot has unknown action discriminant {other}"
            )));
        }
    };
    Automations::validate_action(&action)?;
    Ok(action)
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
mod tests;
