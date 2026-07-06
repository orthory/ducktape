//! the vaults module's public wire surface — types only.
//!
//! a vault is a replicated store of team secrets with owner/reader
//! bookkeeping. THE TRUST MODEL, READ CAREFULLY:
//!
//! - secret VALUES are stored as OPAQUE CIPHERTEXT. clients encrypt to the
//!   vault's recipients BEFORE submitting (e.g. an x25519 envelope per
//!   reader); plaintext never enters consensus state. replicated state is
//!   readable by every validator, so the CRYPTOGRAPHIC envelope is the real
//!   read barrier — the on-chain reader list is recipient BOOKKEEPING that
//!   tells a client whom to encrypt for, not a confidentiality mechanism.
//! - the on-chain ACL is a WRITE-INTEGRITY mechanism: only owners may rotate
//!   secrets or membership, and authorship is trustworthy because the ordered
//!   lane verifies every op frame's ed25519 signature.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VaultMsg {
    /// create a vault; the (verified) submitter becomes its first owner and
    /// first reader.
    CreateVault {
        vault_id: String,
        name: String,
    },
    /// owner-gated membership bookkeeping.
    AddOwner {
        vault_id: String,
        key: Vec<u8>,
    },
    RemoveOwner {
        vault_id: String,
        key: Vec<u8>,
    },
    AddReader {
        vault_id: String,
        key: Vec<u8>,
    },
    RemoveReader {
        vault_id: String,
        key: Vec<u8>,
    },
    /// write (or rotate) a secret's ciphertext. owner-gated. `version` on the
    /// stored entry increments on every put.
    PutSecret {
        vault_id: String,
        name: String,
        ciphertext: Vec<u8>,
    },
    /// remove a secret. owner-gated.
    DeleteSecret {
        vault_id: String,
        name: String,
    },
}

/// one secret's stored envelope (ciphertext + audit metadata).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SecretEntry {
    pub ciphertext: Vec<u8>,
    pub version: u64,
    pub created_at: u64,
    pub updated_at: u64,
}

/// a vault's readable projection (metadata + secret NAMES, not values).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct VaultView {
    pub vault_id: String,
    pub name: String,
    pub created_at: u64,
    pub owners: Vec<Vec<u8>>,
    pub readers: Vec<Vec<u8>>,
    pub secret_names: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VaultQuery {
    /// every vault's metadata view.
    Vaults,
    /// one vault's metadata view.
    Vault { vault_id: String },
    /// one secret's stored envelope.
    Secret { vault_id: String, name: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VaultReply {
    Vaults(Vec<VaultView>),
    Vault(Option<VaultView>),
    Secret(Option<SecretEntry>),
}

pub fn encode_msg(m: &VaultMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}
pub fn decode_msg(b: &[u8]) -> Result<VaultMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_query(q: &VaultQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}
pub fn decode_query(b: &[u8]) -> Result<VaultQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_reply(r: &VaultReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}
pub fn decode_reply(b: &[u8]) -> Result<VaultReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
