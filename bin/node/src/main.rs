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
//! payload dissemination is REAL: each process submits a DISTINCT op (node N
//! writes directory key `kN`), so a peer that finalizes another node's op-digest
//! has NO local bytes for it. `SimplexOrderer::spawn_with_relay` wires a
//! `ConsensusRelay` that, at propose time, gossips the proposed frame's bytes to
//! all peers on `CHANNEL_PAYLOAD`; every peer's STORE-ONLY drain caches them, so
//! when that digest finalizes the reporter resolves it locally and delivers it in
//! BFT order. content-addressing IS the verification (the drain re-hashes on
//! receipt). this is what lets DISTINCT ops converge across processes with
//! per-process stores — quorum votes still cross the real TCP mesh to finalize.
//!
//! ## state-sync service and the sync-only joiner
//!
//! every validator also serves the statesync wire protocol on
//! `CHANNEL_STATE_SYNC`, answered between drains from its latest finalized
//! boundary — so responses are always block-consistent without locks. run with
//! `--sync-only` and the process joins the mesh WITHOUT a consensus engine,
//! pulls a manifest + every module from the bootstrapper over that channel,
//! rebuilds them against their consensus-committed roots, prints its composed
//! `synced app_hash=`, and exits 0 — the network-backed joiner path over real
//! sockets. membership note: `peer_seeds` is the AUTHORIZED MESH (everyone,
//! including sync-only joiners); `validator_seeds` (default: peer_seeds) is the
//! CONSENSUS participant set — the split that lets a non-validator sync.
//!
//! each validator prints its GENESIS app-hash at startup and its CONVERGED
//! app-hash once it has applied ALL validator ops. the demo script asserts every
//! process's genesis line agrees, every converged line agrees, and the sync-only
//! joiner's synced line equals the converged line.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use commonware_consensus::simplex::scheme::ed25519 as simplex_ed25519;
use commonware_consensus::types::Epoch;
use commonware_cryptography::{ed25519, Signer};
use commonware_p2p::authenticated::discovery::{self, Network};
use commonware_codec::DecodeExt as _;
use commonware_p2p::{Manager, Receiver as _, Recipients, Sender as _};
use commonware_runtime::{Clock, IoBuf, Quota, Runner, Spawner, Supervisor};
use commonware_utils::{ordered::Set, NZU32};
use futures::{FutureExt as _, StreamExt as _};

use consensus::{digest_of, ConsensusScheme, ContentStore, Digest, SimplexOrderer};

/// the consensus signature scheme this build runs — a genesis-wide constant. today only
/// V1 (ed25519); see [`ConsensusScheme`]'s rekey/respawn contract for the BLS/V2 path.
const CONSENSUS_SCHEME: ConsensusScheme = ConsensusScheme::V1Ed25519;
use agent::Agent;
use chat::Chat;
use directory::Directory;
use directory_interface::{decode_reply, encode_msg, encode_query, DirMsg, DirQuery, DirReply};
use document::Document;
use forge::Forge;
use governance::Governance;
use host::Host;
use kv::Kv;
use messaging::Messaging;
use node::OrderedNode;
use saga::SagaModule;
use sdk::{Msg, StateRoot};
use tasks::Tasks;
use valset::Valset;
use vaults::Vaults;
use statesync::p2p::P2pSyncClient;
use statesync::qmdb::RemoteQmdbResolver;
use statesync::{fetch_manifest, fetch_snapshot, SyncServer};

/// the peer-set index. every node must `track` the same authorized set at the
/// same index for discovery's bit-vector gossip to line up.
const PEER_SET: u64 = 0;
/// max wire message size we accept on a channel (1 MiB) — generous for the small
/// json frames + BFT metadata, and the statesync chunk size (256 KiB) plus
/// framing stays far below it.
const MAX_MESSAGE_SIZE: u32 = 1 << 20;
/// inbound backlog before a channel applies receive backpressure.
const MAX_BACKLOG: usize = 128;
/// the statesync rpc channel: joiners request manifests / snapshot chunks /
/// qmdb op-ranges here; validators answer between drains.
const CHANNEL_STATE_SYNC: u64 = 4;
/// how many epochs of engine channels are PRE-REGISTERED. discovery channels
/// can only be registered before `network.start()`, and every epoch's respawned
/// engine needs FRESH channels (an aborted old engine must never collide with
/// its successor) — so a bank is reserved up front. exhausting it is a
/// fail-stop: restart the mesh with a wider bank (a config/build constant, not
/// consensus state).
const EPOCH_CHANNEL_BANK: u64 = 16;
/// finalized views between OBSERVING a membership change and CUTTING OVER —
/// the grace window in which every honest node sees the same change and arms
/// the same deterministic discard ceiling. small for the demo network; a
/// production mesh would size this in minutes of views.
const CUTOVER_DELAY: u64 = 3;

