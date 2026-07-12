//! The device-link ceremony's LAN relay — the QR path of "link this device to
//! an existing account" (account-console spec §3, whose copy/paste blobs stay
//! as the no-network fallback).
//!
//! The INVITER (the device that already holds the account) stands up an
//! EPHEMERAL, session-token-gated HTTP server on its Wi-Fi LAN and renders its
//! URL as a QR + short address. The NEW device types (or scans) that address
//! during onboarding; its app fetches the link CHALLENGE over the LAN, signs
//! possession locally, and posts the link RESPONSE straight back — replacing
//! the two manual blob pastes. Both blobs are the exact same versioned wire
//! shapes the paste path uses (`link-device.ts`); this server only relays
//! them.
//!
//! Like `enroll.rs`, this is a RELAY ONLY: the challenge it serves is public
//! data (chain id, account id, nonce, display name — the same facts the QR
//! sticker would show), and nothing lands on-chain until the inviter's UI
//! approves the response and signs `AddMemberKey` with the account key. A
//! rogue LAN peer can at most offer a candidate response the user must still
//! approve. The fragment-carried token, the bind-only-while-the-panel-is-open
//! lifetime, and strict blob shape checks are defense-in-depth on top.

use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tiny_http::{Method, Request, Response, Server};

use crate::daemon::require_main_window;
use crate::lan_http::{html, json, lan_ipv4, random_token, read_json, serve, status, token_matches};

// ── blob shapes (mirrors link-device.ts, relay-side checks only) ─────────

const CHALLENGE_PREFIX: &str = "ducktape-link-challenge-v1:";
const RESPONSE_PREFIX: &str = "ducktape-link-response-v1:";

/// both blobs are `<prefix><base64 json>` and small; anything else is
/// rejected before it is stored or returned.
const MAX_BLOB_BYTES: usize = 4096;

fn valid_blob(value: &str, prefix: &str) -> bool {
    value.len() <= MAX_BLOB_BYTES
        && value.strip_prefix(prefix).is_some_and(|body| {
            !body.is_empty()
                && body
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
        })
}

// ── session state ────────────────────────────────────────

/// one in-flight link relay. dropped (server unblocked) on cancel or restart.
struct LinkState {
    token: String,
    challenge: String,
    server: Arc<Server>,
    response: Option<String>,
}

static STATE: Mutex<Option<LinkState>> = Mutex::new(None);

fn state() -> std::sync::MutexGuard<'static, Option<LinkState>> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

// ── wire shapes ──────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkRelayStart {
    /// the URL to render as a QR / short address — carries the token in the
    /// fragment, so it never reaches the server on a page GET.
    pub url: String,
}

#[derive(Deserialize)]
struct ChallengeReq {
    token: String,
}

#[derive(Deserialize)]
struct ResponseReq {
    token: String,
    response: String,
}

// ── the served info page ─────────────────────────────────

