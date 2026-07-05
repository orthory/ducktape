//! the valset module's public wire surface — types only.
//!
//! valset is the ed25519 validator set as replicated state, split into two
//! classes: **active** validators (the consensus quorum) and **standby**
//! validators (registered by governance, tracked on the transport mesh, not
//! yet counted for quorum). [`ValsetMsg::Join`] registers a key as standby;
//! [`ValsetMsg::Online`] — carrying the key's own proof of possession —
//! moves it standby -> active once the node is genuinely up;
//! [`ValsetMsg::Leave`] removes a key from either class. reads go via
//! [`ValsetQuery`] -> [`ValsetReply`]. each `key` is a 32-byte ed25519
//! public key encoding (the impl crate validates the curve point; this
//! crate stays types-only).

use serde::{Deserialize, Serialize};

/// the signing domain for [`ValsetMsg::Online`] proofs: the standby key
/// signs `key || signed_height (u64 le)` under this namespace.
pub const ONLINE_PROOF_NS: &[u8] = b"ducktape:valset-online:v1";

/// how many blocks an online proof stays valid past its `signed_height` —
/// long enough for lobby relay + inclusion, short enough that a proof from
/// a previous standby term cannot be replayed much later.
pub const ONLINE_PROOF_TTL_BLOCKS: u64 = 1800;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ValsetMsg {
    /// register a validator as STANDBY. `key` MUST be a 32-byte ed25519
    /// public key; the impl rejects a malformed key with `Error::Module`.
    /// a key that is already active or standby is left as-is.
    Join { key: Vec<u8> },
    /// remove a validator by key, active or standby. a no-op if the key is
    /// in neither set.
    Leave { key: Vec<u8> },
    /// move a STANDBY key to ACTIVE at the next cutover. `signature` is the
    /// standby key's own ed25519 signature over `key || signed_height` under
    /// [`ONLINE_PROOF_NS`] — proof of possession, so a relaying member
    /// cannot activate a node that never announced. valid while
    /// `signed_height <= height <= signed_height + ONLINE_PROOF_TTL_BLOCKS`.
    Online {
        key: Vec<u8>,
        signed_height: u64,
        signature: Vec<u8>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ValsetQuery {
    /// the committed ACTIVE set — the consensus-quorum projection.
    Validators,
    /// the full committed membership picture, active and standby.
    Members,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ValsetReply {
    /// the committed active validators, sorted (order-independent).
    Validators(Vec<Vec<u8>>),
    /// the full membership: both lists sorted.
    Members {
        active: Vec<Vec<u8>>,
        standby: Vec<Vec<u8>>,
    },
}

/// the exact byte stream an online proof signs: `key || signed_height` little
/// endian — shared by the announcing node and the module's verifier so the
/// two can never drift.
pub fn online_proof_message(key: &[u8], signed_height: u64) -> Vec<u8> {
    let mut msg = Vec::with_capacity(key.len() + 8);
    msg.extend_from_slice(key);
    msg.extend_from_slice(&signed_height.to_le_bytes());
    msg
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
