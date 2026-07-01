//! the forge module's public wire surface — types only.
//!
//! forge is a git-backed module: its state is a real git repo, its `root()` is
//! `sha256` of the repo's HEAD commit oid. writes go via [`ForgeMsg`] (a file
//! put + commit); reads via [`ForgeQuery`] -> [`ForgeReply`], returning the HEAD
//! oid as hex.

use serde::{Deserialize, Serialize};

/// a write intent at forge: put `content` at `path` in the repo and commit it.
/// one op == one commit, so the HEAD (and thus `root()`) advances per message.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ForgeMsg {
    Commit { path: String, content: String, message: String },
}

/// reads: the current canonical head of the repo.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ForgeQuery {
    Head,
}

/// the git oid hex of HEAD (a 40-char sha1 oid), or `None` on an unborn repo (no
/// commits yet). forge's `root()` is `sha256` of the oid's 20 raw bytes, so this
/// hex is the state root's PREIMAGE: a consumer can git-address the exact commit
/// forge committed while the app-hash keeps sha256-strength.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ForgeReply {
    Head(Option<String>),
}

pub fn encode_msg(m: &ForgeMsg) -> Vec<u8> { serde_json::to_vec(m).expect("serializable") }
pub fn decode_msg(b: &[u8]) -> Result<ForgeMsg, String> { serde_json::from_slice(b).map_err(|e| e.to_string()) }
pub fn encode_query(q: &ForgeQuery) -> Vec<u8> { serde_json::to_vec(q).expect("serializable") }
pub fn decode_query(b: &[u8]) -> Result<ForgeQuery, String> { serde_json::from_slice(b).map_err(|e| e.to_string()) }
pub fn encode_reply(r: &ForgeReply) -> Vec<u8> { serde_json::to_vec(r).expect("serializable") }
pub fn decode_reply(b: &[u8]) -> Result<ForgeReply, String> { serde_json::from_slice(b).map_err(|e| e.to_string()) }
