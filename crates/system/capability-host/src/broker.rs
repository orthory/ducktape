//! Run-scoped model brokers — one per provider wire shape.
//!
//! The provider child never receives the operator's API/OAuth credential.
//! Only this host process reads it, and serves a single-run loopback endpoint
//! the child dials with an unrelated random bearer. Direct/Podman bind
//! loopback; Tart binds the host side of its private NAT so the VM can reach it
//! by a guest-only hostname.
//!
//! Two wire shapes ship: the OpenAI Responses API (`codex exec`, aimed by argv)
//! and the Anthropic Messages API (`claude`, aimed by env — see
//! [`RunBroker::start_anthropic`]). They share the endpoint/bearer/teardown
//! scaffolding and the request/byte caps; they differ in the upstream
//! credential, the route, and — critically — Anthropic STREAMS the SSE response
//! through unbuffered where Codex buffers.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, HeaderValue, Response, StatusCode};
use axum::routing::{head, post};
use futures::StreamExt as _;
use rand::RngCore as _;
use tokio::sync::{Semaphore, oneshot};

const MAX_REQUEST_BYTES: usize = 8 * 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 32 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 128 * 1024 * 1024;
const MAX_REQUESTS: u32 = 64;
/// Anthropic-broker concurrency. Codex serialises to 1; Claude Code fans out
/// (parallel tool sub-requests, a haiku title generator), and with a STREAMING
/// response a permit is held for the whole stream — so 1 would deadlock a
/// client that opens a second request before the first's body drains.
/// ponytail: fixed 8, revisit only if a real session starves it.
const MAX_CONCURRENT: usize = 8;

#[derive(Clone)]
struct UpstreamCredential {
    bearer: String,
    account_id: Option<String>,
    url: String,
}

impl UpstreamCredential {
    fn from_host() -> Result<Self, String> {
        if let Some(key) = std::env::var_os("OPENAI_API_KEY")
            .and_then(|value| value.into_string().ok())
            .filter(|value| !value.is_empty())
        {
            return Ok(Self {
                bearer: key,
                account_id: None,
                url: "https://api.openai.com/v1/responses".into(),
            });
        }

        let auth_root = std::env::var_os("CODEX_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".codex"))
            })
            .ok_or_else(|| {
                "Codex broker has neither OPENAI_API_KEY nor a host HOME/CODEX_HOME".to_string()
            })?;
        let auth_path = auth_root.join("auth.json");
        let auth: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&auth_path)
                .map_err(|e| format!("read host Codex auth {}: {e}", auth_path.display()))?,
        )
        .map_err(|e| format!("parse host Codex auth {}: {e}", auth_path.display()))?;
        let tokens = auth
            .get("tokens")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| {
                format!(
                    "host Codex auth {} has no tokens object",
                    auth_path.display()
                )
            })?;
        let bearer = tokens
            .get("access_token")
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                format!(
                    "host Codex auth {} has no access_token; log in or set OPENAI_API_KEY",
                    auth_path.display()
                )
            })?
            .to_string();
        let account_id = tokens
            .get("account_id")
            .or_else(|| auth.get("account_id"))
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        Ok(Self {
            bearer,
            account_id,
            url: "https://chatgpt.com/backend-api/codex/responses".into(),
        })
    }
}

struct BrokerState {
    run_bearer: String,
    upstream: UpstreamCredential,
    client: reqwest::Client,
    requests: AtomicU32,
    bytes: AtomicU64,
    concurrent: Semaphore,
}

/// The only information that crosses into the provider child. Neither value
/// can recover the host credential; both die with this run's broker.
pub(crate) struct BrokerEndpoint {
    pub(crate) base_url: String,
    pub(crate) run_bearer: String,
}

/// how the provider child reaches this run's broker — which drives BOTH the
/// bind address and the `base_url` the child is handed.
///
/// `Loopback` is a same-netns child (Direct, or Podman under `--network=host`):
/// bind `127.0.0.1`, hand it `http://127.0.0.1:<port>`.
///
/// `HostGateway(host)` is a child in a SEPARATE netns that reaches the host only
/// through a gateway name in its `/etc/hosts` — a Tart VM guest (`ducktape-host`)
/// or a private-netns Podman container (`host.containers.internal`). The broker
/// binds a routable interface and the base_url names the gateway. The opaque
/// per-run bearer still gates it; binding beyond loopback is the reachability
/// cost of the stronger network isolation.
#[derive(Clone, Copy)]
pub(crate) enum Reachability {
    Loopback,
    HostGateway(&'static str),
}

impl Reachability {
    fn bind(self) -> std::net::Ipv4Addr {
        match self {
            Self::Loopback => std::net::Ipv4Addr::LOCALHOST,
            Self::HostGateway(_) => std::net::Ipv4Addr::UNSPECIFIED,
        }
    }

