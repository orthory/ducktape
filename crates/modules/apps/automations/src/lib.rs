//! qmdb-backed deterministic user-defined automations over chat hooks.
//!
//! a user registers rules — a [`Trigger`] (chat post filters) plus an
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
//! ## Rule ownership — CREATING a rule and RUNNING one are different principals
//!
//! every rule records an [`owner`](Rule::owner): the authenticated external
//! submitter of its `CreateRule`, in the same raw-key domain `chat::Channel`
//! records for its own owner. `SetEnabled` and `DeleteRule` are refused unless
//! the submitter IS that owner, and [`rule_owner`] refuses every origin that
//! cannot be one — the pre-consensus default `Origin::External(vec![])`, and
//! `Origin::Module`/`Origin::System` outright. so an
//! ownerless rule is not a shape this module can mint.
//!
//! that gate binds rule AUTHORSHIP only. a FIRING rule still emits its action
//! under `Origin::Module("automations")` — the host stamps that origin on the
//! emitter, and those follow-ups go to chat/tasks/inbox, never back through
//! this module's admin path. the hook arm is likewise routed by origin BEFORE
//! the owner gate is reached. so gating creation costs a rule nothing at fire
//! time, and the module's own authority can never be turned on itself: a
//! module origin cannot create a rule.
//!
//! ## The OWNER's standing scopes every fire, both directions
//!
//! a firing rule wears this module's origin, and chat's post policy admits a
//! module unconditionally (modules are genesis-fixed trusted code) — so the
//! origin on the wire cannot scope a rule. the [`Rule::owner`] does, at FIRE
//! time, asked of chat itself ([`ChatQuery::Access`], read-only) and never
//! re-derived here:
//!
//! - READ: a rule matches an event only if its owner may read the event's
//!   channel (a member, or the channel is [`chat::PostPolicy::Open`]). so a
//!   `None` trigger channel means "every channel the OWNER can read", not
//!   every channel this module is hooked into — registering the hook on a
//!   members-only channel no longer hands its traffic to every rule on the
//!   network. a rule its owner cannot read for is not a match: no run record,
//!   because recording the channels a rule cannot see leaks exactly what the
//!   gate withholds.
//! - WRITE: a `PostMessage` action is emitted only if the owner's own post
//!   into the TARGET channel would be admitted — chat's post gate, verbatim.
//!   a refusal is a staged no-op recorded as a [`RunRecord`] the owner reads
//!   back through `AutomationsQuery::RunHistory`, never a block failure.
//!
//! both answers are memoized per `(owner, channel)` for the event and capped
//! at [`MAX_ACCESS_PROBES_PER_EVENT`], because they are sibling reads on the
//! consensus path.
//!
//! that is still NOT the channel owner's call over the rule itself: a rule is
//! not attached to a channel, and its action may target a different channel,
//! or tasks, or an inbox member entirely. a channel owner's lever over the
//! automations reaching their channel remains `ChatMsg::UnregisterHook`, which
//! is theirs already and is better scoped: it detaches this module from that
//! one channel instead of deleting a rule that also serves others.
//!
//! ## One author rendering, and it is the ACTOR domain
//!
//! [`actor_of`] is the ONE rendering of a chat author in this module:
//! `sdk::Origin::actor_string` of the origin that author handle names. it feeds
//! all three consumers — the `{author}`/`{mention}` substitutions, and the
//! `mention` trigger filter — because the alternative is a rendering whose
//! meaning depends on WHERE it is substituted, and the member path cannot
//! afford that.
//!
//! an inbox member is not a display handle: it is a QUEUE NAME in the
//! actor-string domain, and inbox refuses a `MarkRead`/`Clear` from anyone but
//! `member`'s own origin. so a `member_template` of `{author}` produces
//! `ext:{hex}` — the very string the triggering author's own signed frames
//! carry — and the notification is ackable. rendering the same author as the
//! index tier's `user:{hex}` display handle instead would create a queue no
//! origin can ever own: mail that is delivered, counted, and unackable forever.
//! that is exactly why there is one rendering here and it is the machine one.
//!
//! ## Loop prevention
//!
//! a rule fires ONLY when the event author is `AuthorRef::User(_)`. posts authored
//! by modules or agents — including this module's own `PostMessage` follow-ups —
//! never trigger rules, so an automation posting into a hooked channel cannot
//! cascade. this mirrors the agent module's user-author-only decision.
//!
//! a `DeliverInbox` action may reach ONLY the rule owner's OWN inbox queue —
//! `member_template` must be the exact literal `sdk::Origin::External(owner)
//! .actor_string()`, checked at `CreateRule` (so a rule that could never
//! legally fire never burns a roster slot) and re-checked at fire time. this
//! module fires under `Origin::Module("automations")`, which inbox admits
//! UNCONDITIONALLY (module origins may deliver to any member) — so without
//! this gate a rule owner could deliver to a stranger's queue under the
//! module's authority, exactly the write inbox's own `deliver_is_permitted`
//! refuses on the direct path. `{author}`/`{mention}`/a literal foreign
//! member all fail this check the same way: none of them can ever equal the
//! FIXED owner queue, so the only member_template a `CreateRule` accepts is
//! the owner's own `ext:{hex}`.
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
//! wedge the rule — id squatting), and that a `CreateTask` id is unused and its
//! owner (the rule owner, not this module — see below) is under
//! [`tasks::MAX_OPEN_TASKS_PER_OWNER`]. a probe rejection downgrades to a
//! `RunRecord`, protecting the posting user's block from every
//! structurally-KNOWABLE follow-up failure — including the RULE's own owner
//! being at cap, which must refuse the rule's action, never the triggering
//! post.
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
//! ## Hook registration is a separate op, and a separate authority
//!
//! registering a rule does NOT subscribe this module to any channel. the
//! channel's OWNER separately submits
//! `ChatMsg::RegisterHook { channel_id, module_id: "automations" }` to chat for
//! each channel whose posts should reach these rules — chat gates that on
//! channel-admin authority, so a rule owner cannot wire their own rule into a
//! channel they do not own. the hook is a delivery path, never a grant: what
//! reaches a rule through it is still bounded by the owner's own read standing
//! above.
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

