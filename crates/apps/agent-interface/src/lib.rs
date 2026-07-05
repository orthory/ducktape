//! the agent module's public wire surface — types only.
//!
//! the agent module is the collaboration-loop orchestrator (design §3, §5):
//! it registers agents, watches chat channels through hooks, turns engaged
//! posts into runs, pins each run to the exact transcript prefix it saw, and
//! validates the LLM's output before any cross-module write happens. four
//! payload families cross this surface:
//!
//! - [`AgentMsg`] — writes: registry admin, channel watches, explicit run
//!   requests, and run cancellation.
//! - [`LlmRequest`] — the saga work spec a trigger carries: everything an
//!   off-consensus LLM worker needs to re-derive the prompt input (P4) and
//!   answer under the right idempotency key.
//! - [`AgentOutput`] — the agreed oracle result: reply blocks for chat plus a
//!   bounded list of [`AgentAction`]s. it is DATA until the agent module
//!   deterministically validates it against the agent's allowed-action set.
//! - [`AgentQuery`] -> [`AgentReply`] — reads over agents, watches, and runs.

use chat_interface::{Block, MessageView};
use saga_interface::SagaOrigin;
use serde::{Deserialize, Serialize};

pub const DEFAULT_AGENT_TARGET: &str = "agent";

// ---- consensus constants ----------------------------------------------------

/// hard cap on the actions one run's output may carry — the blast-radius
/// bound on the follow-up fan-out a single oracle result can cause.
pub const MAX_ACTIONS_PER_RUN: usize = 8;

/// hard cap on a serialized [`AgentRecord`] — registry entries live in the
/// root preimage and every snapshot, so registration is size-gated up front.
pub const MAX_AGENT_RECORD_BYTES: usize = 4 * 1024;

/// hard cap on the serialized `reply_blocks` of one output. deliberately well
/// under chat's `MAX_MESSAGE_HEAD_BYTES`: the reply is re-emitted as a chat
/// post in the SAME block as the saga's terminal transition, and a post that
/// chat rejects would abort that block and wedge the saga (the no-fail rule) —
/// so the agent module must be able to prove the post will fit BEFORE emitting.
pub const MAX_REPLY_BLOCKS_BYTES: usize = 32 * 1024;

/// required byte length of an agent's prompt hash (a sha256 digest). prompt
/// CONTENT lives off-registry (e.g. in `document`); consensus commits to the
/// hash, so which prompt an agent runs is part of the app-hash.
pub const PROMPT_HASH_LEN: usize = 32;

/// query page bound; larger limits are clamped down to this.
pub const MAX_QUERY_LIMIT: u64 = 256;

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
/// pausing does not cancel runs already awaiting their oracle.
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

/// one channel watch — the agent-module-side mirror of the chat hook it was
/// registered with (the two are staged atomically, P2).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct WatchView {
    pub channel_id: String,
    pub policy: TurnPolicy,
}

// ---- runs ---------------------------------------------------------------------

/// where a run is in its lifecycle. a run is created already awaiting its
/// oracle (the saga trigger is staged in the same dispatch) and only ordered
/// ops move it to a terminal state.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum RunStatus {
    /// the saga carrying this run's LLM work, awaited for its callback.
    AwaitingOracle { saga_id: String },
    /// the output validated and its follow-ups were emitted.
    Done,
    /// the saga failed / timed out, or the output failed validation.
    Failed { reason: String },
    /// cancelled by the run's creator or the agent's owner.
    Cancelled,
}

impl RunStatus {
    /// true for every state a run can never leave.
    pub fn is_terminal(&self) -> bool {
        !matches!(self, RunStatus::AwaitingOracle { .. })
    }
}

/// a run's observable state — the full read projection.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RunView {
    /// `"chat\x1f{channel_id}\x1f{anchor_seq}\x1f{agent_id}"` for chat runs
    /// and `"job\x1f{job_id}\x1f{agent_id}\x1f{job_claim_height}"` for job runs:
    /// the first creation in consensus order wins, duplicates are no-ops.
    pub run_id: String,
    pub agent_id: String,
    pub channel_id: String,
    /// the message sequence this run answers; the reply is never presented as
    /// ordered before it (P4).
    pub anchor_seq: u64,
    /// the anchor's thread root, if the anchor was a thread reply — the reply
    /// posts into the same thread.
    pub thread_root: Option<u64>,
    /// present for jobs-board runs. chat-triggered runs leave this `None`.
    pub job_id: Option<String>,
    /// the jobs claim height for job-backed runs. chat-triggered runs use 0.
    pub job_claim_height: u64,
    /// the run-creating origin (the hook's chat module, or the explicit
    /// `RequestRun` submitter) — a cancel capability alongside the owner.
    pub requester: SagaOrigin,
    pub status: RunStatus,
    /// sha256 over the pinned transcript window up to `anchor_seq` — any
    /// validator can re-derive the prompt input from the log (P4).
    pub context_hash: Vec<u8>,
    pub created_at: u64,
    pub updated_at: u64,
}

