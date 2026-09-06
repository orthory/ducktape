//! the client half of the WebAuthn relying-party page.
//!
//! Nothing here talks WebAuthn: the browser does (`ops/auth-page/index.html`
//! at [`AUTH_PAGE`], RP ID = its own host). A client
//!
//! 1. builds a [`Request`] and opens the browser to [`request_url`] — the
//!    request rides the URL FRAGMENT, so it never reaches a server;
//! 2. binds a one-shot [`Listener`] on loopback; the page's result arrives as
//!    a top-level form POST (`result=<JSON>`) — a navigation, which Chrome's
//!    local-network-access rules and CORS both leave alone, unlike a `fetch`;
//!    or, when the request is shown as a QR for a phone, mints a [`Relay`]
//!    slot on the auth host and polls it for the same result;
//! 3. turns the [`Outcome`] into signed bytes the node accepts: a frame whose
//!    origin is the passkey/wallet ([`passkey_frame`], [`wallet_frame`]) or a
//!    passkey's consent to admit this device ([`login_consent`]).
//!
//! The contract is `ops/auth-page/README.md`; the verifier it must satisfy is
//! `keyscheme` (`Secp256r1` = the assertion envelope, `Secp256k1` =
//! `personal_sign` over [`keyscheme::personal_message`]).
//!
//! Three ceremony facts every caller sequences around:
//! - a passkey REGISTRATION is two touches: `create` yields the public key,
//!   then `get` over the `AddKey` frame preimage proves possession (a
//!   `webauthn.create` attestation carries no signature we can verify);
//! - a wallet is two touches: it reveals no public key on its own, so touch 1
//!   signs [`reveal_message`] and the key is recovered from that signature,
//!   touch 2 signs the real preimage;
//! - a LOGIN is two touches for the same reason: an add-key consent names the
//!   account it admits into, and a passkey only says which account it belongs
//!   to by answering ([`account_request`] -> [`assertion_account`]). Touch 1
//!   asks; touch 2 ([`login_request`]) is the consent, bound to that answer.

use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use keyscheme::KeyScheme;

/// the live page. Its host IS the RP ID every passkey is scoped to — changing
/// it invalidates every registered passkey (acceptable at zero live networks).
pub const AUTH_PAGE: &str = "https://auth.ducktape.industries/";

/// the domain tag of a wallet's key-reveal touch: the wallet signs this ‖ 16
/// random bytes, and the client recovers its public key from the signature.
/// Nothing on chain ever verifies a reveal signature, so it authorizes nothing.
pub const REVEAL_NS: &[u8] = b"ducktape:reveal-key:v1";

/// the largest form body the listener reads — an assertion is a few KiB.
const MAX_BODY_BYTES: usize = 256 * 1024;

/// one ceremony, as the page's fragment names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    /// `navigator.credentials.create()` — a new passkey for account `user`.
    /// The challenge is pass-through (a create attestation proves nothing
    /// here), so any 32 bytes do — [`create_challenge`].
    Create {
        challenge: [u8; 32],
        user: u64,
        name: String,
    },
    /// `navigator.credentials.get()` with `allowCredentials: []` — the
    /// discoverable passkey signs `challenge` (already `SHA-256(ns ‖ preimage)`).
    Get { challenge: [u8; 32] },
    /// `personal_sign(message)` — the EXACT bytes, un-hashed; the wallet
    /// applies the EIP-191 envelope itself.
    Eth { message: Vec<u8> },
}

/// the page's result, decoded. The `create` attestation object is not kept:
/// possession is proven by the `get` that follows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Create {
        credential_id: Vec<u8>,
        /// the 33-byte compressed SEC1 point (the page lifts it out of SPKI).
        public_key: Vec<u8>,
    },
    Get {
        authenticator_data: Vec<u8>,
        client_data_json: Vec<u8>,
        /// raw `R‖S`, 64 bytes (the page normalizes DER away).
        signature: Vec<u8>,
        /// the account number a registration wrote as `user.id`; `None` for a
        /// credential registered without one.
        user_handle: Option<u64>,
    },
    Eth {
        address: String,
        /// `r‖s‖v`, 65 bytes.
        signature: Vec<u8>,
        /// the bytes the wallet was handed (echoed, so a client can check
        /// the touch answered THIS request).
        message: Vec<u8>,
    },
}

// ============================================================================
// the request URL
// ============================================================================

/// the URL to open: `page#op=…&challenge=…[&user=…&name=…]&cb=<callback>`.
/// Binary fields base64url without padding, `name` and `cb` percent-encoded.
pub fn request_url(page: &str, request: &Request, callback: &str) -> String {
    let mut params: Vec<String> = Vec::new();
    match request {
        Request::Create {
            challenge,
            user,
            name,
        } => {
            params.push("op=create".into());
            params.push(format!("challenge={}", B64.encode(challenge)));
            params.push(format!("user={}", B64.encode(user.to_le_bytes())));
            params.push(format!("name={}", url_encode(name)));
        }
        Request::Get { challenge } => {
            params.push("op=get".into());
            params.push(format!("challenge={}", B64.encode(challenge)));
        }
        Request::Eth { message } => {
            params.push("op=eth".into());
            params.push(format!("challenge={}", B64.encode(message)));
        }
    }
    params.push(format!("cb={}", url_encode(callback)));
    format!("{page}#{}", params.join("&"))
}

