//! the replication layer — a [`Node`] wraps a [`host::Host`] and gives it a
//! byte-oriented [`Transport`] seam so one host can run REPLICATED across peers.
//!
//! ## the two msg flows
//!
//! **outbound** (a locally-originated msg): [`Node::apply_local`] submits the msg
//! to the local host first (so our own view advances immediately — the "echo"),
//! then propagates the msg's bytes to peers over its [`Transport`]. this is the
//! ONLY path that ever touches the wire.
//!
//! **inbound** (a msg that arrived from a peer): [`Node::poll_inbound`] drains
//! the transport's inbound queue, decodes each batch, and submits every msg to
//! the local host — and NEVER re-propagates. that wire-level asymmetry (outbound
//! propagates, inbound does not) IS the local-only rule: it is what keeps a
//! two-node loop from ping-ponging a msg back and forth forever.
//!
//! ## why the node's re-entry rule is only wire-level
//!
//! [`host::Host::submit`] already runs the intra-block follow-up drain: a module
//! that emits a [`Msg`] via `ctx.emit_msg` has it re-dispatched as a LOCAL-ONLY
//! follow-up op (`Origin::Module`), capped at `host::MAX_DISPATCHES`, never
//! surfaced for broadcast. so module-level re-entry is already contained inside
//! one block. the node only has to enforce the rule at the network boundary:
//! ops that came off the wire are applied, not rebroadcast.
//!
//! ## pull-based, single-owner
//!
//! unlike the legacy background-task node, inbound is PULL-based
//! ([`Node::poll_inbound`]) rather than a spawned reader loop. that lets the
//! node OWN its `Host` directly (no `Arc<Mutex>`), keeps the convergence test
//! deterministic (no interval / notify race to wait on), and keeps this crate
//! runtime-agnostic — it spawns nothing and depends on no async runtime. the
//! real commonware transport (a later slice) will add its own inbound plumbing
//! behind the same [`Transport`] seam.

use std::sync::{Arc, Mutex};
use std::sync::mpsc;

use serde::{Deserialize, Serialize};

use host::{BlockOutcome, Host};
use sdk::{Msg, StateRoot};

