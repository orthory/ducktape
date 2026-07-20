//! Airlock over the gateway overlay: `cred != compute` end to end on two real,
//! TUN-less WireGuard nodes.
//!
//! Alice (the credential node) runs an airlock gateway on loopback, seals a
//! credential into it, registers its port, and publishes `airlock.alice.duck`
//! as a `LoopbackHttp` route with `allow_authorization = true`. The airlock
//! CLIENT then drives the full protocol — attest, session-key handshake, and a
//! proxied `/v1/messages` — through BOB's browser-gateway door and over the
//! authenticated WireGuard overlay to Alice's enclave, and gets the swapped
//! credential's reply back. This proves the remote topology the design's §graft
//! calls out, minus silicon-backed quote GENERATION and the real Anthropic
//! upstream (a testkit-minted SNP quote — checked by the REAL chain verifier
//! under the test enclave's roots — and a mock upstream stand in, so the test
//! is deterministic and spends no quota).
//!
//! The session-key handshake is load-bearing HERE: the quote is fetched and
//! verified OVER the untrusted overlay before any token is derived, so a relaying
//! node cannot substitute its key or read the token.

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::{Cluster, poll_until, serial};
use commonware_cryptography::{Signer as _, ed25519};
use gateway::{
    DuckDnsName, GatewayMsg, GatewayQuery, GatewayReply, MemberAuthorization, RouteAudience,
    RouteDefinition, RouteMethod, RouteName, RoutePolicy, RouteStatement, RouteTarget,
};
use identity::{AccountView, IdentityMsg, IdentityQuery, IdentityReply, MemberAuth};

use airlock::attest::{self, Measurement};
use airlock::client::Gateway as AirlockClient;
use airlock::server::{self, GatewayConfig};
use airlock::wire::CredentialPayload;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::json;
use tokio::runtime::Runtime;

const READY: Duration = Duration::from_secs(180);
const FINALIZE: Duration = Duration::from_secs(60);

fn measurement_hex() -> String {
    "11".repeat(attest::MRTD_LEN)
}

/// ONE test enclave (measures `0x11`x48) shared by the tests in this file. Its
/// minted SNP chain is verified through the REAL `airlock::verify` path — but
/// only under its own roots, never under the AMD builtins.
fn test_enclave() -> &'static Arc<airlock::testkit::SnpTestEnclave> {
    static ENCLAVE: std::sync::OnceLock<Arc<airlock::testkit::SnpTestEnclave>> =
        std::sync::OnceLock::new();
    ENCLAVE.get_or_init(|| {
        let m = Measurement::from_hex(&measurement_hex()).unwrap();
        Arc::new(airlock::testkit::SnpTestEnclave::new(&m).unwrap())
    })
}

// ----- identity + duckdns helpers (mirrors gateway_e2e) ---------------------

fn bind_auth(member: &ed25519::PrivateKey, chain: &str, node: &[u8]) -> MemberAuth {
    MemberAuth {
        key: member.public_key().as_ref().to_vec(),
        kind: identity::KeyKind::Ed25519,
        proof: identity::MemberProof::Signature {
            sig: member
                .sign(
                    identity::IDENTITY_BIND_NS,
                    &identity::bind_preimage(chain, node, 0),
                )
                .as_ref()
                .to_vec(),
        },
    }
}

fn account_of_node(cluster: &Cluster, reader: usize, node: &[u8]) -> Option<AccountView> {
    let bytes = cluster.query(
        reader,
        "identity",
        &identity::encode_query(&IdentityQuery::OfNode {
            node_key: node.to_vec(),
        }),
    )?;
    match identity::decode_reply(&bytes).ok()? {
        IdentityReply::Account(account) => account,
        IdentityReply::Accounts(_) => None,
    }
}

fn resolve_handle(cluster: &Cluster, reader: usize, handle: &str) -> Option<Vec<u8>> {
    let bytes = cluster.query(
        reader,
        "gateway",
        &gateway::encode_query(&GatewayQuery::Resolve {
            name: DuckDnsName {
                handle: handle.into(),
            },
        }),
    )?;
    match gateway::decode_reply(&bytes).ok()? {
        GatewayReply::Resolved(Some(account)) => Some(account.account_id),
        _ => None,
    }
}

