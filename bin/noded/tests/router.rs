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

fn gateway_route() -> gateway::RouteRecord {
    gateway::RouteRecord {
        statement: gateway::RouteStatement {
            version: gateway::ROUTE_FORMAT_VERSION,
            chain_id: "test".into(),
            account_id: vec![1],
            name: gateway::RouteName::named("app"),
            publisher_node: vec![2; 32],
            revision: 7,
            route: Some(gateway::RouteDefinition {
                target: gateway::RouteTarget::LoopbackHttp,
                policy: gateway::RoutePolicy {
                    audience: gateway::RouteAudience::Network,
                    methods: vec![gateway::RouteMethod::Get, gateway::RouteMethod::Post],
                    max_request_bytes: 1024,
                    max_response_bytes: 4096,
                    allow_authorization: false,
                    allow_upgrade: false,
                },
            }),
        },
        authorization: gateway::MemberAuthorization {
            signer: vec![3; 32],
            signature: vec![4; 64],
        },
    }
}

fn spawn_gateway_actor(mut cmds: mpsc::Receiver<NodeCommand>, replies: usize) {
    tokio::spawn(async move {
        for _ in 0..replies {
            let NodeCommand::Query { target, req, reply } = cmds.next().await.unwrap() else {
                panic!("gateway only queries route state");
            };
            assert_eq!(target, "gateway");
            assert_eq!(
                gateway::decode_query(&req).unwrap(),
                gateway::GatewayQuery::Get {
                    account_id: vec![1],
                    name: gateway::RouteName::named("app"),
                }
            );
            let _ = reply.send(Ok(gateway::encode_reply(&gateway::GatewayReply::Route(
                Box::new(Some(gateway_route())),
            ))));
        }
    });
}

#[tokio::test]
async fn gateway_proxy_resolves_the_signed_route_and_forwards_post_body() {
    use base64::Engine as _;
    let (handle, cmds, _events) = NodeHandle::channel();
    spawn_gateway_actor(cmds, 1);
    let (lane, mut jobs) = tokio::sync::mpsc::channel::<noded::GatewayJob>(1);
    tokio::spawn(async move {
        let job = jobs.recv().await.expect("one gateway job");
        let noded::GatewayJob::Http { publisher_node, head, body, reply, .. } = job else {
            panic!("expected an http gateway job");
        };
        assert_eq!(publisher_node, [2; 32]);
        assert_eq!(head.name, gateway::RouteName::named("app"));
        assert_eq!(head.method, gateway::RouteMethod::Post);
        assert_eq!(head.path_and_query, "/api/items");
        assert_eq!(body, br#"{"name":"duck"}"#);
        let _ = reply.send(Ok(noded::GatewayResponse {
            head: gateway::ProxyResponseHead {
                status: 201,
                headers: vec![gateway::ProxyHeader {
                    name: "content-type".into(),
                    value: "application/json".into(),
                }],
            },
            body: br#"{"ok":true}"#.to_vec(),
        }));
    });
    let request_body = br#"{"name":"duck"}"#;
    let mut request = post(
        "/v1/gateway/proxy",
        serde_json::json!({
            "head": {
                "account_id": [1],
                "name": { "label": "app" },
                "revision": 7,
                "method": "post",
                "path_and_query": "/api/items",
                "headers": [{ "name": "content-type", "value": "application/json" }],
                "body_len": request_body.len(),
            },
            "bodyB64": base64::engine::general_purpose::STANDARD.encode(request_body),
        }),
    );
    request
        .headers_mut()
        .insert(header::ORIGIN, "tauri://localhost".parse().unwrap());
    let response = noded::router(handle.with_gateway(lane))
        .oneshot(request)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["head"]["status"], 201);
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(body["bodyB64"].as_str().unwrap())
            .unwrap(),
        br#"{"ok":true}"#
    );
}

