//! Fixed-purpose account signing APIs for the iced shell.

use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use super::Backend;
use super::identity::{SecretString, signing_secrets};
use super::node_control::{last_line, run_verb_with_stdin};
use crate::view_api::MemberKeyKind;

const MAX_IDENTIFIER_BYTES: usize = 256;
const MAX_LABEL_BYTES: usize = 64;
const MAX_STATEMENT_BYTES: usize = 4 * 1024;
const MAX_POSSESSION_BYTES: usize = 16 * 1024;
const MAX_PAYLOAD_HEX_BYTES: usize = 47 * 1024;
const MAX_ADMIN_PATH_BYTES: usize = 2 * 1024;

#[derive(Debug, Clone)]
pub struct BindRequest {
    pub chain_id: String,
    pub node_pubkey: String,
    pub nonce: u64,
}

#[derive(Debug, Clone)]
pub struct PossessionRequest {
    pub chain_id: String,
    pub account_id: String,
    pub nonce: u64,
}

#[derive(Debug, Clone)]
pub struct AddMemberRequest {
    pub chain_id: String,
    pub account_id: String,
    pub new_pubkey: String,
    pub new_kind: MemberKeyKind,
    pub nonce: u64,
    pub possession: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RemoveMemberRequest {
    pub chain_id: String,
    pub account_id: String,
    pub target_pubkey: String,
    pub nonce: u64,
}

#[derive(Clone)]
pub struct AdminProofRequest {
    pub method: String,
    pub path: String,
    pub node_pubkey: String,
}

impl std::fmt::Debug for AdminProofRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AdminProofRequest")
            .field("method", &self.method)
            .field("path", &"[REDACTED]")
            .field("node_pubkey", &self.node_pubkey)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentTarget {
    Chat,
    Pages,
    Files,
    Forge,
    Tasks,
    Kv,
    Directory,
    Tagging,
    Inbox,
    Governance,
    Agent,
    Runs,
}

impl ContentTarget {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Pages => "pages",
            Self::Files => "files",
            Self::Forge => "forge",
            Self::Tasks => "tasks",
            Self::Kv => "kv",
            Self::Directory => "directory",
            Self::Tagging => "tagging",
            Self::Inbox => "inbox",
            Self::Governance => "governance",
            Self::Agent => "agent",
            Self::Runs => "runs",
        }
    }
}

impl Backend {
    pub async fn sign_bind(&self, request: BindRequest) -> Result<String, String> {
        let root = self.root.clone();
        self.control
            .run(move || {
                validate_identifier(&request.chain_id, "chain id")?;
                validate_ed25519_key(&request.node_pubkey, "node public key")?;
                let nonce = request.nonce.to_string();
                run_sign_verb(
                    &root,
                    "user-sign-bind",
                    &[
                        ("--chain-id", &request.chain_id),
                        ("--node-pub", &request.node_pubkey),
                        ("--nonce", &nonce),
                    ],
                )
            })
            .await
    }

    pub async fn sign_unbind(&self, request: BindRequest) -> Result<String, String> {
        let root = self.root.clone();
        self.control
            .run(move || {
                validate_identifier(&request.chain_id, "chain id")?;
                validate_ed25519_key(&request.node_pubkey, "node public key")?;
                let nonce = request.nonce.to_string();
                run_sign_verb(
                    &root,
                    "user-sign-unbind",
                    &[
                        ("--chain-id", &request.chain_id),
                        ("--node-pub", &request.node_pubkey),
                        ("--nonce", &nonce),
                    ],
                )
            })
            .await
    }

    pub async fn sign_gateway_route(&self, statement: String) -> Result<String, String> {
        let root = self.root.clone();
        self.control
            .run(move || {
                validate_json(&statement, "gateway route statement", MAX_STATEMENT_BYTES)?;
                run_sign_verb(
                    &root,
                    "user-sign-gateway-route",
                    &[("--statement", &statement)],
                )
            })
            .await
    }

    pub async fn sign_possession(&self, request: PossessionRequest) -> Result<String, String> {
        let root = self.root.clone();
        self.control
            .run(move || {
                validate_identifier(&request.chain_id, "chain id")?;
                validate_ed25519_key(&request.account_id, "account id")?;
                let nonce = request.nonce.to_string();
                run_sign_verb(
                    &root,
                    "user-sign-possession",
                    &[
                        ("--chain-id", &request.chain_id),
                        ("--account-id", &request.account_id),
                        ("--nonce", &nonce),
                    ],
                )
            })
            .await
    }