    /// `suffix` is `/v1` for codex (base points at the provider root) and `""`
    /// for Anthropic (Claude Code appends `/v1/messages` to `ANTHROPIC_BASE_URL`).
    fn base_url(self, addr: std::net::SocketAddr, suffix: &str) -> String {
        match self {
            Self::Loopback => format!("http://{addr}{suffix}"),
            Self::HostGateway(host) => format!("http://{host}:{}{suffix}", addr.port()),
        }
    }
}

pub(crate) struct RunBroker {
    pub(crate) endpoint: BrokerEndpoint,
    shutdown: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
}

impl RunBroker {
    pub(crate) async fn start() -> Result<Self, String> {
        Self::start_with(UpstreamCredential::from_host()?, Reachability::Loopback).await
    }

    pub(crate) async fn start_for_tart() -> Result<Self, String> {
        Self::start_with(
            UpstreamCredential::from_host()?,
            Reachability::HostGateway("ducktape-host"),
        )
        .await
    }

    /// a private-netns Podman container reaches the loopback host only via the
    /// `host.containers.internal` gateway podman adds to its `/etc/hosts`.
    pub(crate) async fn start_for_podman_private() -> Result<Self, String> {
        Self::start_with(
            UpstreamCredential::from_host()?,
            Reachability::HostGateway("host.containers.internal"),
        )
        .await
    }

    async fn start_with(upstream: UpstreamCredential, reach: Reachability) -> Result<Self, String> {
        let bind = reach.bind();
        let listener = tokio::net::TcpListener::bind((bind, 0))
            .await
            .map_err(|e| format!("bind run-scoped provider broker: {e}"))?;
        let addr = listener
            .local_addr()
            .map_err(|e| format!("read run-scoped provider broker address: {e}"))?;
        let mut secret = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut secret);
        let run_bearer = secret
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let state = Arc::new(BrokerState {
            run_bearer: run_bearer.clone(),
            upstream,
            client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|e| format!("build provider broker client: {e}"))?,
            requests: AtomicU32::new(0),
            bytes: AtomicU64::new(0),
            concurrent: Semaphore::new(1),
        });
        let app = Router::new()
            .route("/v1/responses", post(forward_responses))
            .fallback(reject)
            .layer(DefaultBodyLimit::max(MAX_REQUEST_BYTES))
            .with_state(state);
        let (shutdown, rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = rx.await;
                })
                .await;
        });
        Ok(Self {
            endpoint: BrokerEndpoint {
                base_url: reach.base_url(addr, "/v1"),
                run_bearer,
            },
            shutdown: Some(shutdown),
            task,
        })
    }
}

impl Drop for RunBroker {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.task.abort();
    }
}

async fn reject() -> StatusCode {
    StatusCode::FORBIDDEN
}

fn incoming_authorized(headers: &HeaderMap, expected: &str) -> bool {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == format!("Bearer {expected}"))
}

async fn forward_responses(
    State(state): State<Arc<BrokerState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response<Body> {
    if !incoming_authorized(&headers, &state.run_bearer) {
        return response(StatusCode::UNAUTHORIZED, "run broker credential rejected");
    }
    if state.requests.fetch_add(1, Ordering::Relaxed) >= MAX_REQUESTS {
        return response(
            StatusCode::TOO_MANY_REQUESTS,
            "run broker request budget exhausted",
        );
    }
    if state
        .bytes
        .fetch_add(body.len() as u64, Ordering::Relaxed)
        .saturating_add(body.len() as u64)
        > MAX_TOTAL_BYTES
    {
        return response(
            StatusCode::PAYLOAD_TOO_LARGE,
            "run broker byte budget exhausted",
        );
    }
    let Ok(_permit) = state.concurrent.try_acquire() else {
        return response(
            StatusCode::TOO_MANY_REQUESTS,
            "run broker concurrency exhausted",
        );
    };

    let mut request = state
        .client
        .post(&state.upstream.url)
        .bearer_auth(&state.upstream.bearer)
        .body(body);
    // Match the official Codex responses-api-proxy posture: preserve Codex's
    // protocol/version/session headers, but replace auth and hop-by-hop HTTP
    // framing. The incoming authorization is only the opaque run bearer.
    for (name, value) in &headers {
        if matches!(
            name.as_str(),
            "authorization" | "host" | "content-length" | "connection" | "transfer-encoding"
        ) {
            continue;
        }
        if let (Ok(name), Ok(value)) = (
            reqwest::header::HeaderName::from_bytes(name.as_str().as_bytes()),
            reqwest::header::HeaderValue::from_bytes(value.as_bytes()),
        ) {
            request = request.header(name, value);
        }
    }
    if let Some(account_id) = &state.upstream.account_id {
        request = request.header("ChatGPT-Account-ID", account_id);
    }
    let mut upstream = match request.send().await {
        Ok(response) => response,
        Err(e) => {
            return response(
                StatusCode::BAD_GATEWAY,
                &format!("provider upstream failed: {e}"),
            );
        }
    };
    let status =
        StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let content_type = upstream
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| HeaderValue::from_bytes(value.as_bytes()).ok());
    let mut output = Vec::new();
    loop {
        match upstream.chunk().await {
            Ok(Some(chunk)) => {
                if output.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
                    return response(
                        StatusCode::BAD_GATEWAY,
                        "provider upstream response exceeded cap",
                    );
                }
                output.extend_from_slice(&chunk);
            }
            Ok(None) => break,
            Err(e) => {
                return response(
                    StatusCode::BAD_GATEWAY,
                    &format!("provider upstream stream failed: {e}"),
                );
            }
        }
    }
    if state
        .bytes
        .fetch_add(output.len() as u64, Ordering::Relaxed)
        .saturating_add(output.len() as u64)
        > MAX_TOTAL_BYTES
    {
        return response(StatusCode::BAD_GATEWAY, "run broker byte budget exhausted");
    }
    let mut response = Response::new(Body::from(output));
    *response.status_mut() = status;
    if let Some(content_type) = content_type {
        response
            .headers_mut()
            .insert(axum::http::header::CONTENT_TYPE, content_type);
    }
    response
}