#[tokio::test]
async fn gateway_api_rejects_untrusted_browser_origins_before_network_work() {
    let (handle, _cmds, _events) = NodeHandle::channel();
    for origin in [
        "https://evil.example",
        "http://0123456789abcdef0123456789abcdef.localhost:49152",
    ] {
        let mut request = post(
            "/v1/gateway/session",
            serde_json::json!({
                "accountId": [1],
                "name": { "label": "app" },
                "revision": 7,
            }),
        );
        request
            .headers_mut()
            .insert(header::ORIGIN, origin.parse().unwrap());
        request
            .headers_mut()
            .insert("sec-fetch-site", "cross-site".parse().unwrap());
        let response = noded::router(handle.clone())
            .oneshot(request)
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::FORBIDDEN,
            "accepted {origin}"
        );
    }

    let mut originless_browser = post(
        "/v1/gateway/session",
        serde_json::json!({
            "accountId": [1],
            "name": { "label": "app" },
            "revision": 7,
        }),
    );
    originless_browser
        .headers_mut()
        .insert("sec-fetch-site", "cross-site".parse().unwrap());
    let response = noded::router(handle)
        .oneshot(originless_browser)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn gateway_browser_session_is_route_scoped_and_cross_origin_cookie_safe() {
    let (handle, cmds, _events) = NodeHandle::channel();
    spawn_gateway_actor(cmds, 2);
    let (lane, mut jobs) = tokio::sync::mpsc::channel::<noded::GatewayJob>(1);
    let handle = handle
        .with_gateway(lane)
        .with_browser_gateway("127.0.0.1:49152".parse().unwrap());
    let response = noded::router(handle.clone())
        .oneshot(post(
            "/v1/gateway/session",
            serde_json::json!({
                "accountId": [1],
                "name": { "label": "app" },
                "revision": 7,
            }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    let host = body["url"]
        .as_str()
        .unwrap()
        .strip_prefix("http://")
        .and_then(|value| value.strip_suffix('/'))
        .unwrap()
        .to_string();
    assert!(host.ends_with(".localhost:49152"));
    assert_eq!(host.len(), 32 + ".localhost:49152".len());

    tokio::spawn(async move {
        let job = jobs.recv().await.unwrap();
        let noded::GatewayJob::Http { head, body, reply, .. } = job else {
            panic!("expected an http gateway job");
        };
        assert_eq!(head.method, gateway::RouteMethod::Post);
        assert_eq!(head.path_and_query, "/api");
        assert_eq!(body, b"payload");
        let _ = reply.send(Ok(noded::GatewayResponse {
            head: gateway::ProxyResponseHead {
                status: 201,
                headers: vec![gateway::ProxyHeader {
                    name: "content-type".into(),
                    value: "application/json".into(),
                }],
            },
            body: br#"{"ok":true}"#.to_vec(),
        }));
    });
    let origin = format!("http://{host}");
    let response = noded::gateway_browser_router(handle.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api")
                .header(header::HOST, &host)
                .header(header::ORIGIN, &origin)
                .header(header::CONTENT_TYPE, "text/plain")
                .body(Body::from("payload"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(response.headers()["access-control-allow-origin"], origin);
    let csp = response.headers()[header::CONTENT_SECURITY_POLICY]
        .to_str()
        .unwrap();
    assert!(csp.contains(&format!("connect-src http://{host}")));
    assert!(csp.contains("worker-src 'none'"));
    assert!(csp.contains("frame-ancestors 'none'"));
    assert!(csp.contains("sandbox allow-scripts allow-same-origin allow-forms"));
    assert!(csp.contains("webrtc 'block'"));

    let response = noded::gateway_browser_router(handle.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api")
                .header(header::HOST, &host)
                .header(header::ORIGIN, "http://evil.localhost:49152")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    // (v2: ambient Cookie now flows end to end; the v1 rejection was removed.)

    let response = noded::gateway_browser_router(handle)
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/")
                .header(header::HOST, "guessed.localhost:49152")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::MISDIRECTED_REQUEST);
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

use duckfs_core::{DiffEntry, DiffKind, FilesQuery, FilesReply, RefsInfo};

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
                    let q = duckfs_core::decode_query(&req).expect("files query decodes");
                    let bytes = match q {
                        FilesQuery::Refs {} => {
                            duckfs_core::encode_reply(&FilesReply::Refs(RefsInfo {
                                head: Some("ab".repeat(32)),
                                pins: BTreeMap::new(),
                                window_len: 4,
                            }))
                        }
                        FilesQuery::Diff { .. } => {
                            duckfs_core::encode_reply(&FilesReply::Diff(vec![DiffEntry {
                                path: "/a".into(),
                                kind: DiffKind::Modified,
                            }]))
                        }
                        FilesQuery::HasChunks { ids } => {
                            // present iff the id starts with "aa" — proves the reply
                            // order maps back to the request order over the wire.
                            let present = ids.iter().map(|id| id.starts_with("aa")).collect();
                            duckfs_core::encode_reply(&FilesReply::HasChunks { present })
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

// ---- duckfs workspace RPC: 503 when unconfigured, slug validation -----------

#[tokio::test]
async fn fs_workspaces_is_503_when_unconfigured() {
    // a handle that never injected the workspace root (the fake actor's) answers
    // the seam with a clean 503, not a panic. no actor needed: the config guard
    // returns before any command crosses the lane.
    let (handle, _cmd_rx, _events) = NodeHandle::channel();

    let response = noded::router(handle)
        .oneshot(post(
            "/v1/fs/workspaces",
            serde_json::json!({ "prefix": "/shared/x" }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn fs_workspace_commit_rejects_a_bad_slug() {
    // a non-`[a-z0-9]` id (here uppercase) is refused BEFORE any disk touch —
    // the slug guard is the traversal defense on the path param.
    let root = tempfile::tempdir().expect("workspace root");
    let (handle, _cmd_rx, _events) = NodeHandle::channel();
    let handle = handle.with_duckfs_workspaces(root.path().to_path_buf());

    let response = noded::router(handle)
        .oneshot(post(
            "/v1/fs/workspaces/BAD/commit",
            serde_json::json!({ "message": "m" }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["error"], "invalid workspace id");
}