/// what a phone that scans the QR sees: this address is meant for the app on
/// the new computer, not a browser.
const PAGE_HTML: &str = r#"<!doctype html><html><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Link a new device to your Ducktape account</title>
<style>body{font-family:system-ui;max-width:28rem;margin:2rem auto;padding:0 1rem;color:#111}</style></head>
<body><h2>Link a new device</h2>
<p>This address is for the Ducktape app on your new computer. On that computer,
choose <b>Link device</b> during setup and enter the address exactly as shown
next to the QR code.</p>
<p>To add this phone instead, use <b>Add a key from your phone</b> on your
account screen.</p></body></html>"#;

// ── request handling ─────────────────────────────────────

fn handle(req: &mut Request) -> Response<Cursor<Vec<u8>>> {
    let url = req.url().to_string();
    let path = url.split('?').next().unwrap_or("/");
    match (req.method(), path) {
        (Method::Get, "/link") => html(PAGE_HTML),
        (Method::Post, "/link/challenge") => challenge(req),
        (Method::Post, "/link/response") => response(req),
        _ => status(404),
    }
}

/// POST /link/challenge {token} → the inviter's challenge blob, verbatim.
fn challenge(req: &mut Request) -> Response<Cursor<Vec<u8>>> {
    let Some(body) = read_json::<ChallengeReq>(req) else {
        return status(400);
    };
    let guard = state();
    let Some(s) = guard.as_ref() else {
        return status(410);
    };
    if !token_matches(&s.token, &body.token) {
        return status(403);
    }
    json(serde_json::json!({ "challenge": s.challenge }).to_string())
}

/// POST /link/response {token,response} → store the new device's response blob
/// for the inviter UI to pick up via `link_relay_poll`.
fn response(req: &mut Request) -> Response<Cursor<Vec<u8>>> {
    let Some(body) = read_json::<ResponseReq>(req) else {
        return status(400);
    };
    let mut guard = state();
    let Some(s) = guard.as_mut() else {
        return status(410);
    };
    if !token_matches(&s.token, &body.token) || !valid_blob(&body.response, RESPONSE_PREFIX) {
        return status(403);
    }
    s.response = Some(body.response);
    json(r#"{"ok":true}"#.to_string())
}

// ── Tauri commands: the inviter half ─────────────────────

/// stand the link relay up for a freshly-minted challenge blob and return the
/// QR URL. replaces any previous relay session (fresh token by construction).
#[tauri::command]
pub fn link_relay_start(
    window: crate::rt::WebviewWindow,
    challenge: String,
) -> Result<LinkRelayStart, String> {
    require_main_window(&window)?;
    if !valid_blob(&challenge, CHALLENGE_PREFIX) {
        return Err("malformed link challenge".into());
    }
    let token = random_token()?;
    let ip = lan_ipv4()?;
    let server = Arc::new(Server::http("0.0.0.0:0").map_err(|e| format!("bind: {e}"))?);
    let port = server
        .server_addr()
        .to_ip()
        .ok_or("server has no ip address")?
        .port();

    {
        let mut guard = state();
        if let Some(prev) = guard.take() {
            prev.server.unblock();
        }
        *guard = Some(LinkState {
            token: token.clone(),
            challenge,
            server: server.clone(),
            response: None,
        });
    }
    thread::spawn(move || serve(server, handle));

    // token rides the fragment: it stays client-side on a page GET and is
    // sent back only on the explicit /link/challenge and /link/response calls.
    Ok(LinkRelayStart {
        url: format!("http://{ip}:{port}/link#{token}"),
    })
}

/// poll for the new device's response blob. returns `null` until it posts;
/// then the inviter reviews + approves it exactly like a pasted response.
#[tauri::command]
pub fn link_relay_poll(window: crate::rt::WebviewWindow) -> Result<Option<String>, String> {
    require_main_window(&window)?;
    Ok(link_relay_poll_inner())
}

fn link_relay_poll_inner() -> Option<String> {
    state().as_ref().and_then(|s| s.response.clone())
}

/// tear the relay down (on approve, cancel, or leaving the panel).
#[tauri::command]
pub fn link_relay_cancel(window: crate::rt::WebviewWindow) -> Result<(), String> {
    require_main_window(&window)?;
    link_relay_cancel_inner();
    Ok(())
}

fn link_relay_cancel_inner() {
    if let Some(s) = state().take() {
        s.server.unblock();
    }
}

// ── Tauri commands: the new-device half ──────────────────

/// a parsed relay address: the rebuilt origin plus the fragment token. parsing
/// is strict and total — the address arrives from a typed input box.
struct RelayAddr {
    base: String,
    token: String,
}

fn parse_link_url(raw: &str) -> Option<RelayAddr> {
    let rest = raw.trim().strip_prefix("http://")?;
    let (hostport, fragment) = rest.split_once("/link#")?;
    // the token is exactly the 32-hex shape random_token mints.
    if fragment.len() != 32 || !fragment.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    // host:port only — IPv4/IPv6/hostname characters, no path or userinfo.
    let host_ok = !hostport.is_empty()
        && hostport.len() <= 253
        && hostport
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b':' | b'-' | b'[' | b']'));
    if !host_ok {
        return None;
    }
    Some(RelayAddr {
        base: format!("http://{hostport}"),
        token: fragment.to_lowercase(),
    })
}

