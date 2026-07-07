//! the ADR phase-1 loopback pair proof: two (three, for the replace case)
//! userspace backends on 127.0.0.1-class loopback underlay, driven ONLY
//! through the `WireGuardEffect` boundary — handshake, datagram echo through
//! the virtual stack, TCP dial/listen through the tunnel, forced rekey,
//! session preservation across an identical re-apply, and the atomic peer
//! replace.
//!
//! everything here runs unprivileged: no TUN, no CAP_NET_ADMIN, no external
//! binaries — the property the whole ADR exists to win.

use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use defguard_wireguard_rs::{InterfaceConfiguration, key::Key, net::IpAddrMask, peer::Peer};
use overlay_net::userspace::UserspaceWireGuardEffect;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use wireguard_effect::WireGuardEffect;

/// a fixture chain /48 (the shape `ula_v6_prefix` mints) with per-node
/// member /128s.
fn ula(host: u16) -> Ipv6Addr {
    Ipv6Addr::new(0xfda2, 0x8ad3, 0xeaee, 0, 0, 0, 0, host)
}

/// one node of the pair: its effect (the only handle the test drives) plus
/// the identity facts a peer needs to know it.
struct Node {
    effect: UserspaceWireGuardEffect,
    secret: Key,
    ula: Ipv6Addr,
    /// the loopback underlay endpoint of the node's bound WG socket.
    endpoint: SocketAddr,
}

fn peer_entry(of: &Node, endpoint: Option<SocketAddr>) -> Peer {
    let mut peer = Peer::new(of.secret.public_key());
    peer.endpoint = endpoint;
    peer.set_allowed_ips(vec![IpAddrMask::new(IpAddr::V6(of.ula), 128)]);
    peer
}

fn config(node: &Node, port: u16, peers: Vec<Peer>) -> InterfaceConfiguration {
    InterfaceConfiguration {
        name: "dt-loopback".into(),
        prvkey: node.secret.to_string(),
        addresses: vec![IpAddrMask::new(IpAddr::V6(node.ula), 128)],
        port,
        peers,
        mtu: None,
        fwmark: None,
    }
}

/// stand a node up through the effect boundary: create + first apply with an
/// empty peer set and port 0 (the OS allocates), so nodes can learn each
/// other's real underlay ports before the peered re-apply.
fn stand_up(key_seed: u8, host: u16) -> Node {
    let secret = Key::new(
        defguard_boringtun_secret(key_seed), // clamped X25519 scalar bytes
    );
    let mut node = Node {
        effect: UserspaceWireGuardEffect::new(tokio::runtime::Handle::current()),
        secret,
        ula: ula(host),
        endpoint: SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0),
    };
    node.effect.create_interface().expect("create");
    node.effect
        .apply(&config(&node, 0, Vec::new()))
        .expect("first apply binds the underlay");
    let bound = node.effect.local_underlay_addr().expect("underlay bound");
    node.endpoint.set_port(bound.port());
    node
}

/// derive a deterministic private key from a seed byte. any 32 bytes are a
/// valid X25519 secret (the curve clamps), so a filled array is fine for a
/// fixture — but each node needs a distinct one.
fn defguard_boringtun_secret(seed: u8) -> [u8; 32] {
    let mut bytes = [seed; 32];
    bytes[0] = seed.wrapping_add(1); // avoid the all-equal degenerate look
    bytes
}

/// peer `a` and `b`: `a` knows `b`'s endpoint; `b` runs the passive side
/// (NO endpoint for `a`) so the pair also proves endpoint learning from the
/// first authenticated inbound datagram — the zero-config joiner shape.
fn peer_up(a: &mut Node, b: &mut Node) {
    let a_port = a.endpoint.port();
    let b_port = b.endpoint.port();
    let peers_for_a = vec![peer_entry(b, Some(b.endpoint))];
    let peers_for_b = vec![peer_entry(a, None)];
    a.effect
        .apply(&config(a, a_port, peers_for_a))
        .expect("peered re-apply on a");
    b.effect
        .apply(&config(b, b_port, peers_for_b))
        .expect("peered re-apply on b");
}

const ECHO_PORT: u16 = 7777;

