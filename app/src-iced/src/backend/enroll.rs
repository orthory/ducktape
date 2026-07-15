//! Ephemeral LAN relay for adding a phone-minted P-256 account key.

use std::io::{Cursor, Read as _};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tiny_http::{Header, Method, Request, Response, Server};

use super::Backend;
use super::identity::SecretString;
use super::link::{lan_ipv4, lan_server, random_token, token_matches};
use super::node_control::{last_line, run_verb};

const MAX_REQUEST_BODY_BYTES: u64 = 8 * 1024;
const MAX_PAYLOAD_HEX_BYTES: usize = 2 * 1024;
const ENROLLMENT_LIFETIME: Duration = Duration::from_secs(10 * 60);

#[derive(Clone, PartialEq, Eq)]
pub struct PhoneEnrollmentStart {
    pub url: String,
}

impl std::fmt::Debug for PhoneEnrollmentStart {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PhoneEnrollmentStart")
            .field("url", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PhoneCandidate {
    pub new_key: String,
    pub signature: String,
}

struct EnrollState {
    token: SecretString,
    chain_id: String,
    account_id: String,
    nonce: u64,
    backend: Backend,
    server: Arc<Server>,
    result: Option<PhoneCandidate>,
    deadline: Instant,
    timer: Option<thread::Thread>,
}

static STATE: Mutex<Option<EnrollState>> = Mutex::new(None);

fn state() -> std::sync::MutexGuard<'static, Option<EnrollState>> {
    STATE.lock().unwrap_or_else(|error| error.into_inner())
}

impl Backend {
    pub async fn phone_enrollment_start(
        &self,
        chain_id: String,
        account_id: String,
        nonce: u64,
    ) -> Result<PhoneEnrollmentStart, String> {
        validate_identifier(&chain_id, "chain id")?;
        validate_hex_len(&account_id, 64, "account id")?;
        let backend = self.clone();
        self.control
            .run(move || start_enrollment(backend, chain_id, account_id, nonce))
            .await
    }

    pub async fn phone_enrollment_poll(&self) -> Result<Option<PhoneCandidate>, String> {
        self.control.run(poll_enrollment).await
    }