const ADDRESS_HINT: &str =
    "that doesn't look like a link address — it should look like http://192.168.1.23:40000/link#… (shown under the QR on your other device)";

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| format!("http client: {e}"))
}

fn relay_status_error(code: u16) -> String {
    match code {
        410 => "the link panel was closed on the other device — reopen it there and retry".into(),
        403 => "the other device didn't accept this address — retype it exactly as shown".into(),
        code => format!("the other device answered unexpectedly (HTTP {code})"),
    }
}

fn unreachable_error(err: &reqwest::Error) -> String {
    if err.is_timeout() {
        "timed out reaching the other device — are both devices on the same network?".into()
    } else {
        "couldn't reach the other device — check the address and that both devices are on the same network".into()
    }
}

/// POST to the relay and hand back the body, bounded — a rogue endpoint must
/// not stream unbounded data into the shell.
async fn post_bounded(
    client: &reqwest::Client,
    url: String,
    body: String,
) -> Result<(u16, String), String> {
    let resp = client
        .post(url)
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await
        .map_err(|e| unreachable_error(&e))?;
    let code = resp.status().as_u16();
    if resp.content_length().is_some_and(|len| len > 16 * 1024) {
        return Err("the other device sent an oversized reply".into());
    }
    let text = resp.text().await.map_err(|e| unreachable_error(&e))?;
    if text.len() > 16 * 1024 {
        return Err("the other device sent an oversized reply".into());
    }
    Ok((code, text))
}

async fn fetch_challenge_from(addr: &RelayAddr) -> Result<String, String> {
    let client = http_client()?;
    let body = serde_json::json!({ "token": addr.token }).to_string();
    let (code, text) = post_bounded(&client, format!("{}/link/challenge", addr.base), body).await?;
    if code != 200 {
        return Err(relay_status_error(code));
    }
    let parsed: serde_json::Value =
        serde_json::from_str(&text).map_err(|_| "the other device sent an unexpected reply".to_string())?;
    let challenge = parsed
        .get("challenge")
        .and_then(|v| v.as_str())
        .ok_or("the other device sent an unexpected reply")?;
    if !valid_blob(challenge, CHALLENGE_PREFIX) {
        return Err("the other device sent a malformed link challenge".into());
    }
    Ok(challenge.to_string())
}

async fn send_response_to(addr: &RelayAddr, response: &str) -> Result<(), String> {
    let client = http_client()?;
    let body = serde_json::json!({ "token": addr.token, "response": response }).to_string();
    let (code, _) = post_bounded(&client, format!("{}/link/response", addr.base), body).await?;
    if code != 200 {
        return Err(relay_status_error(code));
    }
    Ok(())
}

/// NEW device: fetch the inviter's challenge blob from a typed/scanned link
/// address. returns the same blob the paste path would receive.
#[tauri::command]
pub async fn link_fetch_challenge(
    window: crate::rt::WebviewWindow,
    url: String,
) -> Result<String, String> {
    require_main_window(&window)?;
    let addr = parse_link_url(&url).ok_or(ADDRESS_HINT)?;
    fetch_challenge_from(&addr).await
}