/// percent-encode everything but the unreserved set; the page decodes with
/// `URLSearchParams`, which understands both this and the raw form.
fn url_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        let unreserved = byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~');
        if unreserved {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// a fresh pass-through challenge for a `create` — not verified by anything,
/// distinct per ceremony so a replayed result is recognizable as stale.
pub fn create_challenge() -> [u8; 32] {
    rand::random()
}

/// the bytes a wallet signs to reveal its key: [`REVEAL_NS`] ‖ 16 random bytes.
pub fn reveal_message() -> Vec<u8> {
    let nonce: [u8; 16] = rand::random();
    let mut message = REVEAL_NS.to_vec();
    message.extend_from_slice(&nonce);
    message
}

// ============================================================================
// the result
// ============================================================================

/// decode one result JSON line into an [`Outcome`]; the page's
/// `{"op","error","message"}` failure shape is an `Err` naming both.
pub fn parse_result(json: &str) -> Result<Outcome, String> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("auth page result is not JSON: {e}"))?;
    let op = value["op"].as_str().unwrap_or_default().to_string();
    if let Some(error) = value.get("error") {
        return Err(format!(
            "the {op} ceremony failed: {}: {}",
            error.as_str().unwrap_or("error"),
            value["message"].as_str().unwrap_or_default()
        ));
    }
    match op.as_str() {
        "create" => {
            // the browser is a trust boundary: the page compresses the SPKI
            // point itself, and a key spelled any other way is one the chain
            // would refuse as an origin AFTER a consent and a second touch had
            // already been spent on it.
            let public_key = binary(&value, "publicKey")?;
            if !KeyScheme::Secp256r1.pubkey_wellformed(&public_key) {
                return Err(format!(
                    "the auth page returned {} bytes that are not a compressed SEC1 P-256 point",
                    public_key.len()
                ));
            }
            Ok(Outcome::Create {
                credential_id: binary(&value, "credentialId")?,
                public_key,
            })
        }
        "get" => Ok(Outcome::Get {
            authenticator_data: binary(&value, "authenticatorData")?,
            client_data_json: binary(&value, "clientDataJSON")?,
            signature: binary(&value, "signature")?,
            user_handle: user_handle(&value)?,
        }),
        "eth" => Ok(Outcome::Eth {
            address: value["address"].as_str().unwrap_or_default().to_string(),
            signature: hex_0x(value["signature"].as_str().unwrap_or_default())?,
            message: binary(&value, "message")?,
        }),
        other => Err(format!("auth page result names an unknown op {other:?}")),
    }
}

fn binary(value: &serde_json::Value, field: &str) -> Result<Vec<u8>, String> {
    let Some(text) = value[field].as_str() else {
        return Err(format!("auth page result is missing {field:?}"));
    };
    B64.decode(text)
        .map_err(|e| format!("auth page result field {field:?} is not base64url: {e}"))
}

fn user_handle(value: &serde_json::Value) -> Result<Option<u64>, String> {
    let Some(text) = value["userHandle"].as_str() else {
        return Ok(None);
    };
    let bytes = B64
        .decode(text)
        .map_err(|e| format!("auth page userHandle is not base64url: {e}"))?;
    let Ok(le) = <[u8; 8]>::try_from(bytes.as_slice()) else {
        return Err("auth page userHandle is not an 8-byte account number".into());
    };
    Ok(Some(u64::from_le_bytes(le)))
}

fn hex_0x(text: &str) -> Result<Vec<u8>, String> {
    let hex = text.strip_prefix("0x").unwrap_or(text);
    let odd = !hex.len().is_multiple_of(2);
    if odd {
        return Err("auth page eth signature has odd hex length".into());
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&hex[i..i + 2], 16)
                .map_err(|e| format!("auth page eth signature is not hex: {e}"))
        })
        .collect()
}

// ============================================================================
// the loopback callback
// ============================================================================

/// a one-shot loopback HTTP listener the page delivers its result to. Bound
/// on an ephemeral 127.0.0.1 port; [`Listener::wait`] serves exactly one
/// result POST and returns.
pub struct Listener {
    listener: TcpListener,
}

impl Listener {
    pub fn bind() -> std::io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        Ok(Self { listener })
    }

    /// the `cb` to put in the request URL — loopback, which is all the page
    /// will deliver to.
    pub fn callback_url(&self) -> String {
        let port = self
            .listener
            .local_addr()
            .map(|addr| addr.port())
            .unwrap_or_default();
        format!("http://127.0.0.1:{port}/")
    }

    /// block until the page POSTs a result (a stray GET — a favicon probe, a
    /// tab reload — is answered and ignored), then answer it and return.
    pub fn wait(self) -> Result<Outcome, String> {
        loop {
            let (stream, _) = self
                .listener
                .accept()
                .map_err(|e| format!("auth callback listener: {e}"))?;
            if let Some(outcome) = serve_one(stream)? {
                return Ok(outcome);
            }
        }
    }
}

/// one HTTP exchange: `Some(outcome)` for a result POST, `None` for anything
/// else (answered with a holding page).
fn serve_one(mut stream: TcpStream) -> Result<Option<Outcome>, String> {
    let (method, body) = read_request(&mut stream)?;
    if method != "POST" {
        respond(&mut stream, 200, "Waiting for the ceremony to finish…");
        return Ok(None);
    }
    let Some(result) = form_field(&body, "result") else {
        respond(&mut stream, 400, "The callback carried no result.");
        return Err("auth page POSTed no `result` field".into());
    };
    match parse_result(&result) {
        Ok(outcome) => {
            respond(&mut stream, 200, "Done — you can return to ducktape.");
            Ok(Some(outcome))
        }
        Err(message) => {
            respond(
                &mut stream,
                200,
                "The ceremony did not complete; ducktape has the details.",
            );
            Err(message)
        }
    }
}

