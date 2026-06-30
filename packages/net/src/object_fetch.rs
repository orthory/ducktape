//! catch-up fetch for the git objects a parked ref move is missing.
//!
//! a node finalizes a ref move ([`op::Op::Vcs`] → `RefUpdate`) by consensus, but
//! the target commit's object closure may not be present locally — so the engine
//! PARKS the move and fills the closure object-by-object (the frontier walk in
//! [`vcs::objects::missing_frontier`]). this module is the wire for that fill: a
//! [`commonware_resolver`] p2p engine that
//!
//! - [`ObjectProducer`] SERVES a git object's loose bytes by its oid from the
//!   local [`GitOdb`] to peers fetching it (a local miss drops the sender unsent
//!   so the resolver retries another peer — never a wrong or empty object under
//!   the oid), and
//! - [`ObjectConsumer`] RECEIVES fetched bytes, re-hashes them to verify the
//!   content address, stores them in the ODB, and pokes a repoll nudge so the
//!   engine re-checks the parked ref. a hash mismatch resolves `false`, which
//!   blocks the lying peer.
//!
//! content-addressing IS the verification, exactly as for consensus payloads: a
//! sha256-mode git oid is `sha256` of the object's loose form `<type> <len>\0…`,
//! and the bytes on the wire ARE that loose form ([`GitOdb::get`] returns it), so
//! `digest_of(value) == requested oid` is the whole check — the same
//! [`digest_of`] the payload lane uses, over a different value space. byzantine
//! garbage hashes to something else and can never land under the oid we asked
//! for; no signature needed.
//!
//! the wire KEY is reused as [`Digest`] (`sha256::Digest`) rather than a new
//! `vcs::ObjectId` span type: a git oid and a consensus digest are both a 32-byte
//! sha256 content address, so the two convert by moving bytes
//! (`Digest::from(*id.as_bytes())` / `ObjectId::from_bytes(key.0)`) with no
//! re-hash. that keeps `vcs` free of any commonware codec bound — the conversion
//! lives only here, at the net boundary.

use std::path::PathBuf;

use bytes::Bytes;
use commonware_resolver::p2p::Producer;
use commonware_resolver::{Consumer, Delivery};
use commonware_utils::channel::oneshot;
use objstore::ObjectStore;
use tokio::sync::mpsc;
use vcs::{GitOdb, ObjectId};

use crate::consensus::{digest_of, Digest};

/// serves git objects by oid from a local [`GitOdb`] to peers fetching them over
/// the resolver. holds the repo path (not a `GitOdb` handle, which isn't `Clone`)
/// and opens the store per request — opening is just wrapping the path; the
/// shared store IS the on-disk repo.
#[derive(Clone)]
pub struct ObjectProducer {
    repo: PathBuf,
}

impl ObjectProducer {
    /// `repo` MUST be this node's git repo — the same one its engine moves refs
    /// in — so a peer fetching an oid gets the very object this node holds.
    pub fn new(repo: PathBuf) -> Self {
        Self { repo }
    }
}

impl Producer for ObjectProducer {
    type Key = Digest;

    fn produce(&mut self, key: Self::Key) -> oneshot::Receiver<Bytes> {
        let (tx, rx) = oneshot::channel();
        // the oid and the wire digest are the same 32 bytes.
        let id = ObjectId::from_bytes(key.0);
        // serve the loose bytes if we hold the object; otherwise drop `tx` UNSENT
        // (a git error reads as a miss too). the resolver treats a dropped sender
        // as "no data" and retries another peer — never a wrong object under the
        // oid.
        if let Ok(Some(bytes)) = GitOdb::open(&self.repo).get(&id) {
            let _ = tx.send(Bytes::from(bytes));
        }
        rx
    }
}

/// receives fetched git objects, verifies the content address, stores them in the
/// ODB, and pokes a repoll nudge so the engine re-checks its parked refs. clone
/// shares the repo path + the nudge sender.
#[derive(Clone)]
pub struct ObjectConsumer {
    repo: PathBuf,
    /// a coalescing nudge: a unit on this channel asks the engine to re-poll its
    /// parked ref moves now (a freshly-landed object may complete a closure).
    /// best-effort — a full channel already has a pending nudge.
    repoll: mpsc::Sender<()>,
}

impl ObjectConsumer {
    /// `repo` MUST be the node's git repo (the one its engine parks refs against),
    /// so a fetched object that lands here completes the closure the engine is
    /// waiting on; `repoll` MUST be the nudge the node's poll loop selects on.
    pub fn new(repo: PathBuf, repoll: mpsc::Sender<()>) -> Self {
        Self { repo, repoll }
    }
}

impl Consumer for ObjectConsumer {
    type Key = Digest;
    type Value = Bytes;
    type Subscriber = ();

