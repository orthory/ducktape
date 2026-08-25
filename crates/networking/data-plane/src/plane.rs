//! The plane controller: demux, admission, per-flow queues, bulk pacing.
//!
//! All isolation lives here, above the transport seam:
//! - **Class isolation** is structural (separate datagram/stream paths in
//!   the transport).
//! - **Link isolation**: every stream write draws from one global bulk
//!   token bucket, so bulk self-limits below the link and never queues
//!   ahead of real-time datagrams.
//! - **Flow isolation**: per-flow bounded queues; a datagram flow overflows
//!   by dropping its own oldest, never a neighbor's.
//! - **Admission** (consensus-derived, injected): default-deny on receive
//!   AND send. Unadmitted traffic is dropped at demux, counted, and
//!   attributed; a correct node cannot emit rogue traffic because flow
//!   handles are the only send surface.

use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::task::{Context, Poll};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::sync::{Notify, mpsc};
use tokio::time::{Instant, Sleep, sleep_until, timeout};

use crate::Service;
use crate::flow::{DatagramPolicy, FlowId, StreamPolicy};
use crate::monitor::{PlaneObservation, PlaneWatch};
use crate::transport::{DataPlaneTransport, PeerId, TransportError};
use crate::wire::{self, HELLO_ACK, Hello, WireError};

/// How long an acceptor waits for the opener's hello, and an opener for the
/// ack, before treating the stream as dead.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
/// Largest single token grant — bounds how bursty one paced write can be.
const MAX_GRANT: usize = 16 * 1024;

/// The consensus-derived admission view, injected by the node layer (e.g. a
/// view over finalized channel membership / valset state). Both ends of a
/// flow evaluate the same replicated state, so no admission signaling
/// crosses the wire. Must be cheap: called per datagram.
pub trait AdmissionPolicy: Send + Sync + 'static {
    fn permits(&self, peer: PeerId, service: Service, flow: FlowId) -> bool;
}

