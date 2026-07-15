//! Authenticated `duck://` bridge for the direct CEF browser.
//!
//! The renderer never sees the node API. Requests are translated onto the
//! active node's dedicated loopback browser-gateway listener, stamped with the
//! trusted Duck authority, and streamed back through CEF with bounded
//! backpressure.

use std::collections::VecDeque;
use std::io::Read as _;
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, sync_channel};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use cef::*;
use reqwest::blocking::{Client, Response as UpstreamResponse};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{Method, Url};

use crate::duck_url::validate_duck_host;

const CHANNEL_DEPTH: usize = 8;
const MAX_REQUEST_BODY_BYTES: usize = 1024 * 1024;
const MAX_PATH_AND_QUERY_BYTES: usize = 2 * 1024;
const MAX_REQUEST_HEADERS: usize = 64;
const MAX_REQUEST_HEADER_BYTES: usize = 32 * 1024;
const READ_CHUNK_BYTES: usize = 32 * 1024;
const MAX_LOCAL_DOCUMENT_BYTES: usize = 64 * 1024 * 1024;
const MAX_LOCAL_DOCUMENTS: usize = 16;
const MAX_CONCURRENT_REQUESTS: usize = 32;
const MAX_WEBSOCKET_CAPABILITIES: usize = 32;
const WEBSOCKET_CAPABILITY_TTL: Duration = Duration::from_secs(30);

const LOCAL_DOCUMENT_CSP: &str = "sandbox; default-src 'none'; script-src 'none'; connect-src 'none'; img-src data:; style-src 'unsafe-inline'; font-src 'none'; media-src 'none'; frame-src 'none'; child-src 'none'; worker-src 'none'; object-src 'none'; form-action 'none'; base-uri 'none'";

const FORWARDED_HEADERS: &[&str] = &[
    "content-type",
    "content-security-policy",
    "cache-control",
    "x-content-type-options",
    "referrer-policy",
    "cross-origin-resource-policy",
    "cross-origin-opener-policy",
    "permissions-policy",
    "access-control-allow-origin",
    "vary",
    "set-cookie",
    "etag",
    "last-modified",
    "location",
    "retry-after",
];

#[derive(Debug, Clone, PartialEq, Eq)]
struct GatewayTarget {
    base: String,
    port: u16,
    generation: u64,
}

#[derive(Debug, Default)]
struct GatewayState {
    active: Option<GatewayTarget>,
    local_documents: LocalDocumentStore,
    websocket_capabilities: VecDeque<WebSocketCapability>,
}

#[derive(Debug, Default)]
struct LocalDocumentStore {
    documents: VecDeque<LocalDocument>,
    total_bytes: usize,
}

#[derive(Debug, Clone)]
struct LocalDocument {
    url: String,
    bytes: Arc<[u8]>,
    generation: u64,
}

#[derive(Debug, Clone)]
struct WebSocketCapability {
    url: String,
    origin: String,
    generation: u64,
    expires_at: std::time::Instant,
}

/// Cloneable process-local capability for the currently active workspace's
/// dedicated browser gateway. Replacing or clearing it invalidates in-flight
/// responses before their next body chunk.
#[derive(Debug, Clone)]
pub(crate) struct GatewayProxy {
    state: Arc<Mutex<GatewayState>>,
    next_generation: Arc<AtomicU64>,
    active_requests: Arc<AtomicUsize>,
}

impl Default for GatewayProxy {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(GatewayState::default())),
            next_generation: Arc::new(AtomicU64::new(1)),
            active_requests: Arc::new(AtomicUsize::new(0)),
        }
    }
}

struct RequestPermit(Arc<AtomicUsize>);

impl Drop for RequestPermit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Release);
    }
}

impl GatewayProxy {
    fn try_admit(&self) -> Option<RequestPermit> {
        self.active_requests
            .fetch_update(Ordering::Acquire, Ordering::Relaxed, |active| {
                (active < MAX_CONCURRENT_REQUESTS).then_some(active + 1)
            })
            .ok()
            .map(|_| RequestPermit(self.active_requests.clone()))
    }

    /// Revoke every clone held by the retiring workspace, then give the
    /// runtime an unrelated capability for the next workspace.
    pub(crate) fn retire_workspace(&mut self) -> Result<(), String> {
        self.set_gateway_base(None)?;
        *self = Self::default();
        Ok(())
    }

    pub(crate) fn set_gateway_base(&self, base: Option<String>) -> Result<(), String> {
        let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
        let clear_local = base.is_none();
        let active = base
            .map(|base| validate_gateway_base(&base, generation))
            .transpose()?;
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        state.active = active;
        state.websocket_capabilities.clear();
        if clear_local {
            state.local_documents = LocalDocumentStore::default();
        }
        Ok(())
    }

    pub(crate) fn install_local_document(&self, url: &str, bytes: Arc<[u8]>) -> Result<(), String> {
        if bytes.len() > MAX_LOCAL_DOCUMENT_BYTES {
            return Err("local browser document exceeds the 64 MiB limit".into());
        }
        let url = canonical_local_document_url(url)?;
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let store = &mut state.local_documents;
        let replaced = store
            .documents
            .iter()
            .position(|document| document.url == url);
        let replaced_bytes = replaced
            .and_then(|index| store.documents.get(index))
            .map_or(0, |document| document.bytes.len());
        if let Some(index) = replaced {
            store.documents.remove(index);
        }
        store.total_bytes = store.total_bytes.saturating_sub(replaced_bytes);
        while store.documents.len() >= MAX_LOCAL_DOCUMENTS
            || store.total_bytes.saturating_add(bytes.len()) > MAX_LOCAL_DOCUMENT_BYTES
        {
            let evicted = store
                .documents
                .pop_front()
                .expect("one bounded document can always fit an empty store");
            store.total_bytes = store.total_bytes.saturating_sub(evicted.bytes.len());
        }
        let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
        store.total_bytes += bytes.len();
        store.documents.push_back(LocalDocument {
            url,
            bytes,
            generation,
        });
        Ok(())
    }

    pub(crate) fn clear_local_documents(&self) {
        self.next_generation.fetch_add(1, Ordering::Relaxed);
        self.state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .local_documents = LocalDocumentStore::default();
    }

