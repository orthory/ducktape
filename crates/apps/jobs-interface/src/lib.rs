//! the jobs module's public wire surface -- types only.
//!
//! `jobs` is a consensus-native work board: a submitter posts a job, any worker
//! claims it, exactly one claim wins by consensus order, the claimant processes
//! off-platform and reports a result. writes go via [`JobsMsg`]; reads via
//! [`JobsQuery`] -> [`JobsReply`]. identities (`Job::submitter`, `Claim::worker`)
//! are ALWAYS derived by the module from the dispatch origin, never carried on
//! the wire -- so they cannot be spoofed by a caller.

use serde::{Deserialize, Serialize};

/// the lifecycle of a job. `Done`, `Failed`, and `Cancelled` are terminal.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    Pending,
    Processing,
    Done,
    Failed,
    Cancelled,
}

impl JobStatus {
    /// terminal states never transition again (only `Prune` removes them).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            JobStatus::Done | JobStatus::Failed | JobStatus::Cancelled
        )
    }
}

/// the winning claim on a job. `worker` is origin-derived, never caller-supplied.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    pub worker: String,
    pub claimed_at_height: u64,
    pub lease_views: u64,
}

/// the claimant's reported outcome, stored once (result singularity).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct JobResult {
    pub ok: bool,
    pub payload: String,
}

/// a single work item on the board.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub job_id: String,
    pub kind: String,
    pub spec: String,
    /// origin-derived submitter identity (module id, lowercase-hex external key,
    /// or "system"). set by the module, never carried on the wire.
    pub submitter: String,
    pub status: JobStatus,
    /// total number of successful claims over this job's life.
    pub attempt: u64,
    pub claim: Option<Claim>,
    pub result: Option<JobResult>,
    pub created_at_height: u64,
    pub updated_at_height: u64,
}

/// write intents against the board. all identity fields are derived from the
/// dispatch origin inside the module -- none appear here.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum JobsMsg {
    /// post a new job (status `Pending`, attempt 0).
    Submit {
        job_id: String,
        kind: String,
        spec: String,
    },
    /// claim a `Pending` job; a claim on a non-pending job is rejected -- the
    /// consensus order already picked the winner.
    Claim { job_id: String, lease_views: u64 },
    /// the current claimant reports a result on a `Processing` job.
    Finalize {
        job_id: String,
        ok: bool,
        payload: String,
    },
    /// the current claimant hands a `Processing` job back to `Pending`.
    Release { job_id: String },
    /// permissionless requeue of a `Processing` job whose lease has expired.
    Reclaim { job_id: String },
    /// the submitter cancels a still-`Pending` job.
    Cancel { job_id: String },
    /// the submitter removes a terminal job's record entirely.
    Prune { job_id: String },
}

/// read projections over the board.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum JobsQuery {
    Get {
        job_id: String,
    },
    List {
        status: Option<JobStatus>,
        kind_prefix: String,
        limit: u64,
    },
    Counts {},
}

/// a per-status census of the board.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct BoardCounts {
    pub pending: u64,
    pub processing: u64,
    pub done: u64,
    pub failed: u64,
    pub cancelled: u64,
}

/// replies to [`JobsQuery`].
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum JobsReply {
    Job(Option<Job>),
    Jobs(Vec<Job>),
    Counts(BoardCounts),
}

pub fn encode_msg(m: &JobsMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}

pub fn decode_msg(b: &[u8]) -> Result<JobsMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

pub fn encode_query(q: &JobsQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}

pub fn decode_query(b: &[u8]) -> Result<JobsQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

pub fn encode_reply(r: &JobsReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}

pub fn decode_reply(b: &[u8]) -> Result<JobsReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
