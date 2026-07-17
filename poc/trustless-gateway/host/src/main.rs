//! `tcg-host` — the Trustless Gateway. Runs (canonically) inside an Intel TDX
//! confidential VM. Holds a sealed OAuth refresh token in enclave memory,
//! proxies the Claude messages API, and issues scoped session tokens. The host
//! operator cannot read the credential.
//!
//! Subcommands:
//!   serve          the gateway itself
//!   mock-upstream  a fake OAuth + messages server, for the hermetic demo

mod mock_upstream;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{header::AUTHORIZATION, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use clap::{Args, Parser, Subcommand};
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand_core::OsRng;
use tokio::net::TcpListener;

use tcg_core::attest::{self, AttestMode, Measurement};
use tcg_core::seal::{self, SealKeypair};
use tcg_core::token::{self, Claims};
use tcg_core::wire::{
    AttestationResponse, CredentialPayload, CredentialUpload, SessionRequest, SessionResponse,
};

#[derive(Parser)]
#[command(name = "tcg-host", about = "Trustless credential gateway (TEE exit node)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Serve(ServeArgs),
    MockUpstream(MockArgs),
}

#[derive(Args)]
struct ServeArgs {
    #[arg(long, default_value = "127.0.0.1:9100")]
    listen: String,
    /// mock | tdx
    #[arg(long, default_value = "mock")]
    attest: String,
    /// Expected/embedded measurement (48-byte hex). Required for --attest mock.
    #[arg(long)]
    measurement: Option<String>,
    #[arg(long, default_value = "http://127.0.0.1:9101")]
    anthropic_base: String,
    #[arg(long, default_value = "http://127.0.0.1:9101/oauth/token")]
    oauth_token_url: String,
    #[arg(long, default_value = "9d1c250a-e61b-44d9-88ed-5944d1962f5e")]
    oauth_client_id: String,
    #[arg(long, default_value_t = 3600)]
    session_ttl_secs: u64,
    #[arg(long, default_value_t = 1000)]
    max_requests: u32,
}

#[derive(Args)]
struct MockArgs {
    #[arg(long, default_value = "127.0.0.1:9101")]
    listen: String,
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
    http: reqwest::Client,
    cfg: Config,
    oauth: Mutex<Option<Oauth>>,
    /// Remaining request budget per session `sub`.
    budgets: Mutex<HashMap<String, u32>>,
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

#[tokio::main]
async fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Serve(a) => serve(a).await,
        Cmd::MockUpstream(a) => mock_upstream::run(&a.listen).await,
    }
}

async fn serve(args: ServeArgs) -> Result<()> {
    let mode: AttestMode = args.attest.parse()?;

    // Enclave-bound keys, memory only.
    let seal_kp = SealKeypair::generate();
    let sess_sk = SigningKey::generate(&mut OsRng);
    let sess_pk = sess_sk.verifying_key();

    let seal_pk = seal_kp.public_bytes();
    let report_data = attest::make_report_data(&seal_pk, &sess_pk.to_bytes());

    let quote = match mode {
        AttestMode::Mock => {
            let m = args
                .measurement
                .as_deref()
                .context("--measurement is required for --attest mock")?;
            attest::mock_quote(&report_data, &Measurement::from_hex(m)?)
        }
        AttestMode::Tdx => tdx_gen_quote(&report_data)?,
    };
    eprintln!(
        "[host] attest={:?} quote={} bytes; seal_pk+sess_pk bound in REPORTDATA",
        mode,
        quote.len()
    );

    let state = Arc::new(AppState {
        seal_kp,
        sess_sk,
        sess_pk,
        quote,
        http: reqwest::Client::new(),
        cfg: Config {
            anthropic_base: args.anthropic_base,
            oauth_token_url: args.oauth_token_url,
            oauth_client_id: args.oauth_client_id,
            session_ttl_secs: args.session_ttl_secs,
            max_requests: args.max_requests,
        },
        oauth: Mutex::new(None),
        budgets: Mutex::new(HashMap::new()),
    });

    let app = Router::new()
        .route("/attestation", get(attestation))
        .route("/credential", post(credential))
        .route("/session", post(session))
        .route("/v1/messages", post(messages))
        .with_state(state);

    let listener = TcpListener::bind(&args.listen).await?;
    eprintln!("[host] listening on {}", args.listen);
    axum::serve(listener, app).await?;
    Ok(())
}

