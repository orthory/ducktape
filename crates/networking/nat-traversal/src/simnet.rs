//! The simulated datagram network: [`SimNat`] models attached to an
//! in-process router, so the PRODUCTION rendezvous stack — `NatClient`, the
//! coordinator loop, reachability's `NatResolver` — runs unmodified over a
//! deterministic NAT topology through [`NatSocket::Simulated`].
//!
//! Delivery is synchronous: a send applies the sender's NAT (mapping +
//! pinhole), routes on the destination's public address, applies the
//! receiver's NAT filter, and either drops the datagram or places it in the
//! receiver's inbox before the send returns. The only loss in the model is
//! NAT filtering (and a destination nobody owns), so a test program's runs
//! are identical — no real sockets, no scheduling-dependent packet loss.
//!
//! [`NatSocket::Simulated`]: crate::NatSocket::Simulated

use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;

use crate::simnat::SimNat;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct EndpointId(u64);

/// What stands between an endpoint's socket and the public network.
enum Front {
    /// Directly on the public network at its bound address — the
    /// coordinator's posture.
    Public,
    /// Behind its own NAT.
    Nat(SimNat),
}

struct Endpoint {
    front: Front,
    internal: SocketAddr,
    inbox: mpsc::UnboundedSender<(Vec<u8>, SocketAddr)>,
}

#[derive(Default)]
struct Inner {
    endpoints: HashMap<EndpointId, Endpoint>,
    /// Public address → owner. A public endpoint claims its bound address at
    /// attach; a NATed endpoint's mappings are claimed as its NAT allocates
    /// them. Mappings stale after a rebind stay claimed — [`SimNat`] never
    /// rewinds a port, and the cleared pinholes already refuse the traffic.
    routes: HashMap<SocketAddr, EndpointId>,
    next_id: u64,
}

/// The router every [`SimSocket`] sends through. Cloning shares the network.
#[derive(Clone, Default)]
pub struct SimNetwork {
    inner: Arc<Mutex<Inner>>,
}

impl SimNetwork {
    pub fn new() -> Self {
        Self::default()
    }

    /// Attach a public (un-NATed) endpoint at `addr` — peers address it
    /// directly and every inbound datagram is admitted.
    pub fn public(&self, addr: SocketAddr) -> SimSocket {
        let (sock, _id) = self.attach(Front::Public, addr);
        sock
    }

    /// Attach an endpoint behind its own NAT. `internal` is the private
    /// socket address; peers only ever observe the NAT's public mappings.
    /// The handle drives the NAT from the outside (a rebind), which the
    /// socket's owner — buried inside a `NatClient` — cannot.
    pub fn behind(&self, nat: SimNat, internal: SocketAddr) -> (SimSocket, SimHandle) {
        let (sock, id) = self.attach(Front::Nat(nat), internal);
        let handle = SimHandle {
            net: self.clone(),
            id,
        };
        (sock, handle)
    }

    /// Drop the endpoint that owns `addr`: the address stops resolving and
    /// the endpoint's inbox closes (its `recv` errors). Abort the task
    /// serving the socket too — a removed endpoint cannot receive, and a
    /// loop that ignores receive errors would spin.
    pub fn remove(&self, addr: SocketAddr) {
        let mut inner = self.inner.lock().expect("sim network lock");
        let Some(id) = inner.routes.remove(&addr) else {
            return;
        };
        inner.endpoints.remove(&id);
        inner.routes.retain(|_, owner| *owner != id);
    }

    fn attach(&self, front: Front, internal: SocketAddr) -> (SimSocket, EndpointId) {
        let (inbox_tx, inbox_rx) = mpsc::unbounded_channel();
        let mut inner = self.inner.lock().expect("sim network lock");
        let id = EndpointId(inner.next_id);
        inner.next_id += 1;
        let publicly_addressed = matches!(front, Front::Public);
        if publicly_addressed {
            let taken = inner.routes.insert(internal, id);
            assert!(
                taken.is_none(),
                "public address {internal} already attached"
            );
        }
        inner.endpoints.insert(
            id,
            Endpoint {
                front,
                internal,
                inbox: inbox_tx,
            },
        );
        drop(inner);
        let sock = SimSocket {
            net: self.clone(),
            id,
            internal,
            inbox: tokio::sync::Mutex::new(inbox_rx),
        };
        (sock, id)
    }

