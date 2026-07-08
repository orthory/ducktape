//! the ADR phase-2 wire-compat probe: one binary that stands up EITHER
//! overlay backend behind the same `WireGuardEffect` + `SocketFactory`
//! surfaces, so the container smoke (`ops/wg-smoke/run-interop.sh`) can pit
//! them against each other on one WireGuard network:
//!
//! - `--mode tun`    — today's production backend (`DefguardWireGuardEffect`,
//!   BoringTun over a TUN device, kernel TCP/IP), needs `CAP_NET_ADMIN` +
//!   `/dev/net/tun`.
//! - `--mode socket` — the userspace backend (`UserspaceWireGuardEffect`,
//!   in-process smoltcp host), runs with NO privilege at all.
//!
//! both modes serve a TCP echo (port 7000) and a UDP echo (port 7002) at
//! their overlay ULA through their `SocketFactory`, and `--dial` runs the
//! same echoes as a client against the peer — so a passing pair proves the
//! Noise handshake, the wire format, cryptokey routing, and both transport
//! surfaces match across backends.
//!
//! both modes also serve a TCP bulk SINK (port 7004: drain to EOF, print
//! bytes + elapsed), and `--bulk <bytes>` pushes that many bytes at the
//! peer's sink through the same factory — the overlay-net ADR's phase-4
//! throughput probe. it measures the raw stack (kernel TCP over TUN vs
//! smoltcp), deliberately NOT a `DataPlane`: no bulk token bucket in the
//! path. the sink's first-byte→EOF rate is the number that matters; the
//! push side prints its own as a cross-check.
//!
//! subcommands:
//!   keygen <seed-byte>                      print a deterministic keypair
//!   serve --mode tun|socket --priv <b64> --ula <v6> --wg-port <port>
//!         --peer-pub <b64> --peer-ula <v6> [--peer-endpoint <ip:port>]
//!         [--dial] [--bulk <bytes>]         bring the tunnel up and serve
//!   client tcp|udp <[v6]:port>              plain-OS echo client (run inside
//!                                           the tun container: the kernel
//!                                           routes it through the tunnel)

use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use data_plane::{OsSocketFactory, SocketFactory};
use defguard_wireguard_rs::{InterfaceConfiguration, key::Key, net::IpAddrMask, peer::Peer};
use overlay_net::userspace::{UserspaceWireGuardEffect, VirtualSocketFactory};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use wireguard_effect::{DefguardWireGuardEffect, WireGuardEffect};

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
    eprintln!("       wg_interop serve --mode tun|socket --priv <b64> --ula <v6> \\");
    eprintln!("                        --wg-port <port> --peer-pub <b64> --peer-ula <v6> \\");
    eprintln!("                        [--peer-endpoint <ip:port>] [--dial] [--bulk <bytes>]");
    eprintln!("       wg_interop client tcp|udp <[v6]:port>");
    std::process::exit(2);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("keygen") => keygen(&args[1..]),
        Some("serve") => serve(&args[1..]),
        Some("client") => client(&args[1..]),
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
    let secret = Key::new(bytes);
    println!("PRIV {secret}");
    println!("PUB {}", secret.public_key());
}

// ── serve ───────────────────────────────────────────────