fn response(status: StatusCode, message: &str) -> Response<Body> {
    let mut response = Response::new(Body::from(message.to_string()));
    *response.status_mut() = status;
    response
}

// ===== Anthropic Messages broker (Claude Code) ===============================

/// The real upstream, api.anthropic.com. Claude Code posts to
/// `<ANTHROPIC_BASE_URL>/v1/messages`; the broker forwards to this fixed URL
/// (the `?beta=…` query the client adds is a Claude Code convention, not an API
/// input — the beta capability rides the `anthropic-beta` HEADER, forwarded
/// verbatim).
const ANTHROPIC_MESSAGES_URL: &str = "https://api.anthropic.com/v1/messages";

// PENDING live validation — the Anthropic OAuth token endpoint and client id
// are NOT verifiable from this codebase and were not confirmed against a live
// Claude Code login on this box. The refresh STRUCTURE below is complete and
// exercised, but these two constants must be confirmed before the subscription
// refresh path can be trusted. Until then an expired access token with no live
// refresh simply proxies as-is and the client surfaces the upstream 401 (whose
// wording Claude Code's own retry/re-auth already handles).
const ANTHROPIC_OAUTH_TOKEN_URL: &str = "https://console.anthropic.com/v1/oauth/token"; // PENDING live validation
const ANTHROPIC_OAUTH_CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e"; // PENDING live validation

/// refresh this many ms before the stated expiry, to cover clock skew and the
/// round-trip.
const OAUTH_EXPIRY_SKEW_MS: u64 = 60_000;

/// The Anthropic upstream credential — THE SWAPPABLE KNOB (design §ToS). The
/// operator has chosen the subscription path; making the compliant Console
/// API-key path the only one is a one-line change to which arm [`from_host`]
/// returns, and nothing downstream changes. The child never sees any of this.
enum AnthropicAuth {
    /// Console API key → `x-api-key: <key>` upstream. The compliant path.
    ApiKey(String),
    /// Pro/Max subscription OAuth → `Authorization: Bearer <access_token>`,
    /// refreshed when expired. The operator's flagged, chosen path.
    Oauth(OauthTokens),
}

struct OauthTokens {
    access_token: String,
    refresh_token: Option<String>,
    /// unix-ms expiry when known; `None` = unknown (assume valid, upstream 401s
    /// if not — the client re-auths on that).
    expires_at: Option<u64>,
}

impl OauthTokens {
    fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(exp) => now_ms() >= exp.saturating_sub(OAUTH_EXPIRY_SKEW_MS),
            None => false,
        }
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn env_nonempty(key: &str) -> Option<String> {
    std::env::var_os(key)
        .and_then(|v| v.into_string().ok())
        .filter(|v| !v.is_empty())
}

