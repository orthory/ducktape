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

use commonware_codec::DecodeExt as _;
use commonware_consensus::simplex::scheme::ed25519 as simplex_ed25519;
use commonware_consensus::types::Epoch;
use commonware_cryptography::{Signer, ed25519};
use commonware_p2p::authenticated::discovery::{self, Network};
use commonware_p2p::{Manager, Receiver as _, Recipients, Sender as _};
use commonware_runtime::{Clock, IoBuf, Quota, Runner, Spawner, Supervisor};
use commonware_utils::{NZU32, ordered::Set};
use futures::{FutureExt as _, StreamExt as _};

use consensus::{ConsensusScheme, ContentStore, Digest, SimplexOrderer, digest_of};

mod config;
use config::{Resolved, hex_bytes, unhex};

/// the consensus signature scheme this build runs — a genesis-wide constant. today only
/// V1 (ed25519); see [`ConsensusScheme`]'s rekey/respawn contract for the BLS/V2 path.
const CONSENSUS_SCHEME: ConsensusScheme = ConsensusScheme::V1Ed25519;
use chat::Chat;
use directory::Directory;
use directory_interface::{DirMsg, DirQuery, DirReply, decode_reply, encode_msg, encode_query};
use document::Document;
use forge::Forge;
use governance::Governance;
use host::Host;
use kv::Kv;
use node::OrderedNode;
use saga::SagaModule;
use sdk::{Msg, StateRoot};
use statesync::p2p::P2pSyncClient;
use statesync::qmdb::RemoteQmdbResolver;
use statesync::{SyncServer, fetch_manifest, fetch_snapshot};
use tasks::Tasks;
use valset::Valset;
use vaults::Vaults;

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
/// every module in the production genesis set, in status-report order. keep in
/// sync with [`genesis_host`] — status endpoints report exactly these roots.
const MODULE_IDS: [&str; 10] = [
    "kv",
    "document",
    "chat",
    "forge",
    "valset",
    "governance",
    "saga",
    "tasks",
    "vaults",
    "directory",
];
/// how long an app-surface submit reply may be held awaiting finalization
/// before it errors out (the op may still land later; clients re-query on
/// block events). mirrors the rpc bridge's stuck-node budget.
const SUBMIT_HOLD: Duration = Duration::from_secs(10);

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

/// read the valset module's current membership projection (committed state —
/// called between drains, outside any block).
async fn read_valset_members(host: &Host) -> Vec<Vec<u8>> {
    use valset_interface::{ValsetQuery, ValsetReply, decode_reply, encode_query};
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
    let chat = Chat::init(context.child("chat"), "chat").await;
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
        Box::new(chat),
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
        Self {
            ok: true,
            error: None,
            reply_hex: None,
            status: None,
        }
    }
    fn err(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            error: Some(msg.into()),
            reply_hex: None,
            status: None,
        }
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
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("keygen") => return cmd_keygen(&args[1..]),
        Some("init") => return cmd_init(&args[1..]),
        Some("invite") => return cmd_invite(&args[1..]),
        Some("admit") => return cmd_admit(&args[1..]),
        Some("join") => return cmd_join(&args[1..]),
        _ => {}
    }

    // the run path: `--config <path> [--sync-only]`.
    let mut cfg_path: Option<PathBuf> = None;
    let mut sync_only = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--config" => cfg_path = it.next().map(PathBuf::from),
            "--sync-only" => sync_only = true,
            other => {
                return Err(format!(
                    "unexpected arg {other:?} (want a subcommand — keygen|init|invite|admit|join \
                     — or --config <path> [--sync-only])"
                )
                .into());
            }
        }
    }
    let cfg_path = cfg_path.ok_or("missing --config <path>")?;

    // opt-in internals visibility: RUST_LOG=commonware_p2p=debug etc.
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    run_node(config::resolve(&cfg_path)?, sync_only)
}

// ============================================================================
// onboarding verbs — keygen / init / invite / admit / join.
// ============================================================================

