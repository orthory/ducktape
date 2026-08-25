//! the overlay wire probe: stands up the userspace overlay backend
//! (`UserspaceWireGuardEffect`, in-process smoltcp host — the node's only
//! backend, no privilege at all) behind the `WireGuardEffect` +
//! `SocketFactory` surfaces, so the container smoke
//! (`ops/wg-smoke/run-interop.sh`) can prove the Noise handshake, wire
//! format, and cryptokey routing over a real network between two fully
//! unprivileged containers.
//!
//! serves a TCP echo (port 7000) and a UDP echo (port 7002) at the overlay
//! ULA through the `SocketFactory`, and `--dial` runs the same echoes as a
//! client against the peer.
//!
//! also serves a TCP bulk SINK (port 7004: drain to EOF, print bytes +
//! elapsed), and `--bulk <bytes>` pushes that many bytes at the peer's sink
//! through the same factory — the throughput probe. it measures the raw
//! stack, deliberately NOT a `DataPlane`: no bulk token bucket in the path.
//! the sink's first-byte→EOF rate is the number that matters; the push side
//! prints its own as a cross-check.
//!
//! subcommands:
//!   keygen <seed-byte>                      print a deterministic keypair
//!   serve --priv <b64> --ula <v6> --wg-port <port>
//!         --peer-pub <b64> --peer-ula <v6> [--peer-endpoint <ip:port>]
//!         [--dial] [--bulk <bytes>]         bring the tunnel up and serve

use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use data_plane::SocketFactory;
use defguard_boringtun::x25519::{PublicKey, StaticSecret};
use overlay_net::userspace::{UserspaceWireGuardEffect, VirtualSocketFactory};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use wireguard::effect::{InterfaceConfig, PeerTunnelConfig, WireGuardEffect};
use wireguard::{AllowedIp, X25519PublicKey};

const TCP_ECHO_PORT: u16 = 7000;
const UDP_ECHO_PORT: u16 = 7002;
const TCP_BULK_PORT: u16 = 7004;
const TCP_PING: &[u8] = b"interop-tcp-ping";
const UDP_PING: &[u8] = b"interop-udp-ping";
/// how long the dial legs keep retrying: covers handshake latency and the
/// far side still coming up.
const DIAL_DEADLINE: Duration = Duration::from_secs(60);
/// bulk push chunk: large enough that the syscall/poll overhead is not what
/// the probe measures, small enough to keep write_all latency bounded.
const BULK_CHUNK: usize = 256 * 1024;

fn usage() -> ! {
    eprintln!("usage: wg_interop keygen <seed-byte>");
    eprintln!("       wg_interop serve --priv <b64> --ula <v6> \\");
    eprintln!("                        --wg-port <port> --peer-pub <b64> --peer-ula <v6> \\");
    eprintln!("                        [--peer-endpoint <ip:port>] [--dial] [--bulk <bytes>]");
    std::process::exit(2);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("keygen") => keygen(&args[1..]),
        Some("serve") => serve(&args[1..]),
        _ => usage(),
    }
}

/// a deterministic keypair from a seed byte — smoke fixtures, not secrets
/// (X25519 clamps any 32 bytes into a valid scalar).
fn keygen(args: &[String]) {
    let Some(seed) = args.first().and_then(|s| s.parse::<u8>().ok()) else {
        usage()
    };
    let mut bytes = [seed; 32];
    bytes[0] = seed.wrapping_add(1);
    let secret = StaticSecret::from(bytes);
    println!("PRIV {}", B64.encode(secret.to_bytes()));
    println!("PUB {}", B64.encode(PublicKey::from(&secret).to_bytes()));
}

// ── serve ───────────────────────────────────────────────

struct ServeArgs {
    prvkey: String,
    ula: Ipv6Addr,
    wg_port: u16,
    peer_pub: String,
    peer_ula: Ipv6Addr,
    peer_endpoint: Option<SocketAddr>,
    dial: bool,
    bulk: Option<u64>,
}

fn parse_serve(args: &[String]) -> Option<ServeArgs> {
    let (mut prvkey, mut ula, mut wg_port) = (None, None, None);
    let (mut peer_pub, mut peer_ula, mut peer_endpoint, mut dial) = (None, None, None, false);
    let mut bulk = None;
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--priv" => prvkey = it.next().cloned(),
            "--ula" => ula = it.next()?.parse().ok(),
            "--wg-port" => wg_port = it.next()?.parse().ok(),
            "--peer-pub" => peer_pub = it.next().cloned(),
            "--peer-ula" => peer_ula = it.next()?.parse().ok(),
            "--peer-endpoint" => peer_endpoint = Some(it.next()?.parse().ok()?),
            "--dial" => dial = true,
            "--bulk" => bulk = Some(it.next()?.parse().ok()?),
            _ => return None,
        }
    }
    Some(ServeArgs {
        prvkey: prvkey?,
        ula: ula?,
        wg_port: wg_port?,
        peer_pub: peer_pub?,
        peer_ula: peer_ula?,
        peer_endpoint,
        dial,
        bulk,
    })
}

