//! the valset module's public wire surface — types only.
//!
//! valset is the ed25519 membership registry as replicated state: VALIDATORS
//! (the consensus quorum) via [`ValsetMsg::Join`] / [`ValsetMsg::Leave`], and
//! RESIDENTS (mesh + statesync standing, NO consensus participation) via
//! [`ValsetMsg::Grant`] / [`ValsetMsg::Revoke`] — the staged-admission tier a
//! joiner syncs in before promotion. reads go via [`ValsetQuery`] ->
//! [`ValsetReply`]. each `key` is a 32-byte ed25519 public key encoding (the
//! impl crate validates the curve point; this crate stays types-only).

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ValsetMsg {
    /// add a validator. `key` MUST be a 32-byte ed25519 public key; the impl
    /// rejects a malformed key with `Error::Module`. a key holding resident
    /// standing is PROMOTED: the same op removes it from the resident set —
    /// one boundary carries the whole transition.
    Join { key: Vec<u8> },
    /// remove a validator by key. a no-op if the key is not in the set.
    Leave { key: Vec<u8> },
    /// grant RESIDENT standing: mesh + statesync access, no quorum seat.
    Grant { key: Vec<u8> },
    /// revoke resident standing by key. a no-op if the key is not a
    /// resident.
    Revoke { key: Vec<u8> },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ValsetQuery {
    /// the full committed validator set.
    Validators,
    /// the full committed resident set.
    Residents,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ValsetReply {
    /// the committed validators, sorted (order-independent).
    Validators(Vec<Vec<u8>>),
    /// the committed residents, sorted (order-independent).
    Residents(Vec<Vec<u8>>),
}

pub fn encode_msg(m: &ValsetMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}
pub fn decode_msg(b: &[u8]) -> Result<ValsetMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_query(q: &ValsetQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}
pub fn decode_query(b: &[u8]) -> Result<ValsetQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_reply(r: &ValsetReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}
pub fn decode_reply(b: &[u8]) -> Result<ValsetReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