/// tiny flag parser: `--name value` pairs plus positionals; no deps.
fn parse_flags(
    args: &[String],
) -> Result<(Vec<String>, std::collections::BTreeMap<String, String>), String> {
    let mut positional = Vec::new();
    let mut flags = std::collections::BTreeMap::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if let Some(name) = a.strip_prefix("--") {
            let v = it.next().ok_or_else(|| format!("--{name} needs a value"))?;
            flags.insert(name.to_string(), v.clone());
        } else {
            positional.push(a.clone());
        }
    }
    Ok((positional, flags))
}

/// the addr peers should dial, if one is real: prefer --advertised, else the
/// listen addr when it has a concrete port (an ephemeral port 0 is not an
/// address anyone can dial later).
fn dialable(
    advertised: Option<&str>,
    listen: &str,
) -> Result<Option<SocketAddr>, Box<dyn std::error::Error>> {
    if let Some(a) = advertised {
        return Ok(Some(a.parse()?));
    }
    let l: SocketAddr = listen.parse()?;
    Ok((l.port() != 0).then_some(l))
}

/// `keygen [--out <path>]` — generate (or reuse) a persisted ed25519 identity.
/// pubkey on stdout (scriptable); provenance on stderr.
fn cmd_keygen(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let out = PathBuf::from(
        flags
            .get("out")
            .map(String::as_str)
            .unwrap_or("identity.key"),
    );
    let (key, generated) = config::load_or_generate_identity(&out)?;
    println!("{}", hex_bytes(key.public_key().as_ref()));
    eprintln!(
        "{} identity at {}",
        if generated { "generated" } else { "reusing" },
        out.display()
    );
    Ok(())
}

/// `init --name <human name> [--dir .] [--listen a] [--advertised a] [--http a]
/// [--rpc a]` — found a network: mint the chain-id, write the descriptor +
/// node config, seed the genesis validator set with this identity.
fn cmd_init(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let name = flags
        .get("name")
        .ok_or("init needs --name <human-readable network name>")?;
    let dir = PathBuf::from(flags.get("dir").map(String::as_str).unwrap_or("."));
    std::fs::create_dir_all(&dir)?;
    let listen = flags
        .get("listen")
        .map(String::as_str)
        .unwrap_or("127.0.0.1:0");

    let (key, generated) = config::load_or_generate_identity(&dir.join("identity.key"))?;
    let me = key.public_key();
    let chain_id = config::mint_chain_id(name, &me);
    let mut descriptor = config::NetworkDescriptor {
        chain_id: chain_id.clone(),
        scheme: config::SCHEME_ED25519.into(),
        validators: vec![hex_bytes(me.as_ref())],
        bootstrap: Vec::new(),
    };
    if let Some(addr) = dialable(flags.get("advertised").map(String::as_str), listen)? {
        descriptor.add_bootstrap(&me, &addr);
    }
    descriptor.save(&dir.join("network.toml"))?;
    config::write_node_toml(
        &dir,
        listen,
        flags.get("advertised").map(String::as_str),
        flags.get("http").map(String::as_str),
        flags.get("rpc").map(String::as_str),
    )?;
    eprintln!(
        "{} identity {}",
        if generated { "generated" } else { "reusing" },
        hex_bytes(me.as_ref())
    );
    eprintln!("network {chain_id} initialized in {}", dir.display());
    eprintln!("start:  ducktape-node --config {}/node.toml", dir.display());
    eprintln!(
        "invite: ducktape-node invite --config {}/node.toml",
        dir.display()
    );
    println!("{chain_id}");
    Ok(())
}

/// `invite [--config node.toml]` — emit the one-line paste blob: the network
/// descriptor with THIS member's dial hint folded in (and persisted, so every
/// future invite carries it).
fn cmd_invite(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let cfg_path = PathBuf::from(
        flags
            .get("config")
            .map(String::as_str)
            .unwrap_or("node.toml"),
    );
    let (raw, base) = config::load_node_toml(&cfg_path)?;
    let network_rel = raw
        .network
        .as_deref()
        .ok_or("invite needs a network-shape config (no `network` field found)")?;
    let descriptor_path = base.join(network_rel);
    let mut descriptor = config::NetworkDescriptor::load(&descriptor_path)?;
    let key = config::load_identity(&base.join(raw.key_file.as_deref().unwrap_or("identity.key")))?;
    match dialable(raw.advertised.as_deref(), &raw.listen)? {
        Some(addr) => descriptor.add_bootstrap(&key.public_key(), &addr),
        // an invite must carry SOME dialable member.
        None if descriptor.bootstrap.is_empty() => {
            return Err(
                "no dialable address: give node.toml a concrete `listen` port or an \
                        `advertised` addr so a joiner can reach the network"
                    .into(),
            );
        }
        None => {}
    }
    descriptor.save(&descriptor_path)?;
    println!("{}", config::encode_invite(&descriptor));
    Ok(())
}

