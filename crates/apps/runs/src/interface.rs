//! the runs module's public wire surface — types only.
//!
//! the runs module is the collaboration loop's actor — the consumer on the
//! tagging and dispatch planes: it watches chat channels through the tagging
//! plane's engagement events, composes each engaged post's model input in
//! consensus, dispatches it under the agent's recipe, and validates the
//! model's response before any cross-module write happens. the agents it runs
//! live in the agent REGISTRY (`agent`) — this module reads them by
//! query and never holds registry state. run LIFECYCLE is not this module's
//! state either — a dispatched task's lifecycle lives in the dispatch module
//! (and its saga); this surface only exposes the module's own correlation
//! entries for still-pending work. two payload families cross this surface:
//!
//! - [`RunsMsg`] — writes: channel watches, the jobs-worker toggle, explicit
//!   run requests, and run cancellation.
//! - [`RunsQuery`] -> [`RunsReply`] — reads over watches and the pending
//!   (not-yet-delivered) runs.

use saga::SagaOrigin;
use serde::{Deserialize, Serialize};

pub const DEFAULT_RUNS_TARGET: &str = "runs";

// ---- watches ------------------------------------------------------------------

/// how a watched channel selects which agents a user post engages.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
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

/// one channel watch — the runs-module-side mirror of the tagging-plane
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
/// `dispatch_id` under receiver "runs".
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

// ---- ops ----------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunsMsg {
    /// watch a channel under `policy` AND subscribe on the tagging plane —
    /// one atomic block (P2), so the watch and the subscription cannot drift.
    WatchChannel {
        channel_id: String,
        policy: TurnPolicy,
    },
    /// drop the watch and the plane subscription, atomically.
    UnwatchChannel { channel_id: String },
    /// opt the runs module into or out of jobs-board submit notifications.
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
#[serde(rename_all = "snake_case")]
pub enum RunsQuery {
    /// every in-flight correlation entry, ascending by dispatch id. bounded:
    /// entries prune on delivery, and every dispatch has a deadline.
    PendingRuns,
    Watches,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunsReply {
    PendingRuns(Vec<PendingRun>),
    Watches(Vec<WatchView>),
}

// ---- codecs -------------------------------------------------------------------

pub fn encode_msg(m: &RunsMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}
pub fn decode_msg(b: &[u8]) -> Result<RunsMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_query(q: &RunsQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}
pub fn decode_query(b: &[u8]) -> Result<RunsQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_reply(r: &RunsReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}
pub fn decode_reply(b: &[u8]) -> Result<RunsReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
