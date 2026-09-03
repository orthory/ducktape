//! router contract tests over a FAKE node actor: the http layer must forward
//! commands, translate replies, and map failures — without a live host. the
//! real actor is exercised end-to-end by running the binary.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use commonware_cryptography::Signer as _;
use futures::StreamExt as _;
use futures::channel::mpsc;
use http_body_util::BodyExt as _;
use noded::{
    AdminConfig, AdminExposure, BlockSummary, ModuleCategory, ModuleStatus, NodeCommand,
    NodeHandle, NodeStatus,
};
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
                                root_hash: String::from_utf8_lossy(&origin).into_owned(),
                            })
                        }
                    };
                    let _ = reply.send(result);
                }
                // the signed-frame lane, answered as BOTH real binaries answer
                // it: the origin is the frame's VERIFIED signer, never a caller
                // string. echoed back through root_hash — the same origin probe
                // the frameless arm above uses.
                NodeCommand::SubmitFrame { frame, reply } => {
                    let result = match node::decode_frame(&frame) {
                        Ok((sdk::Origin::External(key), msg)) => {
                            assert_eq!(msg.target, "chat");
                            Ok(BlockSummary {
                                height: 11,
                                root_hash: noded::hex_bytes(&key),
                            })
                        }
                        Ok((origin, ..)) => Err(format!("a frame cannot carry {origin:?}")),
                        Err(err) => Err(err.to_string()),
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

/// the acting key a test signs as. ANY ed25519 key may act — the gate proves
/// possession, not membership — so a test mints one exactly like a client.
fn caller() -> commonware_cryptography::ed25519::PrivateKey {
    commonware_cryptography::ed25519::PrivateKey::from_seed(77)
}

/// one SIGNED mutating request, built the way every in-tree client builds one:
/// through `noded::signed_req::request_headers`, never a hand-rolled trio.
/// `NodeHandle::channel()` carries no node key, so the salt is empty here.
fn signed(method: &str, uri: &str, body: serde_json::Value) -> Request<Body> {
    let bytes = serde_json::to_vec(&body).unwrap();
    let mut req = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json");
    for (name, value) in noded::signed_req::request_headers(&caller(), method, uri, &[], &bytes) {
        req = req.header(name, value);
    }
    req.body(Body::from(bytes)).unwrap()
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
    assert_eq!(body["root_hash"], "noded");
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
    assert_eq!(body["root_hash"], "jess");
}

// ---- the signed-write gate over the mutating routes -------------------------

/// an UNSIGNED mutation is refused before the handler runs — and the refusal
/// names one stable reason, which is what a dashboard counts.
#[tokio::test]
async fn an_unsigned_mutation_is_refused_and_never_reaches_the_actor() {
    let (handle, cmd_rx, _events) = NodeHandle::channel();
    // an actor that would PANIC if a command crossed: the gate must refuse
    // before anything reaches the lane.
    tokio::spawn(async move {
        let mut cmds = cmd_rx;
        if cmds.next().await.is_some() {
            panic!("an unsigned mutation reached the node actor");
        }
    });
    let app = noded::router(handle);

    for (method, uri) in [
        ("POST", "/v1/log-filter"),
        ("POST", "/v1/fs/workspaces"),
        ("POST", "/v1/fs/workspaces/abc/commit"),
        ("DELETE", "/v1/fs/workspaces/abc"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "{method} {uri} must refuse an unsigned request"
        );
        let body = body_json(response).await;
        assert_eq!(body["reason"], "signature_missing");
    }
}

/// a signature that does not bind THIS body is not a signature for THIS
/// request: the gate hashes the body precisely so an authenticated caller's
/// bytes cannot be swapped in flight.
#[tokio::test]
async fn a_swapped_body_defeats_the_signature() {
    let (handle, _cmds, _events) = NodeHandle::channel();
    let app = noded::router(handle);

    let signed_for = serde_json::json!({ "prefix": "/shared/mine" });
    let mut request = signed("POST", "/v1/fs/workspaces", signed_for);
    *request.body_mut() = Body::from(r#"{"prefix":"/shared/yours"}"#);

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(body_json(response).await["reason"], "signature_invalid");
}

/// reads are NOT gated: the ruling is about mutation, and a status probe with
/// no key must keep working.
#[tokio::test]
async fn reads_stay_open() {
    let (handle, cmd_rx, _events) = NodeHandle::channel();
    spawn_fake_actor(cmd_rx, None);
    let app = noded::router(handle);

    let status = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status.status(), StatusCode::OK);

    // the object facade shares its path with a gated PUT/DELETE — the GET half
    // must not inherit the gate. (what it answers depends on the actor; that it
    // is not a 401 is the property.)
    let read = app
        .oneshot(
            Request::builder()
                .uri("/v1/files/object/shared/x.txt")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(read.status(), StatusCode::UNAUTHORIZED);
}

// ---- the signed-frame lane (`POST /v1/submit/frame`) -----------------------
//
// the lane exists so an op can carry authorship consensus CHECKS instead of a
// caller string it has to trust. these cases pin exactly that: the origin the
// block sees is the frame's verified signer, and a frame whose signature does
// not bind never reaches the actor at all.

/// the raw-bytes request the lane takes — no json envelope, no origin field.
fn post_frame(frame: Vec<u8>) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/v1/submit/frame")
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .body(Body::from(frame))
        .unwrap()
}

/// the op every frame case submits (the fake actor asserts the target).
fn chat_op() -> sdk::Msg {
    sdk::Msg {
        target: "chat".into(),
        payload: serde_json::to_vec(
            &serde_json::json!({ "create_channel": { "channel_id": "general", "name": "General" } }),
        )
        .unwrap(),
    }
}

#[tokio::test]
async fn a_signed_frame_lands_with_the_signers_key_as_the_origin() {
    let (handle, cmd_rx, _events) = NodeHandle::channel();
    spawn_fake_actor(cmd_rx, None);

    let signer = commonware_cryptography::ed25519::PrivateKey::from_seed(42);
    let frame = node::encode_frame(&signer, 1, &chat_op());
    let response = noded::router(handle)
        .oneshot(post_frame(frame))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["height"], 11);
    // the actor echoes the origin it VERIFIED — the signer's public key, which
    // no part of the request could have claimed.
    assert_eq!(
        body["root_hash"],
        noded::hex_bytes(signer.public_key().as_ref())
    );
    // the receipt addresses the op PAYLOAD, exactly as the frameless lane does.
    assert_eq!(
        body["op_hash"].as_str().map(str::len),
        Some(64),
        "the frame lane returns the same receipt shape"
    );
}

#[tokio::test]
async fn a_tampered_frame_is_refused_before_it_reaches_the_actor() {
    let (handle, cmd_rx, _events) = NodeHandle::channel();
    spawn_fake_actor(cmd_rx, None);

    let signer = commonware_cryptography::ed25519::PrivateKey::from_seed(42);
    let op = chat_op();
    let mut frame = node::encode_frame(&signer, 1, &op);
    // flip one byte of the PAYLOAD: the proof binds (scheme, origin, seq,
    // target, payload), so the frame no longer verifies — and the actor never
    // sees it.
    // located by SEARCHING for the payload rather than counting back from the
    // tail: an offset measured against the frame's trailer silently slides onto
    // a structural byte when the layout gains a field, and the refusal becomes
    // "does not parse" — a different gate than the one under test.
    let at = frame
        .windows(op.payload.len())
        .position(|window| window == op.payload.as_slice())
        .expect("the frame carries the op payload verbatim");
    frame[at] ^= 0x01;

    let response = noded::router(handle)
        .oneshot(post_frame(frame))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    let err = body["error"].as_str().expect("a verbatim refusal");
    assert!(
        err.contains("proof does not bind"),
        "the refusal names the cause: {err}"
    );
}

#[tokio::test]
async fn a_frame_cannot_claim_another_keys_origin() {
    let (handle, cmd_rx, _events) = NodeHandle::channel();
    spawn_fake_actor(cmd_rx, None);

    // key A signs; the frame is then re-stamped to claim key B's origin — the
    // forgery the whole lane exists to make impossible. the origin is INSIDE the
    // signed preimage, so B's key cannot be swapped in without breaking it.
    let a = commonware_cryptography::ed25519::PrivateKey::from_seed(1);
    let b = commonware_cryptography::ed25519::PrivateKey::from_seed(2);
    let mut frame = node::encode_frame(&a, 1, &chat_op());
    let b_key = b.public_key();
    // the origin is the first length-prefixed field: 8 bytes of length, then the
    // 32 key bytes.
    frame[8..8 + 32].copy_from_slice(b_key.as_ref());

    let response = noded::router(handle)
        .oneshot(post_frame(frame))
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "a frame signed by A cannot act as B"
    );
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
    let op_hash = body["op_hash"].as_str().expect("receipt carries op_hash");
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

/// The ws surface's policy (`stream.rs` `ClientMsg`) on the HTTP bodies: a
/// field this build does not know is refused BY NAME, never dropped on the
/// floor while the rest of the body is served (#1325). No actor is spawned on
/// purpose — the refusal happens in the extractor, before dispatch.
#[tokio::test]
async fn submit_refuses_an_unknown_field_by_name() {
    let (handle, _cmd_rx, _events) = NodeHandle::channel();

    let response = noded::router(handle)
        .oneshot(post(
            "/v1/submit",
            serde_json::json!({ "target": "chat", "payload": {}, "orgin": "jess" }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(
        text.contains("unknown field `orgin`"),
        "the refusal names the field: {text}"
    );
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
async fn status_reports_root_hash_height_and_module_roots() {
    // deliberately NO actor: /v1/status serves the last-published snapshot
    // straight off the handle's cell, so it must answer even when nothing
    // drains the command lane — the wedged-behind-sync regression this cell
    // exists to prevent.
    let (handle, _cmd_rx, _events) = NodeHandle::channel();
    handle.status_cell().publish(NodeStatus {
        version: "9.9.9".into(),
        root_hash: "cd".repeat(32),
        height: 3,
        modules: vec![ModuleStatus {
            id: "chat".into(),
            root: "ef".repeat(32),
            category: ModuleCategory::of("chat"),
        }],
        public_key: "ab".repeat(32),
        chain_id: String::new(),
        operations: noded::OperationalStatus {
            role: noded::NodeRole::Validator,
            phase: noded::NodePhase::Validating,
            phase_since: 1_720_000_000,
            ..Default::default()
        },
    });

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
    assert_eq!(body["root_hash"], "cd".repeat(32));
    assert_eq!(body["height"], 3);
    assert_eq!(body["modules"][0]["id"], "chat");
    assert_eq!(body["modules"][0]["root"], "ef".repeat(32));
    // the catalog category rides on the wire as a lowercase string.
    assert_eq!(body["modules"][0]["category"], "workspace");
    assert_eq!(body["operations"]["role"], "validator");
    assert_eq!(body["operations"]["phase"], "validating");
    assert_eq!(body["operations"]["phase_since"], 1_720_000_000u64);
}

#[tokio::test]
async fn peers_reports_the_direct_peer_sample() {
    // NO actor: the sample parses from the wired exposition source and the
    // published standing — like status, peers must answer while a sync
    // stage has the pump busy.
    let (handle, _cmd_rx, _events) = NodeHandle::channel();
    let cell = handle.status_cell();
    cell.wire_exposition(|| "network_tracker_directory_connected{peer=\"ab\"} 1000\n".to_string());
    cell.publish_peers(noded::PeersStanding {
        validators: ["ab".to_string()].into(),
        residents: Default::default(),
        height: 41,
        epoch: Some(7),
        builds: [("ab".to_string(), "abc1234".to_string())].into(),
    });

    let response = noded::router(handle)
        .oneshot(
            Request::builder()
                .uri("/v1/peers")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["height"], 41);
    assert_eq!(body["epoch"], 7);
    assert_eq!(body["peers"][0]["peer"], "ab");
    assert_eq!(body["peers"][0]["connected"], true);
    assert_eq!(body["peers"][0]["connected_since_ms"], 1000);
    assert_eq!(
        body["peers"][0]["role"], "validator",
        "the published standing stamps roles onto the live sample"
    );
    assert_eq!(
        body["peers"][0]["build"], "abc1234",
        "and the build stamp the peer reported about itself, where one was heard"
    );
}

#[tokio::test]
async fn metrics_forwards_the_encoded_registry_as_openmetrics_text() {
    // NO actor: the scrape reads the wired exposition source directly.
    let (handle, _cmd_rx, _events) = NodeHandle::channel();
    handle.status_cell().wire_exposition(|| {
        "# HELP ducktape_blocks_total blocks\nducktape_blocks_total 3\n# EOF\n".to_string()
    });

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
        "the wired exposition passed through: {text:?}"
    );
}

/// this test node's operator credential — what a real node mints 0600 into its
/// workspace and hands only to whoever can read that directory.
const OPERATOR: &str = "0f1e2d3c4b5a69788796a5b4c3d2e1f00f1e2d3c4b5a69788796a5b4c3d2e1f0";

/// a handle whose admin namespace is gated on [`OPERATOR`], default exposure.
fn operator_handle() -> (NodeHandle, mpsc::Receiver<NodeCommand>) {
    let (handle, cmd_rx, _events) = NodeHandle::channel();
    let handle = handle.with_admin(AdminConfig {
        operator_token: Some(OPERATOR.to_string()),
        ..Default::default()
    });
    (handle, cmd_rx)
}

/// an admin request with the method the route actually serves — `logs/tail` is
/// a GET, and `post()` would answer 405 there long before the gate ran.
fn admin_request(method: &str, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{}"))
        .unwrap()
}

/// stamp the operator credential onto a request the way a client that read
/// `admin.token` out of the node's workspace would.
fn with_operator(mut req: Request<Body>) -> Request<Body> {
    req.headers_mut().insert(
        noded::admin::ADMIN_TOKEN_HEADER,
        OPERATOR.parse().expect("token is a header value"),
    );
    req
}

#[tokio::test]
async fn shutdown_acknowledges_then_signals() {
    let (handle, cmd_rx) = operator_handle();
    spawn_fake_actor(cmd_rx, None);
    let signal = handle.clone();

    // shutdown moved to the owner-gated admin namespace. the default
    // handle has no on-chain owner, so the operator credential is the gate; the
    // loopback check is FAIL-CLOSED on a missing ConnectInfo, so the test stamps
    // a loopback peer exactly as the connect-info make-service would.
    let response = noded::router(handle)
        .oneshot(with_operator(with_peer(
            post("/v1/admin/shutdown", serde_json::json!({})),
            "127.0.0.1:40000",
        )))
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

/// stamp a peer address onto a request the way `into_make_service_with_connect_info`
/// would, so the admin guard's loopback check has something to read.
fn with_peer(mut req: Request<Body>, addr: &str) -> Request<Body> {
    req.extensions_mut().insert(axum::extract::ConnectInfo(
        addr.parse::<std::net::SocketAddr>()
            .expect("test peer addr"),
    ));
    req
}

/// shutdown left the unauthenticated public surface entirely. the old
/// path is a 404 — flag-day, no alias.
#[tokio::test]
async fn the_old_public_shutdown_route_is_gone() {
    let (handle, cmd_rx, _events) = NodeHandle::channel();
    spawn_fake_actor(cmd_rx, None);
    let response = noded::router(handle)
        .oneshot(post("/v1/shutdown", serde_json::json!({})))
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "shutdown must not answer on the public surface anymore"
    );
}

/// `DUCKTAPE_ADMIN=off` leaves the control surface simply ABSENT — the admin
/// routes are never registered, so they 404 (not a gated-but-present 403).
#[tokio::test]
async fn a_disabled_admin_namespace_is_absent() {
    let (handle, cmd_rx, _events) = NodeHandle::channel();
    spawn_fake_actor(cmd_rx, None);
    let handle = handle.with_admin(AdminConfig {
        exposure: AdminExposure::Disabled,
        node_key: None,
        ..Default::default()
    });
    let response = noded::router(handle)
        .oneshot(post("/v1/admin/shutdown", serde_json::json!({})))
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "a disabled admin namespace is absent, not forbidden"
    );
}

/// under the default `Loopback` exposure a non-loopback peer is refused before
/// any owner check — the exposure gate is the outer wall.
#[tokio::test]
async fn a_non_loopback_peer_is_refused_under_loopback_exposure() {
    let (handle, cmd_rx, _events) = NodeHandle::channel();
    spawn_fake_actor(cmd_rx, None);
    let request = with_peer(
        post("/v1/admin/shutdown", serde_json::json!({})),
        "203.0.113.7:5555",
    );
    let response = noded::router(handle).oneshot(request).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "loopback-only admin refuses a remote peer"
    );
}

/// THE regression this gate exists for: a LOOPBACK process with no operator
/// credential — a service daemon, a stray script, anything that can dial the
/// port — must not be able to stop the node or stage module wasm. The operator,
/// who read `admin.token` out of the node's own workspace, still can.
#[tokio::test]
async fn a_loopback_caller_without_the_operator_credential_cannot_drive_admin() {
    // the two destructive routes AND the one that READS: shutdown stops the
    // process, module-code/stage ingests a wasm artifact and fans it out to
    // members, and logs/tail drains the 4096-line ring — every line the node
    // ever logged, which is a real secret-read surface, not merely a noisy one.
    // Each route with the method it actually serves: a POST to logs/tail is a
    // 405 that would pass this assertion for entirely the wrong reason.
    for (method, route) in [
        ("POST", "/v1/admin/shutdown"),
        ("POST", "/v1/admin/module-code/stage"),
        ("GET", "/v1/admin/logs/tail"),
    ] {
        let (handle, cmd_rx) = operator_handle();
        spawn_fake_actor(cmd_rx, None);
        let response = noded::router(handle)
            .oneshot(with_peer(admin_request(method, route), "127.0.0.1:40000"))
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "{route} must refuse an uncredentialed loopback caller"
        );
        let body = body_json(response).await;
        assert_eq!(body["reason"], "operator_token_missing");
        // the refusal must never hand back the credential it wanted.
        assert!(
            !body.to_string().contains(OPERATOR),
            "a refusal must never echo the expected credential"
        );
    }

    // a WRONG credential is a distinct, and distinctly named, refusal.
    let (handle, cmd_rx) = operator_handle();
    spawn_fake_actor(cmd_rx, None);
    let mut guessed = with_peer(
        post("/v1/admin/shutdown", serde_json::json!({})),
        "127.0.0.1:40000",
    );
    guessed.headers_mut().insert(
        noded::admin::ADMIN_TOKEN_HEADER,
        "deadbeef".parse().unwrap(),
    );
    let response = noded::router(handle).oneshot(guessed).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        body_json(response).await["reason"],
        "operator_token_mismatch"
    );

    // and the operator still gets through — a gate that locks the owner out is
    // a worse bug than the one it closes.
    let (handle, cmd_rx) = operator_handle();
    spawn_fake_actor(cmd_rx, None);
    let signal = handle.clone();
    let response = noded::router(handle)
        .oneshot(with_operator(with_peer(
            post("/v1/admin/shutdown", serde_json::json!({})),
            "127.0.0.1:40000",
        )))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    // wait on the system's own event, not on a duration.
    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        signal.shutdown_requested(),
    )
    .await
    .expect("the operator's shutdown reached the node");
}

