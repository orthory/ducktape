use std::net::SocketAddr;

use tokio::net::UdpSocket;

use crate::{Coordinator, Msg, NodeKey, Side};

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
            let (n, from) = self.sock.recv_from(&mut buf).await?;
            // Only the coordinator's own reply is trustworthy: anyone else
            // on the network can send a well-formed BindResponse and, absent
            // this check, have it accepted as the coordinator's observation.
            if from != self.coord {
                continue;
            }
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

    /// Ask the coordinator to resolve `peer`'s reflexive address via the real
    /// Lookup/LookupResponse rendezvous path (never the peer's socket
    /// directly).
    pub async fn lookup(&self, peer: NodeKey) -> std::io::Result<SocketAddr> {
        self.sock
            .send_to(&Msg::Lookup { key: peer }.encode(), self.coord)
            .await?;
        let mut buf = [0u8; 64];
        loop {
            let (n, from) = self.sock.recv_from(&mut buf).await?;
            if from != self.coord {
                continue;
            }
            match Msg::decode(&buf[..n]) {
                Ok(Msg::LookupResponse { key, reflexive: Some(addr) }) if key == peer => {
                    return Ok(addr);
                }
                Ok(Msg::LookupResponse { key, reflexive: None }) if key == peer => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "peer not registered with coordinator",
                    ));
                }
                _ => continue,
            }
        }
    }

    /// Wait for the coordinator's unsolicited PunchSync — the fan-out it
    /// sends to the *other* side of somebody else's Lookup — and return the
    /// peer's reflexive address it carries. This is how the passive side of
    /// a rendezvous learns where to punch, without ever touching the
    /// initiator's socket directly.
    pub async fn recv_punch_sync(&self) -> std::io::Result<SocketAddr> {
        let mut buf = [0u8; 64];
        loop {
            let (n, from) = self.sock.recv_from(&mut buf).await?;
            if from != self.coord {
                continue;
            }
            if let Ok(Msg::PunchSync { peer_reflexive, .. }) = Msg::decode(&buf[..n]) {
                return Ok(peer_reflexive);
            }
        }
    }

    pub async fn send_punch_to(&self, peer: SocketAddr) -> std::io::Result<()> {
        self.sock
            .send_to(&Msg::Punch { from: self.key }.encode(), peer)
            .await?;
        Ok(())
    }

    /// Receive a `Punch` datagram, but only accept it if it actually arrived
    /// from `expected` — the peer's rendezvous-resolved socket address.
    /// Discarding the sender address here would let any third party forge a
    /// `Punch` claiming to be from the peer.
    pub async fn recv_punch_from(&self, expected: SocketAddr) -> std::io::Result<Msg> {
        let mut buf = [0u8; 64];
        loop {
            let (n, from) = self.sock.recv_from(&mut buf).await?;
            if from != expected {
                continue;
            }
            if let Ok(m @ Msg::Punch { .. }) = Msg::decode(&buf[..n]) {
                return Ok(m);
            }
        }
    }

    /// Ask the coordinator to allocate a relay session to `peer`; return the
    /// session id and THIS side's relay endpoint — the address to point the
    /// WireGuard peer at on hole-punch failure (`peer_endpoint_override`).
    pub async fn request_relay(&self, peer: NodeKey) -> std::io::Result<(u64, SocketAddr)> {
        self.sock
            .send_to(&Msg::RelayRequest { peer }.encode(), self.coord)
            .await?;
        let mut buf = [0u8; 64];
        loop {
            let (n, from) = self.sock.recv_from(&mut buf).await?;
            if from != self.coord {
                continue;
            }
            if let Ok(Msg::RelayGrant { session, relay }) = Msg::decode(&buf[..n]) {
                return Ok((session, relay));
            }
        }
    }

    /// Send an OPAQUE datagram to a relay endpoint. The relay forwards it
    /// verbatim; the bytes are never interpreted by this crate.
    pub async fn relay_send(&self, relay: SocketAddr, payload: &[u8]) -> std::io::Result<()> {
        self.sock.send_to(payload, relay).await?;
        Ok(())
    }

    /// Receive a relayed OPAQUE datagram (up to one MTU). Returns the raw bytes
    /// as delivered by the relay — no decode.
    pub async fn relay_recv(&self) -> std::io::Result<Vec<u8>> {
        let mut buf = [0u8; 1500];
        let (n, _from) = self.sock.recv_from(&mut buf).await?;
        Ok(buf[..n].to_vec())
    }
}

