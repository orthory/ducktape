use std::net::SocketAddr;

use tokio::net::UdpSocket;

use crate::auth::{now_secs, sign_authenticator, AuthPolicy, CoordCap};
use crate::AuthRequest;
use crate::{Coordinator, Msg, NodeKey};
use commonware_cryptography::ed25519;

pub struct NatClient {
    sock: UdpSocket,
    key: NodeKey,
    coord: SocketAddr,
    coords: Vec<SocketAddr>,
    signer: Option<ed25519::PrivateKey>,
    cap: Option<CoordCap>,
}

impl NatClient {
    pub async fn bind(key: NodeKey, coord: SocketAddr) -> std::io::Result<Self> {
        let sock = UdpSocket::bind("0.0.0.0:0").await?;
        Ok(Self { sock, key, coord, coords: vec![coord], signer: None, cap: None })
    }

    /// Bind with an ordered set of coordinator hints (the reach `Vec`). The
    /// primary is `coords[0]`; single-coordinator methods use it, while
    /// `discover_reflexive_failover` walks the whole set.
    pub async fn bind_multi(key: NodeKey, coords: Vec<SocketAddr>) -> std::io::Result<Self> {
        let coord = *coords.first().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "empty coordinator set")
        })?;
        let sock = UdpSocket::bind("0.0.0.0:0").await?;
        Ok(Self { sock, key, coord, coords, signer: None, cap: None })
    }

    /// Bind with an authenticating identity: every request to the coordinator
    /// is wrapped in an `AuthRequest` signed by `signer`, carrying `cap`
    /// (private mode) or `None` (public / PoP-only).
    pub async fn bind_multi_auth(
        key: NodeKey,
        coords: Vec<SocketAddr>,
        signer: ed25519::PrivateKey,
        cap: Option<CoordCap>,
    ) -> std::io::Result<Self> {
        let coord = *coords.first().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "empty coordinator set")
        })?;
        let sock = UdpSocket::bind("0.0.0.0:0").await?;
        Ok(Self { sock, key, coord, coords, signer: Some(signer), cap })
    }

    /// Encode a client→coordinator request, wrapping it in a signed
    /// `AuthRequest` when this client authenticates, or sending it bare
    /// otherwise (tests / no-auth dev path).
    fn authed(&self, inner: Msg) -> Vec<u8> {
        match &self.signer {
            Some(signer) => {
                let auth = sign_authenticator(signer, &inner.encode(), now_secs(), self.cap.clone());
                AuthRequest { inner, auth }.encode()
            }
            None => inner.encode(),
        }
    }

    pub async fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.sock.local_addr()
    }

    pub async fn discover_reflexive(&self) -> std::io::Result<SocketAddr> {
        self.sock
            .send_to(&self.authed(Msg::BindRequest { from: self.key }), self.coord)
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
    ///
    /// Crucially, on success this REPOINTS `self.coord` at the coordinator that
    /// actually answered, so every subsequent `register`/`lookup` uses the live
    /// coordinator too. Without that, failover would only cover reflexive
    /// discovery while the dead primary stayed uniquely load-bearing for the
    /// rest of the join path.
    pub async fn discover_reflexive_failover(
        &mut self,
        per_try: std::time::Duration,
    ) -> std::io::Result<(usize, SocketAddr)> {
        // Iterate a local snapshot of the hint set so the loop's borrow does not
        // conflict with repointing `self.coord` on success.
        let coords = self.coords.clone();
        for (i, c) in coords.iter().copied().enumerate() {
            self.sock
                .send_to(&self.authed(Msg::BindRequest { from: self.key }), c)
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
                Ok(Ok(reflexive)) => {
                    // Repoint the join path at the coordinator that answered.
                    self.coord = c;
                    return Ok((i, reflexive));
                }
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
            .send_to(&self.authed(Msg::Register { key: self.key }), self.coord)
            .await?;
        Ok(())
    }

    /// Republish this node's reflexive to the coordinator after a NAT rebind,
    /// under a strictly-higher `nonce` than any prior advert for this key. This
    /// is the wire path a rebound node uses to move its mapping: the coordinator
    /// re-observes the datagram's NEW source and applies the nonce-staleness
    /// guard, so a replayed/reordered lower-or-equal nonce cannot supersede the
    /// fresh mapping — unlike `register`, whose nonce-0 baseline a stale
    /// duplicate could otherwise roll back.
    pub async fn readvertise(&self, nonce: u64) -> std::io::Result<()> {
        self.sock
            .send_to(&self.authed(Msg::Readvertise { key: self.key, nonce }), self.coord)
            .await?;
        Ok(())
    }

    /// Ask the coordinator to resolve `peer`'s reflexive address via the real
    /// Lookup/LookupResponse rendezvous path (never the peer's socket
    /// directly).
    pub async fn lookup(&self, peer: NodeKey) -> std::io::Result<SocketAddr> {
        self.sock
            .send_to(&self.authed(Msg::Lookup { key: peer }), self.coord)
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
}

/// The coordinator event loop: decode control datagrams (authenticated or, under
/// a fully-open policy, legacy), enforce the auth policy, feed the pure handler,
/// send replies. Pure rendezvous — never binds a data socket, never carries
/// peer traffic.
pub async fn run_coordinator(sock: UdpSocket, policy: AuthPolicy) {
    let mut coord = Coordinator::with_policy(policy);
    // Big enough for an AuthRequest with a cap (~219 bytes worst case).
    let mut buf = [0u8; 512];
    loop {
        let (n, from) = match sock.recv_from(&mut buf).await {
            Ok(v) => v,
            Err(_) => continue,
        };
        let now = now_secs();
        // Tag 11 -> authenticated envelope; anything else -> legacy Msg. The two
        // are mutually exclusive by tag, so try the envelope first and fall back.
        let out = match AuthRequest::decode(&buf[..n]) {
            Ok(req) => coord.handle_auth(from, req, now),
            Err(_) => match Msg::decode(&buf[..n]) {
                Ok(m) => coord.handle_legacy(from, m),
                Err(_) => continue,
            },
        };
        for (dst, reply) in out {
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
    async fn authorized_client_rendezvous_under_private_policy_but_unauthorized_is_dropped() {
        use crate::auth::{mint_coord_cap, AuthPolicy};
        use commonware_cryptography::{ed25519, Signer as _};

        let g = ed25519::PrivateKey::from_seed(100);
        let policy = AuthPolicy::Private { genesis_set: vec![g.public_key()] };

        let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let coord_addr = coord_sock.local_addr().unwrap();
        tokio::spawn(run_coordinator(coord_sock, policy));

        // Two authorized nodes (joiners) with genesis caps.
        let a_signer = ed25519::PrivateKey::from_seed(200);
        let b_signer = ed25519::PrivateKey::from_seed(201);
        let a_key = { let mut k=[0u8;32]; k.copy_from_slice(a_signer.public_key().as_ref()); NodeKey(k) };
        let b_key = { let mut k=[0u8;32]; k.copy_from_slice(b_signer.public_key().as_ref()); NodeKey(k) };
        let a_cap = mint_coord_cap(&g, a_key, crate::auth::now_secs() + 3600);
        let b_cap = mint_coord_cap(&g, b_key, crate::auth::now_secs() + 3600);

        let a = NatClient::bind_multi_auth(a_key, vec![coord_addr], a_signer, Some(a_cap)).await.unwrap();
        let b = NatClient::bind_multi_auth(b_key, vec![coord_addr], b_signer, Some(b_cap)).await.unwrap();
        a.register().await.unwrap();
        b.register().await.unwrap();

        // Per the committed wire semantics, a `Lookup`'s `subject_key()` is the
        // LOOKED-UP key, so under Private policy the authenticator must be signed
        // by (and admitted for) that key — a node resolves its OWN mapping. This
        // proves an authorized register+lookup completes end-to-end over the real
        // signed UDP path (a cross-node `a.lookup(b_key)` is impossible here: a
        // does not hold b's signer, so its PoP would fail and be dropped).
        let a_reflexive = timeout(Duration::from_secs(2), a.lookup(a_key)).await.expect("no timeout").expect("lookup");
        assert_eq!(a_reflexive.port(), a.local_addr().await.unwrap().port());
        let b_reflexive = timeout(Duration::from_secs(2), b.lookup(b_key)).await.expect("no timeout").expect("lookup");
        assert_eq!(b_reflexive.port(), b.local_addr().await.unwrap().port());

        // Unauthorized: a node with NO signer (bare Msg) cannot register under
        // Private policy — its lookup for itself finds nothing.
        let outsider = NatClient::bind(NodeKey([0xcd; 32]), coord_addr).await.unwrap();
        outsider.register().await.unwrap(); // dropped by handle_legacy
        let miss = timeout(Duration::from_millis(500), outsider.lookup(NodeKey([0xcd; 32]))).await;
        assert!(miss.is_err() || miss.unwrap().is_err(), "unauthenticated register never created a mapping");
    }

    #[tokio::test]
    async fn dead_primary_falls_through_to_live_secondary() {
        // A live coordinator (the secondary).
        let live = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let live_addr = live.local_addr().unwrap();
        tokio::spawn(run_coordinator(live, crate::auth::AuthPolicy::Open { require_pop: false }));

        // A DEAD primary: a bound socket nobody ever serves. Datagrams sent to
        // it are buffered and never answered, so the per-try budget elapses.
        let dead = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let dead_addr = dead.local_addr().unwrap();

        let mut client = NatClient::bind_multi(NodeKey([1u8; 32]), vec![dead_addr, live_addr])
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
        tokio::spawn(run_coordinator(live, crate::auth::AuthPolicy::Open { require_pop: false }));
        let dead = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let dead_addr = dead.local_addr().unwrap();

        let mut client = NatClient::bind_multi(NodeKey([2u8; 32]), vec![live_addr, dead_addr])
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
        tokio::spawn(run_coordinator(coord_sock, crate::auth::AuthPolicy::Open { require_pop: false }));

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
        tokio::spawn(run_coordinator(coord_sock, crate::auth::AuthPolicy::Open { require_pop: false }));

        let client = NatClient::bind(NodeKey([2u8; 32]), coord_addr).await.unwrap();
        let client_addr = client.local_addr().await.unwrap();

        // A forger — some socket that is not the coordinator — races the
        // real coordinator reply with a bogus BindResponse. The client binds
        // the wildcard, so target its port on loopback (macOS refuses a send
        // to a 0.0.0.0 destination; the on-path forger is loopback here).
        let forger = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let client_dst = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), client_addr.port());
        let forged = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 9)), 5555);
        forger
            .send_to(&Msg::BindResponse { reflexive: forged }.encode(), client_dst)
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
        tokio::spawn(run_coordinator(coord_sock, crate::auth::AuthPolicy::Open { require_pop: false }));

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

        // A third party sends a forged Punch — with a *different* claimed
        // identity, so the test can tell the two datagrams apart by content —
        // from its own socket, not A's rendezvous-resolved address. It lands
        // first.
        let forger = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        forger
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
    async fn direct_path_survives_coordinator_shutdown() {
        let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let coord_addr = coord_sock.local_addr().unwrap();
        let coord = tokio::spawn(run_coordinator(coord_sock, crate::auth::AuthPolicy::Open { require_pop: false }));

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
    async fn wire_readvertise_supersedes_stale_mapping_over_the_real_udp_path() {
        // The nonce-gated rebind must be reachable over the LIVE protocol, not
        // only via the in-process `Coordinator::readvertise` API: a rebound node
        // sends `Msg::Readvertise` over UDP and a peer re-resolves the new mapping.
        let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let coord_addr = coord_sock.local_addr().unwrap();
        tokio::spawn(run_coordinator(coord_sock, crate::auth::AuthPolicy::Open { require_pop: false }));

        let a_key = NodeKey([0xaa; 32]);
        let b_key = NodeKey([0xbb; 32]);
        let a = NatClient::bind(a_key, coord_addr).await.unwrap();
        let b = NatClient::bind(b_key, coord_addr).await.unwrap();
        a.register().await.unwrap();
        b.register().await.unwrap();

        // B resolves A's original mapping.
        let a_first = timeout(Duration::from_secs(2), b.lookup(a_key))
            .await
            .expect("no timeout")
            .expect("lookup a");
        assert_eq!(a_first.port(), a.local_addr().await.unwrap().port());

        // A rebinds: model the fresh reflexive with a NEW socket, and republish it
        // over the wire under a strictly-higher nonce. The coordinator observes
        // the new socket's source and must supersede the stale mapping.
        let a2 = NatClient::bind(a_key, coord_addr).await.unwrap();
        let a2_port = a2.local_addr().await.unwrap().port();
        assert_ne!(a2_port, a_first.port(), "the rebound socket has a fresh port");
        a2.readvertise(1).await.unwrap();

        // B re-resolves and now sees A's NEW mapping, not the stale one. Poll to
        // absorb cross-socket datagram-scheduling jitter (bounded).
        let mut resolved = None;
        for _ in 0..50 {
            if let Ok(Ok(addr)) = timeout(Duration::from_millis(100), b.lookup(a_key)).await
                && addr.port() == a2_port
            {
                resolved = Some(addr);
                break;
            }
        }
        let new = resolved.expect("B must re-resolve A's superseding mapping over the wire");
        assert_eq!(new.port(), a2_port);
        assert_ne!(
            new.port(),
            a_first.port(),
            "the wire Readvertise superseded the stale mapping end-to-end"
        );
    }

    #[tokio::test]
    async fn failover_repoints_coord_so_join_path_uses_the_live_secondary() {
        // Discovery failover is worthless if register/lookup still hardcode
        // the dead primary. After failover, the WHOLE join path must use the
        // coordinator that answered.
        let live = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let live_addr = live.local_addr().unwrap();
        tokio::spawn(run_coordinator(live, crate::auth::AuthPolicy::Open { require_pop: false }));
        let dead = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let dead_addr = dead.local_addr().unwrap();

        // A joins via failover: primary dead, secondary live.
        let mut a = NatClient::bind_multi(NodeKey([1u8; 32]), vec![dead_addr, live_addr])
            .await
            .unwrap();
        let (idx, _reflexive) = timeout(
            Duration::from_secs(2),
            a.discover_reflexive_failover(Duration::from_millis(150)),
        )
        .await
        .expect("bounded")
        .expect("secondary answers");
        assert_eq!(idx, 1, "the live secondary answered discovery");

        // B registers directly with the live secondary.
        let b_key = NodeKey([2u8; 32]);
        let b = NatClient::bind(b_key, live_addr).await.unwrap();
        b.register().await.unwrap();

        // A registers and looks B up. If `self.coord` still pointed at the dead
        // primary, this Register would land nowhere and the Lookup would hang
        // (bounded by the timeout) and fail — the whole point of the fix.
        a.register().await.unwrap();
        let b_reflexive = timeout(Duration::from_secs(2), a.lookup(b_key))
            .await
            .expect("lookup must reach the live secondary, not the dead primary")
            .expect("b resolved");
        assert_eq!(
            b_reflexive.port(),
            b.local_addr().await.unwrap().port(),
            "A's join path resolved B via the coordinator that actually answered"
        );
    }
}