    fn deliver(
        &mut self,
        delivery: Delivery<Self::Key, Self::Subscriber>,
        value: Self::Value,
    ) -> oneshot::Receiver<bool> {
        let (tx, rx) = oneshot::channel();
        // `into` is zero-copy when the fetched `Bytes` uniquely owns its buffer.
        let bytes: Vec<u8> = value.into();
        if digest_of(&bytes) == delivery.key {
            // content address verified: the fetched bytes hash to the oid we
            // asked for. store them (put re-derives the same oid), then nudge the
            // engine to re-poll — the landed object may complete a parked ref's
            // closure.
            match GitOdb::open(&self.repo).put(bytes) {
                Ok(_) => {
                    let _ = self.repoll.try_send(());
                    let _ = tx.send(true);
                }
                // the bytes verified but the store faulted (io): don't claim the
                // object. resolve false so the resolver retries.
                Err(_) => {
                    let _ = tx.send(false);
                }
            }
        } else {
            // the bytes do NOT hash to the requested oid — a lying peer. resolving
            // `false` tells the resolver to block it and retry another.
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
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    /// a collision-proof fresh sha256 repo (mirrors the vcs test fixtures:
    /// {pid}-{nanos} alone can collide under parallel init on macos).
    fn fresh_repo(tag: &str) -> GitOdb {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "net-objfetch-{}-{}-{}-{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            COUNTER.fetch_add(1, Ordering::Relaxed),
        ));
        GitOdb::init(&dir).expect("init sha256 odb")
    }

    fn cleanup(odb: &GitOdb) {
        std::fs::remove_dir_all(odb.repo()).ok();
    }

    /// git's loose-object form for a blob: `blob <len>\0<body>`. its sha256 is the
    /// object's oid, so `digest_of(loose_blob(b)) ==` the oid `put` will assign.
    fn loose_blob(body: &[u8]) -> Vec<u8> {
        let mut v = format!("blob {}\0", body.len()).into_bytes();
        v.extend_from_slice(body);
        v
    }

    /// the producer serves a present object's loose bytes by oid, and drops the
    /// sender unsent on a miss (so the resolver retries another peer).
    #[test]
    fn object_producer_serves_present_object_and_drops_on_miss() {
        let executor = deterministic::Runner::timed(Duration::from_secs(10));
        executor.start(|_context| async move {
            let odb = fresh_repo("producer");
            let repo = odb.repo().to_path_buf();
            let loose = loose_blob(b"served object\n");
            let id = odb.put(loose.clone()).expect("put");
            let mut producer = ObjectProducer::new(repo);

            // present -> serves the exact loose bytes under the oid.
            let key = Digest::from(*id.as_bytes());
            let served = producer
                .produce(key)
                .await
                .expect("a present object resolves");
            assert_eq!(served.as_ref(), loose.as_slice());

            // absent -> the sender is dropped UNSENT, so the receiver cancels.
            let absent = Digest::from([0u8; 32]);
            assert!(
                producer.produce(absent).await.is_err(),
                "a miss drops the producer sender unsent"
            );

            cleanup(&odb);
        });
    }

    /// the accept path: bytes that hash to the requested oid are stored in the ODB
    /// and the repoll nudge fires, and the fetch resolves `true`.
    #[test]
    fn object_consumer_verifies_stores_and_pokes_repoll() {
        let executor = deterministic::Runner::timed(Duration::from_secs(10));
        executor.start(|_context| async move {
            let odb = fresh_repo("consumer-accept");
            let repo = odb.repo().to_path_buf();
            let (repoll_tx, mut repoll_rx) = mpsc::channel::<()>(8);
            let mut consumer = ObjectConsumer::new(repo, repoll_tx);

            let loose = loose_blob(b"fetched object\n");
            let key = digest_of(&loose); // the git oid of the loose form
            let delivery = Delivery {
                key,
                subscribers: NonEmptyVec::new(()),
            };

            let ok = consumer
                .deliver(delivery, Bytes::from(loose.clone()))
                .await
                .expect("consumer resolves a verdict");
            assert!(ok, "bytes that hash to the requested oid are accepted");

            // landed in the ODB under its oid, round-tripping byte-identically...
            let id = ObjectId::from_bytes(key.0);
            assert!(odb.has(&id).expect("has"), "accepted object is in the odb");
            assert_eq!(odb.get(&id).expect("get"), Some(loose));
            // ...and the repoll nudge fired.
            assert!(
                repoll_rx.try_recv().is_ok(),
                "accept pokes the repoll nudge"
            );

            cleanup(&odb);
        });
    }

    /// the load-bearing safety property: bytes that do NOT hash to the requested
    /// oid are REJECTED — the fetch resolves `false` (blocks the peer), the ODB is
    /// left untouched, and the repoll nudge does NOT fire.
    #[test]
    fn object_consumer_rejects_bytes_whose_git_oid_mismatches_the_key() {
        let executor = deterministic::Runner::timed(Duration::from_secs(10));
        executor.start(|_context| async move {
            let odb = fresh_repo("consumer-reject");
            let repo = odb.repo().to_path_buf();
            let (repoll_tx, mut repoll_rx) = mpsc::channel::<()>(8);
            let mut consumer = ObjectConsumer::new(repo, repoll_tx);

            // we asked for the oid of the REAL object, but the peer hands back
            // unrelated bytes that hash to something else.
            let key = digest_of(&loose_blob(b"the object we wanted"));
            let tampered = Bytes::from_static(b"blob 9\0byzantine");
            let delivery = Delivery {
                key,
                subscribers: NonEmptyVec::new(()),
            };

            let ok = consumer
                .deliver(delivery, tampered)
                .await
                .expect("consumer resolves a verdict");
            assert!(!ok, "a hash mismatch must resolve false (blocks the peer)");

            // nothing stored under the requested oid...
            let id = ObjectId::from_bytes(key.0);
            assert!(!odb.has(&id).expect("has"), "reject must not store");
            // ...and the repoll nudge did NOT fire.
            assert!(
                repoll_rx.try_recv().is_err(),
                "reject must not poke the repoll nudge"
            );

            cleanup(&odb);
        });
    }
}
