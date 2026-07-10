//! DuckDNS over the real node stack: three OS processes, real consensus, the
//! userspace WireGuard/socket overlay, data-plane `Service::Web`, and DuckFS.
//! The requester is deliberately not a provider, so every successful byte has
//! crossed an authenticated remote-provider stream rather than the solo-node
//! fast path.

mod common;

use std::collections::BTreeMap;
use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use common::{Cluster, NetworkShapeCluster, poll_until, serial};
use duckdns::{DuckDnsName, DuckDnsQuery, DuckDnsReply, ResolvedService};
use duckfs_client::api::NodeApi;
use duckfs_client::checkout::{CheckoutOptions, checkout_with};
use duckfs_client::commit::commit;
use duckfs_client::http::HttpNode;
use tungstenite::client::IntoClientRequest as _;
use tungstenite::http::{HeaderValue, StatusCode};
use tungstenite::{Message, accept};

const READY: Duration = Duration::from_secs(90);
const FINALIZE: Duration = Duration::from_secs(60);

#[derive(Debug)]
struct HttpResponse {
    status: u16,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
}

struct WebSocketEcho {
    address: std::net::SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl WebSocketEcho {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind WebSocket target");
        let address = listener.local_addr().expect("WebSocket target address");
        listener
            .set_nonblocking(true)
            .expect("nonblocking WebSocket target");
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = Arc::clone(&stop);
        let thread = std::thread::spawn(move || {
            while !stop_thread.load(Ordering::Relaxed) {
                let stream = match listener.accept() {
                    Ok((stream, _)) => stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(20));
                        continue;
                    }
                    Err(_) => return,
                };
                if stop_thread.load(Ordering::Relaxed) {
                    return;
                }
                let Ok(mut socket) = accept(stream) else {
                    continue;
                };
                while let Ok(message) = socket.read() {
                    if message.is_close() {
                        let _ = socket.close(None);
                        break;
                    }
                    if socket.send(message).is_err() {
                        break;
                    }
                }
            }
        });
        Self {
            address,
            stop,
            thread: Some(thread),
        }
    }
}

impl Drop for WebSocketEcho {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(self.address);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn request(
    port: u16,
    method: &str,
    path: &str,
    host: &str,
    headers: &[(&str, &str)],
) -> HttpResponse {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("DuckDNS ingress connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .expect("DuckDNS ingress read timeout");
    request_on(&mut stream, method, path, host, headers, "close")
}

fn request_on(
    stream: &mut TcpStream,
    method: &str,
    path: &str,
    host: &str,
    headers: &[(&str, &str)],
    connection: &str,
) -> HttpResponse {
    let mut request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nContent-Length: 0\r\nConnection: {connection}\r\n"
    );
    for (name, value) in headers {
        request.push_str(name);
        request.push_str(": ");
        request.push_str(value);
        request.push_str("\r\n");
    }
    request.push_str("\r\n");
    stream
        .write_all(request.as_bytes())
        .expect("DuckDNS ingress request");
    let mut raw = Vec::new();
    loop {
        let mut chunk = [0u8; 16 * 1024];
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => raw.extend_from_slice(&chunk[..read]),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                break;
            }
            Err(error) => panic!("DuckDNS ingress response: {error}"),
        }
        let Some(split) = raw.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let head = String::from_utf8_lossy(&raw[..split]);
        let status = head
            .split_whitespace()
            .nth(1)
            .and_then(|value| value.parse::<u16>().ok());
        let content_length = head.lines().skip(1).find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        });
        if matches!(status, Some(100..=199 | 204 | 304))
            || content_length.is_some_and(|length| raw.len() >= split + 4 + length)
        {
            break;
        }
    }
    let Some(split) = raw.windows(4).position(|window| window == b"\r\n\r\n") else {
        return HttpResponse {
            status: 0,
            headers: BTreeMap::new(),
            body: raw,
        };
    };
    let head = String::from_utf8(raw[..split].to_vec()).expect("ASCII HTTP response head");
    let status = head
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse().ok())
        .expect("HTTP response status");
    let headers = head
        .lines()
        .skip(1)
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    HttpResponse {
        status,
        headers,
        body: raw[split + 4..].to_vec(),
    }
}

fn resolve(cluster: &Cluster, name: DuckDnsName) -> Option<ResolvedService> {
    let bytes = cluster.query(
        0,
        "duckdns",
        &duckdns::encode_query(&DuckDnsQuery::Resolve { name }),
    )?;
    match duckdns::decode_reply(&bytes).ok()? {
        DuckDnsReply::Resolved(service) => service,
        _ => None,
    }
}

