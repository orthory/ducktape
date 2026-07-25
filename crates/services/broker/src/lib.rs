//! Run-scoped model brokers — one per provider wire shape.
//!
//! The provider child never receives the operator's API/OAuth credential.
//! Only this host process reads it, and serves a single-run loopback endpoint
//! the child dials with an unrelated random bearer. Podman binds
//! loopback; Tart binds the host side of its private NAT so the VM can reach it
//! by a guest-only hostname.
//!
//! Two wire shapes ship: the OpenAI Responses API (`codex exec`, aimed by argv)
//! and the Anthropic Messages API (`claude`, aimed by env — see
//! [`RunBroker::start_anthropic`]). They share the endpoint/bearer/teardown
//! scaffolding and the request/byte caps; they differ in the upstream
//! credential, the route, and — critically — Anthropic STREAMS the SSE response
//! through unbuffered where Codex buffers.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::body::{Body, Bytes, to_bytes};
use axum::extract::{DefaultBodyLimit, Request, State};
use axum::http::{HeaderMap, HeaderValue, Response, StatusCode};
use axum::routing::{head, post};
use futures::StreamExt as _;
use rand::RngCore as _;
use serde::{Deserialize, Serialize};
use tokio::sync::{Semaphore, oneshot, watch};

use airlock::attest::{self, AttestMode, Measurement};
use airlock::verify::{SnpProduct, SnpRoots, TdxRoots, TrustRoots, VcekSource};
use airlock::client::Gateway;

const MAX_REQUEST_BYTES: usize = 8 * 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 32 * 1024 * 1024;
// Lifetime spend guards for ONE broker. A headless run makes a handful of
// requests, but an INTERACTIVE session is long-lived — every user turn is 1+
// requests (plus tool sub-requests, title/compaction calls) — so these must
// bound a whole work session, not a one-shot: at 64 requests an interactive
// session would silently 429 (model access dies) after ~an hour of use. Sized
// for a long session while still capping a runaway that burns the operator's
// subscription; the per-request/-response byte caps + concurrency still hold.
const MAX_TOTAL_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_REQUESTS: u32 = 4096;
const CONTROL_HEADER: &str = "x-ducktape-provider-control";
const MAX_CONTROL_REQUEST_BYTES: usize = 4 * 1024;
const MAX_CONTROL_REQUESTS: usize = 8;
const MAX_CONTROL_REQUEST_ID_BYTES: usize = 64;
const MAX_CONTROL_REQUESTED_SECS: u64 = 30 * 60;
const MAX_CONTROL_CUMULATIVE_SECS: u64 = 2 * 60 * 60;
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

/// The codex broker's upstream credential SOURCE — the mirror of
/// [`AnthropicAuth`] on the OpenAI/Responses wire shape. `Host` is the operator's
/// local Codex credential proxied straight to the provider; `Airlock` is a
/// verified/pinned gateway that HOLDS the credential and is reached with a
/// scoped, sealed session, so the sandbox host never sees the secret — the same
/// execution/auth separation as the claude side.
enum CodexAuth {
    Host(UpstreamCredential),
    Airlock(AirlockSession),
}

impl CodexAuth {
    /// Establish the airlock credential source and return the arm plus the URL
    /// the broker forwards `/v1/responses` to (the gateway, which swaps the
    /// scoped session token for the real credential in-enclave).
    async fn airlock(cfg: AirlockConfig) -> Result<(Self, String), String> {
        let (session, base) = open_airlock_session(cfg).await?;
        let responses_url = format!("{}/v1/responses", base.trim_end_matches('/'));
        Ok((Self::Airlock(session), responses_url))
    }
}

/// Resolve the codex upstream: a per-run airlock gateway when configured, else
/// the operator's local Codex credential → the provider directly. Mirrors
/// [`resolve_anthropic_upstream`], but codex has no env airlock arm — only the
/// explicit per-run config (a self-host resolution) or a host credential — so a
/// codex run never picks up a claude-shaped `DUCKTAPE_AIRLOCK_*` gateway.
async fn resolve_codex_upstream(
    explicit: Option<AirlockConfig>,
) -> Result<(CodexAuth, String), String> {
    if let Some(cfg) = explicit {
        return CodexAuth::airlock(cfg).await;
    }
    let host = UpstreamCredential::from_host()?;
    let responses_url = host.url.clone();
    Ok((CodexAuth::Host(host), responses_url))
}

struct BrokerState {
    run_bearer: String,
    /// behind a lock because the airlock path RE-MINTS the session on a 401.
    auth: tokio::sync::Mutex<CodexAuth>,
    /// where the broker POSTs the responses request: the provider directly on the
    /// host path, the gateway's `/v1/responses` on the airlock path.
    responses_url: String,
    client: reqwest::Client,
    requests: AtomicU32,
    bytes: AtomicU64,
    concurrent: Semaphore,
    idle_control: Arc<IdleControl>,
}

struct IdleControl {
    state: Mutex<IdleControlState>,
}

struct IdleControlState {
    token: String,
    hard_deadline: Option<tokio::time::Instant>,
    deadline: Option<watch::Sender<Option<tokio::time::Instant>>>,
    requests: BTreeMap<String, StoredDecision>,
    cumulative_secs: u64,
    limit_logged: bool,
}

#[derive(Clone)]
struct StoredDecision {
    requested_secs: u64,
    status: StatusCode,
    reply: IdleControlReply,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdleControlRequest {
    request_id: String,
    requested_secs: u64,
}

#[derive(Clone, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum IdleControlReply {
    Granted {
        request_id: String,
        requested_secs: u64,
        granted_secs: u64,
        effective_idle_secs: u64,
        hard_cap_truncated: bool,
    },
    Denied {
        #[serde(skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
        reason: &'static str,
    },
}

pub struct BrokerInvocation {
    pub endpoint: BrokerEndpoint,
    pub idle_deadline: watch::Receiver<Option<tokio::time::Instant>>,
    idle_control: Arc<IdleControl>,
}

impl BrokerInvocation {
    pub fn arm(&self, hard_deadline: tokio::time::Instant) {
        let mut state = self.idle_control.state.lock().unwrap();
        if state.token == self.endpoint.control_token {
            state.hard_deadline = Some(hard_deadline);
        }
    }

    pub fn revoke(&self) {
        let mut state = self.idle_control.state.lock().unwrap();
        if state.token == self.endpoint.control_token {
            revoke_idle_control(&mut state);
        }
    }

    /// Linearize a provisional timer wake against control grants. If a grant
    /// won the mutex first, its watched deadline is observed and the child
    /// continues. Otherwise expiry revokes the token before a later request
    /// can be reported as granted.
    pub fn continue_after_timeout_wake(
        &self,
        last_activity: tokio::time::Instant,
        idle: Duration,
        hard: tokio::time::Instant,
        now: tokio::time::Instant,
    ) -> bool {
        let mut state = self.idle_control.state.lock().unwrap();
        if state.token != self.endpoint.control_token {
            return false;
        }
        let explicit = state
            .deadline
            .as_ref()
            .and_then(|deadline| *deadline.borrow());
        let refreshed = (last_activity + idle)
            .max(explicit.unwrap_or(last_activity + idle))
            .min(hard);
        if now < refreshed {
            return true;
        }
        revoke_idle_control(&mut state);
        false
    }
}

impl Drop for BrokerInvocation {
    fn drop(&mut self) {
        self.revoke();
    }
}

fn revoke_idle_control(state: &mut IdleControlState) {
    state.token.clear();
    state.hard_deadline = None;
    state.deadline = None;
    state.requests.clear();
    state.cumulative_secs = 0;
}

/// The only information that crosses into the provider child. None of these
/// values can recover the host credential; all die with this run's broker.
pub struct BrokerEndpoint {
    pub base_url: String,
    pub run_bearer: String,
    pub control_url: String,
    pub control_token: String,
}

/// how the provider child reaches this run's broker — which drives BOTH the
/// bind address and the `base_url` the child is handed.
///
/// `Loopback` is a same-netns child that shares the host's loopback: only the
/// test-only Bare host, which runs the executor directly with no container.
/// bind `127.0.0.1`, hand it `http://127.0.0.1:<port>`.
///
/// `HostGateway(host)` is a child in a SEPARATE netns that reaches the host only
/// through a gateway name in its `/etc/hosts` — a Tart VM guest (`ducktape-host`)
/// or a private-netns Podman container (`host.containers.internal`, which every
/// Podman run now uses). The broker binds a routable interface and the base_url
/// names the gateway. The opaque per-run bearer still gates it; binding beyond
/// loopback is the reachability cost of the stronger network isolation.
#[derive(Clone, Copy)]
pub enum Reachability {
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

pub struct RunBroker {
    pub endpoint: BrokerEndpoint,
    idle_control: Arc<IdleControl>,
    shutdown: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
}

impl RunBroker {
    /// `airlock` is the per-run credential source (a self-host resolution); when
    /// `None` the operator's local Codex credential proxies to the provider.
    pub async fn start(airlock: Option<AirlockConfig>) -> Result<Self, String> {
        let (auth, url) = resolve_codex_upstream(airlock).await?;
        Self::start_codex(auth, url, Reachability::Loopback).await
    }

    pub async fn start_for_tart(airlock: Option<AirlockConfig>) -> Result<Self, String> {
        let (auth, url) = resolve_codex_upstream(airlock).await?;
        Self::start_codex(auth, url, Reachability::HostGateway("ducktape-host")).await
    }

    /// a private-netns Podman container reaches the loopback host only via the
    /// `host.containers.internal` gateway podman adds to its `/etc/hosts`.
    pub async fn start_for_podman_private(
        airlock: Option<AirlockConfig>,
    ) -> Result<Self, String> {
        let (auth, url) = resolve_codex_upstream(airlock).await?;
        Self::start_codex(auth, url, Reachability::HostGateway("host.containers.internal")).await
    }

    /// Test-only: a broker whose upstream is a dead port. `testkit` exposes it
    /// to the consumer crates' tests too (capability-host drives a run against
    /// it); it is compiled OUT of any build that doesn't ask for the feature.
    #[cfg(any(test, feature = "testkit"))]
    pub async fn start_for_test() -> Self {
        Self::start_with(
            UpstreamCredential {
                bearer: "unused".into(),
                account_id: None,
                url: "http://127.0.0.1:1/responses".into(),
            },
            Reachability::Loopback,
        )
        .await
        .unwrap()
    }

    /// host-path convenience for tests: wrap a literal credential and serve. The
    /// live path goes through [`Self::start`]/[`resolve_codex_upstream`].
    #[cfg(any(test, feature = "testkit"))]
    async fn start_with(upstream: UpstreamCredential, reach: Reachability) -> Result<Self, String> {
        let responses_url = upstream.url.clone();
        Self::start_codex(CodexAuth::Host(upstream), responses_url, reach).await
    }