/// the stage lane's body cap is EXPLICIT, and over it is a NAMED refusal.
///
/// Two cliffs, one test. Without a `DefaultBodyLimit` layer axum applies its
/// implicit 2 MiB default, and `crates/modules/apps/runs/component.wasm` is
/// already 1.73 MB of that — so the next module to grow would have become
/// un-stageable behind an opaque tower error with no reason token. Above the
/// real cap the refusal must still be a reason a client can branch on.
#[tokio::test]
async fn the_module_stage_body_cap_is_explicit_and_its_refusal_is_named() {
    fn stage(body: Vec<u8>) -> Request<Body> {
        with_operator(with_peer(
            Request::builder()
                .method("POST")
                // fanout=false: this handle wires no code plane, and the
                // network fan-out is not what the body cap is about.
                .uri("/v1/admin/module-code/stage?fanout=false")
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .body(Body::from(body))
                .unwrap(),
            "127.0.0.1:40000",
        ))
    }

    // 3 MiB — over axum's implicit default, under ours. The cliff is gone.
    let (handle, cmd_rx) = operator_handle();
    spawn_fake_actor(cmd_rx, None);
    let response = noded::router(handle)
        .oneshot(stage(vec![7u8; 3 * 1024 * 1024]))
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "an artifact past axum's implicit 2 MiB default must still stage"
    );

    // over the explicit cap — refused, with a token rather than tower's prose.
    let (handle, cmd_rx) = operator_handle();
    spawn_fake_actor(cmd_rx, None);
    let response = noded::router(handle)
        .oneshot(stage(vec![7u8; 16 * 1024 * 1024 + 1]))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(
        body_json(response).await["reason"],
        "module_artifact_too_large"
    );
}