/// The real opaque splice for one relay session: forward datagrams between two
/// UDP sockets (one per side), learning each side's source on its first
/// datagram, and tear down after `idle` of total inactivity. Never decodes a
/// payload — it holds only the two learned source addresses.
pub async fn run_relay_pair(a_sock: UdpSocket, b_sock: UdpSocket, idle: std::time::Duration) {
    let mut a_src: Option<SocketAddr> = None;
    let mut b_src: Option<SocketAddr> = None;
    let mut a_buf = [0u8; 1500];
    let mut b_buf = [0u8; 1500];
    loop {
        tokio::select! {
            r = a_sock.recv_from(&mut a_buf) => {
                let (n, from) = match r { Ok(v) => v, Err(_) => continue };
                a_src = Some(from);
                if let Some(dst) = b_src {
                    let _ = b_sock.send_to(&a_buf[..n], dst).await;
                }
            }
            r = b_sock.recv_from(&mut b_buf) => {
                let (n, from) = match r { Ok(v) => v, Err(_) => continue };
                b_src = Some(from);
                if let Some(dst) = a_src {
                    let _ = a_sock.send_to(&b_buf[..n], dst).await;
                }
            }
            _ = tokio::time::sleep(idle) => {
                // Idle timeout: no datagram on either side within `idle`. The
                // sleep re-arms every loop iteration, so this fires only after
                // `idle` of continuous inactivity. Bounded teardown.
                return;
            }
        }
    }
}