    pub(crate) fn remove_local_document(&self, url: &str) {
        let Ok(url) = canonical_local_document_url(url) else {
            return;
        };
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let store = &mut state.local_documents;
        if let Some(index) = store
            .documents
            .iter()
            .position(|document| document.url == url)
            && let Some(removed) = store.documents.remove(index)
        {
            store.total_bytes = store.total_bytes.saturating_sub(removed.bytes.len());
            self.next_generation.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn snapshot(&self) -> Option<GatewayTarget> {
        self.state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .active
            .clone()
    }

    fn is_current(&self, target: &GatewayTarget) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .active
            .as_ref()
            .is_some_and(|active| active.generation == target.generation)
    }

    fn local_document(&self, url: &str) -> Option<LocalDocument> {
        self.state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .local_documents
            .documents
            .iter()
            .find(|document| document.url == url)
            .cloned()
    }

    fn is_local_document_current(&self, document: &LocalDocument) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .local_documents
            .documents
            .iter()
            .any(|active| active.url == document.url && active.generation == document.generation)
    }

    fn remember_websocket(&self, target: &GatewayTarget, origin: &str, url: String) -> bool {
        let now = std::time::Instant::now();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if state
            .active
            .as_ref()
            .is_none_or(|active| active.generation != target.generation)
        {
            return false;
        }
        state
            .websocket_capabilities
            .retain(|capability| capability.expires_at > now);
        while state.websocket_capabilities.len() >= MAX_WEBSOCKET_CAPABILITIES {
            state.websocket_capabilities.pop_front();
        }
        state.websocket_capabilities.push_back(WebSocketCapability {
            url,
            origin: origin.to_string(),
            generation: target.generation,
            expires_at: now + WEBSOCKET_CAPABILITY_TTL,
        });
        true
    }

    pub(super) fn permits_loopback_for_origin(&self, raw_origin: &str) -> bool {
        let Some(origin) = canonical_duck_origin(raw_origin) else {
            return false;
        };
        let now = std::time::Instant::now();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let generation = state.active.as_ref().map(|active| active.generation);
        state
            .websocket_capabilities
            .retain(|capability| capability.expires_at > now);
        state.websocket_capabilities.iter().any(|capability| {
            Some(capability.generation) == generation && capability.origin == origin
        })
    }

    pub(super) fn allows_websocket(
        &self,
        raw_url: &str,
        raw_origin: Option<&str>,
        method: &str,
    ) -> bool {
        if method != "GET" {
            return false;
        }
        let Some(origin) = raw_origin.and_then(canonical_duck_origin) else {
            return false;
        };
        let Ok(url) = Url::parse(raw_url) else {
            return false;
        };
        if url.scheme() != "ws"
            || url.host_str() != Some("127.0.0.1")
            || url.port().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return false;
        }
        let canonical_url = url.to_string();
        let now = std::time::Instant::now();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let generation = state.active.as_ref().map(|active| active.generation);
        state
            .websocket_capabilities
            .retain(|capability| capability.expires_at > now);
        state.websocket_capabilities.iter().any(|capability| {
            Some(capability.generation) == generation
                && capability.origin == origin
                && capability.url == canonical_url
        })
    }

    fn serve(&self, request: ProxyRequest, responder: StreamResponder) {
        let route = match classify_request(&request) {
            Ok(route) => route,
            Err(failure) => return responder.fail(failure.status, failure.message),
        };

        match route {
            Route::LocalDocument { url, head } => self.serve_local_document(&url, head, responder),
            Route::WebSocketBootstrap { authority, origin } => {
                let Some(target) = self.snapshot() else {
                    return responder.fail(503, "duck: no active gateway — open a workspace first");
                };
                self.serve_ws_bootstrap(&target, &authority, &origin, responder)
            }
            Route::Proxy {
                authority,
                path_and_query,
            } => {
                let Some(target) = self.snapshot() else {
                    return responder.fail(503, "duck: no active gateway — open a workspace first");
                };
                self.serve_proxy(&target, &authority, &path_and_query, request, responder)
            }
        }
    }

    fn serve_local_document(&self, url: &str, head: bool, responder: StreamResponder) {
        let Some(document) = self.local_document(url) else {
            return responder.fail(404, "duck: local document is not installed");
        };
        if !self.is_local_document_current(&document) {
            return responder.fail(409, "duck: local document changed");
        }
        let mut writer = responder.respond(ResponseHead {
            status: 200,
            headers: vec![
                ("content-type".into(), "text/html; charset=utf-8".into()),
                ("content-security-policy".into(), LOCAL_DOCUMENT_CSP.into()),
                ("cache-control".into(), "no-store".into()),
                ("x-content-type-options".into(), "nosniff".into()),
                ("referrer-policy".into(), "no-referrer".into()),
                ("cross-origin-resource-policy".into(), "same-origin".into()),
                ("cross-origin-opener-policy".into(), "same-origin".into()),
                (
                    "permissions-policy".into(),
                    "camera=(), microphone=(), display-capture=(), geolocation=(), payment=(), usb=()"
                        .into(),
                ),
            ],
        });
        if head {
            return;
        }
        for chunk in document.bytes.chunks(READ_CHUNK_BYTES) {
            if !self.is_local_document_current(&document) || writer.write(chunk.to_vec()).is_err() {
                return;
            }
        }
    }

    fn serve_proxy(
        &self,
        target: &GatewayTarget,
        authority: &str,
        path_and_query: &str,
        request: ProxyRequest,
        responder: StreamResponder,
    ) {
        let client = match http_client() {
            Ok(client) => client,
            Err(()) => return responder.fail(500, "duck: could not create gateway client"),
        };
        let method = match Method::from_bytes(request.method.as_bytes()) {
            Ok(method) => method,
            Err(_) => return responder.fail(405, "duck: method is not allowed"),
        };
        let mut outbound = client.request(method, format!("{}{path_and_query}", target.base));
        for (name, value) in
            forwarded_request_headers(request.headers, request.initiator.as_deref())
        {
            outbound = outbound.header(name, value);
        }
        outbound = outbound.header("x-duck-authority", authority);
        if !request.body.is_empty() {
            outbound = outbound.body(request.body);
        }
        let response = match outbound.send() {
            Ok(response) => response,
            Err(_) => return responder.fail(502, "duck: gateway is unreachable"),
        };
        if !self.is_current(target) {
            return responder.fail(409, "duck: active workspace changed");
        }
        self.stream_response(target, response, responder);
    }

    fn stream_response(
        &self,
        target: &GatewayTarget,
        mut response: UpstreamResponse,
        responder: StreamResponder,
    ) {
        let head = ResponseHead {
            status: response.status().as_u16(),
            headers: forwarded_response_headers(response.headers()),
        };
        let mut writer = responder.respond(head);
        let mut buffer = vec![0u8; READ_CHUNK_BYTES];
        loop {
            if !self.is_current(target) {
                return;
            }
            match response.read(&mut buffer) {
                Ok(0) => return,
                Ok(count) => {
                    if writer.write(buffer[..count].to_vec()).is_err() {
                        return;
                    }
                }
                Err(_) => {
                    tracing::warn!(
                        target: "ducktape::browser",
                        reason = "gateway_body_read",
                        "gateway response stream ended with an error"
                    );
                    return;
                }
            }
        }
    }

    fn serve_ws_bootstrap(
        &self,
        target: &GatewayTarget,
        authority: &str,
        origin: &str,
        responder: StreamResponder,
    ) {
        let client = match http_client() {
            Ok(client) => client,
            Err(()) => return responder.fail(500, "duck: could not create gateway client"),
        };
        let minted = client
            .post(format!("{}/.duck/ws-token", target.base))
            .json(&serde_json::json!({ "authority": authority, "origin": origin }))
            .send()
            .and_then(reqwest::blocking::Response::error_for_status)
            .and_then(reqwest::blocking::Response::json::<serde_json::Value>);
        let token = match minted {
            Ok(value) => value
                .get("token")
                .and_then(serde_json::Value::as_str)
                .filter(|token| {
                    token.len() == 32 && token.bytes().all(|byte| byte.is_ascii_hexdigit())
                })
                .map(str::to_ascii_lowercase),
            Err(_) => return responder.fail(502, "duck: websocket token mint failed"),
        };
        let Some(token) = token else {
            return responder.fail(502, "duck: websocket token mint returned invalid data");
        };
        if !self.is_current(target) {
            return responder.fail(409, "duck: active workspace changed");
        }
        let url = format!("ws://127.0.0.1:{}/.duck/ws/{token}", target.port);
        if !self.remember_websocket(target, origin, url.clone()) {
            return responder.fail(409, "duck: active workspace changed");
        }
        let payload = serde_json::json!({ "url": url }).to_string();
        let mut writer = responder.respond(ResponseHead {
            status: 200,
            headers: vec![
                ("content-type".into(), "application/json".into()),
                ("cache-control".into(), "no-store".into()),
            ],
        });
        let _ = writer.write(payload.into_bytes());
    }
}

