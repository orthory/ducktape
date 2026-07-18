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
use axum::{Json, Router};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand_core::OsRng;

use crate::attest::{self, AttestMode, Measurement};
use crate::handshake;
use crate::seal::{self, SealKeypair};
use crate::token::{self, Claims};
use crate::wire::{
    AttestationResponse, CredentialPayload, CredentialUpload, SessionRequest, SessionResponse,
};

/// Everything the gateway needs to serve. Keys are minted inside [`build`].
pub struct GatewayConfig {
    /// mock | tdx | snp | auto.
    pub attest: String,
    /// 48-byte hex; required for `--attest mock`.
    pub measurement: Option<String>,
    pub anthropic_base: String,
    pub oauth_token_url: String,
    pub oauth_client_id: String,
    pub session_ttl_secs: u64,
    pub max_requests: u32,
}

struct Config {
    anthropic_base: String,
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

struct AppState {
    seal_kp: SealKeypair,
    sess_sk: SigningKey,
    sess_pk: VerifyingKey,
    quote: Vec<u8>,
    /// "mock" | "tdx" | "snp" — advertised so the client picks the right verifier.
    vendor: String,
    http: reqwest::Client,
    cfg: Config,
    oauth: Mutex<Option<Oauth>>,
    /// Single-flight gate around the OAuth refresh: held across the token POST so
    /// two concurrent callers cannot both spend the same rotating refresh token.
    refresh_gate: tokio::sync::Mutex<()>,
    /// Remaining request budget per session `sub`. Refillable by asking for a new
    /// /session and unbounded in `sub`; the overlay ACL gates who may reach it.
    budgets: Mutex<HashMap<String, u32>>,
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

/// Build the gateway router and report the detected vendor ("mock"/"tdx"/"snp").
/// Mints the enclave keys and generates the attestation quote.
pub fn build(cfg: GatewayConfig) -> Result<(Router, String)> {
    // Enclave-bound keys, memory only.
    let seal_kp = SealKeypair::generate();
    let sess_sk = SigningKey::generate(&mut OsRng);
    let sess_pk = sess_sk.verifying_key();

    let seal_pk = seal_kp.public_bytes();
    let report_data = attest::make_report_data(&seal_pk, &sess_pk.to_bytes());

    // `auto` picks the vendor from the hardware; explicit mock|tdx|snp are as named.
    let (mode, quote) = if cfg.attest == "auto" {
        tsm_gen_quote(None, &report_data)?
    } else {
        let mode: AttestMode = cfg.attest.parse()?;
        match mode {
            AttestMode::Mock => {
                let m = cfg
                    .measurement
                    .as_deref()
                    .context("measurement is required for attest=mock")?;
                (mode, attest::mock_quote(&report_data, &Measurement::from_hex(m)?))
            }
            AttestMode::Tdx | AttestMode::Snp => tsm_gen_quote(Some(mode), &report_data)?,
        }
    };
    let vendor = mode.as_str().to_string();

    let state = Arc::new(AppState {
        seal_kp,
        sess_sk,
        sess_pk,
        quote,
        vendor: vendor.clone(),
        http: reqwest::Client::new(),
        cfg: Config {
            anthropic_base: cfg.anthropic_base,
            oauth_token_url: cfg.oauth_token_url,
            oauth_client_id: cfg.oauth_client_id,
            session_ttl_secs: cfg.session_ttl_secs,
            max_requests: cfg.max_requests,
        },
        oauth: Mutex::new(None),
        refresh_gate: tokio::sync::Mutex::new(()),
        budgets: Mutex::new(HashMap::new()),
    });

    let app = Router::new()
        .route("/attestation", get(attestation))
        .route("/credential", post(credential))
        .route("/session", post(session))
        // Proxy the whole Anthropic /v1/* surface (Claude Code calls
        // /v1/messages and /v1/messages/count_tokens, not just messages).
        .route("/v1/{*rest}", any(proxy))
        .with_state(state);
    Ok((app, vendor))
}

/// Bind-and-serve helper for the binary.
pub async fn serve(listener: tokio::net::TcpListener, cfg: GatewayConfig) -> Result<()> {
    let (app, _vendor) = build(cfg)?;
    axum::serve(listener, app).await?;
    Ok(())
}

/// Generate a real confidential-VM quote via `configfs-tsm` (kernel >= 6.7,
/// only inside a CVM guest). Vendor-generic: Intel TDX and AMD SEV-SNP both write
/// REPORTDATA to `inblob` and read the raw report/quote from `outblob`; the
/// `provider` attribute names the vendor. `expected` is the operator's requested
/// vendor (`None` = auto); a mismatch errors. Untested off-hardware.
fn tsm_gen_quote(
    expected: Option<AttestMode>,
    report_data: &[u8; attest::REPORT_DATA_LEN],
) -> Result<(AttestMode, Vec<u8>)> {
    use std::fs;
    let dir = format!("/sys/kernel/config/tsm/report/airlock-{}", std::process::id());
    fs::create_dir(&dir)
        .with_context(|| format!("create {dir} (are we inside a TDX/SEV-SNP guest?)"))?;
    let result = (|| -> Result<(AttestMode, Vec<u8>)> {
        let provider =
            fs::read_to_string(format!("{dir}/provider")).context("read configfs-tsm provider")?;
        let detected = match provider.trim() {
            "tdx_guest" => AttestMode::Tdx,
            "sev_guest" => AttestMode::Snp,
            other => bail!("unsupported configfs-tsm provider {other:?} (want tdx_guest/sev_guest)"),
        };
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

    *st.oauth.lock().unwrap() = Some(Oauth {
        access_token: String::new(),
        refresh_token: payload.refresh_token,
        expires_at: 0,
    });
    // Prove the credential works now, so a later /v1/messages isn't the first
    // time we learn it's broken.
    refresh_now(&st)
        .await
        .map_err(|e| AppErr(StatusCode::BAD_GATEWAY, format!("initial refresh failed: {e}")))?;
    Ok(StatusCode::OK)
}

async fn session(
    State(st): State<Arc<AppState>>,
    Json(req): Json<SessionRequest>,
) -> Result<Json<SessionResponse>, AppErr> {
    // Enclave side of the handshake: derive the shared key from the client's
    // ephemeral key and our static seal secret, then seal the token under it — so
    // only the client that ECDH'd against the *attested* seal_pk can open it.
    let eph = BASE64
        .decode(&req.client_eph_pk_b64)
        .ok()
        .and_then(|v| <[u8; 32]>::try_from(v).ok())
        .ok_or_else(|| AppErr(StatusCode::BAD_REQUEST, "bad client_eph_pk".into()))?;
    let session_key = handshake::enclave_session_key(&st.seal_kp, &eph);

    let now = now_secs();
    let claims = Claims {
        sub: req.sub.clone(),
        iat: now,
        exp: now + st.cfg.session_ttl_secs,
        max_requests: st.cfg.max_requests,
    };
    st.budgets.lock().unwrap().insert(req.sub, st.cfg.max_requests);
    let token = token::issue(&st.sess_sk, &claims);
    let sealed = handshake::seal_token(&session_key, token.as_bytes());
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
    let stale = st
        .oauth
        .lock()
        .unwrap()
        .as_ref()
        .map(|o| o.access_token.is_empty() || o.expires_at <= now)
        .unwrap_or(true);
    if stale {
        refresh_now(st)
            .await
            .map_err(|e| AppErr(StatusCode::BAD_GATEWAY, format!("refresh: {e}")))?;
    }
    let access = st
        .oauth
        .lock()
        .unwrap()
        .as_ref()
        .map(|o| o.access_token.clone())
        .filter(|a| !a.is_empty())
        .ok_or_else(|| AppErr(StatusCode::BAD_GATEWAY, "no credential loaded".into()))?;

    let url = format!("{}{}", st.cfg.anthropic_base.trim_end_matches('/'), path_and_query);
    let mut rb = st.http.request(method, &url).body(body.to_vec());
    // Forward the caller's headers verbatim, minus ones we own or that would
    // break the relay. bearer_auth then plants the real credential.
    for (name, value) in headers.iter() {
        if matches!(
            name.as_str(),
            "authorization" | "host" | "content-length" | "accept-encoding"
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
    let mut builder = Response::builder().status(status.as_u16());
    if let Some(v) = ct {
        builder = builder.header("content-type", v);
    }
    builder
        .body(Body::from_stream(resp.bytes_stream()))
        .map_err(|e| AppErr(StatusCode::INTERNAL_SERVER_ERROR, format!("build response: {e}")))
}

/// Exchange the refresh token for a fresh access token (and rotated refresh
/// token), single-flighted so concurrent callers never double-spend it.
async fn refresh_now(st: &AppState) -> Result<()> {
    let _gate = st.refresh_gate.lock().await;
    // Re-check under the gate — a caller we queued behind may have just done it.
    let refresh = {
        let g = st.oauth.lock().unwrap();
        let o = g.as_ref().context("no credential to refresh")?;
        if !o.access_token.is_empty() && o.expires_at > now_secs() {
            return Ok(());
        }
        o.refresh_token.clone()
    };

    let resp = st
        .http
        .post(&st.cfg.oauth_token_url)
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh.as_str()),
            ("client_id", st.cfg.oauth_client_id.as_str()),
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
    let mut g = st.oauth.lock().unwrap();
    if let Some(o) = g.as_mut() {
        o.access_token = access;
        if let Some(r) = new_refresh {
            o.refresh_token = r; // memory-only; lost on restart, re-seal to recover
        }
        o.expires_at = now + expires_in.saturating_sub(60);
    }
    Ok(())
}
