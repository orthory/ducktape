//! Ephemeral, token-gated LAN relay for the device-link ceremony.

use std::fs;
use std::io::{Cursor, Read as _};
use std::net::{IpAddr, Ipv4Addr, SocketAddrV4, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine as _;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tiny_http::{Header, Method, Request, Response, Server};
use zeroize::Zeroize as _;

use super::Backend;
use super::identity::SecretString;
use super::private_fs;
use super::workspace_service::write_atomic;
#[cfg(test)]
use crate::view_api::MemberKeyKind;
use crate::view_api::{LinkResponse, decode_link_response, encode_link_response};

const CHALLENGE_PREFIX: &str = "ducktape-link-challenge-v1:";
const MAX_BLOB_BYTES: usize = 4 * 1024;
const MAX_REQUEST_BODY_BYTES: u64 = 8 * 1024;
const MAX_REPLY_BYTES: usize = 16 * 1024;
const MAX_TEXT_BYTES: usize = 256;
const MAX_ACCOUNT_NAME_BYTES: usize = 64;
const RELAY_LIFETIME: Duration = Duration::from_secs(10 * 60);
const ADDRESS_HINT: &str = "that doesn't look like a private-LAN link address — enter the http://192.168.x.x address shown on your other device";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LinkChallenge {
    pub chain_id: String,
    pub account_id: String,
    pub nonce: u64,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LinkPending {
    pub chain_id: String,
    pub account_id: String,
    pub member_key: String,
}

/// A capability-bearing relay URL. Its token is scrubbed on drop and redacted
/// from `Debug`; use [`LinkAddress::as_str`] only for display/copy or a relay call.
#[derive(Clone)]
pub struct LinkAddress(SecretString);

impl LinkAddress {
    pub fn parse(value: String) -> Result<Self, String> {
        let value = SecretString::new(value);
        parse_link_url(&value).ok_or_else(|| ADDRESS_HINT.to_string())?;
        Ok(Self(SecretString::new(value.trim().to_string())))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for LinkAddress {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("LinkAddress([REDACTED])")
    }
}

#[derive(Debug, Clone)]
pub struct LinkRelayStart {
    pub url: LinkAddress,
}

struct LinkState {
    token: SecretString,
    challenge: String,
    server: Arc<Server>,
    response: Option<String>,
    deadline: Instant,
    timer: Option<thread::Thread>,
}

static STATE: Mutex<Option<LinkState>> = Mutex::new(None);

fn state() -> std::sync::MutexGuard<'static, Option<LinkState>> {
    STATE.lock().unwrap_or_else(|error| error.into_inner())
}

impl Backend {
    pub async fn link_relay_start(
        &self,
        challenge: LinkChallenge,
    ) -> Result<LinkRelayStart, String> {
        self.control.run(move || start_relay(challenge)).await
    }

    pub async fn link_relay_poll(&self) -> Result<Option<LinkResponse>, String> {
        self.control.run(poll_relay).await
    }

    pub async fn link_relay_cancel(&self) -> Result<(), String> {
        self.control
            .run(|| {
                cancel_relay();
                Ok(())
            })
            .await
    }

    pub async fn link_fetch_challenge(
        &self,
        address: LinkAddress,
    ) -> Result<LinkChallenge, String> {
        let address = parse_link_url(address.as_str()).ok_or(ADDRESS_HINT)?;
        let blob = fetch_challenge_from(&address).await?;
        decode_link_challenge(&blob)
    }

    pub async fn link_send_response(
        &self,
        address: LinkAddress,
        response: LinkResponse,
    ) -> Result<(), String> {
        let address = parse_link_url(address.as_str()).ok_or(ADDRESS_HINT)?;
        let blob = encode_link_response(&response)?;
        send_response_to(&address, &blob).await
    }

    pub async fn link_pending_mark(&self, pending: LinkPending) -> Result<(), String> {
        let root = self.root.clone();
        self.control
            .run(move || {
                validate_text(&pending.chain_id, "pending link chain id", false)?;
                validate_ed25519_key(&pending.account_id, "pending link account id")?;
                validate_ed25519_key(&pending.member_key, "pending link member key")?;
                private_fs::ensure_private_dir(&root)?;
                let bytes = serde_json::to_vec(&pending)
                    .map_err(|error| format!("encode pending device link: {error}"))?;
                write_atomic(&root.join("account-link-pending.json"), &bytes)
            })
            .await
    }

    pub async fn link_pending(&self) -> Result<Option<LinkPending>, String> {
        let root = self.root.clone();
        self.control
            .run(move || {
                let path = root.join("account-link-pending.json");
                let bytes = match private_fs::read(&path)? {
                    Some(bytes) => bytes,
                    None => return Ok(None),
                };
                let pending: LinkPending = serde_json::from_slice(&bytes)
                    .map_err(|_| "pending device link state is malformed".to_string())?;
                validate_text(&pending.chain_id, "pending link chain id", false)?;
                validate_ed25519_key(&pending.account_id, "pending link account id")?;
                validate_ed25519_key(&pending.member_key, "pending link member key")?;
                Ok(Some(pending))
            })
            .await
    }

    pub async fn link_pending_clear(&self) -> Result<(), String> {
        let root = self.root.clone();
        self.control
            .run(
                move || match fs::remove_file(root.join("account-link-pending.json")) {
                    Ok(()) => Ok(()),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                    Err(error) => Err(format!("clear pending device link: {error}")),
                },
            )
            .await
    }
}

pub fn encode_link_challenge(challenge: &LinkChallenge) -> Result<String, String> {
    validate_challenge(challenge)?;
    encode_prefixed(challenge, CHALLENGE_PREFIX)
}

pub fn decode_link_challenge(blob: &str) -> Result<LinkChallenge, String> {
    let challenge = decode_prefixed(blob, CHALLENGE_PREFIX, "link challenge")?;
    validate_challenge(&challenge)?;
    Ok(challenge)
}

fn encode_prefixed<T: Serialize>(value: &T, prefix: &str) -> Result<String, String> {
    let json = serde_json::to_vec(value).map_err(|_| "could not encode link data".to_string())?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(json);
    let blob = format!("{prefix}{encoded}");
    if blob.len() > MAX_BLOB_BYTES {
        return Err("link data is too large".to_string());
    }
    Ok(blob)
}

fn decode_prefixed<T: DeserializeOwned>(blob: &str, prefix: &str, kind: &str) -> Result<T, String> {
    let blob = blob.trim();
    if blob.len() > MAX_BLOB_BYTES {
        return Err(format!("malformed {kind}"));
    }
    let encoded = blob
        .strip_prefix(prefix)
        .filter(|encoded| !encoded.is_empty())
        .ok_or_else(|| format!("malformed {kind}"))?;
    let json = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| format!("malformed {kind}"))?;
    serde_json::from_slice(&json).map_err(|_| format!("malformed {kind}"))
}

fn validate_challenge(challenge: &LinkChallenge) -> Result<(), String> {
    validate_text(&challenge.chain_id, "challenge chain id", false)?;
    validate_ed25519_key(&challenge.account_id, "challenge account id")?;
    if challenge.nonce > 9_007_199_254_740_991 {
        return Err("challenge nonce is outside the cross-device range".to_string());
    }
    if let Some(name) = challenge.name.as_deref() {
        validate_bounded_text(name, "challenge account name", MAX_ACCOUNT_NAME_BYTES, true)?;
    }
    Ok(())
}

fn validate_text(value: &str, field: &str, empty_ok: bool) -> Result<(), String> {
    validate_bounded_text(value, field, MAX_TEXT_BYTES, empty_ok)
}

fn validate_bounded_text(
    value: &str,
    field: &str,
    max_bytes: usize,
    empty_ok: bool,
) -> Result<(), String> {
    if (!empty_ok && value.is_empty())
        || value.len() > max_bytes
        || value.chars().any(char::is_control)
    {
        return Err(format!(
            "{field} is missing, too long, or contains controls"
        ));
    }
    Ok(())
}

fn validate_ed25519_key(value: &str, field: &str) -> Result<(), String> {
    if value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(format!("{field} is not a 32-byte hexadecimal key"))
    }
}