    async fn start_codex(
        auth: CodexAuth,
        responses_url: String,
        reach: Reachability,
    ) -> Result<Self, String> {
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
        let idle_control = Arc::new(IdleControl {
            state: Mutex::new(IdleControlState {
                token: String::new(),
                hard_deadline: None,
                deadline: None,
                requests: BTreeMap::new(),
                cumulative_secs: 0,
                limit_logged: false,
            }),
        });
        let state = Arc::new(BrokerState {
            run_bearer: run_bearer.clone(),
            auth: tokio::sync::Mutex::new(auth),
            responses_url,
            client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|e| format!("build provider broker client: {e}"))?,
            requests: AtomicU32::new(0),
            bytes: AtomicU64::new(0),
            concurrent: Semaphore::new(1),
            idle_control: idle_control.clone(),
        });
        let app = Router::new()
            .route("/v1/responses", post(forward_responses))
            .route("/v1/control/provider-idle", post(provider_idle_control))
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
            // the base endpoint carries only base_url + run_bearer; the control
            // url/token are minted PER-INVOCATION by begin_invocation (rotated),
            // so they are empty placeholders here.
            endpoint: BrokerEndpoint {
                base_url: reach.base_url(addr, "/v1"),
                run_bearer,
                control_url: String::new(),
                control_token: String::new(),
            },
            idle_control,
            shutdown: Some(shutdown),
            task,
        })
    }

    /// Arm a fresh control credential and deadline channel for one child
    /// invocation. A resume fallback rotates the credential before spawning
    /// its replacement child, so an old MCP process cannot control the new one.
    pub fn begin_invocation(&self) -> BrokerInvocation {
        let control_token = random_token();
        let (deadline, idle_deadline) = watch::channel(None);
        *self.idle_control.state.lock().unwrap() = IdleControlState {
            token: control_token.clone(),
            hard_deadline: None,
            deadline: Some(deadline),
            requests: BTreeMap::new(),
            cumulative_secs: 0,
            limit_logged: false,
        };
        BrokerInvocation {
            endpoint: BrokerEndpoint {
                base_url: self.endpoint.base_url.clone(),
                run_bearer: self.endpoint.run_bearer.clone(),
                control_url: format!("{}/control/provider-idle", self.endpoint.base_url),
                control_token,
            },
            idle_deadline,
            idle_control: self.idle_control.clone(),
        }
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

fn random_token() -> String {
    let mut secret = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut secret);
    secret.iter().map(|byte| format!("{byte:02x}")).collect()
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

    let (mut upstream, mut binding) = match send_codex(&state, &headers, &body).await {
        Ok(sent) => sent,
        Err(e) => {
            return response(
                StatusCode::BAD_GATEWAY,
                &format!("provider upstream failed: {e}"),
            );
        }
    };
    // Airlock only: a gateway 401 means the scoped session token's TTL lapsed.
    // Re-handshake once and retry. Host runs pass straight through (reauth is a
    // no-op and returns false).
    let token_expired = upstream.status() == StatusCode::UNAUTHORIZED;
    if token_expired && codex_airlock_reauth(&state).await {
        (upstream, binding) = match send_codex(&state, &headers, &body).await {
            Ok(sent) => sent,
            Err(e) => {
                return response(
                    StatusCode::BAD_GATEWAY,
                    &format!("provider upstream failed: {e}"),
                );
            }
        };
    }
    let status =
        StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let content_type = upstream
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| HeaderValue::from_bytes(value.as_bytes()).ok());
    let content_type_str = upstream
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
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
    // Airlock sealed session: the enclave's success body is an opaque sealed
    // stream — unseal it to the plaintext the unmodified codex sandbox expects.
    // A gateway ERROR body (minted before the proxy path) is plaintext and
    // relays as-is; a plaintext SUCCESS on a sealed session can only be a path
    // host forging one, so it is refused.
    let seal_keys = {
        let auth = state.auth.lock().await;
        match &*auth {
            CodexAuth::Airlock(session) => Some(session.keys.clone()),
            CodexAuth::Host(_) => None,
        }
    };
    if let Some(keys) = seal_keys {
        let sealed_outer = content_type_str
            .as_deref()
            .is_some_and(|ct| ct.starts_with("application/octet-stream"));
        if sealed_outer {
            return match open_sealed_buffer(&keys, &binding, &output) {
                Ok((inner_ct, plain)) => codex_body(status, inner_ct, plain),
                Err(e) => response(StatusCode::BAD_GATEWAY, &format!("airlock: {e}")),
            };
        }
        if status.is_success() {
            return response(
                StatusCode::BAD_GATEWAY,
                "airlock: sealed session received a plaintext success body",
            );
        }
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

/// Build the codex upstream request (target URL + forwarded headers + the
/// current credential) and send it, WITHOUT consuming the body — so a gateway
/// 401 can be retried after an airlock re-handshake. Returns the response plus
/// the sealed request's BINDING (empty when unsealed) — the response is opened
/// under it.
async fn send_codex(
    state: &BrokerState,
    headers: &HeaderMap,
    body: &Bytes,
) -> reqwest::Result<(reqwest::Response, Vec<u8>)> {
    let mut request = state.client.post(&state.responses_url);
    // Match the official Codex responses-api-proxy posture: preserve Codex's
    // protocol/version/session headers, but replace auth and hop-by-hop framing.
    // The overlay routing headers are OURS to set on the airlock path, never the
    // child's (see the anthropic broker's note) — drop any it injected.
    for (name, value) in headers {
        if matches!(
            name.as_str(),
            "authorization"
                | "host"
                | "content-length"
                | "connection"
                | "transfer-encoding"
                | "x-duck-authority"
                | "origin"
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
    let (request, binding) = {
        let auth = state.auth.lock().await;
        match &*auth {
            CodexAuth::Host(host) => {
                let mut request = request.bearer_auth(&host.bearer).body(body.clone());
                if let Some(account_id) = &host.account_id {
                    request = request.header("ChatGPT-Account-ID", account_id);
                }
                (request, Vec::new())
            }
            // Sealed-body airlock session: encrypt under the handshake body key
            // (fresh nonce per attempt, so the 401-retry re-seals safely) and
            // carry the scoped session token; the enclave refuses plaintext.
            CodexAuth::Airlock(session) => {
                let sealed = airlock::bodyseal::seal_request(&session.keys, body);
                let binding = airlock::bodyseal::request_binding(&sealed);
                let request = request
                    .body(sealed)
                    .header(airlock::bodyseal::SEAL_HEADER, airlock::bodyseal::SEAL_V1);
                (session.gateway.route(request.bearer_auth(&session.token)), binding)
            }
        }
    };
    Ok((request.send().await?, binding))
}

/// Airlock only: re-mint the scoped session against the already-trusted seal key
/// after a gateway 401. `true` iff it re-handshook (the caller retries once);
/// the host arm and a failed handshake return `false`.
async fn codex_airlock_reauth(state: &BrokerState) -> bool {
    let mut auth = state.auth.lock().await;
    let CodexAuth::Airlock(session) = &mut *auth else {
        return false;
    };
    match session.gateway.open_session_sealed(&session.seal_pk, &session.sub).await {
        Ok((token, keys)) => {
            session.token = token;
            session.keys = keys;
            true
        }
        Err(_) => {
            tracing::debug!(
                target: "ducktape::agent",
                event = "airlock_reauth",
                reason = "handshake_failed",
                "airlock session re-handshake failed"
            );
            false
        }
    }
}

/// Open a fully-buffered sealed response stream: feed every byte to the opener,
/// require the authenticated Final marker, and return the inner content-type and
/// concatenated plaintext. A stream that ends without Final is truncation, not a
/// clean EOF — refused.
fn open_sealed_buffer(
    keys: &airlock::handshake::SessionKeys,
    binding: &[u8],
    sealed: &[u8],
) -> Result<(Option<String>, Vec<u8>), String> {
    use airlock::bodyseal::OpenedItem;
    let mut opener = airlock::bodyseal::StreamOpener::new(keys, binding);
    let items = opener.feed(sealed).map_err(|e| e.to_string())?;
    if !opener.finished() {
        return Err("sealed response truncated".into());
    }
    let mut inner_ct = None;
    let mut out = Vec::new();
    for item in items {
        match item {
            OpenedItem::Head(ct) => inner_ct = Some(ct),
            OpenedItem::Data(data) => out.extend_from_slice(&data),
            OpenedItem::Final => {}
        }
    }
    Ok((inner_ct, out))
}

/// Assemble the buffered codex response the sandbox sees, tagged with the inner
/// content-type recovered from the sealed head.
fn codex_body(status: StatusCode, inner_ct: Option<String>, body: Vec<u8>) -> Response<Body> {
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = status;
    if let Some(value) = inner_ct.and_then(|ct| HeaderValue::from_str(&ct).ok()) {
        response.headers_mut().insert(axum::http::header::CONTENT_TYPE, value);
    }
    response
}

async fn provider_idle_control(
    State(state): State<Arc<BrokerState>>,
    headers: HeaderMap,
    request: Request,
) -> Response<Body> {
    let token = headers
        .get(CONTROL_HEADER)
        .and_then(|value| value.to_str().ok());
    {
        let control = state.idle_control.state.lock().unwrap();
        if control.token.is_empty() || token != Some(control.token.as_str()) {
            return json_response(
                StatusCode::UNAUTHORIZED,
                &IdleControlReply::Denied {
                    request_id: None,
                    reason: "unauthorized",
                },
            );
        }
    }

    let body = match to_bytes(request.into_body(), MAX_CONTROL_REQUEST_BYTES).await {
        Ok(body) => body,
        Err(_) => {
            return json_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                &IdleControlReply::Denied {
                    request_id: None,
                    reason: "invalid_body",
                },
            );
        }
    };
    let request: IdleControlRequest = match serde_json::from_slice(&body) {
        Ok(request) => request,
        Err(_) => {
            return json_response(
                StatusCode::BAD_REQUEST,
                &IdleControlReply::Denied {
                    request_id: None,
                    reason: "invalid_body",
                },
            );
        }
    };
    let (status, reply, log) = state.idle_control.decide(token, request);
    if log.is_some() {
        match &reply {
            IdleControlReply::Granted {
                request_id,
                requested_secs,
                hard_cap_truncated,
                ..
            } => tracing::debug!(
                target: "ducktape::agent",
                event = "provider_idle_control",
                request_id = request_id.as_str(),
                requested_secs = *requested_secs,
                status = "granted",
                hard_cap_truncated = *hard_cap_truncated,
                "provider idle control"
            ),
            IdleControlReply::Denied { request_id, reason } => tracing::warn!(
                target: "ducktape::agent",
                event = "provider_idle_control",
                request_id = request_id.as_deref().unwrap_or_default(),
                status = "denied",
                reason = *reason,
                "provider idle control denied"
            ),
        }
    }
    json_response(status, &reply)
}

impl IdleControl {
    fn decide(
        &self,
        token: Option<&str>,
        request: IdleControlRequest,
    ) -> (StatusCode, IdleControlReply, Option<String>) {
        let mut state = self.state.lock().unwrap();
        if state.token.is_empty() || token != Some(state.token.as_str()) {
            return (
                StatusCode::UNAUTHORIZED,
                IdleControlReply::Denied {
                    request_id: None,
                    reason: "unauthorized",
                },
                None,
            );
        }
        if !valid_request_id(&request.request_id) {
            return (
                StatusCode::BAD_REQUEST,
                IdleControlReply::Denied {
                    request_id: None,
                    reason: "invalid_request_id",
                },
                None,
            );
        }
        if request.requested_secs == 0 || request.requested_secs > MAX_CONTROL_REQUESTED_SECS {
            return denied(
                &request.request_id,
                "invalid_requested_secs",
                StatusCode::BAD_REQUEST,
            );
        }

        if let Some(previous) = state.requests.get(&request.request_id) {
            if previous.requested_secs == request.requested_secs {
                return (previous.status, previous.reply.clone(), None);
            }
            return denied(
                &request.request_id,
                "request_id_conflict",
                StatusCode::CONFLICT,
            );
        }
        if state.requests.len() >= MAX_CONTROL_REQUESTS {
            let log = (!state.limit_logged).then(|| {
                state.limit_logged = true;
                "status=denied reason=request_limit_exhausted".to_string()
            });
            return (
                StatusCode::TOO_MANY_REQUESTS,
                IdleControlReply::Denied {
                    request_id: Some(request.request_id),
                    reason: "request_limit_exhausted",
                },
                log,
            );
        }

        let Some(hard_deadline) = state.hard_deadline else {
            return denied(&request.request_id, "inactive", StatusCode::CONFLICT);
        };
        let now = tokio::time::Instant::now();
        if now >= hard_deadline {
            return store_denial(
                &mut state,
                request,
                "hard_cap_reached",
                StatusCode::CONFLICT,
            );
        }
        let Some(cumulative_secs) = state.cumulative_secs.checked_add(request.requested_secs)
        else {
            return store_denial(
                &mut state,
                request,
                "cumulative_limit_exhausted",
                StatusCode::TOO_MANY_REQUESTS,
            );
        };
        if cumulative_secs > MAX_CONTROL_CUMULATIVE_SECS {
            return store_denial(
                &mut state,
                request,
                "cumulative_limit_exhausted",
                StatusCode::TOO_MANY_REQUESTS,
            );
        }

        let requested = Duration::from_secs(request.requested_secs);
        let remaining_to_hard = hard_deadline.saturating_duration_since(now);
        let hard_cap_truncated = requested > remaining_to_hard;
        let candidate = now + requested.min(remaining_to_hard);
        let current = state
            .deadline
            .as_ref()
            .and_then(|deadline| *deadline.borrow());
        let installed = current.map_or(candidate, |deadline| deadline.max(candidate));
        if let Some(deadline) = &state.deadline {
            deadline.send_replace(Some(installed));
        }
        state.cumulative_secs = cumulative_secs;
        let reply = IdleControlReply::Granted {
            request_id: request.request_id.clone(),
            requested_secs: request.requested_secs,
            granted_secs: duration_secs_ceil(candidate.saturating_duration_since(now)),
            effective_idle_secs: duration_secs_ceil(installed.saturating_duration_since(now)),
            hard_cap_truncated,
        };
        state.requests.insert(
            request.request_id.clone(),
            StoredDecision {
                requested_secs: request.requested_secs,
                status: StatusCode::OK,
                reply: reply.clone(),
            },
        );
        let log = Some(format!(
            "request_id={} requested_secs={} status=granted hard_cap_truncated={hard_cap_truncated}",
            request.request_id, request.requested_secs
        ));
        (StatusCode::OK, reply, log)
    }
}

fn denied(
    request_id: &str,
    reason: &'static str,
    status: StatusCode,
) -> (StatusCode, IdleControlReply, Option<String>) {
    (
        status,
        IdleControlReply::Denied {
            request_id: Some(request_id.to_string()),
            reason,
        },
        None,
    )
}

fn store_denial(
    state: &mut IdleControlState,
    request: IdleControlRequest,
    reason: &'static str,
    status: StatusCode,
) -> (StatusCode, IdleControlReply, Option<String>) {
    let reply = IdleControlReply::Denied {
        request_id: Some(request.request_id.clone()),
        reason,
    };
    state.requests.insert(
        request.request_id.clone(),
        StoredDecision {
            requested_secs: request.requested_secs,
            status,
            reply: reply.clone(),
        },
    );
    let log = Some(format!(
        "request_id={} requested_secs={} status=denied reason={reason}",
        request.request_id, request.requested_secs
    ));
    (status, reply, log)
}

fn valid_request_id(request_id: &str) -> bool {
    !request_id.is_empty()
        && request_id.len() <= MAX_CONTROL_REQUEST_ID_BYTES
        && request_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.:".contains(&byte))
}

fn duration_secs_ceil(duration: Duration) -> u64 {
    duration
        .as_secs()
        .saturating_add(u64::from(duration.subsec_nanos() != 0))
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
    /// A verified airlock TEE gateway is the credential SOURCE — the broker holds
    /// NO Anthropic credential. It forwards each request to the gateway carrying a
    /// scoped session token, and the gateway swaps the token for the real
    /// credential inside its enclave. This is execution/auth separation: the host
    /// operator running the sandbox cannot read the credential. Selected by
    /// `DUCKTAPE_AIRLOCK_*` env; local or remote-overlay topology (see
    /// [`AirlockGateway`]).
    Airlock(AirlockSession),
}

/// A verified handshake with an airlock gateway. `seal_pk` is cached from the
/// attested quote so a re-handshake needs no re-verify; `token` is the current
/// scoped session token, re-minted on a gateway 401 (its TTL lapsed).
struct AirlockSession {
    gateway: Gateway,
    seal_pk: [u8; 32],
    sub: String,
    token: String,
    /// Handshake keys for the body AEAD (`bodyseal`): requests are sealed,
    /// responses unsealed, so path hosts (incl. the publisher node outside the
    /// enclave) see only ciphertext and a stolen bearer alone is useless.
    keys: airlock::handshake::SessionKeys,
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
            // the scoped session token, plus `x-duck-authority` on the remote
            // topology so the local node's browser-gateway routes it onto the
            // overlay (a no-op locally).
            Self::Airlock(session) => {
                session.gateway.route(request.bearer_auth(&session.token))
            }
        }
    }

    /// Establish the airlock credential source: verify the gateway quote, read
    /// the attested seal key, and handshake for the initial session token.
    /// Returns the `Airlock` arm and the URL the broker forwards `/v1/messages`
    /// to — the gateway itself, which swaps the token for the real credential.
    async fn airlock(cfg: AirlockConfig) -> Result<(Self, String), String> {
        let (session, base) = open_airlock_session(cfg).await?;
        let messages_url = format!("{}/v1/messages", base.trim_end_matches('/'));
        Ok((Self::Airlock(session), messages_url))
    }
}

