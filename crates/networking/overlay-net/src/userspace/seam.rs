//! the commonware-runtime face of the virtual sockets — the seam
//! wiring: what [`crate::OverlayContext`] routes overlay dials and binds to
//! when the backend is userspace, as the `Virtual` arm of
//! [`crate::OverlayListener`]/[`crate::OverlaySink`]/[`crate::OverlayStream`].
//!
//! `bind`/`dial` mirror the private `crate::tun` helpers: one function per
//! `Network` verb, resolving the live stack from the [`StackSlot`] PER CALL
//! (a rebuilt backend serves the next connection with no consumer rewiring;
//! an empty slot is the tunnel being down, surfaced as the same bind/dial
//! failure a downed TUN interface yields).
//!
//! the adapter types deliberately mirror commonware's own tokio arm, because
//! consumers above the seam were written against its semantics: sends and
//! recvs carry the same [`IO_TIMEOUT`] the node boots the OS arm with, a
//! cancelled (dropped mid-flight) send or recv poisons the half permanently,
//! and `peek` answers from a `BufReader`'s already-buffered bytes without
//! performing I/O.

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use commonware_runtime::{Error, IoBufs};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _, BufReader, ReadHalf, WriteHalf};
use tokio::time::timeout;

use super::sockets::{VirtualTcpListener, VirtualTcpStream};
use super::stack::{LISTEN_BACKLOG, StackSlot, accept_via_slot};

/// the per-read/write deadline on every mesh socket, virtual and OS arm
/// alike (the node boots commonware's tokio runtime with this same value —
/// `constants::MESH_IO_TIMEOUT` aliases this const so the two arms cannot
/// drift). this deadline is the ONLY detector for a half-open connection: a
/// laptop that slept, roamed Wi-Fi, or lost its NAT mapping keeps a
/// silently-dead socket until a read times out, and with 1s blocks every
/// such event freezes block delivery for the full window. commonware's
/// default is 60s; the discovery mesh pings every connection each 5s (the
/// `local` preset's bit-vec gossip), so 15s = three missed keepalives —
/// ample against jitter, 4x faster to heal. the same deadline bounds writes:
/// a max-size 2 MiB message must move at ~1.1 Mbps or the connection is
/// treated as dead.
pub const IO_TIMEOUT: Duration = Duration::from_secs(15);

/// match the tokio arm's read buffer (64 KiB): `peek` can only see what the
/// buffer holds, so capacity is part of the peek contract, not just a perf
/// knob.
const READ_BUFFER: usize = 64 * 1024;

/// carry an overlay bind on the virtual stack: a smoltcp listener at the
/// node's own `/128`. binds on any other address are refused — the virtual
/// host owns exactly one address, so anything else is a routing bug upstream.
pub(crate) async fn bind(slot: &StackSlot, socket: SocketAddr) -> Result<VirtualListener, Error> {
    let stack = slot.get().ok_or(Error::BindFailed)?;
    if socket.ip() != IpAddr::V6(stack.local_ip()) {
        return Err(Error::BindFailed);
    }
    let listener = stack
        .listen_tcp(socket.port(), LISTEN_BACKLOG)
        .map_err(|_| Error::BindFailed)?;
    Ok(VirtualListener(listener))
}

/// carry an overlay dial on the virtual stack: a smoltcp connect through the
/// tunnel, split into the seam's sink/stream halves.
pub(crate) async fn dial(
    slot: &StackSlot,
    socket: SocketAddr,
) -> Result<(VirtualSink, VirtualStream), Error> {
    let stack = slot.get().ok_or(Error::ConnectionFailed)?;
    let stream = stack
        .connect_tcp(socket)
        .await
        .map_err(|_| Error::ConnectionFailed)?;
    Ok(split(stream))
}

/// split an established virtual connection into the seam's halves.
pub(crate) fn split(stream: VirtualTcpStream) -> (VirtualSink, VirtualStream) {
    let (read, write) = tokio::io::split(stream);
    (
        VirtualSink {
            half: write,
            state: SinkState::Open,
        },
        VirtualStream {
            half: BufReader::with_capacity(READ_BUFFER, read),
            poisoned: false,
        },
    )
}

