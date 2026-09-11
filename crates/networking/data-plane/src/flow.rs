//! Flow identity and per-flow policy.
//!
//! A *flow* is the isolation unit inside a service: one voice channel, one
//! state-sync session. Both ends derive the same [`FlowId`] from replicated
//! state (a channel id, a snapshot digest), so flows need no allocation
//! handshake — agreement on the domain bytes IS the agreement on the flow.

use sha2::{Digest, Sha256};

/// A flow id inside one service. Derived, never allocated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FlowId(u64);

impl FlowId {
    /// Derive a flow id from a domain identifier (e.g. a chat-module voice
    /// channel id, a snapshot digest): the first 8 bytes of sha256, big
    /// endian. Collisions across *different* services are harmless — flows
    /// are always keyed `(Service, FlowId)`.
    pub fn derive(domain: &[u8]) -> Self {
        let digest = Sha256::digest(domain);
        let mut first = [0u8; 8];
        first.copy_from_slice(&digest[..8]);
        FlowId(u64::from_be_bytes(first))
    }

    /// A raw id, for wire decoding and tests.
    pub const fn from_raw(raw: u64) -> Self {
        FlowId(raw)
    }

    pub const fn as_u64(&self) -> u64 {
        self.0
    }
}

/// Policy for one datagram flow. The queue bound is a receive-side budget
/// PER SENDING PEER: overflow drops that sender's OLDEST queued datagram
/// (late real-time data is dead data), never a quieter peer's and never
/// another flow's — one hot sender cannot spill into either.
#[derive(Clone, Copy, Debug)]
pub struct DatagramPolicy {
    /// Max datagrams queued per sender before drop-oldest kicks in. The
    /// flow's ceiling is this times the peers admission lets in.
    pub max_queued: usize,
}

/// Policy for one stream service (all flows of one consumer).
#[derive(Clone, Copy, Debug)]
pub struct StreamPolicy {
    /// Max accepted-but-not-yet-claimed inbound streams. When full, further
    /// inbound streams are refused (closed without hello-ack) — the opener
    /// sees a refused open, never a silent queue.
    pub accept_backlog: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_is_stable_and_input_sensitive() {
        let a = FlowId::derive(b"voice-channel:general");
        let b = FlowId::derive(b"voice-channel:general");
        let c = FlowId::derive(b"voice-channel:standup");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