#[derive(Clone, Copy, Debug)]
pub struct PlaneConfig {
    /// The bulk (stream-class) ceiling all streams share. Size it below the
    /// expected link rate — the headroom is what keeps datagram latency flat.
    pub bulk_bytes_per_sec: u64,
    /// Bucket depth: the largest instantaneous bulk burst, and therefore
    /// the worst-case transient queue bulk can put in front of a datagram.
    pub bulk_burst_bytes: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum RegisterError {
    #[error("flow or service already registered")]
    AlreadyRegistered,
}

#[derive(Debug, thiserror::Error)]
pub enum SendError {
    /// Consensus state does not admit this (peer, service, flow) — refusing
    /// to emit is what keeps a correct node from unknowingly sending rogue
    /// traffic.
    #[error("not admitted")]
    NotAdmitted,
    #[error(transparent)]
    Wire(#[from] WireError),
    #[error(transparent)]
    Transport(#[from] TransportError),
}

#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error("not admitted")]
    NotAdmitted,
    #[error(transparent)]
    Wire(#[from] WireError),
    #[error(transparent)]
    Transport(#[from] TransportError),
    /// No ack: the far side dropped the hello (unadmitted, unregistered
    /// service, or backlog full) or timed out.
    #[error("open refused by peer")]
    Refused,
}

#[derive(Default)]
struct Stats {
    rogue_datagrams: AtomicU64,
    rogue_streams: AtomicU64,
    malformed_datagrams: AtomicU64,
    unregistered_datagrams: AtomicU64,
    unregistered_streams: AtomicU64,
    /// Inbound streams whose opener never completed a well-formed hello
    /// within the handshake timeout (dropped without ack).
    hello_failed_streams: AtomicU64,
    /// Admitted, registered inbound streams refused because the service's
    /// accept backlog was full (dropped without ack).
    backlog_refused_streams: AtomicU64,
    refused_sends: AtomicU64,
    rogue_by_peer: Mutex<HashMap<PeerId, u64>>,
}

impl Stats {
    fn count_rogue(&self, counter: &AtomicU64, peer: PeerId) {
        counter.fetch_add(1, Ordering::Relaxed);
        *self
            .rogue_by_peer
            .lock()
            .expect("stats lock")
            .entry(peer)
            .or_insert(0) += 1;
    }

    fn snapshot(&self) -> StatsSnapshot {
        StatsSnapshot {
            rogue_datagrams: self.rogue_datagrams.load(Ordering::Relaxed),
            rogue_streams: self.rogue_streams.load(Ordering::Relaxed),
            malformed_datagrams: self.malformed_datagrams.load(Ordering::Relaxed),
            unregistered_datagrams: self.unregistered_datagrams.load(Ordering::Relaxed),
            unregistered_streams: self.unregistered_streams.load(Ordering::Relaxed),
            hello_failed_streams: self.hello_failed_streams.load(Ordering::Relaxed),
            backlog_refused_streams: self.backlog_refused_streams.load(Ordering::Relaxed),
            refused_sends: self.refused_sends.load(Ordering::Relaxed),
        }
    }
}

/// Successful-traffic accounting, one set per plane. Datagram byte counts are
/// WIRE frames (header included); stream byte counts are the payload bytes
/// consumers read/write through their [`PacedStream`]s (the one-frame hello
/// handshake is not counted). All counters are cumulative for the plane's
/// life — rates are the reader's derivation.
#[derive(Default)]
struct Traffic {
    datagrams_tx: AtomicU64,
    datagram_bytes_tx: AtomicU64,
    datagrams_rx: AtomicU64,
    datagram_bytes_rx: AtomicU64,
    /// Datagrams shed by per-flow drop-oldest overflow, summed across all of
    /// this plane's flows (survives individual flow teardown, unlike
    /// [`DatagramFlow::dropped`]).
    datagrams_shed: AtomicU64,
    stream_bytes_tx: AtomicU64,
    stream_bytes_rx: AtomicU64,
    streams_opened: AtomicU64,
    streams_accepted: AtomicU64,
    /// Set when a demux/accept pump exits (transport closed): the plane still
    /// holds its sockets but no longer moves traffic.
    halted: AtomicBool,
}

impl Traffic {
    fn snapshot(&self) -> TrafficSnapshot {
        TrafficSnapshot {
            datagrams_tx: self.datagrams_tx.load(Ordering::Relaxed),
            datagram_bytes_tx: self.datagram_bytes_tx.load(Ordering::Relaxed),
            datagrams_rx: self.datagrams_rx.load(Ordering::Relaxed),
            datagram_bytes_rx: self.datagram_bytes_rx.load(Ordering::Relaxed),
            datagrams_shed: self.datagrams_shed.load(Ordering::Relaxed),
            stream_bytes_tx: self.stream_bytes_tx.load(Ordering::Relaxed),
            stream_bytes_rx: self.stream_bytes_rx.load(Ordering::Relaxed),
            streams_opened: self.streams_opened.load(Ordering::Relaxed),
            streams_accepted: self.streams_accepted.load(Ordering::Relaxed),
            halted: self.halted.load(Ordering::Relaxed),
        }
    }
}

/// A point-in-time copy of the plane's successful-traffic accounting — see
/// [`Traffic`] for what each counter measures.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TrafficSnapshot {
    pub datagrams_tx: u64,
    pub datagram_bytes_tx: u64,
    pub datagrams_rx: u64,
    pub datagram_bytes_rx: u64,
    pub datagrams_shed: u64,
    pub stream_bytes_tx: u64,
    pub stream_bytes_rx: u64,
    pub streams_opened: u64,
    pub streams_accepted: u64,
    /// A demux/accept pump has exited (transport closed): the plane is bound
    /// but no longer moves traffic.
    pub halted: bool,
}

/// A point-in-time copy of the plane's drop accounting. No drop is silent:
/// every inbound stream or datagram the plane refuses lands in exactly one
/// counter here (rogue traffic attributed to its peer).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StatsSnapshot {
    pub rogue_datagrams: u64,
    pub rogue_streams: u64,
    pub malformed_datagrams: u64,
    pub unregistered_datagrams: u64,
    pub unregistered_streams: u64,
    /// Inbound streams dropped for a missing or malformed hello.
    pub hello_failed_streams: u64,
    /// Inbound streams dropped because the service's accept backlog was full.
    pub backlog_refused_streams: u64,
    pub refused_sends: u64,
}

struct DatagramQueue {
    max: usize,
    queue: Mutex<VecDeque<(PeerId, Vec<u8>)>>,
    dropped: AtomicU64,
    notify: Notify,
}

impl DatagramQueue {
    fn new(max: usize) -> Self {
        DatagramQueue {
            max: max.max(1),
            queue: Mutex::new(VecDeque::new()),
            dropped: AtomicU64::new(0),
            notify: Notify::new(),
        }
    }