/// `admit <hex pubkey> [--config node.toml]` — pre-genesis membership: add an
/// identity to the descriptor's validator set. once the network has state,
/// membership changes go through governance (AddValidator), not genesis edits.
fn cmd_admit(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    let [pubkey_hex] = pos.as_slice() else {
        return Err("admit needs exactly one <hex pubkey>".into());
    };
    let key = config::decode_key(pubkey_hex)?;
    let cfg_path = PathBuf::from(
        flags
            .get("config")
            .map(String::as_str)
            .unwrap_or("node.toml"),
    );
    let (raw, base) = config::load_node_toml(&cfg_path)?;
    let network_rel = raw
        .network
        .as_deref()
        .ok_or("admit needs a network-shape config")?;
    let storage = base.join(raw.storage_dir.as_deref().unwrap_or("storage"));
    if storage.exists() {
        return Err(format!(
            "{} already has state — a running network admits members via governance \
             (AddValidator), not by editing genesis",
            storage.display()
        )
        .into());
    }
    let descriptor_path = base.join(network_rel);
    let mut descriptor = config::NetworkDescriptor::load(&descriptor_path)?;
    descriptor.admit(&key);
    descriptor.save(&descriptor_path)?;
    eprintln!("admitted {pubkey_hex} into {}", descriptor.chain_id);
    eprintln!(
        "re-run `ducktape-node invite` and share the REFRESHED invite — genesis must be \
         identical on every member"
    );
    Ok(())
}

/// `join <invite blob> [--dir .] [--listen a] [--advertised a] [--http a]
/// [--rpc a]` — materialize a workspace from an invite: descriptor + identity
/// (kept across re-joins) + node config. prints this identity for the
/// inviter's pre-genesis `admit`.
fn cmd_join(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    let [blob] = pos.as_slice() else {
        return Err("join needs exactly one <invite blob>".into());
    };
    let descriptor = config::decode_invite(blob)?;
    let dir = PathBuf::from(flags.get("dir").map(String::as_str).unwrap_or("."));
    std::fs::create_dir_all(&dir)?;
    let listen = flags
        .get("listen")
        .map(String::as_str)
        .unwrap_or("127.0.0.1:0");
    descriptor.save(&dir.join("network.toml"))?;
    let (key, generated) = config::load_or_generate_identity(&dir.join("identity.key"))?;
    let me_hex = hex_bytes(key.public_key().as_ref());
    config::write_node_toml(
        &dir,
        listen,
        flags.get("advertised").map(String::as_str),
        flags.get("http").map(String::as_str),
        flags.get("rpc").map(String::as_str),
    )?;
    eprintln!(
        "{} identity {me_hex}",
        if generated { "generated" } else { "reusing" }
    );
    eprintln!(
        "workspace for {} written to {}",
        descriptor.chain_id,
        dir.display()
    );
    if descriptor.validators.contains(&me_hex) {
        eprintln!(
            "this identity is a member — start: ducktape-node --config {}/node.toml",
            dir.display()
        );
    } else {
        eprintln!("NOT yet a member. send this identity to a member, who runs:");
        eprintln!("    ducktape-node admit {me_hex}");
        eprintln!("then join again with the refreshed invite (the identity here is kept).");
    }
    println!("{me_hex}");
    Ok(())
}

