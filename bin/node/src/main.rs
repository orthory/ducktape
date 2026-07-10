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
use commonware_p2p::{Ingress, Manager, Receiver as P2pReceiver, Recipients, Sender as P2pSender};
use commonware_runtime::{Clock, IoBuf, Metrics, Quota, Runner, Spawner, Supervisor};
use commonware_utils::{NZU32, ordered::Set};
use futures::{FutureExt as _, StreamExt as _};
use tracing_subscriber::prelude::*;

use consensus::{ConsensusScheme, ContentStore, SimplexOrderer};

mod blob_fetch;
mod cli;
mod cli_flags;
mod config;
mod constants;
mod explorer;
mod first_contact_join;
mod host_reads;
mod host_state;
#[cfg(test)]
mod joiner_mesh_tests;
mod lobby;
#[cfg(test)]
mod main_tests;
mod oracle_pool;
mod relay;
mod relay_runtime;
mod replica;
mod resident_announce;
mod resident_dispatch;
mod resource_limits;
mod statesync_plane;
mod sync;
mod userkey;
mod userkey_cli;
mod util;
mod voice;
mod voice_plane;
use config::{Resolved, WireGuardEffectKind, hex_bytes, unhex};
use constants::*;
use explorer::{
    IndexFold, boundary_block_row, explorer_root_op, heal_index, sealed_frame_block_row,
    ship_index_blobs, stage_shipped_index,
};
use host_reads::{
    joiner_epoch_mesh, read_members_from_host, read_redemptions_from_host, read_upgrade_state,
    read_upgrade_status_raw, read_upgrade_version_fields, read_valset_members,
    read_valset_residents, resume_member_keys, resume_resident_keys,
};
use host_state::{
    NetworkBindings, SyncSubstrates, genesis_host, restore_host, run_output_sink, sync_all_modules,
};
use sync::catchup::{
    advance_next_seq_from_frames, apply_post_reboot_catchup_frames, apply_verified_suffix_frame,
    catch_up_post_reboot_frames, derive_pending_boot, write_post_reboot_catchup_checkpoint,
    BootP2pSyncClient, PostRebootCatchupError,
};
use sync::serve::{
    ServedSeal, SyncBoundary, SyncStateRequest, assert_floor_binds_view, drive_sync_request,
    reopen_preflight_synced_host, reopen_recovery, replica_backfill, replica_orchestrator_at,
    replica_verifier, verify_manifest_floor, write_boundary_checkpoint,
};
use util::{diag_log, epoch_floor, hex, participant_bytes, resident_bytes, unix_ms};

use directory::{DirMsg, DirQuery, DirReply, decode_reply, encode_msg, encode_query};
use duckfs_disk::SyncScratch;
use host::Host;
use node::OrderedNode;
use recovery::{Manifest, Recovery};
use sdk::{Msg, StateRoot};
use statesync::p2p::P2pSyncClient;
use statesync::{SyncServer, fetch_manifest, fetch_tip_coords};

/// the node-local worker that self-emits a validator-origin `SignalReady` op
/// ONCE per pending upgrade this binary can execute. deliberately NOT a
/// `reactor::Worker`: readiness must survive restart/late-join, so it polls the
/// COMMITTED upgrade state each pump tick and re-derives its decision idempotently
/// rather than reacting to a one-shot block effect. "ready" is a truthful machine
/// statement about the running binary — it signals iff `MAX_PROTOCOL_VERSION >=
/// to_version` (never a version it cannot execute).
struct ReadinessSignaller {
    /// the highest protocol version this binary can execute (`MAX_PROTOCOL_VERSION`).
    max_version: u32,
    /// this node's own validator pubkey bytes — the readiness identity.
    me: Vec<u8>,
    /// the `(name, to_version)` we have already emitted a signal for, latched so a
    /// signal in flight (not yet committed into the module's `ready` set, several
    /// ticks out) is not re-emitted every pump tick (risk R10 — local dedupe atop
    /// module idempotence).
    signaled: Option<(String, u32)>,
}

impl ReadinessSignaller {
    fn new(max_version: u32, me: Vec<u8>) -> Self {
        Self {
            max_version,
            me,
            signaled: None,
        }
    }

    /// the PURE decision core: given the committed status, decide whether to emit a
    /// `SignalReady` and latch it. returns the `(name, to_version)` to signal, or
    /// `None`. truthful (binary can execute `to_version`), member-gated (self is a
    /// current boundary member), and idempotent (module already holds our signal, or
    /// one is already in flight).
    fn decide(&mut self, status: &upgrade::UpgradeStatus) -> Option<(String, u32)> {
        let pending = status.pending.as_ref()?;
        // never lie: a binary that cannot execute the target version stays silent so
        // the boundary cleanly aborts rather than arming onto an under-versioned node.
        if pending.to_version > self.max_version {
            return None;
        }
        // only a CURRENT boundary member is in the readiness denominator (R = n).
        if !status.members.iter().any(|m| m == &self.me) {
            return None;
        }
        // the module already recorded our (committed) signal — nothing to do.
        if status.ready.iter().any(|k| k == &self.me) {
            return None;
        }
        // a signal for this exact upgrade is already in flight (submitted, awaiting
        // finalization) — do not re-submit every tick.
        if self.signaled.as_ref() == Some(&(pending.name.clone(), pending.to_version)) {
            return None;
        }
        self.signaled = Some((pending.name.clone(), pending.to_version));
        Some((pending.name.clone(), pending.to_version))
    }

    /// query committed upgrade state and, when a signal is due, build the
    /// validator-origin `SignalReady` op. gracefully `None` when the module is
    /// absent (pre-retrofit) or the reply is unreadable — no panic on a baseline net.
    async fn maybe_signal(&mut self, host: &Host) -> Option<(Msg, String, u32)> {
        use upgrade::{
            UpgradeMsg, UpgradeQuery, UpgradeReply, decode_reply, encode_msg, encode_query,
        };
        let reply = host
            .query("upgrade", &encode_query(&UpgradeQuery::Status))
            .await
            .ok()?;
        let UpgradeReply::Status(status) = decode_reply(&reply).ok()?;
        let (name, to_version) = self.decide(&status)?;
        let msg = Msg {
            target: "upgrade".into(),
            payload: encode_msg(&UpgradeMsg::SignalReady {
                name: name.clone(),
                to_version,
                commitment: None,
            }),
        };
        Some((msg, name, to_version))
    }
}

/// the capability self-announcer: the state-driven twin of
/// [`ReadinessSignaller`] for the capability registry. it polls the committed
/// registry each pump tick and, when this node's announced set differs from
/// what discovery found locally, self-submits ONE declarative
/// [`CapabilityMsg::Announce`]. state-driven (survives restart/late-join) and
/// idempotent: once the committed set matches, it stays quiet. a node with no
/// providers announces nothing.
struct CapabilityAnnouncer {
    /// this node's own validator pubkey bytes — the registry identity.
    me: Vec<u8>,
    /// the capability tags discovery found on this host, sorted — the truthful
    /// set to announce. empty means this node provides nothing.
    capabilities: Vec<String>,
    /// the set we last SUBMITTED (not yet observed committed), latched so an
    /// in-flight announce is not re-sent every tick.
    announced: Option<Vec<String>>,
}

impl CapabilityAnnouncer {
    fn new(me: Vec<u8>, capabilities: Vec<String>) -> Self {
        Self {
            me,
            capabilities,
            announced: None,
        }
    }

    /// the PURE decision core: given this node's committed announced set,
    /// decide whether to (re)announce. `None` when the registry already matches
    /// what we'd announce, or an identical announce is already in flight.
    fn decide(&mut self, committed: &[String]) -> Option<Vec<String>> {
        // nothing to provide and nothing recorded: stay silent (genesis state).
        if self.capabilities.is_empty() && committed.is_empty() {
            return None;
        }
        // the registry already reflects our providers — nothing to do.
        if committed == self.capabilities.as_slice() {
            self.announced = None;
            return None;
        }
        // an announce for this exact set is already in flight.
        if self.announced.as_deref() == Some(self.capabilities.as_slice()) {
            return None;
        }
        self.announced = Some(self.capabilities.clone());
        Some(self.capabilities.clone())
    }

    /// query this node's committed capability set and, when an announce is due,
    /// build the external-origin `Announce` op. gracefully `None` when the
    /// module is absent (pre-retrofit net) or the reply is unreadable.
    async fn maybe_announce(&mut self, host: &Host) -> Option<Msg> {
        use capability::{
            CapabilityMsg, CapabilityQuery, CapabilityReply, decode_reply, encode_msg, encode_query,
        };
        let reply = host
            .query(
                "capability",
                &encode_query(&CapabilityQuery::Node {
                    node: self.me.clone(),
                }),
            )
            .await
            .ok()?;
        let CapabilityReply::Node(committed) = decode_reply(&reply).ok()? else {
            return None;
        };
        let capabilities = self.decide(&committed)?;
        Some(Msg {
            target: "capability".into(),
            payload: encode_msg(&CapabilityMsg::Announce { capabilities }),
        })
    }
}

/// the committed dispatch mailbox's undelivered-result count — the nudge
/// pump's read. `0` when the module is absent or the mailbox is empty.
async fn dispatch_pending_deliveries(host: &Host) -> u64 {
    use dispatch::{DispatchQuery, DispatchReply, decode_reply, encode_query};
    let Ok(reply) = host
        .query("dispatch", &encode_query(&DispatchQuery::PendingDeliveries))
        .await
    else {
        return 0;
    };
    match decode_reply(&reply) {
        Ok(DispatchReply::PendingDeliveries(n)) => n,
        _ => 0,
    }
}

/// the committed saga ledger's earliest pending lease-expiry/deadline — the
/// crank pump's read. `None` when the module is absent or nothing pending
/// carries one.
async fn saga_next_expiry(host: &Host) -> Option<u64> {
    use saga::{SagaQuery, SagaReply, decode_reply, encode_query};
    let reply = host
        .query("saga", &encode_query(&SagaQuery::NextExpiry))
        .await
        .ok()?;
    match decode_reply(&reply).ok()? {
        SagaReply::NextExpiry(v) => v,
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PromotionBoundarySource {
    Latest,
}

impl PromotionBoundarySource {
    fn as_str(self) -> &'static str {
        match self {
            PromotionBoundarySource::Latest => "latest",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PromotionBoundary<'a> {
    Promote {
        boundary: &'a statesync::Manifest,
        source: PromotionBoundarySource,
    },
    Retry,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ManifestFetchRetry {
    log_line: String,
    announce: bool,
}

fn joiner_manifest_fetch_retry(
    label: &str,
    resident_standing: bool,
    error: impl std::fmt::Display,
) -> ManifestFetchRetry {
    if resident_standing {
        return ManifestFetchRetry {
            log_line: format!("[node {label}] resident: boundary fetch retrying ({error})"),
            announce: false,
        };
    }
    ManifestFetchRetry {
        log_line: format!(
            "[node {label}] joining: redemption not landed yet (or the mesh is unreachable) — \
             the announce keeps retrying and a member node redeems it automatically. retrying \
             ({error})"
        ),
        announce: true,
    }
}

fn latest_boundary_has_floor(latest: &statesync::Manifest) -> bool {
    latest.height <= latest.view_base || latest.floor_cert.is_some()
}

fn choose_promotion_boundary<'a>(
    synced_host_hash: StateRoot,
    latest: &'a statesync::Manifest,
    self_public_key: &[u8],
) -> PromotionBoundary<'a> {
    if !latest.participants.iter().any(|key| key == self_public_key) {
        return PromotionBoundary::Retry;
    }
    if latest.app_hash == synced_host_hash {
        return if latest_boundary_has_floor(latest) {
            PromotionBoundary::Promote {
                boundary: latest,
                source: PromotionBoundarySource::Latest,
            }
        } else {
            PromotionBoundary::Retry
        };
    }
    PromotionBoundary::Retry
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
    /// the verified join requests parked joiners announced to THIS member —
    /// the queue the approve button (or `invite-accept`) settles.
    JoinRequests,
    /// graceful stop: replies ok, then exits 0 after the current pump turn.
    Shutdown,
}

/// one verified, unapproved join announce (node-local, in-memory; the parked
/// joiner re-announces every few seconds, so nothing here is durable state).
struct JoinRequestRecord {
    issuer: Vec<u8>,
    first_seen_ms: u64,
    last_seen_ms: u64,
}

/// the rpc/console projection of one [`JoinRequestRecord`].
#[derive(serde::Serialize)]
struct JoinRequestView {
    /// the key asking to join, hex.
    joiner: String,
    /// the member whose invite token authorized the announce, hex.
    issuer: String,
    first_seen_ms: u64,
    last_seen_ms: u64,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    join_requests: Option<Vec<JoinRequestView>>,
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
            join_requests: None,
        }
    }
    fn err(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            error: Some(msg.into()),
            reply_hex: None,
            status: None,
            join_requests: None,
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

fn main() {
    resource_limits::raise_open_file_limit();
    // Convert any terminal error into the same stable `FATAL:` marker the node
    // already prints for its other fatal paths (recovery, admission, promotion),
    // plus a non-zero exit. This closes the run-path boot failures (bind
    // conflict, config parse) that used to propagate as a bare `Error: …` the
    // desktop app's classify() didn't recognize — now the app surfaces the
    // reason immediately instead of inferring death. (Onboarding subcommands
    // still surface their own stderr via run_verb; the prefix is harmless there.)
    if let Err(err) = run() {
        eprintln!("FATAL: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(command) = args.first()
        && let Some(result) = cli::dispatch(command, &args[1..])
    {
        return result;
    }

    // the run path: `--config <path> | -n/--network <chain id> [--sync-only]`.
    let mut cfg_path: Option<PathBuf> = None;
    let mut network: Option<String> = None;
    let mut sync_only = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--config" => cfg_path = it.next().map(PathBuf::from),
            "-n" | "--network" => network = it.next().cloned(),
            "--sync-only" => sync_only = true,
            other => {
                return Err(format!(
                    "unexpected arg {other:?} (want a subcommand — \
                     keygen|user-key|user-sign-bind|user-sign-unbind|\
                     user-sign-possession|user-sign-add-member|user-sign-remove-member|\
                     user-webauthn-challenge|user-p256-payload|\
                     init|invite|admit|\
                     invite-accept|promote|resident-remove|\
                     join-requests|member-remove|member-leave|member-status|join|\
                     upgrade-status — or \
                     --config <path> | -n/--network <chain id> [--sync-only])"
                )
                .into());
            }
        }
    }
    // `--network` addresses a workspace by its chain id through the registry;
    // `--config` stays the explicit path. exactly one selects the node.
    let cfg_path = match (network, cfg_path) {
        (Some(needle), None) => config::find_workspace_config(&needle)?,
        (None, Some(path)) => path,
        (Some(_), Some(_)) => {
            return Err("pass either --network <chain id> or --config <path>, not both".into());
        }
        (None, None) => {
            return Err("missing --config <path> (or -n/--network <chain id>)".into());
        }
    };

    let log_ring = noded::LogRing::default();
    init_tracing(log_ring.clone());

    run_node(config::resolve(&cfg_path)?, sync_only, log_ring)
}

fn init_tracing(log_ring: noded::LogRing) {
    // opt-in internals visibility: RUST_LOG=commonware_p2p=debug etc.
    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(tracing_subscriber::EnvFilter::from_default_env());
    // the stream's `logs` topic: info floor by default so hot-path debug/trace
    // events never pay per-event formatting into the ring; RUST_LOG overrides.
    let ring_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_writer(log_ring)
        .with_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        );
    let _ = tracing_subscriber::registry()
        .with(stderr_layer)
        .with(ring_layer)
        .try_init();
}

/// the reachability plane's thread body: derive the plane's endpoints, bind
/// the nat client against the coordinated-reach coordinators, and drive
/// `reachability::run` with the configured WireGuard effect — real (an
/// actual interface via the userspace WireGuard runtime) by default,
/// in-memory fake when `wireguard_effect = "fake"` opts out. every failure
/// path prints and returns — the plane is an overlay on a working node,
/// never a reason to take the node down.
/// Wire the staged WireGuard reachability plane onto an already-registered
/// mesh channel: the orchestrator runs on its own plain-tokio OS thread (the
/// app-surface split exactly), and two pump tasks bridge it — mesh datagrams
/// in as `Deliver` commands, `Send` events out as mesh datagrams, everything
/// else printed as operator-visible progress. Returns the plane's command
/// sender. Shared by the validator path and the parked standby path (which
/// pre-warms its tunnels ahead of activation); the callers differ only in
/// where their `Retarget`/`ViewTick` commands come from.
#[allow(clippy::too_many_arguments)]
fn wire_reachability_plane<S, R>(
    context: &commonware_runtime::tokio::Context,
    label: &str,
    chain_id: &str,
    signer: &ed25519::PrivateKey,
    wireguard_key_file: &std::path::Path,
    mesh_state_file: &std::path::Path,
    wireguard_listen: std::net::SocketAddr,
    wireguard_effect: WireGuardEffectKind,
    overlay_slot: overlay_net::userspace::StackSlot,
    advertised: Ingress,
    coordinators: Vec<Ingress>,
    intro_listen: Option<std::net::SocketAddr>,
    // the genesis-issued admission capability presented on every coordinator
    // request (private coordination); `None` for a genesis validator, a public
    // coordinator, or the dev shape.
    coord_cap: Option<nat_traversal::CoordCap>,
    reach_p2p_tx: S,
    mut reach_p2p_rx: R,
) -> tokio::sync::mpsc::Sender<reachability::ReachabilityCommand>
where
    S: P2pSender<PublicKey = ed25519::PublicKey> + Send + Sync + 'static,
    R: P2pReceiver<PublicKey = ed25519::PublicKey> + Send + 'static,
{
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel::<reachability::ReachabilityCommand>(256);
    let (ev_tx, mut ev_rx) = tokio::sync::mpsc::channel::<reachability::ReachabilityEvent>(256);

    let thread_label = label.to_string();
    let reach_signer = signer.clone();
    let reach_coord_cap = coord_cap;
    let plane_chain_id = chain_id.to_string();
    let key_file = wireguard_key_file.to_path_buf();
    let state_file = mesh_state_file.to_path_buf();
    let nudge_tx = cmd_tx.clone();
    std::thread::Builder::new()
        .name("reachability".into())
        .spawn(move || {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("reachability tokio runtime")
                .block_on(reachability_plane(
                    thread_label,
                    plane_chain_id,
                    reach_signer,
                    key_file,
                    state_file,
                    wireguard_listen,
                    wireguard_effect,
                    overlay_slot,
                    advertised,
                    coordinators,
                    intro_listen,
                    reach_coord_cap,
                    cmd_rx,
                    nudge_tx,
                    ev_tx,
                ));
        })
        .expect("spawn reachability thread");

    // pump in: mesh datagrams -> orchestrator commands.
    {
        let cmd = cmd_tx.clone();
        context
            .child("reachability_in")
            .spawn(move |_ctx| async move {
                while let Ok((peer, msg)) = reach_p2p_rx.recv().await {
                    let bytes: Vec<u8> = msg.into();
                    let deliver = reachability::ReachabilityCommand::Deliver { from: peer, bytes };
                    if cmd.send(deliver).await.is_err() {
                        break;
                    }
                }
            });
    }
    // pump out: orchestrator sends -> mesh; everything else is
    // operator-visible progress.
    {
        let pump_label = label.to_string();
        let mut tx = reach_p2p_tx;
        context.child("reachability_out").spawn(move |_ctx| async move {
            while let Some(event) = ev_rx.recv().await {
                match event {
                    reachability::ReachabilityEvent::Send { to, bytes } => {
                        let _ = tx.send(Recipients::One(to), IoBuf::from(bytes), false);
                    }
                    reachability::ReachabilityEvent::MeshReady { epoch, .. } => {
                        println!(
                            "[node {pump_label}] reachability: epoch {epoch} mesh verified"
                        )
                    }
                    reachability::ReachabilityEvent::TunnelsApplied {
                        epoch,
                        interface,
                        peers,
                    } => match wireguard_effect {
                        WireGuardEffectKind::Tun => println!(
                            "[node {pump_label}] reachability: epoch {epoch} tunnels applied \
                             on {interface} ({peers} peer(s))"
                        ),
                        // socket mode has no OS interface: {interface} is the
                        // orchestrator's label for the in-process backend.
                        WireGuardEffectKind::Socket => println!(
                            "[node {pump_label}] reachability: epoch {epoch} tunnels applied \
                             on {interface} ({peers} peer(s); userspace socket backend)"
                        ),
                        WireGuardEffectKind::Fake => println!(
                            "[node {pump_label}] reachability: epoch {epoch} tunnel config \
                             staged on {interface} ({peers} peer(s); fake effect — no real \
                             interface)"
                        ),
                    },
                    reachability::ReachabilityEvent::StandbyTunnelsApplied {
                        epoch,
                        interface,
                        peers,
                    } => println!(
                        "[node {pump_label}] reachability: epoch {epoch} standby pre-warm \
                         tunnels on {interface} ({peers} peer(s))"
                    ),
                    reachability::ReachabilityEvent::InvitePeerInstalled { peer, interface } => {
                        println!(
                            "[node {pump_label}] reachability: invite tunnel to {} on {interface}",
                            hex_bytes(&peer.as_ref()[..4])
                        )
                    }
                    reachability::ReachabilityEvent::PeerFailed { peer, reason } => {
                        println!(
                            "[node {pump_label}] reachability: peer {}: {reason}",
                            hex_bytes(&peer.as_ref()[..4])
                        )
                    }
                    reachability::ReachabilityEvent::EpochFailed { epoch, reason } => println!(
                        "[node {pump_label}] reachability: epoch {epoch} failed: {reason}"
                    ),
                    reachability::ReachabilityEvent::MeshRestored {
                        epoch,
                        interface,
                        peers,
                    } => println!(
                        "[node {pump_label}] reachability: persisted mesh (epoch {epoch}) \
                         restored on {interface} ({peers} peer(s)) — awaiting live assembly"
                    ),
                    reachability::ReachabilityEvent::RestoreFailed { reason } => {
                        println!(
                            "[node {pump_label}] reachability: persisted mesh not restored \
                             ({reason}); continuing on live assembly only"
                        )
                    }
                    reachability::ReachabilityEvent::PersistFailed { reason } => {
                        println!(
                            "[node {pump_label}] reachability: WARNING: mesh state not \
                             persisted ({reason}) — a cold restart will not restore this epoch"
                        )
                    }
                }
            }
        });
    }
    cmd_tx
}

#[allow(clippy::too_many_arguments)]
async fn reachability_plane(
    label: String,
    chain_id: String,
    signer: ed25519::PrivateKey,
    wireguard_key_file: PathBuf,
    // where the plane persists each applied epoch's verified mesh and
    // re-applies it from at boot (the cold-restart path).
    mesh_state_file: PathBuf,
    wireguard_listen: std::net::SocketAddr,
    effect_kind: WireGuardEffectKind,
    // the seam's stack handle (socket mode): created by the node so the mesh
    // context and the data-plane factory hold it BEFORE this thread exists;
    // the socket-mode effect publishes/clears the live stack through it.
    overlay_slot: overlay_net::userspace::StackSlot,
    advertised: Ingress,
    coordinators: Vec<Ingress>,
    // the invite intro listener: where a fresh joiner announces its keys
    // (token-authenticated) so its tunnel exists before any p2p.
    intro_listen: Option<std::net::SocketAddr>,
    // the genesis-issued admission capability presented on every coordinator
    // request (private coordination); `None` for a genesis validator, a public
    // coordinator, or the dev shape.
    coord_cap: Option<nat_traversal::CoordCap>,
    commands: tokio::sync::mpsc::Receiver<reachability::ReachabilityCommand>,
    // a clone of the `commands` sender, for the plane's own nudge ticker.
    nudges: tokio::sync::mpsc::Sender<reachability::ReachabilityCommand>,
    events: tokio::sync::mpsc::Sender<reachability::ReachabilityEvent>,
) {
    use std::net::ToSocketAddrs as _;
    let policy = reachability::open_port_policy();
    // the plane's records carry IP literals only (the endpoint parser
    // rejects DNS); a hostname ingress resolves ONCE at plane start.
    let resolve_ingress = |ingress: &Ingress| match ingress {
        Ingress::Socket(addr) => Some(*addr),
        Ingress::Dns { host, port } => (host.as_str(), *port)
            .to_socket_addrs()
            .ok()
            .and_then(|mut addrs| addrs.next()),
    };
    let Some(control_addr) = resolve_ingress(&advertised) else {
        eprintln!(
            "[node {label}] reachability: advertised {advertised:?} did not resolve — plane \
             not started"
        );
        return;
    };
    let control_endpoint = match wireguard_upgrade::Endpoint::new(
        control_addr.ip(),
        control_addr.port(),
        wireguard_upgrade::Transport::Tcp,
        &policy,
    ) {
        Ok(endpoint) => endpoint,
        Err(err) => {
            eprintln!(
                "[node {label}] reachability: advertised control endpoint rejected ({err:?}) — \
                 set `advertised` to a dialable address; plane not started"
            );
            return;
        }
    };
    // an UNSPECIFIED wireguard_listen address (0.0.0.0/[::], cmd_join's
    // NAT'd-joiner default) means "bind the port, advertise NO endpoint":
    // the plane runs endpoint-less — peers install this node's tunnel
    // without an endpoint and this node's own initiations complete it
    // (WireGuard roams to the authenticated source). A concrete address
    // advertises exactly as before.
    if wireguard_listen.port() == 0 {
        eprintln!(
            "[node {label}] reachability: wireguard_listen needs a concrete UDP port — plane \
             not started"
        );
        return;
    }
    let wireguard_advertised = if wireguard_listen.ip().is_unspecified() {
        None
    } else {
        match wireguard_upgrade::Endpoint::new(
            wireguard_listen.ip(),
            wireguard_listen.port(),
            wireguard_upgrade::Transport::Udp,
            &policy,
        ) {
            Ok(endpoint) => Some(endpoint),
            Err(err) => {
                eprintln!(
                    "[node {label}] reachability: wireguard_listen rejected ({err:?}) — plane \
                     not started"
                );
                return;
            }
        }
    };
    let mut coords: Vec<std::net::SocketAddr> = Vec::new();
    for ingress in &coordinators {
        match resolve_ingress(ingress) {
            Some(addr) if !coords.contains(&addr) => coords.push(addr),
            Some(_) => {}
            None => eprintln!(
                "[node {label}] reachability: coordinator {ingress:?} did not resolve — skipped"
            ),
        }
    }
    let me = reachability::node_key(reachability::identity_of(&signer.public_key()));
    // socket mode owns the underlay socket from PLANE START, not first
    // apply: the NAT client below rides it (reflexive discovery,
    // registration, keepalives, and the punch all originate from the
    // tunnel's own 5-tuple — the pinhole a punch opens is only good for the
    // socket it originated from), and it survives interface rebuilds so the
    // coordinator mapping stays warm while a tunnel is torn down/re-applied.
    let socket_underlay = match effect_kind {
        WireGuardEffectKind::Socket => {
            match overlay_net::userspace::UnderlaySocket::bind(
                &tokio::runtime::Handle::current(),
                wireguard_listen.port(),
            ) {
                Ok(underlay) => Some(underlay),
                Err(err) => {
                    eprintln!(
                        "[node {label}] reachability: underlay udp/{} bind failed: {err} — \
                         plane not started",
                        wireguard_listen.port()
                    );
                    return;
                }
            }
        }
        WireGuardEffectKind::Tun | WireGuardEffectKind::Fake => None,
    };
    let (invite_intro_tx, mut invite_intro_rx) =
        if socket_underlay.is_some() && intro_listen.is_some() {
            let (tx, rx) = tokio::sync::mpsc::channel(32);
            (Some(tx), Some(rx))
        } else {
            (None, None)
        };
    // authenticate every coordinator request: the node signs a
    // proof-of-possession with its identity key and, in private coordination,
    // carries the genesis-issued cap. A fully-open coordinator ignores the
    // authenticator; a public/private one requires it. With no coordinators
    // configured `bind` short-circuits to pass-through and never touches this.
    let resolver = match &socket_underlay {
        Some(underlay) if !coords.is_empty() => {
            let bypass = underlay
                .take_bypass()
                .expect("a fresh underlay socket still holds its bypass lane");
            let client =
                nat_traversal::NatSocket::shared(underlay.sender(), bypass).and_then(|sock| {
                    nat_traversal::NatClient::with_socket(
                        sock,
                        me,
                        coords.clone(),
                        Some(signer.clone()),
                        coord_cap,
                    )
                });
            let resolver = match client {
                Ok(client) => {
                    reachability::NatResolver::from_client_with_datagram_sink(
                        client,
                        reachability::RENDEZVOUS_KEEPALIVE,
                        invite_intro_tx,
                    )
                    .await
                }
                Err(err) => Err(err),
            };
            match resolver {
                Ok(resolver) => resolver,
                // An unreachable coordinator must NOT take down the whole plane.
                // The WireGuard underlay is already bound, and DIRECT / front
                // candidates (InstallInvitePeer + this node's own initiations)
                // need no rendezvous at all. Degrade to the pass-through
                // resolver so those paths still come up; only COORDINATED
                // (by-identity) candidates go dark until a coordinator responds.
                // This keeps a fully-direct / self-hosted join working even when
                // the ambient default coordinator is firewalled, down, or a
                // founder disabled coordination outright.
                Err(err) => {
                    eprintln!(
                        "[node {label}] reachability: coordinator rendezvous unavailable \
                         ({err}) — continuing WITHOUT rendezvous (direct/front paths still \
                         work; coordinated-by-identity paths disabled until a coordinator \
                         responds)"
                    );
                    reachability::NatResolver::bind(me, Vec::new(), None)
                        .await
                        .expect("empty-coordinator pass-through resolver is infallible")
                }
            }
        }
        _ => {
            let auth = Some((signer.clone(), coord_cap));
            match reachability::NatResolver::bind(me, coords.clone(), auth).await {
                Ok(resolver) => resolver,
                // Same degrade-don't-die rule on the TUN/fake path: a dead
                // coordinator disables coordinated candidates, never direct ones.
                Err(err) => {
                    eprintln!(
                        "[node {label}] reachability: coordinator rendezvous unavailable \
                         ({err}) — continuing WITHOUT rendezvous (direct/front paths still \
                         work; coordinated-by-identity paths disabled until a coordinator \
                         responds)"
                    );
                    reachability::NatResolver::bind(me, Vec::new(), None)
                        .await
                        .expect("empty-coordinator pass-through resolver is infallible")
                }
            }
        }
    };
    if let Some(reflexive) = resolver.reflexive() {
        println!("[node {label}] reachability: coordinator-observed reflexive {reflexive}");
    }
    // a parked standby's gossip arrives under the network's derived lobby
    // identity (its own key is untracked until the grant cutover) — admit
    // that ingress; content signatures still authenticate every message.
    // the namespace is a TOML-sourced string, so `as_bytes` reproduces the
    // exact bytes the transport derived the lobby key from.
    let gossip_ingress = Some(config::lobby_identity(chain_id.as_bytes()).public_key());
    let config = reachability::ReachabilityConfig {
        chain_id,
        signer,
        wireguard_key_file,
        wireguard_port: wireguard_listen.port(),
        wireguard_advertised,
        control_endpoint,
        coordinators: coords,
        port_policy: policy,
        persist_file: Some(mesh_state_file),
        gossip_ingress,
    };
    // the invite intro listener: a fresh joiner's first contact. one
    // datagram carries the token, the joiner's identity + proof, and its
    // WireGuard key (identity-bound); a verified intro installs the
    // join-window tunnel peer (endpoint = the datagram's observed source —
    // WireGuard roams to the joiner's authenticated initiation anyway) and
    // the ack goes back only after the interface really carries it.
    // membership is NOT checked here (this task has no state access) — the
    // in-consensus redemption enforces it; a revoked member's token can at
    // worst open a tunnel that admits nothing.
    if let Some(intro_addr) = intro_listen {
        let intro_cmds = nudges.clone().downgrade();
        let intro_label = label.clone();
        // `chain_id` (the namespace string) moved into the plane config
        // above; the binding tokens sign over is those same bytes.
        let binding = config.chain_id.clone().into_bytes();
        tokio::spawn(async move {
            let socket = match tokio::net::UdpSocket::bind(intro_addr).await {
                Ok(socket) => socket,
                Err(err) => {
                    eprintln!(
                        "[node {intro_label}] invite intro listener bind {intro_addr} failed: \
                         {err} — joins via this node's invites need another member"
                    );
                    return;
                }
            };
            println!("[node {intro_label}] invite intro listening on udp/{intro_addr}");
            let mut buf = vec![0u8; 4096];
            loop {
                let Ok((n, src)) = socket.recv_from(&mut buf).await else {
                    continue;
                };
                let Ok(msg) = lobby::decode_intro(&buf[..n]) else {
                    continue; // junk on the doorbell — drop.
                };
                let ack = |installed: bool, detail: String| {
                    let ack = lobby::IntroAck {
                        nonce: msg.nonce.clone(),
                        installed,
                        detail,
                    };
                    let bytes = lobby::encode_intro_ack(&ack);
                    let socket = &socket;
                    async move {
                        let _ = socket.send_to(&bytes, src).await;
                    }
                };
                let verified = match lobby::verify_intro(&msg, &binding) {
                    Ok(v) => v,
                    Err(e) => {
                        ack(false, e).await;
                        continue;
                    }
                };
                let Some(cmds) = intro_cmds.upgrade() else {
                    break;
                };
                let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
                let install = reachability::ReachabilityCommand::InstallInvitePeer {
                    peer: verified.joiner.clone(),
                    wireguard_public_key: wireguard_upgrade::X25519PublicKey(
                        verified.wg_public_key,
                    ),
                    endpoint: src,
                    reply: reachability::InstallReply(reply_tx),
                };
                if cmds.send(install).await.is_err() {
                    break;
                }
                match reply_rx.await {
                    Ok(Ok(())) => {
                        println!(
                            "[node {intro_label}] invite intro: tunnel peer installed for {}",
                            config::hex_bytes(&verified.joiner.as_ref()[..4])
                        );
                        ack(true, "tunnel installed".into()).await;
                    }
                    Ok(Err(e)) => ack(false, e).await,
                    Err(_) => ack(false, "plane exited".into()).await,
                }
            }
        });
    }
    if let Some(mut invite_intro_rx) = invite_intro_rx.take() {
        let intro_cmds = nudges.clone().downgrade();
        let intro_label = label.clone();
        let binding = config.chain_id.clone().into_bytes();
        tokio::spawn(async move {
            while let Some((src, bytes)) = invite_intro_rx.recv().await {
                let Ok(msg) = lobby::decode_intro(&bytes) else {
                    continue;
                };
                let verified = match lobby::verify_intro(&msg, &binding) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let Some(cmds) = intro_cmds.upgrade() else {
                    break;
                };
                let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
                let install = reachability::ReachabilityCommand::InstallInvitePeer {
                    peer: verified.joiner.clone(),
                    wireguard_public_key: wireguard_upgrade::X25519PublicKey(
                        verified.wg_public_key,
                    ),
                    endpoint: src,
                    reply: reachability::InstallReply(reply_tx),
                };
                if cmds.send(install).await.is_err() {
                    break;
                }
                let ack = match reply_rx.await {
                    Ok(Ok(())) => {
                        println!(
                            "[node {intro_label}] invite intro: coordinated tunnel peer \
                             installed for {}",
                            config::hex_bytes(&verified.joiner.as_ref()[..4])
                        );
                        lobby::IntroAck {
                            nonce: msg.nonce.clone(),
                            installed: true,
                            detail: "tunnel installed".into(),
                        }
                    }
                    Ok(Err(e)) => lobby::IntroAck {
                        nonce: msg.nonce.clone(),
                        installed: false,
                        detail: e,
                    },
                    Err(_) => lobby::IntroAck {
                        nonce: msg.nonce.clone(),
                        installed: false,
                        detail: "plane exited".into(),
                    },
                };
                let bytes = lobby::encode_intro_ack(&ack);
                let _ = cmds
                    .send(reachability::ReachabilityCommand::SendResolverDatagram {
                        endpoint: src,
                        bytes,
                    })
                    .await;
            }
        });
    }

    // the boot `Retarget`'s record fan-out fires before the p2p actors have
    // a single live connection, and mesh sends are best-effort — when both
    // sides of a link lose that first datagram the plane deadlocks in record
    // gossip. the nudge re-offers un-acked gossip until the epoch assembles
    // (a no-op afterwards). the ticker holds only a WEAK sender: the plane's
    // exit is "every command sender dropped", and a strong clone here would
    // keep its own channel alive forever.
    let nudges = {
        let weak = nudges.downgrade();
        // the strong param must die NOW — holding it for the plane's
        // lifetime would itself keep the channel open.
        drop(nudges);
        weak
    };
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(NUDGE_INTERVAL);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            let Some(tx) = nudges.upgrade() else { break };
            if tx
                .send(reachability::ReachabilityCommand::Nudge)
                .await
                .is_err()
            {
                break;
            }
        }
    });
    match effect_kind {
        WireGuardEffectKind::Fake => {
            println!(
                "[node {label}] reachability: wireguard_effect = \"fake\" — tunnel configs are \
                 recorded in memory; no real interface is touched"
            );
            if let Err(err) = reachability::run(
                config,
                wireguard_effect::FakeWireGuardEffect::default(),
                resolver,
                commands,
                events,
            )
            .await
            {
                eprintln!("[node {label}] reachability plane exited: {err}");
            }
        }
        WireGuardEffectKind::Socket => {
            let underlay = socket_underlay.expect("bound above for socket mode");
            println!(
                "[node {label}] reachability: driving the userspace socket backend (TUN-less; \
                 no interface, no privilege — overlay reachability lives inside this process)"
            );
            let effect = overlay_net::userspace::UserspaceWireGuardEffect::with_shared_underlay(
                tokio::runtime::Handle::current(),
                overlay_slot,
                underlay,
            );
            if let Err(err) = reachability::run(config, effect, resolver, commands, events).await {
                eprintln!("[node {label}] reachability plane exited: {err}");
            }
        }
        WireGuardEffectKind::Tun => {
            #[cfg(unix)]
            {
                // same name the orchestrator writes into every
                // InterfaceConfiguration it applies — the WGApi handle and
                // the configs it receives must agree on the interface.
                let ifname = reachability::interface_name(&config.chain_id);
                let effect = match wireguard_effect::DefguardWireGuardEffect::new(&ifname) {
                    Ok(effect) => effect,
                    Err(err) => {
                        eprintln!(
                            "[node {label}] reachability: wireguard api handle for {ifname:?} \
                             failed ({err}) — plane not started; set wireguard_effect = \
                             \"fake\" to run without a real interface"
                        );
                        return;
                    }
                };
                println!("[node {label}] reachability: driving wireguard interface {ifname}");
                if let Err(err) =
                    reachability::run(config, effect, resolver, commands, events).await
                {
                    eprintln!("[node {label}] reachability plane exited: {err}");
                }
            }
            #[cfg(not(unix))]
            {
                eprintln!(
                    "[node {label}] reachability: the real wireguard effect needs a unix host — \
                     plane not started; set wireguard_effect = \"fake\" to run without a real \
                     interface"
                );
            }
        }
    }
}

