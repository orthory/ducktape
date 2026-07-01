//! a runnable multi-process ducktape node over REAL sockets.
//!
//! this is the in-sim N-validator simplex proof (consensus/tests/
//! simplex_agreed_order.rs) turned into an actual network: instead of N
//! `SimplexOrderer`s over ONE `p2p::simulated` network under the DETERMINISTIC
//! clock, each process here stands up its OWN live simplex `Engine` over a real
//! `authenticated::discovery` encrypted TCP mesh on the REAL tokio runtime, and
//! drives an `OrderedNode<SimplexOrderer>` over a `host::Host`.
//!
//! the machinery is REUSED verbatim: `consensus::SimplexOrderer::spawn` is
//! already generic over the runtime context + the three engine channel pairs, so
//! the only substrate that changes vs the sim is (a) `tokio::Runner` instead of
//! `deterministic::Runner` (discovery live-locks under the deterministic clock),
//! (b) `discovery::Network` channels instead of `simulated::Network`, and (c) a
//! per-process `ContentStore`.
//!
//! payload dissemination is sidestepped honestly by CONTENT-ADDRESSING: every
//! node submits the byte-identical startup op, so every process independently
//! `put`s the same bytes under the same sha256 digest into its own store. when
//! simplex finalizes that digest each node's reporter resolves it locally — no
//! shared store, no gossip relay needed (the crate's `NoopRelay` is correct
//! as-is). this removes the payload-availability problem; it does NOT remove the
//! cross-process AGREEMENT requirement — with N nodes, quorum votes still have to
//! cross the real TCP mesh before anything finalizes.
//!
//! each process prints two greppable lines: its GENESIS app-hash at startup and
//! its CONVERGED app-hash the first time consensus moves it off genesis. the demo
//! script asserts every process's genesis line agrees (no pre-op fork) and every
//! process's converged line agrees (real cross-process BFT convergence).

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use commonware_consensus::simplex::scheme::ed25519 as simplex_ed25519;
use commonware_consensus::types::Epoch;
use commonware_cryptography::{ed25519, Signer};
use commonware_p2p::authenticated::discovery::{self, Network};
use commonware_p2p::Manager;
use commonware_runtime::{Clock, Quota, Runner, Supervisor};
use commonware_utils::{ordered::Set, NZU32};

use consensus::{digest_of, ContentStore, Digest, SimplexOrderer};
use directory::Directory;
use host::Host;
use kv::Kv;
use kv_interface::{encode, KvMsg};
use node::OrderedNode;
use sdk::{Msg, StateRoot};

/// the peer-set index. every node must `track` the same authorized set at the
/// same index for discovery's bit-vector gossip to line up.
const PEER_SET: u64 = 0;
/// max wire message size we accept on a channel (1 MiB) — generous for the small
/// json frames + BFT metadata.
const MAX_MESSAGE_SIZE: u32 = 1 << 20;
/// inbound backlog before a channel applies receive backpressure.
const MAX_BACKLOG: usize = 128;

/// the plain-data node config, parsed from toml. mirrors legacy examples/node*.toml
/// (seed identity, listen/advertised addrs, shared namespace + authorized peer
/// seeds, a bootstrapper addr for non-zero nodes, an isolated storage root).
#[derive(serde::Deserialize)]
struct NodeConfig {
    /// ed25519 identity seed (dev): the identity is `PrivateKey::from_seed(id)`.
    id: u64,
    /// address to bind/listen on.
    listen: String,
    /// address advertised to peers for dialing; defaults to `listen`.
    advertised: Option<String>,
    /// application namespace — MUST match across the mesh (domain-separates the
    /// discovery handshake, the simplex scheme, and the genesis floor).
    namespace: String,
    /// the authorized participant set as identity seeds; every node lists the
    /// SAME set (including its own), sorted into the ed25519 participant `Set`.
    peer_seeds: Vec<u64>,
    /// node 0's dialable address. required for `id != 0` (they bootstrap off it).
    bootstrapper_addr: Option<String>,
    /// per-process FS storage root. REQUIRED to be distinct per process: the qmdb
    /// `kv` module uses a fixed "kv" partition, so two processes sharing a root
    /// would corrupt each other's state. defaults to a per-id temp dir.
    storage_dir: Option<String>,
}

