//! The huddle media planes — voice and camera video over the WireGuard
//! overlay, one per-use [`DataPlane`] per service (the per-use data-plane ADR,
//! `docs/adr/2026-07-07-per-use-data-plane.mdx`).
//!
//! Media used to ride the authenticated TCP mesh through a datagram
//! `ChannelTransport` (the video-call ADR's stated interim shortcut). That
//! defeated the plane's headline isolation guarantee: every mesh channel to a
//! peer funnels through ONE per-peer priority relay, so a multi-megabit video
//! burst and the 32 kbps voice stream shared a single send queue and voice
//! starved behind video under load. This module retires that arm: voice binds
//! `Service::Voice`'s overlay datagram port and video binds `Service::Video`'s,
//! so the two streams never share a socket, a queue, or a byte of head-of-line.
//!
//! Shape mirrors [`crate::statesync_plane`]: the host supplies the tracked
//! peer set ([`MediaPeers`], refreshed on every valset cutover) and the socket
//! factory; the plane binds lazily (the overlay `/128` only exists once the
//! reachability plane has the interface up, so the bind retries in the
//! background). Unlike statesync, media is datagram-only and fire-and-forget,
//! and — per the overlay-only cutover decision — has NO mesh fallback: with no
//! overlay there is simply no media transport.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use commonware_cryptography::ed25519;
use data_plane::{
    AddressBook, AdmissionPolicy, DataPlane, OverlaySockets, PeerId, PlaneConfig, Service,
    SocketFactory,
};

/// Media runs no stream class, so the plane's bulk-pacing budget is inert —
/// these values only need to exist. (The stream listeners the sockets bind are
/// never dialled; see [`bind_service`].)
const MEDIA_PLANE_CONFIG: PlaneConfig = PlaneConfig {
    bulk_bytes_per_sec: 1 << 20,
    bulk_burst_bytes: 1 << 20,
};

/// The overlay `/128` only exists once the reachability plane has the
/// interface configured; the bind retries on this cadence until it lands (or
/// the process exits).
const BIND_RETRY: Duration = Duration::from_secs(3);

/// Derive a peer's overlay ULA from its raw ed25519 key — the same
/// `(namespace, identity)` function statesync's book and the reachability
/// plane route by, so all three agree on every member's `/128`.
fn ula_of(namespace: &str, raw: &[u8; 32]) -> Ipv6Addr {
    wireguard::ula_v6_member_addr(namespace, wireguard::ValidatorIdentity(*raw))
}

/// The tracked media peer set: every workspace member's identity → overlay
/// `/128`. Forward resolution is a pure function of identity (a member's ULA
/// is derivable without the set); the set is what REVERSE resolution needs —
/// an inbound datagram's source `/128` authenticates to a peer only if that
/// peer is tracked. The host rebuilds it at boot and on every valset cutover
/// (mirroring statesync's `OverlayBook::set_peers`), so a just-added member's
/// media is admitted from the next re-track.
pub struct MediaPeers {
    namespace: String,
    /// source `/128` → authenticated peer, rebuilt on every `set_peers`.
    reverse: RwLock<HashMap<IpAddr, PeerId>>,
}

impl MediaPeers {
    pub fn new(namespace: String) -> Arc<Self> {
        Arc::new(MediaPeers {
            namespace,
            reverse: RwLock::new(HashMap::new()),
        })
    }

    /// Replace the tracked set (the view's transport members ∪ residents —
    /// exactly who the mesh authenticates as reachable overlay peers).
    pub fn set_peers<'a>(&self, keys: impl Iterator<Item = &'a ed25519::PublicKey>) {
        let reverse = keys
            .map(|key| {
                let raw: [u8; 32] = key.as_ref().try_into().expect("ed25519 keys are 32 bytes");
                (IpAddr::V6(ula_of(&self.namespace, &raw)), PeerId(raw))
            })
            .collect();
        *self.reverse.write().expect("media peers lock") = reverse;
    }

    /// This node's own overlay `/128` — where its media sockets bind.
    fn own_ip(&self, me: &[u8; 32]) -> IpAddr {
        IpAddr::V6(ula_of(&self.namespace, me))
    }

    fn overlay_ip(&self, raw: &[u8; 32]) -> IpAddr {
        IpAddr::V6(ula_of(&self.namespace, raw))
    }

    fn peer_at(&self, src: IpAddr) -> Option<PeerId> {
        self.reverse
            .read()
            .expect("media peers lock")
            .get(&src)
            .copied()
    }
}

