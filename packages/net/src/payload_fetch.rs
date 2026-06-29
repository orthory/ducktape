//! catch-up fetch for finalized consensus payloads missing from the local store.
//!
//! the eager path ([`spawn_payload_drain`](crate::spawn_payload_drain)) has the
//! leader's relay gossip a proposed batch's bytes so every peer caches them
//! store-only ahead of finalization. but a validator that finalizes a digest it
//! never saw broadcast — a late join that recovered the finalization certificate
//! through backfill, or one that simply missed the relay gossip — has the digest
//! committed yet no bytes to resolve it. before this module its reporter found
//! `store.get -> None` and SILENTLY DROPPED the batch.
//!
//! this is the miss-path backstop: a [`commonware_resolver`] p2p engine that
//!
//! - [`PayloadProducer`] SERVES payload bytes by digest from the local
//!   [`ContentStore`] to peers fetching them (a local miss returns an unresolved
//!   receiver so the resolver retries another peer), and
//! - [`PayloadConsumer`] RECEIVES fetched bytes, re-hashes them to verify the
//!   content address (`sha256(value) == requested digest`), stores them, and
//!   delivers through the same [`deliver_payload`] seam the reporter uses. a hash
//!   mismatch resolves `false`, which blocks the lying peer.
//!
//! content-addressing IS the verification: byzantine garbage hashes to something
//! other than the digest we asked for, so it can never be accepted under a
//! finalized digest — no signature check needed.

use bytes::Bytes;
use commonware_resolver::p2p::Producer;
use commonware_resolver::{Consumer, Delivery};
use commonware_utils::channel::oneshot;
use op::Lane;
use tokio::sync::mpsc;
use transport::Inbound;

use crate::consensus::{deliver_payload, digest_of, ContentStore, Digest};

/// serves payload bytes by digest from the local [`ContentStore`] to peers that
/// fetch them over the resolver. clone shares the backing store (`Arc`).
#[derive(Clone)]
pub struct PayloadProducer {
    store: ContentStore,
}

impl PayloadProducer {
    /// `store` MUST be this validator's shared [`ContentStore`] — the one its
    /// reporter resolves finalized digests against and its payload drain caches
    /// relay broadcasts into. then a peer fetching a digest gets the very bytes
    /// this node finalized.
    pub fn new(store: ContentStore) -> Self {
        Self { store }
    }
}

impl Producer for PayloadProducer {
    type Key = Digest;

    fn produce(&mut self, key: Self::Key) -> oneshot::Receiver<Bytes> {
        let (tx, rx) = oneshot::channel();
        // serve from the store if we hold the bytes; otherwise drop `tx` UNSENT.
        // the resolver reads a dropped producer sender as "no data" and replies
        // to the requester with an error so it retries another peer — never a
        // wrong or empty payload under the digest.
        if let Some(bytes) = self.store.get(&key) {
            let _ = tx.send(Bytes::from(bytes));
        }
        rx
    }
}

/// receives fetched payload bytes, verifies the content address, stores, and
/// delivers them onto the consensus inbound. clone shares the store + inbound.
#[derive(Clone)]
pub struct PayloadConsumer {
    store: ContentStore,
    inbound: mpsc::Sender<Inbound>,
}

impl PayloadConsumer {
    /// `store` MUST be this validator's shared [`ContentStore`] (the reporter's),
    /// so a fetched payload that lands here resolves the digest the reporter is
    /// waiting on; `inbound` MUST be the same inbound mpsc the reporter forwards
    /// onto, so a catch-up delivery is indistinguishable from an eager one.
    pub fn new(store: ContentStore, inbound: mpsc::Sender<Inbound>) -> Self {
        Self { store, inbound }
    }
}

impl Consumer for PayloadConsumer {
    type Key = Digest;
    type Value = Bytes;
    type Subscriber = ();

