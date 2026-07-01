//! the real commonware-simplex BFT [`node::Orderer`].
//!
//! `SimplexOrderer` is a drop-in for `RoundOrderer` behind node's async
//! [`Orderer`](node::Orderer) trait, but the total order comes from a LIVE
//! simplex [`Engine`](commonware_consensus::simplex::Engine) reaching
//! `Activity::Finalization` — not a deterministic sort. it solves the three
//! seam-crossing costs INSIDE this one type:
//!
//! 1. **sync reporter -> async drain.** simplex's [`Reporter::report`] is SYNC
//!    (it runs inside the engine task) but host apply is async. so `report`
//!    resolves the finalized digest's bytes from the shared [`ContentStore`] and
//!    inserts `(finalized view -> frame bytes)` into an `Arc`-shared
//!    [`FinalizedInbox`]; the sync, non-blocking [`Orderer::poll_delivered`]
//!    (called by `OrderedNode`) drains that buffer in ASCENDING VIEW order (a
//!    `BTreeMap` keyed by view). the host applies in that agreed order.
//!
//! 2. **multi-op/multi-view liveness (peek-not-pop).** [`ConsensusAutomaton::
//!    propose`] PEEKS the front of this node's pending FIFO — it never pops. a
//!    leader view that nullifies before quorum keeps its queued digest and
//!    re-proposes next turn; the digest is removed at EXACTLY one point,
//!    finalization (in [`SimplexReporter::report`], by value not `pop_front`), so
//!    an op survives any number of nullified views yet applies at most once.
//!
//! 3. **payload availability.** simplex orders opaque `sha256(frame)` DIGESTS,
//!    not payloads. two constructors resolve the bytes behind a finalized digest:
//!    [`SimplexOrderer::spawn`] uses a [`NoopRelay`] over one shared
//!    [`ContentStore`] (the in-process-sim simplification — one store cloned into
//!    every validator), while [`SimplexOrderer::spawn_with_relay`] runs a real
//!    [`ConsensusRelay`] over a PER-PROCESS store: the leader gossips a proposed
//!    frame's bytes at propose time, peers cache them STORE-ONLY (see
//!    [`spawn_payload_drain`]), so a non-proposer resolves a digest for an op it
//!    never originated. either way the ORDER comes purely from finalization; the
//!    store only resolves an already-agreed digest back to bytes.
//!
//! the whole thing is additive: `node::Orderer` / `OrderedNode` / the frame
//! codec / `RoundOrderer` are all UNCHANGED — `SimplexOrderer` slots in behind
//! the identical trait.

use std::collections::{BTreeMap, HashSet, HashMap, VecDeque};
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
use commonware_utils::channel::oneshot;
use commonware_utils::channel::fallible::OneshotExt;
use commonware_p2p::{Recipients, Sender};
use commonware_runtime::IoBuf;

/// the concrete digest the consensus lane orders over: a sha256 of the frame
/// bytes. fixing it here lets the [`ContentStore`] key on a plain `Copy` type.
pub type Digest = sha256::Digest;

/// hash a frame's bytes into the [`Digest`] simplex will order — the
/// content-address (identical bytes always map to the same digest).
pub fn digest_of(bytes: &[u8]) -> Digest {
    let mut hasher = Sha256::default();
    hasher.update(bytes);
    hasher.finalize()
}

// ============================================================================
// the shared content store — digest -> frame bytes.
// ============================================================================

/// digest->bytes map: resolves the opaque digests simplex finalizes back into
/// the frame bytes the host applies. cloning shares the backing store (`Arc`),
/// so the automaton, reporter, and submit handle all hold the SAME content — the
/// blessed in-process-sim shortcut (one store cloned into every validator).
#[derive(Clone, Default)]
pub struct ContentStore {
    inner: Arc<Mutex<HashMap<Digest, Vec<u8>>>>,
}

