//! Hermetic mock of the Anthropic OAuth + messages endpoints, so `demo.sh`
//! runs with zero real credentials and zero ToS exposure.
//!
//! `POST /oauth/token`  -> rotates the refresh token every call, returns
//!                         `access_token = acc-<n>`.
//! `POST /v1/messages`  -> requires `Authorization: Bearer acc-<n>` (the
//!                         current access token, NOT the session token). This is
//!                         the load-bearing assertion: if the host failed to
//!                         swap session-token -> access-token, this 401s.

use std::sync::{Arc, Mutex};

use anyhow::Result;
use axum::extract::State;
use axum::http::{header::AUTHORIZATION, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use tokio::net::TcpListener;

#[derive(Default)]
struct MockState {
    /// Count of /oauth/token calls; current access token is `acc-<n>`.
    n: Mutex<u64>,
}

pub async fn run(listen: &str) -> Result<()> {
    let st = Arc::new(MockState::default());
    let app = Router::new()
        .route("/oauth/token", post(oauth))
        .route("/v1/messages", post(messages))
        .with_state(st);
    let listener = TcpListener::bind(listen).await?;
    eprintln!("[mock-upstream] listening on {listen}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn oauth(State(st): State<Arc<MockState>>) -> Json<serde_json::Value> {
    let n = {
        let mut n = st.n.lock().unwrap();
        *n += 1;
        *n
    };
    Json(serde_json::json!({
        "access_token": format!("acc-{n}"),
        "refresh_token": format!("ref-{n}"), // rotated every call
        "expires_in": 3600,
    }))
}

async fn messages(State(st): State<Arc<MockState>>, headers: HeaderMap, _body: axum::body::Bytes) -> Response {
    let want = format!("Bearer acc-{}", *st.n.lock().unwrap());
    let got = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if got != want {
        return (
            StatusCode::UNAUTHORIZED,
            format!("mock upstream: wrong access token (got {got:?}, want {want:?})"),
        )
            .into_response();
    }
    let sse = "event: message_start\ndata: {\"type\":\"message_start\"}\n\n\
               event: content_block_delta\n\
               data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"MOCK-REPLY-OK\"}}\n\n\
               event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
    ([("content-type", "text/event-stream")], sse).into_response()
}
