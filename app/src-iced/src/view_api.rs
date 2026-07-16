//! Capability-free types shared by native views and their trusted host.
//!
//! Keep this module free of backend, transport, browser, filesystem, and
//! platform imports so a view can later move to its own package unchanged.

use base64::Engine as _;
use serde::{Deserialize, Serialize};

const LINK_RESPONSE_PREFIX: &str = "ducktape-link-response-v1:";
const MAX_LINK_BLOB_BYTES: usize = 4 * 1024;
const MAX_LINK_LABEL_BYTES: usize = 64;
const MAX_LINK_PROOF_BYTES: usize = 4 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewId {
    Home,
    Chat,
    Pages,
    Files,
}

/// Mint a fresh client-side entity id. Pure (clock only) so capability-free
/// screens may mint ids for optimistic selection (e.g. pages create-and-open).
pub(crate) fn fresh_id(prefix: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{prefix}-{nanos:x}")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resource<T> {
    Loading,
    Empty,
    Error(String),
    Ready(T),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    // Modules can request the root route even though the built-in views do not
    // currently emit it.
    #[allow(dead_code)]
    Home,
    Chat {
        channel: Option<String>,
        message: Option<u64>,
    },
    Page {
        page: String,
        block: Option<String>,
    },
    File {
        path: String,
        directory: bool,
    },
    Forge {
        repository: String,
        item: Option<u64>,
    },
    Member {
        key: String,
        account: Option<String>,
    },
    Agent {
        id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppIntent {
    Navigate(Route),
    OpenExternal(String),
    PopOutHuddle,
}

/// Opaque authority to consume one native file drop.
///
/// Views may pass this value back to the host, but cannot obtain or inspect the
/// native path it represents. The trusted host validates and consumes it once.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct DropToken([u8; 16]);

impl DropToken {
    pub(crate) const fn from_host(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }
}

impl std::fmt::Debug for DropToken {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("DropToken(..)")
    }
}

#[cfg(test)]
pub(crate) const fn test_drop_token() -> DropToken {
    DropToken([7; 16])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberKeyKind {
    Ed25519,
    P256,
    WebauthnP256,
}

impl MemberKeyKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Ed25519 => "ed25519",
            Self::P256 => "p256",
            Self::WebauthnP256 => "webauthn_p256",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LinkResponse {
    pub pubkey: String,
    pub kind: MemberKeyKind,
    pub possession: String,
    pub label: Option<String>,
}

pub fn encode_link_response(response: &LinkResponse) -> Result<String, String> {
    validate_link_response(response)?;
    let json =
        serde_json::to_vec(response).map_err(|_| "could not encode link data".to_string())?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(json);
    let blob = format!("{LINK_RESPONSE_PREFIX}{encoded}");
    if blob.len() > MAX_LINK_BLOB_BYTES {
        return Err("link data is too large".to_string());
    }
    Ok(blob)
}

pub fn decode_link_response(blob: &str) -> Result<LinkResponse, String> {
    let blob = blob.trim();
    if blob.len() > MAX_LINK_BLOB_BYTES {
        return Err("malformed link response".to_string());
    }
    let encoded = blob
        .strip_prefix(LINK_RESPONSE_PREFIX)
        .filter(|encoded| !encoded.is_empty())
        .ok_or_else(|| "malformed link response".to_string())?;
    let json = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| "malformed link response".to_string())?;
    let response =
        serde_json::from_slice(&json).map_err(|_| "malformed link response".to_string())?;
    validate_link_response(&response)?;
    Ok(response)
}

fn validate_link_response(response: &LinkResponse) -> Result<(), String> {
    if response.pubkey.len() != 64 || !response.pubkey.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("response public key is not a 32-byte hexadecimal key".to_string());
    }
    if response.kind != MemberKeyKind::Ed25519 {
        return Err("the desktop link flow only accepts Ed25519 device keys".to_string());
    }
    if response.possession.is_empty() || response.possession.len() > MAX_LINK_PROOF_BYTES {
        return Err("response possession proof is missing or too large".to_string());
    }
    serde_json::from_str::<serde_json::Value>(&response.possession)
        .map_err(|_| "response possession proof is malformed".to_string())?;
    if let Some(label) = response.label.as_deref()
        && (label.len() > MAX_LINK_LABEL_BYTES || label.chars().any(char::is_control))
    {
        return Err("response device label is missing, too long, or contains controls".to_string());
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SubmitReceipt {
    pub height: u64,
    pub app_hash: String,
    #[serde(default)]
    pub op_hash: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_response_round_trips_without_backend_authority() {
        let response = LinkResponse {
            pubkey: "11".repeat(32),
            kind: MemberKeyKind::Ed25519,
            possession: "{}".into(),
            label: Some("Laptop".into()),
        };
        let encoded = encode_link_response(&response).unwrap();
        assert_eq!(decode_link_response(&encoded).unwrap(), response);
    }
}