fn validate_gateway_base(raw: &str, generation: u64) -> Result<GatewayTarget, String> {
    let url = Url::parse(raw).map_err(|_| "gateway base is not a URL".to_string())?;
    let ip = url
        .host_str()
        .and_then(|host| host.trim_matches(['[', ']']).parse::<IpAddr>().ok())
        .filter(IpAddr::is_loopback)
        .ok_or_else(|| "gateway base must use a literal loopback address".to_string())?;
    let port = url
        .port()
        .ok_or_else(|| "gateway base must include a port".to_string())?;
    if url.scheme() != "http"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
        || port == 0
    {
        return Err("gateway base must be an uncredentialed loopback HTTP origin".into());
    }
    let host = match ip {
        IpAddr::V4(ip) => ip.to_string(),
        IpAddr::V6(ip) => format!("[{ip}]"),
    };
    Ok(GatewayTarget {
        base: format!("http://{host}:{port}"),
        port,
        generation,
    })
}

fn canonical_duck_origin(raw: &str) -> Option<String> {
    let url = Url::parse(raw).ok()?;
    if url.scheme() != "duck"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || !matches!(url.path(), "" | "/")
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    let host = url.host_str()?.to_ascii_lowercase();
    validate_duck_host(&host).ok()?;
    Some(format!("duck://{host}"))
}

fn http_client() -> Result<&'static Client, ()> {
    static CLIENT: OnceLock<Result<Client, ()>> = OnceLock::new();
    match CLIENT.get_or_init(|| {
        Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .build()
            .map_err(|_| ())
    }) {
        Ok(client) => Ok(client),
        Err(()) => Err(()),
    }
}

#[derive(Debug, Clone)]
struct ProxyRequest {
    url: String,
    method: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    initiator: Option<String>,
    main_frame: bool,
}

/// Preserve CEF's authoritative initiator and distinguish an actual
/// main-frame navigation from a subresource requested by the main frame.
pub(super) fn request_provenance(
    initiator: Option<String>,
    is_navigation: i32,
    frame_is_main: bool,
) -> (Option<String>, bool) {
    (initiator, is_navigation == 1 && frame_is_main)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Route {
    LocalDocument {
        url: String,
        head: bool,
    },
    Proxy {
        authority: String,
        path_and_query: String,
    },
    WebSocketBootstrap {
        authority: String,
        origin: String,
    },
}

#[derive(Debug, Clone, Copy)]
struct Failure {
    status: u16,
    message: &'static str,
}

fn classify_request(request: &ProxyRequest) -> Result<Route, Failure> {
    let url = Url::parse(&request.url).map_err(|_| Failure {
        status: 400,
        message: "duck: malformed URL",
    })?;
    if url.scheme() != "duck" || url.port().is_some() || !url.username().is_empty() {
        return Err(Failure {
            status: 400,
            message: "duck: malformed authority",
        });
    }
    let authority = url
        .host_str()
        .map(str::to_ascii_lowercase)
        .filter(|host| validate_duck_host(host).is_ok())
        .ok_or(Failure {
            status: 400,
            message: "duck: missing or invalid authority",
        })?;
    let path = if url.path().is_empty() {
        "/"
    } else {
        url.path()
    };
    let path_and_query = match url.query() {
        Some(query) => format!("{path}?{query}"),
        None => path.to_string(),
    };
    if !valid_path_and_query(&path_and_query) {
        return Err(Failure {
            status: 400,
            message: "duck: invalid path",
        });
    }
    if authority == "net.duck" {
        if url.query().is_some() {
            return Err(Failure {
                status: 400,
                message: "duck: net.duck does not accept query strings",
            });
        }
        if !request.main_frame {
            return Err(Failure {
                status: 403,
                message: "duck: net.duck is a main-frame document only",
            });
        }
        if !request.body.is_empty() {
            return Err(Failure {
                status: 400,
                message: "duck: net.duck request body is not allowed",
            });
        }
        if !matches!(request.method.as_str(), "GET" | "HEAD") {
            return Err(Failure {
                status: 405,
                message: "duck: net.duck permits only GET and HEAD",
            });
        }
        let url = canonical_local_document_url(&request.url).map_err(|_| Failure {
            status: 400,
            message: "duck: net.duck path is not canonical HTML",
        })?;
        return Ok(Route::LocalDocument {
            url,
            head: request.method == "HEAD",
        });
    }
    if url.path() == "/.duck/ws" {
        let expected = format!("duck://{authority}");
        if request.initiator.as_deref() != Some(expected.as_str()) {
            return Err(Failure {
                status: 403,
                message: "duck: websocket bootstrap is same-origin only",
            });
        }
        return Ok(Route::WebSocketBootstrap {
            authority,
            origin: expected,
        });
    }
    if url.path().starts_with("/.duck/") {
        return Err(Failure {
            status: 404,
            message: "duck: reserved path",
        });
    }
    if !matches!(
        request.method.as_str(),
        "GET" | "HEAD" | "POST" | "PUT" | "PATCH" | "DELETE"
    ) {
        return Err(Failure {
            status: 405,
            message: "duck: method is not allowed",
        });
    }
    let expected = format!("duck://{authority}");
    if !request.main_frame && request.initiator.as_deref() != Some(expected.as_str()) {
        return Err(Failure {
            status: 403,
            message: "duck: subresources are same-origin only",
        });
    }
    Ok(Route::Proxy {
        authority,
        path_and_query,
    })
}

fn canonical_local_document_url(raw: &str) -> Result<String, String> {
    if raw.is_empty()
        || raw.len() > MAX_PATH_AND_QUERY_BYTES + 32
        || raw
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return Err("local document URL is empty, oversized, or malformed".into());
    }
    let url = Url::parse(raw).map_err(|_| "local document URL is malformed".to_string())?;
    if url.scheme() != "duck"
        || url.host_str() != Some("net.duck")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
    {
        return Err("local document URL must be an exact net.duck origin".into());
    }
    // Inspect origin-form before `Url` can normalize dot segments away.
    let (_, rest) = raw
        .split_once("://")
        .ok_or_else(|| "local document URL is malformed".to_string())?;
    let origin_form = rest.split_once('#').map_or(rest, |(path, _)| path);
    let raw_path = origin_form
        .find('/')
        .map_or("/", |index| &origin_form[index..]);
    if raw_path.starts_with("//")
        || raw_path.contains(['\\', '?', '#', '%'])
        || raw_path
            .split('/')
            .any(|segment| matches!(segment, "." | ".."))
    {
        return Err("local document path is not canonical".into());
    }
    let path = if url.path().is_empty() {
        "/"
    } else {
        url.path()
    };
    if raw_path != path {
        return Err("local document path is not canonical".into());
    }
    if path != "/" {
        let relative = path
            .strip_prefix('/')
            .ok_or_else(|| "local document path is not origin-form".to_string())?;
        if relative.len() > 512 || !relative.to_ascii_lowercase().ends_with(".html") {
            return Err("local document path is not bounded HTML".into());
        }
        for segment in relative.split('/') {
            if segment.is_empty()
                || matches!(segment, "." | "..")
                || segment.len() > 128
                || !segment
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
            {
                return Err("local document path is not canonical".into());
            }
        }
    }
    Ok(format!("duck://net.duck{path}"))
}