fn start_relay(challenge: LinkChallenge) -> Result<LinkRelayStart, String> {
    let challenge = encode_link_challenge(&challenge)?;
    cancel_relay();
    let token = SecretString::new(random_token()?);
    let ip = lan_ipv4()?;
    let (server, port) = lan_server(ip)?;
    let deadline = Instant::now() + RELAY_LIFETIME;

    let expiry_token = token.clone();
    let timer = thread::Builder::new()
        .name("link-relay-timeout".to_string())
        .spawn(move || {
            thread::park_timeout(RELAY_LIFETIME);
            expire_token(&expiry_token);
        })
        .map_err(|error| format!("start link relay timeout: {error}"))?;
    let timer_thread = timer.thread().clone();
    drop(timer);

    *state() = Some(LinkState {
        token: token.clone(),
        challenge,
        server: server.clone(),
        response: None,
        deadline,
        timer: Some(timer_thread),
    });

    if let Err(error) = thread::Builder::new()
        .name("link-relay".to_string())
        .spawn(move || serve(server))
    {
        expire_token(&token);
        return Err(format!("start link relay: {error}"));
    }

    LinkAddress::parse(format!("http://{ip}:{port}/link#{}", token.as_ref()))
        .map(|url| LinkRelayStart { url })
}

fn poll_relay() -> Result<Option<LinkResponse>, String> {
    expire_stale();
    state()
        .as_ref()
        .and_then(|session| session.response.as_deref())
        .map(decode_link_response)
        .transpose()
}

