//! commonware-backed transport: the broadcast lane over real p2p gossip.
//!
//! this satisfies the same [`transport::Transport`] seam the loopback impl does,
//! but instead of an in-process channel it speaks a real commonware p2p
//! dialect. the production path ([`CommonwareTransport::new`]) uses
//! `authenticated::discovery` — ed25519-identified, encrypted, fully-connected
//! peers with automatic address discovery. the transport itself is generic over
//! the gossip `Sender`, so tests drive it over the deterministic
//! `p2p::simulated` dialect via [`CommonwareTransport::from_channels`] (instant
//! links, no dial/handshake/discovery) — matching how commonware's own
//! consensus tests run multi-node sims.
//!
//! shape (mirrors the loopback contract so the convergence test can target it):
//! - [`CommonwareTransport::new`] is context-bound: commonware only lives inside
//!   a runtime context (`runner.start(|ctx| async {...})`), so unlike the sync
//!   `LoopbackHub::node()` this takes the runtime `context` and returns the
//!   transport handle plus an inbound `mpsc::Receiver<(Lane, Vec<u8>)>` — the
//!   exact tuple shape the trait's inbound side uses.
//! - `send(Lane::Broadcast, bytes)` gossips to ALL peers via
//!   `Sender::send(Recipients::All, ..)`. commonware's `Sender` is `Clone` and
//!   its `send` is synchronous, so we clone-per-send — no mutex, no actor, no
//!   guard held across an `.await`.
//! - `send(Lane::Consensus, bytes)` (p1.2, tier C): functional ordered gossip
//!   on a SECOND, dedicated p2p channel (`CHANNEL_CONSENSUS`). this gives the
//!   consensus lane real delivery semantics — every peer receives the bytes
//!   tagged `Lane::Consensus` — without (yet) BFT total-order guarantees. true
//!   byzantine-fault-tolerant ordering via commonware-simplex is the documented
//!   TODO: the [`consensus`] module holds the simplex Automaton/Relay/Reporter
//!   scaffolding (digest-addressed content store + finalization plumbing) ready
//!   to swap in once a simplex `Engine` is instantiated. until then the seam is
//!   a separate channel so the lane is FUNCTIONAL and distinguishable on the
//!   wire from broadcast.
//! - inbound: TWO spawned tasks, one per channel, each draining
//!   `Receiver::recv()` and forwarding `(peer, IoBuf)` into the inbound mpsc
//!   tagged with that channel's lane (`Broadcast` or `Consensus`).
//!
//! the whole thing is generic over the runtime context (the full bound set
//! commonware's `Network` requires) so it runs under `deterministic::Runner` in
//! tests AND a real tokio runtime in production.

pub mod consensus;
pub mod payload_fetch;

use std::future::Future;
use std::net::SocketAddr;

use commonware_cryptography::{ed25519, Signer};
use commonware_p2p::{
    authenticated::discovery::{self, Network},
    Manager, Recipients,
};
use commonware_runtime::{
    BufferPooler, Clock, Handle, IoBuf, Metrics, Network as RNetwork, Quota, Resolver, Spawner,
    Storage,
};
use commonware_utils::{ordered::Set, NZU32};
use op::Lane;
use rand_core::CryptoRngCore;
use tokio::sync::mpsc;
use transport::{Error, Inbound, Transport};

/// the p2p channel index we register the broadcast op-gossip stream on.
const CHANNEL_BROADCAST: u64 = 0;

/// the p2p channel index for the tier-C consensus GOSSIP lane (p1.2). a SEPARATE
/// channel from broadcast so the inbound drain can tag the lane correctly and so
/// consensus traffic is wire-distinguishable. still used by the gossip-lane path
/// ([`CommonwareTransport::from_channels`] and the hermetic tests). the live
/// simplex engine that [`CommonwareTransport::new`] now builds does NOT use this
/// single channel — it splits into the three dedicated channels below.
///
/// `allow(dead_code)`: since the engine flip, only the gossip-lane tests (built
/// on [`CommonwareTransport::from_channels`]) register this channel, so a non-test
/// build sees no use. it stays as the named protocol index for that still-public
/// path — `from_channels` callers register their own consensus channel against it.
#[allow(dead_code)]
const CHANNEL_CONSENSUS: u64 = 1;

/// the simplex engine's three sub-channels, registered by the production
/// [`CommonwareTransport::new`] path alongside [`CHANNEL_BROADCAST`]. the engine
/// consumes these positionally in `engine.start(vote, certificate, resolver)`;
/// they carry the BFT protocol traffic (individual votes, finalized
/// certificates, and request/response certificate fetches respectively) — NOT
/// op-batch payloads, which ride out-of-band through the [`consensus`]
/// `ContentStore`. distinct indices from broadcast since they share one network.
const CHANNEL_VOTE: u64 = 1;
const CHANNEL_CERTIFICATE: u64 = 2;
const CHANNEL_RESOLVER: u64 = 3;

/// the dedicated channel the live engine's
/// [`ConsensusRelay`](consensus::ConsensusRelay) gossips proposed op-batch
/// PAYLOADS on — the bytes behind a digest — so a non-proposer can resolve a
/// finalized digest it only learned through consensus. distinct from the
/// vote/cert/resolver protocol channels above: those carry BFT metadata, this
/// carries the application bytes. peers drain it STORE-ONLY (see
/// [`spawn_payload_drain`]) — never into the app inbound mpsc — so delivery stays
/// exclusively in the reporter's `Activity::Finalization` arm, in BFT order.
const CHANNEL_PAYLOAD: u64 = 4;

/// the peer-set index. all peers must `track` the same authorized set at the
/// same index for discovery's bit-vector gossip to line up.
const PEER_SET: u64 = 0;

/// max wire message size we'll accept on the gossip channel (1 MiB). op batches
/// are small json blobs, so this is generous headroom.
const MAX_MESSAGE_SIZE: u32 = 1 << 20;

/// inbound backlog before the channel applies backpressure on receive.
const MAX_BACKLOG: usize = 128;

/// the sender half commonware's discovery `register` hands back, specialized to
/// ed25519 identities and our runtime context `E`. this is the concrete `S` the
/// production authenticated path plugs into the generic transport.
type GossipSender<E> = discovery::Sender<ed25519::PublicKey, E>;

/// spawn a task that drains one registered channel's `Receiver`, forwarding
/// each `(peer, IoBuf)` into the shared inbound mpsc tagged with `lane`.
///
/// generic over the `Receiver` impl so both the broadcast and consensus
/// channels reuse the exact same drain loop — the only difference is which
/// `Lane` the bytes get stamped with. the task ends when either the channel
/// closes or the inbound consumer drops its receiver.
fn spawn_inbound_drain<E, R>(context: E, mut receiver: R, lane: Lane, in_tx: mpsc::Sender<Inbound>)
where
    E: Spawner + Send + 'static,
    R: commonware_p2p::Receiver + Send + 'static,
{
    context.spawn(move |_ctx| async move {
        while let Ok((_peer, msg)) = receiver.recv().await {
            let bytes: Vec<u8> = msg.into();
            if in_tx.send((lane, bytes)).await.is_err() {
                // consumer dropped the inbound receiver; nothing left to do.
                break;
            }
        }
    });
}

/// spawn a task that drains the payload-gossip channel ([`CHANNEL_PAYLOAD`]),
/// caching each received op-batch's bytes into the shared
/// [`ContentStore`](consensus::ContentStore) and doing NOTHING else.
///
/// this is how a non-proposer obtains the bytes behind a digest the leader's
/// [`ConsensusRelay`](consensus::ConsensusRelay) broadcast, so that when that
/// digest later finalizes the reporter's `store.get(&digest)` resolves and
/// delivers it — in BFT-agreed order, via the SAME finalization path the
/// proposer uses.
///
/// CRUCIAL — and the one real trap of this design: unlike [`spawn_inbound_drain`]
/// this NEVER forwards into the inbound app mpsc. emitting the batch here would
/// surface it to the application pre-finalization (out of BFT order, and then
/// AGAIN on finalization). a payload receipt does exactly one thing — populate
/// the store. `ContentStore::put` re-hashes the bytes as the key, so byzantine
/// garbage simply stores under its own hash and can never match a finalized
/// digest; content-addressing is the whole verification.
fn spawn_payload_drain<E, R>(context: E, mut receiver: R, store: consensus::ContentStore)
where
    E: Spawner + Send + 'static,
    R: commonware_p2p::Receiver + Send + 'static,
{
    context.spawn(move |_ctx| async move {
        while let Ok((_peer, msg)) = receiver.recv().await {
            let bytes: Vec<u8> = msg.into();
            // store-only: NO inbound forward. delivery is the reporter's job.
            store.put(bytes);
        }
    });
}

