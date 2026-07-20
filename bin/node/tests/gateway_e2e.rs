//! Process-level gateway proof over two real, TUN-less WireGuard nodes.
//!
//! Alice publishes `api.alice.duck` to one loopback HTTP server. Bob resolves
//! the finalized route, sends POST and browser traffic over the authenticated
//! userspace WireGuard stream, and is identified to Alice by node/account.
//! The same live cluster proves that stale revisions, undeclared methods,
//! ambient credentials, cross-origin browser calls, and owner-only policy fail
//! before reaching loopback.

mod common;

use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use base64::Engine as _;
use common::{Cluster, hex, poll_until, serial};
use commonware_cryptography::{Signer as _, ed25519};
use gateway::{
    DuckDnsName, GatewayMsg, GatewayQuery, GatewayReply, MemberAuthorization, RouteAudience,
    RouteDefinition, RouteMethod, RouteName, RoutePolicy, RouteStatement, RouteTarget,
};
use identity::{AccountView, IdentityMsg, IdentityQuery, IdentityReply, MemberAuth};

const READY: Duration = Duration::from_secs(180);
const FINALIZE: Duration = Duration::from_secs(60);

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

fn resolve_alice(cluster: &Cluster, reader: usize) -> Option<Vec<u8>> {
    let bytes = cluster.query(
        reader,
        "gateway",
        &gateway::encode_query(&GatewayQuery::Resolve {
            name: DuckDnsName {
                handle: "alice".into(),
            },
        }),
    )?;
    match gateway::decode_reply(&bytes).ok()? {
        GatewayReply::Resolved(Some(account)) => Some(account.account_id),
        _ => None,
    }
}