/// The coordinator event loop: decode control datagrams, feed the pure handler,
/// send replies. `RelayRequest` is handled specially — it must bind real relay
/// sockets, which the transport-free `Coordinator::handle` cannot do.
pub async fn run_coordinator(sock: UdpSocket) {
    use std::collections::HashMap;

    let mut coord = Coordinator::new();
    let mut buf = [0u8; 64];
    let bind_ip = sock
        .local_addr()
        .map(|a| a.ip())
        .unwrap_or_else(|_| std::net::IpAddr::from([0, 0, 0, 0]));
    // session id -> (side-A relay addr, side-B relay addr)
    let mut relay_addrs: HashMap<u64, (SocketAddr, SocketAddr)> = HashMap::new();
    loop {
        let (n, from) = match sock.recv_from(&mut buf).await {
            Ok(v) => v,
            Err(_) => continue,
        };
        let msg = match Msg::decode(&buf[..n]) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if let Msg::RelayRequest { peer } = msg {
            if let Some((session, side)) = coord.request_relay(from, peer, 0) {
                let pair = match relay_addrs.get(&session) {
                    Some(&pair) => pair,
                    None => {
                        // Bind two ephemeral relay sockets on the coordinator's
                        // own IP and spawn the opaque splice for this session.
                        let a = match UdpSocket::bind(SocketAddr::new(bind_ip, 0)).await {
                            Ok(s) => s,
                            Err(_) => continue,
                        };
                        let b = match UdpSocket::bind(SocketAddr::new(bind_ip, 0)).await {
                            Ok(s) => s,
                            Err(_) => continue,
                        };
                        let (a_addr, b_addr) = match (a.local_addr(), b.local_addr()) {
                            (Ok(x), Ok(y)) => (x, y),
                            _ => continue,
                        };
                        tokio::spawn(run_relay_pair(a, b, std::time::Duration::from_secs(30)));
                        relay_addrs.insert(session, (a_addr, b_addr));
                        (a_addr, b_addr)
                    }
                };
                let relay = match side {
                    Side::A => pair.0,
                    Side::B => pair.1,
                };
                let _ = sock
                    .send_to(&Msg::RelayGrant { session, relay }.encode(), from)
                    .await;
            }
            continue;
        }
        for (dst, reply) in coord.handle(from, msg) {
            let _ = sock.send_to(&reply.encode(), dst).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NodeKey;
    use std::net::{IpAddr, Ipv4Addr};
    use tokio::net::UdpSocket;
    use tokio::time::{Duration, timeout};

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

    #[tokio::test]
    async fn discover_reflexive_ignores_forged_bind_response_from_non_coordinator() {
        let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let coord_addr = coord_sock.local_addr().unwrap();
        tokio::spawn(run_coordinator(coord_sock));

        let client = NatClient::bind(NodeKey([2u8; 32]), coord_addr).await.unwrap();
        let client_addr = client.local_addr().await.unwrap();

        // A forger — some socket that is not the coordinator — races the
        // real coordinator reply with a bogus BindResponse.
        let forger = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let forged = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 9)), 5555);
        forger
            .send_to(&Msg::BindResponse { reflexive: forged }.encode(), client_addr)
            .await
            .unwrap();

        let reflexive = client.discover_reflexive().await.unwrap();
        assert_ne!(
            reflexive, forged,
            "a BindResponse from a non-coordinator sender must be ignored"
        );
        assert_eq!(reflexive.port(), client_addr.port());
    }

    #[tokio::test]
    async fn recv_punch_from_ignores_spoofed_punch_from_wrong_sender() {
        let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let coord_addr = coord_sock.local_addr().unwrap();
        tokio::spawn(run_coordinator(coord_sock));

        let a_key = NodeKey([0xaa; 32]);
        let a = NatClient::bind(a_key, coord_addr).await.unwrap();
        let b = NatClient::bind(NodeKey([0xbb; 32]), coord_addr).await.unwrap();
        // Sockets bind 0.0.0.0:0, so local_addr() reports the wildcard IP,
        // but a loopback send is observed from 127.0.0.1 — same caveat as
        // `client_discovers_its_reflexive_via_coordinator` above. Use the
        // address a real peer would actually observe.
        let a_addr = SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            a.local_addr().await.unwrap().port(),
        );
        let b_addr = SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            b.local_addr().await.unwrap().port(),
        );

        // A relay/third party sends a forged Punch — with a *different*
        // claimed identity, so the test can tell the two datagrams apart by
        // content — from its own socket, not A's rendezvous-resolved
        // address. It lands first.
        let relay = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        relay
            .send_to(&Msg::Punch { from: NodeKey([0xcc; 32]) }.encode(), b_addr)
            .await
            .unwrap();

        // A's real punch follows, from A's own socket, second.
        a.send_punch_to(b_addr).await.unwrap();

        let got = timeout(Duration::from_secs(2), b.recv_punch_from(a_addr))
            .await
            .expect("no timeout")
            .expect("recv");
        assert_eq!(got, Msg::Punch { from: a_key });
    }

    #[tokio::test]
    async fn two_clients_relay_opaque_datagrams_both_ways() {
        let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let coord_addr = coord_sock.local_addr().unwrap();
        tokio::spawn(run_coordinator(coord_sock));

        let a_key = NodeKey([0xaa; 32]);
        let b_key = NodeKey([0xbb; 32]);
        let a = NatClient::bind(a_key, coord_addr).await.unwrap();
        let b = NatClient::bind(b_key, coord_addr).await.unwrap();
        a.register().await.unwrap();
        b.register().await.unwrap();

        let (s_a, a_relay) = timeout(Duration::from_secs(2), a.request_relay(b_key))
            .await
            .expect("no timeout")
            .expect("grant a");
        let (s_b, b_relay) = timeout(Duration::from_secs(2), b.request_relay(a_key))
            .await
            .expect("no timeout")
            .expect("grant b");
        assert_eq!(s_a, s_b, "one session per pair");
        assert_ne!(a_relay, b_relay, "one relay port per side");

        // The relay learns a side's source on its first datagram, so the far
        // side's source must already be known before a payload can be
        // forwarded (real WireGuard retransmits). Sequence the sends: B first
        // (learned, dropped), then A (A->B delivered), then B again (B->A).
        b.relay_send(b_relay, b"drop-until-a-known").await.unwrap();
        a.relay_send(a_relay, b"opaque-ciphertext-A").await.unwrap();
        let got_b = timeout(Duration::from_secs(2), b.relay_recv())
            .await
            .expect("no timeout")
            .expect("recv b");
        assert_eq!(got_b, b"opaque-ciphertext-A");

        b.relay_send(b_relay, b"opaque-ciphertext-B").await.unwrap();
        let got_a = timeout(Duration::from_secs(2), a.relay_recv())
            .await
            .expect("no timeout")
            .expect("recv a");
        assert_eq!(got_a, b"opaque-ciphertext-B");
    }

    #[tokio::test]
    async fn relay_pair_tears_down_after_idle() {
        let a = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let b = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let handle = tokio::spawn(run_relay_pair(a, b, Duration::from_millis(50)));
        // No traffic on either side -> the task returns within a bounded time.
        timeout(Duration::from_secs(1), handle)
            .await
            .expect("relay pair did not idle out")
            .expect("join");
    }
}