use borsh::{BorshDeserialize, BorshSerialize};
use chat::{
    AuthorRef, Block, ChannelAccess, ChatEvent, ChatMsg, ChatQuery, ChatReply,
    decode_event as chat_decode_event, decode_reply as chat_decode_reply,
    encode_msg as chat_encode_msg, encode_query as chat_encode_query,
};
use inbox::{
    InboxMsg, MAX_BODY_BYTES as INBOX_MAX_BODY_BYTES, MAX_KIND_BYTES, MAX_MEMBER_BYTES,
    encode_msg as inbox_encode_msg,
};
use sdk::{
    Ctx, Error, MerkleStore, Module, ModuleId, Msg, Origin, ResolverSyncTarget, StagedStore,
    StateRoot, StateSyncHandle, require_non_empty,
};
use tasks::{
    TaskMsg, TaskQuery, TaskReply, decode_task_reply as tasks_decode_reply,
    encode_task_msg as tasks_encode_msg, encode_task_query as tasks_encode_query,
};

/// max rules retained NETWORK-WIDE. registering beyond this is rejected at
/// execute. [`MAX_RULES_PER_OWNER`] is the per-account bound that keeps any
/// ONE account from being the reason this global roster ever fills.
pub const MAX_RULES: usize = 1024;
/// max rules one owner may hold at once, the tasks board's
/// [`tasks::MAX_OPEN_TASKS_PER_OWNER`] shape applied to the rule roster: no
/// single account can fill [`MAX_RULES`] and permanently deny the feature to
/// everyone else. [`AutomationsMsg::DeleteRule`] is how an owner recedes from
/// it.
pub const MAX_RULES_PER_OWNER: usize = 32;
/// `rule_id` byte bound (also the `channel_id`/`task_id_prefix` bound).
pub const MAX_ID_BYTES: usize = 256;
/// trigger filter (`mention`, `text_contains`) byte bound.
pub const MAX_FILTER_BYTES: usize = 256;
/// action template byte bound.
pub const MAX_TEMPLATE_BYTES: usize = 4096;
/// byte bound on a SUBSTITUTED template — the same budget as the template it
/// was rendered from. capping the template alone does not bound the render:
/// substitution AMPLIFIES, and 17 `{text}` tokens on a 64 KiB chat message
/// produce a ~1.09 MiB string, past both tasks' record cap and chat's
/// message-head cap. a follow-up must never be able to fail the op that
/// triggered it, so an over-cap render is TRUNCATED rather than refused:
/// refusing is safe for the block (a `build_and_emit` error is RECORDED, not
/// emitted) but silently stops the rule from firing on every large message
/// forever, and a clipped title or post body is the honest lesser harm.
pub const MAX_SUBSTITUTED_BYTES: usize = MAX_TEMPLATE_BYTES;
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
/// distinct `(rule owner, channel)` standing lookups ONE event may spend at
/// chat. every lookup is a sibling read, and the wasm host bounds a dispatch at
/// `MAX_SIBLING_READS` = 64 distinct sibling reads TOTAL — a
/// budget this event's text fetch and per-action probes also draw on. a roster
/// of rules with distinct owners would otherwise trap the dispatch and abort
/// the posting user's block, so the lookups are capped here and a rule past the
/// cap does not fire (fail closed, deterministically on every validator).
pub const MAX_ACCESS_PROBES_PER_EVENT: usize = 32;