fn valid_path_and_query(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_PATH_AND_QUERY_BYTES
        && value.starts_with('/')
        && !value.starts_with("//")
        && !value.contains(['\\', '#'])
        && value
            .bytes()
            .all(|byte| byte.is_ascii() && !byte.is_ascii_control())
}

fn forwarded_request_headers(
    headers: Vec<(String, String)>,
    initiator: Option<&str>,
) -> Vec<(HeaderName, HeaderValue)> {
    let mut forwarded = Vec::new();
    let first_origin = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("origin"))
        .map(|(_, value)| value.as_str());
    let repaired_origin = initiator
        .filter(|_| first_origin.is_none_or(|origin| origin == "null"))
        .and_then(|origin| HeaderValue::from_str(origin).ok());
    for (name, value) in headers {
        let lower = name.to_ascii_lowercase();
        if lower == "origin" && repaired_origin.is_some() {
            continue;
        }
        if lower.starts_with("x-duck-")
            || matches!(
                lower.as_str(),
                "host" | "content-length" | "accept-encoding"
            )
        {
            continue;
        }
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(lower.as_bytes()),
            HeaderValue::from_str(&value),
        ) {
            forwarded.push((name, value));
        }
    }
    if let Some(value) = repaired_origin {
        forwarded.push((reqwest::header::ORIGIN, value));
    }
    forwarded
}

fn forwarded_response_headers(headers: &HeaderMap) -> Vec<(String, String)> {
    let mut forwarded = Vec::new();
    for name in FORWARDED_HEADERS {
        for value in headers.get_all(*name) {
            if let Ok(value) = value.to_str() {
                forwarded.push(((*name).to_string(), value.to_string()));
            }
        }
    }
    forwarded
}

fn read_cef_request(
    request: &mut Request,
    initiator: Option<String>,
    main_frame: bool,
) -> Result<ProxyRequest, Failure> {
    let body = read_request_body(request)?;
    let headers = read_request_headers(request)?;
    Ok(ProxyRequest {
        url: CefString::from(&request.url()).to_string(),
        method: CefString::from(&request.method()).to_string(),
        headers,
        body,
        initiator,
        main_frame,
    })
}

fn read_request_body(request: &mut Request) -> Result<Vec<u8>, Failure> {
    let Some(post_data) = request.post_data() else {
        return Ok(Vec::new());
    };
    if post_data.has_excluded_elements() != 0 {
        return Err(Failure {
            status: 413,
            message: "duck: request body is not available",
        });
    }
    let mut elements = vec![None; post_data.element_count()];
    post_data.elements(Some(&mut elements));
    let mut body = Vec::new();
    for element in elements.into_iter().flatten() {
        if element.get_type().as_ref() != &cef::sys::cef_postdataelement_type_t::PDE_TYPE_BYTES {
            return Err(Failure {
                status: 400,
                message: "duck: file-backed request bodies are not allowed",
            });
        }
        let count = element.bytes_count();
        if body.len().saturating_add(count) > MAX_REQUEST_BODY_BYTES {
            return Err(Failure {
                status: 413,
                message: "duck: request body is too large",
            });
        }
        let start = body.len();
        body.resize(start + count, 0);
        let copied = element.bytes(count, body[start..].as_mut_ptr());
        body.truncate(start + copied.min(count));
    }
    Ok(body)
}