/// stand up the real-socket node from `cfg` and run it until killed (validator)
/// or until state sync completes (`--sync-only`).
///
/// deliberately NOT `#[tokio::main]`: `tokio::Runner` owns its OWN tokio runtime,
/// and you cannot start a runtime from inside one. so `main` is sync and hands
/// off to `Runner::start`, which drives everything (including the engine's spawned
/// tasks) on the runtime it owns.
fn run_node(
    resolved: Resolved,
    sync_only: bool,
    log_ring: noded::LogRing,
) -> Result<(), Box<dyn std::error::Error>> {
    let Resolved {
        signer,
        label,
        namespace,
        // the descriptor's own chain-id (network shape) or the raw dev-shape
        // namespace — NOT `namespace` below, which is `genesis_namespace()`
        // (chain_id@fingerprint) on the network shape. this is the string the
        // desktop app records as `Workspace.chain_id`; threaded into
        // `identity`'s certificate domain separation.
        chain_id: identity_chain_id,
        mesh: peers,
        validators,
        bootstrappers,
        coordinated,
        listen,
        advertised,
        storage_dir: storage,
        rpc_listen,
        http_listen,
        wireguard_listen,
        wireguard_effect,
        wireguard_key_file,
        invite_listen,
        dev_demo,
        checkpoint_blocks,
        invite_token,
        invite_wireguard,
        invite_fronts,
        sync_index,
        announce_capabilities,
        duckdns_announcements,
        coordination,
        coord_cap,
        workspace,
    } = resolved;
    // Production descriptors already carry `<name>#<8-hex>`. The legacy
    // dev-seed shape predates that format, so give only that explicit shape a
    // deterministic zero salt without changing identity's signing domain.
    let duckdns_chain_id = match duckdns::derive_chain_label(&identity_chain_id) {
        Ok(_) => identity_chain_id.clone(),
        Err(_) if dev_demo => {
            let candidate = format!("{identity_chain_id}#00000000");
            duckdns::derive_chain_label(&candidate)
                .map_err(|e| format!("dev DuckDNS chain label: {e}"))?;
            candidate
        }
        Err(error) => {
            return Err(format!("network chain id is not DuckDNS-compatible: {error}").into());
        }
    };
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
        if invite_token.is_some() {
            println!(
                "[node {label}] identity {} is not in the genesis validator set — joiner \
                 mode: announcing this key with the invite token; a member node redeems it \
                 automatically (the mint was the approval) and full-node standing lands at \
                 the next block",
                hex_bytes(signer.public_key().as_ref())
            );
        } else {
            println!(
                "[node {label}] identity {} is not in the genesis validator set — joiner \
                 mode: no invite token on disk, so a member must grant standing manually \
                 (`ducktape-node invite-accept {}`)",
                hex_bytes(signer.public_key().as_ref()),
                hex_bytes(signer.public_key().as_ref())
            );
        }
    }

    // keep the raw (key, addr) pairs for statesync source selection before
    // discovery's bootstrapper list converts to its own ingress address type.
    let sync_candidates = bootstrappers.clone();
    // cold-restart dial seeding: the persisted mesh's control endpoints (the
    // chain-derived ULAs for overlay-advertised members) join the dial
    // hints, so a member restarting with every configured ingress gone
    // still dials its peers over the tunnels the reachability plane
    // re-applies at boot. Config hints win: a peer that already has a
    // configured entry gets no seed, in case discovery keeps one ingress
    // per key — a possibly-dead persisted address must never displace a
    // live operator-provided one. Load refusals (tamper, format, chain) are
    // the plane's to surface at its restore; here they just mean no seeds.
    let chain_id = String::from_utf8_lossy(&namespace).to_string();
    let mesh_state_file = storage.join("mesh-state.json");
    // fail-closed check for private coordination: a node that is neither a
    // genesis validator (admitted by membership) nor holding a `coord.cap`
    // will have every rendezvous request silently dropped by a private
    // coordinator. Surface that loudly instead of pretending the plane is
    // healthy — the tunnels never come up, and the operator needs to know it
    // is a missing credential, not a network fault.
    if wireguard_listen.is_some()
        && !coordinated.is_empty()
        && coordination == config::Coordination::Private
        && coord_cap.is_none()
        && !validators.contains(&signer.public_key())
    {
        eprintln!(
            "[node {label}] reachability: private coordination but no coord.cap and not a \
             genesis validator — rendezvous will be denied; provide coord.cap or use a \
             fronted/direct reach hint"
        );
    }
    let mesh_dial_seeds: Vec<(ed25519::PublicKey, Ingress)> =
        match reachability::store::load(&mesh_state_file, &chain_id) {
            Ok(Some(mesh)) => {
                let me = reachability::identity_of(&signer.public_key());
                let seeds: Vec<(ed25519::PublicKey, Ingress)> = mesh
                    .adverts
                    .iter()
                    .map(|advert| &advert.record)
                    .filter(|record| record.validator_identity != me)
                    .filter_map(|record| {
                        let pk =
                            ed25519::PublicKey::decode(&record.validator_identity.0[..]).ok()?;
                        if bootstrappers.iter().any(|(hinted, _)| *hinted == pk) {
                            return None;
                        }
                        Some((pk, Ingress::Socket(record.control_endpoint.socket_addr())))
                    })
                    .collect();
                if !seeds.is_empty() {
                    println!(
                        "[node {label}] {} mesh dial seed(s) from the persisted mesh (epoch {})",
                        seeds.len(),
                        mesh.epoch
                    );
                }
                seeds
            }
            _ => Vec::new(),
        };
    let bootstrappers: Vec<(ed25519::PublicKey, _)> =
        bootstrappers.into_iter().chain(mesh_dial_seeds).collect();

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
    // coordinated reach targets are split OUT of the TCP mesh dialer (a
    // coordinator's UDP rendezvous port is not a TCP mesh peer — dialing it
    // there was a silent no-op). reaching them is the reachability plane's
    // job: gossip relays through whatever mesh links exist, the nat client
    // rendezvouses via the coordinator and hole-punches the WireGuard path, and
    // once tunnels apply the mesh dials the target's advertised overlay
    // address over the tunnel (the target sets `advertised = "overlay"`).
    // what still needs a TCP foothold is the gossip itself: with ZERO
    // bootstrap links nothing carries this node's records anywhere. a
    // RESTART has one without config: the persisted mesh re-applies at boot
    // and its ULA seeds joined `bootstrappers` above. what remains uncovered
    // is the FIRST join (nothing persisted yet) on a coordinated-only
    // config — surface that loudly rather than park silently.
    if !coordinated.is_empty() {
        if bootstrappers.is_empty() {
            println!(
                "[node {label}] WARNING: {} coordinated reach target(s) but NO direct/fronted \
                 bootstrap link and no persisted mesh — tunnel bring-up gossip has no path to \
                 ride, so these peers stay unreachable. add at least one direct/fronted hint \
                 (an ephemeral ingress is enough) for the join window; after the first \
                 converged mesh, restarts ride the persisted state.",
                coordinated.len()
            );
        } else {
            println!(
                "[node {label}] {} coordinated reach target(s): mesh traffic flows over the \
                 WireGuard tunnel once the reachability plane converges.",
                coordinated.len()
            );
        }
        for (target, coord, _coord_key) in &coordinated {
            println!(
                "[node {label}]   coordinated target {} via coordinator {coord:?}",
                hex_bytes(&target.as_ref()[..4])
            );
        }
    }
    if let Some(wg) = &wireguard_listen {
        let advertise = if wg.ip().is_unspecified() {
            format!(
                "endpoint-less on udp port {} (roaming: peers learn this node's address from its own initiations)",
                wg.port()
            )
        } else {
            format!("advertising WireGuard endpoint udp/{wg}")
        };
        match wireguard_effect {
            WireGuardEffectKind::Tun => {
                println!("[node {label}] reachability plane: {advertise}")
            }
            WireGuardEffectKind::Socket => println!(
                "[node {label}] reachability plane: {advertise}; userspace socket backend \
                 (TUN-less — overlay reachability lives inside this process)"
            ),
            WireGuardEffectKind::Fake => println!(
                "[node {label}] reachability plane: {advertise}; \
                 records, advertisements, and tunnel handshakes run for real, the interface \
                 effect is the in-memory fake (no real tunnel)."
            ),
        }
    }

    // the rpc listener binds OUTSIDE the runtime (plain std tcp on OS threads)
    // so a bind failure is a clean startup error, not an async surprise. a
    // JOINER binds too: the park loop pumps the same surface — a resident
    // serves local reads from its pre-synced boundary, a still-parked joiner
    // answers with a clear not-admitted error instead of a dead port.
    let rpc_listener = match rpc_listen.as_deref() {
        Some(addr) if !sync_only => Some(std::net::TcpListener::bind(addr)?),
        _ => None,
    };
    // the http/ws app surface: same bind-early rule. the server itself runs on
    // its OWN plain-tokio OS thread (noded's exact split — the host never
    // leaves the commonware runner thread; http handlers only send
    // NodeCommands over the lane), so the pump below is its single consumer.
    let (http_handle, http_cmds, stream_hub) = noded::NodeHandle::channel_with_log_ring(log_ring);
    // the derived per-module index (noded's exact store, <storage>/index),
    // plus the blocks database the explorer reads: the pump folds sealed
    // blocks into it, boot heals it from verified state at sync/recovery
    // boundaries, a resident's follow arm heals it at every state-changing
    // boundary it serves, and the already-routed GET /v1/blocks +
    // /v1/index/* lanes light up through the handle below. an open failure
    // is fatal-with-remedy rather than a silent no-index run: the tier is
    // rebuildable, so the fix is always "delete <storage>/index".
    let index = noded::open_index_store(&storage, &MODULE_IDS)?;
    stream_hub.prime(index.resume_height()?, String::new());
    // the voice hub's session lane: /v1/call/ws handlers ask for huddle
    // audio sessions here. created up front because the app-surface thread
    // starts before the mesh exists; only the validator path below spawns the
    // hub that drains it — on every other path the receiver just drops and
    // the route answers with a refusal.
    let (voice_lane, voice_requests) = tokio::sync::mpsc::channel::<noded::CallSessionRequest>(8);
    // point the http handle at this node's forge repo base (the same
    // `storage/forge-repo` the host materializes into) so the git upload-pack
    // (clone/fetch) route can open a repo READ-ONLY and serve its objects.
    let http_handle = http_handle
        // persist node-local blobs (op receipts, agent prompt pins) under
        // <storage>/blobstore so a daemon restart keeps serving them.
        .with_blob_root(storage.join("blobstore"))?
        .with_forge_repo(storage.join("forge-repo"))
        .with_index_store(index.clone())
        .with_call(voice_lane)
        // the duckfs workspace RPC's managed-checkout root (disk state, separate
        // from the module's own `<storage>/duckfs` dir).
        .with_duckfs_workspaces(storage.join("duckfs-workspaces"));
    let blobs = http_handle.blob_handle();
    // the REAL portable-agent-run provisioner, built from a clone of the http
    // handle BEFORE the serve/drop match consumes it. portable (v3) runs
    // materialize a per-run duckfs checkout under a root VALIDATED to be
    // outside <storage> (D7) and drive checkout/commit over this SAME
    // NodeHandle actor lane the /v1/fs/workspaces RPC already rides here.
    // LIVE for every agent run: this binary wires the files module
    // unconditionally, so the runs composer emits v3 (the de-versioned
    // activation — no flag day, pre-production re-genesis). a misconfigured
    // root (inside <storage>) is a boot error, never a silent D7 hole.
    let agent_provisioner: Option<dispatch_oracle::SharedProvisioner> = Some(std::sync::Arc::new(
        noded::agent_provision::NodedProvisioner::new(
            http_handle.clone(),
            noded::agent_provision::agent_runs_root(&storage)
                .unwrap_or_else(|e| panic!("agent runs root failed D7 validation: {e}")),
        ),
    ));
    // (like the rpc surface above, a joiner binds and the park loop pumps —
    // reads only until promotion re-execs this process into a validator.)
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
    // per-agent host state, rooted OUTSIDE <storage> (D7 isolation floor): the
    // persistent executor workspaces + session files must NOT be descendants of
    // the key/consensus/blob tree, so a `..` from a run's cwd can't reach
    // user.key/node keys/qmdb/blobstore. `DUCKTAPE_AGENT_WORKSPACES` / _SESSIONS
    // override — see capability-host. host-local only, never consensus.
    // non-portable (v2/persistent) agent workspaces stay under <storage>, exactly
    // as today — relocating them would be a live (non-dormant) durability change.
    // D7 relocation applies to the PORTABLE provisioner mount (agent_runs_root),
    // which is out of <storage>; the pre-existing non-portable D7 gap is a
    // separate, migration-aware hardening (tracked as a follow-up).
    let agent_dirs = capability_host::AgentDirs::under(&storage);
    let rt_cfg = commonware_runtime::tokio::Config::default().with_storage_directory(storage);
    let executor = commonware_runtime::tokio::Runner::new(rt_cfg);

    // the seam's stack handle (socket mode): one slot for the process,
    // created HERE so every consumer — the mesh context's backend, the
    // statesync plane's socket factory, and the reachability plane's effect
    // (which owns the writes) — holds the same one. in tun/fake mode it just
    // stays empty.
    let overlay_slot = overlay_net::userspace::StackSlot::new();

    executor.start(|context| async move {
        // the validator's own `ducktape_*` Prometheus series, registered on the
        // SAME runtime registry `context.encode()` (GET /metrics) serves — the
        // drain loop below folds each applied block in (height, count, apply
        // latency, per-module dispatch counters), so the networked node reports
        // the series the local daemon does and one Grafana board reads both.
        let metrics = noded::NodeMetrics::register(&context);

        // the authorized MESH set, SORTED — what discovery tracks. the
        // consensus scheme uses the (possibly smaller) validator set derived
        // from committed valset state after the recovery boot below.
        let mesh_participants: Set<ed25519::PublicKey> =
            Set::try_from(peers.clone()).expect("authorized peer set has no duplicates");

        // this node's mesh identity, as served on /v1/status — clients stamp
        // it into peer-routed ops (chat's JoinHuddle.node).
        let status_public_key = hex_bytes(signer.public_key().as_ref());

        // the statesync source a --sync-only joiner pulls from: only
        // validators serve the channel, so the candidate must be a validator
        // that is not us (a non-validator hint or our own key would be
        // retried forever — discovery never connects a node to itself).
        let sync_sources =
            config::sync_source_candidates(&sync_candidates, &validators, &signer.public_key());
        let sync_source = sync_sources.first().cloned();

        // the real encrypted TCP mesh. `local` is the dev preset (allows private
        // ips). MUST be the real tokio runtime — discovery live-locks under the
        // deterministic clock.
        // reachability plane (docs/deploy/sentry-deployment.md): a forward sentry on a
        // private network relies on this `local` preset's allow_private_ips:true;
        // switching to a preset with allow_private_ips:false would reject the
        // forwarded connection from a private source IP — use a public-IP sentry
        // or a reverse tunnel then.
        //
        // TRANSPORT IDENTITY: a parked joiner's own key is usually untracked
        // on every member (that is what admission changes), so it would be
        // bounced at the handshake and could neither announce itself nor poll
        // the statesync manifest. such a joiner connects AS the network's
        // derived LOBBY identity — the one key every member tracks that any
        // invite holder can derive. its REAL key still signs everything that
        // matters (the join proof, and consensus after the promotion reboot).
        // the door only exists where the mesh tracks it: the network shape
        // folds the lobby key into every member's mesh, so `peers` carries it
        // here too (both sides derive the same mesh). a mesh WITHOUT a lobby
        // key (the dev-seed shape) keeps the old behavior — the joiner parks
        // under its real identity, refused until the cutover re-tracks it.
        let lobby = config::lobby_identity(&namespace);
        let p2p_signer = if joiner && peers.contains(&lobby.public_key()) {
            lobby
        } else {
            signer.clone()
        };
        // the staged reachability plane derives its advertised control
        // endpoint from the mesh `advertised`; keep a copy — discovery's
        // config consumes the original.
        let advertised_reach = advertised.clone();
        let p2p_cfg = discovery::Config::local(
            p2p_signer,
            &namespace,
            listen,
            advertised,
            bootstrappers,
            MAX_MESSAGE_SIZE,
        );
        // the overlay-net seam (ADR 2026-07-07): the mesh dials/binds through
        // a wrapper context whose Network routes BY ADDRESS — sockets on this
        // chain's ULA /48 go to the active overlay backend (today: the TUN
        // pass-through, i.e. the same OS socket the kernel routes through the
        // wireguard interface), everything else straight to the OS. the p2p
        // dialer never connect()s an overlay ULA on a raw OS socket as an
        // assumption again; the userspace backend lands behind this seam.
        // the prefix derives from the SAME namespace string statesync_plane's
        // OverlayBook and the reachability plane use, so all three agree on
        // what "overlay" means.
        let overlay_router = overlay_net::OverlayRouter::for_prefix48(
            wireguard_upgrade::ula_v6_prefix(&String::from_utf8_lossy(&namespace)),
        );
        // ADR phase 3: the backend follows `wireguard_effect`. socket mode
        // routes overlay dials/binds into the in-process virtual stack (and
        // gives the wildcard mesh listener its virtual leg); tun AND fake
        // keep the OS pass-through — fake stages no data plane at all, so
        // pass-through preserves its long-standing "overlay dials just fail
        // like a downed interface" behavior.
        let overlay_backend = match wireguard_effect {
            WireGuardEffectKind::Socket => {
                overlay_net::OverlayBackend::Userspace(overlay_slot.clone())
            }
            WireGuardEffectKind::Tun | WireGuardEffectKind::Fake => {
                overlay_net::OverlayBackend::Tun
            }
        };
        let (mut network, mut oracle) = Network::new(
            overlay_net::OverlayContext::with_backend(
                context.child("network"),
                overlay_router,
                overlay_backend,
            ),
            p2p_cfg,
        );

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
            // the submit-relay lane: a sync-only resident holds no standing,
            // relays no writes, and answers nothing — but an unregistered
            // channel kills the sender, so black-hole.
            {
                let (_tx, mut rx) = network.register(CHANNEL_SUBMIT_RELAY, quota, MAX_BACKLOG);
                context
                    .child("blackhole_submit_relay")
                    .spawn(move |_ctx| async move { while rx.recv().await.is_ok() {} });
            }
            // the lobby lane: a sync-only resident never announces or answers,
            // but an unregistered channel is a protocol violation — black-hole.
            {
                let (_tx, mut rx) = network.register(CHANNEL_LOBBY, quota, MAX_BACKLOG);
                context.child("blackhole_lobby").spawn(move |_ctx| async move {
                    while rx.recv().await.is_ok() {}
                });
            }
            // the reachability lane: a sync-only resident runs no WireGuard
            // plane, but the channel must exist — black-hole.
            {
                let (_tx, mut rx) = network.register(CHANNEL_REACHABILITY, quota, MAX_BACKLOG);
                context
                    .child("blackhole_reachability")
                    .spawn(move |_ctx| async move { while rx.recv().await.is_ok() {} });
            }
            // the voice lane: a sync-only resident serves no huddle audio,
            // but the channel must exist — black-hole. dropping the session
            // lane makes /v1/call/ws refuse instead of hang (this branch
            // never reaches the validator hub below).
            drop(voice_requests);
            {
                let (_tx, mut rx) = network.register(CHANNEL_VOICE, quota, MAX_BACKLOG);
                context
                    .child("blackhole_voice")
                    .spawn(move |_ctx| async move { while rx.recv().await.is_ok() {} });
            }
            // the video lane: a sync-only resident serves no huddle video, but
            // the channel must exist — black-hole.
            {
                let (_tx, mut rx) = network.register(CHANNEL_VIDEO, quota, MAX_BACKLOG);
                context
                    .child("blackhole_video")
                    .spawn(move |_ctx| async move { while rx.recv().await.is_ok() {} });
            }
            network.start();

            if sync_sources.is_empty() {
                eprintln!(
                    "[node {label}] no statesync source: no validator other than this node \
                     is available to serve (only validators answer the statesync channel)"
                );
                std::process::exit(1);
            }
            // rotate across every validator that can serve — the payloads
            // verify against consensus roots, so source choice is pure
            // availability.
            let client = P2pSyncClient::with_sources(
                context.child("sync_client"),
                sync_tx,
                sync_rx,
                sync_sources.clone(),
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

            // BOOT PREFLIGHT (design §5 / plan Task 7.3): refuse an under-versioned
            // binary against the SERVED boundary before installing/composing, so a
            // too-old joiner fails with a clear "install the newer binary" message
            // rather than an opaque post-compose app-hash mismatch. the served
            // `required_min_version` is an unauthenticated hint (untrusted-server
            // model): a lying value can at worst refuse-to-boot this joiner, never
            // fork. inert on a baseline manifest.
            if let Err(e) = manifest.preflight(MAX_PROTOCOL_VERSION) {
                eprintln!("[node {label}] SYNC REFUSED: {e}");
                std::process::exit(1);
            }

            // rebuild EVERY module in the manifest (a REAL joiner owns its
            // disk, so every store opens under its canonical module id) and
            // print the greppable line the demo script asserts on.
            let forge_repo = storage_for_sync.join("forge-repo");
            let duckfs_dir = storage_for_sync.join("duckfs");
            match sync_all_modules(
                &context,
                &client,
                &manifest,
                NetworkBindings {
                    invite: &namespace,
                    identity_chain_id: &identity_chain_id,
                    duckdns_chain_id: &duckdns_chain_id,
                },
                SyncSubstrates {
                    forge_repo: &forge_repo,
                    duckfs_dir: &duckfs_dir,
                    blobs: blobs.clone(),
                },
                0,
            )
            .await
            {
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
        let duckfs_dir = storage_for_sync.join("duckfs");
        // boot sweep (#219): no sync attempt is in flight yet, so any leftover
        // `duckfs_scratch_a*` dir (a crashed attempt, or a promoted scratch
        // whose final removal was interrupted) is safe to remove. best-effort.
        SyncScratch::sweep_stale(&duckfs_dir);

        // ---- the JOINER / REPLICA: park on the mesh, bootstrap a boundary,
        // then FOLD the head (unified-node phase 2) ----
        //
        // decided from the REAL store (the pre-runtime probe only gated
        // listeners): a key outside the genesis set that no checkpoint seats
        // as a participant. a fresh join has no checkpoint at all; a
        // RESTARTED replica has one that names it a resident — it re-enters
        // here and re-ascends (a fresh bootstrap into its existing journal;
        // recovering the folded state by journal replay instead is the
        // remaining phase-2 follow-up). after PROMOTION the checkpoint
        // seats this key, so a rebooted process falls through to the
        // validator path below.
        let checkpoint_seats_me = manifest.as_ref().is_some_and(|m| {
            m.participants
                .iter()
                .any(|k| k.as_slice() == signer.public_key().as_ref())
        });
        if !checkpoint_seats_me && !validators.contains(&signer.public_key()) {
            if manifest.is_none() && !recovery.journal_is_empty().await {
                eprintln!(
                    "[node {label}] FATAL: recovery journal exists but the checkpoint is \
                     missing — wipe the app state and re-join (KEEP any consensus journal \
                     partitions: they are what prevents this key from double-voting)"
                );
                std::process::exit(1);
            }
            // the parked mesh identity: genesis set at the base index (no
            // consensus coordinates yet). engine lanes are NOT black-holed
            // like the sync-only resident — the replica pipeline (phase 2)
            // consumes them:
            // - CERT lanes bridge their raw bytes to the fold driver, which
            //   decodes finalizations and verifies them against the epoch's
            //   quorum (the phase-1 gate). pre-standing, the same bytes fire
            //   the park loop's wake (a byte's arrival is the old nudge).
            // - PAYLOAD lanes drain store-only into the shared content store
            //   (content-addressing is the verification), so a finalization's
            //   bytes are usually already local when its certificate lands.
            // - vote/resolver/fetch lanes stay black-holed. the follower runs
            //   WITHOUT a payload resolver: a gossip-missed payload surfaces
            //   as Unresolvable and backfills over the Frames lane (the
            //   backstop that must exist anyway). a banked-but-unread lane is
            //   NOT an option — validators' resolvers send fetch requests to
            //   every tracked peer, and an unread backlog jams the very
            //   connection the sync client rides.
            oracle.track(PEER_SET, mesh_participants.clone());
            let replica_store = ContentStore::new();
            let (head_wake_tx, mut head_wake) = futures::channel::mpsc::channel::<()>(1);
            // raw cert-lane bytes for the fold driver: bounded, drop-on-full —
            // a shed certificate is re-anchored by the next one's parent
            // linkage (the planner backfills the gap), so the drain never
            // blocks the peer connection.
            let (cert_bridge_tx, mut cert_bridge) =
                futures::channel::mpsc::channel::<Vec<u8>>(256);
            for epoch in 0..EPOCH_CHANNEL_BANK {
                let (vote, cert, res, payload, fetch) = engine_channels(epoch);
                for ch in [vote, res, fetch] {
                    let (_tx, mut rx) = network.register(ch, quota, MAX_BACKLOG);
                    let label: &'static str =
                        Box::leak(format!("blackhole_{ch}").into_boxed_str());
                    context.child(label).spawn(move |_ctx| async move {
                        while rx.recv().await.is_ok() {}
                    });
                }
                {
                    let (_tx, mut payload_rx) = network.register(payload, quota, MAX_BACKLOG);
                    let store = replica_store.clone();
                    let label: &'static str =
                        Box::leak(format!("payload_store_{payload}").into_boxed_str());
                    context.child(label).spawn(move |_ctx| async move {
                        while let Ok((_peer, msg)) = payload_rx.recv().await {
                            let bytes: Vec<u8> = msg.into();
                            // store-ONLY, never delivered: delivery is the
                            // fold driver's verified-finalization arm.
                            store.put(bytes);
                        }
                    });
                }
                let (_tx, mut cert_rx) = network.register(cert, quota, MAX_BACKLOG);
                let label: &'static str =
                    Box::leak(format!("certbridge_{cert}").into_boxed_str());
                let mut wake = head_wake_tx.clone();
                let mut bridge = cert_bridge_tx.clone();
                context.child(label).spawn(move |_ctx| async move {
                    while let Ok((_peer, msg)) = cert_rx.recv().await {
                        let bytes: Vec<u8> = msg.into();
                        // full == a wake is already pending: coalesce, never
                        // block the drain (an unread lane kills the peer).
                        let _ = wake.try_send(());
                        // drop-on-full: parent linkage re-covers shed certs.
                        let _ = bridge.try_send(bytes);
                    }
                });
            }
            let (sync_tx, sync_rx) = network.register(CHANNEL_STATE_SYNC, quota, MAX_BACKLOG);
            // the reachability lane: a parked joiner with a WireGuard config
            // runs the plane in its STANDBY role — once resident standing
            // lands (the park loop below drives Retargets off the manifest),
            // it pre-warms tunnels with every member so activation, and the
            // promotion reboot via the persisted mesh, start connected
            // instead of assembling. Without `wireguard_listen` the channel
            // just stays legal — black-hole.
            let reach_cmd: Option<tokio::sync::mpsc::Sender<reachability::ReachabilityCommand>> = {
                let (reach_tx, mut reach_rx) =
                    network.register(CHANNEL_REACHABILITY, quota, MAX_BACKLOG);
                match wireguard_listen {
                    Some(wg_addr) => {
                        // AMBIENT coordinator: the joiner resolves coordinated
                        // rendezvous through its OWN configured/default
                        // coordinator, NEVER one baked into the invite (the
                        // unified invite carries no coordinator address). See
                        // docs/superpowers/specs/2026-07-08-fully-nated-inviter-design.md.
                        let coordinators: Vec<Ingress> = match config::coordinator_ingress(None) {
                            Ok(Some(ingress)) => vec![ingress],
                            Ok(None) => Vec::new(),
                            Err(e) => {
                                eprintln!(
                                    "[node {label}] invite: ambient coordinator unusable ({e}) — \
                                     coordinated first-contact paths disabled"
                                );
                                Vec::new()
                            }
                        };
                        Some(wire_reachability_plane(
                            &context,
                            &label,
                            &chain_id,
                            &signer,
                            &wireguard_key_file,
                            &mesh_state_file,
                            wg_addr,
                            wireguard_effect,
                            overlay_slot.clone(),
                            advertised_reach,
                            coordinators,
                            // a joiner serves no intros — only members mint
                            // redeemable invites.
                            None,
                            coord_cap.clone(),
                            reach_tx,
                            reach_rx,
                        ))
                    }
                    None => {
                        context
                            .child("blackhole_reachability")
                            .spawn(move |_ctx| async move { while reach_rx.recv().await.is_ok() {} });
                        drop(reach_tx);
                        None
                    }
                }
            };
            // the TUNNEL-FIRST join window: an invite that carried a WireGuard
            // bootstrap makes the tunnel the join's carrier — before any p2p,
            // (a) this node's interface gains the INVITER as a peer (endpoint
            // straight from the blob), and (b) an intro announcer delivers
            // this node's identity + WireGuard key to the inviter's intro
            // listener until acked, at which point the inviter's side of the
            // tunnel exists too. the mesh dialer below then reaches the
            // inviter's overlay ULA (the join-minted Direct hint) the moment
            // the tunnel routes, and everything else — lobby announce,
            // redemption, statesync — rides it.
            // the TUNNEL-FIRST join window races the invite's UNIFIED path
            // set: the inviter PLUS every offered front, in one candidate list.
            // The first candidate to install this joiner's token-signed intro
            // wins and the rest are cancelled; the mesh dialer below then
            // reaches that member's overlay ULA (the join-minted Direct hints)
            // the moment the tunnel routes, and everything else — lobby
            // announce, redemption, statesync — rides it. If every offered path
            // is exhausted the race is HONEST-terminal (a distinct exit, never
            // a silent success). The mechanics live in `first_contact_join`;
            // this is just the glue.
            if let (Some(reach), Some(token)) = (&reach_cmd, &invite_token) {
                let inviter = invite_wireguard.as_ref().and_then(|wg| {
                    match (wg.issuer_key(), wg.public_key_bytes()) {
                        (Ok(key), Ok(wg_key)) => Some(first_contact_join::InviterContact {
                            key,
                            wg: wg_key,
                            mesh_port: wg.mesh_port,
                            // the inviter's underlay endpoint; `None` => the
                            // inviter is coordinated-only (reached by identity).
                            endpoint: wg.endpoint.clone(),
                            // the inviter's explicitly-advertised intro listener
                            // (honors a custom `invite_listen`); the direct path
                            // uses it verbatim instead of re-deriving wg_port+1.
                            intro: wg.intro.clone(),
                        }),
                        _ => {
                            eprintln!(
                                "[node {label}] invite: inviter wireguard bootstrap is malformed \
                                 — racing the offered fronts alone"
                            );
                            None
                        }
                    }
                });
                let raw = first_contact_join::build_candidates(inviter, &invite_fronts);
                if raw.is_empty() {
                    // the invite offered no wireguard/front bootstrap — the join
                    // rides the descriptor's reach hints, exactly as before.
                } else {
                    let candidates = first_contact_join::plan_race(
                        raw,
                        matches!(wireguard_effect, config::WireGuardEffectKind::Tun),
                    );
                    match reachability::WireGuardKeypair::load_or_generate(&wireguard_key_file) {
                        Ok((keypair, _)) => {
                            // this joiner's own token-signed intro, built once
                            // and reused across every candidate in the race.
                            let intro = lobby::encode_intro(&lobby::intro_request(
                                &signer,
                                &namespace,
                                token,
                                keypair.public_key().0,
                            ));
                            let token_nonce = token.nonce.to_vec();
                            let reach = reach.clone();
                            let race_label = label.clone();
                            context.child("first_contact").spawn(move |_ctx| async move {
                                let outcome = first_contact_join::drive_first_contact(
                                    reach,
                                    candidates,
                                    intro,
                                    token_nonce,
                                    race_label.clone(),
                                    std::time::Duration::from_secs(90),
                                )
                                .await;
                                match outcome {
                                    first_contact_join::FirstContactOutcome::Installed {
                                        key,
                                        via,
                                    } => println!(
                                        "[node {race_label}] invite: first contact via {via} to \
                                         {} — join rides the overlay",
                                        hex_bytes(&key.as_ref()[..4])
                                    ),
                                    first_contact_join::FirstContactOutcome::Terminal {
                                        tried,
                                        reason,
                                    } => {
                                        eprintln!(
                                            "[node {race_label}] FATAL: first contact failed \
                                             across all {tried} offered path(s) — {reason}. ask \
                                             the inviter for a fresh invite once the mesh is \
                                             reachable."
                                        );
                                        std::process::exit(3);
                                    }
                                }
                            });
                        }
                        Err(e) => eprintln!(
                            "[node {label}] invite: wireguard key unreadable ({e}) — first \
                             contact not started; falling back to the descriptor's reach hints"
                        ),
                    }
                }
            }
            // the voice lane: a parked joiner serves no huddle audio, but the
            // channel must exist — black-hole. dropping the session lane makes
            // /v1/call/ws refuse instead of hang (this branch always ends in
            // the promotion reboot, never the validator hub below).
            drop(voice_requests);
            {
                let (_tx, mut rx) = network.register(CHANNEL_VOICE, quota, MAX_BACKLOG);
                context
                    .child("blackhole_voice")
                    .spawn(move |_ctx| async move { while rx.recv().await.is_ok() {} });
            }
            // the video lane: a parked joiner serves no huddle video, but the
            // channel must exist — black-hole.
            {
                let (_tx, mut rx) = network.register(CHANNEL_VIDEO, quota, MAX_BACKLOG);
                context
                    .child("blackhole_video")
                    .spawn(move |_ctx| async move { while rx.recv().await.is_ok() {} });
            }
            // the submit-relay lane: once resident standing lands, writes leave
            // here — this node signs its own frames and a validator takes
            // custody. replies (the frame's consensus fate) come back on the
            // same lane. bound `mut` because the serve window's relay helper
            // sends on `relay_tx`; `relay_rx` is bridged into the serve window
            // below (a torn-down select must never drop its `recv()` mid-flight).
            let (mut relay_tx, relay_rx) = network.register(CHANNEL_SUBMIT_RELAY, quota, MAX_BACKLOG);
            // the lobby lane: where this parked node announces its key. member
            // replies are drained by a printer task — purely informational.
            let (mut lobby_tx, mut lobby_rx) = network.register(CHANNEL_LOBBY, quota, MAX_BACKLOG);
            {
                let label = label.clone();
                // the parked joiner persists a coord.cap delivered over a
                // JoinReply into its workspace, so a later boot presents it to
                // the private coordinator (loaded via `load_coord_cap`).
                let cap_dir = workspace.clone();
                context.child("lobby_replies").spawn(move |_ctx| async move {
                    while let Ok((peer, msg)) = lobby_rx.recv().await {
                        let bytes: Vec<u8> = msg.into();
                        if let Ok(lobby::LobbyMsg::JoinReply {
                            recorded,
                            detail,
                            cap,
                            fatal,
                        }) = lobby::decode_msg(&bytes)
                        {
                            println!(
                                "[node {label}] member {}: {}{detail}",
                                hex_bytes(&peer.as_ref()[..4]),
                                if recorded { "" } else { "join request refused — " },
                            );
                            if fatal {
                                // this invite can NEVER redeem (e.g. its
                                // single-use token is already spent by
                                // another key) — retrying is a silent
                                // forever-spin. stop loudly: the FATAL
                                // marker is the app/operator contract.
                                eprintln!(
                                    "[node {label}] FATAL: {detail} — this invite cannot \
                                     be redeemed (an invite admits exactly one person). \
                                     ask the inviter for a fresh invite and re-join with \
                                     the new blob."
                                );
                                std::process::exit(1);
                            }
                            // a delivered cap (private coordination): unpack
                            // the opaque bytes and persist beside identity.
                            if let Some(cap_bytes) = cap {
                                match config::unpack_coord_cap(&cap_bytes) {
                                    Ok(cap) => match config::save_coord_cap(&cap_dir, &cap) {
                                        Ok(()) => println!(
                                            "[node {label}] coordinator cap delivered by \
                                             member {} — saved (issuer {}, expires {})",
                                            hex_bytes(&peer.as_ref()[..4]),
                                            hex_bytes(&cap.issuer.as_ref()[..4]),
                                            cap.not_after,
                                        ),
                                        Err(e) => eprintln!(
                                            "[node {label}] coordinator cap delivered but \
                                             could not be saved: {e}"
                                        ),
                                    },
                                    Err(e) => eprintln!(
                                        "[node {label}] member {} sent a malformed \
                                         coordinator cap: {e}",
                                        hex_bytes(&peer.as_ref()[..4]),
                                    ),
                                }
                            }
                        }
                    }
                });
            }
            network.start();

            let Some(server_peer) = sync_source else {
                eprintln!(
                    "[node {label}] no statesync source: no validator other than this node \
                     is available to serve (only validators answer the statesync channel)"
                );
                std::process::exit(1);
            };
            // the joiner's sync client: the mesh path always works and
            // ROTATES across every validator that can serve; with the
            // statesync plane enabled, requests PREFER an overlay stream to
            // the primary source and fall back on transport failure — the
            // plane binds lazily once the invite tunnel brings the interface
            // up.
            let mesh_client = P2pSyncClient::with_sources(
                context.child("sync_client"),
                sync_tx,
                sync_rx,
                sync_sources.clone(),
            );
            let client = {
                let plane_slot: statesync_plane::PlaneSlot =
                    std::sync::Arc::new(std::sync::OnceLock::new());
                if statesync_plane::enabled() && wireguard_listen.is_some() {
                    let book = statesync_plane::OverlayBook::new(
                        String::from_utf8(namespace.clone()).expect("namespace is utf-8"),
                    );
                    book.set_peers(peers.iter());
                    statesync_plane::spawn_bring_up(
                        label.clone(),
                        book,
                        signer.public_key(),
                        std::sync::Arc::clone(&plane_slot),
                        statesync_plane::socket_factory(wireguard_effect, &overlay_slot),
                        None,
                    );
                }
                statesync_plane::PlaneFallbackClient::new(plane_slot, &server_peer, mesh_client)
            };

            // the announce, built once: this key + the invite token + the
            // proof-of-possession binding them. re-sent (round-robin over the
            // known members) until the manifest shows this key admitted —
            // members keep the request queue in memory, so a member restart
            // just gets the next re-announce.
            let announce_frame = invite_token
                .as_ref()
                .map(|t| IoBuf::from(lobby::encode_msg(&lobby::join_request(&signer, &namespace, t))));
            let mut announce_targets: Vec<ed25519::PublicKey> = validators.clone();

            let me_bytes = signer.public_key().as_ref().to_vec();
            let mut last_tracked = PEER_SET;
            // the epoch the reachability plane last retargeted to (standby
            // role) — one Retarget per observed epoch.
            let mut last_plane_epoch: Option<u64> = None;
            let mut attempt = 0usize;
            let mut announce_round = 0usize;
            // once resident standing is seen, parking is the STEADY state
            // (awaiting a deliberate promote) — the not-admitted bail below
            // must never fire.
            let mut resident_standing = false;
            let mut send_announce = |targets: &[ed25519::PublicKey], attempt: usize| {
                let Some(frame) = &announce_frame else { return };
                if attempt % LOBBY_ANNOUNCE_EVERY != 1 || targets.is_empty() {
                    return;
                }
                let target = targets[announce_round % targets.len()].clone();
                announce_round += 1;
                let attempted = lobby_tx.send(Recipients::One(target.clone()), frame.clone(), false);
                if !attempted.is_empty() {
                    println!(
                        "[node {label}] invite announce sent to member {} — redemption follows",
                        hex_bytes(&target.as_ref()[..4])
                    );
                }
            };

            // ---- the RESIDENT's serving lanes ------------------------------
            //
            // the same two local surfaces a validator exposes, pumped by the
            // park loop's serve window below: a resident answers reads from
            // its last pre-synced boundary; a still-parked joiner answers
            // with a clear not-admitted error instead of a dead port. writes
            // are refused — ops enter the chain through validators only.
            // promotion re-execs this process (`reboot_self`), which closes
            // these listeners (CLOEXEC) and re-binds them on the validator
            // path.
            let (rpc_tx, mut rpc_ingress) = futures::channel::mpsc::channel::<RpcJob>(64);
            if let Some(listener) = rpc_listener {
                println!(
                    "[node {label}] rpc listening on {}",
                    listener.local_addr().map(|a| a.to_string()).unwrap_or_default()
                );
                spawn_rpc_listener(listener, rpc_tx);
            } else {
                drop(rpc_tx); // rpc off: the ingress arm stays terminated.
            }
            let mut http_ingress = http_cmds;
            // the last pre-synced boundary this resident serves reads from:
            // (boundary height, the composed host). exactly ONE live host may
            // exist — the sync path reopens the same on-disk partitions, so
            // this is dropped before every re-sync.
            // the REPLICA node: the same OrderedNode a validator drains, a
            // FollowerOrderer in the engine's seat, this node's real recovery
            // journal as the sink. None while knocking / bootstrapping; Some
            // from ascension on. reads serve from `.1.host()` through the
            // serve window; the fold driver feeds `.1.orderer_mut()`.
            let mut serving: Option<(
                u64,
                node::OrderedNode<
                    consensus::FollowerOrderer,
                    Recovery<commonware_runtime::tokio::Context>,
                >,
            )> = None;
            // the joiner's recovery journal, slot-shaped: ascension moves it
            // into the replica node (it IS the node's block sink); a descend
            // (epoch cutover / promotion) reopens a fresh handle after the
            // node drops. every path out of this branch diverges (reboot),
            // so the validator path below never observes the move.
            let mut recovery_slot = Some(recovery);
            let mut recovery_reopens = 0u32;
            // fold-driver state, all epoch-scoped and reset at (re)ascension:
            // the verifier for the CURRENT epoch's certificates, the view
            // coordinates, and the admitted-view watermark plan_fold plans
            // against (main-side twin of the follower's internal guard).
            let mut replica_scheme: Option<simplex_ed25519::Scheme> = None;
            let mut replica_epoch: u64 = 0;
            let mut replica_view_base: u64 = 0;
            let mut replica_watermark: Option<u64> = None;
            // served seals awaiting the post-fold cross-check: a BACKFILLED
            // frame's trust is the served seal, verified against what OUR
            // fold produced (height -> served (disposition, app_hash)).
            let mut pending_seal_checks: std::collections::HashMap<u64, ServedSeal> =
                std::collections::HashMap::new();
            let mut blocks_since_checkpoint: u64 = 0;
            let mut last_cert_height: Option<u64> = None;
            // the serving replica's manifest-fetch pacer (see the gate at the
            // fetch site). absolute, so per-cert window closes can't starve it.
            let mut next_manifest_fetch = std::time::Instant::now();
            // the replica's valset orchestrator — Some exactly when serving.
            // observe/ceiling/cutover mirror the validator drain; the SWAP
            // exchanges the follower orderer where a validator respawns an
            // engine.
            let mut replica_orchestrator: Option<
                consensus::ValsetOrchestrator<ed25519::PublicKey>,
            > = None;
            // the last checkpoint's (height, oplog position) — the prune
            // anchor: the journal below it drops once the floor passes it.
            let mut replica_prev_ckpt: (Option<u64>, u64) = (None, 0);
            // the app-hash of the last boundary the derived tier followed:
            // the index feed (heal + explorer row + ws event) fires only when
            // the verified app-hash MOVED. an unchanged hash is an idle
            // stride — state is byte-identical, the read models are already
            // exact, and the explorer stays as quiet as the validator's nop
            // gate keeps it. in-memory on purpose: after a restart the first
            // boundary re-fires and every write below is idempotent.
            let mut last_indexed_root: Option<StateRoot> = None;
            // ---- REPLICA RESTART: recover by journal replay --------------
            //
            // a checkpoint that routed us here (it names this key a resident,
            // not a participant) is a real recovery base: replay the journal
            // exactly as a validator restart would — restore the checkpoint
            // host, fold the retained suffix, verify the recomposed app-hash
            // — and enter the park loop ALREADY serving at the recovered tip.
            // no re-bootstrap: the fold driver closes any offline gap over
            // the Frames lane the moment the first certificate's parent
            // linkage names it.
            if let Some(ckpt) = manifest.as_ref() {
                if let Err(e) = ckpt.preflight(MAX_PROTOCOL_VERSION) {
                    eprintln!(
                        "[node {label}] FATAL: cannot recover — {e} (recovered boundary needs \
                         protocol v{}, this binary supports up to v{MAX_PROTOCOL_VERSION})",
                        ckpt.required_min_version()
                    );
                    std::process::exit(1);
                }
                let restored = restore_host(
                    &context,
                    &forge_repo,
                    &duckfs_dir,
                    ckpt,
                    NetworkBindings {
                        invite: &namespace,
                        identity_chain_id: &identity_chain_id,
                        duckdns_chain_id: &duckdns_chain_id,
                    },
                    blobs.clone(),
                )
                .await;
                let mut host = match restored {
                    Ok(h) => h,
                    Err(e) => {
                        eprintln!("[node {label}] FATAL: replica checkpoint restore: {e}");
                        std::process::exit(1);
                    }
                };
                // heal the derived index against the CHECKPOINT boundary
                // before replay, so the suffix folds land contiguously.
                if let Some(ckpt_height) = ckpt.height {
                    heal_index(&index, &host, ckpt_height, &label).await;
                }
                let mut recovery = recovery_slot
                    .take()
                    .expect("the journal slot is filled before the first ascension");
                let rec = match recovery.recover_with_sink(&mut host, ckpt, None).await {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!(
                            "[node {label}] FATAL: {e}\n\
                             [node {label}] replica state cannot be locally recovered. wipe \
                             the app-state partitions and re-join — but ALWAYS keep the \
                             consensus journal partitions: they are the anti-equivocation \
                             record for this key."
                        );
                        std::process::exit(1);
                    }
                };
                // seed the shared store with every retained frame so a
                // re-observed certificate resolves locally instead of
                // wedging the gate awaiting a fetch nobody owes us.
                for frame in &rec.frames {
                    replica_store.pin(frame.clone());
                }
                let tip = rec.height.unwrap_or(rec.view_base);
                let root = rec.app_hash;
                let follower = consensus::FollowerOrderer::new(replica_store.clone());
                let node_r = node::OrderedNode::resume(
                    host,
                    follower,
                    recovery,
                    rec.height.map(|height| host::FinalizedBlock {
                        height,
                        app_hash: root,
                    }),
                    rec.view_base,
                );
                replica_scheme = Some(replica_verifier(&namespace, &rec.participants));
                replica_orchestrator = Some(replica_orchestrator_at(
                    rec.epoch,
                    rec.view_base,
                    &rec.participants,
                    &rec.residents,
                ));
                replica_prev_ckpt = (ckpt.height, ckpt.oplog_pos);
                replica_epoch = rec.epoch;
                replica_view_base = rec.view_base;
                replica_watermark = Some(tip.saturating_sub(rec.view_base));
                resident_standing = rec
                    .residents
                    .iter()
                    .any(|k| k.as_slice() == me_bytes.as_slice());
                println!(
                    "[node {label}] replica: restart replayed the journal to {} \
                     (epoch {}, replayed {}, already-on-disk {}{}, app_hash={})",
                    tip,
                    rec.epoch,
                    rec.applied,
                    rec.skipped,
                    if rec.rolled_forward {
                        ", rolled 1 forward"
                    } else {
                        ""
                    },
                    hex(&root)
                );
                // the e2e / operator serve marker, truthful here too: the
                // node serves a verified boundary — the recovered tip.
                println!(
                    "[node {label}] resident: pre-synced boundary {tip} app_hash={}",
                    hex(&root)
                );
                heal_index(&index, node_r.host(), tip, &label).await;
                last_indexed_root = Some(root);
                serving = Some((tip, node_r));
            }
            let not_serving = |standing: bool| -> String {
                if standing {
                    "resident: no boundary pre-synced yet — retry shortly".into()
                } else {
                    "joining: redemption not landed yet — no state to serve".into()
                }
            };
            // The relay runtime owns caller holds, Forge pack fanout, and the
            // persisted resident sequence. This loop only supplies current
            // validator targets and consumes unclaimed pump replies.
            let mut resident_relay = relay_runtime::ResidentRelay::new(
                storage_for_sync.join("relay-submit-seq"),
                blobs.clone(),
            );
            // bridge the relay lane ONCE, before the park loop: the serve
            // window's select is torn down every 2s tick, and dropping the p2p
            // receiver's actor-backed `recv()` mid-flight could eat a delivered
            // reply. a bounded drop-on-full mpsc survives the tick losslessly;
            // a dropped reply degrades to the caller's honest SUBMIT_HOLD sweep.
            let (relay_bridge_tx, mut relay_ingress) =
                futures::channel::mpsc::channel::<(ed25519::PublicKey, Vec<u8>)>(64);
            context.child("relay_replies").spawn(move |_ctx| {
                let mut receiver = relay_rx;
                let mut bridge_tx = relay_bridge_tx;
                async move {
                    loop {
                        match receiver.recv().await {
                            Ok((peer, msg)) => {
                                let bytes: Vec<u8> = msg.into();
                                let _ = bridge_tx.try_send((peer, bytes));
                            }
                            Err(_) => return, // network shutdown — nothing to serve.
                        }
                    }
                }
            });
            // ---- the RESIDENT-tier pumps -----------------------------------
            //
            // the state-driven twins of the validator loop's announce pump and
            // reactor seam, adapted to a node that installs boundaries instead
            // of executing blocks (see resident_announce.rs /
            // resident_dispatch.rs). discovery here mirrors the validator
            // boot: the discovered tag set is BOTH what the worker can run and
            // what this node announces, so a resident announce can never claim
            // more than the host provides; a broken operator spec is a boot
            // error, not a silently dropped executor. execution is OFF-LOOP —
            // the same DispatchPool wiring the validator runs: the gate is
            // inline, the provider CLI runs on spawned children, completed
            // results come back over `resident_oracle_results` and are
            // drained by the park loop's pump pass, so a minutes-long run
            // never stalls the serve window, boundary follow, or promotion
            // detection.
            let resident_provider_set = capability_host::discover_with_dirs_and_output_sink(
                agent_dirs.clone(),
                run_output_sink(stream_hub.run_output()),
            )
            .unwrap_or_else(|e| panic!("capability specs failed to load: {e}"));
            let resident_capabilities = resident_provider_set.capabilities();
            let mut resident_announcer = resident_announce::ResidentAnnouncer::new(
                me_bytes.clone(),
                resident_capabilities,
            );
            let mut resident_duckdns_announcer =
                duckdns_node::ResidentAnnouncer::new(
                    me_bytes.clone(),
                    duckdns_announcements.clone(),
                );
            let (resident_pool, mut resident_oracle_results) = oracle_pool::build(
                &context,
                resident_provider_set,
                me_bytes.clone(),
                blobs.clone(),
                agent_provisioner.clone(),
                // the resident path keeps local-only resolution for now: its
                // statesync channel is owned by the park loop's client, so
                // the mesh fetch lane needs its own demux there (#298).
                None,
            );
            let mut resident_dispatch =
                resident_dispatch::ResidentDispatch::new(resident_pool, me_bytes.clone());
            let (boundary, host, floor) = loop {
                attempt += 1;
                if attempt > 900 && !resident_standing {
                    // ~30 minutes of 2s retries: parking forever is operator
                    // guidance territory, not a silent spin. (a RESIDENT
                    // holds standing indefinitely — that bail is gated off.)
                    eprintln!(
                        "[node {label}] FATAL: still no standing after {attempt} attempts — \
                         the invite may be spent or expired, or no member is reachable; \
                         ask for a fresh invite (manual fallback: `ducktape-node \
                         invite-accept {}`)",
                        hex_bytes(&me_bytes)
                    );
                    std::process::exit(1);
                }
                // the serve window: between manifest polls, pump the local
                // read surfaces from the last pre-synced boundary. the window
                // closes on EITHER a head wake (cert-lane traffic — a boundary
                // just sealed, fetch now) or the fallback tick; a knocking or
                // not-yet-serving joiner keeps the fast tick, a serving
                // resident stretches it since wakes carry the follow. (a sync
                // in flight below queues jobs here — bounded by the rpc
                // bridge's buffer and the listener's reply timeout — so every
                // answer reflects a whole boundary, never a torn one.)
                {
                    let fallback = if resident_standing && serving.is_some() {
                        RESIDENT_FALLBACK_POLL
                    } else {
                        JOINER_POLL
                    };
                    let tick = context.sleep(fallback).fuse();
                    futures::pin_mut!(tick);
                    loop {
                        futures::select_biased! {
                            job = rpc_ingress.next() => {
                                let Some((req, reply)) = job else { continue };
                                let resp = match req {
                                    // WITH standing AND a pre-synced boundary, a
                                    // write leaves here: sign it, relay to a
                                    // validator, HOLD this caller's reply keyed by
                                    // the frame id (answered on the relay Reply arm
                                    // or the sweep). the refusal stays for the
                                    // un-standing / not-yet-serving cases.
                                    RpcRequest::Submit { target, payload_hex } => {
                                        if !resident_standing || serving.is_none() {
                                            RpcReply::err(not_serving(resident_standing))
                                        } else {
                                            match unhex(&payload_hex) {
                                                Ok(payload) => match resident_relay.submit(
                                                    &signer,
                                                    &announce_targets,
                                                    &mut relay_tx,
                                                    target,
                                                    payload,
                                                    relay_runtime::ResidentHold::Rpc(reply.clone()),
                                                ) {
                                                    Ok(_) => {
                                                        continue;
                                                    }
                                                    Err((_hold, e)) => RpcReply::err(e),
                                                },
                                                Err(e) => {
                                                    RpcReply::err(format!("bad payload_hex: {e}"))
                                                }
                                            }
                                        }
                                    }
                                    RpcRequest::Query { target, req_hex } => match &serving {
                                        Some((_, node_r)) => match unhex(&req_hex) {
                                            Ok(req_bytes) => {
                                                match node_r.host().query(&target, &req_bytes).await
                                                {
                                                    Ok(bytes) => RpcReply {
                                                        reply_hex: Some(hex_bytes(&bytes)),
                                                        ..RpcReply::ok()
                                                    },
                                                    Err(e) => RpcReply::err(format!(
                                                        "query failed: {e}"
                                                    )),
                                                }
                                            }
                                            Err(e) => RpcReply::err(format!("bad req_hex: {e}")),
                                        },
                                        None => RpcReply::err(not_serving(resident_standing)),
                                    },
                                    RpcRequest::Status => match &serving {
                                        Some((height, node_r)) => {
                                            let mut modules = std::collections::BTreeMap::new();
                                            for m in MODULE_IDS {
                                                if let Some(root) = node_r.host().module_root(m) {
                                                    modules.insert(m.to_string(), hex(&root));
                                                }
                                            }
                                            RpcReply {
                                                status: Some(RpcStatus {
                                                    height: Some(*height),
                                                    app_hash: hex(&node_r.host().app_hash()),
                                                    modules,
                                                }),
                                                ..RpcReply::ok()
                                            }
                                        }
                                        None => RpcReply::err(not_serving(resident_standing)),
                                    },
                                    RpcRequest::JoinRequests => RpcReply::err(
                                        "this node is not a member — join requests queue on \
                                         validators",
                                    ),
                                    RpcRequest::Shutdown => {
                                        // a resident writes no checkpoint — nothing to
                                        // flush; a restart parks straight back here.
                                        let _ = reply.send(RpcReply::ok());
                                        println!(
                                            "[node {label}] shutdown requested via rpc — exiting"
                                        );
                                        std::process::exit(0);
                                    }
                                };
                                let _ = reply.send(resp);
                            }
                            cmd = http_ingress.next() => {
                                let Some(cmd) = cmd else { continue };
                                match cmd {
                                    // `origin` is the caller's CLAIMED submitter — but
                                    // this lane signs frames with THIS node's identity
                                    // (authorship = status.publicKey), so it is ignored.
                                    // WITH standing AND a boundary, relay and HOLD the
                                    // oneshot keyed by the frame id; otherwise refuse.
                                    noded::NodeCommand::Submit {
                                        target,
                                        payload,
                                        origin: _,
                                        reply,
                                    } => {
                                        if !resident_standing || serving.is_none() {
                                            let _ =
                                                reply.send(Err(not_serving(resident_standing)));
                                        } else {
                                            match resident_relay.submit(
                                                &signer,
                                                &announce_targets,
                                                &mut relay_tx,
                                                target,
                                                payload,
                                                relay_runtime::ResidentHold::Http(reply),
                                            ) {
                                                Ok(_) => {}
                                                Err((hold, e)) => hold.fail(e),
                                            }
                                        }
                                    }
                                    noded::NodeCommand::Query { target, req, reply } => {
                                        let result = match &serving {
                                            Some((_, node_r)) => node_r
                                                .host()
                                                .query(&target, &req)
                                                .await
                                                .map_err(|e| e.to_string()),
                                            None => Err(not_serving(resident_standing)),
                                        };
                                        let _ = reply.send(result);
                                    }
                                    noded::NodeCommand::Status { reply } => {
                                        // pre-first-sync the surface still answers (the
                                        // app's liveness heartbeat): a zeroed status is
                                        // honest — no boundary is served yet.
                                        let (height, app_hash, modules) = match &serving {
                                            Some((height, node_r)) => (
                                                *height,
                                                hex(&node_r.host().app_hash()),
                                                MODULE_IDS
                                                    .iter()
                                                    .map(|m| noded::ModuleStatus {
                                                        id: (*m).into(),
                                                        root: node_r
                                                            .host()
                                                            .module_root(m)
                                                            .map(|r| hex(&r))
                                                            .unwrap_or_default(),
                                                        category: noded::ModuleCategory::of(m),
                                                    })
                                                    .collect(),
                                            ),
                                            None => (0, String::new(), Vec::new()),
                                        };
                                        let _ = reply.send(noded::NodeStatus {
                                            version: env!("CARGO_PKG_VERSION").into(),
                                            app_hash,
                                            height,
                                            modules,
                                            public_key: status_public_key.clone(),
                                        });
                                    }
                                    noded::NodeCommand::Metrics { reply } => {
                                        let _ = reply.send(context.encode());
                                    }
                                }
                            }
                            // a validator's answer for a frame we relayed: match it
                            // to the held caller by frame id and release the reply.
                            // an unknown id (already swept, or a stray) drops.
                            answer = relay_ingress.next() => {
                                let Some((peer, bytes)) = answer else { continue };
                                let Ok(msg) = relay::decode_msg(&bytes) else { continue };
                                let Some((frame_id, outcome)) =
                                    resident_relay.on_message(peer, msg, &mut relay_tx)
                                else {
                                    continue;
                                };
                                // Unclaimed final replies belong to the
                                // resident-owned capability/dispatch pumps.
                                let applied =
                                    matches!(outcome, relay::RelayOutcome::Applied { .. });
                                if let Some(ok) = resident_announcer.on_reply(&frame_id, applied) {
                                    if ok {
                                        println!(
                                            "[node {label}] resident: announced capabilities {:?}",
                                            resident_announcer.capabilities()
                                        );
                                    } else {
                                        eprintln!(
                                            "[node {label}] resident: capability announce did not \
                                             apply ({outcome:?}) - will retry"
                                        );
                                    }
                                } else if let Some(ok) = resident_duckdns_announcer
                                    .on_reply(&frame_id, applied)
                                {
                                    if ok {
                                        println!(
                                            "[node {label}] resident: announced DuckDNS services \
                                             {:?}",
                                            resident_duckdns_announcer.announcements()
                                        );
                                    } else {
                                        eprintln!(
                                            "[node {label}] resident: DuckDNS announce did not \
                                             apply ({outcome:?}) - will retry"
                                        );
                                    }
                                } else if let Some((saga_id, attempt)) =
                                    resident_dispatch.on_reply(&frame_id, applied)
                                {
                                    if applied {
                                        println!(
                                            "[node {label}] resident: dispatch result for saga \
                                             {saga_id} attempt {attempt} applied"
                                        );
                                    } else {
                                        eprintln!(
                                            "[node {label}] resident: dispatch result for saga \
                                             {saga_id} attempt {attempt} did not apply \
                                             ({outcome:?}) - will retry while leased"
                                        );
                                    }
                                }
                            }
                            // a raw certificate arrived. FOLDING replica:
                            // decode, plan against the watermark, admit
                            // through the verified follower gate (backfilling
                            // any parent-linkage gap over the Frames lane
                            // first), then close the window so the post-
                            // window pass drains the fold. NOT yet folding:
                            // fall through — the coalesced wake below carries
                            // the old poll-now semantics.
                            cert = cert_bridge.next() => {
                                let Some(raw) = cert else { continue };
                                let (Some((_, node_r)), Some(scheme)) =
                                    (serving.as_mut(), replica_scheme.as_ref())
                                else {
                                    continue;
                                };
                                let Some(anchor) = replica::anchor_from_cert_msg(scheme, &raw)
                                else {
                                    continue;
                                };
                                if anchor.epoch != replica_epoch {
                                    // another epoch's certificate: our epoch
                                    // ended. the manifest fallback observes
                                    // the new epoch and descends/re-ascends.
                                    break;
                                }
                                if let replica::FoldStep::Stale =
                                    replica::plan_fold(replica_watermark, &anchor)
                                {
                                    continue;
                                }
                                if let replica::FoldStep::BackfillThenObserve {
                                    after_view,
                                    up_to_view,
                                } = replica::plan_fold(replica_watermark, &anchor)
                                    && let Err(e) = replica_backfill(
                                        &client,
                                        node_r,
                                        replica_view_base,
                                        (after_view, up_to_view),
                                        &mut replica_watermark,
                                        &mut pending_seal_checks,
                                        &label,
                                    )
                                    .await
                                {
                                    println!(
                                        "[node {label}] replica: backfill ({after_view}, \
                                         {up_to_view}] unavailable: {e} — retrying on the \
                                         next certificate"
                                    );
                                    break;
                                }
                                match node_r.orderer_mut().observe_finalization(
                                    &mut rand::rngs::OsRng,
                                    scheme,
                                    &anchor.finalization,
                                ) {
                                    Ok(consensus::Observed::Admitted(view)) => {
                                        replica_watermark = Some(view);
                                        // fold in the post-window drain pass.
                                        break;
                                    }
                                    Ok(consensus::Observed::Stale(_)) => continue,
                                    Ok(consensus::Observed::Unresolvable(view)) => {
                                        // payload gossip missed this block's
                                        // bytes and the follower runs without
                                        // a resolver: fetch the frame itself
                                        // over the Frames lane (seal
                                        // cross-checked post-fold), which
                                        // also admits it.
                                        if let Err(e) = replica_backfill(
                                            &client,
                                            node_r,
                                            replica_view_base,
                                            (replica_watermark.unwrap_or(0), view),
                                            &mut replica_watermark,
                                            &mut pending_seal_checks,
                                            &label,
                                        )
                                        .await
                                        {
                                            println!(
                                                "[node {label}] replica: unresolvable view \
                                                 {view} backfill failed: {e} — retrying on \
                                                 the next certificate"
                                            );
                                        }
                                        break;
                                    }
                                    Err(e) => {
                                        // quorum verification failed: a lying
                                        // certificate source. drop it loudly.
                                        eprintln!(
                                            "[node {label}] replica: certificate refused: {e}"
                                        );
                                        continue;
                                    }
                                }
                            },
                            // a sealed boundary's certificate arrived: stop
                            // serving the window and go fetch the manifest.
                            // (None — every drain gone — only happens at mesh
                            // shutdown; fall through to the tick's exit.)
                            wake = head_wake.next() => if wake.is_some() { break },
                            _ = tick => break,
                        }
                    }
                }
                // ---- the replica drain pass ------------------------------
                //
                // fold whatever the gate released, then the validator drain's
                // per-block side effects, minus its validator-only concerns
                // (submit holds, engine orchestration): the seal cross-check
                // for backfilled heights, the per-block derived-index fold
                // (no more healing), the explorer row, the ws block event,
                // the finalization floor, and the checkpoint cadence.
                if let Some((served_height, node_r)) = serving.as_mut() {
                    if let Err(e) = node_r.drain_delivered().await {
                        eprintln!("[node {label}] FATAL: replica fold: {e}");
                        std::process::exit(1);
                    }
                    let drained = node_r.take_drained();
                    // System-injection traces, merged per height after the
                    // members' dispatches — the same row order the validator
                    // drain and every replay path derive.
                    let mut system_dispatches: std::collections::BTreeMap<
                        u64,
                        Vec<host::DispatchRecord>,
                    > = node_r.take_system_dispatches().into_iter().collect();
                    let mut gi = 0;
                    while gi < drained.len() {
                        let height = drained[gi].height;
                        let mut block_dispatches: Vec<host::DispatchRecord> = Vec::new();
                        let mut block_ops: Vec<noded::RootOp> = Vec::new();
                        let mut block_hash: Option<node::FrameId> = None;
                        let mut block_app_hash: Option<StateRoot> = None;
                        let mut sealed_hash: Option<StateRoot> = None;
                        while gi < drained.len() && drained[gi].height == height {
                            let d = &drained[gi];
                            gi += 1;
                            if d.disposition == node::Disposition::Discarded {
                                continue;
                            }
                            sealed_hash = Some(d.app_hash);
                            if let (node::Disposition::Applied, Some(op)) =
                                (&d.disposition, &d.op)
                            {
                                block_dispatches.extend(op.dispatches.iter().cloned());
                            }
                            if let Some(op) = &d.op
                                && op.target != NOP_TARGET
                            {
                                let disposition = match d.disposition {
                                    node::Disposition::Applied => {
                                        noded::BlockDisposition::Applied
                                    }
                                    node::Disposition::Rejected => {
                                        noded::BlockDisposition::Rejected
                                    }
                                    node::Disposition::Discarded => continue,
                                };
                                if block_hash.is_none() {
                                    block_hash = Some(d.id);
                                    block_app_hash = Some(d.app_hash);
                                }
                                block_ops.push(explorer_root_op(
                                    &blobs,
                                    &op.origin,
                                    &op.target,
                                    &op.payload,
                                    &op.dispatches,
                                    disposition,
                                ));
                            }
                        }
                        // System-injection dispatches index after the
                        // members' — see the validator drain's twin merge.
                        if let Some(sys) = system_dispatches.remove(&height) {
                            block_dispatches.extend(sys);
                        }
                        // a BACKFILLED height's trust is the served seal:
                        // what our fold produced must match it exactly, or
                        // this replica has diverged from the quorum's fold.
                        if let Some((_, served_hash, served_roots)) =
                            pending_seal_checks.remove(&height)
                            && sealed_hash.is_some_and(|h| h != served_hash)
                        {
                            // name the diverging module(s) — the one lead an
                            // operator (or the next debugger) needs first.
                            for (module, served_root) in &served_roots {
                                let ours = node_r.host().module_root(module);
                                if ours.as_ref() != Some(served_root) {
                                    eprintln!(
                                        "[node {label}] replica: diverged module={module} \
                                         served={} ours={}",
                                        hex(served_root),
                                        ours.map(|r| hex(&r)).unwrap_or_else(|| "none".into())
                                    );
                                }
                            }
                            eprintln!(
                                "[node {label}] FATAL: backfilled height {height} folded to \
                                 {} but the quorum sealed {} — state diverged",
                                hex(&sealed_hash.expect("checked above")),
                                hex(&served_hash)
                            );
                            std::process::exit(1);
                        }
                        let record = (!block_ops.is_empty()).then(|| {
                            noded::block_row(&noded::BlockRecord {
                                height,
                                hash: block_hash
                                    .map(|h| noded::hex_bytes(&h))
                                    .unwrap_or_default(),
                                commit_hash: block_app_hash
                                    .map(|h| hex(&h))
                                    .unwrap_or_default(),
                                ops: block_ops,
                            })
                        });
                        let ops = indexer::BlockOps {
                            record,
                            ..noded::index_block_ops(height, height, &block_dispatches)
                        };
                        if let Err(err) = index.apply_block(&ops) {
                            eprintln!(
                                "[node {label}] replica index apply failed at height \
                                 {height}: {err} — wipe <storage>/index to rebuild"
                            );
                        }
                        if let Some(root) = sealed_hash {
                            stream_hub.publish_block(height, hex(&root));
                            last_indexed_root = Some(root);
                        }
                        *served_height = height;
                        blocks_since_checkpoint += 1;
                    }
                    // ---- valset orchestration (the replica mirror) --------
                    //
                    // observe → ceiling → cutover, exactly the validator
                    // drain's discipline. the CEILING is correctness, not
                    // bookkeeping: a frame finalized before the cutover but
                    // landing after it is DISCARDED by every validator, and
                    // a replica without the ceiling would apply it — silent
                    // divergence. the cutover SWAPS the follower orderer
                    // (journaling Record::Cutover) where a validator
                    // respawns an engine; the manifest-epoch descend remains
                    // the safety net for anything this mirror missed.
                    if !drained.is_empty()
                        && let Some(orch) = replica_orchestrator.as_mut()
                    {
                        let folded_view = served_height.saturating_sub(replica_view_base);
                        let members_raw = read_valset_members(node_r.host()).await;
                        let observed: Vec<ed25519::PublicKey> = members_raw
                            .iter()
                            .filter_map(|k| ed25519::PublicKey::decode(k.as_slice()).ok())
                            .collect();
                        let residents_raw = read_valset_residents(node_r.host()).await;
                        let observed_residents: Vec<ed25519::PublicKey> = residents_raw
                            .iter()
                            .filter_map(|k| ed25519::PublicKey::decode(k.as_slice()).ok())
                            .collect();
                        if let consensus::ObservationOutcome::Scheduled(cutover) = orch
                            .observe_members(
                                folded_view,
                                observed.iter().cloned(),
                                observed_residents.iter().cloned(),
                            )
                        {
                            println!(
                                "[node {label}] replica: membership change observed at view {} \
                                 — cutover to epoch {} at view {}",
                                cutover.observed_view(),
                                cutover.next_epoch(),
                                cutover.cutover_view()
                            );
                            node_r.set_view_ceiling(cutover.cutover_view());
                        }
                        let boundary_upgrade = read_upgrade_state(node_r.host()).await;
                        if let Some(pending) = &boundary_upgrade.pending
                            && let consensus::ObservationOutcome::Scheduled(cutover) =
                                orch.observe_upgrade(folded_view, pending.activation_height)
                        {
                            println!(
                                "[node {label}] replica: upgrade '{}' armed — cutover to epoch \
                                 {} at view {} (activation height {})",
                                pending.name,
                                cutover.next_epoch(),
                                cutover.cutover_view(),
                                pending.activation_height
                            );
                            node_r.set_view_ceiling(cutover.cutover_view());
                        }
                        if let Some(plan) = orch.respawn_if_due(
                            folded_view,
                            observed,
                            observed_residents,
                            boundary_upgrade,
                        ) {
                            let members = plan.valset().consensus_members();
                            let member_bytes: Vec<Vec<u8>> =
                                members.iter().map(|k| k.as_ref().to_vec()).collect();
                            let plan_residents: Vec<ed25519::PublicKey> = plan
                                .valset()
                                .transport_members()
                                .difference(members)
                                .cloned()
                                .collect();
                            let plan_resident_bytes: Vec<Vec<u8>> = plan_residents
                                .iter()
                                .map(|k| k.as_ref().to_vec())
                                .collect();
                            // transport first, exactly like the validator:
                            // the new epoch's mesh must admit its members.
                            oracle.track(
                                plan.epoch(),
                                joiner_epoch_mesh(&peers, &member_bytes, &plan_resident_bytes),
                            );
                            last_tracked = plan.epoch();
                            // the follower swap: same OrderedNode, fresh
                            // orderer, cutover journaled — the epoch-local
                            // view clock restarts with the new base.
                            let follower =
                                consensus::FollowerOrderer::new(replica_store.clone());
                            if let Err(e) = node_r
                                .cutover(
                                    follower,
                                    plan.epoch(),
                                    plan.cutover_app_height(),
                                    &member_bytes,
                                    &plan_resident_bytes,
                                )
                                .await
                            {
                                eprintln!(
                                    "[node {label}] FATAL: replica cutover journal write: {e}"
                                );
                                std::process::exit(1);
                            }
                            node_r.host_mut().set_active_version(plan.boundary_version());
                            replica_scheme =
                                Some(replica_verifier(&namespace, &member_bytes));
                            replica_epoch = plan.epoch();
                            replica_view_base = plan.cutover_app_height();
                            replica_watermark = None;
                            pending_seal_checks.clear();
                            // force a checkpoint on the next pass — the
                            // validator writes one immediately post-cutover
                            // for the same restart-boundary reason.
                            blocks_since_checkpoint = checkpoint_blocks;
                            println!(
                                "[node {label}] replica: epoch cutover to {} at base {} — \
                                 follower swapped in-loop",
                                plan.epoch(),
                                plan.cutover_app_height()
                            );
                        }
                    }
                    // persist the finalization floor once everything at or
                    // below it has drained — cert first, gate second, same
                    // ordering proof as the validator drain.
                    if let Some((view, cert)) = node_r.orderer().latest_finalization()
                        && view != 0
                        && node_r.orderer().unreleased_len() == 0
                    {
                        let height = replica_view_base + view;
                        if last_cert_height.is_none_or(|h| height > h) {
                            let fc = recovery::FloorCert {
                                epoch: replica_epoch,
                                height,
                                cert,
                            };
                            match node_r.sink_mut().write_floor_cert(&fc).await {
                                Ok(()) => last_cert_height = Some(height),
                                Err(e) => eprintln!(
                                    "[node {label}] replica floor cert write failed \
                                     (will retry): {e}"
                                ),
                            }
                        }
                    }
                    // periodic checkpoint at the folded tip: a restart
                    // recovers here and replays only the suffix — exactly a
                    // validator restart. participants/residents read from the
                    // FOLDED state (the same projection the checkpoint's
                    // epoch coordinates describe). journal pruning stays the
                    // validator's concern for now (a replica's journal prunes
                    // at its next ascension checkpoint).
                    if blocks_since_checkpoint >= checkpoint_blocks
                        && let Some(f) = node_r.finalized()
                    {
                        let pos = node_r.sink_mut().oplog_pos().await;
                        let (cv, pu) = read_upgrade_version_fields(node_r.host()).await;
                        let members = read_valset_members(node_r.host()).await;
                        let residents = read_valset_residents(node_r.host()).await;
                        let captured = Manifest::capture(
                            node_r.host(),
                            Some(f.height),
                            replica_epoch,
                            replica_view_base,
                            members,
                            residents,
                            None,
                            cv,
                            pu,
                            pos,
                            1,
                        );
                        match captured {
                            Ok(ckpt) => match node_r.sink_mut().write_manifest(&ckpt).await {
                                Ok(()) => {
                                    // prune the journal below the PREVIOUS
                                    // checkpoint once the persisted floor
                                    // passed it — the validator's exact
                                    // prune discipline. without this a
                                    // long-lived replica's journal grows
                                    // without bound (pruned frames must
                                    // never be needed to resolve a
                                    // re-reported finalization; the floor
                                    // gate guarantees it).
                                    let floor_passed = matches!(
                                        node_r.sink_mut().floor_cert(),
                                        Ok(Some(fc))
                                            if replica_prev_ckpt
                                                .0
                                                .is_none_or(|h| fc.height >= h)
                                    );
                                    if floor_passed
                                        && let Err(e) = node_r
                                            .sink_mut()
                                            .prune_oplog(replica_prev_ckpt.1)
                                            .await
                                    {
                                        eprintln!(
                                            "[node {label}] replica oplog prune failed: {e}"
                                        );
                                    }
                                    replica_prev_ckpt = (ckpt.height, pos);
                                    blocks_since_checkpoint = 0;
                                }
                                Err(e) => eprintln!(
                                    "[node {label}] replica checkpoint write failed \
                                     (will retry): {e}"
                                ),
                            },
                            Err(e) => eprintln!(
                                "[node {label}] replica checkpoint capture failed \
                                 (will retry): {e}"
                            ),
                        }
                    }
                }
                resident_relay.expire(std::time::Instant::now());
                // a FOLDING replica's window closes per certificate; this
                // poll is only the fallback DETECTION lane now (standing
                // detection pre-ascension; promotion, cutover, and revocation
                // detection after). it reads tip COORDINATES — membership,
                // epoch, height — which the server answers from loop-owned
                // state with no capture, no lease, and no floor-cert gate;
                // the transitions that consume an actual boundary (ascension,
                // promotion) fetch a full manifest inside their branch. pace
                // it on an ABSOLUTE deadline — the window's own tick restarts
                // per close and would never fire under steady cert traffic —
                // so a fleet of replicas doesn't besiege the serve window per
                // block, yet detection stays bounded by the fallback cadence.
                if serving.is_some() && std::time::Instant::now() < next_manifest_fetch {
                    continue;
                }
                next_manifest_fetch = std::time::Instant::now() + RESIDENT_FALLBACK_POLL;
                let tip = match fetch_tip_coords(&client).await {
                    Ok(tip) => tip,
                    Err(e) => {
                        let retry = joiner_manifest_fetch_retry(&label, resident_standing, &e);
                        println!("{}", retry.log_line);
                        if retry.announce {
                            send_announce(&announce_targets, attempt);
                        }
                        continue;
                    }
                };
                // follow the mesh rotation while parked. the participant
                // list is an unverified serving hint — the union with the
                // descriptor mesh keeps the real members reachable, and
                // promotion re-derives everything from verified state.
                if tip.epoch > last_tracked {
                    if tip.epoch >= EPOCH_CHANNEL_BANK {
                        println!(
                            "[node {label}] warning: the network is at epoch {} — beyond this \
                             process's pre-registered channel bank ({EPOCH_CHANNEL_BANK}); \
                             expect reconnect churn while parked",
                            tip.epoch
                        );
                    }
                    oracle.track(
                        tip.epoch,
                        joiner_epoch_mesh(&peers, &tip.participants, &tip.residents),
                    );
                    last_tracked = tip.epoch;
                }
                // drive the reachability plane's standby role off the
                // manifest: membership and resident standing come from the
                // synced boundary, whose height doubles as the plane's
                // freshness clock (the same app-height regime the members'
                // ViewTicks run — within the advert TTL's generous window).
                // Nothing is sent before standing: no member would admit the
                // gossip yet.
                if let Some(cmd) = &reach_cmd
                    && tip.residents.iter().any(|k| k == &me_bytes)
                {
                    // NON-BLOCKING sends throughout: the plane is not this
                    // loop's dependency. a shed ViewTick is one beat of
                    // advert staleness (the next poll carries a fresher one);
                    // a refused Retarget retries naturally — the epoch latch
                    // below only advances when the send is taken.
                    let clock = tip.view_base.max(tip.height);
                    let _ = cmd.try_send(reachability::ReachabilityCommand::ViewTick(clock));
                    if last_plane_epoch != Some(tip.epoch) {
                        let members: Vec<ed25519::PublicKey> = tip
                            .participants
                            .iter()
                            .filter_map(|k| ed25519::PublicKey::decode(k.as_slice()).ok())
                            .collect();
                        let standbys: Vec<ed25519::PublicKey> = tip
                            .residents
                            .iter()
                            .filter_map(|k| ed25519::PublicKey::decode(k.as_slice()).ok())
                            .collect();
                        if cmd
                            .try_send(reachability::ReachabilityCommand::Retarget(
                                reachability::MeshEpochEvent {
                                    epoch: tip.epoch,
                                    members,
                                    standbys,
                                    current_view: clock,
                                },
                            ))
                            .is_ok()
                        {
                            last_plane_epoch = Some(tip.epoch);
                        }
                    }
                }
                if !tip.participants.iter().any(|k| k == &me_bytes) {
                    // the tip names the CURRENT members — better announce
                    // targets than the genesis descriptor's list.
                    let current: Vec<ed25519::PublicKey> = tip
                        .participants
                        .iter()
                        .filter_map(|k| ed25519::PublicKey::decode(k.as_slice()).ok())
                        .collect();
                    if !current.is_empty() {
                        announce_targets = current;
                    }
                    if serving.is_some() && tip.epoch > replica_epoch {
                        // the network cut over to a new epoch: our follower's
                        // verifier and fetch lane are the old epoch's, so its
                        // certs stopped verifying here. DESCEND — drop the
                        // node (journal checkpointed on cadence), reopen the
                        // journal handle — and re-ascend at the new epoch's
                        // boundary below. the in-loop follower swap
                        // (node.cutover, no re-bootstrap) is the promotion
                        // collapse's concern (phase 3).
                        println!(
                            "[node {label}] replica: epoch cutover {} -> {} — re-ascending",
                            replica_epoch, tip.epoch
                        );
                        serving = None;
                        replica_scheme = None;
                        replica_orchestrator = None;
                        recovery_slot =
                            Some(reopen_recovery(&context, &mut recovery_reopens, &label).await);
                    }
                    if tip.residents.iter().any(|k| k == &me_bytes) {
                        if !resident_standing {
                            resident_standing = true;
                            println!(
                                "[node {label}] resident: standing granted — following \
                                 boundaries and serving local reads"
                            );
                        }
                        // RESIDENT standing (staged admission): granted, so
                        // stop knocking — and ASCEND to the replica pipeline
                        // (unified-node phase 2): bootstrap ONE boundary,
                        // journal it as this node's recovery-boot base, fold
                        // the frame suffix to the live tip through that same
                        // journal, then follow the head by folding finalized
                        // frames exactly like a validator — the boundary
                        // re-install loop is gone. reads serve from the
                        // node's host through the serve window above, and
                        // `promote` finds a node already at head.
                        if serving.is_none() {
                            // ascension consumes the BOUNDARY itself — module
                            // entries to sync and the floor certificate to
                            // verify — so this transition (and only this
                            // transition) rides the full Manifest lane.
                            let m = match fetch_manifest(&client).await {
                                Ok(m) => m,
                                Err(e) => {
                                    let retry = joiner_manifest_fetch_retry(
                                        &label,
                                        resident_standing,
                                        &e,
                                    );
                                    println!("{}", retry.log_line);
                                    continue;
                                }
                            };
                            if let Err(e) = m.preflight(MAX_PROTOCOL_VERSION) {
                                eprintln!(
                                    "[node {label}] FATAL: cannot observe this network — {e}"
                                );
                                std::process::exit(1);
                            }
                            println!(
                                "[node {label}] replica: bootstrapping at boundary {} ({} modules)",
                                m.height,
                                m.entries.len()
                            );
                            match sync_all_modules(
                                &context,
                                &client,
                                &m,
                                NetworkBindings {
                                    invite: &namespace,
                                    identity_chain_id: &identity_chain_id,
                                    duckdns_chain_id: &duckdns_chain_id,
                                },
                                SyncSubstrates {
                                    forge_repo: &forge_repo,
                                    duckfs_dir: &duckfs_dir,
                                    blobs: blobs.clone(),
                                },
                                attempt,
                            )
                            .await
                            {
                                Ok(mut host) => {
                                    // the boundary's floor must verify (real
                                    // quorum signatures) before it becomes
                                    // this journal's genesis — the same gate
                                    // promotion runs.
                                    let floor = match verify_manifest_floor(&namespace, &m) {
                                        Ok(cert) => cert.map(|cert| recovery::FloorCert {
                                            epoch: m.epoch,
                                            height: m.height,
                                            cert,
                                        }),
                                        Err(e) => {
                                            println!(
                                                "[node {label}] replica: boundary {} floor \
                                                 refused ({e}) — retrying",
                                                m.height
                                            );
                                            continue;
                                        }
                                    };
                                    let mut recovery = recovery_slot
                                        .take()
                                        .expect("the journal slot is filled whenever serving is None");
                                    let ckpt_pos = write_boundary_checkpoint(
                                        &mut recovery,
                                        &host,
                                        &m,
                                        &floor,
                                        &label,
                                        "replica_checkpoint",
                                    )
                                    .await;
                                    replica_prev_ckpt = (Some(m.height), ckpt_pos);
                                    // close the boundary -> live-tip gap
                                    // through the SAME journal a validator
                                    // restart would replay; every served
                                    // frame is seal-verified inside.
                                    let caught = match catch_up_post_reboot_frames(
                                        &client,
                                        &mut recovery,
                                        &mut host,
                                        None,
                                        m.height,
                                        POST_REBOOT_CATCHUP_MAX_ITERS,
                                    )
                                    .await
                                    {
                                        Ok(c) => c,
                                        Err(PostRebootCatchupError::Fatal(e)) => {
                                            eprintln!(
                                                "[node {label}] FATAL: replica suffix fold: {e}"
                                            );
                                            std::process::exit(1);
                                        }
                                        Err(e) => {
                                            println!(
                                                "[node {label}] replica: suffix fold at \
                                                 boundary {} unavailable ({e:?}) — re-bootstrapping",
                                                m.height
                                            );
                                            recovery_slot = Some(recovery);
                                            continue;
                                        }
                                    };
                                    let tip = caught.to_height.max(m.height);
                                    // seed the shared store with the folded
                                    // suffix: peers' resolvers can fetch these
                                    // from us, and a re-reported cert for a
                                    // just-folded height resolves locally.
                                    for bytes in &caught.frame_bytes {
                                        replica_store.put(bytes.clone());
                                    }
                                    let root = host.app_hash();
                                    // the fold pipeline: the follower orderer
                                    // in the engine's seat of the SAME
                                    // OrderedNode a validator drains, this
                                    // journal as its sink. resolver-less by
                                    // design (see the lane wiring above): a
                                    // store miss surfaces as Unresolvable and
                                    // the driver backfills over the Frames
                                    // lane.
                                    let follower =
                                        consensus::FollowerOrderer::new(replica_store.clone());
                                    let node_r = node::OrderedNode::resume(
                                        host,
                                        follower,
                                        recovery,
                                        Some(host::FinalizedBlock {
                                            height: tip,
                                            app_hash: root,
                                        }),
                                        m.view_base,
                                    );
                                    replica_scheme =
                                        Some(replica_verifier(&namespace, &m.participants));
                                    replica_orchestrator = Some(replica_orchestrator_at(
                                        m.epoch,
                                        m.view_base,
                                        &m.participants,
                                        &m.residents,
                                    ));
                                    replica_epoch = m.epoch;
                                    replica_view_base = m.view_base;
                                    replica_watermark = Some(tip.saturating_sub(m.view_base));
                                    blocks_since_checkpoint = 0;
                                    pending_seal_checks.clear();
                                    // the stable serve marker: "this node now
                                    // serves a verified boundary" — the line
                                    // the e2e suite (and operators) key on,
                                    // truthful under both the old re-install
                                    // model and the fold pipeline.
                                    println!(
                                        "[node {label}] resident: pre-synced boundary {} \
                                         app_hash={}",
                                        tip,
                                        hex(&root)
                                    );
                                    println!(
                                        "[node {label}] replica: following the head from {} \
                                         (epoch {}, app_hash={})",
                                        tip,
                                        m.epoch,
                                        hex(&root)
                                    );
                                    // the derived tier starts exact at the
                                    // ascension tip; per-block folds keep it
                                    // current from here (no more healing).
                                    if last_indexed_root.as_ref() != Some(&root) {
                                        heal_index(&index, node_r.host(), tip, &label).await;
                                        if let Err(err) = index.apply_block_record(
                                            tip,
                                            boundary_block_row(tip, &root),
                                        ) {
                                            eprintln!(
                                                "[node {label}] replica: explorer row at \
                                                 ascension tip {tip} refused: {err}"
                                            );
                                        }
                                        stream_hub.publish_block(tip, hex(&root));
                                        last_indexed_root = Some(root);
                                    }
                                    serving = Some((tip, node_r));
                                }
                                Err(e) => println!(
                                    "[node {label}] replica bootstrap at boundary {} failed: {e}",
                                    m.height
                                ),
                            }
                        }
                        // ---- the resident-tier pumps, one pass per poll ----
                        //
                        // both read the served boundary (committed state) and
                        // write through the relay lane — the resident's only
                        // write path. state-driven and idempotent like their
                        // validator-loop twins: quiet once committed state
                        // matches, deadline-based retry over the lossy lane.
                        if let Some((_, node_r)) = &serving {
                            let host = node_r.host();
                            let now = std::time::Instant::now();
                            // CAPABILITY ANNOUNCE (resident tier): mirrors the
                            // validator pump, including the config gate — an
                            // `announce_capabilities = false` resident stays an
                            // accept-lane-only provider and never enters a
                            // tag's rendezvous pool.
                            if announce_capabilities
                                && let Some(msg) =
                                    resident_announcer.maybe_announce(host, now).await
                            {
                                match resident_relay.submit_unheld(
                                    &signer,
                                    &announce_targets,
                                    &mut relay_tx,
                                    msg.target,
                                    msg.payload,
                                ) {
                                    Ok(id) => {
                                        resident_announcer.sent(id, now);
                                        println!(
                                            "[node {label}] resident: capability announce \
                                             relayed ({:?})",
                                            resident_announcer.capabilities()
                                        );
                                    }
                                    Err(e) => {
                                        resident_announcer.send_failed();
                                        eprintln!(
                                            "[node {label}] resident: capability announce \
                                             relay failed: {e}"
                                        );
                                    }
                                }
                            }
                            // DUCKDNS ANNOUNCE (resident tier): an empty list
                            // is meaningful and clears stale declarations.
                            if let Some(msg) = resident_duckdns_announcer
                                .maybe_announce(host, now)
                                .await
                            {
                                match resident_relay.submit_unheld(
                                    &signer,
                                    &announce_targets,
                                    &mut relay_tx,
                                    msg.target,
                                    msg.payload,
                                ) {
                                    Ok(id) => {
                                        resident_duckdns_announcer.sent(id, now);
                                        println!(
                                            "[node {label}] resident: DuckDNS announce relayed \
                                             ({:?})",
                                            resident_duckdns_announcer.announcements()
                                        );
                                    }
                                    Err(e) => {
                                        resident_duckdns_announcer.send_failed();
                                        eprintln!(
                                            "[node {label}] resident: DuckDNS announce relay \
                                             failed: {e}"
                                        );
                                    }
                                }
                            }
                            // DISPATCH EXECUTION (resident tier): serve the
                            // saga attempts leased to this key, so an announced
                            // resident never stalls an assignment. completed
                            // off-loop runs are drained FIRST (they become due
                            // relay sends in this same pass); the tick itself
                            // only gates and spawns — it never awaits a
                            // provider.
                            while let Ok(msg) = resident_oracle_results.try_recv() {
                                resident_dispatch.completed(msg);
                            }
                            for (key, msg) in resident_dispatch.tick(host, now).await {
                                match resident_relay.submit_unheld(
                                    &signer,
                                    &announce_targets,
                                    &mut relay_tx,
                                    msg.target,
                                    msg.payload,
                                ) {
                                    Ok(id) => {
                                        resident_dispatch.sent(&key, id, now);
                                        println!(
                                            "[node {label}] resident: dispatch result for \
                                             saga {} attempt {} relayed",
                                            key.0, key.1
                                        );
                                    }
                                    Err(e) => eprintln!(
                                        "[node {label}] resident: dispatch result relay \
                                         failed for saga {} attempt {}: {e}",
                                        key.0, key.1
                                    ),
                                }
                            }
                        }
                        continue;
                    }
                    println!(
                        "[node {label}] joining: awaiting redemption (epoch {} has {} validators)",
                        tip.epoch,
                        tip.participants.len()
                    );
                    send_announce(&announce_targets, attempt);
                    continue;
                }
                // in the epoch set: PROMOTION consumes the boundary itself —
                // module entries and the real floor certificate — so it rides
                // the full Manifest lane from here.
                let m = match fetch_manifest(&client).await {
                    Ok(m) => m,
                    Err(e) => {
                        let retry = joiner_manifest_fetch_retry(&label, resident_standing, &e);
                        println!("{}", retry.log_line);
                        continue;
                    }
                };
                // a boundary PAST the epoch base needs its
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
                // BOOT PREFLIGHT (design §5 / plan Task 7.3): refuse an
                // under-versioned binary against the served boundary before
                // install/replay — a clear early refusal, not a post-sync app-hash
                // mismatch. inert on a baseline manifest.
                if let Err(e) = m.preflight(MAX_PROTOCOL_VERSION) {
                    eprintln!("[node {label}] FATAL: cannot promote — {e}");
                    std::process::exit(1);
                }
                // THE PROMOTION COLLAPSE for a FOLDING replica: it is already
                // at head with a journal that proved every block it folded —
                // checkpoint OUR OWN state as the validator boot base and
                // reboot. no re-sync against the source, no boundary wait: a
                // quorum-widening cutover HALTS the source awaiting this very
                // node's votes, so any wait-for-the-source flow deadlocks —
                // the freshest member seats itself from its own state.
                if serving.is_some() {
                    let (folded_tip, mut node_r) =
                        serving.take().expect("checked serving above");
                    let mut base = m.clone();
                    base.height = folded_tip;
                    base.app_hash = node_r.host().app_hash();
                    // a boundary at/below its epoch base needs no floor (the
                    // fresh epoch starts from its genesis floor — exactly the
                    // halted-cutover promotion); past the base, OUR persisted
                    // floor cert anchors the replay window.
                    let floor = if folded_tip <= base.view_base {
                        None
                    } else {
                        match node_r.sink_mut().floor_cert() {
                            Ok(fc) => fc.filter(|fc| fc.height <= folded_tip),
                            Err(e) => {
                                eprintln!(
                                    "[node {label}] FATAL: replica promotion floor read: {e}"
                                );
                                std::process::exit(1);
                            }
                        }
                    };
                    let (sink, folded_host) = node_r.sink_and_host();
                    write_boundary_checkpoint(
                        sink,
                        folded_host,
                        &base,
                        &floor,
                        &label,
                        "replica_promotion_checkpoint",
                    )
                    .await;
                    println!(
                        "[node {label}] promoted: validator at epoch {} boundary {} — rebooting",
                        base.epoch, base.height
                    );
                    if let Some(cmd) = &reach_cmd {
                        let _ = cmd.try_send(reachability::ReachabilityCommand::Shutdown);
                        let deadline = std::time::Instant::now() + Duration::from_secs(2);
                        while !cmd.is_closed() && std::time::Instant::now() < deadline {
                            context.sleep(Duration::from_millis(20)).await;
                        }
                    }
                    reboot_self();
                }
                // pre-ascension promotion (direct, un-staged admission): the
                // node never folded, so the classic flow stands — sync the
                // served boundary, fabricate its checkpoint, reboot.
                if recovery_slot.is_none() {
                    recovery_slot =
                        Some(reopen_recovery(&context, &mut recovery_reopens, &label).await);
                }
                match sync_all_modules(
                    &context,
                    &client,
                    &m,
                    NetworkBindings {
                        invite: &namespace,
                        identity_chain_id: &identity_chain_id,
                        duckdns_chain_id: &duckdns_chain_id,
                    },
                    SyncSubstrates {
                        forge_repo: &forge_repo,
                        duckfs_dir: &duckfs_dir,
                        blobs: blobs.clone(),
                    },
                    attempt,
                )
                .await
                {
                    Ok(host) => {
                        let latest = match fetch_manifest(&client).await {
                            Ok(latest) => latest,
                            Err(e) => {
                                println!(
                                    "[node {label}] synced boundary {} but could not revalidate \
                                     latest manifest ({e}); retrying",
                                    m.height
                                );
                                continue;
                            }
                        };
                        let host_hash = host.app_hash();
                        diag_log(format!(
                            "DIAG admission_revalidate synced_height={} synced_hash={} \
                             latest_height={} latest_hash={} host_hash={} latest_matches_host={} \
                             latest_floor_present={}",
                            m.height,
                            hex(&m.app_hash),
                            latest.height,
                            hex(&latest.app_hash),
                            hex(&host_hash),
                            latest.app_hash == host_hash,
                            latest.floor_cert.is_some()
                        ));
                        if let Err(e) = reopen_preflight_synced_host(&host, m.app_hash) {
                            eprintln!("[node {label}] FATAL: promotion preflight failed: {e}");
                            std::process::exit(1);
                        }
                        match choose_promotion_boundary(host_hash, &latest, &me_bytes) {
                            PromotionBoundary::Promote { boundary, source } => {
                                diag_log(format!(
                                    "DIAG promotion_boundary chosen_height={} chosen_hash={} \
                                     chosen_floor_present={} source={}",
                                    boundary.height,
                                    hex(&boundary.app_hash),
                                    boundary.floor_cert.is_some(),
                                    source.as_str()
                                ));
                                let boundary = boundary.clone();
                                let boundary_floor =
                                    match verify_manifest_floor(&namespace, &boundary) {
                                        Ok(floor) => floor,
                                        Err(e) => {
                                            eprintln!(
                                                "[node {label}] FATAL: promotion floor verify: {e}"
                                            );
                                            std::process::exit(1);
                                        }
                                    };
                                diag_log(format!(
                                    "DIAG suffix_install from={} to={} frames=0",
                                    boundary.height, boundary.height
                                ));
                                let floor = boundary_floor.map(|cert| recovery::FloorCert {
                                    epoch: boundary.epoch,
                                    height: boundary.height,
                                    cert,
                                });
                                break (boundary, host, floor);
                            }
                            PromotionBoundary::Retry => {}
                        }
                        println!(
                            "[node {label}] boundary {} drifted during sync ({} -> latest {}); \
                             discarding scratch and retrying",
                            m.height,
                            hex(&m.app_hash),
                            hex(&latest.app_hash)
                        );
                    }
                    Err(e) => println!("[node {label}] sync at boundary {} failed: {e}", m.height),
                }
            };
            println!("[node {label}] synced app_hash={}", hex(&host.app_hash()));

            // the optional shipped-index warm start rides the same sync
            // connection, staged BEFORE the promotion checkpoint lands: a
            // crash mid-fetch reboots back into joiner mode and refetches,
            // and a torn staging directory is discarded at adoption. the
            // promoted reboot's IndexStore::open adopts what committed here.
            if sync_index {
                stage_shipped_index(&client, boundary.boundary_id(), &storage_for_sync, &label)
                    .await;
            }

            // fabricate the checkpoint a restart would have left; the normal
            // recovery boot turns it into a live validator. (a REJOINING key
            // that later resubmits a byte-identical (seq, payload) pair could
            // be dropped by a peer's in-process digest gate; accepted edge
            // until submit sequences ride app state.)
            let mut recovery = recovery_slot
                .take()
                .expect("the journal slot is filled whenever the loop breaks to promote");
            write_boundary_checkpoint(
                &mut recovery,
                &host,
                &boundary,
                &floor,
                &label,
                "promotion_checkpoint",
            )
            .await;
            // tear the pre-warm interface down cleanly before the exec: the
            // in-process boringtun device dies with the process either way,
            // but only an orderly Shutdown unlinks its UAPI socket path —
            // a stale one would fail the rebooted validator's restore-time
            // create. Bounded: the reboot must not hang on a wedged plane —
            // try_send (a plane whose queue is full would never process the
            // Shutdown anyway), then a 2s grace for the orderly unlink.
            if let Some(cmd) = &reach_cmd {
                let _ = cmd.try_send(reachability::ReachabilityCommand::Shutdown);
                let deadline = std::time::Instant::now() + Duration::from_secs(2);
                while !cmd.is_closed() && std::time::Instant::now() < deadline {
                    context.sleep(Duration::from_millis(20)).await;
                }
            }
            println!(
                "[node {label}] promoted: validator at epoch {} boundary {} — rebooting",
                boundary.epoch, boundary.height
            );
            reboot_self();
        }
        // (host, recovered-state, next local submit seq, last checkpoint
        // ONE index fold for the whole boot (journal replay + post-reboot
        // catch-up + post-sync refreshes): its stop flag must persist across
        // phases — a later phase folding past a gap an earlier phase detected
        // would advance watermarks over the hole and hide it from the final
        // heal below.
        let mut boot_fold = IndexFold::new(&index, blobs.clone());
        // (height, oplog position) for the pump's prune bookkeeping, and the
        // manifest that recovery used as its replay baseline).
        type BootState = (
            Host,
            Option<recovery::Recovered>,
            u64,
            (Option<u64>, u64),
            Option<Manifest>,
        );
        let (
            mut host,
            mut resumed,
            mut next_seq,
            mut prev_ckpt,
            mut recovery_manifest_for_resume,
        ): BootState = match manifest.clone() {
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
                let host = genesis_host(
                    &context,
                    &forge_repo,
                    &duckfs_dir,
                    &validators,
                    NetworkBindings {
                        invite: &namespace,
                        identity_chain_id: &identity_chain_id,
                        duckdns_chain_id: &duckdns_chain_id,
                    },
                    blobs.clone(),
                )
                .await;
                let pos = recovery.oplog_pos().await;
                let genesis_participants: Vec<Vec<u8>> =
                    validators.iter().map(|k| k.as_ref().to_vec()).collect();
                // seq 0 is the dev demo op's; real submits start at 1.
                let (cv, pu) = read_upgrade_version_fields(&host).await;
                let genesis_manifest =
                    match Manifest::capture(
                        &host,
                        None,
                        0,
                        0,
                        genesis_participants,
                        Vec::new(),
                        None,
                        cv,
                        pu,
                        pos,
                        1,
                    )
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
                // BOOT PREFLIGHT (design §5 / plan Task 7.3): fail loud EARLY when
                // this binary is too old to apply the blocks at/after the recovered
                // boundary, instead of falling through to an opaque post-replay
                // `AppHashMismatch`. inert on a baseline checkpoint (required_min ==
                // baseline always passes).
                if let Err(e) = manifest.preflight(MAX_PROTOCOL_VERSION) {
                    eprintln!(
                        "[node {label}] FATAL: cannot recover — {e} (recovered boundary needs \
                         protocol v{}, this binary supports up to v{MAX_PROTOCOL_VERSION})",
                        manifest.required_min_version()
                    );
                    std::process::exit(1);
                }
                let restored = restore_host(
                    &context,
                    &forge_repo,
                    &duckfs_dir,
                    &manifest,
                    NetworkBindings {
                        invite: &namespace,
                        identity_chain_id: &identity_chain_id,
                        duckdns_chain_id: &duckdns_chain_id,
                    },
                    blobs.clone(),
                )
                .await;
                let mut host = match restored {
                    Ok(h) => h,
                    Err(e) => {
                        eprintln!("[node {label}] FATAL: checkpoint restore: {e}");
                        std::process::exit(1);
                    }
                };
                // heal the derived index against the CHECKPOINT boundary
                // BEFORE replay: a wiped or trailing per-module database
                // re-derives from the verified checkpoint state, so the
                // journal-suffix fold lands contiguously on top instead of
                // folding forward over a pre-checkpoint hole.
                if let Some(ckpt_height) = manifest.height {
                    heal_index(&index, &host, ckpt_height, &label).await;
                }
                let rec = match recovery
                    .recover_with_sink(&mut host, &manifest, Some(&mut boot_fold))
                    .await
                {
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
                advance_next_seq_from_frames(&mut next_seq, &rec.frames, &me_bytes);
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
                let prev = (manifest.height, manifest.oplog_pos);
                (host, Some(rec), next_seq, prev, Some(manifest))
            }
        };

        // consensus membership comes from the RECOVERY RECORD: the epoch's
        // ENGINE PARTICIPANT SET (at genesis: exactly the config seed). the
        // recovered valset projection is NOT it — a restart inside a cutover
        // window would read a membership change whose boundary has not been
        // crossed and spawn a different scheme than its peers are running.
        let initial_member_keys = match resume_member_keys(resumed.as_ref(), &validators) {
            Ok(keys) => keys,
            Err(e) => {
                eprintln!("[node {label}] FATAL: {e}");
                std::process::exit(1);
            }
        };
        if !initial_member_keys.contains(&signer.public_key()) {
            println!(
                "[node {label}] this identity is not in the recovered validator set — \
                 halting (restart with --sync-only to observe)"
            );
            std::process::exit(0);
        }
        let initial_resume_epoch = resumed.as_ref().map(|r| r.epoch).unwrap_or(0);

        // the TRANSPORT baseline adds the committed RESIDENT set (granted,
        // quorum-exempt keys the mesh must admit so they can sync). read
        // LIVE from the recovered host, unlike the frozen participant set
        // above: a resident grant arms its own cutover, so within any epoch
        // the resident set is constant — except a reboot inside that cutover
        // window, where this node briefly tracks the wider set alone; the
        // boundary re-tracks identically a few views later.
        let initial_resident_keys: Vec<ed25519::PublicKey> = read_valset_residents(&host)
            .await
            .iter()
            .filter_map(|key| ed25519::PublicKey::decode(key.as_slice()).ok())
            .collect();

        // the validator-owned transport mesh, tracked at index = epoch: the
        // epoch's TRANSPORT members (participants ∪ standby registrants) ∪
        // the descriptor mesh (genesis members + [dev] extras — kept
        // authorized so demoted members and pre-genesis peers can still
        // reach the statesync service). the SAME set on every node at this
        // index: discovery kills peers whose bit-vector length disagrees at
        // a shared index, and boundary-read membership is the only set every
        // node agrees on epoch-for-epoch.
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
            initial_resume_epoch,
            mesh_at(
                &initial_member_keys
                    .iter()
                    .chain(initial_resident_keys.iter())
                    .cloned()
                    .collect(),
            ),
        );

        // lanes for epochs BELOW the resume epoch are registered and
        // black-holed (the sync-only arm's exact trick): a lagging peer still
        // gossips there, and an unregistered channel is a protocol violation
        // that would kill its connection — cutting off the very fetch lane it
        // needs to catch up.
        for epoch in 0..initial_resume_epoch {
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
        let bank_base = initial_resume_epoch;
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
        let (mut sync_tx, mut sync_rx) = network.register(CHANNEL_STATE_SYNC, quota, MAX_BACKLOG);
        // the lobby lane: parked joiners announce their keys here (connected
        // as the derived lobby identity); this member verifies each announce
        // against the invite token it carries and RECORDS it for approval.
        let (mut lobby_tx, lobby_rx) = network.register(CHANNEL_LOBBY, quota, MAX_BACKLOG);
        // the submit-relay lane: a resident-standing node ships its own
        // signed frame here; this validator takes custody and answers on
        // drain/expiry. bound `mut` because the pump uses `relay_tx` from BOTH
        // the ingress select arm and the drain-resolution/expiry code.
        let (mut relay_tx, relay_rx) = network.register(CHANNEL_SUBMIT_RELAY, quota, MAX_BACKLOG);

        // the voice + video hub: huddle media between members. per the per-use
        // data-plane ADR (docs/adr/2026-07-07-per-use-data-plane.mdx), media
        // rides the OVERLAY — audio+control on Service::Voice's overlay socket
        // (45902), camera on Service::Video's (45903) — NOT the mesh: two mesh
        // channels to a peer funnel through one per-peer priority relay, so a
        // multi-megabit video burst starved the 32 kbps voice stream behind it.
        // CHANNEL_VOICE/CHANNEL_VIDEO stay REGISTERED + BLACKHOLED (an
        // unregistered channel is a protocol violation that kills the peer's
        // connection) so a peer still on the mesh-media build is absorbed, not
        // disconnected; this node sends no media on them.
        let media_peers = {
            let (_voice_p2p_tx, mut voice_p2p_rx) =
                network.register(CHANNEL_VOICE, quota, MAX_BACKLOG);
            let video_quota = Quota::per_second(NZU32!(512));
            let (_video_p2p_tx, mut video_p2p_rx) =
                network.register(CHANNEL_VIDEO, video_quota, MAX_BACKLOG);
            context
                .child("voice_blackhole")
                .spawn(move |_ctx| async move { while voice_p2p_rx.recv().await.is_ok() {} });
            context
                .child("video_blackhole")
                .spawn(move |_ctx| async move { while video_p2p_rx.recv().await.is_ok() {} });

            // media needs the overlay: with no overlay (fake effect, or the
            // reachability plane unconfigured) there is no media transport at
            // all (the overlay-only cutover — no mesh fallback), so drop the
            // session lane and huddle joins refuse fast instead of hanging.
            let overlay_capable = wireguard_listen.is_some()
                && !matches!(wireguard_effect, config::WireGuardEffectKind::Fake);
            if overlay_capable {
                // tracked media set = transport members ∪ residents, refreshed
                // on every valset cutover (below, beside the statesync book).
                let peers = voice_plane::MediaPeers::new(
                    String::from_utf8(namespace.clone()).expect("namespace is utf-8"),
                );
                peers.set_peers(initial_member_keys.iter().chain(initial_resident_keys.iter()));
                let me: [u8; 32] = signer
                    .public_key()
                    .as_ref()
                    .try_into()
                    .expect("ed25519 keys are 32 bytes");
                voice::spawn_hub(
                    voice_requests,
                    statesync_plane::socket_factory(wireguard_effect, &overlay_slot),
                    std::sync::Arc::clone(&peers),
                    me,
                );
                Some(peers)
            } else {
                drop(voice_requests);
                None
            }
        };

        // the reachability lane + the staged WireGuard plane. the channel is
        // registered unconditionally (an unregistered channel is a protocol
        // violation that kills the sender's connection); the plane itself
        // runs only when `wireguard_listen` is configured, on its OWN
        // plain-tokio OS thread (the app-surface split exactly), talking to
        // the mesh through the two pump tasks below.
        let (reach_p2p_tx, mut reach_p2p_rx) =
            network.register(CHANNEL_REACHABILITY, quota, MAX_BACKLOG);
        let reach_cmd: Option<tokio::sync::mpsc::Sender<reachability::ReachabilityCommand>> =
            match wireguard_listen {
                Some(wg_addr) => {
                    // rendezvous coordinators = every coordinated-reach hint's
                    // coordinator ingress; hostnames resolve once at plane start.
                    let coordinators: Vec<Ingress> =
                        coordinated.iter().map(|(_, c, _)| c.clone()).collect();
                    Some(wire_reachability_plane(
                        &context,
                        &label,
                        &chain_id,
                        &signer,
                        &wireguard_key_file,
                        &mesh_state_file,
                        wg_addr,
                        wireguard_effect,
                        overlay_slot.clone(),
                        advertised_reach,
                        coordinators,
                        // members serve the invite intro: a fresh joiner's
                        // tunnel comes up against this listener before any p2p.
                        invite_listen,
                        coord_cap.clone(),
                        reach_p2p_tx,
                        reach_p2p_rx,
                    ))
                }
                None => {
                    context
                        .child("blackhole_reachability")
                        .spawn(move |_ctx| async move { while reach_p2p_rx.recv().await.is_ok() {} });
                    drop(reach_p2p_tx);
                    None
                }
            };
        // boot: target the resume epoch's member set immediately (with the
        // committed resident set as the pre-warm standbys); cutovers
        // retarget from the orchestrator loop below. the recovered view base
        // keeps advert expiries in the same view regime as live peers.
        if let Some(cmd) = &reach_cmd {
            let _ = cmd
                .send(reachability::ReachabilityCommand::Retarget(
                    reachability::MeshEpochEvent {
                        epoch: initial_resume_epoch,
                        members: initial_member_keys.clone(),
                        standbys: initial_resident_keys.clone(),
                        current_view: resumed.as_ref().map(|r| r.view_base).unwrap_or(0),
                    },
                ))
                .await;
        }

        // start the network actors (dialer/listener/router/tracker). registered
        // receivers buffer regardless, so starting before the engine is fine.
        network.start();

        let promoted_validator_boot = promoted && !validators.contains(&signer.public_key());
        if promoted_validator_boot {
            let Some(server_peer) = sync_source else {
                eprintln!(
                    "[node {label}] FATAL: promoted validator has no statesync source for \
                     post-reboot catch-up"
                );
                std::process::exit(1);
            };
            // like the parked joiner's client: prefer the plane (lazy bind —
            // the promotion reboot restores its tunnels from disk) and fall
            // back to the mesh path on transport failure.
            let mesh_client = BootP2pSyncClient::new(sync_tx, sync_rx, server_peer.clone());
            let client = {
                let plane_slot: statesync_plane::PlaneSlot =
                    std::sync::Arc::new(std::sync::OnceLock::new());
                if statesync_plane::enabled() && wireguard_listen.is_some() {
                    let book = statesync_plane::OverlayBook::new(
                        String::from_utf8(namespace.clone()).expect("namespace is utf-8"),
                    );
                    book.set_peers(peers.iter());
                    statesync_plane::spawn_bring_up(
                        label.clone(),
                        book,
                        signer.public_key(),
                        std::sync::Arc::clone(&plane_slot),
                        statesync_plane::socket_factory(wireguard_effect, &overlay_slot),
                        None,
                    );
                }
                statesync_plane::PlaneFallbackClient::new(plane_slot, &server_peer, mesh_client)
            };
            let mut attempts = 0usize;
            loop {
                attempts += 1;
                let recovered_height = resumed.as_ref().and_then(|rec| rec.height).unwrap_or(0);
                match catch_up_post_reboot_frames(
                    &client,
                    &mut recovery,
                    &mut host,
                    Some(&mut boot_fold),
                    recovered_height,
                    POST_REBOOT_CATCHUP_MAX_ITERS,
                )
                .await
                {
                    Ok(summary) => {
                        println!(
                            "[node {label}] post-reboot catch-up {} -> {} ({} frames)",
                            summary.from_height, summary.to_height, summary.frames
                        );
                        let Some(target) = summary.target.as_ref() else {
                            if summary.to_height == recovered_height {
                                // the source trails us: a quorum-widening
                                // cutover halts the chain awaiting this very
                                // node's votes, and a promoted replica boots
                                // at its own folded tip — ahead of anything
                                // the halted source can serve. the recovered
                                // state is journal-proven; seat ourselves and
                                // the chain resumes.
                                println!(
                                    "[node {label}] post-reboot catch-up: the source trails \
                                     the recovered height {recovered_height} — proceeding as \
                                     the freshest member"
                                );
                                break;
                            }
                            eprintln!(
                                "[node {label}] FATAL: post-catch-up target manifest unavailable"
                            );
                            std::process::exit(1);
                        };
                        if !target
                            .participants
                            .iter()
                            .any(|key| key.as_slice() == signer.public_key().as_ref())
                        {
                            eprintln!(
                                "[node {label}] FATAL: catch-up target height {} no longer \
                                 includes this validator",
                                target.height
                            );
                            std::process::exit(1);
                        }
                        let floor = match verify_manifest_floor(&namespace, target) {
                            Ok(floor) => floor,
                            Err(e) => {
                                eprintln!(
                                    "[node {label}] FATAL: catch-up target floor verify: {e}"
                                );
                                std::process::exit(1);
                            }
                        };
                        if target.epoch > resumed.as_ref().map(|rec| rec.epoch).unwrap_or(0)
                            && let Err(e) = node::BlockSink::cutover(
                                &mut recovery,
                                target.epoch,
                                target.view_base,
                                &target.participants,
                                &target.residents,
                            )
                            .await
                        {
                                eprintln!(
                                    "[node {label}] FATAL: catch-up cutover journal write: {e}"
                                );
                                std::process::exit(1);
                        }
                        let me_bytes = signer.public_key().as_ref().to_vec();
                        advance_next_seq_from_frames(
                            &mut next_seq,
                            &summary.frame_bytes,
                            &me_bytes,
                        );
                        let ckpt = match write_post_reboot_catchup_checkpoint(
                            &mut recovery,
                            &host,
                            recovery_manifest_for_resume.as_ref(),
                            target,
                            &summary.blocks,
                            next_seq,
                        )
                        .await
                        {
                            Ok(ckpt) => ckpt,
                            Err(e) => {
                                eprintln!("[node {label}] FATAL: {e}");
                                std::process::exit(1);
                            }
                        };
                        if let Some(cert) = floor {
                            let floor = recovery::FloorCert {
                                epoch: target.epoch,
                                height: target.height,
                                cert,
                            };
                            if let Err(e) = recovery.write_floor_cert(&floor).await {
                                eprintln!("[node {label}] FATAL: catch-up floor-cert write: {e}");
                                std::process::exit(1);
                            }
                        }
                        prev_ckpt = (ckpt.height, ckpt.oplog_pos);
                        let refreshed = match recovery
                            .recover_with_sink(&mut host, &ckpt, Some(&mut boot_fold))
                            .await
                        {
                            Ok(rec) => rec,
                            Err(e) => {
                                eprintln!(
                                    "[node {label}] FATAL: post-catch-up checkpoint recovery: {e}"
                                );
                                std::process::exit(1);
                            }
                        };
                        advance_next_seq_from_frames(&mut next_seq, &refreshed.frames, &me_bytes);
                        resumed = Some(refreshed);
                        recovery_manifest_for_resume = Some(ckpt);
                        break;
                    }
                    Err(PostRebootCatchupError::RangePruned {
                        target,
                        requested_after,
                        retained_from,
                    }) => {
                        println!(
                            "[node {label}] post-reboot frame range pruned after \
                             {requested_after} (retained from {retained_from}); full syncing \
                             boundary {}",
                            target.height
                        );
                        if !target
                            .participants
                            .iter()
                            .any(|key| key.as_slice() == signer.public_key().as_ref())
                        {
                            eprintln!(
                                "[node {label}] FATAL: full-sync target height {} no longer \
                                 includes this validator",
                                target.height
                            );
                            std::process::exit(1);
                        }
                        let floor = match verify_manifest_floor(&namespace, &target) {
                            Ok(floor) => floor,
                            Err(e) => {
                                eprintln!(
                                    "[node {label}] FATAL: full-sync target floor verify: {e}"
                                );
                                std::process::exit(1);
                            }
                        };
                        let synced = match sync_all_modules(
                            &context,
                            &client,
                            &target,
                            NetworkBindings {
                                invite: &namespace,
                                identity_chain_id: &identity_chain_id,
                                duckdns_chain_id: &duckdns_chain_id,
                            },
                            SyncSubstrates {
                                forge_repo: &forge_repo,
                                duckfs_dir: &duckfs_dir,
                                blobs: blobs.clone(),
                            },
                            10_000 + attempts,
                        )
                        .await
                        {
                            Ok(host) => host,
                            Err(e) => {
                                eprintln!(
                                    "[node {label}] FATAL: full state-sync fallback failed at \
                                     boundary {}: {e}",
                                    target.height
                                );
                                std::process::exit(1);
                            }
                        };
                        host = synced;
                        if target.epoch > resumed.as_ref().map(|rec| rec.epoch).unwrap_or(0)
                            && let Err(e) = node::BlockSink::cutover(
                                &mut recovery,
                                target.epoch,
                                target.view_base,
                                &target.participants,
                                &target.residents,
                            )
                            .await
                        {
                                eprintln!(
                                    "[node {label}] FATAL: full-sync cutover journal write: {e}"
                                );
                                std::process::exit(1);
                        }
                        let pos = recovery.oplog_pos().await;
                        let ckpt = match Manifest::capture(
                            &host,
                            Some(target.height),
                            target.epoch,
                            target.view_base,
                            target.participants.clone(),
                            target.residents.clone(),
                            None,
                            target.current_version,
                            target.pending_upgrade.clone(),
                            pos,
                            next_seq,
                        ) {
                            Ok(m) => m,
                            Err(e) => {
                                eprintln!(
                                    "[node {label}] FATAL: full-sync checkpoint capture: {e}"
                                );
                                std::process::exit(1);
                            }
                        };
                        if let Err(e) = recovery.write_manifest(&ckpt).await {
                            eprintln!(
                                "[node {label}] FATAL: full-sync checkpoint write: {e}"
                            );
                            std::process::exit(1);
                        }
                        if let Some(cert) = floor {
                            let floor = recovery::FloorCert {
                                epoch: target.epoch,
                                height: target.height,
                                cert,
                            };
                            if let Err(e) = recovery.write_floor_cert(&floor).await {
                                eprintln!(
                                    "[node {label}] FATAL: full-sync floor-cert write: {e}"
                                );
                                std::process::exit(1);
                            }
                        }
                        prev_ckpt = (ckpt.height, ckpt.oplog_pos);
                        let refreshed = match recovery
                            .recover_with_sink(&mut host, &ckpt, Some(&mut boot_fold))
                            .await
                        {
                            Ok(rec) => rec,
                            Err(e) => {
                                eprintln!(
                                    "[node {label}] FATAL: full-sync recovery refresh: {e}"
                                );
                                std::process::exit(1);
                            }
                        };
                        let me_bytes = signer.public_key().as_ref().to_vec();
                        advance_next_seq_from_frames(&mut next_seq, &refreshed.frames, &me_bytes);
                        resumed = Some(refreshed);
                        recovery_manifest_for_resume = Some(ckpt);
                        break;
                    }
                    Err(PostRebootCatchupError::Retry(e))
                        if attempts < POST_REBOOT_CATCHUP_MAX_ATTEMPTS =>
                    {
                        println!(
                            "[node {label}] post-reboot catch-up unavailable \
                             (attempt {attempts}/{POST_REBOOT_CATCHUP_MAX_ATTEMPTS}): {e}; \
                             retrying"
                        );
                        // escalate toward a 5s beat: an overlay-only source
                        // (a fully-NATed inviter) is reachable only once the
                        // reachability plane's tunnels assemble, which can
                        // take a while after a promotion reboot — a restart
                        // would not arrive any sooner, it would just redo the
                        // plane restore from zero.
                        let beat = Duration::from_millis(500)
                            .saturating_mul(attempts as u32)
                            .min(Duration::from_secs(5));
                        context.sleep(beat).await;
                    }
                    Err(PostRebootCatchupError::Retry(e)) => {
                        eprintln!(
                            "[node {label}] FATAL: post-reboot catch-up unavailable after \
                             {attempts} attempts: {e}"
                        );
                        std::process::exit(1);
                    }
                    Err(PostRebootCatchupError::Fatal(e)) => {
                        eprintln!("[node {label}] FATAL: post-reboot catch-up failed: {e}");
                        std::process::exit(1);
                    }
                }
            }
            match client.into_inner().into_parts() {
                Ok((tx, rx)) => {
                    sync_tx = tx;
                    sync_rx = rx;
                }
                Err(e) => {
                    eprintln!("[node {label}] FATAL: cannot hand statesync channel to server: {e}");
                    std::process::exit(1);
                }
            }
        }

        // the FINAL index heal, at the boot tip every path converged on:
        // whatever the replay/catch-up fold could not reproduce (opaque
        // blocks, a state-sync jump, a stopped fold) re-derives here from
        // state that has verified against the boundary app-hash.
        drop(boot_fold);
        if let Some(boot_height) = resumed.as_ref().and_then(|r| r.height) {
            heal_index(&index, &host, boot_height, &label).await;
        }

        let member_keys = match resume_member_keys(resumed.as_ref(), &validators) {
            Ok(keys) => keys,
            Err(e) => {
                eprintln!("[node {label}] FATAL: {e}");
                std::process::exit(1);
            }
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
        mesh_oracle.track(
            resume_epoch,
            mesh_at(&member_keys.iter().cloned().collect()),
        );
        if resume_epoch < bank_base || resume_epoch >= bank_base + EPOCH_CHANNEL_BANK {
            eprintln!(
                "[node {label}] FATAL: recovered epoch {resume_epoch} outside the \
                 pre-registered channel bank [{bank_base}, {})",
                bank_base + EPOCH_CHANNEL_BANK
            );
            std::process::exit(1);
        }
        for epoch in bank_base..resume_epoch {
            let Some(slot) = channel_bank
                .get_mut((epoch - bank_base) as usize)
                .and_then(|slot| slot.take())
            else {
                continue;
            };
            let ((_, vote_rx), (_, cert_rx), (_, res_rx), (_, payload_rx), (_, fetch_rx)) = slot;
            for (suffix, mut rx) in [
                ("vote", vote_rx),
                ("cert", cert_rx),
                ("resolver", res_rx),
                ("payload", payload_rx),
                ("fetch", fetch_rx),
            ] {
                let label: &'static str =
                    Box::leak(format!("blackhole_e{epoch}_{suffix}").into_boxed_str());
                context.child(label).spawn(move |_ctx| async move {
                    while rx.recv().await.is_ok() {}
                });
            }
        }
        let mut pending_boot = recovery_manifest_for_resume
            .as_ref()
            .zip(resumed.as_ref())
            .and_then(|(manifest, rec)| derive_pending_boot(manifest, rec));
        // If no membership cutover already claimed the resume slot, re-arm a
        // pending upgrade at the same deterministic activation boundary an
        // uninterrupted node would use. This runs after post-reboot catch-up, so
        // it reads the freshest recovered host/record.
        if pending_boot.is_none()
            && let Some(rec) = resumed.as_ref()
        {
            pending_boot = read_upgrade_state(&host).await.pending.and_then(|p| {
                let crossed = rec.height.is_some_and(|h| h >= p.activation_height);
                if crossed {
                    None
                } else {
                    p.activation_height.checked_sub(rec.view_base)
                }
            });
        }

        // the statesync INGRESS task: owns the channel receiver and loops a
        // clean `recv().await`, forwarding frames into a local bounded queue.
        // the pump then selects on THAT queue — dropping an mpsc `next()`
        // future between ticks is lossless, whereas dropping the p2p receiver's
        // actor-backed `recv()` future mid-flight could eat a delivered
        // message. bounded + drop-on-full: clients time out and retry, so a
        // flood degrades to retries instead of unbounded memory. the queue
        // carries BOTH statesync carriers — mesh rpc frames and data-plane
        // request streams — so one serve task answers both.
        let (bridge_tx, sync_ingress) =
            futures::channel::mpsc::channel::<statesync_plane::SyncJob>(64);
        // the blob fetch-on-miss lane (the #298 prompt-blob cross-node gap):
        // the oracle pool's resolver asks current peers for a digest its own
        // store lacks, over this same statesync channel. the pending map is
        // the serve loop's demux — frames answering OUR fetches never enter
        // the request path — and the peer set follows every cutover re-track
        // beside the other planes' books.
        let blob_pending: blob_fetch::PendingMap = Default::default();
        let blob_peers: std::sync::Arc<std::sync::RwLock<Vec<ed25519::PublicKey>>> =
            std::sync::Arc::new(std::sync::RwLock::new(
                initial_member_keys
                    .iter()
                    .chain(initial_resident_keys.iter())
                    .cloned()
                    .collect(),
            ));
        let blob_fetcher = blob_fetch::MeshBlobFetcher::new(
            sync_tx.clone(),
            blob_pending.clone(),
            std::sync::Arc::clone(&blob_peers),
            signer.public_key(),
        )
        .into_fetch_fn();
        {
            let mut bridge_tx = bridge_tx.clone();
            context.child("sync_ingress").spawn(move |_ctx| {
                let mut receiver = sync_rx;
                async move {
                    loop {
                        match receiver.recv().await {
                            Ok((peer, msg)) => {
                                let bytes: Vec<u8> = msg.into();
                                // full bridge = flood pressure: drop; clients retry.
                                let _ = bridge_tx
                                    .try_send(statesync_plane::SyncJob::Mesh(peer, bytes));
                            }
                            Err(_) => return, // network shutdown — nothing to serve.
                        }
                    }
                }
            });
        }
        // statesync's per-use data plane (env-gated, default off): the same
        // requests over overlay stream sockets, accepted into the same queue.
        // the address book doubles as admission — members + standbys of the
        // tracked view, updated at every cutover re-track below.
        let sync_plane_book = statesync_plane::enabled().then(|| {
            let book = statesync_plane::OverlayBook::new(
                String::from_utf8(namespace.clone()).expect("namespace is utf-8"),
            );
            book.set_peers(initial_member_keys.iter().chain(initial_resident_keys.iter()));
            statesync_plane::spawn_bring_up(
                label.clone(),
                std::sync::Arc::clone(&book),
                signer.public_key(),
                std::sync::Arc::new(std::sync::OnceLock::new()),
                statesync_plane::socket_factory(wireguard_effect, &overlay_slot),
                Some(bridge_tx.clone()),
            );
            book
        });
        drop(bridge_tx);
        // the statesync SERVE task (the [`SyncStateRequest`] seam): owns the
        // capture cache and both statesync carriers end-to-end — decode,
        // leases, chunk slicing, and the mesh/plane replies — so serving a
        // joiner never occupies the consensus loop. the loop answers only
        // the bounded state touches crossing `sync_state_tx`; when the loop
        // is busy the serve lane backpressures, never the reverse.
        let (sync_state_tx, mut sync_state_rx) =
            futures::channel::mpsc::channel::<SyncStateRequest>(8);
        {
            let state_tx = sync_state_tx;
            let mut sync_tx = sync_tx;
            let mut ingress = sync_ingress;
            let blob_pending = blob_pending.clone();
            let sync_blobs = blobs.clone();
            context
                .child("statesync_serve")
                .spawn(move |_ctx| async move {
                    let mut server = SyncServer::new();
                    while let Some(job) = ingress.next().await {
                        // both carriers land here: mesh frames ride an rpc
                        // envelope (multiplexed channel — the id correlates);
                        // a plane stream IS its own correlation and reply path.
                        let (reply_to, rpc_id, request) = match job {
                            statesync_plane::SyncJob::Mesh(peer, bytes) => {
                                let Ok((rpc_id, body)) = statesync::decode_rpc(&bytes) else {
                                    continue; // malformed rpc envelope: drop, never crash.
                                };
                                // the mesh demux: OUR fetch answers are consumed,
                                // stray responses (a blob answer landing after its
                                // fan-out's sweep) and unparseable frames are
                                // DROPPED — answering either is how two serve
                                // loops bounce Error frames forever. only a real
                                // request proceeds; the reply-on-bad-frame lane is
                                // stream-only below.
                                match blob_fetch::classify_mesh_frame(
                                    &blob_pending,
                                    rpc_id,
                                    body,
                                ) {
                                    blob_fetch::MeshFrame::OurResponse
                                    | blob_fetch::MeshFrame::StrayResponse
                                    | blob_fetch::MeshFrame::Junk => continue,
                                    blob_fetch::MeshFrame::Request(req) => (
                                        statesync_plane::SyncReplyTo::Mesh(peer),
                                        rpc_id,
                                        Ok(req),
                                    ),
                                }
                            }
                            statesync_plane::SyncJob::Plane(stream, req) => (
                                statesync_plane::SyncReplyTo::Plane(stream),
                                0,
                                statesync::decode_request(&req),
                            ),
                        };
                        let resp = match request {
                            // blob fetches are host state — answered from the
                            // node-local store, never routed into SyncServer.
                            Ok(statesync::SyncRequest::Blob { digest }) => {
                                blob_fetch::serve_blob(&sync_blobs, &digest)
                            }
                            Ok(req) => drive_sync_request(&mut server, &state_tx, req).await,
                            // stream-only by construction: a plane stream is a
                            // one-shot request/response, so an Error reply here
                            // can never re-enter a serve loop and oscillate.
                            Err(e) => statesync::SyncResponse::Error(format!(
                                "bad request frame: {e}"
                            )),
                        };
                        let resp = statesync::encode_response(&resp);
                        match reply_to {
                            statesync_plane::SyncReplyTo::Mesh(peer) => {
                                let _ = sync_tx.send(
                                    Recipients::One(peer),
                                    IoBuf::from(statesync::encode_rpc(rpc_id, &resp)),
                                    false,
                                );
                            }
                            statesync_plane::SyncReplyTo::Plane(mut stream) => {
                                // one request per stream: write the response
                                // and drop — the close is the client's
                                // completion.
                                let _ =
                                    statesync::dataplane::write_frame(&mut stream, &resp).await;
                            }
                        }
                    }
                });
        }
        // the lobby lane rides the same bridge pattern: announces are consumed
        // by the pump between drains. drop-on-full is doubly safe here — a
        // parked joiner re-announces every few seconds anyway.
        let (lobby_bridge_tx, mut lobby_ingress) =
            futures::channel::mpsc::channel::<(ed25519::PublicKey, Vec<u8>)>(64);
        context.child("lobby_ingress").spawn(move |_ctx| {
            let mut receiver = lobby_rx;
            let mut bridge_tx = lobby_bridge_tx;
            async move {
                loop {
                    match receiver.recv().await {
                        Ok((peer, msg)) => {
                            let bytes: Vec<u8> = msg.into();
                            let _ = bridge_tx.try_send((peer, bytes));
                        }
                        Err(_) => return,
                    }
                }
            }
        });
        // the submit-relay lane rides the same bounded drop-on-full bridge: a
        // dropped relay degrades to the resident client's honest timeout +
        // re-submit, so flood pressure never blocks the pump.
        let (relay_bridge_tx, mut relay_ingress) =
            futures::channel::mpsc::channel::<(ed25519::PublicKey, Vec<u8>)>(64);
        context.child("relay_ingress").spawn(move |_ctx| {
            let mut receiver = relay_rx;
            let mut bridge_tx = relay_bridge_tx;
            async move {
                loop {
                    match receiver.recv().await {
                        Ok((peer, msg)) => {
                            let bytes: Vec<u8> = msg.into();
                            let _ = bridge_tx.try_send((peer, bytes));
                        }
                        Err(_) => return,
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
        let recovered_height = resumed
            .as_ref()
            .and_then(|rec| rec.height)
            .map(|height| height.to_string())
            .unwrap_or_else(|| "none".to_string());
        let recovered_hash = resumed
            .as_ref()
            .map(|rec| hex(&rec.app_hash))
            .unwrap_or_else(|| "none".to_string());
        let replayed = resumed.as_ref().map(|rec| rec.applied).unwrap_or(0);
        let boot_floor_height = latest_floor
            .as_ref()
            .map(|floor| floor.height.to_string())
            .unwrap_or_else(|| "none".to_string());
        diag_log(format!(
            "DIAG promotion_recovered recovered_height={} recovered_hash={} replayed={} \
             boot_floor_height={}",
            recovered_height, recovered_hash, replayed, boot_floor_height
        ));
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
        let resident_keys = match resume_resident_keys(resumed.as_ref()) {
            Ok(keys) => keys,
            Err(e) => {
                eprintln!("[node {label}] FATAL: {e}");
                std::process::exit(1);
            }
        };
        let mut orchestrator = consensus::ValsetOrchestrator::resume(
            CUTOVER_DELAY,
            member_keys.clone(),
            resident_keys.clone(),
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
        // relayed submits held for a wire answer, keyed like pending_submits by
        // the frame's content address: resolved by the SAME drain that resolves
        // local holds, expired on the same SUBMIT_HOLD budget. the peer is where
        // the Reply goes.
        let mut pending_relays: std::collections::HashMap<
            node::FrameId,
            (ed25519::PublicKey, std::time::Instant),
        > = std::collections::HashMap::new();
        let mut validator_relay = relay_runtime::ValidatorRelay::new(blobs.clone());
        let mut last_published: Option<u64> = None;
        // verified-but-unapproved join requests, keyed by joiner key. NODE-
        // LOCAL and in-memory by design: this is a doorbell, not state — the
        // parked joiner re-announces every few seconds, so a restart loses
        // nothing durable. read by the `join-requests` rpc; entries whose key
        // has since become a member are dropped at read time.
        let mut join_requests: std::collections::BTreeMap<Vec<u8>, JoinRequestRecord> =
            std::collections::BTreeMap::new();
        // recovery cadence: sealed blocks since the last checkpoint manifest.
        let mut blocks_since_checkpoint: u64 = 0;
        // the last absolute view ticked to the reachability plane — one
        // ViewTick per actual advance, not one per 100ms drain pass.
        let mut last_reach_view: Option<u64> = None;
        // the per-block-time flush cadence: packs the window's enqueued frames
        // (real ops and/or an idle nop) into one batch block. see the flush loop.
        let mut last_flush = std::time::Instant::now();
        // a cutover Retarget the plane's command queue could not take yet
        // (NON-BLOCKING sends: the plane is not consensus, so the loop never
        // waits on it). retried every drain beat until it lands; a newer
        // epoch's Retarget supersedes an undelivered older one.
        let mut pending_retarget: Option<reachability::MeshEpochEvent> = None;
        // dev override (`make dev` sets DUCKTAPE_DISABLE_HEARTBEAT): keep an idle
        // dev chain quiet — no nop blocks — so every committed block is real
        // activity and the journal/logs carry no idle churn. NEVER set this on a
        // multi-node or upgrade-driving network: the heartbeat is what ticks an
        // idle chain across a pending cutover and keeps the console height
        // visibly live.
        let heartbeat_disabled = std::env::var_os("DUCKTAPE_DISABLE_HEARTBEAT").is_some();
        // throttle for the saga crank pump below.
        let mut last_crank = std::time::Instant::now();
        // throttle for the dispatch delivery-nudge pump below.
        let mut last_nudge = std::time::Instant::now();
        // the host-owned worker set (reactor seam): effects of finalized
        // blocks are offered here, and claimed follow-ups re-enter the ordered
        // lane as their own blocks.
        // load capability specs and discover this host's installed executor
        // CLIs (BYO — no credential handling here). the discovered tag set is
        // BOTH what the oracle worker can run and what this node announces to
        // the capability registry, so an announce can never claim more than
        // the host provides (`announce_capabilities = false` narrows the
        // announced set to nothing — never the reverse). routing and
        // default models live in the specs (docs/records/specs/capability-spec.md); a broken
        // operator spec is a boot error, not a silently dropped executor.
        let providers = capability_host::discover_with_dirs_and_output_sink(
            agent_dirs.clone(),
            run_output_sink(stream_hub.run_output()),
        )
        .unwrap_or_else(|e| panic!("capability specs failed to load: {e}"));
        let my_capabilities = providers.capabilities();
        // OFF-LOOP execution: the pool gates effects inline (lease check —
        // WorkerRequests leased to another node's key are skipped, not
        // double-run — under this node's submit key) but runs the provider
        // CLI on spawned background tasks; completed results come back over
        // `oracle_results` (an ingress arm below) and re-enter the ordered
        // lane as ordinary signed submits, so a minutes-long run never
        // stalls the drain/rpc/heartbeat arms of this loop.
        let (oracle_worker, mut oracle_results) = oracle_pool::build(
            &context,
            providers,
            signer.public_key().as_ref().to_vec(),
            blobs.clone(),
            agent_provisioner.clone(),
            // fetch-on-miss over the mesh: a prompt pin staged on another
            // node's blob store resolves here instead of failing the run.
            Some(blob_fetcher),
        );
        let workers: Vec<Box<dyn reactor::Worker>> = vec![oracle_worker];
        // the readiness self-signaller: polls COMMITTED upgrade state between drains
        // and emits ONE truthful validator-origin `SignalReady` per pending upgrade
        // this binary can execute. survives restart/late-join (state-driven, not a
        // one-shot effect). inert before the module is registered.
        let mut signaller =
            ReadinessSignaller::new(MAX_PROTOCOL_VERSION, signer.public_key().as_ref().to_vec());
        // the capability self-announcer: publishes this node's discovered
        // provider set into the capability registry once (state-driven,
        // idempotent). inert when this host installed no executor CLIs.
        let mut announcer =
            CapabilityAnnouncer::new(signer.public_key().as_ref().to_vec(), my_capabilities);
        // Local targets never enter this pump: it carries only the replicated
        // declarations projected from the validated node config.
        let mut duckdns_announcer = duckdns_node::Announcer::new(
            signer.public_key().as_ref().to_vec(),
            duckdns_announcements.clone(),
        );
        // one-shot upgrade transition markers keyed off COMMITTED upgrade state,
        // modeled on the `converged` latch: `upgrade armed …` fires when readiness
        // first reaches R==n (every current boundary member signaled) for the
        // pending upgrade — the pre-boundary observable the e2e keys on; `upgrade
        // cleared …` fires when a previously-observed pending clears (the boundary
        // `Advance` reconciliation at H, on ARM or ABORT). the boundary crossing
        // itself prints the `upgrade activated …` / `upgrade aborted …` verdict.
        let mut upgrade_armed_latch: Option<(String, u32)> = None;
        let mut upgrade_pending_seen: Option<String> = None;

        // graceful checkpoint on process signals (SIGTERM/SIGINT): the desktop
        // shell SIGTERMs the daemon on quit, so it must take the SAME safe path
        // as an rpc `Shutdown` — a best-effort final manifest + journal barrier
        // — instead of tearing down mid-block and leaving the disk ahead of the
        // last in-memory checkpoint (the recovery brick). the streams are made
        // INSIDE the tokio async context so the signal driver is live; a
        // failure to install them is non-fatal: log and carry on WITHOUT the
        // graceful-quit arm rather than aborting daemon boot — a hard SIGKILL /
        // power loss already lands on the same WAL-forward recovery, so the
        // worst case of a missing handler is the pre-fix behavior, not a brick.
        let mut sigterm =
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                Ok(s) => Some(s),
                Err(e) => {
                    eprintln!(
                        "[node {label}] WARN: SIGTERM handler install failed ({e}); \
                         graceful-quit checkpoint disabled (a hard kill still recovers)"
                    );
                    None
                }
            };
        let mut sigint =
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt()) {
                Ok(s) => Some(s),
                Err(e) => {
                    eprintln!(
                        "[node {label}] WARN: SIGINT handler install failed ({e}); \
                         graceful-quit checkpoint disabled (a hard kill still recovers)"
                    );
                    None
                }
            };

        // the graceful checkpoint sequence, shared by the rpc `Shutdown` arm and
        // the signal arm so the two can never drift. a macro (not a fn) because
        // it borrows `node` mutably while reading `orchestrator`/`next_seq` and
        // `node`'s type is a large generic — it runs on the SAME single-threaded
        // select loop, so it can never race the periodic checkpoint below.
        // captures the committed upgrade version fields the same way the periodic
        // checkpoint does, so a graceful-quit manifest is byte-identical to one.
        macro_rules! graceful_checkpoint {
            () => {{
                if let Some(f) = node.finalized() {
                    let pos = node.sink_mut().oplog_pos().await;
                    let (cv, pu) = read_upgrade_version_fields(node.host()).await;
                    if let Ok(m) = Manifest::capture(
                        node.host(),
                        Some(f.height),
                        orchestrator.epoch(),
                        orchestrator.epoch_base(),
                        participant_bytes(&orchestrator),
                        resident_bytes(&orchestrator),
                        orchestrator.pending_cutover().map(|c| c.cutover_view()),
                        cv,
                        pu,
                        pos,
                        next_seq,
                    ) {
                        let _ = node.sink_mut().write_manifest(&m).await;
                    }
                }
                let _ = node.sink_mut().sync().await;
            }};
        }
        // the drain deadline (see the drain arm): ABSOLUTE, so the
        // per-iteration select rebuild cannot reset it under ingress load.
        let mut next_drain = context.current() + DRAIN_TICK;
        loop {
            // resolve on whichever signal stream installed; if neither did,
            // this arm simply never fires (pending forever) and the loop runs
            // exactly as before the fix.
            let signalled = async {
                match (sigterm.as_mut(), sigint.as_mut()) {
                    (Some(t), Some(i)) => {
                        let t = t.recv();
                        let i = i.recv();
                        futures::pin_mut!(t, i);
                        futures::future::select(t, i).await;
                    }
                    (Some(t), None) => {
                        t.recv().await;
                    }
                    (None, Some(i)) => {
                        i.recv().await;
                    }
                    (None, None) => futures::future::pending::<()>().await,
                }
            }
            .fuse();
            futures::pin_mut!(signalled);
            futures::select_biased! {
                _ = signalled => {
                    println!(
                        "[node {label}] SIGTERM/SIGINT — graceful checkpoint then exit"
                    );
                    graceful_checkpoint!();
                    std::process::exit(0);
                }
                // DRAIN CADENCE — an ABSOLUTE deadline, hoisted ABOVE the ingress
                // arms. this select is rebuilt every loop iteration, so an
                // arm-local `sleep(100ms)` restarts from zero whenever any other
                // arm completes first — a saturating rpc-submit stream (requests
                // landing well inside 100ms) then resets the timer forever and
                // the drain NEVER runs: heights and status freeze, held submit
                // replies starve, and the epoch cutover (`respawn_if_due` below
                // is drain-driven) stalls for exactly as long as the flood lasts
                // while the armed boundary's discard window swallows every
                // accepted op. an absolute deadline survives the select rebuild,
                // and sitting above the ingress arms makes `select_biased!` take
                // it the moment it is due — load can delay one drain by one
                // request's service time, never starve it.
                _ = context.sleep_until(next_drain).fuse() => {
                    next_drain = context.current() + DRAIN_TICK;
                    // FAIL-STOP: a drain error is a node-local block-boundary
                    // fault — this node's state is indeterminate relative to its
                    // peers, so applying even one more finalized op could
                    // silently fork it. exit loudly; an operator (or supervisor)
                    // restarts the node, which then re-joins via state sync.
                    let drained_count = match node.drain_delivered().await {
                        Ok(n) => n,
                        Err(e) => {
                            eprintln!("[node {label}] FATAL: {e} — halting");
                            std::process::exit(1);
                        }
                    };
                    applied += drained_count;
                    // durabilize the tip seal when the chain goes idle. a seal is a
                    // plain journal append made durable only by the NEXT block's
                    // pre-apply sync; on an idle chain the tip block's seal can sit
                    // un-synced for a whole block-time, and a crash there loses it,
                    // turning the tip into a TRAILING block. that is fine for most
                    // ops, but a trailing SELF-READING op — a files CAS commit whose
                    // re-execution reads the claimant's already-durable post-state —
                    // cannot be selective-replayed and would brick a SOLO node (no
                    // peer to re-sync from). syncing on the idle transition closes
                    // the window; a busy chain amortizes durability against the next
                    // pre-apply and needs no extra sync here.
                    if drained_count > 0
                        && node.pending_batch_len() == 0
                        && node.orderer().pending_len() == 0
                        && let Err(e) = node.sink_mut().sync().await
                    {
                        eprintln!("[node {label}] tip-seal sync failed: {e}");
                    }
                    // resolve held app-surface submits against what this
                    // drain finished with; every disposition is deterministic,
                    // so the reply faithfully reports the op's consensus fate.
                    let drained = node.take_drained();
                    // the once-per-block System-injection traces (upgrade
                    // Advance, mailbox DeliverPending follow-ups) ride beside
                    // the member frames; each height's entry indexes AFTER
                    // that height's member dispatches, matching the replay
                    // paths' row order exactly.
                    let mut system_dispatches: std::collections::BTreeMap<
                        u64,
                        Vec<host::DispatchRecord>,
                    > = node.take_system_dispatches().into_iter().collect();
                    // sealed = journaled: one seal per BLOCK (height), whatever a
                    // batch's member count. count DISTINCT sealed heights so the
                    // checkpoint cadence stays per-block; applied and rejected
                    // members both seal, discarded frames never sealed a height.
                    blocks_since_checkpoint += drained
                        .iter()
                        .filter(|d| d.disposition != node::Disposition::Discarded)
                        .map(|d| d.height)
                        .collect::<std::collections::BTreeSet<u64>>()
                        .len() as u64;
                    // fold every SEALED frame into the derived per-module
                    // index: an applied frame contributes its dispatch trace,
                    // a rejected one folds EMPTY (it still consumed its
                    // height, and every module's watermark must track the
                    // sealed tip or restart staleness checks would rebuild
                    // spuriously). discarded frames never sealed a height.
                    // a frame the explorer shows — a decoded op that isn't
                    // the heartbeat nop (the deliberately-empty block that
                    // only ticks an idle chain) — additionally carries its
                    // explorer row, so GET /v1/blocks survives restarts.
                    // canonical state committed above, so an index failure
                    // degrades read models only — the store poisons itself
                    // and stays loud until rebuilt.
                    // fold each BLOCK once: a batch delivers N DrainedFrames at
                    // ONE height (its members, contiguous in agreed order). the
                    // per-module index and the `ducktape_*` metrics series are
                    // per-BLOCK — folding per frame would over-count blocks as ops
                    // AND lose every member after the first to the index's
                    // idempotent same-height skip. group the run of same-height
                    // frames, concatenate their dispatch traces under one running
                    // seq (so `op_key(height, seq)` stays unique across members),
                    // and fold once. canonical state committed above, so an index
                    // failure degrades read models only — it stays loud.
                    let mut gi = 0;
                    while gi < drained.len() {
                        let height = drained[gi].height;
                        let mut block_dispatches: Vec<host::DispatchRecord> = Vec::new();
                        let mut block_latency = 0u64;
                        let mut any_applied = false;
                        // the block record carries a RootOp for EVERY non-nop
                        // member (agreed order); the block hash is the first
                        // member's frame id and the commit is the members' shared
                        // app-hash. a pure nop/idle block shows no ops.
                        let mut block_ops: Vec<noded::RootOp> = Vec::new();
                        let mut block_hash: Option<node::FrameId> = None;
                        let mut block_app_hash: Option<StateRoot> = None;
                        while gi < drained.len() && drained[gi].height == height {
                            let d = &drained[gi];
                            gi += 1;
                            // a DISCARD never sealed this height (it is carried, not
                            // applied) — it contributes nothing to the fold.
                            if d.disposition == node::Disposition::Discarded {
                                continue;
                            }
                            if let (node::Disposition::Applied, Some(op)) =
                                (&d.disposition, &d.op)
                            {
                                any_applied = true;
                                block_latency = block_latency.saturating_add(op.latency_us);
                                block_dispatches.extend(op.dispatches.iter().cloned());
                            }
                            if let Some(op) = &d.op
                                && op.target != NOP_TARGET
                            {
                                let disposition = match d.disposition {
                                    node::Disposition::Applied => noded::BlockDisposition::Applied,
                                    node::Disposition::Rejected => noded::BlockDisposition::Rejected,
                                    // unreachable: Discarded is filtered at the top
                                    // of the inner loop; kept for match exhaustiveness.
                                    node::Disposition::Discarded => continue,
                                };
                                if block_hash.is_none() {
                                    block_hash = Some(d.id);
                                    block_app_hash = Some(d.app_hash);
                                }
                                block_ops.push(explorer_root_op(
                                    &blobs,
                                    &op.origin,
                                    &op.target,
                                    &op.payload,
                                    &op.dispatches,
                                    disposition,
                                ));
                            }
                        }
                        // the block's System-injection dispatches index AFTER
                        // every member's (the replay paths' merge order) — an
                        // agent reply delivered via the mailbox injection is
                        // an op row here like anywhere else.
                        if let Some(sys) = system_dispatches.remove(&height) {
                            block_dispatches.extend(sys);
                        }
                        // one block per height: an APPLIED block records fully
                        // (count, this node's summed apply latency, per-module
                        // dispatch counters); an all-rejected block (the idle nop
                        // lands here) only follows the height gauge. ops_total
                        // counts the aggregated member ops.
                        if any_applied {
                            metrics.record_block(height, block_latency, &block_dispatches);
                        } else {
                            metrics.record_height(height);
                        }
                        metrics.record_ops(block_ops.len());
                        let record = (!block_ops.is_empty()).then(|| {
                            noded::block_row(&noded::BlockRecord {
                                height,
                                hash: block_hash.map(|h| noded::hex_bytes(&h)).unwrap_or_default(),
                                commit_hash: block_app_hash.map(|h| hex(&h)).unwrap_or_default(),
                                ops: block_ops,
                            })
                        });
                        // this lane's agreed clock IS the height: the drain stamps
                        // BlockContext { consensus_time: height } for every block.
                        let ops = indexer::BlockOps {
                            record,
                            ..noded::index_block_ops(height, height, &block_dispatches)
                        };
                        if let Err(err) = index.apply_block(&ops) {
                            eprintln!(
                                "[node {label}] module index apply failed at height {height}: {err} \
                                 — wipe <storage>/index to rebuild"
                            );
                        }
                    }
                    for d in drained {
                        // a DISCARD is not this hold's outcome: the cutover
                        // carries the frame into the new epoch under the SAME
                        // FrameId, so the hold stays open until the carried
                        // frame finalizes there (or SUBMIT_HOLD expires into
                        // the truthful re-query reply).
                        if d.disposition == node::Disposition::Discarded {
                            continue;
                        }
                        // resolve a relayed hold FIRST: a relayed frame has no
                        // local pending_submits entry, so this must precede the
                        // `else { continue }` below or the wire Reply is lost.
                        if let Some((peer, _)) = pending_relays.remove(&d.id) {
                            let outcome = match d.disposition {
                                node::Disposition::Applied => relay::RelayOutcome::Applied {
                                    height: d.height,
                                    app_hash: hex(&d.app_hash),
                                },
                                node::Disposition::Rejected => relay::RelayOutcome::Rejected {
                                    // carry the module's VERBATIM reason (node-
                                    // local observability off the DrainedFrame)
                                    // so the resident forwards it to its caller
                                    // — the duckfs-client engine keys on the
                                    // "files: conflict:" prefix. generic wording
                                    // only when the drain captured no reason.
                                    detail: d.reason.clone().unwrap_or_else(|| {
                                        "op finalized but rejected (deterministic no-op)".into()
                                    }),
                                },
                                node::Disposition::Discarded => unreachable!("filtered at the loop top"),
                            };
                            let msg = relay::RelayMsg::Reply { frame_id: d.id, outcome };
                            let _ = relay_tx.send(
                                Recipients::One(peer),
                                IoBuf::from(relay::encode_msg(&msg)),
                                false,
                            );
                        }
                        let Some((reply, _)) = pending_submits.remove(&d.id) else { continue };
                        let _ = reply.send(match d.disposition {
                            node::Disposition::Applied => Ok(noded::BlockSummary {
                                height: d.height,
                                // the PER-BLOCK boundary this frame settled at
                                // (not the end-of-drain hash — a drain can
                                // apply several blocks).
                                app_hash: hex(&d.app_hash),
                            }),
                            node::Disposition::Rejected => Err(d.reason.clone().unwrap_or_else(
                                || {
                                    // the module's VERBATIM reason when the drain
                                    // captured one (duckfs-client keys on the
                                    // "files: conflict:" prefix); generic wording
                                    // otherwise.
                                    "op finalized but rejected (deterministic no-op)".into()
                                },
                            )),
                            // unreachable — filtered at the loop top — but
                            // stay total rather than panic.
                            node::Disposition::Discarded => continue,
                        });
                    }
                    validator_relay.expire(std::time::Instant::now(), &mut relay_tx);
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
                    // the same expiry contract for relayed holds: the mesh never
                    // finalized in time, so answer the resident truthfully — the
                    // op may still land, it re-queries on the next block.
                    if !pending_relays.is_empty() {
                        let now = std::time::Instant::now();
                        let expired: Vec<node::FrameId> = pending_relays
                            .iter()
                            .filter(|(_, (_, deadline))| *deadline <= now)
                            .map(|(k, _)| *k)
                            .collect();
                        for k in expired {
                            if let Some((peer, _)) = pending_relays.remove(&k) {
                                let msg = relay::RelayMsg::Reply {
                                    frame_id: k,
                                    outcome: relay::RelayOutcome::Refused {
                                        detail: "timed out awaiting finalization — re-query on the next block".into(),
                                    },
                                };
                                let _ = relay_tx.send(
                                    Recipients::One(peer),
                                    IoBuf::from(relay::encode_msg(&msg)),
                                    false,
                                );
                            }
                        }
                    }
                    // publish each newly-applied boundary to ws subscribers
                    // (send only errs when nobody is subscribed — fine). the
                    // drain loop above already folded each block into the
                    // metrics series; this tip seam carries the ws block
                    // summary only — it fires once per drain.
                    if let Some(f) = node.finalized()
                        && last_published != Some(f.height)
                    {
                        stream_hub.publish_block(f.height, hex(&f.app_hash));
                        last_published = Some(f.height);
                    }

                    // persist the finalization floor once everything at or
                    // below it has drained. read the certificate FIRST, the
                    // gate second: releases happen only on this thread, so a
                    // zero gate proves the cert's view is fully applied — a
                    // floor ahead of app state would suppress replay of
                    // finalized ops a restart still needs.
                    if let Some((view, cert)) = node.orderer().latest_finalization()
                        && view != 0
                        && node.orderer().unreleased_len() == 0
                    {
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

                    // periodic checkpoint: snapshot the in-memory cohort and
                    // prune the op journal below the PREVIOUS checkpoint once
                    // the persisted floor has passed it (pruned frames must
                    // never be needed to resolve a re-reported finalization).
                    if blocks_since_checkpoint >= checkpoint_blocks
                        && let Some(f) = node.finalized()
                    {
                        let pos = node.sink_mut().oplog_pos().await;
                        let (cv, pu) = read_upgrade_version_fields(node.host()).await;
                        let captured = Manifest::capture(
                            node.host(),
                            Some(f.height),
                            orchestrator.epoch(),
                            orchestrator.epoch_base(),
                            participant_bytes(&orchestrator),
                            resident_bytes(&orchestrator),
                            orchestrator.pending_cutover().map(|c| c.cutover_view()),
                            cv,
                            pu,
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
                                    if floor_passed
                                        && let Err(e) =
                                            node.sink_mut().prune_oplog(prev_ckpt.1).await
                                    {
                                        eprintln!("[node {label}] oplog prune failed: {e}");
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

                    // the VALSET ORCHESTRATION step: observe the finalized
                    // membership projection; a change schedules a deterministic
                    // cutover (arming the discard ceiling), and crossing the
                    // cutover view tears the engine down and respawns it over
                    // the set read AT the boundary. the observation barrier
                    // guarantees this tick's last view IS the changing block's
                    // view when membership moved.
                    if let Some(engine_view) = node.last_engine_view() {
                        // tick the reachability plane's freshness clock.
                        // engine views are EPOCH-LOCAL (they reset at every
                        // cutover), so convert to the absolute app-height
                        // clock (`epoch_base + view`) — the regime the boot
                        // Retarget's `view_base` put the plane's advert and
                        // handshake expiries in.
                        if let Some(cmd) = &reach_cmd {
                            let absolute_view = orchestrator.app_height(engine_view);
                            if last_reach_view.is_none_or(|v| v < absolute_view) {
                                // NON-BLOCKING: the plane is not consensus. a
                                // full command queue (a wedged or slow plane)
                                // sheds this tick — the next drain beat carries
                                // a fresher one — instead of stalling the loop
                                // behind an actor that may never drain.
                                let _ = cmd.try_send(
                                    reachability::ReachabilityCommand::ViewTick(absolute_view),
                                );
                                last_reach_view = Some(absolute_view);
                            }
                            // flush a staged cutover Retarget (see
                            // `pending_retarget`) — MUST eventually land, so
                            // it retries every beat rather than being shed.
                            if let Some(event) = pending_retarget.take()
                                && let Err(tokio::sync::mpsc::error::TrySendError::Full(
                                    reachability::ReachabilityCommand::Retarget(event),
                                )) = cmd.try_send(reachability::ReachabilityCommand::Retarget(
                                    event,
                                ))
                            {
                                pending_retarget = Some(event);
                            }
                        }
                        let members_raw = read_valset_members(node.host()).await;
                        let mut observed: Vec<ed25519::PublicKey> = Vec::new();
                        for key in &members_raw {
                            if let Ok(pk) = ed25519::PublicKey::decode(key.as_slice()) {
                                observed.push(pk);
                            }
                        }
                        // the RESIDENT projection, read at the same frozen
                        // point: a grant/revoke arms the same single cutover
                        // slot (mesh admission is epoch-scoped).
                        let residents_raw = read_valset_residents(node.host()).await;
                        let mut observed_residents: Vec<ed25519::PublicKey> = Vec::new();
                        for key in &residents_raw {
                            if let Ok(pk) = ed25519::PublicKey::decode(key.as_slice()) {
                                observed_residents.push(pk);
                            }
                        }
                        if let consensus::ObservationOutcome::Scheduled(cutover) =
                            orchestrator.observe_members(
                                engine_view,
                                observed.iter().cloned(),
                                observed_residents.iter().cloned(),
                            )
                        {
                            println!(
                                "[node {label}] membership change observed at view {} — cutover to epoch {} at view {}",
                                cutover.observed_view(),
                                cutover.next_epoch(),
                                cutover.cutover_view()
                            );
                            node.set_view_ceiling(cutover.cutover_view());
                        }
                        // a pending upgrade arms the SAME single cutover slot at its
                        // activation height (design §"One boundary carries both
                        // concerns") — never a competing arm: when a membership
                        // cutover already holds the slot `observe_upgrade` returns
                        // Pending and the version flip rides that boundary via the
                        // boundary read in `respawn_if_due`. inert until the module is
                        // registered (`read_upgrade_state` returns baseline/no-pending).
                        let boundary_upgrade = read_upgrade_state(node.host()).await;
                        if let Some(pending) = &boundary_upgrade.pending
                            && let consensus::ObservationOutcome::Scheduled(cutover) =
                                orchestrator.observe_upgrade(engine_view, pending.activation_height)
                        {
                            println!(
                                "[node {label}] upgrade '{}' armed — cutover to epoch {} at view {} (activation height {})",
                                pending.name,
                                cutover.next_epoch(),
                                cutover.cutover_view(),
                                pending.activation_height
                            );
                            node.set_view_ceiling(cutover.cutover_view());
                        }
                        if let Some(plan) = orchestrator.respawn_if_due(
                            engine_view,
                            observed,
                            observed_residents,
                            boundary_upgrade,
                        ) {
                            let members = plan.valset().consensus_members();
                            let member_bytes: Vec<Vec<u8>> =
                                members.iter().map(|k| k.as_ref().to_vec()).collect();
                            let plan_residents: Vec<ed25519::PublicKey> = plan
                                .valset()
                                .transport_members()
                                .difference(members)
                                .cloned()
                                .collect();
                            let plan_resident_bytes: Vec<Vec<u8>> = plan_residents
                                .iter()
                                .map(|k| k.as_ref().to_vec())
                                .collect();
                            // transport FIRST: the new epoch's mesh must admit
                            // its members (a fresh joiner — or a granted
                            // resident — above all) before anything is
                            // expected of them. the mesh tracks the TRANSPORT
                            // union; the engine below gets validators only.
                            // index = epoch, strictly increasing across
                            // cutovers.
                            mesh_oracle.track(plan.epoch(), mesh_at(plan.valset().transport_members()));
                            // the statesync plane serves (and admits) exactly
                            // who the mesh tracks — follow the re-track.
                            if let Some(book) = &sync_plane_book {
                                book.set_peers(plan.valset().transport_members().iter());
                            }
                            // the media planes authenticate inbound by the same
                            // tracked set — follow the re-track too, so a
                            // just-added member's huddle media is admitted.
                            if let Some(peers) = &media_peers {
                                peers.set_peers(plan.valset().transport_members().iter());
                            }
                            // the blob fetch-on-miss lane fans out to the same
                            // tracked set — follow the re-track.
                            *blob_peers.write().expect("blob peers lock") =
                                plan.valset().transport_members().iter().cloned().collect();
                            // the reachability plane retunnels for the new
                            // member set the moment transport admits it —
                            // with the epoch's resident tier as the pre-warm
                            // standbys, so a registered joiner's tunnels
                            // assemble ahead of its activation cutover.
                            // cutover_app_height IS the new epoch's absolute
                            // view at engine view 0 — the raw engine_view
                            // here would be epoch-local, a different clock
                            // than the ViewTicks above and the boot
                            // Retarget's view_base.
                            if reach_cmd.is_some() {
                                // STAGED, not sent inline: the flush below
                                // (every drain beat) try_sends it, so a plane
                                // whose queue is full delays retunneling by
                                // beats — it can never stall the cutover or
                                // the loop.
                                pending_retarget = Some(reachability::MeshEpochEvent {
                                    epoch: plan.epoch(),
                                    members: members.iter().cloned().collect(),
                                    standbys: plan_residents.clone(),
                                    current_view: plan.cutover_app_height(),
                                });
                            }
                            if !members.contains(&signer.public_key()) {
                                println!(
                                    "[node {label}] demoted from the validator set at epoch {} — halting (restart to serve as sync/resident)",
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
                            match node
                                .cutover(
                                    orderer,
                                    plan.epoch(),
                                    plan.cutover_app_height(),
                                    &member_bytes,
                                    &plan_resident_bytes,
                                )
                                .await
                            {
                                // the accept contract crossing the boundary:
                                // every locally-accepted op the old epoch
                                // never resolved was re-proposed into the
                                // new engine.
                                Ok(carried) if carried > 0 => println!(
                                    "[node {label}] carried {carried} accepted ops across the cutover into epoch {}",
                                    plan.epoch()
                                ),
                                Ok(_) => {}
                                Err(e) => {
                                    eprintln!("[node {label}] FATAL: {e} — halting");
                                    std::process::exit(1);
                                }
                            }
                            // ACTIVATION (design §4): realize the agreed boundary
                            // protocol version into every dual-path module's
                            // active_version (branch selector) at H. driven ONLY by
                            // the agreed `plan.boundary_version()` — deterministic,
                            // non-hashed. the upgrade module's OWN committed
                            // reconciliation (current_version flip + pending clear on
                            // ARM, clear-only on ABORT) is NOT done here: it rides the
                            // single in-block System `Advance` the host drain injects
                            // at the same finalized view (Task 6.3), so both concerns
                            // land at ONE boundary and every node agrees. do NOT branch
                            // a separate abort-only follow-up — the one Advance owns both.
                            node.host_mut().set_active_version(plan.boundary_version());
                            match plan.upgrade_verdict() {
                                consensus::UpgradeVerdict::Armed { name, to_version } => println!(
                                    "[node {label}] upgrade activated name={name} version={to_version} at height {}",
                                    plan.cutover_app_height()
                                ),
                                consensus::UpgradeVerdict::Abort { name } => println!(
                                    "[node {label}] upgrade aborted name={name} (unmet readiness) at height {} — network continues on version {}",
                                    plan.cutover_app_height(),
                                    plan.boundary_version()
                                ),
                                consensus::UpgradeVerdict::None => {}
                            }
                            // checkpoint IMMEDIATELY: the manifest must record
                            // the new epoch's participant set (the journal's
                            // cutover record alone covers only the crash
                            // window until this write lands).
                            let pos = node.sink_mut().oplog_pos().await;
                            // post-boundary committed version fields: after an armed
                            // Advance the module holds `current_version = to_version`
                            // + no pending, so this checkpoint stamps the new baseline.
                            let (cv, pu) = read_upgrade_version_fields(node.host()).await;
                            let captured = Manifest::capture(
                                node.host(),
                                node.finalized().map(|f| f.height),
                                orchestrator.epoch(),
                                orchestrator.epoch_base(),
                                participant_bytes(&orchestrator),
                                resident_bytes(&orchestrator),
                                None,
                                cv,
                                pu,
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

                    // BLOCK CADENCE + heartbeat, unified. `submit`/`submit_frame`
                    // now ENQUEUE into the node's `pending_batch`; this is the one
                    // place per block-time that FLUSHES the window — packing every
                    // frame that arrived in it (real ops and/or an idle nop) into
                    // ONE batch super-frame and proposing it as a single block.
                    // that is the aggregation: at most one block per BLOCK_TIME,
                    // carrying all the window's txs, never 1-tx-1-block.
                    //
                    // the idle nop still exists: finalized views only advance with
                    // a proposed frame, so an idle network would freeze (its height
                    // never ticks and a pending cutover, which crosses only when
                    // finalized views REACH it, would park forever). so on an EMPTY
                    // window inject one deterministically-rejected nop (unknown
                    // module target: rejects identically everywhere, leaves no
                    // state) and flush that. a window with real ops needs no nop —
                    // the ops ARE the block.
                    //
                    // GATE the idle nop on an empty orderer FIFO too: a nop pushed
                    // while a batch still awaits finalization only piles behind a
                    // finalization stall (a flapping quorum peer would stack idle
                    // blocks). real ops are never gated — they must not wait.
                    if !heartbeat_disabled && last_flush.elapsed() >= consensus::BLOCK_TIME {
                        last_flush = std::time::Instant::now();
                        if node.pending_batch_len() == 0 && node.orderer().pending_len() == 0 {
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
                                eprintln!("[node {label}] heartbeat nop submit failed: {e}");
                            }
                        }
                        // flush the window: no-op when `pending_batch` is empty
                        // (idle with a batch already in flight — wait for it).
                        if let Err(e) = node.flush_batch().await {
                            eprintln!("[node {label}] batch flush failed: {e}");
                        }
                    }

                    // READINESS SIGNAL (design §3 / plan Task 7.1): a current
                    // boundary member whose binary can execute the pending upgrade
                    // self-submits ONE `SignalReady`. gated to a current member (the
                    // R = n readiness denominator); the signaller's own committed
                    // read + local latch keep it idempotent. inert on a baseline net.
                    if orchestrator
                        .current_members()
                        .contains(&signer.public_key())
                        && let Some((msg, name, to_version)) =
                            signaller.maybe_signal(node.host()).await
                    {
                        let seq = next_seq;
                        next_seq += 1;
                        match node.submit(&signer, seq, msg).await {
                            Ok(_) => println!(
                                "[node {label}] signaled ready name={name} to_version={to_version}"
                            ),
                            Err(e) => {
                                // un-latch so a transient submit failure retries on
                                // the next tick (the module stays idempotent).
                                signaller.signaled = None;
                                eprintln!("[node {label}] readiness signal submit failed: {e}");
                            }
                        }
                    }

                    // CAPABILITY ANNOUNCE: a current member whose discovered
                    // provider set differs from the committed registry
                    // self-submits ONE declarative `Announce`. member-gated (the
                    // module rejects non-members) and idempotent (committed-read
                    // + local latch). inert on a host with no executor CLIs, and
                    // suppressed entirely under `announce_capabilities = false`
                    // (the accept-lane-only provider: this node still executes
                    // what it can, but only by claiming unassigned announcements
                    // — it never enters a tag's rendezvous pool).
                    if announce_capabilities
                        && orchestrator
                            .current_members()
                            .contains(&signer.public_key())
                        && let Some(msg) = announcer.maybe_announce(node.host()).await
                    {
                        let seq = next_seq;
                        next_seq += 1;
                        match node.submit(&signer, seq, msg).await {
                            Ok(_) => println!(
                                "[node {label}] announced capabilities {:?}",
                                announcer.capabilities
                            ),
                            Err(e) => {
                                // un-latch so a transient submit failure retries.
                                announcer.announced = None;
                                eprintln!("[node {label}] capability announce submit failed: {e}");
                            }
                        }
                    }

                    // DUCKDNS ANNOUNCE: same state-driven declarative replace
                    // discipline as capabilities, but an empty local config
                    // deliberately clears any stale prior publication.
                    if orchestrator
                        .current_members()
                        .contains(&signer.public_key())
                        && let Some(msg) = duckdns_announcer.maybe_announce(node.host()).await
                    {
                        let seq = next_seq;
                        next_seq += 1;
                        match node.submit(&signer, seq, msg).await {
                            Ok(_) => println!(
                                "[node {label}] announced DuckDNS services {:?}",
                                duckdns_announcer.announcements()
                            ),
                            Err(e) => {
                                duckdns_announcer.send_failed();
                                eprintln!("[node {label}] DuckDNS announce submit failed: {e}");
                            }
                        }
                    }

                    // SAGA CRANK (P7 liveness, host side): nothing else ever
                    // submits `SagaMsg::Crank`, and under strict leases a
                    // saga whose assignee went dark advances ONLY via a crank
                    // (lease re-lease or deadline timeout). state-driven:
                    // when the committed next expiry is at or past the latest
                    // finalized height, push one permissionless crank —
                    // throttled like the heartbeat, since a backlog wider
                    // than CRANK_BUDGET legitimately needs several. duplicate
                    // cranks from other nodes are deterministic no-ops.
                    if last_crank.elapsed() >= consensus::BLOCK_TIME
                        && let Some(finalized_height) = node.finalized().map(|f| f.height)
                        && let Some(expiry) = saga_next_expiry(node.host()).await
                        && expiry <= finalized_height
                    {
                        last_crank = std::time::Instant::now();
                        let seq = next_seq;
                        next_seq += 1;
                        if let Err(e) = node
                            .submit(
                                &signer,
                                seq,
                                Msg {
                                    target: "saga".into(),
                                    payload: saga::encode_msg(
                                        &saga::SagaMsg::Crank {},
                                    ),
                                },
                            )
                            .await
                        {
                            eprintln!("[node {label}] saga crank submit failed: {e}");
                        } else {
                            println!(
                                "[node {label}] saga crank submitted \
                                 (next expiry {expiry} <= height {finalized_height})"
                            );
                        }
                    }

                    // DISPATCH DELIVERY NUDGE (never-pop-stack liveness): a
                    // result committed into the dispatch mailbox delivers via
                    // the drain's DeliverPending injection in the NEXT
                    // successful block — and heartbeat nops are rejected
                    // frames that never apply, so a quiet chain would sit on
                    // its mailbox. state-driven: while the committed mailbox
                    // is non-empty, push one permissionless Nudge — a no-op
                    // whose block carries the injection. duplicate nudges
                    // from other nodes are free.
                    if last_nudge.elapsed() >= consensus::BLOCK_TIME
                        && dispatch_pending_deliveries(node.host()).await > 0
                    {
                        last_nudge = std::time::Instant::now();
                        let seq = next_seq;
                        next_seq += 1;
                        if let Err(e) = node
                            .submit(
                                &signer,
                                seq,
                                Msg {
                                    target: "dispatch".into(),
                                    payload: dispatch::encode_msg(
                                        &dispatch::DispatchMsg::Nudge {},
                                    ),
                                },
                            )
                            .await
                        {
                            eprintln!("[node {label}] dispatch nudge submit failed: {e}");
                        } else {
                            println!("[node {label}] dispatch delivery nudge submitted");
                        }
                    }

                    // UPGRADE TRANSITION MARKERS (one-shot, committed-state driven):
                    // the greppable proof surface the e2e keys on. `armed` is the
                    // module's own R==n verdict (pending set, boundary non-empty,
                    // every current member signaled), so this fires exactly when
                    // readiness first reaches the full set — before H is crossed.
                    if let Some(st) = read_upgrade_status_raw(node.host()).await {
                        match &st.pending {
                            Some(up) => {
                                upgrade_pending_seen = Some(up.name.clone());
                                let key = (up.name.clone(), up.to_version);
                                if st.armed && upgrade_armed_latch.as_ref() != Some(&key) {
                                    println!(
                                        "[node {label}] upgrade armed name={} to_version={} height={}",
                                        up.name, up.to_version, up.activation_height
                                    );
                                    upgrade_armed_latch = Some(key);
                                }
                            }
                            None => {
                                if let Some(name) = upgrade_pending_seen.take() {
                                    // the boundary Advance reconciled the pending
                                    // (ARM flip or ABORT clear) — the slot is free.
                                    println!("[node {label}] upgrade cleared name={name}");
                                    upgrade_armed_latch = None;
                                }
                            }
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
                                Ok(reactor::WorkOutcome::Handled(Some(follow))) => {
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
                                // a deliberate skip (e.g. leased to another
                                // node): claimed, nothing to submit.
                                Ok(reactor::WorkOutcome::Handled(None)) => {
                                    claimed = true;
                                    break;
                                }
                                Ok(reactor::WorkOutcome::NotMine) => {}
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
                        RpcRequest::JoinRequests => {
                            // read-time hygiene: an approved joiner holds
                            // STANDING now (resident or already validator) —
                            // its request is settled, drop it.
                            let members = read_members_from_host(node.host()).await;
                            let residents_now = read_valset_residents(node.host()).await;
                            join_requests.retain(|joiner, _| {
                                !members.contains(joiner) && !residents_now.contains(joiner)
                            });
                            let views = join_requests
                                .iter()
                                .map(|(joiner, r)| JoinRequestView {
                                    joiner: hex_bytes(joiner),
                                    issuer: hex_bytes(&r.issuer),
                                    first_seen_ms: r.first_seen_ms,
                                    last_seen_ms: r.last_seen_ms,
                                })
                                .collect();
                            RpcReply {
                                join_requests: Some(views),
                                ..RpcReply::ok()
                            }
                        }
                        RpcRequest::Shutdown => {
                            // best-effort final checkpoint + journal barrier so
                            // the restart replays a minimal suffix; a failure
                            // here is just the crash path, which also recovers.
                            // SAME sequence as the signal arm (shared macro).
                            graceful_checkpoint!();
                            let _ = reply.send(RpcReply::ok());
                            println!("[node {label}] shutdown requested via rpc — exiting");
                            std::process::exit(0);
                        }
                    };
                    let _ = reply.send(resp);
                }
                result = oracle_results.next() => {
                    // a completed off-loop provider run: its OracleResult op
                    // re-enters the ordered lane as an ordinary signed
                    // submit — the oracle-as-op, unchanged; only WHERE the
                    // provider ran moved.
                    let Some(msg) = result else { continue };
                    let seq = next_seq;
                    next_seq += 1;
                    if let Err(e) = node.submit(&signer, seq, msg).await {
                        eprintln!("[node {label}] oracle result submit failed: {e}");
                    }
                }
                announce = lobby_ingress.next() => {
                    let Some((peer, bytes)) = announce else { continue };
                    // `fatal: true` marks the refusal PERMANENT for this
                    // invite — the joiner stops re-announcing instead of
                    // spinning on a token that can never redeem.
                    let mut send_reply = |recorded: bool, detail: String, cap: Option<Vec<u8>>, fatal: bool| {
                        let msg = lobby::LobbyMsg::JoinReply { recorded, detail, cap, fatal };
                        let _ = lobby_tx.send(
                            Recipients::One(peer.clone()),
                            IoBuf::from(lobby::encode_msg(&msg)),
                            false,
                        );
                    };
                    let msg = match lobby::decode_msg(&bytes) {
                        Ok(m) => m,
                        Err(_) => continue, // junk on the doorbell — drop.
                    };
                    // crypto first (pure, cheap): the token must verify for
                    // THIS network and the announced key must prove itself.
                    let verified = match lobby::verify_join_request(&msg, &namespace) {
                        Ok(v) => v,
                        Err(e) => {
                            send_reply(false, e, None, false);
                            continue;
                        }
                    };
                    // then membership: the issuer must still be a member (a
                    // removed member's outstanding invites die with it), and a
                    // joiner that already holds standing — VALIDATOR or
                    // RESIDENT — has nothing pending.
                    let members = read_members_from_host(node.host()).await;
                    let residents_now = read_valset_residents(node.host()).await;
                    let joiner_bytes = verified.joiner.as_ref().to_vec();
                    if members.contains(&joiner_bytes) {
                        send_reply(false, "already a validator".into(), None, false);
                        continue;
                    }
                    if residents_now.contains(&joiner_bytes) {
                        send_reply(
                            false,
                            "already a resident — a member promotes it into the quorum".into(),
                            None,
                            false,
                        );
                        continue;
                    }
                    if !members.contains(&verified.issuer.as_ref().to_vec()) {
                        send_reply(
                            false,
                            "the inviting member is no longer part of this network".into(),
                            None,
                            false,
                        );
                        continue;
                    }
                    // SPENT-INVITE check: the token's nonce is the
                    // exactly-once key (governance's Redeem handler). a nonce
                    // already redeemed by ANOTHER key can never redeem again —
                    // resubmitting the op is pointless and the joiner would
                    // spin on "redemption not landed yet" forever. fail it
                    // loudly and permanently on both ends instead. (redeemed
                    // by the SAME key = standing already granted; the
                    // validator/resident checks above answered that.)
                    let redemptions = read_redemptions_from_host(node.host()).await;
                    if let Some(spent) = redemptions
                        .iter()
                        .find(|r| r.nonce == verified.nonce.as_slice() && r.joiner != joiner_bytes)
                    {
                        println!(
                            "[node {label}] lobby: {} presented an ALREADY-REDEEMED invite \
                             (spent by {} at height {}) — refusing permanently; an invite \
                             admits exactly one person, mint a fresh one per joiner",
                            hex_bytes(&joiner_bytes[..4]),
                            hex_bytes(&spent.joiner[..4.min(spent.joiner.len())]),
                            spent.height,
                        );
                        send_reply(
                            false,
                            "invite already redeemed — an invite admits exactly one person; \
                             ask the inviter for a fresh invite"
                                .into(),
                            None,
                            true,
                        );
                        continue;
                    }
                    // AUTO-REDEMPTION: minting the invite WAS the approval, so
                    // a verified announce submits the governance Redeem op on
                    // the joiner's behalf — no human step. every validator
                    // re-verifies the token in-consensus and the nonce set
                    // makes it single-use, so racing members (the joiner
                    // round-robins its announce) collapse to one grant and
                    // deterministic rejects. the in-memory map only throttles
                    // re-submits across the joiner's ~3s re-announces.
                    let now = unix_ms();
                    let fresh = !join_requests.contains_key(&joiner_bytes);
                    let record = join_requests
                        .entry(joiner_bytes)
                        .or_insert(JoinRequestRecord {
                            issuer: verified.issuer.as_ref().to_vec(),
                            first_seen_ms: now,
                            last_seen_ms: 0,
                        });
                    // MINT the coordinator capability for the joiner, additive
                    // and side-effect-free (a pure ed25519 sign — no consensus,
                    // no valset change). Gated: only a GENESIS validator on a
                    // PRIVATE network issues one — its key is in the
                    // coordinator's pinned genesis set, so the cap it signs
                    // actually admits. A public network needs no cap; a
                    // non-genesis member cannot mint one the coordinator trusts.
                    // The cap cannot ride the invite (the joiner's key did not
                    // exist at invite-mint time), so the JoinReply is its only
                    // delivery channel — re-delivered on every re-announce in
                    // case a reply was lost. Rotation is DEFERRED — the cap is
                    // long-lived (COORD_CAP_TTL_SECS).
                    let minted_cap = if coordination == config::Coordination::Private
                        && validators.contains(&signer.public_key())
                    {
                        let mut subj = [0u8; 32];
                        subj.copy_from_slice(verified.joiner.as_ref());
                        let cap = nat_traversal::mint_coord_cap(
                            &signer,
                            nat_traversal::NodeKey(subj),
                            nat_traversal::now_secs() + nat_traversal::COORD_CAP_TTL_SECS,
                        );
                        Some(config::pack_coord_cap(&cap))
                    } else {
                        None
                    };
                    const REDEEM_RESUBMIT_MS: u64 = 30_000;
                    if !fresh && now.saturating_sub(record.last_seen_ms) < REDEEM_RESUBMIT_MS {
                        send_reply(
                            true,
                            "redemption in flight — standing lands shortly".into(),
                            minted_cap,
                            false,
                        );
                        continue;
                    }
                    record.last_seen_ms = now;
                    let redeem = governance::GovMsg::Redeem {
                        issuer: verified.issuer.as_ref().to_vec(),
                        nonce: verified.nonce.to_vec(),
                        token_sig: match &msg {
                            lobby::LobbyMsg::JoinRequest { token_sig, .. } => token_sig.clone(),
                            _ => unreachable!("verified above"),
                        },
                        joiner: verified.joiner.as_ref().to_vec(),
                        proof: match &msg {
                            lobby::LobbyMsg::JoinRequest { proof, .. } => proof.clone(),
                            _ => unreachable!("verified above"),
                        },
                    };
                    let seq = next_seq;
                    next_seq += 1;
                    match node
                        .submit(
                            &signer,
                            seq,
                            Msg {
                                target: "governance".into(),
                                payload: governance::encode_msg(&redeem),
                            },
                        )
                        .await
                    {
                        Ok(_) => {
                            println!(
                                "[node {label}] invite redemption submitted: {} (invited by {})",
                                hex_bytes(verified.joiner.as_ref()),
                                hex_bytes(verified.issuer.as_ref())
                            );
                            send_reply(
                                true,
                                "invite verified — redemption submitted, resident standing \
                                 lands at the next block"
                                    .into(),
                                minted_cap,
                                false,
                            );
                        }
                        Err(e) => {
                            send_reply(false, format!("redemption submit failed: {e}"), None, false);
                        }
                    }
                }
                relayed = relay_ingress.next() => {
                    let Some((peer, bytes)) = relayed else { continue };
                    let Ok(msg) = relay::decode_msg(&bytes) else { continue };
                    let needs_standing = matches!(
                        msg,
                        relay::RelayMsg::BlobOffer { .. } | relay::RelayMsg::Submit { .. }
                    );
                    let (members_now, residents_now) = if needs_standing {
                        (
                            read_valset_members(node.host()).await,
                            read_valset_residents(node.host()).await,
                        )
                    } else {
                        (Vec::new(), Vec::new())
                    };
                    let Some(action) = validator_relay.on_message(
                        peer,
                        msg,
                        &members_now,
                        &residents_now,
                        &mut relay_tx,
                    ) else {
                        continue;
                    };
                    match action {
                        relay_runtime::ValidatorAction::SubmitResident {
                            frame_id,
                            frame,
                            peer,
                        } => match node.submit_frame(frame).await {
                            Ok(id) => {
                                debug_assert_eq!(id, frame_id);
                                pending_relays.insert(
                                    id,
                                    (peer, std::time::Instant::now() + SUBMIT_HOLD),
                                );
                            }
                            Err(e) => relay_runtime::send_reply(
                                &mut relay_tx,
                                &peer,
                                frame_id,
                                relay::RelayOutcome::Refused {
                                    detail: format!("submit failed: {e}"),
                                },
                            ),
                        },
                        relay_runtime::ValidatorAction::SubmitLocal {
                            frame_id,
                            frame,
                            reply,
                            deadline,
                        } => match node.submit_frame(frame).await {
                            Ok(id) => {
                                debug_assert_eq!(id, frame_id);
                                pending_submits.insert(id, (reply, deadline));
                            }
                            Err(e) => {
                                let _ = reply.send(Err(format!("submit failed: {e}")));
                            }
                        },
                    }
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
                            let frame = node::encode_frame(&signer, seq, &Msg { target, payload });
                            let peers: Vec<ed25519::PublicKey> =
                                if relay::required_blob_digest(&frame).is_some() {
                                    read_valset_members(node.host())
                                        .await
                                        .iter()
                                        .filter_map(|raw| {
                                            ed25519::PublicKey::decode(raw.as_slice()).ok()
                                        })
                                        .filter(|key| key != &signer.public_key())
                                        .collect()
                                } else {
                                    Vec::new()
                                };
                            match validator_relay.prepare_local(
                                frame,
                                reply,
                                peers,
                                &mut relay_tx,
                            ) {
                                Ok(Some(relay_runtime::ValidatorAction::SubmitLocal {
                                    frame_id,
                                    frame,
                                    reply,
                                    deadline,
                                })) => match node.submit_frame(frame).await {
                                    Ok(id) => {
                                        debug_assert_eq!(id, frame_id);
                                        pending_submits.insert(id, (reply, deadline));
                                    }
                                    Err(e) => {
                                        let _ = reply.send(Err(format!("submit failed: {e}")));
                                    }
                                },
                                Ok(Some(relay_runtime::ValidatorAction::SubmitResident { .. })) => {
                                    unreachable!("local preparation returns a local action")
                                }
                                Ok(None) => {}
                                Err((reply, detail)) => {
                                    let _ = reply.send(Err(detail));
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
                                    category: noded::ModuleCategory::of(m),
                                })
                                .collect();
                            let _ = reply.send(noded::NodeStatus {
                                version: env!("CARGO_PKG_VERSION").into(),
                                app_hash: hex(&node.app_hash()),
                                height: node.finalized().map(|f| f.height).unwrap_or(0),
                                modules,
                                public_key: status_public_key.clone(),
                            });
                        }
                        noded::NodeCommand::Metrics { reply } => {
                            // one registry: commonware's runtime series plus the
                            // `ducktape_*` block series the drain loop records.
                            let _ = reply.send(context.encode());
                        }
                    }
                }
                req = sync_state_rx.next() => {
                    // the statesync serve task's state touches (the
                    // [`SyncStateRequest`] seam): each is one bounded read
                    // against loop-owned state — the heavy serving (decode,
                    // captures, slicing, replies) lives on the serve task.
                    let Some(req) = req else {
                        // the serve task ended (network shutdown) — nothing
                        // left to answer; keep draining consensus regardless.
                        continue;
                    };
                    match req {
                        SyncStateRequest::Boundary { known, reply } => {
                            // the boundary's consensus coordinates ride the manifest.
                            // the floor certificate is served only when it certifies
                            // exactly the current boundary — a cert behind the
                            // boundary would make a joiner skip history it needs.
                            // stamp the served boundary's committed version fields from
                            // live upgrade state (like epoch/view_base). a joiner installs
                            // its dual-path modules at `current_version` and preflights
                            // against `required_min_version` — both derived from these.
                            let (bc_current, bc_pending) =
                                read_upgrade_version_fields(node.host()).await;
                            let coords = statesync::BoundaryCoords {
                                epoch: orchestrator.epoch(),
                                view_base: orchestrator.epoch_base(),
                                participants: participant_bytes(&orchestrator),
                                residents: resident_bytes(&orchestrator),
                                current_version: bc_current,
                                pending_upgrade: bc_pending,
                                floor_cert: latest_floor
                                    .as_ref()
                                    .filter(|fc| fc.epoch == orchestrator.epoch())
                                    .filter(|fc| {
                                        node.finalized().is_some_and(|f| f.height == fc.height)
                                    })
                                    .map(|fc| fc.cert.clone()),
                            };
                            let finalized_for_sync = node.finalized().filter(|f| {
                                f.height <= coords.view_base || coords.floor_cert.is_some()
                            });
                            let answer = match finalized_for_sync {
                                // two refusals, named apart: no boundary at
                                // all (pre-first-block), vs the per-block
                                // window where the tip advanced but its
                                // finalization certificate has not persisted
                                // yet — a retry lands once they align.
                                None => Err(match node.finalized() {
                                    Some(f) => format!(
                                        "boundary {} awaiting its finalization certificate — \
                                         retry",
                                        f.height
                                    ),
                                    None => "no finalized boundary to serve yet".to_string(),
                                }),
                                Some(finalized) => {
                                    let id = statesync::BoundaryId {
                                        height: finalized.height,
                                        app_hash: finalized.app_hash,
                                    };
                                    if known.contains(&id) {
                                        // the serve task holds this boundary's
                                        // payload — coordinates only.
                                        Ok(SyncBoundary { id, coords, data: None })
                                    } else {
                                        statesync::capture_boundary(
                                            node.host(),
                                            finalized,
                                            &coords,
                                        )
                                        .await
                                        .map(|(id, data)| SyncBoundary {
                                            id,
                                            coords,
                                            data: Some(data),
                                        })
                                    }
                                }
                            };
                            let _ = reply.send(answer);
                        }
                        SyncStateRequest::ModuleServe { module_id, body, reply } => {
                            let served = node
                                .host()
                                .serve_sync(&module_id, &body)
                                .await
                                .map_err(|e| format!("module {module_id} serve_sync: {e}"));
                            let _ = reply.send(served);
                        }
                        SyncStateRequest::Frames { after_height, up_to_height, reply } => {
                            let read = node
                                .sink_mut()
                                .read_finalized_frames(after_height, up_to_height)
                                .await;
                            let _ = reply.send(read);
                        }
                        SyncStateRequest::IndexCut { reply } => {
                            let _ = reply.send(ship_index_blobs(&index, &label));
                        }
                        SyncStateRequest::TipCoords { reply } => {
                            // the detection lane: everything here is already
                            // loop-owned state — no capture, and deliberately
                            // no floor-cert alignment gate. that gate protects
                            // a JOINER from syncing a boundary whose history
                            // it would skip; a detection reply carries a
                            // presence bit, never certificate bytes, and every
                            // action taken on it (ascension, promotion)
                            // re-fetches a full manifest through the gated
                            // Boundary path.
                            let answer = match node.finalized() {
                                None => Err("no finalized boundary to serve yet".to_string()),
                                Some(f) => Ok(statesync::TipCoords {
                                    height: f.height,
                                    app_hash: f.app_hash,
                                    epoch: orchestrator.epoch(),
                                    view_base: orchestrator.epoch_base(),
                                    participants: participant_bytes(&orchestrator),
                                    residents: resident_bytes(&orchestrator),
                                    has_floor: latest_floor
                                        .as_ref()
                                        .filter(|fc| fc.epoch == orchestrator.epoch())
                                        .is_some_and(|fc| fc.height == f.height),
                                }),
                            };
                            let _ = reply.send(answer);
                        }
                    }
                }
            }
        }
    });

    Ok(())
}