/// hex-encode a state root for a stable, greppable log line.
fn hex(root: &StateRoot) -> String {
    let mut s = String::with_capacity(root.0.len() * 2);
    for b in root.0.iter() {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // parse `--config <path>`.
    let mut args = std::env::args().skip(1);
    let mut cfg_path: Option<PathBuf> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--config" => cfg_path = args.next().map(PathBuf::from),
            other => return Err(format!("unexpected arg {other:?} (want --config <path>)").into()),
        }
    }
    let cfg_path = cfg_path.ok_or("missing --config <path>")?;
    let cfg: NodeConfig = toml::from_str(&std::fs::read_to_string(&cfg_path)?)?;

    run_node(cfg)
}

/// stand up the real-socket node from `cfg` and run it until killed.
///
/// deliberately NOT `#[tokio::main]`: `tokio::Runner` owns its OWN tokio runtime,
/// and you cannot start a runtime from inside one. so `main` is sync and hands
/// off to `Runner::start`, which drives everything (including the engine's spawned
/// tasks) on the runtime it owns.
fn run_node(cfg: NodeConfig) -> Result<(), Box<dyn std::error::Error>> {
    let id = cfg.id;
    let listen: SocketAddr = cfg.listen.parse()?;
    let advertised: SocketAddr = match cfg.advertised.as_deref() {
        Some(a) => a.parse()?,
        None => listen,
    };
    let namespace = cfg.namespace.clone().into_bytes();

    // each seed -> an ed25519 identity; together the authorized participant set.
    let peers: Vec<ed25519::PublicKey> = cfg
        .peer_seeds
        .iter()
        .map(|s| ed25519::PrivateKey::from_seed(*s).public_key())
        .collect();

    // node 0 bootstraps nobody; everyone else dials node 0 (= peer_seeds[0]).
    let bootstrappers: Vec<(ed25519::PublicKey, _)> = if id == 0 {
        Vec::new()
    } else {
        let boot_seed = *cfg
            .peer_seeds
            .first()
            .ok_or("a bootstrapping node needs peer_seeds[0] = node 0")?;
        let boot_key = ed25519::PrivateKey::from_seed(boot_seed).public_key();
        let boot_addr: SocketAddr = cfg
            .bootstrapper_addr
            .as_deref()
            .ok_or("a non-zero node needs bootstrapper_addr set")?
            .parse()?;
        vec![(boot_key, boot_addr.into())]
    };

    // per-process storage isolation (see NodeConfig::storage_dir).
    let storage = cfg
        .storage_dir
        .clone()
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join(format!("ducktape-node-{id}")));

    println!(
        "[node #{id}] starting on {listen} ({} peers), storage {}",
        cfg.peer_seeds.len(),
        storage.display()
    );

    // run on commonware's OWN tokio runtime, rooted at our per-process storage dir.
    let rt_cfg = commonware_runtime::tokio::Config::default().with_storage_directory(storage);
    let executor = commonware_runtime::tokio::Runner::new(rt_cfg);

    executor.start(|context| async move {
        let signer = ed25519::PrivateKey::from_seed(id);

        // the authorized participant set, SORTED — shared by discovery (the
        // tracked peer set) AND the simplex scheme (participant indices line up).
        let participants: Set<ed25519::PublicKey> =
            Set::try_from(peers).expect("authorized peer set has no duplicates");

        // the real encrypted TCP mesh. `local` is the dev preset (allows private
        // ips). MUST be the real tokio runtime — discovery live-locks under the
        // deterministic clock.
        let p2p_cfg = discovery::Config::local(
            signer.clone(),
            &namespace,
            listen,
            advertised,
            bootstrappers,
            MAX_MESSAGE_SIZE,
        );
        let (mut network, mut oracle) = Network::new(context.child("network"), p2p_cfg);
        oracle.track(PEER_SET, participants.clone());

        // the simplex engine's three sub-channels, consumed positionally by
        // `engine.start(vote, certificate, resolver)`. no payload/fetch channels —
        // content-addressing (identical op on every node) makes the NoopRelay
        // correct, so dissemination is unnecessary.
        let quota = Quota::per_second(NZU32!(128));
        let vote = network.register(0, quota, MAX_BACKLOG);
        let certificate = network.register(1, quota, MAX_BACKLOG);
        let resolver = network.register(2, quota, MAX_BACKLOG);

        // start the network actors (dialer/listener/router/tracker). registered
        // receivers buffer regardless, so starting before the engine is fine.
        network.start();

        // genesis host: the SAME module set on every node -> identical genesis
        // app-hash. qmdb `kv` (order-dependent root) + in-memory `directory`.
        let kv = Kv::init(context.child("kv"), "kv").await;
        let host = Host::genesis(vec![Box::new(kv), Box::new(Directory::new("directory"))])
            .expect("genesis host");

        // scheme built the production way: `signer` finds our key's index in the
        // sorted participant set, so we sign as exactly the participant our
        // discovery identity represents. (NOT the mocks-gated `fixture`.)
        let scheme =
            simplex_ed25519::Scheme::signer(&namespace, participants.clone(), signer.clone())
                .expect("our key is in the authorized participant set");

        // genesis floor: domain-separated by namespace so every node in THIS app
        // computes the identical digest (else engines never agree -> hang). NOT
        // the mocks-gated `mocks::application::genesis`.
        let genesis_floor: Digest =
            digest_of(&[b"ducktape:consensus:genesis:v1:".as_ref(), &namespace].concat());

        // per-process content store (NOT shared across processes). the identical
        // startup op means every process holds the finalized digest's bytes.
        let store = ContentStore::new();

        // the live simplex Engine, wired to a fresh SimplexOrderer — REUSED
        // verbatim from the sim. the discovery Oracle IS the Blocker; pubkey hex
        // is an FS-safe per-node partition. the engine keepalive handle lives
        // inside the returned orderer (held by the OrderedNode below, which the
        // loop never drops — so the engine never aborts).
        let orderer = SimplexOrderer::spawn(
            context.child("consensus"),
            scheme,
            oracle.clone(),
            signer.public_key().to_string(),
            Epoch::new(0),
            genesis_floor,
            store,
            vote,
            certificate,
            resolver,
        );
        let mut node = OrderedNode::new(host, orderer);

        // the genesis app-hash BEFORE any op — the demo asserts this agrees across
        // processes (a fork here would be a genesis-determinism bug, not consensus).
        let genesis_hash = node.app_hash();
        println!("[node #{id}] genesis app_hash={}", hex(&genesis_hash));

        // introduce the op: EVERY node submits the byte-identical startup frame.
        // identical (origin, seq, msg) -> identical frame -> identical digest, so
        // every node's store already holds the bytes when that digest finalizes.
        // ONE submit — the automaton PEEKS (never pops), so the digest rides out
        // every nullified early view until the mesh forms and a leader proposes it
        // (no resend loop needed — that was the gossip lane's crutch).
        let op = Msg {
            target: "kv".into(),
            payload: encode(&KvMsg::Set { key: b"startup".to_vec(), value: b"1".to_vec() }),
        };
        node.submit(b"startup", 0, op).await.expect("submit startup op");

        // pump: drain finalized frames on an interval and apply them in agreed
        // (ascending-view) order. print ONCE the first time the app-hash moves off
        // genesis — that only happens after a real simplex finalization delivered
        // the op across the mesh. this infinite loop IS the "run forever" park.
        let mut converged = false;
        loop {
            context.sleep(Duration::from_millis(100)).await;
            node.drain_delivered().await.expect("drain delivered");
            let h = node.app_hash();
            if !converged && h != genesis_hash {
                println!("[node #{id}] converged app_hash={}", hex(&h));
                converged = true;
            }
        }
    });

    Ok(())
}
