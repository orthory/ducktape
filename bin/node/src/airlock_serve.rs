//! Embedded airlock gateway for a credential-provider node.
//!
//! Two ways a node comes to serve credentials, resolved by [`AirlockServe::resolve`]:
//!
//! - **Self-host store** (the `user cred add` path): a disk-backed, named
//!   credential store under `<storage>/airlock-creds/`. Serving turns on whenever
//!   that store holds at least one credential. There is no TEE — the trust anchor
//!   is the seal public key published on consensus, which the compute node's
//!   broker pins. The seal keypair persists at `<storage>/airlock-creds/seal.key`
//!   (0600) so the seal_pk this gateway seals under matches the one on-chain.
//! - **TEE env** (`DUCKTAPE_AIRLOCK_SERVE`, unchanged and taking precedence): the
//!   node runs inside a confidential VM and attests with a real quote. Config is
//!   env (the single `SERVE_*` boundary, parsed here once).
//!
//! Either way the node runs the gateway in-process on a loopback port and
//! registers that port as the `airlock` gateway route, so a compute node can
//! reach it over the overlay (`airlock.<handle>.duck`). Route PUBLICATION (the
//! signed `SetRoute`) stays a one-time operator step — it is a signed ownership
//! act; this module only runs the gateway + registers its loopback port.
//!
//! TEE env knobs:
//! - `DUCKTAPE_AIRLOCK_SERVE=1`                        enable
//! - `DUCKTAPE_AIRLOCK_SERVE_ATTEST=tdx|snp|auto`      REQUIRED (no mock exists;
//!   the node must run inside a confidential VM). Clients pin the measurement
//!   on their side; the serving node takes none.
//! - `DUCKTAPE_AIRLOCK_SERVE_NAME=<name>`              credential name (default
//!   `compute-provider`, matching the broker's default `sub`).
//! - `DUCKTAPE_AIRLOCK_SERVE_ANTHROPIC_BASE=<url>`     default api.anthropic.com
//! - `DUCKTAPE_AIRLOCK_SERVE_OPENAI_BASE=<url>`        default chatgpt codex base
//! - `DUCKTAPE_AIRLOCK_SERVE_OAUTH_TOKEN_URL=<url>`    default console oauth
//! - `DUCKTAPE_AIRLOCK_SERVE_OAUTH_CLIENT_ID=<id>`     default the Claude Code id
//! - `DUCKTAPE_AIRLOCK_SERVE_PORT=<port>`              default ephemeral
//! - `DUCKTAPE_AIRLOCK_SERVE_CREDENTIALS=<path>`       seal source (~/.claude/.credentials.json)
//! - `DUCKTAPE_AIRLOCK_SERVE_CRED_KIND=bearer|refresh` default bearer (no rotation)

use std::path::{Path, PathBuf};

use airlock::seal::SealKeypair;
use airlock::server::{AttestMode, GatewayConfig};
use airlock::wire::{CredentialKind, CredentialPayload};
use futures::SinkExt as _;
use futures::channel::{mpsc, oneshot};
use gateway::{GatewayQuery, GatewayReply, credential_use_allowed};
use noded::NodeCommand;

const ANTHROPIC_BASE: &str = "https://api.anthropic.com";
const OPENAI_BASE: &str = "https://chatgpt.com/backend-api/codex";
const OAUTH_TOKEN_URL: &str = "https://console.anthropic.com/v1/oauth/token";
const OAUTH_CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";

/// The resolved config for an embedded airlock gateway.
pub struct AirlockServe {
    pub cfg: GatewayConfig,
    /// The named credentials to seed the gateway store with. One per credential
    /// dir (self-host) or the single env-configured credential (TEE).
    pub seeds: Vec<(String, CredentialKind, CredentialPayload)>,
    /// requested loopback port, or `None` for an ephemeral one.
    pub port: Option<u16>,
    /// Whether this gateway LENDS credentials to other accounts — the self-host
    /// store path, whose credentials are registered on-chain with owner-signed
    /// grants. When set, the caller wires the committed-state grant gate (see
    /// [`committed_grant_check`]) so a session claiming an ungranted account is
    /// refused at the owner's own gateway. The TEE env path (a single, locally
    /// configured enclave credential) is not lent, so it leaves the gate off.
    pub grant_gated: bool,
}

fn env_nonempty(key: &str) -> Option<String> {
    std::env::var_os(key)
        .and_then(|value| value.into_string().ok())
        .filter(|value| !value.is_empty())
}