    pub async fn sign_add_member(&self, request: AddMemberRequest) -> Result<String, String> {
        let root = self.root.clone();
        self.control
            .run(move || {
                validate_identifier(&request.chain_id, "chain id")?;
                validate_ed25519_key(&request.account_id, "account id")?;
                validate_member_key(&request.new_pubkey, request.new_kind)?;
                validate_json(
                    &request.possession,
                    "possession proof",
                    MAX_POSSESSION_BYTES,
                )?;
                if let Some(label) = request.label.as_deref() {
                    validate_label(label)?;
                }

                let nonce = request.nonce.to_string();
                let mut flags = vec![
                    ("--chain-id", request.chain_id.as_str()),
                    ("--account-id", request.account_id.as_str()),
                    ("--new-key", request.new_pubkey.as_str()),
                    ("--new-kind", request.new_kind.as_str()),
                    ("--nonce", nonce.as_str()),
                    ("--possession", request.possession.as_str()),
                ];
                if let Some(label) = request.label.as_deref() {
                    flags.push(("--label", label));
                }
                run_sign_verb(&root, "user-sign-add-member", &flags)
            })
            .await
    }

    pub async fn sign_remove_member(&self, request: RemoveMemberRequest) -> Result<String, String> {
        let root = self.root.clone();
        self.control
            .run(move || {
                validate_identifier(&request.chain_id, "chain id")?;
                validate_ed25519_key(&request.account_id, "account id")?;
                validate_member_key_shape(&request.target_pubkey, "target public key")?;
                let nonce = request.nonce.to_string();
                run_sign_verb(
                    &root,
                    "user-sign-remove-member",
                    &[
                        ("--chain-id", &request.chain_id),
                        ("--account-id", &request.account_id),
                        ("--target-key", &request.target_pubkey),
                        ("--nonce", &nonce),
                    ],
                )
            })
            .await
    }

    pub async fn sign_files_frame(&self, payload_hex: String) -> Result<String, String> {
        self.sign_content_frame(ContentTarget::Files, payload_hex)
            .await
    }

    pub async fn sign_content_frame(
        &self,
        target: ContentTarget,
        payload_hex: String,
    ) -> Result<String, String> {
        let root = self.root.clone();
        let payload = SecretString::new(payload_hex);
        self.control
            .run(move || sign_frame(&root, target, payload))
            .await
    }

    pub async fn sign_admin_proof(&self, request: AdminProofRequest) -> Result<String, String> {
        let root = self.root.clone();
        self.control
            .run(move || {
                validate_admin_method(&request.method)?;
                validate_admin_path(&request.path)?;
                validate_ed25519_key(&request.node_pubkey, "node public key")?;
                run_sign_verb(
                    &root,
                    "user-sign-admin",
                    &[
                        ("--method", &request.method),
                        ("--path", &request.path),
                        ("--node-key", &request.node_pubkey),
                    ],
                )
            })
            .await
    }
}

fn run_sign_verb(root: &Path, verb: &str, flags: &[(&str, &str)]) -> Result<String, String> {
    let secrets = signing_secrets(root)?;
    let secret_refs: Vec<&str> = secrets.iter().map(SecretString::as_ref).collect();
    let key = root.join("user.key").to_string_lossy().into_owned();
    let mut args = vec![verb, "--key", &key];
    for (flag, value) in flags {
        args.push(flag);
        args.push(value);
    }
    let stdout = run_verb_with_stdin(&args, &secret_refs)?;
    nonempty_output(&stdout, verb)
}

fn sign_frame(root: &Path, target: ContentTarget, payload: SecretString) -> Result<String, String> {
    validate_hex(&payload, "frame payload", MAX_PAYLOAD_HEX_BYTES)?;
    let mut secrets = signing_secrets(root)?;
    secrets.push(payload);
    let secret_refs: Vec<&str> = secrets.iter().map(SecretString::as_ref).collect();
    let key = root.join("user.key").to_string_lossy().into_owned();
    let sequence = next_frame_sequence().to_string();
    let stdout = run_verb_with_stdin(
        &[
            "user-sign-frame",
            "--key",
            &key,
            "--target",
            target.as_str(),
            "--seq",
            &sequence,
        ],
        &secret_refs,
    )?;
    nonempty_output(&stdout, "user-sign-frame")
}

fn next_frame_sequence() -> u64 {
    static SEQUENCE: OnceLock<AtomicU64> = OnceLock::new();
    SEQUENCE
        .get_or_init(|| {
            let millis = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_millis() as u64)
                .unwrap_or(0);
            AtomicU64::new(millis)
        })
        .fetch_add(1, Ordering::Relaxed)
}

fn validate_identifier(value: &str, field: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > MAX_IDENTIFIER_BYTES {
        return Err(format!("{field} is missing or too long"));
    }
    if value.chars().any(char::is_control) {
        return Err(format!("{field} contains an unsupported character"));
    }
    Ok(())
}

fn validate_hex(value: &str, field: &str, max_bytes: usize) -> Result<(), String> {
    if value.is_empty()
        || value.len() > max_bytes
        || !value.len().is_multiple_of(2)
        || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(format!("{field} is not bounded, even-length hexadecimal"));
    }
    Ok(())
}

fn validate_ed25519_key(value: &str, field: &str) -> Result<(), String> {
    validate_hex(value, field, 64)?;
    if value.len() != 64 {
        return Err(format!("{field} is not a 32-byte Ed25519 key"));
    }
    Ok(())
}

