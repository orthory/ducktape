use std::net::SocketAddr;

use tokio::net::UdpSocket;

use crate::{Coordinator, Msg, NodeKey, Side};

pub struct NatClient {
    sock: UdpSocket,
    key: NodeKey,
    coord: SocketAddr,
    coords: Vec<SocketAddr>,
}

impl NatClient {
    pub async fn bind(key: NodeKey, coord: SocketAddr) -> std::io::Result<Self> {
        let sock = UdpSocket::bind("0.0.0.0:0").await?;
        Ok(Self { sock, key, coord, coords: vec![coord] })
    }

    /// Bind with an ordered set of coordinator hints (the reach `Vec`). The
    /// primary is `coords[0]`; single-coordinator methods use it, while
    /// `discover_reflexive_failover` walks the whole set.
    pub async fn bind_multi(key: NodeKey, coords: Vec<SocketAddr>) -> std::io::Result<Self> {
        let coord = *coords.first().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "empty coordinator set")
        })?;
        let sock = UdpSocket::bind("0.0.0.0:0").await?;
        Ok(Self { sock, key, coord, coords })
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

    /// Discover this node's reflexive address, trying each coordinator hint in
    /// order and falling through a dead/unresponsive one after `per_try` to the
    /// next. Returns the index of the coordinator that answered plus the
    /// reflexive it observed. Total wait is bounded by `per_try * coords.len()`,
    /// so a dead coordinator never wedges the joiner — the coordinator set is
    /// not uniquely load-bearing.
    pub async fn discover_reflexive_failover(
        &self,
        per_try: std::time::Duration,
    ) -> std::io::Result<(usize, SocketAddr)> {
        for (i, &c) in self.coords.iter().enumerate() {
            self.sock
                .send_to(&Msg::BindRequest { from: self.key }.encode(), c)
                .await?;
            let attempt = async {
                let mut buf = [0u8; 64];
                loop {
                    let (n, from) = self.sock.recv_from(&mut buf).await?;
                    // Only THIS coordinator's own reply counts; a stray/forged
                    // datagram from anyone else is ignored (same rule as the
                    // single-coordinator discover_reflexive).
                    if from != c {
                        continue;
                    }
                    if let Ok(Msg::BindResponse { reflexive }) = Msg::decode(&buf[..n]) {
                        return Ok::<SocketAddr, std::io::Error>(reflexive);
                    }
                }
            };
            match tokio::time::timeout(per_try, attempt).await {
                Ok(Ok(reflexive)) => return Ok((i, reflexive)),
                // Timeout or socket error on this coordinator -> try the next.
                _ => continue,
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::AddrNotAvailable,
            "no coordinator in the hint set responded",
        ))
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
/// datagram and PINNING it thereafter, and tear down after `idle` of total
/// inactivity. Never decodes a payload — it holds only the two learned source
/// addresses.
pub async fn run_relay_pair(a_sock: UdpSocket, b_sock: UdpSocket, idle: std::time::Duration) {
    let mut a_src: Option<SocketAddr> = None;
    let mut b_src: Option<SocketAddr> = None;
    let mut a_buf = [0u8; 1500];
    let mut b_buf = [0u8; 1500];
    // A single idle deadline, reset only when a datagram is ACCEPTED (passes
    // source pinning). A spoofer spraying wrong-source datagrams is dropped
    // without refreshing this, so it can neither hijack routing nor hold the
    // session open past `idle`.
    let deadline = tokio::time::sleep(idle);
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            r = a_sock.recv_from(&mut a_buf) => {
                let (n, from) = match r { Ok(v) => v, Err(_) => continue };
                // Learn A's source on the first datagram, then pin it: a
                // datagram from any other source on A's socket is an injection
                // attempt (session hijack) and is dropped without disturbing the
                // learned source or the idle deadline.
                match a_src {
                    Some(pinned) if pinned != from => continue,
                    _ => a_src = Some(from),
                }
                deadline.as_mut().reset(tokio::time::Instant::now() + idle);
                if let Some(dst) = b_src {
                    let _ = b_sock.send_to(&a_buf[..n], dst).await;
                }
            }
            r = b_sock.recv_from(&mut b_buf) => {
                let (n, from) = match r { Ok(v) => v, Err(_) => continue };
                match b_src {
                    Some(pinned) if pinned != from => continue,
                    _ => b_src = Some(from),
                }
                deadline.as_mut().reset(tokio::time::Instant::now() + idle);
                if let Some(dst) = a_src {
                    let _ = a_sock.send_to(&b_buf[..n], dst).await;
                }
            }
            _ = &mut deadline => {
                // `idle` elapsed with no accepted datagram on either side.
                // Bounded teardown: both sockets drop when this task returns.
                return;
            }
        }
    }
}