fn read_request_headers(request: &mut Request) -> Result<Vec<(String, String)>, Failure> {
    let mut map = CefStringMultimap::new();
    request.header_map(Some(&mut map));
    let mut headers = Vec::new();
    let mut bytes = 0usize;
    for (name, values) in map {
        for value in values {
            bytes = bytes.saturating_add(name.len()).saturating_add(value.len());
            if headers.len() >= MAX_REQUEST_HEADERS || bytes > MAX_REQUEST_HEADER_BYTES {
                return Err(Failure {
                    status: 431,
                    message: "duck: request headers are too large",
                });
            }
            headers.push((name.clone(), value));
        }
    }
    Ok(headers)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResponseHead {
    status: u16,
    headers: Vec<(String, String)>,
}

type WakeSlot = Arc<Mutex<Option<Box<dyn FnOnce() + Send>>>>;

struct StreamResponder {
    head: Arc<Mutex<Option<ResponseHead>>>,
    on_head: Box<dyn FnOnce() + Send>,
    body_tx: SyncSender<Vec<u8>>,
    wake: WakeSlot,
}

impl StreamResponder {
    fn respond(self, head: ResponseHead) -> StreamWriter {
        *self
            .head
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = Some(head);
        (self.on_head)();
        StreamWriter {
            body_tx: self.body_tx,
            wake: self.wake,
        }
    }

    fn fail(self, status: u16, message: &'static str) {
        let mut writer = self.respond(ResponseHead {
            status,
            headers: vec![
                ("content-type".into(), "text/plain; charset=utf-8".into()),
                ("cache-control".into(), "no-store".into()),
            ],
        });
        let _ = writer.write(message.as_bytes().to_vec());
    }
}

struct StreamWriter {
    body_tx: SyncSender<Vec<u8>>,
    wake: WakeSlot,
}

impl StreamWriter {
    fn write(&mut self, chunk: Vec<u8>) -> Result<(), ()> {
        if chunk.is_empty() {
            return Ok(());
        }
        self.body_tx.send(chunk).map_err(|_| ())?;
        self.wake();
        Ok(())
    }

    fn wake(&self) {
        let wake = self
            .wake
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .take();
        if let Some(wake) = wake {
            wake();
        }
    }
}

impl Drop for StreamWriter {
    fn drop(&mut self) {
        self.wake();
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ReadOutcome {
    Copied(usize),
    Pending,
    Done,
}

struct StreamBody {
    body_rx: Receiver<Vec<u8>>,
    leftover: Vec<u8>,
    cursor: usize,
    wake: WakeSlot,
}

impl StreamBody {
    fn read(&mut self, out: &mut [u8], wake: impl FnOnce() + Send + 'static) -> ReadOutcome {
        if out.is_empty() {
            return ReadOutcome::Pending;
        }
        if self.cursor >= self.leftover.len() {
            match self.body_rx.try_recv() {
                Ok(chunk) => {
                    self.leftover = chunk;
                    self.cursor = 0;
                }
                Err(TryRecvError::Empty) => {
                    *self
                        .wake
                        .lock()
                        .unwrap_or_else(|poison| poison.into_inner()) = Some(Box::new(wake));
                    match self.body_rx.try_recv() {
                        Ok(chunk) => {
                            self.wake
                                .lock()
                                .unwrap_or_else(|poison| poison.into_inner())
                                .take();
                            self.leftover = chunk;
                            self.cursor = 0;
                        }
                        Err(TryRecvError::Empty) => return ReadOutcome::Pending,
                        Err(TryRecvError::Disconnected) => {
                            self.wake
                                .lock()
                                .unwrap_or_else(|poison| poison.into_inner())
                                .take();
                            return ReadOutcome::Done;
                        }
                    }
                }
                Err(TryRecvError::Disconnected) => return ReadOutcome::Done,
            }
        }
        let count = (self.leftover.len() - self.cursor).min(out.len());
        out[..count].copy_from_slice(&self.leftover[self.cursor..self.cursor + count]);
        self.cursor += count;
        ReadOutcome::Copied(count)
    }
}

fn make_stream(
    on_head: Box<dyn FnOnce() + Send>,
) -> (
    StreamResponder,
    Arc<Mutex<Option<ResponseHead>>>,
    StreamBody,
) {
    let (body_tx, body_rx) = sync_channel(CHANNEL_DEPTH);
    let head = Arc::new(Mutex::new(None));
    let wake = Arc::new(Mutex::new(None));
    (
        StreamResponder {
            head: head.clone(),
            on_head,
            body_tx,
            wake: wake.clone(),
        },
        head,
        StreamBody {
            body_rx,
            leftover: Vec::new(),
            cursor: 0,
            wake,
        },
    )
}

struct ResourceState {
    head: Arc<Mutex<Option<ResponseHead>>>,
    body: StreamBody,
}

type StreamCell = Arc<Mutex<Option<ResourceState>>>;

cef::wrap_resource_handler! {
    struct DuckResourceHandler {
        proxy: GatewayProxy,
        initiator: Option<String>,
        main_frame: bool,
        stream: StreamCell,
    }

    impl ResourceHandler {
        fn process_request(
            &self,
            request: Option<&mut Request>,
            callback: Option<&mut Callback>,
        ) -> ::std::os::raw::c_int {
            let Some(request) = request else { return 0 };
            let Some(callback) = callback else { return 0 };
            let Some(permit) = self.proxy.try_admit() else {
                tracing::warn!(
                    target: "ducktape::browser",
                    reason = "gateway_request_limit",
                    "browser gateway request refused at the concurrency limit"
                );
                return 0;
            };
            let request = match read_cef_request(
                request,
                self.initiator.clone(),
                self.main_frame,
            ) {
                Ok(request) => request,
                Err(failure) => {
                    let callback = ThreadSafe(callback.clone());
                    let (responder, head, body) = make_stream(Box::new(move || {
                        callback.into_owned().cont();
                    }));
                    *self.stream.lock().unwrap_or_else(|poison| poison.into_inner()) =
                        Some(ResourceState { head, body });
                    let spawned = std::thread::Builder::new()
                        .name("duck-browser-request".into())
                        .spawn(move || {
                            let _permit = permit;
                            responder.fail(failure.status, failure.message);
                        });
                    if spawned.is_err() {
                        *self.stream.lock().unwrap_or_else(|poison| poison.into_inner()) = None;
                        return 0;
                    }
                    return 1;
                }
            };
            let callback = ThreadSafe(callback.clone());
            let (responder, head, body) = make_stream(Box::new(move || {
                callback.into_owned().cont();
            }));
            *self.stream.lock().unwrap_or_else(|poison| poison.into_inner()) =
                Some(ResourceState { head, body });
            let proxy = self.proxy.clone();
            let spawned = std::thread::Builder::new()
                .name("duck-browser-request".into())
                .spawn(move || {
                    let _permit = permit;
                    proxy.serve(request, responder);
                });
            if spawned.is_err() {
                *self.stream.lock().unwrap_or_else(|poison| poison.into_inner()) = None;
                return 0;
            }
            1
        }

        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        fn read(
            &self,
            data_out: *mut u8,
            bytes_to_read: ::std::os::raw::c_int,
            bytes_read: Option<&mut ::std::os::raw::c_int>,
            callback: Option<&mut ResourceReadCallback>,
        ) -> ::std::os::raw::c_int {
            let Ok(bytes_to_read) = usize::try_from(bytes_to_read) else { return 0 };
            let mut guard = self.stream.lock().unwrap_or_else(|poison| poison.into_inner());
            let Some(state) = guard.as_mut() else { return 0 };
            if bytes_to_read == 0 {
                if let Some(bytes_read) = bytes_read {
                    *bytes_read = 0;
                }
                return 1;
            }
            let out = unsafe { std::slice::from_raw_parts_mut(data_out, bytes_to_read) };
            let stream = self.stream.clone();
            let callback = callback.map(|callback| ThreadSafe(callback.clone()));
            let retained = ThreadSafe((data_out, bytes_to_read));
            let wake = move || {
                let (pointer, length) = retained.into_owned();
                let out = unsafe { std::slice::from_raw_parts_mut(pointer, length) };
                let mut guard = stream.lock().unwrap_or_else(|poison| poison.into_inner());
                let count = match guard.as_mut() {
                    Some(state) => match state.body.read(out, || {}) {
                        ReadOutcome::Copied(count) => count as ::std::os::raw::c_int,
                        ReadOutcome::Pending | ReadOutcome::Done => 0,
                    },
                    None => 0,
                };
                if let Some(callback) = callback {
                    callback.into_owned().cont(count);
                }
            };
            match state.body.read(out, wake) {
                ReadOutcome::Copied(count) => {
                    if let Some(bytes_read) = bytes_read {
                        *bytes_read = count as ::std::os::raw::c_int;
                    }
                    1
                }
                ReadOutcome::Pending => {
                    if let Some(bytes_read) = bytes_read {
                        *bytes_read = 0;
                    }
                    1
                }
                ReadOutcome::Done => {
                    if let Some(bytes_read) = bytes_read {
                        *bytes_read = 0;
                    }
                    0
                }
            }
        }

        fn response_headers(
            &self,
            response: Option<&mut Response>,
            response_length: Option<&mut i64>,
            _redirect_url: Option<&mut CefString>,
        ) {
            let Some(response) = response else { return };
            let guard = self.stream.lock().unwrap_or_else(|poison| poison.into_inner());
            let Some(state) = guard.as_ref() else { return };
            let head = state.head.lock().unwrap_or_else(|poison| poison.into_inner());
            let Some(head) = head.as_ref() else { return };
            response.set_status(i32::from(head.status));
            let mut headers = CefStringMultimap::new();
            let mut content_type = None;
            for (name, value) in &head.headers {
                if name.eq_ignore_ascii_case("content-type") {
                    content_type = Some(value.as_str());
                }
                headers.append(name, value);
            }
            response.set_header_map(Some(&mut headers));
            let mime = content_type
                .and_then(|value| value.split(';').next())
                .map(str::trim)
                .unwrap_or("text/plain");
            response.set_mime_type(Some(&CefString::from(mime)));
            if let Some(response_length) = response_length {
                *response_length = -1;
            }
        }

        fn cancel(&self) {
            self.stream
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .take();
        }
    }
}

pub(super) fn resource_handler(
    proxy: GatewayProxy,
    initiator: Option<String>,
    main_frame: bool,
) -> ResourceHandler {
    DuckResourceHandler::new(proxy, initiator, main_frame, Arc::new(Mutex::new(None)))
}

struct ThreadSafe<T>(T);

impl<T> ThreadSafe<T> {
    fn into_owned(self) -> T {
        self.0
    }
}

// CEF callbacks are reference-counted and the resource-handler contract
// explicitly permits completing them from the producer thread.
unsafe impl<T> Send for ThreadSafe<T> {}
unsafe impl<T> Sync for ThreadSafe<T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn request(url: &str, initiator: Option<&str>) -> ProxyRequest {
        ProxyRequest {
            url: url.into(),
            method: "GET".into(),
            headers: Vec::new(),
            body: Vec::new(),
            initiator: initiator.map(str::to_string),
            main_frame: false,
        }
    }

    fn local_request(url: &str, method: &str) -> ProxyRequest {
        ProxyRequest {
            url: url.into(),
            method: method.into(),
            headers: Vec::new(),
            body: Vec::new(),
            initiator: None,
            main_frame: true,
        }
    }

    fn serve(proxy: &GatewayProxy, request: ProxyRequest) -> (ResponseHead, Vec<u8>) {
        let (responder, head, mut stream) = make_stream(Box::new(|| {}));
        proxy.serve(request, responder);
        let head = head.lock().unwrap().clone().unwrap();
        let mut body = Vec::new();
        let mut buffer = [0u8; 1024];
        loop {
            match stream.read(&mut buffer, || {}) {
                ReadOutcome::Copied(count) => body.extend_from_slice(&buffer[..count]),
                ReadOutcome::Done => break,
                ReadOutcome::Pending => panic!("synchronous local response must not pend"),
            }
        }
        (head, body)
    }

    #[test]
    fn gateway_base_is_literal_loopback_and_generation_scoped() {
        assert!(validate_gateway_base("http://127.0.0.1:49152", 1).is_ok());
        assert!(validate_gateway_base("http://[::1]:49152", 1).is_ok());
        for rejected in [
            "http://localhost:49152",
            "http://8.8.8.8:49152",
            "https://127.0.0.1:49152",
            "http://127.0.0.1:49152/path",
            "http://user@127.0.0.1:49152",
        ] {
            assert!(validate_gateway_base(rejected, 1).is_err(), "{rejected}");
        }

        let proxy = GatewayProxy::default();
        proxy
            .set_gateway_base(Some("http://127.0.0.1:49152".into()))
            .unwrap();
        let old = proxy.snapshot().unwrap();
        proxy
            .set_gateway_base(Some("http://127.0.0.1:49153".into()))
            .unwrap();
        assert!(!proxy.is_current(&old));
    }

    #[test]
    fn retiring_workspace_rotates_the_shared_gateway_capability() {
        let mut proxy = GatewayProxy::default();
        proxy
            .set_gateway_base(Some("http://127.0.0.1:49152".into()))
            .unwrap();
        let retired_surface = proxy.clone();

        proxy.retire_workspace().unwrap();
        proxy
            .set_gateway_base(Some("http://127.0.0.1:49153".into()))
            .unwrap();

        assert!(retired_surface.snapshot().is_none());
        assert_eq!(proxy.snapshot().unwrap().port, 49153);
    }

    #[test]
    fn renderer_request_admission_is_bounded_and_released() {
        let proxy = GatewayProxy::default();
        let permits = (0..MAX_CONCURRENT_REQUESTS)
            .map(|_| proxy.try_admit().expect("request within the cap"))
            .collect::<Vec<_>>();
        assert!(proxy.try_admit().is_none());
        drop(permits);
        assert!(proxy.try_admit().is_some());
    }

    #[test]
    fn minted_websocket_is_exact_origin_target_and_generation_scoped() {
        let proxy = GatewayProxy::default();
        proxy
            .set_gateway_base(Some("http://127.0.0.1:49152".into()))
            .unwrap();
        let target = proxy.snapshot().unwrap();
        let url = "ws://127.0.0.1:49152/.duck/ws/0123456789abcdef0123456789abcdef";
        assert!(proxy.remember_websocket(&target, "duck://app.demo.duck", url.into()));

        assert!(proxy.permits_loopback_for_origin("duck://app.demo.duck/"));
        assert!(proxy.allows_websocket(url, Some("duck://app.demo.duck"), "GET"));
        assert!(!proxy.allows_websocket(url, Some("duck://other.demo.duck"), "GET"));
        assert!(!proxy.allows_websocket(
            "ws://127.0.0.1:49152/.duck/ws/ffffffffffffffffffffffffffffffff",
            Some("duck://app.demo.duck"),
            "GET"
        ));
        assert!(!proxy.allows_websocket(url, Some("duck://app.demo.duck"), "POST"));

        proxy
            .set_gateway_base(Some("http://127.0.0.1:49153".into()))
            .unwrap();
        assert!(!proxy.permits_loopback_for_origin("duck://app.demo.duck"));
        assert!(!proxy.allows_websocket(url, Some("duck://app.demo.duck"), "GET"));
    }

    #[test]
    fn net_duck_is_an_exact_main_frame_get_or_head_route() {
        assert_eq!(
            classify_request(&local_request("duck://net.duck", "GET")).unwrap(),
            Route::LocalDocument {
                url: "duck://net.duck/".into(),
                head: false,
            }
        );
        assert_eq!(
            classify_request(&local_request("duck://net.duck/docs/start.HTML", "HEAD")).unwrap(),
            Route::LocalDocument {
                url: "duck://net.duck/docs/start.HTML".into(),
                head: true,
            }
        );
        assert_eq!(
            classify_request(&local_request(
                "duck://net.duck/docs/start.html#part",
                "GET"
            ))
            .unwrap(),
            Route::LocalDocument {
                url: "duck://net.duck/docs/start.html".into(),
                head: false,
            }
        );

        for rejected in [
            "duck://net.duck/docs/../index.html",
            "duck://net.duck/%2e%2e/secret.html",
            "duck://net.duck/docs//index.html",
            "duck://net.duck/index.html?x=1",
            "duck://net.duck/app.js",
        ] {
            assert!(
                classify_request(&local_request(rejected, "GET")).is_err(),
                "accepted {rejected}"
            );
        }
        for method in ["POST", "PUT", "PATCH", "DELETE"] {
            assert!(
                classify_request(&local_request("duck://net.duck", method)).is_err(),
                "accepted {method}"
            );
        }
        let mut body = local_request("duck://net.duck", "GET");
        body.body.push(1);
        assert!(classify_request(&body).is_err());
        let mut subframe = local_request("duck://net.duck", "GET");
        subframe.main_frame = false;
        assert!(classify_request(&subframe).is_err());
    }

    #[test]
    fn local_documents_are_url_keyed_inert_and_do_not_require_a_gateway() {
        let proxy = GatewayProxy::default();
        proxy
            .install_local_document("duck://net.duck", Arc::from(&b"first"[..]))
            .unwrap();
        let first = proxy.local_document("duck://net.duck/").unwrap();
        proxy
            .install_local_document(
                "duck://net.duck/docs/second.html",
                Arc::from(&b"second"[..]),
            )
            .unwrap();

        let (head, body) = serve(&proxy, local_request("duck://net.duck", "GET"));
        assert_eq!(head.status, 200);
        assert_eq!(body, b"first");
        for required in [
            "content-security-policy",
            "cache-control",
            "x-content-type-options",
            "referrer-policy",
            "cross-origin-resource-policy",
            "cross-origin-opener-policy",
            "permissions-policy",
        ] {
            assert!(head.headers.iter().any(|(name, _)| name == required));
        }
        assert!(head.headers.iter().any(|(name, value)| {
            name == "content-security-policy" && value.contains("sandbox")
        }));

        let (_, head_body) = serve(&proxy, local_request("duck://net.duck", "HEAD"));
        assert!(head_body.is_empty());
        let (_, second) = serve(
            &proxy,
            local_request("duck://net.duck/docs/second.html", "GET"),
        );
        assert_eq!(second, b"second");
        let (missing, _) = serve(
            &proxy,
            local_request("duck://net.duck/docs/missing.html", "GET"),
        );
        assert_eq!(missing.status, 404);

        proxy
            .install_local_document("duck://net.duck", Arc::from(&b"replacement"[..]))
            .unwrap();
        assert!(!proxy.is_local_document_current(&first));
        assert!(
            proxy
                .local_document("duck://net.duck/docs/second.html")
                .is_some()
        );
        proxy.clear_local_documents();
        assert!(proxy.local_document("duck://net.duck/").is_none());
        assert!(
            proxy
                .local_document("duck://net.duck/docs/second.html")
                .is_none()
        );
    }

    #[test]
    fn local_document_store_evicts_oldest_url_at_its_count_bound() {
        let proxy = GatewayProxy::default();
        for index in 0..MAX_LOCAL_DOCUMENTS {
            proxy
                .install_local_document(&format!("duck://net.duck/{index}.html"), Arc::from([]))
                .unwrap();
        }
        proxy
            .install_local_document("duck://net.duck/overflow.html", Arc::from(&b"x"[..]))
            .unwrap();
        assert!(proxy.local_document("duck://net.duck/0.html").is_none());
        assert!(
            proxy
                .local_document("duck://net.duck/overflow.html")
                .is_some()
        );
    }

    #[test]
    fn websocket_mint_is_trusted_same_origin_and_control_namespace_is_closed() {
        let allowed = classify_request(&request(
            "duck://app.demo.duck/.duck/ws",
            Some("duck://app.demo.duck"),
        ));
        assert!(matches!(allowed, Ok(Route::WebSocketBootstrap { .. })));
        assert!(
            classify_request(&request(
                "duck://app.demo.duck/.duck/ws",
                Some("duck://other.demo.duck"),
            ))
            .is_err()
        );
        assert!(
            classify_request(&request(
                "duck://app.demo.duck/.duck/ws-token",
                Some("duck://app.demo.duck"),
            ))
            .is_err()
        );
    }

    #[test]
    fn subresources_require_an_exact_duck_initiator() {
        assert!(
            classify_request(&request(
                "duck://app.demo.duck/static/app.js",
                Some("duck://app.demo.duck"),
            ))
            .is_ok()
        );
        for initiator in [None, Some("null")] {
            let (initiator, main_frame) =
                request_provenance(initiator.map(str::to_string), 0, true);
            let mut subresource =
                request("duck://app.demo.duck/static/app.js", initiator.as_deref());
            subresource.main_frame = main_frame;
            assert!(
                classify_request(&subresource).is_err(),
                "a main-frame subresource with an opaque or missing initiator must be refused"
            );
        }
        assert!(
            classify_request(&request(
                "duck://app.demo.duck/static/app.js",
                Some("duck://other.demo.duck"),
            ))
            .is_err()
        );

        let (initiator, main_frame) = request_provenance(None, 1, true);
        let mut navigation = request("duck://app.demo.duck/", initiator.as_deref());
        navigation.main_frame = main_frame;
        assert!(classify_request(&navigation).is_ok());
    }

    #[test]
    fn request_headers_cannot_spoof_the_mediator() {
        let forwarded = forwarded_request_headers(
            vec![
                ("X-Duck-Authority".into(), "evil.duck".into()),
                ("Host".into(), "evil".into()),
                ("Accept-Encoding".into(), "gzip".into()),
                ("Cookie".into(), "a=b".into()),
                ("Origin".into(), "null".into()),
            ],
            Some("duck://app.demo.duck"),
        );
        let rendered: Vec<_> = forwarded
            .into_iter()
            .map(|(name, value)| (name.to_string(), value.to_str().unwrap().to_string()))
            .collect();
        assert_eq!(
            rendered,
            vec![
                ("cookie".into(), "a=b".into()),
                ("origin".into(), "duck://app.demo.duck".into()),
            ]
        );

        let opaque = forwarded_request_headers(vec![("Origin".into(), "null".into())], None);
        assert_eq!(opaque[0].1, "null");

        let missing = forwarded_request_headers(Vec::new(), Some("duck://app.demo.duck"));
        assert_eq!(missing[0].1, "duck://app.demo.duck");
    }

    #[test]
    fn response_headers_are_allowlisted_and_set_cookie_repeats() {
        let mut headers = HeaderMap::new();
        headers.append("set-cookie", "a=1".parse().unwrap());
        headers.append("set-cookie", "b=2".parse().unwrap());
        headers.insert("content-type", "text/html".parse().unwrap());
        headers.insert("server", "secret".parse().unwrap());
        let forwarded = forwarded_response_headers(&headers);
        assert_eq!(
            forwarded
                .iter()
                .filter(|(name, _)| name == "set-cookie")
                .count(),
            2
        );
        assert!(!forwarded.iter().any(|(name, _)| name == "server"));
    }

    #[test]
    fn stream_drains_chunks_with_partial_reads() {
        let (responder, _, mut body) = make_stream(Box::new(|| {}));
        let mut writer = responder.respond(ResponseHead {
            status: 200,
            headers: Vec::new(),
        });
        writer.write(b"hello".to_vec()).unwrap();
        writer.write(b"world".to_vec()).unwrap();
        drop(writer);
        let mut output = Vec::new();
        let mut buffer = [0u8; 3];
        loop {
            match body.read(&mut buffer, || {}) {
                ReadOutcome::Copied(count) => output.extend_from_slice(&buffer[..count]),
                ReadOutcome::Done => break,
                ReadOutcome::Pending => panic!("finished stream must not pend"),
            }
        }
        assert_eq!(output, b"helloworld");
    }

    #[test]
    fn proxy_stamps_authority_repairs_origin_and_preserves_streamed_reply() {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let address = server.server_addr().to_ip().unwrap();
        let observed = Arc::new(Mutex::new(None));
        let server_observed = observed.clone();
        let server_thread = std::thread::spawn(move || {
            let mut request = server.recv().unwrap();
            let mut body = Vec::new();
            request.as_reader().read_to_end(&mut body).unwrap();
            let headers: Vec<_> = request
                .headers()
                .iter()
                .map(|header| {
                    (
                        header.field.as_str().as_str().to_ascii_lowercase(),
                        header.value.as_str().to_string(),
                    )
                })
                .collect();
            *server_observed.lock().unwrap() = Some((
                request.method().as_str().to_string(),
                request.url().to_string(),
                headers,
                body,
            ));
            let response = tiny_http::Response::from_data(b"first-second".to_vec())
                .with_status_code(201)
                .with_header(tiny_http::Header::from_bytes("content-type", "text/plain").unwrap())
                .with_header(tiny_http::Header::from_bytes("set-cookie", "a=1").unwrap())
                .with_header(tiny_http::Header::from_bytes("set-cookie", "b=2").unwrap())
                .with_header(tiny_http::Header::from_bytes("server", "hidden").unwrap());
            request.respond(response).unwrap();
        });

        let proxy = GatewayProxy::default();
        proxy
            .set_gateway_base(Some(format!("http://{address}")))
            .unwrap();
        let (head_tx, head_rx) = mpsc::channel();
        let (responder, head, mut stream) = make_stream(Box::new(move || {
            head_tx.send(()).unwrap();
        }));
        let proxy_thread = {
            let proxy = proxy.clone();
            std::thread::spawn(move || {
                proxy.serve(
                    ProxyRequest {
                        url: "duck://app.demo.duck/api?q=1".into(),
                        method: "POST".into(),
                        headers: vec![
                            ("origin".into(), "null".into()),
                            ("x-duck-authority".into(), "evil.demo.duck".into()),
                            ("content-type".into(), "text/plain".into()),
                        ],
                        body: b"payload".to_vec(),
                        initiator: Some("duck://app.demo.duck".into()),
                        main_frame: false,
                    },
                    responder,
                );
            })
        };
        head_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let head = head.lock().unwrap().clone().unwrap();
        assert_eq!(head.status, 201);
        assert_eq!(
            head.headers
                .iter()
                .filter(|(name, _)| name == "set-cookie")
                .count(),
            2
        );
        assert!(!head.headers.iter().any(|(name, _)| name == "server"));

        let mut body = Vec::new();
        let mut buffer = [0u8; 4];
        loop {
            match stream.read(&mut buffer, || {}) {
                ReadOutcome::Copied(count) => body.extend_from_slice(&buffer[..count]),
                ReadOutcome::Pending => std::thread::yield_now(),
                ReadOutcome::Done => break,
            }
        }
        assert_eq!(body, b"first-second");
        proxy_thread.join().unwrap();
        server_thread.join().unwrap();

        let (method, path, headers, request_body) = observed.lock().unwrap().take().unwrap();
        assert_eq!(method, "POST");
        assert_eq!(path, "/api?q=1");
        assert_eq!(request_body, b"payload");
        assert!(headers.contains(&("x-duck-authority".into(), "app.demo.duck".into())));
        assert!(headers.contains(&("origin".into(), "duck://app.demo.duck".into())));
        assert!(!headers.iter().any(|(_, value)| value == "evil.demo.duck"));
    }
}
