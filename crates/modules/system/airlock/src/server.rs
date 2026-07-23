//! The gateway (credential-side) HTTP service, behind the `server` feature. The
//! `airlock-gateway` binary is a thin wrapper over [`build`]/[`serve`]; tests
//! drive the same router in-process. Enclave keys are minted per process and
//! never leave memory; the operator cannot read the sealed credential back out.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use axum::body::{Body, Bytes};
use axum::extract::{OriginalUri, State};
use axum::http::{header::AUTHORIZATION, HeaderMap, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use axum::Json;
pub use axum::Router;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand_core::OsRng;

use crate::attest;
use crate::bodyseal;
use crate::handshake;
use crate::seal::{self, SealKeypair};
use crate::token::{self, Claims};
use crate::wire::{
    AttestationResponse, CredentialKind, CredentialPayload, CredentialUpload, SessionRequest,
    SessionResponse,
};

/// How the gateway proves its seal key to the broker.
#[derive(Clone)]
pub enum AttestMode {
    /// configfs-tsm hardware attestation. Carries the operator's requested
    /// vendor: `tdx` | `snp` | `auto` (auto probes the silicon).
    Tsm(String),
    /// No TEE. There is no quote; the trust anchor is the seal_pk published on
    /// consensus, which the broker pins. The gateway serves an empty quote under
    /// vendor `self-host`.
    SelfHost,
}

/// Everything the gateway needs to serve. Keys are minted inside [`build`]
/// unless `seal_keypair` injects one (the self-host path pins the on-chain key).
pub struct GatewayConfig {
    pub attest: AttestMode,
    /// The enclave seal keypair. `None` mints a fresh one — the TEE path binds it
    /// into the quote. Self-host injects the on-chain-published keypair so the
    /// broker's pinned seal_pk matches what this gateway seals under.
    pub seal_keypair: Option<SealKeypair>,
    /// Upstream base for `CredentialKind::Claude` credentials.
    pub anthropic_base: String,
    /// Upstream base for `CredentialKind::Codex` credentials.
    pub openai_base: String,
    pub oauth_token_url: String,
    pub oauth_client_id: String,
    pub session_ttl_secs: u64,
    pub max_requests: u32,
}

struct Config {
    anthropic_base: String,
    openai_base: String,
    oauth_token_url: String,
    oauth_client_id: String,
    session_ttl_secs: u64,
    max_requests: u32,
}

struct Oauth {
    access_token: String,
    refresh_token: String,
    expires_at: u64,
}

/// One named credential: its vendor, its (refreshable) token state, and a
/// single-flight gate so two concurrent proxied calls never double-spend one
/// rotating refresh token.
struct CredEntry {
    kind: CredentialKind,
    oauth: Mutex<Oauth>,
    refresh_gate: tokio::sync::Mutex<()>,
}

struct AppState {
    seal_kp: SealKeypair,
    sess_sk: SigningKey,
    sess_pk: VerifyingKey,
    quote: Vec<u8>,
    /// "tdx" | "snp" | "self-host" — advertised so the client picks the right
    /// verifier (or, for self-host, pins the on-chain seal_pk instead).
    vendor: String,
    http: reqwest::Client,
    cfg: Config,
    /// The named credential store, keyed by credential name (== session `sub`).
    /// Seeded at build and/or filled by sealed `/credential` uploads.
    creds: Mutex<HashMap<String, Arc<CredEntry>>>,
    /// Remaining request budget per session `sub` (credential name). Refilled by
    /// asking for a new /session; the overlay ACL gates who may reach it.
    budgets: Mutex<HashMap<String, u32>>,
    /// Per-name sealed-request nonces already served — replay dedupe. Bounded
    /// by the request budget per name; dies with the process like every key.
    seen_nonces: Mutex<HashMap<String, std::collections::HashSet<Vec<u8>>>>,
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

/// Quote generation, injected. Production uses configfs-tsm; the testkit
/// injects a minted-chain quoter. A process that injects a quoter already
/// controls the process — clients only trust what verifies against pinned
/// vendor roots, so this seam grants no forgery power.
pub type Quoter = Box<dyn Fn(&[u8; attest::REPORT_DATA_LEN]) -> Result<Vec<u8>> + Send + Sync>;

/// Build the gateway router and report the vendor ("tdx"/"snp"/"self-host").
pub fn build(cfg: GatewayConfig) -> Result<(Router, String)> {
    build_seeded(cfg, Vec::new())
}

/// Like [`build`], but seed the store with named credentials directly instead of
/// waiting for sealed `/credential` uploads. Used when the credential provider IS
/// the gateway process (the node embed): there is no host-vs-enclave boundary to
/// seal across, so credentials are handed in-process. A `Bearer` is static (no
/// rotation); a claude `Refresh` is refreshed lazily; a codex `Refresh` is
/// refused (bearer-only lane).
pub fn build_seeded(
    cfg: GatewayConfig,
    seeds: Vec<(String, CredentialKind, CredentialPayload)>,
) -> Result<(Router, String)> {
    match cfg.attest.clone() {
        AttestMode::Tsm(spec) => {
            let mode = if spec == "auto" {
                tsm_probe_provider()?
            } else {
                spec.parse::<attest::AttestMode>()?
            };
            build_with_quoter(cfg, mode.as_str(), tsm_quoter(mode), seeds)
        }
        AttestMode::SelfHost => build_self_host(cfg, seeds),
    }
}

fn tsm_quoter(expected: attest::AttestMode) -> Quoter {
    Box::new(move |rd| tsm_gen_quote(Some(expected), rd).map(|(_, quote)| quote))
}

/// Build the gateway with an injected quote generator (see [`build_seeded`]).
/// Mints/takes the enclave seal key, mints the session key, and calls `quoter`
/// once on the freshly bound REPORTDATA.
pub fn build_with_quoter(
    mut cfg: GatewayConfig,
    vendor: &str,
    quoter: Quoter,
    seeds: Vec<(String, CredentialKind, CredentialPayload)>,
) -> Result<(Router, String)> {
    let seal_kp = cfg.seal_keypair.take().unwrap_or_else(SealKeypair::generate);
    let sess_sk = SigningKey::generate(&mut OsRng);
    let sess_pk = sess_sk.verifying_key();

    let report_data = attest::make_report_data(&seal_kp.public_bytes(), &sess_pk.to_bytes());
    let quote = quoter(&report_data)?;
    assemble(cfg, vendor.to_string(), quote, seal_kp, sess_sk, sess_pk, seeds)
}

/// Non-TEE build: no quote, vendor "self-host". The broker pins the seal_pk from
/// consensus, so there is nothing to attest here.
fn build_self_host(
    mut cfg: GatewayConfig,
    seeds: Vec<(String, CredentialKind, CredentialPayload)>,
) -> Result<(Router, String)> {
    let seal_kp = cfg.seal_keypair.take().unwrap_or_else(SealKeypair::generate);
    let sess_sk = SigningKey::generate(&mut OsRng);
    let sess_pk = sess_sk.verifying_key();
    assemble(cfg, "self-host".to_string(), Vec::new(), seal_kp, sess_sk, sess_pk, seeds)
}

/// Shared assembly: build the named store from the seeds, wire the state and the
/// router. The two build paths differ only in vendor/quote/keys, all resolved
/// before this point.
#[allow(clippy::too_many_arguments)]
fn assemble(
    cfg: GatewayConfig,
    vendor: String,
    quote: Vec<u8>,
    seal_kp: SealKeypair,
    sess_sk: SigningKey,
    sess_pk: VerifyingKey,
    seeds: Vec<(String, CredentialKind, CredentialPayload)>,
) -> Result<(Router, String)> {
    let mut creds = HashMap::new();
    for (name, kind, payload) in seeds {
        creds.insert(name, Arc::new(cred_entry(kind, payload)?));
    }

    let state = Arc::new(AppState {
        seal_kp,
        sess_sk,
        sess_pk,
        quote,
        vendor: vendor.clone(),
        http: reqwest::Client::new(),
        cfg: Config {
            anthropic_base: cfg.anthropic_base,
            openai_base: cfg.openai_base,
            oauth_token_url: cfg.oauth_token_url,
            oauth_client_id: cfg.oauth_client_id,
            session_ttl_secs: cfg.session_ttl_secs,
            max_requests: cfg.max_requests,
        },
        creds: Mutex::new(creds),
        budgets: Mutex::new(HashMap::new()),
        seen_nonces: Mutex::new(HashMap::new()),
    });

    let app = Router::new()
        .route("/attestation", get(attestation))
        .route("/credential", post(credential))
        .route("/session", post(session))
        // Proxy the whole /v1/* surface (Claude Code calls /v1/messages and
        // /v1/messages/count_tokens, not just messages).
        .route("/v1/{*rest}", any(proxy))
        .with_state(state);
    Ok((app, vendor))
}

/// Turn a seed/upload payload into a [`CredEntry`]. A `Bearer` is static
/// (`expires_at = MAX`, never refreshed); a claude `Refresh` starts empty and is
/// exchanged lazily; a codex `Refresh` is rejected — codex is bearer-only in v1.
fn cred_entry(kind: CredentialKind, payload: CredentialPayload) -> Result<CredEntry> {
    let oauth = match (kind, payload) {
        (_, CredentialPayload::Bearer { access_token }) => Oauth {
            access_token,
            refresh_token: String::new(),
            expires_at: u64::MAX,
        },
        (CredentialKind::Claude, CredentialPayload::Refresh { refresh_token }) => Oauth {
            access_token: String::new(),
            refresh_token,
            expires_at: 0,
        },
        (CredentialKind::Codex, CredentialPayload::Refresh { .. }) => {
            bail!("codex credentials must be a static bearer token; oauth refresh is not supported")
        }
    };
    Ok(CredEntry { kind, oauth: Mutex::new(oauth), refresh_gate: tokio::sync::Mutex::new(()) })
}

/// Serve an already-built gateway router. The node embed BUILDS (and thus
/// attests) at boot — before registering its route or claiming to listen —
/// and only serves here; a box that cannot attest never reaches this point.
pub async fn serve_router(listener: tokio::net::TcpListener, app: Router) -> Result<()> {
    axum::serve(listener, app).await?;
    Ok(())
}

/// Generate a real confidential-VM quote via `configfs-tsm` (kernel >= 6.7,
/// only inside a CVM guest). Vendor-generic: Intel TDX and AMD SEV-SNP both write
/// REPORTDATA to `inblob` and read the raw report/quote from `outblob`; the
/// `provider` attribute names the vendor. `expected` is the operator's requested
/// vendor (`None` = auto); a mismatch errors. Untested off-hardware.
fn tsm_gen_quote(
    expected: Option<attest::AttestMode>,
    report_data: &[u8; attest::REPORT_DATA_LEN],
) -> Result<(attest::AttestMode, Vec<u8>)> {
    use std::fs;
    let dir = format!("/sys/kernel/config/tsm/report/airlock-{}", std::process::id());
    fs::create_dir(&dir)
        .with_context(|| format!("create {dir} (are we inside a TDX/SEV-SNP guest?)"))?;
    let result = (|| -> Result<(attest::AttestMode, Vec<u8>)> {
        let provider =
            fs::read_to_string(format!("{dir}/provider")).context("read configfs-tsm provider")?;
        let detected = provider_to_mode(provider.trim())?;
        if let Some(want) = expected
            && want != detected
        {
            bail!(
                "attest={} but the guest reports {} ({provider:?})",
                want.as_str(),
                detected.as_str()
            );
        }
        fs::write(format!("{dir}/inblob"), report_data).context("write inblob")?;
        let quote = fs::read(format!("{dir}/outblob")).context("read outblob")?;
        Ok((detected, quote))
    })();
    let _ = fs::remove_dir(&dir);
    result
}

fn provider_to_mode(provider: &str) -> Result<attest::AttestMode> {
    match provider {
        "tdx_guest" => Ok(attest::AttestMode::Tdx),
        "sev_guest" => Ok(attest::AttestMode::Snp),
        other => bail!("unsupported configfs-tsm provider {other:?} (want tdx_guest/sev_guest)"),
    }
}

/// Probe the configfs-tsm provider to learn the hardware vendor without
/// generating a quote (the `auto` mode).
fn tsm_probe_provider() -> Result<attest::AttestMode> {
    use std::fs;
    let dir = format!("/sys/kernel/config/tsm/report/airlock-probe-{}", std::process::id());
    fs::create_dir(&dir)
        .with_context(|| format!("create {dir} (are we inside a TDX/SEV-SNP guest?)"))?;
    let provider = fs::read_to_string(format!("{dir}/provider"));
    let _ = fs::remove_dir(&dir);
    provider_to_mode(provider.context("read configfs-tsm provider")?.trim())
}

// -------- handlers --------

struct AppErr(StatusCode, String);
impl IntoResponse for AppErr {
    fn into_response(self) -> Response {
        (self.0, self.1).into_response()
    }
}

async fn attestation(State(st): State<Arc<AppState>>) -> Json<AttestationResponse> {
    Json(AttestationResponse {
        quote_b64: BASE64.encode(&st.quote),
        vendor: st.vendor.clone(),
    })
}

async fn credential(
    State(st): State<Arc<AppState>>,
    Json(up): Json<CredentialUpload>,
) -> Result<StatusCode, AppErr> {
    let blob = BASE64
        .decode(up.sealed_b64)
        .map_err(|e| AppErr(StatusCode::BAD_REQUEST, format!("bad base64: {e}")))?;
    let pt = seal::unseal(&st.seal_kp, &blob)
        .map_err(|e| AppErr(StatusCode::BAD_REQUEST, e.to_string()))?;
    let payload: CredentialPayload = serde_json::from_slice(&pt)
        .map_err(|e| AppErr(StatusCode::BAD_REQUEST, format!("bad payload: {e}")))?;

    let entry = Arc::new(
        cred_entry(up.kind, payload).map_err(|e| AppErr(StatusCode::BAD_REQUEST, e.to_string()))?,
    );
    // A claude refresh credential starts with no access token — prove it works
    // now, so a later /v1/messages isn't the first time we learn it's broken. A
    // static bearer already holds its token; nothing to probe.
    let needs_probe = entry.oauth.lock().unwrap().access_token.is_empty();
    if needs_probe {
        refresh_now(&st.cfg, &st.http, &entry).await.map_err(|e| {
            AppErr(StatusCode::BAD_GATEWAY, format!("initial refresh failed: {e}"))
        })?;
    }
    st.creds.lock().unwrap().insert(up.name, entry);
    Ok(StatusCode::OK)
}

async fn session(
    State(st): State<Arc<AppState>>,
    Json(req): Json<SessionRequest>,
) -> Result<Json<SessionResponse>, AppErr> {
    // A session names the credential it draws on; refuse an unknown name before
    // any handshake work (the broker resolves the name from consensus first, so
    // a miss here is a race or a stale record).
    let known_credential = st.creds.lock().unwrap().contains_key(&req.sub);
    if !known_credential {
        return Err(AppErr(StatusCode::NOT_FOUND, "credential_not_found".into()));
    }
    // Enclave side of the handshake: derive the shared key from the client's
    // ephemeral key and our static seal secret, then seal the token under it — so
    // only the client that ECDH'd against the *attested* seal_pk can open it.
    let eph = BASE64
        .decode(&req.client_eph_pk_b64)
        .ok()
        .and_then(|v| <[u8; 32]>::try_from(v).ok())
        .ok_or_else(|| AppErr(StatusCode::BAD_REQUEST, "bad client_eph_pk".into()))?;
    let keys = handshake::enclave_session_keys(&st.seal_kp, &eph);

    let now = now_secs();
    let claims = Claims {
        sub: req.sub.clone(),
        iat: now,
        exp: now + st.cfg.session_ttl_secs,
        max_requests: st.cfg.max_requests,
        eph: req.client_eph_pk_b64.clone(),
        seal: req.body_seal,
    };
    st.budgets.lock().unwrap().insert(req.sub, st.cfg.max_requests);
    let token = token::issue(&st.sess_sk, &claims);
    let sealed = handshake::seal_token(&keys.session, token.as_bytes());
    Ok(Json(SessionResponse { sealed_token_b64: BASE64.encode(sealed) }))
}

async fn proxy(
    State(st): State<Arc<AppState>>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    match proxy_inner(&st, method, &uri, &headers, body).await {
        Ok(r) => r,
        Err(e) => e.into_response(),
    }
}

async fn proxy_inner(
    st: &AppState,
    method: Method,
    uri: &axum::http::Uri,
    headers: &HeaderMap,
    body: Bytes,
) -> Result<Response, AppErr> {
    let path_and_query = uri.path_and_query().map(|pq| pq.as_str()).unwrap_or(uri.path());

    let bearer = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or_else(|| AppErr(StatusCode::UNAUTHORIZED, "missing bearer token".into()))?;

    let claims = token::verify(&st.sess_pk, bearer)
        .map_err(|e| AppErr(StatusCode::UNAUTHORIZED, format!("bad session token: {e}")))?;

    let now = now_secs();
    if claims.exp < now {
        return Err(AppErr(StatusCode::UNAUTHORIZED, "session token expired".into()));
    }
    // The session's `sub` names the credential it draws on; resolve it now — its
    // kind selects the upstream and its own token state is what we refresh/spend.
    let entry = st
        .creds
        .lock()
        .unwrap()
        .get(&claims.sub)
        .cloned()
        .ok_or_else(|| AppErr(StatusCode::NOT_FOUND, "credential_not_found".into()))?;
    // Sealed-body session: re-derive the handshake keys statelessly from the
    // claims' ephemeral pk, unseal the request, and REFUSE plaintext — a
    // stolen bearer alone (visible to path hosts) cannot produce a sealable
    // body. A plaintext session must not send the seal header. The raw blob's
    // nonce becomes (a) the per-sub replay-dedup key and (b) the binding the
    // response stream key is derived under, so an authentic response cannot
    // be replayed as the answer to a different request.
    let seal_keys = if claims.seal {
        let eph = BASE64
            .decode(&claims.eph)
            .ok()
            .and_then(|v| <[u8; 32]>::try_from(v).ok())
            .ok_or_else(|| AppErr(StatusCode::UNAUTHORIZED, "bad eph in claims".into()))?;
        Some(handshake::enclave_session_keys(&st.seal_kp, &eph))
    } else {
        None
    };
    let sealed_request = headers
        .get(bodyseal::SEAL_HEADER)
        .and_then(|v| v.to_str().ok())
        == Some(bodyseal::SEAL_V1);
    let binding = bodyseal::request_binding(&body);
    let body = match (&seal_keys, sealed_request) {
        (Some(keys), true) => {
            let replayed = !st
                .seen_nonces
                .lock()
                .unwrap()
                .entry(claims.sub.clone())
                .or_default()
                .insert(binding.clone());
            if replayed {
                return Err(AppErr(
                    StatusCode::BAD_REQUEST,
                    "airlock: replayed sealed request".into(),
                ));
            }
            Bytes::from(
                bodyseal::open_request(keys, &body)
                    .map_err(|e| AppErr(StatusCode::BAD_REQUEST, format!("airlock: {e}")))?,
            )
        }
        (Some(_), false) if !body.is_empty() => {
            return Err(AppErr(
                StatusCode::BAD_REQUEST,
                "airlock: sealed session requires a sealed body".into(),
            ));
        }
        (Some(_), false) => body,
        (None, true) => {
            return Err(AppErr(
                StatusCode::BAD_REQUEST,
                "airlock: session was not opened for sealed bodies".into(),
            ));
        }
        (None, false) => body,
    };

    // Budget spends only AFTER the sealed body validated — a path host feeding
    // garbage blobs must not burn the session's requests (review finding).
    {
        let mut b = st.budgets.lock().unwrap();
        let rem = b
            .get_mut(&claims.sub)
            .ok_or_else(|| AppErr(StatusCode::FORBIDDEN, "no budget for sub".into()))?;
        if *rem == 0 {
            return Err(AppErr(StatusCode::TOO_MANY_REQUESTS, "budget exhausted".into()));
        }
        *rem -= 1;
    }

    // Ensure a fresh access token, then swap session token -> real credential.
    let stale = {
        let o = entry.oauth.lock().unwrap();
        o.access_token.is_empty() || o.expires_at <= now
    };
    if stale {
        refresh_now(&st.cfg, &st.http, &entry)
            .await
            .map_err(|e| AppErr(StatusCode::BAD_GATEWAY, format!("refresh: {e}")))?;
    }
    let access = {
        let o = entry.oauth.lock().unwrap();
        if o.access_token.is_empty() {
            return Err(AppErr(StatusCode::BAD_GATEWAY, "no credential loaded".into()));
        }
        o.access_token.clone()
    };

    // Upstream base is chosen by the credential's vendor kind.
    let upstream_base = match entry.kind {
        CredentialKind::Claude => &st.cfg.anthropic_base,
        CredentialKind::Codex => &st.cfg.openai_base,
    };
    let url = format!("{}{}", upstream_base.trim_end_matches('/'), path_and_query);
    let mut rb = st.http.request(method, &url).body(body.to_vec());
    // Forward the caller's headers verbatim, minus ones we own or that would
    // break the relay. bearer_auth then plants the real credential.
    for (name, value) in headers.iter() {
        if matches!(
            name.as_str(),
            "authorization" | "host" | "content-length" | "accept-encoding" | "x-airlock-body-seal"
        ) {
            continue;
        }
        rb = rb.header(name, value);
    }
    let resp = rb
        .bearer_auth(&access)
        .send()
        .await
        .map_err(|e| AppErr(StatusCode::BAD_GATEWAY, format!("upstream: {e}")))?;

    let status = resp.status();
    let ct = resp.headers().get("content-type").cloned();
    let Some(keys) = seal_keys else {
        // Plaintext session: passthrough, streamed.
        let mut builder = Response::builder().status(status.as_u16());
        if let Some(v) = ct {
            builder = builder.header("content-type", v);
        }
        return builder
            .body(Body::from_stream(resp.bytes_stream()))
            .map_err(|e| AppErr(StatusCode::INTERNAL_SERVER_ERROR, format!("build response: {e}")));
    };

    // Sealed session: re-seal the upstream stream chunk by chunk. The inner
    // content-type rides the sealed head chunk; the outer body is opaque. An
    // upstream error mid-stream ends WITHOUT the final marker — the broker
    // sees authenticated truncation, never a silent clean EOF.
    let inner_ct = ct
        .as_ref()
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();
    let (mut sealer, salt) = bodyseal::StreamSealer::new(&keys, &binding);
    let head_chunk = sealer.seal_head(&inner_ct);
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, std::io::Error>>(8);
    tokio::spawn(async move {
        use futures_util::StreamExt as _;
        let mut opening = salt;
        opening.extend(head_chunk);
        if tx.send(Ok(Bytes::from(opening))).await.is_err() {
            return;
        }
        let mut upstream = resp.bytes_stream();
        while let Some(chunk) = upstream.next().await {
            match chunk {
                Ok(chunk) => {
                    if tx.send(Ok(Bytes::from(sealer.seal_chunk(&chunk)))).await.is_err() {
                        return;
                    }
                }
                Err(_) => return, // no Final marker -> authenticated truncation
            }
        }
        let _ = tx.send(Ok(Bytes::from(sealer.seal_final()))).await;
    });
    Response::builder()
        .status(status.as_u16())
        .header("content-type", "application/octet-stream")
        .body(Body::from_stream(tokio_stream::wrappers::ReceiverStream::new(rx)))
        .map_err(|e| AppErr(StatusCode::INTERNAL_SERVER_ERROR, format!("build response: {e}")))
}

/// Exchange one credential's refresh token for a fresh access token (and rotated
/// refresh token), single-flighted per credential so concurrent callers never
/// double-spend it.
async fn refresh_now(cfg: &Config, http: &reqwest::Client, entry: &CredEntry) -> Result<()> {
    let _gate = entry.refresh_gate.lock().await;
    // Re-check under the gate — a caller we queued behind may have just done it.
    let refresh = {
        let o = entry.oauth.lock().unwrap();
        if !o.access_token.is_empty() && o.expires_at > now_secs() {
            return Ok(());
        }
        o.refresh_token.clone()
    };

    let resp = http
        .post(&cfg.oauth_token_url)
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh.as_str()),
            ("client_id", cfg.oauth_client_id.as_str()),
        ])
        .send()
        .await?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        bail!("oauth token endpoint {status}: {text}");
    }
    let j: serde_json::Value = serde_json::from_str(&text).context("oauth response json")?;
    let access = j["access_token"].as_str().context("no access_token")?.to_string();
    let new_refresh = j["refresh_token"].as_str().map(|s| s.to_string());
    let expires_in = j["expires_in"].as_u64().unwrap_or(3600);

    let now = now_secs();
    let mut o = entry.oauth.lock().unwrap();
    o.access_token = access;
    if let Some(r) = new_refresh {
        o.refresh_token = r; // memory-only; lost on restart, re-seal to recover
    }
    o.expires_at = now + expires_in.saturating_sub(60);
    Ok(())
}