/// commonware-backed transport, generic over the broadcast gossip `Sender` `S`.
///
/// holds a clone-able broadcast gossip `Sender` plus a [`ConsensusLane`] for the
/// consensus side (either a gossip `Sender` of the same `S`, or a live-engine
/// submit handle). every field is cheap to clone (`Sender`s are `Clone`; the
/// engine handle is `Arc`-backed), which is why this can satisfy the `&self` +
/// `Send` shape of [`Transport::send`] without interior mutability.
///
/// the struct is decoupled from the p2p dialect: the production authenticated
/// path instantiates `S = GossipSender<E>` (see [`CommonwareTransport::new`]),
/// while tests instantiate `S = simulated::Sender<..>` via
/// [`CommonwareTransport::from_channels`] / [`CommonwareTransport::with_consensus_engine`].
#[derive(Clone)]
pub struct CommonwareTransport<S>
where
    S: commonware_p2p::Sender + Clone + Send + Sync + 'static,
{
    /// broadcast-lane gossip sender (CHANNEL_BROADCAST).
    broadcast: S,
    /// how the consensus lane carries bytes out — see [`ConsensusLane`].
    consensus: ConsensusLane<S>,
}

/// the two ways the consensus lane can ship an op-batch, chosen at construction;
/// [`Transport::send`] dispatches on it for [`Lane::Consensus`].
///
/// - [`ConsensusLane::Gossip`] — tier C: ordered best-effort gossip on
///   `CHANNEL_CONSENSUS`. functional delivery, NO BFT total order. this is what
///   the production [`CommonwareTransport::new`] path and [`from_channels`]
///   still build, so wiring the engine in is purely ADDITIVE — nothing that
///   constructs a `Gossip` transport changes behavior.
/// - [`ConsensusLane::Engine`] — tier A: stage the bytes into a [`ContentStore`]
///   and queue their digest for a live simplex
///   [`Engine`](commonware_consensus::simplex::Engine) to BFT-order, via the
///   [`ConsensusHandle`](consensus::ConsensusHandle) intake. the engine runs as
///   spawned tasks alongside; this lane only holds its (cheap, `Arc`-backed)
///   submit handle, so the whole transport stays `Clone`.
///
/// [`from_channels`]: CommonwareTransport::from_channels
#[derive(Clone)]
enum ConsensusLane<S> {
    Gossip(S),
    Engine(consensus::ConsensusHandle),
}

/// config knobs for standing up a node. addresses are plain `SocketAddr`s; the
/// authorized peer set and bootstrappers are the ed25519 public keys (and, for
/// bootstrappers, their dialable address) every node must agree on.
pub struct Config {
    /// deterministic seed for this node's ed25519 identity. in production this
    /// would come from a real key file; for sims a small integer is fine.
    pub seed: u64,
    /// application namespace — prevents cross-app handshake replay. all peers
    /// must share it.
    pub namespace: Vec<u8>,
    /// address to bind/listen on.
    pub listen: SocketAddr,
    /// address advertised to peers for dialing (often == listen).
    pub advertised: SocketAddr,
    /// the authorized peer set: every node's ed25519 public key, including this
    /// node's own. discovery's bit vectors are sorted over this set.
    pub peers: Vec<ed25519::PublicKey>,
    /// bootstrappers to dial on startup: (their public key, their address).
    pub bootstrappers: Vec<(ed25519::PublicKey, SocketAddr)>,
}

impl<S> CommonwareTransport<S>
where
    S: commonware_p2p::Sender + Clone + Send + Sync + 'static,
{
    /// wire a transport from already-registered channel pairs, decoupled from
    /// the p2p dialect.
    ///
    /// takes the broadcast and consensus `(sender, receiver)` pairs (whatever
    /// network minted them — discovery in production, simulated in tests),
    /// spawns one inbound drain per receiver into a shared inbound mpsc, and
    /// holds the two senders. returns the transport plus the inbound receiver
    /// (the same `(Lane, Vec<u8>)` tuple shape the trait's inbound side uses).
    ///
    /// the `context` is only used to derive child spawn contexts for the two
    /// inbound drains, so it's bounded by just `Spawner` — independent of the
    /// heavy `RNetwork`/`Resolver`/… bounds the authenticated `new` needs.
    pub fn from_channels<E, RB, RC>(
        context: E,
        broadcast: (S, RB),
        consensus: (S, RC),
    ) -> (Self, mpsc::Receiver<Inbound>)
    where
        E: Spawner + Send + 'static,
        RB: commonware_p2p::Receiver + Send + 'static,
        RC: commonware_p2p::Receiver + Send + 'static,
    {
        let (broadcast_sender, broadcast_receiver) = broadcast;
        let (consensus_sender, consensus_receiver) = consensus;

        // inbound drains: one task per channel, each forwarding (peer, bytes)
        // into the shared inbound mpsc tagged with that channel's lane. the
        // registered receivers buffer their channel's backlog regardless of
        // when these tasks start, so ordering vs network.start() is immaterial.
        let (in_tx, in_rx) = mpsc::channel::<Inbound>(MAX_BACKLOG);
        // `child` derives a fresh task context (commonware contexts are
        // spawn-once and not `Clone`), so we never need `E: Clone`.
        spawn_inbound_drain(
            context.child("inbound_broadcast"),
            broadcast_receiver,
            Lane::Broadcast,
            in_tx.clone(),
        );
        spawn_inbound_drain(
            context.child("inbound_consensus"),
            consensus_receiver,
            Lane::Consensus,
            in_tx,
        );

        (
            Self {
                broadcast: broadcast_sender,
                consensus: ConsensusLane::Gossip(consensus_sender),
            },
            in_rx,
        )
    }

    /// build a transport whose consensus lane feeds a LIVE simplex engine via
    /// `consensus` (tier A), while broadcast still gossips on `broadcast`.
    ///
    /// the caller stands up + keeps the engine alive separately (its handle must
    /// outlive the transport — dropping it aborts the engine task); this just
    /// holds the engine's submit handle. `send(Lane::Consensus, ..)` then does
    /// `store.put + enqueue` instead of gossip. dialect-agnostic: `S` and the
    /// handle are minted by whatever network registered the channels (simulated
    /// in tests, discovery in production).
    pub fn with_consensus_engine(broadcast: S, consensus: consensus::ConsensusHandle) -> Self {
        Self {
            broadcast,
            consensus: ConsensusLane::Engine(consensus),
        }
    }
}