    /// One datagram through the topology. Mirrors UDP's contract: a send is
    /// accepted even when nothing receives it (unknown destination, removed
    /// endpoint, NAT filter drop).
    fn send(&self, from: EndpointId, bytes: &[u8], dst: SocketAddr) -> io::Result<usize> {
        let mut guard = self.inner.lock().expect("sim network lock");
        let inner = &mut *guard;
        let sender = inner
            .endpoints
            .get_mut(&from)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "sim endpoint removed"))?;
        let src = match &mut sender.front {
            // A public endpoint's source IS its bound address, claimed at attach.
            Front::Public => sender.internal,
            Front::Nat(nat) => {
                let mapped = nat.send(sender.internal, dst);
                inner.routes.insert(mapped, from);
                mapped
            }
        };
        let Some(&to) = inner.routes.get(&dst) else {
            return Ok(bytes.len());
        };
        let Some(receiver) = inner.endpoints.get(&to) else {
            return Ok(bytes.len());
        };
        let admitted = match &receiver.front {
            Front::Public => true,
            Front::Nat(nat) => nat.allow_inbound(dst, src),
        };
        if admitted {
            let _ = receiver.inbox.send((bytes.to_vec(), src));
        }
        Ok(bytes.len())
    }
}

/// The out-of-band control a test holds on a NATed endpoint.
pub struct SimHandle {
    net: SimNetwork,
    id: EndpointId,
}

impl SimHandle {
    /// The NAT rebinds (lease expiry, reboot): mappings and pinholes drop,
    /// so the stale reflexive admits nobody and the endpoint's next
    /// outbound datagram maps to a fresh public port.
    pub fn rebind(&self) {
        let mut inner = self.net.inner.lock().expect("sim network lock");
        let endpoint = inner
            .endpoints
            .get_mut(&self.id)
            .expect("rebind on a removed endpoint");
        match &mut endpoint.front {
            Front::Nat(nat) => nat.rebind(),
            Front::Public => unreachable!("SimHandle only exists for NATed endpoints"),
        }
    }
}

/// One endpoint's socket: what [`NatSocket::Simulated`] wraps. Sends route
/// through the network synchronously; receives drain the endpoint's inbox.
///
/// [`NatSocket::Simulated`]: crate::NatSocket::Simulated
pub struct SimSocket {
    net: SimNetwork,
    id: EndpointId,
    internal: SocketAddr,
    /// Behind a `Mutex` only to keep `&self` receive methods, exactly like
    /// the shared-underlay arm — single consumer by construction.
    inbox: tokio::sync::Mutex<mpsc::UnboundedReceiver<(Vec<u8>, SocketAddr)>>,
}

impl SimSocket {
    pub(crate) fn send_to(&self, buf: &[u8], dst: SocketAddr) -> io::Result<usize> {
        self.net.send(self.id, buf, dst)
    }

