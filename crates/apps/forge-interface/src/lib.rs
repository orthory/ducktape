//! the forge module's public wire surface — types only.
//!
//! forge is a git-backed module: its state is a real git repo, its `root()` is
//! `sha256` of the repo's HEAD commit oid. writes go via [`ForgeMsg`] (a file
//! put + commit); reads via [`ForgeQuery`] -> [`ForgeReply`], returning the HEAD
//! oid as hex.

use serde::{Deserialize, Serialize};

/// a write intent at forge: either the file-by-file [`ForgeMsg::Commit`] (forge
/// builds the commit object itself) or [`ForgeMsg::Push`] — a git-faithful ref
/// update that adopts a client's REAL commit history by oid, with the objects
/// carried out-of-band in a node-local packfile (never in consensus).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ForgeMsg {
    Commit {
        path: String,
        content: String,
        message: String,
    },
    /// a git ref update over consensus. the ONLY consensus effect is a
    /// compare-and-swap on the committed HEAD: forge's current HEAD must equal
    /// `prev_oid`, and on match HEAD becomes `new_oid` (so `root()` becomes
    /// `sha256(new_oid)` on EVERY validator, pack-holder or not). the git
    /// objects themselves are node-local — fetched from the files blob store by
    /// `pack_digest` and installed lazily — and NEVER influence root/accept.
    Push {
        /// the CAS guard: forge's committed HEAD must equal this or the push is
        /// rejected (non-fast-forward). `None` == the repo is unborn (pushing to
        /// an empty remote). 20 raw sha1 bytes when `Some`.
        prev_oid: Option<Vec<u8>>,
        /// the new committed HEAD after the push. 20 raw sha1 bytes.
        new_oid: Vec<u8>,
        /// sha256 digest of the packfile (full object closure of `new_oid`) in
        /// the node's files blob store. objects are NODE-LOCAL, never consensus
        /// state; this 32-byte locator has ZERO effect on root/accept-reject.
        pack_digest: Vec<u8>,
    },
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

pub fn encode_msg(m: &ForgeMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}
pub fn decode_msg(b: &[u8]) -> Result<ForgeMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_query(q: &ForgeQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}
pub fn decode_query(b: &[u8]) -> Result<ForgeQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_reply(r: &ForgeReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}
pub fn decode_reply(b: &[u8]) -> Result<ForgeReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
