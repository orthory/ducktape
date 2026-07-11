//! the capability module's public wire surface — types only.
//!
//! the capability module is the network-wide registry of what each node's
//! host can execute ("codex", "claude", ...): node key -> announced tag set,
//! replicated in consensus state so every node holds an identical view of
//! who provides what. announcements are declarative and self-scoped: a node
//! states its OWN full set (identity comes from the verified submit origin,
//! never the payload), and an empty set removes the node from the registry.
//! tags are open-set strings so a new kind of executor is data, not code —
//! the impl crate validates shape (charset/length/count), not meaning.

use serde::{Deserialize, Serialize};

/// longest single capability tag, in bytes — a wire-format constant shared by
/// everything that mints or checks a tag (the registry's Announce validation,
/// host spec parsing, agent registration).
pub const MAX_TAG_LEN: usize = 64;

/// the ONE definition of a well-formed capability tag: non-empty, at most
/// [`MAX_TAG_LEN`] bytes, charset `[a-z0-9._-]`. every layer that accepts a
/// tag validates through this — a tag that passes here can never bounce off
/// another layer's copy of the rule, because there are no copies.
pub fn validate_tag(tag: &str) -> Result<(), String> {
    if tag.is_empty() {
        return Err("capability tag must be non-empty".into());
    }
    if tag.len() > MAX_TAG_LEN {
        return Err(format!(
            "capability tag exceeds {MAX_TAG_LEN} bytes: {} bytes",
            tag.len()
        ));
    }
    if !tag
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
    {
        return Err(format!(
            "capability tag has invalid characters (want [a-z0-9._-]): {tag:?}"
        ));
    }
    Ok(())
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityMsg {
    /// declaratively replace the submitter's announced capability set. the
    /// announced node is the SUBMIT ORIGIN's external key — a node can only
    /// speak for itself. announcing an empty set removes the node. re-sending
    /// the current set is an idempotent no-op state-wise, so hosts may
    /// re-derive and re-announce freely (restart, rediscovery).
    Announce { capabilities: Vec<String> },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityQuery {
    /// every node that announced `capability`, sorted by key.
    Providers { capability: String },
    /// the capability set a single node announced (empty if absent).
    Node { node: Vec<u8> },
    /// the full registry, sorted by node key. no Rust constructor exists —
    /// this is the desktop app's registry-enumeration read (the executor
    /// picker and per-member capability display), submitted as wire JSON.
    All,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityReply {
    Providers(Vec<Vec<u8>>),
    Node(Vec<String>),
    All(Vec<(Vec<u8>, Vec<String>)>),
}

pub fn encode_msg(m: &CapabilityMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}
pub fn decode_msg(b: &[u8]) -> Result<CapabilityMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_query(q: &CapabilityQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}
pub fn decode_query(b: &[u8]) -> Result<CapabilityQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_reply(r: &CapabilityReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}
pub fn decode_reply(b: &[u8]) -> Result<CapabilityReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
