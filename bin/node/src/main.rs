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
//! has NO local bytes for it. `SimplexOrderer::spawn_with_resolver` wires a
//! `ConsensusRelay` that, at propose time, gossips the proposed frame's bytes to
//! all peers on the payload channel; every peer's STORE-ONLY drain caches them, so
//! when that digest finalizes the reporter resolves it locally and delivers it in
//! BFT order. content-addressing IS the verification (the drain re-hashes on
//! receipt). the relay gossip is one-shot, and quorum is a SUBSET — a validator
//! can finalize a view whose gossip it missed — so a lazy payload FETCH lane
//! backstops it: the resolver pulls missing bytes by digest from the tracked
//! mesh and fills the finalized slot instead of wedging the apply prefix. this
//! is what lets DISTINCT ops converge across processes with per-process stores
//! — quorum votes still cross the real TCP mesh to finalize.
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
use automations::Automations;
use chat::Chat;
use directory::Directory;
use directory_interface::{DirMsg, DirQuery, DirReply, decode_reply, encode_msg, encode_query};
use document::Document;
use files::Files;
use forge::Forge;
use governance::Governance;
use host::Host;
use inbox::Inbox;
use jobs::Jobs;
use kv::Kv;
use memory::Memory;
use node::OrderedNode;
use recovery::{Manifest, Recovery};
use saga::SagaModule;
use sdk::{Msg, StateRoot};
use statesync::p2p::P2pSyncClient;
use statesync::qmdb::RemoteQmdbResolver;
use statesync::{SyncServer, fetch_manifest, fetch_snapshot};
use tasks::Tasks;
use valset::Valset;
use vaults::Vaults;

/// the peer-set index a node WITHOUT consensus coordinates tracks (a parked
/// joiner, a sync-only observer): the genesis mesh at index 0. a VALIDATOR
/// tracks its epoch's mesh at index = epoch instead — discovery requires
/// strictly increasing indexes per `track`, ignores indexes a peer does not
/// know, but KILLS a peer whose set at a SHARED index has a different
/// length, so the set tracked at a given index must be identical on every
/// node that tracks it (epoch participant sets are; instantaneous valset
/// projections are not).
const PEER_SET: u64 = 0;
/// a deliberately-unregistered module target. while an epoch cutover is
/// pending, validators submit empty frames against it: finalized views only
/// advance with ops, so an idle network would otherwise park AT the armed
/// boundary forever. the frame finalizes, rejects deterministically on every
/// node (unknown module), advances the engine clock, and leaves no state.
const NOP_TARGET: &str = "consensus.nop";
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
const MODULE_IDS: [&str; 15] = [
    "kv", "document", "chat", "forge", "valset", "governance", "saga", "tasks", "vaults", "inbox",
    "directory", "automations", "files", "memory", "jobs",
];
/// how long an app-surface submit reply may be held awaiting finalization
/// before it errors out (the op may still land later; clients re-query on
/// block events). mirrors the rpc bridge's stuck-node budget.
const SUBMIT_HOLD: Duration = Duration::from_secs(10);

/// the five channels epoch `e`'s engine uses: vote, certificate, resolver, the
/// eager payload-relay lane, and the payload FETCH lane (the lazy catch-up
/// backstop — a validator that missed the one-shot relay gossip for a
/// finalized op fetches its bytes by digest instead of wedging its apply
/// prefix forever). starts at 8, clear of the statesync channel.
fn engine_channels(epoch: u64) -> (u64, u64, u64, u64, u64) {
    let base = 8 + epoch * 5;
    (base, base + 1, base + 2, base + 3, base + 4)
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

/// the orchestrator's current epoch participant set as raw key bytes — what
/// checkpoints, cutover records, and the statesync manifest carry.
fn participant_bytes(
    orchestrator: &consensus::ValsetOrchestrator<ed25519::PublicKey>,
) -> Vec<Vec<u8>> {
    orchestrator
        .current_members()
        .iter()
        .map(|k| k.as_ref().to_vec())
        .collect()
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
        // per-member notification queues; other modules deliver via follow-up
        // ops so a notification commits atomically with the causing event (P2).
        Box::new(Inbox::new("inbox")),
        Box::new(Files::new("files")),
        // the shared agent workspace: a filesystem-shaped namespace with
        // write-once publish, immutable generations, snapshots, and watches.
        Box::new(Memory::new("memory")),
        Box::new(Jobs::new("jobs")),
        Box::new(Directory::new("directory")),
        // user-defined rules over chat posts: trusts the "chat" origin for hook
        // events and emits chat/tasks follow-ups.
        Box::new(Automations::new("automations", "chat", "tasks")),
    ])
    .expect("genesis host")
}

/// the RESTORE twin of [`genesis_host`]: the disk substrates (qmdb modules,
/// forge's git repo) reopen themselves at their own committed positions; the
/// in-memory cohort installs its checkpoint snapshots, root-checked. the
/// recovery replay then rolls everything forward to the journal tip.
async fn restore_host(
    context: &commonware_runtime::tokio::Context,
    forge_repo: &std::path::Path,
    manifest: &Manifest,
) -> Result<Host, String> {
    let kv = Kv::init(context.child("kv"), "kv").await;
    let document = Document::init(context.child("document"), "document").await;
    let chat = Chat::init(context.child("chat"), "chat").await;
    let forge = Forge::init("forge", forge_repo.to_path_buf()).map_err(|e| format!("forge: {e}"))?;

    let snapshot_of = |id: &str| -> Result<(&[u8], StateRoot), String> {
        let bytes = manifest
            .snapshot(id)
            .ok_or_else(|| format!("checkpoint has no snapshot for module {id}"))?;
        let root = manifest
            .root(id)
            .ok_or_else(|| format!("checkpoint has no root for module {id}"))?;
        Ok((bytes, root))
    };

    let mut valset = Valset::new("valset");
    let (bytes, root) = snapshot_of("valset")?;
    valset.install(bytes, root).map_err(|e| format!("valset install: {e}"))?;

    let mut governance = Governance::new("governance", "valset");
    let (bytes, root) = snapshot_of("governance")?;
    governance.install(bytes, root).map_err(|e| format!("governance install: {e}"))?;

    let mut saga = SagaModule::new("saga");
    let (bytes, root) = snapshot_of("saga")?;
    saga.install(bytes, root).map_err(|e| format!("saga install: {e}"))?;

    let mut tasks = Tasks::new("tasks");
    let (bytes, root) = snapshot_of("tasks")?;
    tasks.install(bytes, root).map_err(|e| format!("tasks install: {e}"))?;

    let mut vaults = Vaults::new("vaults");
    let (bytes, root) = snapshot_of("vaults")?;
    vaults.install(bytes, root).map_err(|e| format!("vaults install: {e}"))?;

    let mut inbox = Inbox::new("inbox");
    let (bytes, root) = snapshot_of("inbox")?;
    inbox.install(bytes, root).map_err(|e| format!("inbox install: {e}"))?;

    let mut files = Files::new("files");
    let (bytes, root) = snapshot_of("files")?;
    files.install(bytes, root).map_err(|e| format!("files install: {e}"))?;

    let mut memory = Memory::new("memory");
    let (bytes, root) = snapshot_of("memory")?;
    memory.install(bytes, root).map_err(|e| format!("memory install: {e}"))?;

    let mut jobs = Jobs::new("jobs");
    let (bytes, root) = snapshot_of("jobs")?;
    jobs.install(bytes, root).map_err(|e| format!("jobs install: {e}"))?;

    let mut directory = Directory::new("directory");
    let (bytes, root) = snapshot_of("directory")?;
    directory.install(bytes, root).map_err(|e| format!("directory install: {e}"))?;

    let mut automations = Automations::new("automations", "chat", "tasks");
    let (bytes, root) = snapshot_of("automations")?;
    automations.install(bytes, root).map_err(|e| format!("automations install: {e}"))?;

    Host::genesis(vec![
        Box::new(kv),
        Box::new(document),
        Box::new(chat),
        Box::new(forge),
        Box::new(valset),
        Box::new(governance),
        Box::new(saga),
        Box::new(tasks),
        Box::new(vaults),
        Box::new(inbox),
        Box::new(files),
        Box::new(memory),
        Box::new(jobs),
        Box::new(directory),
        Box::new(automations),
    ])
    .map_err(|e| format!("restore host: {e}"))
}

