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

pub const DEFAULT_AGENT_TARGET: &str = "agent";

// ---- consensus constants ----------------------------------------------------

/// hard cap on the actions one run's response may carry — the blast-radius
/// bound on the follow-up fan-out a single delivery can cause.
pub const MAX_ACTIONS_PER_RUN: usize = 8;

/// hard cap on a serialized [`AgentRecord`] — registry entries live in the
/// root preimage and every snapshot, so registration is size-gated up front.
pub const MAX_AGENT_RECORD_BYTES: usize = 4 * 1024;

/// hard cap on the serialized CHAT blocks a response's `reply_blocks` map to.
/// deliberately well under chat's `MAX_MESSAGE_HEAD_BYTES`: the reply is
/// emitted as a chat post inside the delivery block, and a post that chat
/// rejects would abort that block (the no-fail rule) — so the delivering
/// module must be able to prove the post will fit BEFORE emitting.
pub const MAX_REPLY_BLOCKS_BYTES: usize = 32 * 1024;

/// required byte length of a [`PromptRef`]'s sha256 pin. prompt CONTENT lives
/// in the source module the ref names (e.g. `memory`); consensus commits to
/// the pin, so which prompt an agent runs is part of the app-hash.
pub const PROMPT_HASH_LEN: usize = 32;

/// the v1 prompt renderer: the ref's `target` is `"<path>@<generation>"`,
/// resolved at compose time via the source module's
/// `MemoryQuery::Read { path, generation }`; the content must be inline and
/// must sha256 to the registered pin.
pub const RENDERER_MEMORY_GENERATION: &str = "memory.generation";

/// every prompt renderer the platform knows. registration rejects a
/// [`PromptRef`] whose `renderer` is outside this set, so a registered prompt
/// is always resolvable by the module that composes payloads.
pub const KNOWN_PROMPT_RENDERERS: [&str; 1] = [RENDERER_MEMORY_GENERATION];

/// a consensus-resident reference to an agent's prompt content: which module
/// holds it (`module`), where inside that module (`target`, a
/// renderer-specific coordinate), how to resolve it (`renderer`, from
/// [`KNOWN_PROMPT_RENDERERS`]), and the sha256 pin the resolved content must
/// hash to (exactly [`PROMPT_HASH_LEN`] bytes). registration validates all
/// four; resolution happens at payload-compose time in the module that runs
/// agents, and content that no longer hashes to the pin fails the run.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct PromptRef {
    pub module: String,
    pub target: String,
    pub renderer: String,
    pub sha256: Vec<u8>,
}

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

/// every action name the platform knows. `RegisterAgent`/`UpdateAgent` reject
/// an `allowed_actions` entry outside this vocabulary, so a granted permission
/// always means something.
pub const KNOWN_ACTIONS: [&str; 3] = [
    ACTION_CHAT_POST,
    ACTION_TASKS_CREATE,
    ACTION_TASKS_UPDATE_STATUS,
];

// ---- registry ----------------------------------------------------------------

/// whether an agent may engage new runs. a paused agent never engages — but
/// pausing does not cancel work already dispatched. a tombstoned agent is
/// TERMINAL: it never engages, never claims jobs, and cannot be resumed or
/// otherwise mutated — the record stays for audit, the id stays reserved.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Active,
    Paused,
    Tombstoned,
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
    /// where the agent's prompt content lives and what it must hash to.
    /// `None` means the composing module's generic default prompt.
    pub prompt: Option<PromptRef>,
    /// granted action names (shape-validated open-set tags), sorted and deduped.
    pub allowed_actions: Vec<String>,
    pub status: AgentStatus,
    pub created_at: u64,
    pub updated_at: u64,
}

// ---- the response wire spec ----------------------------------------------------

/// one reply block in this surface's OWN vocabulary — exactly the shape the
/// strict-output instruction asks the model for. `kind` is one of
/// "Paragraph", "Heading", or "Code" (anything else drops in normalization);
/// the consuming module maps these to chat blocks at emission.
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
}

impl AgentAction {
    /// the vocabulary name this action needs in the agent's `allowed_actions`.
    pub fn vocabulary_name(&self) -> &'static str {
        match self {
            AgentAction::CreateTask { .. } => ACTION_TASKS_CREATE,
            AgentAction::UpdateTaskStatus { .. } => ACTION_TASKS_UPDATE_STATUS,
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
        /// `None` keeps the composing module's generic default prompt.
        prompt: Option<PromptRef>,
        allowed_actions: Vec<String>,
    },
    /// owner-gated partial update; `None` fields keep their current value
    /// (clearing a registered prompt means re-registering). a capability
    /// change also notifies the hook target
    /// ([`AgentEvent::CapabilityChanged`]) in the same block.
    UpdateAgent {
        agent_id: String,
        display_name: Option<String>,
        capability: Option<String>,
        prompt: Option<PromptRef>,
        allowed_actions: Option<Vec<String>>,
    },
    /// owner-gated: stop the agent from engaging new runs.
    PauseAgent { agent_id: String },
    /// owner-gated: resume engagement.
    ResumeAgent { agent_id: String },
    /// owner-gated and TERMINAL: retire the agent for good. the record stays
    /// for audit (the id stays reserved), but every later mutation — resume
    /// included — is rejected. the hook target is notified
    /// ([`AgentEvent::Tombstoned`]) in the same block so the agent's
    /// dispatch-plane recipe is removed atomically with the retirement.
    TombstoneAgent { agent_id: String },
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
    /// an agent was retired for good; the hook removes its recipe.
    Tombstoned { agent_id: String },
}

// ---- queries ------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentQuery {
    Agents,
    Agent { agent_id: String },
}

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
