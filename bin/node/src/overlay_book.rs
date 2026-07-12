//! the one overlay address book + admission policy every per-use data plane
//! shares (gateway today): forward resolution DERIVES (a ULA is a pure
//! function of identity), reverse resolution and admission consult the
//! tracked peer set — members + standbys of the current view, maintained by
//! the node at boot and at every cutover re-track. which plane a book serves
//! is a type parameter (its [`Plane`] tag carries the service + stream flow),
//! so admission stays default-deny per service without duplicating the book.
//!
//! this module also carries the two seams every per-use plane shares: the
//! socket-factory selection ([`socket_factory`]) and the process-wide bulk
//! budget ([`shared_bulk_pacer`]).

use std::collections::HashMap;
use std::marker::PhantomData;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, RwLock};

use commonware_cryptography::ed25519;
use data_plane::{AddressBook, AdmissionPolicy, BulkPacer, FlowId, PeerId, Service};

/// the socket seam's factory selection, in one place for every plane
/// bring-up caller: a plane's backend follows `wireguard_effect` exactly as
/// the mesh context's does (fake stages no data plane, so it keeps the OS
/// factory's downed-interface behavior).
pub fn socket_factory(
    kind: crate::config::WireGuardEffectKind,
    slot: &overlay_net::userspace::StackSlot,
) -> Arc<dyn data_plane::SocketFactory> {
    match kind {
        crate::config::WireGuardEffectKind::Socket => Arc::new(
            overlay_net::userspace::VirtualSocketFactory::new(slot.clone()),
        ),
        crate::config::WireGuardEffectKind::Tun | crate::config::WireGuardEffectKind::Fake => {
            Arc::new(data_plane::OsSocketFactory)
        }
    }
}

/// bulk ceiling shared by every stream-class per-use plane in this process
/// (gateway responses and agent telemetry today): a static compromise between
/// transfer time (~24 MB/s ≈ 42 s/GB) and real-time headroom; adaptive
/// per-link shaping is a separate concern.
const BULK_BYTES_PER_SEC: u64 = 24_000_000;
const BULK_BURST_BYTES: u64 = 512 * 1024;

/// One link-headroom budget shared by every stream-class per-use plane in
/// this process.
pub fn shared_bulk_pacer() -> BulkPacer {
    BulkPacer::new(BULK_BYTES_PER_SEC, BULK_BURST_BYTES)
}

/// derive a peer's overlay ULA from its raw ed25519 key bytes — the same
/// `(namespace, identity)` function the reachability plane routes by.
pub fn ula_of(namespace: &str, raw_key: &[u8; 32]) -> std::net::Ipv6Addr {
    wireguard::ula_v6_member_addr(namespace, wireguard::ValidatorIdentity(*raw_key))
}

/// a per-use plane's identity tag: the service its sockets register as and
/// the one stream flow its admission permits.
pub trait Plane: 'static {
    const SERVICE: Service;
    fn flow() -> FlowId;
}

/// the address book AND admission policy for one per-use plane, one object:
/// both answer from the same tracked peer set.
pub struct OverlayBook<P> {
    namespace: String,
    /// source `/128` -> authenticated peer, rebuilt on every `set_peers`.
    reverse: RwLock<HashMap<IpAddr, PeerId>>,
    /// `fn() -> P` keeps the book Send+Sync regardless of the tag type.
    _plane: PhantomData<fn() -> P>,
}

impl<P: Plane> OverlayBook<P> {
    pub fn new(namespace: String) -> Arc<Self> {
        Arc::new(Self {
            namespace,
            reverse: RwLock::new(HashMap::new()),
            _plane: PhantomData,
        })
    }

    /// replace the tracked peer set (members + standbys of the current view).
    pub fn set_peers<'a>(&self, keys: impl Iterator<Item = &'a ed25519::PublicKey>) {
        let reverse = keys
            .map(|key| {
                let raw: [u8; 32] = key.as_ref().try_into().expect("ed25519 keys are 32 bytes");
                (IpAddr::V6(ula_of(&self.namespace, &raw)), PeerId(raw))
            })
            .collect();
        *self.reverse.write().expect("book lock") = reverse;
    }

    /// this node's own overlay `/128` — where the plane's sockets bind.
    pub fn own_addr(&self, me: &ed25519::PublicKey) -> IpAddr {
        let raw: [u8; 32] = me.as_ref().try_into().expect("ed25519 keys are 32 bytes");
        IpAddr::V6(ula_of(&self.namespace, &raw))
    }

    fn overlay_ip(&self, raw: &[u8; 32]) -> IpAddr {
        IpAddr::V6(ula_of(&self.namespace, raw))
    }
}

impl<P: Plane> AddressBook for OverlayBook<P> {
    fn datagram_addr(&self, peer: PeerId) -> Option<SocketAddr> {
        Some(SocketAddr::new(
            self.overlay_ip(&peer.0),
            P::SERVICE.overlay_datagram_port(),
        ))
    }

    fn stream_addr(&self, peer: PeerId) -> Option<SocketAddr> {
        Some(SocketAddr::new(
            self.overlay_ip(&peer.0),
            P::SERVICE.overlay_stream_port(),
        ))
    }

    fn peer_at(&self, src: IpAddr) -> Option<PeerId> {
        self.reverse.read().expect("book lock").get(&src).copied()
    }
}

impl<P: Plane> AdmissionPolicy for OverlayBook<P> {
    fn permits(&self, peer: PeerId, service: Service, flow: FlowId) -> bool {
        service == P::SERVICE
            && flow == P::flow()
            && self
                .reverse
                .read()
                .expect("book lock")
                .values()
                .any(|known| *known == peer)
    }
}
