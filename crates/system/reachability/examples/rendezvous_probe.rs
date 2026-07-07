//! Two-sided live probe for the coordinator rendezvous plane.
//!
//! Drives the PRODUCTION resolver path (`NatResolver::bind` → reflexive
//! discovery + `register` + keepalive readvertise + rendezvous pump;
//! `resolve()` → per-try re-`Lookup` + hole-punch) against a real, deployed
//! coordinator — the same code `bin/node` runs, minus the node around it.
//!
//! One machine idles (its pump answers coordinator-vouched `PunchSync` while
//! it does nothing), the other resolves it. A `Punched` result is
//! datagram-level proof of a direct path in BOTH directions: the resolver
//! only returns `Punched` after receiving the peer's punch datagram from the
//! exact reflexive address the coordinator vouched for, and that punch was
//! itself the peer's answer to the fan-out.
//!
//!   idle:    cargo run -p reachability --example rendezvous_probe --release -- \
//!              idle --coord p2p.example.org:3478 --seed 22
//!   resolve: cargo run -p reachability --example rendezvous_probe --release -- \
//!              resolve --coord p2p.example.org:3478 --seed 11 --peer <hex64>
//!
//! Keys are seed-derived (demo identities, not node keys); every request
//! carries the proof-of-possession the deployed public coordinator requires.

use commonware_cryptography::{Signer as _, ed25519};
use nat_traversal::NodeKey;
use reachability::{EndpointResolver as _, NatResolver, Resolution};
use std::net::SocketAddr;

fn usage() -> ! {
    eprintln!(
        "usage: rendezvous_probe idle    --coord <host:port> --seed <u64>\n       rendezvous_probe resolve --coord <host:port> --seed <u64> --peer <hex64>"
    );
    std::process::exit(2);
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
        out[i] = u8::from_str_radix(std::str::from_utf8(chunk).ok()?, 16).ok()?;
    }
    Some(out)
}

fn arg(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).cloned())
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = args.first().cloned().unwrap_or_default();
    let coord_host = arg(&args, "--coord").unwrap_or_else(|| usage());
    let seed: u64 = arg(&args, "--seed")
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| usage());

    let coord: SocketAddr = tokio::net::lookup_host(&coord_host)
        .await
        .ok()
        .and_then(|mut a| a.next())
        .unwrap_or_else(|| {
            eprintln!("cannot resolve coordinator {coord_host}");
            std::process::exit(1);
        });

    let signer = ed25519::PrivateKey::from_seed(seed);
    let mut key = [0u8; 32];
    key.copy_from_slice(signer.public_key().as_ref());
    let key = NodeKey(key);

    println!("node key:    {}", hex(&key.0));
    println!("coordinator: {coord_host} -> {coord}");

    let started = std::time::Instant::now();
    let mut resolver = NatResolver::bind(key, vec![coord], Some((signer, None)))
        .await
        .unwrap_or_else(|e| {
            eprintln!("bind/register against coordinator failed: {e}");
            std::process::exit(1);
        });
    println!(
        "reflexive:   {} (coordinator-observed public mapping, {}ms)",
        resolver.reflexive().expect("coordinator set is non-empty"),
        started.elapsed().as_millis()
    );

    match mode.as_str() {
        "idle" => {
            println!("registered; idling — pump answers PunchSync, keepalive readvertises every 25s. Ctrl-C to stop.");
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
            }
        }
        "resolve" => {
            let peer = arg(&args, "--peer")
                .and_then(|p| unhex32(&p))
                .unwrap_or_else(|| usage());
            let peer = NodeKey(peer);
            println!("resolving:   {}", hex(&peer.0));
            let t = std::time::Instant::now();
            // The advertised addr is what a failed punch would fall back to
            // in the node; the probe has none, so any non-Punched outcome is
            // reported honestly as a failure.
            match resolver.resolve(peer, "0.0.0.0:0".parse().unwrap()).await {
                Ok(Resolution::Punched(addr)) => {
                    println!(
                        "PUNCHED:     direct path up, peer at {addr} ({}ms) — punch datagrams exchanged both ways",
                        t.elapsed().as_millis()
                    );
                }
                Ok(Resolution::Advertised) => {
                    println!("ADVERTISED:  resolver fell through without a punch (unexpected for the probe)");
                    std::process::exit(1);
                }
                Err(e) => {
                    println!("FAILED:      {e} ({}ms)", t.elapsed().as_millis());
                    std::process::exit(1);
                }
            }
        }
        _ => usage(),
    }
}