/// FAIL CLOSED: a node that minted no operator credential verifies nothing, so
/// it refuses every admin request. There is no "unauthenticated if unset" arm —
/// that fallback IS the hole this gate closes.
#[tokio::test]
async fn a_node_with_no_minted_credential_refuses_every_admin_request() {
    let (handle, cmd_rx, _events) = NodeHandle::channel();
    spawn_fake_actor(cmd_rx, None);
    // the default config carries no token, and the default exposure is Loopback.
    assert_eq!(AdminConfig::default().operator_token, None);
    let response = noded::router(handle)
        .oneshot(with_operator(with_peer(
            post("/v1/admin/shutdown", serde_json::json!({})),
            "127.0.0.1:40000",
        )))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        body_json(response).await["reason"],
        "operator_token_unavailable"
    );
}

/// an actor that answers exactly one thing: `identity` `OfKey` for the
/// operator's `owner_key` → the account it is on, whose sole member it is.
/// everything else is a module error (the admin owner path only ever asks this
/// one question).
fn spawn_owner_actor(mut cmds: mpsc::Receiver<NodeCommand>, owner_key: Vec<u8>) {
    tokio::spawn(async move {
        while let Some(cmd) = cmds.next().await {
            if let NodeCommand::Query { target, req, reply } = cmd {
                assert_eq!(target, "identity");
                let identity::IdentityQuery::OfKey { key } =
                    identity::decode_query(&req).expect("an identity query")
                else {
                    panic!("the admin owner path asks only OfKey");
                };
                assert_eq!(
                    key, owner_key,
                    "the owner path resolves the operator's own key"
                );
                let view = identity::AccountView {
                    number: 1,
                    name: "owner".into(),
                    keys: vec![identity::KeyView {
                        scheme: identity::KeyScheme::Ed25519,
                        pubkey: owner_key.clone(),
                        label: None,
                        added_at: 0,
                    }],
                    avatar: None,
                    bio: None,
                    updated_at: 0,
                };
                let bytes = identity::encode_reply(&identity::IdentityReply::Account(Some(view)));
                let _ = reply.send(Ok(bytes));
            }
        }
    });
}