fn cancel_relay() {
    let session = state().take();
    stop_state(session);
}

fn expire_stale() {
    let expired = state()
        .as_ref()
        .is_some_and(|session| Instant::now() >= session.deadline);
    if expired {
        cancel_relay();
    }
}

fn expire_token(token: &str) {
    let matches = state()
        .as_ref()
        .is_some_and(|session| token_matches(&session.token, token));
    if matches {
        cancel_relay();
    }
}

fn stop_state(session: Option<LinkState>) {
    if let Some(session) = session {
        session.server.unblock();
        if let Some(timer) = session.timer {
            timer.unpark();
        }
    }
}

pub(super) fn lan_ipv4() -> Result<Ipv4Addr, String> {
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|error| format!("udp bind: {error}"))?;
    socket
        .connect("8.8.8.8:80")
        .map_err(|error| format!("udp connect: {error}"))?;
    let IpAddr::V4(ip) = socket
        .local_addr()
        .map_err(|error| format!("local address: {error}"))?
        .ip()
    else {
        return Err("no private LAN IPv4 address is available".to_string());
    };
    if !ip.is_private() {
        return Err("no private LAN IPv4 address is available".to_string());
    }
    Ok(ip)
}

pub(super) fn lan_server(ip: Ipv4Addr) -> Result<(Arc<Server>, u16), String> {
    let server = Arc::new(Server::http((ip, 0)).map_err(|error| format!("bind relay: {error}"))?);
    let port = server
        .server_addr()
        .to_ip()
        .ok_or_else(|| "relay server has no IP address".to_string())?
        .port();
    Ok((server, port))
}

pub(super) fn random_token() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).map_err(|_| "could not generate a relay token".to_string())?;
    let mut token = String::with_capacity(32);
    use std::fmt::Write as _;
    for byte in bytes {
        write!(token, "{byte:02x}").expect("write to String");
    }
    bytes.zeroize();
    Ok(token)
}

