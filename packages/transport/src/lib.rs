//! byte-oriented transport seam.
//!
//! this mirrors how commonware actually ships data: raw bytes on the gossip
//! lane, opaque digests on consensus. the same `Transport` trait will later be
//! satisfied by a commonware impl AND by the loopback impl here, validated by
//! the same convergence test.
//!
//! shape:
//! - the trait is minimal: just `send`. it sends already-serialized bytes out
//!   on a [`Lane`] to peers.
//! - the inbound side is NOT on the trait. each transport hands back its
//!   receiver at construction (see [`LoopbackHub::node`]) — this sidesteps the
//!   object-safety question (the `-> impl Future` on `send` already makes the
//!   trait non-dyn-compatible, which is fine: callers hold the concrete type).
//! - encode/decode are free fns over `op::Op` batches via serde_json. `Lane`
//!   itself never gets serialized — it rides the in-memory channel as a tuple
//!   field; only the op bytes go over the wire.

use std::sync::{Arc, Mutex};

use op::Lane;
use tokio::sync::mpsc;

/// inbound channel item: which lane the bytes arrived on, and the bytes.
pub type Inbound = (Lane, Vec<u8>);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("decode failed: {0}")]
    Decode(#[from] serde_json::Error),
}

pub trait Transport: Send + Sync {
    /// send a serialized op-batch out on a lane to peers.
    fn send(
        &self,
        lane: Lane,
        bytes: Vec<u8>,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send;
}

/// serialize an op-batch to bytes. infallible: ops are plain serde types.
pub fn encode_batch(ops: &[op::Op]) -> Vec<u8> {
    serde_json::to_vec(ops).expect("op batch serializes")
}

/// deserialize an op-batch from bytes.
pub fn decode_batch(bytes: &[u8]) -> Result<Vec<op::Op>, Error> {
    Ok(serde_json::from_slice(bytes)?)
}

/// mints N connected in-memory transports. when one node sends on a lane, every
/// OTHER node's inbound receiver gets `(lane, bytes)` — the sender does not.
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
        let (tx, rx) = mpsc::channel(64);
        let id = {
            let mut peers = self.peers.lock().expect("hub lock poisoned");
            peers.push(tx);
            peers.len() - 1
        };
        (
            LoopbackTransport {
                id,
                peers: self.peers.clone(),
            },
            rx,
        )
    }
}

#[derive(Clone)]
pub struct LoopbackTransport {
    id: usize,
    peers: Arc<Mutex<Vec<mpsc::Sender<Inbound>>>>,
}

impl Transport for LoopbackTransport {
    fn send(
        &self,
        lane: Lane,
        bytes: Vec<u8>,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send {
        // snapshot every other peer's sender BEFORE the async block so we never
        // hold the mutex guard across an .await (which would break `+ Send`).
        let targets: Vec<mpsc::Sender<Inbound>> = {
            let peers = self.peers.lock().expect("hub lock poisoned");
            peers
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != self.id)
                .map(|(_, tx)| tx.clone())
                .collect()
        };
        async move {
            // best-effort gossip: a gone peer shouldn't fail the whole send.
            for tx in targets {
                let _ = tx.send((lane, bytes.clone())).await;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn other_nodes_receive_sender_does_not() {
        let hub = LoopbackHub::new();
        let (node0, mut node0_rx) = hub.node();
        let (_node1, mut node1_rx) = hub.node();

        node0
            .send(Lane::Broadcast, b"hi".to_vec())
            .await
            .expect("send ok");

        // node1 receives it.
        let (lane, bytes) = node1_rx.recv().await.expect("node1 got msg");
        assert_eq!(lane, Lane::Broadcast);
        assert_eq!(bytes, b"hi");

        // node0 (the sender) does not — try_recv is empty (recv would block).
        assert!(matches!(
            node0_rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }
}
