use std::net::SocketAddr;

use tokio::net::UdpSocket;

use crate::{Coordinator, Msg, NodeKey};

pub struct NatClient {
    sock: UdpSocket,
    key: NodeKey,
    coord: SocketAddr,
}

impl NatClient {
    pub async fn bind(key: NodeKey, coord: SocketAddr) -> std::io::Result<Self> {
        let sock = UdpSocket::bind("0.0.0.0:0").await?;
        Ok(Self { sock, key, coord })
    }

    pub async fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.sock.local_addr()
    }

    pub async fn discover_reflexive(&self) -> std::io::Result<SocketAddr> {
        self.sock
            .send_to(&Msg::BindRequest { from: self.key }.encode(), self.coord)
            .await?;
        let mut buf = [0u8; 64];
        loop {
            let (n, _from) = self.sock.recv_from(&mut buf).await?;
            if let Ok(Msg::BindResponse { reflexive }) = Msg::decode(&buf[..n]) {
                return Ok(reflexive);
            }
        }
    }

    pub async fn register(&self) -> std::io::Result<()> {
        self.sock
            .send_to(&Msg::Register { key: self.key }.encode(), self.coord)
            .await?;
        Ok(())
    }

    pub async fn send_punch_to(&self, peer: SocketAddr) -> std::io::Result<()> {
        self.sock
            .send_to(&Msg::Punch { from: self.key }.encode(), peer)
            .await?;
        Ok(())
    }

    pub async fn recv_punch(&self) -> std::io::Result<Msg> {
        let mut buf = [0u8; 64];
        loop {
            let (n, _from) = self.sock.recv_from(&mut buf).await?;
            if let Ok(m @ Msg::Punch { .. }) = Msg::decode(&buf[..n]) {
                return Ok(m);
            }
        }
    }
}

/// The coordinator event loop: decode, feed the pure handler, send replies.
pub async fn run_coordinator(sock: UdpSocket) {
    let mut coord = Coordinator::new();
    let mut buf = [0u8; 64];
    loop {
        let (n, from) = match sock.recv_from(&mut buf).await {
            Ok(v) => v,
            Err(_) => continue,
        };
        let msg = match Msg::decode(&buf[..n]) {
            Ok(m) => m,
            Err(_) => continue,
        };
        for (dst, reply) in coord.handle(from, msg) {
            let _ = sock.send_to(&reply.encode(), dst).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NodeKey;
    use tokio::net::UdpSocket;

    #[tokio::test]
    async fn client_discovers_its_reflexive_via_coordinator() {
        let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let coord_addr = coord_sock.local_addr().unwrap();
        tokio::spawn(run_coordinator(coord_sock));

        let client = NatClient::bind(NodeKey([1u8; 32]), coord_addr).await.unwrap();
        let reflexive = client.discover_reflexive().await.unwrap();
        // The socket binds 0.0.0.0:0, so local_addr() reports the wildcard IP
        // while the coordinator observes 127.0.0.1 as the source — the IPs
        // differ by design. The port is the load-bearing invariant.
        assert_eq!(reflexive.port(), client.local_addr().await.unwrap().port());
    }
}