/// Shared airlock bring-up for both broker wire shapes: resolve the gateway
/// handle, ESTABLISH TRUST (verify a TEE quote, or pin the on-chain seal_pk on
/// the self-host path), then open ONE sealed session. Returns the session plus
/// the base URL the broker forwards to (the gateway itself locally, or the
/// browser-gateway that routes the overlay hop remotely).
async fn open_airlock_session(cfg: AirlockConfig) -> Result<(AirlockSession, String), String> {
    let (gateway, base) = match &cfg.gateway {
        AirlockGateway::Local { url } => (Gateway::local(url.clone()), url.clone()),
        AirlockGateway::Remote { handle, via } => {
            (Gateway::remote(handle.clone(), via.clone()), via.clone())
        }
    };
    // ONE discriminant: how the seal key is trusted. `Attested` fetches and
    // verifies the TEE quote and reads the seal_pk out of the attested
    // REPORTDATA; `PinnedSealPk` is the self-host anchor — the on-chain seal_pk
    // pinned directly, no quote to verify.
    let (seal_pk, pinned) = match &cfg.trust {
        AirlockTrust::Attested { measurement, attest } => {
            let mode: AttestMode =
                attest.parse().map_err(|e| format!("airlock attest mode: {e}"))?;
            let expected = Measurement::from_hex(measurement)
                .map_err(|e| format!("airlock measurement: {e}"))?;
            (verify_gateway(&gateway, &cfg, mode, &expected).await?, false)
        }
        AirlockTrust::PinnedSealPk(pk) => (*pk, true),
    };
    // A lending session names the account it draws the grant on behalf of, so the
    // owner's gateway can check it against the on-chain record and 403 an
    // ungranted account (`credential_not_granted`). `None` on the env/Attested
    // path, which runs no grant gate — the account is then simply unsent.
    let handshake = match &cfg.account {
        Some(account) => gateway.open_session_sealed_as(&seal_pk, &cfg.sub, account).await,
        None => gateway.open_session_sealed(&seal_pk, &cfg.sub).await,
    };
    // On the pinned path a handshake failure IS the trust failure: the gateway's
    // real seal key differs from the pin, so the sealed session token cannot be
    // opened. Surface it as a named error BEFORE any credentialed request.
    let (token, keys) = handshake.map_err(|e| match pinned {
        true => "gateway_seal_pk_mismatch".to_string(),
        false => format!("airlock handshake: {e}"),
    })?;
    Ok((AirlockSession { gateway, seal_pk, sub: cfg.sub, token, keys }, base))
}

/// Where the airlock gateway lives.
#[derive(Debug, Clone, PartialEq, Eq)]
enum AirlockGateway {
    /// Same machine (Credential Provider == Computation Provider): a loopback URL.
    Local { url: String },
    /// A remote node reached by duckdns `handle` through `via` (the local node's
    /// browser-gateway URL), which routes `x-duck-authority` onto the overlay.
    Remote { handle: String, via: String },
}

/// capability-host's OWN vendor tag for a credential — a MIRROR of the gateway
/// module's `CredentialKind`. Kept separate on purpose: this crate must not
/// depend on the gateway module crate, so the node maps between the two at the
/// boundary when it resolves a record into a [`ResolvedCredential`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialKind {
    Claude,
    Codex,
}