/// The airlock route: `LoopbackHttp` with `allow_authorization = true` (the
/// session-token bearer must reach the enclave) and a real 4 MiB response cap.
/// GET (attestation) + POST (session, messages) are the methods the airlock
/// protocol uses.
///
/// NB: `max_response_bytes = 0` ("unbounded/SSE") is enforced LITERALLY as a
/// 0-byte cap on today's BUFFERED proxy path (`proxy_current`), so it 502s any
/// non-empty response. Unbounded/streaming is the deferred SSE-over-overlay
/// slice; until then a real cap is required, so short turns fit and long
/// interactive streaming waits on that slice.
fn signed_airlock_route(
    member: &ed25519::PrivateKey,
    chain: &str,
    publisher: &[u8],
    revision: u64,
) -> GatewayMsg {
    let statement = RouteStatement {
        version: 1,
        chain_id: chain.into(),
        account_id: member.public_key().as_ref().to_vec(),
        name: RouteName::named("airlock"),
        publisher_node: publisher.to_vec(),
        revision,
        route: Some(RouteDefinition {
            target: RouteTarget::LoopbackHttp,
            policy: RoutePolicy {
                audience: RouteAudience::Network,
                methods: vec![RouteMethod::Get, RouteMethod::Head, RouteMethod::Post],
                max_request_bytes: 1024 * 1024,
                max_response_bytes: 4 * 1024 * 1024,
                allow_authorization: true,
                allow_upgrade: false,
            },
        }),
    };
    let signature = member
        .sign(
            gateway::GATEWAY_ROUTE_NS,
            &gateway::route_signing_preimage(&statement).unwrap(),
        )
        .as_ref()
        .to_vec();
    GatewayMsg::SetRoute {
        statement,
        authorization: MemberAuthorization {
            signer: member.public_key().as_ref().to_vec(),
            signature,
        },
    }
}

fn airlock_route_revision(cluster: &Cluster, reader: usize, account: &[u8]) -> Option<u64> {
    let bytes = cluster.query(
        reader,
        "gateway",
        &gateway::encode_query(&GatewayQuery::Get {
            account_id: account.to_vec(),
            name: RouteName::named("airlock"),
        }),
    )?;
    match gateway::decode_reply(&bytes).ok()? {
        GatewayReply::Route(record) => record
            .as_ref()
            .as_ref()
            .map(|record| record.statement.revision),
        _ => None,
    }
}

// ----- in-process mock Anthropic upstream + airlock gateway -----------------

/// A mock Anthropic upstream: `/oauth/token` mints `acc-N`; `/v1/messages`
/// accepts ONLY `Bearer acc-N` (so a 200 proves the gateway swapped the session
/// token for the real credential) and returns the `AIRLOCK-OK` marker.
#[derive(Default)]
struct MockUpstream {
    n: Mutex<u64>,
}

async fn mock_oauth(State(st): State<Arc<MockUpstream>>) -> Json<serde_json::Value> {
    let mut n = st.n.lock().unwrap();
    *n += 1;
    Json(json!({ "access_token": format!("acc-{n}"), "refresh_token": format!("ref-{n}"), "expires_in": 3600 }))
}

async fn mock_messages(
    State(st): State<Arc<MockUpstream>>,
    headers: axum::http::HeaderMap,
) -> axum::response::Response {
    use axum::response::IntoResponse as _;
    let want = format!("Bearer acc-{}", *st.n.lock().unwrap());
    let got = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if got != want {
        return (axum::http::StatusCode::UNAUTHORIZED, "bad upstream bearer").into_response();
    }
    ([("content-type", "text/event-stream")], "data: AIRLOCK-OK\n\n").into_response()
}