/// The coordinator event loop: decode control datagrams, feed the pure handler,
/// send replies. `RelayRequest` is handled specially — it must bind real relay
/// sockets, which the transport-free `Coordinator::handle` cannot do. Relay
/// sessions idle out after 30s.
pub async fn run_coordinator(sock: UdpSocket) {
    run_coordinator_with_idle(sock, std::time::Duration::from_secs(30)).await
}

/// As [`run_coordinator`], but with a caller-chosen relay idle timeout. Split
/// out so tests can force fast teardown; production calls `run_coordinator`.
pub async fn run_coordinator_with_idle(sock: UdpSocket, relay_idle: std::time::Duration) {
    use std::collections::HashMap;

    let mut coord = Coordinator::new();
    let mut buf = [0u8; 64];
    let bind_ip = sock
        .local_addr()
        .map(|a| a.ip())
        .unwrap_or_else(|_| std::net::IpAddr::from([0, 0, 0, 0]));
    // session id -> (side-A relay addr, side-B relay addr)
    let mut relay_addrs: HashMap<u64, (SocketAddr, SocketAddr)> = HashMap::new();
    // Each relay-pair task signals its session id here once it idles out and its
    // sockets are gone. The coordinator can't observe data-plane relay activity
    // (it flows through the spawned splice, not this control socket), so a
    // wall-clock prune would be blind to live sessions or tear down active
    // ones. Reclaiming exactly on task completion keeps `relay_addrs` and the
    // coordinator's session table in lockstep with the live sockets, so a
    // re-request after teardown allocates a FRESH session instead of handing
    // back a dead relay port.
    let (done_tx, mut done_rx) = tokio::sync::mpsc::channel::<u64>(64);
    loop {
        let (n, from) = tokio::select! {
            r = sock.recv_from(&mut buf) => match r {
                Ok(v) => v,
                Err(_) => continue,
            },
            Some(session) = done_rx.recv() => {
                relay_addrs.remove(&session);
                coord.release_relay(session);
                continue;
            }
        };
        let msg = match Msg::decode(&buf[..n]) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if let Msg::RelayRequest { peer } = msg {
            // Reclaim any sessions whose splice has already torn down before
            // allocating, so a re-request can never be handed a dead relay port
            // (closes the small window between task exit and the select branch
            // above draining the signal).
            while let Ok(session) = done_rx.try_recv() {
                relay_addrs.remove(&session);
                coord.release_relay(session);
            }
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
                        // Spawn the opaque splice; when it idles out, signal its
                        // session id so the coordinator reclaims the entry.
                        let done = done_tx.clone();
                        tokio::spawn(async move {
                            run_relay_pair(a, b, relay_idle).await;
                            let _ = done.send(session).await;
                        });
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
    async fn dead_primary_falls_through_to_live_secondary() {
        // A live coordinator (the secondary).
        let live = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let live_addr = live.local_addr().unwrap();
        tokio::spawn(run_coordinator(live));

        // A DEAD primary: a bound socket nobody ever serves. Datagrams sent to
        // it are buffered and never answered, so the per-try budget elapses.
        let dead = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let dead_addr = dead.local_addr().unwrap();

        let client = NatClient::bind_multi(NodeKey([1u8; 32]), vec![dead_addr, live_addr])
            .await
            .unwrap();
        let (idx, reflexive) =
            timeout(Duration::from_secs(2), client.discover_reflexive_failover(Duration::from_millis(150)))
                .await
                .expect("failover must be bounded, never stuck")
                .expect("secondary answers");

        assert_eq!(idx, 1, "the dead primary is skipped; the live secondary answers");
        assert_eq!(reflexive.port(), client.local_addr().await.unwrap().port());
    }

    #[tokio::test]
    async fn no_single_coordinator_is_load_bearing_either_position_works() {
        // Same live coordinator, but now in PRIMARY position with a dead
        // secondary: discovery still succeeds, via index 0. Together with the
        // previous test this proves neither position is uniquely required.
        let live = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let live_addr = live.local_addr().unwrap();
        tokio::spawn(run_coordinator(live));
        let dead = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let dead_addr = dead.local_addr().unwrap();

        let client = NatClient::bind_multi(NodeKey([2u8; 32]), vec![live_addr, dead_addr])
            .await
            .unwrap();
        let (idx, reflexive) =
            timeout(Duration::from_secs(2), client.discover_reflexive_failover(Duration::from_millis(150)))
                .await
                .expect("no timeout")
                .expect("primary answers");
        assert_eq!(idx, 0, "a live primary is used directly");
        assert_eq!(reflexive.port(), client.local_addr().await.unwrap().port());
    }

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
        // side's source must be known before a payload can be forwarded, and
        // that first datagram is dropped. `run_relay_pair`'s `tokio::select!`
        // also processes the two sockets in a nondeterministic order when both
        // are ready, so a fixed send sequence cannot guarantee delivery. Model
        // real WireGuard, which simply retransmits until a reply arrives: each
        // side re-sends its OWN opaque payload until the far side receives it.
        // `a` only ever emits `A_PAYLOAD` and `b` only ever emits `B_PAYLOAD`,
        // and the relay only forwards a->b and b->a, so whatever a side
        // receives is unambiguously the peer's ciphertext.
        const A_PAYLOAD: &[u8] = b"opaque-ciphertext-A";
        const B_PAYLOAD: &[u8] = b"opaque-ciphertext-B";

        // Prime both sources so each direction can eventually forward.
        a.relay_send(a_relay, A_PAYLOAD).await.unwrap();
        b.relay_send(b_relay, B_PAYLOAD).await.unwrap();

        // A -> B, retransmitting until B receives A's ciphertext (bounded so a
        // genuinely broken relay fails fast instead of hanging CI).
        let mut got_b = None;
        for _ in 0..50 {
            a.relay_send(a_relay, A_PAYLOAD).await.unwrap();
            if let Ok(v) = timeout(Duration::from_millis(100), b.relay_recv()).await {
                got_b = Some(v.expect("recv b"));
                break;
            }
        }
        assert_eq!(got_b.expect("B received A's ciphertext").as_slice(), A_PAYLOAD);

        // B -> A, retransmitting until A receives B's ciphertext.
        let mut got_a = None;
        for _ in 0..50 {
            b.relay_send(b_relay, B_PAYLOAD).await.unwrap();
            if let Ok(v) = timeout(Duration::from_millis(100), a.relay_recv()).await {
                got_a = Some(v.expect("recv a"));
                break;
            }
        }
        assert_eq!(got_a.expect("A received B's ciphertext").as_slice(), B_PAYLOAD);
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

    #[tokio::test]
    async fn run_relay_pair_pins_source_against_injection() {
        // The relay's two per-side sockets.
        let a_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let b_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let a_relay = a_sock.local_addr().unwrap();
        let b_relay = b_sock.local_addr().unwrap();
        tokio::spawn(run_relay_pair(a_sock, b_sock, Duration::from_secs(30)));

        let a = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let b = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let attacker = UdpSocket::bind("127.0.0.1:0").await.unwrap();

        // 1. A sends first so the relay learns and PINS a_src = A's address.
        a.send_to(b"a", a_relay).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;

        // 2. The attacker sprays A's relay socket from its OWN address. With
        //    source pinning this is dropped and a_src stays pinned to A;
        //    without it, a_src would be hijacked to the attacker and B's return
        //    traffic redirected there.
        attacker.send_to(b"inject", a_relay).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;

        // 3. B sends toward A. It must reach the REAL A, never the attacker.
        //    Retransmit to absorb scheduling nondeterminism (as WireGuard does).
        const B_PAYLOAD: &[u8] = b"from-b-to-a";
        let mut got_a = None;
        for _ in 0..50 {
            b.send_to(B_PAYLOAD, b_relay).await.unwrap();
            let mut buf = [0u8; 1500];
            if let Ok(Ok((n, _))) =
                timeout(Duration::from_millis(100), a.recv_from(&mut buf)).await
            {
                got_a = Some(buf[..n].to_vec());
                break;
            }
        }
        assert_eq!(
            got_a.as_deref(),
            Some(B_PAYLOAD),
            "B->A must reach the real A; a pinned relay ignores the injector"
        );

        // The attacker must never have received relayed traffic.
        let mut buf = [0u8; 1500];
        assert!(
            timeout(Duration::from_millis(200), attacker.recv_from(&mut buf))
                .await
                .is_err(),
            "the injector must not receive relayed traffic (no session hijack)"
        );
    }

    #[tokio::test]
    async fn direct_path_survives_coordinator_shutdown() {
        let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let coord_addr = coord_sock.local_addr().unwrap();
        let coord = tokio::spawn(run_coordinator(coord_sock));

        let a_key = NodeKey([0xaa; 32]);
        let b_key = NodeKey([0xbb; 32]);
        let a = NatClient::bind(a_key, coord_addr).await.unwrap();
        let b = NatClient::bind(b_key, coord_addr).await.unwrap();
        a.register().await.unwrap();
        b.register().await.unwrap();

        // Rendezvous via the coordinator to learn each other's addresses.
        let _b_reflexive = timeout(Duration::from_secs(2), a.lookup(b_key))
            .await
            .expect("no timeout")
            .expect("lookup");
        let b_addr = SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            b.local_addr().await.unwrap().port(),
        );
        let a_addr = SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            a.local_addr().await.unwrap().port(),
        );

        // The coordinator dies.
        coord.abort();

        // The direct path still works: A sends straight to B, no coordinator.
        // Retransmit to absorb any scheduling nondeterminism (as WireGuard does)
        // and to prove the path survives regardless of send order.
        let mut got = None;
        for _ in 0..50 {
            a.send_punch_to(b_addr).await.unwrap();
            if let Ok(r) = timeout(Duration::from_millis(100), b.recv_punch_from(a_addr)).await {
                got = Some(r.expect("recv"));
                break;
            }
        }
        assert_eq!(
            got.expect("direct path must survive coordinator downtime"),
            Msg::Punch { from: a_key }
        );
    }

    #[tokio::test]
    async fn relay_setup_requires_a_live_coordinator() {
        let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let coord_addr = coord_sock.local_addr().unwrap();
        let coord = tokio::spawn(run_coordinator(coord_sock));

        let a = NatClient::bind(NodeKey([0xaa; 32]), coord_addr).await.unwrap();
        a.register().await.unwrap();

        // Coordinator down -> a relayed path cannot even be established: the
        // grant never comes. (Unlike a punched path, which needs nothing.)
        coord.abort();
        let res = timeout(Duration::from_millis(400), a.request_relay(NodeKey([0xbb; 32]))).await;
        assert!(
            res.is_err(),
            "without a live coordinator a relay session cannot be allocated"
        );
    }

    #[tokio::test]
    async fn coordinator_reclaims_idle_relay_and_regrants_fresh_session() {
        let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let coord_addr = coord_sock.local_addr().unwrap();
        // Short relay idle so the splice tears down quickly.
        tokio::spawn(run_coordinator_with_idle(coord_sock, Duration::from_millis(150)));

        let a_key = NodeKey([0xaa; 32]);
        let b_key = NodeKey([0xbb; 32]);
        let a = NatClient::bind(a_key, coord_addr).await.unwrap();
        let b = NatClient::bind(b_key, coord_addr).await.unwrap();
        a.register().await.unwrap();
        b.register().await.unwrap();

        // First allocation for the pair.
        let (s0, _relay0) = timeout(Duration::from_secs(2), a.request_relay(b_key))
            .await
            .expect("no timeout")
            .expect("grant");

        // Let the relay pair idle out (no data-plane traffic) and the
        // coordinator reclaim the session.
        tokio::time::sleep(Duration::from_millis(450)).await;

        // Re-request the SAME pair: the coordinator must allocate a fresh
        // session and bind a live relay, not hand back the torn-down one.
        let (s1, relay1) = timeout(Duration::from_secs(2), a.request_relay(b_key))
            .await
            .expect("no timeout")
            .expect("regrant");
        assert_ne!(
            s0, s1,
            "a re-request after teardown must allocate a new session, not reuse the dead one"
        );

        // The fresh relay must actually deliver: prove it end to end so the test
        // fails if the grant points at a dead port. Both sides drive the new
        // session and retransmit their own opaque payload until it arrives.
        let (_s1b, relay1_b) = timeout(Duration::from_secs(2), b.request_relay(a_key))
            .await
            .expect("no timeout")
            .expect("regrant b");
        assert_ne!(relay1, relay1_b, "one relay port per side");
        const PAYLOAD: &[u8] = b"post-reclaim-A";
        a.relay_send(relay1, PAYLOAD).await.unwrap();
        b.relay_send(relay1_b, b"post-reclaim-B").await.unwrap();
        let mut got_b = None;
        for _ in 0..50 {
            a.relay_send(relay1, PAYLOAD).await.unwrap();
            if let Ok(v) = timeout(Duration::from_millis(100), b.relay_recv()).await {
                got_b = Some(v.expect("recv b"));
                break;
            }
        }
        assert_eq!(
            got_b.expect("B received A's ciphertext on the fresh relay").as_slice(),
            PAYLOAD,
            "the re-granted relay must be live, not a stale dead port"
        );
    }
}