/// rebuild EVERY production module from a peer's statesync service at
/// `manifest`'s boundary and compose them into a [`Host`], verified against
/// the manifest's app-hash. the disk substrates land under their canonical
/// ids in this process's storage root — this IS the node's state afterwards,
/// not a scratch copy. `attempt` disambiguates runtime child labels across
/// retries (a busy source moves its qmdb targets past the captured boundary;
/// the caller refetches the manifest and tries again, and metrics labels
/// must not collide).
async fn sync_all_modules<C: statesync::SyncClient>(
    context: &commonware_runtime::tokio::Context,
    client: &C,
    manifest: &statesync::Manifest,
    forge_repo: &std::path::Path,
    attempt: usize,
) -> Result<Host, String> {
    let entry_root = |module: &str| -> Result<StateRoot, String> {
        Ok(manifest
            .entry(module)
            .ok_or_else(|| format!("module {module} missing from the manifest"))?
            .root)
    };
    let child_label = |name: &str| -> &'static str {
        Box::leak(format!("{name}_a{attempt}").into_boxed_str())
    };

    // resolver lane: live target through the module lane, gated on the
    // manifest root (a busy source has moved on -> Err -> the caller
    // refetches the manifest at the new boundary), then merkle-verified op
    // batches through the remote resolver.
    let fetch_target = |module: &'static str| {
        let resolver = RemoteQmdbResolver::new(client.clone(), module);
        let root = entry_root(module);
        async move {
            let root = root?;
            let target = resolver
                .fetch_target()
                .await
                .map_err(|e| format!("{module} target: {e}"))?;
            if StateRoot(target.root.0) != root {
                return Err(format!(
                    "{module} live target moved past the captured boundary (busy source)"
                ));
            }
            Ok::<_, String>((target, resolver))
        }
    };

    let (target, resolver) = fetch_target("kv").await?;
    let kv = Kv::sync_from(context.child(child_label("kv")), "kv", target, resolver).await;

    let (target, resolver) = fetch_target("document").await?;
    let document =
        Document::sync_from(context.child(child_label("document")), "document", target, resolver)
            .await;

    let (target, resolver) = fetch_target("chat").await?;
    let chat = Chat::sync_from(context.child(child_label("chat")), "chat", target, resolver).await;

    // snapshot lane: chunked bytes from the captured boundary, install gated
    // on the manifest root (verify-then-adopt inside each module).
    let snapshot_of = |module: &'static str| {
        let client = client.clone();
        let height = manifest.height;
        let root = entry_root(module);
        async move {
            let root = root?;
            let bytes = fetch_snapshot(&client, height, module)
                .await
                .map_err(|e| format!("{module} snapshot: {e}"))?;
            Ok::<_, String>((bytes, root))
        }
    };

    let (bytes, root) = snapshot_of("directory").await?;
    let mut directory = Directory::new("directory");
    directory.install(&bytes, root).map_err(|e| format!("directory install: {e}"))?;

    let (bytes, root) = snapshot_of("valset").await?;
    let mut valset = Valset::new("valset");
    valset.install(&bytes, root).map_err(|e| format!("valset install: {e}"))?;

    let (bytes, root) = snapshot_of("saga").await?;
    let mut saga = SagaModule::new("saga");
    saga.install(&bytes, root).map_err(|e| format!("saga install: {e}"))?;

    let (bytes, root) = snapshot_of("governance").await?;
    let mut governance = Governance::new("governance", "valset");
    governance.install(&bytes, root).map_err(|e| format!("governance install: {e}"))?;

    let (bytes, root) = snapshot_of("tasks").await?;
    let mut tasks = Tasks::new("tasks");
    tasks.install(&bytes, root).map_err(|e| format!("tasks install: {e}"))?;

    let (bytes, root) = snapshot_of("vaults").await?;
    let mut vaults = Vaults::new("vaults");
    vaults.install(&bytes, root).map_err(|e| format!("vaults install: {e}"))?;

    let (bytes, root) = snapshot_of("inbox").await?;
    let mut inbox = Inbox::new("inbox");
    inbox.install(&bytes, root).map_err(|e| format!("inbox install: {e}"))?;

    let (bytes, root) = snapshot_of("files").await?;
    let mut files = Files::new("files");
    files.install(&bytes, root).map_err(|e| format!("files install: {e}"))?;

    let (bytes, root) = snapshot_of("memory").await?;
    let mut memory = Memory::new("memory");
    memory.install(&bytes, root).map_err(|e| format!("memory install: {e}"))?;

    let (bytes, root) = snapshot_of("jobs").await?;
    let mut jobs = Jobs::new("jobs");
    jobs.install(&bytes, root).map_err(|e| format!("jobs install: {e}"))?;

    let (bytes, root) = snapshot_of("automations").await?;
    let mut automations = Automations::new("automations", "chat", "tasks");
    automations.install(&bytes, root).map_err(|e| format!("automations install: {e}"))?;

    let (bytes, root) = snapshot_of("forge").await?;
    let mut forge =
        Forge::init("forge", forge_repo.to_path_buf()).map_err(|e| format!("forge init: {e}"))?;
    forge.install(&bytes, root).map_err(|e| format!("forge install: {e}"))?;

    // compose and check THE property: the rebuilt app-hash IS the manifest's.
    // keep this registry in sync with [`genesis_host`] — a missing module
    // composes a different app-hash and the join fails its final check.
    let host = Host::genesis(vec![
        Box::new(kv),
        Box::new(document),
        Box::new(chat),
        Box::new(forge),
        Box::new(valset),
        Box::new(governance),
        Box::new(saga),
        Box::new(tasks),
        Box::new(vaults),
        Box::new(inbox),
        Box::new(files),
        Box::new(memory),
        Box::new(jobs),
        Box::new(automations),
        Box::new(directory),
    ])
    .map_err(|e| format!("compose synced host: {e}"))?;
    if host.app_hash() != manifest.app_hash {
        return Err(format!(
            "composed {} != manifest {}",
            hex(&host.app_hash()),
            hex(&manifest.app_hash)
        ));
    }
    Ok(host)
}