/// Generate a real TDX quote via configfs-tsm (kernel >= 6.7, only inside a TD).
/// Untested off-hardware; validate on the TDX box.
fn tdx_gen_quote(report_data: &[u8; attest::REPORT_DATA_LEN]) -> Result<Vec<u8>> {
    use std::fs;
    let dir = format!("/sys/kernel/config/tsm/report/tcg-{}", std::process::id());
    fs::create_dir(&dir)
        .with_context(|| format!("create {dir} (are we inside a TDX guest?)"))?;
    let write_res = (|| -> Result<Vec<u8>> {
        fs::write(format!("{dir}/inblob"), report_data).context("write inblob")?;
        fs::read(format!("{dir}/outblob")).context("read outblob")
    })();
    let _ = fs::remove_dir(&dir);
    write_res
}

// -------- handlers --------

struct AppErr(StatusCode, String);
impl IntoResponse for AppErr {
    fn into_response(self) -> Response {
        (self.0, self.1).into_response()
    }
}

async fn attestation(State(st): State<Arc<AppState>>) -> Json<AttestationResponse> {
    Json(AttestationResponse { quote_b64: BASE64.encode(&st.quote) })
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
    eprintln!("[host] credential sealed-in and refreshed; access token ready");
    Ok(StatusCode::OK)
}

async fn session(
    State(st): State<Arc<AppState>>,
    Json(req): Json<SessionRequest>,
) -> Json<SessionResponse> {
    let now = now_secs();
    let claims = Claims {
        sub: req.sub.clone(),
        iat: now,
        exp: now + st.cfg.session_ttl_secs,
        max_requests: st.cfg.max_requests,
    };
    st.budgets.lock().unwrap().insert(req.sub, st.cfg.max_requests);
    Json(SessionResponse { token: token::issue(&st.sess_sk, &claims) })
}

async fn messages(State(st): State<Arc<AppState>>, headers: HeaderMap, body: Bytes) -> Response {
    match messages_inner(&st, &headers, body).await {
        Ok(r) => r,
        Err(e) => e.into_response(),
    }
}

async fn messages_inner(st: &AppState, headers: &HeaderMap, body: Bytes) -> Result<Response, AppErr> {
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

    let url = format!("{}/v1/messages", st.cfg.anthropic_base.trim_end_matches('/'));
    let mut rb = st.http.post(&url).bearer_auth(&access).body(body.to_vec());
    for h in ["content-type", "anthropic-version", "anthropic-beta", "accept"] {
        if let Some(v) = headers.get(h) {
            rb = rb.header(h, v);
        }
    }
    let resp = rb
        .send()
        .await
        .map_err(|e| AppErr(StatusCode::BAD_GATEWAY, format!("upstream: {e}")))?;

    let status = resp.status();
    let ct = resp.headers().get("content-type").cloned();
    let mut builder = Response::builder().status(status.as_u16());
    if let Some(v) = ct {
        builder = builder.header("content-type", v);
    }
    Ok(builder
        .body(Body::from_stream(resp.bytes_stream()))
        .expect("valid response"))
}

/// Exchange the refresh token for a fresh access token (and rotated refresh
/// token). Mirrors the current broker's OAuth refresh.
async fn refresh_now(st: &AppState) -> Result<()> {
    let refresh = st
        .oauth
        .lock()
        .unwrap()
        .as_ref()
        .map(|o| o.refresh_token.clone())
        .context("no credential to refresh")?;

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
            o.refresh_token = r; // ponytail: memory-only; lost on TD restart, re-seal to recover
        }
        o.expires_at = now + expires_in.saturating_sub(60);
    }
    Ok(())
}
