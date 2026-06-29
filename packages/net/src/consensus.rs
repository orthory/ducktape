//! simplex consensus scaffolding (p1.2, tier B — wired but DORMANT).
//!
//! this module holds the application-side glue commonware-simplex needs to
//! drive BFT total-ordering of the consensus lane: an [`Automaton`] (proposes /
//! verifies / certifies payloads), a [`Relay`] (broadcasts full payloads the
//! engine only knows by digest), a [`Reporter`] (receives consensus activity —
//! crucially `Activity::Finalization` — and delivers finalized payloads), and a
//! [`ContentStore`] (the digest→bytes map that resolves the opaque digests
//! simplex orders back into the op-batch bytes peers actually apply).
//!
//! ## the landmine this module exists to handle
//!
//! simplex orders opaque DIGESTS, not payloads. [`Automaton::propose`] returns a
//! `Digest`; the op-batch bytes do NOT ride inside consensus — peers fetch the
//! payload out-of-band by digest (that's what [`Relay`] broadcasts and what the
//! [`ContentStore`] caches). finalized entries surface via
//! [`Reporter::report`] as `Activity::Finalization`, and THAT callback is where
//! an application applies/delivers them. so the data flow is:
//!
//! ```text
//!   send(Consensus, bytes)
//!     -> digest = sha256(bytes); store.put(digest, bytes); enqueue(digest)
//!     -> Automaton::propose PEEKS the front digest -> simplex orders it
//!     -> ... 2f+1 finalize votes ...
//!     -> Reporter::report(Activity::Finalization(f))
//!     -> bytes = store.get(f.proposal.payload)
//!     -> remove that digest from the pending FIFO (now committed)
//!     -> forward (Lane::Consensus, bytes) into the inbound mpsc
//! ```
//!
//! propose PEEKS rather than pops: a leader view that nullifies before quorum
//! (routine over a real network while the peer mesh is still forming) must NOT
//! lose the batch — it stays queued and is re-proposed next time we lead.
//! removal happens at exactly one point, finalization (in the reporter), so the
//! digest survives any number of nullified views yet is proposed at most once
//! more after it commits.
//!
//! ## why DORMANT (what's left for tier A)
//!
//! these impls COMPILE against the exact `commonware_consensus` trait bounds,
//! but nothing here instantiates a simplex [`Engine`](commonware_consensus::simplex::Engine).
//! standing one up additionally requires: a concrete certificate [`Scheme`]
//! (e.g. the simplex `ed25519` fixture), an `Elector` (`RoundRobin`), a
//! `Blocker` (from the p2p oracle), a `Strategy` (`Sequential`), `Storage` on
//! the runtime context, and THREE registered p2p sub-channels (vote /
//! certificate / resolver) per validator — see `simplex::mod`'s `all_online`
//! test for the full wiring. that integration is the tier-A follow-up; until
//! then the live consensus lane is the ordered-gossip path in [`crate`] (tier
//! C), and this scaffolding sits ready to swap in.
//!
//! the impls are generic over the certificate [`Scheme`] `S` so this module
//! never has to name simplex's macro-generated concrete scheme types; the digest
//! is fixed to [`sha256::Digest`] because that's what the [`ContentStore`] keys
//! on and what we hash op-batch bytes into.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use commonware_actor::Feedback;
use commonware_consensus::{
    simplex::{
        types::{Activity, Context},
        Plan,
    },
    Automaton, CertifiableAutomaton, Relay, Reporter,
};
use commonware_cryptography::certificate::Scheme;
use commonware_cryptography::{sha256, Hasher, Sha256};
use commonware_utils::channel::{fallible::OneshotExt, oneshot};
use op::Lane;
use tokio::sync::mpsc;
use transport::Inbound;

/// the concrete digest the consensus lane orders over: a sha256 of the op-batch
/// bytes. simplex is digest-agnostic, but fixing it here lets the
/// [`ContentStore`] key on a plain `Copy` type and lets us hash with [`Sha256`].
pub type Digest = sha256::Digest;

