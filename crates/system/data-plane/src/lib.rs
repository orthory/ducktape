//! The data plane: off-consensus byte transport between nodes, designed to
//! ride the reachability plane's WireGuard overlay (`dt-*` interface,
//! fd::/48 ULA, per-peer /128 AllowedIPs).
//!
//! Wire surface (this crate root): [`Service`] ids, the datagram header and
//! stream hello frames in [`wire`], and [`flow::FlowId`] derivation. Peers
//! must agree on all three; everything else is node-local policy.
//!
//! Boundary — what this plane is and is not:
//! - It carries **opaque bytes off-consensus**. Nothing here is BFT-ordered,
//!   nothing lands in replicated state. Durable sent/received facts, when a
//!   consumer needs them, are ordinary module ops on the consensus lane —
//!   outside this crate.
//! - **Admission derives from consensus.** A flow is admissible only if the
//!   injected [`plane::AdmissionPolicy`] — a node-layer view over finalized
//!   module state (channel membership, valset, ...) — permits the
//!   `(peer, service, flow)` triple. Default-deny: unadmitted traffic is
//!   dropped at demux, counted, and attributed to its sender; it never
//!   reaches a consumer queue. There is no raw send surface either — the
//!   only way to emit traffic is through a flow handle, and every send is
//!   admission-checked, so a correct node cannot unknowingly send rogue
//!   traffic.
//! - **Identity is the transport's.** On the real overlay, WireGuard
//!   cryptokey routing binds a packet's source /128 to exactly one peer, so
//!   [`transport::PeerId`] arrives authenticated; this crate adds no session
//!   crypto and no handshake beyond the one-frame stream hello.
//!
//! Two service classes, two APIs, never unified:
//! - **Datagram class** — unreliable, unordered, latency-first (voice).
//!   Per-flow bounded queues, drop-oldest: late real-time data is dead data.
//! - **Stream class** — reliable, backpressured, throughput-with-headroom
//!   (state sync, blob fetch). Every stream's writes draw from one global
//!   bulk token bucket so bulk self-limits below the link and real-time
//!   traffic never queues behind it.

pub mod flow;
pub mod plane;
#[cfg(feature = "sim")]
pub mod sim;
pub mod transport;
pub mod wire;

pub use flow::{DatagramPolicy, FlowId, StreamPolicy};
pub use plane::{
    AdmissionPolicy, DataPlane, DatagramFlow, OpenError, PlaneConfig, RegisterError, SendError,
    StatsSnapshot, StreamService,
};
pub use transport::{DataPlaneTransport, PeerId, TransportError};
pub use wire::{Hello, MAX_DATAGRAM, MAX_DATAGRAM_PAYLOAD};

/// The compile-time service registry: every data-plane consumer claims one
/// id here. Wire-stable — never renumber, only append.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Service {
    /// Kernel state sync: snapshot/chunk pulls off the consensus mesh.
    StateSync = 1,
    /// Real-time voice channels (chat module).
    Voice = 2,
}

impl TryFrom<u8> for Service {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, u8> {
        match value {
            1 => Ok(Service::StateSync),
            2 => Ok(Service::Voice),
            other => Err(other),
        }
    }
}
