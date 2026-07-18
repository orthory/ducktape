//! End-to-end: the whole custody path in one process, over real HTTP —
//! attest → seal credential → handshake → proxied call → credential swap →
//! reply. Run with `cargo test -p airlock --features server,client`.
#![cfg(all(feature = "server", feature = "client"))]

use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::{header::AUTHORIZATION, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::json;
use tokio::net::TcpListener;

use airlock::attest::{self, Measurement};
use airlock::client::Gateway;
use airlock::server::{self, GatewayConfig};

/// 48-byte measurement (all 0x11) shared by the gateway and the verifying client.
fn measurement_hex() -> String {
    "11".repeat(attest::MRTD_LEN)
}

/// A mock Anthropic upstream. `/oauth/token` mints `acc-1`; `/v1/messages`
/// accepts ONLY `Bearer acc-1` — so a 200 proves the gateway swapped the
/// session token for the real access token.
#[derive(Default)]
struct MockUpstream {
    n: Mutex<u64>,
}

async fn oauth(State(st): State<Arc<MockUpstream>>) -> Json<serde_json::Value> {
    let mut n = st.n.lock().unwrap();
    *n += 1;
    Json(json!({ "access_token": format!("acc-{n}"), "refresh_token": format!("ref-{n}"), "expires_in": 3600 }))
}

async fn messages(State(st): State<Arc<MockUpstream>>, headers: HeaderMap) -> Response {
    let want = format!("Bearer acc-{}", *st.n.lock().unwrap());
    let got = headers.get(AUTHORIZATION).and_then(|v| v.to_str().ok()).unwrap_or("");
    if got != want {
        return (StatusCode::UNAUTHORIZED, format!("want {want:?} got {got:?}")).into_response();
    }
    let sse = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"AIRLOCK-OK\"}}\n\n\
               event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
    ([("content-type", "text/event-stream")], sse).into_response()
}

/// Bind a listener on an ephemeral port and serve `app`; returns the base URL.
/// `bind` makes the socket accept into the backlog immediately, so a client may
/// connect right away — no readiness sleep needed.
async fn spawn(app: Router) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

async fn boot_gateway(upstream: &str) -> String {
    let (app, vendor) = server::build(GatewayConfig {
        attest: "mock".into(),
        measurement: Some(measurement_hex()),
        anthropic_base: upstream.into(),
        oauth_token_url: format!("{upstream}/oauth/token"),
        oauth_client_id: "test-client".into(),
        session_ttl_secs: 3600,
        max_requests: 100,
    })
    .unwrap();
    assert_eq!(vendor, "mock");
    spawn(app).await
}

async fn boot_upstream() -> String {
    let app = Router::new()
        .route("/oauth/token", post(oauth))
        .route("/v1/messages", post(messages))
        .with_state(Arc::new(MockUpstream::default()));
    spawn(app).await
}

/// The verified `seal_pk` the client trusts only because it checked the quote.
async fn attested_seal_pk(gw: &Gateway) -> [u8; 32] {
    let (quote, vendor) = gw.fetch_quote().await.unwrap();
    assert_eq!(vendor, "mock");
    let expected = Measurement::from_hex(&measurement_hex()).unwrap();
    let rd = attest::mock_verify(&quote, &expected).unwrap();
    attest::split_report_data(&rd).0
}

#[tokio::test]
async fn full_custody_path_swaps_session_token_for_the_credential() {
    let upstream = boot_upstream().await;
    let gateway_url = boot_gateway(&upstream).await;
    let gw = Gateway::local(gateway_url.clone());

    // Credential Provider: verify the quote, then seal + upload the refresh token.
    let seal_pk = attested_seal_pk(&gw).await;
    gw.upload_sealed_credential(&seal_pk, &airlock::wire::CredentialPayload::Refresh {
        refresh_token: "ref-seed".into(),
    })
    .await
    .unwrap();

    // Computation Provider: handshake for a scoped token, then a proxied call.
    let token = gw.open_session(&seal_pk, "test-sub").await.unwrap();
    let resp = reqwest::Client::new()
        .post(format!("{gateway_url}/v1/messages"))
        .bearer_auth(&token)
        .header("content-type", "application/json")
        .body(r#"{"model":"claude-sonnet-5","max_tokens":16,"stream":true,"messages":[{"role":"user","content":"hi"}]}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let body = resp.text().await.unwrap();
    assert!(body.contains("AIRLOCK-OK"), "reply should stream back through the gateway: {body}");
}

#[tokio::test]
async fn proxy_rejects_a_request_without_a_valid_session_token() {
    let upstream = boot_upstream().await;
    let gateway_url = boot_gateway(&upstream).await;

    // A bare bearer that is not a gateway-issued session token is refused before
    // any credential is spent.
    let resp = reqwest::Client::new()
        .post(format!("{gateway_url}/v1/messages"))
        .bearer_auth("not-a-real-session-token")
        .body("{}")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), reqwest::StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_forged_gateway_cannot_mint_a_token_the_client_opens() {
    // If the client handshakes against a DIFFERENT enclave's seal_pk than the one
    // that answered, open_token fails — the session binds to the attested key.
    let upstream = boot_upstream().await;
    let gateway_url = boot_gateway(&upstream).await;
    let gw = Gateway::local(gateway_url);

    let wrong_seal_pk = [0x42u8; 32]; // not the gateway's attested key
    let err = gw.open_session(&wrong_seal_pk, "test-sub").await;
    assert!(err.is_err(), "a token derived against the wrong seal_pk must not open");
}

#[tokio::test]
async fn static_bearer_credential_is_used_without_any_oauth_refresh() {
    use std::sync::atomic::{AtomicU64, Ordering};
    // An upstream that FAILS if /oauth/token is ever hit, and accepts only the
    // exact static bearer on /v1/messages — proving a sealed Bearer is used as-is,
    // never refreshed (so a live subscription's token chain is not rotated).
    let oauth_hits = Arc::new(AtomicU64::new(0));
    let oh = oauth_hits.clone();
    let app = Router::new()
        .route(
            "/oauth/token",
            post(move || {
                let oh = oh.clone();
                async move {
                    oh.fetch_add(1, Ordering::SeqCst);
                    (StatusCode::INTERNAL_SERVER_ERROR, "oauth must not be called for a static bearer")
                        .into_response()
                }
            }),
        )
        .route(
            "/v1/messages",
            post(|headers: HeaderMap| async move {
                let got = headers.get(AUTHORIZATION).and_then(|v| v.to_str().ok()).unwrap_or("");
                if got != "Bearer static-access-xyz" {
                    return (StatusCode::UNAUTHORIZED, format!("got {got:?}")).into_response();
                }
                ([("content-type", "text/event-stream")], "data: AIRLOCK-OK\n\n").into_response()
            }),
        );
    let upstream = spawn(app).await;
    let gateway_url = boot_gateway(&upstream).await;
    let gw = Gateway::local(gateway_url.clone());

    let seal_pk = attested_seal_pk(&gw).await;
    gw.upload_sealed_credential(
        &seal_pk,
        &airlock::wire::CredentialPayload::Bearer { access_token: "static-access-xyz".into() },
    )
    .await
    .unwrap();

    let token = gw.open_session(&seal_pk, "test-sub").await.unwrap();
    let resp = reqwest::Client::new()
        .post(format!("{gateway_url}/v1/messages"))
        .bearer_auth(&token)
        .body("{}")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let body = resp.text().await.unwrap();
    assert!(body.contains("AIRLOCK-OK"), "static bearer should reach upstream: {body}");
    assert_eq!(
        oauth_hits.load(Ordering::SeqCst),
        0,
        "a static bearer must NOT trigger an OAuth refresh (no rotation)"
    );
}
