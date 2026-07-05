use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use nat_traversal::run_coordinator_advertised;
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
    // quietly expose the untrusted control/relay port on every interface.
    let listen: SocketAddr = match arg_value("--listen") {
        Some(s) => parse_addr("--listen", &s)?,
        None => "0.0.0.0:3478".parse().expect("default addr parses"),
    };

    // the reachable IP peers dial for the relay data plane. when the listen IP
    // is concrete we reuse it; when it is the wildcard, `--advertise <ip>` is
    // REQUIRED — otherwise the coordinator would hand peers an unroutable
    // `0.0.0.0:<port>` relay endpoint and the relay fallback would carry no
    // traffic (silent in CI, which binds loopback).
    let advertise_ip: IpAddr = match arg_value("--advertise") {
        Some(s) => s.parse().map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("--advertise {s:?} is not a valid IP: {e}"),
            )
        })?,
        None if !listen.ip().is_unspecified() => listen.ip(),
        None => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "listening on a wildcard address requires --advertise <public-ip> so relayed \
                 peers get a routable relay endpoint",
            ));
        }
    };

    let sock = UdpSocket::bind(listen).await?;
    // the address line stays parseable (tooling/tests read its tail); the relay
    // advertise ip is a separate line.
    eprintln!("coordinator listening on {}", sock.local_addr()?);
    eprintln!("relay endpoint advertised as {advertise_ip}");
    run_coordinator_advertised(sock, advertise_ip, Duration::from_secs(30)).await;
    Ok(())
}