impl ContentStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// cache `bytes` under their content-address and return that digest. called
    /// on the `submit` path before the digest is ever proposed.
    pub fn put(&self, bytes: Vec<u8>) -> Digest {
        let digest = digest_of(&bytes);
        self.inner.lock().expect("content store poisoned").insert(digest, bytes);
        digest
    }

    /// look up the bytes for a finalized digest. `None` means we never saw the
    /// payload (impossible with the shared in-sim store; a real node fetches).
    pub fn get(&self, digest: &Digest) -> Option<Vec<u8>> {
        self.inner.lock().expect("content store poisoned").get(digest).cloned()
    }
}

// ============================================================================
// the submit handle — stage bytes + enqueue their digest.
// ============================================================================

/// the [`Orderer::submit`] intake: content-address the frame bytes into the
/// store (so the digest resolves on finalization) and queue that digest for this
/// node's simplex proposals. shares the [`ContentStore`] and this node's pending
/// FIFO (both `Arc`-backed); clone shares both.
#[derive(Clone)]
pub struct ConsensusHandle {
    store: ContentStore,
    /// the same FIFO [`ConsensusAutomaton::propose`] peeks. minted via
    /// [`ConsensusAutomaton::handle`] so submit + propose agree on the queue.
    pending: Arc<Mutex<VecDeque<Digest>>>,
}

impl ConsensusHandle {
    /// stage `bytes` for consensus: content-address them into the store and
    /// queue that digest for proposal. the entire `submit` body — NO local apply.
    pub fn submit(&self, bytes: Vec<u8>) {
        let digest = self.store.put(bytes);
        self.pending.lock().expect("pending queue poisoned").push_back(digest);
    }
}

// ============================================================================
// the automaton — peek-not-pop propose (cost 2).
// ============================================================================

/// proposes the next queued frame digest, and (trivially) verifies everything.
///
/// `propose` PEEKS the front of a shared FIFO that `submit` pushes onto (the
/// paired [`SimplexReporter`] removes a digest only once it finalizes). verify
/// is a no-op `true` — in this single-app sim every payload asked about is one we
/// stored. generic over the public key `P` so `Context<Digest, P>` lines up with
/// whatever scheme the engine runs.
#[derive(Clone)]
pub struct ConsensusAutomaton<P> {
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

    /// queue a digest to be proposed on the next `propose`. the bytes must
    /// already be in the [`ContentStore`] so peers can resolve them.
    pub fn enqueue(&self, digest: Digest) {
        self.pending.lock().expect("pending queue poisoned").push_back(digest);
    }

    /// mint a [`ConsensusHandle`] sharing THIS automaton's pending FIFO and the
    /// given `store`. `handle.submit(bytes)` then stages + enqueues onto the very
    /// FIFO this automaton's `propose` peeks.
    ///
    /// PRECONDITION (load-bearing): `store` MUST be the same [`ContentStore`]
    /// handed to this validator's [`SimplexReporter`] — the handle `put`s under a
    /// digest, the reporter `get`s by it on finalization. a mismatched store
    /// silently drops the finalized frame.
    pub fn handle(&self, store: ContentStore) -> ConsensusHandle {
        ConsensusHandle { store, pending: Arc::clone(&self.pending) }
    }

