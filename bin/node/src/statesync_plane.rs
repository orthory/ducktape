//! statesync's per-use data plane — the node-side wiring for
//! `statesync::dataplane` over the WireGuard overlay.
//!
//! per the per-use data-plane ADR (docs/adr/2026-07-07-per-use-data-plane.mdx),
//! statesync instantiates its OWN `DataPlane` over `OverlaySockets`, on the
//! node's own runtime (the qmdb resolver lane must be pollable across the
//! plane's futures — a bridged second runtime would wedge it). the host owns
//! instantiation: this module supplies the address book + admission (one
//! object — both derive from the same tracked peer set), the lazy bring-up
//! (the overlay `/128` only exists once the reachability plane has the
//! interface up, so binding retries in the background), and the
//! prefer-plane-fall-back-to-mesh client the joiner paths consume.
//!
//! env-gated, default OFF (`DUCKTAPE_STATESYNC_PLANE=1`): the plane's on-path
//! only does anything with real tunnels, and the mesh statesync path is the
//! retained fallback either way.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, OnceLock, RwLock};

use commonware_cryptography::ed25519;
use data_plane::{
    AddressBook, AdmissionPolicy, DataPlane, FlowId, OverlaySockets, PeerId, PlaneConfig, Service,
    StreamPolicy, StreamService,
};
use statesync::dataplane::{DataPlaneSyncClient, statesync_flow};
use statesync::{SyncClient, SyncError, SyncRequest, SyncResponse};

/// the enable gate: the plane's sockets bind only when the operator opts in.
pub fn enabled() -> bool {
    std::env::var("DUCKTAPE_STATESYNC_PLANE").is_ok_and(|v| v == "1")
}

/// the socket seam's factory selection, in one place for every
/// [`spawn_bring_up`] caller: the plane's backend follows `wireguard_effect`
/// exactly as the mesh context's does (fake stages no data plane, so it
/// keeps the OS factory's downed-interface behavior).
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

/// bulk ceiling for the statesync plane instance: a static compromise between
/// sync time (~24 MB/s ≈ 42 s/GB) and real-time headroom — on uplinks faster
/// than 192 Mbit/s other planes keep headroom, on slower ones bulk can still
/// crowd them (adaptive cross-plane coordination arrives with the second bulk
/// consumer).
const BULK_BYTES_PER_SEC: u64 = 24_000_000;
const BULK_BURST_BYTES: u64 = 512 * 1024;

/// derive a peer's overlay ULA from its raw ed25519 key bytes — the same
/// `(namespace, identity)` function the reachability plane routes by.
fn ula_of(namespace: &str, raw_key: &[u8; 32]) -> std::net::Ipv6Addr {
    wireguard_upgrade::ula_v6_member_addr(namespace, wireguard_upgrade::ValidatorIdentity(*raw_key))
}

/// the address book AND admission policy for the statesync plane, one object:
/// both answer from the same tracked peer set (members + standbys — exactly
/// who the mesh serves statesync to), maintained by the node at boot and at
/// every cutover re-track. forward resolution derives (ULA is a pure function
/// of identity); reverse resolution and admission consult the set.
pub struct OverlayBook {
    namespace: String,
    /// source `/128` -> authenticated peer, rebuilt on every `set_peers`.
    reverse: RwLock<HashMap<IpAddr, PeerId>>,
}

impl OverlayBook {
    pub fn new(namespace: String) -> Arc<Self> {
        Arc::new(OverlayBook {
            namespace,
            reverse: RwLock::new(HashMap::new()),
        })
    }

    /// replace the tracked peer set (members + standbys of the current view).
    pub fn set_peers<'a>(&self, keys: impl Iterator<Item = &'a ed25519::PublicKey>) {
        let mut reverse = HashMap::new();
        for key in keys {
            let raw: [u8; 32] = key.as_ref().try_into().expect("ed25519 keys are 32 bytes");
            reverse.insert(
                IpAddr::V6(ula_of(&self.namespace, &raw)),
                PeerId(raw),
            );
        }
        *self.reverse.write().expect("book lock") = reverse;
    }

    /// this node's own overlay `/128` — where the plane's sockets bind.
    pub fn own_addr(&self, me: &ed25519::PublicKey) -> IpAddr {
        let raw: [u8; 32] = me.as_ref().try_into().expect("ed25519 keys are 32 bytes");
        IpAddr::V6(ula_of(&self.namespace, &raw))
    }
}

impl AddressBook for OverlayBook {
    fn datagram_addr(&self, peer: PeerId) -> Option<SocketAddr> {
        Some(SocketAddr::new(
            IpAddr::V6(ula_of(&self.namespace, &peer.0)),
            Service::StateSync.overlay_datagram_port(),
        ))
    }
    fn stream_addr(&self, peer: PeerId) -> Option<SocketAddr> {
        Some(SocketAddr::new(
            IpAddr::V6(ula_of(&self.namespace, &peer.0)),
            Service::StateSync.overlay_stream_port(),
        ))
    }
    fn peer_at(&self, src: IpAddr) -> Option<PeerId> {
        self.reverse.read().expect("book lock").get(&src).copied()
    }
}

impl AdmissionPolicy for OverlayBook {
    fn permits(&self, peer: PeerId, service: Service, flow: FlowId) -> bool {
        service == Service::StateSync
            && flow == statesync_flow()
            && self
                .reverse
                .read()
                .expect("book lock")
                .values()
                .any(|p| *p == peer)
    }
}

/// the plane's stream service once the overlay bind succeeds — filled by
/// [`spawn_bring_up`]'s retry task, read by clients and the serve acceptor.
pub type PlaneSlot = Arc<OnceLock<Arc<StreamService<OverlaySockets>>>>;

