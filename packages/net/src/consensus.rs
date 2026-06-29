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
use commonware_cryptography::{ed25519, sha256, Hasher, Sha256};
use commonware_p2p::{Recipients, Sender};
use commonware_resolver::Resolver as _;
use commonware_runtime::IoBuf;
use commonware_utils::channel::{fallible::OneshotExt, oneshot};
use op::Lane;
use tokio::sync::mpsc;
use transport::Inbound;

/// the concrete catch-up fetch handle the reporter holds: a
/// [`commonware_resolver`] p2p mailbox keyed by op-batch [`Digest`], over our
/// ed25519 peer identities, with no subscriber annotation (`()`). fetches are
/// fire-and-forget — `fetch(digest)` enqueues a request and the resolver's
/// consumer (see [`crate::payload_fetch`]) verifies + stores + delivers when the
/// bytes arrive. concrete (NOT a new generic on the reporter) so the existing
/// reporter call sites are untouched.
pub type PayloadFetcher = commonware_resolver::p2p::Mailbox<Digest, ed25519::PublicKey, ()>;

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

/// `ContentStore` is the sha256 half of the substrate's one storage contract
/// ([`objstore::ObjectStore`]) — the same trait the git ODB implements, over a
/// different id space (consensus op-batch payloads keyed by sha256 of the bytes,
/// NOT git oids). the in-memory map never faults, so the error is [`Infallible`].
/// the inherent `put`/`get` stay for existing consensus callers; the trait
/// methods delegate to them (disambiguated via `ContentStore::`, never `self.`).
impl objstore::ObjectStore<Digest> for ContentStore {
    type Error = std::convert::Infallible;

    fn put(&self, bytes: Vec<u8>) -> Result<Digest, Self::Error> {
        Ok(ContentStore::put(self, bytes))
    }