/// derive the principal an admin op acts as — the ONLY ownership path, and the
/// only place a [`Rule::owner`] is ever minted.
///
/// exhaustive on purpose: a rule is a standing capability that fires under this
/// module's own authority, so only an authenticated external submitter may own
/// one. `Origin::Module` is refused even though the host assigns it honestly —
/// no module registers rules, and admitting one would let this module's own
/// execution identity mint more of itself. `Origin::System` is refused for the
/// same reason: nothing seeds a rule at genesis. that leaves the pre-consensus
/// default `Origin::External(vec![])`, which is not a submitter.
fn rule_owner(origin: &Origin) -> Result<Vec<u8>, Error> {
    match origin {
        Origin::External(key) => {
            let is_authenticated_submitter = !key.is_empty();
            if !is_authenticated_submitter {
                return Err(Error::Module(
                    "external origin must carry a non-empty submitter id".into(),
                ));
            }
            Ok(key.clone())
        }
        Origin::Module(id) => Err(Error::Module(format!(
            "a module origin cannot own an automation rule: {id}"
        ))),
        Origin::System => Err(Error::Module(
            "a system origin cannot own an automation rule".into(),
        )),
    }
}

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

/// per-owner rule census key: prefix + 0 + the owner's raw key bytes — the
/// tasks board's `t@{owner}` shape ([`tasks::MAX_OPEN_TASKS_PER_OWNER`]),
/// what [`MAX_RULES_PER_OWNER`] is checked against.
fn owner_rule_count_key(owner: &[u8]) -> Vec<u8> {
    let mut key = Vec::with_capacity(7 + 1 + owner.len());
    key.extend_from_slice(b"rulecnt");
    key.push(0);
    key.extend_from_slice(owner);
    key
}

/// the roster record's whole key. collides with no `rule\0...`/`run\0...`/
/// `rulecnt\0...` key.
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

/// this event's answers to "what may THIS rule owner do in THIS channel",
/// asked of chat once per distinct pair and spent against
/// [`MAX_ACCESS_PROBES_PER_EVENT`]. a rule's owner is fixed and the event
/// channel is one, so a roster of rules by the same owner costs one lookup.
#[derive(Default)]
struct AccessMemo {
    seen: Vec<((Vec<u8>, String), ChannelAccess)>,
}

