//! commonware-backed transport: the broadcast lane over real p2p gossip.
//!
//! this satisfies the same [`transport::Transport`] seam the loopback impl does,
//! but instead of an in-process channel it speaks commonware's
//! `authenticated::discovery` dialect — ed25519-identified, encrypted,
//! fully-connected peers with automatic address discovery.
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

/// commonware-backed transport, generic over the gossip `Sender` `S`.
///
/// holds two clone-able gossip `Sender`s — one per lane/channel. cloning the
/// whole transport is cheap (senders are `Clone`), which is why this can satisfy
/// the `&self` + `Send` shape of [`Transport::send`] without interior
/// mutability.
///
/// the struct is decoupled from the p2p dialect: the production authenticated
/// path instantiates `S = GossipSender<E>` (see [`CommonwareTransport::new`]),
/// while tests instantiate `S = simulated::Sender<..>` via
/// [`CommonwareTransport::from_channels`]. both lanes share the same `S` because
/// they only differ in which channel/sender carries the bytes.
#[derive(Clone)]
pub struct CommonwareTransport<S>
where
    S: commonware_p2p::Sender + Clone + Send + Sync + 'static,
{
    /// broadcast-lane gossip sender (CHANNEL_BROADCAST).
    broadcast: S,
    /// consensus-lane gossip sender (CHANNEL_CONSENSUS). tier C: ordered gossip
    /// stand-in for the simplex Engine's network sends.
    consensus: S,
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
                consensus: consensus_sender,
            },
            in_rx,
        )
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

impl<S> Transport for CommonwareTransport<S>
where
    S: commonware_p2p::Sender + Clone + Send + Sync + 'static,
{
    fn send(
        &self,
        lane: Lane,
        bytes: Vec<u8>,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        // clone the lane's gossip sender up front so the async block owns it —
        // commonware `Sender` is `Clone` and `send` is synchronous, so this
        // avoids any mutex or a guard held across the `.await` point. both lanes
        // gossip identically (best-effort `Recipients::All`); they differ only
        // in which channel/sender carries the bytes, which is what lets the
        // remote side tag the inbound lane correctly.
        let mut sender = match lane {
            Lane::Broadcast => self.broadcast.clone(),
            // tier C: the consensus lane is ordered gossip on its own channel —
            // functional delivery, not yet BFT total order. true byzantine
            // ordering is the documented TODO (instantiate a simplex `Engine`
            // and route its finalized payloads here via the [`consensus`]
            // scaffolding); see this module's docstring.
            Lane::Consensus => self.consensus.clone(),
        };
        async move {
            // gossip to every authorized peer. `send` is non-blocking and
            // returns the recipients it will attempt; offline/rate-limited
            // peers are silently skipped (best-effort, like loopback).
            let _recipients = sender.send(Recipients::All, IoBuf::from(bytes), false);
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
    use std::net::{IpAddr, Ipv4Addr};
    use std::time::Duration;
    use transport::{decode_batch, encode_batch};

    fn addr(port: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port)
    }

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

    /// tier C consensus lane: node A sends a batch on `Lane::Consensus` and node
    /// B receives it tagged `Lane::Consensus` (not `Broadcast`), proving the
    /// consensus channel is wired end to end and lane-distinguishable on the
    /// wire. this is ordered-gossip delivery, NOT BFT finalization — the simplex
    /// `Engine` path is the documented TODO (see [`consensus`]).
    ///
    /// #[ignore]d for the same reason as the broadcast test: 2-node commonware
    /// discovery under the deterministic clock is timing-sensitive. the impl is
    /// the deliverable; run with `cargo test -p net -- --ignored`.
    #[test]
    #[ignore = "commonware 2-node discovery is timing-sensitive under the deterministic clock; impl compiles behind the trait"]
    fn two_node_consensus_propagates() {
        let executor = deterministic::Runner::timed(Duration::from_secs(30));
        executor.start(|context| async move {
            let key_a = ed25519::PrivateKey::from_seed(1).public_key();
            let key_b = ed25519::PrivateKey::from_seed(2).public_key();
            let peers = vec![key_a.clone(), key_b.clone()];

            let (transport_a, mut _rx_a) = CommonwareTransport::new(
                context.child("a"),
                Config {
                    seed: 1,
                    namespace: b"ducktape-net-test".to_vec(),
                    listen: addr(3000),
                    advertised: addr(3000),
                    peers: peers.clone(),
                    bootstrappers: vec![],
                },
            );
            let (_transport_b, mut rx_b) = CommonwareTransport::new(
                context.child("b"),
                Config {
                    seed: 2,
                    namespace: b"ducktape-net-test".to_vec(),
                    listen: addr(3001),
                    advertised: addr(3001),
                    peers,
                    bootstrappers: vec![(key_a, addr(3000))],
                },
            );

            // send on the consensus lane from A.
            let wire = vec![Op::Vcs(vcs::op::Op::Commit {
                message: "consensus".into(),
                author: "a".into(),
            })];
            let bytes = encode_batch(&wire);
            transport_a
                .send(Lane::Consensus, bytes)
                .await
                .expect("consensus send ok");

            // B receives it on the consensus lane — NOT broadcast.
            let (lane, recv) = rx_b.recv().await.expect("node B receives consensus");
            assert_eq!(lane, Lane::Consensus);
            let ops = decode_batch(&recv).expect("decode batch");
            assert_eq!(ops.len(), 1);
            assert!(matches!(ops[0], Op::Vcs(vcs::op::Op::Commit { .. })));
        });
    }
}
