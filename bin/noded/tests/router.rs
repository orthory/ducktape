//! router contract tests over a FAKE node actor: the http layer must forward
//! commands, translate replies, and map failures — without a live host. the
//! real actor is exercised end-to-end by running the binary.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use futures::StreamExt as _;
use futures::channel::mpsc;
use http_body_util::BodyExt as _;
use noded::{BlockSummary, ModuleCategory, ModuleStatus, NodeCommand, NodeHandle, NodeStatus};
use tower::ServiceExt as _;

/// a scripted actor: answers every command the same way, like a module host
/// that always succeeds (or always fails, for the error-path tests).
fn spawn_fake_actor(mut cmds: mpsc::Receiver<NodeCommand>, submit_err: Option<&'static str>) {
    tokio::spawn(async move {
        while let Some(cmd) = cmds.next().await {
            match cmd {
                NodeCommand::Submit {
                    target,
                    payload,
                    origin,
                    reply,
                } => {
                    let result = match submit_err {
                        Some(err) => Err(err.to_string()),
                        None => {
                            // echo enough back to prove the request crossed intact.
                            // the wire casing is the interface crates' serde
                            // convention: snake_case variants and fields.
                            assert_eq!(target, "chat");
                            let value: serde_json::Value =
                                serde_json::from_slice(&payload).expect("payload is json");
                            assert_eq!(value["create_channel"]["channel_id"], "general");
                            // the block reply doubles as the origin probe: echo
                            // the stamped origin so tests assert per-request
                            // identity without a second channel.
                            Ok(BlockSummary {
                                height: 7,
                                app_hash: String::from_utf8_lossy(&origin).into_owned(),
                            })
                        }
                    };
                    let _ = reply.send(result);
                }
                NodeCommand::Query { target, req, reply } => {
                    assert_eq!(target, "tasks");
                    let value: serde_json::Value =
                        serde_json::from_slice(&req).expect("query is json");
                    assert_eq!(value, serde_json::json!("list"));
                    let _ = reply.send(Ok(
                        serde_json::to_vec(&serde_json::json!({ "tasks": [] })).unwrap()
                    ));
                }
                NodeCommand::Status { reply } => {
                    let _ = reply.send(NodeStatus {
                        version: "9.9.9".into(),
                        app_hash: "cd".repeat(32),
                        height: 3,
                        modules: vec![ModuleStatus {
                            id: "chat".into(),
                            root: "ef".repeat(32),
                            category: ModuleCategory::of("chat"),
                        }],
                        public_key: "ab".repeat(32),
                    });
                }
                NodeCommand::Metrics { reply } => {
                    let _ = reply.send(
                        "# HELP ducktape_blocks_total blocks\nducktape_blocks_total 3\n# EOF\n"
                            .to_string(),
                    );
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

    let response = noded::router(handle)
        .oneshot(post(
            "/v1/submit",
            serde_json::json!({
                "target": "chat",
                "payload": { "create_channel": { "channel_id": "general", "name": "General" } },
            }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["height"], 7);
    // the fake actor echoes the stamped origin here — no origin sent, so the
    // daemon default applies
    assert_eq!(body["appHash"], "noded");
}

#[tokio::test]
async fn submit_stamps_the_client_origin() {
    let (handle, cmd_rx, _events) = NodeHandle::channel();
    spawn_fake_actor(cmd_rx, None);

    let response = noded::router(handle)
        .oneshot(post(
            "/v1/submit",
            serde_json::json!({
                "target": "chat",
                "payload": { "create_channel": { "channel_id": "general", "name": "General" } },
                "origin": "jess",
            }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["appHash"], "jess");
}

#[tokio::test]
async fn submit_receipt_op_hash_addresses_the_committed_payload() {
    let (handle, cmd_rx, _events) = NodeHandle::channel();
    spawn_fake_actor(cmd_rx, None);
    let app = noded::router(handle);

    let payload =
        serde_json::json!({ "create_channel": { "channel_id": "general", "name": "General" } });
    let response = app
        .clone()
        .oneshot(post(
            "/v1/submit",
            serde_json::json!({ "target": "chat", "payload": payload }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    let op_hash = body["opHash"].as_str().expect("receipt carries opHash");
    assert_eq!(op_hash.len(), 64);
    assert!(
        op_hash
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    );

    // the hash is ADDRESSABLE, not just informational: the blob lane serves the
    // committed op bytes back under it.
    let fetched = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/files/blob/{op_hash}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fetched.status(), StatusCode::OK);
    let bytes = fetched.into_body().collect().await.unwrap().to_bytes();
    let round_trip: serde_json::Value =
        serde_json::from_slice(&bytes).expect("blob is the op json");
    assert_eq!(
        round_trip,
        serde_json::json!({ "create_channel": { "channel_id": "general", "name": "General" } })
    );
}

#[tokio::test]
async fn submit_maps_a_module_error_to_bad_request() {
    let (handle, cmd_rx, _events) = NodeHandle::channel();
    spawn_fake_actor(cmd_rx, Some("module error: channel already exists"));

    let response = noded::router(handle)
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

    let response = noded::router(handle)
        .oneshot(post(
            "/v1/query",
            serde_json::json!({ "target": "tasks", "query": "list" }),
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

    let response = noded::router(handle)
        .oneshot(
            Request::builder()
                .uri("/v1/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["version"], "9.9.9");
    assert_eq!(body["appHash"], "cd".repeat(32));
    assert_eq!(body["height"], 3);
    assert_eq!(body["modules"][0]["id"], "chat");
    assert_eq!(body["modules"][0]["root"], "ef".repeat(32));
    // the catalog category rides on the wire as a lowercase string.
    assert_eq!(body["modules"][0]["category"], "workspace");
}

#[tokio::test]
async fn metrics_forwards_the_encoded_registry_as_openmetrics_text() {
    let (handle, cmd_rx, _events) = NodeHandle::channel();
    spawn_fake_actor(cmd_rx, None);

    let response = noded::router(handle)
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(
        content_type.contains("openmetrics-text"),
        "scrape content type, got {content_type:?}",
    );
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8(bytes.to_vec()).expect("metrics body is utf-8");
    assert!(
        text.contains("ducktape_blocks_total 3"),
        "actor body passed through: {text:?}"
    );
}

#[tokio::test]
async fn shutdown_acknowledges_then_signals() {
    let (handle, cmd_rx, _events) = NodeHandle::channel();
    spawn_fake_actor(cmd_rx, None);
    let signal = handle.clone();

    let response = noded::router(handle)
        .oneshot(post("/v1/shutdown", serde_json::json!({})))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["ok"], true);
    // the permit is stored, so awaiting after the request must resolve —
    // guarded by a timeout so a broken signal fails instead of hanging.
    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        signal.shutdown_requested(),
    )
    .await
    .expect("shutdown signal fired");
}

#[tokio::test]
async fn a_dead_actor_maps_to_service_unavailable() {
    let (handle, cmd_rx, _events) = NodeHandle::channel();
    drop(cmd_rx); // no actor at all

    let response = noded::router(handle)
        .oneshot(post(
            "/v1/submit",
            serde_json::json!({ "target": "chat", "payload": {} }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

/// a GET carrying the RFC 6455 upgrade headers axum's `WebSocketUpgrade`
/// extractor checks. NOTE the oneshot transport can never actually upgrade:
/// hyper's `OnUpgrade` state only exists on a real served connection, so the
/// extractor stops these requests with 426 BEFORE the handler body runs. that
/// still separates "route is wired" (426) from "route is gone" (404); the
/// handler's own no-hub refusal (503 + a body that says why) is exercised
/// against the real spawned binary in `daemon_e2e.rs`.
fn ws_upgrade(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .header(header::CONNECTION, "upgrade")
        .header(header::UPGRADE, "websocket")
        .header(header::SEC_WEBSOCKET_VERSION, "13")
        .header(header::SEC_WEBSOCKET_KEY, "dGhlIHNhbXBsZSBub25jZQ==")
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn call_ws_route_is_wired() {
    let (handle, cmd_rx, _events) = NodeHandle::channel();
    spawn_fake_actor(cmd_rx, None);

    let response = noded::router(handle)
        .oneshot(ws_upgrade("/v1/call/ws?channel=general"))
        .await
        .unwrap();

    // 426 = axum's ConnectionNotUpgradable: the route matched and websocket
    // extraction ran — anything but 404 proves the route exists.
    assert_eq!(response.status(), StatusCode::UPGRADE_REQUIRED);
}

#[tokio::test]
async fn the_old_voice_ws_route_is_gone() {
    // app and node ship lockstep: `/v1/voice/ws` was replaced by `/v1/call/ws`,
    // so the old path is simply unrouted now — a 404, not a refusal.
    let (handle, cmd_rx, _events) = NodeHandle::channel();
    spawn_fake_actor(cmd_rx, None);

    let response = noded::router(handle)
        .oneshot(ws_upgrade("/v1/voice/ws?channel=general"))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ---- duckfs read/probe surface: refs / diff / has-chunks --------------------

use std::collections::BTreeMap;

use files::{DiffEntry, DiffKind, FilesQuery, FilesReply, RefsInfo};

/// a scripted files actor: decodes each `FilesQuery` and answers the matching
/// canned `FilesReply`, or fails a submit with `submit_err` (the 400-envelope
/// contract test). proves the router forwards `target = "files"` and translates
/// the typed reply to json without a live module.
fn spawn_files_actor(
    mut cmds: futures::channel::mpsc::Receiver<NodeCommand>,
    submit_err: Option<&'static str>,
) {
    tokio::spawn(async move {
        while let Some(cmd) = cmds.next().await {
            match cmd {
                NodeCommand::Query { target, req, reply } => {
                    assert_eq!(target, "files");
                    let q = files::decode_query(&req).expect("files query decodes");
                    let bytes = match q {
                        FilesQuery::Refs {} => files::encode_reply(&FilesReply::Refs(RefsInfo {
                            head: Some("ab".repeat(32)),
                            pins: BTreeMap::new(),
                            window_len: 4,
                        })),
                        FilesQuery::Diff { .. } => {
                            files::encode_reply(&FilesReply::Diff(vec![DiffEntry {
                                path: "/a".into(),
                                kind: DiffKind::Modified,
                            }]))
                        }
                        FilesQuery::HasChunks { ids } => {
                            // present iff the id starts with "aa" — proves the reply
                            // order maps back to the request order over the wire.
                            let present = ids.iter().map(|id| id.starts_with("aa")).collect();
                            files::encode_reply(&FilesReply::HasChunks { present })
                        }
                        other => panic!("unexpected files query: {other:?}"),
                    };
                    let _ = reply.send(Ok(bytes));
                }
                NodeCommand::Submit { target, reply, .. } => {
                    assert_eq!(target, "files");
                    let result = match submit_err {
                        Some(err) => Err(err.to_string()),
                        None => Ok(BlockSummary {
                            height: 9,
                            app_hash: "ab".repeat(32),
                        }),
                    };
                    let _ = reply.send(result);
                }
                _ => {}
            }
        }
    });
}

fn get(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn files_refs_route_returns_head() {
    let (handle, cmd_rx, _events) = NodeHandle::channel();
    spawn_files_actor(cmd_rx, None);

    let response = noded::router(handle)
        .oneshot(get("/v1/files/refs"))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["head"], "ab".repeat(32));
    assert_eq!(body["window_len"], 4);
}

#[tokio::test]
async fn files_diff_route_returns_entries() {
    let (handle, cmd_rx, _events) = NodeHandle::channel();
    spawn_files_actor(cmd_rx, None);

    let response = noded::router(handle)
        .oneshot(get("/v1/files/diff?from=aa&to=bb&prefix=/"))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["entries"][0]["path"], "/a");
    assert_eq!(body["entries"][0]["kind"], "modified");
}

#[tokio::test]
async fn files_has_chunks_route_preserves_request_order() {
    let (handle, cmd_rx, _events) = NodeHandle::channel();
    spawn_files_actor(cmd_rx, None);

    let present = "aa".repeat(32);
    let absent = "bb".repeat(32);
    let response = noded::router(handle)
        .oneshot(get(&format!("/v1/files/has-chunks?ids={present},{absent}")))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["present"], serde_json::json!([true, false]));
}

#[tokio::test]
async fn files_module_rejection_is_a_verbatim_400_envelope() {
    // the engine's conflict taxonomy keys on the module error string arriving
    // untouched inside a 400 {"error": "files: ..."} — pin the envelope here.
    let (handle, cmd_rx, _events) = NodeHandle::channel();
    spawn_files_actor(cmd_rx, Some("files: conflict: /x changed since base"));

    let response = noded::router(handle)
        .oneshot(post(
            "/v1/files/commit",
            serde_json::json!({ "message": "m", "changes": [] }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["error"], "files: conflict: /x changed since base");
}