    /// share THIS automaton's pending FIFO with its paired [`SimplexReporter`],
    /// so the reporter can remove a digest once it finalizes (peek-until-
    /// finalized: propose peeks the front, the reporter removes on finalization).
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
        // PEEK the front queued digest — do NOT remove it. if this view nullifies
        // (routine while a peer mesh forms) the digest stays queued so we
        // re-propose it next time we lead; popping here would lose it forever.
        // removal happens at exactly one point — finalization, in
        // `SimplexReporter::report`. if nothing is queued we drop `tx` (the trait
        // reads that as "can't propose right now") and the engine moves on.
        if let Some(digest) = self.pending.lock().expect("pending queue poisoned").front().copied() {
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

// ============================================================================
// the no-op relay — a shared store makes payload dissemination unnecessary.
// ============================================================================

/// simplex requires a [`Relay`], but with one shared [`ContentStore`] every node
/// already resolves any finalized digest, so there is nothing to disseminate.
/// `broadcast` is a no-op that just satisfies the trait. (a real deployment
/// swaps this for the legacy gossip relay behind the same seam.)
#[derive(Clone, Default)]
pub struct NoopRelay<P>(std::marker::PhantomData<fn() -> P>);

impl<P> NoopRelay<P> {
    pub fn new() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<P> Relay for NoopRelay<P>
where
    P: commonware_cryptography::PublicKey,
{
    type Digest = Digest;
    type PublicKey = P;
    type Plan = Plan<P>;

    fn broadcast(&mut self, _payload: Self::Digest, _plan: Self::Plan) -> Feedback {
        Feedback::Ok
    }
}

// ============================================================================
// the eager gossip relay — disseminates a proposed frame's bytes to peers.
// ============================================================================

/// the real relay: on the leader's propose, gossips the proposed frame's full
/// bytes to every peer so non-proposers — which learn only the DIGEST through
/// consensus — can resolve it. simplex hands `broadcast` the digest of the frame
/// the leader is proposing; we look those bytes up in the per-process
/// [`ContentStore`] (the proposer staged them on `submit`) and `send` them to
/// `Recipients::All` on a dedicated payload channel. peers drain that channel
/// STORE-ONLY (see [`spawn_payload_drain`]), so when the digest later finalizes
/// their reporter resolves `store.get(&digest)` and delivers — via the SAME
/// finalization path the proposer uses.
///
/// content-addressing IS the verification: the receiver re-hashes the bytes as
/// the store key, so byzantine garbage stores under its own hash and can never
/// match a finalized digest — no signature check needed.
///
/// generic over the gossip `Sender` `S` (so the production discovery sender and a
/// test simulated sender both plug in) and over the public key `P` for the
/// [`Relay`] trait. `broadcast` is synchronous and `Sender::send` is too, so this
/// fits the trait with no actor/await — it clones the (cheap) sender per call.
#[derive(Clone)]
pub struct ConsensusRelay<S, P> {
    store: ContentStore,
    /// gossip sender for the dedicated payload channel. cloned per `broadcast`.
    sender: S,
    _marker: std::marker::PhantomData<fn() -> P>,
}

impl<S, P> ConsensusRelay<S, P> {
    /// `sender` gossips on the payload channel; `store` MUST be the same
    /// [`ContentStore`] the proposer staged into and the reporter resolves from.
    pub fn new(sender: S, store: ContentStore) -> Self {
        Self { store, sender, _marker: std::marker::PhantomData }
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
        // gossip the proposed frame's bytes to every peer so non-proposers can
        // resolve the digest when it finalizes. if we don't hold the bytes (we're
        // not the proposer, or never staged this digest) there's nothing to relay
        // — accept and move on. `Sender::send` is synchronous (returns the
        // recipients it will attempt, no await), so this fits the sync signature;
        // best-effort — offline peers are skipped and the proposer re-gossips each
        // leadership turn (propose peeks-until-finalized, re-firing until commit).
        if let Some(bytes) = self.store.get(&payload) {
            let mut sender = self.sender.clone();
            let _ = sender.send(Recipients::All, IoBuf::from(bytes), false);
        }
        Feedback::Ok
    }
}

/// spawn a task that drains the payload-gossip channel, caching each received
/// frame's bytes into the per-process [`ContentStore`] and doing NOTHING else.
///
/// this is how a non-proposer obtains the bytes behind a digest the leader's
/// [`ConsensusRelay`] broadcast, so that when that digest later finalizes the
/// reporter's `store.get(&digest)` resolves and delivers it — in BFT-agreed
/// order, via the SAME finalization path the proposer uses.
///
/// THE ONE TRAP OF THIS DESIGN: this NEVER forwards into the app's finalized
/// inbox / the pending FIFO. emitting the frame here would surface it to the app
/// pre-finalization (out of BFT order, and then AGAIN on finalization). a payload
/// receipt does exactly one thing — `store.put`. `ContentStore::put` re-hashes
/// the bytes as the key, so byzantine garbage stores under its own hash and can
/// never match a finalized digest; content-addressing is the whole verification.
fn spawn_payload_drain<E, R>(context: E, mut receiver: R, store: ContentStore)
where
    E: commonware_runtime::Spawner + Send + 'static,
    R: commonware_p2p::Receiver + Send + 'static,
{
    context.spawn(move |_ctx| async move {
        while let Ok((_peer, msg)) = receiver.recv().await {
            let bytes: Vec<u8> = msg.into();
            // store-ONLY: NO delivery. delivery stays the reporter's finalization arm.
            store.put(bytes);
        }
    });
}

// ============================================================================
// the finalized inbox — the sync-reporter -> async-drain buffer (cost 1).
// ============================================================================

/// finalized frames waiting to be drained, keyed by finalized view so a single
/// `poll_delivered` emits them in ASCENDING-VIEW (agreed) order. `Arc`-shared:
/// the SYNC reporter (inside the engine task) inserts; the SYNC `poll_delivered`
/// (in `OrderedNode`) takes.
///
/// PRECONDITION: the reporter observes finalizations in ascending view order
/// (true under perfect links — simplex finalizes views monotonically per node).
/// the `BTreeMap` orders WITHIN one poll; the precondition is what makes the
/// cross-poll order correct. `seen` makes application exactly-once even if a
/// re-finalization race ever re-reports a digest — no watermark cursor (a
/// cursor would silently drop a late lower view, a bug not robustness).
#[derive(Clone, Default)]
pub struct FinalizedInbox {
    inner: Arc<Mutex<FinalizedInner>>,
}

#[derive(Default)]
struct FinalizedInner {
    ready: BTreeMap<u64, Vec<u8>>,
    seen: HashSet<Digest>,
}

impl FinalizedInbox {
    pub fn new() -> Self {
        Self::default()
    }

    /// record a finalized `(view, digest)` whose bytes resolve in `store`. keyed
    /// by view for ascending-order drain; deduped by digest for exactly-once.
    fn deliver(&self, view: u64, digest: Digest, store: &ContentStore) {
        let mut inner = self.inner.lock().expect("finalized inbox poisoned");
        if inner.seen.insert(digest) {
            if let Some(bytes) = store.get(&digest) {
                inner.ready.insert(view, bytes);
            }
            // a miss is impossible in-sim (shared store); a real node fetches.
        }
    }

    /// take every ready frame in ascending-view order. non-blocking.
    fn drain(&self) -> Vec<Vec<u8>> {
        let mut inner = self.inner.lock().expect("finalized inbox poisoned");
        std::mem::take(&mut inner.ready).into_values().collect()
    }
}

// ============================================================================
// the reporter — sync finalization -> the inbox (costs 1 + 2).
// ============================================================================

/// the delivery seam. simplex calls `report` for every activity; we act on
/// exactly one — `Activity::Finalization`. on finalization we (a) remove the
/// committed digest from the pending FIFO BY VALUE (closing peek-until-
/// finalized), and (b) buffer `(finalized view -> frame bytes)` into the shared
/// [`FinalizedInbox`] for `poll_delivered` to drain in agreed order.
///
/// generic over the certificate scheme `S` so we never name a concrete scheme;
/// the only `Activity` fields touched are `Finalization::proposal::{payload,
/// round}`, both scheme-independent.
#[derive(Clone)]
pub struct SimplexReporter<S> {
    store: ContentStore,
    pending: Arc<Mutex<VecDeque<Digest>>>,
    inbox: FinalizedInbox,
    _marker: std::marker::PhantomData<fn() -> S>,
}

impl<S> SimplexReporter<S> {
    /// `store` MUST be the shared [`ContentStore`] the submit side staged into;
    /// `pending` MUST be the paired automaton's FIFO (from
    /// [`ConsensusAutomaton::pending`]); `inbox` MUST be the one this validator's
    /// [`SimplexOrderer`] drains.
    pub fn new(
        store: ContentStore,
        pending: Arc<Mutex<VecDeque<Digest>>>,
        inbox: FinalizedInbox,
    ) -> Self {
        Self { store, pending, inbox, _marker: std::marker::PhantomData }
    }
}

impl<S> Reporter for SimplexReporter<S>
where
    S: Scheme + 'static,
{
    type Activity = Activity<S, Digest>;

    fn report(&mut self, activity: Self::Activity) -> Feedback {
        // the ONLY activity we deliver on is a recovered finalization certificate
        // — the BFT-agreed "this frame is committed".
        if let Activity::Finalization(finalization) = activity {
            let digest = finalization.proposal.payload;
            let view = finalization.proposal.round.view().get();
            // committed: drop it from the pending FIFO so `propose` (peek-only)
            // advances and never re-proposes it. remove BY VALUE, not blind
            // pop_front — a node that didn't propose this digest won't contain it
            // (no-op), and we must never discard a different still-pending frame.
            {
                let mut queue = self.pending.lock().expect("pending queue poisoned");
                if let Some(pos) = queue.iter().position(|d| *d == digest) {
                    queue.remove(pos);
                }
            }
            // buffer for the async drain, keyed by view (agreed order), deduped.
            self.inbox.deliver(view, digest, &self.store);
        }
        Feedback::Ok
    }
}

// ============================================================================
// THE ORDERER — the one new `node::Orderer` impl.
// ============================================================================

/// the real commonware-simplex [`node::Orderer`]. `submit` stages a frame +
/// queues its digest for this node's proposals; a live simplex `Engine` (started
/// in [`SimplexOrderer::spawn`], kept alive by the `_engine` handle) BFT-orders
/// it; `poll_delivered` non-blocking-drains the finalized frames in ascending-
/// view (agreed) order. concrete (non-generic) so the `Orderer` impl is clean —
/// the engine's scheme/context generics live only in `spawn`.
pub struct SimplexOrderer {
    handle: ConsensusHandle,
    inbox: FinalizedInbox,
    /// the engine task keepalive: dropping the orderer aborts its engine.
    _engine: commonware_runtime::Handle<()>,
}

impl node::Orderer for SimplexOrderer {
    fn submit(
        &mut self,
        frame: Vec<u8>,
    ) -> impl std::future::Future<Output = Result<(), node::Error>> {
        // store.put + enqueue digest; NO local apply. never actually suspends —
        // it just satisfies the async seam (same shape as RoundOrderer).
        let handle = self.handle.clone();
        async move {
            handle.submit(frame);
            Ok(())
        }
    }

    fn poll_delivered(&mut self) -> Vec<Vec<u8>> {
        // drain the finalization buffer in ascending-view order (BTreeMap).
        self.inbox.drain()
    }
}

// ============================================================================
// the engine builder — stand up a live simplex Engine wired to this orderer.
// ============================================================================

impl SimplexOrderer {
    /// stand up a LIVE simplex [`Engine`](commonware_consensus::simplex::Engine)
    /// on `context` and wire it to a fresh [`SimplexOrderer`]: the automaton
    /// peeks this node's pending FIFO, the reporter buffers finalized frames into
    /// the returned orderer's inbox, and the returned orderer's `submit` stages +
    /// enqueues onto the very FIFO the automaton peeks — all over the shared
    /// `store`.
    ///
    /// concrete over the simplex ed25519 [`Scheme`](commonware_consensus::simplex::
    /// scheme::ed25519::Scheme) (NOT the mocks-gated `fixture`, so this compiles
    /// without the `mocks` feature); generic over the runtime `context` E, the
    /// `blocker` B, and the three engine channel pairs (forwarded to
    /// `engine.start`). config is the tuned legacy default. the engine's keepalive
    /// handle lives inside the returned orderer — dropping it aborts the engine.
    #[allow(clippy::too_many_arguments)]
    fn build<E, B, R, VS, VR, CS, CR, RS, RR>(
        context: E,
        scheme: commonware_consensus::simplex::scheme::ed25519::Scheme,
        blocker: B,
        partition: String,
        epoch: commonware_consensus::types::Epoch,
        genesis: Digest,
        store: ContentStore,
        relay: R,
        vote: (VS, VR),
        certificate: (CS, CR),
        resolver: (RS, RR),
    ) -> Self
    where
        E: commonware_runtime::Spawner
            + commonware_runtime::Clock
            + commonware_runtime::Storage
            + commonware_runtime::Metrics
            + commonware_runtime::BufferPooler
            + rand_core::CryptoRngCore
            + Send
            + Sync
            + 'static,
        B: commonware_p2p::Blocker<PublicKey = commonware_cryptography::ed25519::PublicKey>,
        R: Relay<
                Digest = Digest,
                PublicKey = commonware_cryptography::ed25519::PublicKey,
                Plan = Plan<commonware_cryptography::ed25519::PublicKey>,
            > + Send + 'static,
        VS: commonware_p2p::Sender<PublicKey = commonware_cryptography::ed25519::PublicKey>,
        VR: commonware_p2p::Receiver<PublicKey = commonware_cryptography::ed25519::PublicKey>,
        CS: commonware_p2p::Sender<PublicKey = commonware_cryptography::ed25519::PublicKey>,
        CR: commonware_p2p::Receiver<PublicKey = commonware_cryptography::ed25519::PublicKey>,
        RS: commonware_p2p::Sender<PublicKey = commonware_cryptography::ed25519::PublicKey>,
        RR: commonware_p2p::Receiver<PublicKey = commonware_cryptography::ed25519::PublicKey>,
    {
        use commonware_consensus::simplex::{
            config::{Config as SimplexConfig, Floor, ForwardingPolicy},
            elector::RoundRobin,
            Engine,
        };
        use commonware_consensus::types::ViewDelta;
        use commonware_cryptography::{ed25519, Sha256};
        use commonware_parallel::Sequential;
        use commonware_runtime::buffer::paged::CacheRef;
        use commonware_utils::{NZUsize, NZU16};
        use std::time::Duration;

        // this validator's consensus triple over the ONE shared store: the
        // automaton peeks the FIFO, the submit handle pushes onto it, the reporter
        // removes on finalization and buffers into the inbox we return.
        let automaton = ConsensusAutomaton::<ed25519::PublicKey>::new();
        let handle = automaton.handle(store.clone());
        let inbox = FinalizedInbox::new();
        let reporter = SimplexReporter::<
            commonware_consensus::simplex::scheme::ed25519::Scheme,
        >::new(store.clone(), automaton.pending(), inbox.clone());

        // page cache borrows the pooler context BEFORE we hand a child to Engine.
        let page_cache = CacheRef::from_pooler(&context, NZU16!(1024), NZUsize!(10));

        let cfg = SimplexConfig {
            scheme,
            elector: RoundRobin::<Sha256>::default(),
            blocker,
            automaton,
            relay,
            reporter,
            strategy: Sequential,
            partition,
            mailbox_size: NZUsize!(1024),
            epoch,
            floor: Floor::Genesis(genesis),
            leader_timeout: Duration::from_secs(1),
            certification_timeout: Duration::from_secs(2),
            timeout_retry: Duration::from_secs(10),
            fetch_timeout: Duration::from_secs(1),
            activity_timeout: ViewDelta::new(10),
            skip_timeout: ViewDelta::new(5),
            fetch_concurrent: NZUsize!(4),
            replay_buffer: NZUsize!(1024 * 1024),
            write_buffer: NZUsize!(1024 * 1024),
            page_cache,
            forwarding: ForwardingPolicy::Disabled,
        };

        let engine = Engine::new(context.child("engine"), cfg);
        // KEEP the handle alive inside the orderer — dropping it aborts the engine.
        let engine_handle = engine.start(vote, certificate, resolver);

        SimplexOrderer { handle, inbox, _engine: engine_handle }
    }

    /// stand up a live simplex engine with a [`NoopRelay`] — the in-process-sim
    /// path where ONE [`ContentStore`] is cloned into every validator, so there is
    /// nothing to disseminate. signature UNCHANGED from before the relay split, so
    /// the in-sim proof calls this untouched.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn<E, B, VS, VR, CS, CR, RS, RR>(
        context: E,
        scheme: commonware_consensus::simplex::scheme::ed25519::Scheme,
        blocker: B,
        partition: String,
        epoch: commonware_consensus::types::Epoch,
        genesis: Digest,
        store: ContentStore,
        vote: (VS, VR),
        certificate: (CS, CR),
        resolver: (RS, RR),
    ) -> Self
    where
        E: commonware_runtime::Spawner
            + commonware_runtime::Clock
            + commonware_runtime::Storage
            + commonware_runtime::Metrics
            + commonware_runtime::BufferPooler
            + rand_core::CryptoRngCore
            + Send
            + Sync
            + 'static,
        B: commonware_p2p::Blocker<PublicKey = commonware_cryptography::ed25519::PublicKey>,
        VS: commonware_p2p::Sender<PublicKey = commonware_cryptography::ed25519::PublicKey>,
        VR: commonware_p2p::Receiver<PublicKey = commonware_cryptography::ed25519::PublicKey>,
        CS: commonware_p2p::Sender<PublicKey = commonware_cryptography::ed25519::PublicKey>,
        CR: commonware_p2p::Receiver<PublicKey = commonware_cryptography::ed25519::PublicKey>,
        RS: commonware_p2p::Sender<PublicKey = commonware_cryptography::ed25519::PublicKey>,
        RR: commonware_p2p::Receiver<PublicKey = commonware_cryptography::ed25519::PublicKey>,
    {
        let relay = NoopRelay::<commonware_cryptography::ed25519::PublicKey>::new();
        Self::build(
            context, scheme, blocker, partition, epoch, genesis, store, relay, vote,
            certificate, resolver,
        )
    }

    /// stand up a live simplex engine with a real [`ConsensusRelay`] over a
    /// PER-PROCESS [`ContentStore`] — the real-socket path. `payload` is a
    /// dedicated p2p channel pair: at propose time the relay gossips the proposed
    /// frame's bytes to all peers on its sender, and a STORE-ONLY drain
    /// ([`spawn_payload_drain`]) caches every peer-relayed frame into `store` from
    /// its receiver. a peer thereby holds the bytes behind a digest it never
    /// originated, so when that digest finalizes its reporter resolves and delivers
    /// it — in BFT order, via the SAME finalization arm the proposer uses.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_with_relay<E, B, PS, PR, VS, VR, CS, CR, RS, RR>(
        context: E,
        scheme: commonware_consensus::simplex::scheme::ed25519::Scheme,
        blocker: B,
        partition: String,
        epoch: commonware_consensus::types::Epoch,
        genesis: Digest,
        store: ContentStore,
        vote: (VS, VR),
        certificate: (CS, CR),
        resolver: (RS, RR),
        payload: (PS, PR),
    ) -> Self
    where
        E: commonware_runtime::Spawner
            + commonware_runtime::Clock
            + commonware_runtime::Storage
            + commonware_runtime::Metrics
            + commonware_runtime::BufferPooler
            + rand_core::CryptoRngCore
            + Send
            + Sync
            + 'static,
        B: commonware_p2p::Blocker<PublicKey = commonware_cryptography::ed25519::PublicKey>,
        PS: commonware_p2p::Sender<PublicKey = commonware_cryptography::ed25519::PublicKey>
            + Clone
            + Send
            + Sync
            + 'static,
        PR: commonware_p2p::Receiver<PublicKey = commonware_cryptography::ed25519::PublicKey>
            + Send
            + 'static,
        VS: commonware_p2p::Sender<PublicKey = commonware_cryptography::ed25519::PublicKey>,
        VR: commonware_p2p::Receiver<PublicKey = commonware_cryptography::ed25519::PublicKey>,
        CS: commonware_p2p::Sender<PublicKey = commonware_cryptography::ed25519::PublicKey>,
        CR: commonware_p2p::Receiver<PublicKey = commonware_cryptography::ed25519::PublicKey>,
        RS: commonware_p2p::Sender<PublicKey = commonware_cryptography::ed25519::PublicKey>,
        RR: commonware_p2p::Receiver<PublicKey = commonware_cryptography::ed25519::PublicKey>,
    {
        let (payload_sender, payload_receiver) = payload;
        // store-ONLY drain: cache peer-relayed frames into THIS process's store.
        spawn_payload_drain(context.child("payload_drain"), payload_receiver, store.clone());
        let relay = ConsensusRelay::<PS, commonware_cryptography::ed25519::PublicKey>::new(
            payload_sender,
            store.clone(),
        );
        Self::build(
            context, scheme, blocker, partition, epoch, genesis, store, relay, vote,
            certificate, resolver,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_store_round_trips_by_digest() {
        let store = ContentStore::new();
        let bytes = b"a queued frame".to_vec();
        let digest = store.put(bytes.clone());
        assert_eq!(digest, digest_of(&bytes));
        assert_eq!(store.get(&digest), Some(bytes));
        assert_eq!(store.get(&digest_of(b"unseen")), None);
    }

    #[test]
    fn finalized_inbox_drains_in_ascending_view_order() {
        // the reporter observes finalizations ascending per node; the BTreeMap
        // keeps a single drain in that order even if inserts arrive out of order.
        let store = ContentStore::new();
        let d_lo = store.put(b"view 1".to_vec());
        let d_hi = store.put(b"view 2".to_vec());
        let inbox = FinalizedInbox::new();
        // insert the HIGHER view first — the drain must still yield lo before hi.
        inbox.deliver(2, d_hi, &store);
        inbox.deliver(1, d_lo, &store);
        assert_eq!(inbox.drain(), vec![b"view 1".to_vec(), b"view 2".to_vec()]);
        // a second drain is empty (take semantics).
        assert!(inbox.drain().is_empty());
    }

    #[test]
    fn finalized_inbox_is_exactly_once_per_digest() {
        // a re-finalization race must not double-apply a frame.
        let store = ContentStore::new();
        let d = store.put(b"once".to_vec());
        let inbox = FinalizedInbox::new();
        inbox.deliver(1, d, &store);
        inbox.deliver(1, d, &store); // same digest again -> ignored by `seen`.
        assert_eq!(inbox.drain(), vec![b"once".to_vec()]);
    }

    #[test]
    fn propose_peeks_so_a_nullified_view_can_repropose() {
        // the load-bearing guard for the peek-not-pop fix in `propose`.
        //
        // a proposed view that NULLIFIES (never reaches quorum) must not lose the
        // queued frame — the engine just calls `propose` again next time this node
        // leads. driving `propose` twice with NO finalization between models that
        // nullify-then-re-lead path. with peek both calls yield the same digest;
        // the old `pop_front` would find an empty queue on the second call, drop
        // its sender, and the receiver resolves to `Err` — the lane stalls forever.
        // removal happens at one place only: `SimplexReporter` on finalization,
        // which this test deliberately never triggers.
        use commonware_consensus::simplex::types::Context;
        use commonware_consensus::types::{Epoch, Round, View};
        use commonware_cryptography::ed25519::PrivateKey;
        use commonware_cryptography::Signer as _;
        use commonware_runtime::{deterministic, Runner};

        let executor = deterministic::Runner::timed(std::time::Duration::from_secs(5));
        executor.start(|_context| async move {
            let store = ContentStore::new();
            let digest = store.put(b"queued frame".to_vec());

            let mut automaton =
                ConsensusAutomaton::<commonware_cryptography::ed25519::PublicKey>::new();
            automaton.enqueue(digest);

            let leader = PrivateKey::from_seed(0).public_key();
            let context = || Context {
                round: Round::new(Epoch::new(0), View::new(1)),
                leader: leader.clone(),
                parent: (View::new(0), digest),
            };

            let first = automaton
                .propose(context())
                .await
                .await
                .expect("first propose yields the queued digest");
            assert_eq!(first, digest, "propose should offer the queued frame");

            // that view nullified — no finalization fired, nothing was removed.
            // lead again: the SAME digest must still be proposable.
            let second = automaton
                .propose(context())
                .await
                .await
                .expect("a nullified view keeps the frame queued — re-propose succeeds");
            assert_eq!(second, digest, "peek must keep the frame proposable after a nullified view");
        });
    }
}