// ---- the saga spec + the oracle result ----------------------------------------

/// the work spec an agent run's saga trigger carries — what the off-consensus
/// LLM worker decodes. `context_hash` pins the exact transcript prefix the
/// prompt must be built from: the worker re-derives the window from its
/// replica and verifies it hashes to this value before calling the model.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct LlmRequest {
    pub run_id: String,
    pub agent_id: String,
    /// the capability tag the executing host resolves to a local provider.
    pub capability: String,
    pub prompt_hash: Vec<u8>,
    pub channel_id: String,
    pub anchor_seq: u64,
    /// present for jobs-board runs. for those runs `context_hash` pins the
    /// submitted job spec bytes instead of a chat transcript prefix.
    pub job_id: Option<String>,
    pub context_hash: Vec<u8>,
    /// the pinned chat transcript window the run was staged from. older specs
    /// decode with an empty transcript; job-backed runs also leave this empty.
    #[serde(default)]
    pub transcript: Vec<MessageView>,
}

/// one validated cross-module write an agent's output may request.
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

/// the oracle result payload of an agent run: what the LLM answered, as data.
/// the agent module validates it DETERMINISTICALLY (non-empty, caps, the
/// agent's allowed actions) before emitting any follow-up; an invalid output
/// fails the run and writes nothing.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AgentOutput {
    pub reply_blocks: Vec<Block>,
    pub actions: Vec<AgentAction>,
}

// ---- ops ----------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum AgentMsg {
    /// register an agent under the submitter's origin (a non-empty external
    /// key or a module — the owner capability). a duplicate `agent_id` is an
    /// error.
    RegisterAgent {
        agent_id: String,
        display_name: String,
        capability: String,
        prompt_hash: Vec<u8>,
        allowed_actions: Vec<String>,
    },
    /// owner-gated partial update; `None` fields keep their current value.
    UpdateAgent {
        agent_id: String,
        display_name: Option<String>,
        capability: Option<String>,
        prompt_hash: Option<Vec<u8>>,
        allowed_actions: Option<Vec<String>>,
    },
    /// owner-gated: stop the agent from engaging new runs.
    PauseAgent { agent_id: String },
    /// owner-gated: resume engagement.
    ResumeAgent { agent_id: String },
    /// watch a channel under `policy` AND register the agent module as a chat
    /// hook — one atomic block (P2), so the watch and the hook cannot drift.
    WatchChannel {
        channel_id: String,
        policy: TurnPolicy,
    },
    /// drop the watch and unregister the hook, atomically.
    UnwatchChannel { channel_id: String },
    /// opt the agent module into or out of jobs-board submit notifications.
    /// the jobs module derives the worker id from this module's follow-up origin.
    EnableJobWorker { enabled: bool },
    /// explicitly run `agent_id` against `channel_id`/`anchor_seq` without a
    /// hook. the duplicate of an existing run is a deterministic no-op — the
    /// turn claim: first creation in consensus order wins.
    RequestRun {
        agent_id: String,
        channel_id: String,
        anchor_seq: u64,
    },
    /// cancel an awaiting run — only the run-creating origin or the agent's
    /// owner. cancels the underlying saga in the same block.
    CancelRun { run_id: String },
}

// ---- queries ------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum AgentQuery {
    Agents,
    Agent {
        agent_id: String,
    },
    /// runs, ascending by run id, optionally filtered to one channel; `limit`
    /// is clamped to [`MAX_QUERY_LIMIT`].
    Runs {
        channel_id: Option<String>,
        limit: u64,
    },
    Run {
        run_id: String,
    },
    Watches,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum AgentReply {
    Agents(Vec<AgentRecord>),
    Agent(Option<AgentRecord>),
    Runs(Vec<RunView>),
    Run(Option<RunView>),
    Watches(Vec<WatchView>),
}

// ---- codecs -------------------------------------------------------------------

pub fn encode_msg(m: &AgentMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}
pub fn decode_msg(b: &[u8]) -> Result<AgentMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_llm_request(r: &LlmRequest) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}
pub fn decode_llm_request(b: &[u8]) -> Result<LlmRequest, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_output(o: &AgentOutput) -> Vec<u8> {
    serde_json::to_vec(o).expect("serializable")
}
pub fn decode_output(b: &[u8]) -> Result<AgentOutput, String> {
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