/// the four channels epoch `e`'s engine uses: vote, certificate, resolver, and
/// the eager payload-relay lane. starts at 8, clear of the statesync channel.
fn engine_channels(epoch: u64) -> (u64, u64, u64, u64) {
    let base = 8 + epoch * 4;
    (base, base + 1, base + 2, base + 3)
}

/// the per-epoch genesis floor: domain-separated by namespace AND epoch, so a
/// respawned engine can never confuse an old epoch's certificates with its own
/// (an old-epoch floor fails `Floor::assert` against the new epoch).
fn epoch_floor(namespace: &[u8], epoch: u64) -> Digest {
    digest_of(
        &[
            b"ducktape:consensus:genesis:v1:".as_ref(),
            namespace,
            b":epoch:",
            &epoch.to_le_bytes(),
        ]
        .concat(),
    )
}

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
    /// the authorized MESH participant set as identity seeds; every node lists
    /// the SAME set (including its own and any sync-only joiners), sorted into
    /// the ed25519 `Set` discovery tracks.
    peer_seeds: Vec<u64>,
    /// the CONSENSUS participant subset (identity seeds). defaults to
    /// `peer_seeds`. a seed in `peer_seeds` but not here is mesh-only — e.g. a
    /// sync-only joiner that may fetch state but casts no votes.
    validator_seeds: Option<Vec<u64>>,
    /// node 0's dialable address. required for `id != 0` (they bootstrap off it).
    bootstrapper_addr: Option<String>,
    /// per-process FS storage root. REQUIRED to be distinct per process: the qmdb
    /// `kv` module uses a fixed "kv" partition, so two processes sharing a root
    /// would corrupt each other's state. defaults to a per-id temp dir.
    storage_dir: Option<String>,
    /// local rpc listen address (json-lines over tcp) for op submission,
    /// queries, status, and shutdown. OFF when unset. bind loopback only —
    /// the rpc trusts its caller (it stamps this node's own origin on submits).
    rpc_listen: Option<String>,
}

/// read the valset module's current membership projection (committed state —
/// called between drains, outside any block).
async fn read_valset_members(host: &Host) -> Vec<Vec<u8>> {
    use valset_interface::{decode_reply, encode_query, ValsetQuery, ValsetReply};
    let Ok(reply) = host
        .query("valset", &encode_query(&ValsetQuery::Validators))
        .await
    else {
        return Vec::new();
    };
    match decode_reply(&reply) {
        Ok(ValsetReply::Validators(v)) => v,
        Err(_) => Vec::new(),
    }
}

/// hex-encode a state root for a stable, greppable log line.
fn hex(root: &StateRoot) -> String {
    hex_bytes(&root.0)
}