fn resolve_network_shape(
    cluster: &NetworkShapeCluster,
    name: DuckDnsName,
) -> Option<ResolvedService> {
    resolve_network_shape_at(cluster, 0, name)
}

fn resolve_network_shape_at(
    cluster: &NetworkShapeCluster,
    idx: usize,
    name: DuckDnsName,
) -> Option<ResolvedService> {
    let bytes = cluster.query(
        idx,
        "duckdns",
        &duckdns::encode_query(&DuckDnsQuery::Resolve { name }),
    )?;
    match duckdns::decode_reply(&bytes).ok()? {
        DuckDnsReply::Resolved(service) => service,
        _ => None,
    }
}

fn wait_head(cluster: &Cluster, idx: usize, snapshot: &str) {
    let node = HttpNode::new(cluster.http_base(idx));
    poll_until(
        &format!("node {idx} to finalize DuckFS snapshot {snapshot}"),
        FINALIZE,
        || {
            node.refs()
                .ok()
                .and_then(|refs| refs.head)
                .filter(|head| head == snapshot)
                .map(|_| ())
        },
    );
}

fn provider_order_key(hostname: &str, node: &[u8]) -> u64 {
    let mut preimage = Vec::with_capacity(hostname.len() + node.len());
    preimage.extend_from_slice(hostname.as_bytes());
    preimage.extend_from_slice(node);
    data_plane::FlowId::derive(&preimage).as_u64()
}

