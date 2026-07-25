//! qmdb-backed deterministic user-defined automations over chat hooks.
//!
//! an operator registers rules — a [`Trigger`] (chat post filters) plus an
//! [`Action`] (post a chat message, create a task, or deliver an inbox
//! notification). when chat fans a post out to its hooks, this module evaluates
//! every enabled rule and emits the matching actions as follow-up [`sdk::Msg`]s
//! in the SAME block as the event (P2).
//!
//! [`Trigger`] is a chat message-posted filter (chat is the only event source
//! today) and is a flat single-shape struct on the wire. A future non-chat
//! trigger is its own state break (flag day).
//!
//! ## Origin-gated intake (spoof-proofing)
//!
//! dispatch is routed by the host-assigned origin:
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
//! the no-fail-arm pattern also applies to follow-ups): host-routed queries against
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
//! pure logic over a host-injected [`sdk::MerkleStore`]: the HOST constructs
//! the concrete store (qmdb today — `statesync::qmdb::QmdbStore`) and hands it
//! to [`Automations::new`], so this crate never names a storage crate. one
//! logical record per rule plus TWO aggregate records consensus itself
//! consumes, so both stay canonical (never index-tier scan machinery):
//!
//! - the ROSTER (the sorted rule-id list, bounded by [`MAX_RULES`]) — every
//!   hook event evaluates every rule inside `execute`, so consensus consumes
//!   the enumeration on every hooked post;
//! - the run-history CURSOR (`head`/`next` ring bounds) — the write path
//!   consumes it on every append to place the new record and trim the ring to
//!   [`MAX_RUN_HISTORY`] (point deletes, no scan).
//!
//! run records are seq-keyed point records between the cursor's bounds; the
//! `RunHistory` query walks them by derived key, never by store iteration.
//!
//! writes are staged during a block and flushed to the store in one batch at
//! `commit_block`; the module root IS the store's merkle root. sync belongs
//! to the store, not this module: a joiner rebuilds the concrete store from a
//! peer (`QmdbStore::sync_from`) and wraps a fresh `Automations` around it.
//!
//! oversized values never reach the store (the poison-value lesson — the qmdb
//! wire codec bounds a value at decode, so an over-cap committed value would
//! wedge every syncing peer): rule fields are individually capped at execute,
//! which bounds the rule record; the roster record is byte-capped at create
//! ([`MAX_ROSTER_RECORD_BYTES`]); and a run record is bounded by chat's own
//! channel-record cap plus this module's id/template caps.