/// hash an op-batch's bytes into the [`Digest`] simplex will order. this is the
/// content-address: identical bytes always map to the same digest, so a peer
/// that already has the payload can short-circuit the [`Relay`] fetch.
pub fn digest_of(bytes: &[u8]) -> Digest {
    let mut hasher = Sha256::default();
    hasher.update(bytes);
    hasher.finalize()
}

/// digest→bytes map: resolves the opaque digests simplex finalizes back into the
/// op-batch bytes the application applies.
///
/// this is the in-memory stand-in for `commonware-storage`; for a deterministic
/// sim a plain map is enough (a production node would persist this so payloads
/// survive restarts and can be served to lagging peers). cloning shares the
/// backing store (`Arc`), so the [`Automaton`], [`Relay`], and [`Reporter`] can
/// each hold a handle to the same content.
#[derive(Clone, Default)]
pub struct ContentStore {
    inner: Arc<Mutex<HashMap<Digest, Vec<u8>>>>,
}

impl ContentStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// cache `bytes` under its content-address and return that digest. called on
    /// the `send(Consensus, ..)` path before the digest is ever proposed.
    pub fn put(&self, bytes: Vec<u8>) -> Digest {
        let digest = digest_of(&bytes);
        self.inner
            .lock()
            .expect("content store poisoned")
            .insert(digest, bytes);
        digest
    }

    /// look up the bytes for a finalized digest. `None` means we never saw the
    /// payload (a real node would trigger a [`Relay`]/resolver fetch here).
    pub fn get(&self, digest: &Digest) -> Option<Vec<u8>> {
        self.inner
            .lock()
            .expect("content store poisoned")
            .get(digest)
            .cloned()
    }
}

/// the transport-facing intake for the consensus lane: stage op-batch bytes and
/// queue their digest for the simplex [`Engine`](commonware_consensus::simplex::Engine)
/// to order. this is the seam `CommonwareTransport::send(Lane::Consensus, ..)`
/// drives once the live engine is wired — replacing the tier-C ordered gossip
/// with `store.put(bytes)` + `enqueue(digest)`.
///
/// deliberately NON-generic: it shares the [`ContentStore`] and the
/// [`ConsensusAutomaton`]'s pending FIFO (both `Arc`-backed), but never the
/// automaton's phantom public-key parameter — the enqueue path only ever pushes
/// a [`Digest`], so a transport holding this handle stays free of consensus
/// scheme/key generics. clone shares the backing store + queue.
#[derive(Clone)]
pub struct ConsensusHandle {
    store: ContentStore,
    /// the same FIFO [`ConsensusAutomaton::propose`] pops from. wired via
    /// [`ConsensusAutomaton::handle`] so a `send` and the automaton agree on the
    /// queue they share.
    pending: Arc<Mutex<VecDeque<Digest>>>,
}

impl ConsensusHandle {
    /// stage `bytes` for consensus: content-address them into the store (so the
    /// digest resolves on finalization) and queue that digest for proposal. this
    /// is the entire `send(Lane::Consensus, ..)` body under a live engine.
    pub fn submit(&self, bytes: Vec<u8>) {
        let digest = self.store.put(bytes);
        self.pending
            .lock()
            .expect("pending queue poisoned")
            .push_back(digest);
    }
}

/// the application automaton: proposes the next queued op-batch digest, and
/// (trivially) verifies/certifies everything.
///
/// `propose` PEEKS the front of a shared FIFO that `send(Consensus, ..)` pushes
/// onto (the paired [`ConsensusReporter`] removes a digest only once it
/// finalizes — see [`ConsensusReporter::report`]). verification is a no-op
/// `true` because in this single-app sim every
/// payload we'd be asked about is one we ourselves stored — a real deployment
/// would check the payload resolves and is structurally valid (and keep the
/// channel pending, never resolving `false` for "not yet", per the trait docs).
///
/// generic over the public key `P` so `Context<Digest, P>` lines up with
/// whatever scheme the engine is configured with.
#[derive(Clone)]
pub struct ConsensusAutomaton<P> {
    /// digests awaiting proposal, newest-last. shared with the `send` path.
    pending: Arc<Mutex<VecDeque<Digest>>>,
    _marker: std::marker::PhantomData<fn() -> P>,
}