/// A credential name resolved from consensus (the on-chain gateway record) into
/// everything the broker needs to reach the owner's self-host gateway: WHERE it
/// is (the `authority` duckdns handle, reached through the local node's browser
/// gateway `via`), WHAT it is (`kind`), and the seal_pk the broker pins as its
/// trust anchor — there is no TEE quote in self-host, so the on-chain key is the
/// anchor.
pub struct ResolvedCredential {
    pub name: String,
    pub kind: CredentialKind,
    pub authority: String,
    pub via: String,
    pub seal_pk: [u8; 32],
    /// the ACCOUNT the run acts on behalf of — the credential-grant subject the
    /// owner's gateway checks the session against (the owner itself, or a granted
    /// account). Sent as the sealed session's `account_b64`; without it a
    /// grant-gated gateway refuses every session `credential_not_granted`.
    pub account: Vec<u8>,
}

/// How the broker decides to trust a gateway's seal key — the one discriminant
/// [`AnthropicAuth::airlock`] branches on. `Attested` verifies a TEE quote and
/// reads the seal_pk out of the attested REPORTDATA; `PinnedSealPk` is the
/// self-host anchor: the seal_pk published on consensus, pinned directly, with
/// no quote to verify.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AirlockTrust {
    Attested { measurement: String, attest: String },
    PinnedSealPk([u8; 32]),
}

/// Opt-in airlock credential-source config. Either read from the environment
/// ([`from_env`], which builds [`AirlockTrust::Attested`]) or constructed from a
/// consensus-resolved credential ([`self_host`], which pins the on-chain
/// seal_pk). Absent on the default broker path (a host-held Anthropic credential
/// → api.anthropic.com), which is unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AirlockConfig {
    gateway: AirlockGateway,
    trust: AirlockTrust,
    sub: String,
    /// self-host lending: the grant subject the session claims to act on behalf
    /// of, sent as `account_b64` so the owner's grant-gated gateway can enforce
    /// the grant. `None` on the env/Attested path (no grant gate).
    account: Option<Vec<u8>>,
    /// attest=snp: the pinned AMD platform generation (parsed at config time).
    snp_product: Option<SnpProduct>,
    /// attest=snp: an out-of-band VCEK (file READ at config time); KDS otherwise.
    snp_vcek: Option<VcekSource>,
    /// attest=tdx: collateral endpoint override; Intel PCS otherwise.
    pccs_url: Option<String>,
}

impl AirlockConfig {
    /// Build a self-host airlock config from a consensus-resolved credential:
    /// reach the owner's gateway over the overlay (`authority` through `via`),
    /// draw on the named credential (`sub` = the credential name), and PIN its
    /// on-chain seal_pk as the trust anchor. No env is read on this path.
    pub fn self_host(resolved: &ResolvedCredential) -> AirlockConfig {
        AirlockConfig {
            gateway: AirlockGateway::Remote {
                handle: resolved.authority.clone(),
                via: resolved.via.clone(),
            },
            trust: AirlockTrust::PinnedSealPk(resolved.seal_pk),
            sub: resolved.name.clone(),
            account: Some(resolved.account.clone()),
            snp_product: None,
            snp_vcek: None,
            pccs_url: None,
        }
    }

    /// `DUCKTAPE_AIRLOCK_GATEWAY=<url>` (local), or `DUCKTAPE_AIRLOCK_REMOTE=<handle>.duck`
    /// with `DUCKTAPE_AIRLOCK_VIA=<url>` (remote), turns airlock on;
    /// `DUCKTAPE_AIRLOCK_MEASUREMENT` (hex) and `DUCKTAPE_AIRLOCK_ATTEST` are then
    /// required — the latter has NO default, so nobody silently gets forgeable
    /// mock attestation while believing they pinned a TEE. `Some(Err)` = airlock
    /// is enabled but misconfigured (fail the run, don't silently fall back to a
    /// host credential); `None` = airlock is off.
    fn from_env() -> Option<Result<Self, String>> {
        let gateway = match (
            env_nonempty("DUCKTAPE_AIRLOCK_GATEWAY"),
            env_nonempty("DUCKTAPE_AIRLOCK_REMOTE"),
        ) {
            (Some(url), _) => AirlockGateway::Local { url },
            (None, Some(handle)) => match env_nonempty("DUCKTAPE_AIRLOCK_VIA") {
                Some(via) => AirlockGateway::Remote { handle, via },
                None => {
                    return Some(Err("DUCKTAPE_AIRLOCK_REMOTE requires DUCKTAPE_AIRLOCK_VIA \
                                     (the local node's browser-gateway URL)"
                        .into()));
                }
            },
            (None, None) => return None,
        };
        let Some(measurement) = env_nonempty("DUCKTAPE_AIRLOCK_MEASUREMENT") else {
            return Some(Err("airlock is enabled but DUCKTAPE_AIRLOCK_MEASUREMENT \
                             (the audited-image measurement hex) is not set"
                .into()));
        };
        let Some(attest) = env_nonempty("DUCKTAPE_AIRLOCK_ATTEST") else {
            return Some(Err(
                "airlock is enabled but DUCKTAPE_AIRLOCK_ATTEST is not set ('tdx'/'snp')".into(),
            ));
        };
        // Typed at the boundary: THIS is the one place `DUCKTAPE_AIRLOCK_*`
        // env is read, so misconfig fails here, not mid-verify.
        let snp_product = match env_nonempty("DUCKTAPE_AIRLOCK_SNP_PRODUCT") {
            Some(p) => match p.parse::<SnpProduct>() {
                Ok(p) => Some(p),
                Err(e) => return Some(Err(format!("airlock SNP product: {e}"))),
            },
            None => None,
        };
        let snp_vcek = match env_nonempty("DUCKTAPE_AIRLOCK_SNP_VCEK") {
            Some(path) => match std::fs::read(&path) {
                Ok(der) => Some(VcekSource::Der(der)),
                Err(e) => return Some(Err(format!("airlock read DUCKTAPE_AIRLOCK_SNP_VCEK: {e}"))),
            },
            None => None,
        };
        Some(Ok(Self {
            gateway,
            trust: AirlockTrust::Attested { measurement, attest },
            sub: env_nonempty("DUCKTAPE_AIRLOCK_SUB").unwrap_or_else(|| "compute-provider".into()),
            // the boundary/TEE path runs no grant gate; the account is unsent.
            account: None,
            snp_product,
            snp_vcek,
            pccs_url: env_nonempty("DUCKTAPE_AIRLOCK_PCCS_URL"),
        }))
    }
}

/// Fetch + verify the gateway quote and return the attested seal key, via the
/// real vendor verifier (`airlock::verify`) against pinned Intel/AMD roots.
async fn verify_gateway(
    gateway: &Gateway,
    cfg: &AirlockConfig,
    mode: AttestMode,
    expected: &Measurement,
) -> Result<[u8; 32], String> {
    let (quote, _vendor) = gateway
        .fetch_quote()
        .await
        .map_err(|e| format!("airlock fetch quote: {e}"))?;
    let roots = trust_roots(cfg, mode)?;
    let report_data = airlock::verify::verify_quote(&quote, expected, &roots)
        .await
        .map_err(|e| format!("airlock verify: {e}"))?;
    Ok(attest::split_report_data(&report_data).0)
}

/// Production: pinned roots assembled from the ALREADY-PARSED typed config
/// (the Intel root lives inside dcap-qvl, the AMD ARK/ASK inside the sev
/// builtins — nothing here can swap a trust anchor). Tests: an injected
/// override, compiled OUT of non-test builds, so an in-process test enclave
/// is verified through the real verify path.
fn trust_roots(cfg: &AirlockConfig, mode: AttestMode) -> Result<TrustRoots, String> {
    #[cfg(test)]
    if let Some(roots) = test_trust_roots().lock().unwrap().clone() {
        return Ok(roots);
    }
    match mode {
        AttestMode::Tdx => Ok(TrustRoots::Tdx(TdxRoots { pccs_url: cfg.pccs_url.clone() })),
        AttestMode::Snp => {
            let product = cfg.snp_product.ok_or_else(|| {
                "airlock attest=snp requires DUCKTAPE_AIRLOCK_SNP_PRODUCT (milan|genoa|turin)"
                    .to_string()
            })?;
            let vcek = cfg.snp_vcek.clone().unwrap_or(VcekSource::Kds);
            SnpRoots::amd(product, vcek)
                .map(|r| TrustRoots::Snp(Box::new(r)))
                .map_err(|e| format!("airlock SNP roots: {e}"))
        }
    }
}

#[cfg(test)]
fn test_trust_roots() -> &'static std::sync::Mutex<Option<TrustRoots>> {
    static ROOTS: std::sync::OnceLock<std::sync::Mutex<Option<TrustRoots>>> =
        std::sync::OnceLock::new();
    ROOTS.get_or_init(|| std::sync::Mutex::new(None))
}

