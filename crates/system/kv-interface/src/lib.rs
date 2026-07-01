//! the kv module's public wire surface — types only, no logic, no sdk dep.
//! a module that wants to write kv depends on THIS, never on the kv impl.

use serde::{Deserialize, Serialize};

/// messages the kv module accepts (its `execute` payload).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum KvMsg {
    Set { key: Vec<u8>, value: Vec<u8> },
}

pub fn encode(m: &KvMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("KvMsg is always serializable")
}

pub fn decode(bytes: &[u8]) -> Result<KvMsg, String> {
    serde_json::from_slice(bytes).map_err(|e| e.to_string())
}

/// read requests the kv module serves via `Module::query`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum KvQuery {
    Get { key: Vec<u8> },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum KvReply {
    Value(Option<Vec<u8>>),
}

pub fn encode_query(q: &KvQuery) -> Vec<u8> { serde_json::to_vec(q).expect("serializable") }
pub fn decode_query(b: &[u8]) -> Result<KvQuery, String> { serde_json::from_slice(b).map_err(|e| e.to_string()) }
pub fn encode_reply(r: &KvReply) -> Vec<u8> { serde_json::to_vec(r).expect("serializable") }
pub fn decode_reply(b: &[u8]) -> Result<KvReply, String> { serde_json::from_slice(b).map_err(|e| e.to_string()) }
