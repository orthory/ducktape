use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::UdpSocket;
use tokio::sync::{Mutex, mpsc};

use crate::AuthRequest;
use crate::auth::{AuthPolicy, CoordCap, now_secs, sign_authenticator};
use crate::{Coordinator, Msg, NodeKey};
use commonware_cryptography::ed25519;

/// where a [`NatClient`]'s datagrams ride.
///
/// hole punching only works when the punch and the tunnel share one
/// 5-tuple: the pinhole a punch opens admits traffic to the ADDRESS AND
/// PORT it originated from, so a punch sent from a socket the tunnel does
/// not use vouches for nothing. `Shared` is that fix (the overlay-net
/// ADR's phase 3): the client rides the node's own WireGuard underlay
/// socket — sends go out through it directly, and receives are the
/// underlay demux's non-WireGuard lane. `Owned` is the standalone posture
/// (a socket of this client's own), kept for the TUN backend, whose
/// in-device socket cannot be shared.
pub enum NatSocket {
    Owned(UdpSocket),
    Shared {
        /// the underlay socket, for sends (concurrent-safe by itself).
        socket: Arc<UdpSocket>,
        /// the demux's bypass lane: every inbound datagram on the underlay
        /// that is not WireGuard. behind a `Mutex` only to keep `&self`
        /// receive methods — the receive side has a single consumer (the
        /// rendezvous pump) by construction.
        bypass: Mutex<mpsc::Receiver<(Vec<u8>, SocketAddr)>>,
        /// what `local_addr` answers: the shared socket's bound address.
        local: SocketAddr,
    },
}

impl NatSocket {
    /// wrap a shared underlay socket + its bypass lane.
    pub fn shared(
        socket: Arc<UdpSocket>,
        bypass: mpsc::Receiver<(Vec<u8>, SocketAddr)>,
    ) -> std::io::Result<Self> {
        let local = socket.local_addr()?;
        Ok(Self::Shared {
            socket,
            bypass: Mutex::new(bypass),
            local,
        })
    }

    async fn send_to(&self, buf: &[u8], dst: SocketAddr) -> std::io::Result<usize> {
        match self {
            Self::Owned(sock) => sock.send_to(buf, dst).await,
            Self::Shared { socket, local, .. } => {
                // the shared underlay binds dual-stack `[::]` — a V4
                // destination (a v4 coordinator, a v4 reflexive) must ride
                // it as v4-MAPPED v6, or the send is EINVAL on macOS (and
                // family-mismatched everywhere).
                let dst = match (local, dst) {
                    (SocketAddr::V6(_), SocketAddr::V4(v4)) => SocketAddr::new(
                        std::net::IpAddr::V6(v4.ip().to_ipv6_mapped()),
                        v4.port(),
                    ),
                    _ => dst,
                };
                socket.send_to(buf, dst).await
            }
        }
    }

    async fn recv_from(&self, buf: &mut [u8]) -> std::io::Result<(usize, SocketAddr)> {
        match self {
            Self::Owned(sock) => sock.recv_from(buf).await,
            Self::Shared { bypass, .. } => {
                let mut lane = bypass.lock().await;
                let (datagram, src) = lane.recv().await.ok_or_else(|| {
                    // the underlay (and its demux pump) is gone — the socket
                    // this client rode no longer exists.
                    std::io::Error::new(std::io::ErrorKind::NotConnected, "underlay demux closed")
                })?;
                let len = datagram.len().min(buf.len());
                buf[..len].copy_from_slice(&datagram[..len]);
                // a v4 peer observed through the dual-stack underlay reports
                // as `::ffff:a.b.c.d` — canonicalize to V4 so reply/punch
                // source validation matches the V4 addresses the coordinator
                // hands out.
                Ok((len, canonical_v4(src)))
            }
        }
    }

    fn local_addr(&self) -> std::io::Result<SocketAddr> {
        match self {
            Self::Owned(sock) => sock.local_addr(),
            Self::Shared { local, .. } => Ok(*local),
        }
    }
}

