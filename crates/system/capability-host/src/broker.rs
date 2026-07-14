//! Run-scoped Codex Responses broker.
//!
//! The provider child never receives the operator's API/OAuth credential.
//! Only this host process reads it. Codex talks to a single-run endpoint using
//! an unrelated random bearer. Direct/Podman bind loopback; Tart binds the host
//! side of its private NAT so the VM can reach it by a guest-only hostname.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::body::{Body, Bytes, to_bytes};
use axum::extract::{DefaultBodyLimit, Request, State};
use axum::http::{HeaderMap, HeaderValue, Response, StatusCode};
use axum::routing::post;
use rand::RngCore as _;
use serde::{Deserialize, Serialize};
use tokio::sync::{Semaphore, oneshot, watch};

const MAX_REQUEST_BYTES: usize = 8 * 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 32 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 128 * 1024 * 1024;
const MAX_REQUESTS: u32 = 64;
const CONTROL_HEADER: &str = "x-ducktape-provider-control";
const MAX_CONTROL_REQUEST_BYTES: usize = 4 * 1024;
const MAX_CONTROL_REQUESTS: usize = 8;
const MAX_CONTROL_REQUEST_ID_BYTES: usize = 64;
const MAX_CONTROL_REQUESTED_SECS: u64 = 30 * 60;
const MAX_CONTROL_CUMULATIVE_SECS: u64 = 2 * 60 * 60;

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

pub(crate) struct BrokerInvocation {
    pub(crate) endpoint: BrokerEndpoint,
    pub(crate) idle_deadline: watch::Receiver<Option<tokio::time::Instant>>,
    idle_control: Arc<IdleControl>,
}

impl BrokerInvocation {
    pub(crate) fn arm(&self, hard_deadline: tokio::time::Instant) {
        let mut state = self.idle_control.state.lock().unwrap();
        if state.token == self.endpoint.control_token {
            state.hard_deadline = Some(hard_deadline);
        }
    }

    pub(crate) fn revoke(&self) {
        let mut state = self.idle_control.state.lock().unwrap();
        if state.token == self.endpoint.control_token {
            revoke_idle_control(&mut state);
        }
    }

    /// Linearize a provisional timer wake against control grants. If a grant
    /// won the mutex first, its watched deadline is observed and the child
    /// continues. Otherwise expiry revokes the token before a later request
    /// can be reported as granted.
    pub(crate) fn continue_after_timeout_wake(
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
pub(crate) struct BrokerEndpoint {
    pub(crate) base_url: String,
    pub(crate) run_bearer: String,
    pub(crate) control_url: String,
    pub(crate) control_token: String,
}

pub(crate) struct RunBroker {
    base_url: String,
    run_bearer: String,
    idle_control: Arc<IdleControl>,
    shutdown: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
}

impl RunBroker {
    pub(crate) async fn start() -> Result<Self, String> {
        Self::start_with(UpstreamCredential::from_host()?, false).await
    }

    pub(crate) async fn start_for_tart() -> Result<Self, String> {
        Self::start_with(UpstreamCredential::from_host()?, true).await
    }

    #[cfg(test)]
    pub(crate) async fn start_for_test() -> Self {
        Self::start_with(
            UpstreamCredential {
                bearer: "unused".into(),
                account_id: None,
                url: "http://127.0.0.1:1/responses".into(),
            },
            false,
        )
        .await
        .unwrap()
    }

    async fn start_with(upstream: UpstreamCredential, tart_guest: bool) -> Result<Self, String> {
        let bind = if tart_guest {
            std::net::Ipv4Addr::UNSPECIFIED
        } else {
            std::net::Ipv4Addr::LOCALHOST
        };
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
            upstream,
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
        let base_url = if tart_guest {
            format!("http://ducktape-host:{}/v1", addr.port())
        } else {
            format!("http://{addr}/v1")
        };
        Ok(Self {
            base_url,
            run_bearer,
            idle_control,
            shutdown: Some(shutdown),
            task,
        })
    }

    /// Arm a fresh control credential and deadline channel for one child
    /// invocation. A resume fallback rotates the credential before spawning
    /// its replacement child, so an old MCP process cannot control the new one.
    pub(crate) fn begin_invocation(&self) -> BrokerInvocation {
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
                base_url: self.base_url.clone(),
                run_bearer: self.run_bearer.clone(),
                control_url: format!("{}/control/provider-idle", self.base_url),
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
    if let Some(log) = log {
        eprintln!("[capability-host] provider idle control {log}");
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
            true,
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
            false,
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

    #[tokio::test]
    async fn idle_control_is_separately_authenticated_idempotent_and_rotated() {
        let broker = RunBroker::start_with(
            UpstreamCredential {
                bearer: "unused".into(),
                account_id: None,
                url: "http://127.0.0.1:1/responses".into(),
            },
            false,
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
            false,
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
            false,
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
}
