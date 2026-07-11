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
//! env-gated, default OFF (`DUCKTAPE_STATESYNC_PLANE=1`, see [`enabled`]):
//! the plane's on-path only does anything with real tunnels.
//!
//! the mesh leg is not a legacy path pending deletion — it is NOT deletable,
//! for four verified reasons (the wire-standardization ADR's D4 decision):
//!
//! 1. **default-off gate.** with the env var unset, mesh is today's PRIMARY
//!    statesync path, not a fallback of a fallback.
//! 2. **park-phase admission polling.** a parked (pre-admission) joiner
//!    polls the manifest over statesync to detect its OWN admission, and
//!    [`OverlayBook`]'s admission (members + standbys only) refuses that
//!    joiner on the plane by construction — only the mesh can answer a
//!    peer the network has not admitted yet.
//! 3. **async bind window.** [`spawn_bring_up`]'s overlay bind retries every
//!    3s until the interface exists, so a real window always exists where
//!    the plane is enabled but not yet up.
//! 4. **terminal punch failure.** a failed NAT punch is terminal (no relay
//!    since its removal) — some peer pairs never get a tunnel at all, and
//!    the mesh is their only path, permanently.
//!
//! [`PlaneFallbackClient`] is the resulting policy: prefer the plane,
//! degrade to the mesh on transport failure, and log every fallback — the
//! plane's steady-state degradation must be diagnosable, never silent.

use std::sync::{Arc, OnceLock};

use commonware_cryptography::ed25519;
use data_plane::{
    BulkPacer, FlowId, OverlaySockets, PeerId, Service, StreamPacing, StreamPlaneSpec,
    StreamPolicy, StreamService, bind_stream_plane,
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
/// sync time (~24 MB/s ≈ 42 s/GB) and real-time headroom. State sync and
/// gateway and agent telemetry share this one process ceiling; adaptive
/// per-link shaping is a separate concern.
const BULK_BYTES_PER_SEC: u64 = 24_000_000;
const BULK_BURST_BYTES: u64 = 512 * 1024;

/// One link-headroom budget shared by every stream-class per-use plane in this
/// process (state sync, gateway responses, and agent telemetry today).
pub fn shared_bulk_pacer() -> BulkPacer {
    BulkPacer::new(BULK_BYTES_PER_SEC, BULK_BURST_BYTES)
}

/// the statesync plane's tag for the shared [`crate::overlay_book::OverlayBook`]:
/// admission permits exactly the statesync service + flow, from the tracked
/// peer set (members + standbys — exactly who the mesh serves statesync to).
pub struct StateSyncPlane;

impl crate::overlay_book::Plane for StateSyncPlane {
    const SERVICE: Service = Service::StateSync;
    fn flow() -> FlowId {
        statesync_flow()
    }
}

pub type OverlayBook = crate::overlay_book::OverlayBook<StateSyncPlane>;

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
    pacer: BulkPacer,
    serve: Option<futures::channel::mpsc::Sender<SyncJob>>,
) {
    tokio::spawn(async move {
        let own = book.own_addr(&me);
        let spec = StreamPlaneSpec {
            own_ip: own,
            service: Service::StateSync,
            pacing: StreamPacing::Shared(pacer),
            policy: StreamPolicy { accept_backlog: 32 },
            // the interface (or our /128) is not up yet — retry quietly.
            retry: std::time::Duration::from_secs(3),
        };
        let (plane, svc) = match bind_stream_plane(spec, factory, book).await {
            Ok(bound) => bound,
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
    /// the node's own log label — steady-state fallbacks are otherwise
    /// invisible (silent per-request degradation), and a label-less
    /// `eprintln` is useless once more than one node's logs interleave.
    label: String,
}

impl<M: Clone> Clone for PlaneFallbackClient<M> {
    fn clone(&self) -> Self {
        Self {
            plane: Arc::clone(&self.plane),
            server: self.server,
            mesh: self.mesh.clone(),
            label: self.label.clone(),
        }
    }
}

impl<M> PlaneFallbackClient<M> {
    pub fn new(plane: PlaneSlot, server: &ed25519::PublicKey, mesh: M, label: String) -> Self {
        let raw: [u8; 32] = server
            .as_ref()
            .try_into()
            .expect("ed25519 keys are 32 bytes");
        Self {
            plane,
            server: PeerId(raw),
            mesh,
            label,
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
                Err(SyncError::Transport(e)) => {
                    let label = &self.label;
                    eprintln!(
                        "[node {label}] statesync: plane request failed, falling back to mesh: {e}"
                    );
                }
                outcome => return outcome,
            }
        }
        self.mesh.request(req).await
    }
}
