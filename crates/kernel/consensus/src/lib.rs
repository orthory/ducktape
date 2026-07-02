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

use std::collections::{HashSet, HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use commonware_actor::Feedback;
use commonware_codec::{Decode as _, Encode as _};
use commonware_consensus::{
    simplex::{
        types::{Activity, Context, Finalization},
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

use bytes::Bytes;
use commonware_cryptography::Signer as _;
use commonware_cryptography::bls12381::primitives::variant::MinPk;
use commonware_cryptography::bls12381::primitives::{ops as bls_ops, variant as bls_variant};
use commonware_resolver::p2p::{
    Config as ResolverConfig, Engine as ResolverEngine, Mailbox as ResolverMailbox,
    Producer as ResolverProducer,
};
use commonware_resolver::{Consumer as ResolverConsumer, Delivery, Resolver as _};
use commonware_utils::TryCollect as _;
use commonware_utils::ordered::BiMap;
use rand_core::SeedableRng as _;

mod valset_orchestrator;
pub use valset_orchestrator::{
    EpochMembership, ObservationOutcome, ObservedValset, RespawnPlan, ScheduledCutover,
    ValsetOrchestrator, ValsetRoot,
};

/// the concrete digest the consensus lane orders over: a sha256 of the frame
/// bytes. fixing it here lets the [`ContentStore`] key on a plain `Copy` type.
pub type Digest = sha256::Digest;

/// the resolver mailbox this node's reporter fetches missing finalized payloads
/// through — keyed by [`Digest`], over ed25519 peers, no subscribers.
type PayloadMailbox = ResolverMailbox<Digest, commonware_cryptography::ed25519::PublicKey, ()>;

/// the versioned consensus signature / certificate scheme — a GENESIS-WIDE constant
/// every validator must agree on (it domain-separates the simplex scheme + certificates;
/// a mismatch means engines never agree and the mesh hangs).
///
/// # variants
/// - [`V1Ed25519`](ConsensusScheme::V1Ed25519) — the DEFAULT. each validator signs with
///   its own ed25519 key; a certificate is a COLLECTION of ed25519 signatures, so cert
///   size (and verification cost) grows linearly with the validator set.
/// - [`V2Bls`](ConsensusScheme::V2Bls) — bls12381 MULTISIG over the MinPk variant
///   (48-byte bls public keys). quorum votes aggregate into ONE bls signature per
///   certificate (plus a signer-index bitmap), so cert size stays essentially FLAT as
///   the set grows — the reason to migrate at scale. still attributable: the signer
///   indices ride along, so per-validator liveness/fault evidence keeps working. the
///   scheme is DUAL-KEY: ed25519 remains the transport/p2p IDENTITY everywhere (peer
///   ordering, discovery, blocking); the bls key ONLY signs votes/certificates.
///   deliberately NOT the bls threshold schemes — those need DKG/resharing, which
///   fights the epoch teardown-respawn contract below.
///
/// # rogue-key / proof-of-possession (V2)
/// naive bls aggregation is rogue-key-attackable: a registrant that can choose its
/// public key as a function of the others' can forge aggregate signatures. in this
/// slice the (identity key -> bls key) map fed to the scheme is TRUSTED input from
/// config/genesis, so no proof-of-possession check is performed here. PoP verification
/// lands with valset membership authentication — a join must prove knowledge of its bls
/// secret before its key ever enters the map.
///
/// # the rekey / respawn contract (read before wiring V2 or dynamic validators)
/// the scheme AND the validator set are fixed at simplex `Engine` construction — neither
/// can be hot-swapped in a running engine. changing EITHER (a scheme migration, or a
/// validator join/leave) requires an **epoch transition**: at a height the OLD engine
/// finalizes, every validator tears down the current engine and RE-SPAWNS a new one with
/// the new `(scheme, participants)`. finalizing the switch through the old engine FIRST is
/// what makes every node cut over at the SAME point (else they fork). this one
/// teardown-and-respawn mechanism backs both BLS migration and dynamic valset. the same
/// epoch boundary is where validator-owned transport membership rotates: bootnodes,
/// relayers, and control participants must be derived from that epoch's validator set,
/// not from a static external relay.
///
/// # implementation note
/// [`SimplexOrderer`]'s spawn fns are GENERIC over the simplex scheme `S` (with
/// `S::PublicKey` pinned to ed25519 — the transport identity), and the orderer itself is
/// scheme-erased. so selecting a variant is purely a construction-time choice: build the
/// matching scheme value (`simplex::scheme::ed25519::Scheme::signer` for V1,
/// [`BlsScheme::signer`] / [`bls_dev_scheme`] for V2) and hand it to the same spawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConsensusScheme {
    /// per-validator ed25519 signatures; certificates are collections of them.
    #[default]
    V1Ed25519,
    /// dual-key bls12381 multisig (MinPk): ed25519 identity, bls votes, ONE aggregated
    /// signature (+ signer indices) per certificate.
    V2Bls,
}

// ============================================================================
// the V2Bls scheme surface — dual-key (ed25519 identity, bls12381 signing).
// ============================================================================

/// the V2 simplex scheme: bls12381 multisig over the MinPk variant, keyed by ed25519
/// IDENTITY keys — the dual-key shape. participant ORDER (and so signer indices, leader
/// rotation, and the certificate bitmap) comes from the sorted ed25519 identity keys,
/// exactly like V1; only the vote/certificate signatures are bls. constructed via
/// `BlsScheme::signer(namespace, participants, secret)` /
/// `BlsScheme::verifier(namespace, participants)` where `participants` is the
/// (identity key -> bls key) [`BiMap`](commonware_utils::ordered::BiMap) — TRUSTED
/// config/genesis input in this slice (see the rogue-key section on
/// [`ConsensusScheme`]).
pub type BlsScheme = commonware_consensus::simplex::scheme::bls12381_multisig::Scheme<
    commonware_cryptography::ed25519::PublicKey,
    MinPk,
>;

/// a V2 bls signing (private) key — the scalar behind a validator's vote signatures.
pub type BlsPrivateKey = commonware_cryptography::bls12381::primitives::group::Private;

/// a V2 bls public key (MinPk: 48 bytes) — the `values` side of the participant BiMap.
pub type BlsPublicKey = <MinPk as bls_variant::Variant>::Public;

/// a V2 certificate: ONE aggregated bls signature + the signer-index bitmap.
pub type BlsCertificate =
    commonware_cryptography::bls12381::certificate::multisig::Certificate<MinPk>;

/// derive a validator's V2 bls signing secret from its DEV seed — the bls analog of
/// `ed25519::PrivateKey::from_seed` (INSECURE; examples/tests/dev config only). the
/// chacha seed is sha256-domain-separated from the ed25519 derivation so the two dev
/// keys never share key material even for the same seed value.
pub fn bls_dev_secret(seed: u64) -> BlsPrivateKey {
    let mut hasher = Sha256::default();
    hasher.update(b"ducktape:consensus:bls12381:dev-seed:v1:");
    hasher.update(&seed.to_be_bytes());
    let digest = hasher.finalize();
    let mut chacha_seed = [0u8; 32];
    chacha_seed.copy_from_slice(digest.as_ref());
    let mut rng = rand_chacha::ChaCha20Rng::from_seed(chacha_seed);
    bls_ops::keypair::<_, MinPk>(&mut rng).0
}

/// the bls public key for a DEV seed — the counterpart of [`bls_dev_secret`], used to
/// build the participant BiMap from peer seed lists.
pub fn bls_dev_public(seed: u64) -> BlsPublicKey {
    bls_ops::compute_public::<MinPk>(&bls_dev_secret(seed))
}

/// build a V2 signer over the DEV validator set `seeds` for `my_seed` — the bls analog
/// of the V1 dev path `simplex_ed25519::Scheme::signer(ns, participants, from_seed(id))`
/// (bin/node, tests). pairs every seed's ed25519 IDENTITY key with its derived bls
/// signing key into the participant BiMap; `None` when `my_seed`'s bls key is not in
/// the set (or `seeds` contains duplicates). the pairs are TRUSTED input here — see the
/// rogue-key / proof-of-possession section on [`ConsensusScheme`].
pub fn bls_dev_scheme(namespace: &[u8], seeds: &[u64], my_seed: u64) -> Option<BlsScheme> {
    let participants: BiMap<commonware_cryptography::ed25519::PublicKey, BlsPublicKey> = seeds
        .iter()
        .map(|s| {
            (
                commonware_cryptography::ed25519::PrivateKey::from_seed(*s).public_key(),
                bls_dev_public(*s),
            )
        })
        .try_collect()
        .ok()?;
    BlsScheme::signer(namespace, participants, bls_dev_secret(my_seed))
}

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

/// cap on CACHED (non-pinned) entries. everything that arrives from a peer —
/// eager relay gossip, resolver fetches — is best-effort cache, and a byzantine
/// peer can flood that lane with garbage blobs forever, so it must be bounded:
/// past the cap the OLDEST cached entry is evicted FIFO. own submissions are
/// PINNED instead (never evicted) until finalization demotes them, so this
/// node's proposals always resolve locally and always remain servable to a
/// fetching peer while in flight. sizing: worst case cap × max payload
/// (1 MiB on the node's mesh) bounds cache memory; ops are typically small
/// json frames, so in practice this holds thousands of blocks of history for
/// peers catching up. a peer that has fallen further behind than the cache
/// window must rebuild through module state sync, not per-op fetch.
pub const PAYLOAD_CACHE_CAP: usize = 16_384;

/// digest->bytes map: resolves the opaque digests simplex finalizes back into
/// the frame bytes the host applies. cloning shares the backing store (`Arc`),
/// so the automaton, reporter, and submit handle all hold the SAME content — the
/// blessed in-process-sim shortcut (one store cloned into every validator).
///
/// two retention classes:
/// - PINNED — this node's own submissions ([`ContentStore::pin`]): must survive
///   until finalized (the automaton re-proposes them across nullified views, and
///   peers resolve them via fetch), so they are exempt from eviction. the
///   reporter demotes a digest to cached on finalization.
/// - CACHED — peer-relayed / fetched bytes ([`ContentStore::put`]): best-effort,
///   FIFO-bounded at [`PAYLOAD_CACHE_CAP`]. content-addressing keeps a flood
///   inert for CORRECTNESS (garbage can never match a finalized digest); the cap
///   keeps it inert for MEMORY.
#[derive(Clone, Default)]
pub struct ContentStore {
    inner: Arc<Mutex<StoreInner>>,
}

#[derive(Default)]
struct StoreInner {
    /// own in-flight submissions — never evicted; demoted on finalization.
    pinned: HashMap<Digest, Vec<u8>>,
    /// best-effort cache, FIFO-bounded by `order` at [`PAYLOAD_CACHE_CAP`].
    cached: HashMap<Digest, Vec<u8>>,
    /// insertion order of `cached` keys — the FIFO eviction queue.
    order: VecDeque<Digest>,
}

impl StoreInner {
    fn insert_cached(&mut self, digest: Digest, bytes: Vec<u8>) {
        // an entry we already hold (either class) keeps its place — re-inserting
        // would double it into the eviction queue and skew the FIFO window.
        if self.pinned.contains_key(&digest) || self.cached.contains_key(&digest) {
            return;
        }
        self.cached.insert(digest, bytes);
        self.order.push_back(digest);
        while self.cached.len() > PAYLOAD_CACHE_CAP {
            // pop until an entry still live in `cached` is found: `order` may
            // carry keys a demote raced in (harmless — each pop shrinks it).
            if let Some(old) = self.order.pop_front() {
                self.cached.remove(&old);
            } else {
                break;
            }
        }
    }
}

impl ContentStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// PIN `bytes` under their content-address: this node's own submission,
    /// exempt from cache eviction until [`ContentStore::demote`] on
    /// finalization. called on the `submit` path before the digest is proposed.
    pub fn pin(&self, bytes: Vec<u8>) -> Digest {
        let digest = digest_of(&bytes);
        let mut inner = self.inner.lock().expect("content store poisoned");
        // already cached (e.g. a peer relayed our identical frame first): lift
        // it into the pinned class so eviction can no longer drop it.
        inner.cached.remove(&digest);
        inner.pinned.insert(digest, bytes);
        digest
    }

    /// CACHE `bytes` under their content-address (best-effort, FIFO-bounded).
    /// the peer-facing intake: the eager payload drain and the resolver's fetch
    /// consumer land here. re-hashing the bytes as the key IS the verification —
    /// byzantine garbage stores under its own hash and can never match a
    /// finalized digest.
    pub fn put(&self, bytes: Vec<u8>) -> Digest {
        let digest = digest_of(&bytes);
        let mut inner = self.inner.lock().expect("content store poisoned");
        inner.insert_cached(digest, bytes);
        digest
    }

    /// demote a PINNED digest into the bounded cache — called by the reporter
    /// once the digest FINALIZES: the automaton will never re-propose it (the
    /// pending FIFO dropped it) and local apply holds its bytes in the ordered
    /// gate, so the only remaining reader is a peer fetching recent history,
    /// which the cache window serves. a digest that was never pinned is a no-op.
    pub fn demote(&self, digest: &Digest) {
        let mut inner = self.inner.lock().expect("content store poisoned");
        if let Some(bytes) = inner.pinned.remove(digest) {
            inner.insert_cached(*digest, bytes);
        }
    }

    /// look up the bytes for a digest — pinned first, then cache. `None` means
    /// this node never saw the payload (or the cache window evicted it); a real
    /// node fetches through the resolver, a deep joiner uses module state sync.
    pub fn get(&self, digest: &Digest) -> Option<Vec<u8>> {
        let inner = self.inner.lock().expect("content store poisoned");
        inner
            .pinned
            .get(digest)
            .or_else(|| inner.cached.get(digest))
            .cloned()
    }

    /// count of PINNED entries (own in-flight submissions) — an ops/metrics
    /// surface: sustained growth means this node's proposals are not finalizing.
    pub fn pinned_len(&self) -> usize {
        self.inner.lock().expect("content store poisoned").pinned.len()
    }

    /// count of CACHED entries — bounded by [`PAYLOAD_CACHE_CAP`].
    pub fn cached_len(&self) -> usize {
        self.inner.lock().expect("content store poisoned").cached.len()
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
    /// stage `bytes` for consensus: content-address them into the store (PINNED
    /// — an own submission must survive any number of nullified views and stay
    /// servable to fetching peers until it finalizes) and queue that digest for
    /// proposal. the entire `submit` body — NO local apply.
    pub fn submit(&self, bytes: Vec<u8>) {
        let digest = self.store.pin(bytes);
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

/// drain a payload-gossip channel to a BLACK HOLE: receive every message and
/// DISCARD it. STARVES a node of the eager payload cache so every finalization it
/// did not originate misses the store and routes through the resolver fetch path
/// — the deterministic knob behind [`SimplexOrderer::spawn_with_resolver`]'s
/// `starve`. consuming (not dropping) the receiver keeps the channel from backing
/// up while still leaving the store cold.
fn spawn_blackhole_drain<E, R>(context: E, mut receiver: R)
where
    E: commonware_runtime::Spawner + Send + 'static,
    R: commonware_p2p::Receiver + Send + 'static,
{
    context.spawn(move |_ctx| async move {
        while receiver.recv().await.is_ok() {}
    });
}

// ============================================================================
// the lazy catch-up fetch — resolver Producer/Consumer over the ContentStore.
// ============================================================================

/// serves payload bytes by digest from the local [`ContentStore`] to peers that
/// fetch them over the resolver. clone shares the backing store (`Arc`). a local
/// miss drops the sender UNSENT — the resolver reads that as "no data" and retries
/// another peer, never a wrong payload under the digest.
#[derive(Clone)]
struct PayloadProducer {
    store: ContentStore,
}

impl ResolverProducer for PayloadProducer {
    type Key = Digest;

    fn produce(&mut self, key: Self::Key) -> oneshot::Receiver<Bytes> {
        let (tx, rx) = oneshot::channel();
        if let Some(bytes) = self.store.get(&key) {
            tx.send_lossy(Bytes::from(bytes));
        }
        rx
    }
}

/// receives fetched payload bytes, verifies the content address, stores them, and
/// FILLS the awaiting slot in the ordered gate ([`FinalizedInbox`]). this is the
/// async delivery seam a SYNC [`SimplexReporter::report`] can't drive: the reporter
/// only LOGS a missing finalized slot + issues the fetch; the bytes arrive HERE,
/// off that sync path, and complete the slot so the next `poll_delivered` releases
/// it in finalization order. content-addressing IS the verification — bytes that
/// do not hash to the requested digest resolve `false` (blocking the lying peer)
/// and touch neither the store nor the gate.
#[derive(Clone)]
struct PayloadConsumer {
    store: ContentStore,
    inbox: FinalizedInbox,
}

impl ResolverConsumer for PayloadConsumer {
    type Key = Digest;
    type Value = Bytes;
    type Subscriber = ();

    fn deliver(
        &mut self,
        delivery: Delivery<Self::Key, Self::Subscriber>,
        value: Self::Value,
    ) -> oneshot::Receiver<bool> {
        let (tx, rx) = oneshot::channel();
        let bytes: Vec<u8> = value.into();
        if digest_of(&bytes) == delivery.key {
            // verified: cache under the content address AND fill the gate slot the
            // reporter logged for this digest, so ordered release can advance.
            let digest = self.store.put(bytes.clone());
            self.inbox.fill_fetched(digest, bytes);
            tx.send_lossy(true);
        } else {
            // the bytes do NOT hash to the requested digest — a lying peer. `false`
            // tells the resolver to block it and retry another.
            tx.send_lossy(false);
        }
        rx
    }
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

/// the dense-index ordered-release gate. the SYNC reporter records each
/// finalization in ascending-view order onto `log`; a store HIT resolves its
/// bytes into `ready` at once (the eager path), a MISS leaves the slot AWAITING an
/// async resolver fetch that later calls [`FinalizedInbox::fill_fetched`]. `drain`
/// releases the LONGEST all-ready PREFIX from the queue's front, so a slot still
/// waiting on a fetch HALTS the prefix — everything behind it waits, never
/// dropped, never reordered. `submit_at` applies in call order and the qmdb root
/// is order-dependent, so this is what makes a fetched (late) op converge.
///
/// on an all-HIT (eager) node every slot is ready the instant it lands, so the
/// prefix is always the whole log: behavior is byte-identical to a take-all drain
/// and every existing eager-path suite stays green. `seen` makes `record`
/// exactly-once; `fill_fetched` is deliberately NOT seen-gated — it completes a
/// slot `record` already logged. release pops the slot off the queue, so release
/// is exactly-once and the log never grows past the unreleased window.
///
/// ## why `seen` is deliberately UNBOUNDED
///
/// `seen` is currently the replicated state machine's ONLY replay guard: the
/// frame codec carries `(origin, seq)` but nothing enforces seq monotonicity in
/// state, and simplex happily finalizes the same digest again if a peer
/// re-proposes byte-identical frame bytes years later. pruning `seen` would
/// reopen exactly that replay. the cost is one 32-byte digest per finalized op
/// for the process lifetime (~76 MB per 1M ops with set overhead) — acceptable
/// until replay protection moves where it belongs: a deterministic per-origin
/// nonce check in replicated state, at which point `seen` can shrink to a
/// re-finalization-race window.
#[derive(Default)]
struct FinalizedInner {
    /// UNRELEASED committed digests in finalization (ascending-view) order — the
    /// release order. released slots are popped off the front, so this only ever
    /// holds the awaiting window, not all history.
    log: VecDeque<(u64, Digest)>,
    /// resolved bytes per digest (store hit at `record`, or fetched later).
    /// entries leave on release, so this is bounded by the unreleased window.
    ready: HashMap<Digest, Vec<u8>>,
    /// exactly-once guard on `record` (NOT on `fill_fetched`) — and the replay
    /// guard for the whole ordered lane (see the type doc). unbounded on purpose.
    seen: HashSet<Digest>,
}

impl FinalizedInbox {
    pub fn new() -> Self {
        Self::default()
    }

    /// record a finalized `(view, digest)`. appends to the ordered log and, on a
    /// store HIT, resolves its bytes now. returns `true` when the slot is left
    /// AWAITING a fetch (a store miss WITH the resolver enabled) so the caller
    /// issues `resolver.fetch(digest)`. a miss WITHOUT a resolver drops the slot
    /// entirely (never logged) — exactly the old behavior. deduped by `seen`.
    fn record(
        &self,
        view: u64,
        digest: Digest,
        store: &ContentStore,
        resolver_enabled: bool,
    ) -> bool {
        let mut inner = self.inner.lock().expect("finalized inbox poisoned");
        if !inner.seen.insert(digest) {
            return false;
        }
        if let Some(bytes) = store.get(&digest) {
            inner.log.push_back((view, digest));
            inner.ready.insert(digest, bytes);
            false
        } else if resolver_enabled {
            // miss: log the slot so it holds its place; the async fetch fills it.
            inner.log.push_back((view, digest));
            true
        } else {
            // no resolver: nothing can ever resolve this digest — drop (old path).
            false
        }
    }

    /// complete an AWAITING slot with fetched bytes, off the sync reporter (from
    /// the resolver's `Consumer::deliver`). NOT seen-gated: `record` already logged
    /// the slot; this only supplies its bytes so the next `drain` can release it. a
    /// fill for a digest not (yet) logged simply waits in `ready` for its `record`.
    fn fill_fetched(&self, digest: Digest, bytes: Vec<u8>) {
        let mut inner = self.inner.lock().expect("finalized inbox poisoned");
        inner.ready.insert(digest, bytes);
    }

    /// count of UNRELEASED slots (the awaiting window) — an ops/metrics surface:
    /// sustained growth means a missing payload is halting the release prefix.
    pub fn unreleased_len(&self) -> usize {
        self.inner.lock().expect("finalized inbox poisoned").log.len()
    }

    /// release the longest all-ready PREFIX of the log, in finalization
    /// (ascending-view) order. a slot whose bytes have not resolved yet halts
    /// the prefix; each released slot is POPPED off the queue so every frame
    /// emits exactly once and the log stays bounded by the awaiting window.
    /// non-blocking.
    fn drain(&self) -> Vec<(u64, Vec<u8>)> {
        let mut inner = self.inner.lock().expect("finalized inbox poisoned");
        let mut out = Vec::new();
        while let Some(&(view, digest)) = inner.log.front() {
            match inner.ready.remove(&digest) {
                Some(bytes) => {
                    out.push((view, bytes));
                    inner.log.pop_front();
                }
                None => break,
            }
        }
        out
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
/// the newest finalization certificate the reporter has observed, shared with
/// the orderer: `(engine view, scheme-encoded Finalization bytes)`. a recovery
/// layer persists this once the app has drained everything at or below its
/// view; a restart then respawns the engine on it (`Floor::Finalized`), which
/// suppresses journal-replay re-reports below the floor — without it, a
/// reopened journal re-reports history into a fresh (empty) content store and
/// the ordered gate wedges awaiting bytes no peer may hold.
pub type LatestFinalization = Arc<Mutex<Option<(u64, Vec<u8>)>>>;

#[derive(Clone)]
pub struct SimplexReporter<S> {
    store: ContentStore,
    pending: Arc<Mutex<VecDeque<Digest>>>,
    inbox: FinalizedInbox,
    /// the shared latest-finalization slot (see [`LatestFinalization`]).
    latest_final: LatestFinalization,
    /// the catch-up fetch mailbox, `Some` only when a resolver engine is wired
    /// (see [`SimplexOrderer::spawn_with_resolver`]). on a finalization MISS the
    /// reporter fetches through this instead of dropping the frame.
    mailbox: Option<PayloadMailbox>,
    /// fetches the mailbox did NOT accept (endpoint closed / rejected): the
    /// awaiting gate slot would stall forever if the request were silently
    /// dropped, so they are retried at the next `report` call. bounded by the
    /// number of outstanding missing payloads.
    deferred_fetches: VecDeque<Digest>,
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
        mailbox: Option<PayloadMailbox>,
        latest_final: LatestFinalization,
    ) -> Self {
        Self {
            store,
            pending,
            inbox,
            latest_final,
            mailbox,
            deferred_fetches: VecDeque::new(),
            _marker: std::marker::PhantomData,
        }
    }

    /// issue (or re-issue) a payload fetch. an unaccepted submission
    /// ([`Feedback::accepted`] false — the resolver endpoint is closed or
    /// rejected the work) parks the digest for retry on the next `report` call
    /// instead of silently dropping it and stalling its gate slot forever.
    fn fetch_or_defer(&mut self, digest: Digest) {
        let Some(mailbox) = self.mailbox.as_mut() else {
            return;
        };
        if !mailbox.fetch(digest).accepted() {
            self.deferred_fetches.push_back(digest);
        }
    }
}

impl<S> Reporter for SimplexReporter<S>
where
    S: Scheme + 'static,
{
    type Activity = Activity<S, Digest>;

    fn report(&mut self, activity: Self::Activity) -> Feedback {
        // retry fetches a previous report failed to enqueue — a dropped fetch
        // would stall its awaiting gate slot (and the whole release prefix
        // behind it) forever.
        for _ in 0..self.deferred_fetches.len() {
            let Some(digest) = self.deferred_fetches.pop_front() else {
                break;
            };
            self.fetch_or_defer(digest);
        }
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
            // buffer for the async drain in ascending-view order (deduped). a
            // store HIT resolves NOW (the eager path, unchanged); a MISS with a
            // resolver enabled logs an AWAITING slot and we fetch the bytes —
            // moving delivery for the fetched case OFF this sync path into the
            // resolver's `Consumer::deliver`, which fills the slot.
            let need_fetch =
                self.inbox.record(view, digest, &self.store, self.mailbox.is_some());
            if need_fetch {
                self.fetch_or_defer(digest);
            }
            // finalized: this digest can never be re-proposed, so its pin (if it
            // was OUR submission) is released into the bounded cache — recent
            // history stays servable to fetching peers without growing forever.
            // ordered AFTER record: the gate captured its own copy from the pin,
            // so a flood-pressured cache eviction can no longer lose our own
            // finalized frame between demote and record.
            self.store.demote(&digest);
            // surface the certificate for the recovery layer (the respawn
            // floor). encoded here because only the reporter holds the typed
            // Finalization<S, _>.
            *self
                .latest_final
                .lock()
                .expect("latest finalization poisoned") =
                Some((view, finalization.encode().to_vec()));
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
    /// the shared latest-finalization slot (see [`LatestFinalization`]).
    latest_final: LatestFinalization,
    /// the engine task keepalive: dropping the orderer aborts its engine.
    _engine: commonware_runtime::Handle<()>,
    /// the payload-fetch resolver engine keepalive — `Some` only when built via
    /// [`SimplexOrderer::spawn_with_resolver`]. dropping it stops catch-up fetch.
    _resolver: Option<commonware_runtime::Handle<()>>,
}

impl SimplexOrderer {
    /// the newest finalization certificate the engine reported: `(engine
    /// view, scheme-encoded bytes)`. see [`LatestFinalization`] for why a
    /// recovery layer persists this.
    pub fn latest_finalization(&self) -> Option<(u64, Vec<u8>)> {
        self.latest_final
            .lock()
            .expect("latest finalization poisoned")
            .clone()
    }

    /// count of finalized slots not yet released by `poll_delivered`. a
    /// recovery layer persists a finalization floor only when this is 0 —
    /// read the certificate FIRST, then this: releases happen only on the
    /// caller's own drain thread, so a zero here proves everything reported
    /// before the certificate read has been released (and applied).
    pub fn unreleased_len(&self) -> usize {
        self.inbox.unreleased_len()
    }
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

    fn poll_delivered(&mut self) -> Vec<(u64, Vec<u8>)> {
        // drain the finalization buffer in ascending-view order (BTreeMap). the
        // view is the agreed block height the host stamps into `Env`.
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
    /// GENERIC over the simplex scheme `S` (the [`ConsensusScheme`] seam) with
    /// `S::PublicKey` pinned to ed25519 — the transport identity every p2p bound in
    /// this crate keys on; only the vote/certificate signatures vary by scheme. also
    /// generic over the runtime `context` E, the `blocker` B, and the three engine
    /// channel pairs (forwarded to `engine.start`). config is the tuned legacy
    /// default. the engine's keepalive handle lives inside the returned orderer —
    /// dropping it aborts the engine.
    #[allow(clippy::too_many_arguments)]
    fn build<E, S, B, R, VS, VR, CS, CR, RS, RR>(
        context: E,
        scheme: S,
        blocker: B,
        partition: String,
        epoch: commonware_consensus::types::Epoch,
        genesis: Digest,
        floor: Option<Finalization<S, Digest>>,
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
        S: commonware_consensus::simplex::scheme::Scheme<
                Digest,
                PublicKey = commonware_cryptography::ed25519::PublicKey,
            >,
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
        let latest_final = LatestFinalization::default();
        let reporter = SimplexReporter::<S>::new(
            store.clone(),
            automaton.pending(),
            inbox.clone(),
            None,
            latest_final.clone(),
        );

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
            // a RESTART respawn passes the persisted finalization floor: the
            // engine then skips journal-replay re-reports at or below it. a
            // fresh epoch starts from its genesis floor.
            floor: match floor {
                Some(finalization) => Floor::Finalized(finalization),
                None => Floor::Genesis(genesis),
            },
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

        SimplexOrderer { handle, inbox, latest_final, _engine: engine_handle, _resolver: None }
    }

    /// stand up a live simplex engine with a [`NoopRelay`] — the in-process-sim
    /// path where ONE [`ContentStore`] is cloned into every validator, so there is
    /// nothing to disseminate. signature UNCHANGED from before the relay split, so
    /// the in-sim proof calls this untouched.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn<E, S, B, VS, VR, CS, CR, RS, RR>(
        context: E,
        scheme: S,
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
        S: commonware_consensus::simplex::scheme::Scheme<
                Digest,
                PublicKey = commonware_cryptography::ed25519::PublicKey,
            >,
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
            context, scheme, blocker, partition, epoch, genesis, None, store, relay, vote,
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
    pub fn spawn_with_relay<E, S, B, PS, PR, VS, VR, CS, CR, RS, RR>(
        context: E,
        scheme: S,
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
        S: commonware_consensus::simplex::scheme::Scheme<
                Digest,
                PublicKey = commonware_cryptography::ed25519::PublicKey,
            >,
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
            context, scheme, blocker, partition, epoch, genesis, None, store, relay, vote,
            certificate, resolver,
        )
    }

    /// stand up a live simplex engine WITH the lazy [`commonware_resolver`]
    /// catch-up fetch backstop, over a PER-PROCESS [`ContentStore`]. this is the
    /// eager relay path of [`spawn_with_relay`] PLUS a second
    /// [`commonware_resolver::p2p::Engine`] on a dedicated `fetch` channel:
    ///
    /// - [`PayloadProducer`] serves this node's stored bytes by digest to peers
    ///   fetching them, and
    /// - [`PayloadConsumer`] receives fetched bytes, verifies the content address,
    ///   stores them, and FILLS the awaiting gate slot — the async delivery seam a
    ///   SYNC reporter can't drive.
    ///
    /// on a finalization the reporter resolves the bytes eagerly when the store
    /// already holds them (unchanged) and on a MISS fetches through the resolver
    /// instead of dropping. the ordered gate releases the fetched op ONLY in its
    /// finalization-order slot, so a node that missed the eager broadcast for some
    /// views still converges on the identical order-dependent root.
    ///
    /// `provider` gives the resolver its fetch candidates (in the sim,
    /// `oracle.manager()`); `me` excludes self from those candidates; `blocker`
    /// (cloned) blocks peers that serve garbage. `starve`, when true, BLACK-HOLES
    /// the eager payload drain so this node never caches a relayed payload — every
    /// finalization it did not originate misses and routes through the fetch path.
    /// it exists to exercise the miss/fetch/ordered-release path deterministically.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_with_resolver<E, S, B, D, PS, PR, FS, FR, VS, VR, CS, CR, RS, RR>(
        context: E,
        scheme: S,
        blocker: B,
        provider: D,
        me: commonware_cryptography::ed25519::PublicKey,
        partition: String,
        epoch: commonware_consensus::types::Epoch,
        genesis: Digest,
        floor: Option<Finalization<S, Digest>>,
        store: ContentStore,
        vote: (VS, VR),
        certificate: (CS, CR),
        resolver: (RS, RR),
        payload: (PS, PR),
        fetch: (FS, FR),
        starve: bool,
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
        S: commonware_consensus::simplex::scheme::Scheme<
                Digest,
                PublicKey = commonware_cryptography::ed25519::PublicKey,
            >,
        B: commonware_p2p::Blocker<PublicKey = commonware_cryptography::ed25519::PublicKey>
            + Clone,
        D: commonware_p2p::Provider<PublicKey = commonware_cryptography::ed25519::PublicKey>,
        PS: commonware_p2p::Sender<PublicKey = commonware_cryptography::ed25519::PublicKey>
            + Clone
            + Send
            + Sync
            + 'static,
        PR: commonware_p2p::Receiver<PublicKey = commonware_cryptography::ed25519::PublicKey>
            + Send
            + 'static,
        FS: commonware_p2p::Sender<PublicKey = commonware_cryptography::ed25519::PublicKey>,
        FR: commonware_p2p::Receiver<PublicKey = commonware_cryptography::ed25519::PublicKey>,
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

        let (payload_sender, payload_receiver) = payload;
        let (fetch_sender, fetch_receiver) = fetch;

        // eager drain: cache peer-relayed frames store-only — UNLESS starved, in
        // which case receive+discard so the store stays cold and every
        // non-originated finalization routes through the resolver fetch path.
        if starve {
            spawn_blackhole_drain(context.child("payload_starve"), payload_receiver);
        } else {
            spawn_payload_drain(context.child("payload_drain"), payload_receiver, store.clone());
        }

        let relay = ConsensusRelay::<PS, ed25519::PublicKey>::new(payload_sender, store.clone());

        // the consensus triple; its ordered gate `inbox` is SHARED with the
        // resolver's consumer so a fetched payload FILLS the exact slot the reporter
        // logged for that digest.
        let automaton = ConsensusAutomaton::<ed25519::PublicKey>::new();
        let handle = automaton.handle(store.clone());
        let inbox = FinalizedInbox::new();

        // the catch-up fetch engine: producer serves our store to peers; consumer
        // verifies + stores + fills the gate. short timeouts so a miss retries
        // quickly within the deterministic pump loop.
        let fetch_cfg = ResolverConfig {
            peer_provider: provider,
            blocker: blocker.clone(),
            consumer: PayloadConsumer { store: store.clone(), inbox: inbox.clone() },
            producer: PayloadProducer { store: store.clone() },
            mailbox_size: NZUsize!(1024),
            me: Some(me),
            initial: Duration::from_millis(100),
            timeout: Duration::from_millis(400),
            fetch_retry_timeout: Duration::from_millis(100),
            priority_requests: false,
            priority_responses: false,
        };
        let (fetch_engine, mailbox) =
            ResolverEngine::new(context.child("payload_fetch"), fetch_cfg);
        // KEEP the fetch handle alive inside the orderer — dropping it stops catch-up.
        let fetch_handle = fetch_engine.start((fetch_sender, fetch_receiver));

        let latest_final = LatestFinalization::default();
        let reporter = SimplexReporter::<S>::new(
            store.clone(),
            automaton.pending(),
            inbox.clone(),
            Some(mailbox),
            latest_final.clone(),
        );

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
            // restart respawn: the persisted floor suppresses journal-replay
            // re-reports at or below the already-applied boundary.
            floor: match floor {
                Some(finalization) => Floor::Finalized(finalization),
                None => Floor::Genesis(genesis),
            },
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
        let engine_handle = engine.start(vote, certificate, resolver);

        SimplexOrderer {
            handle,
            inbox,
            latest_final,
            _engine: engine_handle,
            _resolver: Some(fetch_handle),
        }
    }
}

/// decode a persisted finalization certificate back to the typed
/// [`Finalization`] a respawn floor needs, under `scheme`'s certificate codec
/// bounds. the counterpart of [`SimplexOrderer::latest_finalization`]'s
/// encoding; a decode failure means the persisted floor is damaged — callers
/// FAIL rather than silently fall back to a genesis floor, which would
/// resurrect the journal-replay wedge the floor exists to prevent.
pub fn decode_finalization<S>(scheme: &S, bytes: &[u8]) -> Result<Finalization<S, Digest>, String>
where
    S: Scheme,
{
    Finalization::<S, Digest>::decode_cfg(bytes, &scheme.certificate_codec_config())
        .map_err(|e| format!("persisted finalization floor does not decode: {e}"))
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
    fn cached_entries_evict_fifo_at_the_cap() {
        // the flood-memory bound: cached (peer-facing) inserts past the cap
        // evict the OLDEST cached entry; the newest CAP entries survive.
        let store = ContentStore::new();
        let first = store.put(b"blob-first".to_vec());
        for i in 0..PAYLOAD_CACHE_CAP {
            store.put(format!("blob-{i:08}").into_bytes());
        }
        assert_eq!(store.cached_len(), PAYLOAD_CACHE_CAP, "cache holds exactly the cap");
        assert_eq!(store.get(&first), None, "the oldest cached entry was evicted");
        let last = digest_of(format!("blob-{:08}", PAYLOAD_CACHE_CAP - 1).as_bytes());
        assert!(store.get(&last).is_some(), "the newest entry survives");
    }

    #[test]
    fn pinned_entries_survive_flood_pressure_and_demote_into_the_cache() {
        // an own submission must survive ANY amount of cached churn (it is the
        // only copy of an unfinalized proposal), and demote must move it into
        // the bounded cache once finalization retires it.
        let store = ContentStore::new();
        let own = store.pin(b"our proposed frame".to_vec());
        for i in 0..(PAYLOAD_CACHE_CAP + 64) {
            store.put(format!("flood-{i:08}").into_bytes());
        }
        assert_eq!(
            store.get(&own),
            Some(b"our proposed frame".to_vec()),
            "a pinned entry is exempt from cache eviction"
        );
        assert_eq!(store.pinned_len(), 1);

        store.demote(&own);
        assert_eq!(store.pinned_len(), 0, "demote releases the pin");
        assert_eq!(
            store.get(&own),
            Some(b"our proposed frame".to_vec()),
            "a freshly demoted entry is still servable from the cache"
        );
        // demoting an unknown digest is a no-op.
        store.demote(&digest_of(b"never pinned"));
    }

    #[test]
    fn duplicate_cache_puts_do_not_skew_the_fifo_window() {
        // re-relaying the same bytes (routine gossip duplication) must not
        // re-enter the eviction queue: the entry keeps its original place and
        // the cache length stays exact.
        let store = ContentStore::new();
        let d = store.put(b"dup".to_vec());
        for _ in 0..8 {
            store.put(b"dup".to_vec());
        }
        assert_eq!(store.cached_len(), 1);
        assert_eq!(store.get(&d), Some(b"dup".to_vec()));
    }

    #[test]
    fn released_slots_are_popped_so_the_gate_stays_bounded() {
        // the unreleased window is the ONLY thing the gate retains: after a
        // clean record+drain cycle the log is empty again, cycle after cycle.
        let store = ContentStore::new();
        let inbox = FinalizedInbox::new();
        for view in 0..1024u64 {
            let d = store.put(format!("op-{view}").into_bytes());
            inbox.record(view, d, &store, false);
            assert_eq!(inbox.unreleased_len(), 1, "one awaiting slot before drain");
            let released = inbox.drain();
            assert_eq!(released.len(), 1);
            assert_eq!(inbox.unreleased_len(), 0, "released slots are popped, not retained");
        }
    }

    #[test]
    fn finalized_inbox_releases_in_finalization_order() {
        // the reporter records finalizations in ascending view order; each is a
        // store HIT so the gate resolves it at once and releases the whole log as
        // one ready prefix. a second drain is empty (the cursor advanced).
        let store = ContentStore::new();
        let d_lo = store.put(b"view 1".to_vec());
        let d_hi = store.put(b"view 2".to_vec());
        let inbox = FinalizedInbox::new();
        // resolver disabled: both are hits, logged + ready immediately (no await).
        assert!(!inbox.record(1, d_lo, &store, false));
        assert!(!inbox.record(2, d_hi, &store, false));
        assert_eq!(inbox.drain(), vec![(1, b"view 1".to_vec()), (2, b"view 2".to_vec())]);
        assert!(inbox.drain().is_empty());
    }

    #[test]
    fn finalized_inbox_holds_a_missing_slot_until_its_fetch_fills() {
        // the ordered-release gate — the load-bearing convergence guard. a MISS at
        // view 1 (resolver enabled) HOLDS the whole prefix, even though view 2 is
        // ready, until the async fetch fills view 1's bytes. THEN both release, in
        // finalization order. this is what keeps a late/fetched op from applying
        // out of order (which would fork the order-dependent qmdb root).
        let store = ContentStore::new();
        let d1 = digest_of(b"view 1"); // NOT in the store: a missed eager broadcast.
        let d2 = store.put(b"view 2".to_vec());
        let inbox = FinalizedInbox::new();
        assert!(inbox.record(1, d1, &store, true), "miss + resolver -> awaiting (fetch)");
        assert!(!inbox.record(2, d2, &store, true), "hit -> ready, but held behind view 1");
        assert!(inbox.drain().is_empty(), "gate holds the prefix behind the missing slot");
        // the fetch resolves view 1's bytes (as the resolver's Consumer would).
        inbox.fill_fetched(d1, b"view 1".to_vec());
        assert_eq!(inbox.drain(), vec![(1, b"view 1".to_vec()), (2, b"view 2".to_vec())]);
        assert!(inbox.drain().is_empty());
    }

    #[test]
    fn payload_consumer_rejects_bytes_that_mismatch_the_fetched_digest() {
        // content-addressing IS the verification: a peer that returns bytes which
        // do NOT hash to the requested digest is rejected — `deliver` resolves
        // `false` (blocking it), the store is untouched, and the gate slot stays
        // unfilled. the matching bytes DO verify: stored, gate filled, releasable.
        use commonware_runtime::{deterministic, Runner};
        use commonware_utils::vec::NonEmptyVec;

        deterministic::Runner::timed(std::time::Duration::from_secs(5)).start(|_ctx| async move {
            let store = ContentStore::new();
            let inbox = FinalizedInbox::new();
            let mut consumer = PayloadConsumer { store: store.clone(), inbox: inbox.clone() };

            let key = digest_of(b"the real finalized frame");
            let tampered = Bytes::from_static(b"byzantine garbage");
            let bad = Delivery { key, subscribers: NonEmptyVec::new(()) };
            let valid = consumer.deliver(bad, tampered).await.expect("verdict");
            assert!(!valid, "a hash mismatch resolves false (blocks the lying peer)");
            assert_eq!(store.get(&key), None, "reject must not store the garbage");

            let good = b"the real finalized frame".to_vec();
            let dg = digest_of(&good);
            let ok = Delivery { key: dg, subscribers: NonEmptyVec::new(()) };
            let valid = consumer.deliver(ok, Bytes::from(good.clone())).await.expect("verdict");
            assert!(valid, "matching bytes verify (resolves true)");
            assert_eq!(store.get(&dg), Some(good.clone()), "accepted bytes are stored");
            // once the reporter logs this finalized slot, the filled bytes release.
            inbox.record(1, dg, &store, true);
            assert_eq!(inbox.drain(), vec![(1, good)], "the filled slot releases in order");
        });
    }

    #[test]
    fn finalized_inbox_is_exactly_once_per_digest() {
        // a re-finalization race must not double-apply a frame.
        let store = ContentStore::new();
        let d = store.put(b"once".to_vec());
        let inbox = FinalizedInbox::new();
        assert!(!inbox.record(1, d, &store, false));
        inbox.record(1, d, &store, false); // same digest again -> ignored by `seen`.
        assert_eq!(inbox.drain(), vec![(1, b"once".to_vec())]);
    }

    #[test]
    fn bls_dev_keys_are_deterministic_and_domain_separated_per_seed() {
        // dev key derivation must be a PURE function of the seed — every process
        // in a mesh derives the same participant map from the same peer seeds.
        assert_eq!(
            bls_dev_public(7),
            bls_dev_public(7),
            "same seed -> same bls key"
        );
        assert_ne!(
            bls_dev_public(7),
            bls_dev_public(8),
            "distinct seeds -> distinct keys"
        );
    }

    #[test]
    fn bls_dev_scheme_orders_participants_by_identity_key() {
        // the dual-key contract: participant ORDER (signer indices, leader
        // rotation, the certificate bitmap) comes from the sorted ed25519 IDENTITY
        // keys — byte-identical to V1's participant Set — regardless of the order
        // seeds appear in config.
        use commonware_cryptography::ed25519;
        use commonware_utils::ordered::Set;

        let seeds = [3u64, 0, 2];
        let scheme = bls_dev_scheme(b"ns", &seeds, 2).expect("seed 2 is a member");
        let expected: Set<ed25519::PublicKey> = Set::try_from(
            seeds
                .iter()
                .map(|s| ed25519::PrivateKey::from_seed(*s).public_key())
                .collect::<Vec<_>>(),
        )
        .expect("distinct dev keys");
        assert_eq!(
            scheme.participants(),
            &expected,
            "identity keys order the set"
        );

        // and this validator signs as EXACTLY its identity's slot in that order.
        let me = ed25519::PrivateKey::from_seed(2).public_key();
        assert_eq!(
            scheme.me().map(usize::from),
            expected.position(&me),
            "signer index == identity position"
        );

        // a seed outside the set cannot construct a signer.
        assert!(
            bls_dev_scheme(b"ns", &seeds, 9).is_none(),
            "non-member seed -> None"
        );
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