/// one statesync request as the drain loop's serve arm consumes it: from the
/// mesh channel (an rpc envelope, answered back over the mesh) or from a
/// plane stream (the stream itself is the correlation AND the reply path).
pub enum SyncJob {
    Mesh(ed25519::PublicKey, Vec<u8>),
    Plane(DynStream, Vec<u8>),
}

/// where a [`SyncJob`]'s answer goes — split from the job so the serve arm
/// can consume the request bytes while holding the reply path.
pub enum SyncReplyTo {
    Mesh(ed25519::PublicKey),
    Plane(DynStream),
}

/// bring the statesync plane up in the background: the overlay `/128` exists
/// only once the reachability plane has the interface configured, so the bind
/// retries until it lands (or the process exits). on success the stream
/// service lands in `slot` and, when `serve` is set, an acceptor task starts
/// feeding accepted request streams into it.
pub fn spawn_bring_up(
    label: String,
    book: Arc<OverlayBook>,
    me: ed25519::PublicKey,
    slot: PlaneSlot,
    // the socket seam (overlay-net ADR): `OsSocketFactory` in tun mode (the
    // kernel routes the /128 through the wireguard interface),
    // `VirtualSocketFactory` in socket mode (the /128 lives in the
    // in-process stack). either way its bind errors while the overlay is
    // down are absorbed by the retry loop below.
    factory: Arc<dyn data_plane::SocketFactory>,
    serve: Option<futures::channel::mpsc::Sender<SyncJob>>,
) {
    tokio::spawn(async move {
        let own = book.own_addr(&me);
        let datagram_bind = SocketAddr::new(own, Service::StateSync.overlay_datagram_port());
        let stream_bind = SocketAddr::new(own, Service::StateSync.overlay_stream_port());
        let sockets = loop {
            match OverlaySockets::bind_with(
                factory.clone(),
                datagram_bind,
                stream_bind,
                book.clone(),
            )
            .await
            {
                Ok(sockets) => break sockets,
                // the interface (or our /128) is not up yet — retry quietly.
                Err(_) => tokio::time::sleep(std::time::Duration::from_secs(3)).await,
            }
        };
        let admission: Arc<dyn AdmissionPolicy> = book;
        let plane = DataPlane::new(
            sockets,
            admission,
            PlaneConfig {
                bulk_bytes_per_sec: BULK_BYTES_PER_SEC,
                bulk_burst_bytes: BULK_BURST_BYTES,
            },
        );
        let svc = match plane.stream_service(Service::StateSync, StreamPolicy { accept_backlog: 32 })
        {
            Ok(svc) => Arc::new(svc),
            Err(e) => {
                eprintln!("[node {label}] statesync plane: register failed ({e}) — mesh only");
                return;
            }
        };
        println!("[node {label}] statesync plane: overlay sockets bound on {own}");
        let _ = slot.set(Arc::clone(&svc));
        // the plane must outlive this task's loops — its pumps stop when it
        // drops, so it lives in this scope for the process life.
        let _plane = plane;
        let Some(jobs) = serve else {
            // client-only plane (a joiner): nothing to accept, just stay
            // alive so the pumps keep running.
            std::future::pending::<()>().await;
            return;
        };
        // the serve acceptor: one accepted stream = one request/response.
        // reading the request frame happens per-stream (never blocks the
        // accept loop); the drain loop answers and writes the response.
        loop {
            let Some((_peer, _hello, mut stream)) = svc.accept().await else {
                return;
            };
            let mut jobs = jobs.clone();
            tokio::spawn(async move {
                let Ok(req) = statesync::dataplane::read_frame(&mut stream).await else {
                    return;
                };
                // full queue = flood pressure: drop; clients time out + retry.
                let _ = jobs.try_send(SyncJob::Plane(Box::new(stream) as DynStream, req));
            });
        }
    });
}

/// a duplex byte stream as the drain loop sees it — concrete plane stream
/// types stay in this module.
pub trait DuplexStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin {}
impl<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin> DuplexStream for S {}
pub type DynStream = Box<dyn DuplexStream>;

/// the joiner-side client: prefer the plane once it is up, fall back to the
/// mesh client on transport failure (Unreachable/Refused) — the mesh path IS
/// the retained fallback per the design. before the plane binds, every
/// request rides the mesh.
pub struct PlaneFallbackClient<M> {
    plane: PlaneSlot,
    server: PeerId,
    mesh: M,
}

impl<M: Clone> Clone for PlaneFallbackClient<M> {
    fn clone(&self) -> Self {
        Self {
            plane: Arc::clone(&self.plane),
            server: self.server,
            mesh: self.mesh.clone(),
        }
    }
}

impl<M> PlaneFallbackClient<M> {
    pub fn new(plane: PlaneSlot, server: &ed25519::PublicKey, mesh: M) -> Self {
        let raw: [u8; 32] = server
            .as_ref()
            .try_into()
            .expect("ed25519 keys are 32 bytes");
        Self {
            plane,
            server: PeerId(raw),
            mesh,
        }
    }

    /// unwrap the mesh client (the boot path hands its channel halves back
    /// to the serve loop once catch-up completes).
    pub fn into_inner(self) -> M {
        self.mesh
    }
}

impl<M: SyncClient + Sync> SyncClient for PlaneFallbackClient<M> {
    async fn request(&self, req: SyncRequest) -> Result<SyncResponse, SyncError> {
        if let Some(svc) = self.plane.get() {
            let client = DataPlaneSyncClient::new(Arc::clone(svc), self.server);
            match client.request(req.clone()).await {
                // a transport-level failure (tunnel down, peer refused)
                // falls back; protocol-level outcomes are authoritative.
                Err(SyncError::Transport(_)) => {}
                outcome => return outcome,
            }
        }
        self.mesh.request(req).await
    }
}
