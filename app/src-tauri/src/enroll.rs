//! In-app LAN key enrollment — a phone adds a `secp256r1` key to the account by
//! scanning a QR, with no hosting, no system libraries, and no WebAuthn.
//!
//! The desktop stands up an EPHEMERAL, session-token-gated HTTP server on its
//! Wi-Fi LAN; the phone opens the served page over `http://<lan-ip>:<port>`
//! (an insecure context, so the page uses a pure-JS signer, not WebAuthn),
//! generates a P-256 key, asks the server for the exact bytes to sign, signs
//! them, and posts the signature back. The desktop then authorizes + submits
//! the `AddMemberKey` from the UI.
//!
//! This server is a RELAY ONLY. The bytes to sign come from the node
//! (`user-p256-payload`) — one source of truth with the on-chain verifier, the
//! page never reconstructs them — and the actual authority to admit the key is
//! the desktop `user.key` signing the add-member cert in the UI, NOT anything
//! this server does. So a rogue peer on the LAN can at most offer a candidate
//! key the user must still approve. The token, the bind-only-during-enrollment
//! lifetime, and the strict input validation are defense-in-depth on top.

use std::io::{Cursor, Read as _};
use std::net::{IpAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;

use serde::{Deserialize, Serialize};
use tiny_http::{Header, Method, Request, Response, Server};

use crate::daemon::{NodeControl, last_line, require_main_window, run_verb};

// ── session state ────────────────────────────────────────

/// the possession a phone hands back: the new key and its raw R‖S signature
/// over the node's payload, both hex.
#[derive(Clone, Serialize)]
struct Possession {
    new_key: String,
    sig: String,
}

/// one in-flight enrollment. dropped (server unblocked) on cancel or restart.
struct EnrollState {
    token: String,
    chain_id: String,
    account_id: String,
    nonce: u64,
    control: NodeControl,
    server: Arc<Server>,
    result: Option<Possession>,
}

static STATE: Mutex<Option<EnrollState>> = Mutex::new(None);

fn state() -> std::sync::MutexGuard<'static, Option<EnrollState>> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

// ── wire shapes ──────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnrollStart {
    /// the URL to render as a QR — carries the token in the fragment, so it
    /// never reaches the server on the page GET.
    pub url: String,
}

#[derive(Deserialize)]
struct PayloadReq {
    token: String,
    new_key: String,
}

#[derive(Deserialize)]
struct PossessionReq {
    token: String,
    new_key: String,
    sig: String,
}

// ── helpers ──────────────────────────────────────────────

/// the LAN-facing IPv4: connect a UDP socket at a public address (no packets
/// are sent) and read back which local interface the OS would route through.
fn lan_ipv4() -> Result<IpAddr, String> {
    let sock = UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("udp bind: {e}"))?;
    sock.connect("8.8.8.8:80")
        .map_err(|e| format!("udp connect: {e}"))?;
    Ok(sock
        .local_addr()
        .map_err(|e| format!("local addr: {e}"))?
        .ip())
}

/// a 128-bit hex session token from OS randomness.
fn random_token() -> Result<String, String> {
    let mut buf = [0u8; 16];
    getrandom::getrandom(&mut buf).map_err(|err| format!("os randomness: {err}"))?;
    Ok(buf.iter().map(|b| format!("{b:02x}")).collect())
}

