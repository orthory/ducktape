//! the agent module's public wire surface — types only.
//!
//! the agent module is the collaboration-loop consumer on the tagging and
//! dispatch planes: it registers agents, watches chat channels through the
//! tagging plane's engagement events, composes each engaged post's model
//! input in consensus, dispatches it under the agent's own recipe, and
//! validates the model's response before any cross-module write happens.
//! run LIFECYCLE is not this module's state — a dispatched task's lifecycle
//! lives in the dispatch module (and its saga); this surface only exposes the
//! module's own correlation entries for still-pending work. three payload
//! families cross this surface:
//!
//! - [`AgentMsg`] — writes: registry admin, channel watches, explicit run
//!   requests, and run cancellation.
//! - [`AgentResponse`] — the formal response wire spec: what a model's raw
//!   answer is normalized into, and what the strict-output instruction asks
//!   for. reply blocks are this module's OWN vocabulary (kind + text), not
//!   another module's — the agent module maps them to chat blocks when it
//!   emits the reply. it is DATA until the module deterministically validates
//!   it against the agent's allowed-action set.
//! - [`AgentQuery`] -> [`AgentReply`] — reads over agents, watches, and the
//!   pending (not-yet-delivered) runs.

use saga_interface::SagaOrigin;
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
/// rejects would abort that block (the no-fail rule) — so the agent module
/// must be able to prove the post will fit BEFORE emitting.
pub const MAX_REPLY_BLOCKS_BYTES: usize = 32 * 1024;

/// required byte length of an agent's prompt hash (a sha256 digest). prompt
/// CONTENT lives off-registry (e.g. in `document`); consensus commits to the
/// hash, so which prompt an agent runs is part of the app-hash.
pub const PROMPT_HASH_LEN: usize = 32;

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
/// pausing does not cancel work already dispatched.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
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
    pub prompt_hash: Vec<u8>,
    /// the document module doc holding the prompt CONTENT, when the prompt is
    /// consensus-resident. the canonical rendering (block texts joined by
    /// blank lines) must hash to `prompt_hash` — verified at dispatch time,
    /// so a drifted document fails the run's staging, never the block.
    /// `None` = no stored prompt; runs use generic instructions.
    pub prompt_doc: Option<String>,
    /// granted action names, each from [`KNOWN_ACTIONS`], sorted and deduped.
    pub allowed_actions: Vec<String>,
    pub status: AgentStatus,
    pub created_at: u64,
    pub updated_at: u64,
}

/// how a watched channel selects which agents a user post engages.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum TurnPolicy {
    /// agents whose `AuthorRef::Agent` ref appears in the post's mentions.
    Mention,
    /// every active agent.
    All,
    /// exactly this agent.
    Assigned(String),
    /// the sorted active agents indexed by `anchor_seq % n`.
    RoundRobin,
}

/// one channel watch — the agent-module-side mirror of the tagging-plane
/// subscription it was registered with (the two are staged atomically, P2).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct WatchView {
    pub channel_id: String,
    pub policy: TurnPolicy,
}

// ---- pending runs ---------------------------------------------------------------

/// one in-flight run's correlation entry: everything the module needs to act
/// on the dispatch plane's eventual `ResultEvent` — and NOTHING more. this is
/// not a lifecycle record: the entry exists exactly while the dispatch is
/// outstanding and is pruned when the result delivers. status and outcome
/// live in the dispatch module (`DispatchQuery::Dispatch`), keyed by
/// `dispatch_id` under receiver "agent".
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct PendingRun {
    /// `"chat\x1f{channel_id}\x1f{anchor_seq}\x1f{agent_id}"` for chat runs
    /// and `"job\x1f{job_id}\x1f{agent_id}\x1f{job_claim_height}"` for job
    /// runs — the turn-claim key; the first creation in consensus order wins.
    pub run_id: String,
    /// hex sha256 of `run_id` — the dispatch-plane id this entry correlates.
    pub dispatch_id: String,
    pub agent_id: String,
    /// empty for job-backed runs.
    pub channel_id: String,
    /// 0 for job-backed runs.
    pub anchor_seq: u64,
    /// the anchor's thread root, if the anchor was a thread reply — the reply
    /// posts into the same thread.
    pub thread_root: Option<u64>,
    /// present for jobs-board runs. chat-triggered runs leave this `None`.
    pub job_id: Option<String>,
    /// the jobs claim height a job-backed run is bound to; chat runs use 0.
    pub job_claim_height: u64,
    /// the run-creating origin (the tagging plane, or the explicit
    /// `RequestRun` submitter) — a cancel capability alongside the owner.
    pub requester: SagaOrigin,
    pub created_at: u64,
}

