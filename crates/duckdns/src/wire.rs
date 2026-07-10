//! DuckDNS wire surface: one optional human name for one Identity account.
//!
//! `.duck` is Ducktape presentation syntax. It is not installed into the host
//! DNS stack and it never returns nodes, routes, endpoints, or transports.
//! Resolution stops at the stable AccountId; Identity and the existing peer /
//! reachability planes own every later step.

use serde::{Deserialize, Serialize};

pub const DUCKDNS_ZONE: &str = "duck";
/// Structural labels reserved directly below `.duck`. They are intentionally
/// non-functional until a real consumer justifies a namespace beneath them.
pub const RESERVED_ROOT_LABELS: &[&str] = &["net"];
pub const MAX_LABEL_LEN: usize = 63;
pub const MAX_QUERY_LIMIT: u64 = 256;

/// One parsed account name in Ducktape's internal `.duck` namespace.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DuckDnsName {
    /// `<handle>.duck` — an optional human alias for one stable AccountId.
    pub handle: String,
}

/// The stable result of resolving one account name. Node selection is
/// deliberately absent and must go through Identity plus peer management.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAccount {
    pub account_id: Vec<u8>,
}

/// One optional human-readable alias for an Identity account. AccountId is
/// authority; `handle` is only a mutable presentation key.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct HandleRegistration {
    pub handle: String,
    pub account_id: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DuckDnsMsg {
    /// Declaratively replace the authenticated account's optional handle.
    /// `None` unregisters it without changing Identity.
    SetHandle { handle: Option<String> },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DuckDnsQuery {
    Resolve { name: DuckDnsName },
    Registrations { from: u64, limit: u64 },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DuckDnsReply {
    Resolved(Option<ResolvedAccount>),
    Registrations(Vec<HandleRegistration>),
}

pub fn encode_msg(message: &DuckDnsMsg) -> Vec<u8> {
    serde_json::to_vec(message).expect("serializable")
}

pub fn decode_msg(bytes: &[u8]) -> Result<DuckDnsMsg, String> {
    serde_json::from_slice(bytes).map_err(|error| error.to_string())
}

pub fn encode_query(query: &DuckDnsQuery) -> Vec<u8> {
    serde_json::to_vec(query).expect("serializable")
}

pub fn decode_query(bytes: &[u8]) -> Result<DuckDnsQuery, String> {
    serde_json::from_slice(bytes).map_err(|error| error.to_string())
}

pub fn encode_reply(reply: &DuckDnsReply) -> Vec<u8> {
    serde_json::to_vec(reply).expect("serializable")
}

pub fn decode_reply(bytes: &[u8]) -> Result<DuckDnsReply, String> {
    serde_json::from_slice(bytes).map_err(|error| error.to_string())
}