/// the standing a caller gets when it may not ask: fail closed.
const NO_ACCESS: ChannelAccess = ChannelAccess {
    may_read: false,
    may_post: false,
};

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

    /// one owner's live rule count, read through the staged overlay — what
    /// [`MAX_RULES_PER_OWNER`] is checked against. absent reads as zero, the
    /// tasks board's `owner_count` shape.
    async fn owner_rule_count(&self, owner: &[u8]) -> Result<u64, Error> {
        let Some(bytes) = self.staged.get(&owner_rule_count_key(owner)).await? else {
            return Ok(0);
        };
        let raw: [u8; 8] = bytes
            .as_slice()
            .try_into()
            .map_err(|_| Error::Module("owner rule census record is not a u64".into()))?;
        Ok(u64::from_le_bytes(raw))
    }

    /// stage an owner's rule census. a zero count DELETES the key, so an
    /// owner with no rules left hashes the same as one who never created any
    /// (the tasks board's `stage_owner_count` rule).
    fn stage_owner_rule_count(&mut self, owner: &[u8], count: u64) {
        let key = owner_rule_count_key(owner);
        if count == 0 {
            self.staged.delete(key);
            return;
        }
        self.staged.stage(key, count.to_le_bytes().to_vec());
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

    /// a `DeliverInbox` action's `member_template` must be the exact literal
    /// inbox queue the rule owner itself would own — see the module doc's
    /// confused-deputy note. checked here at `CreateRule` (a rule that could
    /// never legally fire never burns a roster slot) and again at fire time
    /// in [`Self::build_and_emit`] (defense in depth against a future path
    /// that mutates a rule's owner). every other action is unconstrained.
    fn validate_deliver_inbox_owner(owner: &[u8], action: &Action) -> Result<(), Error> {
        let Action::DeliverInbox { member_template, .. } = action else {
            return Ok(());
        };
        let owner_queue = owner_queue(owner);
        let resolves_to_owner = member_template == &owner_queue;
        if !resolves_to_owner {
            return Err(Error::Module(format!(
                "inbox member_template must be the rule owner's own queue ({owner_queue}); \
                 no {{seq}}/{{author}}/other substitution can ever resolve to it"
            )));
        }
        Ok(())
    }

    // ---- admin ops ----------------------------------------------------------

    /// authorize an op on an EXISTING rule. owner-only, and that is the whole
    /// rule: a rule fires under this module's authority, so admitting anyone
    /// but the principal who took responsibility for it hands a stranger
    /// either a kill switch (`SetEnabled`/`DeleteRule` on someone else's
    /// automation) or, worse, a way to swap the standing grant for their own.
    fn check_rule_owner(rule: &Rule, submitter: &[u8]) -> Result<(), Error> {
        let is_owner = rule.owner == submitter;
        if !is_owner {
            return Err(Error::Module(format!(
                "only the owner may administer rule {}",
                rule.rule_id
            )));
        }
        Ok(())
    }

    async fn stage_create_rule(
        &mut self,
        owner: Vec<u8>,
        rule_id: String,
        trigger: Trigger,
        action: Action,
        consensus_time: u64,
    ) -> Result<(), Error> {
        require_non_empty("rule_id", &rule_id)?;
        Self::validate_len("rule_id", &rule_id, MAX_ID_BYTES)?;
        Self::validate_trigger(&trigger)?;
        Self::validate_action(&action)?;
        Self::validate_deliver_inbox_owner(&owner, &action)?;
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
        let owner_rules = self.owner_rule_count(&owner).await?;
        if owner_rules >= MAX_RULES_PER_OWNER as u64 {
            return Err(Error::Module(format!(
                "rule owner at cap: {MAX_RULES_PER_OWNER} rules"
            )));
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
                owner: owner.clone(),
                enabled: true,
                trigger,
                action,
                created_at: consensus_time,
                fire_count: 0,
            },
        );
        self.stage_owner_rule_count(&owner, owner_rules + 1);
        Ok(())
    }

    async fn stage_set_enabled(
        &mut self,
        submitter: &[u8],
        rule_id: String,
        enabled: bool,
    ) -> Result<(), Error> {
        require_non_empty("rule_id", &rule_id)?;
        let Some(mut rule) = self.rule(&rule_id).await? else {
            return Err(Error::Module(format!("unknown rule: {rule_id}")));
        };
        // BEFORE the idempotency short-circuit: a gate a no-op walks past is
        // not a gate, and a stranger must not learn a rule's enabled state
        // from which of the two refusals comes back.
        Self::check_rule_owner(&rule, submitter)?;
        if rule.enabled == enabled {
            // idempotent: staging nothing keeps the op log — and the root —
            // byte-identical to no write at all.
            return Ok(());
        }
        rule.enabled = enabled;
        self.store(rule_key(&rule_id), &rule);
        Ok(())
    }

    async fn stage_delete_rule(&mut self, submitter: &[u8], rule_id: String) -> Result<(), Error> {
        require_non_empty("rule_id", &rule_id)?;
        let mut roster = self.roster().await?;
        let Ok(position) = roster.binary_search(&rule_id) else {
            return Err(Error::Module(format!("unknown rule: {rule_id}")));
        };
        // the roster is the existence authority; the RECORD carries the owner.
        // a rostered id without a record is a store bug — loud, as everywhere.
        let Some(rule) = self.rule(&rule_id).await? else {
            return Err(Error::Module(format!("missing rule record: {rule_id}")));
        };
        Self::check_rule_owner(&rule, submitter)?;
        roster.remove(position);
        self.staged.delete(rule_key(&rule_id));
        // shrinking keeps the roster under its create-time byte gate.
        self.store(ROSTER_KEY.to_vec(), &roster);
        let owner_rules = self.owner_rule_count(&rule.owner).await?;
        self.stage_owner_rule_count(&rule.owner, owner_rules.saturating_sub(1));
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

        // the OWNER's standing decides what a rule OBSERVES: a rule matches
        // this event only if its owner may read the event's channel, so a
        // wildcard trigger means "every channel the owner can read" and never
        // "every channel this module is hooked into". a rule whose owner is not
        // admitted is simply NOT A MATCH — no record, because recording every
        // channel a rule cannot see is both a history bomb and the very
        // disclosure the gate exists to prevent.
        let mut access = AccessMemo::default();
        let mut candidates: Vec<&Rule> = Vec::new();
        for rule in &rules {
            if !rule.enabled || !Self::matches_channel_and_mention(rule, &channel_id, &mentions) {
                continue;
            }
            let owner_may_read = self
                .owner_access(&*ctx, &mut access, &rule.owner, &channel_id)
                .await
                .may_read;
            if !owner_may_read {
                continue;
            }
            candidates.push(rule);
        }

        // fetch the post's text ONCE, and only if some matching rule needs it
        // (a `text_contains` filter, or a `{text}` placeholder). `None` = the
        // fetch FAILED (query error / message absent), which is distinct from a
        // legitimately empty body (`Some("")`): rules that need text record a
        // failure on `None` instead of silently matching against emptiness.
        let needs_text = candidates.iter().copied().any(Self::rule_wants_text);
        let text: Option<String> = if needs_text {
            self.fetch_text(&*ctx, &channel_id, seq).await
        } else {
            Some(String::new())
        };
        let author_actor = actor_of(&author);
        let mention_actor = mentions.first().map(actor_of).unwrap_or_else(String::new);

        // evaluate in deterministic rule_id order (roster order).
        let mut budget = 0usize;
        let mut fired: Vec<Rule> = Vec::new();
        let mut records: Vec<RunRecord> = Vec::new();
        for rule in candidates {
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
                author: &author_actor,
                text,
                mention: &mention_actor,
            };
            match self
                .build_and_emit(ctx, &mut access, rule, &channel_id, seq, &vars)
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
        access: &mut AccessMemo,
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
                let body = substitute_bounded(template, vars);
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
                // probe 2, the AUTHORIZATION one, and the only probe that is
                // not about whether chat would ACCEPT the post: it rides out
                // under `Origin::Module("automations")`, which chat's post
                // policy admits unconditionally, so nothing downstream can
                // tell this text is a user's. the rule OWNER's own standing is
                // the gate — a rule may write only where its owner could have
                // written by hand. AFTER the existence probe, so a channel
                // that simply does not exist is named as that.
                let owner_may_post = self
                    .owner_access(&*ctx, access, &rule.owner, channel_id)
                    .await
                    .may_post;
                if !owner_may_post {
                    return Err(format!("rule owner may not post to {channel_id}"));
                }
                // probe 3: the deterministic message id must be unused — ids
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
                let title = substitute_bounded(title_template, vars);
                if title.is_empty() {
                    return Err("task template produced an empty title".into());
                }
                // deterministic, collision-free per (prefix, message).
                let task_id = format!("{task_id_prefix}-{event_channel}-{seq}");
                // composed-id guard BEFORE the probe (see PostMessage), held
                // against TASKS' rule rather than this module's: an id tasks
                // rejects at apply (over MAX_TASK_ID, or carrying the reserved
                // KEY_SEP a rule author is free to put in the prefix) unwinds
                // the triggering post. MAX_ID_BYTES happens to equal
                // MAX_TASK_ID today; that coincidence is not the constraint.
                sdk::validate_id("task_id", &task_id, tasks::MAX_TASK_ID)
                    .map_err(|e| e.to_string())?;
                // probe: the composed task id must be unused — tasks rejects
                // duplicates, which would abort the block. ONE by-id read: the
                // board walk this replaced cost a store read per task, so a
                // matching chat post on a large board blew the wasm host's
                // per-dispatch read budget and failed every rule from then on.
                // the read is overlay-aware on the tasks side, so a task staged
                // earlier in THIS block is seen (that visibility is why the
                // probe works at all).
                let req = tasks_encode_query(&TaskQuery::Get {
                    task_id: task_id.clone(),
                });
                match ctx.query(&self.tasks, &req).await {
                    Err(e) => return Err(format!("tasks probe failed: {e}")),
                    Ok(bytes) => match tasks_decode_reply(&bytes) {
                        Ok(TaskReply::Task(Some(_))) => {
                            return Err(format!("task id already exists: {task_id}"));
                        }
                        Ok(TaskReply::Task(None)) => {}
                        Ok(TaskReply::Tasks(_) | TaskReply::OwnerOpenCount(_)) => {
                            return Err("tasks answered a page, not a task".into());
                        }
                        Err(_) => return Err("tasks probe returned an unexpected reply".into()),
                    },
                }
                // probe: the RULE OWNER's own open-task census must be under
                // cap — the task is created under the owner, not this
                // module's identity (see the created task's `owner` below),
                // so a full owner refuses the RULE's action here, never the
                // triggering post's block.
                let owner_actor = owner_queue(&rule.owner);
                let req = tasks_encode_query(&TaskQuery::OwnerOpenCount {
                    owner: owner_actor.clone(),
                });
                match ctx.query(&self.tasks, &req).await {
                    Err(e) => return Err(format!("tasks probe failed: {e}")),
                    Ok(bytes) => match tasks_decode_reply(&bytes) {
                        Ok(TaskReply::OwnerOpenCount(count)) => {
                            if count >= tasks::MAX_OPEN_TASKS_PER_OWNER as u64 {
                                return Err(format!(
                                    "rule owner at task cap: {} open tasks",
                                    tasks::MAX_OPEN_TASKS_PER_OWNER
                                ));
                            }
                        }
                        _ => return Err("tasks probe returned an unexpected reply".into()),
                    },
                }
                ctx.emit_msg(Msg {
                    target: self.tasks.clone(),
                    payload: tasks_encode_msg(&TaskMsg::CreateTask {
                        task_id: task_id.clone(),
                        title,
                        // the created task's owner is the RULE OWNER, never
                        // this module's own id — see #1740: a task owned by
                        // the literal module id "automations" shares one
                        // 128-task budget across every rule on the network
                        // and can never be deleted (nothing submits as that
                        // actor). tasks honors this override only because the
                        // dispatch origin here is `Origin::Module`.
                        owner: Some(rule.owner.clone()),
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
                // re-check the confused-deputy gate at fire time (mirrors the
                // owner_access probe above): `CreateRule` already refuses any
                // member_template that cannot equal this, but a firing rule
                // never gets to deliver past it either, deterministically on
                // every validator.
                let owner_queue = owner_queue(&rule.owner);
                if member != owner_queue {
                    return Err(format!("rule owner may not deliver to {member}"));
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

    /// what the RULE OWNER may do in `channel_id`, per chat's own membership
    /// and post policy — the decider for BOTH what a rule may observe and
    /// where it may write. the owner is asked about, never this module: a rule
    /// fires under `Origin::Module("automations")`, which chat admits
    /// unconditionally, so the owner's standing is the only thing that can
    /// keep a rule from reaching past its author.
    ///
    /// one lookup per distinct `(owner, channel)` in an event fan-out. past
    /// [`MAX_ACCESS_PROBES_PER_EVENT`], and on a failed or unexpected reply,
    /// the answer is [`NO_ACCESS`] — deterministic on every validator, and
    /// closed.
    async fn owner_access(
        &self,
        ctx: &dyn Ctx,
        memo: &mut AccessMemo,
        owner: &[u8],
        channel_id: &str,
    ) -> ChannelAccess {
        let memoized = memo
            .seen
            .iter()
            .find(|((key_owner, key_channel), _)| key_owner == owner && key_channel == channel_id);
        if let Some((_, access)) = memoized {
            return *access;
        }
        if memo.seen.len() >= MAX_ACCESS_PROBES_PER_EVENT {
            return NO_ACCESS;
        }
        let req = chat_encode_query(&ChatQuery::Access {
            channel_id: channel_id.to_string(),
            user: owner.to_vec(),
        });
        let access = match ctx.query(&self.chat, &req).await {
            Ok(bytes) => match chat_decode_reply(&bytes) {
                Ok(ChatReply::Access(access)) => access,
                _ => NO_ACCESS,
            },
            Err(_) => NO_ACCESS,
        };
        memo.seen
            .push(((owner.to_vec(), channel_id.to_string()), access));
        access
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
                .any(|author| actor_of(author).contains(want.as_str()))
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

// ---- the one author rendering: the ACTOR-STRING domain -----------------------

struct TemplateVars<'a> {
    channel: &'a str,
    seq: Option<u64>,
    author: &'a str,
    text: &'a str,
    mention: &'a str,
}

/// the ONE deterministic rendering of a chat author: its
/// [`sdk::Origin::actor_string`], DERIVED from the origin the author handle
/// carries and never spelled here. it is what `{author}`/`{mention}` substitute
/// to and what a `mention` filter matches against.
///
/// the actor domain is not cosmetic — a `member_template` substitutes through
/// this same function, and an inbox queue IS named in this domain, so the
/// rendering a rule produces has to be one an origin can actually own. the
/// index tier's `user:{hex}` display handle is a DIFFERENT domain, and this
/// module deliberately does not speak it: no origin's actor string is ever
/// `user:…`, so a queue named that could never be marked read or cleared.
///
/// `AuthorRef::Agent` is the one arm with no origin of its own — an agent posts
/// under `Origin::Module(module)`, and `agent_id` REFINES that module's actor
/// string so a `mention` filter can address one agent rather than every post
/// its module makes.
fn actor_of(author: &AuthorRef) -> String {
    match author {
        AuthorRef::User(key) => Origin::External(key.clone()).actor_string(),
        AuthorRef::Agent { module, agent_id } => {
            format!(
                "{}/{agent_id}",
                Origin::Module(module.clone()).actor_string()
            )
        }
        AuthorRef::Module(module) => Origin::Module(module.clone()).actor_string(),
        AuthorRef::System => Origin::System.actor_string(),
    }
}

/// the inbox queue a rule OWNER owns — `sdk::Origin::External(owner)
/// .actor_string()`, the exact string [`inbox::deliver_is_permitted`] admits
/// for that owner on the direct path. every rule owner is an authenticated
/// external submitter ([`rule_owner`]), so this is total.
fn owner_queue(owner: &[u8]) -> String {
    Origin::External(owner.to_vec()).actor_string()
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

/// substitute, then clip the render to [`MAX_SUBSTITUTED_BYTES`] on a UTF-8
/// char boundary — the guard for every substituted string that rides a
/// FOLLOW-UP into another module (a post body, a task title). the inbox arm
/// does not use it: its fields are bounded by the inbox module's own caps.
fn substitute_bounded(template: &str, vars: &TemplateVars<'_>) -> String {
    let mut rendered = substitute_vars(template, vars);
    if rendered.len() <= MAX_SUBSTITUTED_BYTES {
        return rendered;
    }
    let mut keep = MAX_SUBSTITUTED_BYTES;
    while !rendered.is_char_boundary(keep) {
        keep -= 1;
    }
    rendered.truncate(keep);
    rendered
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
        // follow-ups reach the hook arm; everything else is an admin op. the
        // hook lane returns HERE, before the owner gate — a rule RUNS under a
        // module origin and is CREATED under a submitter's, and conflating the
        // two would either break every fire or leave creation ungated.
        let origin = ctx.env().origin.clone();
        let is_chat_hook = origin == Origin::Module(self.chat.clone());
        if is_chat_hook {
            return self.on_chat_event(ctx, &msg.payload).await;
        }
        // every admin op below is owner-bound, so the submitter is derived
        // ONCE — before the payload is even decoded — and every arm receives
        // it. an arm that took no submitter would be the whole class of bug
        // this gate exists to close.
        let submitter = rule_owner(&origin)?;
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            AutomationsMsg::CreateRule {
                rule_id,
                trigger,
                action,
            } => {
                let consensus_time = ctx.env().consensus_time;
                self.stage_create_rule(submitter, rule_id, trigger, action, consensus_time)
                    .await
            }
            AutomationsMsg::SetEnabled { rule_id, enabled } => {
                self.stage_set_enabled(&submitter, rule_id, enabled).await
            }
            AutomationsMsg::DeleteRule { rule_id } => {
                self.stage_delete_rule(&submitter, rule_id).await
            }
            AutomationsMsg::HookEvent(_) => Err(Error::Module(
                "hook events must originate from the chat module".into(),
            )),
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