// ---- the response wire spec ----------------------------------------------------

/// one reply block in the module's OWN vocabulary — exactly the shape the
/// strict-output instruction asks the model for. `kind` is one of
/// "Paragraph", "Heading", or "Code" (anything else drops in normalization);
/// the agent module maps these to chat blocks at emission.
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
/// module wraps it as one; validation (grants, caps, probes) is a separate,
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
pub enum AgentAction {
    CreateTask {
        task_id: String,
        title: String,
    },
    /// `status` is the wire name of a `tasks_interface::TaskStatus`:
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
pub enum AgentMsg {
    /// register an agent under the submitter's origin (a non-empty external
    /// key or a module — the owner capability). a duplicate `agent_id` is an
    /// error. registration also registers the agent's dispatch-plane recipe
    /// (`agent/{agent_id}`) in the same block.
    RegisterAgent {
        agent_id: String,
        display_name: String,
        capability: String,
        prompt_hash: Vec<u8>,
        prompt_doc: Option<String>,
        allowed_actions: Vec<String>,
    },
    /// owner-gated partial update; `None` fields keep their current value
    /// (clearing `prompt_doc` means re-registering). a capability change also
    /// retunes the agent's dispatch-plane recipe in the same block.
    UpdateAgent {
        agent_id: String,
        display_name: Option<String>,
        capability: Option<String>,
        prompt_hash: Option<Vec<u8>>,
        prompt_doc: Option<String>,
        allowed_actions: Option<Vec<String>>,
    },
    /// owner-gated: stop the agent from engaging new runs.
    PauseAgent { agent_id: String },
    /// owner-gated: resume engagement.
    ResumeAgent { agent_id: String },
    /// watch a channel under `policy` AND subscribe on the tagging plane —
    /// one atomic block (P2), so the watch and the subscription cannot drift.
    WatchChannel {
        channel_id: String,
        policy: TurnPolicy,
    },
    /// drop the watch and the plane subscription, atomically.
    UnwatchChannel { channel_id: String },
    /// opt the agent module into or out of jobs-board submit notifications.
    /// the jobs module derives the worker id from this module's follow-up origin.
    EnableJobWorker { enabled: bool },
    /// explicitly run `agent_id` against `channel_id`/`anchor_seq` without an
    /// engagement. the duplicate of a pending or already-dispatched turn is a
    /// deterministic no-op — the turn claim: first in consensus order wins.
    RequestRun {
        agent_id: String,
        channel_id: String,
        anchor_seq: u64,
    },
    /// cancel a PENDING run — only the run-creating origin or the agent's
    /// owner. cancels the underlying dispatch in the same block; the plane's
    /// Err("cancelled") delivery then prunes the entry (and finalizes a
    /// job-backed run's job) through the one result path.
    CancelRun { run_id: String },
}

// ---- queries ------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum AgentQuery {
    Agents,
    Agent { agent_id: String },
    /// every in-flight correlation entry, ascending by dispatch id. bounded:
    /// entries prune on delivery, and every dispatch has a deadline.
    PendingRuns,
    Watches,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum AgentReply {
    Agents(Vec<AgentRecord>),
    Agent(Option<AgentRecord>),
    PendingRuns(Vec<PendingRun>),
    Watches(Vec<WatchView>),
}

// ---- codecs -------------------------------------------------------------------

pub fn encode_msg(m: &AgentMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}
pub fn decode_msg(b: &[u8]) -> Result<AgentMsg, String> {
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
