//! Live proof that the node's SHARED WireGuard underlay reaches a coordinator
//! on the same real-IPv4 mapping the standalone resolver does — the seam
//! macOS 464XLAT (CLAT46) broke.
//!
//! Two `BindRequest`s from THIS machine to the same public coordinator, one
//! per socket posture the node can run:
//!
//!   * OWNED  — `NatSocket::Owned(0.0.0.0:0)`, a real AF_INET socket. The
//!     standalone resolver path (`NatClient::bind`); it always punched.
//!   * SHARED — `overlay_net::userspace::UnderlaySocket` + `NatSocket::shared`,
//!     the exact wiring `bin/node`'s socket mode runs (the punch rides the
//!     tunnel's own 5-tuple).
//!
//! On a normal network both report the same coordinator-observed public IP.
//! On CLAT46 a DUAL-STACK `[::]` underlay with v4-mapped sends reported a
//! DIFFERENT public IP than the owned AF_INET socket — the coordinator
//! answered it but the peer punch it vouched for never landed. With the
//! underlay bound as a real IPv4 socket, the two IPs must match again.
//!
//!   cargo run -p node-bin --example shared_underlay_reflexive -- \
//!     --coord p2p.example.org:3478 --seed 11
//!
//! Exit 0 iff both reflexives resolved AND their public IPs match.

use std::net::SocketAddr;
use std::time::Duration;

use commonware_cryptography::{Signer, ed25519};
use nat_traversal::{NatClient, NatSocket};
use overlay_net::userspace::UnderlaySocket;
use tokio::net::UdpSocket;

const PER_TRY: Duration = Duration::from_secs(2);

fn arg(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).cloned())
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let coord_host = arg(&args, "--coord").unwrap_or_else(|| {
        eprintln!("usage: shared_underlay_reflexive --coord <host:port> [--seed <u64>]");
        std::process::exit(2);
    });
    let seed: u64 = arg(&args, "--seed")
        .and_then(|s| s.parse().ok())
        .unwrap_or(11);

    let coord: SocketAddr = tokio::net::lookup_host(&coord_host)
        .await
        .ok()
        .and_then(|mut a| a.next())
        .unwrap_or_else(|| {
            eprintln!("cannot resolve coordinator {coord_host}");
            std::process::exit(1);
        });

    let signer = ed25519::PrivateKey::from_seed(seed);
    let me = reachability::node_key(reachability::identity_of(&signer.public_key()));
    println!("coordinator: {coord_host} -> {coord}");

    // ── OWNED: the standalone resolver's real AF_INET socket ──
    let owned_sock = UdpSocket::bind("0.0.0.0:0").await.expect("owned bind");
    let owned_local = owned_sock.local_addr().expect("owned local");
    let mut owned = NatClient::with_socket(
        NatSocket::Owned(owned_sock),
        me,
        vec![coord],
        signer.clone(),
        None,
    )
    .expect("owned client");
    let owned_reflexive = owned
        .discover_reflexive_failover(PER_TRY)
        .await
        .map(|(_, r)| r);
    println!(
        "owned:  local {owned_local} ({})  ->  reflexive {}",
        family(owned_local),
        show(&owned_reflexive),
    );

    // ── SHARED: the node's own underlay socket + NAT bypass lane ──
    let underlay =
        UnderlaySocket::bind(&tokio::runtime::Handle::current(), 0).expect("underlay bind");
    let shared_local = underlay.local_addr().expect("shared local");
    let bypass = underlay.take_bypass().expect("bypass lane");
    let mut shared = NatClient::with_socket(
        NatSocket::shared(underlay.sender(), bypass).expect("shared socket"),
        me,
        vec![coord],
        signer.clone(),
        None,
    )
    .expect("shared client");
    let shared_reflexive = shared
        .discover_reflexive_failover(PER_TRY)
        .await
        .map(|(_, r)| r);
    println!(
        "shared: local {shared_local} ({}) ->  reflexive {}",
        family(shared_local),
        show(&shared_reflexive),
    );

    // The verdict: the shared underlay must (a) be a real IPv4 socket and
    // (b) reach the coordinator on the SAME public IP the owned socket does.
    let ok = match (owned_reflexive, shared_reflexive) {
        (Ok(o), Ok(s)) => {
            let match_ip = o.ip() == s.ip();
            println!(
                "\nverdict: shared underlay is {} bound; reflexive IPs {} (owned {} / shared {})",
                if shared_local.is_ipv4() {
                    "IPv4"
                } else {
                    "IPv6"
                },
                if match_ip { "MATCH" } else { "DIVERGE" },
                o.ip(),
                s.ip(),
            );
            shared_local.is_ipv4() && match_ip
        }
        (o, s) => {
            eprintln!(
                "\nverdict: a reflexive never resolved (owned {}, shared {})",
                show(&o),
                show(&s),
            );
            false
        }
    };
    std::process::exit(if ok { 0 } else { 1 });
}

fn family(addr: SocketAddr) -> &'static str {
    if addr.is_ipv4() {
        "AF_INET"
    } else {
        "AF_INET6"
    }
}

fn show(r: &std::io::Result<SocketAddr>) -> String {
    match r {
        Ok(addr) => addr.to_string(),
        Err(e) => format!("<none: {e}>"),
    }
}
