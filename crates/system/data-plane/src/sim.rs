//! The deterministic in-memory transport arm (`sim` feature): a virtual
//! network of point-to-point links with latency, serialization (bandwidth),
//! and optional datagram loss — the test/simulation counterpart of the
//! future overlay-socket arm.
//!
//! The link model is what makes the isolation proofs honest: BOTH classes'
//! bytes are serialized through the same directed link (a shared
//! `busy_until` horizon), so an unpaced bulk stream genuinely queues ahead
//! of datagrams exactly as it would at a real bottleneck. Run under
//! `tokio::test(start_paused = true)` for deterministic virtual time.

use std::collections::{HashMap, VecDeque};
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::{Notify, Semaphore, mpsc};
use tokio::time::{Instant, sleep_until};

use crate::transport::{DataPlaneTransport, PeerId, TransportError};

/// One direction of a link. `bytes_per_sec` models the bottleneck rate,
/// `latency` the propagation delay on top, `drop_every` deterministic
/// datagram loss (every Nth datagram vanishes; streams never lose — they
/// stand in for a reliable transport).
#[derive(Clone, Copy, Debug)]
pub struct LinkModel {
    pub latency: Duration,
    pub bytes_per_sec: u64,
    pub drop_every: Option<u32>,
}

/// Bytes a stream hands to the link per scheduling step — a stand-in for an
/// MTU-sized segment.
const STREAM_SEGMENT: usize = 1400;
/// Bytes buffered writer-side before `poll_write` backpressures.
const STREAM_WRITE_BUFFER: usize = 64 * 1024;
/// Segments scheduled-but-undelivered per stream direction — the stand-in
/// for a congestion/flow-control window. Together with the write buffer
/// this bounds how far ahead of the receiver a sender can run.
const STREAM_IN_FLIGHT_SEGMENTS: u32 = 32;

struct DirectedLink {
    model: LinkModel,
    /// The serialization horizon: when the link finishes transmitting
    /// everything scheduled so far. Shared by datagrams and stream segments
    /// — that sharing IS the contention being modeled.
    busy_until: Instant,
    datagrams: u64,
}

/// Reserve link time for `len` bytes; returns the delivery instant.
fn schedule(link: &Mutex<DirectedLink>, len: usize) -> Instant {
    let mut link = link.lock().expect("link lock");
    let now = Instant::now();
    let start = link.busy_until.max(now);
    let tx = Duration::from_secs_f64(len as f64 / link.model.bytes_per_sec as f64);
    link.busy_until = start + tx;
    link.busy_until + link.model.latency
}

struct PeerHandles {
    datagrams: mpsc::UnboundedSender<(PeerId, Vec<u8>)>,
    accepts: mpsc::UnboundedSender<(PeerId, SimStream)>,
}

struct NetInner {
    links: Mutex<HashMap<(PeerId, PeerId), Arc<Mutex<DirectedLink>>>>,
    peers: Mutex<HashMap<PeerId, PeerHandles>>,
}

/// The virtual network. Register peers with [`SimNet::endpoint`], wire them
/// with [`SimNet::set_link`]; no link = unreachable.
#[derive(Clone)]
pub struct SimNet {
    inner: Arc<NetInner>,
}

