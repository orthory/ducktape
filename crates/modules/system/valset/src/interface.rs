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
    /// the retained mesh-generation window: the last few membership
    /// snapshots, keyed by generation. every node tracks this identical
    /// window on the mesh oracle, so peer-set knowledge is a function of
    /// replicated state, not of when a node joined.
    MeshWindow,
}

/// one membership generation: the full transport membership AFTER the op
/// that created it. `validators` and `residents` are strictly sorted
/// 32-byte ed25519 keys, disjoint by construction (grant refuses a current
/// validator; join promotes a resident out of its tier in the same op).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct GenerationSet {
    pub generation: u64,
    pub validators: Vec<Vec<u8>>,
    pub residents: Vec<Vec<u8>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ValsetReply {
    /// the committed validators, sorted (order-independent).
    Validators(Vec<Vec<u8>>),
    /// the committed residents, sorted (order-independent).
    Residents(Vec<Vec<u8>>),
    /// the retained generation snapshots, ascending by generation.
    MeshWindow(Vec<GenerationSet>),
}

pub fn encode_msg(m: &ValsetMsg) -> Vec<u8> {
    sdk::wire::encode(m)
}
pub fn decode_msg(b: &[u8]) -> Result<ValsetMsg, String> {
    sdk::wire::decode(b)
}
pub fn encode_query(q: &ValsetQuery) -> Vec<u8> {
    sdk::wire::encode(q)
}
pub fn decode_query(b: &[u8]) -> Result<ValsetQuery, String> {
    sdk::wire::decode(b)
}
pub fn encode_reply(r: &ValsetReply) -> Vec<u8> {
    sdk::wire::encode(r)
}
pub fn decode_reply(b: &[u8]) -> Result<ValsetReply, String> {
    sdk::wire::decode(b)
}
