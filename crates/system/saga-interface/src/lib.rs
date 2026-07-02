//! the saga module's public wire surface — types only.
//!
//! saga v2 is the deterministic async-RPC ledger: one effect, one agreed
//! result, with attempts, leases, deadlines, and a requester callback. five op
//! shapes cross this surface, all as [`SagaMsg`]:
//!
//! - `Trigger` STARTS a saga. the module records a pending saga and emits a
//!   [`WorkerRequest`] effect asking the host-owned worker to run the async
//!   work. a duplicate `saga_id` is a deterministic no-op.
//! - `OracleResult` COMPLETES an attempt — submitted by a host-owned worker as
//!   a NORMAL op (the oracle-as-transaction), so every validator applies the
//!   AGREED result through the same ordered path. the `(saga_id, attempt)`
//!   pair is the idempotency key: exactly one result transitions a given
//!   attempt; duplicates and stale attempts are deterministic no-ops.
//! - `Crank` is the permissionless liveness op: it deterministically fires
//!   past-deadline timeouts and expires stale leases (retry or fail). anyone
//!   may crank; safety never depends on who does.
//! - `Cancel` terminates a pending saga — gated to the trigger origin.
//! - `Prune` removes TERMINAL sagas — gated to the trigger origin per id. GC
//!   is explicit; there is no lazy retention sweep.
//!
//! every terminal transition with a `reply_to` emits a [`SagaCallback`] msg to
//! the requester in the SAME block — requesters depend only on this crate to
//! decode it. reads go via [`SagaQuery`] -> [`SagaReply`].

use serde::{Deserialize, Serialize};

/// a saga's stable id, chosen by the trigger.
pub type SagaId = String;

/// hard cap on an accepted oracle result's byte length — a consensus constant,
/// enforced at execute time so an oversized result can never commit into the
/// root preimage (and, from there, joiner snapshots). a finalized op carrying
/// a larger `Ok` payload ABORTS its block instead of landing.
pub const MAX_RESULT_BYTES: usize = 256 * 1024;

/// hard cap on a trigger's work spec — the same commit-into-the-root-preimage
/// class as [`MAX_RESULT_BYTES`]: the spec is stored on the saga AND re-emitted
/// inside every retry's `WorkerRequest`. enforced at trigger time.
pub const MAX_SPEC_BYTES: usize = 256 * 1024;

/// hard cap on a trigger's `reply_payload` — stored on the saga and echoed in
/// the terminal callback. enforced at trigger time.
pub const MAX_REPLY_PAYLOAD_BYTES: usize = 64 * 1024;

/// hard cap on an oracle `Err` string — the `Failed` arm stores it in the root
/// preimage and echoes it in the callback. enforced at execute time; a larger
/// error ABORTS its block, exactly like an oversized `Ok` result.
pub const MAX_ERROR_BYTES: usize = 16 * 1024;

/// the canonical, serializable, orderable mirror of `sdk::Origin`, recorded on
/// every saga at trigger time. it gates `Cancel` and `Prune` (only the
/// recorded trigger origin may act) and rides in the committed encoding —
/// `sdk::Origin` itself is neither `Ord` nor serializable, so this type is the
/// wire/state form.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum SagaOrigin {
    /// an external submitter, identified by (e.g.) an ed25519 id.
    External(Vec<u8>),
    /// a module that triggered the saga as a follow-up.
    Module(String),
    /// genesis / system-internal.
    System,
}

