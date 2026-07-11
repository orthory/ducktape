//! the agent module's public wire surface — types only.
//!
//! the agent module is the platform's agent REGISTRY and nothing more: a
//! self-contained record book of which agents exist — owner, capability tag,
//! prompt pin, granted actions, status. it consumes no other module's events
//! and emits exactly one follow-up shape ([`AgentEvent`], to a
//! genesis-configured hook target) so the module that runs agents can keep
//! each agent's dispatch-plane recipe in lockstep, atomically with the
//! registration that changed it. engagement, run orchestration, and response
//! delivery live in the runs module (`runs`). three payload
//! families cross this surface:
//!
//! - [`AgentMsg`] — writes: registry admin only.
//! - [`AgentResponse`] — the formal response wire spec: what a model's raw
//!   answer is normalized into, and what the strict-output instruction asks
//!   for. reply blocks are this surface's OWN vocabulary (kind + text), not
//!   another module's — the consuming module maps them to chat blocks when it
//!   emits the reply. it is DATA until deterministically validated against
//!   the agent's allowed-action set.
//! - [`AgentQuery`] -> [`AgentReply`] — reads over the registry.

use saga::SagaOrigin;
use serde::{Deserialize, Serialize};

// ---- consensus constants ----------------------------------------------------

/// hard cap on the actions one run's response may carry — the blast-radius
/// bound on the follow-up fan-out a single delivery can cause.
pub const MAX_ACTIONS_PER_RUN: usize = 8;

/// hard cap on the SERIALIZED bytes of a response's actions — the byte peer of
/// [`MAX_ACTIONS_PER_RUN`]'s count cap. action payloads (task ids, titles,
/// statuses) are otherwise unbounded strings, and the delivering module embeds
/// the validated response in a bounded job-finalize payload — so, like
/// [`MAX_REPLY_BLOCKS_BYTES`], it must be able to prove the size BEFORE
/// emitting.
pub const MAX_ACTIONS_BYTES: usize = 8 * 1024;

/// hard cap on a serialized [`AgentRecord`] — registry entries live in the
/// root preimage and every snapshot, so registration is size-gated up front.
pub const MAX_AGENT_RECORD_BYTES: usize = 4 * 1024;

/// hard cap on the serialized CHAT blocks a response's `reply_blocks` map to.
/// deliberately well under chat's `MAX_MESSAGE_HEAD_BYTES`: the reply is
/// emitted as a chat post inside the delivery block, and a post that chat
/// rejects would abort that block (the no-fail rule) — so the delivering
/// module must be able to prove the post will fit BEFORE emitting.
pub const MAX_REPLY_BLOCKS_BYTES: usize = 32 * 1024;

/// required byte length of an agent's prompt hash (a sha256 digest). prompt
/// CONTENT lives off-registry (e.g. in `document`); consensus commits to the
/// hash, so which prompt an agent runs is part of the app-hash.
pub const PROMPT_HASH_LEN: usize = 32;

/// the reserved unit separator agent ids must never contain: the runs module
/// keys its run records with `\x1f`-delimited fields, and an agent id
/// carrying the delimiter would make those keys ambiguous. the registry
/// rejects it at registration; downstream modules rely on that.
pub const RESERVED_ID_SEPARATOR: char = '\u{1f}';

// ---- the action vocabulary ---------------------------------------------------

/// permission to post reply blocks into chat.
pub const ACTION_CHAT_POST: &str = "chat.post";
/// permission to create a task ([`AgentAction::CreateTask`]).
pub const ACTION_TASKS_CREATE: &str = "tasks.create";
/// permission to move a task ([`AgentAction::UpdateTaskStatus`]).
pub const ACTION_TASKS_UPDATE_STATUS: &str = "tasks.update_status";
/// permission to anchor a comment to a page or block
/// ([`AgentAction::AddPageComment`]).
pub const ACTION_PAGES_COMMENT: &str = "pages.comment";
/// permission to flip a todo block's checked state
/// ([`AgentAction::SetPageChecked`]).
pub const ACTION_PAGES_SET_CHECKED: &str = "pages.set_checked";