fn interface_config(args: &ServeArgs) -> InterfaceConfig {
    let peer = PeerTunnelConfig {
        wireguard_public_key: X25519PublicKey(key_bytes(&args.peer_pub, "peer pubkey")),
        endpoint: args.peer_endpoint,
        allowed_ips: vec![member_route(args.peer_ula)],
        // keep the pair's NAT-ish container path warm both ways.
        keepalive_seconds: Some(5),
    };
    InterfaceConfig {
        name: "dt-interop0".into(),
        private_key: key_bytes(&args.prvkey, "private key"),
        listen_port: args.wg_port,
        addresses: vec![member_route(args.ula)],
        peers: vec![peer],
    }
}

/// a member's cryptokey route: its overlay `/128`.
fn member_route(ula: Ipv6Addr) -> AllowedIp {
    AllowedIp::new(IpAddr::V6(ula), 128).expect("a /128 is a valid route")
}

/// a WireGuard key in the `wg` tool's base64 form, as the CLI carries it.
fn key_bytes(encoded: &str, what: &str) -> [u8; 32] {
    let bytes = B64
        .decode(encoded)
        .unwrap_or_else(|_| panic!("{what} is valid base64"));
    bytes
        .try_into()
        .unwrap_or_else(|_| panic!("{what} is a 32-byte key"))
}

fn serve(args: &[String]) {
    let Some(args) = parse_serve(args) else {
        usage()
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async move {
        let config = interface_config(&args);
        // the effect must outlive the echo loops — dropping it tears the
        // tunnel down. held here for the process life.
        let mut effect = UserspaceWireGuardEffect::new(tokio::runtime::Handle::current());
        effect
            .create_interface()
            .expect("create userspace interface");
        effect.apply(&config).expect("apply userspace config");
        let factory: Arc<dyn SocketFactory> =
            Arc::new(VirtualSocketFactory::new(effect.stack_slot()));
        println!("INTEROP: interface up at {}", args.ula);

        spawn_tcp_echo(factory.clone(), args.ula).await;
        spawn_udp_echo(factory.clone(), args.ula).await;
        spawn_tcp_bulk_sink(factory.clone(), args.ula).await;
        println!("INTEROP: serving at {}", args.ula);

        if args.dial {
            tcp_echo_client(factory.clone(), args.ula, args.peer_ula).await;
            println!("INTEROP: tcp echo PASS");
            udp_echo_client(factory.clone(), args.ula, args.peer_ula).await;
            println!("INTEROP: udp echo PASS");
        }
        if let Some(bytes) = args.bulk {
            bulk_push_client(factory, args.ula, args.peer_ula, bytes).await;
        }
        // stay up: the peer dials us on its own schedule.
        std::future::pending::<()>().await;
    });
}

/// bind the TCP echo at the ULA (retrying while the address settles) and
/// serve every accepted stream until EOF.
async fn spawn_tcp_echo(factory: Arc<dyn SocketFactory>, ula: Ipv6Addr) {
    let bind = SocketAddr::new(IpAddr::V6(ula), TCP_ECHO_PORT);
    let listener = bind_retry(|| factory.bind_listener(bind)).await;
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _remote)) = listener.accept().await else {
                continue;
            };
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                loop {
                    match stream.read(&mut buf).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if stream.write_all(&buf[..n]).await.is_err() {
                                break;
                            }
                            let _ = stream.flush().await;
                        }
                    }
                }
            });
        }
    });
}

/// bind the UDP echo at the ULA and bounce every datagram to its source.
async fn spawn_udp_echo(factory: Arc<dyn SocketFactory>, ula: Ipv6Addr) {
    let bind = SocketAddr::new(IpAddr::V6(ula), UDP_ECHO_PORT);
    let socket = bind_retry(|| factory.bind_udp(bind)).await;
    tokio::spawn(async move {
        let mut buf = [0u8; 4096];
        loop {
            let Ok((n, from)) = socket.recv_from(&mut buf).await else {
                continue;
            };
            let _ = socket.send_to(&buf[..n], from).await;
        }
    });
}

/// bind the TCP bulk sink at the ULA: drain every accepted stream to EOF and
/// print the first-byte→EOF rate — the receive-side throughput number the
/// bench harness greps for.
async fn spawn_tcp_bulk_sink(factory: Arc<dyn SocketFactory>, ula: Ipv6Addr) {
    let bind = SocketAddr::new(IpAddr::V6(ula), TCP_BULK_PORT);
    let listener = bind_retry(|| factory.bind_listener(bind)).await;
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _remote)) = listener.accept().await else {
                continue;
            };
            tokio::spawn(async move {
                let mut buf = vec![0u8; BULK_CHUNK];
                let mut total: u64 = 0;
                let mut started: Option<tokio::time::Instant> = None;
                loop {
                    match stream.read(&mut buf).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            started.get_or_insert_with(tokio::time::Instant::now);
                            total += n as u64;
                        }
                    }
                }
                let secs = started.map_or(0.0, |t| t.elapsed().as_secs_f64());
                println!("{}", bulk_report("sink", total, secs));
            });
        }
    });
}