#[test]
fn remote_duckfs_site_streams_and_fails_over_across_real_nodes() {
    let _serial = serial();
    let websocket_target = WebSocketEcho::start();
    let ports = common::alloc_ports(6);
    let wireguard = &ports[..3];
    let ingress = &ports[3..];
    let mut cluster = Cluster::new(&[0, 1, 2], &[0, 1, 2]);

    cluster.extra_toml_by_node[0].push(format!(
        "wireguard_listen = \"127.0.0.1:{}\"\n\
         wireguard_effect = \"socket\"\n\
         [duckdns]\n\
         ingress_listen = \"127.0.0.1:{}\"",
        wireguard[0], ingress[0]
    ));
    for (idx, wireguard_port) in wireguard.iter().enumerate().skip(1) {
        let websocket = if idx == 1 {
            format!(
                "\n[[duckdns.services]]\n\
                 scope = \"network\"\n\
                 service = \"echo\"\n\
                 target = \"{}\"",
                websocket_target.address
            )
        } else {
            String::new()
        };
        cluster.extra_toml_by_node[idx].push(format!(
            "wireguard_listen = \"127.0.0.1:{}\"\n\
             wireguard_effect = \"socket\"\n\
             [duckdns]\n\
             [[duckdns.services]]\n\
             scope = \"network\"\n\
             service = \"docs\"\n\
             [duckdns.services.duckfs]\n\
             prefix = \"/shared/sites/docs\"\n\
             [[duckdns.services]]\n\
             scope = \"network\"\n\
             service = \"corsdocs\"\n\
             allow_cross_site = true\n\
             [duckdns.services.duckfs]\n\
             prefix = \"/shared/sites/docs\"{}",
            wireguard_port, websocket
        ));
    }

    for idx in 0..3 {
        cluster.spawn(idx);
    }
    for idx in 0..3 {
        cluster.wait_marker(idx, "rpc listening on", READY);
        cluster.wait_marker(idx, "DuckDNS web plane bound", READY);
    }
    for idx in 0..3 {
        cluster.wait_marker(idx, "converged app_hash=", READY);
    }
    poll_until("validator app hashes to converge", READY, || {
        let hashes: Vec<_> = (0..3)
            .map(|idx| cluster.status(idx)["app_hash"].clone())
            .collect();
        (hashes[0] == hashes[1] && hashes[0] == hashes[2]).then_some(())
    });

    // Upload through the shipping DuckFS client. The large file forces three
    // module reads while the provider streams one immutable snapshot.
    let node = HttpNode::new(cluster.http_base(0));
    poll_until("requester files surface", READY, || {
        node.refs().ok().map(|_| ())
    });
    let checkout = tempfile::tempdir().expect("DuckFS site checkout");
    let options = CheckoutOptions {
        node_url: cluster.http_base(0),
        ..Default::default()
    };
    checkout_with(&node, checkout.path(), "/shared/sites/docs", None, &options)
        .expect("checkout empty DuckDNS site");
    std::fs::write(
        checkout.path().join("index.html"),
        b"<!doctype html><title>remote duckdns</title><h1>quack</h1>",
    )
    .expect("write site index");
    let large: Vec<u8> = (0..(2 * 1024 * 1024 + 17))
        .map(|offset| (offset % 251) as u8)
        .collect();
    std::fs::write(checkout.path().join("large.bin"), &large).expect("write large site asset");
    let committed =
        commit(&node, checkout.path(), "publish remote DuckDNS site").expect("commit DuckDNS site");
    wait_head(&cluster, 1, &committed.snapshot);
    wait_head(&cluster, 2, &committed.snapshot);

    let chain = duckdns::derive_chain_label(&format!("{}#00000000", cluster.namespace))
        .expect("dev chain label");
    let logical_name = DuckDnsName::NetworkService {
        service: "docs".into(),
        chain: chain.clone(),
    };
    let hostname = logical_name.hostname();
    let resolved = poll_until("both remote DuckDNS announcements", FINALIZE, || {
        resolve(&cluster, logical_name.clone()).filter(|service| service.providers.len() == 2)
    });
    assert!(
        resolved
            .providers
            .iter()
            .all(|provider| provider.node != Cluster::identity(0)),
        "the requester must not be able to take the self-provider fast path"
    );
    let root = poll_until("remote DuckFS homepage over the web plane", READY, || {
        let response = request(ingress[0], "GET", "/", &hostname, &[]);
        (response.status == 200).then_some(response)
    });
    assert_eq!(
        root.body,
        b"<!doctype html><title>remote duckdns</title><h1>quack</h1>"
    );
    assert_eq!(
        root.headers.get("content-type").map(String::as_str),
        Some("text/html")
    );
    let etag = root.headers.get("etag").expect("DuckFS ETag").clone();
    let cached = request(
        ingress[0],
        "GET",
        "/",
        &hostname,
        &[("If-None-Match", &etag)],
    );
    assert_eq!(cached.status, 304);
    assert!(cached.body.is_empty());

    let streamed = request(ingress[0], "GET", "/large.bin", &hostname, &[]);
    assert_eq!(streamed.status, 200);
    assert_eq!(streamed.body, large, "multi-chunk body crossed byte-exact");

    let cross_site = request(
        ingress[0],
        "POST",
        "/",
        &hostname,
        &[("Sec-Fetch-Site", "cross-site")],
    );
    assert_eq!(cross_site.status, 403);

    // Policy is evaluated for every request on one persistent downstream
    // connection, not just its first safe request.
    let mut keep_alive =
        TcpStream::connect(("127.0.0.1", ingress[0])).expect("DuckDNS keep-alive ingress");
    keep_alive
        .set_read_timeout(Some(Duration::from_secs(20)))
        .expect("DuckDNS keep-alive read timeout");
    assert_eq!(
        request_on(&mut keep_alive, "GET", "/", &hostname, &[], "keep-alive").status,
        200
    );
    assert_eq!(
        request_on(
            &mut keep_alive,
            "POST",
            "/",
            &hostname,
            &[("Sec-Fetch-Site", "cross-site")],
            "close",
        )
        .status,
        403,
        "a safe first request must not launder an unsafe second request"
    );

    let cors_name = DuckDnsName::NetworkService {
        service: "corsdocs".into(),
        chain: chain.clone(),
    };
    poll_until("cross-site opt-in announcement", FINALIZE, || {
        resolve(&cluster, cors_name.clone()).filter(|service| service.providers.len() == 2)
    });
    let allowed = request(
        ingress[0],
        "POST",
        "/",
        &cors_name.hostname(),
        &[("Sec-Fetch-Site", "cross-site")],
    );
    assert_eq!(
        allowed.status, 405,
        "opted-in cross-site POST reached the read-only DuckFS service"
    );

    for (method, path, expected) in [
        ("GET", "/v1/status", 404),
        ("POST", "/v1/shutdown", 405),
        ("POST", "/v1/query", 405),
        ("GET", "/metrics", 404),
    ] {
        let admin = request(ingress[0], method, path, &hostname, &[]);
        assert_eq!(
            admin.status, expected,
            "the node administration route {method} {path} is unreachable"
        );
    }
    assert!(
        cluster.status(0).get("app_hash").is_some(),
        "the shutdown administration route did not reach the node router"
    );
    let unpublished = DuckDnsName::NetworkService {
        service: "admin".into(),
        chain: chain.clone(),
    }
    .hostname();
    assert_eq!(
        request(ingress[0], "GET", "/", &unpublished, &[]).status,
        404,
        "an undeclared service identity cannot select an arbitrary local port"
    );

    let echo_name = DuckDnsName::NetworkService {
        service: "echo".into(),
        chain: chain.clone(),
    };
    poll_until("remote WebSocket announcement", FINALIZE, || {
        resolve(&cluster, echo_name.clone()).filter(|service| service.providers.len() == 1)
    });
    let echo_hostname = echo_name.hostname();
    let mut websocket_request = format!("ws://{echo_hostname}/echo")
        .into_client_request()
        .expect("WebSocket request");
    websocket_request.headers_mut().insert(
        "Origin",
        HeaderValue::from_str(&format!("https://{echo_hostname}")).unwrap(),
    );
    let stream = TcpStream::connect(("127.0.0.1", ingress[0])).expect("WebSocket ingress");
    let (mut websocket, response) =
        tungstenite::client(websocket_request, stream).expect("remote DuckDNS WebSocket handshake");
    assert_eq!(response.status(), StatusCode::SWITCHING_PROTOCOLS);
    websocket
        .send(Message::Text("remote duckdns websocket".into()))
        .expect("send remote WebSocket frame");
    assert_eq!(
        websocket.read().expect("remote WebSocket echo"),
        Message::Text("remote duckdns websocket".into())
    );
    websocket.close(None).expect("close remote WebSocket");

    let mut bad_origin = format!("ws://{echo_hostname}/echo")
        .into_client_request()
        .expect("bad-origin WebSocket request");
    bad_origin
        .headers_mut()
        .insert("Origin", HeaderValue::from_static("https://evil.example"));
    let stream = TcpStream::connect(("127.0.0.1", ingress[0])).expect("WebSocket ingress");
    match tungstenite::client(bad_origin, stream) {
        Err(tungstenite::HandshakeError::Failure(tungstenite::Error::Http(response)))
            if response.status() == StatusCode::FORBIDDEN => {}
        other => panic!("unexpected bad-Origin result: {other:?}"),
    }

    // Prove both canonical provider names independently reach their remote
    // process before injecting a fault.
    for provider in &resolved.providers {
        let qualified = DuckDnsName::NodeService {
            service: "docs".into(),
            node: provider.node_label.clone(),
            chain: chain.clone(),
        }
        .hostname();
        let response = request(ingress[0], "GET", "/", &qualified, &[]);
        assert_eq!(response.status, 200, "node-qualified provider must answer");
    }

    // Kill whichever provider deterministic shuffling chooses first. The
    // registry intentionally still contains its declaration, so success can
    // only come from pre-request failover to the second provider.
    let first = resolved
        .providers
        .iter()
        .min_by_key(|provider| provider_order_key(&hostname, &provider.node))
        .expect("provider pool")
        .clone();
    let dead_idx = [1usize, 2]
        .into_iter()
        .find(|idx| Cluster::identity(cluster.peer_ids[*idx]) == first.node)
        .expect("selected provider belongs to this cluster");
    cluster.kill(dead_idx);

    let failed_over = poll_until("logical service to fail over", READY, || {
        let response = request(ingress[0], "GET", "/", &hostname, &[]);
        (response.status == 200).then_some(response)
    });
    assert_eq!(failed_over.body, root.body);

    let dead_name = DuckDnsName::NodeService {
        service: "docs".into(),
        node: first.node_label,
        chain,
    }
    .hostname();
    let pinned = request(ingress[0], "GET", "/", &dead_name, &[]);
    assert_eq!(pinned.status, 502, "node-qualified names never fail over");
}

