//! Wire types for the experimental QMDB-backed EVM.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvmTx {
    Create {
        init_code: Vec<u8>,
        gas_limit: u64,
    },
    Call {
        to: [u8; 20],
        input: Vec<u8>,
        gas_limit: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvmMsg {
    Execute(EvmTx),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvmQuery {
    Simulate(EvmTx),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvmStatus {
    Success,
    Revert,
    Halt { reason: String },
}

/// The receipt emitted by `Execute` and returned by `Simulate`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvmResult {
    pub status: EvmStatus,
    pub gas_used: u64,
    pub output: Vec<u8>,
    pub created_address: Option<[u8; 20]>,
    pub logs: Vec<EvmLog>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvmLog {
    pub address: [u8; 20],
    pub topics: Vec<[u8; 32]>,
    pub data: Vec<u8>,
}

pub fn encode_msg(msg: &EvmMsg) -> Vec<u8> {
    serde_json::to_vec(msg).expect("EvmMsg is always serializable")
}

pub fn decode_msg(bytes: &[u8]) -> Result<EvmMsg, String> {
    serde_json::from_slice(bytes).map_err(|e| e.to_string())
}

pub fn encode_query(query: &EvmQuery) -> Vec<u8> {
    serde_json::to_vec(query).expect("EvmQuery is always serializable")
}

pub fn decode_query(bytes: &[u8]) -> Result<EvmQuery, String> {
    serde_json::from_slice(bytes).map_err(|e| e.to_string())
}

pub fn encode_result(result: &EvmResult) -> Vec<u8> {
    serde_json::to_vec(result).expect("EvmResult is always serializable")
}

pub fn decode_result(bytes: &[u8]) -> Result<EvmResult, String> {
    serde_json::from_slice(bytes).map_err(|e| e.to_string())
}