fn hex_bytes(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn unhex(s: &str) -> Result<Vec<u8>, String> {
    if s.len() % 2 != 0 {
        return Err("hex string has odd length".into());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

/// the PRODUCTION module set — genesis state, identical on every node (a
/// different set composes a different app-hash and the network forks at
/// genesis). system infrastructure (kv, valset seeded with the genesis
/// validators, saga) plus every product module. `forge_repo` is this node's
/// on-disk git substrate; wrapper modules run EMBEDDED substrates for now.
async fn genesis_host(
    context: &commonware_runtime::tokio::Context,
    forge_repo: &std::path::Path,
    genesis_validators: &[ed25519::PublicKey],
) -> Host {
    let kv = Kv::init(context.child("kv"), "kv").await;
    let document = Document::init(context.child("document"), "document").await;
    let messaging = Messaging::init(context.child("messaging"), "messaging").await;
    let chat = Chat::init(context.child("chat"), "chat").await;
    let agent =
        Agent::init_with_messaging_id(context.child("agent"), "agent", "agent-messaging").await;
    let forge = Forge::init("forge", forge_repo.to_path_buf()).expect("forge init");
    let mut valset = Valset::new("valset");
    // genesis-seed the validator set from config — deterministic and identical
    // on every node, so membership is IN consensus state from block zero (the
    // substrate epoch cutover + governance will drive).
    for v in genesis_validators {
        valset.insert(v.as_ref().to_vec());
    }
    Host::genesis(vec![
        Box::new(kv),
        Box::new(document),
        Box::new(messaging),
        Box::new(chat),
        Box::new(agent),
        Box::new(forge),
        Box::new(valset),
        // governance is the SOLE authorized author of valset changes: member
        // proposals + ballots, deterministic tally, follow-up membership ops.
        Box::new(Governance::new("governance", "valset")),
        Box::new(SagaModule::new("saga")),
        Box::new(Tasks::new("tasks")),
        Box::new(Vaults::new("vaults")),
        Box::new(Directory::new("directory")),
    ])
    .expect("genesis host")
}

// ============================================================================
// the local rpc: json-lines over tcp, bridged from blocking threads.
// ============================================================================

/// one rpc request, parsed from a json line.
#[derive(serde::Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
enum RpcRequest {
    /// submit an op into the ordered lane (accepted != finalized — poll status).
    Submit { target: String, payload_hex: String },
    /// read-only query against a module's committed+staged projection.
    Query { target: String, req_hex: String },
    /// node status: latest applied boundary + every module root.
    Status,
    /// graceful stop: replies ok, then exits 0 after the current pump turn.
    Shutdown,
}

#[derive(serde::Serialize)]
struct RpcReply {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reply_hex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<RpcStatus>,
}

#[derive(serde::Serialize)]
struct RpcStatus {
    height: Option<u64>,
    app_hash: String,
    modules: std::collections::BTreeMap<String, String>,
}

impl RpcReply {
    fn ok() -> Self {
        Self { ok: true, error: None, reply_hex: None, status: None }
    }
    fn err(msg: impl Into<String>) -> Self {
        Self { ok: false, error: Some(msg.into()), reply_hex: None, status: None }
    }
}

/// a parsed request plus the blocking thread's reply slot.
type RpcJob = (RpcRequest, std::sync::mpsc::Sender<RpcReply>);

/// serve json-lines rpc on `listener`, one OS thread per connection (local,
/// low-volume — an operator console, a script). each line becomes an [`RpcJob`]
/// pushed into the pump's bounded queue; the pump answers between drains, so
/// every reply reflects a block boundary. this runs on PLAIN OS THREADS: it
/// must never touch the async runtime, only the mpsc bridge.
fn spawn_rpc_listener(
    listener: std::net::TcpListener,
    bridge: futures::channel::mpsc::Sender<RpcJob>,
) {
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(conn) = conn else { continue };
            let mut bridge = bridge.clone();
            std::thread::spawn(move || {
                use std::io::{BufRead as _, BufReader, Write as _};
                let reader = BufReader::new(conn.try_clone().expect("clone rpc conn"));
                let mut conn = conn;
                for line in reader.lines() {
                    let Ok(line) = line else { break };
                    if line.trim().is_empty() {
                        continue;
                    }
                    let reply = match serde_json::from_str::<RpcRequest>(&line) {
                        Ok(req) => {
                            let (tx, rx) = std::sync::mpsc::channel();
                            if bridge.try_send((req, tx)).is_err() {
                                RpcReply::err("node busy (rpc queue full)")
                            } else {
                                // the pump answers within a tick; a stuck node
                                // must not park the operator's console forever.
                                rx.recv_timeout(std::time::Duration::from_secs(10))
                                    .unwrap_or_else(|_| RpcReply::err("node unresponsive"))
                            }
                        }
                        Err(e) => RpcReply::err(format!("bad request: {e}")),
                    };
                    let mut out = serde_json::to_string(&reply).expect("reply serializes");
                    out.push('\n');
                    if conn.write_all(out.as_bytes()).is_err() {
                        break;
                    }
                }
            });
        }
    });
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // parse `--config <path> [--sync-only]`.
    let mut args = std::env::args().skip(1);
    let mut cfg_path: Option<PathBuf> = None;
    let mut sync_only = false;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--config" => cfg_path = args.next().map(PathBuf::from),
            "--sync-only" => sync_only = true,
            other => {
                return Err(
                    format!("unexpected arg {other:?} (want --config <path> [--sync-only])")
                        .into(),
                )
            }
        }
    }
    let cfg_path = cfg_path.ok_or("missing --config <path>")?;
    let cfg: NodeConfig = toml::from_str(&std::fs::read_to_string(&cfg_path)?)?;

    // opt-in internals visibility: RUST_LOG=commonware_p2p=debug etc.
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    run_node(cfg, sync_only)
}

