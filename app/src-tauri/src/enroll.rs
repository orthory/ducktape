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

use std::io::Cursor;
use std::net::{IpAddr, UdpSocket};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use serde::{Deserialize, Serialize};
use tiny_http::{Header, Method, Request, Response, Server};

use crate::workspaces::{last_line, run_verb};

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
    node_bin: PathBuf,
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
    sock.connect("8.8.8.8:80").map_err(|e| format!("udp connect: {e}"))?;
    Ok(sock.local_addr().map_err(|e| format!("local addr: {e}"))?.ip())
}

/// a 128-bit hex session token from OS randomness.
fn random_token() -> String {
    let mut buf = [0u8; 16];
    getrandom::getrandom(&mut buf).expect("os rng");
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// hex bytes only — reject anything else before it reaches the node verb.
fn is_hex(s: &str) -> bool {
    !s.is_empty() && s.len().is_multiple_of(2) && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn json(body: String) -> Response<Cursor<Vec<u8>>> {
    Response::from_string(body).with_header(
        Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).expect("header"),
    )
}
fn html(body: &str) -> Response<Cursor<Vec<u8>>> {
    Response::from_string(body).with_header(
        Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..])
            .expect("header"),
    )
}
fn js(body: &str) -> Response<Cursor<Vec<u8>>> {
    Response::from_string(body).with_header(
        Header::from_bytes(&b"Content-Type"[..], &b"text/javascript; charset=utf-8"[..])
            .expect("header"),
    )
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

fn read_json<T: for<'de> Deserialize<'de>>(req: &mut Request) -> Option<T> {
    let mut body = String::new();
    req.as_reader().read_to_string(&mut body).ok()?;
    serde_json::from_str(&body).ok()
}

/// POST /payload {token,new_key} → the exact hex bytes to ECDSA-sign, from the
/// node (`user-p256-payload`) — never computed here.
fn payload(req: &mut Request) -> Response<Cursor<Vec<u8>>> {
    let Some(body) = read_json::<PayloadReq>(req) else {
        return status(400);
    };
    let guard = state();
    let Some(s) = guard.as_ref() else {
        return status(410);
    };
    if body.token != s.token || !is_hex(&body.new_key) {
        return status(403);
    }
    let out = run_verb(
        &s.node_bin,
        &[
            "user-p256-payload",
            "--chain-id",
            &s.chain_id,
            "--account-id",
            &s.account_id,
            "--new-key",
            &body.new_key,
            "--nonce",
            &s.nonce.to_string(),
        ],
    );
    match out {
        Ok(out) => json(format!(r#"{{"payload":"{}"}}"#, last_line(&out))),
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
    if body.token != s.token || !is_hex(&body.new_key) || !is_hex(&body.sig) {
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
    chain_id: String,
    account_id: String,
    nonce: u64,
) -> Result<EnrollStart, String> {
    if !is_hex(&account_id) {
        return Err("account_id must be hex".into());
    }
    let node_bin = crate::daemon::resolve_node_bin()?;
    let token = random_token();
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
            node_bin,
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
pub fn enroll_poll() -> Option<(String, String)> {
    state()
        .as_ref()
        .and_then(|s| s.result.clone())
        .map(|p| (p.new_key, p.sig))
}

/// tear the enrollment server down (on success, cancel, or leaving the screen).
#[tauri::command]
pub fn enroll_cancel() {
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
            node_bin: PathBuf::from("/nonexistent"), // /payload isn't exercised here
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
        assert!(req("GET /e.js HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n").contains("200 OK"));

        // a wrong token is refused and stores nothing.
        assert!(req(&post("/possession", r#"{"token":"nope","new_key":"00ff","sig":"aabb"}"#))
            .contains("403"));
        assert!(enroll_poll().is_none());

        // the right token stores the possession for enroll_poll to hand off.
        assert!(req(&post("/possession", r#"{"token":"tok123","new_key":"00ff","sig":"aabb"}"#))
            .contains("200 OK"));
        assert_eq!(enroll_poll(), Some(("00ff".into(), "aabb".into())));

        enroll_cancel();
        let _ = handle.join();
    }
}