impl AirlockServe {
    /// Decide whether — and how — this node serves airlock credentials.
    /// `Some(Ok)` = serve; `Some(Err)` = configured but broken (fail boot loudly,
    /// never silently skip); `None` = off. The TEE env path takes precedence over
    /// the self-host store.
    pub fn resolve(storage: &Path) -> Option<Result<Self, String>> {
        let tee_requested = env_nonempty("DUCKTAPE_AIRLOCK_SERVE").is_some();
        if tee_requested {
            return Some(Self::resolve_env());
        }
        Self::resolve_store(storage).transpose()
    }

    fn resolve_env() -> Result<Self, String> {
        let attest = env_nonempty("DUCKTAPE_AIRLOCK_SERVE_ATTEST").ok_or_else(|| {
            "DUCKTAPE_AIRLOCK_SERVE is set but DUCKTAPE_AIRLOCK_SERVE_ATTEST is not \
             ('tdx'|'snp'|'auto'; the node must run inside a confidential VM)"
                .to_string()
        })?;
        let port = env_nonempty("DUCKTAPE_AIRLOCK_SERVE_PORT")
            .map(|p| p.parse::<u16>().map_err(|e| format!("airlock serve port: {e}")))
            .transpose()?;
        // The env path seeds one credential; it is claude-only (all it ever was).
        let name = env_nonempty("DUCKTAPE_AIRLOCK_SERVE_NAME")
            .unwrap_or_else(|| "compute-provider".into());
        let seeds = resolve_env_credential()?
            .map(|payload| (name, CredentialKind::Claude, payload))
            .into_iter()
            .collect();
        let (anthropic_base, openai_base, oauth_token_url, oauth_client_id) = base_fields();
        let cfg = GatewayConfig {
            attest: AttestMode::Tsm(attest),
            seal_keypair: None,
            anthropic_base,
            openai_base,
            oauth_token_url,
            oauth_client_id,
            session_ttl_secs: 3600,
            max_requests: 4096,
        };
        // A single, locally-configured enclave credential — not lent, no grant gate.
        Ok(Self { cfg, seeds, port, grant_gated: false })
    }

    /// Serve from the disk-backed store when it holds at least one credential.
    /// `Ok(None)` = store empty (airlock off). Reads the persisted seal keypair
    /// (minting it on first boot) so the seal_pk matches what `cred add` put on
    /// consensus.
    fn resolve_store(storage: &Path) -> Result<Option<Self>, String> {
        let root = cred_store_root(storage);
        let seeds = load_seeds(&root)?;
        if seeds.is_empty() {
            return Ok(None);
        }
        let seal = load_or_create_seal_keypair(&root)?;
        let (anthropic_base, openai_base, oauth_token_url, oauth_client_id) = base_fields();
        let cfg = GatewayConfig {
            attest: AttestMode::SelfHost,
            seal_keypair: Some(seal),
            anthropic_base,
            openai_base,
            oauth_token_url,
            oauth_client_id,
            session_ttl_secs: 3600,
            max_requests: 4096,
        };
        // Store credentials are registered on-chain with owner-signed grants, so
        // the owner's own gateway enforces those grants (see `committed_grant_check`).
        Ok(Some(Self { cfg, seeds, port: None, grant_gated: true }))
    }
}

/// The upstream base fields, honoring the `DUCKTAPE_AIRLOCK_SERVE_*` overrides and
/// defaulting to the production endpoints. Shared by both serve paths so the
/// self-host store can be pointed at a test or proxy upstream the same way the
/// TEE path already can.
fn base_fields() -> (String, String, String, String) {
    (
        env_nonempty("DUCKTAPE_AIRLOCK_SERVE_ANTHROPIC_BASE").unwrap_or_else(|| ANTHROPIC_BASE.into()),
        env_nonempty("DUCKTAPE_AIRLOCK_SERVE_OPENAI_BASE").unwrap_or_else(|| OPENAI_BASE.into()),
        env_nonempty("DUCKTAPE_AIRLOCK_SERVE_OAUTH_TOKEN_URL").unwrap_or_else(|| OAUTH_TOKEN_URL.into()),
        env_nonempty("DUCKTAPE_AIRLOCK_SERVE_OAUTH_CLIENT_ID").unwrap_or_else(|| OAUTH_CLIENT_ID.into()),
    )
}