fn validate_member_key(value: &str, kind: MemberKeyKind) -> Result<(), String> {
    validate_hex(value, "new member public key", 130)?;
    let valid_length = match kind {
        MemberKeyKind::Ed25519 => value.len() == 64,
        MemberKeyKind::P256 | MemberKeyKind::WebauthnP256 => {
            value.len() == 66 || value.len() == 130
        }
    };
    if !valid_length {
        return Err("new member public key has the wrong length for its kind".to_string());
    }
    Ok(())
}

fn validate_member_key_shape(value: &str, field: &str) -> Result<(), String> {
    validate_hex(value, field, 130)?;
    if matches!(value.len(), 64 | 66 | 130) {
        Ok(())
    } else {
        Err(format!("{field} has an unsupported member-key length"))
    }
}

fn validate_json(value: &str, field: &str, max_bytes: usize) -> Result<(), String> {
    if value.is_empty() || value.len() > max_bytes {
        return Err(format!("{field} is missing or too long"));
    }
    serde_json::from_str::<serde_json::Value>(value)
        .map(|_| ())
        .map_err(|_| format!("{field} is not valid JSON"))
}

fn validate_label(label: &str) -> Result<(), String> {
    if label.len() > MAX_LABEL_BYTES || label.chars().any(char::is_control) {
        return Err("member label is too long or contains a control character".to_string());
    }
    Ok(())
}

fn validate_admin_method(method: &str) -> Result<(), String> {
    if matches!(method, "GET" | "POST" | "PUT" | "PATCH" | "DELETE") {
        Ok(())
    } else {
        Err("admin request method is not allowed".to_string())
    }
}

fn validate_admin_path(path: &str) -> Result<(), String> {
    if path.len() <= MAX_ADMIN_PATH_BYTES
        && path.starts_with("/v1/admin/")
        && !path.contains('#')
        && !path.chars().any(char::is_control)
    {
        Ok(())
    } else {
        Err("admin request path is not a bounded /v1/admin path".to_string())
    }
}

fn nonempty_output(stdout: &str, verb: &str) -> Result<String, String> {
    let output = last_line(stdout);
    if output.is_empty() {
        Err(format!("{verb} returned no signed output"))
    } else {
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_targets_are_closed_and_stable() {
        assert_eq!(ContentTarget::Files.as_str(), "files");
        assert_eq!(ContentTarget::Governance.as_str(), "governance");
        let names = [
            ContentTarget::Chat,
            ContentTarget::Pages,
            ContentTarget::Files,
            ContentTarget::Forge,
            ContentTarget::Tasks,
            ContentTarget::Kv,
            ContentTarget::Directory,
            ContentTarget::Tagging,
            ContentTarget::Inbox,
            ContentTarget::Governance,
        ]
        .map(ContentTarget::as_str);
        assert!(!names.contains(&"identity"));
        assert!(!names.contains(&"valset"));
        assert!(!names.contains(&"gateway"));
    }

    #[test]
    fn keys_and_payloads_are_strictly_bounded() {
        let ed = "ab".repeat(32);
        assert!(validate_ed25519_key(&ed, "key").is_ok());
        assert!(validate_ed25519_key("ab12", "key").is_err());
        assert!(validate_hex("00ff", "payload", 8).is_ok());
        assert!(validate_hex("0ff", "payload", 8).is_err());
        assert!(validate_hex("00xx", "payload", 8).is_err());
        assert!(validate_hex(&"aa".repeat(5), "payload", 8).is_err());
    }

    #[test]
    fn member_kind_controls_key_length() {
        assert!(validate_member_key(&"11".repeat(32), MemberKeyKind::Ed25519).is_ok());
        assert!(validate_member_key(&"02".repeat(33), MemberKeyKind::P256).is_ok());
        assert!(validate_member_key(&"04".repeat(65), MemberKeyKind::WebauthnP256).is_ok());
        assert!(validate_member_key(&"11".repeat(32), MemberKeyKind::P256).is_err());
        assert!(validate_label(&"x".repeat(MAX_LABEL_BYTES)).is_ok());
        assert!(validate_label(&"x".repeat(MAX_LABEL_BYTES + 1)).is_err());
    }

    #[test]
    fn admin_proofs_are_scoped_to_the_admin_surface() {
        assert!(validate_admin_method("POST").is_ok());
        assert!(validate_admin_method("CONNECT").is_err());
        assert!(validate_admin_path("/v1/admin/shutdown").is_ok());
        assert!(validate_admin_path("/v1/submit/frame").is_err());
        assert!(validate_admin_path("https://example.test/v1/admin/x").is_err());
    }

    #[test]
    fn frame_sequence_is_monotonic() {
        let first = next_frame_sequence();
        let second = next_frame_sequence();
        assert_eq!(second, first + 1);
    }

    #[test]
    fn malformed_json_errors_do_not_echo_input() {
        let error = validate_json("{secret-token", "statement", 100).unwrap_err();
        assert!(!error.contains("secret-token"));
        assert!(validate_json(r#"{"ok":true}"#, "statement", 100).is_ok());
    }
}
