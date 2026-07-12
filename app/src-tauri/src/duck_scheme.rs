//! The `duck://` scheme handler: renders gateway routes at stable origins.
//!
//! Every `duck://` request the CEF renderer makes is answered here. We are the
//! trusted mediator between the untrusted page and the local node: the page only
//! ever sees `duck://`, and we translate each request into a call to the node's
//! dedicated browser-gateway listener (never the node API on `/v1`). The node
//! resolves the authority (DuckDNS + the signed route) and proxies; we hand the
//! answer — status, the node's CSP/security headers, and body — back to CEF.
//!
//! WebSocket bootstrap (spec D6): a page cannot open a socket on its own scheme,
//! so it fetches the synthetic same-origin `duck://<host>/.duck/ws`; we mint a
//! single-use loopback token from the node and return a `ws://127.0.0.1` URL.

use std::sync::Mutex;
use std::time::Duration;

use tauri_runtime_cef::{InitiatorOrigin, StreamResponder, register_streaming_scheme_handler};

/// The dedicated browser-gateway base (`http://127.0.0.1:<ephemeral-port>`) the
/// active node reports via `/v1/gateway/browser`. The frontend sets it when a
/// workspace becomes active (`duck_set_gateway_base`); the handler reads it per
/// request. `None` until a workspace is up.
static GATEWAY_BASE: Mutex<Option<String>> = Mutex::new(None);

/// Response headers the node already vetted that we forward verbatim to CEF.
/// `set-cookie` is here and repeatable (see the `get_all` in `serve`): the node
/// forwards each host-only-scrubbed cookie, and CEF stores it against the
/// page's duck origin (the scheme is registered cookieable).
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

pub fn set_gateway_base(base: Option<String>) {
    *GATEWAY_BASE.lock().expect("gateway base poisoned") = base;
}

fn gateway_base() -> Option<String> {
    GATEWAY_BASE.lock().expect("gateway base poisoned").clone()
}

/// Register the streaming `duck` scheme handler. Call once before the Tauri
/// builder runs; `"duck"` must also be in `CefConfig::custom_schemes`.
pub fn register() {
    register_streaming_scheme_handler(
        "duck",
        Box::new(|_label, request, responder| serve(request, responder)),
    );
}

fn text_head(status: u16) -> http::Response<()> {
    // no-store: CEF caches scheme responses; a cached error (e.g. "no active
    // gateway" during boot) must never mask a later healthy load.
    http::Response::builder()
        .status(status)
        .header("content-type", "text/plain; charset=utf-8")
        .header("cache-control", "no-store")
        .body(())
        .unwrap()
}

fn fail(responder: StreamResponder, status: u16, message: &str) {
    let mut writer = responder.respond(text_head(status));
    let _ = writer.write(message.as_bytes().to_vec());
}