#[test]
fn validator_and_resident_serve_each_other_then_revocation_cuts_publication() {
    let _serial = serial();
    let ports = common::alloc_ports(4);
    let mut cluster = NetworkShapeCluster::new();

    // Seed node-local plumbing before the product init/join verbs. Their
    // existing merge/write path preserves these declarations while replacing
    // this minimal seed with the canonical network-shape node.toml.
    std::fs::create_dir_all(&cluster.founder_dir).expect("founder dir");
    std::fs::write(
        cluster.founder_dir.join("node.toml"),
        format!(
            "listen = \"127.0.0.1:{}\"\n\
             wireguard_listen = \"127.0.0.1:{}\"\n\
             wireguard_effect = \"socket\"\n\
             [duckdns]\n\
             ingress_listen = \"127.0.0.1:{}\"\n\
             [[duckdns.services]]\n\
             scope = \"network\"\n\
             service = \"member\"\n\
             [duckdns.services.duckfs]\n\
             prefix = \"/shared/sites/member\"\n",
            cluster.p2p_ports[0], ports[0], ports[2]
        ),
    )
    .expect("founder seed config");
    let chain_id = cluster.init_founder("duckdns-resident");
    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", READY);
    cluster.wait_marker(0, "DuckDNS web plane bound", READY);

    let founder = HttpNode::new(format!("http://127.0.0.1:{}", cluster.http_ports[0]));
    poll_until("founder files surface", READY, || {
        founder.refs().ok().map(|_| ())
    });
    let checkout = tempfile::tempdir().expect("member site checkout");
    let options = CheckoutOptions {
        node_url: format!("http://127.0.0.1:{}", cluster.http_ports[0]),
        ..Default::default()
    };
    checkout_with(
        &founder,
        checkout.path(),
        "/shared/sites/member",
        None,
        &options,
    )
    .expect("checkout member site");
    std::fs::write(
        checkout.path().join("index.html"),
        b"<!doctype html><h1>member site</h1>",
    )
    .expect("write member site");
    commit(&founder, checkout.path(), "publish member site").expect("commit member site");

    let invite = cluster.invite();
    std::fs::create_dir_all(&cluster.friend_dir).expect("friend dir");
    std::fs::write(
        cluster.friend_dir.join("node.toml"),
        format!(
            "listen = \"127.0.0.1:{}\"\n\
             wireguard_listen = \"127.0.0.1:{}\"\n\
             wireguard_effect = \"socket\"\n\
             [duckdns]\n\
             ingress_listen = \"127.0.0.1:{}\"\n\
             [[duckdns.services]]\n\
             scope = \"network\"\n\
             service = \"resident\"\n\
             [duckdns.services.duckfs]\n\
             prefix = \"/shared/sites/member\"\n",
            cluster.p2p_ports[1], ports[1], ports[3]
        ),
    )
    .expect("friend seed config");
    let friend_key = cluster.join_friend(&invite);
    cluster.spawn(1);
    cluster.wait_marker(1, "resident: standing granted", READY);
    cluster.wait_marker(1, "resident: pre-synced boundary", READY);
    cluster.wait_marker(1, "DuckDNS web plane bound", READY);
    cluster.wait_marker(1, "resident: announced DuckDNS services", READY);

    let chain = duckdns::derive_chain_label(&chain_id).expect("network chain label");
    let member_name = DuckDnsName::NetworkService {
        service: "member".into(),
        chain: chain.clone(),
    };
    let resident_name = DuckDnsName::NetworkService {
        service: "resident".into(),
        chain,
    };
    poll_until("validator and resident declarations", FINALIZE, || {
        let member = resolve_network_shape(&cluster, member_name.clone())?;
        let resident = resolve_network_shape(&cluster, resident_name.clone())?;
        let resident_member = resolve_network_shape_at(&cluster, 1, member_name.clone())?;
        let resident_resident = resolve_network_shape_at(&cluster, 1, resident_name.clone())?;
        (member.providers.len() == 1
            && resident.providers.len() == 1
            && resident_member.providers.len() == 1
            && resident_resident.providers.len() == 1)
            .then_some(())
    });

    let from_validator = poll_until("validator to resident DuckDNS stream", READY, || {
        let response = request(ports[2], "GET", "/", &resident_name.hostname(), &[]);
        (response.status == 200).then_some(response)
    });
    assert_eq!(from_validator.body, b"<!doctype html><h1>member site</h1>");
    let from_resident = poll_until("resident to validator DuckDNS stream", READY, || {
        let response = request(ports[3], "GET", "/", &member_name.hostname(), &[]);
        (response.status == 200).then_some(response)
    });
    assert_eq!(from_resident.body, from_validator.body);

    let (ok, output) = cluster.run_membership_verb("resident-remove", &friend_key);
    assert!(ok, "resident-remove failed:\n{output}");
    cluster.wait_marker(1, "joining: awaiting redemption", READY);
    poll_until("revoked resident declaration to disappear", READY, || {
        resolve_network_shape(&cluster, resident_name.clone())
            .is_none()
            .then_some(())
    });
    let removed = request(ports[2], "GET", "/", &resident_name.hostname(), &[]);
    assert_eq!(
        removed.status, 404,
        "revoked provider is no longer published"
    );
    let refused = poll_until("revoked resident requester to be refused", READY, || {
        let response = request(ports[3], "GET", "/", &member_name.hostname(), &[]);
        (response.status == 403).then_some(response)
    });
    assert_eq!(refused.status, 403, "an outsider cannot request DuckDNS");
}
