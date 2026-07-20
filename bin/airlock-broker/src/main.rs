//! `airlock-broker` — the Computation Provider's local api-snatch. This is the piece
//! that lets an UNMODIFIED agent sandbox (the real `claude` CLI) run against a
//! gateway it never authenticates to directly:
//!
//!   sandbox --ANTHROPIC_BASE_URL--> airlock-broker (loopback) --session token--> gateway (TEE)
//!
//! On startup the broker verifies the gateway quote and runs the session-key
//! handshake ONCE, holding the scoped session token. The sandbox talks only to
//! the broker with an opaque per-run bearer; it never sees the session token or
//! the credential. The gateway may be LOCAL (`--gateway-host`, Credential
//! Provider == Computation Provider) or REMOTE (`--remote <handle>.duck --via`,
//! routed onto the overlay). This mirrors the production capability-host broker's
//! Remote mode; here it is a standalone binary so the whole flow runs on one box.

use std::sync::Arc;

use anyhow::{Context, Result};
use axum::body::{Body, Bytes};
use axum::extract::{OriginalUri, State};
use axum::http::{header::AUTHORIZATION, HeaderMap, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, head};
use axum::Router;
use rand::RngCore as _;
use tokio::net::TcpListener;
use tokio::sync::Mutex;

use airlock::attest::{self, AttestMode, Measurement};
use airlock::bodyseal::{self, OpenedItem};
use airlock::handshake::SessionKeys;
use airlock::client::Gateway;

/// `--flag value` / `--flag=value` lookup over argv (no clap — house rule).
fn arg(name: &str) -> Option<String> {
    let mut it = std::env::args();
    while let Some(a) = it.next() {
        if a == name {
            return it.next();
        }
        if let Some(v) = a.strip_prefix(&format!("{name}=")) {
            return Some(v.to_string());
        }
    }
    None
}

fn arg_or(name: &str, default: &str) -> String {
    arg(name).unwrap_or_else(|| default.to_string())
}

struct ServeArgs {
    /// Loopback address the sandbox uses as ANTHROPIC_BASE_URL.
    listen: String,
    /// LOCAL gateway URL (Credential Provider == Computation Provider).
    gateway_host: String,
    /// REMOTE gateway duckdns handle (Credential Provider != Computation
    /// Provider). Requires --via.
    remote: Option<String>,
    /// The local node's browser-gateway base URL that routes duck:// authorities
    /// onto the overlay. Required with --remote.
    via: Option<String>,
    /// tdx | snp — how to verify the gateway quote. No default.
    attest: String,
    /// Expected audited-image measurement (48-byte hex).
    measurement: String,
    /// Session scope reported to the gateway (on the mesh, the overlay AccountId).
    sub: String,
}

fn parse_args() -> Result<ServeArgs> {
    Ok(ServeArgs {
        listen: arg_or("--listen", "127.0.0.1:9200"),
        gateway_host: arg_or("--gateway-host", "http://127.0.0.1:9100"),
        remote: arg("--remote"),
        via: arg("--via"),
        attest: arg("--attest").context("--attest is required (tdx|snp)")?,
        measurement: arg("--measurement").context("--measurement is required")?,
        sub: arg_or("--sub", "compute-provider"),
    })
}

/// Flags -> typed trust roots (tdx/snp only). The roots themselves are pinned
/// (Intel inside dcap-qvl, AMD ARK/ASK from the sev builtins); flags select the
/// product and transport (PCCS URL, VCEK file).
fn resolve_roots(mode: AttestMode) -> Result<airlock::verify::TrustRoots> {
    use airlock::verify::{SnpProduct, SnpRoots, TdxRoots, TrustRoots, VcekSource};
    match mode {
        AttestMode::Tdx => Ok(TrustRoots::Tdx(TdxRoots { pccs_url: arg("--pccs-url") })),
        AttestMode::Snp => {
            let product: SnpProduct = arg("--snp-product")
                .context("--attest snp requires --snp-product milan|genoa|turin")?
                .parse()?;
            let vcek = match arg("--snp-vcek") {
                Some(path) => VcekSource::Der(
                    std::fs::read(&path).with_context(|| format!("read {path}"))?,
                ),
                None => VcekSource::Kds,
            };
            Ok(TrustRoots::Snp(Box::new(SnpRoots::amd(product, vcek)?)))
        }
    }
}

struct BrokerState {
    gateway: Gateway,
    /// The attested seal key, cached so a re-handshake needs no re-verify.
    seal_pk: [u8; 32],
    sub: String,
    /// The opaque per-run bearer the sandbox must present. Unrelated to the
    /// session token; dies with this process.
    run_bearer: String,
    /// The current sealed session: scoped token + the body-AEAD keys, re-minted
    /// together on demand (expiry / first use).
    session: Mutex<(String, SessionKeys)>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = parse_args()?;
    let mode: AttestMode = args.attest.parse()?;
    let expected = Measurement::from_hex(&args.measurement)?;
    let gateway = resolve_gateway(&args)?;