impl SimNet {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        SimNet {
            inner: Arc::new(NetInner {
                links: Mutex::new(HashMap::new()),
                peers: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Register a peer and hand back its transport endpoint. One endpoint
    /// per peer.
    pub fn endpoint(&self, peer: PeerId) -> SimEndpoint {
        let (datagram_tx, datagram_rx) = mpsc::unbounded_channel();
        let (accept_tx, accept_rx) = mpsc::unbounded_channel();
        self.inner.peers.lock().expect("peers lock").insert(
            peer,
            PeerHandles {
                datagrams: datagram_tx,
                accepts: accept_tx,
            },
        );
        SimEndpoint {
            peer,
            net: self.inner.clone(),
            datagram_rx: tokio::sync::Mutex::new(datagram_rx),
            accept_rx: tokio::sync::Mutex::new(accept_rx),
        }
    }

    /// Wire both directions between two peers with the same model.
    pub fn set_link(&self, a: PeerId, b: PeerId, model: LinkModel) {
        self.set_directed_link(a, b, model);
        self.set_directed_link(b, a, model);
    }

    pub fn set_directed_link(&self, from: PeerId, to: PeerId, model: LinkModel) {
        self.inner.links.lock().expect("links lock").insert(
            (from, to),
            Arc::new(Mutex::new(DirectedLink {
                model,
                busy_until: Instant::now(),
                datagrams: 0,
            })),
        );
    }
}

/// One peer's attachment to the [`SimNet`].
pub struct SimEndpoint {
    peer: PeerId,
    net: Arc<NetInner>,
    datagram_rx: tokio::sync::Mutex<mpsc::UnboundedReceiver<(PeerId, Vec<u8>)>>,
    accept_rx: tokio::sync::Mutex<mpsc::UnboundedReceiver<(PeerId, SimStream)>>,
}

impl SimEndpoint {
    fn link_to(&self, to: PeerId) -> Result<Arc<Mutex<DirectedLink>>, TransportError> {
        self.net
            .links
            .lock()
            .expect("links lock")
            .get(&(self.peer, to))
            .cloned()
            .ok_or(TransportError::Unreachable(to))
    }
}

impl DataPlaneTransport for SimEndpoint {
    type Stream = SimStream;

    fn max_datagram(&self) -> usize {
        crate::wire::MAX_DATAGRAM
    }

    async fn send_datagram(&self, to: PeerId, frame: Vec<u8>) -> Result<(), TransportError> {
        let link = self.link_to(to)?;
        let deliver_at = {
            let dropped = {
                let mut l = link.lock().expect("link lock");
                l.datagrams += 1;
                matches!(l.model.drop_every, Some(n) if n > 0 && l.datagrams % n as u64 == 0)
            };
            if dropped {
                // Lost in transit — fire-and-forget contract, sender sees Ok.
                return Ok(());
            }
            schedule(&link, frame.len())
        };
        let tx = self
            .net
            .peers
            .lock()
            .expect("peers lock")
            .get(&to)
            .map(|p| p.datagrams.clone())
            .ok_or(TransportError::Unreachable(to))?;
        let from = self.peer;
        tokio::spawn(async move {
            sleep_until(deliver_at).await;
            let _ = tx.send((from, frame));
        });
        Ok(())
    }

    async fn recv_datagram(&self) -> Result<(PeerId, Vec<u8>), TransportError> {
        self.datagram_rx
            .lock()
            .await
            .recv()
            .await
            .ok_or(TransportError::Closed)
    }

    async fn connect(&self, to: PeerId) -> Result<SimStream, TransportError> {
        let out_link = self.link_to(to)?;
        let back_link = self
            .net
            .links
            .lock()
            .expect("links lock")
            .get(&(to, self.peer))
            .cloned()
            .ok_or(TransportError::Unreachable(to))?;
        let accept_tx = self
            .net
            .peers
            .lock()
            .expect("peers lock")
            .get(&to)
            .map(|p| p.accepts.clone())
            .ok_or(TransportError::Unreachable(to))?;
        let (local, remote) = SimStream::pair(out_link, back_link);
        accept_tx
            .send((self.peer, remote))
            .map_err(|_| TransportError::Unreachable(to))?;
        Ok(local)
    }

    async fn accept(&self) -> Result<(PeerId, SimStream), TransportError> {
        self.accept_rx
            .lock()
            .await
            .recv()
            .await
            .ok_or(TransportError::Closed)
    }
}

// ---------------------------------------------------------------------------
// SimStream: a reliable in-order byte pipe whose bytes are serialized
// through the directed link model, one MTU-ish segment at a time.

struct WriteState {
    chunks: VecDeque<Vec<u8>>,
    buffered: usize,
    closed: bool,
    /// Reader side went away; writes fail loudly.
    broken: bool,
    write_waker: Option<Waker>,
    flush_waker: Option<Waker>,
}

struct WriteShared {
    state: Mutex<WriteState>,
    notify: Notify,
}

impl WriteShared {
    fn new() -> Arc<Self> {
        Arc::new(WriteShared {
            state: Mutex::new(WriteState {
                chunks: VecDeque::new(),
                buffered: 0,
                closed: false,
                broken: false,
                write_waker: None,
                flush_waker: None,
            }),
            notify: Notify::new(),
        })
    }

    fn mark_broken(&self) {
        let mut st = self.state.lock().expect("write state lock");
        st.broken = true;
        if let Some(w) = st.write_waker.take() {
            w.wake();
        }
        if let Some(w) = st.flush_waker.take() {
            w.wake();
        }
    }
}

/// Drain one direction: pop writer chunks, reserve link time, deliver after
/// the scheduled instant. The in-flight semaphore bounds
/// scheduled-but-undelivered segments (the window); the bounded delivery
/// channel is the receiver's buffer — a non-reading receiver stalls the
/// window and, transitively, the writer.
async fn pump(
    shared: Arc<WriteShared>,
    link: Arc<Mutex<DirectedLink>>,
    tx: mpsc::Sender<Vec<u8>>,
    window: Arc<Semaphore>,
) {
    enum Next {
        Chunk(Vec<u8>),
        Eof,
        Wait,
    }
    loop {
        let notified = shared.notify.notified();
        let next = {
            let mut st = shared.state.lock().expect("write state lock");
            match st.chunks.pop_front() {
                Some(c) => {
                    st.buffered -= c.len();
                    if let Some(w) = st.write_waker.take() {
                        w.wake();
                    }
                    if st.chunks.is_empty()
                        && let Some(w) = st.flush_waker.take()
                    {
                        w.wake();
                    }
                    Next::Chunk(c)
                }
                None if st.closed || st.broken => Next::Eof,
                None => Next::Wait,
            }
        };
        let chunk = match next {
            Next::Chunk(c) => c,
            Next::Eof => break,
            Next::Wait => {
                notified.await;
                continue;
            }
        };
        let permit = window
            .clone()
            .acquire_owned()
            .await
            .expect("window semaphore never closed");
        let deliver_at = schedule(&link, chunk.len());
        let tx = tx.clone();
        let shared = shared.clone();
        tokio::spawn(async move {
            sleep_until(deliver_at).await;
            if tx.send(chunk).await.is_err() {
                shared.mark_broken();
            }
            drop(permit);
        });
    }
    // EOF ordering: all in-flight deliveries hold window permits until their
    // send completes; acquiring the whole window means every byte is out.
    let _ = window.acquire_many_owned(STREAM_IN_FLIGHT_SEGMENTS).await;
    // tx drops here — the last sender clone signals EOF to the reader.
}

/// A reliable in-order byte stream through the sim network. Implements
/// `AsyncRead`/`AsyncWrite` like the real arm's TCP stream would.
pub struct SimStream {
    write: Arc<WriteShared>,
    read_rx: mpsc::Receiver<Vec<u8>>,
    read_partial: Option<Vec<u8>>,
    read_offset: usize,
}

impl SimStream {
    fn half(link: Arc<Mutex<DirectedLink>>) -> (Arc<WriteShared>, mpsc::Receiver<Vec<u8>>) {
        let shared = WriteShared::new();
        let (tx, rx) = mpsc::channel(STREAM_IN_FLIGHT_SEGMENTS as usize);
        let window = Arc::new(Semaphore::new(STREAM_IN_FLIGHT_SEGMENTS as usize));
        tokio::spawn(pump(shared.clone(), link, tx, window));
        (shared, rx)
    }

    fn pair(
        out_link: Arc<Mutex<DirectedLink>>,
        back_link: Arc<Mutex<DirectedLink>>,
    ) -> (Self, Self) {
        let (out_write, out_rx) = SimStream::half(out_link);
        let (back_write, back_rx) = SimStream::half(back_link);
        (
            SimStream {
                write: out_write,
                read_rx: back_rx,
                read_partial: None,
                read_offset: 0,
            },
            SimStream {
                write: back_write,
                read_rx: out_rx,
                read_partial: None,
                read_offset: 0,
            },
        )
    }
}

impl Drop for SimStream {
    fn drop(&mut self) {
        let mut st = self.write.state.lock().expect("write state lock");
        st.closed = true;
        drop(st);
        self.write.notify.notify_one();
    }
}

impl AsyncRead for SimStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        loop {
            if let Some(chunk) = self.read_partial.as_ref() {
                let offset = self.read_offset;
                let n = (chunk.len() - offset).min(buf.remaining());
                buf.put_slice(&chunk[offset..offset + n]);
                if offset + n == chunk.len() {
                    self.read_partial = None;
                    self.read_offset = 0;
                } else {
                    self.read_offset = offset + n;
                }
                return Poll::Ready(Ok(()));
            }
            match self.read_rx.poll_recv(cx) {
                Poll::Ready(Some(chunk)) => {
                    self.read_partial = Some(chunk);
                    self.read_offset = 0;
                }
                Poll::Ready(None) => return Poll::Ready(Ok(())), // EOF
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

impl AsyncWrite for SimStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let mut st = self.write.state.lock().expect("write state lock");
        if st.broken {
            return Poll::Ready(Err(io::Error::from(io::ErrorKind::BrokenPipe)));
        }
        if st.closed {
            return Poll::Ready(Err(io::Error::from(io::ErrorKind::NotConnected)));
        }
        if st.buffered >= STREAM_WRITE_BUFFER {
            st.write_waker = Some(cx.waker().clone());
            return Poll::Pending;
        }
        let n = buf
            .len()
            .min(STREAM_SEGMENT)
            .min(STREAM_WRITE_BUFFER - st.buffered);
        st.chunks.push_back(buf[..n].to_vec());
        st.buffered += n;
        drop(st);
        self.write.notify.notify_one();
        Poll::Ready(Ok(n))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let mut st = self.write.state.lock().expect("write state lock");
        if st.broken {
            return Poll::Ready(Err(io::Error::from(io::ErrorKind::BrokenPipe)));
        }
        if st.chunks.is_empty() {
            Poll::Ready(Ok(()))
        } else {
            st.flush_waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let mut st = self.write.state.lock().expect("write state lock");
        st.closed = true;
        drop(st);
        self.write.notify.notify_one();
        Poll::Ready(Ok(()))
    }
}