/// one datagram round trip a → b → a through both virtual stacks (and both
/// tunnels). the echo payload proves the data path; the returned source
/// addresses prove the overlay addressing.
async fn udp_round_trip(a: &Node, b: &Node, payload: &[u8]) {
    let stack_a = a.effect.stack().expect("a stack");
    let stack_b = b.effect.stack().expect("b stack");
    let socket_a = stack_a.bind_udp(0).expect("a udp");
    let socket_b = stack_b.bind_udp(ECHO_PORT).expect("b udp");

    socket_a
        .send_to(payload, SocketAddr::new(IpAddr::V6(b.ula), ECHO_PORT))
        .await
        .expect("send a→b");

    let mut buf = [0u8; 2048];
    let (len, from) = socket_b.recv_from(&mut buf).await.expect("recv at b");
    assert_eq!(&buf[..len], payload, "payload survived the tunnel");
    assert_eq!(from.ip(), IpAddr::V6(a.ula), "source is a's overlay /128");

    socket_b.send_to(&buf[..len], from).await.expect("echo b→a");
    let (len, from) = socket_a.recv_from(&mut buf).await.expect("recv at a");
    assert_eq!(&buf[..len], payload, "echo survived the return path");
    assert_eq!(from.ip(), IpAddr::V6(b.ula), "echo source is b's /128");
}

/// the ADR's core loopback proof: handshake + datagram echo, with the
/// passive side learning the initiator's endpoint from the wire.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn handshake_and_datagram_echo() {
    let (mut a, mut b) = (stand_up(0x11, 0xa), stand_up(0x22, 0xb));
    peer_up(&mut a, &mut b);

    tokio::time::timeout(Duration::from_secs(10), udp_round_trip(&a, &b, b"quack"))
        .await
        .expect("echo within deadline");

    // the observable handshake: both tunnels completed one.
    let device_a = a.effect.device().expect("a device");
    assert!(
        device_a.time_since_last_handshake(b.ula).is_some(),
        "a completed a handshake with b"
    );
    let device_b = b.effect.device().expect("b device");
    assert!(
        device_b.time_since_last_handshake(a.ula).is_some(),
        "b completed a handshake with a"
    );
}

/// TCP dial/listen through the tunnel: the virtual stack's stream surface,
/// authenticated by overlay source address, carrying bytes both ways.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tcp_dial_listen_echo() {
    let (mut a, mut b) = (stand_up(0x31, 0xa), stand_up(0x42, 0xb));
    peer_up(&mut a, &mut b);

    let stack_a = a.effect.stack().expect("a stack");
    let stack_b = b.effect.stack().expect("b stack");
    let mut listener = stack_b.listen_tcp(8443, 4).expect("listen at b");

    let a_ula = a.ula;
    let b_ula = b.ula;
    let dial = tokio::spawn(async move {
        let mut stream = stack_a
            .connect_tcp(SocketAddr::new(IpAddr::V6(b_ula), 8443))
            .await
            .expect("dial through the tunnel");
        stream.write_all(b"hello over wireguard").await.expect("write");
        stream.flush().await.expect("flush");
        let mut echo = vec![0u8; 20];
        stream.read_exact(&mut echo).await.expect("read echo");
        echo
    });

    let accept = async {
        let (mut stream, remote) = listener.accept().await.expect("accept");
        assert_eq!(
            remote.ip(),
            IpAddr::V6(a_ula),
            "accepted stream authenticated by a's overlay /128"
        );
        let mut request = vec![0u8; 20];
        stream.read_exact(&mut request).await.expect("read");
        assert_eq!(&request, b"hello over wireguard");
        stream.write_all(&request).await.expect("write back");
        stream.flush().await.expect("flush");
        // hold the stream until the dialer has read the echo.
        tokio::time::sleep(Duration::from_millis(200)).await;
    };

    tokio::time::timeout(Duration::from_secs(10), accept)
        .await
        .expect("accept within deadline");
    let echo = tokio::time::timeout(Duration::from_secs(10), dial)
        .await
        .expect("dial within deadline")
        .expect("dial task");
    assert_eq!(&echo, b"hello over wireguard");
}