    pub async fn phone_enrollment_cancel(&self) -> Result<(), String> {
        self.control
            .run(|| {
                cancel_enrollment();
                Ok(())
            })
            .await
    }
}

fn start_enrollment(
    backend: Backend,
    chain_id: String,
    account_id: String,
    nonce: u64,
) -> Result<PhoneEnrollmentStart, String> {
    cancel_enrollment();
    let token = SecretString::new(random_token()?);
    let ip = lan_ipv4()?;
    let (server, port) = lan_server(ip)?;
    let deadline = Instant::now() + ENROLLMENT_LIFETIME;

    let expiry_token = token.clone();
    let timer = thread::Builder::new()
        .name("phone-enrollment-timeout".into())
        .spawn(move || {
            thread::park_timeout(ENROLLMENT_LIFETIME);
            expire_token(&expiry_token);
        })
        .map_err(|error| format!("start enrollment timeout: {error}"))?;
    let timer_thread = timer.thread().clone();
    drop(timer);

    *state() = Some(EnrollState {
        token: token.clone(),
        chain_id,
        account_id,
        nonce,
        backend,
        server: server.clone(),
        result: None,
        deadline,
        timer: Some(timer_thread),
    });
    if let Err(error) = thread::Builder::new()
        .name("phone-enrollment".into())
        .spawn(move || serve(server))
    {
        expire_token(&token);
        return Err(format!("start enrollment server: {error}"));
    }

    Ok(PhoneEnrollmentStart {
        url: format!("http://{ip}:{port}/enroll#{}", token.as_ref()),
    })
}

fn poll_enrollment() -> Result<Option<PhoneCandidate>, String> {
    expire_stale();
    Ok(state().as_ref().and_then(|session| session.result.clone()))
}

fn cancel_enrollment() {
    stop_state(state().take());
}

fn stop_state(session: Option<EnrollState>) {
    if let Some(session) = session {
        session.server.unblock();
        if let Some(timer) = session.timer {
            timer.unpark();
        }
    }
}

fn expire_stale() {
    if state()
        .as_ref()
        .is_some_and(|session| Instant::now() >= session.deadline)
    {
        cancel_enrollment();
    }
}

fn expire_token(token: &str) {
    if state()
        .as_ref()
        .is_some_and(|session| token_matches(&session.token, token))
    {
        cancel_enrollment();
    }
}

fn serve(server: Arc<Server>) {
    for mut request in server.incoming_requests() {
        let response = handle(&mut request);
        let _ = request.respond(response);
    }
}

fn handle(request: &mut Request) -> Response<Cursor<Vec<u8>>> {
    expire_stale();
    let path = request.url().split('?').next().unwrap_or("/");
    match (request.method(), path) {
        (Method::Get, "/enroll") => html(PAGE_HTML),
        (Method::Get, "/e.js") => javascript(BUNDLE_JS),
        (Method::Post, "/payload") => payload(request),
        (Method::Post, "/possession") => possession(request),
        _ => status(404),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PayloadRequest {
    token: String,
    new_key: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PossessionRequest {
    token: String,
    new_key: String,
    sig: String,
}

fn payload(request: &mut Request) -> Response<Cursor<Vec<u8>>> {
    let Some(body) = read_json::<PayloadRequest>(request) else {
        return status(400);
    };
    let (backend, chain_id, account_id, nonce) = {
        let guard = state();
        let Some(session) = guard.as_ref() else {
            return status(410);
        };
        if !token_matches(&session.token, &body.token) || !valid_p256_key(&body.new_key) {
            return status(403);
        }
        (
            session.backend.clone(),
            session.chain_id.clone(),
            session.account_id.clone(),
            session.nonce,
        )
    };
    let new_key = body.new_key;
    let output = backend.control.run_blocking(move || {
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
    match output {
        Ok(output) => {
            let payload = last_line(&output);
            if payload.is_empty()
                || payload.len() > MAX_PAYLOAD_HEX_BYTES
                || !payload.len().is_multiple_of(2)
                || !payload.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                status(500)
            } else {
                json(serde_json::json!({ "payload": payload }).to_string())
            }
        }
        Err(error) => {
            tracing::debug!(
                target: "ducktape::account",
                event = "phone_enrollment_payload_refused",
                reason = "payload_generation_failed",
                detail = %error,
                "phone enrollment payload generation failed"
            );
            status(500)
        }
    }
}

fn possession(request: &mut Request) -> Response<Cursor<Vec<u8>>> {
    let Some(body) = read_json::<PossessionRequest>(request) else {
        return status(400);
    };
    let mut guard = state();
    let Some(session) = guard.as_mut() else {
        return status(410);
    };
    if !token_matches(&session.token, &body.token)
        || !valid_p256_key(&body.new_key)
        || !valid_p256_signature(&body.sig)
    {
        return status(403);
    }
    if session.result.is_some() {
        return status(409);
    }
    session.result = Some(PhoneCandidate {
        new_key: body.new_key.to_ascii_lowercase(),
        signature: body.sig.to_ascii_lowercase(),
    });
    json(r#"{"ok":true}"#.into())
}

fn read_json<T: DeserializeOwned>(request: &mut Request) -> Option<T> {
    let mut body = String::new();
    request
        .as_reader()
        .take(MAX_REQUEST_BODY_BYTES + 1)
        .read_to_string(&mut body)
        .ok()?;
    if body.len() as u64 > MAX_REQUEST_BODY_BYTES {
        return None;
    }
    serde_json::from_str(&body).ok()
}

fn valid_p256_key(value: &str) -> bool {
    value.len() == 66
        && matches!(value.get(..2), Some("02" | "03"))
        && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_p256_signature(value: &str) -> bool {
    value.len() == 128 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn validate_identifier(value: &str, field: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        Err(format!(
            "{field} is missing, too long, or contains controls"
        ))
    } else {
        Ok(())
    }
}

fn validate_hex_len(value: &str, len: usize, field: &str) -> Result<(), String> {
    if value.len() == len && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(format!("{field} is not bounded hexadecimal"))
    }
}

fn header(name: &[u8], value: &[u8]) -> Header {
    Header::from_bytes(name, value).expect("static response header")
}

fn response(body: &str, content_type: &[u8]) -> Response<Cursor<Vec<u8>>> {
    Response::from_string(body)
        .with_header(header(b"Content-Type", content_type))
        .with_header(header(b"Cache-Control", b"no-store"))
        .with_header(header(b"X-Content-Type-Options", b"nosniff"))
}

fn html(body: &str) -> Response<Cursor<Vec<u8>>> {
    response(body, b"text/html; charset=utf-8").with_header(header(
        b"Content-Security-Policy",
        b"default-src 'none'; script-src 'self'; style-src 'unsafe-inline'; connect-src 'self'; base-uri 'none'; form-action 'none'",
    ))
}

fn javascript(body: &str) -> Response<Cursor<Vec<u8>>> {
    response(body, b"text/javascript; charset=utf-8")
}

fn json(body: String) -> Response<Cursor<Vec<u8>>> {
    response(&body, b"application/json")
}

fn status(code: u16) -> Response<Cursor<Vec<u8>>> {
    Response::from_string("").with_status_code(code)
}

const PAGE_HTML: &str = r#"<!doctype html><html><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Add this key to your Ducktape account</title>
<style>body{font-family:system-ui;max-width:28rem;margin:2rem auto;padding:0 1rem;color:#111}button{font-size:1rem;padding:.7rem 1.2rem;border-radius:.5rem;border:0;background:#111;color:#fff}#s{margin-top:1rem;white-space:pre-wrap}</style></head>
<body><h2>Add this device to your account</h2><p>This generates a key on this phone and adds it to your account. Nothing leaves your network.</p><button id="go">Generate &amp; add key</button><div id="s"></div><script type="module" src="/e.js"></script></body></html>"#;

// Kept byte-for-byte with the audited phone signer. Regenerate from the source
// beside it; neither native shell owns a second copy.
const BUNDLE_JS: &str = include_str!("../../../src/enroll/enroll_bundle.js");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phone_crypto_fields_are_strictly_shaped() {
        assert!(valid_p256_key(&format!("02{}", "11".repeat(32))));
        assert!(valid_p256_key(&format!("03{}", "aa".repeat(32))));
        assert!(!valid_p256_key(&format!("04{}", "11".repeat(32))));
        assert!(!valid_p256_key(&format!("02{}", "11".repeat(31))));
        assert!(valid_p256_signature(&"22".repeat(64)));
        assert!(!valid_p256_signature(&"22".repeat(63)));
    }

    #[tokio::test]
    async fn enrollment_relay_gates_token_body_and_first_candidate() {
        use std::io::{Read as _, Write as _};
        use std::net::TcpStream;

        cancel_enrollment();
        let root = tempfile::tempdir().unwrap();
        let backend = Backend::at_root(root.path()).await.unwrap();
        let server = Arc::new(Server::http("127.0.0.1:0").unwrap());
        let port = server.server_addr().to_ip().unwrap().port();
        *state() = Some(EnrollState {
            token: SecretString::new("tok123".into()),
            chain_id: "chain".into(),
            account_id: "aa".repeat(32),
            nonce: 0,
            backend,
            server: server.clone(),
            result: None,
            deadline: Instant::now() + Duration::from_secs(30),
            timer: None,
        });
        let join = {
            let server = server.clone();
            thread::spawn(move || serve(server))
        };
        let request = |raw: &str| -> String {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
            stream.write_all(raw.as_bytes()).unwrap();
            let mut response = String::new();
            stream.read_to_string(&mut response).unwrap();
            response
        };
        let post = |body: &str| {
            request(&format!(
                "POST /possession HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            ))
        };
        let key = format!("02{}", "11".repeat(32));
        let signature = "22".repeat(64);
        let wrong = serde_json::json!({
            "token": "wrong",
            "new_key": key.clone(),
            "sig": signature.clone(),
        });
        assert!(post(&wrong.to_string()).contains("403"));
        assert!(poll_enrollment().unwrap().is_none());

        let oversized = "x".repeat(MAX_REQUEST_BODY_BYTES as usize + 1);
        assert!(post(&oversized).contains("400"));
        assert!(poll_enrollment().unwrap().is_none());

        let accepted = serde_json::json!({
            "token": "tok123",
            "new_key": key.clone(),
            "sig": signature.clone(),
        });
        assert!(post(&accepted.to_string()).contains("200"));
        assert_eq!(
            poll_enrollment().unwrap(),
            Some(PhoneCandidate {
                new_key: key,
                signature,
            })
        );
        assert!(post(&accepted.to_string()).contains("409"));

        cancel_enrollment();
        join.join().unwrap();
    }
}
