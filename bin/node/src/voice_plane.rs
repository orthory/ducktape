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
//! Shape mirrors [`crate::gateway_plane`]: the host supplies the tracked
//! peer set ([`OverlayPeers`], refreshed on every valset cutover) and the
//! socket factory; the plane binds lazily (the overlay `/128` only exists once
//! the reachability plane has the interface up, so the bind retries in the
//! background). Unlike gateway, media is datagram-only and fire-and-forget,
//! and — per the overlay-only cutover decision — has NO mesh fallback: with no
//! overlay there is simply no media transport.

use std::sync::Arc;

use data_plane::{
    AddressBook, AdmissionPolicy, DataPlane, OverlaySockets, PlaneConfig, Service, SocketFactory,
    host::bind_overlay_sockets,
};

use crate::overlay_book::{BIND_RETRY, OverlayBook, OverlayPeers, Plane};

/// Media runs no stream class, so the plane's bulk-pacing budget is inert —
/// these values only need to exist. (The stream listeners the sockets bind are
/// never dialled; see [`bind_service`].)
const MEDIA_PLANE_CONFIG: PlaneConfig = PlaneConfig {
    bulk_bytes_per_sec: 1 << 20,
    bulk_burst_bytes: 1 << 20,
};

/// The voice plane's tag for the shared [`OverlayBook`]: address resolution
/// only — media admission is the hub's session-driven active-flow set, so the
/// tag is a [`Plane`], never a stream plane.
struct VoicePlane;

impl Plane for VoicePlane {
    const SERVICE: Service = Service::Voice;
}

/// The video plane's tag, same standing as [`VoicePlane`].
struct VideoPlane;

impl Plane for VideoPlane {
    const SERVICE: Service = Service::Video;
}

/// Bind the voice and video overlay planes on this node's `/128`, retrying
/// until the reachability plane has the interface up. Returns a per-service
/// [`DataPlane`] over each socket, sharing the one `admission` (the hub's
/// session-driven active-flow set answers for both services).
///
/// MUST be called on the runtime that will own the media pipeline: like
/// `DataPlane::new` everywhere, the plane's demux/accept pumps live on the
/// calling runtime. The qmdb-resolver constraint that pins statesync's plane
/// to the node runtime does not apply here, so this runs on the voice hub's
/// own runtime (proven safe: the overlay stack polls on the reachability
/// runtime and its virtual sockets are woken cross-runtime).
pub async fn bind_media_planes(
    factory: Arc<dyn SocketFactory>,
    peers: Arc<OverlayPeers>,
    me: [u8; 32],
    admission: Arc<dyn AdmissionPolicy>,
) -> (DataPlane<OverlaySockets>, DataPlane<OverlaySockets>) {
    let voice_sockets = bind_service::<VoicePlane>(&factory, &peers, me).await;
    let video_sockets = bind_service::<VideoPlane>(&factory, &peers, me).await;
    let voice_plane = DataPlane::new(voice_sockets, admission.clone(), MEDIA_PLANE_CONFIG);
    let video_plane = DataPlane::new(video_sockets, admission, MEDIA_PLANE_CONFIG);
    (voice_plane, video_plane)
}

/// Bind one media service's overlay sockets on this node's `/128`, retrying
/// the seconds the reachability plane needs to bring it up. The per-service
/// [`OverlayBook`] stamps this service's ports on egress so datagrams land on
/// the peer's matching socket.
async fn bind_service<P: Plane>(
    factory: &Arc<dyn SocketFactory>,
    peers: &Arc<OverlayPeers>,
    me: [u8; 32],
) -> OverlaySockets {
    let book: Arc<dyn AddressBook> = OverlayBook::<P>::new(Arc::clone(peers));
    bind_overlay_sockets(
        factory.clone(),
        peers.own_ip(&me),
        P::SERVICE,
        book,
        BIND_RETRY,
    )
    .await
}