impl AnthropicAuth {
    /// THE knob. Precedence: `ANTHROPIC_API_KEY` (API-key path), else
    /// `CLAUDE_CODE_OAUTH_TOKEN` (OAuth, no refresh material), else the Linux
    /// credentials file `~/.claude/.credentials.json` (OAuth with refresh).
    fn from_host() -> Result<Self, String> {
        if let Some(auth) = Self::from_host_from(
            env_nonempty("ANTHROPIC_API_KEY"),
            env_nonempty("CLAUDE_CODE_OAUTH_TOKEN"),
        ) {
            return Ok(auth);
        }
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from).ok_or_else(|| {
            "Anthropic broker has no ANTHROPIC_API_KEY, no CLAUDE_CODE_OAUTH_TOKEN, and no HOME \
             to read ~/.claude/.credentials.json"
                .to_string()
        })?;
        let path = home.join(".claude").join(".credentials.json");
        let creds: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&path)
                .map_err(|e| format!("read host Claude credentials {}: {e}", path.display()))?,
        )
        .map_err(|e| format!("parse host Claude credentials {}: {e}", path.display()))?;
        // Claude Code (Linux) stores { "claudeAiOauth": { accessToken, refreshToken, expiresAt } }.
        let oauth = creds.get("claudeAiOauth").unwrap_or(&creds);
        let access_token = oauth
            .get("accessToken")
            .and_then(serde_json::Value::as_str)
            .filter(|v| !v.is_empty())
            .ok_or_else(|| {
                format!(
                    "host Claude credentials {} have no claudeAiOauth.accessToken; run `claude` \
                     to log in or set ANTHROPIC_API_KEY",
                    path.display()
                )
            })?
            .to_string();
        let refresh_token = oauth
            .get("refreshToken")
            .and_then(serde_json::Value::as_str)
            .filter(|v| !v.is_empty())
            .map(str::to_string);
        // expiresAt is unix-MS in the credentials file.
        let expires_at = oauth.get("expiresAt").and_then(serde_json::Value::as_u64);
        Ok(Self::Oauth(OauthTokens {
            access_token,
            refresh_token,
            expires_at,
        }))
    }

    /// the env-arm precedence, factored out so the knob's contract is testable
    /// without env mutation. `None` = neither env is set (the caller falls back
    /// to the credentials file).
    fn from_host_from(api_key: Option<String>, oauth_token: Option<String>) -> Option<Self> {
        if let Some(key) = api_key {
            return Some(Self::ApiKey(key));
        }
        oauth_token.map(|token| {
            Self::Oauth(OauthTokens {
                access_token: token,
                refresh_token: None,
                expires_at: None,
            })
        })
    }

    /// stamp the current upstream credential onto a request builder.
    fn authorize(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match self {
            Self::ApiKey(key) => request.header("x-api-key", key),
            Self::Oauth(tokens) => request.bearer_auth(&tokens.access_token),
        }
    }
}

struct AnthropicBrokerState {
    run_bearer: String,
    /// behind a lock because the OAuth path MUTATES it on refresh.
    auth: tokio::sync::Mutex<AnthropicAuth>,
    client: reqwest::Client,
    /// upstream messages URL — the const in production, a mock in tests.
    messages_url: String,
    requests: AtomicU32,
    bytes: AtomicU64,
    concurrent: Arc<Semaphore>,
}

impl AnthropicBrokerState {
    /// subscription-only: refresh an expired access token before proxying. A
    /// no-op for the API-key path, for an unexpired token, or when no refresh
    /// token is held (then the stale token proxies and upstream 401s).
    async fn refresh_if_needed(&self) -> Result<(), String> {
        let mut auth = self.auth.lock().await;
        let AnthropicAuth::Oauth(tokens) = &mut *auth else {
            return Ok(());
        };
        if !tokens.is_expired() {
            return Ok(());
        }
        let Some(refresh_token) = tokens.refresh_token.clone() else {
            return Ok(());
        };
        *tokens = oauth_refresh(&self.client, &refresh_token).await?;
        Ok(())
    }
}

/// Exchange a refresh token for a fresh access token at the Anthropic OAuth
/// endpoint. Endpoint + client id are PENDING live validation (see the
/// constants); the request/response SHAPE is the standard OAuth
/// `grant_type=refresh_token` exchange.
async fn oauth_refresh(
    client: &reqwest::Client,
    refresh_token: &str,
) -> Result<OauthTokens, String> {
    let body = serde_json::json!({
        "grant_type": "refresh_token",
        "refresh_token": refresh_token,
        "client_id": ANTHROPIC_OAUTH_CLIENT_ID,
    });
    let resp = client
        .post(ANTHROPIC_OAUTH_TOKEN_URL)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("anthropic oauth refresh request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "anthropic oauth refresh returned {}",
            resp.status()
        ));
    }
    let v: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("anthropic oauth refresh response was not json: {e}"))?;
    let access_token = v
        .get("access_token")
        .and_then(serde_json::Value::as_str)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| "anthropic oauth refresh response had no access_token".to_string())?
        .to_string();
    let refresh_token = v
        .get("refresh_token")
        .and_then(serde_json::Value::as_str)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
        .or_else(|| Some(refresh_token.to_string()));
    // expires_in is SECONDS from now (standard OAuth), unlike the credentials
    // file's absolute expiresAt in ms.
    let expires_at = v
        .get("expires_in")
        .and_then(serde_json::Value::as_u64)
        .map(|secs| now_ms().saturating_add(secs.saturating_mul(1000)));
    Ok(OauthTokens {
        access_token,
        refresh_token,
        expires_at,
    })
}

impl RunBroker {
    /// start the Anthropic Messages broker for a Direct/Podman run (loopback).
    pub(crate) async fn start_anthropic() -> Result<Self, String> {
        Self::start_anthropic_with(
            AnthropicAuth::from_host()?,
            Reachability::Loopback,
            ANTHROPIC_MESSAGES_URL.into(),
        )
        .await
    }

