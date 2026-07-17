//! Hermetic mock of the Anthropic OAuth + messages endpoints, so the demos run
//! with zero real credentials and zero ToS exposure. Emits a *valid* Anthropic
//! streaming response so the real `claude` CLI accepts it.
//!
//! `POST /oauth/token`            -> rotates the refresh token, returns access_token = acc-<n>
//! `POST /v1/messages`           -> valid SSE reply; requires Bearer acc-<n> (the access token,
//!                                  NOT the session token — proves the host did the swap)
//! `POST /v1/messages/count_tokens` -> {"input_tokens": N}

use std::sync::{Arc, Mutex};

use anyhow::Result;
use axum::extract::State;
use axum::http::{header::AUTHORIZATION, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::json;
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
        .route("/v1/messages/count_tokens", post(count_tokens))
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
    Json(json!({
        "access_token": format!("acc-{n}"),
        "refresh_token": format!("ref-{n}"), // rotated every call
        "expires_in": 3600,
    }))
}

/// Returns the 401 response iff the caller does NOT present the current access
/// token — i.e. the host failed to swap the session token for the credential.
fn access_denied(st: &MockState, headers: &HeaderMap) -> Option<Response> {
    let want = format!("Bearer acc-{}", *st.n.lock().unwrap());
    let got = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if got == want {
        None
    } else {
        Some(
            (
                StatusCode::UNAUTHORIZED,
                format!("mock upstream: wrong access token (got {got:?}, want {want:?})"),
            )
                .into_response(),
        )
    }
}

async fn count_tokens(State(st): State<Arc<MockState>>, headers: HeaderMap) -> Response {
    if let Some(r) = access_denied(&st, &headers) {
        return r;
    }
    Json(json!({ "input_tokens": 5 })).into_response()
}

async fn messages(State(st): State<Arc<MockState>>, headers: HeaderMap, body: axum::body::Bytes) -> Response {
    if let Some(r) = access_denied(&st, &headers) {
        return r;
    }
    let model = serde_json::from_slice::<serde_json::Value>(&body)
        .ok()
        .and_then(|j| j["model"].as_str().map(str::to_string))
        .unwrap_or_else(|| "claude-sonnet-5".into());
    let reply = "TRUSTLESS-GATEWAY-OK";

    let start = json!({"type":"message_start","message":{"id":"msg_mock01","type":"message","role":"assistant","model":model,"content":[],"stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":5,"output_tokens":1}}});
    let block_start = json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}});
    let delta = json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":reply}});
    let msg_delta = json!({"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"output_tokens":8}});

    let sse = format!(
        "event: message_start\ndata: {start}\n\n\
         event: content_block_start\ndata: {block_start}\n\n\
         event: ping\ndata: {{\"type\":\"ping\"}}\n\n\
         event: content_block_delta\ndata: {delta}\n\n\
         event: content_block_stop\ndata: {{\"type\":\"content_block_stop\",\"index\":0}}\n\n\
         event: message_delta\ndata: {msg_delta}\n\n\
         event: message_stop\ndata: {{\"type\":\"message_stop\"}}\n\n"
    );
    ([("content-type", "text/event-stream")], sse).into_response()
}