fn admin_signed_post(
    uri: &str,
    signer: &commonware_cryptography::ed25519::PrivateKey,
    claimed_key_hex: &str,
    node_key: &[u8],
    ts: u64,
) -> Request<Body> {
    let sig = noded::admin::sign_admin(signer, "POST", uri, node_key, ts);
    Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .header(noded::admin::ADMIN_KEY_HEADER, claimed_key_hex)
        .header(noded::admin::ADMIN_TS_HEADER, ts.to_string())
        .header(
            noded::admin::ADMIN_SIG_HEADER,
            duckfs_core::to_hex(sig.as_ref()),
        )
        .body(Body::from("{}"))
        .unwrap()
}

/// the full `Public` owner path over the actor lane: unsigned is refused, a
/// non-owner signature is refused, the committed owner's signature passes.
#[tokio::test]
async fn public_admin_enforces_the_committed_owner_pop() {
    use commonware_cryptography::Signer as _;
    let owner = commonware_cryptography::ed25519::PrivateKey::from_seed(77);
    let owner_key = owner.public_key().as_ref().to_vec();
    let node_key = vec![0xabu8; 32];
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let owner_key_hex = duckfs_core::to_hex(&owner_key);

    // the handle carries a REAL operator credential on purpose: a `Public` node
    // with a committed owner must be on the owner path and nothing else, so
    // every assertion below is made against a node that HAS the other secret.
    let mk_handle = || {
        let (handle, cmd_rx, _e) = NodeHandle::channel();
        spawn_owner_actor(cmd_rx, owner_key.clone());
        handle.with_admin(AdminConfig {
            exposure: AdminExposure::Public,
            node_key: Some(node_key.clone()),
            owner_key: Some(owner_key.clone()),
            operator_token: Some(OPERATOR.to_string()),
            ..Default::default()
        })
    };

    // unsigned ⇒ 401.
    let bare = noded::router(mk_handle())
        .oneshot(post("/v1/admin/shutdown", serde_json::json!({})))
        .await
        .unwrap();
    assert_eq!(
        bare.status(),
        StatusCode::UNAUTHORIZED,
        "public admin needs a signature"
    );

    // signed by a non-owner (valid PoP, wrong account) ⇒ 403.
    let attacker = commonware_cryptography::ed25519::PrivateKey::from_seed(99);
    let attacker_hex = duckfs_core::to_hex(attacker.public_key().as_ref());
    let forged = noded::router(mk_handle())
        .oneshot(admin_signed_post(
            "/v1/admin/shutdown",
            &attacker,
            &attacker_hex,
            &node_key,
            ts,
        ))
        .await
        .unwrap();
    assert_eq!(
        forged.status(),
        StatusCode::FORBIDDEN,
        "a non-owner signer is refused"
    );

    // the owner's signature bound to a DIFFERENT node ⇒ 401 (cross-node replay).
    let replayed = noded::router(mk_handle())
        .oneshot(admin_signed_post(
            "/v1/admin/shutdown",
            &owner,
            &owner_key_hex,
            &[0xcd; 32],
            ts,
        ))
        .await
        .unwrap();
    assert_eq!(
        replayed.status(),
        StatusCode::UNAUTHORIZED,
        "a signature minted for another node is refused here"
    );

    // THE cross-check: the operator credential is NOT an alternative credential
    // here. Once an owner is committed, `Public` is the owner path and only the
    // owner path — otherwise anyone who can read the workspace would keep a
    // standing bypass around the very PoP that `Public` exposure exists for,
    // and the two gates would be an OR instead of a ladder. A loopback peer
    // presenting a VALID operator token and no signature is still refused.
    let mut token_only = with_operator(with_peer(
        post("/v1/admin/shutdown", serde_json::json!({})),
        "127.0.0.1:40000",
    ));
    // and a smuggled owner-key header changes nothing without the signature.
    token_only.headers_mut().insert(
        noded::admin::ADMIN_KEY_HEADER,
        owner_key_hex.parse().unwrap(),
    );
    let refused = noded::router(mk_handle())
        .oneshot(token_only)
        .await
        .unwrap();
    assert_eq!(
        refused.status(),
        StatusCode::UNAUTHORIZED,
        "the operator token must not stand in for the owner PoP"
    );
    assert_eq!(
        body_json(refused).await["reason"],
        "owner_signature_invalid",
        "a token-only caller must fail the OWNER check, not pass some operator arm"
    );

    // the committed owner's signature for THIS node ⇒ 200.
    let ok = noded::router(mk_handle())
        .oneshot(admin_signed_post(
            "/v1/admin/shutdown",
            &owner,
            &owner_key_hex,
            &node_key,
            ts,
        ))
        .await
        .unwrap();
    assert_eq!(
        ok.status(),
        StatusCode::OK,
        "the node owner may drive control"
    );
}

