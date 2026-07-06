use std::net::SocketAddr;

use coordinator_bin::select_policy;
use nat_traversal::run_coordinator;
use tokio::net::UdpSocket;

fn arg_value(flag: &str) -> Option<String> {
    std::env::args().skip_while(|a| a != flag).nth(1)
}

fn parse_addr(flag: &str, raw: &str) -> std::io::Result<SocketAddr> {
    raw.parse().map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{flag} {raw:?} is not a valid host:port: {e}"),
        )
    })
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // `--listen <addr>` selects the bind; a malformed value is a HARD error, not
    // a silent fall-through to 0.0.0.0 — a typo'd flag or address must never
    // quietly expose the untrusted control port on every interface.
    let listen: SocketAddr = match arg_value("--listen") {
        Some(s) => parse_addr("--listen", &s)?,
        None => "0.0.0.0:3478".parse().expect("default addr parses"),
    };

    // The per-network authorization policy, selected from CLI flags:
    //   --genesis-set <network.toml>  => Private (PoP + pinned valset admission)
    //   --allow-anonymous             => fully-open (legacy, no auth)
    //   (no flag)                     => public with proof-of-possession
    // A malformed --genesis-set path/file is a HARD error, never a silent
    // fall-through to a weaker policy.
    let args: Vec<String> = std::env::args().skip(1).collect();
    let policy = select_policy(&args)?;

    let sock = UdpSocket::bind(listen).await?;
    // the address line stays parseable (tooling/tests read its tail).
    eprintln!("coordinator listening on {}", sock.local_addr()?);
    run_coordinator(sock, policy).await;
    Ok(())
}