/// Resolve the Anthropic upstream: a verified airlock gateway when configured
/// (env), else the operator's local credential + api.anthropic.com. Returns the
/// auth arm and the URL the broker forwards `/v1/messages` to.
async fn resolve_anthropic_upstream(
    explicit: Option<AirlockConfig>,
) -> Result<(AnthropicAuth, String), String> {
    // Precedence: a per-run config passed in by the caller wins outright — and
    // reads no env, so the self-host path is env-free. Only when absent do we
    // fall back to the env boundary, then to a host-held credential.
    if let Some(cfg) = explicit {
        return AnthropicAuth::airlock(cfg).await;
    }
    match AirlockConfig::from_env() {
        Some(cfg) => AnthropicAuth::airlock(cfg?).await,
        None => Ok((AnthropicAuth::from_host()?, ANTHROPIC_MESSAGES_URL.into())),
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
    /// subscription-only: BEST-EFFORT refresh of an expired access token before
    /// proxying. A no-op for the API-key path, for an unexpired token, or when no
    /// refresh token is held. A refresh FAILURE is swallowed on purpose: the
    /// OAuth endpoint/client-id are PENDING live validation, and 502-ing here
    /// would also break the existing HEADLESS claude path the moment a token
    /// expires. Instead the stale token proxies and the upstream 401 surfaces to
    /// the client, which re-authenticates.
    async fn refresh_if_needed(&self) {
        let mut auth = self.auth.lock().await;
        let AnthropicAuth::Oauth(tokens) = &mut *auth else {
            return;
        };
        if !tokens.is_expired() {
            return;
        }
        let Some(refresh_token) = tokens.refresh_token.clone() else {
            return;
        };
        if let Ok(fresh) = oauth_refresh(&self.client, &refresh_token).await {
            *tokens = fresh;
        }
    }

    /// Airlock only: a gateway 401 means the scoped session token's TTL lapsed.
    /// Re-mint it against the already-verified seal key. Returns `true` iff it
    /// re-handshook, so the caller retries the request exactly once; other
    /// credential arms return `false` (their 401 is authoritative and flows to
    /// the client, which re-auths). A re-handshake failure also returns `false`.
    async fn airlock_reauth(&self) -> bool {
        let mut auth = self.auth.lock().await;
        let AnthropicAuth::Airlock(session) = &mut *auth else {
            return false;
        };
        match session.gateway.open_session_sealed(&session.seal_pk, &session.sub).await {
            Ok((token, keys)) => {
                session.token = token;
                session.keys = keys;
                true
            }
            Err(_) => {
                // per-request (fires once per forwarded request under a wedged
                // gateway) → debug, not a warn log-bomb. The 401 itself flows to
                // the client, which re-auths.
                tracing::debug!(
                    target: "ducktape::agent",
                    event = "airlock_reauth",
                    reason = "handshake_failed",
                    "airlock session re-handshake failed"
                );
                false
            }
        }
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
    /// start the Anthropic Messages broker for a Podman run (loopback).
    /// `airlock` is the per-run credential source (a self-host resolution); when
    /// `None` the env boundary then a host credential decide the upstream.
    pub async fn start_anthropic(airlock: Option<AirlockConfig>) -> Result<Self, String> {
        let (auth, url) = resolve_anthropic_upstream(airlock).await?;
        Self::start_anthropic_with(auth, Reachability::Loopback, url).await
    }

    /// start it for a Tart guest — bind the host gateway the guest reaches as
    /// `ducktape-host`.
    pub async fn start_anthropic_for_tart(
        airlock: Option<AirlockConfig>,
    ) -> Result<Self, String> {
        let (auth, url) = resolve_anthropic_upstream(airlock).await?;
        Self::start_anthropic_with(auth, Reachability::HostGateway("ducktape-host"), url).await
    }

    /// start it for a private-netns Podman container (`host.containers.internal`).
    pub async fn start_anthropic_for_podman_private(
        airlock: Option<AirlockConfig>,
    ) -> Result<Self, String> {
        let (auth, url) = resolve_anthropic_upstream(airlock).await?;
        Self::start_anthropic_with(
            auth,
            Reachability::HostGateway("host.containers.internal"),
            url,
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
                // the Anthropic broker has no idle-control plane — unused.
                control_url: String::new(),
                control_token: String::new(),
            },
            // idle-control is codex-specific (the `/v1/control/provider-idle`
            // route). A no-op instance satisfies the shared RunBroker shape;
            // begin_invocation rotates a token into it, harmlessly (the anthropic
            // child never receives PROVIDER_CONTROL_* env, so it never dials it).
            idle_control: Arc::new(IdleControl {
                state: Mutex::new(IdleControlState {
                    token: String::new(),
                    hard_deadline: None,
                    deadline: None,
                    requests: BTreeMap::new(),
                    cumulative_secs: 0,
                    limit_logged: false,
                }),
            }),
            shutdown: Some(shutdown),
            task,
        })
    }
}

async fn probe_ok() -> StatusCode {
    StatusCode::OK
}