/// push `bytes` at the peer's bulk sink through the factory and print the
/// send-side rate (connect excluded, final flush included).
async fn bulk_push_client(
    factory: Arc<dyn SocketFactory>,
    own: Ipv6Addr,
    peer: Ipv6Addr,
    bytes: u64,
) {
    let dest = SocketAddr::new(IpAddr::V6(peer), TCP_BULK_PORT);
    let deadline = tokio::time::Instant::now() + DIAL_DEADLINE;
    let mut stream = loop {
        match factory.dial_from(IpAddr::V6(own), dest).await {
            Ok(stream) => break stream,
            Err(err) => {
                assert!(
                    tokio::time::Instant::now() < deadline,
                    "bulk dial did not land within {DIAL_DEADLINE:?}: {err}"
                );
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    };
    let chunk = vec![0xd7u8; BULK_CHUNK];
    let started = tokio::time::Instant::now();
    let mut sent: u64 = 0;
    while sent < bytes {
        let n = BULK_CHUNK.min((bytes - sent) as usize);
        stream.write_all(&chunk[..n]).await.expect("bulk write");
        sent += n as u64;
    }
    stream.flush().await.expect("bulk flush");
    // dropping the stream sends FIN — the sink's EOF and end-of-measurement.
    drop(stream);
    println!(
        "{}",
        bulk_report("push", sent, started.elapsed().as_secs_f64())
    );
}

/// one line per measurement, fixed shape for the harness:
/// `INTEROP: bulk <side> <bytes> bytes in <secs>s = <rate> MB/s`
fn bulk_report(side: &str, bytes: u64, secs: f64) -> String {
    let rate = if secs > 0.0 {
        bytes as f64 / secs / 1_000_000.0
    } else {
        0.0
    };
    format!("INTEROP: bulk {side} {bytes} bytes in {secs:.2}s = {rate:.1} MB/s")
}

/// retry a factory bind until it lands: right after `apply` the stack can
/// still be settling, which is exactly the node's bring-up shape.
async fn bind_retry<T, F, Fut>(bind: F) -> T
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = std::io::Result<T>>,
{
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        match bind().await {
            Ok(bound) => return bound,
            Err(err) => {
                assert!(
                    tokio::time::Instant::now() < deadline,
                    "bind did not land within 30s: {err}"
                );
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }
}

/// dial the peer's TCP echo through the factory and verify one round trip.
async fn tcp_echo_client(factory: Arc<dyn SocketFactory>, own: Ipv6Addr, peer: Ipv6Addr) {
    let dest = SocketAddr::new(IpAddr::V6(peer), TCP_ECHO_PORT);
    let deadline = tokio::time::Instant::now() + DIAL_DEADLINE;
    let mut stream = loop {
        match factory.dial_from(IpAddr::V6(own), dest).await {
            Ok(stream) => break stream,
            Err(err) => {
                assert!(
                    tokio::time::Instant::now() < deadline,
                    "tcp dial did not land within {DIAL_DEADLINE:?}: {err}"
                );
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    };
    stream.write_all(TCP_PING).await.expect("tcp write");
    stream.flush().await.expect("tcp flush");
    let mut echo = vec![0u8; TCP_PING.len()];
    tokio::time::timeout(Duration::from_secs(10), stream.read_exact(&mut echo))
        .await
        .expect("tcp echo within deadline")
        .expect("tcp read");
    assert_eq!(echo, TCP_PING, "tcp echo payload intact");
}

/// bounce a datagram off the peer's UDP echo through the factory, retrying
/// sends (datagrams may drop while the handshake completes).
async fn udp_echo_client(factory: Arc<dyn SocketFactory>, own: Ipv6Addr, peer: Ipv6Addr) {
    let socket = factory
        .bind_udp(SocketAddr::new(IpAddr::V6(own), 0))
        .await
        .expect("bind udp client");
    let dest = SocketAddr::new(IpAddr::V6(peer), UDP_ECHO_PORT);
    let deadline = tokio::time::Instant::now() + DIAL_DEADLINE;
    let mut buf = [0u8; 4096];
    loop {
        socket.send_to(UDP_PING, dest).await.expect("udp send");
        match tokio::time::timeout(Duration::from_secs(2), socket.recv_from(&mut buf)).await {
            Ok(Ok((n, from))) => {
                assert_eq!(&buf[..n], UDP_PING, "udp echo payload intact");
                assert_eq!(from, dest, "udp echo came from the peer's echo port");
                return;
            }
            _ => assert!(
                tokio::time::Instant::now() < deadline,
                "udp echo did not land within {DIAL_DEADLINE:?}"
            ),
        }
    }
}