    fn deliver(
        &mut self,
        delivery: Delivery<Self::Key, Self::Subscriber>,
        value: Self::Value,
    ) -> oneshot::Receiver<bool> {
        let (tx, rx) = oneshot::channel();
        let bytes: Vec<u8> = value.to_vec();
        if digest_of(&bytes) == delivery.key {
            // content address verified: the fetched bytes hash to the digest we
            // asked for. cache them so the store resolves this (and any future)
            // finalization of the digest, then forward.
            let digest = self.store.put(bytes);
            // ORDERED-DELIVERY CURSOR (future): a round/view-ordered consensus
            // would interpose HERE — between verify+store and the inbound forward
            // — to release a caught-up payload only in its finalization slot.
            // today there is no cursor: forwarding immediately is SAFE (these are
            // post-finalization, BFT-committed bytes), though a catch-up delivery
            // can land out of finalization order relative to eager deliveries —
            // imposing that order is exactly what this cursor would add. no round
            // or view state lives on this consumer yet.
            deliver_payload(&self.store, &self.inbound, digest);
            let _ = tx.send(true);
        } else {
            // the bytes do NOT hash to the requested digest — a lying peer.
            // resolving `false` tells the resolver to block it and retry another.
            let _ = tx.send(false);
        }
        rx
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use commonware_runtime::{deterministic, Runner};
    use commonware_utils::vec::NonEmptyVec;
    use std::time::Duration;

    /// the load-bearing safety property of the consumer in isolation: a peer that
    /// returns bytes that do NOT hash to the requested digest is REJECTED. the
    /// fetch resolves `false` (which blocks the peer), the store is left
    /// untouched, and nothing is forwarded onto the inbound. content-addressing
    /// is the whole verification, so this is the seam that keeps a byzantine peer
    /// from injecting garbage under a finalized digest.
    #[test]
    fn consumer_rejects_payload_whose_hash_mismatches_the_fetched_key() {
        let executor = deterministic::Runner::timed(Duration::from_secs(5));
        executor.start(|_context| async move {
            let store = ContentStore::new();
            let (in_tx, mut in_rx) = mpsc::channel::<Inbound>(8);
            let mut consumer = PayloadConsumer::new(store.clone(), in_tx);

            // we fetched the digest of the REAL batch, but the peer hands back
            // unrelated bytes that hash to something else.
            let key = digest_of(b"the real finalized batch");
            let tampered = Bytes::from_static(b"byzantine garbage");
            let delivery = Delivery {
                key,
                subscribers: NonEmptyVec::new(()),
            };

            let valid = consumer
                .deliver(delivery, tampered)
                .await
                .expect("consumer resolves a verdict");
            assert!(!valid, "a hash mismatch must resolve false (blocks the peer)");

            // nothing was stored under the requested digest...
            assert_eq!(store.get(&key), None, "reject must not store the garbage");
            // ...and nothing was forwarded onto the inbound.
            assert!(
                in_rx.try_recv().is_err(),
                "reject must not forward onto the inbound"
            );
        });
    }

    /// the matching accept path: bytes that DO hash to the requested digest are
    /// stored and forwarded byte-identically onto the inbound tagged
    /// `Lane::Consensus`, and the fetch resolves `true`.
    #[test]
    fn consumer_accepts_and_forwards_payload_matching_the_fetched_key() {
        let executor = deterministic::Runner::timed(Duration::from_secs(5));
        executor.start(|_context| async move {
            let store = ContentStore::new();
            let (in_tx, mut in_rx) = mpsc::channel::<Inbound>(8);
            let mut consumer = PayloadConsumer::new(store.clone(), in_tx);

            let payload = b"the real finalized batch".to_vec();
            let key = digest_of(&payload);
            let delivery = Delivery {
                key,
                subscribers: NonEmptyVec::new(()),
            };

            let valid = consumer
                .deliver(delivery, Bytes::from(payload.clone()))
                .await
                .expect("consumer resolves a verdict");
            assert!(valid, "matching bytes verify (resolves true)");

            // stored under its content address...
            assert_eq!(store.get(&key), Some(payload.clone()));
            // ...and forwarded byte-identically on the consensus lane.
            let (lane, recv) = in_rx.recv().await.expect("forwarded to inbound");
            assert_eq!(lane, Lane::Consensus);
            assert_eq!(recv, payload);
        });
    }
}
