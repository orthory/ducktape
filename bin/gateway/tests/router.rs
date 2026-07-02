//! router contract tests over a FAKE node actor: the http layer must forward
//! commands, translate replies, and map failures — without a live host. the
//! real actor is exercised end-to-end by running the binary.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use futures::StreamExt as _;
use futures::channel::mpsc;
use gateway::{BlockSummary, ModuleStatus, NodeCommand, NodeHandle, NodeStatus};
use http_body_util::BodyExt as _;
use tower::ServiceExt as _;

/// a scripted actor: answers every command the same way, like a module host
/// that always succeeds (or always fails, for the error-path tests).
fn spawn_fake_actor(mut cmds: mpsc::Receiver<NodeCommand>, submit_err: Option<&'static str>) {
    tokio::spawn(async move {
        while let Some(cmd) = cmds.next().await {
            match cmd {
                NodeCommand::Submit { target, payload, reply } => {
                    let result = match submit_err {
                        Some(err) => Err(err.to_string()),
                        None => {
                            // echo enough back to prove the request crossed intact.
                            // the wire casing is the interface crates' serde
                            // default: PascalCase variants, snake_case fields.
                            assert_eq!(target, "chat");
                            let value: serde_json::Value =
                                serde_json::from_slice(&payload).expect("payload is json");
                            assert_eq!(value["CreateChannel"]["channel_id"], "general");
                            Ok(BlockSummary {
                                height: 7,
                                app_hash: "ab".repeat(32),
                            })
                        }
                    };
                    let _ = reply.send(result);
                }
                NodeCommand::Query { target, req, reply } => {
                    assert_eq!(target, "tasks");
                    let value: serde_json::Value =
                        serde_json::from_slice(&req).expect("query is json");
                    assert_eq!(value, serde_json::json!("List"));
                    let _ = reply.send(Ok(
                        serde_json::to_vec(&serde_json::json!({ "tasks": [] })).unwrap()
                    ));
                }
                NodeCommand::Status { reply } => {
                    let _ = reply.send(NodeStatus {
                        app_hash: "cd".repeat(32),
                        height: 3,
                        modules: vec![ModuleStatus {
                            id: "chat".into(),
                            root: "ef".repeat(32),
                        }],
                    });
                }
            }
        }
    });
}

fn post(uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

async fn body_json(response: axum::response::Response) -> serde_json::Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).expect("response body is json")
}

#[tokio::test]
async fn submit_forwards_the_payload_and_returns_the_block() {
    let (handle, cmd_rx, _events) = NodeHandle::channel();
    spawn_fake_actor(cmd_rx, None);

    let response = gateway::router(handle)
        .oneshot(post(
            "/v1/submit",
            serde_json::json!({
                "target": "chat",
                "payload": { "CreateChannel": { "channel_id": "general", "name": "General" } },
            }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["height"], 7);
    assert_eq!(body["appHash"], "ab".repeat(32));
}

#[tokio::test]
async fn submit_maps_a_module_error_to_bad_request() {
    let (handle, cmd_rx, _events) = NodeHandle::channel();
    spawn_fake_actor(cmd_rx, Some("module error: channel already exists"));

    let response = gateway::router(handle)
        .oneshot(post(
            "/v1/submit",
            serde_json::json!({ "target": "chat", "payload": {} }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["error"], "module error: channel already exists");
}

#[tokio::test]
async fn query_returns_the_decoded_module_reply() {
    let (handle, cmd_rx, _events) = NodeHandle::channel();
    spawn_fake_actor(cmd_rx, None);

    let response = gateway::router(handle)
        .oneshot(post(
            "/v1/query",
            serde_json::json!({ "target": "tasks", "query": "List" }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body, serde_json::json!({ "tasks": [] }));
}

#[tokio::test]
async fn status_reports_app_hash_height_and_module_roots() {
    let (handle, cmd_rx, _events) = NodeHandle::channel();
    spawn_fake_actor(cmd_rx, None);

    let response = gateway::router(handle)
        .oneshot(Request::builder().uri("/v1/status").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["appHash"], "cd".repeat(32));
    assert_eq!(body["height"], 3);
    assert_eq!(body["modules"][0]["id"], "chat");
    assert_eq!(body["modules"][0]["root"], "ef".repeat(32));
}

#[tokio::test]
async fn a_dead_actor_maps_to_service_unavailable() {
    let (handle, cmd_rx, _events) = NodeHandle::channel();
    drop(cmd_rx); // no actor at all

    let response = gateway::router(handle)
        .oneshot(post(
            "/v1/submit",
            serde_json::json!({ "target": "chat", "payload": {} }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}