pub(super) fn token_matches(expected: &str, supplied: &str) -> bool {
    let expected = expected.as_bytes();
    let supplied = supplied.as_bytes();
    let mut different = expected.len() ^ supplied.len();
    for (index, byte) in expected.iter().enumerate() {
        different |= usize::from(*byte ^ supplied.get(index).copied().unwrap_or(0));
    }
    different == 0
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
        (Method::Get, "/link") => html(PAGE_HTML),
        (Method::Post, "/link/challenge") => serve_challenge(request),
        (Method::Post, "/link/response") => receive_response(request),
        _ => status(404),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChallengeRequest {
    token: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResponseRequest {
    token: String,
    response: String,
}

fn serve_challenge(request: &mut Request) -> Response<Cursor<Vec<u8>>> {
    let Some(body) = read_json::<ChallengeRequest>(request) else {
        return status(400);
    };
    let guard = state();
    let Some(session) = guard.as_ref() else {
        return status(410);
    };
    if !token_matches(&session.token, &body.token) {
        return status(403);
    }
    json(serde_json::json!({ "challenge": session.challenge }).to_string())
}

fn receive_response(request: &mut Request) -> Response<Cursor<Vec<u8>>> {
    let Some(body) = read_json::<ResponseRequest>(request) else {
        return status(400);
    };
    let mut guard = state();
    let Some(session) = guard.as_mut() else {
        return status(410);
    };
    if !token_matches(&session.token, &body.token) {
        return status(403);
    }
    if decode_link_response(&body.response).is_err() {
        return status(403);
    }
    if session.response.is_some() {
        return status(409);
    }
    session.response = Some(body.response);
    json(r#"{"ok":true}"#.to_string())
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
            b"default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'",
        ))
}

fn status(code: u16) -> Response<Cursor<Vec<u8>>> {
    Response::from_string("").with_status_code(code)
}

const PAGE_HTML: &str = r#"<!doctype html><html><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Link a new Ducktape device</title>
<style>body{font-family:system-ui;max-width:28rem;margin:2rem auto;padding:0 1rem;color:#111}</style></head>
<body><h2>Link a new device</h2><p>Enter this address in the Ducktape app on your new computer.</p></body></html>"#;

struct RelayAddress {
    base: String,
    token: SecretString,
}

fn parse_link_url(raw: &str) -> Option<RelayAddress> {
    let rest = raw.trim().strip_prefix("http://")?;
    let (authority, fragment) = rest.split_once("/link#")?;
    if fragment.len() != 32 || !fragment.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let socket: SocketAddrV4 = authority.parse().ok()?;
    if socket.port() < 1024 || !socket.ip().is_private() {
        return None;
    }
    Some(RelayAddress {
        base: format!("http://{socket}"),
        token: SecretString::new(fragment.to_ascii_lowercase()),
    })
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .build()
        .map_err(|error| format!("HTTP client: {error}"))
}

async fn post_bounded(
    client: &reqwest::Client,
    url: String,
    body: String,
) -> Result<(u16, String), String> {
    let mut response = client
        .post(url)
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await
        .map_err(|error| unreachable_error(&error))?;
    let code = response.status().as_u16();
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| unreachable_error(&error))?
    {
        if bytes.len().saturating_add(chunk.len()) > MAX_REPLY_BYTES {
            return Err("the other device sent an oversized reply".to_string());
        }
        bytes.extend_from_slice(&chunk);
    }
    let body = String::from_utf8(bytes)
        .map_err(|_| "the other device sent an unexpected reply".to_string())?;
    Ok((code, body))
}

async fn fetch_challenge_from(address: &RelayAddress) -> Result<String, String> {
    let client = http_client()?;
    let body = serde_json::json!({ "token": address.token.as_ref() }).to_string();
    let (code, response) =
        post_bounded(&client, format!("{}/link/challenge", address.base), body).await?;
    if code != 200 {
        return Err(relay_status_error(code));
    }
    let value: serde_json::Value = serde_json::from_str(&response)
        .map_err(|_| "the other device sent an unexpected reply".to_string())?;
    let challenge = value
        .get("challenge")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "the other device sent an unexpected reply".to_string())?;
    decode_link_challenge(challenge)
        .map(|_| challenge.to_string())
        .map_err(|_| "the other device sent a malformed link challenge".to_string())
}

async fn send_response_to(address: &RelayAddress, response: &str) -> Result<(), String> {
    decode_link_response(response)?;
    let client = http_client()?;
    let body = serde_json::json!({
        "token": address.token.as_ref(),
        "response": response
    })
    .to_string();
    let (code, _) = post_bounded(&client, format!("{}/link/response", address.base), body).await?;
    if code == 200 {
        Ok(())
    } else {
        Err(relay_status_error(code))
    }
}

fn relay_status_error(code: u16) -> String {
    match code {
        410 => "the link panel was closed on the other device — reopen it and retry".to_string(),
        409 => "a reply already reached the other device — restart the link there".to_string(),
        403 => {
            "the other device did not accept this address — enter it exactly as shown".to_string()
        }
        code => format!("the other device answered unexpectedly (HTTP {code})"),
    }
}