    /// start it for a Tart guest — bind the host gateway the guest reaches as
    /// `ducktape-host`.
    pub(crate) async fn start_anthropic_for_tart() -> Result<Self, String> {
        Self::start_anthropic_with(
            AnthropicAuth::from_host()?,
            Reachability::HostGateway("ducktape-host"),
            ANTHROPIC_MESSAGES_URL.into(),
        )
        .await
    }

    /// start it for a private-netns Podman container (`host.containers.internal`).
    pub(crate) async fn start_anthropic_for_podman_private() -> Result<Self, String> {
        Self::start_anthropic_with(
            AnthropicAuth::from_host()?,
            Reachability::HostGateway("host.containers.internal"),
            ANTHROPIC_MESSAGES_URL.into(),
        )
        .await
    }

    async fn start_anthropic_with(
        auth: AnthropicAuth,
        reach: Reachability,
        messages_url: String,
    ) -> Result<Self, String> {
        let bind = reach.bind();
        let listener = tokio::net::TcpListener::bind((bind, 0))
            .await
            .map_err(|e| format!("bind run-scoped anthropic broker: {e}"))?;
        let addr = listener
            .local_addr()
            .map_err(|e| format!("read run-scoped anthropic broker address: {e}"))?;
        let mut secret = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut secret);
        let run_bearer = secret
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let state = Arc::new(AnthropicBrokerState {
            run_bearer: run_bearer.clone(),
            auth: tokio::sync::Mutex::new(auth),
            client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|e| format!("build anthropic broker client: {e}"))?,
            messages_url,
            requests: AtomicU32::new(0),
            bytes: AtomicU64::new(0),
            concurrent: Arc::new(Semaphore::new(MAX_CONCURRENT)),
        });
        let app = Router::new()
            // MATCH ON PATH — axum ignores the query string, so `/v1/messages`
            // matches the `?beta=true` Claude Code posts.
            .route("/v1/messages", post(forward_messages))
            // tolerate the client's `HEAD /` reachability probe.
            .route("/", head(probe_ok))
            .fallback(reject)
            .layer(DefaultBodyLimit::max(MAX_REQUEST_BYTES))
            .with_state(state);
        let (shutdown, rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = rx.await;
                })
                .await;
        });
        Ok(Self {
            endpoint: BrokerEndpoint {
                // NO `/v1` suffix: ANTHROPIC_BASE_URL is the API ROOT and Claude
                // Code appends `/v1/messages` itself (unlike codex, whose argv
                // appends `/responses` to a `…/v1` base).
                base_url: reach.base_url(addr, ""),
                run_bearer,
            },
            shutdown: Some(shutdown),
            task,
        })
    }
}

async fn probe_ok() -> StatusCode {
    StatusCode::OK
}