/// A per-service [`AddressBook`] view over the shared [`MediaPeers`]: forward
/// resolution stamps THIS service's overlay port (voice and video ride
/// distinct ports so their sockets never collide), reverse resolution and the
/// tracked set are shared (a sender's `/128` is one identity regardless of
/// which service port it is sending on).
struct MediaBook {
    peers: Arc<MediaPeers>,
    service: Service,
}

impl AddressBook for MediaBook {
    fn datagram_addr(&self, peer: PeerId) -> Option<SocketAddr> {
        Some(SocketAddr::new(
            self.peers.overlay_ip(&peer.0),
            self.service.overlay_datagram_port(),
        ))
    }

    fn stream_addr(&self, peer: PeerId) -> Option<SocketAddr> {
        // Media opens no streams; the listener the socket binds is never
        // dialled. A valid address keeps the trait total.
        Some(SocketAddr::new(
            self.peers.overlay_ip(&peer.0),
            self.service.overlay_stream_port(),
        ))
    }

    fn peer_at(&self, src: IpAddr) -> Option<PeerId> {
        self.peers.peer_at(src)
    }
}

/// Bind the voice (45902) and video (45903) overlay planes on this node's
/// `/128`, retrying until the reachability plane has the interface up. Returns
/// a per-service [`DataPlane`] over each socket, sharing the one `admission`
/// (the hub's session-driven active-flow set answers for both services).
///
/// MUST be called on the runtime that will own the media pipeline: like
/// `DataPlane::new` everywhere, the plane's demux/accept pumps live on the
/// calling runtime. The qmdb-resolver constraint that pins statesync's plane
/// to the node runtime does not apply here, so this runs on the voice hub's
/// own runtime (proven safe: the overlay stack polls on the reachability
/// runtime and its virtual sockets are woken cross-runtime).
pub async fn bind_media_planes(
    factory: Arc<dyn SocketFactory>,
    peers: Arc<MediaPeers>,
    me: [u8; 32],
    admission: Arc<dyn AdmissionPolicy>,
) -> (DataPlane<OverlaySockets>, DataPlane<OverlaySockets>) {
    let own = peers.own_ip(&me);
    let voice_sockets = bind_service(&factory, &peers, own, Service::Voice).await;
    let video_sockets = bind_service(&factory, &peers, own, Service::Video).await;
    let voice_plane = DataPlane::new(voice_sockets, admission.clone(), MEDIA_PLANE_CONFIG);
    let video_plane = DataPlane::new(video_sockets, admission, MEDIA_PLANE_CONFIG);
    (voice_plane, video_plane)
}

/// Bind one service's overlay sockets on `own`, retrying the seconds the
/// reachability plane needs to bring the `/128` up. The per-service
/// [`MediaBook`] stamps this service's ports on egress so datagrams land on
/// the peer's matching socket.
async fn bind_service(
    factory: &Arc<dyn SocketFactory>,
    peers: &Arc<MediaPeers>,
    own: IpAddr,
    service: Service,
) -> OverlaySockets {
    let book: Arc<dyn AddressBook> = Arc::new(MediaBook {
        peers: peers.clone(),
        service,
    });
    let datagram_bind = SocketAddr::new(own, service.overlay_datagram_port());
    let stream_bind = SocketAddr::new(own, service.overlay_stream_port());
    // Say ONCE why the plane is not up yet: an interface that never arrives
    // (an unprivileged tun, an epoch that never applies) otherwise reads as a
    // huddle that hangs in "connecting" with an empty log.
    let mut logged = false;
    loop {
        match OverlaySockets::bind_with(factory.clone(), datagram_bind, stream_bind, book.clone())
            .await
        {
            Ok(sockets) => return sockets,
            // The interface (or our `/128`) is not up yet — retry quietly.
            Err(err) => {
                if !logged {
                    logged = true;
                    eprintln!(
                        "[voice-plane] {service:?} bind on {datagram_bind} waiting on the \
                         overlay interface ({err}) — retrying until it is up"
                    );
                }
                tokio::time::sleep(BIND_RETRY).await;
            }
        }
    }
}
