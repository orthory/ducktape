//! DuckDNS wire surface — verified names and provider identities only.
//!
//! `.duck` is Ducktape's internal presentation syntax. It is deliberately not
//! installed into the host DNS stack and never resolves to an IP address here;
//! callers resolve a name to stable account/service identities and eligible
//! node keys, then use reachability and a purpose-specific data plane.

use serde::{Deserialize, Serialize};

pub const DUCKDNS_ZONE: &str = "duck";
/// Labels directly below `.duck` that route structural namespaces instead of
/// account handles. Future structural roots must be reserved before use.
pub const RESERVED_ROOT_LABELS: &[&str] = &["net"];
pub const MAX_LABEL_LEN: usize = 63;
pub const NODE_LABEL_HEX_LEN: usize = 12;
pub const NODE_KEY_LEN: usize = 32;
pub const MAX_ANNOUNCEMENTS_PER_NODE: usize = 128;
pub const MAX_QUERY_LIMIT: u64 = 256;

/// One parsed name in the active Ducktape workspace.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DuckDnsName {
    /// `<handle>.duck` — a human name for one stable account.
    Account { handle: String },
    /// `<service>.<handle>.duck` — account-authorized service discovery.
    AccountService { service: String, handle: String },
    /// `<service>.<chain>.net.duck` — a network-wide provider pool.
    NetworkService { service: String, chain: String },
    /// `<service>.<node>.<chain>.net.duck` — one pinned provider.
    NodeService {
        service: String,
        node: String,
        chain: String,
    },
}

/// Consensus-visible authority scope of a service declaration.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ServiceScope {
    /// The authenticated submitting node's current account owns the service.
    /// A `.duck` handle is an optional lookup alias, not declaration authority.
    Account,
    /// Any validator or admitted resident may join this provider pool.
    Network,
}

/// One node's replicated discovery declaration. Endpoints, ports, health, and
/// transport configuration are intentionally absent.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ServiceAnnouncement {
    pub scope: ServiceScope,
    pub service: String,
}

/// Stable logical service identity. A connection protocol binds its own intent
/// to this identity; DuckDNS does not prescribe a transport.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ServiceIdentity {
    pub scope: ServiceScope,
    pub service: String,
}

/// One eligible node identity, in deterministic full-key order. `node_label`
/// is display/routing syntax only; callers authenticate the full `node` key.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ResolvedNode {
    pub node: Vec<u8>,
    pub node_label: String,
}

/// A human name resolved to its stable account plus currently eligible nodes.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAccount {
    pub account_id: Vec<u8>,
    pub nodes: Vec<ResolvedNode>,
}

/// One optional human-readable alias for an identity account. The account id
/// remains the stable authority; `handle` is only a mutable presentation key.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct HandleRegistration {
    pub handle: String,
    pub account_id: Vec<u8>,
}

/// One node's committed service declaration plus the account authority captured
/// for account-scoped entries. `account_id` is absent for network-only entries.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct NodeRegistration {
    pub account_id: Option<Vec<u8>>,
    pub announcements: Vec<ServiceAnnouncement>,
}

/// Stable authority behind a logical service name. Account-scoped resolution
/// carries the AccountId so a mutable handle is never used as authentication.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceAuthority {
    Account { account_id: Vec<u8> },
    Network,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ResolvedService {
    pub identity: ServiceIdentity,
    pub authority: ServiceAuthority,
    pub providers: Vec<ResolvedNode>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResolvedName {
    Account(ResolvedAccount),
    Service(ResolvedService),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DuckDnsMsg {
    /// Declaratively replace the authenticated account's optional `.duck`
    /// handle. `None` unregisters it without touching service declarations.
    SetHandle { handle: Option<String> },
    /// Full declarative replacement for the authenticated submitting node.
    ReplaceAnnouncements {
        announcements: Vec<ServiceAnnouncement>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DuckDnsQuery {
    Resolve { name: DuckDnsName },
    Registrations { from: u64, limit: u64 },
    NodeRegistration { node: Vec<u8> },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DuckDnsReply {
    Resolved(Option<ResolvedName>),
    Registrations(Vec<HandleRegistration>),
    NodeRegistration {
        registration: Option<NodeRegistration>,
        /// Whether a captured account authority still matches Identity.
        /// `false` forces the declarative announcer to replace even an
        /// otherwise identical (or newly empty) declaration set.
        authority_current: bool,
    },
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