fn serve(request: http::Request<Vec<u8>>, responder: StreamResponder) {
    // The browser-process-tracked initiator origin (a request extension the
    // runtime stamps — never a caller header). The renderer serializes duck://
    // page origins as `Origin: null` on the wire (POST bodies, WS handshakes),
    // so the header is useless for same-origin decisions; this is the trusted
    // replacement. `None` means "not a known non-opaque frame" — fail closed.
    let initiator = request
        .extensions()
        .get::<InitiatorOrigin>()
        .and_then(|origin| origin.0.clone());
    let uri = request.uri();
    let authority = uri.host().unwrap_or_default().to_ascii_lowercase();
    if authority.is_empty() {
        return fail(responder, 400, "duck: missing authority");
    }
    let path_and_query = uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/")
        .to_string();

    let Some(base) = gateway_base() else {
        return fail(
            responder,
            503,
            "duck: no active gateway — open a workspace first",
        );
    };

    if uri.path() == "/.duck/ws" {
        // Mint strictly same-origin, judged by the TRUSTED initiator: a page
        // must never mint a door token for another authority's route (the
        // cross-account WS pivot #415 closed). The renderer's own Origin
        // header serializes as "null" here, so it cannot carry this check.
        let expected = format!("duck://{authority}");
        if initiator.as_deref() != Some(expected.as_str()) {
            return fail(responder, 403, "duck: ws bootstrap is same-origin only");
        }
        return serve_ws_bootstrap(&base, &authority, &expected, responder);
    }
    // `/.duck/` is the node's control-plane namespace (the WS-token mint and
    // door live there). A page must never reach it by proxy — only the synthetic
    // /.duck/ws above is the page's to call — or it could mint tokens for other
    // authorities. Everything else under /.duck/ is reserved, not app content.
    if uri.path().starts_with("/.duck/") {
        return fail(responder, 404, "duck: reserved path");
    }

    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
    {
        Ok(client) => client,
        Err(error) => return fail(responder, 500, &format!("duck: client error: {error}")),
    };
    let method = reqwest::Method::from_bytes(request.method().as_str().as_bytes())
        .unwrap_or(reqwest::Method::GET);
    let mut outbound = client.request(method, format!("{base}{path_and_query}"));
    // Forward the page's own request headers — Cookie, Origin (the REAL page
    // origin, so the node's cross-origin guard is meaningful), Accept,
    // Content-Type — and let the node's denylist strip the dangerous ones. The
    // one thing a page must NOT be able to set is `x-duck-*`: the node trusts
    // those as ours, so we drop any page-set copy before stamping our own
    // authority. `host`/`content-length` are reqwest's to set from the target
    // URL + body; `accept-encoding` is dropped so the backend never compresses
    // (this hop does not decode, and content-encoding is not forwarded).
    // Forward the page's own request headers — Origin included, verbatim. The
    // runtime has already repaired a renderer-serialized `Origin: null` to the
    // real duck origin, and left it ABSENT where absence is correct (a
    // navigation, a same-origin GET). Both facts matter to the node's
    // cross-origin guard, so neither is second-guessed here.
    for (name, value) in request.headers() {
        let lower = name.as_str().to_ascii_lowercase();
        if lower.starts_with("x-duck-")
            || lower == "host"
            || lower == "content-length"
            || lower == "accept-encoding"
        {
            continue;
        }
        outbound = outbound.header(name, value);
    }
    outbound = outbound.header("x-duck-authority", &authority);
    if !request.body().is_empty() {
        outbound = outbound.body(request.body().clone());
    }
    let response = match outbound.send() {
        Ok(response) => response,
        Err(error) => return fail(responder, 502, &format!("duck: gateway unreachable: {error}")),
    };

    let mut head = http::Response::builder().status(response.status().as_u16());
    for name in FORWARDED_HEADERS {
        // set-cookie is the one legitimately repeatable response header; every
        // other forwarded header appears at most once, so get_all is exact.
        for value in response.headers().get_all(*name) {
            head = head.header(*name, value);
        }
    }
    let body = response.bytes().unwrap_or_default().to_vec();
    let mut writer = responder.respond(head.body(()).expect("duck head is valid"));
    let _ = writer.write(body);
}

/// Answer `duck://<host>/.duck/ws`: mint a single-use loopback token from the
/// node and return `{ "url": "ws://127.0.0.1:<port>/.duck/ws/<token>" }` for the
/// page to open a `WebSocket` with.
fn serve_ws_bootstrap(base: &str, authority: &str, origin: &str, responder: StreamResponder) {
    let Some(port) = base
        .rsplit_once(':')
        .and_then(|(_, port)| port.parse::<u16>().ok())
    else {
        return fail(responder, 500, "duck: malformed gateway base");
    };
    let client = reqwest::blocking::Client::new();
    let minted = client
        .post(format!("{base}/.duck/ws-token"))
        .json(&serde_json::json!({ "authority": authority, "origin": origin }))
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.json::<serde_json::Value>());
    let token = match minted {
        Ok(value) => value
            .get("token")
            .and_then(|token| token.as_str())
            .map(str::to_string),
        Err(error) => return fail(responder, 502, &format!("duck: ws mint failed: {error}")),
    };
    let Some(token) = token else {
        return fail(responder, 502, "duck: ws mint returned no token");
    };
    let payload =
        serde_json::json!({ "url": format!("ws://127.0.0.1:{port}/.duck/ws/{token}") });
    let head = http::Response::builder()
        .status(200)
        .header("content-type", "application/json")
        .header("cache-control", "no-store")
        .body(())
        .unwrap();
    let mut writer = responder.respond(head);
    let _ = writer.write(payload.to_string().into_bytes());
}