/// the request line's method and the body (`content-length` bounded).
fn read_request(stream: &mut TcpStream) -> Result<(String, Vec<u8>), String> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|e| format!("auth callback: {e}"))?;
    let method = line
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_string();
    let mut content_length = 0usize;
    loop {
        line.clear();
        let read = reader
            .read_line(&mut line)
            .map_err(|e| format!("auth callback: {e}"))?;
        let end_of_headers = read == 0 || line == "\r\n" || line == "\n";
        if end_of_headers {
            break;
        }
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.eq_ignore_ascii_case("content-length") {
            content_length = value.trim().parse().unwrap_or(0);
        }
    }
    if content_length > MAX_BODY_BYTES {
        return Err(format!("auth callback body exceeds {MAX_BODY_BYTES} bytes"));
    }
    let mut body = vec![0u8; content_length];
    reader
        .read_exact(&mut body)
        .map_err(|e| format!("auth callback body: {e}"))?;
    Ok((method, body))
}

fn respond(stream: &mut TcpStream, status: u16, text: &str) {
    let reason = match status {
        200 => "OK",
        _ => "Bad Request",
    };
    // the same card the page and the relay's "Done" wear (ops/auth-page).
    let html = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>ducktape</title>\
         <style>:root{{color-scheme:light dark;--fg:#1b1b1f;--bg:#f3f3f6;--card:#fff;--muted:#6b6b76;--line:#e2e2e8}}\
         @media (prefers-color-scheme:dark){{:root{{--fg:#ececf1;--bg:#111114;--card:#1b1b20;--muted:#9a9aa6;--line:#2a2a33}}}}\
         body{{margin:0;min-height:100vh;display:grid;place-items:center;background:var(--bg);color:var(--fg);\
         font:16px/1.5 system-ui,-apple-system,\"Segoe UI\",sans-serif}}\
         main{{width:min(26rem,calc(100vw - 2rem));background:var(--card);border:1px solid var(--line);border-radius:16px;padding:2rem}}\
         .brand{{font-size:.8rem;font-weight:600;letter-spacing:.06em;text-transform:uppercase;color:var(--muted)}}\
         p{{margin:1rem 0 0}}</style></head>\
         <body><main><div class=\"brand\">🦆 ducktape</div><p>{text}</p></main></body></html>"
    );
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{html}",
        html.len()
    );
    // the page has already delivered its result; a peer that hung up before
    // reading the acknowledgement lost nothing.
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

/// one `application/x-www-form-urlencoded` field, decoded.
fn form_field(body: &[u8], name: &str) -> Option<String> {
    body.split(|b| *b == b'&').find_map(|pair| {
        let (key, value) = split_once(pair, b'=')?;
        let matches = form_decode(key) == name.as_bytes();
        if !matches {
            return None;
        }
        String::from_utf8(form_decode(value)).ok()
    })
}

fn split_once(bytes: &[u8], sep: u8) -> Option<(&[u8], &[u8])> {
    let at = bytes.iter().position(|b| *b == sep)?;
    Some((&bytes[..at], &bytes[at + 1..]))
}

/// `+` → space, `%XX` → byte; a malformed escape is kept verbatim.
fn form_decode(value: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(value.len());
    let mut i = 0;
    while i < value.len() {
        let byte = value[i];
        let escape = byte == b'%' && i + 2 < value.len();
        if byte == b'+' {
            out.push(b' ');
            i += 1;
            continue;
        }
        if escape {
            let hex = std::str::from_utf8(&value[i + 1..i + 3]).unwrap_or_default();
            if let Ok(decoded) = u8::from_str_radix(hex, 16) {
                out.push(decoded);
                i += 3;
                continue;
            }
        }
        out.push(byte);
        i += 1;
    }
    out
}

/// give up on a ceremony: deliver an error result to `callback_url` ourselves,
/// so a [`Listener::wait`] blocked on it returns `Err` and its thread ends —
/// the one way to unblock a std accept. Best-effort; a listener already gone
/// needs nothing.
pub fn abandon(callback_url: &str, reason: &str) {
    let Some(port) = callback_url
        .trim_start_matches("http://127.0.0.1:")
        .trim_end_matches('/')
        .parse::<u16>()
        .ok()
    else {
        return;
    };
    let Ok(mut stream) = TcpStream::connect((Ipv4Addr::LOCALHOST, port)) else {
        return;
    };
    let result = serde_json::json!({ "op": "", "error": "abandoned", "message": reason });
    let body = format!("result={}", url_encode(&result.to_string()));
    let request = format!(
        "POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/x-www-form-urlencoded\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(request.as_bytes());
}

/// open `url` in the system browser; `false` when no opener is available (a
/// headless box), in which case the caller prints the URL for a human.
pub fn open_browser(url: &str) -> bool {
    let attempts: &[(&str, &[&str])] = if cfg!(target_os = "macos") {
        &[("open", &[])]
    } else if cfg!(target_os = "windows") {
        &[("cmd", &["/C", "start", ""])]
    } else {
        &[("xdg-open", &[])]
    };
    attempts.iter().any(|(program, args)| {
        std::process::Command::new(program)
            .args(*args)
            .arg(url)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .is_ok()
    })
}

// ============================================================================
// the relay callback — a ceremony that ran on a phone
// ============================================================================

/// how often [`Relay::wait`] asks the auth host whether the phone answered.
pub const RELAY_POLL: Duration = Duration::from_millis(1500);

/// the auth host's `/r/<id>` slot the page POSTs to when the ceremony ran on
/// a phone that cannot reach this machine (the app showed the request as a
/// QR); [`Relay::wait`] polls it. Contract: `ops/auth-page/README.md` §Relay.
pub struct Relay {
    base: String,
    /// 32 random bytes, base64url — unguessable, so nobody else can poll it.
    pub id: String,
}

impl Default for Relay {
    fn default() -> Self {
        Self::new()
    }
}

impl Relay {
    /// at the live page.
    pub fn new() -> Self {
        Self::at(AUTH_PAGE)
    }

    /// at another deployment (`--auth-page`, tests). `base` ends with `/`.
    pub fn at(base: &str) -> Self {
        let raw: [u8; 32] = rand::random();
        Self {
            base: base.to_string(),
            id: B64.encode(raw),
        }
    }

    /// the `cb` to put in the request URL — the page accepts its own origin's
    /// `/r/<id>`.
    pub fn callback_url(&self) -> String {
        format!("{}r/{}", self.base, self.id)
    }

    /// block until the phone's result lands (200) or `deadline` passes; a 204
    /// is "not yet". Blocking on purpose — callers run it on a blocking
    /// thread, exactly like [`Listener::wait`].
    pub fn wait(self, deadline: Duration) -> Result<Outcome, String> {
        self.wait_reporting(deadline, |_| {})
    }

    /// [`Relay::wait`] that tells `progress` how long the code has left
    /// before every poll — a terminal's countdown line.
    pub fn wait_reporting(
        self,
        deadline: Duration,
        mut progress: impl FnMut(Duration),
    ) -> Result<Outcome, String> {
        let url = self.callback_url();
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| format!("relay client: {e}"))?;
        let started = std::time::Instant::now();
        let mut polls: u32 = 0;
        loop {
            polls += 1;
            progress(deadline.saturating_sub(started.elapsed()));
            let response = client.get(&url).send().map_err(|e| {
                tracing::warn!(target: "ducktape::auth", event = "relay_unreachable", relay = %self.id, polls, error = %e);
                format!("relay: {e}")
            })?;
            match response.status().as_u16() {
                200 => {
                    let body = response.text().map_err(|e| format!("relay body: {e}"))?;
                    let outcome = parse_result(&body);
                    tracing::info!(
                        target: "ducktape::auth",
                        event = "relay_answered",
                        relay = %self.id,
                        polls,
                        waited_ms = started.elapsed().as_millis() as u64,
                        ok = outcome.is_ok(),
                    );
                    return outcome;
                }
                204 => {
                    tracing::trace!(target: "ducktape::auth", event = "relay_poll", relay = %self.id, polls)
                }
                other => {
                    tracing::warn!(target: "ducktape::auth", event = "relay_refused", relay = %self.id, polls, status = other);
                    return Err(format!("relay answered {other}"));
                }
            }
            let elapsed = started.elapsed();
            if elapsed >= deadline {
                tracing::warn!(target: "ducktape::auth", event = "relay_timeout", relay = %self.id, polls);
                return Err("the phone did not answer in time".into());
            }
            std::thread::sleep(RELAY_POLL.min(deadline - elapsed));
        }
    }
}

/// `m:ss` of a duration, whole seconds — the countdown beside a QR.
pub fn countdown(left: Duration) -> String {
    let secs = left.as_secs();
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// the ceremony URL as a QR a phone can scan OFF A TERMINAL: half-block
/// cells (two module rows per text line), a quiet zone, dark modules drawn in
/// the terminal's foreground. A ~280-byte URL comes out around 37 lines by
/// 77 columns — inside an 80-column terminal.
pub fn terminal_qr(text: &str) -> Result<String, String> {
    let code = qrcode::QrCode::with_error_correction_level(text, qrcode::EcLevel::L)
        .map_err(|e| format!("qr: {e}"))?;
    Ok(code
        .render::<qrcode::render::unicode::Dense1x2>()
        .dark_color(qrcode::render::unicode::Dense1x2::Light)
        .light_color(qrcode::render::unicode::Dense1x2::Dark)
        .quiet_zone(true)
        .build())
}

// ============================================================================
// ceremony builders — pure
// ============================================================================

/// the `get` a passkey answers to sign an op frame AS ITS ORIGIN, and the
/// preimage the answer completes ([`passkey_frame`]).
pub fn passkey_frame_request(pubkey: &[u8], seq: u64, msg: &sdk::Msg) -> (Request, Vec<u8>) {
    let preimage = node::frame_preimage(KeyScheme::Secp256r1, pubkey, seq, msg);
    let challenge = keyscheme::webauthn_challenge(node::FRAME_NS, &preimage);
    (Request::Get { challenge }, preimage)
}

/// the frame: preimage ‖ the assertion envelope — what `/v1/submit/frame`
/// verifies under `Secp256r1`.
pub fn passkey_frame(mut preimage: Vec<u8>, outcome: &Outcome) -> Result<Vec<u8>, String> {
    let Outcome::Get {
        authenticator_data,
        client_data_json,
        signature,
        ..
    } = outcome
    else {
        return Err("expected a passkey assertion (op=get)".into());
    };
    preimage.extend_from_slice(&keyscheme::webauthn_proof(
        authenticator_data,
        client_data_json,
        signature,
    ));
    Ok(preimage)
}

/// the `personal_sign` a wallet answers to sign an op frame AS ITS ORIGIN,
/// and the preimage the answer completes ([`wallet_frame`]).
pub fn wallet_frame_request(pubkey: &[u8], seq: u64, msg: &sdk::Msg) -> (Request, Vec<u8>) {
    let preimage = node::frame_preimage(KeyScheme::Secp256k1, pubkey, seq, msg);
    let message = keyscheme::personal_message(node::FRAME_NS, &preimage);
    (Request::Eth { message }, preimage)
}

/// the frame: preimage ‖ `r‖s‖v` — what `/v1/submit/frame` verifies under
/// `Secp256k1` (recovery against the origin).
pub fn wallet_frame(mut preimage: Vec<u8>, outcome: &Outcome) -> Result<Vec<u8>, String> {
    let Outcome::Eth { signature, .. } = outcome else {
        return Err("expected a wallet signature (op=eth)".into());
    };
    if signature.len() != 65 {
        return Err(format!(
            "a wallet signature is 65 bytes (r‖s‖v), got {}",
            signature.len()
        ));
    }
    preimage.extend_from_slice(signature);
    Ok(preimage)
}

/// the wallet's public key (33-byte compressed SEC1) recovered from its
/// key-reveal touch over `reveal` ([`reveal_message`]); the outcome must echo
/// exactly that message.
pub fn wallet_pubkey(reveal: &[u8], outcome: &Outcome) -> Result<Vec<u8>, String> {
    let Outcome::Eth {
        signature, message, ..
    } = outcome
    else {
        return Err("expected a wallet signature (op=eth)".into());
    };
    if message != reveal {
        return Err("the wallet signed a different message than the key reveal".into());
    }
    keyscheme::recover_personal_sign(message, signature)
        .ok_or_else(|| "the wallet signature does not recover to a key".to_string())
}

/// login, touch 1: a `get` that asks the passkey nothing but WHICH account it
/// belongs to. Its challenge is random and authorizes nothing — no consent is
/// minted from this answer; the account it reveals is what touch 2's consent
/// is bound to.
pub fn account_request() -> Request {
    Request::Get {
        challenge: create_challenge(),
    }
}

/// the account a passkey assertion names in its `userHandle`. Unsigned, so it
/// is a HINT: it picks which account to ask about, and [`login_add_key`] then
/// accepts only a consent a key OF that account actually signed.
pub fn assertion_account(outcome: &Outcome) -> Result<u64, String> {
    let Outcome::Get { user_handle, .. } = outcome else {
        return Err("expected a passkey assertion (op=get)".into());
    };
    user_handle.ok_or_else(|| {
        "the passkey names no account (no userHandle) — register it with \
         `ducktape account key add --passkey` from a member device"
            .to_string()
    })
}

/// login, touch 2: the `get` a passkey answers to CONSENT to admitting
/// `device_key` (ed25519) into `account` at its `generation` on `chain_id`,
/// until `expires_at` — the identity module's own `AddKey` preimage, hashed
/// into the challenge.
pub fn login_request(
    chain_id: &str,
    device_key: &[u8],
    generation: u64,
    account: u64,
    expires_at: u64,
) -> Request {
    let preimage = identity::add_key_preimage(
        chain_id,
        KeyScheme::Ed25519,
        device_key,
        generation,
        account,
        expires_at,
    );
    let challenge = keyscheme::webauthn_challenge(identity::IDENTITY_ADD_KEY_NS, &preimage);
    Request::Get { challenge }
}

/// the login answer: the account the passkey names (its `userHandle`) and the
/// envelope proof an `AddKey { authorizer: { key: <that passkey>, proof } }`
/// carries. Which of the account's `Secp256r1` keys signed is the caller's to
/// find — by verifying the proof against each.
pub fn login_consent(outcome: &Outcome) -> Result<(u64, Vec<u8>), String> {
    let number = assertion_account(outcome)?;
    let Outcome::Get {
        authenticator_data,
        client_data_json,
        signature,
        ..
    } = outcome
    else {
        return Err("expected a passkey assertion (op=get)".into());
    };
    Ok((
        number,
        keyscheme::webauthn_proof(authenticator_data, client_data_json, signature),
    ))
}

/// the `AddKey` a login submits: the authorizer is whichever of the account's
/// passkeys verifies the assertion (the page does not say which one signed),
/// carrying the assertion envelope as its proof. `Err` when none does — a
/// consent at another generation, or a passkey off this account.
pub fn login_add_key(
    chain_id: &str,
    device_key: &[u8],
    generation: u64,
    account: &identity::AccountView,
    label: Option<String>,
    proof: Vec<u8>,
    expires_at: u64,
) -> Result<identity::IdentityMsg, String> {
    let preimage = identity::add_key_preimage(
        chain_id,
        KeyScheme::Ed25519,
        device_key,
        generation,
        account.number,
        expires_at,
    );
    let signer = account
        .keys
        .iter()
        .filter(|key| key.scheme == KeyScheme::Secp256r1)
        .find(|key| {
            KeyScheme::Secp256r1.verify(
                &key.pubkey,
                identity::IDENTITY_ADD_KEY_NS,
                &preimage,
                &proof,
            )
        });
    let Some(signer) = signer else {
        return Err(format!(
            "no passkey on account {} signed this consent",
            account.number
        ));
    };
    Ok(identity::IdentityMsg::AddKey {
        scheme: KeyScheme::Ed25519,
        label,
        authorizer: identity::Authorizer {
            key: signer.pubkey.clone(),
            account: account.number,
            expires_at,
            proof,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use keyscheme::testkit::{
        eth_key, eth_proof, eth_pubkey, eth_sign_message, passkey, passkey_assertion_parts,
        passkey_pubkey,
    };

    const RP: &str = "auth.ducktape.industries";

    fn msg() -> sdk::Msg {
        sdk::Msg {
            target: "identity".into(),
            payload: b"{\"set_name\":{\"name\":\"x\"}}".to_vec(),
        }
    }

    /// the fragment is the README's, field for field: b64url no padding,
    /// `user` = 8-byte LE, `name`/`cb` percent-encoded.
    #[test]
    fn the_request_url_is_the_pages_contract() {
        let mut challenge = [0u8; 32];
        challenge[..3].copy_from_slice(&[1, 2, 3]);
        let url = request_url(
            "https://p/",
            &Request::Create {
                challenge,
                user: 42,
                name: "de mo".into(),
            },
            "http://127.0.0.1:9/",
        );
        let (page, fragment) = url.split_once('#').unwrap();
        assert_eq!(page, "https://p/");
        let params: Vec<&str> = fragment.split('&').collect();
        assert_eq!(params[0], "op=create");
        assert!(params[1].starts_with("challenge=AQID"), "{}", params[1]);
        assert_eq!(params[2], "user=KgAAAAAAAAA");
        assert_eq!(params[3], "name=de%20mo");
        assert_eq!(params[4], "cb=http%3A%2F%2F127.0.0.1%3A9%2F");

        let eth = request_url(
            "https://p/",
            &Request::Eth {
                message: vec![1, 2, 3],
            },
            "cb",
        );
        assert!(eth.ends_with("#op=eth&challenge=AQID&cb=cb"), "{eth}");
        let get = request_url("https://p/", &Request::Get { challenge }, "cb");
        assert!(get.contains("#op=get&challenge=AQID"), "{get}");
    }

    #[test]
    fn results_decode_and_a_failure_names_itself() {
        let registered = passkey_pubkey(&passkey(0x51));
        let create = parse_result(&format!(
            r#"{{"op":"create","credentialId":"AQID","publicKey":"{}","alg":-7,"attestationObject":"","clientDataJSON":""}}"#,
            B64.encode(&registered)
        ))
        .unwrap();
        assert_eq!(
            create,
            Outcome::Create {
                credential_id: vec![1, 2, 3],
                public_key: registered
            }
        );
        // a key the page did not compress is refused right here, before a
        // consent or a second touch is spent on it.
        assert!(
            parse_result(r#"{"op":"create","credentialId":"AQID","publicKey":"AgME","alg":-7}"#)
                .is_err()
        );
        let get = parse_result(
            r#"{"op":"get","credentialId":"AQID","authenticatorData":"AQ","clientDataJSON":"Ag","signature":"Aw","userHandle":"KgAAAAAAAAA"}"#,
        )
        .unwrap();
        assert_eq!(
            get,
            Outcome::Get {
                authenticator_data: vec![1],
                client_data_json: vec![2],
                signature: vec![3],
                user_handle: Some(42)
            }
        );
        let anonymous = parse_result(
            r#"{"op":"get","credentialId":"AQID","authenticatorData":"AQ","clientDataJSON":"Ag","signature":"Aw","userHandle":null}"#,
        )
        .unwrap();
        assert!(matches!(
            anonymous,
            Outcome::Get {
                user_handle: None,
                ..
            }
        ));
        let eth =
            parse_result(r#"{"op":"eth","address":"0xab","signature":"0x0102","message":"AQID"}"#)
                .unwrap();
        assert_eq!(
            eth,
            Outcome::Eth {
                address: "0xab".into(),
                signature: vec![1, 2],
                message: vec![1, 2, 3]
            }
        );
        let err = parse_result(r#"{"op":"get","error":"NotAllowedError","message":"cancelled"}"#)
            .unwrap_err();
        assert_eq!(err, "the get ceremony failed: NotAllowedError: cancelled");
        assert!(parse_result("nope").is_err());
        assert!(parse_result(r#"{"op":"get","authenticatorData":"AQ"}"#).is_err());
    }

    /// the page's delivery, as bytes on the wire: a form POST to the callback,
    /// answered 200 with a page the user reads, the outcome handed back.
    #[test]
    fn the_listener_serves_one_form_post_and_ignores_a_probe() {
        let listener = Listener::bind().unwrap();
        let cb = listener.callback_url();
        let port: u16 = cb
            .trim_start_matches("http://127.0.0.1:")
            .trim_end_matches('/')
            .parse()
            .unwrap();
        let served = std::thread::spawn(move || listener.wait());

        // a stray GET first (a favicon probe) — answered, not the result.
        let mut probe = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        probe
            .write_all(b"GET /favicon.ico HTTP/1.1\r\nHost: x\r\n\r\n")
            .unwrap();
        let mut answer = String::new();
        probe.read_to_string(&mut answer).unwrap();
        assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");

        let body = "result=%7B%22op%22%3A%22eth%22%2C%22address%22%3A%220xab%22%2C%22signature%22%3A%220x01%22%2C%22message%22%3A%22AQID%22%7D&x=y+z";
        let mut post = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        post.write_all(
            format!(
                "POST / HTTP/1.1\r\nHost: x\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            )
            .as_bytes(),
        )
        .unwrap();
        let mut answer = String::new();
        post.read_to_string(&mut answer).unwrap();
        assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
        assert!(answer.contains("return to ducktape"), "{answer}");

        let outcome = served.join().unwrap().unwrap();
        assert_eq!(
            outcome,
            Outcome::Eth {
                address: "0xab".into(),
                signature: vec![1],
                message: vec![1, 2, 3]
            }
        );
    }

    /// a passkey-origin frame decodes at the node with the passkey as its
    /// verified origin — the assertion faked exactly as an authenticator
    /// produces it, over the challenge the request named.
    #[test]
    fn a_passkey_signs_a_frame_as_its_origin() {
        let sk = passkey(3);
        let pubkey = passkey_pubkey(&sk);
        let (request, preimage) = passkey_frame_request(&pubkey, 7, &msg());
        let Request::Get { challenge } = &request else {
            panic!("a get")
        };
        assert_eq!(
            *challenge,
            keyscheme::webauthn_challenge(node::FRAME_NS, &preimage)
        );
        let (authenticator_data, client_data_json, signature) =
            passkey_assertion_parts(&sk, RP, node::FRAME_NS, &preimage);
        let outcome = Outcome::Get {
            authenticator_data,
            client_data_json,
            signature,
            user_handle: None,
        };
        let frame = passkey_frame(preimage, &outcome).unwrap();
        let (origin, decoded) = node::decode_frame(&frame).expect("the node verifies it");
        assert_eq!(origin, sdk::Origin::External(pubkey));
        assert_eq!(decoded.target, "identity");
        assert!(
            passkey_frame(
                Vec::new(),
                &Outcome::Create {
                    credential_id: vec![],
                    public_key: vec![]
                }
            )
            .is_err()
        );
    }

    /// a wallet-origin frame: touch 1 reveals the key, touch 2 signs the
    /// frame; the node recovers the same key from the frame's proof.
    #[test]
    fn a_wallet_reveals_its_key_then_signs_a_frame_as_its_origin() {
        let sk = eth_key(3);
        let reveal = reveal_message();
        assert!(reveal.starts_with(REVEAL_NS));
        let touch1 = Outcome::Eth {
            address: "0x".into(),
            signature: eth_sign_message(&sk, &reveal),
            message: reveal.clone(),
        };
        let pubkey = wallet_pubkey(&reveal, &touch1).unwrap();
        assert_eq!(pubkey, eth_pubkey(&sk));
        let other = Outcome::Eth {
            address: "0x".into(),
            signature: eth_sign_message(&sk, b"something else"),
            message: b"something else".to_vec(),
        };
        assert!(
            wallet_pubkey(&reveal, &other).is_err(),
            "a stale touch is refused"
        );

        let (request, preimage) = wallet_frame_request(&pubkey, 9, &msg());
        let Request::Eth { message } = &request else {
            panic!("an eth touch")
        };
        assert_eq!(
            *message,
            keyscheme::personal_message(node::FRAME_NS, &preimage)
        );
        let touch2 = Outcome::Eth {
            address: "0x".into(),
            signature: eth_proof(&sk, node::FRAME_NS, &preimage),
            message: message.clone(),
        };
        let frame = wallet_frame(preimage, &touch2).unwrap();
        let (origin, _) = node::decode_frame(&frame).expect("the node verifies it");
        assert_eq!(origin, sdk::Origin::External(pubkey));
    }

    /// QR login: the passkey's assertion over the device key's AddKey
    /// preimage IS the consent the identity module verifies, and the
    /// userHandle names the account to admit it into.
    #[test]
    fn a_login_assertion_is_the_add_key_consent() {
        let sk = passkey(2);
        let device_key = [7u8; 32];
        let request = login_request("chain-a", &device_key, 0, 11, 900);
        let preimage =
            identity::add_key_preimage("chain-a", KeyScheme::Ed25519, &device_key, 0, 11, 900);
        assert_eq!(
            request,
            Request::Get {
                challenge: keyscheme::webauthn_challenge(identity::IDENTITY_ADD_KEY_NS, &preimage)
            }
        );
        let (authenticator_data, client_data_json, signature) =
            passkey_assertion_parts(&sk, RP, identity::IDENTITY_ADD_KEY_NS, &preimage);
        let outcome = Outcome::Get {
            authenticator_data: authenticator_data.clone(),
            client_data_json: client_data_json.clone(),
            signature: signature.clone(),
            user_handle: Some(11),
        };
        let (number, proof) = login_consent(&outcome).unwrap();
        assert_eq!(number, 11);
        assert!(KeyScheme::Secp256r1.verify(
            &passkey_pubkey(&sk),
            identity::IDENTITY_ADD_KEY_NS,
            &preimage,
            &proof
        ));
        let anonymous = Outcome::Get {
            authenticator_data,
            client_data_json,
            signature,
            user_handle: None,
        };
        assert!(
            login_consent(&anonymous)
                .unwrap_err()
                .contains("no userHandle")
        );
    }

    /// a login: the assertion is the consent on THIS device's `AddKey`, and the
    /// builder finds the passkey that signed it among the account's keys.
    #[test]
    fn login_builds_the_add_key_the_passkey_consented_to() {
        let device_key = [7u8; 32];
        let (other, mine) = (passkey(2), passkey(3));
        let key = |scheme, pubkey: Vec<u8>| identity::KeyView {
            scheme,
            pubkey,
            label: None,
            added_at: 0,
        };
        let account = identity::AccountView {
            number: 11,
            name: "alice".into(),
            keys: vec![
                key(KeyScheme::Ed25519, vec![1; 32]),
                key(KeyScheme::Secp256r1, passkey_pubkey(&other)),
                key(KeyScheme::Secp256r1, passkey_pubkey(&mine)),
            ],
            avatar: None,
            bio: None,
            updated_at: 0,
        };
        let preimage =
            identity::add_key_preimage("chain-a", KeyScheme::Ed25519, &device_key, 4, 11, 900);
        let (a, c, s) =
            passkey_assertion_parts(&mine, RP, identity::IDENTITY_ADD_KEY_NS, &preimage);
        let proof = keyscheme::webauthn_proof(&a, &c, &s);
        let msg = login_add_key(
            "chain-a",
            &device_key,
            4,
            &account,
            Some("laptop".into()),
            proof.clone(),
            900,
        )
        .unwrap();
        assert_eq!(
            msg,
            identity::IdentityMsg::AddKey {
                scheme: KeyScheme::Ed25519,
                label: Some("laptop".into()),
                authorizer: identity::Authorizer {
                    key: passkey_pubkey(&mine),
                    account: 11,
                    expires_at: 900,
                    proof: proof.clone(),
                },
            },
            "the passkey that signed is the authorizer"
        );
        // another generation, or a passkey off the account: no signer.
        assert!(login_add_key("chain-a", &device_key, 5, &account, None, proof, 900).is_err());
        let (a, c, s) =
            passkey_assertion_parts(&passkey(4), RP, identity::IDENTITY_ADD_KEY_NS, &preimage);
        let foreign = keyscheme::webauthn_proof(&a, &c, &s);
        assert!(login_add_key("chain-a", &device_key, 4, &account, None, foreign, 900).is_err());
    }

    /// abandoning delivers the page's error shape to the callback, so the
    /// blocked wait returns an `Err` naming the reason and its thread ends.
    #[test]
    fn abandoning_unblocks_the_wait_with_the_reason() {
        let listener = Listener::bind().unwrap();
        let cb = listener.callback_url();
        let served = std::thread::spawn(move || listener.wait());
        abandon(&cb, "no answer from the browser");
        let err = served.join().unwrap().unwrap_err();
        assert!(err.contains("no answer from the browser"), "{err}");
        abandon("not a callback url", "ignored");
        abandon("http://127.0.0.1:1/", "nobody listening");
    }

    #[test]
    fn form_decoding_handles_plus_and_escapes() {
        assert_eq!(form_decode(b"a+b%20c%zz"), b"a b c%zz");
        assert_eq!(
            form_field(b"x=1&result=%7B%7D", "result").as_deref(),
            Some("{}")
        );
        assert_eq!(form_field(b"x=1", "result"), None);
    }

    /// a relay that answers 204 `absent` times, then the JSON once, then 204.
    fn fake_relay(absent: usize, json: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}/", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for (served, stream) in listener.incoming().enumerate() {
                let mut stream = stream.unwrap();
                let mut line = String::new();
                BufReader::new(&stream).read_line(&mut line).unwrap();
                assert!(line.starts_with("GET /r/"), "{line}");
                let is_the_answer = served == absent;
                let response = match is_the_answer {
                    true => format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                         Content-Length: {}\r\nConnection: close\r\n\r\n{json}",
                        json.len()
                    ),
                    false => "HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n".to_string(),
                };
                stream.write_all(response.as_bytes()).unwrap();
            }
        });
        base
    }

    #[test]
    fn a_relay_id_is_43_url_safe_chars_and_names_the_callback() {
        let relay = Relay::at("https://auth.example/");
        assert_eq!(relay.id.len(), 43);
        assert!(
            relay
                .id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        );
        assert_eq!(
            relay.callback_url(),
            format!("https://auth.example/r/{}", relay.id)
        );
        assert_ne!(Relay::at("x").id, Relay::at("x").id);
    }

    #[test]
    fn a_relay_waits_through_204s_and_takes_the_first_200() {
        let base = fake_relay(
            2,
            r#"{"op":"get","credentialId":"AQ","authenticatorData":"AQ","clientDataJSON":"AQ","signature":"AQ","userHandle":"KgAAAAAAAAA"}"#,
        );
        let outcome = Relay::at(&base).wait(Duration::from_secs(20)).unwrap();
        assert!(matches!(
            outcome,
            Outcome::Get {
                user_handle: Some(42),
                ..
            }
        ));
    }

    #[test]
    fn a_relay_gives_up_at_the_deadline() {
        let base = fake_relay(usize::MAX, "{}");
        let err = Relay::at(&base)
            .wait(Duration::from_millis(10))
            .unwrap_err();
        assert!(err.contains("did not answer"), "{err}");
    }

    #[test]
    fn a_countdown_is_minutes_and_two_digit_seconds() {
        assert_eq!(countdown(Duration::from_secs(300)), "5:00");
        assert_eq!(countdown(Duration::from_millis(61_900)), "1:01");
        assert_eq!(countdown(Duration::ZERO), "0:00");
    }

    /// the poller hears how long is left before each poll, shrinking.
    #[test]
    fn a_relay_reports_the_time_left_before_each_poll() {
        let base = fake_relay(
            2,
            r#"{"op":"get","credentialId":"AQ","authenticatorData":"AQ","clientDataJSON":"AQ","signature":"AQ","userHandle":"KgAAAAAAAAA"}"#,
        );
        let mut seen = Vec::new();
        Relay::at(&base)
            .wait_reporting(Duration::from_secs(60), |left| seen.push(left))
            .unwrap();
        assert_eq!(seen.len(), 3, "{seen:?}");
        assert!(seen[0] > seen[1] && seen[1] > seen[2], "{seen:?}");
    }

    /// a real ceremony URL renders inside an 80-column terminal: half-block
    /// rows, every line the same width, a quiet zone around the code.
    #[test]
    fn a_terminal_qr_of_a_ceremony_url_fits_eighty_columns() {
        let relay = Relay::new();
        let url = request_url(
            AUTH_PAGE,
            &Request::Create {
                challenge: [9u8; 32],
                user: 1234,
                name: "byeongsu".into(),
            },
            &relay.callback_url(),
        );
        let qr = terminal_qr(&url).unwrap();
        let lines: Vec<&str> = qr.lines().collect();
        let width = lines[0].chars().count();
        assert!(lines.iter().all(|l| l.chars().count() == width));
        assert!((30..=48).contains(&lines.len()), "{} lines", lines.len());
        assert!(width <= 80, "{width} columns");
        assert!(
            qr.chars()
                .all(|c| matches!(c, '█' | '▀' | '▄' | ' ' | '\n'))
        );
    }
}