/// the virtual half of socket mode's mesh listener: a lazy
/// TCP acceptor at the node's own ULA on a fixed port.
///
/// the mesh binds its listener once, at node start, on the UNSPECIFIED
/// address — but in socket mode a tunnel-carried inbound connection
/// terminates in the virtual stack, which the OS listener can never see, so
/// the seam's bind adds this leg alongside it ([`crate::OverlayListener`]'s
/// `Dual` arm). "lazy" because the stack does not exist until the first
/// `apply` (and is replaced on interface rebuilds): the leg (re)binds
/// whenever the [`StackSlot`] serves a stack it is not yet listening on,
/// and simply pends while the tunnel is down.
pub struct LazyVirtualListener {
    slot: StackSlot,
    port: u16,
    /// the live leg, tagged with the stack it bound on so a rebuild
    /// (different `Arc`) is detected and re-bound.
    leg: Option<(
        std::sync::Arc<super::stack::VirtualStack>,
        VirtualTcpListener,
    )>,
}

impl LazyVirtualListener {
    pub fn new(slot: StackSlot, port: u16) -> Self {
        Self {
            slot,
            port,
            leg: None,
        }
    }

    /// accept the next tunnel-carried connection. never fails permanently:
    /// a down tunnel or a mid-rebuild window is time, not an error — exactly
    /// the OS listener's posture toward a link being down.
    pub async fn accept(&mut self) -> (SocketAddr, VirtualSink, VirtualStream) {
        let (stream, addr) = accept_via_slot(&self.slot, None, self.port, &mut self.leg)
            .await
            .expect("no required ip: the shared accept loop cannot fail");
        let (sink, stream) = split(stream);
        (addr, sink, stream)
    }
}

/// a virtual TCP acceptor under the seam's `Listener` contract.
pub struct VirtualListener(VirtualTcpListener);

impl VirtualListener {
    pub(crate) async fn accept(
        &mut self,
    ) -> Result<(SocketAddr, VirtualSink, VirtualStream), Error> {
        let (stream, addr) = self.0.accept().await.map_err(|_| Error::Closed)?;
        let (sink, stream) = split(stream);
        Ok((addr, sink, stream))
    }

    pub(crate) fn local_addr(&self) -> Result<SocketAddr, std::io::Error> {
        Ok(self.0.local_addr())
    }
}

/// lifecycle state for the write half — the tokio arm's cancellation
/// discipline: a send future dropped mid-write leaves the half unusable.
enum SinkState {
    Open,
    Sending,
    Closed,
}

/// the write half of a virtual connection under the seam's `Sink` contract.
pub struct VirtualSink {
    half: WriteHalf<VirtualTcpStream>,
    state: SinkState,
}

impl VirtualSink {
    async fn close(&mut self) {
        if matches!(self.state, SinkState::Closed) {
            return;
        }
        let _ = self.half.shutdown().await;
        self.state = SinkState::Closed;
    }

    pub async fn send(&mut self, bufs: impl Into<IoBufs> + Send) -> Result<(), Error> {
        match self.state {
            SinkState::Open => {}
            SinkState::Sending => {
                self.close().await;
                return Err(Error::Closed);
            }
            SinkState::Closed => return Err(Error::Closed),
        }
        // mark as sending before awaiting so a cancelled send is detected by
        // the next call (the tokio arm's exact discipline).
        self.state = SinkState::Sending;

        let bufs = bufs.into();
        let send = async {
            match bufs.try_into_single() {
                Ok(buf) => self
                    .half
                    .write_all(buf.as_ref())
                    .await
                    .map_err(|_| Error::SendFailed),
                Err(mut bufs) => self
                    .half
                    .write_all_buf(&mut bufs)
                    .await
                    .map_err(|_| Error::SendFailed),
            }
        };
        let result = timeout(IO_TIMEOUT, send)
            .await
            .unwrap_or(Err(Error::Timeout));

        if result.is_err() {
            self.close().await;
            return result;
        }
        self.state = SinkState::Open;
        Ok(())
    }
}

/// the read half of a virtual connection under the seam's `Stream` contract.
pub struct VirtualStream {
    half: BufReader<ReadHalf<VirtualTcpStream>>,
    poisoned: bool,
}

impl VirtualStream {
    pub async fn recv(&mut self, len: usize) -> Result<IoBufs, Error> {
        if self.poisoned {
            return Err(Error::Closed);
        }
        // pre-poison so cancellation leaves the stream permanently closed
        // rather than silently missing bytes (the tokio arm's discipline).
        self.poisoned = true;

        let recv = async {
            let mut buf = vec![0u8; len];
            self.half
                .read_exact(&mut buf)
                .await
                .map_err(|_| Error::RecvFailed)?;
            Ok(IoBufs::from(buf))
        };
        let result = timeout(IO_TIMEOUT, recv)
            .await
            .unwrap_or(Err(Error::Timeout));

        if result.is_ok() {
            self.poisoned = false;
        }
        result
    }

    pub(crate) fn peek(&self, max_len: usize) -> &[u8] {
        let buffered = self.half.buffer();
        &buffered[..buffered.len().min(max_len)]
    }
}