/// hex bytes only — reject anything else before it reaches the node verb.
fn is_hex(s: &str) -> bool {
    !s.is_empty() && s.len().is_multiple_of(2) && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn token_matches(expected: &str, supplied: &str) -> bool {
    let expected = expected.as_bytes();
    let supplied = supplied.as_bytes();
    let mut different = expected.len() ^ supplied.len();
    for (index, byte) in expected.iter().enumerate() {
        different |= usize::from(*byte ^ supplied.get(index).copied().unwrap_or(0));
    }
    different == 0
}

fn valid_p256_key(value: &str) -> bool {
    value.len() == 66 && is_hex(value) && (value.starts_with("02") || value.starts_with("03"))
}

fn valid_compact_p256_signature(value: &str) -> bool {
    value.len() == 128 && is_hex(value)
}

fn header(name: &[u8], value: &[u8]) -> Header {
    Header::from_bytes(name, value).expect("static response header")
}

fn json(body: String) -> Response<Cursor<Vec<u8>>> {
    Response::from_string(body)
        .with_header(header(b"Content-Type", b"application/json"))
        .with_header(header(b"Cache-Control", b"no-store"))
        .with_header(header(b"X-Content-Type-Options", b"nosniff"))
}
fn html(body: &str) -> Response<Cursor<Vec<u8>>> {
    Response::from_string(body)
        .with_header(header(b"Content-Type", b"text/html; charset=utf-8"))
        .with_header(header(b"Cache-Control", b"no-store"))
        .with_header(header(b"X-Content-Type-Options", b"nosniff"))
        .with_header(header(
            b"Content-Security-Policy",
            b"default-src 'none'; script-src 'self'; style-src 'unsafe-inline'; connect-src 'self'; base-uri 'none'; form-action 'none'",
        ))
}
fn js(body: &str) -> Response<Cursor<Vec<u8>>> {
    Response::from_string(body)
        .with_header(header(b"Content-Type", b"text/javascript; charset=utf-8"))
        .with_header(header(b"Cache-Control", b"no-store"))
        .with_header(header(b"X-Content-Type-Options", b"nosniff"))
}
fn status(code: u16) -> Response<Cursor<Vec<u8>>> {
    Response::from_string("").with_status_code(code)
}

// ── the served phone page ────────────────────────────────

/// the enrollment page. bundled separately (page logic + a pure-JS P-256
/// signer) at `/e.js` — see `enroll_bundle.js`, produced by `bun build`.
const PAGE_HTML: &str = r#"<!doctype html><html><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Add this key to your Ducktape account</title>
<style>body{font-family:system-ui;max-width:28rem;margin:2rem auto;padding:0 1rem;color:#111}
button{font-size:1rem;padding:.7rem 1.2rem;border-radius:.5rem;border:0;background:#111;color:#fff}
#s{margin-top:1rem;white-space:pre-wrap}</style></head>
<body><h2>Add this device to your account</h2>
<p>This generates a key on this phone and adds it to your account. Nothing leaves your network.</p>
<button id="go">Generate &amp; add key</button><div id="s"></div>
<script type="module" src="/e.js"></script></body></html>"#;

/// the bundled phone-page logic + a pure-JS P-256 signer (@noble), one
/// self-contained file so the server has no runtime asset deps. it is a BUILT
/// artifact — regenerate after editing `app/src/enroll/enroll-page.ts` with:
///   bun build app/src/enroll/enroll-page.ts \
///     --outfile app/src-tauri/src/enroll_bundle.js --minify --target browser
const BUNDLE_JS: &str = include_str!("enroll_bundle.js");

// ── request handling ─────────────────────────────────────

fn handle(req: &mut Request) -> Response<Cursor<Vec<u8>>> {
    let url = req.url().to_string();
    let path = url.split('?').next().unwrap_or("/");
    match (req.method(), path) {
        (Method::Get, "/enroll") => html(PAGE_HTML),
        (Method::Get, "/e.js") => js(BUNDLE_JS),
        (Method::Post, "/payload") => payload(req),
        (Method::Post, "/possession") => possession(req),
        _ => status(404),
    }
}

const MAX_REQUEST_BODY_BYTES: u64 = 8 * 1024;

fn read_json<T: for<'de> Deserialize<'de>>(req: &mut Request) -> Option<T> {
    let mut body = String::new();
    req.as_reader()
        .take(MAX_REQUEST_BODY_BYTES + 1)
        .read_to_string(&mut body)
        .ok()?;
    if body.len() as u64 > MAX_REQUEST_BODY_BYTES {
        return None;
    }
    serde_json::from_str(&body).ok()
}

/// POST /payload {token,new_key} → the exact hex bytes to ECDSA-sign, from the
/// node (`user-p256-payload`) — never computed here.
fn payload(req: &mut Request) -> Response<Cursor<Vec<u8>>> {
    let Some(body) = read_json::<PayloadReq>(req) else {
        return status(400);
    };
    let (control, chain_id, account_id, nonce) = {
        let guard = state();
        let Some(s) = guard.as_ref() else {
            return status(410);
        };
        if !token_matches(&s.token, &body.token) || !valid_p256_key(&body.new_key) {
            return status(403);
        }
        (
            s.control.clone(),
            s.chain_id.clone(),
            s.account_id.clone(),
            s.nonce,
        )
    };
    let new_key = body.new_key;
    let out = control.run_blocking(move || {
        run_verb(&[
            "user-p256-payload",
            "--chain-id",
            &chain_id,
            "--account-id",
            &account_id,
            "--new-key",
            &new_key,
            "--nonce",
            &nonce.to_string(),
        ])
    });
    match out {
        Ok(out) => {
            let payload = last_line(&out);
            if payload.len() > 2048 || !is_hex(&payload) {
                return status(500);
            }
            json(serde_json::json!({ "payload": payload }).to_string())
        }
        Err(_) => status(500),
    }
}

/// POST /possession {token,new_key,sig} → store the phone's signed proof for the
/// desktop UI to pick up via `enroll_poll`.
fn possession(req: &mut Request) -> Response<Cursor<Vec<u8>>> {
    let Some(body) = read_json::<PossessionReq>(req) else {
        return status(400);
    };
    let mut guard = state();
    let Some(s) = guard.as_mut() else {
        return status(410);
    };
    if !token_matches(&s.token, &body.token)
        || !valid_p256_key(&body.new_key)
        || !valid_compact_p256_signature(&body.sig)
    {
        return status(403);
    }
    s.result = Some(Possession {
        new_key: body.new_key,
        sig: body.sig,
    });
    json(r#"{"ok":true}"#.to_string())
}

fn serve(server: Arc<Server>) {
    // `incoming_requests` ends when the server is `unblock`ed (cancel/restart).
    for mut req in server.incoming_requests() {
        let resp = handle(&mut req);
        let _ = req.respond(resp);
    }
}

// ── Tauri commands ───────────────────────────────────────

/// start an enrollment: bind an ephemeral LAN server and return the QR URL.
/// `account_id` is hex (the account this device is enrolling INTO); `nonce` is
/// its current nonce (the caller reads it from the chain).
#[tauri::command]
pub fn enroll_start(
    window: tauri::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
    chain_id: String,
    account_id: String,
    nonce: u64,
) -> Result<EnrollStart, String> {
    require_main_window(&window)?;
    if chain_id.is_empty() || chain_id.len() > 256 {
        return Err("chain_id must be between 1 and 256 bytes".into());
    }
    if account_id.len() > 130 || !is_hex(&account_id) {
        return Err("account_id must be hex".into());
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
        *guard = Some(EnrollState {
            token: token.clone(),
            chain_id,
            account_id,
            nonce,
            control: control.inner().clone(),
            server: server.clone(),
            result: None,
        });
    }
    thread::spawn(move || serve(server));

    // token rides the fragment: it stays client-side on the page GET and is
    // sent back only on the explicit /context, /payload, /possession calls.
    Ok(EnrollStart {
        url: format!("http://{ip}:{port}/enroll#{token}"),
    })
}

/// poll for the phone's result. returns `null` until the phone posts its
/// signature; then the caller signs the add-member authorizer + submits.
#[tauri::command]
pub fn enroll_poll(window: tauri::WebviewWindow) -> Result<Option<(String, String)>, String> {
    require_main_window(&window)?;
    Ok(enroll_poll_inner())
}

fn enroll_poll_inner() -> Option<(String, String)> {
    state()
        .as_ref()
        .and_then(|s| s.result.clone())
        .map(|p| (p.new_key, p.sig))
}

/// tear the enrollment server down (on success, cancel, or leaving the screen).
#[tauri::command]
pub fn enroll_cancel(window: tauri::WebviewWindow) -> Result<(), String> {
    require_main_window(&window)?;
    enroll_cancel_inner();
    Ok(())
}

fn enroll_cancel_inner() {
    if let Some(s) = state().take() {
        s.server.unblock();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_hex_accepts_only_even_length_hex() {
        assert!(is_hex("00ff"));
        assert!(is_hex("deadbeef"));
        assert!(!is_hex(""));
        assert!(!is_hex("abc")); // odd length
        assert!(!is_hex("zz")); // non-hex
        assert!(!is_hex("00 ff")); // space
    }

    #[test]
    fn enrollment_auth_and_crypto_fields_are_strictly_shaped() {
        assert!(token_matches("0123456789abcdef", "0123456789abcdef"));
        assert!(!token_matches("0123456789abcdef", "0123456789abcdee"));
        assert!(!token_matches("0123456789abcdef", "0123456789abcdef00"));
        assert!(!token_matches("0123456789abcdef", "01234567"));

        let compressed = format!("02{}", "11".repeat(32));
        assert!(valid_p256_key(&compressed));
        assert!(valid_p256_key(&format!("03{}", "aa".repeat(32))));
        assert!(!valid_p256_key(&format!("04{}", "11".repeat(32))));
        assert!(!valid_p256_key(&format!("02{}", "11".repeat(31))));
        assert!(valid_compact_p256_signature(&"22".repeat(64)));
        assert!(!valid_compact_p256_signature(&"22".repeat(63)));
    }

    #[test]
    fn server_serves_the_page_gates_on_token_and_stores_possession() {
        use std::io::{Read, Write};
        use std::net::TcpStream;

        let server = Arc::new(Server::http("127.0.0.1:0").expect("bind loopback"));
        let port = server.server_addr().to_ip().expect("ip").port();
        *state() = Some(EnrollState {
            token: "tok123".into(),
            chain_id: "chain".into(),
            account_id: "aa".into(),
            nonce: 0,
            control: NodeControl::new().unwrap(), // /payload isn't exercised here
            server: server.clone(),
            result: None,
        });
        let handle = {
            let srv = server.clone();
            thread::spawn(move || serve(srv))
        };

        let req = |raw: &str| -> String {
            let mut s = TcpStream::connect(("127.0.0.1", port)).expect("connect");
            s.write_all(raw.as_bytes()).expect("write");
            let mut resp = String::new();
            s.read_to_string(&mut resp).expect("read");
            resp
        };
        let post = |path: &str, body: &str| {
            format!(
                "POST {path} HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
        };

        // the page + its bundled signer are served.
        let page = req("GET /enroll HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n");
        assert!(page.contains("200 OK") && page.contains("Add this device"));
        assert!(
            req("GET /e.js HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n").contains("200 OK")
        );

        let new_key = format!("02{}", "11".repeat(32));
        let sig = "22".repeat(64);

        // a wrong token is refused and stores nothing.
        let wrong = serde_json::json!({ "token": "nope", "new_key": new_key, "sig": sig });
        assert!(req(&post("/possession", &wrong.to_string())).contains("403"));
        assert!(enroll_poll_inner().is_none());

        // oversized bodies are refused before JSON parsing or state mutation.
        let oversized = "x".repeat(MAX_REQUEST_BODY_BYTES as usize + 1);
        assert!(req(&post("/possession", &oversized)).contains("400"));
        assert!(enroll_poll_inner().is_none());

        // the right token stores the possession for enroll_poll to hand off.
        let accepted = serde_json::json!({
            "token": "tok123",
            "new_key": new_key,
            "sig": sig,
        });
        assert!(req(&post("/possession", &accepted.to_string())).contains("200 OK"));
        assert_eq!(
            enroll_poll_inner(),
            Some((new_key.to_string(), sig.to_string()))
        );

        enroll_cancel_inner();
        let _ = handle.join();
    }
}