/// ops targeting the saga module.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum SagaMsg {
    /// start a saga running the async work described by `spec`. a duplicate
    /// `saga_id` (staged or committed) is a deterministic no-op. `reply_to`
    /// must name a registered module (validated at trigger time — the
    /// callback-poison rule) and `max_attempts` must be >= 1.
    Trigger {
        saga_id: SagaId,
        /// opaque work spec (e.g. an LlmRequest), echoed to the worker.
        spec: Vec<u8>,
        /// callback target: on EVERY terminal transition the module sends
        /// this module a [`SagaCallback`] in the same block. `None` = fire
        /// and forget.
        reply_to: Option<String>,
        /// opaque requester correlation bytes, echoed back in the callback.
        reply_payload: Vec<u8>,
        /// absolute view by which the WHOLE saga must finish; a crank at or
        /// past it transitions the saga to `TimedOut`.
        deadline: Option<u64>,
        /// total attempts allowed (>= 1); an `Err` outcome or an expired
        /// lease consumes one.
        max_attempts: u32,
        /// lease window in views for each attempt. `None` defaults to
        /// `DEFAULT_LEASE_VIEWS` when an assignee exists, else no lease.
        lease_views: Option<u64>,
    },
    /// worker completion for one attempt, submitted as an op. `outcome` is
    /// the agreed result (`Ok`, capped at [`MAX_RESULT_BYTES`]) or the
    /// attempt's failure (`Err`, which retries while attempts remain).
    OracleResult {
        saga_id: SagaId,
        /// the attempt this result answers — echoed from the
        /// [`WorkerRequest`]; a stale attempt is a deterministic no-op.
        attempt: u32,
        outcome: Result<Vec<u8>, String>,
    },
    /// permissionless deterministic sweep: fire past-deadline timeouts and
    /// expire stale leases (retry or fail), bounded per op.
    Crank {},
    /// terminate a pending saga — only the recorded trigger origin; anything
    /// else is a deterministic no-op.
    Cancel { saga_id: SagaId },
    /// remove TERMINAL sagas — only the recorded trigger origin per id;
    /// non-terminal, foreign, and unknown ids are skipped as no-ops.
    Prune { saga_ids: Vec<SagaId> },
}

/// the payload of the [`sdk::Effect`] a trigger (or retry) emits: the
/// host-owned worker's work order. `(saga_id, attempt)` is the idempotency key
/// the worker echoes back in its `OracleResult`; `assignee` is the lease
/// holder that should execute (advisory under the open policy, enforced under
/// strict).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct WorkerRequest {
    pub saga_id: SagaId,
    pub attempt: u32,
    pub spec: Vec<u8>,
    pub deadline: Option<u64>,
    pub assignee: Option<Vec<u8>>,
}

/// where a saga is in its (deterministic) lifecycle. `Pending` until an
/// ordered op resolves it into one of the four terminal states — the ledger
/// only advances via ordered ops, never node-local.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SagaStatus {
    Pending,
    Done,
    Failed,
    TimedOut,
    Cancelled,
}

impl SagaStatus {
    /// true for every state a saga can never leave.
    pub fn is_terminal(&self) -> bool {
        !matches!(self, SagaStatus::Pending)
    }
}

/// how a saga ended — the callback's verdict, mirroring the terminal
/// [`SagaStatus`] and carrying its payload.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum SagaOutcome {
    /// the agreed oracle result.
    Done(Vec<u8>),
    /// the final attempt's error, or "lease attempts exhausted".
    Failed(String),
    /// the whole-saga deadline passed before a result landed.
    TimedOut,
    /// the trigger origin cancelled the saga.
    Cancelled,
}

/// the callback msg payload a requester receives on EVERY terminal transition
/// (when the trigger named a `reply_to`), in the SAME block as the op that
/// caused it. `payload` echoes the trigger's `reply_payload` so the requester
/// can correlate without keeping a saga_id index.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SagaCallback {
    pub saga_id: SagaId,
    pub payload: Vec<u8>,
    pub outcome: SagaOutcome,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum SagaQuery {
    Get { saga_id: SagaId },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum SagaReply {
    Saga(Option<SagaView>),
}

/// a saga's observable state — the full read projection.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SagaView {
    pub origin: SagaOrigin,
    pub reply_to: Option<String>,
    pub reply_payload: Vec<u8>,
    pub spec: Vec<u8>,
    pub status: SagaStatus,
    pub attempt: u32,
    pub max_attempts: u32,
    pub assignee: Option<Vec<u8>>,
    pub lease_views: Option<u64>,
    pub lease_expires_at: Option<u64>,
    pub deadline: Option<u64>,
    pub result: Option<Vec<u8>>,
    pub error: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
}

pub fn encode_msg(m: &SagaMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}
pub fn decode_msg(b: &[u8]) -> Result<SagaMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_worker_request(w: &WorkerRequest) -> Vec<u8> {
    serde_json::to_vec(w).expect("serializable")
}
pub fn decode_worker_request(b: &[u8]) -> Result<WorkerRequest, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_callback(c: &SagaCallback) -> Vec<u8> {
    serde_json::to_vec(c).expect("serializable")
}
pub fn decode_callback(b: &[u8]) -> Result<SagaCallback, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_query(q: &SagaQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}
pub fn decode_query(b: &[u8]) -> Result<SagaQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_reply(r: &SagaReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}
pub fn decode_reply(b: &[u8]) -> Result<SagaReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