/// collapse a v4-mapped v6 address (`::ffff:a.b.c.d`) to its canonical V4
/// form; anything else passes through.
fn canonical_v4(addr: SocketAddr) -> SocketAddr {
    match addr {
        SocketAddr::V6(v6) => match v6.ip().to_ipv4_mapped() {
            Some(v4) => SocketAddr::new(std::net::IpAddr::V4(v4), v6.port()),
            None => addr,
        },
        SocketAddr::V4(_) => addr,
    }
}

pub struct NatClient {
    sock: NatSocket,
    key: NodeKey,
    coord: SocketAddr,
    coords: Vec<SocketAddr>,
    signer: Option<ed25519::PrivateKey>,
    cap: Option<CoordCap>,
}

impl NatClient {
    /// Build a client over an explicit transport — the shared-underlay path
    /// (see [`NatSocket`]); the `bind*` constructors below cover the owned
    /// one. Authenticates like [`Self::bind_multi_auth`] when `signer` is
    /// set, sends bare requests otherwise.
    pub fn with_socket(
        sock: NatSocket,
        key: NodeKey,
        coords: Vec<SocketAddr>,
        signer: Option<ed25519::PrivateKey>,
        cap: Option<CoordCap>,
    ) -> std::io::Result<Self> {
        let coord = *coords.first().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "empty coordinator set")
        })?;
        Ok(Self {
            sock,
            key,
            coord,
            coords,
            signer,
            cap,
        })
    }
    pub async fn bind(key: NodeKey, coord: SocketAddr) -> std::io::Result<Self> {
        let sock = NatSocket::Owned(UdpSocket::bind("0.0.0.0:0").await?);
        Ok(Self {
            sock,
            key,
            coord,
            coords: vec![coord],
            signer: None,
            cap: None,
        })
    }

    /// Bind with an ordered set of coordinator hints (the reach `Vec`). The
    /// primary is `coords[0]`; single-coordinator methods use it, while
    /// `discover_reflexive_failover` walks the whole set.
    pub async fn bind_multi(key: NodeKey, coords: Vec<SocketAddr>) -> std::io::Result<Self> {
        let coord = *coords.first().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "empty coordinator set")
        })?;
        let sock = NatSocket::Owned(UdpSocket::bind("0.0.0.0:0").await?);
        Ok(Self {
            sock,
            key,
            coord,
            coords,
            signer: None,
            cap: None,
        })
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
        let sock = NatSocket::Owned(UdpSocket::bind("0.0.0.0:0").await?);
        Ok(Self {
            sock,
            key,
            coord,
            coords,
            signer: Some(signer),
            cap,
        })
    }

    /// Encode a client→coordinator request, wrapping it in a signed
    /// `AuthRequest` when this client authenticates, or sending it bare
    /// otherwise (tests / no-auth dev path).
    fn authed(&self, inner: Msg) -> Vec<u8> {
        match &self.signer {
            Some(signer) => {
                // The caller is THIS node's identity: the PoP is signed with
                // `self.signer` (whose public key is `self.key`), and the
                // coordinator verifies it against `caller`. For a cross-peer
                // `Lookup { key: peer }` the inner key is the peer, but the
                // authenticated principal is still this caller.
                let auth =
                    sign_authenticator(signer, &inner.encode(), now_secs(), self.cap.clone());
                AuthRequest {
                    caller: self.key,
                    inner,
                    auth,
                }
                .encode()
            }
            None => inner.encode(),
        }
    }

    pub async fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.sock.local_addr()
    }

    pub async fn discover_reflexive(&self) -> std::io::Result<SocketAddr> {
        self.sock
            .send_to(
                &self.authed(Msg::BindRequest { from: self.key }),
                self.coord,
            )
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
            .send_to(
                &self.authed(Msg::Readvertise {
                    key: self.key,
                    nonce,
                }),
                self.coord,
            )
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
                Ok(Msg::LookupResponse {
                    key,
                    reflexive: Some(addr),
                }) if key == peer => {
                    return Ok(addr);
                }
                Ok(Msg::LookupResponse {
                    key,
                    reflexive: None,
                }) if key == peer => {
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

    /// Fire-and-forget Lookup — the response arrives as a
    /// [`ClientEvent::LookupResponse`] via [`Self::recv_event`]. The blocking
    /// [`Self::lookup`] stays for sequential callers; a dispatching consumer
    /// (the reachability pump) must NOT mix the two on one socket — every
    /// per-method recv loop silently eats the datagrams it filters out.
    pub async fn send_lookup(&self, peer: NodeKey) -> std::io::Result<()> {
        self.sock
            .send_to(&self.authed(Msg::Lookup { key: peer }), self.coord)
            .await?;
        Ok(())
    }

    /// Receive the next classified event. Never surfaces coordinator-shaped
    /// control (`BindResponse`/`LookupResponse`/`PunchSync`) from a
    /// non-coordinator source — a forged control datagram is dropped exactly
    /// like the per-method recv loops drop it. Undecodable datagrams are
    /// skipped. This is the single-dispatch alternative to those loops: one
    /// consumer sees EVERY datagram, so an unsolicited PunchSync arriving
    /// between operations is delivered instead of eaten.
    pub async fn recv_event(&self) -> std::io::Result<ClientEvent> {
        let mut buf = [0u8; 128];
        loop {
            let (n, from) = self.sock.recv_from(&mut buf).await?;
            let Ok(msg) = Msg::decode(&buf[..n]) else {
                continue;
            };
            let from_coord = from == self.coord;
            match msg {
                Msg::BindResponse { reflexive } if from_coord => {
                    return Ok(ClientEvent::BindResponse { reflexive });
                }
                Msg::LookupResponse { key, reflexive } if from_coord => {
                    return Ok(ClientEvent::LookupResponse { key, reflexive });
                }
                Msg::PunchSync {
                    peer,
                    peer_reflexive,
                } if from_coord => {
                    return Ok(ClientEvent::PunchSync {
                        peer,
                        peer_reflexive,
                    });
                }
                Msg::Punch { from: peer } => {
                    return Ok(ClientEvent::Punch {
                        from: peer,
                        src: from,
                    });
                }
                _ => continue,
            }
        }
    }
}

/// One decoded datagram from the rendezvous socket, classified for a single
/// dispatching consumer ([`NatClient::recv_event`]). Coordinator-originated
/// control is only surfaced when it actually came from the coordinator this
/// client is pointed at. `Punch` is peer-originated by design, so it carries
/// its observed source for the consumer to match against the
/// rendezvous-resolved address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientEvent {
    BindResponse {
        reflexive: SocketAddr,
    },
    LookupResponse {
        key: NodeKey,
        reflexive: Option<SocketAddr>,
    },
    PunchSync {
        peer: NodeKey,
        peer_reflexive: SocketAddr,
    },
    Punch {
        from: NodeKey,
        src: SocketAddr,
    },
}