/// NEW device: post the signed response blob back to the inviter's relay.
#[tauri::command]
pub async fn link_send_response(
    window: crate::rt::WebviewWindow,
    url: String,
    response: String,
) -> Result<(), String> {
    require_main_window(&window)?;
    let addr = parse_link_url(&url).ok_or(ADDRESS_HINT)?;
    if !valid_blob(&response, RESPONSE_PREFIX) {
        return Err("malformed link response".into());
    }
    send_response_to(&addr, &response).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn challenge_blob() -> String {
        format!("{CHALLENGE_PREFIX}eyJjaGFpbklkIjoiY2hhaW4ifQ==")
    }
    fn response_blob() -> String {
        format!("{RESPONSE_PREFIX}eyJwdWJrZXkiOiJhYjEyIn0=")
    }

    #[test]
    fn blob_shape_is_strict() {
        assert!(valid_blob(&challenge_blob(), CHALLENGE_PREFIX));
        assert!(valid_blob(&response_blob(), RESPONSE_PREFIX));
        // wrong prefix, empty body, stray characters, oversize: all refused.
        assert!(!valid_blob(&challenge_blob(), RESPONSE_PREFIX));
        assert!(!valid_blob(CHALLENGE_PREFIX, CHALLENGE_PREFIX));
        assert!(!valid_blob(&format!("{CHALLENGE_PREFIX}ab cd"), CHALLENGE_PREFIX));
        assert!(!valid_blob(&format!("{CHALLENGE_PREFIX}<script>"), CHALLENGE_PREFIX));
        let oversize = format!("{CHALLENGE_PREFIX}{}", "A".repeat(MAX_BLOB_BYTES));
        assert!(!valid_blob(&oversize, CHALLENGE_PREFIX));
    }

    #[test]
    fn link_url_parsing_is_strict_and_total() {
        let token = "0123456789abcdef0123456789abcdef";
        let parsed = parse_link_url(&format!("  http://192.168.1.23:40000/link#{token} ")).unwrap();
        assert_eq!(parsed.base, "http://192.168.1.23:40000");
        assert_eq!(parsed.token, token);
        // uppercase hex normalizes; everything else is refused.
        assert_eq!(
            parse_link_url(&format!("http://10.0.0.2:1/link#{}", token.to_uppercase()))
                .unwrap()
                .token,
            token
        );
        assert!(parse_link_url(&format!("https://192.168.1.23:40000/link#{token}")).is_none());
        assert!(parse_link_url(&format!("http://192.168.1.23:40000/enroll#{token}")).is_none());
        assert!(parse_link_url("http://192.168.1.23:40000/link#short").is_none());
        assert!(parse_link_url(&format!("http://host/evil/link#{token}")).is_none());
        assert!(parse_link_url(&format!("http://user@host:1/link#{token}")).is_none());
        assert!(parse_link_url("ducktape-link-challenge-v1:abcd").is_none());
    }

    /// the mule: a real loopback server + the real client fns, end to end —
    /// challenge out, response back, poll hands it to the inviter.
    #[test]
    fn relay_round_trip_over_loopback() {
        let server = Arc::new(Server::http("127.0.0.1:0").expect("bind loopback"));
        let port = server.server_addr().to_ip().expect("ip").port();
        let token = "0123456789abcdef0123456789abcdef";
        *state() = Some(LinkState {
            token: token.into(),
            challenge: challenge_blob(),
            server: server.clone(),
            response: None,
        });
        let join = {
            let srv = server.clone();
            thread::spawn(move || serve(srv, handle))
        };

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");

        let good = parse_link_url(&format!("http://127.0.0.1:{port}/link#{token}")).unwrap();
        let wrong_token =
            parse_link_url(&format!("http://127.0.0.1:{port}/link#{}", "f".repeat(32))).unwrap();

        // a wrong token is refused on both endpoints and stores nothing.
        assert!(rt.block_on(fetch_challenge_from(&wrong_token)).is_err());
        assert!(rt.block_on(send_response_to(&wrong_token, &response_blob())).is_err());
        assert!(link_relay_poll_inner().is_none());

        // the right token round-trips the challenge and lands the response.
        assert_eq!(rt.block_on(fetch_challenge_from(&good)).unwrap(), challenge_blob());
        assert!(link_relay_poll_inner().is_none());
        rt.block_on(send_response_to(&good, &response_blob())).unwrap();
        assert_eq!(link_relay_poll_inner(), Some(response_blob()));

        // a malformed response blob is refused server-side even with the token.
        let raw = serde_json::json!({ "token": token, "response": "ducktape-link-response-v1:<x>" });
        assert!(rt
            .block_on(post_bounded(
                &http_client().unwrap(),
                format!("http://127.0.0.1:{port}/link/response"),
                raw.to_string(),
            ))
            .is_ok_and(|(code, _)| code == 403));

        link_relay_cancel_inner();
        let _ = join.join();
    }
}