    /// Returns whether an oldest datagram was shed to make room.
    fn push(&self, from: PeerId, payload: Vec<u8>) -> bool {
        let mut q = self.queue.lock().expect("queue lock");
        let shed = q.len() == self.max;
        if shed {
            // Drop-oldest: for real-time traffic the newest datagram is the
            // valuable one, and the overflow stays inside this flow.
            q.pop_front();
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
        q.push_back((from, payload));
        drop(q);
        self.notify.notify_one();
        shed
    }

    async fn recv(&self) -> (PeerId, Vec<u8>) {
        loop {
            let notified = self.notify.notified();
            if let Some(item) = self.queue.lock().expect("queue lock").pop_front() {
                return item;
            }
            notified.await;
        }
    }
}

type IncomingStream<S> = (PeerId, Hello, S);

struct Shared<T: DataPlaneTransport> {
    transport: Arc<T>,
    admission: Arc<dyn AdmissionPolicy>,
    bucket: Arc<TokenBucket>,
    datagram_flows: Mutex<HashMap<(Service, FlowId), Arc<DatagramQueue>>>,
    stream_services: Mutex<HashMap<Service, mpsc::Sender<IncomingStream<T::Stream>>>>,
    stats: Stats,
    /// `Arc` so a [`PacedStream`] (which may outlive every plane handle)
    /// keeps its byte accounting attached to this plane.
    traffic: Arc<Traffic>,
    /// The demux and accept pumps. They hold only a `Weak` back to this
    /// state (plus the transport they block on), so the last handle's drop
    /// runs [`Shared::drop`], which aborts them — a plane's life is exactly
    /// the life of its handles, never extended by its own pumps.
    pumps: Mutex<Vec<tokio::task::JoinHandle<()>>>,
}

impl<T: DataPlaneTransport> Drop for Shared<T> {
    fn drop(&mut self) {
        for pump in self.pumps.lock().expect("pumps lock").drain(..) {
            pump.abort();
        }
    }
}

/// The plane. Cloneable handle; spawns its demux and acceptor loops on
/// construction. The loops run until the transport closes or the last
/// handle (plane, flow, or service) drops, whichever comes first.
pub struct DataPlane<T: DataPlaneTransport> {
    shared: Arc<Shared<T>>,
}

/// A process-wide stream-class byte budget that can be injected into multiple
/// per-use planes. Planes keep separate sockets/queues/admission, while bulk
/// consumers share the one link-headroom ceiling required by the per-use ADR.
#[derive(Clone)]
pub struct BulkPacer {
    bucket: Arc<TokenBucket>,
}

impl BulkPacer {
    pub fn new(bytes_per_sec: u64, burst_bytes: u64) -> Self {
        Self {
            bucket: Arc::new(TokenBucket::new(bytes_per_sec, burst_bytes)),
        }
    }
}

impl<T: DataPlaneTransport> Clone for DataPlane<T> {
    fn clone(&self) -> Self {
        DataPlane {
            shared: self.shared.clone(),
        }
    }
}

impl<T: DataPlaneTransport> DataPlane<T> {
    pub fn new(transport: T, admission: Arc<dyn AdmissionPolicy>, config: PlaneConfig) -> Self {
        Self::new_with_pacer(
            transport,
            admission,
            BulkPacer::new(config.bulk_bytes_per_sec, config.bulk_burst_bytes),
        )
    }