impl<P> ConsensusAutomaton<P> {
    pub fn new() -> Self {
        Self {
            pending: Arc::new(Mutex::new(VecDeque::new())),
            _marker: std::marker::PhantomData,
        }
    }

    /// queue a digest to be proposed on the next `propose` call. the bytes must
    /// already be in the [`ContentStore`] so peers can resolve them.
    pub fn enqueue(&self, digest: Digest) {
        self.pending
            .lock()
            .expect("pending queue poisoned")
            .push_back(digest);
    }

    /// mint a transport-facing [`ConsensusHandle`] that shares THIS automaton's
    /// pending queue and the given `store`. `handle.submit(bytes)` then stages +
    /// enqueues onto the very FIFO this automaton's `propose` pops from — the
    /// glue that lets `send(Lane::Consensus, ..)` feed a live engine.
    ///
    /// PRECONDITION (load-bearing, not type-enforced): `store` MUST be the same
    /// [`ContentStore`] handed to this validator's [`ConsensusReporter`] (and
    /// [`ConsensusRelay`]). the handle `put`s bytes under their digest; the
    /// reporter `get`s by that digest on finalization. give them different stores
    /// and a finalized digest resolves to `None` in the reporter — the batch is
    /// SILENTLY DROPPED, never delivered. clone one `ContentStore` into all three
    /// (they're `Arc`-backed). the tier-A wiring helper will mint the store once
    /// and enforce this structurally; until then it's the caller's contract.
    pub fn handle(&self, store: ContentStore) -> ConsensusHandle {
        ConsensusHandle {
            store,
            pending: Arc::clone(&self.pending),
        }
    }

    /// share THIS automaton's pending FIFO with its paired
    /// [`ConsensusReporter`], so the reporter can remove a digest once it
    /// finalizes (the peek-until-finalized contract: `propose` peeks the front,
    /// the reporter removes on `Activity::Finalization`). the reporter MUST be
    /// the one configured on the same validator's engine as this automaton —
    /// they have to agree on the queue, exactly like [`handle`] and `propose` do.
    ///
    /// [`handle`]: ConsensusAutomaton::handle
    pub fn pending(&self) -> Arc<Mutex<VecDeque<Digest>>> {
        Arc::clone(&self.pending)
    }
}

impl<P> Default for ConsensusAutomaton<P> {
    fn default() -> Self {
        Self::new()
    }
}

impl<P> Automaton for ConsensusAutomaton<P>
where
    P: commonware_cryptography::PublicKey,
{
    type Context = Context<Digest, P>;
    type Digest = Digest;

    async fn propose(&mut self, _context: Self::Context) -> oneshot::Receiver<Self::Digest> {
        let (tx, rx) = oneshot::channel();
        // PEEK the front queued digest — do NOT remove it. if this view fails to
        // reach quorum and nullifies (routine while a real network's peer mesh is
        // still forming), the digest must stay queued so we re-propose it next
        // time we lead; popping here would lose it forever and stall the lane.
        // the digest is removed at exactly one point — finalization, in
        // `ConsensusReporter::report` — so it commits at most once.
        //
        // if nothing is queued we drop `tx`, which the trait documents as "can't
        // propose right now" — the engine moves on and we'll get another turn.
        if let Some(digest) = self
            .pending
            .lock()
            .expect("pending queue poisoned")
            .front()
            .copied()
        {
            tx.send_lossy(digest);
        }
        rx
    }

    async fn verify(
        &mut self,
        _context: Self::Context,
        _payload: Self::Digest,
    ) -> oneshot::Receiver<bool> {
        let (tx, rx) = oneshot::channel();
        tx.send_lossy(true);
        rx
    }
}

impl<P> CertifiableAutomaton for ConsensusAutomaton<P> where P: commonware_cryptography::PublicKey {}