    fn get(&self, id: &Digest) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(ContentStore::get(self, id))
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

/// the relay: gossips a proposed payload's full bytes to every peer so
/// non-proposers — which learn only the DIGEST through consensus — can resolve
/// it. simplex hands `broadcast` the digest of the batch the leader is
/// proposing; we look those bytes up in the [`ContentStore`] (the proposer
/// staged them on `send(Lane::Consensus, ..)`) and `send(Recipients::All, ..)`
/// them out on a dedicated payload channel ([`CHANNEL_PAYLOAD`](crate)). peers
/// drain that channel STORE-ONLY (see `crate::spawn_payload_drain`), so when the
/// digest later finalizes their [`ConsensusReporter`] resolves
/// `store.get(&digest)` and delivers — via the SAME finalization path the
/// proposer already used.
///
/// content-addressing IS the verification: the receiver re-hashes the bytes as
/// the store key, so byzantine garbage stores under its own hash and can never
/// match a finalized digest — no signature check needed.
///
/// generic over the gossip `Sender` `S` (mirroring
/// [`CommonwareTransport`](crate::CommonwareTransport)) so the production
/// discovery sender and the test simulated sender both plug in, and over the
/// public key `P` for the [`Relay`] trait. `broadcast` is synchronous and
/// `Sender::send` is too, so this fits the trait with no actor/await — it clones
/// the (cheap) sender per call, exactly like the broadcast lane in [`crate`].
#[derive(Clone)]
pub struct ConsensusRelay<S, P> {
    store: ContentStore,
    /// gossip sender for the dedicated payload channel. cloned per `broadcast`.
    sender: S,
    _marker: std::marker::PhantomData<fn() -> P>,
}

impl<S, P> ConsensusRelay<S, P> {
    /// `sender` gossips on the payload channel; `store` MUST be the same
    /// [`ContentStore`] the proposer staged into and the reporter resolves from
    /// (see [`ConsensusAutomaton::handle`]'s precondition).
    pub fn new(sender: S, store: ContentStore) -> Self {
        Self {
            store,
            sender,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<S, P> Relay for ConsensusRelay<S, P>
where
    S: Sender + Clone + Send + Sync + 'static,
    P: commonware_cryptography::PublicKey,
{
    type Digest = Digest;
    type PublicKey = P;
    type Plan = Plan<P>;

    fn broadcast(&mut self, payload: Self::Digest, _plan: Self::Plan) -> Feedback {
        // gossip the proposed payload's bytes to every peer so non-proposers can
        // resolve the digest when it finalizes. if we don't hold the bytes (we're
        // not the proposer, or never staged this digest) there's nothing to relay
        // — accept and move on. `Sender::send` is synchronous (returns the
        // recipients it will attempt, no await), so this fits the sync signature;
        // best-effort like the broadcast lane — offline peers are skipped and the
        // proposer re-gossips each leadership turn (propose re-fires until commit).
        if let Some(bytes) = self.store.get(&payload) {
            let mut sender = self.sender.clone();
            let _ = sender.send(Recipients::All, IoBuf::from(bytes), false);
        }
        Feedback::Ok
    }
}

/// resolve a finalized `digest`'s bytes from `store` and forward them onto the
/// inbound mpsc, tagged [`Lane::Consensus`]. returns `true` if the payload was
/// present and forwarded, `false` on a store miss (the caller may then fetch it
/// from peers). this is the single content-addressing forward seam: both the
/// finalization reporter (its hit arm) and the catch-up resolver consumer (after
/// it re-hashes + stores the fetched bytes) deliver through here, so delivery
/// happens in exactly one place.
pub(crate) fn deliver_payload(
    store: &ContentStore,
    inbound: &mpsc::Sender<Inbound>,
    digest: Digest,
) -> bool {
    if let Some(bytes) = store.get(&digest) {
        // best-effort, non-blocking handoff to the inbound side. we use try_send
        // (not an await) because the sync reporter calls this; if the inbound
        // consumer is backed up the message is dropped, matching the best-effort
        // delivery the loopback/gossip paths already use.
        let _ = inbound.try_send((Lane::Consensus, bytes));
        true
    } else {
        false
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
    /// the catch-up fetch handle, if wired. on a finalization whose payload is
    /// MISSING from the store (no eager relay broadcast reached us), the reporter
    /// asks this to fetch the bytes from peers by digest. `None` preserves the
    /// pre-fetch behavior (silent drop on a miss) so existing call sites compile
    /// unchanged — production wiring installs it via [`with_payload_fetcher`].
    ///
    /// [`with_payload_fetcher`]: ConsensusReporter::with_payload_fetcher
    fetcher: Option<PayloadFetcher>,
    _marker: std::marker::PhantomData<fn() -> S>,
}

impl<S> ConsensusReporter<S> {
    /// `store` MUST be the same [`ContentStore`] the proposing side staged bytes
    /// into (see [`ConsensusAutomaton::handle`]'s precondition) — finalization
    /// resolves the digest via `store.get`, so a mismatched store silently drops
    /// the finalized batch. `pending` MUST be the paired automaton's FIFO (from
    /// [`ConsensusAutomaton::pending`]) — that's what closes the
    /// peek-until-finalized loop: the automaton peeks, this reporter removes.
    ///
    /// the catch-up fetcher defaults to `None` (miss => silent drop, the
    /// pre-fetch behavior); wire it with [`with_payload_fetcher`].
    ///
    /// [`with_payload_fetcher`]: ConsensusReporter::with_payload_fetcher
    pub fn new(
        store: ContentStore,
        pending: Arc<Mutex<VecDeque<Digest>>>,
        inbound: mpsc::Sender<Inbound>,
    ) -> Self {
        Self {
            store,
            pending,
            inbound,
            fetcher: None,
            _marker: std::marker::PhantomData,
        }
    }

    /// install the catch-up [`PayloadFetcher`]: on a finalization whose payload is
    /// missing from the store, the reporter fetches the bytes from peers by digest
    /// instead of dropping the batch. without this the miss path silently drops —
    /// so a `new()` that forgets to call this regresses to that drop. wire it on
    /// the SAME path production `new()` uses so a missing wire fails loudly.
    pub fn with_payload_fetcher(mut self, fetcher: PayloadFetcher) -> Self {
        self.fetcher = Some(fetcher);
        self
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
            // resolve + forward through the one delivery seam. a miss means we
            // finalized a digest whose bytes we never cached (no eager relay
            // broadcast reached us — a late join, or we simply missed the gossip).
            if !deliver_payload(&self.store, &self.inbound, digest) {
                if let Some(fetcher) = self.fetcher.as_mut() {
                    // backstop: fetch the bytes from peers by digest. the resolver
                    // consumer (see `crate::payload_fetch`) re-hashes on receipt,
                    // stores, and delivers through `deliver_payload` — so the
                    // caught-up batch reaches the inbound the same way an eager one
                    // does. fire-and-forget: `report` is sync, and `fetch` only
                    // enqueues a request (the delivery happens later, off this call).
                    let _ = fetcher.fetch(digest);
                }
                // with no fetcher wired this is the pre-fetch behavior: the
                // finalized batch is dropped. production `new()` always wires one.
            }
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
    fn content_store_satisfies_object_store_trait() {
        use objstore::ObjectStore;
        let store = ContentStore::new();
        let bytes = b"payload via the trait".to_vec();

        // through the generic trait surface (Infallible, so unwrap is total).
        let id = ObjectStore::put(&store, bytes.clone()).unwrap();
        assert_eq!(id, digest_of(&bytes));
        assert_eq!(ObjectStore::get(&store, &id).unwrap(), Some(bytes));
        assert!(ObjectStore::has(&store, &id).unwrap());
        assert!(!ObjectStore::has(&store, &digest_of(b"nope")).unwrap());
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