    /// Construct a per-use plane that draws stream writes from an injected
    /// process-wide budget. Datagram paths never touch this pacer.
    pub fn new_with_pacer(
        transport: T,
        admission: Arc<dyn AdmissionPolicy>,
        pacer: BulkPacer,
    ) -> Self {
        let transport = Arc::new(transport);
        let shared = Arc::new(Shared {
            transport: transport.clone(),
            admission,
            bucket: pacer.bucket,
            datagram_flows: Mutex::new(HashMap::new()),
            stream_services: Mutex::new(HashMap::new()),
            stats: Stats::default(),
            traffic: Arc::new(Traffic::default()),
            pumps: Mutex::new(Vec::new()),
        });
        let pumps = vec![
            tokio::spawn(demux_loop(transport.clone(), Arc::downgrade(&shared))),
            tokio::spawn(accept_loop(transport, Arc::downgrade(&shared))),
        ];
        *shared.pumps.lock().expect("pumps lock") = pumps;
        DataPlane { shared }
    }

    /// Register the consumer end of a datagram flow. The handle is the ONLY
    /// send surface for the flow; dropping it unregisters.
    pub fn datagram_flow(
        &self,
        service: Service,
        flow: FlowId,
        policy: DatagramPolicy,
    ) -> Result<DatagramFlow<T>, RegisterError> {
        let mut flows = self.shared.datagram_flows.lock().expect("flows lock");
        if flows.contains_key(&(service, flow)) {
            return Err(RegisterError::AlreadyRegistered);
        }
        let queue = Arc::new(DatagramQueue::new(policy.max_queued));
        flows.insert((service, flow), queue.clone());
        Ok(DatagramFlow {
            shared: self.shared.clone(),
            service,
            flow,
            queue,
        })
    }

    /// Register a stream-class service (acceptor + opener). Dropping the
    /// handle unregisters.
    pub fn stream_service(
        &self,
        service: Service,
        policy: StreamPolicy,
    ) -> Result<StreamService<T>, RegisterError> {
        let mut services = self.shared.stream_services.lock().expect("services lock");
        if services.contains_key(&service) {
            return Err(RegisterError::AlreadyRegistered);
        }
        let (tx, rx) = mpsc::channel(policy.accept_backlog.max(1));
        services.insert(service, tx);
        Ok(StreamService {
            shared: self.shared.clone(),
            service,
            incoming: tokio::sync::Mutex::new(rx),
        })
    }

    pub fn stats(&self) -> StatsSnapshot {
        self.shared.stats.snapshot()
    }

    /// A point-in-time copy of the plane's successful-traffic accounting.
    pub fn traffic(&self) -> TrafficSnapshot {
        self.shared.traffic.snapshot()
    }

    /// A type-erased weak observer for this plane — the handle a
    /// [`crate::monitor::PlaneMonitor`] holds. It never keeps the plane
    /// alive: `observe` yields `None` once every handle is gone and the
    /// demux/accept pumps have stopped.
    pub fn watch(&self) -> PlaneWatch {
        let weak: Weak<Shared<T>> = Arc::downgrade(&self.shared);
        PlaneWatch::new(move || {
            weak.upgrade().map(|shared| PlaneObservation {
                stats: shared.stats.snapshot(),
                traffic: shared.traffic.snapshot(),
            })
        })
    }

