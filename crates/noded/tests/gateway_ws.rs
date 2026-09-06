//! End-to-end test of the WebSocket side door on the dedicated gateway
//! listener: mint a token, open a browser WebSocket through the door, and watch
//! a message round-trip to a fake gateway upgrade lane and back.

use axum::body::Body;
use futures::{SinkExt as _, StreamExt as _};
use http_body_util::BodyExt as _;
use noded::{GatewayJob, NodeCommand, NodeHandle, gateway_browser_router};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tower::ServiceExt as _;

fn route_record(publisher: [u8; 32]) -> gateway::RouteRecord {
    gateway::RouteRecord {
        statement: gateway::RouteStatement {
            chain_id: "test".into(),
            account_id: 1,
            name: gateway::RouteName::named("api"),
            publisher_node: publisher.to_vec(),
            revision: 3,
            route: Some(gateway::RouteDefinition {
                target: gateway::RouteTarget::LoopbackHttp,
                policy: gateway::RoutePolicy {
                    audience: gateway::RouteAudience::Network,
                    methods: vec![gateway::RouteMethod::Get],
                    max_request_bytes: 0,
                    max_response_bytes: 4096,
                    allow_authorization: false,
                    allow_upgrade: true,
                },
            }),
        },
        authorization: gateway::MemberAuthorization {
            signer: vec![9; 32],
            signature: vec![8; 64],
        },
    }
}

/// A handle wired with a fake gateway actor (resolves `api.alice.duck` to
/// account 1's `api` route) and a fake upgrade lane that echoes browser
/// messages back, ready to mint and serve real WS-door requests against.
fn test_handle() -> NodeHandle {
    let (handle, mut cmd_rx, _events) = NodeHandle::channel();

    // Fake actor: the merged gateway module answers both the handle authority
    // Resolve and the route Get, dispatched by query variant.
    tokio::spawn(async move {
        while let Some(command) = cmd_rx.next().await {
            if let NodeCommand::Query { target, req, reply } = command {
                assert_eq!(target, "gateway");
                let bytes = match gateway::decode_query(&req).unwrap() {
                    gateway::GatewayQuery::Resolve { .. } => {
                        gateway::encode_reply(&gateway::GatewayReply::Resolved(Some(
                            gateway::ResolvedAccount { account_id: 1 },
                        )))
                    }
                    gateway::GatewayQuery::Get { .. } => gateway::encode_reply(
                        &gateway::GatewayReply::Route(Box::new(Some(route_record([2u8; 32])))),
                    ),
                    other => panic!("unexpected query {other:?}"),
                };
                let _ = reply.send(Ok(bytes));
            }
        }
    });

    // Fake gateway lane: on an Upgrade job, echo browser messages back.
    let (lane_tx, mut lane_rx) = mpsc::channel::<GatewayJob>(8);
    tokio::spawn(async move {
        while let Some(job) = lane_rx.recv().await {
            if let GatewayJob::Upgrade {
                to_browser,
                mut from_browser,
                ..
            } = job
            {
                tokio::spawn(async move {
                    while let Some(message) = from_browser.recv().await {
                        if to_browser.send(message).await.is_err() {
                            break;
                        }
                    }
                });
            }
        }
    });

    handle
        .with_gateway(lane_tx)
        .with_browser_gateway("127.0.0.1:0".parse().unwrap())
}

#[tokio::test]
async fn ws_side_door_mints_consumes_and_bridges_to_the_lane() {
    let handle = test_handle();

    // Mint a token through the real endpoint (shares the handle's token store).
    let origin = "duck://api.alice.duck";
    let mint = gateway_browser_router(handle.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/.duck/ws-token")
                .header("content-type", "application/json")
                .header("x-duck-authority", "api.alice.duck")
                .header("origin", origin)
                .body(Body::from(
                    serde_json::json!({
                        "authority": "api.alice.duck",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(mint.status(), 200);
    let body = mint.into_body().collect().await.unwrap().to_bytes();
    let token = serde_json::from_slice::<serde_json::Value>(&body).unwrap()["token"]
        .as_str()
        .unwrap()
        .to_string();

    // Serve the gateway router on a real listener and open the WS door.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, gateway_browser_router(handle))
            .await
            .unwrap();
    });

    let mut request = format!("ws://127.0.0.1:{port}/.duck/ws/{token}")
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("origin", origin.parse().unwrap());
    let (mut socket, _response) = tokio_tungstenite::connect_async(request).await.unwrap();

    socket.send(Message::text("ping")).await.unwrap();
    let echoed = socket.next().await.unwrap().unwrap();
    assert_eq!(echoed.into_text().unwrap().as_str(), "ping");

    // A second connection with the same (now-consumed) token is refused.
    let mut replay = format!("ws://127.0.0.1:{port}/.duck/ws/{token}")
        .into_client_request()
        .unwrap();
    replay
        .headers_mut()
        .insert("origin", origin.parse().unwrap());
    assert!(tokio_tungstenite::connect_async(replay).await.is_err());
}

#[tokio::test]
async fn mint_without_authority_header_is_refused() {
    let handle = test_handle();
    let mint = gateway_browser_router(handle)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/.duck/ws-token")
                .header("content-type", "application/json")
                .header("origin", "duck://api.alice.duck")
                .body(Body::from(
                    serde_json::json!({ "authority": "api.alice.duck" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(mint.status(), 421);
}

#[tokio::test]
async fn mint_with_mismatched_body_authority_is_refused() {
    let handle = test_handle();
    let mint = gateway_browser_router(handle)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/.duck/ws-token")
                .header("content-type", "application/json")
                .header("x-duck-authority", "api.alice.duck")
                .header("origin", "duck://api.alice.duck")
                .body(Body::from(
                    serde_json::json!({ "authority": "other.mallory.duck" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(mint.status(), 403);
}

#[tokio::test]
async fn handshake_with_null_origin_against_a_real_bound_token_is_refused() {
    let handle = test_handle();

    let mint = gateway_browser_router(handle.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/.duck/ws-token")
                .header("content-type", "application/json")
                .header("x-duck-authority", "api.alice.duck")
                .header("origin", "duck://api.alice.duck")
                .body(Body::from(
                    serde_json::json!({ "authority": "api.alice.duck" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(mint.status(), 200);
    let body = mint.into_body().collect().await.unwrap().to_bytes();
    let token = serde_json::from_slice::<serde_json::Value>(&body).unwrap()["token"]
        .as_str()
        .unwrap()
        .to_string();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, gateway_browser_router(handle))
            .await
            .unwrap();
    });

    let mut request = format!("ws://127.0.0.1:{port}/.duck/ws/{token}")
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("origin", "null".parse().unwrap());
    assert!(tokio_tungstenite::connect_async(request).await.is_err());
}
