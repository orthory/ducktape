use std::net::SocketAddr;

use nat_traversal::run_coordinator;
use tokio::net::UdpSocket;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let listen: SocketAddr = std::env::args()
        .skip_while(|a| a != "--listen")
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| "0.0.0.0:3478".parse().unwrap());

    let sock = UdpSocket::bind(listen).await?;
    eprintln!("coordinator listening on {}", sock.local_addr()?);
    run_coordinator(sock).await;
    Ok(())
}
