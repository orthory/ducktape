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

use std::future::Future;
use std::net::SocketAddr;

use commonware_cryptography::{ed25519, Signer};
use commonware_p2p::{
    authenticated::discovery::{self, Network},
    Manager, Recipients,
};
use commonware_runtime::{
    BufferPooler, Clock, IoBuf, Metrics, Network as RNetwork, Quota, Resolver, Spawner,
};
use commonware_utils::{ordered::Set, NZU32};
use op::Lane;
use rand_core::CryptoRngCore;
use tokio::sync::mpsc;
use transport::{Error, Inbound, Transport};

/// the p2p channel index we register the broadcast op-gossip stream on.
const CHANNEL_BROADCAST: u64 = 0;

/// the p2p channel index for the consensus lane (p1.2, tier C). a SEPARATE
/// channel from broadcast so the inbound drain can tag the lane correctly and
/// so consensus traffic is wire-distinguishable. when the simplex `Engine`
/// lands, this is the channel its vote/certificate/resolver sub-streams will be
/// derived from (or it grows into three; see [`consensus`]).
const CHANNEL_CONSENSUS: u64 = 1;

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
        + Metrics
        + Send
        + Sync
        + 'static,
{
    /// stand up a commonware node on the given runtime `context`.
    ///
    /// returns the transport handle and the inbound receiver (the same
    /// `(Lane, Vec<u8>)` tuple shape the trait's inbound side uses). the network
    /// is started before returning, and a background task is spawned to drain
    /// inbound gossip into the returned receiver.
    pub fn new(context: E, cfg: Config) -> (Self, mpsc::Receiver<Inbound>) {
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

        // register the authorized peer set at PEER_SET. `track` is synchronous
        // (enqueues onto the tracker mailbox); the set must include our own key
        // so every node agrees on the sorted ordering.
        let peer_set: Set<ed25519::PublicKey> =
            Set::try_from(cfg.peers).expect("authorized peer set has no duplicates");
        oracle.track(PEER_SET, peer_set);

        // register both gossip channels. quota caps inbound receive rate.
        let broadcast = network.register(
            CHANNEL_BROADCAST,
            Quota::per_second(NZU32!(128)),
            MAX_BACKLOG,
        );
        let consensus = network.register(
            CHANNEL_CONSENSUS,
            Quota::per_second(NZU32!(128)),
            MAX_BACKLOG,
        );

        // wire the two channel pairs into a dialect-agnostic transport (spawns
        // the inbound drains, holds the senders). the registered receivers
        // buffer up to MAX_BACKLOG regardless, so it's fine to start the network
        // after building the transport.
        let (transport, in_rx) =
            Self::from_channels(context.child("transport"), broadcast, consensus);

        // start the network actors (dialer, listener, router, tracker, ...).
        network.start();

        (transport, in_rx)
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
            let wire = vec![Op::Vcs(vcs::op::Op::Init)];
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
            assert!(matches!(ops[0], Op::Vcs(vcs::op::Op::Init)));
        });
    }

    /// CW.2b — the REAL consensus lane: a commonware simplex `Engine` reaching
    /// `Activity::Finalization`, driven by OUR own
    /// [`consensus::ConsensusAutomaton`] / [`consensus::ConsensusRelay`] /
    /// [`consensus::ConsensusReporter`] (not commonware's mocks), with the
    /// finalized op-batch bytes resolved through OUR [`consensus::ContentStore`]
    /// and delivered into an inbound mpsc tagged `Lane::Consensus`.
    ///
    /// this is the production-shaped round trip: `store.put(bytes)` →
    /// `automaton.enqueue(digest)` on a designated PROPOSER, the engine orders
    /// that digest via BFT consensus, and the proposer's `ConsensusReporter`
    /// resolves the finalized digest back to bytes and forwards `(Lane::Consensus,
    /// bytes)` on finalization. it keeps CW.2a's wiring (same simulated network,
    /// same canonical n=5, same per-validator real `Engine::new`/`start`) and
    /// only swaps the application/relay/reporter triple.
    ///
    /// finalization proof: `ConsensusReporter::report` forwards onto the inbound
    /// mpsc ONLY in the `Activity::Finalization` arm (notarizations/votes never
    /// touch it). so `inbound_rx.recv().await` returning the proposed bytes is
    /// itself proof that a real `Activity::Finalization` was reported by an actual
    /// `Engine` — no View(100) wait needed. `Runner::timed(300s)` is the liveness
    /// guard: if nothing ever finalizes, the deadline panics rather than hanging.
    ///
    /// per decision #3 we assert on the PROPOSER (whose `ContentStore` holds the
    /// bytes). peer nodes resolve `store.get(digest) -> None` and drop — cross-node
    /// payload delivery via the resolver channel is out of scope here.
    ///
    /// non-proposer leaders propose against an empty queue, so their `propose`
    /// drops the sender → that view nullifies (leader_timeout) and advances. that
    /// is standard simplex liveness; within at most n views the proposer is
    /// leader, pops its one digest, and perfect links + our `verify -> true` mean
    /// every validator notarizes + finalizes that view.
    #[test]
    fn simplex_finalizes_and_delivers_on_proposer() {
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
            // that order; the engine's `start(vote, cert, resolver)` consumes them
            // positionally.
            let quota = Quota::per_second(NZU32!(128));
            let mut registrations = std::collections::HashMap::new();
            for validator in participants.iter() {
                let control = oracle.control(validator.clone());
                let vote = control.register(0, quota).await.expect("register vote");
                let certificate = control
                    .register(1, quota)
                    .await
                    .expect("register certificate");
                let resolver = control.register(2, quota).await.expect("register resolver");
                registrations.insert(validator.clone(), (vote, certificate, resolver));
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

            // the PROPOSER: participant 0. it (and only it) gets the op-batch
            // staged into its ContentStore + enqueued for proposal, and its
            // reporter is wired to an inbound mpsc we read in the test.
            let proposer = participants[0].clone();

            // the op-batch we expect to see finalized + delivered. a Vcs::Commit
            // is a consensus-lane op (`op::Op::lane()` routes Vcs to Consensus).
            let wire = vec![Op::Vcs(vcs::op::Op::Commit {
                message: "consensus".into(),
                author: "proposer".into(),
            })];
            let proposed_bytes = encode_batch(&wire);

            // the proposer's inbound side: ConsensusReporter forwards finalized
            // bytes here on `Activity::Finalization`. recv on this rx is the
            // finalization proof.
            let (proposer_in_tx, mut proposer_in_rx) = mpsc::channel::<Inbound>(MAX_BACKLOG);

            // build + start one engine per validator with OUR triple.
            let elector = RoundRobin::<Sha256>::default();
            let mut engine_handlers = Vec::new();
            for (idx, validator) in participants.iter().enumerate() {
                let v_ctx = context.child("validator");
                let is_proposer = *validator == proposer;

                // each validator owns its own content store. only the proposer's
                // gets the payload staged (peers resolve None on finalization and
                // drop — out of scope here).
                let store = ContentStore::new();

                // P = ed25519::PublicKey (the fixture's key) for Automaton/Relay.
                let automaton =
                    ConsensusAutomaton::<commonware_cryptography::ed25519::PublicKey>::new();
                if is_proposer {
                    let digest = store.put(proposed_bytes.clone());
                    automaton.enqueue(digest);
                }
                let relay =
                    ConsensusRelay::<commonware_cryptography::ed25519::PublicKey>::new(store.clone());

                // S = the type of schemes[idx] for ConsensusReporter::<S> (phantom,
                // named explicitly at construction). the proposer's reporter feeds
                // the inbound rx we keep; peers get throwaway senders.
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
                // KEEP the handle alive — dropping it can abort the engine task.
                engine_handlers.push(engine.start(vote, certificate, resolver));
            }

            // await the finalized delivery. recv resolving with the proposed bytes
            // means the proposer's reporter hit `Activity::Finalization`, resolved
            // the digest in its ContentStore, and forwarded the op-batch — a REAL
            // simplex finalization round trip. (timed runner panics on no-show.)
            let (lane, recv) = proposer_in_rx
                .recv()
                .await
                .expect("proposer receives a finalized consensus batch");
            assert_eq!(lane, Lane::Consensus);
            let ops = decode_batch(&recv).expect("decode finalized batch");
            assert_eq!(ops.len(), 1);
            assert!(matches!(ops[0], Op::Vcs(vcs::op::Op::Commit { .. })));
            // exact round-trip: finalized bytes are byte-identical to what we put.
            assert_eq!(recv, proposed_bytes);

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
    /// this is strictly MORE than [`simplex_finalizes_and_delivers_on_proposer`]:
    /// that test pokes `store.put` + `automaton.enqueue` directly, bypassing the
    /// transport; here the ONLY way the digest reaches the engine is through
    /// `transport.send`, so a pass proves the SEND-PATH wiring (not just the
    /// engine) is correct. everything else is identical — n=5, simulated network,
    /// per-validator real `Engine::new`/`start`, our Automaton/Relay/Reporter
    /// triple, proposer-only `ContentStore`. cross-node delivery to non-proposers
    /// stays out of scope (that's #2: Relay/resolver).
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
            // order engine.start consumes positionally.
            let quota = Quota::per_second(NZU32!(128));
            let mut registrations = std::collections::HashMap::new();
            for validator in participants.iter() {
                let control = oracle.control(validator.clone());
                let vote = control.register(0, quota).await.expect("register vote");
                let certificate = control
                    .register(1, quota)
                    .await
                    .expect("register certificate");
                let resolver = control.register(2, quota).await.expect("register resolver");
                registrations.insert(validator.clone(), (vote, certificate, resolver));
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
            let wire = vec![Op::Vcs(vcs::op::Op::Commit {
                message: "consensus".into(),
                author: "proposer".into(),
            })];
            let proposed_bytes = encode_batch(&wire);

            let (proposer_in_tx, mut proposer_in_rx) = mpsc::channel::<Inbound>(MAX_BACKLOG);

            let elector = RoundRobin::<Sha256>::default();
            let mut engine_handlers = Vec::new();
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

                let relay =
                    ConsensusRelay::<commonware_cryptography::ed25519::PublicKey>::new(store.clone());

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
            assert!(matches!(ops[0], Op::Vcs(vcs::op::Op::Commit { .. })));
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

        const N: usize = 3;
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
                let (transport, inbox) = CommonwareTransport::new(node_ctx, cfg);
                transports.push(transport);
                inboxes.push(inbox);
            }

            // the batch node 0 gossips to ALL peers.
            let wire = vec![Op::Vcs(vcs::op::Op::Init)];
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