/// every action name the platform knows. `RegisterAgent`/`UpdateAgent` reject
/// an `allowed_actions` entry outside this vocabulary, so a granted permission
/// always means something.
pub const KNOWN_ACTIONS: [&str; 5] = [
    ACTION_CHAT_POST,
    ACTION_TASKS_CREATE,
    ACTION_TASKS_UPDATE_STATUS,
    ACTION_PAGES_COMMENT,
    ACTION_PAGES_SET_CHECKED,
];

// ---- runtime identity ---------------------------------------------------------

/// the D3 resource-capability grant an agent carries. every list is a
/// canonical SORTED + DEDUPED set (the write path canonicalizes, the committed
/// decoder rejects a non-ascending list) so two logically-equal grants hash
/// identically. `secrets` are OPAQUE vault references (D6) — never a
/// materialized value, never key material (D1). an empty `ResourceCaps` is the
/// default and denies every request except a zero budget check.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct ResourceCaps {
    /// forge repos this agent may READ.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forge_read: Vec<String>,
    /// forge repos this agent may PUSH to (implies read).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forge_push: Vec<String>,
    /// duckfs workspace-relative path prefixes this agent may READ (ro).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub duckfs_read: Vec<String>,
    /// duckfs workspace-relative path prefixes this agent may WRITE (rw).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub duckfs_write: Vec<String>,
    /// tool / mcp ids this agent may invoke.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<String>,
    /// D6 vault references (scoped, opaque). refs only — the value is resolved
    /// host-side and NEVER crosses consensus.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secrets: Vec<String>,
    /// page ids this agent may WRITE (comment on / check off). page ids are
    /// opaque, so matching is exact — no prefix containment — with the one
    /// literal entry `"*"` granting every page.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pages_write: Vec<String>,
    /// the D3 sub-agent spawn ceiling; 0 = none. consumption is the runtime's
    /// concern; the record only states the ceiling.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub subagent_budget: u32,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// whether a [`ResourceCaps`] is the empty default — used to keep the empty
/// record's serialized JSON (and its `MAX_AGENT_RECORD_BYTES` size check)
/// byte-lean, so a pre-v4-shaped record is unchanged on the wire.
pub(crate) fn caps_is_default(c: &ResourceCaps) -> bool {
    *c == ResourceCaps::default()
}

/// a C4 skill reference an agent's runs mount. this pins the REF, never the
/// content: `source_prefix` is a duckfs read-only subtree and
/// `source_snapshot` is its optional consensus pin — `Some` is a PINNED skill
/// (immutable), `None` is a TRACKING skill (the phase-5 composer resolves the
/// committed head at compose time). the list is ORDERED (later entries override
/// earlier), so it is a `Vec`, not a set — order is significant to the hash.
///
/// deliberately a struct (not an enum): the phase-5 envelope composer reads
/// `name` + `source_prefix` + `source_snapshot` straight through into a skill
/// mount, so the same three fields live here.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SkillRef {
    pub name: String,
    pub source_prefix: String,
    /// `Some` = pinned (immutable) snapshot id; `None` = tracking (resolved at
    /// compose time).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_snapshot: Option<String>,
}