/// stand up the real-socket node from `cfg` and run it until killed (validator)
/// or until state sync completes (`--sync-only`).
///
/// deliberately NOT `#[tokio::main]`: `tokio::Runner` owns its OWN tokio runtime,
/// and you cannot start a runtime from inside one. so `main` is sync and hands
/// off to `Runner::start`, which drives everything (including the engine's spawned
/// tasks) on the runtime it owns.
fn run_node(cfg: NodeConfig, sync_only: bool) -> Result<(), Box<dyn std::error::Error>> {
    let id = cfg.id;
    let listen: SocketAddr = cfg.listen.parse()?;
    let advertised: SocketAddr = match cfg.advertised.as_deref() {
        Some(a) => a.parse()?,
        None => listen,
    };
    let namespace = cfg.namespace.clone().into_bytes();

    // each seed -> an ed25519 identity; together the authorized MESH set.
    let peers: Vec<ed25519::PublicKey> = cfg
        .peer_seeds
        .iter()
        .map(|s| ed25519::PrivateKey::from_seed(*s).public_key())
        .collect();
    // the consensus participant subset (defaults to the whole mesh).
    let validator_seeds = cfg
        .validator_seeds
        .clone()
        .unwrap_or_else(|| cfg.peer_seeds.clone());
    let validators: Vec<ed25519::PublicKey> = validator_seeds
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

    for (i, seed) in cfg.peer_seeds.iter().enumerate() {
        let pk = ed25519::PrivateKey::from_seed(*seed).public_key();
        println!("[node #{id}] peer[{i}] seed={seed} identity={}", hex_bytes(pk.as_ref()));
    }
    println!(
        "[node #{id}] starting on {listen} ({} mesh peers, {} validators{}), storage {}",
        cfg.peer_seeds.len(),
        validator_seeds.len(),
        if sync_only { ", sync-only" } else { "" },
        storage.display()
    );

    // the rpc listener binds OUTSIDE the runtime (plain std tcp on OS threads)
    // so a bind failure is a clean startup error, not an async surprise.
    let rpc_listener = match cfg.rpc_listen.as_deref() {
        Some(addr) if !sync_only => Some(std::net::TcpListener::bind(addr)?),
        _ => None,
    };

    // run on commonware's OWN tokio runtime, rooted at our per-process storage dir.
    let storage_for_sync = storage.clone();
    let rt_cfg = commonware_runtime::tokio::Config::default().with_storage_directory(storage);
    let executor = commonware_runtime::tokio::Runner::new(rt_cfg);

    executor.start(|context| async move {
        let signer = ed25519::PrivateKey::from_seed(id);

        // the authorized MESH set, SORTED — what discovery tracks. the
        // consensus scheme uses the (possibly smaller) validator set below.
        let mesh_participants: Set<ed25519::PublicKey> =
            Set::try_from(peers.clone()).expect("authorized peer set has no duplicates");
        let validator_participants: Set<ed25519::PublicKey> =
            Set::try_from(validators.clone()).expect("validator set has no duplicates");

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
        oracle.track(PEER_SET, mesh_participants.clone());

        let quota = Quota::per_second(NZU32!(128));

        if sync_only {
            // ---- the SYNC-ONLY joiner: no engine, no votes — just the wire ----
            //
            // validators broadcast consensus traffic (votes, certificates,
            // payload gossip) to EVERY tracked mesh peer, not only to fellow
            // participants — and a message on an UNREGISTERED channel is a
            // protocol violation that makes the peer actor kill the connection
            // (a permanent connect/kill loop that drops every rpc). so a
            // mesh-member-but-not-validator must register every channel and
            // black-hole the consensus lanes it does not consume.
            for epoch in 0..EPOCH_CHANNEL_BANK {
                let (vote, cert, res, payload) = engine_channels(epoch);
                for ch in [vote, cert, res, payload] {
                    let (_tx, mut rx) = network.register(ch, quota, MAX_BACKLOG);
                    let label: &'static str =
                        Box::leak(format!("blackhole_{ch}").into_boxed_str());
                    context.child(label).spawn(move |_ctx| async move {
                        while rx.recv().await.is_ok() {}
                    });
                }
            }
            let (sync_tx, sync_rx) = network.register(CHANNEL_STATE_SYNC, quota, MAX_BACKLOG);
            network.start();

            let server_peer = peers.first().expect("peer_seeds is non-empty").clone();
            let client = P2pSyncClient::new(
                context.child("sync_client"),
                sync_tx,
                sync_rx,
                server_peer,
            );

            // the mesh takes a moment to connect, and the server only serves
            // once it has a finalized boundary — retry until the manifest lands.
            let manifest = loop {
                match fetch_manifest(&client).await {
                    Ok(m) => break m,
                    Err(e) => {
                        println!("[node #{id}] manifest not ready ({e}); retrying");
                        context.sleep(Duration::from_millis(500)).await;
                    }
                }
            };
            println!(
                "[node #{id}] manifest height={} app_hash={}",
                manifest.height,
                hex(&manifest.app_hash)
            );

            // rebuild EVERY module in the manifest. a REAL joiner owns its
            // disk, so every store opens under its canonical module id.
            //
            // resolver lane: live target through the module lane (must sit at
            // the manifest boundary — a parked demo source guarantees it; a
            // busy source would be retried by refetching the manifest), then
            // merkle-verified op batches through the remote resolver.
            // snapshot lane: chunked bytes, install gated on the manifest root.
            let fetch_target = |module: &'static str| {
                let resolver = RemoteQmdbResolver::new(client.clone(), module);
                let entry_root = manifest.entry(module).expect("module in manifest").root;
                async move {
                    let target = resolver.fetch_target().await.expect("target");
                    assert_eq!(
                        StateRoot(target.root.0),
                        entry_root,
                        "parked source: live {module} target equals the manifest root"
                    );
                    (target, resolver)
                }
            };

            let (target, resolver) = fetch_target("kv").await;
            let kv = Kv::sync_from(context.child("kv"), "kv", target, resolver).await;

            let (target, resolver) = fetch_target("document").await;
            let document =
                Document::sync_from(context.child("document"), "document", target, resolver).await;

            let (target, resolver) = fetch_target("messaging").await;
            let messaging =
                Messaging::sync_from(context.child("messaging"), "messaging", target, resolver)
                    .await;

            // wrappers over EMBEDDED substrates: chat embeds under its own id,
            // agent under "agent-messaging" — mirroring genesis_host exactly.
            let (target, resolver) = fetch_target("chat").await;
            let chat = Chat::sync_from(context.child("chat"), "chat", target, resolver).await;

            let (target, resolver) = fetch_target("agent").await;
            let agent = Agent::sync_from_messaging_id(
                context.child("agent"),
                "agent",
                "agent-messaging",
                target,
                resolver,
            )
            .await;

            let snapshot_of = |module: &'static str| {
                let client = client.clone();
                let height = manifest.height;
                let root = manifest.entry(module).expect("module in manifest").root;
                async move {
                    let bytes = fetch_snapshot(&client, height, module).await.expect("snapshot");
                    (bytes, root)
                }
            };

            let (bytes, root) = snapshot_of("directory").await;
            let mut directory = Directory::new("directory");
            directory.install(&bytes, root).expect("directory install");

            let (bytes, root) = snapshot_of("valset").await;
            let mut valset = Valset::new("valset");
            valset.install(&bytes, root).expect("valset install");

            let (bytes, root) = snapshot_of("saga").await;
            let mut saga = SagaModule::new("saga");
            saga.install(&bytes, root).expect("saga install");

            let (bytes, root) = snapshot_of("governance").await;
            let mut governance = Governance::new("governance", "valset");
            governance.install(&bytes, root).expect("governance install");

            let (bytes, root) = snapshot_of("tasks").await;
            let mut tasks = Tasks::new("tasks");
            tasks.install(&bytes, root).expect("tasks install");

            let (bytes, root) = snapshot_of("vaults").await;
            let mut vaults = Vaults::new("vaults");
            vaults.install(&bytes, root).expect("vaults install");

            let (bytes, root) = snapshot_of("forge").await;
            let forge_repo = storage_for_sync.join("forge-repo");
            let mut forge = Forge::init("forge", forge_repo).expect("joiner forge init");
            forge.install(&bytes, root).expect("forge install");

            // compose and check THE property: the joiner's app-hash IS the
            // manifest's. print the greppable line the demo script asserts on.
            let mods: [&dyn sdk::Module; 12] = [
                &kv, &document, &messaging, &chat, &agent, &directory, &valset, &governance,
                &saga, &tasks, &vaults, &forge,
            ];
            let synced = state::global_root(&mods);
            if synced != manifest.app_hash {
                eprintln!(
                    "[node #{id}] SYNC FAILED: composed {} != manifest {}",
                    hex(&synced),
                    hex(&manifest.app_hash)
                );
                std::process::exit(1);
            }
            println!("[node #{id}] synced app_hash={}", hex(&synced));
            return;
        }

        // ---- a VALIDATOR: consensus engine + state-sync service -------------

        // pre-register the ENTIRE epoch channel bank (registration is only
        // possible before network.start(); every respawned engine needs fresh
        // channels). bank[e] holds epoch e's (vote, certificate, resolver,
        // payload) pairs until that epoch's engine consumes them.
        let mut channel_bank: Vec<Option<_>> = (0..EPOCH_CHANNEL_BANK)
            .map(|epoch| {
                let (vote, cert, res, payload) = engine_channels(epoch);
                Some((
                    network.register(vote, quota, MAX_BACKLOG),
                    network.register(cert, quota, MAX_BACKLOG),
                    network.register(res, quota, MAX_BACKLOG),
                    network.register(payload, quota, MAX_BACKLOG),
                ))
            })
            .collect();
        let (mut sync_tx, sync_rx) = network.register(CHANNEL_STATE_SYNC, quota, MAX_BACKLOG);

        // start the network actors (dialer/listener/router/tracker). registered
        // receivers buffer regardless, so starting before the engine is fine.
        network.start();

        // the statesync INGRESS task: owns the channel receiver and loops a
        // clean `recv().await`, forwarding frames into a local bounded queue.
        // the pump then selects on THAT queue — dropping an mpsc `next()`
        // future between ticks is lossless, whereas dropping the p2p receiver's
        // actor-backed `recv()` future mid-flight could eat a delivered
        // message. bounded + drop-on-full: clients time out and retry, so a
        // flood degrades to retries instead of unbounded memory.
        let (bridge_tx, mut sync_ingress) =
            futures::channel::mpsc::channel::<(ed25519::PublicKey, Vec<u8>)>(64);
        context.child("sync_ingress").spawn(move |_ctx| {
            let mut receiver = sync_rx;
            let mut bridge_tx = bridge_tx;
            async move {
                loop {
                    match receiver.recv().await {
                        Ok((peer, msg)) => {
                            let bytes: Vec<u8> = msg.into();
                            // full bridge = flood pressure: drop; clients retry.
                            let _ = bridge_tx.try_send((peer, bytes));
                        }
                        Err(_) => return, // network shutdown — nothing to serve.
                    }
                }
            }
        });

        // genesis host: the SAME module set on every node -> identical genesis
        // app-hash. the full production set — system infrastructure plus every
        // product module — with the genesis validators seeded into valset.
        let forge_repo = storage_for_sync.join("forge-repo");
        let host = genesis_host(&context, &forge_repo, &validators).await;

        // spawn one epoch's engine from the channel bank. scheme built the
        // production way (`signer` finds our key's index in the sorted
        // participant set); per-epoch genesis floor + per-epoch storage
        // partition, so a respawned engine can never collide with a
        // predecessor. the consensus signature scheme is a GENESIS-WIDE
        // constant (ConsensusScheme); adding V2Bls makes the match
        // non-exhaustive — the compiler-enforced rekey point.
        let spawn_epoch = |bank: &mut Vec<Option<_>>,
                               epoch: u64,
                               participants: Set<ed25519::PublicKey>|
         -> SimplexOrderer {
            let slot = bank
                .get_mut(epoch as usize)
                .and_then(|s| s.take())
                .unwrap_or_else(|| {
                    eprintln!(
                        "[node #{id}] FATAL: epoch {epoch} exhausts the pre-registered                          channel bank ({EPOCH_CHANNEL_BANK}) — rebuild with a wider bank"
                    );
                    std::process::exit(1);
                });
            let (vote, certificate, resolver, payload) = slot;
            let scheme = match CONSENSUS_SCHEME {
                ConsensusScheme::V1Ed25519 => simplex_ed25519::Scheme::signer(
                    &namespace,
                    participants,
                    signer.clone(),
                )
                .expect("our key is in the validator participant set"),
            };
            let label: &'static str =
                Box::leak(format!("consensus_e{epoch}").into_boxed_str());
            SimplexOrderer::spawn_with_relay(
                context.child(label),
                scheme,
                oracle.clone(),
                format!("{}-e{epoch}", signer.public_key()),
                Epoch::new(epoch),
                epoch_floor(&namespace, epoch),
                // per-process, PER-EPOCH content store: pins/pending of a torn
                // down epoch die with it (in-flight ops are resubmitted).
                ContentStore::new(),
                vote,
                certificate,
                resolver,
                payload,
            )
        };

        let orderer = spawn_epoch(&mut channel_bank, 0, validator_participants.clone());
        let mut node = OrderedNode::new(host, orderer);

        // the valset ORCHESTRATOR: watches finalized valset module state and
        // schedules deterministic epoch cutovers. the initial observation is
        // the genesis-seeded membership.
        let genesis_valset_root = node
            .host()
            .module_root("valset")
            .expect("valset is registered");
        let mut orchestrator = consensus::ValsetOrchestrator::new(
            CUTOVER_DELAY,
            consensus::ObservedValset::from_validator_set(
                consensus::ValsetRoot(genesis_valset_root.0),
                validators.clone(),
            ),
        );

        // the genesis app-hash BEFORE any op — the demo asserts this agrees across
        // processes (a fork here would be a genesis-determinism bug, not consensus).
        let genesis_hash = node.app_hash();
        println!("[node #{id}] genesis app_hash={}", hex(&genesis_hash));

        // introduce a DISTINCT op per process: node N writes directory key "kN" =
        // "node-N". distinct key + distinct origin -> distinct frame -> distinct
        // sha256 digest, so a peer that finalizes THIS op's digest has NO local
        // bytes for it — unless the leader's relay gossiped them on CHANNEL_PAYLOAD
        // and this process's store-only drain cached them. directory is order-
        // INDEPENDENT, so both nodes converge on {k0=node-0, k1=node-1} under any
        // interleaving, isolating the property under test (did the peer's payload
        // cross the wire?) from op ordering. ONE submit — the automaton PEEKS
        // (never pops), so the digest rides out every nullified early view until
        // the mesh forms and this node leads and proposes it.
        let op = Msg {
            target: "directory".into(),
            payload: encode_msg(&DirMsg::Set { key: format!("k{id}"), value: format!("node-{id}") }),
        };
        node.submit(&signer, 0, op).await.expect("submit op");

        // the local rpc bridge: blocking listener threads push parsed requests
        // into this bounded queue; the pump answers between drains.
        let (rpc_tx, mut rpc_ingress) = futures::channel::mpsc::channel::<RpcJob>(64);
        if let Some(listener) = rpc_listener {
            println!(
                "[node #{id}] rpc listening on {}",
                listener.local_addr().map(|a| a.to_string()).unwrap_or_default()
            );
            spawn_rpc_listener(listener, rpc_tx);
        } else {
            drop(rpc_tx); // rpc off: the branch below just stays pending forever.
        }

        // the ordered lane SIGNS every frame. rpc submits are signed by THIS
        // node's identity (the node is the local caller's custodian until user
        // keys reach the console); seq starts after the demo op's 0.
        let mut next_seq: u64 = 1;

        // pump: drain finalized frames on an interval, apply them in agreed
        // (ascending-view) order, serve statesync rpcs, answer local rpc, and
        // drive the reactor seam between drains (every response reflects a
        // block boundary — never a torn mid-drain view). print `converged` ONCE
        // this node has applied every VALIDATOR's op. this infinite loop IS the
        // "run forever" park (keeps the mesh + sync service alive for joiners);
        // rpc `shutdown` is the graceful exit.
        let expected = validator_seeds.len();
        let mut applied = 0usize;
        let mut converged = false;
        let mut sync_server = SyncServer::new();
        // the host-owned worker set (reactor seam). EMPTY for now: effects of
        // finalized blocks are drained and logged so the lane is visibly live;
        // the agent LLM worker plugs in here.
        let workers: Vec<Box<dyn reactor::Worker>> = Vec::new();
        loop {
            futures::select_biased! {
                job = rpc_ingress.next() => {
                    let Some((req, reply)) = job else { continue };
                    let resp = match req {
                        RpcRequest::Submit { target, payload_hex } => {
                            match unhex(&payload_hex) {
                                Ok(payload) => {
                                    let seq = next_seq;
                                    next_seq += 1;
                                    match node
                                        .submit(&signer, seq, Msg { target, payload })
                                        .await
                                    {
                                        Ok(()) => RpcReply::ok(),
                                        Err(e) => RpcReply::err(format!("submit failed: {e}")),
                                    }
                                }
                                Err(e) => RpcReply::err(format!("bad payload_hex: {e}")),
                            }
                        }
                        RpcRequest::Query { target, req_hex } => match unhex(&req_hex) {
                            Ok(req_bytes) => match node.host().query(&target, &req_bytes).await {
                                Ok(bytes) => RpcReply {
                                    reply_hex: Some(hex_bytes(&bytes)),
                                    ..RpcReply::ok()
                                },
                                Err(e) => RpcReply::err(format!("query failed: {e}")),
                            },
                            Err(e) => RpcReply::err(format!("bad req_hex: {e}")),
                        },
                        RpcRequest::Status => {
                            let mut modules = std::collections::BTreeMap::new();
                            for m in [
                                "kv", "document", "messaging", "chat", "agent", "forge",
                                "valset", "governance", "saga", "tasks", "vaults", "directory",
                            ] {
                                if let Some(root) = node.host().module_root(m) {
                                    modules.insert(m.to_string(), hex(&root));
                                }
                            }
                            RpcReply {
                                status: Some(RpcStatus {
                                    height: node.finalized().map(|f| f.height),
                                    app_hash: hex(&node.app_hash()),
                                    modules,
                                }),
                                ..RpcReply::ok()
                            }
                        }
                        RpcRequest::Shutdown => {
                            let _ = reply.send(RpcReply::ok());
                            println!("[node #{id}] shutdown requested via rpc — exiting");
                            std::process::exit(0);
                        }
                    };
                    let _ = reply.send(resp);
                }
                msg = sync_ingress.next() => {
                    let Some((peer, bytes)) = msg else {
                        // the ingress task ended (network shutdown) — nothing
                        // left to serve; keep draining consensus regardless.
                        continue;
                    };
                    let Ok((rpc_id, body)) = statesync::decode_rpc(&bytes) else {
                        continue; // malformed rpc envelope: drop, never crash.
                    };
                    let resp = sync_server
                        .handle_frame(node.host(), node.finalized(), body)
                        .await;
                    let _ = sync_tx.send(
                        Recipients::One(peer),
                        IoBuf::from(statesync::encode_rpc(rpc_id, &resp)),
                        false,
                    );
                }
                _ = context.sleep(Duration::from_millis(100)).fuse() => {
                    // FAIL-STOP: a drain error is a node-local block-boundary
                    // fault — this node's state is indeterminate relative to its
                    // peers, so applying even one more finalized op could
                    // silently fork it. exit loudly; an operator (or supervisor)
                    // restarts the node, which then re-joins via state sync.
                    applied += match node.drain_delivered().await {
                        Ok(n) => n,
                        Err(e) => {
                            eprintln!("[node #{id}] FATAL: {e} — halting");
                            std::process::exit(1);
                        }
                    };
                    // the VALSET ORCHESTRATION step: observe the finalized
                    // membership projection; a change schedules a deterministic
                    // cutover (arming the discard ceiling), and crossing the
                    // cutover view tears the engine down and respawns it over
                    // the new participant set at the next epoch.
                    if let Some(engine_view) = node.last_engine_view() {
                        let members_raw = read_valset_members(node.host()).await;
                        let valset_root = node
                            .host()
                            .module_root("valset")
                            .expect("valset is registered");
                        let mut member_keys: Vec<ed25519::PublicKey> = Vec::new();
                        for key in &members_raw {
                            if let Ok(pk) = ed25519::PublicKey::decode(key.as_slice()) {
                                member_keys.push(pk);
                            }
                        }
                        let observed = consensus::ObservedValset::from_validator_set(
                            consensus::ValsetRoot(valset_root.0),
                            member_keys,
                        );
                        if let consensus::ObservationOutcome::Scheduled(cutover) =
                            orchestrator.observe_finalized_valset(engine_view, observed)
                        {
                            println!(
                                "[node #{id}] membership change observed at view {} — cutover to epoch {} at view {}",
                                cutover.observed_view(),
                                cutover.next_epoch(),
                                cutover.cutover_view()
                            );
                            node.set_view_ceiling(cutover.cutover_view());
                        }
                        if let Some(plan) = orchestrator.respawn_if_due(engine_view) {
                            let members = plan.valset().membership().consensus_members();
                            if !members.contains(&signer.public_key()) {
                                println!(
                                    "[node #{id}] demoted from the validator set at epoch {} — halting (restart to serve as sync/observer)",
                                    plan.epoch()
                                );
                                std::process::exit(0);
                            }
                            let participants: Set<ed25519::PublicKey> = Set::try_from(
                                members.iter().cloned().collect::<Vec<_>>(),
                            )
                            .expect("orchestrator membership has no duplicates");
                            let orderer =
                                spawn_epoch(&mut channel_bank, plan.epoch(), participants);
                            node.cutover(orderer, plan.cutover_app_height());
                            println!(
                                "[node #{id}] cutover complete: epoch {} with {} validators (app height base {})",
                                plan.epoch(),
                                members.len(),
                                plan.cutover_app_height()
                            );
                        }
                    }

                    // the reactor seam: offer each finalized block's effects to
                    // the host-owned workers; a claiming worker's follow-up op
                    // re-enters through the ordered lane as its own block (the
                    // oracle-as-op). unclaimed effects are logged, not silently
                    // dropped — a saga stuck Pending should be visible.
                    for eff in node.take_effects() {
                        let mut claimed = false;
                        for w in &workers {
                            match w.run(&eff).await {
                                Ok(Some(follow)) => {
                                    let seq = next_seq;
                                    next_seq += 1;
                                    if let Err(e) =
                                        node.submit(&signer, seq, follow).await
                                    {
                                        eprintln!("[node #{id}] worker follow-up submit failed: {e}");
                                    }
                                    claimed = true;
                                    break;
                                }
                                Ok(None) => {}
                                Err(e) => {
                                    eprintln!("[node #{id}] worker error: {e}");
                                    claimed = true; // errored ≠ unclaimed; don't double-log
                                    break;
                                }
                            }
                        }
                        if !claimed {
                            println!(
                                "[node #{id}] effect with no worker ({} bytes) — dropped",
                                eff.0.len()
                            );
                        }
                    }
                    if !converged && applied >= expected {
                        let h = node.app_hash();
                        println!("[node #{id}] converged app_hash={}", hex(&h));
                        // dump every directory key so the demo can eyeball the ops
                        // (each node ends holding the op it originated AND the peer's).
                        for k in 0..expected {
                            let reply = node
                                .host()
                                .query("directory", &encode_query(&DirQuery::Get { key: format!("k{k}") }))
                                .await
                                .expect("directory query");
                            if let Ok(DirReply::Value(v)) = decode_reply(&reply) {
                                println!("[node #{id}]   directory k{k}={v:?}");
                            }
                        }
                        converged = true;
                    }
                }
            }
        }
    });

    Ok(())
}