    // Verify the gateway BEFORE handshaking, so the session binds to the attested
    // enclave and not whatever answered.
    let seal_pk = attested_seal_pk(&gateway, mode, &expected).await?;
    let session = gateway.open_session_sealed(&seal_pk, &args.sub).await?;
    eprintln!("[broker] gateway verified + sealed session established (sub={})", args.sub);

    let run_bearer = random_bearer();
    let state = Arc::new(BrokerState {
        gateway,
        seal_pk,
        sub: args.sub,
        run_bearer: run_bearer.clone(),
        session: Mutex::new(session),
    });

    let app = Router::new()
        .route("/", head(probe_ok))
        .route("/v1/{*rest}", any(proxy))
        .with_state(state);

    let listener = TcpListener::bind(&args.listen).await?;
    // The sandbox needs both of these; print them so a launcher can wire the env.
    println!("ANTHROPIC_BASE_URL=http://{}", args.listen);
    println!("ANTHROPIC_AUTH_TOKEN={run_bearer}");
    eprintln!("[broker] listening on {} (sandbox -> broker -> gateway)", args.listen);
    axum::serve(listener, app).await?;
    Ok(())
}

fn resolve_gateway(args: &ServeArgs) -> Result<Gateway> {
    let Some(handle) = &args.remote else {
        return Ok(Gateway::local(args.gateway_host.clone()));
    };
    let via = args.via.clone().context("--remote requires --via <browser-gw-url>")?;
    Ok(Gateway::remote(handle.clone(), via))
}

fn random_bearer() -> String {
    let mut secret = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut secret);
    hex::encode(secret)
}

/// Fetch + verify the gateway quote and return the attested seal_pk, via the
/// real vendor verifier (`airlock::verify`) against pinned Intel/AMD roots.
async fn attested_seal_pk(
    gateway: &Gateway,
    mode: AttestMode,
    expected: &Measurement,
) -> Result<[u8; 32]> {
    // Roots come from flags alone — resolve BEFORE any network so a bad
    // --snp-product/--snp-vcek fails fast.
    let roots = resolve_roots(mode)?;
    let (quote, _vendor) = gateway.fetch_quote().await?;
    let report_data = airlock::verify::verify_quote(&quote, expected, &roots).await?;
    Ok(attest::split_report_data(&report_data).0)
}

struct AppErr(StatusCode, String);
impl IntoResponse for AppErr {
    fn into_response(self) -> Response {
        (self.0, self.1).into_response()
    }
}

async fn probe_ok() -> StatusCode {
    StatusCode::OK
}

async fn proxy(
    State(st): State<Arc<BrokerState>>,
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
    st: &BrokerState,
    method: Method,
    uri: &axum::http::Uri,
    headers: &HeaderMap,
    body: Bytes,
) -> Result<Response, AppErr> {
    let sandbox_authorized = presented_bearer(headers) == Some(st.run_bearer.as_str());
    if !sandbox_authorized {
        return Err(AppErr(StatusCode::UNAUTHORIZED, "broker run bearer rejected".into()));
    }

    let path_and_query = uri.path_and_query().map(|pq| pq.as_str()).unwrap_or(uri.path());

    // First attempt with the current session; a 401 means the token expired, so
    // re-handshake once and retry. Anything else passes straight through.
    let first = forward(st, &method, path_and_query, headers, &body).await?;
    if first.status() != StatusCode::UNAUTHORIZED {
        return Ok(first);
    }
    eprintln!("[broker] gateway rejected the session token; re-handshaking");
    self_rehandshake(st).await?;
    forward(st, &method, path_and_query, headers, &body).await
}

fn presented_bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
}