struct ServeArgs {
    mode: String,
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
    let (mut mode, mut prvkey, mut ula, mut wg_port) = (None, None, None, None);
    let (mut peer_pub, mut peer_ula, mut peer_endpoint, mut dial) = (None, None, None, false);
    let mut bulk = None;
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--mode" => mode = it.next().cloned(),
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
        mode: mode?,
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

fn interface_config(args: &ServeArgs) -> InterfaceConfiguration {
    let peer_key = Key::try_from(args.peer_pub.as_str()).expect("peer pubkey is valid base64");
    let mut peer = Peer::new(peer_key);
    peer.endpoint = args.peer_endpoint;
    // keep the pair's NAT-ish container path warm both ways.
    peer.persistent_keepalive_interval = Some(5);
    peer.set_allowed_ips(vec![IpAddrMask::new(IpAddr::V6(args.peer_ula), 128)]);
    InterfaceConfiguration {
        name: "dt-interop0".into(),
        prvkey: args.prvkey.clone(),
        addresses: vec![IpAddrMask::new(IpAddr::V6(args.ula), 128)],
        port: args.wg_port,
        peers: vec![peer],
        // the same tunnel MTU production applies (`wiring::TUNNEL_MTU`) —
        // `None` leaves the TUN at 1500, and every full-size inner packet
        // then rides a FRAGMENTED outer UDP datagram (and overruns the
        // userspace stack's 1420 device MTU on mixed pairs).
        mtu: Some(wireguard_effect::TUNNEL_MTU),
        fwmark: None,
    }
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
        // both effects must outlive the echo loops — dropping them tears the
        // tunnel down. held here for the process life.
        let (_tun_effect, _socket_effect, factory): (
            Option<DefguardWireGuardEffect>,
            Option<UserspaceWireGuardEffect>,
            Arc<dyn SocketFactory>,
        ) = match args.mode.as_str() {
            "tun" => {
                let mut effect =
                    DefguardWireGuardEffect::new("dt-interop0").expect("defguard api handle");
                effect.create_interface().expect("create tun interface");
                effect.apply(&config).expect("apply tun config");
                (Some(effect), None, Arc::new(OsSocketFactory))
            }
            "socket" => {
                let mut effect = UserspaceWireGuardEffect::new(tokio::runtime::Handle::current());
                effect
                    .create_interface()
                    .expect("create userspace interface");
                effect.apply(&config).expect("apply userspace config");
                let factory = Arc::new(VirtualSocketFactory::new(effect.stack_slot()));
                (None, Some(effect), factory)
            }
            _ => usage(),
        };
        println!("INTEROP: {} interface up at {}", args.mode, args.ula);

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
    println!("{}", bulk_report("push", sent, started.elapsed().as_secs_f64()));
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

/// retry a factory bind until it lands: right after `apply` the address can
/// still be settling (tun mode), which is exactly the node's bring-up shape.
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

// ── client (plain-OS, for exec inside the tun container) ─

fn client(args: &[String]) {
    let (Some(proto), Some(dest)) = (args.first(), args.get(1)) else {
        usage()
    };
    let dest: SocketAddr = dest.parse().expect("destination address");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async move {
        match proto.as_str() {
            "tcp" => {
                let mut stream = tokio::time::timeout(
                    Duration::from_secs(20),
                    tokio::net::TcpStream::connect(dest),
                )
                .await
                .expect("tcp connect within deadline")
                .expect("tcp connect");
                stream.write_all(TCP_PING).await.expect("tcp write");
                let mut echo = vec![0u8; TCP_PING.len()];
                tokio::time::timeout(Duration::from_secs(10), stream.read_exact(&mut echo))
                    .await
                    .expect("tcp echo within deadline")
                    .expect("tcp read");
                assert_eq!(echo, TCP_PING);
                println!("CLIENT tcp PASS");
            }
            "udp" => {
                let socket = tokio::net::UdpSocket::bind("[::]:0").await.expect("bind");
                let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
                let mut buf = [0u8; 4096];
                loop {
                    socket.send_to(UDP_PING, dest).await.expect("udp send");
                    match tokio::time::timeout(Duration::from_secs(2), socket.recv_from(&mut buf))
                        .await
                    {
                        Ok(Ok((n, _))) => {
                            assert_eq!(&buf[..n], UDP_PING);
                            println!("CLIENT udp PASS");
                            return;
                        }
                        _ => assert!(
                            tokio::time::Instant::now() < deadline,
                            "udp echo did not land within 20s"
                        ),
                    }
                }
            }
            _ => usage(),
        }
    });
}