/// a D3 capability request the runtime probes an [`AgentRecord`] with before
/// applying an effect or opening a sink (the delivery path calls
/// [`AgentRecord::permits`]). the record carries the grant; this is the ask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapRequest<'a> {
    /// read the named forge repo.
    ForgeRead(&'a str),
    /// push to the named forge repo.
    ForgePush(&'a str),
    /// write the named duckfs workspace-relative path.
    DuckfsWrite(&'a str),
    /// read the named duckfs workspace-relative path.
    DuckfsRead(&'a str),
    /// invoke the named tool / mcp.
    Tool(&'a str),
    /// resolve the named vault secret ref.
    Secret(&'a str),
    /// write (comment on / check off) the named page.
    PagesWrite(&'a str),
    /// spawn a sub-agent (checked against the budget ceiling).
    SpawnSubagent,
}

// ---- registry ----------------------------------------------------------------

/// whether an agent may engage new runs. a paused agent never engages — but
/// pausing does not cancel work already dispatched.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Active,
    Paused,
}

/// one registered agent — an ordered-op registration, so which capability and
/// prompt an agent runs is part of the app-hash and auditable. `owner` is the
/// registration origin and gates every mutation of the record.
///
/// `capability` names WHAT the run needs (an open-set registry tag like
/// "codex" — dispatch selects providers of that tag); HOW it runs — binary,
/// flags, model — is host policy in each provider's capability spec, and
/// consensus never sees it. the record is a recipe, not an executor config.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AgentRecord {
    pub agent_id: String,
    pub owner: SagaOrigin,
    pub display_name: String,
    /// the capability registry tag this agent's runs are dispatched on.
    pub capability: String,
    /// sha256 of the agent's prompt content (exactly [`PROMPT_HASH_LEN`] bytes).
    /// the content itself is content-addressed in the blob store under this
    /// digest; consensus pins only the hash, and the host resolves the prompt.
    pub prompt_hash: Vec<u8>,
    /// granted action names, each from [`KNOWN_ACTIONS`], sorted and deduped.
    pub allowed_actions: Vec<String>,
    pub status: AgentStatus,
    pub created_at: u64,
    pub updated_at: u64,
    /// W4 recipe content-address: empty (unset) or exactly [`PROMPT_HASH_LEN`]
    /// bytes. the committed encoding always carries it (empty when unset).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recipe_hash: Vec<u8>,
    /// D3 resource caps. the committed encoding always carries it (default-empty
    /// when unset).
    #[serde(default, skip_serializing_if = "caps_is_default")]
    pub caps: ResourceCaps,
    /// C4 ordered skill refs. the committed encoding always carries it (empty
    /// when unset).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<SkillRef>,
}

impl AgentRecord {
    /// the pure D3 cap gate. the runtime calls this before applying an effect
    /// or opening a sink; a record with empty caps denies every request except a
    /// positive budget check. forge/tool/
    /// secret use exact membership; duckfs uses path-PREFIX containment (a
    /// prefix grants itself and any child path, but never a sibling that merely
    /// shares a textual prefix — `src` does not grant `srcx`); pages use exact
    /// membership with the literal `"*"` entry granting every page (ids are
    /// opaque — never a prefix). budget
    /// CONSUMPTION is the runtime's concern; this only reads the ceiling.
    pub fn permits(&self, req: &CapRequest) -> bool {
        let c = &self.caps;
        let has = |v: &[String], x: &str| v.iter().any(|s| s == x);
        let under = |v: &[String], p: &str| {
            v.iter()
                .any(|pre| p == pre || p.starts_with(&format!("{pre}/")))
        };
        match req {
            CapRequest::ForgeRead(r) => has(&c.forge_read, r) || has(&c.forge_push, r),
            CapRequest::ForgePush(r) => has(&c.forge_push, r),
            CapRequest::DuckfsWrite(p) => under(&c.duckfs_write, p),
            CapRequest::DuckfsRead(p) => under(&c.duckfs_read, p) || under(&c.duckfs_write, p),
            CapRequest::Tool(t) => has(&c.tools, t),
            CapRequest::Secret(s) => has(&c.secrets, s),
            CapRequest::PagesWrite(p) => has(&c.pages_write, "*") || has(&c.pages_write, p),
            CapRequest::SpawnSubagent => c.subagent_budget > 0,
        }
    }
}

// ---- the response wire spec ----------------------------------------------------

/// one reply block in this surface's OWN vocabulary — exactly the shape the
/// strict-output instruction asks the model for. `kind` is one of
/// "paragraph", "heading", or "code" (lowercase; anything else drops in
/// normalization); the consuming module maps these to chat blocks at emission.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ReplyBlock {
    pub kind: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
}

/// the formal agent response: reply blocks plus a bounded list of
/// [`AgentAction`]s. lenient by construction — both fields default, unknown
/// JSON fields are ignored — so a model answer either IS this shape or the
/// consumer wraps it as one; validation (grants, caps, probes) is a separate,
/// strict step.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentResponse {
    #[serde(default)]
    pub reply_blocks: Vec<ReplyBlock>,
    #[serde(default)]
    pub actions: Vec<AgentAction>,
}

/// one validated cross-module write an agent's response may request.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentAction {
    CreateTask {
        task_id: String,
        title: String,
    },
    /// `status` is the wire name of a `tasks::TaskStatus`:
    /// `"open"`, `"in_progress"`, or `"done"`.
    UpdateTaskStatus {
        task_id: String,
        status: String,
    },
    /// anchor a comment to `target` — a page id or a block id in the pages
    /// module ([`ACTION_PAGES_COMMENT`]).
    AddPageComment {
        target: String,
        body: String,
    },
    /// flip a todo block's checked state ([`ACTION_PAGES_SET_CHECKED`]).
    SetPageChecked {
        block: String,
        checked: bool,
    },
}

impl AgentAction {
    /// the vocabulary name this action needs in the agent's `allowed_actions`.
    pub fn vocabulary_name(&self) -> &'static str {
        match self {
            AgentAction::CreateTask { .. } => ACTION_TASKS_CREATE,
            AgentAction::UpdateTaskStatus { .. } => ACTION_TASKS_UPDATE_STATUS,
            AgentAction::AddPageComment { .. } => ACTION_PAGES_COMMENT,
            AgentAction::SetPageChecked { .. } => ACTION_PAGES_SET_CHECKED,
        }
    }
}

