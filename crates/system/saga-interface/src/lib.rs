//! the saga module's public wire surface — types only.
//!
//! two op shapes cross this surface, both as [`SagaMsg`]:
//! - `Trigger` STARTS a saga (external op). the module records a pending saga and
//!   emits a [`WorkerRequest`] effect asking the host to run the async work.
//! - `OracleResult` COMPLETES it — submitted by a host-owned worker as a NORMAL
//!   op (the oracle-as-transaction), so every validator applies the AGREED result
//!   through the same ordered path. the module never mutates saga state from the
//!   worker directly; the result re-enters as an op.
//!
//! reads go via [`SagaQuery`] -> [`SagaReply`].

use serde::{Deserialize, Serialize};

/// a saga's stable id, chosen by the trigger.
pub type SagaId = String;

/// ops targeting the saga module.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum SagaMsg {
    /// external: start a saga running the async work described by `spec`.
    Trigger { saga_id: SagaId, spec: Vec<u8> },
    /// worker completion, submitted as an op — advances the saga to Done.
    OracleResult { saga_id: SagaId, result: Vec<u8> },
}

/// the payload of the [`sdk::Effect`] a Trigger emits: the host-owned worker's
/// work order. carries everything the worker needs to compute a result and submit
/// the matching `OracleResult` op back through the normal path.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct WorkerRequest {
    pub saga_id: SagaId,
    pub spec: Vec<u8>,
}

/// where a saga is in its (deterministic) lifecycle. `Pending` until the agreed
/// `OracleResult` op resolves it to `Done` — the tracker only advances via ordered
/// ops, never node-local.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SagaStatus {
    Pending,
    Done,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum SagaQuery {
    Get { saga_id: SagaId },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum SagaReply {
    Saga(Option<SagaView>),
}

/// a saga's observable state — the read projection.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SagaView {
    pub step: u32,
    pub status: SagaStatus,
    pub result: Option<Vec<u8>>,
}

pub fn encode_msg(m: &SagaMsg) -> Vec<u8> { serde_json::to_vec(m).expect("serializable") }
pub fn decode_msg(b: &[u8]) -> Result<SagaMsg, String> { serde_json::from_slice(b).map_err(|e| e.to_string()) }
pub fn encode_worker_request(w: &WorkerRequest) -> Vec<u8> { serde_json::to_vec(w).expect("serializable") }
pub fn decode_worker_request(b: &[u8]) -> Result<WorkerRequest, String> { serde_json::from_slice(b).map_err(|e| e.to_string()) }
pub fn encode_query(q: &SagaQuery) -> Vec<u8> { serde_json::to_vec(q).expect("serializable") }
pub fn decode_query(b: &[u8]) -> Result<SagaQuery, String> { serde_json::from_slice(b).map_err(|e| e.to_string()) }
pub fn encode_reply(r: &SagaReply) -> Vec<u8> { serde_json::to_vec(r).expect("serializable") }
pub fn decode_reply(b: &[u8]) -> Result<SagaReply, String> { serde_json::from_slice(b).map_err(|e| e.to_string()) }
