//! the gateway lane: signed-route proxying (`/v1/gateway/*`) and the
//! isolated browser-gateway origin. named `gateway_http` (like `files_http`)
//! because the `gateway` module crate rides alongside as a dependency.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::{Body, Bytes};
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use futures::SinkExt as _;
use futures::channel::oneshot;
use rand::RngCore as _;
use serde::{Deserialize, Serialize};

use crate::{NodeCommand, NodeHandle, error_response, hex_bytes};

/// One bounded invocation through a globally signed gateway route. The
/// full node drains this lane through `Service::Gateway`; the embedded daemon
/// leaves it unwired because it has no authenticated network transport.
pub struct GatewayJob {
    /// Derived from the locally finalized RouteRecord, never client input.
    pub publisher_node: [u8; 32],
    pub max_response_bytes: u64,
    pub head: gateway::ProxyRequestHead,
    pub body: Vec<u8>,
    pub reply: oneshot::Sender<Result<GatewayResponse, GatewayFailure>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayResponse {
    pub head: gateway::ProxyResponseHead,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatewayFailure {
    Invalid(String),
    Forbidden(String),
    NotFound(String),
    Conflict(String),
    Unavailable(String),
}

pub type GatewayLane = tokio::sync::mpsc::Sender<GatewayJob>;

const GATEWAY_SESSION_IDLE: Duration = Duration::from_secs(10 * 60);
const MAX_GATEWAY_SESSIONS: usize = 64;

#[derive(Clone)]
pub(crate) struct GatewaySession {
    account_id: Vec<u8>,
    name: gateway::RouteName,
    revision: u64,
    last_used: Instant,
}

#[derive(Clone)]
pub(crate) struct BrowserGateway {
    pub(crate) listen: SocketAddr,
    pub(crate) sessions: Arc<tokio::sync::Mutex<HashMap<String, GatewaySession>>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GatewayProxyRequest {
    pub head: gateway::ProxyRequestHead,
    pub body_b64: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProxyReply {
    pub head: gateway::ProxyResponseHead,
    pub body_b64: String,
}

/// The node surface predates gateway and intentionally has permissive CORS for
/// the web console. Gateway is a network pivot, so its two API entries add a
/// narrower browser boundary: native clients omit Origin, while only the
/// bundled Tauri console origins may call from a WebView. Publisher sessions
/// and arbitrary websites fail before route resolution or overlay work.
fn gateway_api_origin_allowed(headers: &HeaderMap) -> bool {
    let mut origins = headers.get_all(header::ORIGIN).iter();
    let first = origins.next();
    if origins.next().is_some() {
        return false;
    }
    match first {
        Some(value) => {
            let Ok(origin) = value.to_str() else {
                return false;
            };
            matches!(
                origin,
                "tauri://localhost" | "http://tauri.localhost" | "https://tauri.localhost"
            ) || (cfg!(debug_assertions) && origin == "http://localhost:1430")
        }
        // A real native client has neither header. Browser requests without an
        // Origin still carry Fetch Metadata, so do not let that omission turn
        // into a bypass (for example from a sandboxed publisher document).
        None => !headers.contains_key("sec-fetch-site"),
    }
}

fn gateway_api_origin_guard(headers: &HeaderMap) -> Option<Response> {
    (!gateway_api_origin_allowed(headers)).then(|| {
        error_response(
            StatusCode::FORBIDDEN,
            "gateway API is limited to the trusted Ducktape console and native clients",
        )
    })
}

async fn current_route(
    handle: &NodeHandle,
    account_id: &[u8],
    name: &gateway::RouteName,
) -> Result<gateway::RouteRecord, GatewayFailure> {
    gateway::validate_account_id(account_id).map_err(GatewayFailure::Invalid)?;
    name.validate().map_err(GatewayFailure::Invalid)?;
    let (reply, rx) = oneshot::channel();
    let mut commands = handle.cmds.clone();
    commands
        .send(NodeCommand::Query {
            target: "gateway".into(),
            req: gateway::encode_query(&gateway::GatewayQuery::Get {
                account_id: account_id.to_vec(),
                name: name.clone(),
            }),
            reply,
        })
        .await
        .map_err(|_| GatewayFailure::Unavailable("node actor is gone".into()))?;
    let bytes = tokio::time::timeout(Duration::from_secs(5), rx)
        .await
        .map_err(|_| GatewayFailure::Unavailable("gateway route query timed out".into()))?
        .map_err(|_| GatewayFailure::Unavailable("node actor dropped the query".into()))?
        .map_err(GatewayFailure::Unavailable)?;
    match gateway::decode_reply(&bytes) {
        Ok(gateway::GatewayReply::Route(route)) => match *route {
            Some(record) if record.statement.route.is_some() => Ok(record),
            _ => Err(GatewayFailure::NotFound(
                "gateway route is not published".into(),
            )),
        },
        Ok(gateway::GatewayReply::Routes(_)) => Err(GatewayFailure::Unavailable(
            "gateway returned an unexpected route-list reply".into(),
        )),
        Err(error) => Err(GatewayFailure::Unavailable(error)),
    }
}

async fn proxy_current(
    handle: &NodeHandle,
    head: gateway::ProxyRequestHead,
    body: Vec<u8>,
) -> Result<GatewayResponse, GatewayFailure> {
    gateway::validate_proxy_request_head(&head).map_err(GatewayFailure::Invalid)?;
    if body.len() as u64 != head.body_len {
        return Err(GatewayFailure::Invalid(
            "gateway body length does not match its request head".into(),
        ));
    }
    let record = current_route(handle, &head.account_id, &head.name).await?;
    if record.statement.revision != head.revision {
        return Err(GatewayFailure::Conflict(
            "gateway route changed; resolve the name again".into(),
        ));
    }
    if !gateway::request_matches_record(&head, &record) {
        return Err(GatewayFailure::Forbidden(
            "gateway request is outside the current signed policy".into(),
        ));
    }
    let publisher_node: [u8; 32] = record
        .statement
        .publisher_node
        .as_slice()
        .try_into()
        .map_err(|_| GatewayFailure::Unavailable("invalid publisher in route state".into()))?;
    let max_response_bytes = record
        .statement
        .route
        .as_ref()
        .expect("current_route rejects tombstones")
        .policy
        .max_response_bytes;
    let Some(lane) = handle.gateway.clone() else {
        return Err(GatewayFailure::Unavailable(
            "gateway request requires an active network overlay".into(),
        ));
    };
    let (reply, rx) = oneshot::channel();
    lane.send(GatewayJob {
        publisher_node,
        max_response_bytes,
        head,
        body,
        reply,
    })
    .await
    .map_err(|_| GatewayFailure::Unavailable("gateway plane is not available".into()))?;
    let response = tokio::time::timeout(Duration::from_secs(15), rx)
        .await
        .map_err(|_| GatewayFailure::Unavailable("gateway publisher timed out".into()))?
        .map_err(|_| GatewayFailure::Unavailable("gateway plane dropped the request".into()))??;
    gateway::validate_response_head(&response.head).map_err(GatewayFailure::Unavailable)?;
    if response.body.len() as u64 > max_response_bytes {
        return Err(GatewayFailure::Unavailable(
            "publisher exceeded the route response cap".into(),
        ));
    }
    Ok(response)
}

pub(crate) async fn gateway_proxy(
    State(handle): State<NodeHandle>,
    headers: HeaderMap,
    Json(request): Json<GatewayProxyRequest>,
) -> Response {
    use base64::Engine as _;
    if let Some(response) = gateway_api_origin_guard(&headers) {
        return response;
    }
    let body = match base64::engine::general_purpose::STANDARD.decode(request.body_b64) {
        Ok(body) => body,
        Err(error) => {
            return error_response(StatusCode::BAD_REQUEST, &format!("body_b64: {error}"));
        }
    };
    match proxy_current(&handle, request.head, body).await {
        Ok(response) => Json(GatewayProxyReply {
            head: response.head,
            body_b64: base64::engine::general_purpose::STANDARD.encode(response.body),
        })
        .into_response(),
        Err(failure) => gateway_failure_response(failure),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GatewaySessionRequest {
    pub account_id: Vec<u8>,
    pub name: gateway::RouteName,
    pub revision: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GatewaySessionReply {
    url: String,
}

pub(crate) async fn gateway_session(
    State(handle): State<NodeHandle>,
    headers: HeaderMap,
    Json(request): Json<GatewaySessionRequest>,
) -> Response {
    if let Some(response) = gateway_api_origin_guard(&headers) {
        return response;
    }
    let Some(gateway) = handle.browser_gateway.clone() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "gateway browser gateway is not configured",
        );
    };
    if handle.gateway.is_none() {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "gateway browsing requires an active network overlay",
        );
    }
    let record = match current_route(&handle, &request.account_id, &request.name).await {
        Ok(record) => record,
        Err(failure) => return gateway_failure_response(failure),
    };
    if record.statement.revision != request.revision {
        return error_response(
            StatusCode::CONFLICT,
            "gateway route changed; resolve the name again",
        );
    }

    let now = Instant::now();
    let mut sessions = gateway.sessions.lock().await;
    sessions.retain(|_, session| now.duration_since(session.last_used) < GATEWAY_SESSION_IDLE);
    if sessions.len() >= MAX_GATEWAY_SESSIONS
        && let Some(oldest) = sessions
            .iter()
            .min_by_key(|(_, session)| session.last_used)
            .map(|(token, _)| token.clone())
    {
        sessions.remove(&oldest);
    }
    let token = loop {
        let mut random = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut random);
        let candidate = hex_bytes(&random);
        if !sessions.contains_key(&candidate) {
            break candidate;
        }
    };
    sessions.insert(
        token.clone(),
        GatewaySession {
            account_id: request.account_id,
            name: request.name,
            revision: request.revision,
            last_used: now,
        },
    );
    Json(GatewaySessionReply {
        url: format!("http://{token}.localhost:{}/", gateway.listen.port()),
    })
    .into_response()
}

/// Dedicated gateway-rendering router: no node API, no permissive CORS, and
/// no route that can address another `.duck` route with ambient user power.
pub fn gateway_browser_router(handle: NodeHandle) -> Router {
    Router::new()
        .fallback(gateway_browser_proxy)
        .layer(DefaultBodyLimit::max(
            gateway::MAX_REQUEST_BODY_BYTES as usize,
        ))
        .with_state(handle)
}

fn gateway_method(method: &Method) -> Option<gateway::RouteMethod> {
    match *method {
        Method::GET => Some(gateway::RouteMethod::Get),
        Method::HEAD => Some(gateway::RouteMethod::Head),
        Method::POST => Some(gateway::RouteMethod::Post),
        Method::PUT => Some(gateway::RouteMethod::Put),
        Method::PATCH => Some(gateway::RouteMethod::Patch),
        Method::DELETE => Some(gateway::RouteMethod::Delete),
        _ => None,
    }
}

fn gateway_request_headers(headers: &HeaderMap) -> Result<Vec<gateway::ProxyHeader>, String> {
    if headers.contains_key(header::COOKIE) {
        return Err("gateway browser never accepts ambient Cookie credentials".into());
    }
    let mut forwarded = Vec::new();
    for name in gateway::ALLOWED_REQUEST_HEADERS {
        let values = headers.get_all(*name);
        let mut values = values.iter();
        let Some(value) = values.next() else {
            continue;
        };
        if values.next().is_some() {
            return Err(format!("gateway browser rejects duplicate {name} headers"));
        }
        forwarded.push(gateway::ProxyHeader {
            name: (*name).to_string(),
            value: value
                .to_str()
                .map_err(|_| format!("gateway browser received non-ASCII {name}"))?
                .to_string(),
        });
    }
    gateway::validate_headers(&forwarded, gateway::ALLOWED_REQUEST_HEADERS, "request")?;
    Ok(forwarded)
}

async fn gateway_browser_proxy(
    State(handle): State<NodeHandle>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Some(gateway) = handle.browser_gateway.clone() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "gateway browser is disabled",
        );
    };
    let Some(method) = gateway_method(&method) else {
        return (
            StatusCode::METHOD_NOT_ALLOWED,
            [(header::ALLOW, "GET, HEAD, POST, PUT, PATCH, DELETE")],
            "method is not part of the gateway protocol",
        )
            .into_response();
    };
    let expected_suffix = format!(".localhost:{}", gateway.listen.port());
    let Some(host) = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
    else {
        return error_response(
            StatusCode::MISDIRECTED_REQUEST,
            "invalid gateway session origin",
        );
    };
    let Some(token) = host.strip_suffix(&expected_suffix).filter(|token| {
        token.len() == 32
            && token
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }) else {
        return error_response(
            StatusCode::MISDIRECTED_REQUEST,
            "invalid gateway session origin",
        );
    };
    let session_origin = format!("http://{host}");
    if headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|origin| origin != session_origin)
    {
        return error_response(StatusCode::FORBIDDEN, "cross-origin gateway call denied");
    }
    if headers
        .get("sec-fetch-site")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|site| site != "same-origin" && site != "none")
    {
        return error_response(StatusCode::FORBIDDEN, "cross-site gateway call denied");
    }
    let now = Instant::now();
    let session = {
        let mut sessions = gateway.sessions.lock().await;
        sessions.retain(|_, session| now.duration_since(session.last_used) < GATEWAY_SESSION_IDLE);
        let Some(session) = sessions.get_mut(token) else {
            return error_response(StatusCode::GONE, "gateway session expired");
        };
        session.last_used = now;
        session.clone()
    };
    let forwarded = match gateway_request_headers(&headers) {
        Ok(headers) => headers,
        Err(error) => return error_response(StatusCode::BAD_REQUEST, &error),
    };
    let head = gateway::ProxyRequestHead {
        account_id: session.account_id,
        name: session.name,
        revision: session.revision,
        method,
        path_and_query: uri
            .path_and_query()
            .map(|value| value.as_str())
            .unwrap_or("/")
            .to_string(),
        headers: forwarded,
        body_len: body.len() as u64,
    };
    let response = match proxy_current(&handle, head, body.to_vec()).await {
        Ok(response) => response,
        Err(failure) => return gateway_failure_response(failure),
    };
    let status = StatusCode::from_u16(response.head.status).unwrap_or(StatusCode::BAD_GATEWAY);
    if status.is_informational() || status == StatusCode::SWITCHING_PROTOCOLS {
        return error_response(
            StatusCode::BAD_GATEWAY,
            "publisher returned an invalid status",
        );
    }
    let content_security_policy = format!(
        "default-src 'none'; script-src 'unsafe-inline' {session_origin}; style-src 'unsafe-inline' {session_origin}; img-src {session_origin} data: blob:; connect-src {session_origin}; font-src {session_origin} data:; media-src 'none'; frame-src 'none'; child-src 'none'; worker-src 'none'; manifest-src 'none'; object-src 'none'; form-action {session_origin}; base-uri 'none'; frame-ancestors 'none'; sandbox allow-scripts allow-same-origin allow-forms; webrtc 'block'"
    );
    let mut builder = Response::builder()
        .status(status)
        .header(header::CACHE_CONTROL, "no-store")
        .header("x-content-type-options", "nosniff")
        .header("x-dns-prefetch-control", "off")
        .header("referrer-policy", "no-referrer")
        .header("cross-origin-resource-policy", "same-origin")
        .header("cross-origin-opener-policy", "same-origin")
        .header("origin-agent-cluster", "?1")
        .header("access-control-allow-origin", &session_origin)
        .header(header::VARY, "Origin")
        .header(
            "permissions-policy",
            "accelerometer=(), camera=(), clipboard-read=(), clipboard-write=(), display-capture=(), encrypted-media=(), fullscreen=(), geolocation=(), gyroscope=(), hid=(), idle-detection=(), local-fonts=(), magnetometer=(), microphone=(), midi=(), payment=(), picture-in-picture=(), publickey-credentials-create=(), publickey-credentials-get=(), screen-wake-lock=(), serial=(), storage-access=(), usb=(), window-management=()",
        )
        .header(header::CONTENT_SECURITY_POLICY, content_security_policy);
    builder = builder.header(
        header::CONTENT_TYPE,
        gateway::header_value(&response.head.headers, "content-type")
            .unwrap_or("application/octet-stream"),
    );
    for name in ["etag", "last-modified", "location", "retry-after"] {
        if let Some(value) = gateway::header_value(&response.head.headers, name) {
            builder = builder.header(name, value);
        }
    }
    let body = if method == gateway::RouteMethod::Head
        || status == StatusCode::NO_CONTENT
        || status == StatusCode::NOT_MODIFIED
    {
        Body::empty()
    } else {
        Body::from(response.body)
    };
    builder
        .body(body)
        .unwrap_or_else(|_| error_response(StatusCode::BAD_GATEWAY, "invalid publisher response"))
}

fn gateway_failure_response(failure: GatewayFailure) -> Response {
    match failure {
        GatewayFailure::Invalid(detail) => error_response(StatusCode::BAD_REQUEST, &detail),
        GatewayFailure::Forbidden(detail) => error_response(StatusCode::FORBIDDEN, &detail),
        GatewayFailure::NotFound(detail) => error_response(StatusCode::NOT_FOUND, &detail),
        GatewayFailure::Conflict(detail) => error_response(StatusCode::CONFLICT, &detail),
        GatewayFailure::Unavailable(detail) => error_response(StatusCode::BAD_GATEWAY, &detail),
    }
}

/// Serve only isolated gateway-rendering traffic on the pre-bound loopback
/// listener. The router contains no node API.
pub async fn serve_browser_gateway(
    listener: tokio::net::TcpListener,
    handle: NodeHandle,
) -> std::io::Result<()> {
    let shutdown = handle.clone();
    axum::serve(listener, gateway_browser_router(handle))
        .with_graceful_shutdown(async move { shutdown.shutdown_requested().await })
        .await
}
