//! Embedded airlock gateway for a credential-provider node.
//!
//! When `DUCKTAPE_AIRLOCK_SERVE` is set, the node runs an airlock gateway
//! in-process on a loopback port and registers that port as the `airlock`
//! gateway route, so a compute node can reach it over the overlay
//! (`airlock.<handle>.duck`). The credential provider IS this process, so there
//! is no host-vs-enclave boundary to seal across: the operator's credential is
//! seeded directly (see `airlock::server::build_seeded`), not uploaded sealed.
//!
//! Route PUBLICATION (the signed `SetRoute`) stays a one-time operator step
//! (`user-sign-gateway-route`) — it is a signed ownership act. This module only
//! runs the gateway + registers its loopback port + seeds the credential.
//!
//! Config is env (mirroring the compute-side `DUCKTAPE_AIRLOCK_*` the broker
//! reads — parsed HERE once, the single SERVE_* boundary); a `node.toml
//! [airlock]` section is a later convenience:
//! - `DUCKTAPE_AIRLOCK_SERVE=1`                        enable
//! - `DUCKTAPE_AIRLOCK_SERVE_ATTEST=tdx|snp|auto`      REQUIRED (no mock exists;
//!   the node must run inside a confidential VM). Clients pin the measurement
//!   on their side; the serving node takes none.
//! - `DUCKTAPE_AIRLOCK_SERVE_ANTHROPIC_BASE=<url>`     default api.anthropic.com
//! - `DUCKTAPE_AIRLOCK_SERVE_OAUTH_TOKEN_URL=<url>`    default console oauth
//! - `DUCKTAPE_AIRLOCK_SERVE_OAUTH_CLIENT_ID=<id>`     default the Claude Code id
//! - `DUCKTAPE_AIRLOCK_SERVE_PORT=<port>`              default ephemeral
//! - `DUCKTAPE_AIRLOCK_SERVE_CREDENTIALS=<path>`       seal source (~/.claude/.credentials.json)
//! - `DUCKTAPE_AIRLOCK_SERVE_CRED_KIND=bearer|refresh` default bearer (no rotation)

use airlock::server::GatewayConfig;
use airlock::wire::CredentialPayload;

const ANTHROPIC_BASE: &str = "https://api.anthropic.com";
const OAUTH_TOKEN_URL: &str = "https://console.anthropic.com/v1/oauth/token";
const OAUTH_CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";

/// The resolved config for an embedded airlock gateway.
pub struct AirlockServe {
    pub cfg: GatewayConfig,
    /// the credential to seed the enclave with, if a source was configured.
    pub credential: Option<CredentialPayload>,
    /// requested loopback port, or `None` for an ephemeral one.
    pub port: Option<u16>,
}

fn env_nonempty(key: &str) -> Option<String> {
    std::env::var_os(key)
        .and_then(|value| value.into_string().ok())
        .filter(|value| !value.is_empty())
}

impl AirlockServe {
    /// `Some(Ok)` when `DUCKTAPE_AIRLOCK_SERVE` is set and valid; `Some(Err)` when
    /// set but misconfigured (fail boot loudly, never silently skip); `None` off.
    pub fn from_env() -> Option<Result<Self, String>> {
        env_nonempty("DUCKTAPE_AIRLOCK_SERVE")?;
        Some(Self::resolve())
    }

    fn resolve() -> Result<Self, String> {
        let attest = env_nonempty("DUCKTAPE_AIRLOCK_SERVE_ATTEST").ok_or_else(|| {
            "DUCKTAPE_AIRLOCK_SERVE is set but DUCKTAPE_AIRLOCK_SERVE_ATTEST is not \
             ('tdx'|'snp'|'auto'; the node must run inside a confidential VM)"
                .to_string()
        })?;
        let port = env_nonempty("DUCKTAPE_AIRLOCK_SERVE_PORT")
            .map(|p| p.parse::<u16>().map_err(|e| format!("airlock serve port: {e}")))
            .transpose()?;
        let credential = resolve_credential()?;
        let cfg = GatewayConfig {
            attest,
            anthropic_base: env_nonempty("DUCKTAPE_AIRLOCK_SERVE_ANTHROPIC_BASE")
                .unwrap_or_else(|| ANTHROPIC_BASE.into()),
            oauth_token_url: env_nonempty("DUCKTAPE_AIRLOCK_SERVE_OAUTH_TOKEN_URL")
                .unwrap_or_else(|| OAUTH_TOKEN_URL.into()),
            oauth_client_id: env_nonempty("DUCKTAPE_AIRLOCK_SERVE_OAUTH_CLIENT_ID")
                .unwrap_or_else(|| OAUTH_CLIENT_ID.into()),
            session_ttl_secs: 3600,
            max_requests: 4096,
        };
        Ok(Self { cfg, credential, port })
    }
}

/// Read the credential to seed from `DUCKTAPE_AIRLOCK_SERVE_CREDENTIALS` (a Claude
/// credentials file). `bearer` (default) seals the CURRENT access token (no
/// rotation); `refresh` seals the refresh token. `None` when unset — the operator
/// can seal later via `airlock-cli seal`.
fn resolve_credential() -> Result<Option<CredentialPayload>, String> {
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
        })),
        other => Err(format!(
            "DUCKTAPE_AIRLOCK_SERVE_CRED_KIND must be 'bearer' or 'refresh', got {other:?}"
        )),
    }
}
