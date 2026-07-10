//! DuckDNS wire surface — types only. Writes go via [`DuckDnsMsg`]; reads via
//! [`DuckDnsQuery`] -> [`DuckDnsReply`]. Local loopback targets are deliberately
//! absent because they are node configuration, never replicated state.
//!
//! `.quack` is intentionally Ducktape's sole private suffix. It is not reserved
//! by ICANN, so a future public delegation could collide with these device-local
//! names. The helper must use split DNS for exactly [`DUCKDNS_ZONE`] and never
//! forward this zone publicly.

use serde::{Deserialize, Serialize};

pub const DUCKDNS_ZONE: &str = "ducktape.quack";
pub const MAX_LABEL_LEN: usize = 63;
pub const NODE_LABEL_HEX_LEN: usize = 12;
pub const NODE_KEY_LEN: usize = 32;
pub const MAX_ANNOUNCEMENTS_PER_NODE: usize = 128;
/// Data-plane hello intent for an HTTP/WebSocket publication stream.
pub const WEB_STREAM_INTENT: u8 = 1;

/// One parsed name in the active Ducktape workspace.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DuckDnsName {
    /// `<handle>.ducktape.quack` — the handle's default homepage.
    User { handle: String },
    /// `<service>.<handle>.ducktape.quack`.
    UserService { service: String, handle: String },
    /// `<service>.<chain>.net.ducktape.quack`.
    NetworkService { service: String, chain: String },
    /// `<service>.<node>.<chain>.net.ducktape.quack`.
    NodeService {
        service: String,
        node: String,
        chain: String,
    },
}

/// Consensus-visible ownership scope of a web publication.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ServiceScope {
    /// Only a node bound to the account owning `handle` may publish it.
    User { handle: String },
    /// Any validator or admitted resident may join this provider pool.
    Network,
}

/// One provider's replicated declaration. No address or port is present.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ServiceAnnouncement {
    pub scope: ServiceScope,
    pub service: String,
    /// Also answer this service at the bare user hostname. Only one distinct
    /// service may be the default for a handle.
    pub default_homepage: bool,
    /// Opt out of the local gateway's unsafe cross-site request rejection.
    pub allow_cross_site: bool,
}

/// Stable service identity carried in the authenticated overlay stream hello.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ServiceIdentity {
    pub scope: ServiceScope,
    pub service: String,
}

/// One provider, in deterministic node-key order.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ServiceProvider {
    pub node: Vec<u8>,
    pub node_label: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ResolvedService {
    pub identity: ServiceIdentity,
    pub providers: Vec<ServiceProvider>,
    pub allow_cross_site: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DuckDnsMsg {
    ClaimHandle {
        handle: String,
    },
    ReleaseHandle {
        handle: String,
    },
    /// Full declarative replacement for the authenticated submitting node.
    ReplaceAnnouncements {
        announcements: Vec<ServiceAnnouncement>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DuckDnsQuery {
    Resolve {
        name: DuckDnsName,
    },
    /// Canonical currently published names for the active-workspace helper.
    /// The system adapter applies live standing/identity filtering.
    Namespace,
    HandleOwner {
        handle: String,
    },
    NodeAnnouncements {
        node: Vec<u8>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DuckDnsReply {
    Resolved(Option<ResolvedService>),
    Namespace(Vec<String>),
    HandleOwner(Option<Vec<u8>>),
    NodeAnnouncements(Vec<ServiceAnnouncement>),
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

/// Canonical data-plane hello metadata for one resolved service identity.
pub fn encode_service_identity(identity: &ServiceIdentity) -> Result<Vec<u8>, String> {
    identity.validate()?;
    serde_json::to_vec(identity).map_err(|error| error.to_string())
}

pub fn decode_service_identity(bytes: &[u8]) -> Result<ServiceIdentity, String> {
    let identity: ServiceIdentity =
        serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    identity.validate()?;
    Ok(identity)
}