/// The credential store root: one dir per credential under `<storage>/airlock-creds/`,
/// plus `seal.key` at the top.
pub fn cred_store_root(storage: &Path) -> PathBuf {
    storage.join("airlock-creds")
}

/// Build the co-hosted-lending grant gate the owner's own gateway enforces: given
/// a credential name and the account a session claims, resolve THIS node's
/// committed gateway record and answer whether that account may draw on it (owner
/// or a granted account, per [`credential_use_allowed`]). Wired only for the
/// self-host store path, whose credentials carry owner-signed on-chain grants.
pub fn committed_grant_check(commands: mpsc::Sender<NodeCommand>) -> airlock::server::GrantCheck {
    std::sync::Arc::new(move |name: String, account: Vec<u8>| {
        let commands = commands.clone();
        Box::pin(async move { grant_allows(commands, &name, &account).await })
            as std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>>
    })
}

/// The grant decision behind [`committed_grant_check`]. Refuses on a missing
/// record or any query failure — a credential the node cannot prove a grant for
/// is never lent (fail closed).
async fn grant_allows(commands: mpsc::Sender<NodeCommand>, name: &str, account: &[u8]) -> bool {
    match committed_credential_record(commands, name).await {
        Ok(Some(record)) => credential_use_allowed(&record, account),
        Ok(None) | Err(_) => false,
    }
}

/// Read one credential record from this node's committed gateway-module state over
/// the actor command lane (the same lane the credential resolver and provisioner
/// use), so the gate sees exactly what consensus committed.
async fn committed_credential_record(
    mut commands: mpsc::Sender<NodeCommand>,
    name: &str,
) -> Result<Option<gateway::CredentialRecord>, String> {
    let (reply, rx) = oneshot::channel();
    commands
        .send(NodeCommand::Query {
            target: "gateway".to_string(),
            req: gateway::encode_query(&GatewayQuery::Credential { name: name.to_string() }),
            reply,
        })
        .await
        .map_err(|_| "node actor is gone".to_string())?;
    let bytes = rx.await.map_err(|_| "node actor dropped the query reply".to_string())??;
    match gateway::decode_reply(&bytes)? {
        GatewayReply::Credential(record) => Ok(record),
        other => Err(format!("gateway returned an unexpected reply: {other:?}")),
    }
}

/// Load every credential in the store as a gateway seed. A dir missing its `kind`
/// marker or its login artifact is SKIPPED with a warn (never a hard boot error —
/// one broken credential must not stop the node serving the rest). A missing store
/// root is simply an empty store. Order-stable by name so boot is deterministic.
pub fn load_seeds(
    root: &Path,
) -> Result<Vec<(String, CredentialKind, CredentialPayload)>, String> {
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(format!("read {}: {err}", root.display())),
    };
    let mut seeds = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("read {}: {e}", root.display()))?;
        let path = entry.path();
        let is_cred_dir = path.is_dir();
        if !is_cred_dir {
            continue; // seal.key and any stray files
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        match load_cred_dir(&path) {
            Some((kind, payload)) => seeds.push((name, kind, payload)),
            None => tracing::warn!(
                target: "ducktape::gateway",
                reason = "airlock_cred_incomplete",
                credential = %name,
                "airlock credential dir skipped: missing kind marker or login artifact"
            ),
        }
    }
    seeds.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(seeds)
}

/// One credential dir → its seed, or `None` when incomplete. The `kind` marker
/// selects which artifact to read: claude's `.credentials.json` yields a rotating
/// `Refresh`, codex's `auth.json` a static `Bearer`.
fn load_cred_dir(dir: &Path) -> Option<(CredentialKind, CredentialPayload)> {
    let kind = read_kind(dir)?;
    let payload = match kind {
        CredentialKind::Claude => claude_refresh_payload(dir)?,
        CredentialKind::Codex => codex_bearer_payload(dir)?,
    };
    Some((kind, payload))
}

fn read_kind(dir: &Path) -> Option<CredentialKind> {
    let raw = std::fs::read_to_string(dir.join("kind")).ok()?;
    match raw.trim() {
        "claude" => Some(CredentialKind::Claude),
        "codex" => Some(CredentialKind::Codex),
        _ => None,
    }
}