/// Forward one request to the gateway with the current sealed session:
/// the body is AEAD'd under the handshake keys (path hosts see ciphertext),
/// the response stream is unsealed head-first and relayed unbuffered
/// (Claude SSE must not be buffered).
async fn forward(
    st: &BrokerState,
    method: &Method,
    path_and_query: &str,
    headers: &HeaderMap,
    body: &Bytes,
) -> Result<Response, AppErr> {
    let (session, keys) = st.session.lock().await.clone();
    let url = st.gateway.url(path_and_query);
    let sealed_body = if body.is_empty() {
        Vec::new()
    } else {
        bodyseal::seal_request(&keys, body)
    };
    let mut rb = st
        .gateway
        .http()
        .request(method.clone(), &url)
        .body(sealed_body.clone());
    rb = st.gateway.route(rb);
    if !body.is_empty() {
        rb = rb.header(bodyseal::SEAL_HEADER, bodyseal::SEAL_V1);
    }
    for (name, value) in headers.iter() {
        let ours = matches!(
            name.as_str(),
            "authorization" | "host" | "content-length" | "accept-encoding" | "x-airlock-body-seal"
        );
        if ours {
            continue;
        }
        rb = rb.header(name, value);
    }

    let mut resp = rb
        .bearer_auth(session)
        .send()
        .await
        .map_err(|e| AppErr(StatusCode::BAD_GATEWAY, format!("gateway: {e}")))?;
    let status = resp.status();
    let sealed_outer = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| ct.starts_with("application/octet-stream"));
    if !sealed_outer {
        // Plaintext from the gateway itself (pre-proxy errors). A plaintext
        // SUCCESS on a sealed session can only be a path-host forgery.
        if status.is_success() {
            return Err(AppErr(
                StatusCode::BAD_GATEWAY,
                "airlock: sealed session received a plaintext success body".into(),
            ));
        }
        let content_type = resp.headers().get("content-type").cloned();
        let mut builder = Response::builder().status(status.as_u16());
        if let Some(v) = content_type {
            builder = builder.header("content-type", v);
        }
        return builder
            .body(Body::from_stream(resp.bytes_stream()))
            .map_err(|e| AppErr(StatusCode::INTERNAL_SERVER_ERROR, format!("build response: {e}")));
    }

    // Unseal head-first: the inner content-type rides the sealed head chunk.
    let mut opener = bodyseal::StreamOpener::new(&keys, &bodyseal::request_binding(&sealed_body));
    let mut pending: Vec<Bytes> = Vec::new();
    let mut inner_ct: Option<String> = None;
    while inner_ct.is_none() {
        match resp.chunk().await {
            Ok(Some(chunk)) => {
                let items = opener
                    .feed(&chunk)
                    .map_err(|e| AppErr(StatusCode::BAD_GATEWAY, format!("airlock: {e}")))?;
                for item in items {
                    match item {
                        OpenedItem::Head(ct) => inner_ct = Some(ct),
                        OpenedItem::Data(data) => pending.push(Bytes::from(data)),
                        OpenedItem::Final => {}
                    }
                }
            }
            Ok(None) => {
                return Err(AppErr(
                    StatusCode::BAD_GATEWAY,
                    "airlock: sealed response ended before its head".into(),
                ));
            }
            Err(e) => return Err(AppErr(StatusCode::BAD_GATEWAY, format!("gateway: {e}"))),
        }
    }
    let finished_at_head = opener.finished();
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, std::io::Error>>(8);
    tokio::spawn(async move {
        for data in pending {
            if tx.send(Ok(data)).await.is_err() {
                return;
            }
        }
        if finished_at_head {
            return;
        }
        loop {
            match resp.chunk().await {
                Ok(Some(chunk)) => {
                    let items = match opener.feed(&chunk) {
                        Ok(items) => items,
                        Err(e) => {
                            let _ = tx.send(Err(std::io::Error::other(e.to_string()))).await;
                            return;
                        }
                    };
                    for item in items {
                        if let OpenedItem::Data(data) = item
                            && tx.send(Ok(Bytes::from(data))).await.is_err()
                        {
                            return;
                        }
                    }
                    if opener.finished() {
                        return;
                    }
                }
                Ok(None) => {
                    // ended WITHOUT the Final marker = authenticated truncation
                    let _ = tx
                        .send(Err(std::io::Error::other("airlock: sealed response truncated")))
                        .await;
                    return;
                }
                Err(e) => {
                    let _ = tx.send(Err(std::io::Error::other(e.to_string()))).await;
                    return;
                }
            }
        }
    });
    let mut builder = Response::builder().status(status.as_u16());
    if let Some(ct) = inner_ct {
        builder = builder.header("content-type", ct);
    }
    builder
        .body(Body::from_stream(tokio_stream::wrappers::ReceiverStream::new(rx)))
        .map_err(|e| AppErr(StatusCode::INTERNAL_SERVER_ERROR, format!("build response: {e}")))
}

/// Re-run the handshake against the already-verified seal key and install the
/// fresh token. The gate is the session mutex: a second caller that arrives
/// after the token is replaced simply uses the new one.
async fn self_rehandshake(st: &BrokerState) -> Result<Response, AppErr> {
    let fresh = st
        .gateway
        .open_session_sealed(&st.seal_pk, &st.sub)
        .await
        .map_err(|e| AppErr(StatusCode::BAD_GATEWAY, format!("re-handshake: {e}")))?;
    *st.session.lock().await = fresh;
    Ok(StatusCode::OK.into_response())
}