/// the loopback gate FAILS CLOSED: a request with no ConnectInfo at all (an
/// embedder that forgot the connect-info make-service) is refused, never
/// granted local trust.
#[tokio::test]
async fn a_peer_without_connect_info_is_refused() {
    let (handle, cmd_rx, _events) = NodeHandle::channel();
    spawn_fake_actor(cmd_rx, None);
    let response = noded::router(handle)
        .oneshot(post("/v1/admin/shutdown", serde_json::json!({})))
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "an unknown peer must not inherit loopback trust"
    );
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
            chain_id: "test".into(),
            account_id: 1,
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

/// Answer the duck:// browser proxy's queries against the MERGED gateway
/// module: a handle `Resolve` returns the route's account, a `Get` returns the
/// signed route — both now target the one "gateway" module, dispatched by
/// query variant. `queries` is the total count.
fn spawn_duck_actor(mut cmds: mpsc::Receiver<NodeCommand>, queries: usize) {
    tokio::spawn(async move {
        for _ in 0..queries {
            let NodeCommand::Query { target, req, reply } = cmds.next().await.unwrap() else {
                panic!("gateway only issues queries");
            };
            assert_eq!(target, "gateway");
            let bytes = match gateway::decode_query(&req).unwrap() {
                gateway::GatewayQuery::Resolve { .. } => {
                    gateway::encode_reply(&gateway::GatewayReply::Resolved(Some(
                        gateway::ResolvedAccount { account_id: 1 },
                    )))
                }
                gateway::GatewayQuery::Get { .. } => gateway::encode_reply(
                    &gateway::GatewayReply::Route(Box::new(Some(gateway_route()))),
                ),
                other => panic!("unexpected query {other:?}"),
            };
            let _ = reply.send(Ok(bytes));
        }
    });
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
                    account_id: 1,
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
        let noded::GatewayJob::Http {
            publisher_node,
            head,
            body,
            reply,
            ..
        } = job
        else {
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
            body: {
                let (tx, rx) = tokio::sync::mpsc::channel(1);
                tx.try_send(Ok(bytes::Bytes::from_static(br#"{"ok":true}"#)))
                    .unwrap();
                drop(tx);
                rx
            },
        }));
    });
    let request_body = br#"{"name":"duck"}"#;
    let request = post(
        "/v1/gateway/proxy",
        serde_json::json!({
            "head": {
                "account_id": 1,
                "name": { "label": "app" },
                "revision": 7,
                "method": "post",
                "path_and_query": "/api/items",
                "headers": [{ "name": "content-type", "value": "application/json" }],
                "body_len": request_body.len(),
                "upgrade": false,
            },
            "body_b64": base64::engine::general_purpose::STANDARD.encode(request_body),
        }),
    );
    let response = noded::router(handle.with_gateway(lane))
        .oneshot(request)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["head"]["status"], 201);
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(body["body_b64"].as_str().unwrap())
            .unwrap(),
        br#"{"ok":true}"#
    );
}