/// stand up the real-socket node from `cfg` and run it until killed (validator)
/// or until state sync completes (`--sync-only`).
///
/// deliberately NOT `#[tokio::main]`: `tokio::Runner` owns its OWN tokio runtime,
/// and you cannot start a runtime from inside one. so `main` is sync and hands
/// off to `Runner::start`, which drives everything (including the engine's spawned
/// tasks) on the runtime it owns.
fn run_node(resolved: Resolved, sync_only: bool) -> Result<(), Box<dyn std::error::Error>> {
    let Resolved {
        signer,
        label,
        namespace,
        mesh: peers,
        validators,
        bootstrappers,
        listen,
        advertised,
        storage_dir: storage,
        rpc_listen,
        http_listen,
        dev_demo,
    } = resolved;
    // discovery's bootstrapper list wants its own ingress address type.
    let bootstrappers: Vec<(ed25519::PublicKey, _)> = bootstrappers
        .into_iter()
        .map(|(k, a)| (k, a.into()))
        .collect();

    for (i, pk) in peers.iter().enumerate() {
        println!(
            "[node {label}] peer[{i}] identity={}",
            hex_bytes(pk.as_ref())
        );
    }
    println!(
        "[node {label}] starting on {listen} ({} mesh peers, {} validators{}), storage {}",
        peers.len(),
        validators.len(),
        if sync_only { ", sync-only" } else { "" },
        storage.display()
    );

    // the rpc listener binds OUTSIDE the runtime (plain std tcp on OS threads)
    // so a bind failure is a clean startup error, not an async surprise.
    let rpc_listener = match rpc_listen.as_deref() {
        Some(addr) if !sync_only => Some(std::net::TcpListener::bind(addr)?),
        _ => None,
    };

    // the http/ws app surface: same bind-early rule. the server itself runs on
    // its OWN plain-tokio OS thread (noded's exact split — the host never
    // leaves the commonware runner thread; http handlers only send
    // NodeCommands over the lane), so the pump below is its single consumer.
    let (http_handle, http_cmds, http_events) = noded::NodeHandle::channel();
    match http_listen.as_deref() {
        Some(addr) if !sync_only => {
            let listener = std::net::TcpListener::bind(addr)?;
            listener.set_nonblocking(true)?;
            println!(
                "[node {label}] app surface listening on http://{}",
                listener
                    .local_addr()
                    .map(|a| a.to_string())
                    .unwrap_or_default()
            );
            let thread_label = label.clone();
            std::thread::Builder::new()
                .name("app-surface".into())
                .spawn(move || {
                    tokio::runtime::Builder::new_multi_thread()
                        .enable_all()
                        .build()
                        .expect("app-surface tokio runtime")
                        .block_on(async move {
                            let listener = tokio::net::TcpListener::from_std(listener)
                                .expect("adopt app-surface listener");
                            if let Err(e) = noded::serve(listener, http_handle).await {
                                eprintln!("app surface server error: {e}");
                            }
                        });
                    // a client asked the surface to shut down (POST /v1/shutdown) —
                    // mirror the rpc shutdown: exit the whole process gracefully.
                    println!("[node {thread_label}] shutdown requested via app surface — exiting");
                    std::process::exit(0);
                })?;
        }
        // surface off: dropping the handle terminates the command stream; the
        // pump's select arm sees one None and then never polls it again.
        _ => drop(http_handle),
    }

    // run on commonware's OWN tokio runtime, rooted at our per-process storage dir.
    let storage_for_sync = storage.clone();
    let rt_cfg = commonware_runtime::tokio::Config::default().with_storage_directory(storage);
    let executor = commonware_runtime::tokio::Runner::new(rt_cfg);

    executor.start(|context| async move {
        // the authorized MESH set, SORTED — what discovery tracks. the
        // consensus scheme uses the (possibly smaller) validator set below.
        let mesh_participants: Set<ed25519::PublicKey> =
            Set::try_from(peers.clone()).expect("authorized peer set has no duplicates");
        let validator_participants: Set<ed25519::PublicKey> =
            Set::try_from(validators.clone()).expect("validator set has no duplicates");

        // the statesync source a --sync-only joiner pulls from: a configured
        // bootstrapper if any (the network shape), else the first mesh
        // identity (the dev shape's node 0).
        let sync_source = bootstrappers
            .first()
            .map(|(k, _)| k.clone())
            .unwrap_or_else(|| peers.first().expect("mesh is non-empty").clone());

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

            let server_peer = sync_source;
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
                        println!("[node {label}] manifest not ready ({e}); retrying");
                        context.sleep(Duration::from_millis(500)).await;
                    }
                }
            };
            println!(
                "[node {label}] manifest height={} app_hash={}",
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

            let (target, resolver) = fetch_target("chat").await;
            let chat = Chat::sync_from(context.child("chat"), "chat", target, resolver).await;

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
            let mods: [&dyn sdk::Module; 10] = [
                &kv, &document, &chat, &directory, &valset, &governance,
                &saga, &tasks, &vaults, &forge,
            ];
            let synced = state::global_root(&mods);
            if synced != manifest.app_hash {
                eprintln!(
                    "[node {label}] SYNC FAILED: composed {} != manifest {}",
                    hex(&synced),
                    hex(&manifest.app_hash)
                );
                std::process::exit(1);
            }
            println!("[node {label}] synced app_hash={}", hex(&synced));
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
                        "[node {label}] FATAL: epoch {epoch} exhausts the pre-registered                          channel bank ({EPOCH_CHANNEL_BANK}) — rebuild with a wider bank"
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
                // the engine and tests are V2-capable (see consensus::BlsScheme);
                // wiring V2 into the epoch respawn machinery needs the bls
                // participant BiMap derived per epoch (valset-registered bls
                // keys + proof-of-possession) — fail-stop until that lands.
                ConsensusScheme::V2Bls => {
                    unimplemented!(
                        "V2Bls node wiring lands with valset bls key registration; \
                         the consensus engine itself is V2-capable"
                    )
                }
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
        println!("[node {label}] genesis app_hash={}", hex(&genesis_hash));

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
        // dev shape only — a REAL network's genesis carries no demo scaffolding.
        if dev_demo {
            let n = label.trim_start_matches('#').to_string();
            let op = Msg {
                target: "directory".into(),
                payload: encode_msg(&DirMsg::Set {
                    key: format!("k{n}"),
                    value: format!("node-{n}"),
                }),
            };
            node.submit(&signer, 0, op).await.expect("submit op");
        }

        // the local rpc bridge: blocking listener threads push parsed requests
        // into this bounded queue; the pump answers between drains.
        let (rpc_tx, mut rpc_ingress) = futures::channel::mpsc::channel::<RpcJob>(64);
        if let Some(listener) = rpc_listener {
            println!(
                "[node {label}] rpc listening on {}",
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
        let expected = validators.len();
        let mut applied = 0usize;
        let mut converged = false;
        // the app-surface lane: held submit replies keyed by the submitted
        // frame's content address, resolved when the frame drains (or expired
        // after SUBMIT_HOLD), plus the last block height published to ws
        // subscribers.
        let mut http_ingress = http_cmds;
        let mut pending_submits: std::collections::HashMap<
            node::FrameId,
            (
                futures::channel::oneshot::Sender<Result<noded::BlockSummary, String>>,
                std::time::Instant,
            ),
        > = std::collections::HashMap::new();
        let mut last_published: Option<u64> = None;
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
                                        Ok(_) => RpcReply::ok(),
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
                            for m in MODULE_IDS {
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
                            println!("[node {label}] shutdown requested via rpc — exiting");
                            std::process::exit(0);
                        }
                    };
                    let _ = reply.send(resp);
                }
                cmd = http_ingress.next() => {
                    let Some(cmd) = cmd else { continue };
                    match cmd {
                        // `origin` is the caller's CLAIMED submitter identity —
                        // meaningful on the embedded daemon, but this lane signs
                        // frames, and the signed origin IS this node's pubkey
                        // (authenticated authorship that governance relies on).
                        // a claimed origin cannot ride a signed frame without
                        // making authorship forgeable, so it is ignored here;
                        // display names resolve via the name registry instead.
                        noded::NodeCommand::Submit { target, payload, origin: _, reply } => {
                            let seq = next_seq;
                            next_seq += 1;
                            match node.submit(&signer, seq, Msg { target, payload }).await {
                                // HOLD the reply: it lands when this frame
                                // drains at a finalized boundary, so the app's
                                // follow-up query reads the applied state.
                                Ok(frame) => {
                                    pending_submits.insert(
                                        frame,
                                        (reply, std::time::Instant::now() + SUBMIT_HOLD),
                                    );
                                }
                                Err(e) => {
                                    let _ = reply.send(Err(format!("submit failed: {e}")));
                                }
                            }
                        }
                        noded::NodeCommand::Query { target, req, reply } => {
                            let result = node
                                .host()
                                .query(&target, &req)
                                .await
                                .map_err(|e| e.to_string());
                            let _ = reply.send(result);
                        }
                        noded::NodeCommand::Status { reply } => {
                            let modules = MODULE_IDS
                                .iter()
                                .map(|m| noded::ModuleStatus {
                                    id: (*m).into(),
                                    root: node
                                        .host()
                                        .module_root(m)
                                        .map(|r| hex(&r))
                                        .unwrap_or_default(),
                                })
                                .collect();
                            let _ = reply.send(noded::NodeStatus {
                                version: env!("CARGO_PKG_VERSION").into(),
                                app_hash: hex(&node.app_hash()),
                                height: node.finalized().map(|f| f.height).unwrap_or(0),
                                modules,
                            });
                        }
                    }
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
                            eprintln!("[node {label}] FATAL: {e} — halting");
                            std::process::exit(1);
                        }
                    };
                    // resolve held app-surface submits against what this
                    // drain finished with; every disposition is deterministic,
                    // so the reply faithfully reports the op's consensus fate.
                    let boundary_hash = hex(&node.app_hash());
                    for d in node.take_drained() {
                        let Some((reply, _)) = pending_submits.remove(&d.id) else { continue };
                        let _ = reply.send(match d.disposition {
                            node::Disposition::Applied => Ok(noded::BlockSummary {
                                height: d.height,
                                app_hash: boundary_hash.clone(),
                            }),
                            node::Disposition::Rejected => {
                                Err("op finalized but rejected (deterministic no-op)".into())
                            }
                            node::Disposition::Discarded => {
                                Err("op discarded at an epoch cutover — resubmit".into())
                            }
                        });
                    }
                    // expire holds the mesh never finalized in time. the op may
                    // still land later — clients re-query on block events.
                    if !pending_submits.is_empty() {
                        let now = std::time::Instant::now();
                        let expired: Vec<node::FrameId> = pending_submits
                            .iter()
                            .filter(|(_, (_, deadline))| *deadline <= now)
                            .map(|(k, _)| *k)
                            .collect();
                        for k in expired {
                            if let Some((reply, _)) = pending_submits.remove(&k) {
                                let _ = reply.send(Err(
                                    "timed out awaiting finalization — re-query on the next block"
                                        .into(),
                                ));
                            }
                        }
                    }
                    // publish each newly-applied boundary to ws subscribers
                    // (send only errs when nobody is subscribed — fine).
                    if let Some(f) = node.finalized() {
                        if last_published != Some(f.height) {
                            let _ = http_events.send(noded::BlockSummary {
                                height: f.height,
                                app_hash: hex(&f.app_hash),
                            });
                            last_published = Some(f.height);
                        }
                    }

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
                                "[node {label}] membership change observed at view {} — cutover to epoch {} at view {}",
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
                                    "[node {label}] demoted from the validator set at epoch {} — halting (restart to serve as sync/observer)",
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
                                "[node {label}] cutover complete: epoch {} with {} validators (app height base {})",
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
                                        eprintln!("[node {label}] worker follow-up submit failed: {e}");
                                    }
                                    claimed = true;
                                    break;
                                }
                                Ok(None) => {}
                                Err(e) => {
                                    eprintln!("[node {label}] worker error: {e}");
                                    claimed = true; // errored ≠ unclaimed; don't double-log
                                    break;
                                }
                            }
                        }
                        if !claimed {
                            println!(
                                "[node {label}] effect with no worker ({} bytes) — dropped",
                                eff.0.len()
                            );
                        }
                    }
                    if dev_demo && !converged && applied >= expected {
                        let h = node.app_hash();
                        println!("[node {label}] converged app_hash={}", hex(&h));
                        // dump every directory key so the demo can eyeball the ops
                        // (each node ends holding the op it originated AND the peer's).
                        for k in 0..expected {
                            let reply = node
                                .host()
                                .query("directory", &encode_query(&DirQuery::Get { key: format!("k{k}") }))
                                .await
                                .expect("directory query");
                            if let Ok(DirReply::Value(v)) = decode_reply(&reply) {
                                println!("[node {label}]   directory k{k}={v:?}");
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
