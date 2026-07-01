//! the directory module's public wire surface — types only.
//! writes go via [`DirMsg`]; reads via [`DirQuery`] → [`DirReply`].

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum DirMsg {
    Set { key: String, value: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum DirQuery {
    Get { key: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum DirReply {
    Value(Option<String>),
}

pub fn encode_msg(m: &DirMsg) -> Vec<u8> { serde_json::to_vec(m).expect("serializable") }
pub fn decode_msg(b: &[u8]) -> Result<DirMsg, String> { serde_json::from_slice(b).map_err(|e| e.to_string()) }
pub fn encode_query(q: &DirQuery) -> Vec<u8> { serde_json::to_vec(q).expect("serializable") }
pub fn decode_query(b: &[u8]) -> Result<DirQuery, String> { serde_json::from_slice(b).map_err(|e| e.to_string()) }
pub fn encode_reply(r: &DirReply) -> Vec<u8> { serde_json::to_vec(r).expect("serializable") }
pub fn decode_reply(b: &[u8]) -> Result<DirReply, String> { serde_json::from_slice(b).map_err(|e| e.to_string()) }