fn unreachable_error(error: &reqwest::Error) -> String {
    if error.is_timeout() {
        "timed out reaching the other device — are both devices on the same network?".to_string()
    } else {
        "could not reach the other device — check the address and local network".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn challenge() -> LinkChallenge {
        LinkChallenge {
            chain_id: "chain-a".to_string(),
            account_id: "11".repeat(32),
            nonce: 7,
            name: Some("Team".to_string()),
        }
    }

    fn response() -> LinkResponse {
        LinkResponse {
            pubkey: "22".repeat(32),
            kind: MemberKeyKind::Ed25519,
            possession: r#"{"signature":{"sig":[1,2,3]}}"#.to_string(),
            label: Some("Laptop".to_string()),
        }
    }

    #[test]
    fn blobs_round_trip_with_strict_types_and_bounds() {
        let challenge_blob = encode_link_challenge(&challenge()).unwrap();
        assert_eq!(decode_link_challenge(&challenge_blob).unwrap(), challenge());
        let response_blob = encode_link_response(&response()).unwrap();
        assert_eq!(decode_link_response(&response_blob).unwrap(), response());
        assert!(decode_link_challenge(&response_blob).is_err());
        assert!(decode_link_response("ducktape-link-response-v1:<script>").is_err());
        assert!(
            decode_link_challenge(&format!("{CHALLENGE_PREFIX}{}", "A".repeat(MAX_BLOB_BYTES)))
                .is_err()
        );
        let mut oversized_label = response();
        oversized_label.label = Some("x".repeat(65));
        assert!(encode_link_response(&oversized_label).is_err());
    }

    #[test]
    fn link_addresses_are_private_ipv4_only() {
        let token = "0123456789abcdef0123456789abcdef";
        let parsed = parse_link_url(&format!(" http://192.168.1.23:40000/link#{token} ")).unwrap();
        assert_eq!(parsed.base, "http://192.168.1.23:40000");
        assert!(parse_link_url(&format!("http://10.0.0.2:49152/link#{token}")).is_some());
        assert!(parse_link_url(&format!("http://10.0.0.2:80/link#{token}")).is_none());
        assert!(parse_link_url(&format!("http://127.0.0.1:80/link#{token}")).is_none());
        assert!(parse_link_url(&format!("http://169.254.169.254:80/link#{token}")).is_none());
        assert!(parse_link_url(&format!("http://8.8.8.8:80/link#{token}")).is_none());
        assert!(parse_link_url(&format!("http://router.local:80/link#{token}")).is_none());
        assert!(parse_link_url(&format!("https://192.168.1.2:80/link#{token}")).is_none());
        assert!(parse_link_url(&format!("http://user@192.168.1.2:80/link#{token}")).is_none());
    }

    #[test]
    fn token_comparison_is_shape_strict() {
        assert!(token_matches("0123456789abcdef", "0123456789abcdef"));
        assert!(!token_matches("0123456789abcdef", "0123456789abcdee"));
        assert!(!token_matches("0123456789abcdef", "01234567"));
    }

    #[test]
    fn relay_round_trip_and_first_response_wins() {
        cancel_relay();
        let server = Arc::new(Server::http("127.0.0.1:0").unwrap());
        let port = server.server_addr().to_ip().unwrap().port();
        let token = "0123456789abcdef0123456789abcdef";
        *state() = Some(LinkState {
            token: SecretString::new(token.to_string()),
            challenge: encode_link_challenge(&challenge()).unwrap(),
            server: server.clone(),
            response: None,
            deadline: Instant::now() + Duration::from_secs(30),
            timer: None,
        });
        let server_thread = {
            let server = server.clone();
            thread::spawn(move || serve(server))
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let address = RelayAddress {
            base: format!("http://127.0.0.1:{port}"),
            token: SecretString::new(token.to_string()),
        };

        let fetched = runtime.block_on(fetch_challenge_from(&address)).unwrap();
        assert_eq!(decode_link_challenge(&fetched).unwrap(), challenge());
        let response_blob = encode_link_response(&response()).unwrap();
        runtime
            .block_on(send_response_to(&address, &response_blob))
            .unwrap();
        assert_eq!(poll_relay().unwrap(), Some(response()));
        let error = runtime
            .block_on(send_response_to(&address, &response_blob))
            .unwrap_err();
        assert!(error.contains("already reached"));

        cancel_relay();
        server_thread.join().unwrap();
    }

    #[tokio::test]
    async fn pending_link_is_exact_and_durable_until_cleared() {
        let root = std::env::temp_dir().join(format!(
            "ducktape-link-pending-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let backend = Backend::at_root(&root).await.unwrap();
        let pending = LinkPending {
            chain_id: "chain-a".into(),
            account_id: "11".repeat(32),
            member_key: "22".repeat(32),
        };
        backend.link_pending_mark(pending.clone()).await.unwrap();
        assert_eq!(backend.link_pending().await.unwrap(), Some(pending));
        backend.link_pending_clear().await.unwrap();
        assert_eq!(backend.link_pending().await.unwrap(), None);
        std::fs::remove_dir_all(root).unwrap();
    }
}