/// The claude login artifact (`.credentials.json`, `claudeAiOauth`) as a
/// refresh credential carrying the CURRENT access token + its expiry alongside
/// the rotating refresh token. Seeding the live access token means the gateway
/// serves it as-is until it expires — no refresh fires meanwhile, so the owner's
/// own local login (sharing the refresh chain) is not rotation-invalidated
/// during that window. `expiresAt` is epoch MILLISECONDS in the artifact.
fn claude_refresh_payload(dir: &Path) -> Option<CredentialPayload> {
    let raw = std::fs::read_to_string(dir.join(".credentials.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let oauth = &json["claudeAiOauth"];
    let refresh_token = oauth["refreshToken"]
        .as_str()
        .filter(|value| !value.is_empty())?
        .to_string();
    let access_token = oauth["accessToken"].as_str().unwrap_or("").to_string();
    let expires_at = oauth["expiresAt"].as_u64().map(|ms| ms / 1000).unwrap_or(0);
    Some(CredentialPayload::Refresh { refresh_token, access_token, expires_at })
}

/// The access token out of a codex login artifact (`auth.json`,
/// `tokens.access_token`, mirroring the host-codex broker read) — codex is
/// bearer-only, no rotation.
fn codex_bearer_payload(dir: &Path) -> Option<CredentialPayload> {
    let raw = std::fs::read_to_string(dir.join("auth.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let token = json["tokens"]["access_token"]
        .as_str()
        .filter(|value| !value.is_empty())?;
    Some(CredentialPayload::Bearer { access_token: token.to_string() })
}

/// Load the store's seal keypair, minting and persisting it (0600) on first boot.
/// The PUBLIC key is what `cred add` publishes on consensus; the compute broker
/// pins it, so this secret must be STABLE across boots — hence disk, not memory.
pub fn load_or_create_seal_keypair(root: &Path) -> Result<SealKeypair, String> {
    let path = root.join("seal.key");
    match std::fs::read(&path) {
        Ok(bytes) => {
            let secret: [u8; 32] = bytes.as_slice().try_into().map_err(|_| {
                format!("{}: seal.key must be 32 bytes, got {}", path.display(), bytes.len())
            })?;
            Ok(SealKeypair::from_secret_bytes(secret))
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir_all(root)
                .map_err(|e| format!("create {}: {e}", root.display()))?;
            let keypair = SealKeypair::generate();
            write_secret_0600(&path, &keypair.secret_bytes())?;
            Ok(keypair)
        }
        Err(err) => Err(format!("read {}: {err}", path.display())),
    }
}

/// Write secret bytes to a fresh 0600 file (mirrors `userkey::write_user_key_new`).
fn write_secret_0600(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        opts.mode(0o600);
    }
    let mut file = opts.open(path).map_err(|e| format!("create {}: {e}", path.display()))?;
    if let Err(e) = std::io::Write::write_all(&mut file, bytes) {
        let _ = std::fs::remove_file(path);
        return Err(format!("write {}: {e}", path.display()));
    }
    Ok(())
}

/// TEE env path only: read the single credential to seed from
/// `DUCKTAPE_AIRLOCK_SERVE_CREDENTIALS` (a Claude credentials file). `bearer`
/// (default) seals the CURRENT access token (no rotation); `refresh` seals the
/// refresh token. `None` when unset — the operator can seal later via `airlock-cli`.
fn resolve_env_credential() -> Result<Option<CredentialPayload>, String> {
    let Some(path) = env_nonempty("DUCKTAPE_AIRLOCK_SERVE_CREDENTIALS") else {
        return Ok(None);
    };
    let raw = std::fs::read_to_string(&path).map_err(|e| format!("read {path}: {e}"))?;
    let json: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("{path}: credentials json: {e}"))?;
    let oauth = &json["claudeAiOauth"];
    let field = |key: &str| -> Result<String, String> {
        oauth[key]
            .as_str()
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .ok_or_else(|| format!("{path}: claudeAiOauth.{key} not found"))
    };
    match env_nonempty("DUCKTAPE_AIRLOCK_SERVE_CRED_KIND")
        .unwrap_or_else(|| "bearer".into())
        .as_str()
    {
        "bearer" => Ok(Some(CredentialPayload::Bearer { access_token: field("accessToken")? })),
        "refresh" => Ok(Some(CredentialPayload::Refresh {
            refresh_token: field("refreshToken")?,
            access_token: oauth["accessToken"].as_str().unwrap_or("").to_string(),
            expires_at: oauth["expiresAt"].as_u64().map(|ms| ms / 1000).unwrap_or(0),
        })),
        other => Err(format!(
            "DUCKTAPE_AIRLOCK_SERVE_CRED_KIND must be 'bearer' or 'refresh', got {other:?}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, contents: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    fn seed_claude(root: &Path, name: &str, refresh: &str) {
        let dir = root.join(name);
        write(&dir.join("kind"), "claude\n");
        write(
            &dir.join(".credentials.json"),
            &format!(r#"{{"claudeAiOauth":{{"refreshToken":"{refresh}"}}}}"#),
        );
    }

    fn seed_codex(root: &Path, name: &str, access: &str) {
        let dir = root.join(name);
        write(&dir.join("kind"), "codex\n");
        write(&dir.join("auth.json"), &format!(r#"{{"tokens":{{"access_token":"{access}"}}}}"#));
    }

    #[test]
    fn empty_root_yields_no_seeds() {
        let tmp = tempfile::tempdir().unwrap();
        let root = cred_store_root(tmp.path());
        assert!(load_seeds(&root).unwrap().is_empty());
    }

    #[test]
    fn claude_dir_loads_a_refresh_seed() {
        let tmp = tempfile::tempdir().unwrap();
        let root = cred_store_root(tmp.path());
        seed_claude(&root, "eddy-claude-1", "rt-eddy");
        let seeds = load_seeds(&root).unwrap();
        assert_eq!(seeds.len(), 1);
        let (name, kind, payload) = &seeds[0];
        assert_eq!(name, "eddy-claude-1");
        assert_eq!(*kind, CredentialKind::Claude);
        assert!(matches!(payload, CredentialPayload::Refresh { refresh_token, .. } if refresh_token == "rt-eddy"));
    }

    #[test]
    fn codex_dir_loads_a_bearer_seed() {
        let tmp = tempfile::tempdir().unwrap();
        let root = cred_store_root(tmp.path());
        seed_codex(&root, "eddy-codex-1", "tok-codex");
        let seeds = load_seeds(&root).unwrap();
        assert_eq!(seeds.len(), 1);
        let (name, kind, payload) = &seeds[0];
        assert_eq!(name, "eddy-codex-1");
        assert_eq!(*kind, CredentialKind::Codex);
        assert!(matches!(payload, CredentialPayload::Bearer { access_token } if access_token == "tok-codex"));
    }

    #[test]
    fn seeds_are_order_stable_by_name() {
        let tmp = tempfile::tempdir().unwrap();
        let root = cred_store_root(tmp.path());
        seed_claude(&root, "b", "rt-b");
        seed_claude(&root, "a", "rt-a");
        seed_codex(&root, "c", "tok-c");
        let names: Vec<_> = load_seeds(&root).unwrap().into_iter().map(|(n, ..)| n).collect();
        assert_eq!(names, vec!["a", "b", "c"]);
    }

    #[test]
    fn dir_missing_its_artifact_is_skipped_not_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let root = cred_store_root(tmp.path());
        // a kind marker with no login artifact beside it
        write(&root.join("broken").join("kind"), "claude\n");
        seed_claude(&root, "good", "rt-good");
        let seeds = load_seeds(&root).unwrap();
        assert_eq!(seeds.len(), 1, "the broken dir is skipped, the good one survives");
        assert_eq!(seeds[0].0, "good");
    }

    #[test]
    fn seal_key_file_is_not_mistaken_for_a_credential() {
        let tmp = tempfile::tempdir().unwrap();
        let root = cred_store_root(tmp.path());
        let _kp = load_or_create_seal_keypair(&root).unwrap(); // writes seal.key
        seed_claude(&root, "eddy-claude-1", "rt-eddy");
        let seeds = load_seeds(&root).unwrap();
        assert_eq!(seeds.len(), 1);
        assert_eq!(seeds[0].0, "eddy-claude-1");
    }

    #[test]
    fn seal_keypair_is_created_once_and_stable() {
        let tmp = tempfile::tempdir().unwrap();
        let root = cred_store_root(tmp.path());
        let first = load_or_create_seal_keypair(&root).unwrap();
        let second = load_or_create_seal_keypair(&root).unwrap();
        assert_eq!(first.public_bytes(), second.public_bytes());
        assert_eq!(first.secret_bytes(), second.secret_bytes());
    }

    #[cfg(unix)]
    #[test]
    fn seal_key_is_written_0600() {
        use std::os::unix::fs::PermissionsExt as _;
        let tmp = tempfile::tempdir().unwrap();
        let root = cred_store_root(tmp.path());
        load_or_create_seal_keypair(&root).unwrap();
        let mode = std::fs::metadata(root.join("seal.key")).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