// ---- ops ----------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentMsg {
    /// register an agent under the submitter's origin (a non-empty external
    /// key or a module — the owner capability). a duplicate `agent_id` is an
    /// error. registration also notifies the configured hook target
    /// ([`AgentEvent::Registered`]) in the same block, so the agent's
    /// dispatch-plane recipe lands (or aborts) atomically with the record.
    RegisterAgent {
        agent_id: String,
        display_name: String,
        capability: String,
        prompt_hash: Vec<u8>,
        allowed_actions: Vec<String>,
        /// runtime-identity fields. `default` so a submitter's JSON that omits
        /// them still decodes; the module accepts them unconditionally.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        recipe_hash: Option<Vec<u8>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        caps: Option<ResourceCaps>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        skills: Option<Vec<SkillRef>>,
    },
    /// owner-gated partial update; `None` fields keep their current value. a
    /// capability change also notifies the hook target
    /// ([`AgentEvent::CapabilityChanged`]) in the same block.
    UpdateAgent {
        agent_id: String,
        display_name: Option<String>,
        capability: Option<String>,
        prompt_hash: Option<Vec<u8>>,
        allowed_actions: Option<Vec<String>>,
        /// runtime-identity fields; `None` keeps the current value.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        recipe_hash: Option<Vec<u8>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        caps: Option<ResourceCaps>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        skills: Option<Vec<SkillRef>>,
    },
    /// owner-gated: stop the agent from engaging new runs.
    PauseAgent { agent_id: String },
    /// owner-gated: resume engagement.
    ResumeAgent { agent_id: String },
}

// ---- the registry hook ----------------------------------------------------------

/// the registry's ONE follow-up shape, emitted to a genesis-configured hook
/// target (the runs module) in the same block as the registry write that
/// caused it. the hook keeps the agent's dispatch-plane recipe in lockstep:
/// if the recipe registration is rejected (a squatted id), the whole block
/// aborts and the staged registry write vanishes with it — the agent and its
/// recipe stay ONE atomic unit without the registry referencing the dispatch
/// plane.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentEvent {
    /// a new agent landed; the hook registers its recipe.
    Registered { agent_id: String, capability: String },
    /// an existing agent's capability changed; the hook retunes its recipe.
    CapabilityChanged { agent_id: String, capability: String },
}

// ---- queries ------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentQuery {
    Agents,
    Agent { agent_id: String },
}

// the runtime-identity tail grew `AgentRecord` past clippy's 200-byte
// `large_enum_variant` threshold. this is a query REPLY, built rarely and moved
// once through a channel, not a hot per-op allocation — boxing the variant
// would ripple a wire/type change through every reader for no real benefit, so
// the size asymmetry is accepted (mirrors the saga/reachability reply enums).
#[allow(clippy::large_enum_variant)]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentReply {
    Agents(Vec<AgentRecord>),
    Agent(Option<AgentRecord>),
}

// ---- codecs -------------------------------------------------------------------

pub fn encode_msg(m: &AgentMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}
pub fn decode_msg(b: &[u8]) -> Result<AgentMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_event(e: &AgentEvent) -> Vec<u8> {
    serde_json::to_vec(e).expect("serializable")
}
pub fn decode_event(b: &[u8]) -> Result<AgentEvent, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_response(r: &AgentResponse) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}
pub fn decode_response(b: &[u8]) -> Result<AgentResponse, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_query(q: &AgentQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}
pub fn decode_query(b: &[u8]) -> Result<AgentQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_reply(r: &AgentReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}
pub fn decode_reply(b: &[u8]) -> Result<AgentReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