impl<E> CommonwareTransport<GossipSender<E>>
where
    E: Spawner
        + BufferPooler
        + Clock
        + CryptoRngCore
        + RNetwork
        + Resolver
        + Storage
        + Metrics
        + Send
        + Sync
        + 'static,
{
    /// stand up a commonware node on the given runtime `context`, with a LIVE
    /// simplex BFT engine driving the consensus lane.
    ///
    /// builds the real `authenticated::discovery` network — broadcast gossip on
    /// [`CHANNEL_BROADCAST`], the engine's three sub-channels
    /// ([`CHANNEL_VOTE`]/[`CHANNEL_CERTIFICATE`]/[`CHANNEL_RESOLVER`]), and the
    /// payload channel ([`CHANNEL_PAYLOAD`]) the relay gossips op-batch bytes on —
    /// AND a real [`Engine`](commonware_consensus::simplex::Engine) wired to our
    /// [`ConsensusAutomaton`](consensus::ConsensusAutomaton) /
    /// [`ConsensusRelay`](consensus::ConsensusRelay) /
    /// [`ConsensusReporter`](consensus::ConsensusReporter) triple over one shared
    /// [`ContentStore`](consensus::ContentStore). `send(Lane::Consensus, ..)`
    /// stages a batch into that store and queues its digest; the engine BFT-orders
    /// it; on `Activity::Finalization` the reporter delivers the finalized bytes
    /// back out the returned inbound receiver, tagged [`Lane::Consensus`].
    ///
    /// cross-node delivery: when the leader proposes a batch its relay gossips the
    /// bytes on [`CHANNEL_PAYLOAD`]; every peer's payload drain caches them
    /// store-only ahead of finalization, so the reporter resolves the finalized
    /// digest and delivers on EVERY node — not just the proposer. (a node that
    /// finalizes a digest it never saw broadcast — late-join via certificate
    /// backfill — still can't resolve it; that catch-up path is a resolver follow-on.)
    ///
    /// returns three things:
    /// - the transport (cheap-clone: holds the broadcast `Sender` plus the
    ///   engine's `Arc`-backed submit handle),
    /// - the inbound receiver (the `(Lane, Vec<u8>)` tuple shape; it carries BOTH
    ///   gossip-broadcast bytes AND engine-finalized consensus bytes),
    /// - the engine task [`Handle`]. **the caller MUST keep this alive** — dropping
    ///   it aborts the consensus engine. that's why `new` returns it rather than
    ///   hiding it inside the clonable transport.
    ///
    /// the engine config derives entirely from `cfg`: participants are the
    /// authorized peer set (so simplex's participant indices line up with
    /// discovery's sorted set), the scheme is built the production way via
    /// `Scheme::signer` (this node signs as exactly the participant its discovery
    /// identity represents), and genesis is domain-separated by `namespace` so
    /// distinct apps can never share a `Floor`. timeouts are tuned defaults.
    pub fn new(context: E, cfg: Config) -> (Self, mpsc::Receiver<Inbound>, Handle<()>) {
        use commonware_consensus::simplex::{
            config::{Config as SimplexConfig, Floor, ForwardingPolicy},
            elector::RoundRobin,
            scheme::ed25519 as simplex_ed25519,
            Engine,
        };
        use commonware_consensus::types::{Epoch, ViewDelta};
        use commonware_cryptography::{Hasher, Sha256};
        use commonware_parallel::Sequential;
        use commonware_runtime::buffer::paged::CacheRef;
        use commonware_utils::{NZUsize, NZU16};
        use std::time::Duration;

        use crate::consensus::{
            ConsensusAutomaton, ConsensusRelay, ConsensusReporter, ContentStore, Digest,
        };

        let signer = ed25519::PrivateKey::from_seed(cfg.seed);

        // build the discovery config. `local` is the dev/sim-friendly preset
        // (allows private ips); production would use `recommended`.
        let bootstrappers: Vec<(ed25519::PublicKey, _)> = cfg
            .bootstrappers
            .into_iter()
            .map(|(pk, addr)| (pk, addr.into()))
            .collect();
        let p2p_cfg = discovery::Config::local(
            signer.clone(),
            &cfg.namespace,
            cfg.listen,
            cfg.advertised,
            bootstrappers,
            MAX_MESSAGE_SIZE,
        );

        let (mut network, mut oracle) = Network::new(context.child("network"), p2p_cfg);

        // the authorized participant set, SORTED — shared by discovery (the
        // tracked peer set, which must include our own key so everyone agrees on
        // the ordering) AND the simplex scheme (participant indices line up).
        let participants: Set<ed25519::PublicKey> =
            Set::try_from(cfg.peers).expect("authorized peer set has no duplicates");
        oracle.track(PEER_SET, participants.clone());

        // register the broadcast gossip channel, the engine's three sub-channels,
        // and the payload channel the relay gossips finalizable op-batches on.
        // quota caps inbound receive rate; `Quota` is `Copy`, so one suffices.
        let quota = Quota::per_second(NZU32!(128));
        let (broadcast_sender, broadcast_receiver) =
            network.register(CHANNEL_BROADCAST, quota, MAX_BACKLOG);
        let vote = network.register(CHANNEL_VOTE, quota, MAX_BACKLOG);
        let certificate = network.register(CHANNEL_CERTIFICATE, quota, MAX_BACKLOG);
        let resolver = network.register(CHANNEL_RESOLVER, quota, MAX_BACKLOG);
        let (payload_sender, payload_receiver) =
            network.register(CHANNEL_PAYLOAD, quota, MAX_BACKLOG);

        // one shared inbound mpsc: the broadcast drain feeds it (Lane::Broadcast)
        // and the consensus reporter feeds it on finalization (Lane::Consensus), so
        // the single receiver we return carries both lanes. (no consensus-channel
        // drain — the engine consumes vote/cert/resolver itself; finalized payloads
        // arrive via the reporter, not a gossip drain.)
        let (in_tx, in_rx) = mpsc::channel::<Inbound>(MAX_BACKLOG);
        spawn_inbound_drain(
            context.child("inbound_broadcast"),
            broadcast_receiver,
            Lane::Broadcast,
            in_tx.clone(),
        );

        // our consensus triple over ONE shared ContentStore. `new` mints the store
        // once and clones it into the handle, relay, and reporter — so the
        // shared-store precondition `ConsensusAutomaton::handle` documents (handle
        // puts under a digest, reporter gets by it on finalization) is satisfied
        // STRUCTURALLY here, not just by convention. the reporter also shares the
        // automaton's pending FIFO (peek-until-finalized: propose peeks the front,
        // the reporter removes on finalization).
        let store = ContentStore::new();
        // a non-proposer learns a finalized digest through consensus but not its
        // bytes. the payload drain caches every relay-broadcast payload into THIS
        // store (store-only — see `spawn_payload_drain`, NOT the inbound drain),
        // so on finalization the reporter's `store.get(&digest)` resolves and
        // delivers on EVERY node, not just the one that proposed it.
        spawn_payload_drain(
            context.child("inbound_payload"),
            payload_receiver,
            store.clone(),
        );
        let automaton = ConsensusAutomaton::<ed25519::PublicKey>::new();
        let consensus_handle = automaton.handle(store.clone());
        // the relay gossips a proposed batch's bytes out on CHANNEL_PAYLOAD so
        // peers' drains can cache them ahead of finalization.
        let relay = ConsensusRelay::<GossipSender<E>, ed25519::PublicKey>::new(
            payload_sender,
            store.clone(),
        );
        let reporter =
            ConsensusReporter::<simplex_ed25519::Scheme>::new(store.clone(), automaton.pending(), in_tx);

        // genesis domain-separated by namespace: distinct apps never share a Floor,
        // every peer in THIS app computes the identical digest.
        let genesis: Digest = {
            let mut hasher = Sha256::default();
            hasher.update(b"ducktape:consensus:genesis:v1:");
            hasher.update(&cfg.namespace);
            hasher.finalize()
        };

        // scheme built the production way: `signer` finds OUR private key's index
        // in the sorted participant set, so we sign as exactly the participant our
        // discovery identity represents.
        let scheme = simplex_ed25519::Scheme::signer(&cfg.namespace, participants.clone(), signer.clone())
            .expect("our key is in the authorized peer set");

        let engine_cfg = SimplexConfig {
            scheme,
            elector: RoundRobin::<Sha256>::default(),
            // the discovery Oracle IS the Blocker (impl Blocker for Oracle).
            blocker: oracle.clone(),
            automaton,
            relay,
            reporter,
            strategy: Sequential,
            // pubkey hex is FS-safe → isolated storage partition per node.
            partition: signer.public_key().to_string(),
            mailbox_size: NZUsize!(1024),
            epoch: Epoch::new(0),
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
            page_cache: CacheRef::from_pooler(&context, NZU16!(1024), NZUsize!(10)),
            forwarding: ForwardingPolicy::Disabled,
        };

        // start the network actors (dialer, listener, router, tracker, ...), then
        // the engine on its three channels. the registered receivers buffer up to
        // MAX_BACKLOG regardless, so starting the network here is fine.
        network.start();
        let engine = Engine::new(context.child("engine"), engine_cfg);
        let engine_handle = engine.start(vote, certificate, resolver);

        let transport = Self {
            broadcast: broadcast_sender,
            consensus: ConsensusLane::Engine(consensus_handle),
        };

        (transport, in_rx, engine_handle)
    }
}

/// what a [`Transport::send`] call will do once its future is awaited, resolved
/// up front (cloning the one sender/handle it needs) so the future owns its
/// inputs — no borrow of `self` held across the await — yet stays LAZY: a future
/// that's constructed and dropped without `.await` performs no effect. `Gossip`
/// covers BOTH the broadcast lane and the tier-C consensus lane (byte-identical
/// gossip); `Engine` is the tier-A simplex submit.
enum Outbound<S> {
    Gossip(S),
    Engine(consensus::ConsensusHandle),
}