    /// Rogue traffic attributed to one peer — the transport authenticates
    /// the sender, so this is *knowledge*, not a guess (though pairwise
    /// authentication means it is not third-party-provable evidence).
    pub fn rogue_from(&self, peer: PeerId) -> u64 {
        self.shared
            .stats
            .rogue_by_peer
            .lock()
            .expect("stats lock")
            .get(&peer)
            .copied()
            .unwrap_or(0)
    }
}

/// The pumps block on the transport while holding only a `Weak` to the plane
/// state; a receive that lands after the last handle dropped finds no plane
/// and ends the pump (the abort in [`Shared::drop`] normally gets there
/// first — this is the race's other arm).
async fn demux_loop<T: DataPlaneTransport>(transport: Arc<T>, plane: Weak<Shared<T>>) {
    loop {
        let (from, frame) = match transport.recv_datagram().await {
            Ok(inbound) => inbound,
            Err(_) => {
                if let Some(shared) = plane.upgrade() {
                    shared.traffic.halted.store(true, Ordering::Relaxed);
                }
                return;
            }
        };
        let Some(shared) = plane.upgrade() else {
            return;
        };
        let (service, flow, payload) = match wire::decode_datagram(&frame) {
            Ok(parts) => parts,
            Err(_) => {
                shared
                    .stats
                    .malformed_datagrams
                    .fetch_add(1, Ordering::Relaxed);
                continue;
            }
        };
        if !shared.admission.permits(from, service, flow) {
            shared
                .stats
                .count_rogue(&shared.stats.rogue_datagrams, from);
            continue;
        }
        let queue = shared
            .datagram_flows
            .lock()
            .expect("flows lock")
            .get(&(service, flow))
            .cloned();
        match queue {
            Some(queue) => {
                shared.traffic.datagrams_rx.fetch_add(1, Ordering::Relaxed);
                shared
                    .traffic
                    .datagram_bytes_rx
                    .fetch_add(frame.len() as u64, Ordering::Relaxed);
                if queue.push(from, payload.to_vec()) {
                    shared
                        .traffic
                        .datagrams_shed
                        .fetch_add(1, Ordering::Relaxed);
                }
            }
            None => {
                shared
                    .stats
                    .unregistered_datagrams
                    .fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

async fn accept_loop<T: DataPlaneTransport>(transport: Arc<T>, plane: Weak<Shared<T>>) {
    loop {
        let (peer, stream) = match transport.accept().await {
            Ok(inbound) => inbound,
            Err(_) => {
                if let Some(shared) = plane.upgrade() {
                    shared.traffic.halted.store(true, Ordering::Relaxed);
                }
                return;
            }
        };
        if plane.upgrade().is_none() {
            return;
        }
        // Per-connection task: a stalled opener must not block the acceptor.
        // It, too, holds the plane weakly: a hello still pending when the
        // last handle drops must not keep the plane's state alive.
        tokio::spawn(handle_inbound_stream(plane.clone(), peer, stream));
    }
}

async fn handle_inbound_stream<T: DataPlaneTransport>(
    plane: Weak<Shared<T>>,
    peer: PeerId,
    mut stream: T::Stream,
) {
    let hello = timeout(HANDSHAKE_TIMEOUT, wire::read_hello(&mut stream)).await;
    let Some(shared) = plane.upgrade() else {
        return;
    };
    let hello = match hello {
        Ok(Ok(hello)) => hello,
        // Timeout or garbage: drop without ack, counted.
        _ => {
            shared
                .stats
                .hello_failed_streams
                .fetch_add(1, Ordering::Relaxed);
            return;
        }
    };
    if !shared.admission.permits(peer, hello.service, hello.flow) {
        shared.stats.count_rogue(&shared.stats.rogue_streams, peer);
        return;
    }
    let sender = shared
        .stream_services
        .lock()
        .expect("services lock")
        .get(&hello.service)
        .cloned();
    let Some(sender) = sender else {
        shared
            .stats
            .unregistered_streams
            .fetch_add(1, Ordering::Relaxed);
        return;
    };
    // Reserve the backlog slot BEFORE acking, so an ack always means the
    // consumer will actually see the stream.
    let Ok(slot) = sender.try_reserve() else {
        shared
            .stats
            .backlog_refused_streams
            .fetch_add(1, Ordering::Relaxed);
        return;
    };
    // An ack the opener never reads is the opener's departure, not a drop
    // of ours: the stream was theirs to abandon.
    if stream.write_all(&[HELLO_ACK]).await.is_err() {
        return;
    }
    slot.send((peer, hello, stream));
}

/// The send/receive handle for one datagram flow.
pub struct DatagramFlow<T: DataPlaneTransport> {
    shared: Arc<Shared<T>>,
    service: Service,
    flow: FlowId,
    queue: Arc<DatagramQueue>,
}

impl<T: DataPlaneTransport> DatagramFlow<T> {
    /// Fire-and-forget send. Checks admission per send — membership can
    /// change under a live handle.
    pub async fn send_to(&self, to: PeerId, payload: &[u8]) -> Result<(), SendError> {
        if !self.shared.admission.permits(to, self.service, self.flow) {
            self.shared
                .stats
                .refused_sends
                .fetch_add(1, Ordering::Relaxed);
            return Err(SendError::NotAdmitted);
        }
        let frame = wire::encode_datagram(self.service, self.flow, payload)?;
        let wire_bytes = frame.len() as u64;
        self.shared.transport.send_datagram(to, frame).await?;
        self.shared
            .traffic
            .datagrams_tx
            .fetch_add(1, Ordering::Relaxed);
        self.shared
            .traffic
            .datagram_bytes_tx
            .fetch_add(wire_bytes, Ordering::Relaxed);
        Ok(())
    }

    /// Next datagram for this flow, with its authenticated sender.
    pub async fn recv(&self) -> (PeerId, Vec<u8>) {
        self.queue.recv().await
    }

    /// Datagrams this flow shed via drop-oldest overflow.
    pub fn dropped(&self) -> u64 {
        self.queue.dropped.load(Ordering::Relaxed)
    }
}

impl<T: DataPlaneTransport> Drop for DatagramFlow<T> {
    fn drop(&mut self) {
        self.shared
            .datagram_flows
            .lock()
            .expect("flows lock")
            .remove(&(self.service, self.flow));
    }
}

/// The open/accept handle for one stream-class service. Every stream it
/// yields is paced by the plane's shared bulk bucket.
pub struct StreamService<T: DataPlaneTransport> {
    shared: Arc<Shared<T>>,
    service: Service,
    incoming: tokio::sync::Mutex<mpsc::Receiver<IncomingStream<T::Stream>>>,
}

impl<T: DataPlaneTransport> StreamService<T> {
    pub async fn open(
        &self,
        to: PeerId,
        flow: FlowId,
        intent: u8,
        meta: Vec<u8>,
    ) -> Result<PacedStream<T::Stream>, OpenError> {
        if !self.shared.admission.permits(to, self.service, flow) {
            self.shared
                .stats
                .refused_sends
                .fetch_add(1, Ordering::Relaxed);
            return Err(OpenError::NotAdmitted);
        }
        let mut stream = self.shared.transport.connect(to).await?;
        wire::write_hello(
            &mut stream,
            &Hello {
                service: self.service,
                flow,
                intent,
                meta,
            },
        )
        .await?;
        let mut ack = [0u8; 1];
        match timeout(HANDSHAKE_TIMEOUT, stream.read_exact(&mut ack)).await {
            Ok(Ok(_)) if ack[0] == HELLO_ACK => {}
            _ => return Err(OpenError::Refused),
        }
        self.shared
            .traffic
            .streams_opened
            .fetch_add(1, Ordering::Relaxed);
        Ok(PacedStream::new(
            stream,
            self.shared.bucket.clone(),
            self.shared.traffic.clone(),
        ))
    }

    /// Next accepted stream: `None` once the plane shuts down.
    pub async fn accept(&self) -> Option<(PeerId, Hello, PacedStream<T::Stream>)> {
        let (peer, hello, stream) = self.incoming.lock().await.recv().await?;
        self.shared
            .traffic
            .streams_accepted
            .fetch_add(1, Ordering::Relaxed);
        Some((
            peer,
            hello,
            PacedStream::new(
                stream,
                self.shared.bucket.clone(),
                self.shared.traffic.clone(),
            ),
        ))
    }
}

impl<T: DataPlaneTransport> Drop for StreamService<T> {
    fn drop(&mut self) {
        self.shared
            .stream_services
            .lock()
            .expect("services lock")
            .remove(&self.service);
    }
}

// ---------------------------------------------------------------------------
// Bulk pacing.

enum Grant {
    Take(usize),
    Wait(Instant),
}

/// The shared bulk budget. Tokens are bytes; refilled continuously at
/// `rate`, capped at `burst`.
struct TokenBucket {
    rate: f64,
    burst: f64,
    state: Mutex<BucketState>,
}

struct BucketState {
    tokens: f64,
    refilled_at: Instant,
}

impl TokenBucket {
    fn new(rate_bytes_per_sec: u64, burst_bytes: u64) -> Self {
        TokenBucket {
            rate: rate_bytes_per_sec as f64,
            burst: burst_bytes.max(1) as f64,
            state: Mutex::new(BucketState {
                tokens: burst_bytes as f64,
                refilled_at: Instant::now(),
            }),
        }
    }

    fn grant(&self, want: usize) -> Grant {
        let want = want.clamp(1, MAX_GRANT);
        let mut s = self.state.lock().expect("bucket lock");
        let now = Instant::now();
        s.tokens = (s.tokens + now.duration_since(s.refilled_at).as_secs_f64() * self.rate)
            .min(self.burst);
        s.refilled_at = now;
        let affordable = (want as f64).min(self.burst);
        if s.tokens < affordable {
            // Wait until the whole (bounded) want is affordable: one decent
            // grant per wake instead of byte-dribble churn.
            let deficit = affordable - s.tokens;
            return Grant::Wait(now + Duration::from_secs_f64(deficit / self.rate));
        }
        s.tokens -= affordable;
        Grant::Take(affordable as usize)
    }

    fn refund(&self, n: usize) {
        let mut s = self.state.lock().expect("bucket lock");
        s.tokens = (s.tokens + n as f64).min(self.burst);
    }
}

/// A stream whose writes draw from the shared bulk bucket. Reads pass
/// through untouched — pacing is a sender-side discipline.
pub struct PacedStream<S> {
    inner: S,
    bucket: Arc<TokenBucket>,
    traffic: Arc<Traffic>,
    throttle: Option<Pin<Box<Sleep>>>,
}

impl<S> PacedStream<S> {
    fn new(inner: S, bucket: Arc<TokenBucket>, traffic: Arc<Traffic>) -> Self {
        PacedStream {
            inner,
            bucket,
            traffic,
            throttle: None,
        }
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for PacedStream<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.as_mut().get_mut();
        let before = buf.filled().len();
        let polled = Pin::new(&mut this.inner).poll_read(cx, buf);
        if let Poll::Ready(Ok(())) = polled {
            this.traffic
                .stream_bytes_rx
                .fetch_add((buf.filled().len() - before) as u64, Ordering::Relaxed);
        }
        polled
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for PacedStream<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        if buf.is_empty() {
            return Pin::new(&mut this.inner).poll_write(cx, buf);
        }
        loop {
            if let Some(throttle) = this.throttle.as_mut() {
                std::task::ready!(throttle.as_mut().poll(cx));
                this.throttle = None;
            }
            match this.bucket.grant(buf.len()) {
                Grant::Wait(at) => {
                    // Loop: polling the fresh sleep registers the waker.
                    this.throttle = Some(Box::pin(sleep_until(at)));
                }
                Grant::Take(n) => {
                    return match Pin::new(&mut this.inner).poll_write(cx, &buf[..n]) {
                        Poll::Ready(Ok(written)) => {
                            if written < n {
                                this.bucket.refund(n - written);
                            }
                            this.traffic
                                .stream_bytes_tx
                                .fetch_add(written as u64, Ordering::Relaxed);
                            Poll::Ready(Ok(written))
                        }
                        Poll::Ready(Err(e)) => {
                            this.bucket.refund(n);
                            Poll::Ready(Err(e))
                        }
                        Poll::Pending => {
                            this.bucket.refund(n);
                            Poll::Pending
                        }
                    };
                }
            }
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}
