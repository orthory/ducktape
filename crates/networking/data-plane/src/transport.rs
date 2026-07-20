//! The transport seam: what the plane needs from the medium under it.
//!
//! The real arm binds these operations to sockets on the WireGuard overlay
//! (datagrams = UDP on the node's fd:: address, streams = TCP); the `sim`
//! arm is a deterministic in-memory network. All isolation logic — demux,
//! queues, pacing, admission — lives ABOVE this trait, so every interesting
//! property is provable against the sim arm.

use std::future::Future;

use tokio::io::{AsyncRead, AsyncWrite};

/// A peer's transport identity: the 32 raw bytes of its ed25519 public key.
/// On the real overlay this arrives authenticated — WireGuard cryptokey
/// routing binds the source /128 to exactly one peer — so the transport can
/// assert it without any plane-level handshake. Kept as raw bytes so this
/// crate needs no cryptography dependency; the node layer converts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PeerId(pub [u8; 32]);

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    /// No path to the peer (no tunnel / no link). The plane does not retry;
    /// reachability owns bringing paths up.
    #[error("peer unreachable")]
    Unreachable(PeerId),
    /// The transport is shut down.
    #[error("transport closed")]
    Closed,
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// The medium the plane runs over. Implementations own the PeerId ↔ address
/// mapping; the plane never sees addresses.
///
/// Contract: `recv_datagram` and `accept` each have exactly ONE caller — the
/// plane's demux and acceptor loops.
pub trait DataPlaneTransport: Send + Sync + 'static {
    type Stream: AsyncRead + AsyncWrite + Unpin + Send + 'static;

    /// Fire-and-forget: queue one frame toward a peer. May be silently lost
    /// in transit; must not block on the receiver.
    fn send_datagram(
        &self,
        to: PeerId,
        frame: Vec<u8>,
    ) -> impl Future<Output = Result<(), TransportError>> + Send;

    /// Next inbound datagram, with its authenticated sender.
    fn recv_datagram(
        &self,
    ) -> impl Future<Output = Result<(PeerId, Vec<u8>), TransportError>> + Send;

    /// Open a reliable byte stream to a peer.
    fn connect(
        &self,
        to: PeerId,
    ) -> impl Future<Output = Result<Self::Stream, TransportError>> + Send;

    /// Next inbound stream, with its authenticated opener.
    fn accept(&self)
    -> impl Future<Output = Result<(PeerId, Self::Stream), TransportError>> + Send;
}