async fn bind_and_serve(app: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

/// Boot the mock upstream and the airlock gateway (testkit quoter, verified by
/// the real SNP verifier) pointed at it.
/// Returns `(gateway_base_url, gateway_loopback_port)`.
async fn boot_gateway_and_upstream() -> (String, u16) {
    let upstream = bind_and_serve(
        Router::new()
            .route("/oauth/token", post(mock_oauth))
            .route("/v1/messages", post(mock_messages))
            .with_state(Arc::new(MockUpstream::default())),
    )
    .await;

    let (app, vendor) = server::build_with_quoter(
        GatewayConfig {
            attest: "snp".into(),
            anthropic_base: upstream.clone(),
            oauth_token_url: format!("{upstream}/oauth/token"),
            oauth_client_id: "test-client".into(),
            session_ttl_secs: 3600,
            max_requests: 100,
        },
        "snp",
        test_enclave().quoter(),
        None,
    )
    .unwrap();
    assert_eq!(vendor, "snp");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://127.0.0.1:{port}"), port)
}

// TODO(full-spec, needs hardware): this proves the node-to-node overlay hop with
// a minted SNP chain (real verifier, test roots) and a mock upstream. The full
// 2-node + TEE run — silicon-backed quote GENERATION on a confidential VM and
// the real Anthropic API via the static bearer (PR #681) — is deferred to real
// hardware. See the design spec "TODO — full 2-node + TEE validation".
#[test]
fn airlock_over_gateway_two_wireguard_nodes() {
    let _serial = serial();
    let rt = Runtime::new().unwrap();

    // Alice's loopback services: the airlock gateway + the mock upstream it swaps
    // into. Both live in THIS process; the alice node subprocess reaches the
    // gateway over host loopback (127.0.0.1:port), exactly like a real deployment.
    let (gw_base, gw_port) = rt.block_on(boot_gateway_and_upstream());

    // Credential Provider (local, direct to the enclave loopback): verify the
    // quote, then seal a refresh token (the mock upstream mints access tokens).
    rt.block_on(async {
        let gw = AirlockClient::local(gw_base.clone());
        let (quote, _vendor) = gw.fetch_quote().await.unwrap();
        let expected = Measurement::from_hex(&measurement_hex()).unwrap();
        let rd = airlock::verify::verify_quote(&quote, &expected, &test_enclave().roots())
            .await
            .unwrap();
        let seal_pk = attest::split_report_data(&rd).0;
        gw.upload_sealed_credential(
            &seal_pk,
            &CredentialPayload::Refresh { refresh_token: "seed".into() },
        )
        .await
        .unwrap();
    });

    // Two real WireGuard nodes: alice (0, publisher) and bob (1, compute).
    let mut cluster = Cluster::new(&[0, 1], &[0, 1]);
    cluster.wireguard = true;
    cluster.wireguard_socket = true;
    for index in 0..2 {
        cluster.spawn(index);
    }
    for index in 0..2 {
        cluster.wait_marker(index, "rpc listening on", READY);
        cluster.wait_marker(index, "converged app_hash=", READY);
        cluster.wait_marker(index, "1 peer(s); userspace socket backend", READY);
        cluster.wait_marker(index, "gateway plane: overlay stream bound", READY);
    }

    let alice = ed25519::PrivateKey::from_seed(42);
    let bob = ed25519::PrivateKey::from_seed(43);
    let alice_node = Cluster::identity(0);
    let bob_node = Cluster::identity(1);
    for (index, member, node) in [
        (0usize, &alice, alice_node.as_slice()),
        (1usize, &bob, bob_node.as_slice()),
    ] {
        cluster.submit(
            index,
            "identity",
            &identity::encode_msg(&IdentityMsg::BindNode {
                authorizer: bind_auth(member, &cluster.namespace, node),
            }),
        );
        poll_until("identity binding", FINALIZE, || {
            account_of_node(&cluster, index, node)
                .filter(|account| account.account_id == member.public_key().as_ref())
        });
    }

    // Alice maps `alice.duck`, so `airlock.alice.duck` resolves to her account.
    cluster.submit(
        0,
        "gateway",
        &gateway::encode_msg(&GatewayMsg::SetHandle {
            handle: Some("alice".into()),
        }),
    );
    for reader in 0..2 {
        let resolved = poll_until("alice.duck resolution", FINALIZE, || {
            resolve_handle(&cluster, reader, "alice")
        });
        assert_eq!(resolved, alice.public_key().as_ref());
    }

    // Register the gateway's loopback port node-locally, then publish the signed
    // LoopbackHttp route on consensus.
    let workspace = cluster.workspace(0);
    let (ok, output) = cluster.run_verb(&[
        "gateway-route-bind",
        "--workspace",
        workspace.to_str().unwrap(),
        "--label",
        "airlock",
        "--port",
        &gw_port.to_string(),
    ]);
    assert!(ok, "airlock gateway port bind failed: {output}");

    cluster.submit(
        0,
        "gateway",
        &gateway::encode_msg(&signed_airlock_route(&alice, &cluster.namespace, &alice_node, 1)),
    );
    poll_until("airlock route revision 1", FINALIZE, || {
        (airlock_route_revision(&cluster, 1, alice.public_key().as_ref()) == Some(1)).then_some(())
    });

    // Bob's browser-gateway origin base — the `via` a compute-side host process
    // posts to (no Origin header ⇒ it passes the only guard).
    let (status, browser) = cluster.http(1, "GET", "/v1/gateway/browser", None);
    assert_eq!(status, 200, "browser base failed: {browser}");
    let via = browser["base"].as_str().unwrap().to_string();

    // THE PROOF: drive the whole airlock protocol from Bob, over the overlay, to
    // Alice's enclave. Every hop is REMOTE (bob -> overlay -> alice -> loopback).
    let reply = rt.block_on(async {
        let gw = AirlockClient::remote("airlock.alice.duck".into(), via);
        let expected = Measurement::from_hex(&measurement_hex()).unwrap();

        // 1) attest OVER the overlay, verify the quote, read the attested seal_pk.
        let (quote, _vendor) = gw.fetch_quote().await.expect("fetch quote over overlay");
        let rd = airlock::verify::verify_quote(&quote, &expected, &test_enclave().roots())
            .await
            .unwrap();
        let seal_pk = attest::split_report_data(&rd).0;

        // 2) session-key handshake OVER the overlay → scoped token.
        let token = gw
            .open_session(&seal_pk, "compute-node")
            .await
            .expect("handshake over overlay");

        // 3) proxied /v1/messages with the session-token bearer (forwarded because
        //    the route set allow_authorization). The gateway swaps it for the real
        //    credential and the mock upstream replies AIRLOCK-OK.
        let resp = gw
            .route(gw.http().post(gw.url("/v1/messages")))
            .bearer_auth(&token)
            .header("anthropic-version", "2023-06-01")
            .header("anthropic-beta", "oauth-2025-04-20")
            .header("content-type", "application/json")
            .body(r#"{"model":"claude-sonnet-5","max_tokens":16,"messages":[{"role":"user","content":"hi"}]}"#)
            .send()
            .await
            .expect("proxied messages over overlay");
        let status = resp.status();
        let body = resp.text().await.unwrap();
        (status, body)
    });

    assert_eq!(reply.0, reqwest::StatusCode::OK, "overlay proxied call: {reply:?}");
    assert!(
        reply.1.contains("AIRLOCK-OK"),
        "the swapped credential's reply must return over the overlay: {reply:?}"
    );
}

/// Single-node self-serve: ONE node publishes `airlock.alice.duck` and reaches
/// its OWN route through its browser-gateway (publisher == self → `serve_current`,
/// no WireGuard peer needed). This exercises the whole airlock-over-gateway path
/// — route publish, `x-duck-authority` resolution, the browser door, the
/// `allow_authorization` bearer forward, `proxy_loopback`, and the airlock
/// attest+handshake+swap — MINUS the node-to-node overlay hop (which
/// `airlock_over_gateway_two_wireguard_nodes` covers, on a box where inline
/// 2-node WireGuard peers reliably). Runs green where the 2-node harness can't.
#[test]
fn airlock_single_node_self_serves_its_own_route() {
    let _serial = serial();
    let rt = Runtime::new().unwrap();

    let (gw_base, gw_port) = rt.block_on(boot_gateway_and_upstream());
    rt.block_on(async {
        let gw = AirlockClient::local(gw_base.clone());
        let (quote, _vendor) = gw.fetch_quote().await.unwrap();
        let expected = Measurement::from_hex(&measurement_hex()).unwrap();
        let rd = airlock::verify::verify_quote(&quote, &expected, &test_enclave().roots())
            .await
            .unwrap();
        let seal_pk = attest::split_report_data(&rd).0;
        gw.upload_sealed_credential(
            &seal_pk,
            &CredentialPayload::Refresh { refresh_token: "seed".into() },
        )
        .await
        .unwrap();
    });

    // One validator node; no peer, so "1 peer(s)" never prints — but the gateway
    // plane still binds and serves the node's own routes locally.
    let mut cluster = Cluster::new(&[0], &[0]);
    cluster.wireguard = true;
    cluster.wireguard_socket = true;
    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", READY);
    cluster.wait_marker(0, "converged app_hash=", READY);
    cluster.wait_marker(0, "gateway plane: overlay stream bound", READY);

    let alice = ed25519::PrivateKey::from_seed(42);
    let alice_node = Cluster::identity(0);
    cluster.submit(
        0,
        "identity",
        &identity::encode_msg(&IdentityMsg::BindNode {
            authorizer: bind_auth(&alice, &cluster.namespace, &alice_node),
        }),
    );
    poll_until("identity binding", FINALIZE, || {
        account_of_node(&cluster, 0, &alice_node)
            .filter(|account| account.account_id == alice.public_key().as_ref())
    });

    cluster.submit(
        0,
        "gateway",
        &gateway::encode_msg(&GatewayMsg::SetHandle {
            handle: Some("alice".into()),
        }),
    );
    poll_until("alice.duck resolution", FINALIZE, || {
        resolve_handle(&cluster, 0, "alice")
    });

    let workspace = cluster.workspace(0);
    let (ok, output) = cluster.run_verb(&[
        "gateway-route-bind",
        "--workspace",
        workspace.to_str().unwrap(),
        "--label",
        "airlock",
        "--port",
        &gw_port.to_string(),
    ]);
    assert!(ok, "airlock gateway port bind failed: {output}");

    cluster.submit(
        0,
        "gateway",
        &gateway::encode_msg(&signed_airlock_route(&alice, &cluster.namespace, &alice_node, 1)),
    );
    poll_until("airlock route revision 1", FINALIZE, || {
        (airlock_route_revision(&cluster, 0, alice.public_key().as_ref()) == Some(1)).then_some(())
    });

    let (status, browser) = cluster.http(0, "GET", "/v1/gateway/browser", None);
    assert_eq!(status, 200, "browser base failed: {browser}");
    let via = browser["base"].as_str().unwrap().to_string();

    let reply = rt.block_on(async {
        let gw = AirlockClient::remote("airlock.alice.duck".into(), via);
        let expected = Measurement::from_hex(&measurement_hex()).unwrap();
        let (quote, _vendor) = gw.fetch_quote().await.expect("fetch quote through the gateway");
        let rd = airlock::verify::verify_quote(&quote, &expected, &test_enclave().roots())
            .await
            .unwrap();
        let seal_pk = attest::split_report_data(&rd).0;
        let token = gw
            .open_session(&seal_pk, "self")
            .await
            .expect("handshake through the gateway");
        let resp = gw
            .route(gw.http().post(gw.url("/v1/messages")))
            .bearer_auth(&token)
            .header("anthropic-version", "2023-06-01")
            .header("anthropic-beta", "oauth-2025-04-20")
            .header("content-type", "application/json")
            .body(r#"{"model":"claude-sonnet-5","max_tokens":16,"messages":[{"role":"user","content":"hi"}]}"#)
            .send()
            .await
            .expect("proxied messages through the gateway");
        let status = resp.status();
        let body = resp.text().await.unwrap();
        (status, body)
    });

    assert_eq!(reply.0, reqwest::StatusCode::OK, "self-served proxied call: {reply:?}");
    assert!(
        reply.1.contains("AIRLOCK-OK"),
        "the swapped credential's reply must return: {reply:?}"
    );
}

/// The credential-provider node EMBEDS the airlock gateway itself: with
/// `DUCKTAPE_AIRLOCK_SERVE` set, the node runs the gateway in-process, registers
/// its port as the `airlock` route, and seeds the credential from the configured
/// file — no separate `airlock-gateway serve` / `gateway-route-bind` / seal steps.
/// The client then self-serves through it exactly as above. Proves the node embed.
///
/// The non-CVM half of the embed contract, runnable on any box WITHOUT
/// TDX/SNP silicon: a credential node whose embedded gateway cannot attest
/// must FAIL BOOT loudly — no "listening" claim, no route registration, no
/// half-alive node with a route pointing at a dead gateway.
#[test]
fn airlock_embed_without_tee_fails_boot_loudly() {
    let _serial = serial();
    let mut cluster = Cluster::new(&[0], &[0]);
    cluster.wireguard = true;
    cluster.wireguard_socket = true;
    cluster.env[0] = vec![
        ("DUCKTAPE_AIRLOCK_SERVE".into(), "1".into()),
        ("DUCKTAPE_AIRLOCK_SERVE_ATTEST".into(), "auto".into()),
    ];
    cluster.spawn(0);
    // The attest probe names the reason (configfs-tsm absent on a non-CVM box)…
    let output = cluster.wait_marker(0, "are we inside a TDX/SEV-SNP guest", READY);
    // …and the node must not have claimed the gateway is up before dying.
    assert!(
        !output.contains("airlock gateway listening"),
        "a node that cannot attest must not claim its gateway listens: {output}"
    );
    cluster.wait_exit(0, READY);
}

/// HARDWARE-ONLY since mock attestation was deleted: the embedded gateway
/// attests via configfs-tsm, so the node must run inside a TDX/SNP guest
/// (task #26). On SNP hardware set AIRLOCK_E2E_SNP_PRODUCT=milan|genoa|turin.
#[test]
#[ignore = "needs TDX/SNP silicon: the embedded gateway attests via configfs-tsm (task #26)"]
fn airlock_embedded_gateway_self_serves() {
    let _serial = serial();
    let rt = Runtime::new().unwrap();

    // A mock Anthropic upstream that accepts ONLY the seeded static bearer.
    let upstream = rt.block_on(async {
        use axum::response::IntoResponse as _;
        let app = Router::new().route(
            "/v1/messages",
            post(|headers: axum::http::HeaderMap| async move {
                let got = headers
                    .get(axum::http::header::AUTHORIZATION)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");
                if got != "Bearer test-access-token" {
                    return (axum::http::StatusCode::UNAUTHORIZED, format!("got {got:?}"))
                        .into_response();
                }
                ([("content-type", "text/event-stream")], "data: AIRLOCK-OK\n\n").into_response()
            }),
        );
        bind_and_serve(app).await
    });

    // The Claude credentials file the node seeds its static bearer from.
    let creds = tempfile::Builder::new().suffix(".json").tempfile().unwrap();
    std::fs::write(
        creds.path(),
        r#"{"claudeAiOauth":{"accessToken":"test-access-token"}}"#,
    )
    .unwrap();

    let mut cluster = Cluster::new(&[0], &[0]);
    cluster.wireguard = true;
    cluster.wireguard_socket = true;
    // The node runs the airlock gateway itself — no external serve/bind/seal.
    cluster.env[0] = vec![
        ("DUCKTAPE_AIRLOCK_SERVE".into(), "1".into()),
        ("DUCKTAPE_AIRLOCK_SERVE_ATTEST".into(), "auto".into()),
        ("DUCKTAPE_AIRLOCK_SERVE_ANTHROPIC_BASE".into(), upstream.clone()),
        (
            "DUCKTAPE_AIRLOCK_SERVE_CREDENTIALS".into(),
            creds.path().to_str().unwrap().into(),
        ),
        ("DUCKTAPE_AIRLOCK_SERVE_CRED_KIND".into(), "bearer".into()),
    ];
    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", READY);
    cluster.wait_marker(0, "converged app_hash=", READY);
    cluster.wait_marker(0, "gateway plane: overlay stream bound", READY);
    // the node's OWN airlock gateway came up + registered its route.
    cluster.wait_marker(0, "airlock gateway listening", READY);

    let alice = ed25519::PrivateKey::from_seed(42);
    let alice_node = Cluster::identity(0);
    cluster.submit(
        0,
        "identity",
        &identity::encode_msg(&IdentityMsg::BindNode {
            authorizer: bind_auth(&alice, &cluster.namespace, &alice_node),
        }),
    );
    poll_until("identity binding", FINALIZE, || {
        account_of_node(&cluster, 0, &alice_node)
            .filter(|account| account.account_id == alice.public_key().as_ref())
    });
    cluster.submit(
        0,
        "gateway",
        &gateway::encode_msg(&GatewayMsg::SetHandle {
            handle: Some("alice".into()),
        }),
    );
    poll_until("alice.duck resolution", FINALIZE, || {
        resolve_handle(&cluster, 0, "alice")
    });
    // Publish the route (the one manual, signed operator act). The node already
    // registered the port; this just signs the LoopbackHttp record.
    cluster.submit(
        0,
        "gateway",
        &gateway::encode_msg(&signed_airlock_route(&alice, &cluster.namespace, &alice_node, 1)),
    );
    poll_until("airlock route revision 1", FINALIZE, || {
        (airlock_route_revision(&cluster, 0, alice.public_key().as_ref()) == Some(1)).then_some(())
    });

    let (status, browser) = cluster.http(0, "GET", "/v1/gateway/browser", None);
    assert_eq!(status, 200, "browser base failed: {browser}");
    let via = browser["base"].as_str().unwrap().to_string();

    let reply = rt.block_on(async {
        let gw = AirlockClient::remote("airlock.alice.duck".into(), via);
        let (quote, vendor) = gw.fetch_quote().await.expect("fetch quote via embedded gateway");
        // TOFU on first hardware run: pin the measurement the quote itself
        // carries, then run the REAL vendor verify against pinned roots — the
        // signature chain (not the pin) is what this lane proves.
        let mode: airlock::attest::AttestMode = vendor.parse().expect("vendor mode");
        let (mrtd_hex, _) = airlock::verify::peek_measurement(mode, &quote).expect("peek");
        let expected = Measurement::from_hex(&mrtd_hex).unwrap();
        let roots = match mode {
            airlock::attest::AttestMode::Tdx => {
                airlock::verify::TrustRoots::Tdx(airlock::verify::TdxRoots { pccs_url: None })
            }
            airlock::attest::AttestMode::Snp => {
                let product = std::env::var("AIRLOCK_E2E_SNP_PRODUCT")
                    .expect("snp hardware: set AIRLOCK_E2E_SNP_PRODUCT=milan|genoa|turin")
                    .parse()
                    .expect("snp product");
                airlock::verify::TrustRoots::Snp(Box::new(
                    airlock::verify::SnpRoots::amd(product, airlock::verify::VcekSource::Kds)
                        .expect("snp roots"),
                ))
            }
        };
        let rd = airlock::verify::verify_quote(&quote, &expected, &roots)
            .await
            .expect("real vendor verify of the embedded gateway quote");
        let seal_pk = attest::split_report_data(&rd).0;
        let token = gw
            .open_session(&seal_pk, "self")
            .await
            .expect("handshake via embedded gateway");
        let resp = gw
            .route(gw.http().post(gw.url("/v1/messages")))
            .bearer_auth(&token)
            .header("anthropic-version", "2023-06-01")
            .header("anthropic-beta", "oauth-2025-04-20")
            .header("content-type", "application/json")
            .body(r#"{"model":"claude-sonnet-5","max_tokens":16,"messages":[{"role":"user","content":"hi"}]}"#)
            .send()
            .await
            .expect("proxied messages via embedded gateway");
        let status = resp.status();
        let body = resp.text().await.unwrap();
        (status, body)
    });

    assert_eq!(reply.0, reqwest::StatusCode::OK, "embedded-gateway proxied call: {reply:?}");
    assert!(
        reply.1.contains("AIRLOCK-OK"),
        "the node-embedded gateway must swap the seeded credential and reply: {reply:?}"
    );
    drop(creds); // keep the creds file alive until the node has read it
}