/// replace this process with a fresh invocation of itself (same argv): the
/// clean way to re-enter boot with a different network topology — discovery
/// channels can only be registered before `network.start()`, so a promoted
/// joiner cannot grow a consensus engine in-process.
fn reboot_self() -> ! {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        let exe = std::env::current_exe().expect("current exe path");
        let err = std::process::Command::new(exe)
            .args(std::env::args_os().skip(1))
            .exec();
        eprintln!("FATAL: validator reboot exec failed: {err}");
        std::process::exit(1);
    }
    #[cfg(not(unix))]
    {
        println!("promoted — restart this node to run as a validator");
        std::process::exit(0);
    }
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
        Some("invite-accept") => return cmd_invite_accept(&args[1..]),
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
                    "unexpected arg {other:?} (want a subcommand — \
                     keygen|init|invite|admit|invite-accept|join — or \
                     --config <path> [--sync-only])"
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
    // re-running init would mint a FRESH chain-id and reset the validator set
    // to just this identity — silently un-founding the network under every
    // holder of an existing invite. founding is once per directory.
    let descriptor_path = dir.join("network.toml");
    if descriptor_path.exists() {
        return Err(format!(
            "{} already exists — this directory is already a network. use `invite`/`admit` \
             for membership, or delete the file to re-found from scratch",
            descriptor_path.display()
        )
        .into());
    }
    let plumbing = config::merged_plumbing(
        &dir,
        flags.get("listen").map(String::as_str),
        flags.get("advertised").map(String::as_str),
        flags.get("http").map(String::as_str),
        flags.get("rpc").map(String::as_str),
    )?;

    let (key, generated) = config::load_or_generate_identity(&dir.join("identity.key"))?;
    let me = key.public_key();
    let chain_id = config::mint_chain_id(name, &me);
    let mut descriptor = config::NetworkDescriptor {
        chain_id: chain_id.clone(),
        scheme: config::SCHEME_ED25519.into(),
        validators: vec![hex_bytes(me.as_ref())],
        bootstrap: Vec::new(),
    };
    if let Some(addr) = config::dialable(plumbing.advertised.as_deref(), &plumbing.listen)? {
        descriptor.add_bootstrap(&me, &addr);
    }
    descriptor.save(&descriptor_path)?;
    config::write_node_toml(&dir, &plumbing)?;
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
    match config::dialable(raw.advertised.as_deref(), &raw.listen)? {
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

// ---- invite-accept: post-genesis admission over the local rpc --------------

/// one blocking json-lines rpc round-trip against the LOCAL node.
fn rpc_call(addr: &str, req: &serde_json::Value) -> Result<serde_json::Value, String> {
    use std::io::{BufRead as _, BufReader, Write as _};
    let conn = std::net::TcpStream::connect(addr)
        .map_err(|e| format!("connect rpc {addr}: {e} (is the node running?)"))?;
    conn.set_read_timeout(Some(std::time::Duration::from_secs(15)))
        .map_err(|e| format!("rpc timeout: {e}"))?;
    let mut writer = conn.try_clone().map_err(|e| format!("rpc clone: {e}"))?;
    let mut line = serde_json::to_string(req).expect("rpc request serializes");
    line.push('\n');
    writer.write_all(line.as_bytes()).map_err(|e| format!("rpc write: {e}"))?;
    let mut reply = String::new();
    BufReader::new(conn)
        .read_line(&mut reply)
        .map_err(|e| format!("rpc read: {e}"))?;
    serde_json::from_str(reply.trim()).map_err(|e| format!("rpc reply: {e}"))
}

/// query a module through the rpc; the reply's hex payload, decoded.
fn rpc_query(addr: &str, target: &str, req: &[u8]) -> Result<Vec<u8>, String> {
    let reply = rpc_call(
        addr,
        &serde_json::json!({ "cmd": "query", "target": target, "req_hex": hex_bytes(req) }),
    )?;
    if reply["ok"] != true {
        return Err(format!("query {target}: {}", reply["error"]));
    }
    unhex(reply["reply_hex"].as_str().ok_or("query reply carries no payload")?)
}

/// submit an op through the rpc (accepted != finalized — poll afterwards).
fn rpc_submit(addr: &str, target: &str, payload: &[u8]) -> Result<(), String> {
    let reply = rpc_call(
        addr,
        &serde_json::json!({ "cmd": "submit", "target": target, "payload_hex": hex_bytes(payload) }),
    )?;
    if reply["ok"] != true {
        return Err(format!("submit to {target}: {}", reply["error"]));
    }
    Ok(())
}

fn read_members(addr: &str) -> Result<Vec<Vec<u8>>, String> {
    use valset_interface::{decode_reply, encode_query, ValsetQuery, ValsetReply};
    let raw = rpc_query(addr, "valset", &encode_query(&ValsetQuery::Validators))?;
    match decode_reply(&raw)? {
        ValsetReply::Validators(v) => Ok(v),
    }
}

fn read_proposal(
    addr: &str,
    id: &str,
) -> Result<Option<governance_interface::ProposalView>, String> {
    use governance_interface::{decode_reply, encode_query, GovQuery, GovReply};
    let raw = rpc_query(
        addr,
        "governance",
        &encode_query(&GovQuery::Proposal { proposal_id: id.into() }),
    )?;
    match decode_reply(&raw)? {
        GovReply::Proposal(view) => Ok(view),
        other => Err(format!("unexpected governance reply: {other:?}")),
    }
}

/// poll a proposal until `pred` accepts its view, ~30s budget (ops finalize
/// within a few pump ticks; the budget covers a mesh still forming quorum).
fn poll_proposal(
    addr: &str,
    id: &str,
    what: &str,
    mut pred: impl FnMut(&Option<governance_interface::ProposalView>) -> bool,
) -> Result<Option<governance_interface::ProposalView>, String> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let view = read_proposal(addr, id)?;
        if pred(&view) {
            return Ok(view);
        }
        if std::time::Instant::now() >= deadline {
            return Err(format!("timed out waiting for {what}"));
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
}

/// `invite-accept <hex pubkey> [--config node.toml]` — post-genesis
/// admission: drive a governance AddValidator proposal for `pubkey` through
/// this member's own RUNNING node. idempotent across members — each runs the
/// same command (propose if absent, cast a yes ballot, execute once
/// decidable); the run that lands the deciding ballot executes. the passing
/// proposal's valset Join schedules the epoch cutover that re-tracks the
/// mesh, at which point the parked joiner syncs and promotes itself.
fn cmd_invite_accept(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use governance_interface::{encode_msg, GovAction, GovMsg, ProposalStatus};

    let (pos, flags) = parse_flags(args)?;
    let [pubkey_hex] = pos.as_slice() else {
        return Err("invite-accept needs exactly one <hex pubkey>".into());
    };
    let key = config::decode_key(pubkey_hex)?;
    let key_bytes = key.as_ref().to_vec();
    let cfg_path = PathBuf::from(
        flags
            .get("config")
            .map(String::as_str)
            .unwrap_or("node.toml"),
    );
    let (raw, base) = config::load_node_toml(&cfg_path)?;
    let rpc_addr = raw
        .rpc_listen
        .clone()
        .ok_or("invite-accept drives the node's local rpc — set `rpc_listen` in node.toml")?;
    // the ballots this verb casts are signed by the NODE's identity (the
    // ordered lane signs every rpc submit with it) — that key must be the
    // member, and it is: the node is the local operator's custodian.
    let me = config::load_identity(&base.join(raw.key_file.as_deref().unwrap_or("identity.key")))?;
    let me_bytes = me.public_key().as_ref().to_vec();

    let members = read_members(&rpc_addr)?;
    if members.contains(&key_bytes) {
        eprintln!("{pubkey_hex} is already a validator — nothing to do");
        return Ok(());
    }
    if !members.contains(&me_bytes) {
        return Err("this node's identity is not a current member — only members admit \
                    validators"
            .into());
    }

    // adopt an existing OPEN proposal for exactly this action, else mint an
    // unused id (settled proposals keep their ids forever — a re-admitted
    // key gets a fresh suffix).
    use governance_interface::{decode_reply, encode_query, GovQuery, GovReply};
    let proposals = match decode_reply(&rpc_query(
        &rpc_addr,
        "governance",
        &encode_query(&GovQuery::Proposals),
    )?)? {
        GovReply::Proposals(views) => views,
        other => return Err(format!("unexpected governance reply: {other:?}").into()),
    };
    let wanted = GovAction::AddValidator { key: key_bytes.clone() };
    let proposal_id = match proposals
        .iter()
        .find(|p| p.status == ProposalStatus::Open && p.action == wanted)
    {
        Some(p) => {
            eprintln!("joining open proposal {}", p.proposal_id);
            p.proposal_id.clone()
        }
        None => {
            let prefix: String = pubkey_hex.chars().take(16).collect();
            let id = (0u64..)
                .map(|n| format!("admit:{prefix}:{n}"))
                .find(|id| !proposals.iter().any(|p| &p.proposal_id == id))
                .expect("the id space is unbounded");
            rpc_submit(
                &rpc_addr,
                "governance",
                &encode_msg(&GovMsg::Propose {
                    proposal_id: id.clone(),
                    action: wanted,
                    // a far horizon in consensus-time units (heights advance
                    // about one per finalized op): admission must not expire
                    // under a slow second ballot.
                    voting_period: 1_000_000,
                }),
            )?;
            poll_proposal(&rpc_addr, &id, "the proposal to finalize", |p| p.is_some())?;
            eprintln!("proposed {id}");
            id
        }
    };

    rpc_submit(
        &rpc_addr,
        "governance",
        &encode_msg(&GovMsg::Vote { proposal_id: proposal_id.clone(), approve: true }),
    )?;
    let after_vote = poll_proposal(&rpc_addr, &proposal_id, "this ballot to finalize", |p| {
        p.as_ref().is_some_and(|v| {
            v.status != ProposalStatus::Open
                || v.votes.iter().any(|(voter, yes)| voter == &me_bytes && *yes)
        })
    })?
    .expect("the poll only accepts a present proposal");
    eprintln!("ballot cast as {}", hex_bytes(&me_bytes));

    // execute only when decidable — a strict-majority shortfall is the
    // normal n>=2 intermediate state, not an error.
    let members = read_members(&rpc_addr)?;
    let yes = members
        .iter()
        .filter(|m| {
            after_vote
                .votes
                .iter()
                .any(|(voter, approve)| voter == *m && *approve)
        })
        .count();
    let majority = members.len() / 2 + 1;
    if after_vote.status == ProposalStatus::Open && yes < majority {
        eprintln!(
            "{yes} of {majority} required ballots — waiting on other members. each runs:\n    \
             ducktape-node invite-accept {pubkey_hex} --config <their node.toml>"
        );
        return Ok(());
    }
    if after_vote.status == ProposalStatus::Open {
        rpc_submit(
            &rpc_addr,
            "governance",
            &encode_msg(&GovMsg::Execute { proposal_id: proposal_id.clone() }),
        )?;
    }
    let settled = poll_proposal(&rpc_addr, &proposal_id, "the tally to settle", |p| {
        p.as_ref().is_some_and(|v| v.status != ProposalStatus::Open)
    })?
    .expect("the poll only accepts a present proposal");
    match settled.status {
        ProposalStatus::Passed => {
            eprintln!(
                "admitted {pubkey_hex}: the validator set changes at the next epoch cutover, \
                 and the joiner's parked node will sync and promote itself"
            );
            Ok(())
        }
        status => Err(format!("proposal {proposal_id} settled as {status:?}").into()),
    }
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
    config::guard_join_descriptor(&dir, &descriptor)?;
    // plumbing merges: explicit flags win, an existing node.toml's values
    // (network- or dev-shape) survive, defaults fill the rest. computed
    // BEFORE anything lands on disk so a corrupt existing node.toml aborts
    // the join without leaving a half-migrated dir. the file is ALWAYS
    // rewritten in the network shape — a join must take effect even in a dir
    // holding the app's dev-shape solo config.
    let plumbing = config::merged_plumbing(
        &dir,
        flags.get("listen").map(String::as_str),
        flags.get("advertised").map(String::as_str),
        flags.get("http").map(String::as_str),
        flags.get("rpc").map(String::as_str),
    )?;
    descriptor.save(&dir.join("network.toml"))?;
    let (key, generated) = config::load_or_generate_identity(&dir.join("identity.key"))?;
    let me_hex = hex_bytes(key.public_key().as_ref());
    config::write_node_toml(&dir, &plumbing)?;
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
        eprintln!("NOT yet a member. send this identity to a member, then:");
        eprintln!("  running network: the member runs `ducktape-node invite-accept {me_hex}`,");
        eprintln!(
            "    and you start now — `ducktape-node --config {}/node.toml` parks on the \
             mesh and promotes itself once admitted;",
            dir.display()
        );
        eprintln!("  before genesis: the member runs `ducktape-node admit {me_hex}` and you");
        eprintln!("    join again with the refreshed invite (the identity here is kept).");
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
        checkpoint_blocks,
    } = resolved;
    // a key outside the GENESIS validator set is not an error: post-genesis
    // members are admitted via governance. with a recovery checkpoint on disk
    // (a previous run promoted this identity) boot proceeds as a validator
    // off the recovery record; with a fresh storage dir the node enters
    // JOINER mode — park on the mesh, sync a boundary whose participant set
    // includes this key, fabricate the equivalent recovery checkpoint, and
    // reboot through the normal restore path. this filesystem probe mirrors
    // the recovery store's layout (storage_dir/<partition>/) and only gates
    // which listeners bind — the runtime re-decides joiner-vs-validator from
    // the real store.
    let promoted = storage
        .join("recovery-manifest")
        .read_dir()
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false);
    let joiner = !sync_only && !validators.contains(&signer.public_key()) && !promoted;
    if joiner {
        println!(
            "[node {label}] identity {} is not in the genesis validator set — joiner mode: \
             parking on the mesh until a member runs `ducktape-node invite-accept {}`",
            hex_bytes(signer.public_key().as_ref()),
            hex_bytes(signer.public_key().as_ref())
        );
    }

    // keep the raw (key, addr) pairs for statesync source selection before
    // discovery's bootstrapper list converts to its own ingress address type.
    let sync_candidates = bootstrappers.clone();
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
        "[node {label}] starting on {listen} ({} mesh peers, {} validators{}), namespace {}, storage {}",
        peers.len(),
        validators.len(),
        if sync_only { ", sync-only" } else { "" },
        String::from_utf8_lossy(&namespace),
        storage.display()
    );

    // the rpc listener binds OUTSIDE the runtime (plain std tcp on OS threads)
    // so a bind failure is a clean startup error, not an async surprise.
    let rpc_listener = match rpc_listen.as_deref() {
        Some(addr) if !sync_only && !joiner => Some(std::net::TcpListener::bind(addr)?),
        _ => None,
    };

    // the http/ws app surface: same bind-early rule. the server itself runs on
    // its OWN plain-tokio OS thread (noded's exact split — the host never
    // leaves the commonware runner thread; http handlers only send
    // NodeCommands over the lane), so the pump below is its single consumer.
    let (http_handle, http_cmds, http_events) = noded::NodeHandle::channel();
    match http_listen.as_deref() {
        Some(addr) if !sync_only && !joiner => {
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
        // consensus scheme uses the (possibly smaller) validator set derived
        // from committed valset state after the recovery boot below.
        let mesh_participants: Set<ed25519::PublicKey> =
            Set::try_from(peers.clone()).expect("authorized peer set has no duplicates");

        // the statesync source a --sync-only joiner pulls from: only
        // validators serve the channel, so the candidate must be a validator
        // that is not us (a non-validator hint or our own key would be
        // retried forever — discovery never connects a node to itself).
        let sync_source =
            config::choose_sync_source(&sync_candidates, &validators, &signer.public_key());

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

        let quota = Quota::per_second(NZU32!(128));

        if sync_only {
            // no consensus coordinates yet: track the genesis mesh at the
            // base index. validators ignore this index if they have rotated
            // past keeping it; connection authorization is the UNION of every
            // tracked set on each side, so the descriptor's members stay
            // reachable.
            oracle.track(PEER_SET, mesh_participants.clone());
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
                let (vote, cert, res, payload, fetch) = engine_channels(epoch);
                for ch in [vote, cert, res, payload, fetch] {
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

            let Some(server_peer) = sync_source else {
                eprintln!(
                    "[node {label}] no statesync source: no validator other than this node \
                     is available to serve (only validators answer the statesync channel)"
                );
                std::process::exit(1);
            };
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

            // rebuild EVERY module in the manifest (a REAL joiner owns its
            // disk, so every store opens under its canonical module id) and
            // print the greppable line the demo script asserts on.
            let forge_repo = storage_for_sync.join("forge-repo");
            match sync_all_modules(&context, &client, &manifest, &forge_repo, 0).await {
                Ok(host) => {
                    println!("[node {label}] synced app_hash={}", hex(&host.app_hash()));
                }
                Err(e) => {
                    eprintln!("[node {label}] SYNC FAILED: {e}");
                    std::process::exit(1);
                }
            }
            return;
        }

        // ---- a VALIDATOR: consensus engine + state-sync service -------------

        // recovery-aware boot FIRST: the app state (and with it the epoch to
        // respawn) must be known before the mesh wiring below decides which
        // epochs' channels to live on. everything here is local disk io.
        let mut recovery = match Recovery::open(context.child("recovery")).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[node {label}] FATAL: cannot open the recovery store: {e}");
                std::process::exit(1);
            }
        };
        let manifest = match recovery.manifest() {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[node {label}] FATAL: recovery checkpoint is damaged: {e}");
                std::process::exit(1);
            }
        };
        let forge_repo = storage_for_sync.join("forge-repo");

        // ---- the JOINER: park on the mesh, sync a boundary that includes
        // this key, fabricate the equivalent recovery checkpoint, reboot ----
        //
        // decided from the REAL store (the pre-runtime probe only gated
        // listeners): no checkpoint + a key outside the genesis set. after
        // promotion the checkpoint exists, so a rebooted process falls
        // through to the validator path below.
        if manifest.is_none() && !validators.contains(&signer.public_key()) {
            if !recovery.journal_is_empty().await {
                eprintln!(
                    "[node {label}] FATAL: recovery journal exists but the checkpoint is \
                     missing — wipe the app state and re-join (KEEP any consensus journal \
                     partitions: they are what prevents this key from double-voting)"
                );
                std::process::exit(1);
            }
            // the parked mesh identity: genesis set at the base index (no
            // consensus coordinates yet), engine lanes black-holed exactly
            // like the sync-only observer — an unregistered channel is a
            // protocol violation that kills the very connection the sync
            // client needs.
            oracle.track(PEER_SET, mesh_participants.clone());
            for epoch in 0..EPOCH_CHANNEL_BANK {
                let (vote, cert, res, payload, fetch) = engine_channels(epoch);
                for ch in [vote, cert, res, payload, fetch] {
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

            let Some(server_peer) = sync_source else {
                eprintln!(
                    "[node {label}] no statesync source: no validator other than this node \
                     is available to serve (only validators answer the statesync channel)"
                );
                std::process::exit(1);
            };
            let client =
                P2pSyncClient::new(context.child("sync_client"), sync_tx, sync_rx, server_peer);

            let me_bytes = signer.public_key().as_ref().to_vec();
            let mut last_tracked = PEER_SET;
            let mut attempt = 0usize;
            let (boundary, host) = loop {
                attempt += 1;
                if attempt > 900 {
                    // ~30 minutes of 2s retries: parking forever is operator
                    // guidance territory, not a silent spin.
                    eprintln!(
                        "[node {label}] FATAL: still not admitted after {attempt} attempts — \
                         has a member run `ducktape-node invite-accept {}`?",
                        hex_bytes(&me_bytes)
                    );
                    std::process::exit(1);
                }
                context.sleep(Duration::from_secs(2)).await;
                let m = match fetch_manifest(&client).await {
                    Ok(m) => m,
                    Err(e) => {
                        println!("[node {label}] parked: mesh unreachable ({e}); retrying");
                        continue;
                    }
                };
                // follow the mesh rotation while parked. the participant
                // list is an unverified serving hint — the union with the
                // descriptor mesh keeps the real members reachable, and
                // promotion re-derives everything from verified state.
                if m.epoch > last_tracked {
                    if m.epoch >= EPOCH_CHANNEL_BANK {
                        println!(
                            "[node {label}] warning: the network is at epoch {} — beyond this \
                             process's pre-registered channel bank ({EPOCH_CHANNEL_BANK}); \
                             expect reconnect churn while parked",
                            m.epoch
                        );
                    }
                    let mut union: std::collections::BTreeSet<ed25519::PublicKey> =
                        peers.iter().cloned().collect();
                    for k in &m.participants {
                        if let Ok(pk) = ed25519::PublicKey::decode(k.as_slice()) {
                            union.insert(pk);
                        }
                    }
                    oracle.track(
                        m.epoch,
                        Set::try_from(union.into_iter().collect::<Vec<_>>())
                            .expect("a btree-set union has no duplicates"),
                    );
                    last_tracked = m.epoch;
                }
                if !m.participants.iter().any(|k| k == &me_bytes) {
                    println!(
                        "[node {label}] parked: awaiting admission (epoch {} has {} validators)",
                        m.epoch,
                        m.participants.len()
                    );
                    continue;
                }
                // in the epoch set. a boundary PAST the epoch base needs its
                // finalization floor served alongside, or the respawned
                // engine would re-deliver history the synced state already
                // contains — retry until the source's floor catches up.
                if m.height > m.view_base && m.floor_cert.is_none() {
                    println!(
                        "[node {label}] admitted; boundary {} lacks its finalization floor \
                         yet — retrying",
                        m.height
                    );
                    continue;
                }
                println!(
                    "[node {label}] admitted at epoch {} boundary {} — syncing {} modules",
                    m.epoch,
                    m.height,
                    m.entries.len()
                );
                match sync_all_modules(&context, &client, &m, &forge_repo, attempt).await {
                    Ok(host) => break (m, host),
                    // a busy source moves its live qmdb targets past the
                    // captured boundary mid-sync; refetch and try again at
                    // the new boundary.
                    Err(e) => println!("[node {label}] sync at boundary {} failed: {e}", m.height),
                }
            };
            println!("[node {label}] synced app_hash={}", hex(&host.app_hash()));

            // validate the floor certificate against the epoch's scheme
            // BEFORE persisting it — a lying source must fail the join here,
            // not brick the validator boot after.
            let floor = if boundary.height > boundary.view_base {
                let cert = boundary
                    .floor_cert
                    .clone()
                    .expect("the park loop only breaks past the base with a floor");
                let mut keys = Vec::with_capacity(boundary.participants.len());
                for k in &boundary.participants {
                    match ed25519::PublicKey::decode(k.as_slice()) {
                        Ok(pk) => keys.push(pk),
                        Err(e) => {
                            eprintln!(
                                "[node {label}] FATAL: served participant set holds a \
                                 non-ed25519 key: {e}"
                            );
                            std::process::exit(1);
                        }
                    }
                }
                let participants =
                    Set::try_from(keys).expect("served participant set has no duplicates");
                let scheme = match CONSENSUS_SCHEME {
                    ConsensusScheme::V1Ed25519 => simplex_ed25519::Scheme::signer(
                        &namespace,
                        participants,
                        signer.clone(),
                    )
                    .expect("our key is in the served participant set"),
                    ConsensusScheme::V2Bls => unimplemented!(
                        "V2Bls joiner wiring lands with valset bls key registration"
                    ),
                };
                if let Err(e) = consensus::decode_finalization(&scheme, &cert) {
                    eprintln!(
                        "[node {label}] FATAL: served finalization floor does not verify \
                         against the epoch's participant set: {e}"
                    );
                    std::process::exit(1);
                }
                Some(cert)
            } else {
                None
            };

            // fabricate the checkpoint a restart would have left; the normal
            // recovery boot turns it into a live validator. next_seq starts
            // at 1 — this identity never framed ops on this network. (a
            // REJOINING key that later resubmits a byte-identical (seq,
            // payload) pair could be dropped by a peer's in-process digest
            // gate; accepted edge until submit sequences ride app state.)
            let pos = recovery.oplog_pos().await;
            let ckpt = match Manifest::capture(
                &host,
                Some(boundary.height),
                boundary.epoch,
                boundary.view_base,
                boundary.participants.clone(),
                None,
                pos,
                1,
            ) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("[node {label}] FATAL: promotion checkpoint capture: {e}");
                    std::process::exit(1);
                }
            };
            if let Err(e) = recovery.write_manifest(&ckpt).await {
                eprintln!("[node {label}] FATAL: promotion checkpoint write: {e}");
                std::process::exit(1);
            }
            if let Some(cert) = floor {
                let fc = recovery::FloorCert {
                    epoch: boundary.epoch,
                    height: boundary.height,
                    cert,
                };
                if let Err(e) = recovery.write_floor_cert(&fc).await {
                    eprintln!("[node {label}] FATAL: promotion floor-cert write: {e}");
                    std::process::exit(1);
                }
            }
            println!(
                "[node {label}] promoted: validator at epoch {} boundary {} — rebooting",
                boundary.epoch, boundary.height
            );
            reboot_self();
        }
        // (host, recovered-state, next local submit seq, last checkpoint
        // (height, oplog position) for the pump's prune bookkeeping, and a
        // cutover the pre-crash process had armed but not crossed).
        let (host, resumed, mut next_seq, mut prev_ckpt, pending_boot): (
            Host,
            Option<recovery::Recovered>,
            u64,
            (Option<u64>, u64),
            Option<u64>,
        ) = match manifest {
            None => {
                // a journal without a checkpoint is damage, not a fresh dir —
                // booting genesis over it would silently fork this node.
                if !recovery.journal_is_empty().await {
                    eprintln!(
                        "[node {label}] FATAL: recovery journal exists but the checkpoint is \
                         missing — wipe the app state and re-sync (KEEP the consensus journal \
                         partitions: they are what prevents this key from double-voting)"
                    );
                    std::process::exit(1);
                }
                let host = genesis_host(&context, &forge_repo, &validators).await;
                let pos = recovery.oplog_pos().await;
                let genesis_participants: Vec<Vec<u8>> =
                    validators.iter().map(|k| k.as_ref().to_vec()).collect();
                // seq 0 is the dev demo op's; real submits start at 1.
                let genesis_manifest =
                    match Manifest::capture(&host, None, 0, 0, genesis_participants, None, pos, 1)
                    {
                        Ok(m) => m,
                        Err(e) => {
                            eprintln!("[node {label}] FATAL: genesis checkpoint capture: {e}");
                            std::process::exit(1);
                        }
                    };
                if let Err(e) = recovery.write_manifest(&genesis_manifest).await {
                    eprintln!("[node {label}] FATAL: genesis checkpoint write: {e}");
                    std::process::exit(1);
                }
                (host, None, 1, (None, pos), None)
            }
            Some(manifest) => {
                let mut host = match restore_host(&context, &forge_repo, &manifest).await {
                    Ok(h) => h,
                    Err(e) => {
                        eprintln!("[node {label}] FATAL: checkpoint restore: {e}");
                        std::process::exit(1);
                    }
                };
                let rec = match recovery.recover(&mut host, &manifest).await {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!(
                            "[node {label}] FATAL: {e}\n\
                             [node {label}] app state cannot be locally recovered. wipe the \
                             app-state partitions and re-sync from a peer — but ALWAYS keep \
                             the consensus journal partitions (\"<pubkey>-e<epoch>\"): they \
                             are the anti-equivocation record for this key."
                        );
                        std::process::exit(1);
                    }
                };
                // advance the local submit sequence past everything this
                // identity may already have framed: the checkpointed floor,
                // then any retained frame of ours above it.
                let me_bytes = signer.public_key().as_ref().to_vec();
                let mut next_seq = manifest.next_seq;
                for frame in &rec.frames {
                    if let Some((origin, seq)) = node::frame_origin_seq(frame) {
                        if origin == me_bytes {
                            next_seq = next_seq.max(seq + 1);
                        }
                    }
                }
                println!(
                    "[node {label}] recovered app_hash={} height={} epoch={} (replayed {}, \
                     already-on-disk {}{})",
                    hex(&rec.app_hash),
                    rec.height.map(|h| h.to_string()).unwrap_or_else(|| "genesis".into()),
                    rec.epoch,
                    rec.applied,
                    rec.skipped,
                    if rec.rolled_forward { ", rolled 1 forward" } else { "" },
                );
                // a cutover armed but not crossed before the crash: recorded
                // in the checkpoint (valid only while no cutover happened
                // since — a newer epoch means that boundary was crossed), or
                // derived from the replayed seals: the first CURRENT-epoch
                // block that moved the valset root armed the boundary at its
                // view + the delay. deterministic — the same block arms the
                // same boundary on every node.
                let pending_boot = if rec.epoch == manifest.epoch {
                    manifest.pending_cutover_view
                } else {
                    None
                }
                .or_else(|| {
                    let mut prev_root =
                        manifest.root("valset").expect("valset is a genesis module");
                    let mut armed = None;
                    for (height, roots) in &rec.blocks {
                        let root = roots
                            .iter()
                            .find(|(id, _)| id == "valset")
                            .map(|(_, r)| *r)
                            .expect("every seal carries the full root vector");
                        if root != prev_root && *height > rec.view_base && armed.is_none() {
                            armed = Some(*height - rec.view_base + CUTOVER_DELAY);
                        }
                        prev_root = root;
                    }
                    armed
                });
                let prev = (manifest.height, manifest.oplog_pos);
                (host, Some(rec), next_seq, prev, pending_boot)
            }
        };

        // consensus membership comes from the RECOVERY RECORD: the epoch's
        // ENGINE PARTICIPANT SET (at genesis: exactly the config seed). the
        // recovered valset projection is NOT it — a restart inside a cutover
        // window would read a membership change whose boundary has not been
        // crossed and spawn a different scheme than its peers are running.
        let member_keys: Vec<ed25519::PublicKey> = {
            let raw: Vec<Vec<u8>> = match &resumed {
                Some(rec) => rec.participants.clone(),
                None => validators.iter().map(|k| k.as_ref().to_vec()).collect(),
            };
            let mut keys = Vec::with_capacity(raw.len());
            for k in &raw {
                match ed25519::PublicKey::decode(k.as_slice()) {
                    Ok(pk) => keys.push(pk),
                    Err(e) => {
                        eprintln!(
                            "[node {label}] FATAL: recovered participant set holds a \
                             non-ed25519 key: {e}"
                        );
                        std::process::exit(1);
                    }
                }
            }
            keys
        };
        if !member_keys.contains(&signer.public_key()) {
            println!(
                "[node {label}] this identity is not in the recovered validator set — \
                 halting (restart with --sync-only to observe)"
            );
            std::process::exit(0);
        }
        let participants: Set<ed25519::PublicKey> =
            Set::try_from(member_keys.clone()).expect("valset membership has no duplicates");
        let resume_epoch = resumed.as_ref().map(|r| r.epoch).unwrap_or(0);

        // the validator-owned transport mesh, tracked at index = epoch: the
        // epoch's participants ∪ the descriptor mesh (genesis members + [dev]
        // extras — kept authorized so demoted members and pre-genesis peers
        // can still reach the statesync service). the SAME set on every node
        // at this index: discovery kills peers whose bit-vector length
        // disagrees at a shared index, and epoch participant sets are the
        // only membership every node agrees on epoch-for-epoch.
        let mesh_at = {
            let descriptor_mesh = peers.clone();
            move |epoch_members: &std::collections::BTreeSet<ed25519::PublicKey>| {
                let mut union: std::collections::BTreeSet<ed25519::PublicKey> =
                    descriptor_mesh.iter().cloned().collect();
                union.extend(epoch_members.iter().cloned());
                Set::try_from(union.into_iter().collect::<Vec<_>>())
                    .expect("a btree-set union has no duplicates")
            }
        };
        let mut mesh_oracle = oracle.clone();
        mesh_oracle.track(
            resume_epoch,
            mesh_at(&member_keys.iter().cloned().collect()),
        );

        // lanes for epochs BELOW the resume epoch are registered and
        // black-holed (the sync-only arm's exact trick): a lagging peer still
        // gossips there, and an unregistered channel is a protocol violation
        // that would kill its connection — cutting off the very fetch lane it
        // needs to catch up.
        for epoch in 0..resume_epoch {
            let (vote, cert, res, payload, fetch) = engine_channels(epoch);
            for ch in [vote, cert, res, payload, fetch] {
                let (_tx, mut rx) = network.register(ch, quota, MAX_BACKLOG);
                let label: &'static str = Box::leak(format!("blackhole_{ch}").into_boxed_str());
                context.child(label).spawn(move |_ctx| async move {
                    while rx.recv().await.is_ok() {}
                });
            }
        }

        // pre-register the epoch channel bank from the RESUME epoch up
        // (registration is only possible before network.start(); every
        // respawned engine needs fresh channels). bank[i] holds epoch
        // (bank_base + i)'s (vote, certificate, resolver, payload, fetch)
        // pairs until that epoch's engine consumes them. a restart therefore
        // re-arms the full window — EPOCH_CHANNEL_BANK bounds membership
        // changes per process RUN, not per network lifetime.
        let bank_base = resume_epoch;
        let mut channel_bank: Vec<Option<_>> = (0..EPOCH_CHANNEL_BANK)
            .map(|i| {
                let (vote, cert, res, payload, fetch) = engine_channels(bank_base + i);
                Some((
                    network.register(vote, quota, MAX_BACKLOG),
                    network.register(cert, quota, MAX_BACKLOG),
                    network.register(res, quota, MAX_BACKLOG),
                    network.register(payload, quota, MAX_BACKLOG),
                    network.register(fetch, quota, MAX_BACKLOG),
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

        // spawn one epoch's engine from the channel bank. scheme built the
        // production way (`signer` finds our key's index in the sorted
        // participant set); per-epoch genesis floor + per-epoch storage
        // partition, so a respawned engine can never collide with a
        // predecessor. the consensus signature scheme is a GENESIS-WIDE
        // constant (ConsensusScheme); adding V2Bls makes the match
        // non-exhaustive — the compiler-enforced rekey point.
        let spawn_epoch = |bank: &mut Vec<Option<_>>,
                               epoch: u64,
                               participants: Set<ed25519::PublicKey>,
                               store: ContentStore,
                               floor_bytes: Option<Vec<u8>>|
         -> SimplexOrderer {
            let slot = bank
                .get_mut(epoch.checked_sub(bank_base).expect("epochs never rebase down") as usize)
                .and_then(|s| s.take())
                .unwrap_or_else(|| {
                    eprintln!(
                        "[node {label}] FATAL: epoch {epoch} exhausts the pre-registered                          channel bank ({EPOCH_CHANNEL_BANK}) — rebuild with a wider bank"
                    );
                    std::process::exit(1);
                });
            let (vote, certificate, resolver, payload, fetch) = slot;
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
            // a SAME-EPOCH respawn passes the persisted finalization floor so
            // the reopened journal's replay does not re-report history the
            // recovered state already contains. a damaged floor FAILS — a
            // silent genesis-floor fallback would resurrect the wedge.
            let floor = floor_bytes.map(|bytes| {
                match consensus::decode_finalization(&scheme, &bytes) {
                    Ok(f) => f,
                    Err(e) => {
                        eprintln!("[node {label}] FATAL: {e}");
                        std::process::exit(1);
                    }
                }
            });
            let label: &'static str =
                Box::leak(format!("consensus_e{epoch}").into_boxed_str());
            // spawn WITH the lazy payload-fetch backstop: quorum is a SUBSET
            // (n - floor((n-1)/3)), so a validator can finalize a view it never
            // voted in — and if it also missed the one-shot relay gossip (mesh
            // still forming, transient disconnect), relay-only wiring would
            // silently drop that op's slot and wedge/fork the node. the
            // resolver fetches missing bytes by digest from the tracked mesh
            // (the oracle is provider AND blocker) and fills the ordered slot.
            SimplexOrderer::spawn_with_resolver(
                context.child(label),
                scheme,
                oracle.clone(),
                oracle.clone(),
                signer.public_key(),
                format!("{}-e{epoch}", signer.public_key()),
                Epoch::new(epoch),
                epoch_floor(&namespace, epoch),
                floor,
                // per-process, PER-EPOCH content store: pins/pending of a torn
                // down epoch die with it (in-flight ops are resubmitted). a
                // RESTART's store arrives pre-seeded from the recovery journal.
                store,
                vote,
                certificate,
                resolver,
                payload,
                fetch,
                false,
            )
        };

        // the boot store: seeded with every retained journaled frame so
        // finalizations the reopened engine re-reports (at most the floor
        // cert itself, plus anything finalized-but-undrained at the crash)
        // resolve locally instead of wedging the ordered gate.
        let boot_store = ContentStore::new();
        if let Some(rec) = &resumed {
            for frame in &rec.frames {
                boot_store.pin(frame.clone());
            }
        }
        // the persisted floor is only valid for the epoch it was recorded in
        // (Floor::assert pins the certificate to the engine's epoch).
        let boot_floor = match recovery.floor_cert() {
            Ok(cert) => cert.filter(|c| c.epoch == resume_epoch),
            Err(e) => {
                eprintln!("[node {label}] FATAL: persisted finalization floor is damaged: {e}");
                std::process::exit(1);
            }
        };
        let mut last_cert_height = boot_floor.as_ref().map(|c| c.height);
        // the newest persisted finalization floor, kept in memory so the
        // statesync service can serve it to joiners at a matching boundary.
        let mut latest_floor: Option<recovery::FloorCert> = boot_floor.clone();
        let orderer = spawn_epoch(
            &mut channel_bank,
            resume_epoch,
            participants.clone(),
            boot_store,
            boot_floor.map(|c| c.cert),
        );
        let view_base = resumed.as_ref().map(|r| r.view_base).unwrap_or(0);
        let mut node = match &resumed {
            Some(rec) => OrderedNode::resume(
                host,
                orderer,
                recovery,
                rec.height
                    .map(|height| host::FinalizedBlock { height, app_hash: rec.app_hash }),
                rec.view_base,
            ),
            None => OrderedNode::with_sink(host, orderer, recovery),
        };
        // the observation barrier: every drain batch ends AT a block that
        // moves the valset root, so the orchestration step below observes a
        // membership change at exactly its block's view — the same view on
        // every validator, whatever the local batch shape. without it the
        // armed cutover view (and with it the next epoch's height base)
        // would depend on drain timing: a cross-node fork.
        node.watch_module("valset");

        // the valset ORCHESTRATOR: watches finalized valset module state and
        // schedules deterministic epoch cutovers. it resumes at the recovered
        // epoch coordinates over the epoch's ENGINE PARTICIPANT SET, and
        // re-arms a cutover the pre-crash process had scheduled.
        let mut orchestrator = consensus::ValsetOrchestrator::resume(
            CUTOVER_DELAY,
            member_keys.clone(),
            resume_epoch,
            view_base,
            pending_boot,
        );
        if let Some(ceiling) = pending_boot {
            node.set_view_ceiling(ceiling);
            println!(
                "[node {label}] re-armed pending cutover at view {ceiling} (epoch {})",
                resume_epoch + 1
            );
        }

        // the genesis app-hash BEFORE any op — the demo asserts this agrees across
        // processes (a fork here would be a genesis-determinism bug, not consensus).
        // a RESTORED boot prints its recovered line above instead.
        if resumed.is_none() {
            let genesis_hash = node.app_hash();
            println!("[node {label}] genesis app_hash={}", hex(&genesis_hash));
        }

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
        // dev shape only — a REAL network's genesis carries no demo scaffolding
        // (and a restored boot must not re-frame it: seq 0 was already spent).
        if dev_demo && resumed.is_none() {
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
        // keys reach the console); `next_seq` was set at boot — 1 on a fresh
        // genesis (after the demo op's 0), or past every recovered frame.

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
        // recovery cadence: sealed blocks since the last checkpoint manifest.
        let mut blocks_since_checkpoint: u64 = 0;
        // throttle for the pending-cutover nop pusher below.
        let mut last_nop = std::time::Instant::now();
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
                            // best-effort final checkpoint + journal barrier so
                            // the restart replays a minimal suffix; a failure
                            // here is just the crash path, which also recovers.
                            if let Some(f) = node.finalized() {
                                let pos = node.sink_mut().oplog_pos().await;
                                if let Ok(m) = Manifest::capture(
                                    node.host(),
                                    Some(f.height),
                                    orchestrator.epoch(),
                                    orchestrator.epoch_base(),
                                    participant_bytes(&orchestrator),
                                    orchestrator.pending_cutover().map(|c| c.cutover_view()),
                                    pos,
                                    next_seq,
                                ) {
                                    let _ = node.sink_mut().write_manifest(&m).await;
                                }
                            }
                            let _ = node.sink_mut().sync().await;
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
                    // the boundary's consensus coordinates ride the manifest.
                    // the floor certificate is served only when it certifies
                    // exactly the current boundary — a cert behind the
                    // boundary would make a joiner skip history it needs.
                    let coords = statesync::BoundaryCoords {
                        epoch: orchestrator.epoch(),
                        view_base: orchestrator.epoch_base(),
                        participants: participant_bytes(&orchestrator),
                        floor_cert: latest_floor
                            .as_ref()
                            .filter(|fc| fc.epoch == orchestrator.epoch())
                            .filter(|fc| {
                                node.finalized().is_some_and(|f| f.height == fc.height)
                            })
                            .map(|fc| fc.cert.clone()),
                    };
                    let resp = sync_server
                        .handle_frame(node.host(), node.finalized(), &coords, body)
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
                    let drained = node.take_drained();
                    // sealed = journaled: applied and rejected frames both got
                    // recovery seals; discarded frames were never journaled.
                    blocks_since_checkpoint += drained
                        .iter()
                        .filter(|d| d.disposition != node::Disposition::Discarded)
                        .count() as u64;
                    for d in drained {
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

                    // persist the finalization floor once everything at or
                    // below it has drained. read the certificate FIRST, the
                    // gate second: releases happen only on this thread, so a
                    // zero gate proves the cert's view is fully applied — a
                    // floor ahead of app state would suppress replay of
                    // finalized ops a restart still needs.
                    if let Some((view, cert)) = node.orderer().latest_finalization() {
                        if view != 0 && node.orderer().unreleased_len() == 0 {
                            let height = orchestrator.app_height(view);
                            if last_cert_height.is_none_or(|h| height > h) {
                                let fc = recovery::FloorCert {
                                    epoch: orchestrator.epoch(),
                                    height,
                                    cert,
                                };
                                match node.sink_mut().write_floor_cert(&fc).await {
                                    Ok(()) => {
                                        last_cert_height = Some(height);
                                        latest_floor = Some(fc);
                                    }
                                    Err(e) => eprintln!(
                                        "[node {label}] floor cert write failed (will retry): {e}"
                                    ),
                                }
                            }
                        }
                    }

                    // periodic checkpoint: snapshot the in-memory cohort and
                    // prune the op journal below the PREVIOUS checkpoint once
                    // the persisted floor has passed it (pruned frames must
                    // never be needed to resolve a re-reported finalization).
                    if blocks_since_checkpoint >= checkpoint_blocks {
                        if let Some(f) = node.finalized() {
                            let pos = node.sink_mut().oplog_pos().await;
                            let captured = Manifest::capture(
                                node.host(),
                                Some(f.height),
                                orchestrator.epoch(),
                                orchestrator.epoch_base(),
                                participant_bytes(&orchestrator),
                                orchestrator.pending_cutover().map(|c| c.cutover_view()),
                                pos,
                                next_seq,
                            );
                            match captured {
                                Ok(m) => match node.sink_mut().write_manifest(&m).await {
                                    Ok(()) => {
                                        blocks_since_checkpoint = 0;
                                        let floor_passed = matches!(
                                            node.sink_mut().floor_cert(),
                                            Ok(Some(fc))
                                                if prev_ckpt.0.is_none_or(|h| fc.height >= h)
                                        );
                                        if floor_passed {
                                            if let Err(e) =
                                                node.sink_mut().prune_oplog(prev_ckpt.1).await
                                            {
                                                eprintln!(
                                                    "[node {label}] oplog prune failed: {e}"
                                                );
                                            }
                                        }
                                        prev_ckpt = (m.height, pos);
                                    }
                                    Err(e) => eprintln!(
                                        "[node {label}] checkpoint write failed (will retry): {e}"
                                    ),
                                },
                                Err(e) => eprintln!(
                                    "[node {label}] checkpoint capture failed (will retry): {e}"
                                ),
                            }
                        }
                    }

                    // the VALSET ORCHESTRATION step: observe the finalized
                    // membership projection; a change schedules a deterministic
                    // cutover (arming the discard ceiling), and crossing the
                    // cutover view tears the engine down and respawns it over
                    // the set read AT the boundary. the observation barrier
                    // guarantees this tick's last view IS the changing block's
                    // view when membership moved.
                    if let Some(engine_view) = node.last_engine_view() {
                        let members_raw = read_valset_members(node.host()).await;
                        let mut observed: Vec<ed25519::PublicKey> = Vec::new();
                        for key in &members_raw {
                            if let Ok(pk) = ed25519::PublicKey::decode(key.as_slice()) {
                                observed.push(pk);
                            }
                        }
                        if let consensus::ObservationOutcome::Scheduled(cutover) =
                            orchestrator.observe_members(engine_view, observed.iter().cloned())
                        {
                            println!(
                                "[node {label}] membership change observed at view {} — cutover to epoch {} at view {}",
                                cutover.observed_view(),
                                cutover.next_epoch(),
                                cutover.cutover_view()
                            );
                            node.set_view_ceiling(cutover.cutover_view());
                        }
                        if let Some(plan) = orchestrator.respawn_if_due(engine_view, observed) {
                            let members = plan.valset().consensus_members();
                            let member_bytes: Vec<Vec<u8>> =
                                members.iter().map(|k| k.as_ref().to_vec()).collect();
                            // transport FIRST: the new epoch's mesh must admit
                            // its members (a fresh joiner above all) before
                            // anything is expected of them. index = epoch,
                            // strictly increasing across cutovers.
                            mesh_oracle.track(plan.epoch(), mesh_at(members));
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
                            // a fresh epoch: new store (pins of the torn-down
                            // epoch die with it), genesis floor.
                            let orderer = spawn_epoch(
                                &mut channel_bank,
                                plan.epoch(),
                                participants,
                                ContentStore::new(),
                                None,
                            );
                            if let Err(e) = node
                                .cutover(
                                    orderer,
                                    plan.epoch(),
                                    plan.cutover_app_height(),
                                    &member_bytes,
                                )
                                .await
                            {
                                eprintln!("[node {label}] FATAL: {e} — halting");
                                std::process::exit(1);
                            }
                            // checkpoint IMMEDIATELY: the manifest must record
                            // the new epoch's participant set (the journal's
                            // cutover record alone covers only the crash
                            // window until this write lands).
                            let pos = node.sink_mut().oplog_pos().await;
                            let captured = Manifest::capture(
                                node.host(),
                                node.finalized().map(|f| f.height),
                                orchestrator.epoch(),
                                orchestrator.epoch_base(),
                                participant_bytes(&orchestrator),
                                None,
                                pos,
                                next_seq,
                            );
                            match captured {
                                Ok(m) => match node.sink_mut().write_manifest(&m).await {
                                    Ok(()) => {
                                        blocks_since_checkpoint = 0;
                                        prev_ckpt = (m.height, pos);
                                    }
                                    Err(e) => eprintln!(
                                        "[node {label}] post-cutover checkpoint write failed \
                                         (the journal's cutover record covers a restart): {e}"
                                    ),
                                },
                                Err(e) => eprintln!(
                                    "[node {label}] post-cutover checkpoint capture failed \
                                     (the journal's cutover record covers a restart): {e}"
                                ),
                            }
                            println!(
                                "[node {label}] cutover complete: epoch {} with {} validators (app height base {})",
                                plan.epoch(),
                                members.len(),
                                plan.cutover_app_height()
                            );
                        }
                    }

                    // a pending cutover only crosses when finalized views
                    // REACH it, and views only advance with ops — an idle
                    // network would park at the armed boundary forever. push
                    // a deterministically-rejected nop (unknown module
                    // target: rejects identically on every node, leaves no
                    // state) until the boundary is crossed.
                    if orchestrator.pending_cutover().is_some()
                        && last_nop.elapsed() >= Duration::from_secs(1)
                    {
                        last_nop = std::time::Instant::now();
                        let seq = next_seq;
                        next_seq += 1;
                        if let Err(e) = node
                            .submit(
                                &signer,
                                seq,
                                Msg { target: NOP_TARGET.into(), payload: Vec::new() },
                            )
                            .await
                        {
                            eprintln!("[node {label}] cutover nop submit failed: {e}");
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