/// Build the upstream request (target URL + forwarded headers + the current
/// credential) and send it, WITHOUT consuming the body — factored out of
/// [`forward_messages`] so a gateway 401 can be retried after an airlock
/// re-handshake. The caller streams the returned response body.
/// Returns the response plus the sealed request's BINDING (its blob nonce,
/// empty when unsealed) — the response stream key is derived under it.
async fn send_upstream(
    state: &AnthropicBrokerState,
    headers: &HeaderMap,
    body: &Bytes,
) -> reqwest::Result<(reqwest::Response, Vec<u8>)> {
    let mut request = state.client.post(&state.messages_url).body(body.clone());
    // Forward request headers VERBATIM — including `anthropic-version` and
    // `anthropic-beta` (the subscription OAuth capability rides beta; stripping
    // it 401s) — except hop-by-hop framing and the child's credentials, which we
    // replace with the operator's upstream credential (or, in airlock mode, the
    // scoped session token — see [`AnthropicAuth::authorize`]).
    for (name, value) in headers {
        if matches!(
            name.as_str(),
            "authorization"
                | "x-api-key"
                | "host"
                | "content-length"
                | "connection"
                | "transfer-encoding"
                // SECURITY: the overlay routing headers are OURS to set, never
                // the child's. reqwest `.header()` APPENDS, and the browser-gateway
                // reads the FIRST `x-duck-authority` (+ derives its Origin check
                // from it); leaving a child value in would let the sandbox redirect
                // this request — carrying the scoped session token — to an
                // attacker-chosen overlay node. `route()` re-adds our own authority.
                | "x-duck-authority"
                | "origin"
                // Drop accept-encoding so upstream replies UNCOMPRESSED: our
                // reqwest is built without the gzip/brotli features, so it does
                // NOT auto-decompress, and we forward only content-type (not
                // content-encoding) — leave it in and the client would get gzip
                // bytes labelled as plain and fail with "Failed to parse JSON"
                // (live-verified against api.anthropic.com).
                | "accept-encoding"
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
    let (request, binding) = {
        let auth = state.auth.lock().await;
        // Airlock sessions are sealed-body: encrypt the child's plaintext under
        // the handshake body key (fresh nonce per attempt, so the 401-retry
        // path re-seals safely) and mark the request. The enclave refuses
        // plaintext on this token, so the bearer alone grants nothing.
        let (request, binding) = if let AnthropicAuth::Airlock(session) = &*auth {
            let sealed = airlock::bodyseal::seal_request(&session.keys, body);
            let binding = airlock::bodyseal::request_binding(&sealed);
            (
                request
                    .body(sealed)
                    .header(airlock::bodyseal::SEAL_HEADER, airlock::bodyseal::SEAL_V1),
                binding,
            )
        } else {
            (request, Vec::new())
        };
        (auth.authorize(request), binding)
    };
    Ok((request.send().await?, binding))
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

    // best-effort refresh; a failure proxies the stale token (see the fn doc) —
    // never 502s the session.
    state.refresh_if_needed().await;

    let (mut upstream, mut binding) = match send_upstream(&state, &headers, &body).await {
        Ok(sent) => sent,
        Err(e) => {
            return response(
                StatusCode::BAD_GATEWAY,
                &format!("anthropic upstream failed: {e}"),
            );
        }
    };
    // Airlock only: a gateway 401 means the scoped session token's TTL lapsed.
    // Re-handshake once and retry. Every other credential arm — and every other
    // status — passes straight through (`airlock_reauth` returns false).
    let token_expired = upstream.status() == StatusCode::UNAUTHORIZED;
    if token_expired && state.airlock_reauth().await {
        (upstream, binding) = match send_upstream(&state, &headers, &body).await {
            Ok(sent) => sent,
            Err(e) => {
                return response(
                    StatusCode::BAD_GATEWAY,
                    &format!("anthropic upstream failed: {e}"),
                );
            }
        };
    }
    // Airlock sealed session: the enclave's proxied response is an opaque
    // sealed stream — unseal to plain SSE for the unmodified sandbox. Gateway
    // error bodies (minted before the proxy path) are plaintext and relay as
    // errors below; a plaintext SUCCESS on a sealed session can only be a
    // forgery by a path host, so it is refused.
    let seal_keys = {
        let auth = state.auth.lock().await;
        match &*auth {
            AnthropicAuth::Airlock(session) => Some(session.keys.clone()),
            _ => None,
        }
    };
    if let Some(keys) = seal_keys {
        let sealed_outer = upstream
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|ct| ct.starts_with("application/octet-stream"));
        if sealed_outer {
            return relay_sealed(upstream, keys, binding, permit).await;
        }
        if upstream.status().is_success() {
            return response(
                StatusCode::BAD_GATEWAY,
                "airlock: sealed session received a plaintext success body",
            );
        }
    }
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

/// Unseal an enclave-sealed response stream: open chunks incrementally, take
/// the inner content-type from the sealed head, forward Data payloads as they
/// open, and turn a stream that ends WITHOUT the authenticated Final marker
/// into an ABORT (never a clean EOF the sandbox would trust).
async fn relay_sealed(
    mut upstream: reqwest::Response,
    keys: airlock::handshake::SessionKeys,
    binding: Vec<u8>,
    permit: tokio::sync::OwnedSemaphorePermit,
) -> Response<Body> {
    use airlock::bodyseal::OpenedItem;
    let status =
        StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut opener = airlock::bodyseal::StreamOpener::new(&keys, &binding);
    let mut pending: Vec<Bytes> = Vec::new();
    let mut inner_ct: Option<String> = None;
    // The response head can only be built once the sealed head chunk opens.
    while inner_ct.is_none() {
        match upstream.chunk().await {
            Ok(Some(chunk)) => {
                let items = match opener.feed(&chunk) {
                    Ok(items) => items,
                    Err(e) => return response(StatusCode::BAD_GATEWAY, &format!("airlock: {e}")),
                };
                for item in items {
                    match item {
                        OpenedItem::Head(ct) => inner_ct = Some(ct),
                        OpenedItem::Data(data) => pending.push(Bytes::from(data)),
                        OpenedItem::Final => {}
                    }
                }
            }
            Ok(None) => {
                return response(
                    StatusCode::BAD_GATEWAY,
                    "airlock: sealed response ended before its head",
                );
            }
            Err(e) => return response(StatusCode::BAD_GATEWAY, &format!("airlock: {e}")),
        }
    }
    let finished_at_head = opener.finished();
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, std::io::Error>>(8);
    tokio::spawn(async move {
        let _keep = permit; // concurrency slot held for the stream's life
        let mut seen = 0usize;
        for data in pending {
            seen = seen.saturating_add(data.len());
            if tx.send(Ok(data)).await.is_err() {
                return;
            }
        }
        if finished_at_head {
            return;
        }
        loop {
            match upstream.chunk().await {
                Ok(Some(chunk)) => {
                    let items = match opener.feed(&chunk) {
                        Ok(items) => items,
                        Err(e) => {
                            let _ = tx.send(Err(std::io::Error::other(e.to_string()))).await;
                            return;
                        }
                    };
                    for item in items {
                        if let OpenedItem::Data(data) = item {
                            seen = seen.saturating_add(data.len());
                            if seen > MAX_RESPONSE_BYTES {
                                let _ = tx
                                    .send(Err(std::io::Error::other(
                                        "run broker response byte budget exhausted",
                                    )))
                                    .await;
                                return;
                            }
                            if tx.send(Ok(Bytes::from(data))).await.is_err() {
                                return;
                            }
                        }
                    }
                    if opener.finished() {
                        return;
                    }
                }
                Ok(None) => {
                    let _ = tx
                        .send(Err(std::io::Error::other("airlock: sealed response truncated")))
                        .await;
                    return;
                }
                Err(e) => {
                    let _ = tx.send(Err(std::io::Error::other(e.to_string()))).await;
                    return;
                }
            }
        }
    });
    let mut resp = Response::new(Body::from_stream(
        tokio_stream::wrappers::ReceiverStream::new(rx),
    ));
    *resp.status_mut() = status;
    if let Some(value) = inner_ct.and_then(|ct| HeaderValue::from_str(&ct).ok()) {
        resp.headers_mut().insert(axum::http::header::CONTENT_TYPE, value);
    }
    resp
}

fn json_response(status: StatusCode, reply: &IdleControlReply) -> Response<Body> {
    let mut response = Response::new(Body::from(
        serde_json::to_vec(reply).expect("idle control replies always serialize"),
    ));
    *response.status_mut() = status;
    response.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::post;
    use serde_json::{Value, json};

    async fn control_call(
        endpoint: &BrokerEndpoint,
        token: Option<&str>,
        body: Value,
    ) -> (reqwest::StatusCode, Value) {
        let client = reqwest::Client::new();
        let mut request = client.post(&endpoint.control_url).json(&body);
        if let Some(token) = token {
            request = request.header(CONTROL_HEADER, token);
        }
        let response = request.send().await.unwrap();
        let status = response.status();
        let body = response.json().await.unwrap();
        (status, body)
    }

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
        let invocation = broker.begin_invocation();
        assert!(
            invocation
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
        let invocation = broker.begin_invocation();
        invocation.arm(tokio::time::Instant::now() + Duration::from_secs(60));
        let client = reqwest::Client::new();
        let endpoint = format!("{}/responses", invocation.endpoint.base_url);

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
                .bearer_auth(&invocation.endpoint.run_bearer)
                .send()
                .await
                .unwrap()
                .status(),
            reqwest::StatusCode::METHOD_NOT_ALLOWED
        );
        assert_eq!(
            client
                .post(invocation.endpoint.base_url.clone())
                .bearer_auth(&invocation.endpoint.run_bearer)
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
                .bearer_auth(&invocation.endpoint.run_bearer)
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
        assert_ne!(invocation.endpoint.run_bearer, "host-secret-never-in-child");
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

    #[tokio::test]
    async fn idle_control_is_separately_authenticated_idempotent_and_rotated() {
        let broker = RunBroker::start_with(
            UpstreamCredential {
                bearer: "unused".into(),
                account_id: None,
                url: "http://127.0.0.1:1/responses".into(),
            },
            Reachability::Loopback,
        )
        .await
        .unwrap();
        let mut first = broker.begin_invocation();
        first.arm(tokio::time::Instant::now() + Duration::from_secs(60));
        let body = json!({"request_id":"phase-1", "requested_secs":10});

        let (status, reply) = control_call(&first.endpoint, None, body.clone()).await;
        assert_eq!(status, reqwest::StatusCode::UNAUTHORIZED);
        assert_eq!(reply["reason"], "unauthorized");
        let response_token = first.endpoint.run_bearer.clone();
        let (status, _) = control_call(&first.endpoint, Some(&response_token), body.clone()).await;
        assert_eq!(status, reqwest::StatusCode::UNAUTHORIZED);

        let control_token = first.endpoint.control_token.clone();
        let (status, granted) =
            control_call(&first.endpoint, Some(&control_token), body.clone()).await;
        assert_eq!(status, reqwest::StatusCode::OK);
        assert_eq!(granted["status"], "granted");
        assert_eq!(granted["requested_secs"], 10);
        tokio::time::timeout(Duration::from_secs(1), first.idle_deadline.changed())
            .await
            .unwrap()
            .unwrap();
        assert!(first.idle_deadline.borrow().is_some());
        let boundary = tokio::time::Instant::now();
        assert!(
            first.continue_after_timeout_wake(
                boundary - Duration::from_secs(1),
                Duration::from_millis(10),
                boundary + Duration::from_secs(60),
                boundary,
            ),
            "a synchronously granted watch deadline wins over the expired old timer"
        );

        let (status, replay) =
            control_call(&first.endpoint, Some(&control_token), body.clone()).await;
        assert_eq!(status, reqwest::StatusCode::OK);
        assert_eq!(replay, granted);
        let (status, conflict) = control_call(
            &first.endpoint,
            Some(&control_token),
            json!({"request_id":"phase-1", "requested_secs":11}),
        )
        .await;
        assert_eq!(status, reqwest::StatusCode::CONFLICT);
        assert_eq!(conflict["reason"], "request_id_conflict");

        let second = broker.begin_invocation();
        second.arm(tokio::time::Instant::now() + Duration::from_secs(60));
        let (status, stale) = control_call(&first.endpoint, Some(&control_token), body).await;
        assert_eq!(status, reqwest::StatusCode::UNAUTHORIZED);
        assert_eq!(stale["reason"], "unauthorized");
        assert_ne!(first.endpoint.control_token, second.endpoint.control_token);
        let second_token = second.endpoint.control_token.clone();
        second.revoke();
        let (status, cancelled) = control_call(
            &second.endpoint,
            Some(&second_token),
            json!({"request_id":"after-cancel", "requested_secs":1}),
        )
        .await;
        assert_eq!(status, reqwest::StatusCode::UNAUTHORIZED);
        assert_eq!(cancelled["reason"], "unauthorized");

        let expired = broker.begin_invocation();
        expired.arm(tokio::time::Instant::now() + Duration::from_secs(60));
        let expired_token = expired.endpoint.control_token.clone();
        let boundary = tokio::time::Instant::now();
        assert!(!expired.continue_after_timeout_wake(
            boundary - Duration::from_secs(1),
            Duration::from_millis(10),
            boundary + Duration::from_secs(60),
            boundary,
        ));
        let (status, denied) = control_call(
            &expired.endpoint,
            Some(&expired_token),
            json!({"request_id":"too-late", "requested_secs":1}),
        )
        .await;
        assert_eq!(status, reqwest::StatusCode::UNAUTHORIZED);
        assert_eq!(denied["reason"], "unauthorized");
    }

    #[tokio::test]
    async fn idle_control_bounds_seconds_cumulative_requests_and_body() {
        let broker = RunBroker::start_with(
            UpstreamCredential {
                bearer: "unused".into(),
                account_id: None,
                url: "http://127.0.0.1:1/responses".into(),
            },
            Reachability::Loopback,
        )
        .await
        .unwrap();
        let invocation = broker.begin_invocation();
        invocation.arm(tokio::time::Instant::now() + Duration::from_secs(60 * 60));
        let token = invocation.endpoint.control_token.clone();

        let (status, reply) = control_call(
            &invocation.endpoint,
            Some(&token),
            json!({"request_id":"over", "requested_secs":MAX_CONTROL_REQUESTED_SECS + 1}),
        )
        .await;
        assert_eq!(status, reqwest::StatusCode::BAD_REQUEST);
        assert_eq!(reply["reason"], "invalid_requested_secs");

        for index in 0..MAX_CONTROL_REQUESTS {
            let (status, reply) = control_call(
                &invocation.endpoint,
                Some(&token),
                json!({"request_id":format!("r-{index}"), "requested_secs":1}),
            )
            .await;
            assert_eq!(status, reqwest::StatusCode::OK);
            assert_eq!(reply["status"], "granted");
        }
        let (status, reply) = control_call(
            &invocation.endpoint,
            Some(&token),
            json!({"request_id":"r-8", "requested_secs":1}),
        )
        .await;
        assert_eq!(status, reqwest::StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(reply["reason"], "request_limit_exhausted");
        let (status, replay) = control_call(
            &invocation.endpoint,
            Some(&token),
            json!({"request_id":"r-0", "requested_secs":1}),
        )
        .await;
        assert_eq!(status, reqwest::StatusCode::OK);
        assert_eq!(replay["status"], "granted");

        let client = reqwest::Client::new();
        let response = client
            .post(&invocation.endpoint.control_url)
            .header(CONTROL_HEADER, &token)
            .body(vec![b'x'; MAX_CONTROL_REQUEST_BYTES + 1])
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::PAYLOAD_TOO_LARGE);

        let cumulative = broker.begin_invocation();
        cumulative.arm(tokio::time::Instant::now() + Duration::from_secs(3 * 60 * 60));
        let token = cumulative.endpoint.control_token.clone();
        for index in 0..4 {
            let (status, _) = control_call(
                &cumulative.endpoint,
                Some(&token),
                json!({"request_id":format!("c-{index}"), "requested_secs":1800}),
            )
            .await;
            assert_eq!(status, reqwest::StatusCode::OK);
        }
        let (status, reply) = control_call(
            &cumulative.endpoint,
            Some(&token),
            json!({"request_id":"c-4", "requested_secs":1}),
        )
        .await;
        assert_eq!(status, reqwest::StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(reply["reason"], "cumulative_limit_exhausted");
    }

    #[tokio::test]
    async fn idle_control_reports_hard_cap_truncation() {
        let broker = RunBroker::start_with(
            UpstreamCredential {
                bearer: "unused".into(),
                account_id: None,
                url: "http://127.0.0.1:1/responses".into(),
            },
            Reachability::Loopback,
        )
        .await
        .unwrap();
        let invocation = broker.begin_invocation();
        invocation.arm(tokio::time::Instant::now() + Duration::from_secs(2));
        let token = invocation.endpoint.control_token.clone();
        let (status, reply) = control_call(
            &invocation.endpoint,
            Some(&token),
            json!({"request_id":"bounded", "requested_secs":30}),
        )
        .await;
        assert_eq!(status, reqwest::StatusCode::OK);
        assert_eq!(reply["status"], "granted");
        assert_eq!(reply["hard_cap_truncated"], true);
        assert!(reply["granted_secs"].as_u64().unwrap() <= 2);
    }

    // ---- Airlock credential source (execution/auth separation) --------------

    /// A mock Anthropic upstream the AIRLOCK GATEWAY swaps into: `/oauth/token`
    /// mints `acc-N`; `/v1/messages` accepts ONLY `Bearer acc-N` — so a 200
    /// proves the gateway swapped the session token for the real credential —
    /// and streams `AIRLOCK-OK`.
    async fn airlock_mock_anthropic() -> String {
        use axum::response::IntoResponse;
        let n = Arc::new(Mutex::new(0u64));
        let oauth_n = n.clone();
        let msg_n = n.clone();
        let app = Router::new()
            .route(
                "/oauth/token",
                post(move || {
                    let n = oauth_n.clone();
                    async move {
                        let mut n = n.lock().unwrap();
                        *n += 1;
                        axum::Json(json!({
                            "access_token": format!("acc-{n}"),
                            "refresh_token": format!("ref-{n}"),
                            "expires_in": 3600
                        }))
                    }
                }),
            )
            .route(
                "/v1/messages",
                post(move |headers: HeaderMap| {
                    let n = msg_n.clone();
                    async move {
                        let want = format!("Bearer acc-{}", *n.lock().unwrap());
                        let got = headers
                            .get(axum::http::header::AUTHORIZATION)
                            .and_then(|v| v.to_str().ok())
                            .unwrap_or("");
                        if got != want {
                            return (StatusCode::UNAUTHORIZED, format!("want {want:?} got {got:?}"))
                                .into_response();
                        }
                        (
                            [("content-type", "text/event-stream")],
                            "event: content_block_delta\ndata: AIRLOCK-OK\n\n",
                        )
                            .into_response()
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    /// ONE test enclave (measures `0x11`×48) shared by every airlock test in
    /// this file, so the parallel tests all agree on the injected trust roots.
    /// Its minted chain verifies through the REAL SNP verifier — only under
    /// its own roots, never under the AMD builtins.
    fn test_enclave() -> &'static Arc<airlock::testkit::SnpTestEnclave> {
        static ENCLAVE: std::sync::OnceLock<Arc<airlock::testkit::SnpTestEnclave>> =
            std::sync::OnceLock::new();
        ENCLAVE.get_or_init(|| {
            let m = Measurement([0x11; attest::MRTD_LEN]);
            let enclave = Arc::new(airlock::testkit::SnpTestEnclave::new(&m).unwrap());
            // Route the broker's verify path at the enclave's roots. Set once,
            // same value from every test — no cross-test races.
            *test_trust_roots().lock().unwrap() = Some(enclave.roots());
            enclave
        })
    }

    /// Boot an in-process airlock gateway (measures `0x11`×48) pointed at
    /// `upstream`, and return its base URL.
    async fn boot_airlock_gateway(upstream: &str) -> String {
        let (app, vendor) = airlock::server::build_with_quoter(
            airlock::server::GatewayConfig {
                attest: airlock::server::AttestMode::Tsm("snp".into()),
                seal_keypair: None,
                anthropic_base: upstream.into(),
                openai_base: upstream.into(),
                oauth_token_url: format!("{upstream}/oauth/token"),
                oauth_client_id: "test-client".into(),
                session_ttl_secs: 3600,
                max_requests: 100,
            },
            "snp",
            test_enclave().quoter(),
            Vec::new(),
        )
        .unwrap();
        assert_eq!(vendor, "snp");
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn airlock_broker_uses_the_gateway_as_credential_source() {
        let meas = "11".repeat(attest::MRTD_LEN);
        let upstream = airlock_mock_anthropic().await;
        let gateway_url = boot_airlock_gateway(&upstream).await;

        // Credential Provider: verify the gateway quote through the REAL SNP
        // verifier (under the test enclave's roots), then seal + upload the
        // refresh token (the broker never holds it — the gateway does, sealed).
        let gw = Gateway::local(gateway_url.clone());
        let (quote, _vendor) = gw.fetch_quote().await.unwrap();
        let expected = Measurement::from_hex(&meas).unwrap();
        let rd = airlock::verify::verify_quote(&quote, &expected, &test_enclave().roots())
            .await
            .unwrap();
        let seal_pk = attest::split_report_data(&rd).0;
        gw.upload_sealed_credential(
            &seal_pk,
            "test-sub",
            airlock::wire::CredentialKind::Claude,
            &airlock::wire::CredentialPayload::Refresh {
                refresh_token: "ref-seed".into(),
                access_token: String::new(),
                expires_at: 0,
            },
        )
        .await
        .unwrap();

        // Computation Provider: build the Anthropic broker in AIRLOCK mode —
        // NO host credential, just a verified gateway + session token.
        let (auth, messages_url) = AnthropicAuth::airlock(AirlockConfig {
            gateway: AirlockGateway::Local { url: gateway_url },
            trust: AirlockTrust::Attested { measurement: meas, attest: "snp".into() },
            sub: "test-sub".into(),
            account: None,
            snp_product: None, // the test roots override supplies the chain
            snp_vcek: None,
            pccs_url: None,
        })
        .await
        .unwrap();
        assert!(
            matches!(auth, AnthropicAuth::Airlock(_)),
            "airlock config must yield the Airlock arm"
        );
        let broker = RunBroker::start_anthropic_with(auth, Reachability::Loopback, messages_url)
            .await
            .unwrap();

        // Sandbox: an unmodified client with only the opaque run bearer. The
        // reply streams back only if sandbox → broker → gateway → upstream all
        // held AND the gateway swapped the session token for the real credential.
        let resp = reqwest::Client::new()
            .post(format!("{}/v1/messages", broker.endpoint.base_url))
            .bearer_auth(&broker.endpoint.run_bearer)
            .header("content-type", "application/json")
            .body(r#"{"model":"claude-sonnet-5","stream":true,"messages":[{"role":"user","content":"hi"}]}"#)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::OK);
        let body = resp.text().await.unwrap();
        assert!(body.contains("AIRLOCK-OK"), "custody path should stream the reply back: {body}");
        // the run bearer the sandbox holds is neither the session token nor the credential.
        assert_ne!(broker.endpoint.run_bearer, "ref-seed");
    }

    #[tokio::test]
    async fn airlock_broker_refuses_a_gateway_whose_measurement_mismatches() {
        let upstream = airlock_mock_anthropic().await;
        let gateway_url = boot_airlock_gateway(&upstream).await; // measures 0x11×48

        // Pin a DIFFERENT audited image; the attestation gate must reject the
        // gateway before any session is established or credential spent.
        let refused = AnthropicAuth::airlock(AirlockConfig {
            gateway: AirlockGateway::Local { url: gateway_url },
            trust: AirlockTrust::Attested {
                measurement: "22".repeat(attest::MRTD_LEN),
                attest: "snp".into(),
            },
            sub: "test-sub".into(),
            account: None,
            snp_product: None,
            snp_vcek: None,
            pccs_url: None,
        })
        .await;
        assert!(
            refused.is_err(),
            "a gateway whose measurement != the pinned audited image must be refused"
        );
    }

    /// A recording upstream that captures the headers of one request.
    async fn recording_gateway() -> (String, Arc<Mutex<Option<(HeaderMap, Vec<u8>)>>>) {
        let seen = Arc::new(Mutex::new(None));
        let sink = seen.clone();
        let app = Router::new().route(
            "/v1/messages",
            post(move |headers: HeaderMap, body: axum::body::Bytes| {
                let sink = sink.clone();
                async move {
                    *sink.lock().unwrap() = Some((headers, body.to_vec()));
                    // A recording tap has no enclave to seal a success body;
                    // return a non-success so the sealed session relays it as
                    // a plaintext ERROR (a plaintext SUCCESS would be refused).
                    (StatusCode::BAD_GATEWAY, "tap")
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (format!("http://{addr}"), seen)
    }

    #[tokio::test]
    async fn airlock_remote_strips_child_injected_routing_authority() {
        // SECURITY REGRESSION: in remote topology a sandbox child must not be
        // able to inject `x-duck-authority` and redirect the session-token-bearing
        // request to an attacker-chosen overlay node.
        let (via, seen) = recording_gateway().await;
        let enclave = airlock::seal::SealKeypair::generate();
        let (_eph, keys) = airlock::handshake::client_handshake(&enclave.public_bytes());
        let auth = AnthropicAuth::Airlock(AirlockSession {
            gateway: Gateway::remote("broker.duck".into(), via.clone()),
            seal_pk: [0u8; 32],
            sub: "s".into(),
            token: "sess-tok".into(),
            keys,
        });
        let broker =
            RunBroker::start_anthropic_with(auth, Reachability::Loopback, format!("{via}/v1/messages"))
                .await
                .unwrap();

        let resp = reqwest::Client::new()
            .post(format!("{}/v1/messages", broker.endpoint.base_url))
            .bearer_auth(&broker.endpoint.run_bearer)
            .header("x-duck-authority", "attacker.duck")
            .header("origin", "https://attacker.duck")
            .body(r#"{"secret":"prompt"}"#)
            .send()
            .await
            .unwrap();
        // The tap answers a plaintext non-success; the sealed session relays
        // it as the error it is (a plaintext SUCCESS would be refused).
        assert_eq!(resp.status(), reqwest::StatusCode::BAD_GATEWAY);

        let (headers, body) = seen.lock().unwrap().take().unwrap();
        let authorities: Vec<&str> = headers
            .get_all("x-duck-authority")
            .iter()
            .map(|v| v.to_str().unwrap())
            .collect();
        assert_eq!(
            authorities,
            vec!["broker.duck"],
            "only the broker's OWN routing authority may reach the gateway"
        );
        // the session token rode the request (we replaced auth, not dropped it).
        assert_eq!(headers["authorization"], "Bearer sess-tok");
        // The path host sees CIPHERTEXT: the sealed body carries the marker
        // header and none of the child's plaintext.
        assert_eq!(headers[airlock::bodyseal::SEAL_HEADER], airlock::bodyseal::SEAL_V1);
        assert!(
            !body.windows(6).any(|w| w == b"prompt"),
            "the child's plaintext must never reach a path host"
        );
    }

    // ---- Self-host airlock trust (pinned seal_pk, no TEE) --------------------

    /// A mock provider upstream that accepts ONLY `Bearer {expect}` (proving the
    /// gateway swapped the session token for the seeded static bearer) and streams
    /// a vendor-tagged SSE reply on both the claude and codex paths.
    async fn bearer_upstream(expect: &'static str) -> String {
        use axum::response::IntoResponse;
        let guard = move |body: &'static str| {
            move |headers: HeaderMap| async move {
                let want = format!("Bearer {expect}");
                let got = headers
                    .get(axum::http::header::AUTHORIZATION)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");
                if got != want {
                    return (StatusCode::UNAUTHORIZED, format!("want {want:?} got {got:?}"))
                        .into_response();
                }
                ([("content-type", "text/event-stream")], body).into_response()
            }
        };
        let app = Router::new()
            .route(
                "/v1/messages",
                post(guard("event: content_block_delta\ndata: AIRLOCK-OK\n\n")),
            )
            .route(
                "/v1/responses",
                post(guard("event: response.output_text.delta\ndata: CODEX-OK\n\n")),
            );
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    /// A fresh self-host gateway keypair — the secret the gateway seals under, the
    /// public key the broker pins (what `cred add` would publish on-chain).
    fn seal_pair() -> (airlock::seal::SealKeypair, [u8; 32]) {
        let kp = airlock::seal::SealKeypair::generate();
        let pk = kp.public_bytes();
        (kp, pk)
    }

    /// Boot a NO-TEE self-host gateway pinned to `seal_kp`, seeded with `seeds`,
    /// pointed at `upstream` for both vendors. Returns its base URL.
    async fn boot_self_host_gateway(
        upstream: &str,
        seal_kp: airlock::seal::SealKeypair,
        seeds: Vec<(
            String,
            airlock::wire::CredentialKind,
            airlock::wire::CredentialPayload,
        )>,
    ) -> String {
        let (app, vendor) = airlock::server::build_seeded(
            airlock::server::GatewayConfig {
                attest: airlock::server::AttestMode::SelfHost,
                seal_keypair: Some(seal_kp),
                anthropic_base: upstream.into(),
                openai_base: upstream.into(),
                oauth_token_url: format!("{upstream}/oauth/token"),
                oauth_client_id: "test-client".into(),
                session_ttl_secs: 3600,
                max_requests: 100,
            },
            seeds,
        )
        .unwrap();
        assert_eq!(vendor, "self-host");
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    fn resolved(
        name: &str,
        kind: CredentialKind,
        via: &str,
        seal_pk: [u8; 32],
    ) -> ResolvedCredential {
        ResolvedCredential {
            name: name.into(),
            kind,
            authority: "owner.duck".into(),
            via: via.into(),
            seal_pk,
            account: b"owner-account".to_vec(),
        }
    }

    /// Like [`boot_self_host_gateway`] but with the co-hosted-lending grant gate
    /// wired: a session opens only when its claimed `account_b64` equals
    /// `granted`. This is the production self-host mode (`user cred add` builds an
    /// always-gated gateway), so a broker that fails to send the account 403s
    /// here before any credentialed request.
    async fn boot_grant_gated_gateway(
        upstream: &str,
        seal_kp: airlock::seal::SealKeypair,
        seeds: Vec<(
            String,
            airlock::wire::CredentialKind,
            airlock::wire::CredentialPayload,
        )>,
        granted: Vec<u8>,
    ) -> String {
        let check: airlock::server::GrantCheck = std::sync::Arc::new(move |_sub, account| {
            let granted = granted.clone();
            Box::pin(async move { account == granted })
        });
        let (app, vendor) = airlock::server::build_seeded_gated(
            airlock::server::GatewayConfig {
                attest: airlock::server::AttestMode::SelfHost,
                seal_keypair: Some(seal_kp),
                anthropic_base: upstream.into(),
                openai_base: upstream.into(),
                oauth_token_url: format!("{upstream}/oauth/token"),
                oauth_client_id: "test-client".into(),
                session_ttl_secs: 3600,
                max_requests: 100,
            },
            seeds,
            Some(check),
        )
        .unwrap();
        assert_eq!(vendor, "self-host");
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    /// The production round-trip the lending e2e never exercises through the
    /// broker: a grant-GATED self-host gateway (what `user cred add` always
    /// builds) admits the broker's sealed session ONLY because the broker names
    /// the granted account in `account_b64`. A broker that dropped the account
    /// (the bug this guards) would 403 `credential_not_granted` at session open.
    #[tokio::test]
    async fn broker_sends_the_grant_account_to_a_gated_gateway() {
        let upstream = bearer_upstream("tok-grant").await;
        let (kp, seal_pk) = seal_pair();
        let gateway_url = boot_grant_gated_gateway(
            &upstream,
            kp,
            vec![(
                "owner-claude-1".into(),
                airlock::wire::CredentialKind::Claude,
                airlock::wire::CredentialPayload::Bearer { access_token: "tok-grant".into() },
            )],
            b"grantee".to_vec(),
        )
        .await;
        let mut rc = resolved("owner-claude-1", CredentialKind::Claude, &gateway_url, seal_pk);
        rc.account = b"grantee".to_vec();
        let (auth, messages_url) = AnthropicAuth::airlock(AirlockConfig::self_host(&rc))
            .await
            .expect("a granted account opens the gated session");
        let broker = RunBroker::start_anthropic_with(auth, Reachability::Loopback, messages_url)
            .await
            .unwrap();
        let resp = reqwest::Client::new()
            .post(format!("{}/v1/messages", broker.endpoint.base_url))
            .bearer_auth(&broker.endpoint.run_bearer)
            .header("content-type", "application/json")
            .body(r#"{"model":"claude","stream":true,"messages":[{"role":"user","content":"hi"}]}"#)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::OK);
        assert!(resp.text().await.unwrap().contains("AIRLOCK-OK"), "gated self-host round-trip");
    }

    /// The gate's teeth: an UNGRANTED account is refused at session open, before
    /// any credentialed request — the broker's account reaches the gate and the
    /// gate says no.
    #[tokio::test]
    async fn broker_session_is_refused_for_an_ungranted_account() {
        let upstream = bearer_upstream("tok-grant").await;
        let (kp, seal_pk) = seal_pair();
        let gateway_url = boot_grant_gated_gateway(
            &upstream,
            kp,
            vec![(
                "owner-claude-1".into(),
                airlock::wire::CredentialKind::Claude,
                airlock::wire::CredentialPayload::Bearer { access_token: "tok-grant".into() },
            )],
            b"grantee".to_vec(),
        )
        .await;
        let mut rc = resolved("owner-claude-1", CredentialKind::Claude, &gateway_url, seal_pk);
        rc.account = b"stranger".to_vec();
        let refused = AnthropicAuth::airlock(AirlockConfig::self_host(&rc)).await;
        assert!(refused.is_err(), "an ungranted account must not open a session");
    }

    #[tokio::test]
    async fn pinned_seal_pk_skips_quote_verification_and_seals_to_the_pin() {
        let upstream = bearer_upstream("tok-e2e").await;
        let (kp, seal_pk) = seal_pair();
        let gateway_url = boot_self_host_gateway(
            &upstream,
            kp,
            vec![(
                "owner-claude-1".into(),
                airlock::wire::CredentialKind::Claude,
                airlock::wire::CredentialPayload::Bearer { access_token: "tok-e2e".into() },
            )],
        )
        .await;
        // No quote is fetched — the seal_pk is pinned directly from the record.
        let cfg = AirlockConfig::self_host(&resolved(
            "owner-claude-1",
            CredentialKind::Claude,
            &gateway_url,
            seal_pk,
        ));
        let (auth, messages_url) = AnthropicAuth::airlock(cfg).await.unwrap();
        assert!(matches!(auth, AnthropicAuth::Airlock(_)));
        let broker = RunBroker::start_anthropic_with(auth, Reachability::Loopback, messages_url)
            .await
            .unwrap();
        let resp = reqwest::Client::new()
            .post(format!("{}/v1/messages", broker.endpoint.base_url))
            .bearer_auth(&broker.endpoint.run_bearer)
            .header("content-type", "application/json")
            .body(r#"{"model":"claude","stream":true,"messages":[{"role":"user","content":"hi"}]}"#)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::OK);
        let body = resp.text().await.unwrap();
        assert!(body.contains("AIRLOCK-OK"), "sealed self-host round-trip: {body}");
    }

    #[tokio::test]
    async fn run_context_airlock_resolves_the_upstream_over_env() {
        // the per-run AirlockConfig a RunContext carries reaches the broker
        // through resolve_anthropic_upstream (the seam start_broker feeds
        // start_anthropic): an explicit config wins outright and reads no env, so
        // the self-host gateway is resolved even with DUCKTAPE_AIRLOCK_* unset.
        let upstream = bearer_upstream("tok-ctx").await;
        let (kp, seal_pk) = seal_pair();
        let gateway_url = boot_self_host_gateway(
            &upstream,
            kp,
            vec![(
                "owner-claude-1".into(),
                airlock::wire::CredentialKind::Claude,
                airlock::wire::CredentialPayload::Bearer { access_token: "tok-ctx".into() },
            )],
        )
        .await;
        let cfg = AirlockConfig::self_host(&resolved(
            "owner-claude-1",
            CredentialKind::Claude,
            &gateway_url,
            seal_pk,
        ));
        let (auth, _url) = resolve_anthropic_upstream(Some(cfg)).await.unwrap();
        assert!(
            matches!(auth, AnthropicAuth::Airlock(_)),
            "an explicit RunContext airlock resolves to the self-host gateway, not env/host"
        );
    }

    #[tokio::test]
    async fn pinned_seal_pk_mismatch_refuses_the_gateway() {
        let upstream = bearer_upstream("tok-e2e").await;
        let (kp, _real_pk) = seal_pair();
        let gateway_url = boot_self_host_gateway(
            &upstream,
            kp,
            vec![(
                "owner-claude-1".into(),
                airlock::wire::CredentialKind::Claude,
                airlock::wire::CredentialPayload::Bearer { access_token: "tok-e2e".into() },
            )],
        )
        .await;
        // Pin the WRONG seal key: the sealed session token cannot be opened, so
        // setup fails with the named error before any credentialed request.
        let cfg = AirlockConfig::self_host(&resolved(
            "owner-claude-1",
            CredentialKind::Claude,
            &gateway_url,
            [0u8; 32],
        ));
        let refused = AnthropicAuth::airlock(cfg).await;
        assert_eq!(refused.err().as_deref(), Some("gateway_seal_pk_mismatch"));
    }

    #[tokio::test]
    async fn explicit_airlock_config_beats_env() {
        // No DUCKTAPE_AIRLOCK_* env is configured in this process; the explicit
        // config is still chosen — the Some(_) branch provably never reads env.
        let upstream = bearer_upstream("tok-e2e").await;
        let (kp, seal_pk) = seal_pair();
        let gateway_url = boot_self_host_gateway(
            &upstream,
            kp,
            vec![(
                "owner-claude-1".into(),
                airlock::wire::CredentialKind::Claude,
                airlock::wire::CredentialPayload::Bearer { access_token: "tok-e2e".into() },
            )],
        )
        .await;
        let cfg = AirlockConfig::self_host(&resolved(
            "owner-claude-1",
            CredentialKind::Claude,
            &gateway_url,
            seal_pk,
        ));
        let (auth, _url) = resolve_anthropic_upstream(Some(cfg)).await.unwrap();
        assert!(
            matches!(auth, AnthropicAuth::Airlock(_)),
            "an explicit per-run config must win over the env/host path"
        );
    }

    #[tokio::test]
    async fn codex_airlock_arm_round_trips_through_the_self_host_gateway() {
        let upstream = bearer_upstream("tok-codex").await;
        let (kp, seal_pk) = seal_pair();
        let gateway_url = boot_self_host_gateway(
            &upstream,
            kp,
            vec![(
                "owner-codex-1".into(),
                airlock::wire::CredentialKind::Codex,
                airlock::wire::CredentialPayload::Bearer { access_token: "tok-codex".into() },
            )],
        )
        .await;
        let cfg = AirlockConfig::self_host(&resolved(
            "owner-codex-1",
            CredentialKind::Codex,
            &gateway_url,
            seal_pk,
        ));
        let (auth, responses_url) = CodexAuth::airlock(cfg).await.unwrap();
        assert!(matches!(auth, CodexAuth::Airlock(_)));
        let broker = RunBroker::start_codex(auth, responses_url, Reachability::Loopback)
            .await
            .unwrap();
        // codex posts `/responses` to a base that already ends in `/v1`.
        let resp = reqwest::Client::new()
            .post(format!("{}/responses", broker.endpoint.base_url))
            .bearer_auth(&broker.endpoint.run_bearer)
            .header("content-type", "application/json")
            .body(r#"{"model":"gpt","input":"hi"}"#)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::OK);
        let body = resp.text().await.unwrap();
        assert!(body.contains("CODEX-OK"), "sealed codex self-host round-trip: {body}");
    }
}