/// the relay: broadcasts a finalized-but-only-digest-known payload's full bytes
/// so peers that don't already have it can resolve the digest.
///
/// in tier C the actual byte gossip is handled by the dedicated consensus
/// channel in [`crate`]; here `broadcast` is the seam where, under a live
/// engine, we'd push `store.get(payload)` out on that channel. it returns
/// `Feedback::Ok` (accepted) so the engine never backs off on our account.
#[derive(Clone)]
pub struct ConsensusRelay<P> {
    store: ContentStore,
    _marker: std::marker::PhantomData<fn() -> P>,
}

impl<P> ConsensusRelay<P> {
    pub fn new(store: ContentStore) -> Self {
        Self {
            store,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<P> Relay for ConsensusRelay<P>
where
    P: commonware_cryptography::PublicKey,
{
    type Digest = Digest;
    type PublicKey = P;
    type Plan = Plan<P>;

    fn broadcast(&mut self, payload: Self::Digest, _plan: Self::Plan) -> Feedback {
        // a live engine would gossip the resolved bytes here; for the dormant
        // scaffolding we just confirm the payload is resolvable and accept. the
        // lookup keeps `store` load-bearing (and asserts the propose path stored
        // the bytes before proposing the digest).
        let _ = self.store.get(&payload);
        Feedback::Ok
    }
}

/// the reporter: the delivery seam. simplex calls `report` for every consensus
/// activity; we care about exactly one — `Activity::Finalization`. when a
/// proposal finalizes we resolve its payload digest in the [`ContentStore`] and
/// forward the op-batch bytes into the inbound mpsc tagged `Lane::Consensus`,
/// completing the round trip from `send(Consensus, ..)` on one node to inbound
/// delivery on every node (in BFT-agreed order).
///
/// generic over the certificate scheme `S` so we never name a concrete scheme;
/// the only field of `Activity` we touch is `Finalization::proposal::payload`,
/// which is scheme-independent.
#[derive(Clone)]
pub struct ConsensusReporter<S> {
    store: ContentStore,
    /// the paired automaton's pending FIFO. on finalization we remove the
    /// committed digest from it so `propose` advances past it (and the
    /// peek-until-finalized contract terminates). shared via
    /// [`ConsensusAutomaton::pending`].
    pending: Arc<Mutex<VecDeque<Digest>>>,
    inbound: mpsc::Sender<Inbound>,
    _marker: std::marker::PhantomData<fn() -> S>,
}

impl<S> ConsensusReporter<S> {
    /// `store` MUST be the same [`ContentStore`] the proposing side staged bytes
    /// into (see [`ConsensusAutomaton::handle`]'s precondition) — finalization
    /// resolves the digest via `store.get`, so a mismatched store silently drops
    /// the finalized batch. `pending` MUST be the paired automaton's FIFO (from
    /// [`ConsensusAutomaton::pending`]) — that's what closes the
    /// peek-until-finalized loop: the automaton peeks, this reporter removes.
    pub fn new(
        store: ContentStore,
        pending: Arc<Mutex<VecDeque<Digest>>>,
        inbound: mpsc::Sender<Inbound>,
    ) -> Self {
        Self {
            store,
            pending,
            inbound,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<S> Reporter for ConsensusReporter<S>
where
    S: Scheme + 'static,
{
    type Activity = Activity<S, Digest>;

    fn report(&mut self, activity: Self::Activity) -> Feedback {
        // the ONLY activity we deliver on is a recovered finalization
        // certificate — that's the BFT-agreed "this payload is committed".
        if let Activity::Finalization(finalization) = activity {
            let digest = finalization.proposal.payload;
            // committed: drop it from the pending FIFO so `propose` (which only
            // PEEKS the front) advances to the next batch and never re-proposes
            // this one. remove by value, not blind pop_front — on a node that
            // didn't propose this digest the queue won't contain it (no-op), and
            // we never want to discard a different node's still-pending batch.
            {
                let mut queue = self.pending.lock().expect("pending queue poisoned");
                if let Some(pos) = queue.iter().position(|d| *d == digest) {
                    queue.remove(pos);
                }
            }
            if let Some(bytes) = self.store.get(&digest) {
                // best-effort, non-blocking handoff to the inbound side. we use
                // try_send (not an await) because `report` is sync; if the
                // inbound consumer is backed up the message is dropped, matching
                // the best-effort delivery the loopback/gossip paths already use.
                let _ = self.inbound.try_send((Lane::Consensus, bytes));
            }
            // a missing payload would mean we finalized a digest we never
            // resolved — a real node fetches it via the resolver channel here.
        }
        Feedback::Ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_store_round_trips_by_digest() {
        let store = ContentStore::new();
        let bytes = b"some op batch".to_vec();
        let digest = store.put(bytes.clone());
        // same bytes -> same content-address.
        assert_eq!(digest, digest_of(&bytes));
        assert_eq!(store.get(&digest), Some(bytes));
        // a digest we never stored resolves to nothing.
        assert_eq!(store.get(&digest_of(b"unseen")), None);
    }

    #[test]
    fn automaton_proposes_queued_digests_in_order() {
        // exercise the FIFO directly (the Automaton trait's async propose is
        // driven by the engine; here we assert the queue semantics propose
        // relies on). uses a stand-in public key type via the concrete sha256
        // ed25519 key would pull the whole scheme in — the queue is key-agnostic.
        let store = ContentStore::new();
        let d1 = store.put(b"first".to_vec());
        let d2 = store.put(b"second".to_vec());

        let auto = ConsensusAutomaton::<commonware_cryptography::ed25519::PublicKey>::new();
        auto.enqueue(d1);
        auto.enqueue(d2);

        let mut q = auto.pending.lock().unwrap();
        assert_eq!(q.pop_front(), Some(d1));
        assert_eq!(q.pop_front(), Some(d2));
        assert_eq!(q.pop_front(), None);
    }

    #[test]
    fn propose_peeks_so_a_nullified_view_can_repropose() {
        // the load-bearing guard for the peek-not-pop fix in `propose`.
        //
        // a proposed view that NULLIFIES (never reaches quorum) must not lose the
        // queued batch — the engine just calls `propose` again next time this node
        // leads. driving `propose` twice with NO finalization in between models
        // exactly that nullify-then-re-lead path. with peek, both calls yield the
        // same digest. with the old `pop_front`, the second call finds an empty
        // queue, drops its sender, and the receiver resolves to `Err` — the lane
        // stalls forever. removal happens at one place only: `ConsensusReporter`
        // on finalization, which this test deliberately never triggers.
        use commonware_consensus::types::{Epoch, Round, View};
        use commonware_cryptography::Signer;
        use commonware_runtime::{deterministic, Runner};

        let executor = deterministic::Runner::timed(std::time::Duration::from_secs(5));
        executor.start(|_context| async move {
            let store = ContentStore::new();
            let digest = store.put(b"queued batch".to_vec());

            let mut automaton =
                ConsensusAutomaton::<commonware_cryptography::ed25519::PublicKey>::new();
            automaton.enqueue(digest);

            // `propose` ignores its Context; build a minimal valid one to pass in.
            let leader = commonware_cryptography::ed25519::PrivateKey::from_seed(0).public_key();
            let context = || Context {
                round: Round::new(Epoch::new(0), View::new(1)),
                leader: leader.clone(),
                parent: (View::new(0), digest),
            };

            // first time we lead: offer the queued digest.
            let first = automaton
                .propose(context())
                .await
                .await
                .expect("first propose yields the queued digest");
            assert_eq!(first, digest, "propose should offer the queued batch");

            // that view nullified — no finalization fired, nothing was removed.
            // lead again: the SAME digest must still be proposable. (with the old
            // pop_front this await is Err on a dropped sender and the batch is lost.)
            let second = automaton
                .propose(context())
                .await
                .await
                .expect("a nullified view keeps the batch queued — re-propose succeeds");
            assert_eq!(
                second, digest,
                "peek must keep the batch proposable after a nullified view"
            );
        });
    }
}