/// The coordinator event loop: decode control datagrams (authenticated or, under
/// a fully-open policy, legacy), enforce the auth policy, feed the pure handler,
/// send replies. Pure rendezvous — never binds a data socket, never carries
/// peer traffic.
pub async fn run_coordinator(sock: UdpSocket, policy: AuthPolicy) {
    run_coordinator_with(sock, Coordinator::with_policy(policy)).await
}

/// [`run_coordinator`] with a caller-built [`Coordinator`] — the seam for a
/// custom registration TTL or a pre-seeded book (tests, short-lived rigs).
pub async fn run_coordinator_with(sock: UdpSocket, mut coord: Coordinator) {
    // Big enough for an AuthRequest with a cap (~251 bytes worst case: the
    // 32-byte caller field plus the inner request, authenticator, and cap).
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
                Ok(m) => coord.handle_legacy(from, m, now),
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
        use crate::auth::{AuthPolicy, mint_coord_cap};
        use commonware_cryptography::{Signer as _, ed25519};

        let g = ed25519::PrivateKey::from_seed(100);
        let policy = AuthPolicy::Private {
            genesis_set: vec![g.public_key()],
        };

        let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let coord_addr = coord_sock.local_addr().unwrap();
        tokio::spawn(run_coordinator(coord_sock, policy));

        // Two authorized nodes (joiners) with genesis caps.
        let a_signer = ed25519::PrivateKey::from_seed(200);
        let b_signer = ed25519::PrivateKey::from_seed(201);
        let a_key = {
            let mut k = [0u8; 32];
            k.copy_from_slice(a_signer.public_key().as_ref());
            NodeKey(k)
        };
        let b_key = {
            let mut k = [0u8; 32];
            k.copy_from_slice(b_signer.public_key().as_ref());
            NodeKey(k)
        };
        let a_cap = mint_coord_cap(&g, a_key, crate::auth::now_secs() + 3600);
        let b_cap = mint_coord_cap(&g, b_key, crate::auth::now_secs() + 3600);

        let a = NatClient::bind_multi_auth(a_key, vec![coord_addr], a_signer, Some(a_cap))
            .await
            .unwrap();
        let b = NatClient::bind_multi_auth(b_key, vec![coord_addr], b_signer, Some(b_cap))
            .await
            .unwrap();
        a.register().await.unwrap();
        b.register().await.unwrap();

        // Per the committed wire semantics, a `Lookup`'s `subject_key()` is the
        // LOOKED-UP key, so under Private policy the authenticator must be signed
        // by (and admitted for) that key — a node resolves its OWN mapping. This
        // proves an authorized register+lookup completes end-to-end over the real
        // signed UDP path (a cross-node `a.lookup(b_key)` is impossible here: a
        // does not hold b's signer, so its PoP would fail and be dropped).
        let a_reflexive = timeout(Duration::from_secs(2), a.lookup(a_key))
            .await
            .expect("no timeout")
            .expect("lookup");
        assert_eq!(a_reflexive.port(), a.local_addr().await.unwrap().port());
        let b_reflexive = timeout(Duration::from_secs(2), b.lookup(b_key))
            .await
            .expect("no timeout")
            .expect("lookup");
        assert_eq!(b_reflexive.port(), b.local_addr().await.unwrap().port());

        // Unauthorized: a node with NO signer (bare Msg) cannot register under
        // Private policy — its lookup for itself finds nothing.
        let outsider = NatClient::bind(NodeKey([0xcd; 32]), coord_addr)
            .await
            .unwrap();
        outsider.register().await.unwrap(); // dropped by handle_legacy
        let miss = timeout(
            Duration::from_millis(500),
            outsider.lookup(NodeKey([0xcd; 32])),
        )
        .await;
        assert!(
            miss.is_err() || miss.unwrap().is_err(),
            "unauthenticated register never created a mapping"
        );
    }

    #[tokio::test]
    async fn cross_peer_authenticated_lookup_and_punch_under_private_policy() {
        // The core rendezvous path: two AUTHORIZED nodes A and B (each holding a
        // genesis-minted cap) register, then A looks up B's DIFFERENT key and a
        // simultaneous-open punch completes. This is the previously-impossible
        // path — the coordinator authenticates the CALLER (A), not the looked-up
        // key (B), so A's PoP (signed with A's own key) validates.
        use crate::auth::{AuthPolicy, mint_coord_cap};
        use commonware_cryptography::{Signer as _, ed25519};

        let g = ed25519::PrivateKey::from_seed(700);
        let policy = AuthPolicy::Private {
            genesis_set: vec![g.public_key()],
        };

        let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let coord_addr = coord_sock.local_addr().unwrap();
        tokio::spawn(run_coordinator(coord_sock, policy));

        let a_signer = ed25519::PrivateKey::from_seed(701);
        let b_signer = ed25519::PrivateKey::from_seed(702);
        let a_key = {
            let mut k = [0u8; 32];
            k.copy_from_slice(a_signer.public_key().as_ref());
            NodeKey(k)
        };
        let b_key = {
            let mut k = [0u8; 32];
            k.copy_from_slice(b_signer.public_key().as_ref());
            NodeKey(k)
        };
        let a_cap = mint_coord_cap(&g, a_key, crate::auth::now_secs() + 3600);
        let b_cap = mint_coord_cap(&g, b_key, crate::auth::now_secs() + 3600);

        let a = NatClient::bind_multi_auth(a_key, vec![coord_addr], a_signer, Some(a_cap))
            .await
            .unwrap();
        let b = NatClient::bind_multi_auth(b_key, vec![coord_addr], b_signer, Some(b_cap))
            .await
            .unwrap();
        a.register().await.unwrap();
        b.register().await.unwrap();

        // A resolves B's reflexive via a CROSS-PEER Lookup. Under the OLD code
        // (authenticate the looked-up key) A's PoP is verified against B's key
        // and fails BadPop, so this Lookup is silently dropped and the lookup
        // times out. Under the fix it resolves B's mapping.
        let b_reflexive = timeout(Duration::from_secs(2), a.lookup(b_key))
            .await
            .expect("cross-peer lookup must not time out")
            .expect("A resolves B");
        assert_eq!(b_reflexive.port(), b.local_addr().await.unwrap().port());

        // The fan-out PunchSync reached B (the coordinator told B where to punch
        // A). B learns A's reflexive from that unsolicited PunchSync.
        let a_reflexive = timeout(Duration::from_secs(2), b.recv_punch_sync())
            .await
            .expect("B receives the fan-out PunchSync")
            .expect("punch sync");
        assert_eq!(a_reflexive.port(), a.local_addr().await.unwrap().port());
    }

    #[tokio::test]
    async fn dead_primary_falls_through_to_live_secondary() {
        // A live coordinator (the secondary).
        let live = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let live_addr = live.local_addr().unwrap();
        tokio::spawn(run_coordinator(
            live,
            crate::auth::AuthPolicy::Open { require_pop: false },
        ));

        // A DEAD primary: a bound socket nobody ever serves. Datagrams sent to
        // it are buffered and never answered, so the per-try budget elapses.
        let dead = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let dead_addr = dead.local_addr().unwrap();

        let mut client = NatClient::bind_multi(NodeKey([1u8; 32]), vec![dead_addr, live_addr])
            .await
            .unwrap();
        let (idx, reflexive) = timeout(
            Duration::from_secs(2),
            client.discover_reflexive_failover(Duration::from_millis(150)),
        )
        .await
        .expect("failover must be bounded, never stuck")
        .expect("secondary answers");

        assert_eq!(
            idx, 1,
            "the dead primary is skipped; the live secondary answers"
        );
        assert_eq!(reflexive.port(), client.local_addr().await.unwrap().port());
    }

    #[tokio::test]
    async fn no_single_coordinator_is_load_bearing_either_position_works() {
        // Same live coordinator, but now in PRIMARY position with a dead
        // secondary: discovery still succeeds, via index 0. Together with the
        // previous test this proves neither position is uniquely required.
        let live = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let live_addr = live.local_addr().unwrap();
        tokio::spawn(run_coordinator(
            live,
            crate::auth::AuthPolicy::Open { require_pop: false },
        ));
        let dead = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let dead_addr = dead.local_addr().unwrap();

        let mut client = NatClient::bind_multi(NodeKey([2u8; 32]), vec![live_addr, dead_addr])
            .await
            .unwrap();
        let (idx, reflexive) = timeout(
            Duration::from_secs(2),
            client.discover_reflexive_failover(Duration::from_millis(150)),
        )
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
        tokio::spawn(run_coordinator(
            coord_sock,
            crate::auth::AuthPolicy::Open { require_pop: false },
        ));

        let client = NatClient::bind(NodeKey([1u8; 32]), coord_addr)
            .await
            .unwrap();
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
        tokio::spawn(run_coordinator(
            coord_sock,
            crate::auth::AuthPolicy::Open { require_pop: false },
        ));

        let client = NatClient::bind(NodeKey([2u8; 32]), coord_addr)
            .await
            .unwrap();
        let client_addr = client.local_addr().await.unwrap();

        // A forger — some socket that is not the coordinator — races the
        // real coordinator reply with a bogus BindResponse. The client binds
        // the wildcard, so target its port on loopback (macOS refuses a send
        // to a 0.0.0.0 destination; the on-path forger is loopback here).
        let forger = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let client_dst = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), client_addr.port());
        let forged = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 9)), 5555);
        forger
            .send_to(
                &Msg::BindResponse { reflexive: forged }.encode(),
                client_dst,
            )
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
        tokio::spawn(run_coordinator(
            coord_sock,
            crate::auth::AuthPolicy::Open { require_pop: false },
        ));

        let a_key = NodeKey([0xaa; 32]);
        let a = NatClient::bind(a_key, coord_addr).await.unwrap();
        let b = NatClient::bind(NodeKey([0xbb; 32]), coord_addr)
            .await
            .unwrap();
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
            .send_to(
                &Msg::Punch {
                    from: NodeKey([0xcc; 32]),
                }
                .encode(),
                b_addr,
            )
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

    /// the shared transport (socket mode's wiring): a client over
    /// `NatSocket::Shared` sends from the GIVEN socket and receives through
    /// the bypass lane — so the coordinator observes the shared socket's
    /// mapping (the reflexive IS the tunnel endpoint) and a punch exchange
    /// completes with the punch originating from that same 5-tuple.
    #[tokio::test]
    async fn shared_transport_rides_the_given_socket_for_reflexive_and_punch() {
        let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let coord_addr = coord_sock.local_addr().unwrap();
        tokio::spawn(run_coordinator(
            coord_sock,
            crate::auth::AuthPolicy::Open { require_pop: false },
        ));

        // the "underlay": a plain socket whose receive side is pumped into
        // the bypass lane wholesale — the overlay-net demux with every
        // datagram classified as not-WireGuard, which is exactly what the
        // NAT protocol's inbound looks like to it.
        let underlay = std::sync::Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let underlay_addr = underlay.local_addr().unwrap();
        let (bypass_tx, bypass_rx) = tokio::sync::mpsc::channel(64);
        tokio::spawn({
            let sock = underlay.clone();
            async move {
                let mut buf = [0u8; 2048];
                loop {
                    let Ok((n, src)) = sock.recv_from(&mut buf).await else {
                        break;
                    };
                    if bypass_tx.send((buf[..n].to_vec(), src)).await.is_err() {
                        break;
                    }
                }
            }
        });

        let a_key = NodeKey([0xaa; 32]);
        let a = NatClient::with_socket(
            NatSocket::shared(underlay.clone(), bypass_rx).unwrap(),
            a_key,
            vec![coord_addr],
            None,
            None,
        )
        .unwrap();

        // the coordinator's observation is the SHARED socket's mapping.
        let reflexive = timeout(Duration::from_secs(2), a.discover_reflexive())
            .await
            .expect("no timeout")
            .expect("reflexive");
        assert_eq!(
            reflexive, underlay_addr,
            "the reflexive is the shared socket's own address — punch and tunnel share the 5-tuple"
        );
        a.register().await.unwrap();

        // a punch exchange both ways with an owned-socket peer.
        let b_key = NodeKey([0xbb; 32]);
        let b = NatClient::bind(b_key, coord_addr).await.unwrap();
        let b_addr = SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            b.local_addr().await.unwrap().port(),
        );

        b.send_punch_to(underlay_addr).await.unwrap();
        let got = timeout(Duration::from_secs(2), a.recv_punch_from(b_addr))
            .await
            .expect("no timeout")
            .expect("recv via the bypass lane");
        assert_eq!(got, Msg::Punch { from: b_key });

        a.send_punch_to(b_addr).await.unwrap();
        let got = timeout(Duration::from_secs(2), b.recv_punch_from(underlay_addr))
            .await
            .expect("no timeout")
            .expect("recv at b");
        assert_eq!(
            got,
            Msg::Punch { from: a_key },
            "a's punch left from the shared socket (b saw the underlay address as its source)"
        );
    }

    /// the PRODUCTION underlay shape: socket mode binds dual-stack `[::]`
    /// (`overlay_net::userspace::UnderlaySocket`), while coordinators and
    /// punched reflexives are V4. Sends must ride the v6 socket as v4-MAPPED
    /// v6 (a plain V4 destination is EINVAL on macOS) and received sources
    /// must canonicalize `::ffff:a.b.c.d` back to V4, or reply/punch source
    /// validation never matches. The v4-loopback test above cannot catch
    /// this — its underlay is a V4 socket.
    #[tokio::test]
    async fn dual_stack_shared_transport_reaches_a_v4_coordinator() {
        let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let coord_addr = coord_sock.local_addr().unwrap();
        tokio::spawn(run_coordinator(
            coord_sock,
            crate::auth::AuthPolicy::Open { require_pop: false },
        ));

        // bind exactly like the production underlay: a std dual-stack
        // `[::]:0` socket handed to tokio.
        let std_sock =
            std::net::UdpSocket::bind((std::net::Ipv6Addr::UNSPECIFIED, 0)).unwrap();
        std_sock.set_nonblocking(true).unwrap();
        let underlay = std::sync::Arc::new(UdpSocket::from_std(std_sock).unwrap());
        let underlay_port = underlay.local_addr().unwrap().port();
        let (bypass_tx, bypass_rx) = tokio::sync::mpsc::channel(64);
        tokio::spawn({
            let sock = underlay.clone();
            async move {
                let mut buf = [0u8; 2048];
                loop {
                    let Ok((n, src)) = sock.recv_from(&mut buf).await else {
                        break;
                    };
                    if bypass_tx.send((buf[..n].to_vec(), src)).await.is_err() {
                        break;
                    }
                }
            }
        });

        let a_key = NodeKey([0xaa; 32]);
        let a = NatClient::with_socket(
            NatSocket::shared(underlay.clone(), bypass_rx).unwrap(),
            a_key,
            vec![coord_addr],
            None,
            None,
        )
        .unwrap();

        // reflexive discovery crosses the family seam twice: a v4-mapped
        // send out of the v6 socket, and a reply whose observed source must
        // canonicalize back to the dialed V4 coordinator to be accepted.
        let reflexive = timeout(Duration::from_secs(2), a.discover_reflexive())
            .await
            .expect("no timeout")
            .expect("reflexive discovery over the dual-stack underlay");
        assert_eq!(
            reflexive,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), underlay_port),
            "the v4 coordinator observes the dual-stack socket's V4 mapping"
        );
        a.register().await.unwrap();

        // a punch exchange with an owned V4 peer: a's inbound arrives as
        // `::ffff:127.0.0.1` and must match b's V4 address; a's outbound
        // punch must reach b's V4 socket from the v6 underlay.
        let b_key = NodeKey([0xbb; 32]);
        let b = NatClient::bind(b_key, coord_addr).await.unwrap();
        let b_addr = SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            b.local_addr().await.unwrap().port(),
        );

        b.send_punch_to(reflexive).await.unwrap();
        let got = timeout(Duration::from_secs(2), a.recv_punch_from(b_addr))
            .await
            .expect("no timeout")
            .expect("recv canonicalized to V4 via the bypass lane");
        assert_eq!(got, Msg::Punch { from: b_key });

        a.send_punch_to(b_addr).await.unwrap();
        let got = timeout(Duration::from_secs(2), b.recv_punch_from(reflexive))
            .await
            .expect("no timeout")
            .expect("recv at b");
        assert_eq!(got, Msg::Punch { from: a_key });
    }

    #[tokio::test]
    async fn direct_path_survives_coordinator_shutdown() {
        let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let coord_addr = coord_sock.local_addr().unwrap();
        let coord = tokio::spawn(run_coordinator(
            coord_sock,
            crate::auth::AuthPolicy::Open { require_pop: false },
        ));

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
        tokio::spawn(run_coordinator(
            coord_sock,
            crate::auth::AuthPolicy::Open { require_pop: false },
        ));

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
        assert_ne!(
            a2_port,
            a_first.port(),
            "the rebound socket has a fresh port"
        );
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
        tokio::spawn(run_coordinator(
            live,
            crate::auth::AuthPolicy::Open { require_pop: false },
        ));
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

    #[tokio::test]
    async fn recv_event_dispatches_lookup_response_and_punch_sync_and_filters_forgeries() {
        let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let coord_addr = coord_sock.local_addr().unwrap();
        tokio::spawn(run_coordinator(
            coord_sock,
            crate::auth::AuthPolicy::Open { require_pop: false },
        ));

        let a_key = NodeKey([0xaa; 32]);
        let b_key = NodeKey([0xbb; 32]);
        let a = NatClient::bind(a_key, coord_addr).await.unwrap();
        let b = NatClient::bind(b_key, coord_addr).await.unwrap();
        a.register().await.unwrap();
        b.register().await.unwrap();

        // A forged PunchSync from a non-coordinator must NOT surface as an
        // event on A's socket.
        let forger = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let forged_reflexive = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 66)), 6666);
        let a_dst = SocketAddr::new(
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            a.local_addr().await.unwrap().port(),
        );
        forger
            .send_to(
                &Msg::PunchSync {
                    peer: b_key,
                    peer_reflexive: forged_reflexive,
                }
                .encode(),
                a_dst,
            )
            .await
            .unwrap();

        // B looks A up through the event API: the coordinator-sourced events on
        // B's socket are the LookupResponse and B's own caller-side PunchSync.
        b.send_lookup(a_key).await.unwrap();
        let mut saw_lookup = false;
        let mut saw_sync = false;
        for _ in 0..8 {
            match timeout(Duration::from_secs(2), b.recv_event())
                .await
                .expect("bounded")
                .expect("recv")
            {
                ClientEvent::LookupResponse { key, reflexive } if key == a_key => {
                    assert!(reflexive.is_some(), "A is registered");
                    saw_lookup = true;
                }
                ClientEvent::PunchSync { peer, .. } if peer == a_key => saw_sync = true,
                _ => {}
            }
            if saw_lookup && saw_sync {
                break;
            }
        }
        assert!(
            saw_lookup && saw_sync,
            "lookup response and caller-side punch sync both dispatched as events"
        );

        // A's socket sees the coordinator's fan-out PunchSync about B — and the
        // forged datagram it received earlier was dropped, so the FIRST
        // PunchSync event names B's real reflexive, not the forger's invention.
        let ev = timeout(Duration::from_secs(2), a.recv_event())
            .await
            .expect("bounded")
            .expect("recv");
        match ev {
            ClientEvent::PunchSync {
                peer,
                peer_reflexive,
            } => {
                assert_eq!(peer, b_key);
                assert_ne!(
                    peer_reflexive, forged_reflexive,
                    "forged PunchSync was dropped"
                );
                assert_eq!(peer_reflexive.port(), b.local_addr().await.unwrap().port());
            }
            other => panic!("expected the coordinator fan-out PunchSync first, got {other:?}"),
        }
    }
}