impl<S> Transport for CommonwareTransport<S>
where
    S: commonware_p2p::Sender + Clone + Send + Sync + 'static,
{
    fn send(
        &self,
        lane: Lane,
        bytes: Vec<u8>,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        // resolve WHICH effect this lane takes (cloning the one sender/handle it
        // needs), then defer the effect into the returned future so `send` stays
        // lazy. the flat tuple match keeps dispatch at a single level, and
        // `Outbound` lets the broadcast arm, the tier-C consensus-gossip arm, and
        // the tier-A engine arm all unify to one future type without holding a
        // borrow of `self` across the await.
        let outbound = match (lane, &self.consensus) {
            (Lane::Broadcast, _) => Outbound::Gossip(self.broadcast.clone()),
            (Lane::Consensus, ConsensusLane::Gossip(sender)) => Outbound::Gossip(sender.clone()),
            (Lane::Consensus, ConsensusLane::Engine(handle)) => Outbound::Engine(handle.clone()),
        };
        async move {
            match outbound {
                // gossip to every authorized peer; best-effort, offline/rate-
                // limited peers are silently skipped (like loopback). `send` is
                // synchronous (returns the recipients it will attempt), no await.
                Outbound::Gossip(mut sender) => {
                    let _ = sender.send(Recipients::All, IoBuf::from(bytes), false);
                }
                // tier A: stage the bytes + queue their digest for the live
                // simplex engine to BFT-order. nothing goes on the wire HERE — the
                // engine's own vote/cert/resolver channels carry the protocol and
                // finalized batches arrive via the reporter, not this send.
                Outbound::Engine(handle) => handle.submit(bytes),
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use commonware_p2p::simulated::{self, Link};
    use commonware_p2p::Provider as _;
    use commonware_runtime::{deterministic, Runner, Supervisor as _};
    use commonware_utils::{ordered::Set, NZUsize};
    use op::Op;
    use std::time::Duration;
    use transport::{decode_batch, encode_batch};

    /// 2-node deterministic sim over `p2p::simulated`: node A broadcasts an
    /// encoded op batch and node B receives + decodes it. this is a REAL
    /// distributed proof — two transports wired through commonware's simulated
    /// network (instant deterministic links, no dial/handshake/discovery), so it
    /// actually runs and terminates under the deterministic clock.
    ///
    /// the production `authenticated::discovery` path is unchanged; only the
    /// TEST path swaps in `simulated`, which is exactly how commonware's own
    /// consensus tests drive deterministic multi-node sims. n=2 is fine for a
    /// best-effort broadcast (no BFT quorum involved).
    #[test]
    fn two_node_broadcast_propagates() {
        // `timed` bounds simulated time: if the link/track wiring were wrong and
        // B's recv never landed, the executor hits its deadline and panics
        // rather than live-locking. a pass therefore means B genuinely received
        // + decoded within simulated time, not a stubbed assert.
        let executor = deterministic::Runner::timed(Duration::from_secs(30));
        executor.start(|context| async move {
            let key_a = ed25519::PrivateKey::from_seed(1).public_key();
            let key_b = ed25519::PrivateKey::from_seed(2).public_key();
            let peers = vec![key_a.clone(), key_b.clone()];

            // stand up the simulated network + oracle. one tracked peer set is
            // all a 2-node gossip needs.
            let (network, oracle) = simulated::Network::new(
                context.child("network"),
                simulated::Config {
                    max_size: MAX_MESSAGE_SIZE,
                    disconnect_on_block: true,
                    tracked_peer_sets: NZUsize!(1),
                },
            );
            network.start();

            // register BOTH channels on BOTH peers — `from_channels` needs a
            // broadcast + consensus pair each. the idle consensus drains block on
            // recv() forever, which is harmless: the runner stops when the root
            // future (rx_b.recv below) returns and aborts spawned tasks.
            let quota = Quota::per_second(NZU32!(128));
            let (a_bcast_tx, a_bcast_rx) = oracle
                .control(key_a.clone())
                .register(CHANNEL_BROADCAST, quota)
                .await
                .expect("A registers broadcast channel");
            let (a_cons_tx, a_cons_rx) = oracle
                .control(key_a.clone())
                .register(CHANNEL_CONSENSUS, quota)
                .await
                .expect("A registers consensus channel");
            let (b_bcast_tx, b_bcast_rx) = oracle
                .control(key_b.clone())
                .register(CHANNEL_BROADCAST, quota)
                .await
                .expect("B registers broadcast channel");
            let (b_cons_tx, b_cons_rx) = oracle
                .control(key_b.clone())
                .register(CHANNEL_CONSENSUS, quota)
                .await
                .expect("B registers consensus channel");

            // track the authorized peer set, then await it landing before
            // linking/sending so peers are connectable when A sends.
            let mut manager = oracle.manager();
            manager.track(PEER_SET, Set::from_iter_dedup(peers));
            assert!(
                manager.peer_set(PEER_SET).await.is_some(),
                "peer set tracked"
            );

            // link both directions with a perfect link (success_rate 1.0) so the
            // single gossip message is delivered deterministically.
            let link = Link {
                latency: Duration::from_millis(10),
                jitter: Duration::from_millis(1),
                success_rate: 1.0,
            };
            oracle
                .add_link(key_a.clone(), key_b.clone(), link.clone())
                .await
                .expect("link a->b");
            oracle
                .add_link(key_b.clone(), key_a.clone(), link)
                .await
                .expect("link b->a");

            // build both transports from the registered channel pairs.
            let (transport_a, mut _rx_a) = CommonwareTransport::from_channels(
                context.child("a"),
                (a_bcast_tx, a_bcast_rx),
                (a_cons_tx, a_cons_rx),
            );
            let (_transport_b, mut rx_b) = CommonwareTransport::from_channels(
                context.child("b"),
                (b_bcast_tx, b_bcast_rx),
                (b_cons_tx, b_cons_rx),
            );

            // A broadcasts an encoded op batch.
            let wire = vec![Op::Vcs(vcs::op::Op::Announce { objects: Vec::new() })];
            let bytes = encode_batch(&wire);
            transport_a
                .send(Lane::Broadcast, bytes)
                .await
                .expect("broadcast send ok");

            // B receives the gossip and decodes the same batch.
            let (lane, recv) = rx_b.recv().await.expect("node B receives gossip");
            assert_eq!(lane, Lane::Broadcast);
            let ops = decode_batch(&recv).expect("decode batch");
            assert_eq!(ops.len(), 1);
            assert!(matches!(ops[0], Op::Vcs(vcs::op::Op::Announce { .. })));
        });
    }

    /// CW.2b+ — the REAL consensus lane delivering to EVERY validator: a
    /// commonware simplex `Engine` reaching `Activity::Finalization`, driven by OUR
    /// own [`consensus::ConsensusAutomaton`] / [`consensus::ConsensusRelay`] /
    /// [`consensus::ConsensusReporter`] (not commonware's mocks), with the finalized
    /// op-batch bytes resolved through OUR [`consensus::ContentStore`] and delivered
    /// into a PER-VALIDATOR inbound mpsc tagged `Lane::Consensus`.
    ///
    /// the cross-node payload path under test: ONLY participant 0 (the proposer)
    /// stages the op-batch into its store + enqueues the digest. when it leads and
    /// proposes, its `ConsensusRelay` gossips the bytes on `CHANNEL_PAYLOAD`; every
    /// other validator's STORE-ONLY payload drain caches them. so when the digest
    /// finalizes, ALL n validators — not just the proposer — resolve
    /// `store.get(&digest)` and deliver. that is exactly what the no-op relay used
    /// to prevent (peers finalized the digest but `store.get -> None` and dropped);
    /// this test is the regression guard that they now deliver.
    ///
    /// finalization proof, per validator: `ConsensusReporter::report` forwards onto
    /// its inbound mpsc ONLY in the `Activity::Finalization` arm. so each of the n
    /// `inbox.recv().await` returning the byte-identical batch proves THAT validator
    /// hit a real finalization AND resolved the payload — which a non-proposer could
    /// obtain ONLY via the relay broadcast, since it never staged the bytes itself.
    /// `Runner::timed(300s)` is the liveness guard: a wiring bug (relay not
    /// gossiping, drain forwarding to the wrong place) surfaces as a deadline panic.
    ///
    /// non-proposer leaders propose against an empty queue, so their `propose` drops
    /// the sender → that view nullifies (leader_timeout) and advances. that is
    /// standard simplex liveness; within at most n views the proposer is leader,
    /// proposes its one digest (re-gossiping the payload each turn), and perfect
    /// links + our `verify -> true` mean every validator notarizes + finalizes it.
    #[test]
    fn simplex_relay_delivers_finalized_payload_to_all_validators() {
        use commonware_consensus::simplex::{
            config::{Config as SimplexConfig, Floor, ForwardingPolicy},
            elector::RoundRobin,
            mocks,
            scheme::ed25519 as simplex_ed25519,
            Engine,
        };
        use commonware_consensus::types::{Epoch, ViewDelta};
        use commonware_cryptography::Sha256;
        use commonware_parallel::Sequential;
        use commonware_runtime::buffer::paged::CacheRef;
        use commonware_utils::{NZUsize, NZU16};

        use crate::consensus::{
            ConsensusAutomaton, ConsensusRelay, ConsensusReporter, ContentStore,
        };

        // canonical fixture params, lifted verbatim from all_online / CW.2a.
        let n: u32 = 5;
        let activity_timeout = ViewDelta::new(10);
        let skip_timeout = ViewDelta::new(5);
        let namespace = b"consensus".to_vec();
        let page_size = NZU16!(1024);
        let page_cache_size = NZUsize!(10);

        let executor = deterministic::Runner::timed(Duration::from_secs(300));
        executor.start(|mut context| async move {
            // the ed25519 scheme fixture: n sorted participants + per-validator
            // scheme instances. `deterministic::Context` is the rng source.
            let fixture = simplex_ed25519::fixture(&mut context, &namespace, n);
            let participants = fixture.participants.clone();
            let schemes = fixture.schemes.clone();

            // simulated network seeded with the participant set (instant,
            // deterministic links — no dial/handshake/discovery).
            let (network, oracle) = simulated::Network::new_with_peers(
                context.child("network"),
                simulated::Config {
                    max_size: 1024 * 1024,
                    disconnect_on_block: true,
                    tracked_peer_sets: NZUsize!(1),
                },
                participants.clone(),
            )
            .await;
            network.start();

            // register all validators: vote(0), certificate(1), resolver(2) — in
            // that order, since the engine's `start(vote, cert, resolver)` consumes
            // them positionally — plus the payload channel (CHANNEL_PAYLOAD) the
            // relay gossips op-batch bytes on, one per validator.
            let quota = Quota::per_second(NZU32!(128));
            let mut registrations = std::collections::HashMap::new();
            let mut payload_chans = std::collections::HashMap::new();
            for validator in participants.iter() {
                let control = oracle.control(validator.clone());
                let vote = control.register(0, quota).await.expect("register vote");
                let certificate = control
                    .register(1, quota)
                    .await
                    .expect("register certificate");
                let resolver = control.register(2, quota).await.expect("register resolver");
                let payload = control
                    .register(CHANNEL_PAYLOAD, quota)
                    .await
                    .expect("register payload");
                registrations.insert(validator.clone(), (vote, certificate, resolver));
                payload_chans.insert(validator.clone(), payload);
            }

            // perfect all-pairs links so votes/certs propagate deterministically.
            let link = Link {
                latency: Duration::from_millis(10),
                jitter: Duration::from_millis(1),
                success_rate: 1.0,
            };
            for v1 in participants.iter() {
                for v2 in participants.iter() {
                    if v1 == v2 {
                        continue;
                    }
                    oracle
                        .add_link(v1.clone(), v2.clone(), link.clone())
                        .await
                        .expect("link validators");
                }
            }

            // the PROPOSER: participant 0. it (and only it) stages the op-batch into
            // its ContentStore + enqueues the digest. every OTHER validator obtains
            // the bytes solely through the relay's CHANNEL_PAYLOAD broadcast.
            let proposer = participants[0].clone();

            // the op-batch we expect to see finalized + delivered. a RefUpdate to
            // MAIN_REF is a consensus-lane op (`op::Op::lane()` routes it there).
            let wire = vec![Op::Vcs(vcs::op::Op::RefUpdate {
                name: vcs::op::MAIN_REF.to_string(),
                target: vcs::ObjectId::from_bytes([0u8; 32]),
                prev: None,
            })];
            let proposed_bytes = encode_batch(&wire);

            // build + start one engine per validator with OUR triple. EVERY
            // validator's reporter feeds its OWN inbound mpsc; we collect all n
            // receivers and assert each delivers the finalized batch.
            let elector = RoundRobin::<Sha256>::default();
            let mut engine_handlers = Vec::new();
            let mut inboxes = Vec::new();
            for (idx, validator) in participants.iter().enumerate() {
                let v_ctx = context.child("validator");
                let is_proposer = *validator == proposer;

                // each validator owns its own content store. only the proposer's
                // gets the payload staged directly; peers fill theirs from the relay
                // broadcast via the store-only payload drain below.
                let store = ContentStore::new();

                // the store-only payload drain: cache every relay-broadcast batch
                // into THIS validator's store (NOT into the inbound mpsc — delivery
                // stays the reporter's job, on finalization, in BFT order).
                let (payload_tx, payload_rx) = payload_chans
                    .remove(validator)
                    .expect("validator payload channel registered");
                spawn_payload_drain(v_ctx.child("payload_drain"), payload_rx, store.clone());

                // P = ed25519::PublicKey (the fixture's key) for Automaton/Relay.
                let automaton =
                    ConsensusAutomaton::<commonware_cryptography::ed25519::PublicKey>::new();
                if is_proposer {
                    let digest = store.put(proposed_bytes.clone());
                    automaton.enqueue(digest);
                }
                // the relay gossips proposed bytes on the payload channel so peers'
                // drains can cache them ahead of finalization.
                let relay = ConsensusRelay::<_, commonware_cryptography::ed25519::PublicKey>::new(
                    payload_tx,
                    store.clone(),
                );

                // S = the type of schemes[idx] for ConsensusReporter::<S> (phantom,
                // named explicitly at construction). EVERY validator's reporter feeds
                // its OWN inbound rx, kept in `inboxes` for the all-n delivery assert.
                let (in_tx, in_rx) = mpsc::channel::<Inbound>(MAX_BACKLOG);
                inboxes.push(in_rx);
                let reporter = ConsensusReporter::<simplex_ed25519::Scheme>::new(
                    store.clone(),
                    automaton.pending(),
                    in_tx,
                );

                let blocker = oracle.control(validator.clone());
                let cfg = SimplexConfig {
                    scheme: schemes[idx].clone(),
                    elector: elector.clone(),
                    blocker,
                    automaton,
                    relay,
                    reporter,
                    strategy: Sequential,
                    partition: validator.to_string(),
                    mailbox_size: NZUsize!(1024),
                    epoch: Epoch::new(333),
                    floor: Floor::Genesis(mocks::application::genesis::<Sha256>(Epoch::new(333))),
                    leader_timeout: Duration::from_secs(1),
                    certification_timeout: Duration::from_secs(2),
                    timeout_retry: Duration::from_secs(10),
                    fetch_timeout: Duration::from_secs(1),
                    activity_timeout,
                    skip_timeout,
                    fetch_concurrent: NZUsize!(4),
                    replay_buffer: NZUsize!(1024 * 1024),
                    write_buffer: NZUsize!(1024 * 1024),
                    page_cache: CacheRef::from_pooler(&context, page_size, page_cache_size),
                    forwarding: ForwardingPolicy::Disabled,
                };
                let engine = Engine::new(v_ctx.child("engine"), cfg);

                let (vote, certificate, resolver) = registrations
                    .remove(validator)
                    .expect("validator should be registered");
                // KEEP the handle alive — dropping it can abort the engine task.
                engine_handlers.push(engine.start(vote, certificate, resolver));
            }

            // await the finalized delivery on EVERY validator. each recv resolving
            // with the proposed bytes means THAT validator's reporter hit
            // `Activity::Finalization`, resolved the digest in its ContentStore (the
            // proposer's from staging, every peer's from the relay broadcast), and
            // forwarded the op-batch — a REAL simplex finalization round trip,
            // delivered cross-node. (timed runner panics on any no-show.)
            for (i, mut inbox) in inboxes.into_iter().enumerate() {
                let (lane, recv) = inbox.recv().await.unwrap_or_else(|| {
                    panic!("validator {i} receives a finalized consensus batch")
                });
                assert_eq!(lane, Lane::Consensus);
                let ops = decode_batch(&recv).expect("decode finalized batch");
                assert_eq!(ops.len(), 1);
                assert!(matches!(ops[0], Op::Vcs(vcs::op::Op::RefUpdate { .. })));
                // exact round-trip: byte-identical to what the proposer staged.
                assert_eq!(
                    recv, proposed_bytes,
                    "validator {i} delivered the byte-identical finalized batch"
                );
            }

            // keep engines alive until the very end (drop is the implicit abort).
            drop(engine_handlers);
        });
    }

    /// the production-shaped consensus send: prove that calling the REAL
    /// [`Transport::send`]`(Lane::Consensus, bytes)` on a [`CommonwareTransport`]
    /// whose consensus lane is wired to a live simplex engine
    /// ([`ConsensusLane::Engine`]) drives that batch through BFT finalization and
    /// delivers it on the proposer.
    ///
    /// this is strictly MORE than
    /// [`simplex_relay_delivers_finalized_payload_to_all_validators`]: that test
    /// pokes `store.put` + `automaton.enqueue` directly, bypassing the transport;
    /// here the ONLY way the digest reaches the engine is through `transport.send`,
    /// so a pass proves the SEND-PATH wiring (not just the engine) is correct.
    /// everything else is identical — n=5, simulated network, per-validator real
    /// `Engine::new`/`start`, our Automaton/Relay/Reporter triple. this test
    /// deliberately wires NO payload drains and asserts only the proposer: its axis
    /// is the send path; cross-node delivery to non-proposers is proven by the
    /// all-validators test above.
    ///
    /// finalization proof is the same mechanism: the proposer's
    /// `ConsensusReporter` forwards onto `proposer_in_rx` ONLY in the
    /// `Activity::Finalization` arm, so `recv().await` returning the proposed
    /// bytes IS proof a real engine finalized them. `Runner::timed(300s)` guards
    /// liveness (a wiring bug surfaces as a deadline panic, not a hang).
    #[test]
    fn transport_send_consensus_finalizes_on_proposer() {
        use commonware_consensus::simplex::{
            config::{Config as SimplexConfig, Floor, ForwardingPolicy},
            elector::RoundRobin,
            mocks,
            scheme::ed25519 as simplex_ed25519,
            Engine,
        };
        use commonware_consensus::types::{Epoch, ViewDelta};
        use commonware_cryptography::Sha256;
        use commonware_parallel::Sequential;
        use commonware_runtime::buffer::paged::CacheRef;
        use commonware_utils::{NZUsize, NZU16};

        use crate::consensus::{ConsensusAutomaton, ConsensusRelay, ConsensusReporter, ContentStore};

        // canonical fixture params, identical to the direct-enqueue test.
        let n: u32 = 5;
        let activity_timeout = ViewDelta::new(10);
        let skip_timeout = ViewDelta::new(5);
        let namespace = b"consensus".to_vec();
        let page_size = NZU16!(1024);
        let page_cache_size = NZUsize!(10);

        let executor = deterministic::Runner::timed(Duration::from_secs(300));
        executor.start(|mut context| async move {
            let fixture = simplex_ed25519::fixture(&mut context, &namespace, n);
            let participants = fixture.participants.clone();
            let schemes = fixture.schemes.clone();

            let (network, oracle) = simulated::Network::new_with_peers(
                context.child("network"),
                simulated::Config {
                    max_size: 1024 * 1024,
                    disconnect_on_block: true,
                    tracked_peer_sets: NZUsize!(1),
                },
                participants.clone(),
            )
            .await;
            network.start();

            // register all validators: vote(0)/certificate(1)/resolver(2), the
            // order engine.start consumes positionally — plus the payload channel
            // (CHANNEL_PAYLOAD) every relay now needs a live sender for.
            let quota = Quota::per_second(NZU32!(128));
            let mut registrations = std::collections::HashMap::new();
            let mut payload_chans = std::collections::HashMap::new();
            for validator in participants.iter() {
                let control = oracle.control(validator.clone());
                let vote = control.register(0, quota).await.expect("register vote");
                let certificate = control
                    .register(1, quota)
                    .await
                    .expect("register certificate");
                let resolver = control.register(2, quota).await.expect("register resolver");
                let payload = control
                    .register(CHANNEL_PAYLOAD, quota)
                    .await
                    .expect("register payload");
                registrations.insert(validator.clone(), (vote, certificate, resolver));
                payload_chans.insert(validator.clone(), payload);
            }

            let link = Link {
                latency: Duration::from_millis(10),
                jitter: Duration::from_millis(1),
                success_rate: 1.0,
            };
            for v1 in participants.iter() {
                for v2 in participants.iter() {
                    if v1 == v2 {
                        continue;
                    }
                    oracle
                        .add_link(v1.clone(), v2.clone(), link.clone())
                        .await
                        .expect("link validators");
                }
            }

            let proposer = participants[0].clone();
            let wire = vec![Op::Vcs(vcs::op::Op::RefUpdate {
                name: vcs::op::MAIN_REF.to_string(),
                target: vcs::ObjectId::from_bytes([0u8; 32]),
                prev: None,
            })];
            let proposed_bytes = encode_batch(&wire);

            let (proposer_in_tx, mut proposer_in_rx) = mpsc::channel::<Inbound>(MAX_BACKLOG);

            let elector = RoundRobin::<Sha256>::default();
            let mut engine_handlers = Vec::new();
            // this test asserts ONLY proposer delivery (its axis is the send path,
            // not cross-node), so it wires no payload drains; but each relay still
            // needs a live CHANNEL_PAYLOAD sender to construct. hold the receivers
            // open for the run so those senders stay connected.
            let mut payload_keepalive = Vec::new();
            for (idx, validator) in participants.iter().enumerate() {
                let v_ctx = context.child("validator");
                let is_proposer = *validator == proposer;

                let store = ContentStore::new();
                let automaton =
                    ConsensusAutomaton::<commonware_cryptography::ed25519::PublicKey>::new();

                // THE CHANGE under test: the proposer stages its op-batch by
                // calling the REAL `transport.send(Lane::Consensus, ..)`, not
                // `store.put` + `enqueue` directly. building a transport needs a
                // broadcast sender, so register one throwaway channel for it; the
                // consensus lane here is Engine-backed, so that broadcast sender
                // is never exercised by the send under test — it only satisfies
                // the transport's shape. `handle()` shares THIS automaton's
                // pending FIFO + store, so the submit lands on the very queue the
                // engine's `propose` pops from.
                if is_proposer {
                    let (bcast_tx, _bcast_rx) = oracle
                        .control(validator.clone())
                        .register(3, quota)
                        .await
                        .expect("proposer registers a broadcast channel");
                    let transport = CommonwareTransport::with_consensus_engine(
                        bcast_tx,
                        automaton.handle(store.clone()),
                    );
                    transport
                        .send(Lane::Consensus, proposed_bytes.clone())
                        .await
                        .expect("consensus send ok");
                }

                let (payload_tx, payload_rx) = payload_chans
                    .remove(validator)
                    .expect("validator payload channel registered");
                payload_keepalive.push(payload_rx);
                let relay = ConsensusRelay::<_, commonware_cryptography::ed25519::PublicKey>::new(
                    payload_tx,
                    store.clone(),
                );

                let inbound = if is_proposer {
                    proposer_in_tx.clone()
                } else {
                    let (throwaway, _drop) = mpsc::channel::<Inbound>(MAX_BACKLOG);
                    throwaway
                };
                let reporter = ConsensusReporter::<simplex_ed25519::Scheme>::new(
                    store.clone(),
                    automaton.pending(),
                    inbound,
                );

                let blocker = oracle.control(validator.clone());
                let cfg = SimplexConfig {
                    scheme: schemes[idx].clone(),
                    elector: elector.clone(),
                    blocker,
                    automaton,
                    relay,
                    reporter,
                    strategy: Sequential,
                    partition: validator.to_string(),
                    mailbox_size: NZUsize!(1024),
                    epoch: Epoch::new(333),
                    floor: Floor::Genesis(mocks::application::genesis::<Sha256>(Epoch::new(333))),
                    leader_timeout: Duration::from_secs(1),
                    certification_timeout: Duration::from_secs(2),
                    timeout_retry: Duration::from_secs(10),
                    fetch_timeout: Duration::from_secs(1),
                    activity_timeout,
                    skip_timeout,
                    fetch_concurrent: NZUsize!(4),
                    replay_buffer: NZUsize!(1024 * 1024),
                    write_buffer: NZUsize!(1024 * 1024),
                    page_cache: CacheRef::from_pooler(&context, page_size, page_cache_size),
                    forwarding: ForwardingPolicy::Disabled,
                };
                let engine = Engine::new(v_ctx.child("engine"), cfg);

                let (vote, certificate, resolver) = registrations
                    .remove(validator)
                    .expect("validator should be registered");
                engine_handlers.push(engine.start(vote, certificate, resolver));
            }

            // recv resolving with the proposed bytes means the send drove a real
            // `Activity::Finalization` — the round trip through `transport.send`.
            let (lane, recv) = proposer_in_rx
                .recv()
                .await
                .expect("proposer receives a finalized consensus batch");
            assert_eq!(lane, Lane::Consensus);
            let ops = decode_batch(&recv).expect("decode finalized batch");
            assert_eq!(ops.len(), 1);
            assert!(matches!(ops[0], Op::Vcs(vcs::op::Op::RefUpdate { .. })));
            assert_eq!(recv, proposed_bytes);

            drop(engine_handlers);
        });
    }

    /// PROBE — production `new()` over REAL sockets (de-risks the engine flip).
    ///
    /// every other net test drives [`CommonwareTransport::from_channels`] over the
    /// instant `p2p::simulated` dialect; this one stands up N nodes through the
    /// PRODUCTION [`CommonwareTransport::new`] — real `authenticated::discovery`,
    /// real ed25519 handshakes, real localhost TCP — under `tokio::Runner`, and
    /// proves a `send(Lane::Broadcast)` from the bootstrapper reaches a peer's
    /// inbound mpsc. it gives `new()` its first coverage and is the harness the
    /// simplex-engine flip will reuse.
    ///
    /// why tokio, not deterministic: `authenticated::discovery`'s dial/gossip
    /// timers don't make progress under the deterministic clock — but commonware's
    /// own `test_tokio_connectivity` proves discovery DOES converge in-process
    /// under `tokio::Runner`, so this rides the same grain.
    ///
    /// `#[ignore]` keeps the default `cargo test -p net` hermetic (no real ports);
    /// run explicitly with `cargo test -p net -- --ignored`.
    #[test]
    #[ignore = "real-socket: binds localhost TCP; run with --ignored"]
    fn new_broadcast_propagates_over_real_sockets() {
        use commonware_macros::select;
        use std::net::{IpAddr, Ipv4Addr};

        // n=5: `new()` now also builds a live simplex engine, which wants the
        // canonical BFT participant count (3f+1). the engines just idle here (no
        // consensus submits) — this test only exercises the broadcast lane — but
        // they must stand up cleanly, so we give them a healthy set.
        const N: usize = 5;
        const BASE_PORT: u16 = 52111;

        let executor = commonware_runtime::tokio::Runner::default();
        executor.start(|context| async move {
            // every node agrees on the authorized peer set (n keys from seeds
            // 0..N) and — except node 0 — bootstraps off node 0's address.
            let keys: Vec<ed25519::PublicKey> = (0..N as u64)
                .map(|i| ed25519::PrivateKey::from_seed(i).public_key())
                .collect();
            let node0_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), BASE_PORT);

            // stand up all N nodes through the PRODUCTION new(). keep every handle
            // (a dropped transport's inbound drain would exit; the spawned network
            // actors live on the runtime regardless). per-node `with_attribute`
            // keeps each node's metric paths distinct — reusing one child label
            // across nodes would collide in the registry.
            let mut transports = Vec::new();
            let mut inboxes = Vec::new();
            // keep every engine handle alive — dropping one aborts that node's
            // consensus engine. we never assert on consensus here, but a node whose
            // engine task died is not a faithful production node.
            let mut engines = Vec::new();
            for i in 0..N {
                let addr =
                    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), BASE_PORT + i as u16);
                let bootstrappers = if i == 0 {
                    Vec::new()
                } else {
                    vec![(keys[0].clone(), node0_addr)]
                };
                let cfg = Config {
                    seed: i as u64,
                    namespace: b"ducktape-probe".to_vec(),
                    listen: addr,
                    advertised: addr,
                    peers: keys.clone(),
                    bootstrappers,
                };
                let node_ctx = context.child("node").with_attribute("index", i);
                let (transport, inbox, engine) = CommonwareTransport::new(node_ctx, cfg);
                transports.push(transport);
                inboxes.push(inbox);
                engines.push(engine);
            }

            // the batch node 0 gossips to ALL peers.
            let wire = vec![Op::Vcs(vcs::op::Op::Announce { objects: Vec::new() })];
            let expected = encode_batch(&wire);

            // resend on a loop: right after start() the mesh isn't formed, so a
            // single Recipients::All gossip reaches nobody and is silently dropped.
            // keep sending until a peer's drain delivers it — exactly how
            // commonware's own connectivity test drives sends (retry until landed).
            // hold the spawn handle so the task isn't aborted on drop.
            let sender = transports[0].clone();
            let resend = expected.clone();
            let _resend_handle = context.child("resend").spawn(move |ctx| async move {
                loop {
                    let _ = sender.send(Lane::Broadcast, resend.clone()).await;
                    ctx.sleep(Duration::from_millis(250)).await;
                }
            });

            // node 1 must receive the gossip. a converged mesh delivers in well
            // under a second; the 60s ceiling only exists so a non-converging
            // discovery fails fast (deadline panic) instead of hanging the suite.
            let mut node1 = inboxes.remove(1);
            select! {
                received = node1.recv() => {
                    let (lane, msg) = received.expect("node 1 inbound channel closed");
                    assert_eq!(lane, Lane::Broadcast);
                    assert_eq!(msg, expected, "node 1 got the exact gossiped batch");
                },
                _timeout = context.sleep(Duration::from_secs(60)) => {
                    panic!(
                        "broadcast did not propagate over real sockets within 60s — \
                         discovery never converged through production new()"
                    );
                },
            }

            // hold everything until the assert resolves.
            drop(transports);
            drop(inboxes);
            drop(engines);
        });
    }

    /// PROOF — production [`CommonwareTransport::new`] finalizes a
    /// `send(Lane::Consensus, ..)` over REAL sockets. this is the end-to-end test
    /// of the engine flip: it drives the PUBLIC seam through the REAL production
    /// constructor — real `authenticated::discovery`, real ed25519 handshakes, real
    /// localhost TCP, real per-node FS storage partitions, a real simplex `Engine` —
    /// to a real `Activity::Finalization`, with NO inline wiring and NO mocks. it
    /// supersedes the earlier inline substrate probe: that hand-built the engine to
    /// de-risk the wiring; now `new()` OWNS that wiring, so the proof goes through
    /// `new()` itself.
    ///
    /// what a pass proves (genuine BFT, not a local short-circuit):
    /// `Activity::Finalization` only fires when the engine recovers a finalization
    /// CERTIFICATE — 2f+1 distinct validators' signed votes — which one node holding
    /// one key cannot manufacture. so node 0's reporter delivering the byte-identical
    /// batch back out its inbound receiver IS proof that real signed votes crossed
    /// real TCP and a quorum agreed on exactly the bytes we submitted.
    ///
    /// cross-node delivery is now LIVE: the leader's `ConsensusRelay` gossips the
    /// proposed bytes on `CHANNEL_PAYLOAD` and peers cache them store-only, so a
    /// non-proposer resolves the finalized digest too. this test asserts BOTH the
    /// submitter (node 0) AND a non-proposer (node 1) deliver the byte-identical
    /// batch over real sockets. the deterministic-sim
    /// `simplex_relay_delivers_finalized_payload_to_all_validators` is the
    /// guaranteed all-n proof; this confirms it holds over production discovery +
    /// real TCP. (a node that finalizes via certificate backfill WITHOUT seeing a
    /// live payload broadcast — late-join/catch-up — still can't resolve it; that's
    /// a resolver follow-on, orthogonal to "does `new()` deliver cross-node".)
    ///
    /// the deadline is a wall-clock `select!`/`sleep`, NOT `Runner::timed` (which
    /// bounds *simulated* time — 60 REAL seconds of hang under tokio). a single
    /// `send` enqueues the digest persistently — peek-until-finalized keeps it
    /// across any nullified early views while the mesh forms — so one submit
    /// suffices; finalization lands in seconds and the ceiling only fails-fast a
    /// stall.
    #[test]
    #[ignore = "real-socket: binds localhost TCP + writes FS storage; run with --ignored"]
    fn new_finalizes_consensus_send_over_real_sockets() {
        use commonware_macros::select;
        use std::net::{IpAddr, Ipv4Addr};

        const N: usize = 5;
        const BASE_PORT: u16 = 52120;

        let executor = commonware_runtime::tokio::Runner::default();
        executor.start(|context| async move {
            // every node agrees on the authorized peer set (n keys from seeds
            // 0..N) and — except node 0 — bootstraps off node 0's address.
            let keys: Vec<ed25519::PublicKey> = (0..N as u64)
                .map(|i| ed25519::PrivateKey::from_seed(i).public_key())
                .collect();
            let node0_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), BASE_PORT);

            // stand up all N nodes through production new(); keep every handle alive
            // (a dropped engine handle aborts that node's consensus task).
            let mut transports = Vec::new();
            let mut inboxes = Vec::new();
            let mut engines = Vec::new();
            for i in 0..N {
                let addr =
                    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), BASE_PORT + i as u16);
                let bootstrappers = if i == 0 {
                    Vec::new()
                } else {
                    vec![(keys[0].clone(), node0_addr)]
                };
                let cfg = Config {
                    seed: i as u64,
                    namespace: b"ducktape-consensus-proof".to_vec(),
                    listen: addr,
                    advertised: addr,
                    peers: keys.clone(),
                    bootstrappers,
                };
                let node_ctx = context.child("node").with_attribute("index", i);
                let (transport, inbox, engine) = CommonwareTransport::new(node_ctx, cfg);
                transports.push(transport);
                inboxes.push(inbox);
                engines.push(engine);
            }

            // the op-batch node 0 submits and we expect to see finalized. a
            // RefUpdate to MAIN_REF routes to the consensus lane (op::Op::lane()).
            let wire = vec![Op::Vcs(vcs::op::Op::RefUpdate {
                name: vcs::op::MAIN_REF.to_string(),
                target: vcs::ObjectId::from_bytes([0u8; 32]),
                prev: None,
            })];
            let proposed_bytes = encode_batch(&wire);

            // drive the REAL public seam: stage the batch on node 0's consensus
            // lane. one call enqueues the digest persistently; the engine BFT-orders
            // it once node 0 leads with a formed quorum (peek keeps it queued across
            // any nullified early views while the mesh is still forming).
            transports[0]
                .send(Lane::Consensus, proposed_bytes.clone())
                .await
                .expect("submit to node 0's consensus lane");

            // shared finalization check: right lane, decodes to the one RefUpdate,
            // byte-identical to what node 0 submitted. (nested fn — captures nothing,
            // takes everything it needs.)
            fn assert_finalized(who: &str, lane: Lane, recv: &[u8], expected: &[u8]) {
                assert_eq!(lane, Lane::Consensus, "{who}: lane");
                let ops = decode_batch(recv).expect("decode finalized batch");
                assert_eq!(ops.len(), 1, "{who}: one op");
                assert!(matches!(ops[0], Op::Vcs(vcs::op::Op::RefUpdate { .. })));
                assert_eq!(recv, expected, "{who}: finalized bytes byte-identical to node 0's submission");
            }

            // grab node 1's inbox BEFORE node 0's: remove(0) shifts later indices
            // down, so pulling node 0 first would renumber node 1.
            let mut node1_in = inboxes.remove(1);
            let mut node0_in = inboxes.remove(0);

            // node 0 (the submitter) delivers from its own staged ContentStore on
            // Activity::Finalization — recv == a real BFT round trip over sockets.
            select! {
                received = node0_in.recv() => {
                    let (lane, recv) = received.expect("node 0 inbound channel closed");
                    assert_finalized("node 0 (submitter)", lane, &recv, &proposed_bytes);
                },
                _timeout = context.sleep(Duration::from_secs(60)) => {
                    panic!(
                        "production new() did not finalize a consensus send within 60s — \
                         discovery never converged or the engine stalled"
                    );
                },
            }

            // node 1 (a NON-proposer) delivers ONLY because the leader's relay
            // gossiped the payload on CHANNEL_PAYLOAD and node 1's store-only drain
            // cached it ahead of finalization — the cross-node-payload proof over
            // real discovery + TCP (the sim test guarantees it for all n).
            select! {
                received = node1_in.recv() => {
                    let (lane, recv) = received.expect("node 1 inbound channel closed");
                    assert_finalized("node 1 (non-proposer)", lane, &recv, &proposed_bytes);
                },
                _timeout = context.sleep(Duration::from_secs(60)) => {
                    panic!(
                        "node 1 (non-proposer) never delivered the finalized batch within 60s — \
                         the relay payload broadcast or the store-only drain is broken"
                    );
                },
            }

            // hold everything until the asserts resolve.
            drop(transports);
            drop(inboxes);
            drop(engines);
        });
    }

    /// CW.2a — the known-good baseline: a REAL commonware simplex `Engine`
    /// reaching `Activity::Finalization` on a `p2p::simulated` network, driven
    /// entirely by commonware's OWN mocks. this is a faithful replica of
    /// commonware-consensus's internal `all_online` test (consensus
    /// `src/simplex/mod.rs`), specialized to the ed25519 scheme and with the
    /// three crate-private helpers (`start_test_network_with_peers`,
    /// `register_validators`, `link_validators`) inlined.
    ///
    /// why this lives here: it's the bisect baseline for swapping our own
    /// Automaton/Relay/Reporter into the engine (CW.2b). proving the engine
    /// finalizes with commonware's mocks first isolates "does simplex run under
    /// our test harness at all" from "do our traits satisfy its contracts".
    ///
    /// n = 5 is the CANONICAL validator count from `all_online` (BFT needs
    /// n >= 3f+1 and a 2f+1 quorum; the all_online fixture uses 5). the wait
    /// mirrors all_online exactly: each validator's mock reporter exposes a
    /// `subscribe()` monitor that only emits on `finalization.view()`, so a
    /// completed wait IS proof of real finalization — not a stubbed assert.
    /// `Runner::timed(300s)` bounds simulated time, so a wiring bug surfaces as
    /// a deadline panic rather than a live-lock.
    #[test]
    fn simplex_finalizes_with_mocks() {
        use commonware_consensus::simplex::{
            config::{Config as SimplexConfig, Floor, ForwardingPolicy},
            elector::RoundRobin,
            mocks,
            scheme::ed25519 as simplex_ed25519,
            Engine,
        };
        use commonware_consensus::types::{Epoch, View, ViewDelta};
        use commonware_consensus::Monitor as _;
        use commonware_cryptography::Sha256;
        use commonware_parallel::Sequential;
        use commonware_runtime::buffer::paged::CacheRef;
        use commonware_utils::{NZUsize, NZU16};
        use std::sync::Arc;

        // canonical fixture params, lifted verbatim from all_online.
        let n: u32 = 5;
        let required_containers = View::new(100);
        let activity_timeout = ViewDelta::new(10);
        let skip_timeout = ViewDelta::new(5);
        let namespace = b"consensus".to_vec();
        let page_size = NZU16!(1024);
        let page_cache_size = NZUsize!(10);

        let executor = deterministic::Runner::timed(Duration::from_secs(300));
        executor.start(|mut context| async move {
            // build the ed25519 scheme fixture: n sorted participants, each with
            // a per-validator simplex scheme instance. `deterministic::Context`
            // is the rng source (it impls RngCore + CryptoRng).
            let fixture = simplex_ed25519::fixture(&mut context, &namespace, n);
            let participants = fixture.participants.clone();
            let schemes = fixture.schemes.clone();

            // stand up the simulated network seeded with the participant set
            // (the all_online path: `new_with_peers`, NOT new + manual track).
            // links are instant/deterministic — no dial/handshake/discovery.
            let (network, oracle) = simulated::Network::new_with_peers(
                context.child("network"),
                simulated::Config {
                    max_size: 1024 * 1024,
                    disconnect_on_block: true,
                    tracked_peer_sets: NZUsize!(1),
                },
                participants.clone(),
            )
            .await;
            network.start();

            // register all validators: three channels each — vote(0),
            // certificate(1), resolver(2) — in that order, since the engine's
            // `start(vote, cert, resolver)` consumes them positionally.
            let quota = Quota::per_second(NZU32!(128));
            let mut registrations = std::collections::HashMap::new();
            for validator in participants.iter() {
                let control = oracle.control(validator.clone());
                let vote = control
                    .register(0, quota)
                    .await
                    .expect("register vote channel");
                let certificate = control
                    .register(1, quota)
                    .await
                    .expect("register certificate channel");
                let resolver = control
                    .register(2, quota)
                    .await
                    .expect("register resolver channel");
                registrations.insert(validator.clone(), (vote, certificate, resolver));
            }

            // link every ordered pair of distinct validators with a perfect
            // link (success_rate 1.0) so votes/certs propagate deterministically.
            let link = Link {
                latency: Duration::from_millis(10),
                jitter: Duration::from_millis(1),
                success_rate: 1.0,
            };
            for v1 in participants.iter() {
                for v2 in participants.iter() {
                    if v1 == v2 {
                        continue;
                    }
                    oracle
                        .add_link(v1.clone(), v2.clone(), link.clone())
                        .await
                        .expect("link validators");
                }
            }

            // build + start one engine per validator. all engines share a single
            // mock relay (the proposed-payload broadcast bus); each gets its own
            // mock application (automaton + relay role) and mock reporter.
            let elector = RoundRobin::<Sha256>::default();
            let relay = Arc::new(mocks::relay::Relay::new());
            let mut reporters = Vec::new();
            let mut engine_handlers = Vec::new();
            for (idx, validator) in participants.iter().enumerate() {
                let v_ctx = context.child("validator");

                let reporter_config = mocks::reporter::Config {
                    participants: participants.clone().try_into().unwrap(),
                    scheme: schemes[idx].clone(),
                    elector: elector.clone(),
                };
                let reporter =
                    mocks::reporter::Reporter::new(v_ctx.child("reporter"), reporter_config);
                reporters.push(reporter.clone());

                let application_cfg = mocks::application::Config {
                    hasher: Sha256::default(),
                    relay: relay.clone(),
                    me: validator.clone(),
                    propose_latency: (10.0, 5.0),
                    verify_latency: (10.0, 5.0),
                    certify_latency: (10.0, 5.0),
                    should_certify: mocks::application::Certifier::Always,
                };
                let (actor, application) = mocks::application::Application::new(
                    v_ctx.child("application"),
                    application_cfg,
                );
                // the application actor runs as its own task; we deliberately
                // drop its handle (mirror all_online) — only the engine handles
                // must be kept alive.
                actor.start();

                let blocker = oracle.control(validator.clone());
                let cfg = SimplexConfig {
                    scheme: schemes[idx].clone(),
                    elector: elector.clone(),
                    blocker,
                    automaton: application.clone(),
                    relay: application.clone(),
                    reporter: reporter.clone(),
                    strategy: Sequential,
                    partition: validator.to_string(),
                    mailbox_size: NZUsize!(1024),
                    epoch: Epoch::new(333),
                    floor: Floor::Genesis(mocks::application::genesis::<Sha256>(Epoch::new(333))),
                    leader_timeout: Duration::from_secs(1),
                    certification_timeout: Duration::from_secs(2),
                    timeout_retry: Duration::from_secs(10),
                    fetch_timeout: Duration::from_secs(1),
                    activity_timeout,
                    skip_timeout,
                    fetch_concurrent: NZUsize!(4),
                    replay_buffer: NZUsize!(1024 * 1024),
                    write_buffer: NZUsize!(1024 * 1024),
                    page_cache: CacheRef::from_pooler(&context, page_size, page_cache_size),
                    forwarding: ForwardingPolicy::Disabled,
                };
                let engine = Engine::new(v_ctx.child("engine"), cfg);

                let (vote, certificate, resolver) = registrations
                    .remove(validator)
                    .expect("validator should be registered");
                // KEEP the handle alive — dropping a returned engine handle can
                // abort the engine task and stall finalization.
                engine_handlers.push(engine.start(vote, certificate, resolver));
            }

            // await each validator's reporter monitor until `required_containers`
            // finalizes. the reporter only pushes on `finalization.view()`, so a
            // completed loop proves a real `Activity::Finalization` was reported.
            let mut finalizers = Vec::new();
            for reporter in reporters.iter_mut() {
                let (mut latest, mut monitor) = reporter.subscribe().await;
                finalizers.push(context.child("finalizer").spawn(move |_| async move {
                    while latest < required_containers {
                        latest = monitor.recv().await.expect("finalization event missing");
                    }
                }));
            }
            // sequential await: each finalizer future resolves once its validator
            // crosses the target view; all engines run concurrently regardless,
            // so this converges and avoids a `futures::join_all` dep.
            for finalizer in finalizers {
                finalizer.await.expect("finalizer task joined");
            }

            // sanity: no equivocation/invalid-signature faults were observed on
            // the path to finalization (cheap, public reporter asserts).
            for reporter in reporters.iter() {
                reporter.assert_no_faults();
                reporter.assert_no_invalid();
            }

            // keep engines alive until the very end (drop is the implicit abort).
            drop(engine_handlers);
        });
    }
}