    pub(crate) async fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        let mut inbox = self.inbox.lock().await;
        let (datagram, src) = inbox
            .recv()
            .await
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "sim endpoint removed"))?;
        let len = datagram.len().min(buf.len());
        buf[..len].copy_from_slice(&datagram[..len]);
        Ok((len, src))
    }

    pub(crate) fn local_addr(&self) -> SocketAddr {
        self.internal
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn addr(ip: [u8; 4], port: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::from(ip)), port)
    }

    fn drain(sock: &SimSocket) -> Option<(Vec<u8>, SocketAddr)> {
        let mut inbox = sock.inbox.try_lock().expect("no concurrent consumer");
        inbox.try_recv().ok()
    }

    #[test]
    fn unsolicited_inbound_is_filtered_until_the_peer_punches() {
        let net = SimNetwork::new();
        let (a, _ah) = net.behind(
            SimNat::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1))),
            addr([192, 168, 0, 1], 51820),
        );
        let (b, _bh) = net.behind(
            SimNat::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 2))),
            addr([192, 168, 0, 2], 51820),
        );
        let coord = addr([192, 0, 2, 1], 3478);
        let coord_sock = net.public(coord);

        // Both map toward the coordinator; it observes their reflexives.
        a.send_to(b"a", coord).unwrap();
        b.send_to(b"b", coord).unwrap();
        let (_, a_mapped) = drain(&coord_sock).expect("a's datagram reaches the public endpoint");
        let (_, b_mapped) = drain(&coord_sock).expect("b's datagram reaches the public endpoint");

        // A punches toward B's reflexive before B has punched back: dropped.
        a.send_to(b"punch", b_mapped).unwrap();
        assert!(
            drain(&b).is_none(),
            "B's filter drops the unsolicited punch"
        );

        // B punches toward A (opening its own pinhole); A's earlier punch
        // opened A's pinhole toward B, so B's datagram is admitted…
        b.send_to(b"punch", a_mapped).unwrap();
        assert!(drain(&a).is_some(), "A admits B after A punched toward B");
        // …and A's retransmit now lands at B.
        a.send_to(b"punch", b_mapped).unwrap();
        assert!(drain(&b).is_some(), "B admits A after B punched toward A");
    }

    #[test]
    fn symmetric_nat_reflexive_never_admits_a_peer() {
        let net = SimNetwork::new();
        let (a, _ah) = net.behind(
            SimNat::symmetric(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1))),
            addr([192, 168, 0, 1], 51820),
        );
        let (b, _bh) = net.behind(
            SimNat::symmetric(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 2))),
            addr([192, 168, 0, 2], 51820),
        );
        let coord = addr([192, 0, 2, 1], 3478);
        let coord_sock = net.public(coord);

        a.send_to(b"a", coord).unwrap();
        b.send_to(b"b", coord).unwrap();
        let (_, a_coord_mapped) = drain(&coord_sock).expect("a observed");
        let (_, b_coord_mapped) = drain(&coord_sock).expect("b observed");

        // Each punches toward the other's COORDINATOR-facing mapping — but a
        // symmetric NAT allocated a different mapping for the peer
        // destination, so neither reflexive ever admits the peer.
        a.send_to(b"punch", b_coord_mapped).unwrap();
        b.send_to(b"punch", a_coord_mapped).unwrap();
        a.send_to(b"punch", b_coord_mapped).unwrap();
        b.send_to(b"punch", a_coord_mapped).unwrap();
        assert!(drain(&a).is_none(), "A's coordinator mapping admits nobody");
        assert!(drain(&b).is_none(), "B's coordinator mapping admits nobody");
    }

    #[test]
    fn rebind_moves_the_mapping_and_the_stale_one_refuses() {
        let net = SimNetwork::new();
        let (a, a_handle) = net.behind(
            SimNat::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1))),
            addr([192, 168, 0, 1], 51820),
        );
        let coord = addr([192, 0, 2, 1], 3478);
        let coord_sock = net.public(coord);

        a.send_to(b"a", coord).unwrap();
        let (_, old_mapped) = drain(&coord_sock).expect("observed");

        a_handle.rebind();

        // The coordinator can no longer reach A through the stale mapping…
        coord_sock.send_to(b"sync", old_mapped).unwrap();
        assert!(drain(&a).is_none(), "the stale mapping admits nobody");

        // …and A's next outbound maps afresh, reopening the path.
        a.send_to(b"a", coord).unwrap();
        let (_, new_mapped) = drain(&coord_sock).expect("re-observed");
        assert_ne!(old_mapped, new_mapped, "rebind moved the reflexive");
        coord_sock.send_to(b"sync", new_mapped).unwrap();
        assert!(
            drain(&a).is_some(),
            "the fresh mapping admits the coordinator"
        );
    }

    #[test]
    fn unknown_and_removed_destinations_swallow_the_datagram_like_udp() {
        let net = SimNetwork::new();
        let coord = addr([192, 0, 2, 1], 3478);
        let coord_sock = net.public(coord);
        let (a, _ah) = net.behind(
            SimNat::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1))),
            addr([192, 168, 0, 1], 51820),
        );

        // Nobody owns this address: the send is accepted and vanishes.
        assert!(a.send_to(b"x", addr([203, 0, 113, 9], 9)).is_ok());

        // A removed endpoint behaves the same on the send side…
        net.remove(coord);
        assert!(a.send_to(b"x", coord).is_ok());
        // …and its own receive side reports the closure.
        let mut buf = [0u8; 8];
        let closed = futures_now(coord_sock.recv_from(&mut buf));
        assert!(
            matches!(closed, Some(Err(e)) if e.kind() == io::ErrorKind::NotConnected),
            "a removed endpoint's recv errors instead of hanging"
        );
    }

    /// Poll a future exactly once — the closure signal is synchronous here,
    /// so a ready result means delivered/closed and pending means dropped.
    fn futures_now<F: std::future::Future>(fut: F) -> Option<F::Output> {
        let mut fut = Box::pin(fut);
        let waker = std::task::Waker::noop();
        let mut cx = std::task::Context::from_waker(waker);
        match fut.as_mut().poll(&mut cx) {
            std::task::Poll::Ready(out) => Some(out),
            std::task::Poll::Pending => None,
        }
    }
}