/// the bytes delivered on the inbound channel: a serialized msg-batch.
pub type Inbound = Vec<u8>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("wire decode failed: {0}")]
    Decode(#[from] serde_json::Error),
    #[error("host error: {0}")]
    Host(#[from] sdk::Error),
}

// ============================================================================
// wire codec — encode Msg in THIS crate (encode-in-node), not on sdk::Msg.
// ============================================================================
//
// sdk deliberately carries no serde dep ("async-trait is the one exception"), so
// the wire concern lives here rather than deriving Serialize on `Msg`. `WireMsg`
// is a private serde mirror of the two public `Msg` fields; a batch is a plain
// `Vec<WireMsg>` over serde_json. only the app-hash has to match across nodes,
// not the wire bytes, so a json envelope is free to evolve independently.

#[derive(Serialize, Deserialize)]
struct WireMsg {
    target: String,
    payload: Vec<u8>,
}

impl From<&Msg> for WireMsg {
    fn from(m: &Msg) -> Self {
        WireMsg { target: m.target.clone(), payload: m.payload.clone() }
    }
}

impl From<WireMsg> for Msg {
    fn from(w: WireMsg) -> Self {
        Msg { target: w.target, payload: w.payload }
    }
}

/// serialize a msg-batch to bytes. infallible — the fields are plain data.
pub fn encode_batch(msgs: &[Msg]) -> Vec<u8> {
    let wire: Vec<WireMsg> = msgs.iter().map(WireMsg::from).collect();
    serde_json::to_vec(&wire).expect("msg batch serializes")
}

/// deserialize a msg-batch from bytes.
pub fn decode_batch(bytes: &[u8]) -> Result<Vec<Msg>, Error> {
    let wire: Vec<WireMsg> = serde_json::from_slice(bytes)?;
    Ok(wire.into_iter().map(Msg::from).collect())
}

// ============================================================================
// the transport seam + the in-process loopback impl.
// ============================================================================

/// byte-oriented transport seam: send an already-serialized msg-batch to peers.
///
/// `send` is async so the seam fits an over-the-wire impl (the later commonware
/// p2p transport) without changing shape; the loopback impl's body is a plain
/// synchronous push into each peer's queue wrapped in an `async move`, so it
/// never actually suspends. the inbound side is NOT on the trait: each transport
/// hands back its receiver at construction (see [`LoopbackHub::node`]), which
/// sidesteps the object-safety question and lets a caller hold the concrete
/// receiver type. (the trait is used behind a generic `T`, never `dyn`, so the
/// return-position `impl Future` is fine.)
pub trait Transport {
    /// send a serialized msg-batch out to every peer (not back to self).
    fn send(&self, bytes: Vec<u8>) -> impl std::future::Future<Output = Result<(), Error>>;
}

/// mints N connected in-memory transports. when one node sends, every OTHER
/// node's inbound receiver gets the bytes — the sender does not.
#[derive(Clone, Default)]
pub struct LoopbackHub {
    peers: Arc<Mutex<Vec<mpsc::Sender<Inbound>>>>,
}

impl LoopbackHub {
    pub fn new() -> Self {
        Self::default()
    }

    /// register a new node. returns its transport handle and inbound receiver.
    pub fn node(&self) -> (LoopbackTransport, mpsc::Receiver<Inbound>) {
        let (tx, rx) = mpsc::channel();
        let id = {
            let mut peers = self.peers.lock().expect("hub lock poisoned");
            peers.push(tx);
            peers.len() - 1
        };
        (LoopbackTransport { id, peers: self.peers.clone() }, rx)
    }
}

/// a single node's handle onto the [`LoopbackHub`]. `Clone` so it can be shared;
/// `id` is the sender's own index, skipped on fan-out so it never receives its
/// own sends.
#[derive(Clone)]
pub struct LoopbackTransport {
    id: usize,
    peers: Arc<Mutex<Vec<mpsc::Sender<Inbound>>>>,
}

impl Transport for LoopbackTransport {
    fn send(&self, bytes: Vec<u8>) -> impl std::future::Future<Output = Result<(), Error>> {
        // capture cloned handles so the returned future owns its data (no borrow
        // of `self` escapes). the body never awaits — loopback delivery is a
        // synchronous fan-out — it just satisfies the async seam.
        let peers = self.peers.clone();
        let id = self.id;
        async move {
            let peers = peers.lock().expect("hub lock poisoned");
            for (i, tx) in peers.iter().enumerate() {
                if i == id {
                    continue; // never deliver a node its own send.
                }
                // best-effort gossip: a gone peer must not fail the whole send.
                let _ = tx.send(bytes.clone());
            }
            Ok(())
        }
    }
}

// ============================================================================
// the node — a replicated wrapper over host::Host.
// ============================================================================

/// a replicated host. owns its [`Host`], a [`Transport`] handle, and the inbound
/// receiver the transport handed back at construction. generic over the concrete
/// transport `T` (no `dyn`), so the same type serves loopback today and the
/// commonware transport later.
pub struct Node<T: Transport> {
    host: Host,
    transport: T,
    inbound: mpsc::Receiver<Inbound>,
}

impl<T: Transport> Node<T> {
    /// wrap `host` with a `transport` handle and that transport's `inbound`
    /// receiver.
    pub fn new(host: Host, transport: T, inbound: mpsc::Receiver<Inbound>) -> Self {
        Self { host, transport, inbound }
    }

    /// OUTBOUND — a locally-originated msg. apply it to the local host first (the
    /// echo: our view advances without waiting on a round-trip), then propagate
    /// the msg's bytes to peers. this is the ONLY path that propagates. returns
    /// the local [`BlockOutcome`] so the caller sees the resulting app-hash.
    ///
    /// `Msg` is `Clone`, so — unlike the legacy `!Clone` op — we simply clone for
    /// the wire and submit the original; no encode-first dance, no re-decode.
    pub async fn apply_local(&mut self, msg: Msg) -> Result<BlockOutcome, Error> {
        let bytes = encode_batch(std::slice::from_ref(&msg));
        let outcome = self.host.submit(msg).await?;
        // propagate AFTER the local apply so a slow peer never stalls our block.
        let _ = self.transport.send(bytes).await;
        Ok(outcome)
    }

    /// INBOUND — drain every msg-batch the transport delivered and submit each to
    /// the local host. NEVER re-propagates: that asymmetry vs [`apply_local`] is
    /// the local-only rule. returns the count of msgs applied (0 when idle), so a
    /// test can await convergence deterministically without a wall-clock sleep.
    ///
    /// the inbound queue is drained into an owned `Vec` up front so no channel
    /// borrow is held across the `host.submit(..).await`.
    pub async fn poll_inbound(&mut self) -> Result<usize, Error> {
        let batches: Vec<Inbound> = std::iter::from_fn(|| self.inbound.try_recv().ok()).collect();
        let mut applied = 0usize;
        for bytes in batches {
            for msg in decode_batch(&bytes)? {
                self.host.submit(msg).await?;
                applied += 1;
            }
        }
        Ok(applied)
    }

    /// the current app-hash of the wrapped host.
    pub fn app_hash(&self) -> StateRoot {
        self.host.app_hash()
    }

    /// borrow the wrapped host (queries, module_root inspection, ...).
    pub fn host(&self) -> &Host {
        &self.host
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_other_nodes_receive_sender_does_not() {
        let hub = LoopbackHub::new();
        let (node0, node0_rx) = hub.node();
        let (_node1, node1_rx) = hub.node();

        futures::executor::block_on(node0.send(b"hi".to_vec())).expect("send ok");

        // node1 receives it.
        assert_eq!(node1_rx.recv().expect("node1 got msg"), b"hi");
        // node0 (the sender) does not.
        assert!(matches!(node0_rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
    }

    #[test]
    fn wire_roundtrip_preserves_target_and_payload() {
        let msgs = vec![
            Msg { target: "directory".into(), payload: b"hello".to_vec() },
            Msg { target: "kv".into(), payload: vec![] },
        ];
        let decoded = decode_batch(&encode_batch(&msgs)).expect("roundtrips");
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].target, "directory");
        assert_eq!(decoded[0].payload, b"hello");
        assert_eq!(decoded[1].target, "kv");
        assert!(decoded[1].payload.is_empty());
    }
}