/// a forced re-handshake (the rekey lever) completes and traffic keeps
/// flowing on the new session.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn forced_rekey_keeps_traffic_flowing() {
    let (mut a, mut b) = (stand_up(0x51, 0xa), stand_up(0x62, 0xb));
    peer_up(&mut a, &mut b);

    tokio::time::timeout(Duration::from_secs(10), udp_round_trip(&a, &b, b"before"))
        .await
        .expect("echo before rekey");

    // age the session past one timer-granularity second, then force a new
    // handshake and watch the last-handshake clock rewind — the observable
    // proof a NEW handshake completed (not the old one still standing).
    tokio::time::sleep(Duration::from_millis(1300)).await;
    let device_a = a.effect.device().expect("a device");
    let aged = device_a
        .time_since_last_handshake(b.ula)
        .expect("session established");
    assert!(aged >= Duration::from_secs(1), "session aged: {aged:?}");

    device_a
        .initiate_handshake(b.ula, true)
        .await
        .expect("forced rekey");
    let rekeyed = async {
        loop {
            match device_a.time_since_last_handshake(b.ula) {
                Some(since) if since < aged => break,
                _ => tokio::time::sleep(Duration::from_millis(50)).await,
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(10), rekeyed)
        .await
        .expect("rekey handshake completed");

    tokio::time::timeout(Duration::from_secs(10), udp_round_trip(&a, &b, b"after"))
        .await
        .expect("echo after rekey");
}

/// re-applying an IDENTICAL configuration preserves live sessions — the
/// property the orchestrator's mid-epoch `update_peer_tunnels` re-apply
/// depends on (an apply that re-tunneled every peer would drop traffic on
/// every standby record arrival).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reapply_identical_config_preserves_sessions() {
    let (mut a, mut b) = (stand_up(0x71, 0xa), stand_up(0x82, 0xb));
    peer_up(&mut a, &mut b);

    tokio::time::timeout(Duration::from_secs(10), udp_round_trip(&a, &b, b"first"))
        .await
        .expect("echo before re-apply");

    // age the session so a reset would be visible, then re-apply verbatim.
    tokio::time::sleep(Duration::from_millis(1300)).await;
    let aged = a
        .effect
        .device()
        .expect("a device")
        .time_since_last_handshake(b.ula)
        .expect("session established");
    assert!(aged >= Duration::from_secs(1));

    let a_port = a.endpoint.port();
    let peers_for_a = vec![peer_entry(&b, Some(b.endpoint))];
    a.effect
        .apply(&config(&a, a_port, peers_for_a))
        .expect("identical re-apply");

    let preserved = a
        .effect
        .device()
        .expect("a device")
        .time_since_last_handshake(b.ula)
        .expect("session still established — the tunn survived the re-apply");
    assert!(
        preserved >= aged,
        "the live session survived (no re-handshake): {preserved:?} >= {aged:?}"
    );

    tokio::time::timeout(Duration::from_secs(10), udp_round_trip(&a, &b, b"second"))
        .await
        .expect("echo after re-apply");
}

/// `apply` replaces the peer set atomically: traffic switches to the new
/// peer, and the removed peer is cut off in BOTH directions — its address is
/// no longer routed outbound, and its inbound datagrams no longer decrypt.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn peer_replace_switches_traffic_atomically() {
    let (mut a, mut b) = (stand_up(0x91, 0xa), stand_up(0xa2, 0xb));
    let mut c = stand_up(0xb3, 0xc);
    peer_up(&mut a, &mut b);

    tokio::time::timeout(Duration::from_secs(10), udp_round_trip(&a, &b, b"a and b"))
        .await
        .expect("echo a↔b before the replace");

    // replace: a's peer set becomes {c} (b is gone); c peers a passively.
    let a_port = a.endpoint.port();
    let c_port = c.endpoint.port();
    let peers_for_a = vec![peer_entry(&c, Some(c.endpoint))];
    a.effect
        .apply(&config(&a, a_port, peers_for_a))
        .expect("replace b with c");
    let peers_for_c = vec![peer_entry(&a, None)];
    c.effect
        .apply(&config(&c, c_port, peers_for_c))
        .expect("peer c with a");

    // the new relationship carries traffic (fresh handshake included).
    tokio::time::timeout(Duration::from_secs(10), udp_round_trip(&a, &c, b"a and c"))
        .await
        .expect("echo a↔c after the replace");

    // the removed relationship is dead: nothing a sends reaches b anymore
    // (b's /128 left a's cryptokey table), even though b still peers a.
    let stack_a = a.effect.stack().expect("a stack");
    let stack_b = b.effect.stack().expect("b stack");
    let socket_a = stack_a.bind_udp(0).expect("a udp");
    let socket_b = stack_b.bind_udp(ECHO_PORT).expect("b udp");
    socket_a
        .send_to(b"ghost", SocketAddr::new(IpAddr::V6(b.ula), ECHO_PORT))
        .await
        .expect("send enqueues even when unroutable — datagram semantics");
    let mut buf = [0u8; 2048];
    let silent = tokio::time::timeout(Duration::from_millis(1500), socket_b.recv_from(&mut buf));
    assert!(
        silent.await.is_err(),
        "the replaced peer must receive nothing from a"
    );
}