async fn forward_messages(
    State(state): State<Arc<AnthropicBrokerState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response<Body> {
    if !incoming_authorized(&headers, &state.run_bearer) {
        return response(StatusCode::UNAUTHORIZED, "run broker credential rejected");
    }
    if state.requests.fetch_add(1, Ordering::Relaxed) >= MAX_REQUESTS {
        return response(
            StatusCode::TOO_MANY_REQUESTS,
            "run broker request budget exhausted",
        );
    }
    if state
        .bytes
        .fetch_add(body.len() as u64, Ordering::Relaxed)
        .saturating_add(body.len() as u64)
        > MAX_TOTAL_BYTES
    {
        return response(
            StatusCode::PAYLOAD_TOO_LARGE,
            "run broker byte budget exhausted",
        );
    }
    // held for the WHOLE streamed response (moved into the body stream), so a
    // client that opens a second request before this one drains is bounded.
    let Ok(permit) = state.concurrent.clone().try_acquire_owned() else {
        return response(
            StatusCode::TOO_MANY_REQUESTS,
            "run broker concurrency exhausted",
        );
    };

    if let Err(e) = state.refresh_if_needed().await {
        return response(
            StatusCode::BAD_GATEWAY,
            &format!("anthropic token refresh failed: {e}"),
        );
    }

    let mut request = state.client.post(&state.messages_url).body(body);
    // Forward request headers VERBATIM — including `anthropic-version` and
    // `anthropic-beta` (the subscription OAuth capability rides beta; stripping
    // it 401s) — except hop-by-hop framing and the child's credentials, which we
    // replace with the operator's upstream credential.
    for (name, value) in &headers {
        if matches!(
            name.as_str(),
            "authorization"
                | "x-api-key"
                | "host"
                | "content-length"
                | "connection"
                | "transfer-encoding"
        ) {
            continue;
        }
        if let (Ok(name), Ok(value)) = (
            reqwest::header::HeaderName::from_bytes(name.as_str().as_bytes()),
            reqwest::header::HeaderValue::from_bytes(value.as_bytes()),
        ) {
            request = request.header(name, value);
        }
    }
    request = {
        let auth = state.auth.lock().await;
        auth.authorize(request)
    };

    let upstream = match request.send().await {
        Ok(response) => response,
        Err(e) => {
            return response(
                StatusCode::BAD_GATEWAY,
                &format!("anthropic upstream failed: {e}"),
            );
        }
    };
    let status =
        StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    // Pass the upstream content-type through (text/event-stream for SSE). The
    // BODY streams through unbuffered below — buffering would stall the TUI.
    let content_type = upstream
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| HeaderValue::from_bytes(value.as_bytes()).ok());

    // STREAM the upstream body through as a bounded stream. Error bodies flow
    // through this same path unmodified (Claude Code's retry/downgrade matches
    // on the upstream wording).
    let mut seen = 0usize;
    let stream = upstream.bytes_stream().map(move |chunk| {
        // capture the permit for the stream's whole life (freed on stream drop).
        let _keep = &permit;
        match chunk {
            Ok(bytes) => {
                seen = seen.saturating_add(bytes.len());
                if seen > MAX_RESPONSE_BYTES {
                    Err(std::io::Error::other(
                        "run broker response byte budget exhausted",
                    ))
                } else {
                    Ok(bytes)
                }
            }
            Err(e) => Err(std::io::Error::other(e.to_string())),
        }
    });
    let mut response = Response::new(Body::from_stream(stream));
    *response.status_mut() = status;
    if let Some(content_type) = content_type {
        response
            .headers_mut()
            .insert(axum::http::header::CONTENT_TYPE, content_type);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::post;

    #[tokio::test]
    async fn tart_broker_uses_the_guest_nat_hostname() {
        let broker = RunBroker::start_with(
            UpstreamCredential {
                bearer: "unused".into(),
                account_id: None,
                url: "http://127.0.0.1:1/responses".into(),
            },
            Reachability::HostGateway("ducktape-host"),
        )
        .await
        .unwrap();
        assert!(
            broker
                .endpoint
                .base_url
                .starts_with("http://ducktape-host:")
        );
    }

    #[tokio::test]
    async fn broker_is_route_and_run_token_scoped_and_injects_only_host_auth_upstream() {
        let seen = Arc::new(std::sync::Mutex::new(None));
        let seen_handler = seen.clone();
        let upstream = Router::new().route(
            "/responses",
            post(move |headers: HeaderMap| async move {
                *seen_handler.lock().unwrap() = Some(headers);
                (StatusCode::OK, "ok")
            }),
        );
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        let upstream_task = tokio::spawn(async move {
            axum::serve(listener, upstream).await.unwrap();
        });
        let broker = RunBroker::start_with(
            UpstreamCredential {
                bearer: "host-secret-never-in-child".into(),
                account_id: Some("acct-1".into()),
                url: format!("http://{addr}/responses"),
            },
            Reachability::Loopback,
        )
        .await
        .unwrap();
        let client = reqwest::Client::new();
        let endpoint = format!("{}/responses", broker.endpoint.base_url);

        assert_eq!(
            client
                .post(&endpoint)
                .bearer_auth("another-run")
                .body("{}")
                .send()
                .await
                .unwrap()
                .status(),
            reqwest::StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            client
                .get(&endpoint)
                .bearer_auth(&broker.endpoint.run_bearer)
                .send()
                .await
                .unwrap()
                .status(),
            reqwest::StatusCode::METHOD_NOT_ALLOWED
        );
        assert_eq!(
            client
                .post(broker.endpoint.base_url.clone())
                .bearer_auth(&broker.endpoint.run_bearer)
                .body("{}")
                .send()
                .await
                .unwrap()
                .status(),
            reqwest::StatusCode::FORBIDDEN
        );
        assert_eq!(
            client
                .post(&endpoint)
                .bearer_auth(&broker.endpoint.run_bearer)
                .body("{}")
                .send()
                .await
                .unwrap()
                .status(),
            reqwest::StatusCode::OK
        );
        let headers = seen.lock().unwrap().take().unwrap();
        assert_eq!(
            headers["authorization"],
            "Bearer host-secret-never-in-child"
        );
        assert_eq!(headers["chatgpt-account-id"], "acct-1");
        assert_ne!(broker.endpoint.run_bearer, "host-secret-never-in-child");
        upstream_task.abort();
    }

    // ---- Anthropic Messages broker -----------------------------------------

    use std::sync::Mutex;

    /// what a mock upstream captured from one request.
    #[derive(Default)]
    struct SeenReq {
        headers: Option<HeaderMap>,
        path_and_query: Option<String>,
    }

    /// spin up a mock Anthropic upstream that records the request and replies
    /// with `reply` (status, content-type, body). Returns (url, seen, task).
    async fn mock_upstream(
        status: StatusCode,
        content_type: &'static str,
        body: &'static str,
    ) -> (String, Arc<Mutex<SeenReq>>, tokio::task::JoinHandle<()>) {
        let seen = Arc::new(Mutex::new(SeenReq::default()));
        let seen_handler = seen.clone();
        let app = Router::new().route(
            "/v1/messages",
            post(
                move |uri: axum::http::Uri, headers: HeaderMap, _body: Bytes| async move {
                    let mut s = seen_handler.lock().unwrap();
                    s.headers = Some(headers);
                    s.path_and_query =
                        Some(uri.path_and_query().map(|p| p.as_str().to_string()).unwrap_or_default());
                    Response::builder()
                        .status(status)
                        .header(axum::http::header::CONTENT_TYPE, content_type)
                        .body(Body::from(body))
                        .unwrap()
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (format!("http://{addr}/v1/messages"), seen, task)
    }

    async fn start_anthropic_pointed_at(
        auth: AnthropicAuth,
        url: String,
    ) -> RunBroker {
        RunBroker::start_anthropic_with(auth, Reachability::Loopback, url)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn anthropic_rejects_wrong_or_absent_bearer() {
        let (url, _seen, upstream) =
            mock_upstream(StatusCode::OK, "application/json", "{}").await;
        let broker =
            start_anthropic_pointed_at(AnthropicAuth::ApiKey("host-secret".into()), url).await;
        let client = reqwest::Client::new();
        let endpoint = format!("{}/v1/messages", broker.endpoint.base_url);

        // absent authorization → 401.
        assert_eq!(
            client.post(&endpoint).body("{}").send().await.unwrap().status(),
            reqwest::StatusCode::UNAUTHORIZED
        );
        // wrong bearer → 401.
        assert_eq!(
            client
                .post(&endpoint)
                .bearer_auth("not-the-run-bearer")
                .body("{}")
                .send()
                .await
                .unwrap()
                .status(),
            reqwest::StatusCode::UNAUTHORIZED
        );
        upstream.abort();
    }

    #[tokio::test]
    async fn anthropic_path_matches_ignoring_query_and_injects_api_key() {
        let (url, seen, upstream) =
            mock_upstream(StatusCode::OK, "application/json", "{\"ok\":true}").await;
        let broker =
            start_anthropic_pointed_at(AnthropicAuth::ApiKey("host-secret".into()), url).await;
        let client = reqwest::Client::new();
        // Claude Code posts to `/v1/messages?beta=true` — path match, query ignored.
        let endpoint = format!("{}/v1/messages?beta=true", broker.endpoint.base_url);

        let resp = client
            .post(&endpoint)
            .bearer_auth(&broker.endpoint.run_bearer)
            .header("anthropic-version", "2023-06-01")
            .header("anthropic-beta", "oauth-2025-04-20,fine-grained-tool-streaming-2025-05-14")
            .body("{}")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::OK);
        assert_eq!(resp.text().await.unwrap(), "{\"ok\":true}");

        let seen = seen.lock().unwrap();
        let headers = seen.headers.as_ref().unwrap();
        // the run bearer never reaches upstream — the API key does, as x-api-key.
        assert_eq!(headers["x-api-key"], "host-secret");
        assert!(headers.get("authorization").is_none(), "no bearer upstream");
        assert_ne!(broker.endpoint.run_bearer, "host-secret");
        // anthropic-version / anthropic-beta forwarded VERBATIM.
        assert_eq!(headers["anthropic-version"], "2023-06-01");
        assert_eq!(
            headers["anthropic-beta"],
            "oauth-2025-04-20,fine-grained-tool-streaming-2025-05-14"
        );
        upstream.abort();
    }

    #[tokio::test]
    async fn anthropic_oauth_path_sends_bearer_not_x_api_key() {
        let (url, seen, upstream) =
            mock_upstream(StatusCode::OK, "application/json", "{}").await;
        let broker = start_anthropic_pointed_at(
            AnthropicAuth::Oauth(OauthTokens {
                access_token: "sk-oauth-host".into(),
                refresh_token: None,
                expires_at: None,
            }),
            url,
        )
        .await;
        let client = reqwest::Client::new();
        let endpoint = format!("{}/v1/messages", broker.endpoint.base_url);
        client
            .post(&endpoint)
            .bearer_auth(&broker.endpoint.run_bearer)
            .body("{}")
            .send()
            .await
            .unwrap();
        let seen = seen.lock().unwrap();
        let headers = seen.headers.as_ref().unwrap();
        assert_eq!(headers["authorization"], "Bearer sk-oauth-host");
        assert!(headers.get("x-api-key").is_none(), "oauth path sends no x-api-key");
        upstream.abort();
    }

    #[tokio::test]
    async fn anthropic_forwards_error_bodies_unmodified() {
        let body = "{\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\"}}";
        let (url, _seen, upstream) =
            mock_upstream(StatusCode::from_u16(529).unwrap(), "application/json", body).await;
        let broker =
            start_anthropic_pointed_at(AnthropicAuth::ApiKey("host-secret".into()), url).await;
        let client = reqwest::Client::new();
        let endpoint = format!("{}/v1/messages", broker.endpoint.base_url);
        let resp = client
            .post(&endpoint)
            .bearer_auth(&broker.endpoint.run_bearer)
            .body("{}")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 529);
        assert_eq!(resp.text().await.unwrap(), body, "error body verbatim");
        upstream.abort();
    }

    #[tokio::test]
    async fn anthropic_streams_sse_through_in_chunks() {
        // a mock upstream that emits an event-stream body as SEPARATE chunks
        // with a gap — a buffering broker would still reassemble them, but this
        // exercises the streaming path end to end and asserts content-type +
        // reassembled bytes survive.
        let seen = Arc::new(Mutex::new(SeenReq::default()));
        let app: Router = Router::new().route(
            "/v1/messages",
            post(|| async {
                let chunks: Vec<Result<Bytes, std::io::Error>> = vec![
                    Ok(Bytes::from_static(b"event: message_start\ndata: {\"a\":1}\n\n")),
                    Ok(Bytes::from_static(b"event: message_stop\ndata: {\"b\":2}\n\n")),
                ];
                let stream = futures::stream::iter(chunks);
                Response::builder()
                    .status(StatusCode::OK)
                    .header(axum::http::header::CONTENT_TYPE, "text/event-stream")
                    .body(Body::from_stream(stream))
                    .unwrap()
            }),
        );
        let _ = &seen;
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        let upstream = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let broker = start_anthropic_pointed_at(
            AnthropicAuth::ApiKey("host-secret".into()),
            format!("http://{addr}/v1/messages"),
        )
        .await;
        let client = reqwest::Client::new();
        let endpoint = format!("{}/v1/messages", broker.endpoint.base_url);
        let resp = client
            .post(&endpoint)
            .bearer_auth(&broker.endpoint.run_bearer)
            .body("{}")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::OK);
        assert_eq!(
            resp.headers()[reqwest::header::CONTENT_TYPE],
            "text/event-stream",
            "SSE content-type is preserved"
        );
        // reassemble the streamed body chunk by chunk (StreamExt::next).
        use futures::StreamExt as _;
        let mut body = resp.bytes_stream();
        let mut collected = Vec::new();
        while let Some(chunk) = body.next().await {
            collected.extend_from_slice(&chunk.unwrap());
        }
        let text = String::from_utf8(collected).unwrap();
        assert!(text.contains("message_start"), "got {text:?}");
        assert!(text.contains("message_stop"), "got {text:?}");
        upstream.abort();
    }

    #[tokio::test]
    async fn anthropic_tolerates_head_probe() {
        let (url, _seen, upstream) =
            mock_upstream(StatusCode::OK, "application/json", "{}").await;
        let broker =
            start_anthropic_pointed_at(AnthropicAuth::ApiKey("host-secret".into()), url).await;
        let status = reqwest::Client::new()
            .head(&broker.endpoint.base_url)
            .send()
            .await
            .unwrap()
            .status();
        assert_eq!(status, reqwest::StatusCode::OK, "HEAD / is tolerated");
        upstream.abort();
    }

    #[tokio::test]
    async fn anthropic_tart_broker_uses_the_guest_nat_hostname() {
        let broker = RunBroker::start_anthropic_with(
            AnthropicAuth::ApiKey("unused".into()),
            Reachability::HostGateway("ducktape-host"),
            "http://127.0.0.1:1/v1/messages".into(),
        )
        .await
        .unwrap();
        // the guest reaches the host by name, and there is NO /v1 suffix (Claude
        // Code appends /v1/messages to ANTHROPIC_BASE_URL itself).
        assert!(broker.endpoint.base_url.starts_with("http://ducktape-host:"));
        assert!(!broker.endpoint.base_url.ends_with("/v1"));
    }

    #[tokio::test]
    async fn anthropic_private_netns_podman_uses_host_containers_internal() {
        let broker = RunBroker::start_anthropic_with(
            AnthropicAuth::ApiKey("unused".into()),
            Reachability::HostGateway("host.containers.internal"),
            "http://127.0.0.1:1/v1/messages".into(),
        )
        .await
        .unwrap();
        assert!(
            broker
                .endpoint
                .base_url
                .starts_with("http://host.containers.internal:")
        );
    }

    #[test]
    fn anthropic_auth_knob_precedence_is_apikey_then_oauth_token() {
        // env precedence is the swappable knob's contract; assert the two env
        // arms without touching the filesystem fallback.
        assert!(matches!(
            AnthropicAuth::from_host_from(
                Some("sk-console".into()),
                Some("oauth-tok".into()),
            ),
            Some(AnthropicAuth::ApiKey(k)) if k == "sk-console"
        ));
        assert!(matches!(
            AnthropicAuth::from_host_from(None, Some("oauth-tok".into())),
            Some(AnthropicAuth::Oauth(t)) if t.access_token == "oauth-tok"
        ));
        assert!(AnthropicAuth::from_host_from(None, None).is_none());
    }
}