fn signed_route(
    member: &ed25519::PrivateKey,
    chain: &str,
    publisher: &[u8],
    revision: u64,
    audience: RouteAudience,
) -> GatewayMsg {
    let statement = RouteStatement {
        version: 1,
        chain_id: chain.into(),
        account_id: member.public_key().as_ref().to_vec(),
        name: RouteName::named("api"),
        publisher_node: publisher.to_vec(),
        revision,
        route: Some(RouteDefinition {
            target: RouteTarget::LoopbackHttp,
            policy: RoutePolicy {
                audience,
                methods: vec![RouteMethod::Get, RouteMethod::Head, RouteMethod::Post],
                max_request_bytes: 1024,
                max_response_bytes: 4096,
                allow_authorization: false,
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

fn route_revision(cluster: &Cluster, reader: usize) -> Option<u64> {
    let bytes = cluster.query(
        reader,
        "gateway",
        &gateway::encode_query(&GatewayQuery::Get {
            account_id: ed25519::PrivateKey::from_seed(42)
                .public_key()
                .as_ref()
                .to_vec(),
            name: RouteName::named("api"),
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

fn read_http_request(stream: &mut TcpStream) -> Vec<u8> {
    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .unwrap();
    let mut request = Vec::new();
    let mut chunk = [0u8; 2048];
    loop {
        let count = stream.read(&mut chunk).expect("read loopback request");
        assert!(count > 0, "loopback request closed before its body");
        request.extend_from_slice(&chunk[..count]);
        let Some(header_end) = request.windows(4).position(|part| part == b"\r\n\r\n") else {
            continue;
        };
        let headers = String::from_utf8_lossy(&request[..header_end + 4]);
        let content_length = headers
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("content-length:")
                    .and_then(|value| value.trim().parse::<usize>().ok())
            })
            .unwrap_or(0);
        if request.len() >= header_end + 4 + content_length {
            return request;
        }
    }
}

fn spawn_loopback(
    alice_node: Vec<u8>,
    bob_node: Vec<u8>,
    bob_account: Vec<u8>,
) -> (u16, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind Alice loopback");
    let port = listener.local_addr().unwrap().port();
    let handle = thread::spawn(move || {
        for expected in ["POST /items HTTP/1.1", "GET /page HTTP/1.1"] {
            let (mut stream, _) = listener.accept().expect("accept gateway proxy");
            let raw = read_http_request(&mut stream);
            let text = String::from_utf8_lossy(&raw);
            assert!(
                text.starts_with(expected),
                "unexpected upstream request:\n{text}"
            );
            let lower = text.to_ascii_lowercase();
            assert!(lower.contains(&format!("x-duck-caller-account: {}", hex(&bob_account))));
            assert!(lower.contains(&format!("x-duck-caller-node: {}", hex(&bob_node))));
            assert!(lower.contains(&format!(
                "x-duck-route-account: {}",
                hex(ed25519::PrivateKey::from_seed(42).public_key().as_ref())
            )));
            assert!(lower.contains("x-duck-route-label: api"));
            assert!(!lower.contains("cookie:"));
            assert!(!lower.contains("authorization:"));
            assert!(!lower.contains("x-forwarded-"));
            assert_ne!(alice_node, bob_node);
            if expected.starts_with("POST") {
                assert!(text.ends_with("{\"name\":\"duck\"}"));
                stream
                    .write_all(
                        b"HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nSet-Cookie: secret=nope\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}",
                    )
                    .unwrap();
            } else {
                let body = b"<!doctype html><title>Alice</title><script>document.body.dataset.ok='yes'</script>";
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nSet-Cookie: secret=nope\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(head.as_bytes()).unwrap();
                stream.write_all(body).unwrap();
            }
        }
    });
    (port, handle)
}

fn proxy_request(
    cluster: &Cluster,
    revision: u64,
    method: &str,
    path: &str,
    headers: serde_json::Value,
    body: &[u8],
) -> (u16, serde_json::Value) {
    cluster.http(
        1,
        "POST",
        "/v1/gateway/proxy",
        Some(&serde_json::json!({
            "head": {
                "account_id": ed25519::PrivateKey::from_seed(42).public_key().as_ref(),
                "name": { "label": "api" },
                "revision": revision,
                "method": method,
                "path_and_query": path,
                "headers": headers,
                "body_len": body.len(),
            },
            "bodyB64": base64::engine::general_purpose::STANDARD.encode(body),
        })),
    )
}

fn raw_browser_request(port: u16, authority: &str, extra: &str) -> (u16, String, Vec<u8>) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect browser gateway");
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();
    // The duck:// scheme handler forwards the page's authority and origin; there
    // is no <token>.localhost Host any more.
    let request = format!(
        "GET /page HTTP/1.1\r\nHost: 127.0.0.1\r\nX-Duck-Authority: {authority}\r\nOrigin: duck://{authority}\r\nSec-Fetch-Site: same-origin\r\n{extra}Connection: close\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).unwrap();
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).unwrap();
    let split = raw
        .windows(4)
        .position(|part| part == b"\r\n\r\n")
        .expect("browser response headers");
    let headers = String::from_utf8_lossy(&raw[..split + 4]).into_owned();
    let status = headers
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    (status, headers, raw[split + 4..].to_vec())
}

#[test]
fn gateway_runs_over_inline_wireguard_and_fails_closed() {
    let _serial = serial();
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

    cluster.submit(
        0,
        "gateway",
        &gateway::encode_msg(&GatewayMsg::SetHandle {
            handle: Some("alice".into()),
        }),
    );
    for reader in 0..2 {
        let resolved = poll_until("alice.duck resolution", FINALIZE, || {
            resolve_alice(&cluster, reader)
        });
        assert_eq!(resolved, alice.public_key().as_ref());
    }

    let (loopback_port, upstream) = spawn_loopback(
        alice_node.clone(),
        bob_node.clone(),
        bob.public_key().as_ref().to_vec(),
    );
    let workspace = cluster.workspace(0);
    let (ok, output) = cluster.run_verb(&[
        "gateway-route-bind",
        "--workspace",
        workspace.to_str().unwrap(),
        "--label",
        "api",
        "--port",
        &loopback_port.to_string(),
    ]);
    assert!(ok, "local gateway bind failed: {output}");

    cluster.submit(
        0,
        "gateway",
        &gateway::encode_msg(&signed_route(
            &alice,
            &cluster.namespace,
            &alice_node,
            1,
            RouteAudience::Network,
        )),
    );
    poll_until("gateway route revision 1", FINALIZE, || {
        (route_revision(&cluster, 1) == Some(1)).then_some(())
    });

    let body = br#"{"name":"duck"}"#;
    let (status, response) = proxy_request(
        &cluster,
        1,
        "post",
        "/items",
        serde_json::json!([{ "name": "content-type", "value": "application/json" }]),
        body,
    );
    assert_eq!(status, 200, "gateway POST failed: {response}");
    assert_eq!(response["head"]["status"], 201);
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(response["bodyB64"].as_str().unwrap())
            .unwrap(),
        br#"{"ok":true}"#
    );
    // v2 forwards Set-Cookie end to end (v1 stripped it).
    assert!(
        response["head"]["headers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|header| header["name"] == "set-cookie")
    );

    // Discover the dedicated browser-gateway port, then browse api.alice.duck
    // exactly as the duck:// scheme handler would: the node resolves the
    // authority (duckdns + route) fresh, with no session token.
    let (status, browser) = cluster.http(1, "GET", "/v1/gateway/browser", None);
    assert_eq!(status, 200, "browser base failed: {browser}");
    let browser_port: u16 = browser["base"]
        .as_str()
        .unwrap()
        .rsplit_once(':')
        .unwrap()
        .1
        .parse()
        .unwrap();
    let authority = "api.alice.duck";
    let (status, headers, html) = raw_browser_request(browser_port, authority, "");
    assert_eq!(status, 200, "browser gateway failed: {headers}");
    let lower_headers = headers.to_ascii_lowercase();
    assert!(lower_headers.contains("content-security-policy:"));
    assert!(lower_headers.contains(&format!("connect-src duck://{authority}")));
    assert!(lower_headers.contains("worker-src 'none'"));
    assert!(lower_headers.contains("webrtc 'block'"));
    assert!(!lower_headers.contains("set-cookie:"));
    assert!(String::from_utf8_lossy(&html).contains("<title>Alice</title>"));

    // A page whose Origin does not match the forwarded authority is rejected.
    let mut cross = TcpStream::connect(("127.0.0.1", browser_port)).unwrap();
    let request = format!(
        "GET /page HTTP/1.1\r\nHost: 127.0.0.1\r\nX-Duck-Authority: {authority}\r\nOrigin: duck://evil.alice.duck\r\nConnection: close\r\n\r\n"
    );
    cross.write_all(request.as_bytes()).unwrap();
    let mut raw = String::new();
    cross.read_to_string(&mut raw).unwrap();
    assert!(
        raw.starts_with("HTTP/1.1 403"),
        "cross-origin response: {raw}"
    );
    upstream.join().unwrap();

    for (method, path, headers, expected) in [
        ("delete", "/items", serde_json::json!([]), 403),
        // (v2: Cookie is no longer rejected at the proxy; it flows to upstream.)
        (
            "get",
            "/items",
            serde_json::json!([{ "name": "authorization", "value": "Bearer secret" }]),
            403,
        ),
        ("get", "http://127.0.0.1:9/", serde_json::json!([]), 400),
    ] {
        let (status, _) = proxy_request(&cluster, 1, method, path, headers, &[]);
        assert_eq!(
            status, expected,
            "unexpected policy result for {method} {path}"
        );
    }

    cluster.submit(
        0,
        "gateway",
        &gateway::encode_msg(&signed_route(
            &alice,
            &cluster.namespace,
            &alice_node,
            2,
            RouteAudience::Owner,
        )),
    );
    poll_until("gateway route revision 2", FINALIZE, || {
        (route_revision(&cluster, 1) == Some(2)).then_some(())
    });
    let (status, _) = proxy_request(&cluster, 1, "get", "/items", serde_json::json!([]), &[]);
    assert_eq!(status, 409, "stale revision must conflict");
    let (status, response) =
        proxy_request(&cluster, 2, "get", "/items", serde_json::json!([]), &[]);
    assert_eq!(status, 403, "owner-only audience leaked: {response}");
}