// the wire surface: this module's shared types, flattened at the crate root.
mod interface;
pub use interface::*;

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
use sdk::{
    Ctx, Error, MerkleStore, Module, ModuleId, Msg, Origin, ResolverSyncTarget, StagedStore,
    StateRoot, StateSyncHandle, require_non_empty,
};
use borsh::{BorshDeserialize, BorshSerialize};
use tasks::{
    TaskMsg, TaskQuery, TaskReply, decode_task_reply as tasks_decode_reply,
    encode_task_msg as tasks_encode_msg, encode_task_query as tasks_encode_query,
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
/// serialized roster-record byte bound, enforced at create. the per-field caps
/// do not bound the roster's SERIALIZED form tightly enough on their own:
/// [`MAX_RULES`] ids of [`MAX_ID_BYTES`] control characters JSON-escape past
/// the qmdb wire codec's decode ceiling — a committed over-cap value would
/// wedge every syncing peer (the poison-value lesson), so the create op
/// refuses loudly instead.
pub const MAX_ROSTER_RECORD_BYTES: usize = 512 * 1024;

/// per-rule record key: prefix + 0 + id (the single-component shape chat
/// uses). safe because every key literal below is fixed and none is another
/// followed by a 0 byte.
fn rule_key(rule_id: &str) -> Vec<u8> {
    let mut key = Vec::with_capacity(4 + 1 + rule_id.len());
    key.extend_from_slice(b"rule");
    key.push(0);
    key.extend_from_slice(rule_id.as_bytes());
    key
}

/// per-run-record key: prefix + 0 + seq (big-endian).
fn run_key(seq: u64) -> Vec<u8> {
    let mut key = Vec::with_capacity(3 + 1 + 8);
    key.extend_from_slice(b"run");
    key.push(0);
    key.extend_from_slice(&seq.to_be_bytes());
    key
}

/// the roster record's whole key. collides with no `rule\0...`/`run\0...` key.
const ROSTER_KEY: &[u8] = b"roster";

/// the run-history cursor record's whole key.
const RUN_CURSOR_KEY: &[u8] = b"runcursor";

/// the run-history ring bounds, maintained by the write path: `head` is the
/// oldest live record's seq, `next` the seq the next append takes (empty ring
/// when equal). consensus consumes it on every append — placing the new
/// record and trimming the ring are decided from this record, never from a
/// store scan — so it stays canonical.
#[derive(Default, BorshSerialize, BorshDeserialize)]
struct RunCursor {
    head: u64,
    next: u64,
}

pub struct Automations {
    id: ModuleId,
    /// the chat module id — both the trusted hook origin and the `PostMessage`
    /// follow-up target.
    chat: ModuleId,
    /// the tasks module id — the `CreateTask` follow-up target.
    tasks: ModuleId,
    /// the inbox module id — the `DeliverInbox` follow-up target.
    inbox: ModuleId,
    /// the host-injected authenticated store plus this block's staging overlay
    /// (read-your-writes, folded into `root()` at `commit_block`). store key
    /// is `sha256(logical_key)`, owned by [`StagedStore`].
    staged: StagedStore,
}

impl Automations {
    /// wrap the host-constructed store under module identity `id`.
    pub fn new(
        id: impl Into<ModuleId>,
        store: Box<dyn MerkleStore>,
        chat: impl Into<ModuleId>,
        tasks: impl Into<ModuleId>,
        inbox: impl Into<ModuleId>,
    ) -> Self {
        Self {
            id: id.into(),
            chat: chat.into(),
            tasks: tasks.into(),
            inbox: inbox.into(),
            staged: StagedStore::new(store),
        }
    }

    // ---- staged-over-committed reads ----------------------------------------

    async fn load<T>(&self, key: &[u8]) -> Result<Option<T>, Error>
    where
        T: BorshDeserialize,
    {
        match self.staged.get(key).await? {
            Some(bytes) => Ok(Some(
                borsh::from_slice(&bytes).map_err(|e| Error::Module(e.to_string()))?,
            )),
            None => Ok(None),
        }
    }

    /// stage a value whose serialized size the execute-time field caps already
    /// bound (rule records, run records, the cursor) — see the module doc's
    /// poison-value paragraph. the roster goes through [`Self::store_bounded`].
    fn store<T>(&mut self, key: Vec<u8>, value: &T)
    where
        T: BorshSerialize,
    {
        self.staged.stage(
            key,
            borsh::to_vec(value).expect("automations value is serializable"),
        );
    }

    /// stage a value only if its serialized size fits `cap` — the write-time
    /// guard against poison values (the qmdb codec cap is decode-only).
    fn store_bounded<T>(
        &mut self,
        key: Vec<u8>,
        value: &T,
        cap: usize,
        what: &str,
    ) -> Result<(), Error>
    where
        T: BorshSerialize,
    {
        let bytes = borsh::to_vec(value).expect("automations value is serializable");
        if bytes.len() > cap {
            return Err(Error::Module(format!(
                "{what} record too large: {} > {cap} bytes",
                bytes.len()
            )));
        }
        self.staged.stage(key, bytes);
        Ok(())
    }

    async fn rule(&self, rule_id: &str) -> Result<Option<Rule>, Error> {
        self.load(&rule_key(rule_id)).await
    }

    /// the rule roster — every registered id, sorted. record and roster are
    /// staged (and commit or abort) together, so membership in one is
    /// membership in both; the roster is the ONE existence authority at create.
    async fn roster(&self) -> Result<Vec<String>, Error> {
        Ok(self.load(ROSTER_KEY).await?.unwrap_or_default())
    }

    /// every rule, in roster (rule-id) order — the ONE enumeration read:
    /// consensus itself consumes it (each hook event evaluates each rule), so
    /// it stays canonical, bounded by [`MAX_RULES`]. a rostered id without a
    /// record is a store bug — loud, never skipped.
    async fn all_rules(&self) -> Result<Vec<Rule>, Error> {
        let mut rules = Vec::new();
        for rule_id in self.roster().await? {
            let Some(rule) = self.rule(&rule_id).await? else {
                return Err(Error::Module(format!("missing rule record: {rule_id}")));
            };
            rules.push(rule);
        }
        Ok(rules)
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

    // ---- admin ops ----------------------------------------------------------

    async fn stage_create_rule(
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
        let mut roster = self.roster().await?;
        let position = match roster.binary_search(&rule_id) {
            Ok(_) => {
                return Err(Error::Module(format!("rule already exists: {rule_id}")));
            }
            Err(position) => position,
        };
        if roster.len() >= MAX_RULES {
            return Err(Error::Module(format!("rule cap reached ({MAX_RULES})")));
        }
        roster.insert(position, rule_id.clone());
        // the roster's byte gate first: a refusal must stage NOTHING.
        self.store_bounded(
            ROSTER_KEY.to_vec(),
            &roster,
            MAX_ROSTER_RECORD_BYTES,
            "roster",
        )?;
        self.store(
            rule_key(&rule_id),
            &Rule {
                rule_id: rule_id.clone(),
                enabled: true,
                trigger,
                action,
                created_at: consensus_time,
                fire_count: 0,
            },
        );
        Ok(())
    }

    async fn stage_set_enabled(&mut self, rule_id: String, enabled: bool) -> Result<(), Error> {
        require_non_empty("rule_id", &rule_id)?;
        let Some(mut rule) = self.rule(&rule_id).await? else {
            return Err(Error::Module(format!("unknown rule: {rule_id}")));
        };
        if rule.enabled == enabled {
            // idempotent: staging nothing keeps the op log — and the root —
            // byte-identical to no write at all.
            return Ok(());
        }
        rule.enabled = enabled;
        self.store(rule_key(&rule_id), &rule);
        Ok(())
    }

    async fn stage_delete_rule(&mut self, rule_id: String) -> Result<(), Error> {
        require_non_empty("rule_id", &rule_id)?;
        let mut roster = self.roster().await?;
        let Ok(position) = roster.binary_search(&rule_id) else {
            return Err(Error::Module(format!("unknown rule: {rule_id}")));
        };
        roster.remove(position);
        self.staged.delete(rule_key(&rule_id));
        // shrinking keeps the roster under its create-time byte gate.
        self.store(ROSTER_KEY.to_vec(), &roster);
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
        let rules = self.all_rules().await?;

        // fetch the post's text ONCE, and only if some rule that already matches
        // on channel + mention needs it (a `text_contains` filter, or a `{text}`
        // placeholder). `None` = the fetch FAILED (query error / message absent),
        // which is distinct from a legitimately empty body (`Some("")`): rules
        // that need text record a failure on `None` instead of silently
        // matching against emptiness.
        let needs_text = rules.iter().any(|rule| {
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

        // evaluate in deterministic rule_id order (roster order).
        let mut budget = 0usize;
        let mut fired: Vec<Rule> = Vec::new();
        let mut records: Vec<RunRecord> = Vec::new();
        for rule in &rules {
            if !rule.enabled || !Self::matches_channel_and_mention(rule, &channel_id, &mentions) {
                continue;
            }
            let record = |action_ok: bool, detail: String| RunRecord {
                rule_id: rule.rule_id.clone(),
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
                    fired.push(updated);
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
        for updated in fired {
            self.store(rule_key(&updated.rule_id), &updated);
        }
        self.append_history(records).await
    }

    /// append this event's run records and trim the ring to
    /// [`MAX_RUN_HISTORY`]: place each record at `next`, then point-delete
    /// from `head` — every decision reads the cursor, never a store scan.
    async fn append_history(&mut self, records: Vec<RunRecord>) -> Result<(), Error> {
        if records.is_empty() {
            return Ok(());
        }
        let mut cursor: RunCursor = self.load(RUN_CURSOR_KEY).await?.unwrap_or_default();
        for record in records {
            self.store(run_key(cursor.next), &record);
            cursor.next += 1;
        }
        while cursor.next - cursor.head > MAX_RUN_HISTORY as u64 {
            self.staged.delete(run_key(cursor.head));
            cursor.head += 1;
        }
        self.store(RUN_CURSOR_KEY.to_vec(), &cursor);
        Ok(())
    }

    /// build the action for a firing rule, PROBE its target, and emit it as a
    /// follow-up. returns the success `detail` on emit, or an error `detail`
    /// when the action is structurally impossible or a probe rejects it
    /// (recorded, not a block failure).
    ///
    /// the probe layer (the no-fail-arm pattern applied to follow-ups):
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

#[async_trait::async_trait(?Send)]
impl Module for Automations {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// the store's merkle root over all committed records, verbatim — the
    /// staged overlay is invisible here until `commit_block`.
    fn root(&self) -> StateRoot {
        self.staged.root()
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        self.staged.state_sync_handle()
    }

    /// the network state-sync serve lane: answers the shared qmdb wire requests
    /// (historical proof-carrying op ranges) from committed state. read-only;
    /// the joiner's sync engine merkle-verifies every batch.
    async fn serve_sync(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        self.staged.serve_sync(req).await
    }

    async fn resolver_sync_target(&self) -> Result<ResolverSyncTarget, Error> {
        self.staged.sync_target().await
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
                } => {
                    let consensus_time = ctx.env().consensus_time;
                    self.stage_create_rule(rule_id, trigger, action, consensus_time)
                        .await
                }
                AutomationsMsg::SetEnabled { rule_id, enabled } => {
                    self.stage_set_enabled(rule_id, enabled).await
                }
                AutomationsMsg::DeleteRule { rule_id } => self.stage_delete_rule(rule_id).await,
                AutomationsMsg::HookEvent(_) => Err(Error::Module(
                    "hook events must originate from the chat module".into(),
                )),
            },
        }
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            AutomationsQuery::ListRules => Ok(encode_reply(&AutomationsReply::Rules(
                self.all_rules().await?,
            ))),
            AutomationsQuery::GetRule { rule_id } => Ok(encode_reply(&AutomationsReply::Rule(
                self.rule(&rule_id).await?,
            ))),
            AutomationsQuery::RunHistory { rule_id, limit } => {
                let limit = usize::try_from(limit)
                    .unwrap_or(usize::MAX)
                    .min(MAX_RUN_HISTORY);
                // walk the ring by derived key between the cursor's bounds
                // (≤ MAX_RUN_HISTORY point reads). a seq inside the bounds
                // without a record is a store bug — loud, never skipped.
                let cursor: RunCursor = self.load(RUN_CURSOR_KEY).await?.unwrap_or_default();
                let mut matched: Vec<RunRecord> = Vec::new();
                for seq in cursor.head..cursor.next {
                    let Some(record) = self.load::<RunRecord>(&run_key(seq)).await? else {
                        return Err(Error::Module(format!("missing run record: {seq}")));
                    };
                    if record.rule_id == rule_id {
                        matched.push(record);
                    }
                }
                if matched.len() > limit {
                    matched = matched.split_off(matched.len() - limit);
                }
                Ok(encode_reply(&AutomationsReply::History(matched)))
            }
        }
    }

    /// publish the block's staged writes in ONE store batch. no-op (and no
    /// root movement) if nothing was staged.
    async fn commit_block(&mut self) -> Result<(), Error> {
        self.staged.commit().await
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.staged.abort();
        Ok(())
    }
}

#[cfg(test)]
mod tests;

// the wasm-guest port: the store-backed dispatch shell that adapts this module
// to the ducktape:module world. compiled only by the guest-builder's
// synthesized wasm32 cdylib workspace (feature `guest`), never by the native
// build.
#[cfg(feature = "guest")]
mod guest;