#[tokio::test]
async fn gateway_api_rejects_untrusted_browser_origins_before_network_work() {
    let (handle, _cmds, _events) = NodeHandle::channel();
    for origin in ["https://evil.example", "http://app.demo.duck"] {
        let mut request = Request::builder()
            .method("GET")
            .uri("/v1/gateway/browser")
            .body(Body::empty())
            .unwrap();
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
}

#[tokio::test]
async fn gateway_browser_proxy_is_duck_origin_scoped_and_cross_origin_safe() {
    let (handle, cmds, _events) = NodeHandle::channel();
    // One proxy request resolves once via duckdns and twice via gateway
    // (the revision pre-check plus proxy_current's own resolution).
    spawn_duck_actor(cmds, 3);
    let (lane, mut jobs) = tokio::sync::mpsc::channel::<noded::GatewayJob>(1);
    let handle = handle
        .with_gateway(lane)
        .with_browser_gateway("127.0.0.1:49152".parse().unwrap());

    tokio::spawn(async move {
        let job = jobs.recv().await.unwrap();
        let noded::GatewayJob::Http {
            head, body, reply, ..
        } = job
        else {
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
            body: {
                let (tx, rx) = tokio::sync::mpsc::channel(1);
                tx.try_send(Ok(bytes::Bytes::from_static(br#"{"ok":true}"#)))
                    .unwrap();
                drop(tx);
                rx
            },
        }));
    });
    let authority = "app.demo.duck";
    let origin = format!("duck://{authority}");
    let response = noded::gateway_browser_router(handle.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api")
                .header("x-duck-authority", authority)
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
    assert!(csp.contains(&format!("connect-src {origin} ws://127.0.0.1:49152")));
    assert!(csp.contains("worker-src 'none'"));
    assert!(csp.contains("frame-ancestors 'none'"));
    assert!(csp.contains("sandbox allow-scripts allow-same-origin allow-forms"));
    assert!(csp.contains("webrtc 'block'"));

    // A page whose Origin does not match the forwarded authority is rejected
    // before any network work.
    let response = noded::gateway_browser_router(handle.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api")
                .header("x-duck-authority", authority)
                .header(header::ORIGIN, "duck://evil.demo.duck")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    // A request the scheme handler did not stamp with an authority is refused.
    let response = noded::gateway_browser_router(handle)
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/")
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
async fn pages_presence_ws_route_is_wired() {
    let (handle, cmd_rx, _events) = NodeHandle::channel();
    spawn_fake_actor(cmd_rx, None);

    let response = noded::router(handle)
        .oneshot(ws_upgrade("/v1/presence/ws?page=page-1"))
        .await
        .unwrap();

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
                            root_hash: "ab".repeat(32),
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

#[tokio::test]
async fn upload_pack_have_round_returns_only_plain_nak() {
    fn pkt(payload: &[u8]) -> Vec<u8> {
        let mut line = format!("{:04x}", payload.len() + 4).into_bytes();
        line.extend_from_slice(payload);
        line
    }

    let oid = "11".repeat(20);
    let mut request_body = pkt(format!("want {oid} multi_ack_detailed side-band-64k\n").as_bytes());
    request_body.extend_from_slice(b"0000");
    request_body.extend_from_slice(&pkt(format!("have {oid}\n").as_bytes()));
    request_body.extend_from_slice(b"0000");

    let forge_root = tempfile::tempdir().expect("forge root");
    let (handle, _cmd_rx, _events) = NodeHandle::channel();
    let response = noded::router(handle.with_forge_repo(forge_root.path()))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/forge/repo/git-upload-pack")
                .header(
                    header::CONTENT_TYPE,
                    "application/x-git-upload-pack-request",
                )
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "application/x-git-upload-pack-result"
    );
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"0008NAK\n");
    assert!(!body.windows(4).any(|window| window == b"PACK"));
}

// ---- duckfs workspace RPC: 503 when unconfigured, slug validation -----------

#[tokio::test]
async fn fs_workspaces_is_503_when_unconfigured() {
    // a handle that never injected the workspace root (the fake actor's) answers
    // the seam with a clean 503, not a panic. no actor needed: the config guard
    // returns before any command crosses the lane.
    let (handle, _cmd_rx, _events) = NodeHandle::channel();

    let response = noded::router(handle)
        .oneshot(signed(
            "POST",
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
        .oneshot(signed(
            "POST",
            "/v1/fs/workspaces/BAD/commit",
            serde_json::json!({ "message": "m" }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["error"], "invalid workspace id");
}

/// The invite route, end to end through the real router.
///
/// Three properties, and each one is a way an operator gets hurt without it: a
/// daemon with no workspace must SAY it cannot mint rather than 500 or hang; a
/// nonsense TTL must be refused before the mint touches the descriptor (the
/// mint SAVES that file, so a refusal afterwards is a write nobody asked for);
/// and a successful mint must answer the blob itself, because the caller pastes
/// what comes back.
#[tokio::test]
async fn the_invite_route_mints_refuses_and_says_when_it_cannot() {
    let body_of = |response: axum::response::Response| async move {
        let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
            .await
            .expect("a bounded body");
        String::from_utf8(bytes.to_vec()).expect("utf-8")
    };
    let post = |app: axum::Router, body: &'static str| async move {
        app.oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/invite")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .expect("a request"),
        )
        .await
        .expect("a response")
    };

    // no minter wired: the honest answer is "this daemon does not do that".
    let (handle, _cmds, _events) = NodeHandle::channel();
    let unwired = post(noded::router(handle), r#"{"ttl_days":7}"#).await;
    assert_eq!(unwired.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(body_of(unwired).await.contains("no invite minter"));

    // wired: the TTL reaches the minter, and the blob comes back whole.
    let (handle, _cmds, _events) = NodeHandle::channel();
    handle
        .status_cell()
        .wire_invite_minter(|ttl_days| Ok(format!("duck-invite-for-{ttl_days}-days")));
    let app = noded::router(handle);

    let minted = post(app.clone(), r#"{"ttl_days":7}"#).await;
    assert_eq!(minted.status(), StatusCode::OK);
    assert!(body_of(minted).await.contains("duck-invite-for-7-days"));

    // a TTL outside the bounds never reaches the minter — which means the
    // descriptor is never rewritten for a request that was going to be refused.
    for refused in [r#"{"ttl_days":0}"#, r#"{"ttl_days":4000}"#] {
        let response = post(app.clone(), refused).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{refused}");
        assert!(body_of(response).await.contains("ttl_days"));
    }

    // no body at all is the ONE default the CLI verb mints too, not an error:
    // `curl -XPOST /v1/invite` and `ducktape node invite` hand out the same invite.
    let defaulted = post(app, "{}").await;
    assert_eq!(defaulted.status(), StatusCode::OK);
    let expected = format!("for-{}-days", workspace_config::DEFAULT_INVITE_TTL_DAYS);
    assert!(body_of(defaulted).await.contains(&expected));
}
