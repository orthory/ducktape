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

use agent::AgentModule;
use agent_oracle::LlmWorker;
use commonware_codec::DecodeExt as _;
use commonware_consensus::simplex::scheme::ed25519 as simplex_ed25519;
use commonware_consensus::types::Epoch;
use commonware_cryptography::{Signer, ed25519};
use commonware_p2p::authenticated::discovery::{self, Network};
use commonware_p2p::{Ingress, Manager, Receiver as P2pReceiver, Recipients, Sender as P2pSender};
use commonware_runtime::{Clock, IoBuf, Metrics, Quota, Runner, Spawner, Supervisor};
use commonware_utils::{NZU32, ordered::Set};
use futures::{FutureExt as _, StreamExt as _};

use consensus::{ConsensusScheme, ContentStore, Digest, SimplexOrderer, digest_of};

mod config;
mod lobby;
use config::{Resolved, WireGuardEffectKind, hex_bytes, unhex};

/// the consensus signature scheme this build runs — a genesis-wide constant. today only
/// V1 (ed25519); see [`ConsensusScheme`]'s rekey/respawn contract for the BLS/V2 path.
const CONSENSUS_SCHEME: ConsensusScheme = ConsensusScheme::V1Ed25519;
/// the highest protocol version THIS binary's dual-path modules can execute — a
/// per-node BUILD constant, NEVER consensus state (a lying value can only
/// refuse-to-boot or halt this one node, never fork the network). the
/// `ReadinessSignaller` truthfully signals readiness for a pending upgrade iff
/// `MAX_PROTOCOL_VERSION >= to_version`, and the boot preflight refuses a boundary
/// whose `required_min_version` exceeds it. Phase 9 raised this to 2 when the
/// forge v2 dual path landed — this binary can execute a scheduled `to_version=2`
/// (the forge multi-repo-v2 root/snapshot divergence) and truthfully `SignalReady`.
const MAX_PROTOCOL_VERSION: u32 = 2;
use automations::Automations;
use capability::CapabilityRegistry;
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
use pages::Pages;
use profiles::Profiles;
use recovery::{Manifest, Recovery};
use saga::{LeasePolicy, SagaModule};
use sdk::{ModuleId, Msg, StateRoot};
use statesync::p2p::P2pSyncClient;
use statesync::qmdb::RemoteQmdbResolver;
use statesync::{SyncServer, fetch_frames, fetch_manifest, fetch_snapshot};
use tasks::Tasks;
use upgrade::Upgrade;
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
/// a deliberately-unregistered module target. validators submit empty frames
/// against it to advance the chain when there are no user ops: finalized views
/// only move with ops, so an idle network would otherwise freeze — parking AT an
/// armed cutover boundary forever, and never ticking the height the console
/// shows. the frame finalizes, rejects deterministically on every node (unknown
/// module), advances the engine clock, and leaves no state.
const NOP_TARGET: &str = "consensus.nop";
/// heartbeat cadence: how often a node pushes a [`NOP_TARGET`] frame so an idle
/// chain still finalizes blocks (its height keeps ticking) and any pending
/// cutover still crosses. one block/sec while idle is the accepted cost of a
/// visibly-live height.
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(1);
/// request timeout for the promoted-validator boot catch-up client. it is long
/// enough to let discovery links warm, but bounded so boot cannot hang forever
/// before the statesync server bridge is installed.
const BOOT_SYNC_REQUEST_TIMEOUT: Duration = Duration::from_secs(6);
/// how often the reachability plane re-offers whatever it still waits on —
/// un-acked gossip while assembling, stalled handshake messages after
/// verification (`ReachabilityCommand::Nudge`). fast enough that a lost
/// message costs one beat of mesh convergence, slow enough to be noise-free
/// — the nudge is a no-op once the epoch's handshakes have all completed.
const NUDGE_INTERVAL: Duration = Duration::from_secs(2);
/// post-reboot catch-up should close the reboot gap, not chase a live chain
/// forever. any tiny lag left after this cap is handled by the normal engine.
const POST_REBOOT_CATCHUP_MAX_ITERS: usize = 8;
/// max wire message size we accept on a channel (1 MiB) — generous for the small
/// json frames + BFT metadata, and the statesync chunk size (256 KiB) plus
/// framing stays far below it.
const MAX_MESSAGE_SIZE: u32 = 1 << 20;
/// inbound backlog before a channel applies receive backpressure.
const MAX_BACKLOG: usize = 128;
/// pump drain cadence: how often the pump applies finalized frames (and runs
/// everything that rides the drain arm — checkpoints, valset orchestration,
/// the epoch cutover, the heartbeat). enforced as a FLOOR via an absolute
/// deadline in the pump loop: ingress load can delay one drain by one
/// request's service time, but can never starve the arm.
const DRAIN_TICK: Duration = Duration::from_millis(100);
/// the statesync rpc channel: joiners request manifests / snapshot chunks /
/// qmdb op-ranges here; validators answer between drains.
const CHANNEL_STATE_SYNC: u64 = 4;
/// the lobby channel: a not-yet-admitted joiner (connected as the derived
/// lobby identity) announces `{invite token, pubkey, proof}` here; members
/// verify and RECORD the join request for manual approval, and answer with an
/// informational reply. see the `lobby` module.
const CHANNEL_LOBBY: u64 = 5;
/// the reachability channel: members gossip WireGuard endpoint records and
/// signed advertisements and run the tunnel-upgrade handshake here (the
/// `reachability` crate's staged node-driven WireGuard plane). registered in
/// EVERY mode — an unregistered channel is a protocol violation that kills
/// the sender's connection — and black-holed where the plane does not run.
const CHANNEL_REACHABILITY: u64 = 6;
/// while parked and un-admitted, re-announce every N park-loop attempts
/// (attempts tick ~2s apart, so this is roughly every 10s) — often enough to
/// survive member restarts (the request queue is in-memory), quiet enough to
/// stay out of the members' way.
const LOBBY_ANNOUNCE_EVERY: usize = 5;
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
const MODULE_IDS: [&str; 20] = [
    "kv",
    "document",
    "pages",
    "chat",
    "forge",
    "valset",
    "governance",
    "upgrade",
    "saga",
    "capability",
    "tasks",
    "vaults",
    "profiles",
    "inbox",
    "directory",
    "automations",
    "files",
    "memory",
    "jobs",
    "agent",
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
        Ok(_) | Err(_) => Vec::new(),
    }
}

/// read the full committed membership picture — `(active, standby)`. the
/// active list is the consensus-quorum projection; the union is what the
/// transport mesh tracks.
async fn read_valset_membership(host: &Host) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    use valset_interface::{ValsetQuery, ValsetReply, decode_reply, encode_query};
    let Ok(reply) = host
        .query("valset", &encode_query(&ValsetQuery::Members))
        .await
    else {
        return (Vec::new(), Vec::new());
    };
    match decode_reply(&reply) {
        Ok(ValsetReply::Members { active, standby }) => (active, standby),
        Ok(_) | Err(_) => (Vec::new(), Vec::new()),
    }
}

/// read the upgrade module's committed state as the boundary snapshot the
/// orchestrator reads at a finalized boundary (committed state — called between
/// drains, outside any block). the readiness keys are projected into decoded
/// ed25519 pubkeys (an undecodable key is dropped — dead weight, exactly like the
/// module). falls back to the baseline (no pending) when the module is absent
/// (pre-retrofit) or the reply is unreadable, so this never forks on a decode slip
/// — matching `Host::effective_version`'s graceful fallback.
async fn read_upgrade_state(host: &Host) -> consensus::BoundaryUpgrade<ed25519::PublicKey> {
    use upgrade_interface::{UpgradeQuery, UpgradeReply, decode_reply, encode_query};
    let baseline = || consensus::BoundaryUpgrade::baseline(host::BASELINE_VERSION);
    let Ok(reply) = host
        .query("upgrade", &encode_query(&UpgradeQuery::Status))
        .await
    else {
        return baseline();
    };
    let reply = match decode_reply(&reply) {
        Ok(r) => r,
        Err(_) => return baseline(),
    };
    let UpgradeReply::Status(status) = reply;
    let pending = status.pending.map(|up| {
        let ready: std::collections::BTreeSet<ed25519::PublicKey> = status
            .ready
            .iter()
            .filter_map(|k| ed25519::PublicKey::decode(k.as_slice()).ok())
            .collect();
        consensus::PendingUpgrade {
            name: up.name,
            activation_height: up.activation_height,
            to_version: up.to_version,
            ready,
        }
    });
    consensus::BoundaryUpgrade {
        current_version: status.current_version,
        pending,
    }
}

/// read the upgrade module's committed `current_version` + single pending upgrade
/// as the manifest MIRROR the recovery/statesync captures carry (committed state —
/// called between drains, outside any block). falls back to the baseline (version 0,
/// no pending) when the module is absent (pre-retrofit) or the reply is unreadable,
/// so a checkpoint is never mis-stamped into a decode slip. keeping this a pure
/// committed read (not the raw orchestrator state) means a checkpoint captures the
/// same fields a live node would derive at that height.
async fn read_upgrade_version_fields(host: &Host) -> (u32, Option<sdk::UpgradeCoords>) {
    use upgrade_interface::{UpgradeQuery, UpgradeReply, decode_reply, encode_query};
    let baseline = (host::BASELINE_VERSION, None);
    let Ok(reply) = host
        .query("upgrade", &encode_query(&UpgradeQuery::Status))
        .await
    else {
        return baseline;
    };
    let reply = match decode_reply(&reply) {
        Ok(r) => r,
        Err(_) => return baseline,
    };
    let UpgradeReply::Status(status) = reply;
    let pending = status.pending.map(|up| sdk::UpgradeCoords {
        name: up.name,
        activation_height: up.activation_height,
        to_version: up.to_version,
    });
    (status.current_version, pending)
}

/// read the upgrade module's raw committed [`UpgradeStatus`] (committed state,
/// between drains). `None` when the module is absent (pre-retrofit) or the reply
/// is unreadable — so the transition-marker latches degrade to silent on a
/// baseline net, never panicking.
async fn read_upgrade_status_raw(host: &Host) -> Option<upgrade_interface::UpgradeStatus> {
    use upgrade_interface::{UpgradeQuery, UpgradeReply, decode_reply, encode_query};
    let reply = host
        .query("upgrade", &encode_query(&UpgradeQuery::Status))
        .await
        .ok()?;
    let UpgradeReply::Status(status) = decode_reply(&reply).ok()?;
    Some(status)
}

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
    fn decide(&mut self, status: &upgrade_interface::UpgradeStatus) -> Option<(String, u32)> {
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
        use upgrade_interface::{
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
        use capability_interface::{
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

/// the committed saga ledger's earliest pending lease-expiry/deadline — the
/// crank pump's read. `None` when the module is absent or nothing pending
/// carries one.
async fn saga_next_expiry(host: &Host) -> Option<u64> {
    use saga_interface::{SagaQuery, SagaReply, decode_reply, encode_query};
    let reply = host
        .query("saga", &encode_query(&SagaQuery::NextExpiry))
        .await
        .ok()?;
    match decode_reply(&reply).ok()? {
        SagaReply::NextExpiry(v) => v,
        _ => None,
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
    blobs: files::BlobHandle,
) -> Host {
    let kv = Kv::init(context.child("kv"), "kv").await;
    let document = Document::init(context.child("document"), "document").await;
    let pages = Pages::init(context.child("pages"), "pages").await;
    let chat = Chat::init(context.child("chat"), "chat").await;
    // forge shares the files body plane so a Push's packfile (staged on the blob
    // lane before submit) can materialize locally; the pack never touches root.
    let forge =
        Forge::with_blobs("forge", forge_repo.to_path_buf(), blobs.clone()).expect("forge init");
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
        Box::new(pages),
        Box::new(chat),
        Box::new(forge),
        Box::new(valset),
        // governance is the SOLE authorized author of valset changes: member
        // proposals + ballots, deterministic tally, follow-up membership ops.
        Box::new(Governance::new("governance", "valset", "upgrade")),
        // the no-downtime upgrade coordinator: holds the at-most-one pending
        // upgrade + per-validator readiness set (valset-gated). its mere
        // presence in the registry is its genesis app-hash contribution.
        Box::new(Upgrade::new("upgrade", "valset")),
        // capability-aware strict leases: a saga whose trigger names a
        // capability is assigned over that tag's announced providers, and
        // only the assignee's result lands (no assignee = accept-any).
        Box::new(SagaModule::with_assignment(
            "saga",
            "valset",
            "capability",
            LeasePolicy::Strict,
        )),
        // the network-wide registry of node host capabilities ("codex",
        // "claude", ...): member-gated self-announcements, so every node holds
        // an identical view of who can run what. its genesis contribution is an
        // empty registry (ZERO root) until nodes announce.
        Box::new(CapabilityRegistry::new("capability", Some("valset".into()))),
        Box::new(Tasks::new("tasks")),
        Box::new(Vaults::new("vaults")),
        // the origin-gated display-name registry: each verified submit origin
        // may set its own name, so the ui can resolve authors to names.
        Box::new(Profiles::new("profiles")),
        // per-member notification queues; other modules deliver via follow-up
        // ops so a notification commits atomically with the causing event (P2).
        Box::new(Inbox::new("inbox")),
        Box::new(Files::with_blobs("files", blobs)),
        // the shared agent workspace: a filesystem-shaped namespace with
        // write-once publish, immutable generations, snapshots, and watches.
        Box::new(Memory::new("memory", "files")),
        Box::new(Jobs::new("jobs")),
        Box::new(AgentModule::new(
            "agent",
            "chat",
            "saga",
            Some("tasks".into()),
            Some("jobs".into()),
        )),
        Box::new(Directory::new("directory")),
        // user-defined rules over chat posts: trusts the "chat" origin for hook
        // events and emits chat/tasks follow-ups.
        Box::new(Automations::new(
            "automations",
            "chat",
            "tasks",
            "inbox",
            "memory",
        )),
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
    blobs: files::BlobHandle,
) -> Result<Host, String> {
    let kv = Kv::init(context.child("kv"), "kv").await;
    let document = Document::init(context.child("document"), "document").await;
    let pages = Pages::init(context.child("pages"), "pages").await;
    let chat = Chat::init(context.child("chat"), "chat").await;
    // forge shares the files body plane (see genesis_host) for Push materialization.
    let mut forge = Forge::with_blobs("forge", forge_repo.to_path_buf(), blobs.clone())
        .map_err(|e| format!("forge: {e}"))?;
    // establish the checkpoint boundary's dual-path branch selector so the
    // restored forge `root()` matches at any block the replay SKIPS (disk already
    // held it) before the first replayed block re-derives it per height. the
    // checkpoint is a settled boundary, so `current_version` IS its effective
    // version. baseline no-op before Phase 9.
    forge.set_active_version(manifest.current_version);

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
    valset
        .install(bytes, root)
        .map_err(|e| format!("valset install: {e}"))?;

    let mut governance = Governance::new("governance", "valset", "upgrade");
    let (bytes, root) = snapshot_of("governance")?;
    governance
        .install(bytes, root)
        .map_err(|e| format!("governance install: {e}"))?;

    let mut upgrade = Upgrade::new("upgrade", "valset");
    let (bytes, root) = snapshot_of("upgrade")?;
    upgrade
        .install(bytes, root)
        .map_err(|e| format!("upgrade install: {e}"))?;

    let mut saga = SagaModule::with_assignment("saga", "valset", "capability", LeasePolicy::Strict);
    let (bytes, root) = snapshot_of("saga")?;
    saga.install(bytes, root)
        .map_err(|e| format!("saga install: {e}"))?;

    let mut capability = CapabilityRegistry::new("capability", Some("valset".into()));
    let (bytes, root) = snapshot_of("capability")?;
    capability
        .install(bytes, root)
        .map_err(|e| format!("capability install: {e}"))?;

    let mut tasks = Tasks::new("tasks");
    let (bytes, root) = snapshot_of("tasks")?;
    tasks
        .install(bytes, root)
        .map_err(|e| format!("tasks install: {e}"))?;

    let mut vaults = Vaults::new("vaults");
    let (bytes, root) = snapshot_of("vaults")?;
    vaults
        .install(bytes, root)
        .map_err(|e| format!("vaults install: {e}"))?;

    let mut profiles = Profiles::new("profiles");
    let (bytes, root) = snapshot_of("profiles")?;
    profiles
        .install(bytes, root)
        .map_err(|e| format!("profiles install: {e}"))?;

    let mut inbox = Inbox::new("inbox");
    let (bytes, root) = snapshot_of("inbox")?;
    inbox
        .install(bytes, root)
        .map_err(|e| format!("inbox install: {e}"))?;

    let mut files = Files::with_blobs("files", blobs);
    let (bytes, root) = snapshot_of("files")?;
    files
        .install(bytes, root)
        .map_err(|e| format!("files install: {e}"))?;

    let mut memory = Memory::new("memory", "files");
    let (bytes, root) = snapshot_of("memory")?;
    memory
        .install(bytes, root)
        .map_err(|e| format!("memory install: {e}"))?;

    let mut jobs = Jobs::new("jobs");
    let (bytes, root) = snapshot_of("jobs")?;
    jobs.install(bytes, root)
        .map_err(|e| format!("jobs install: {e}"))?;

    let mut agent = AgentModule::new(
        "agent",
        "chat",
        "saga",
        Some("tasks".into()),
        Some("jobs".into()),
    );
    let (bytes, root) = snapshot_of("agent")?;
    agent
        .install(bytes, root)
        .map_err(|e| format!("agent install: {e}"))?;

    let mut directory = Directory::new("directory");
    let (bytes, root) = snapshot_of("directory")?;
    directory
        .install(bytes, root)
        .map_err(|e| format!("directory install: {e}"))?;

    let mut automations = Automations::new("automations", "chat", "tasks", "inbox", "memory");
    let (bytes, root) = snapshot_of("automations")?;
    automations
        .install(bytes, root)
        .map_err(|e| format!("automations install: {e}"))?;

    Host::genesis(vec![
        Box::new(kv),
        Box::new(document),
        Box::new(pages),
        Box::new(chat),
        Box::new(forge),
        Box::new(valset),
        Box::new(governance),
        Box::new(upgrade),
        Box::new(saga),
        Box::new(capability),
        Box::new(tasks),
        Box::new(vaults),
        Box::new(profiles),
        Box::new(inbox),
        Box::new(files),
        Box::new(memory),
        Box::new(jobs),
        Box::new(agent),
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
/// fetch and root-verify JUST the valset module's snapshot at `manifest`'s
/// boundary — the lightweight probe a parked joiner runs to learn whether its
/// key has been registered as STANDBY (registration shows in the valset
/// state, not in the manifest's engine participant set). returns
/// `(active, standby)` raw key bytes; the manifest entry root is the trust
/// anchor, exactly as in a full sync.
async fn read_standby_membership<C: statesync::SyncClient>(
    client: &C,
    manifest: &statesync::Manifest,
) -> Result<(Vec<Vec<u8>>, Vec<Vec<u8>>), String> {
    let entry = manifest
        .entry("valset")
        .ok_or_else(|| "valset missing from the manifest".to_string())?;
    let bytes = statesync::fetch_snapshot(client, manifest.boundary_id(), "valset")
        .await
        .map_err(|e| format!("valset snapshot: {e}"))?;
    let mut scratch = Valset::new("valset");
    scratch
        .install(&bytes, entry.root)
        .map_err(|e| format!("valset snapshot verify: {e}"))?;
    Ok(scratch.membership())
}

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
    let scratch_context = context.child(Box::leak(
        format!("sync_scratch_a{attempt}").into_boxed_str(),
    ));
    let child_label = |name: &str| -> &'static str {
        Box::leak(format!("{name}_scratch_a{attempt}").into_boxed_str())
    };
    let pinned_target = |module: &'static str| -> Result<statesync::qmdb::SyncTarget, String> {
        let entry = manifest
            .entry(module)
            .ok_or_else(|| format!("module {module} missing from the manifest"))?;
        let pinned = entry
            .resolver_target
            .as_ref()
            .ok_or_else(|| format!("module {module} missing pinned resolver target"))?;
        pinned.to_sync_target().map_err(|e| format!("{module} {e}"))
    };

    // resolver lane: adopt the manifest's pinned target, then fetch only
    // boundary-scoped op batches through the remote resolver.
    let fetch_target = |module: &'static str| {
        let resolver = RemoteQmdbResolver::new(client.clone(), manifest.boundary_id(), module);
        async move {
            let target = pinned_target(module)?;
            let root = entry_root(module)?;
            if StateRoot(target.root.0) != root {
                return Err(format!(
                    "{module} pinned target root does not match the manifest root"
                ));
            }
            Ok::<_, String>((target, resolver))
        }
    };

    let (target, resolver) = fetch_target("kv").await?;
    let kv = Kv::sync_from(
        scratch_context.child(child_label("kv")),
        "kv",
        target,
        resolver,
    )
    .await?;

    let (target, resolver) = fetch_target("document").await?;
    let document = Document::sync_from(
        scratch_context.child(child_label("document")),
        "document",
        target,
        resolver,
    )
    .await?;

    let (target, resolver) = fetch_target("pages").await?;
    let pages = Pages::sync_from(
        scratch_context.child(child_label("pages")),
        "pages",
        target,
        resolver,
    )
    .await?;

    let (target, resolver) = fetch_target("chat").await?;
    let chat = Chat::sync_from(
        scratch_context.child(child_label("chat")),
        "chat",
        target,
        resolver,
    )
    .await?;

    // snapshot lane: chunked bytes from the captured boundary, install gated
    // on the manifest root (verify-then-adopt inside each module).
    let snapshot_of = |module: &'static str| {
        let client = client.clone();
        let boundary = manifest.boundary_id();
        let root = entry_root(module);
        async move {
            let root = root?;
            let bytes = fetch_snapshot(&client, boundary, module)
                .await
                .map_err(|e| format!("{module} snapshot: {e}"))?;
            Ok::<_, String>((bytes, root))
        }
    };

    let (bytes, root) = snapshot_of("directory").await?;
    let mut directory = Directory::new("directory");
    directory
        .install(&bytes, root)
        .map_err(|e| format!("directory install: {e}"))?;

    let (bytes, root) = snapshot_of("valset").await?;
    let mut valset = Valset::new("valset");
    valset
        .install(&bytes, root)
        .map_err(|e| format!("valset install: {e}"))?;

    let (bytes, root) = snapshot_of("saga").await?;
    let mut saga = SagaModule::with_assignment("saga", "valset", "capability", LeasePolicy::Strict);
    saga.install(&bytes, root)
        .map_err(|e| format!("saga install: {e}"))?;

    let (bytes, root) = snapshot_of("capability").await?;
    let mut capability = CapabilityRegistry::new("capability", Some("valset".into()));
    capability
        .install(&bytes, root)
        .map_err(|e| format!("capability install: {e}"))?;

    let (bytes, root) = snapshot_of("governance").await?;
    let mut governance = Governance::new("governance", "valset", "upgrade");
    governance
        .install(&bytes, root)
        .map_err(|e| format!("governance install: {e}"))?;

    let (bytes, root) = snapshot_of("upgrade").await?;
    let mut upgrade = Upgrade::new("upgrade", "valset");
    upgrade
        .install(&bytes, root)
        .map_err(|e| format!("upgrade install: {e}"))?;

    let (bytes, root) = snapshot_of("tasks").await?;
    let mut tasks = Tasks::new("tasks");
    tasks
        .install(&bytes, root)
        .map_err(|e| format!("tasks install: {e}"))?;

    let (bytes, root) = snapshot_of("vaults").await?;
    let mut vaults = Vaults::new("vaults");
    vaults
        .install(&bytes, root)
        .map_err(|e| format!("vaults install: {e}"))?;

    let (bytes, root) = snapshot_of("profiles").await?;
    let mut profiles = Profiles::new("profiles");
    profiles
        .install(&bytes, root)
        .map_err(|e| format!("profiles install: {e}"))?;

    let (bytes, root) = snapshot_of("inbox").await?;
    let mut inbox = Inbox::new("inbox");
    inbox
        .install(&bytes, root)
        .map_err(|e| format!("inbox install: {e}"))?;

    let (bytes, root) = snapshot_of("files").await?;
    let mut files = Files::new("files");
    files
        .install(&bytes, root)
        .map_err(|e| format!("files install: {e}"))?;

    let (bytes, root) = snapshot_of("memory").await?;
    let mut memory = Memory::new("memory", "files");
    memory
        .install(&bytes, root)
        .map_err(|e| format!("memory install: {e}"))?;

    let (bytes, root) = snapshot_of("jobs").await?;
    let mut jobs = Jobs::new("jobs");
    jobs.install(&bytes, root)
        .map_err(|e| format!("jobs install: {e}"))?;

    let (bytes, root) = snapshot_of("agent").await?;
    let mut agent = AgentModule::new(
        "agent",
        "chat",
        "saga",
        Some("tasks".into()),
        Some("jobs".into()),
    );
    agent
        .install(&bytes, root)
        .map_err(|e| format!("agent install: {e}"))?;

    let (bytes, root) = snapshot_of("automations").await?;
    let mut automations = Automations::new("automations", "chat", "tasks", "inbox", "memory");
    automations
        .install(&bytes, root)
        .map_err(|e| format!("automations install: {e}"))?;

    let (bytes, root) = snapshot_of("forge").await?;
    let mut forge =
        Forge::init("forge", forge_repo.to_path_buf()).map_err(|e| format!("forge init: {e}"))?;
    // set the dual-path branch selector to the SERVED boundary version BEFORE
    // install: `Forge::install` (and `root()`) branch on `active_version`, so a
    // joiner installing a post-H snapshot must select the boundary's format or the
    // install/root would mismatch. `manifest.current_version` IS the effective
    // version at any settled boundary (the in-block `Advance` reconciles it before
    // the boundary is captured, so a post-H manifest carries `to_version` with no
    // pending). baseline/no-op before the forge v2 dual path lands (Phase 9).
    forge.set_active_version(manifest.current_version);
    forge
        .install(&bytes, root)
        .map_err(|e| format!("forge install: {e}"))?;

    // compose and check THE property: the rebuilt app-hash IS the manifest's.
    // keep this registry in sync with [`genesis_host`] — a missing module
    // composes a different app-hash and the join fails its final check.
    let host = Host::genesis(vec![
        Box::new(kv),
        Box::new(document),
        Box::new(pages),
        Box::new(chat),
        Box::new(forge),
        Box::new(valset),
        Box::new(governance),
        Box::new(upgrade),
        Box::new(saga),
        Box::new(capability),
        Box::new(tasks),
        Box::new(vaults),
        Box::new(profiles),
        Box::new(inbox),
        Box::new(files),
        Box::new(memory),
        Box::new(jobs),
        Box::new(agent),
        Box::new(automations),
        Box::new(directory),
    ])
    .map_err(|e| format!("compose synced host: {e}"))?;
    // realize the served boundary version into EVERY dual-path module's branch
    // selector so `root()` (and with it the app-hash check below) recomputes over
    // the boundary's format — the state-sync analogue of the activation hook the
    // live/recovery paths run. NON-hashed; idempotent for forge (set pre-install
    // above); baseline no-op before Phase 9.
    let mut host = host;
    host.set_active_version(manifest.current_version);
    if host.app_hash() != manifest.app_hash {
        return Err(format!(
            "composed {} != manifest {}",
            hex(&host.app_hash()),
            hex(&manifest.app_hash)
        ));
    }
    Ok(host)
}

fn assert_floor_binds_view(
    view_base: u64,
    boundary_height: u64,
    cert_view: u64,
) -> Result<(), String> {
    let certified_height = view_base
        .checked_add(cert_view)
        .ok_or_else(|| format!("floor view {cert_view} overflows view_base {view_base}"))?;
    if certified_height != boundary_height {
        return Err(format!(
            "floor certifies height {certified_height}, not boundary {boundary_height}"
        ));
    }
    Ok(())
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

fn diag_log(line: impl AsRef<str>) {
    let Ok(path) = std::env::var("DUCKTAPE_DIAG_LOG") else {
        return;
    };
    let line = line.as_ref();
    println!("{line}");
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        Ok(mut file) => {
            use std::io::Write as _;
            if let Err(e) = writeln!(file, "{line}") {
                eprintln!("DUCKTAPE_DIAG_LOG append failed for {path}: {e}");
            }
        }
        Err(e) => eprintln!("DUCKTAPE_DIAG_LOG open failed for {path}: {e}"),
    }
}

fn reopen_preflight_synced_host(host: &Host, expected: StateRoot) -> Result<(), String> {
    let live = host.app_hash();
    if live != expected {
        return Err(format!(
            "preflight app_hash {} != boundary {}",
            hex(&live),
            hex(&expected)
        ));
    }
    Ok(())
}

fn verify_manifest_floor(
    namespace: &[u8],
    signer: &ed25519::PrivateKey,
    boundary: &statesync::Manifest,
) -> Result<Option<Vec<u8>>, String> {
    if boundary.height <= boundary.view_base {
        return Ok(None);
    }
    let cert = boundary
        .floor_cert
        .clone()
        .ok_or_else(|| "boundary past its epoch base has no finalization floor".to_string())?;
    let mut keys = Vec::with_capacity(boundary.participants.len());
    for k in &boundary.participants {
        let pk = ed25519::PublicKey::decode(k.as_slice())
            .map_err(|e| format!("served participant set holds a non-ed25519 key: {e}"))?;
        keys.push(pk);
    }
    let participants =
        Set::try_from(keys).map_err(|_| "served participant set has duplicates".to_string())?;
    let scheme = match CONSENSUS_SCHEME {
        ConsensusScheme::V1Ed25519 => {
            simplex_ed25519::Scheme::signer(namespace, participants, signer.clone())
                .expect("our key is in the served participant set")
        }
        ConsensusScheme::V2Bls => {
            unimplemented!("V2Bls joiner wiring lands with valset bls key registration")
        }
    };
    let finalization = consensus::decode_finalization(&scheme, &cert).map_err(|e| {
        format!(
            "served finalization floor does not verify against the epoch's participant set: {e}"
        )
    })?;
    assert_floor_binds_view(
        boundary.view_base,
        boundary.height,
        finalization.proposal.round.view().get(),
    )
    .map_err(|e| format!("served finalization floor is stale: {e}"))?;
    Ok(Some(cert))
}

fn to_node_disposition(disposition: statesync::FrameDisposition) -> node::Disposition {
    match disposition {
        statesync::FrameDisposition::Applied => node::Disposition::Applied,
        statesync::FrameDisposition::Rejected => node::Disposition::Rejected,
    }
}

fn to_sync_disposition(
    disposition: node::Disposition,
) -> Result<statesync::FrameDisposition, String> {
    match disposition {
        node::Disposition::Applied => Ok(statesync::FrameDisposition::Applied),
        node::Disposition::Rejected => Ok(statesync::FrameDisposition::Rejected),
        node::Disposition::Discarded => Err("discarded frames are not recovery-journaled".into()),
    }
}

fn recovery_frame_to_sync(
    frame: recovery::JournalFrame,
) -> Result<statesync::FinalizedFrame, String> {
    Ok(statesync::FinalizedFrame {
        height: frame.height,
        frame: frame.frame,
        disposition: to_sync_disposition(frame.disposition)?,
        roots: frame.roots,
        app_hash: frame.app_hash,
    })
}

// ---------------------------------------------------------------------------
// the derived-index boot fold. consensus never depends on it: fold errors
// poison the store and log, heal errors log — recovery and the drain proceed
// identically with or without the index.
// ---------------------------------------------------------------------------

/// folds sealed blocks into the derived per-module index during boot (journal
/// replay + post-reboot frame catch-up), with the GAP DISCIPLINE: once one
/// sealed height's content is unreproducible (opaque) above some module's
/// watermark, folding stops for good. advancing watermarks past the hole
/// would hide it from the post-boot heal, which re-derives from verified
/// state exactly when a watermark trails the boot tip.
struct IndexFold<'a> {
    index: &'a indexer::IndexStore,
    stopped: bool,
}

impl<'a> IndexFold<'a> {
    fn new(index: &'a indexer::IndexStore) -> Self {
        Self {
            index,
            stopped: false,
        }
    }

    /// the LOWEST module watermark: an opaque height at or below it is
    /// already reflected everywhere; above it, at least one module would be
    /// folded past a hole.
    fn min_watermark(&self) -> Option<u64> {
        let mut min: Option<u64> = None;
        for id in self.index.module_ids() {
            match self.index.applied_height(id) {
                Ok(h) => min = Some(min.map_or(h, |m| m.min(h))),
                Err(_) => return None,
            }
        }
        min
    }
}

impl recovery::ReplaySink for IndexFold<'_> {
    fn folded_block(&mut self, height: u64, dispatches: &[host::DispatchRecord]) {
        if self.stopped {
            return;
        }
        // the validator's consensus time IS the height (see BlockContext).
        let ops = noded::index_block_ops(height, height, dispatches);
        if let Err(err) = self.index.apply_block(&ops) {
            eprintln!("[node] module index fold failed at height {height}: {err}");
            self.stopped = true;
        }
    }

    fn opaque_block(&mut self, height: u64) {
        if self.stopped {
            return;
        }
        match self.min_watermark() {
            Some(watermark) if height <= watermark => {}
            _ => self.stopped = true,
        }
    }
}

/// re-derive every index module whose watermark trails `boundary` from the
/// host's VERIFIED canonical state (checkpoint-restored, state-synced, or
/// replay-verified — every boot caller sits after a root/app-hash check).
/// failures poison the store and log; the node boots regardless.
async fn heal_index(index: &indexer::IndexStore, host: &Host, boundary: u64, label: &str) {
    let meta = indexer::RebuildMeta {
        height: boundary,
        // the validator's consensus time IS the height.
        time: boundary,
    };
    match noded::rebuild_stale_modules(index, host, meta).await {
        Ok(rebuilt) => {
            for (module, rows) in rebuilt {
                println!(
                    "[node {label}] index for {module} re-derived from state at height \
                     {boundary} ({rows} rows)"
                );
            }
        }
        Err(err) => eprintln!(
            "[node {label}] index heal at height {boundary} failed: {err} — wipe \
             <storage>/index to rebuild"
        ),
    }
}

/// cut and frame every derived-index database (modules + the blocks db) for
/// the shipped-index lane (indexable spec §7 lane 2). a database that fails
/// to cut is skipped — whatever a joiner does not receive, its staleness
/// heal re-derives — and a poisoned store cuts nothing, so the shipment
/// comes back empty and the joiner falls back entirely.
fn ship_index_blobs(
    index: &indexer::IndexStore,
    label: &str,
) -> std::collections::BTreeMap<String, Vec<u8>> {
    let mut blobs = std::collections::BTreeMap::new();
    let dbs: Vec<String> = index
        .module_ids()
        .map(str::to_string)
        .chain(std::iter::once(indexer::BLOCKS_DB_ID.to_string()))
        .collect();
    for db in dbs {
        match index.checkpoint_files(&db) {
            Ok(files) => {
                blobs.insert(db, statesync::encode_index_archive(&files));
            }
            Err(err) => eprintln!("[node {label}] shipped index skips {db}: {err}"),
        }
    }
    blobs
}

/// fetch the sync source's shipped-index checkpoints and stage them for
/// adoption at the promoted reboot — the OPTIONAL, UNVERIFIED warm start
/// over the from-state rebuild (indexable spec §7 lane 2). every outcome
/// short of a staged-and-committed install converges on the same fallback:
/// the boot heal re-derives whatever the watermarks say is missing, so
/// failures here log and fall through, never abort the promotion.
async fn stage_shipped_index<C: statesync::SyncClient>(
    client: &C,
    boundary: statesync::BoundaryId,
    storage: &std::path::Path,
    label: &str,
) {
    let index_base = storage.join("index");
    let known: std::collections::BTreeSet<&str> = MODULE_IDS
        .iter()
        .copied()
        .chain(std::iter::once(indexer::BLOCKS_DB_ID))
        .collect();
    let staged: Result<usize, String> = async {
        // a retry of the promotion loop may have staged a partial set
        // already; start clean so attempts never interleave.
        indexer::discard_staged(&index_base).map_err(|e| e.to_string())?;
        let entries = statesync::fetch_index_modules(client, boundary)
            .await
            .map_err(|e| e.to_string())?;
        let mut staged = 0usize;
        for (db, _) in &entries {
            // a db this binary does not know (version skew) would sit
            // unopened on disk forever — skip it, its module heals instead.
            if !known.contains(db.as_str()) {
                println!("[node {label}] shipped index skips unknown db {db:?}");
                continue;
            }
            let blob = statesync::fetch_index_db(client, boundary, db)
                .await
                .map_err(|e| format!("{db}: {e}"))?;
            let files =
                statesync::decode_index_archive(&blob).map_err(|e| format!("{db}: {e}"))?;
            indexer::stage_shipped_db(&index_base, db, &files).map_err(|e| e.to_string())?;
            staged += 1;
        }
        if staged > 0 {
            indexer::commit_staged(&index_base).map_err(|e| e.to_string())?;
        }
        Ok(staged)
    }
    .await;
    match staged {
        Ok(0) => println!(
            "[node {label}] source ships no index — views heal from verified state"
        ),
        Ok(n) => println!(
            "[node {label}] shipped index staged ({n} databases) — adopted at the promoted \
             reboot; contents are trusted from the source, not verified (spec §7 lane 2)"
        ),
        Err(e) => {
            eprintln!(
                "[node {label}] shipped index fetch failed: {e} — views heal from verified \
                 state instead"
            );
            if let Err(e) = indexer::discard_staged(&index_base) {
                eprintln!("[node {label}] shipped index staging cleanup failed: {e}");
            }
        }
    }
}

async fn apply_verified_suffix_frame(
    host: &mut Host,
    served: &statesync::FinalizedFrame,
) -> Result<Vec<host::DispatchRecord>, String> {
    let expected = to_node_disposition(served.disposition);
    let mut dispatches = Vec::new();
    let outcome = match node::decode_frame(&served.frame) {
        Ok((origin, msg)) => {
            let protocol_version = host.effective_version(served.height).await;
            host.set_active_version(protocol_version);
            let ctx = host::BlockContext {
                protocol_version,
                height: served.height,
                consensus_time: served.height,
                origin,
            };
            match host.submit_at(ctx, msg).await {
                Ok(outcome) => {
                    dispatches = outcome.dispatches;
                    node::Disposition::Applied
                }
                Err(host::SubmitError::Rejected(_)) => node::Disposition::Rejected,
                Err(host::SubmitError::Fatal(f)) => {
                    return Err(format!("fatal host error applying suffix frame: {f}"));
                }
            }
        }
        Err(_) => node::Disposition::Rejected,
    };
    if outcome != expected {
        return Err(format!(
            "served seal mismatch at height {}: replay landed as {outcome:?}, \
             served as {expected:?}",
            served.height
        ));
    }
    let roots = host.module_roots();
    if roots != served.roots {
        return Err(format!(
            "served seal mismatch at height {}: roots changed to {:?}, served {:?}",
            served.height, roots, served.roots
        ));
    }
    let app_hash = host.app_hash();
    if app_hash != served.app_hash {
        return Err(format!(
            "served seal mismatch at height {}: app_hash {} != served {}",
            served.height,
            hex(&app_hash),
            hex(&served.app_hash)
        ));
    }
    Ok(dispatches)
}

async fn apply_and_journal_verified_frame<E>(
    recovery: &mut Recovery<E>,
    host: &mut Host,
    frame: &statesync::FinalizedFrame,
    fold: Option<&mut IndexFold<'_>>,
) -> Result<(), String>
where
    E: recovery::Context + commonware_runtime::BufferPooler + commonware_runtime::Supervisor,
{
    node::BlockSink::pre_apply(recovery, frame.height, &frame.frame)
        .await
        .map_err(|e| format!("catch-up WAL write: {e}"))?;
    let dispatches = apply_verified_suffix_frame(host, frame).await?;
    let seal = node::BlockSeal {
        height: frame.height,
        disposition: to_node_disposition(frame.disposition),
        roots: host.module_roots(),
        app_hash: host.app_hash(),
    };
    node::BlockSink::seal(recovery, &seal)
        .await
        .map_err(|e| format!("catch-up seal write: {e}"))?;
    if let Some(fold) = fold {
        use recovery::ReplaySink as _;
        fold.folded_block(frame.height, &dispatches);
    }
    Ok(())
}

#[derive(Debug, Default)]
struct PostRebootCatchupApply {
    applied: usize,
    frames: Vec<Vec<u8>>,
    blocks: Vec<(u64, Vec<(ModuleId, StateRoot)>)>,
}

async fn apply_post_reboot_catchup_frames<E>(
    recovery: &mut Recovery<E>,
    host: &mut Host,
    from_height: u64,
    to_height: u64,
    frames: Vec<statesync::FinalizedFrame>,
    mut fold: Option<&mut IndexFold<'_>>,
) -> Result<PostRebootCatchupApply, String>
where
    E: recovery::Context + commonware_runtime::BufferPooler + commonware_runtime::Supervisor,
{
    if to_height < from_height {
        return Err(format!(
            "invalid catch-up range ({from_height}, {to_height}]"
        ));
    }
    if from_height == to_height {
        if !frames.is_empty() {
            return Err(format!(
                "no-gap catch-up received {} unexpected frames",
                frames.len()
            ));
        }
        return Ok(PostRebootCatchupApply::default());
    }
    if frames.last().map(|f| f.height) != Some(to_height) {
        return Err(format!(
            "catch-up frames stopped before target height {to_height}"
        ));
    }

    let mut last = from_height;
    let mut applied = PostRebootCatchupApply::default();
    for frame in frames {
        if frame.height <= last || frame.height > to_height {
            return Err(format!(
                "catch-up frame height {} outside ({last}, {to_height}]",
                frame.height
            ));
        }
        apply_and_journal_verified_frame(recovery, host, &frame, fold.as_deref_mut()).await?;
        last = frame.height;
        applied.applied += 1;
        applied.frames.push(frame.frame.clone());
        applied.blocks.push((frame.height, frame.roots.clone()));
    }
    Ok(applied)
}

fn catchup_pending_cutover_view(
    base_manifest: Option<&Manifest>,
    target: &statesync::Manifest,
    blocks: &[(u64, Vec<(ModuleId, StateRoot)>)],
) -> Result<Option<u64>, String> {
    let Some(base) = base_manifest else {
        return Ok(None);
    };
    if base.epoch == target.epoch && base.pending_cutover_view.is_some() {
        return Ok(base.pending_cutover_view);
    }
    let Some(mut prev_root) = base.root("valset") else {
        return Ok(None);
    };
    for (height, roots) in blocks {
        let root = roots
            .iter()
            .find(|(id, _)| id == "valset")
            .map(|(_, root)| *root)
            .ok_or_else(|| format!("catch-up seal at height {height} has no valset root"))?;
        if root != prev_root && *height > target.view_base {
            return Ok(Some(*height - target.view_base + CUTOVER_DELAY));
        }
        prev_root = root;
    }
    Ok(None)
}

async fn write_post_reboot_catchup_checkpoint<E>(
    recovery: &mut Recovery<E>,
    host: &Host,
    base_manifest: Option<&Manifest>,
    target: &statesync::Manifest,
    blocks: &[(u64, Vec<(ModuleId, StateRoot)>)],
    next_seq: u64,
) -> Result<Manifest, String>
where
    E: recovery::Context + commonware_runtime::BufferPooler + commonware_runtime::Supervisor,
{
    if host.app_hash() != target.app_hash {
        return Err(format!(
            "catch-up checkpoint host hash {} does not match target {}",
            hex(&host.app_hash()),
            hex(&target.app_hash)
        ));
    }
    let pending_cutover_view = catchup_pending_cutover_view(base_manifest, target, blocks)?;
    let pos = recovery.oplog_pos().await;
    let ckpt = Manifest::capture(
        host,
        Some(target.height),
        target.epoch,
        target.view_base,
        target.participants.clone(),
        pending_cutover_view,
        target.current_version,
        target.pending_upgrade.clone(),
        pos,
        next_seq,
    )
    .map_err(|e| format!("catch-up checkpoint capture: {e}"))?;
    recovery
        .write_manifest(&ckpt)
        .await
        .map_err(|e| format!("catch-up checkpoint write: {e}"))?;
    diag_log(format!("DIAG catchup_checkpoint height={}", target.height));
    Ok(ckpt)
}

#[derive(Debug)]
struct PostRebootCatchup {
    from_height: u64,
    to_height: u64,
    frames: usize,
    target: Option<statesync::Manifest>,
    frame_bytes: Vec<Vec<u8>>,
    blocks: Vec<(u64, Vec<(ModuleId, StateRoot)>)>,
}

#[derive(Debug)]
enum PostRebootCatchupError {
    Retry(String),
    RangePruned {
        target: statesync::Manifest,
        requested_after: u64,
        retained_from: u64,
    },
    Fatal(String),
}

async fn catch_up_post_reboot_frames<C, E>(
    client: &C,
    recovery: &mut Recovery<E>,
    host: &mut Host,
    fold: Option<&mut IndexFold<'_>>,
    recovered_height: u64,
    max_iterations: usize,
) -> Result<PostRebootCatchup, PostRebootCatchupError>
where
    C: statesync::SyncClient,
    E: recovery::Context + commonware_runtime::BufferPooler + commonware_runtime::Supervisor,
{
    let mut fold = fold;
    let mut current_height = recovered_height;
    let mut total_frames = 0usize;
    let mut target = None;
    let mut frame_bytes = Vec::new();
    let mut blocks = Vec::new();

    for _ in 0..=max_iterations {
        let tip = fetch_manifest(client).await.map_err(|e| {
            PostRebootCatchupError::Retry(format!("catch-up manifest unavailable: {e}"))
        })?;
        if tip.height <= current_height {
            if tip.height == current_height && host.app_hash() != tip.app_hash {
                return Err(PostRebootCatchupError::Fatal(format!(
                    "catch-up source hash {} at height {} does not match recovered host {}",
                    hex(&tip.app_hash),
                    tip.height,
                    hex(&host.app_hash())
                )));
            }
            diag_log(format!(
                "DIAG post_reboot_catchup from={} to={} frames={}",
                recovered_height, current_height, total_frames
            ));
            return Ok(PostRebootCatchup {
                from_height: recovered_height,
                to_height: current_height,
                frames: total_frames,
                target: target.or_else(|| {
                    (tip.height == current_height && host.app_hash() == tip.app_hash).then_some(tip)
                }),
                frame_bytes,
                blocks,
            });
        }

        let frames = match fetch_frames(client, current_height, tip.height).await {
            Ok(frames) => frames,
            Err(statesync::SyncError::RangePruned {
                requested_after,
                retained_from,
            }) => {
                return Err(PostRebootCatchupError::RangePruned {
                    target: tip,
                    requested_after,
                    retained_from,
                });
            }
            Err(e) => {
                return Err(PostRebootCatchupError::Retry(format!(
                    "catch-up frame suffix unavailable: {e}"
                )));
            }
        };
        let applied = apply_post_reboot_catchup_frames(
            recovery,
            host,
            current_height,
            tip.height,
            frames,
            fold.as_deref_mut(),
        )
        .await
        .map_err(PostRebootCatchupError::Fatal)?;
        if host.app_hash() != tip.app_hash {
            return Err(PostRebootCatchupError::Fatal(format!(
                "catch-up frames landed at {}, target manifest {}",
                hex(&host.app_hash()),
                hex(&tip.app_hash)
            )));
        }
        current_height = tip.height;
        total_frames += applied.applied;
        frame_bytes.extend(applied.frames);
        blocks.extend(applied.blocks);
        target = Some(tip);
    }

    diag_log(format!(
        "DIAG post_reboot_catchup from={} to={} frames={}",
        recovered_height, current_height, total_frames
    ));
    Ok(PostRebootCatchup {
        from_height: recovered_height,
        to_height: current_height,
        frames: total_frames,
        target,
        frame_bytes,
        blocks,
    })
}

struct BootP2pSyncClient<S, R>
where
    S: P2pSender<PublicKey = ed25519::PublicKey>,
    R: P2pReceiver<PublicKey = ed25519::PublicKey>,
{
    sender: S,
    server: ed25519::PublicKey,
    receiver: std::sync::Arc<tokio::sync::Mutex<Option<R>>>,
    next_id: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

impl<S, R> Clone for BootP2pSyncClient<S, R>
where
    S: P2pSender<PublicKey = ed25519::PublicKey>,
    R: P2pReceiver<PublicKey = ed25519::PublicKey>,
{
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            server: self.server.clone(),
            receiver: std::sync::Arc::clone(&self.receiver),
            next_id: std::sync::Arc::clone(&self.next_id),
        }
    }
}

impl<S, R> BootP2pSyncClient<S, R>
where
    S: P2pSender<PublicKey = ed25519::PublicKey>,
    R: P2pReceiver<PublicKey = ed25519::PublicKey>,
{
    fn new(sender: S, receiver: R, server: ed25519::PublicKey) -> Self {
        Self {
            sender,
            server,
            receiver: std::sync::Arc::new(tokio::sync::Mutex::new(Some(receiver))),
            next_id: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    fn into_parts(self) -> Result<(S, R), String> {
        let Self {
            sender, receiver, ..
        } = self;
        let receiver = std::sync::Arc::try_unwrap(receiver)
            .map_err(|_| "boot statesync client still has live clones".to_string())?
            .into_inner()
            .ok_or_else(|| "boot statesync receiver already taken".to_string())?;
        Ok((sender, receiver))
    }
}

impl<S, R> statesync::SyncClient for BootP2pSyncClient<S, R>
where
    S: P2pSender<PublicKey = ed25519::PublicKey> + Send + Sync + 'static,
    R: P2pReceiver<PublicKey = ed25519::PublicKey> + Send + 'static,
{
    fn request(
        &self,
        req: statesync::SyncRequest,
    ) -> impl std::future::Future<Output = Result<statesync::SyncResponse, statesync::SyncError>> + Send
    {
        let mut sender = self.sender.clone();
        let server = self.server.clone();
        let receiver = std::sync::Arc::clone(&self.receiver);
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        async move {
            let mut guard = receiver.lock().await;
            let receiver = guard.as_mut().ok_or_else(|| {
                statesync::SyncError::Transport("boot statesync receiver closed".into())
            })?;
            let frame = statesync::encode_rpc(id, &statesync::encode_request(&req));
            let attempted = sender.send(Recipients::One(server.clone()), IoBuf::from(frame), false);
            if attempted.is_empty() {
                return Err(statesync::SyncError::Transport(
                    "server peer unreachable (send attempted no recipients)".into(),
                ));
            }
            loop {
                let delivered =
                    tokio::time::timeout(BOOT_SYNC_REQUEST_TIMEOUT, receiver.recv()).await;
                let (peer, msg) = match delivered {
                    Ok(Ok(item)) => item,
                    Ok(Err(_)) => {
                        return Err(statesync::SyncError::Transport(
                            "boot statesync channel closed".into(),
                        ));
                    }
                    Err(_) => {
                        return Err(statesync::SyncError::Transport(format!(
                            "boot statesync request {id} timed out"
                        )));
                    }
                };
                if peer != server {
                    continue;
                }
                let bytes: Vec<u8> = msg.into();
                let Ok((rpc_id, body)) = statesync::decode_rpc(&bytes) else {
                    continue;
                };
                if rpc_id != id {
                    continue;
                }
                return Ok(statesync::decode_response(body)?);
            }
        }
    }
}

fn resume_member_keys(
    resumed: Option<&recovery::Recovered>,
    validators: &[ed25519::PublicKey],
) -> Result<Vec<ed25519::PublicKey>, String> {
    let raw: Vec<Vec<u8>> = match resumed {
        Some(rec) => rec.participants.clone(),
        None => validators.iter().map(|k| k.as_ref().to_vec()).collect(),
    };
    let mut keys = Vec::with_capacity(raw.len());
    for k in &raw {
        keys.push(
            ed25519::PublicKey::decode(k.as_slice())
                .map_err(|e| format!("recovered participant set holds a non-ed25519 key: {e}"))?,
        );
    }
    Ok(keys)
}

fn advance_next_seq_from_frames(next_seq: &mut u64, frames: &[Vec<u8>], me: &[u8]) {
    for frame in frames {
        if let Some((origin, seq)) = node::frame_origin_seq(frame) {
            if origin == me {
                *next_seq = (*next_seq).max(seq + 1);
            }
        }
    }
}

fn derive_pending_boot(manifest: &Manifest, rec: &recovery::Recovered) -> Option<u64> {
    let checkpoint_pending = if rec.epoch == manifest.epoch {
        manifest.pending_cutover_view
    } else {
        None
    };
    checkpoint_pending.or_else(|| {
        let mut prev_root = manifest.root("valset").expect("valset is a genesis module");
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
    })
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

fn unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("keygen") => return cmd_keygen(&args[1..]),
        Some("init") => return cmd_init(&args[1..]),
        Some("invite") => return cmd_invite(&args[1..]),
        Some("admit") => return cmd_admit(&args[1..]),
        Some("invite-accept") => return cmd_invite_accept(&args[1..]),
        Some("join-requests") => return cmd_join_requests(&args[1..]),
        Some("member-remove") => return cmd_member_remove(&args[1..]),
        Some("member-leave") => return cmd_member_leave(&args[1..]),
        Some("member-status") => return cmd_member_status(&args[1..]),
        Some("join") => return cmd_join(&args[1..]),
        Some("upgrade-status") => return cmd_upgrade_status(&args[1..]),
        _ => {}
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
                     keygen|init|invite|admit|invite-accept|join-requests|member-remove|\
                     member-leave|member-status|join — or \
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

/// tiny flag parser: `--name value` pairs plus positionals; no deps. `-n` is
/// the one short alias (for `--network`, the workspace selector every verb
/// takes).
fn parse_flags(
    args: &[String],
) -> Result<(Vec<String>, std::collections::BTreeMap<String, String>), String> {
    let mut positional = Vec::new();
    let mut flags = std::collections::BTreeMap::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let name = match a.as_str() {
            "-n" => Some("network"),
            other => other.strip_prefix("--"),
        };
        if let Some(name) = name {
            let v = it.next().ok_or_else(|| format!("--{name} needs a value"))?;
            flags.insert(name.to_string(), v.clone());
        } else {
            positional.push(a.clone());
        }
    }
    Ok((positional, flags))
}

/// the config a verb operates on: `-n`/`--network <chain id>` resolves through
/// the workspace registry (`~/.ducktape/workspaces`), `--config <path>` is the
/// explicit escape hatch, and the default is ./node.toml — the pre-registry
/// behavior, unchanged.
fn config_path(flags: &std::collections::BTreeMap<String, String>) -> Result<PathBuf, String> {
    if let Some(needle) = flags.get("network") {
        return config::find_workspace_config(needle);
    }
    Ok(PathBuf::from(
        flags
            .get("config")
            .map(String::as_str)
            .unwrap_or("node.toml"),
    ))
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
        reach: Vec::new(),
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

/// `invite [--config node.toml] [--manual]` — emit the one-line paste blob:
/// the network descriptor with THIS member's dial hint folded in (and
/// persisted, so every future invite carries it), plus an INVITE TOKEN. the
/// token lets the joiner's parked node deliver its pubkey over the lobby
/// channel automatically — the join request then awaits member approval
/// (`invite-accept`, or the app's approve button); a token never admits by
/// itself. `--manual` omits the token: the joiner's pubkey travels out-of-band
/// exactly as before. any current member may invite (the blob is a low-trust
/// doorbell, gated by the descriptor's genesis fingerprint and the admission
/// ballot — not a signed genesis-only credential).
fn cmd_invite(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    // `--manual` is a bare boolean; strip it before the `--flag value` parser.
    let mut manual = false;
    let args: Vec<String> = args
        .iter()
        .filter(|a| {
            let is_manual = a.as_str() == "--manual";
            manual |= is_manual;
            !is_manual
        })
        .cloned()
        .collect();
    let (pos, flags) = parse_flags(&args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let cfg_path = config_path(&flags)?;
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
        // an invite must carry SOME dialable member. a member that joined via a
        // v3 invite holds its dial hints as `reach` (bootstrap is empty), so
        // check the union, not just bootstrap — else a reachable NAT'd member
        // is wrongly refused.
        None if descriptor.reach_hints().map(|h| h.is_empty()).unwrap_or(true) => {
            return Err(
                "no dialable address: give node.toml a concrete `listen` port or an \
                        `advertised` addr so a joiner can reach the network"
                    .into(),
            );
        }
        None => {}
    }
    descriptor.save(&descriptor_path)?;
    let token = (!manual)
        .then(|| config::mint_invite_token(&key, descriptor.genesis_namespace().as_bytes()));
    println!("{}", config::encode_invite(&descriptor, token.as_ref())?);
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
    let cfg_path = config_path(&flags)?;
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
    writer
        .write_all(line.as_bytes())
        .map_err(|e| format!("rpc write: {e}"))?;
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
    unhex(
        reply["reply_hex"]
            .as_str()
            .ok_or("query reply carries no payload")?,
    )
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
    use valset_interface::{ValsetQuery, ValsetReply, decode_reply, encode_query};
    let raw = rpc_query(addr, "valset", &encode_query(&ValsetQuery::Validators))?;
    match decode_reply(&raw)? {
        ValsetReply::Validators(v) => Ok(v),
        other => Err(format!("unexpected valset reply shape: {other:?}")),
    }
}

/// `join-requests [--config node.toml]` — the verified join announces parked
/// joiners delivered to THIS member's running node, as one JSON array on
/// stdout (machine-parseable — the app's members view renders it). approving
/// is a separate, deliberate act: `invite-accept <joiner>` (or the app's
/// approve button) casts this member's governance ballot, and a strict
/// majority admits.
fn cmd_join_requests(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let cfg_path = config_path(&flags)?;
    let resolved = config::resolve(&cfg_path)?;
    let addr = resolved
        .rpc_listen
        .ok_or("join-requests reads the node's local rpc — set `rpc_listen` in node.toml")?;
    let reply = rpc_call(&addr, &serde_json::json!({ "cmd": "join_requests" }))?;
    if reply["ok"] != true {
        return Err(format!("join-requests: {}", reply["error"]).into());
    }
    println!(
        "{}",
        reply
            .get("join_requests")
            .cloned()
            .unwrap_or_else(|| serde_json::json!([]))
    );
    Ok(())
}

/// `upgrade-status [--config node.toml]` — query the upgrade module Status over
/// this node's local rpc and print `current_version`, the single pending upgrade,
/// the readiness verdict (`ready_count` of `member_count`, `armed`), and the
/// `max_supported` version this binary can execute. degrades gracefully on a net
/// WITHOUT the module (pre-retrofit): the query errors and we report baseline.
fn cmd_upgrade_status(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use upgrade_interface::{UpgradeQuery, UpgradeReply, decode_reply, encode_query};
    let (_, flags) = parse_flags(args)?;
    let cfg_path = config_path(&flags)?;
    let resolved = config::resolve(&cfg_path)?;
    let addr = resolved
        .rpc_listen
        .ok_or("upgrade-status drives the node's local rpc — set `rpc_listen` in node.toml")?;

    let raw = match rpc_query(&addr, "upgrade", &encode_query(&UpgradeQuery::Status)) {
        Ok(bytes) => bytes,
        // module absent (pre-retrofit) or unreachable: report the binary baseline
        // rather than failing — the CLI is inert on a net without the module.
        Err(e) => {
            println!(
                "upgrade module not available ({e}) — this binary supports up to protocol v{MAX_PROTOCOL_VERSION}"
            );
            return Ok(());
        }
    };
    let UpgradeReply::Status(status) = decode_reply(&raw)?;
    println!("current_version: {}", status.current_version);
    println!("max_supported (this binary): {MAX_PROTOCOL_VERSION}");
    match &status.pending {
        Some(up) => {
            println!(
                "pending: name={} activation_height={} to_version={}",
                up.name, up.activation_height, up.to_version
            );
            println!(
                "readiness: {} of {} boundary members ready",
                status.ready_count, status.member_count
            );
            println!("armed (R == n): {}", status.armed);
            if up.to_version > MAX_PROTOCOL_VERSION {
                println!(
                    "WARNING: this binary (v{MAX_PROTOCOL_VERSION}) cannot execute to_version {} \
                     — install the newer node binary before H or this node aborts the upgrade",
                    up.to_version
                );
            }
        }
        None => println!("pending: none"),
    }
    Ok(())
}

fn read_proposal(
    addr: &str,
    id: &str,
) -> Result<Option<governance_interface::ProposalView>, String> {
    use governance_interface::{GovQuery, GovReply, decode_reply, encode_query};
    let raw = rpc_query(
        addr,
        "governance",
        &encode_query(&GovQuery::Proposal {
            proposal_id: id.into(),
        }),
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
    use governance_interface::{GovAction, GovMsg, ProposalStatus, encode_msg};

    let (pos, flags) = parse_flags(args)?;
    let [pubkey_hex] = pos.as_slice() else {
        return Err("invite-accept needs exactly one <hex pubkey>".into());
    };
    let key = config::decode_key(pubkey_hex)?;
    let key_bytes = key.as_ref().to_vec();
    let cfg_path = config_path(&flags)?;
    // full config resolution (network- or dev-shape) so the verb derives the
    // SAME identity the running node signs with — the ballots this verb
    // casts are signed by the NODE (the ordered lane signs every rpc
    // submit), and that key must be the member.
    let resolved = config::resolve(&cfg_path)?;
    let rpc_addr = resolved
        .rpc_listen
        .clone()
        .ok_or("invite-accept drives the node's local rpc — set `rpc_listen` in node.toml")?;
    let me_bytes = resolved.signer.public_key().as_ref().to_vec();

    let members = read_members(&rpc_addr)?;
    if members.contains(&key_bytes) {
        eprintln!("{pubkey_hex} is already a validator — nothing to do");
        return Ok(());
    }
    if !members.contains(&me_bytes) {
        return Err(
            "this node's identity is not a current member — only members admit \
                    validators"
                .into(),
        );
    }

    // adopt an existing OPEN proposal for exactly this action, else mint an
    // unused id (settled proposals keep their ids forever — a re-admitted
    // key gets a fresh suffix).
    use governance_interface::{GovQuery, GovReply, decode_reply, encode_query};
    let proposals = match decode_reply(&rpc_query(
        &rpc_addr,
        "governance",
        &encode_query(&GovQuery::Proposals),
    )?)? {
        GovReply::Proposals(views) => views,
        other => return Err(format!("unexpected governance reply: {other:?}").into()),
    };
    let wanted = GovAction::AddValidator {
        key: key_bytes.clone(),
    };
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
        &encode_msg(&GovMsg::Vote {
            proposal_id: proposal_id.clone(),
            approve: true,
        }),
    )?;
    let after_vote = poll_proposal(&rpc_addr, &proposal_id, "this ballot to finalize", |p| {
        p.as_ref().is_some_and(|v| {
            v.status != ProposalStatus::Open
                || v.votes
                    .iter()
                    .any(|(voter, yes)| voter == &me_bytes && *yes)
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
            &encode_msg(&GovMsg::Execute {
                proposal_id: proposal_id.clone(),
            }),
        )?;
    }
    let settled = poll_proposal(&rpc_addr, &proposal_id, "the tally to settle", |p| {
        p.as_ref().is_some_and(|v| v.status != ProposalStatus::Open)
    })?
    .expect("the poll only accepts a present proposal");
    match settled.status {
        ProposalStatus::Passed => {
            eprintln!(
                "admitted {pubkey_hex} as STANDBY: the joiner's parked node will verify a \
                 state sync, announce itself online, and join the consensus quorum at the \
                 activation cutover — no quorum slot is spent until the node is actually up"
            );
            Ok(())
        }
        status => Err(format!("proposal {proposal_id} settled as {status:?}").into()),
    }
}

// ---- member-remove: post-genesis removal over the local rpc ----------------

/// `member-remove <hex pubkey> [--config node.toml]` — post-genesis removal:
/// drive a governance RemoveValidator proposal for `pubkey` through this
/// member's own RUNNING node. the mirror of `invite-accept` with inverted
/// guards — a no-op when the key is NOT a member, and only members may drive
/// it. idempotent across members: each runs the same command (propose if
/// absent, cast a yes ballot, execute once decidable); the run that lands the
/// deciding ballot executes. the passing proposal's valset Leave schedules the
/// epoch cutover that drops the key from the tracked set.
fn cmd_member_remove(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use governance_interface::{GovAction, GovMsg, ProposalStatus, encode_msg};

    let (pos, flags) = parse_flags(args)?;
    let [pubkey_hex] = pos.as_slice() else {
        return Err("member-remove needs exactly one <hex pubkey>".into());
    };
    let key = config::decode_key(pubkey_hex)?;
    let key_bytes = key.as_ref().to_vec();
    let cfg_path = config_path(&flags)?;
    // full config resolution so the verb derives the SAME identity the running
    // node signs with — the ballots this verb casts are signed by the NODE, and
    // that key must be a current member.
    let resolved = config::resolve(&cfg_path)?;
    let rpc_addr = resolved
        .rpc_listen
        .clone()
        .ok_or("member-remove drives the node's local rpc — set `rpc_listen` in node.toml")?;
    let me_bytes = resolved.signer.public_key().as_ref().to_vec();

    let members = read_members(&rpc_addr)?;
    // inverted admission guards: nothing to remove if the key is not a member,
    // and only current members may open/decide a removal.
    if !members.contains(&key_bytes) {
        eprintln!("{pubkey_hex} is not a validator — nothing to do");
        return Ok(());
    }
    if !members.contains(&me_bytes) {
        return Err(
            "this node's identity is not a current member — only members remove \
                    validators"
                .into(),
        );
    }

    // adopt an existing OPEN proposal for exactly this action, else mint an
    // unused id (settled proposals keep their ids forever — a re-removed key
    // gets a fresh suffix).
    use governance_interface::{GovQuery, GovReply, decode_reply, encode_query};
    let proposals = match decode_reply(&rpc_query(
        &rpc_addr,
        "governance",
        &encode_query(&GovQuery::Proposals),
    )?)? {
        GovReply::Proposals(views) => views,
        other => return Err(format!("unexpected governance reply: {other:?}").into()),
    };
    let wanted = GovAction::RemoveValidator {
        key: key_bytes.clone(),
    };
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
                .map(|n| format!("remove:{prefix}:{n}"))
                .find(|id| !proposals.iter().any(|p| &p.proposal_id == id))
                .expect("the id space is unbounded");
            rpc_submit(
                &rpc_addr,
                "governance",
                &encode_msg(&GovMsg::Propose {
                    proposal_id: id.clone(),
                    action: wanted,
                    // a far horizon in consensus-time units: removal must not
                    // expire under a slow second ballot.
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
        &encode_msg(&GovMsg::Vote {
            proposal_id: proposal_id.clone(),
            approve: true,
        }),
    )?;
    let after_vote = poll_proposal(&rpc_addr, &proposal_id, "this ballot to finalize", |p| {
        p.as_ref().is_some_and(|v| {
            v.status != ProposalStatus::Open
                || v.votes
                    .iter()
                    .any(|(voter, yes)| voter == &me_bytes && *yes)
        })
    })?
    .expect("the poll only accepts a present proposal");
    eprintln!("ballot cast as {}", hex_bytes(&me_bytes));

    // execute only when decidable — a strict-majority shortfall is the normal
    // n>=2 intermediate state, not an error.
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
             ducktape-node member-remove {pubkey_hex} --config <their node.toml>"
        );
        return Ok(());
    }
    if after_vote.status == ProposalStatus::Open {
        rpc_submit(
            &rpc_addr,
            "governance",
            &encode_msg(&GovMsg::Execute {
                proposal_id: proposal_id.clone(),
            }),
        )?;
    }
    let settled = poll_proposal(&rpc_addr, &proposal_id, "the tally to settle", |p| {
        p.as_ref().is_some_and(|v| v.status != ProposalStatus::Open)
    })?
    .expect("the poll only accepts a present proposal");
    match settled.status {
        ProposalStatus::Passed => {
            eprintln!(
                "removed {pubkey_hex}: the validator set changes at the next epoch cutover"
            );
            Ok(())
        }
        status => Err(format!("proposal {proposal_id} settled as {status:?}").into()),
    }
}

// ---- member-leave: this node drives its OWN removal from the set -----------

/// `member-leave [--config node.toml]` — a member drives its OWN removal:
/// resolve this node's identity and route it through the EXACT SAME governance
/// path as `member-remove` (a RemoveValidator proposal targeting self). there
/// is no separate governance logic — it hands off to [`cmd_member_remove`] with
/// this node's own pubkey.
///
/// honesty: leaving is NOT unilateral in a set of n>=2. this casts only this
/// node's own yes-ballot, so the removal stays PENDING until a strict majority
/// (n/2+1) of the members approve — member-remove's own output prints the tally
/// and names the command the remaining members run (`member-remove <this key>`).
/// a lone member (n==1) meets its own majority-of-one and executes at once.
fn cmd_member_leave(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("member-leave takes no positional args (got {pos:?})").into());
    }
    let cfg_path = config_path(&flags)?;
    // resolve the running node's identity — the key it signs ballots with, and
    // the one this verb submits for removal.
    let resolved = config::resolve(&cfg_path)?;
    let me_hex = hex_bytes(resolved.signer.public_key().as_ref());
    eprintln!("leaving the network: opening a self-removal for {me_hex}");
    // delegate to member-remove targeting SELF — same propose+vote+execute
    // path, same strict-majority honesty. rebuild the arg vector so the flags
    // (notably --config) reach the delegate unchanged.
    let mut forwarded = vec![me_hex];
    for (name, value) in &flags {
        forwarded.push(format!("--{name}"));
        forwarded.push(value.clone());
    }
    cmd_member_remove(&forwarded)
}

// ---- member-status: is THIS node still in the validator set? ----------------

/// `member-status [--config node.toml]` — read this node's OWN membership off
/// its RUNNING node's rpc and print one machine-parseable line to stdout:
///
/// ```text
/// in-set=<true|false> validators=<count>
/// ```
///
/// this is the read the desktop shell consults before FORGETTING a workspace
/// (stop + delete): tearing a node down while it is still a current validator of
/// a set of two-or-more strands its pending removal and halts quorum (a live
/// network still needs its signature). the shell refuses a forget when
/// `in-set=true` and `validators>=2`; a lone validator (`validators=1`) or an
/// already-removed key (`in-set=false`) is safe to forget. requires the node to
/// be up (it serves this over the same local rpc as `member-remove`).
fn cmd_member_status(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("member-status takes no positional args (got {pos:?})").into());
    }
    let cfg_path = config_path(&flags)?;
    let resolved = config::resolve(&cfg_path)?;
    let rpc_addr = resolved
        .rpc_listen
        .clone()
        .ok_or("member-status reads the node's local rpc — set `rpc_listen` in node.toml")?;
    let me_bytes = resolved.signer.public_key().as_ref().to_vec();
    let members = read_members(&rpc_addr)?;
    let in_set = members.contains(&me_bytes);
    println!("in-set={in_set} validators={}", members.len());
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
    let (descriptor, token) = config::decode_invite(blob)?;
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
    if let Some(token) = &token {
        // the bearer credential the parked node announces with; a re-join
        // with a fresh invite replaces a stale one.
        config::save_invite_token(&dir, token)?;
    }
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
    } else if token.is_some() {
        eprintln!(
            "NOT yet a member. start now — `ducktape-node --config {}/node.toml` parks on \
             the mesh and DELIVERS this identity to the members automatically (the invite \
             carries a token); a member then approves the join request (the app's approve \
             button, or `ducktape-node invite-accept {me_hex}`), and this node promotes \
             itself.",
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

/// the reachability plane's thread body: derive the plane's endpoints, bind
/// the nat client against the coordinated-reach coordinators, and drive
/// `reachability::run` with the configured WireGuard effect — real (an
/// actual interface via the userspace WireGuard runtime) by default,
/// in-memory fake when `wireguard_effect = "fake"` opts out. every failure
/// path prints and returns — the plane is an overlay on a working node,
/// never a reason to take the node down.
#[allow(clippy::too_many_arguments)]
async fn reachability_plane(
    label: String,
    chain_id: String,
    signer: ed25519::PrivateKey,
    wireguard_key_file: PathBuf,
    wireguard_listen: std::net::SocketAddr,
    effect_kind: WireGuardEffectKind,
    advertised: Ingress,
    coordinators: Vec<Ingress>,
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
    let wireguard_endpoint = match wireguard_upgrade::Endpoint::new(
        wireguard_listen.ip(),
        wireguard_listen.port(),
        wireguard_upgrade::Transport::Udp,
        &policy,
    ) {
        Ok(endpoint) => endpoint,
        Err(err) => {
            eprintln!(
                "[node {label}] reachability: wireguard_listen rejected ({err:?}) — plane not \
                 started"
            );
            return;
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
    let resolver = match reachability::NatResolver::bind(me, coords.clone()).await {
        Ok(resolver) => resolver,
        Err(err) => {
            eprintln!(
                "[node {label}] reachability: nat client bind failed: {err} — plane not started"
            );
            return;
        }
    };
    if let Some(reflexive) = resolver.reflexive() {
        println!("[node {label}] reachability: coordinator-observed reflexive {reflexive}");
    }
    let config = reachability::ReachabilityConfig {
        chain_id,
        signer,
        wireguard_key_file,
        wireguard_listen: wireguard_endpoint,
        control_endpoint,
        coordinators: coords,
        port_policy: policy,
    };
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
        WireGuardEffectKind::Real => {
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
fn run_node(resolved: Resolved, sync_only: bool) -> Result<(), Box<dyn std::error::Error>> {
    let Resolved {
        signer,
        label,
        namespace,
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
        dev_demo,
        checkpoint_blocks,
        invite_token,
        sync_index,
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
        if invite_token.is_some() {
            println!(
                "[node {label}] identity {} is not in the genesis validator set — joiner \
                 mode: parking on the mesh and announcing this key with the invite token; \
                 a member approves the join request (the app, or `ducktape-node \
                 invite-accept {}`)",
                hex_bytes(signer.public_key().as_ref()),
                hex_bytes(signer.public_key().as_ref())
            );
        } else {
            println!(
                "[node {label}] identity {} is not in the genesis validator set — joiner \
                 mode: parking on the mesh until a member runs `ducktape-node \
                 invite-accept {}`",
                hex_bytes(signer.public_key().as_ref()),
                hex_bytes(signer.public_key().as_ref())
            );
        }
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
    // coordinated reach targets are split OUT of the TCP mesh dialer (a
    // coordinator's UDP rendezvous port is not a TCP mesh peer — dialing it
    // there was a silent no-op). reaching them is the reachability plane's
    // job: gossip relays through whatever mesh links exist, the nat client
    // hole-punches/relays the WireGuard path through the coordinator, and
    // once tunnels apply the mesh dials the target's advertised overlay
    // address over the tunnel (the target sets `advertised = "overlay"`).
    // what still needs a TCP foothold is the gossip itself: with ZERO
    // bootstrap links nothing carries this node's records anywhere, so a
    // coordinated-ONLY config parks until the persisted-mesh/coordinator-
    // carried-gossip seam ships — surface that loudly rather than park
    // silently.
    if !coordinated.is_empty() {
        if bootstrappers.is_empty() {
            println!(
                "[node {label}] WARNING: {} coordinated reach target(s) but NO direct/fronted \
                 bootstrap link — tunnel bring-up gossip has no path to ride, so these peers \
                 stay unreachable. add at least one direct/fronted hint (an ephemeral ingress \
                 is enough) for the join window.",
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
        match wireguard_effect {
            WireGuardEffectKind::Real => println!(
                "[node {label}] reachability plane: advertising WireGuard endpoint udp/{wg}"
            ),
            WireGuardEffectKind::Fake => println!(
                "[node {label}] reachability plane: advertising WireGuard endpoint udp/{wg}; \
                 records, advertisements, and tunnel handshakes run for real, the interface \
                 effect is the in-memory fake (no real tunnel)."
            ),
        }
    }

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
    // the derived per-module index (noded's exact store, <storage>/index),
    // plus the blocks database the explorer reads: the pump folds sealed
    // blocks into it, boot heals it from verified state at sync/recovery
    // boundaries, and the already-routed GET /v1/blocks + /v1/index/* lanes
    // light up through the handle below. an open failure is fatal-with-remedy
    // rather than a silent no-index run: the tier is rebuildable, so the fix
    // is always "delete <storage>/index".
    let index = noded::open_index_store(&storage, &MODULE_IDS)?;
    // point the http handle at this node's forge repo base (the same
    // `storage/forge-repo` the host materializes into) so the git upload-pack
    // (clone/fetch) route can open a repo READ-ONLY and serve its objects.
    let http_handle = http_handle
        .with_forge_repo(storage.join("forge-repo"))
        .with_index_store(index.clone());
    let blobs = http_handle.blob_handle();
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
        // reachability plane (docs/sentry-deployment.md): a forward sentry on a
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
            // the lobby lane: a sync-only observer never announces or answers,
            // but an unregistered channel is a protocol violation — black-hole.
            {
                let (_tx, mut rx) = network.register(CHANNEL_LOBBY, quota, MAX_BACKLOG);
                context.child("blackhole_lobby").spawn(move |_ctx| async move {
                    while rx.recv().await.is_ok() {}
                });
            }
            // the reachability lane: a sync-only observer runs no WireGuard
            // plane, but the channel must exist — black-hole.
            {
                let (_tx, mut rx) = network.register(CHANNEL_REACHABILITY, quota, MAX_BACKLOG);
                context
                    .child("blackhole_reachability")
                    .spawn(move |_ctx| async move { while rx.recv().await.is_ok() {} });
            }
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
            // the reachability lane: only MEMBERS run the WireGuard plane; a
            // parked joiner just keeps the channel legal — black-hole. (its
            // promotion reboots the node into the validator path, which wires
            // the plane for real.)
            {
                let (_tx, mut rx) = network.register(CHANNEL_REACHABILITY, quota, MAX_BACKLOG);
                context
                    .child("blackhole_reachability")
                    .spawn(move |_ctx| async move { while rx.recv().await.is_ok() {} });
            }
            // the lobby lane: where this parked node announces its key. member
            // replies are drained by a printer task — purely informational.
            let (mut lobby_tx, mut lobby_rx) = network.register(CHANNEL_LOBBY, quota, MAX_BACKLOG);
            {
                let label = label.clone();
                context.child("lobby_replies").spawn(move |_ctx| async move {
                    while let Ok((peer, msg)) = lobby_rx.recv().await {
                        let bytes: Vec<u8> = msg.into();
                        match lobby::decode_msg(&bytes) {
                            Ok(lobby::LobbyMsg::JoinReply { recorded, detail }) => println!(
                                "[node {label}] member {}: {}{detail}",
                                hex_bytes(&peer.as_ref()[..4]),
                                if recorded { "" } else { "join request refused — " },
                            ),
                            Ok(_) | Err(_) => {}
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
            let client =
                P2pSyncClient::new(context.child("sync_client"), sync_tx, sync_rx, server_peer);

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
            let mut attempt = 0usize;
            let mut announce_round = 0usize;
            // one round-robin lobby sender for BOTH announce kinds: the join
            // request while unregistered, the online announce once standby.
            let mut send_lobby =
                |targets: &[ed25519::PublicKey], attempt: usize, frame: IoBuf, what: &str| {
                    if attempt % LOBBY_ANNOUNCE_EVERY != 1 || targets.is_empty() {
                        return;
                    }
                    let target = targets[announce_round % targets.len()].clone();
                    announce_round += 1;
                    let attempted =
                        lobby_tx.send(Recipients::One(target.clone()), frame, false);
                    if !attempted.is_empty() {
                        println!(
                            "[node {label}] {what} sent to member {}",
                            hex_bytes(&target.as_ref()[..4])
                        );
                    }
                };
            // standby latch: sync capability is proven ONCE, then every
            // announce round carries a FRESH online proof (an expired proof
            // never wedges the flow — the next round re-signs).
            let mut standby_sync_proven = false;
            let (boundary, host, floor) = loop {
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
                        // two causes look identical on the wire here: this key is
                        // not admitted yet (the server's p2p bouncer rejects an
                        // un-tracked peer — the common case), or the bootstrap addr
                        // is genuinely unreachable. lead with admission and demote
                        // the raw transport error: the old "mesh unreachable /
                        // server dead" wording read as a crash and misdirected
                        // debugging. the joiner-mode banner above carries the exact
                        // `invite-accept <key>` command, so we don't repeat the key.
                        println!(
                            "[node {label}] parked: not yet admitted (or the mesh is \
                             unreachable) — a member must run `invite-accept` for this \
                             key; see the joiner-mode banner above. retrying ({e})"
                        );
                        if let Some(frame) = &announce_frame {
                            send_lobby(&announce_targets, attempt, frame.clone(), "join request");
                        }
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
                    // the manifest names the CURRENT members — better announce
                    // targets than the genesis descriptor's list.
                    let current: Vec<ed25519::PublicKey> = m
                        .participants
                        .iter()
                        .filter_map(|k| ed25519::PublicKey::decode(k.as_slice()).ok())
                        .collect();
                    if !current.is_empty() {
                        announce_targets = current;
                    }
                    // STANDBY probe: registration shows in the valset
                    // snapshot, not in `participants` (the engine set). once
                    // registered: prove sync capability ONCE, then announce
                    // online — a member relays the proof into the ordered
                    // lane and the activation cutover puts this key into
                    // `participants`, which the next loop iteration sees.
                    let standby_now = match read_standby_membership(&client, &m).await {
                        Ok((_, standby)) => standby.iter().any(|k| k == &me_bytes),
                        // a boundary hiccup re-probes next attempt.
                        Err(_) => false,
                    };
                    if standby_now {
                        if !standby_sync_proven {
                            println!(
                                "[node {label}] standby: registered — verifying state sync at \
                                 boundary {}",
                                m.height
                            );
                            match sync_all_modules(&context, &client, &m, &forge_repo, attempt)
                                .await
                            {
                                Ok(synced) => {
                                    println!(
                                        "[node {label}] standby: state verified \
                                         (app_hash={}) — announcing online",
                                        hex(&synced.app_hash())
                                    );
                                    standby_sync_proven = true;
                                    // capability proof only; promotion re-syncs
                                    // at its own (fresher) boundary below.
                                    drop(synced);
                                }
                                Err(e) => println!(
                                    "[node {label}] standby: sync not clean yet ({e}); retrying"
                                ),
                            }
                        }
                        if standby_sync_proven {
                            let frame = IoBuf::from(lobby::encode_msg(&lobby::online_announce(
                                &signer, m.height,
                            )));
                            send_lobby(&announce_targets, attempt, frame, "online announce");
                        }
                    } else {
                        println!(
                            "[node {label}] parked: awaiting admission (epoch {} has {} \
                             validators)",
                            m.epoch,
                            m.participants.len()
                        );
                        if let Some(frame) = &announce_frame {
                            send_lobby(&announce_targets, attempt, frame.clone(), "join request");
                        }
                    }
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
                // BOOT PREFLIGHT (design §5 / plan Task 7.3): refuse an
                // under-versioned binary against the served boundary before
                // install/replay — a clear early refusal, not a post-sync app-hash
                // mismatch. inert on a baseline manifest.
                if let Err(e) = m.preflight(MAX_PROTOCOL_VERSION) {
                    eprintln!("[node {label}] FATAL: cannot promote — {e}");
                    std::process::exit(1);
                }
                match sync_all_modules(&context, &client, &m, &forge_repo, attempt).await {
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
                                    match verify_manifest_floor(&namespace, &signer, &boundary) {
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
            // recovery boot turns it into a live validator. next_seq starts
            // at 1 — this identity never framed ops on this network. (a
            // REJOINING key that later resubmits a byte-identical (seq,
            // payload) pair could be dropped by a peer's in-process digest
            // gate; accepted edge until submit sequences ride app state.)
            let pos = recovery.oplog_pos().await;
            let floor_height = floor
                .as_ref()
                .map(|floor| floor.height.to_string())
                .unwrap_or_else(|| "none".to_string());
            diag_log(format!(
                "DIAG promotion_checkpoint checkpoint_height={} checkpoint_hash={} \
                 floor_height={} floor_present={}",
                boundary.height,
                hex(&host.app_hash()),
                floor_height,
                floor.is_some()
            ));
            // stamp the real committed version fields so the fabricated checkpoint
            // carries the same `required_min_version` a live checkpoint would; the
            // promotion boot then preflights against them like any restart.
            let (cv, pu) = read_upgrade_version_fields(&host).await;
            let ckpt = match Manifest::capture(
                &host,
                Some(boundary.height),
                boundary.epoch,
                boundary.view_base,
                boundary.participants.clone(),
                None,
                cv,
                pu,
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
            if let Some(fc) = &floor {
                if let Err(e) = recovery.write_floor_cert(fc).await {
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
        // ONE index fold for the whole boot (journal replay + post-reboot
        // catch-up + post-sync refreshes): its stop flag must persist across
        // phases — a later phase folding past a gap an earlier phase detected
        // would advance watermarks over the hole and hide it from the final
        // heal below.
        let mut boot_fold = IndexFold::new(&index);
        // (height, oplog position) for the pump's prune bookkeeping, and the
        // manifest that recovery used as its replay baseline).
        let (
            mut host,
            mut resumed,
            mut next_seq,
            mut prev_ckpt,
            mut recovery_manifest_for_resume,
        ): (
            Host,
            Option<recovery::Recovered>,
            u64,
            (Option<u64>, u64),
            Option<Manifest>,
        ) = match manifest.clone() {
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
                let host = genesis_host(&context, &forge_repo, &validators, blobs.clone()).await;
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
                let restored = restore_host(&context, &forge_repo, &manifest, blobs.clone()).await;
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

        // the TRANSPORT baseline adds the committed STANDBY set (registered,
        // quorum-exempt keys the mesh must admit so they can sync and
        // announce). read LIVE from the recovered host, unlike the frozen
        // participant set above: a standby registration arms its own cutover,
        // so within any epoch the standby set is constant — except a reboot
        // inside that cutover window, where this node briefly tracks the
        // wider set alone; the boundary re-tracks identically a few views
        // later.
        let initial_standby_keys: Vec<ed25519::PublicKey> = read_valset_membership(&host)
            .await
            .1
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
                    .chain(initial_standby_keys.iter())
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
                    let (cmd_tx, cmd_rx) =
                        tokio::sync::mpsc::channel::<reachability::ReachabilityCommand>(256);
                    let (ev_tx, mut ev_rx) =
                        tokio::sync::mpsc::channel::<reachability::ReachabilityEvent>(256);

                    // rendezvous coordinators = every coordinated-reach hint's
                    // coordinator ingress; hostnames resolve once at plane start.
                    let coordinators: Vec<Ingress> =
                        coordinated.iter().map(|(_, c, _)| c.clone()).collect();
                    let thread_label = label.clone();
                    let reach_signer = signer.clone();
                    let chain_id = String::from_utf8_lossy(&namespace).to_string();
                    let key_file = wireguard_key_file.clone();
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
                                    chain_id,
                                    reach_signer,
                                    key_file,
                                    wg_addr,
                                    wireguard_effect,
                                    advertised_reach,
                                    coordinators,
                                    cmd_rx,
                                    nudge_tx,
                                    ev_tx,
                                ));
                        })
                        .expect("spawn reachability thread");

                    // pump in: mesh datagrams -> orchestrator commands.
                    {
                        let cmd = cmd_tx.clone();
                        context.child("reachability_in").spawn(move |_ctx| async move {
                            while let Ok((peer, msg)) = reach_p2p_rx.recv().await {
                                let bytes: Vec<u8> = msg.into();
                                let deliver =
                                    reachability::ReachabilityCommand::Deliver { from: peer, bytes };
                                if cmd.send(deliver).await.is_err() {
                                    break;
                                }
                            }
                        });
                    }
                    // pump out: orchestrator sends -> mesh; everything else is
                    // operator-visible progress.
                    {
                        let pump_label = label.clone();
                        let mut tx = reach_p2p_tx;
                        context.child("reachability_out").spawn(move |_ctx| async move {
                            while let Some(event) = ev_rx.recv().await {
                                match event {
                                    reachability::ReachabilityEvent::Send { to, bytes } => {
                                        let _ =
                                            tx.send(Recipients::One(to), IoBuf::from(bytes), false);
                                    }
                                    reachability::ReachabilityEvent::MeshReady { epoch, .. } => {
                                        println!(
                                            "[node {pump_label}] reachability: epoch {epoch} mesh \
                                             verified"
                                        )
                                    }
                                    reachability::ReachabilityEvent::TunnelsApplied {
                                        epoch,
                                        interface,
                                        peers,
                                    } => match wireguard_effect {
                                        WireGuardEffectKind::Real => println!(
                                            "[node {pump_label}] reachability: epoch {epoch} \
                                             tunnels applied on {interface} ({peers} peer(s))"
                                        ),
                                        WireGuardEffectKind::Fake => println!(
                                            "[node {pump_label}] reachability: epoch {epoch} \
                                             tunnel config staged on {interface} ({peers} \
                                             peer(s); fake effect — no real interface)"
                                        ),
                                    },
                                    reachability::ReachabilityEvent::PeerFailed { peer, reason } => {
                                        println!(
                                            "[node {pump_label}] reachability: peer {}: {reason}",
                                            hex_bytes(&peer.as_ref()[..4])
                                        )
                                    }
                                    reachability::ReachabilityEvent::EpochFailed {
                                        epoch,
                                        reason,
                                    } => println!(
                                        "[node {pump_label}] reachability: epoch {epoch} failed: \
                                         {reason}"
                                    ),
                                }
                            }
                        });
                    }
                    Some(cmd_tx)
                }
                None => {
                    context
                        .child("blackhole_reachability")
                        .spawn(move |_ctx| async move { while reach_p2p_rx.recv().await.is_ok() {} });
                    drop(reach_p2p_tx);
                    None
                }
            };
        // boot: target the resume epoch's member set immediately; cutovers
        // retarget from the orchestrator loop below. the recovered view base
        // keeps advert expiries in the same view regime as live peers.
        if let Some(cmd) = &reach_cmd {
            let _ = cmd
                .send(reachability::ReachabilityCommand::Retarget(
                    reachability::MeshEpochEvent {
                        epoch: initial_resume_epoch,
                        members: initial_member_keys.clone(),
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
            let client = BootP2pSyncClient::new(sync_tx, sync_rx, server_peer);
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
                        let floor = match verify_manifest_floor(&namespace, &signer, target) {
                            Ok(floor) => floor,
                            Err(e) => {
                                eprintln!(
                                    "[node {label}] FATAL: catch-up target floor verify: {e}"
                                );
                                std::process::exit(1);
                            }
                        };
                        if target.epoch > resumed.as_ref().map(|rec| rec.epoch).unwrap_or(0) {
                            if let Err(e) = node::BlockSink::cutover(
                                &mut recovery,
                                target.epoch,
                                target.view_base,
                                &target.participants,
                            )
                            .await
                            {
                                eprintln!(
                                    "[node {label}] FATAL: catch-up cutover journal write: {e}"
                                );
                                std::process::exit(1);
                            }
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
                        let floor = match verify_manifest_floor(&namespace, &signer, &target) {
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
                            &forge_repo,
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
                        if target.epoch
                            > resumed.as_ref().map(|rec| rec.epoch).unwrap_or(0)
                        {
                            if let Err(e) = node::BlockSink::cutover(
                                &mut recovery,
                                target.epoch,
                                target.view_base,
                                &target.participants,
                            )
                            .await
                            {
                                eprintln!(
                                    "[node {label}] FATAL: full-sync cutover journal write: {e}"
                                );
                                std::process::exit(1);
                            }
                        }
                        let pos = recovery.oplog_pos().await;
                        let ckpt = match Manifest::capture(
                            &host,
                            Some(target.height),
                            target.epoch,
                            target.view_base,
                            target.participants.clone(),
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
                    Err(PostRebootCatchupError::Retry(e)) if attempts < 10 => {
                        println!(
                            "[node {label}] post-reboot catch-up unavailable ({e}); retrying"
                        );
                        context.sleep(Duration::from_millis(500)).await;
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
            match client.into_parts() {
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
        if pending_boot.is_none() {
            if let Some(rec) = resumed.as_ref() {
                pending_boot = read_upgrade_state(&host).await.pending.and_then(|p| {
                    let crossed = rec.height.is_some_and(|h| h >= p.activation_height);
                    if crossed {
                        None
                    } else {
                        p.activation_height.checked_sub(rec.view_base)
                    }
                });
            }
        }

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
        let mut orchestrator = consensus::ValsetOrchestrator::resume_with_transport(
            CUTOVER_DELAY,
            member_keys.clone(),
            member_keys
                .iter()
                .cloned()
                .chain(initial_standby_keys.iter().cloned()),
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
        // verified-but-unapproved join requests, keyed by joiner key. NODE-
        // LOCAL and in-memory by design: this is a doorbell, not state — the
        // parked joiner re-announces every few seconds, so a restart loses
        // nothing durable. read by the `join-requests` rpc; entries whose key
        // has since become a member are dropped at read time.
        let mut join_requests: std::collections::BTreeMap<Vec<u8>, JoinRequestRecord> =
            std::collections::BTreeMap::new();
        // online-announce relay latch: standby key -> the proof height last
        // relayed into the ordered lane. keyed by proof height so a FRESH
        // proof (an expired one re-announced) relays again, while the same
        // proof re-announced every few seconds submits exactly once.
        let mut online_relays: std::collections::BTreeMap<Vec<u8>, u64> =
            std::collections::BTreeMap::new();
        // recovery cadence: sealed blocks since the last checkpoint manifest.
        let mut blocks_since_checkpoint: u64 = 0;
        // the last absolute view ticked to the reachability plane — one
        // ViewTick per actual advance, not one per 100ms drain pass.
        let mut last_reach_view: Option<u64> = None;
        // throttle for the pending-cutover nop pusher below.
        let mut last_nop = std::time::Instant::now();
        // throttle for the saga crank pump below.
        let mut last_crank = std::time::Instant::now();
        // the host-owned worker set (reactor seam): effects of finalized
        // blocks are offered here, and claimed follow-ups re-enter the ordered
        // lane as their own blocks.
        // load capability specs and discover this host's installed executor
        // CLIs (BYO — no credential handling here). the discovered tag set is
        // BOTH what the oracle worker can run and what this node announces to
        // the capability registry, so the two can never drift. routing and
        // default models live in the specs (docs/capability-spec.md); a broken
        // operator spec is a boot error, not a silently dropped executor.
        let providers = capability_host::discover()
            .unwrap_or_else(|e| panic!("capability specs failed to load: {e}"));
        let my_capabilities = providers.capabilities();
        let workers: Vec<Box<dyn reactor::Worker>> = vec![Box::new(LlmWorker::new(
            blobs.clone(),
            providers,
            // this node's submit key: WorkerRequests leased to another
            // node's key are skipped, not double-run.
            signer.public_key().as_ref().to_vec(),
        ))];
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
                    let drained = node.take_drained();
                    // sealed = journaled: applied and rejected frames both got
                    // recovery seals; discarded frames were never journaled.
                    blocks_since_checkpoint += drained
                        .iter()
                        .filter(|d| d.disposition != node::Disposition::Discarded)
                        .count() as u64;
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
                    for d in &drained {
                        if d.disposition == node::Disposition::Discarded {
                            continue;
                        }
                        let dispatches: &[host::DispatchRecord] = match (&d.disposition, &d.op) {
                            (node::Disposition::Applied, Some(op)) => &op.dispatches,
                            _ => &[],
                        };
                        let record = match &d.op {
                            Some(op) if op.target != NOP_TARGET => {
                                let disposition = match d.disposition {
                                    node::Disposition::Applied => noded::BlockDisposition::Applied,
                                    node::Disposition::Rejected => noded::BlockDisposition::Rejected,
                                    // unreachable — filtered at the loop top —
                                    // but stay total on this observability
                                    // lane rather than panic.
                                    node::Disposition::Discarded => continue,
                                };
                                Some(noded::block_row(&noded::BlockRecord {
                                    height: d.height,
                                    hash: noded::hex_bytes(&d.id),
                                    commit_hash: hex(&d.app_hash),
                                    proposer: match &op.origin {
                                        sdk::Origin::External(key) => noded::hex_bytes(key),
                                        // frames only carry verified External
                                        // authorship; label the impossible rest.
                                        sdk::Origin::Module(id) => format!("module:{id}"),
                                        sdk::Origin::System => "system".into(),
                                    },
                                    disposition,
                                    target: op.target.clone(),
                                    operations: op
                                        .dispatches
                                        .iter()
                                        .map(noded::DispatchInfo::from)
                                        .collect(),
                                    payload: noded::payload_preview(&op.payload),
                                    // staging IS hashing: put_chunk keys the
                                    // blob by sha256, so this one call both
                                    // computes the op's content address and
                                    // makes it dereferencable via
                                    // GET /v1/files/blob/{op_hash}.
                                    op_hash: noded::hex_bytes(&blobs.put_chunk(op.payload.clone())),
                                }))
                            }
                            _ => None,
                        };
                        let ops = indexer::BlockOps {
                            record,
                            // this lane's agreed clock IS the height: the
                            // drain stamps BlockContext { consensus_time:
                            // height } for every frame.
                            ..noded::index_block_ops(d.height, d.height, dispatches)
                        };
                        if let Err(err) = index.apply_block(&ops) {
                            eprintln!(
                                "[node {label}] module index apply failed at height {}: {err} \
                                 — wipe <storage>/index to rebuild",
                                d.height
                            );
                        }
                    }
                    for d in drained {
                        let Some((reply, _)) = pending_submits.remove(&d.id) else { continue };
                        let _ = reply.send(match d.disposition {
                            node::Disposition::Applied => Ok(noded::BlockSummary {
                                height: d.height,
                                // the PER-BLOCK boundary this frame settled at
                                // (not the end-of-drain hash — a drain can
                                // apply several blocks).
                                app_hash: hex(&d.app_hash),
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
                    // (send only errs when nobody is subscribed — fine). the
                    // validator serves block frames only — telemetry frames come
                    // from the local `noded` daemon, which owns the dispatch
                    // trace this finalized-boundary seam does not carry.
                    if let Some(f) = node.finalized() {
                        if last_published != Some(f.height) {
                            let _ = http_events.send(noded::WsFrame::Block(noded::BlockSummary {
                                height: f.height,
                                app_hash: hex(&f.app_hash),
                            }));
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
                            let (cv, pu) = read_upgrade_version_fields(node.host()).await;
                            let captured = Manifest::capture(
                                node.host(),
                                Some(f.height),
                                orchestrator.epoch(),
                                orchestrator.epoch_base(),
                                participant_bytes(&orchestrator),
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
                        // tick the reachability plane's freshness clock.
                        // engine views are EPOCH-LOCAL (they reset at every
                        // cutover), so convert to the absolute app-height
                        // clock (`epoch_base + view`) — the regime the boot
                        // Retarget's `view_base` put the plane's advert and
                        // handshake expiries in.
                        if let Some(cmd) = &reach_cmd {
                            let absolute_view = orchestrator.app_height(engine_view);
                            if last_reach_view.is_none_or(|v| v < absolute_view) {
                                let _ = cmd
                                    .send(reachability::ReachabilityCommand::ViewTick(
                                        absolute_view,
                                    ))
                                    .await;
                                last_reach_view = Some(absolute_view);
                            }
                        }
                        let (active_raw, standby_raw) =
                            read_valset_membership(node.host()).await;
                        let decode_keys = |raw: &[Vec<u8>]| -> Vec<ed25519::PublicKey> {
                            raw.iter()
                                .filter_map(|key| ed25519::PublicKey::decode(key.as_slice()).ok())
                                .collect()
                        };
                        let observed = decode_keys(&active_raw);
                        let observed_standby = decode_keys(&standby_raw);
                        if let consensus::ObservationOutcome::Scheduled(cutover) =
                            orchestrator.observe_members(
                                engine_view,
                                observed.iter().cloned(),
                                observed_standby.iter().cloned(),
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
                        if let Some(pending) = &boundary_upgrade.pending {
                            if let consensus::ObservationOutcome::Scheduled(cutover) =
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
                        }
                        if let Some(plan) = orchestrator.respawn_if_due(
                            engine_view,
                            observed,
                            observed_standby,
                            boundary_upgrade,
                        ) {
                            let members = plan.valset().consensus_members();
                            let member_bytes: Vec<Vec<u8>> =
                                members.iter().map(|k| k.as_ref().to_vec()).collect();
                            // transport FIRST: the new epoch's mesh must admit
                            // its members (a fresh joiner above all, standby
                            // registrants included) before anything is
                            // expected of them. index = epoch, strictly
                            // increasing across cutovers.
                            mesh_oracle
                                .track(plan.epoch(), mesh_at(plan.valset().transport_members()));
                            // the reachability plane retunnels for the new
                            // member set the moment transport admits it.
                            // cutover_app_height IS the new epoch's absolute
                            // view at engine view 0 — the raw engine_view
                            // here would be epoch-local, a different clock
                            // than the ViewTicks above and the boot
                            // Retarget's view_base.
                            if let Some(cmd) = &reach_cmd {
                                let _ = cmd
                                    .send(reachability::ReachabilityCommand::Retarget(
                                        reachability::MeshEpochEvent {
                                            epoch: plan.epoch(),
                                            members: members.iter().cloned().collect(),
                                            current_view: plan.cutover_app_height(),
                                        },
                                    ))
                                    .await;
                            }
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

                    // heartbeat: finalized views only advance with ops, so an
                    // idle network freezes — its height never ticks, and a
                    // pending cutover (which crosses only when finalized views
                    // REACH it) would park at the armed boundary forever. push a
                    // deterministically-rejected nop (unknown module target:
                    // rejects identically on every node, leaves no state) once
                    // per block-time so the chain — and the height the console
                    // shows — keeps moving whether or not anyone is active.
                    //
                    // GATE on an EMPTY pending FIFO: the queue is strictly serial
                    // (one frame finalized per block), so a nop pushed while real
                    // frames are pending only builds a backlog that starves real
                    // ops after any finalization stall (a flapping quorum peer piles
                    // nops at 1/s; recovery then drains them ahead of real work —
                    // minutes of head-of-line latency). a nop only ticks an IDLE
                    // chain, and an idle chain has an empty queue, so beat only when
                    // the FIFO is empty — at most one nop outstanding, and only when
                    // it is alone. the reset stays inside the taken branch: when the
                    // gate skips, the timer stays elapsed, so the first tick after
                    // the queue drains injects the next nop immediately.
                    if last_nop.elapsed() >= HEARTBEAT_INTERVAL
                        && node.orderer().pending_len() == 0
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
                            eprintln!("[node {label}] heartbeat nop submit failed: {e}");
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
                    // + local latch). inert on a host with no executor CLIs.
                    if orchestrator
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

                    // SAGA CRANK (P7 liveness, host side): nothing else ever
                    // submits `SagaMsg::Crank`, and under strict leases a
                    // saga whose assignee went dark advances ONLY via a crank
                    // (lease re-lease or deadline timeout). state-driven:
                    // when the committed next expiry is at or past the latest
                    // finalized height, push one permissionless crank —
                    // throttled like the heartbeat, since a backlog wider
                    // than CRANK_BUDGET legitimately needs several. duplicate
                    // cranks from other nodes are deterministic no-ops.
                    if last_crank.elapsed() >= HEARTBEAT_INTERVAL
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
                                    payload: saga_interface::encode_msg(
                                        &saga_interface::SagaMsg::Crank {},
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
                            // read-time hygiene: an approved joiner is
                            // REGISTERED now (standby or already active) —
                            // its request is settled, drop it.
                            let (active, standby) =
                                read_valset_membership(node.host()).await;
                            join_requests.retain(|joiner, _| {
                                !active.contains(joiner) && !standby.contains(joiner)
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
                announce = lobby_ingress.next() => {
                    let Some((peer, bytes)) = announce else { continue };
                    let mut send_reply = |recorded: bool, detail: String| {
                        let msg = lobby::LobbyMsg::JoinReply { recorded, detail };
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
                    // the ONLINE announce: a standby key proving it is up.
                    // verify the proof, gate on committed standby membership,
                    // relay it into the ordered lane as `ValsetMsg::Online` —
                    // the module re-verifies the identical proof, so this
                    // member attests nothing. latched per proof height.
                    if let lobby::LobbyMsg::OnlineAnnounce { signed_height, .. } = &msg {
                        let signed_height = *signed_height;
                        let standby_key = match lobby::verify_online_announce(&msg) {
                            Ok(pk) => pk,
                            Err(e) => {
                                send_reply(false, e);
                                continue;
                            }
                        };
                        let key = standby_key.as_ref().to_vec();
                        let (active, standby) = read_valset_membership(node.host()).await;
                        if active.contains(&key) {
                            // already activated — the announcer will see the
                            // participant set shortly; nothing to relay.
                            continue;
                        }
                        if !standby.contains(&key) {
                            send_reply(false, "not a registered standby key".into());
                            continue;
                        }
                        if online_relays.get(&key) == Some(&signed_height) {
                            continue; // this exact proof already relayed.
                        }
                        // only an active member has standing to carry the
                        // frame (the module enforces the same rule).
                        if !orchestrator.current_members().contains(&signer.public_key()) {
                            continue;
                        }
                        let lobby::LobbyMsg::OnlineAnnounce { key: key_bytes, signature, .. } =
                            msg
                        else {
                            unreachable!("matched above");
                        };
                        let seq = next_seq;
                        next_seq += 1;
                        let submit = node
                            .submit(
                                &signer,
                                seq,
                                Msg {
                                    target: "valset".into(),
                                    payload: valset_interface::encode_msg(
                                        &valset_interface::ValsetMsg::Online {
                                            key: key_bytes,
                                            signed_height,
                                            signature,
                                        },
                                    ),
                                },
                            )
                            .await;
                        match submit {
                            Ok(_) => {
                                online_relays.insert(key, signed_height);
                                println!(
                                    "[node {label}] online announce from standby {} relayed \
                                     (proof height {signed_height})",
                                    hex_bytes(&standby_key.as_ref()[..4])
                                );
                                send_reply(true, "online announce relayed".into());
                            }
                            Err(e) => {
                                eprintln!(
                                    "[node {label}] online relay submit failed: {e} — the \
                                     announcer retries"
                                );
                            }
                        }
                        continue;
                    }
                    // crypto first (pure, cheap): the token must verify for
                    // THIS network and the announced key must prove itself.
                    let verified = match lobby::verify_join_request(&msg, &namespace) {
                        Ok(v) => v,
                        Err(e) => {
                            send_reply(false, e);
                            continue;
                        }
                    };
                    // then membership: the issuer must still be a member (a
                    // removed member's outstanding invites die with it), and a
                    // joiner that is already registered — ACTIVE or STANDBY —
                    // has nothing pending.
                    let (active_members, standby_members) =
                        read_valset_membership(node.host()).await;
                    let joiner_bytes = verified.joiner.as_ref().to_vec();
                    if active_members.contains(&joiner_bytes) {
                        send_reply(false, "already a validator".into());
                        continue;
                    }
                    if standby_members.contains(&joiner_bytes) {
                        send_reply(
                            false,
                            "already registered as standby — announce online instead".into(),
                        );
                        continue;
                    }
                    let members = active_members;
                    if !members.contains(&verified.issuer.as_ref().to_vec()) {
                        send_reply(
                            false,
                            "the inviting member is no longer part of this network".into(),
                        );
                        continue;
                    }
                    let now = unix_ms();
                    let fresh = !join_requests.contains_key(&joiner_bytes);
                    let record = join_requests
                        .entry(joiner_bytes)
                        .or_insert(JoinRequestRecord {
                            issuer: verified.issuer.as_ref().to_vec(),
                            first_seen_ms: now,
                            last_seen_ms: now,
                        });
                    record.last_seen_ms = now;
                    if fresh {
                        println!(
                            "[node {label}] join request: {} asks to join (invited by {}) — \
                             approve in the app, or run `ducktape-node invite-accept {}`",
                            hex_bytes(verified.joiner.as_ref()),
                            hex_bytes(&record.issuer),
                            hex_bytes(verified.joiner.as_ref())
                        );
                    }
                    send_reply(
                        true,
                        "join request recorded — awaiting member approval".into(),
                    );
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
                                    category: noded::ModuleCategory::of(m),
                                })
                                .collect();
                            let _ = reply.send(noded::NodeStatus {
                                version: env!("CARGO_PKG_VERSION").into(),
                                app_hash: hex(&node.app_hash()),
                                height: node.finalized().map(|f| f.height).unwrap_or(0),
                                modules,
                            });
                        }
                        noded::NodeCommand::Metrics { reply } => {
                            // the validator serves commonware's runtime registry;
                            // the `ducktape_*` block series are the local daemon's
                            // (noded's) surface, not wired into this consensus path.
                            let _ = reply.send(context.encode());
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
                    let resp = match statesync::decode_request(body) {
                        Ok(statesync::SyncRequest::Frames {
                            after_height,
                            up_to_height,
                        }) => {
                            let response = match node
                                .sink_mut()
                                .read_finalized_frames(after_height, up_to_height)
                                .await
                            {
                                Ok(frames) => {
                                    let mut out = Vec::new();
                                    let mut err = None;
                                    for frame in frames
                                        .into_iter()
                                        .take(statesync::FRAME_BATCH_LEN)
                                    {
                                        match recovery_frame_to_sync(frame) {
                                            Ok(frame) => out.push(frame),
                                            Err(e) => {
                                                err = Some(e);
                                                break;
                                            }
                                        }
                                    }
                                    match err {
                                        Some(e) => statesync::SyncResponse::Error(e),
                                        None => statesync::SyncResponse::Frames { frames: out },
                                    }
                                }
                                Err(recovery::Error::RangePruned {
                                    after_height,
                                    retained_start,
                                }) => statesync::SyncResponse::RangePruned {
                                    requested_after: after_height,
                                    retained_from: retained_start,
                                },
                                Err(e) => statesync::SyncResponse::Error(format!(
                                    "recovery frame range: {e}"
                                )),
                            };
                            statesync::encode_response(&response)
                        }
                        Ok(req) => {
                            // the shipped-index lane cuts lazily: the FIRST
                            // index request for a boundary checkpoints the
                            // derived databases and attaches the archives to
                            // that capture, so joiners that never opt in cost
                            // nothing. an unleased boundary cannot hold an
                            // attachment — handle() below answers it with the
                            // proper refetch error either way.
                            if let statesync::SyncRequest::IndexModules { boundary } = &req {
                                if !sync_server.index_attached(*boundary) {
                                    let _ = sync_server
                                        .attach_index(*boundary, ship_index_blobs(&index, &label));
                                }
                            }
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
                            let response =
                                sync_server.handle(node.host(), finalized_for_sync, &coords, req).await;
                            statesync::encode_response(&response)
                        }
                        Err(e) => statesync::encode_response(&statesync::SyncResponse::Error(
                            format!("bad request frame: {e}"),
                        )),
                    };
                    let _ = sync_tx.send(
                        Recipients::One(peer),
                        IoBuf::from(statesync::encode_rpc(rpc_id, &resp)),
                        false,
                    );
                }
            }
        }
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdk::{Ctx, Error, Module, StateSyncHandle};
    use std::sync::{Arc, Mutex};
    use upgrade_interface::{Upgrade, UpgradeStatus};

    fn test_root(byte: u8) -> StateRoot {
        StateRoot([byte; sdk::ROOT_LEN])
    }

    fn test_me() -> Vec<u8> {
        vec![1u8; 32]
    }

    fn test_manifest(
        height: u64,
        app_hash: StateRoot,
        floor_cert: Option<Vec<u8>>,
    ) -> statesync::Manifest {
        statesync::Manifest {
            height,
            app_hash,
            epoch: 0,
            view_base: 0,
            participants: vec![test_me()],
            floor_cert,
            current_version: host::BASELINE_VERSION,
            pending_upgrade: None,
            required_min_version: host::BASELINE_VERSION,
            entries: vec![],
        }
    }

    fn test_manifest_with_participants(
        height: u64,
        app_hash: StateRoot,
        floor_cert: Option<Vec<u8>>,
        participants: Vec<Vec<u8>>,
    ) -> statesync::Manifest {
        statesync::Manifest {
            participants,
            ..test_manifest(height, app_hash, floor_cert)
        }
    }

    fn test_manifest_with_base(
        height: u64,
        view_base: u64,
        app_hash: StateRoot,
        floor_cert: Option<Vec<u8>>,
    ) -> statesync::Manifest {
        statesync::Manifest {
            view_base,
            ..test_manifest(height, app_hash, floor_cert)
        }
    }

    fn fresh_directory_host() -> Host {
        Host::genesis(vec![Box::new(Directory::new("directory"))]).expect("genesis")
    }

    #[derive(Clone, Default)]
    struct TestDiskStore(Arc<Mutex<u8>>);

    impl TestDiskStore {
        fn get(&self) -> u8 {
            *self.0.lock().expect("test disk store lock")
        }

        fn set(&self, value: u8) {
            *self.0.lock().expect("test disk store lock") = value;
        }
    }

    struct TestDiskModule {
        store: TestDiskStore,
        staged: Option<u8>,
    }

    impl TestDiskModule {
        fn new(store: TestDiskStore) -> Self {
            Self {
                store,
                staged: None,
            }
        }
    }

    #[async_trait::async_trait(?Send)]
    impl Module for TestDiskModule {
        fn id(&self) -> String {
            "disk".into()
        }

        fn root(&self) -> StateRoot {
            test_root(self.store.get())
        }

        fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
            Ok(StateSyncHandle::ResolverBacked {
                backend: "test-disk".into(),
                detail: "test disk module reopens from shared durable state".into(),
            })
        }

        async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
            let value = *msg
                .payload
                .first()
                .ok_or_else(|| Error::Module("missing test value".into()))?;
            self.staged = Some(value);
            ctx.emit_msg(Msg {
                target: "memory".into(),
                payload: vec![value],
            });
            Ok(())
        }

        async fn commit_block(&mut self) -> Result<(), Error> {
            if let Some(value) = self.staged.take() {
                self.store.set(value);
            }
            Ok(())
        }

        async fn abort_block(&mut self) -> Result<(), Error> {
            self.staged = None;
            Ok(())
        }
    }

    struct TestMemoryModule {
        value: u8,
        staged: Option<u8>,
    }

    impl TestMemoryModule {
        fn new(value: u8) -> Self {
            Self {
                value,
                staged: None,
            }
        }

        fn install(&mut self, bytes: &[u8], root: StateRoot) -> Result<(), Error> {
            let [value] = bytes else {
                return Err(Error::Module("bad test memory snapshot".into()));
            };
            if test_root(*value) != root {
                return Err(Error::Module("test memory root mismatch".into()));
            }
            self.value = *value;
            self.staged = None;
            Ok(())
        }
    }

    #[async_trait::async_trait(?Send)]
    impl Module for TestMemoryModule {
        fn id(&self) -> String {
            "memory".into()
        }

        fn root(&self) -> StateRoot {
            test_root(self.value)
        }

        fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
            Ok(StateSyncHandle::SnapshotBytes(vec![self.value]))
        }

        async fn execute(&mut self, _ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
            let value = *msg
                .payload
                .first()
                .ok_or_else(|| Error::Module("missing test value".into()))?;
            self.staged = Some(value);
            Ok(())
        }

        async fn commit_block(&mut self) -> Result<(), Error> {
            if let Some(value) = self.staged.take() {
                self.value = value;
            }
            Ok(())
        }

        async fn abort_block(&mut self) -> Result<(), Error> {
            self.staged = None;
            Ok(())
        }
    }

    fn mixed_durability_host(store: TestDiskStore, memory_value: u8) -> Host {
        Host::genesis(vec![
            Box::new(TestDiskModule::new(store)),
            Box::new(TestMemoryModule::new(memory_value)),
        ])
        .expect("mixed host")
    }

    fn restore_mixed_durability_host(store: TestDiskStore, manifest: &Manifest) -> Host {
        let mut memory = TestMemoryModule::new(0);
        memory
            .install(
                manifest.snapshot("memory").expect("memory snapshot"),
                manifest.root("memory").expect("memory root"),
            )
            .expect("memory install");
        Host::genesis(vec![Box::new(TestDiskModule::new(store)), Box::new(memory)])
            .expect("restored mixed host")
    }

    fn dir_set(key: &str, value: &str) -> Msg {
        Msg {
            target: "directory".into(),
            payload: encode_msg(&DirMsg::Set {
                key: key.into(),
                value: value.into(),
            }),
        }
    }

    async fn dir_value(host: &Host, key: &str) -> Option<String> {
        let reply = host
            .query(
                "directory",
                &encode_query(&DirQuery::Get { key: key.into() }),
            )
            .await
            .expect("query");
        match decode_reply(&reply).expect("decode") {
            DirReply::Value(v) => v,
        }
    }

    async fn served_directory_frame(
        expected: &mut Host,
        signer: &ed25519::PrivateKey,
        height: u64,
        seq: u64,
        msg: Msg,
    ) -> statesync::FinalizedFrame {
        let frame = node::encode_frame(signer, seq, &msg);
        let (origin, msg) = node::decode_frame(&frame).expect("decode frame");
        expected
            .submit_at(
                host::BlockContext {
                    protocol_version: host::BASELINE_VERSION,
                    height,
                    consensus_time: height,
                    origin,
                },
                msg,
            )
            .await
            .expect("apply");
        statesync::FinalizedFrame {
            height,
            frame,
            disposition: statesync::FrameDisposition::Applied,
            roots: expected.module_roots(),
            app_hash: expected.app_hash(),
        }
    }

    async fn served_mixed_frame(
        expected: &mut Host,
        signer: &ed25519::PrivateKey,
        height: u64,
        seq: u64,
        value: u8,
    ) -> statesync::FinalizedFrame {
        let frame = node::encode_frame(
            signer,
            seq,
            &Msg {
                target: "disk".into(),
                payload: vec![value],
            },
        );
        let (origin, msg) = node::decode_frame(&frame).expect("decode frame");
        expected
            .submit_at(
                host::BlockContext {
                    protocol_version: host::BASELINE_VERSION,
                    height,
                    consensus_time: height,
                    origin,
                },
                msg,
            )
            .await
            .expect("apply mixed frame");
        statesync::FinalizedFrame {
            height,
            frame,
            disposition: statesync::FrameDisposition::Applied,
            roots: expected.module_roots(),
            app_hash: expected.app_hash(),
        }
    }

    #[test]
    fn floor_cert_view_must_map_to_boundary_height() {
        assert!(assert_floor_binds_view(30, 36, 6).is_ok());
        assert!(assert_floor_binds_view(30, 36, 4).is_err());
    }

    #[test]
    fn promotion_boundary_prefers_latest_same_state_height() {
        let host_hash = test_root(7);
        let latest = test_manifest(12, host_hash, Some(vec![2]));

        match choose_promotion_boundary(host_hash, &latest, &test_me()) {
            PromotionBoundary::Promote { boundary, source } => {
                assert_eq!(boundary.height, 12);
                assert_eq!(source, PromotionBoundarySource::Latest);
            }
            PromotionBoundary::Retry => panic!("same-state latest boundary should promote"),
        }
    }

    #[test]
    fn promotion_boundary_retries_when_latest_excludes_self() {
        let host_hash = test_root(7);
        let me = vec![1u8; 32];
        let latest =
            test_manifest_with_participants(12, host_hash, Some(vec![2]), vec![vec![9u8; 32]]);

        match choose_promotion_boundary(host_hash, &latest, &me) {
            PromotionBoundary::Retry => {}
            PromotionBoundary::Promote { .. } => {
                panic!("latest boundary excluding this node must not promote")
            }
        }
    }

    #[test]
    fn promotion_boundary_accepts_latest_at_view_base_without_floor() {
        let host_hash = test_root(7);
        let latest = test_manifest_with_base(12, 12, host_hash, None);

        match choose_promotion_boundary(host_hash, &latest, &test_me()) {
            PromotionBoundary::Promote { boundary, source } => {
                assert_eq!(boundary.height, 12);
                assert_eq!(source, PromotionBoundarySource::Latest);
            }
            PromotionBoundary::Retry => panic!("view-base latest boundary should promote"),
        }
    }

    #[test]
    fn promotion_boundary_requires_latest_floor_past_view_base() {
        let host_hash = test_root(7);
        let latest = test_manifest_with_base(12, 10, host_hash, None);

        match choose_promotion_boundary(host_hash, &latest, &test_me()) {
            PromotionBoundary::Retry => {}
            PromotionBoundary::Promote { .. } => {
                panic!("past-base latest boundary without a floor should retry")
            }
        }
    }

    #[test]
    fn promotion_boundary_retries_when_latest_changed() {
        let host_hash = test_root(7);
        let latest = test_manifest(12, test_root(9), Some(vec![2]));

        match choose_promotion_boundary(host_hash, &latest, &test_me()) {
            PromotionBoundary::Retry => {}
            PromotionBoundary::Promote { .. } => {
                panic!("changed latest boundary should retry")
            }
        }
    }

    #[test]
    fn promotion_boundary_retries_when_no_manifest_matches_host() {
        let host_hash = test_root(7);
        let latest = test_manifest(12, test_root(9), Some(vec![2]));

        match choose_promotion_boundary(host_hash, &latest, &test_me()) {
            PromotionBoundary::Retry => {}
            PromotionBoundary::Promote { .. } => panic!("changed roots should retry"),
        }
    }

    #[test]
    fn suffix_installer_rejects_mismatched_served_seal() {
        let executor = commonware_runtime::deterministic::Runner::default();
        executor.start(|_| async move {
            let signer = commonware_cryptography::ed25519::PrivateKey::from_seed(77);
            let msg = Msg {
                target: "directory".into(),
                payload: encode_msg(&DirMsg::Set {
                    key: "k".into(),
                    value: "v".into(),
                }),
            };
            let frame = node::encode_frame(&signer, 0, &msg);

            let mut expected_host =
                Host::genesis(vec![Box::new(Directory::new("directory"))]).expect("genesis");
            let (origin, msg) = node::decode_frame(&frame).expect("decode frame");
            expected_host
                .submit_at(
                    host::BlockContext {
                        protocol_version: host::BASELINE_VERSION,
                        height: 1,
                        consensus_time: 1,
                        origin,
                    },
                    msg,
                )
                .await
                .expect("apply");

            let served = statesync::FinalizedFrame {
                height: 1,
                frame,
                disposition: statesync::FrameDisposition::Applied,
                roots: expected_host.module_roots(),
                app_hash: StateRoot([0xA5; sdk::ROOT_LEN]),
            };
            let mut host =
                Host::genesis(vec![Box::new(Directory::new("directory"))]).expect("genesis");
            let err = apply_verified_suffix_frame(&mut host, &served)
                .await
                .expect_err("served seal mismatch must abort");
            assert!(
                err.contains("served seal"),
                "unexpected mismatch error: {err}"
            );
        });
    }

    #[test]
    fn post_reboot_catchup_applies_verifies_and_journals_served_suffix() {
        let executor = commonware_runtime::deterministic::Runner::default();
        executor.start(|context| async move {
            let signer = ed25519::PrivateKey::from_seed(78);
            let mut expected = fresh_directory_host();
            let frames = vec![
                served_directory_frame(&mut expected, &signer, 1, 0, dir_set("a", "1")).await,
                served_directory_frame(&mut expected, &signer, 2, 1, dir_set("b", "2")).await,
            ];

            let mut host = fresh_directory_host();
            let mut recovery = Recovery::open(context.child("post_catchup_ok"))
                .await
                .expect("open recovery");
            let applied =
                apply_post_reboot_catchup_frames(&mut recovery, &mut host, 0, 2, frames.clone(), None)
                    .await
                    .expect("catch up");

            assert_eq!(applied.applied, 2);
            assert_eq!(host.app_hash(), expected.app_hash());
            assert_eq!(dir_value(&host, "a").await.as_deref(), Some("1"));
            assert_eq!(dir_value(&host, "b").await.as_deref(), Some("2"));
            let journaled = recovery
                .read_finalized_frames(0, 2)
                .await
                .expect("read frames");
            assert_eq!(journaled.len(), 2);
            assert_eq!(journaled[0].height, 1);
            assert_eq!(journaled[1].height, 2);
        });
    }

    #[test]
    fn post_reboot_catchup_checkpoint_makes_mixed_durability_suffix_recoverable() {
        let executor = commonware_runtime::deterministic::Runner::default();
        executor.start(|context| async move {
            let signer = ed25519::PrivateKey::from_seed(81);
            let durable_store = TestDiskStore::default();
            let base_host = mixed_durability_host(durable_store.clone(), 0);
            let base_manifest = Manifest::capture(
                &base_host,
                None,
                0,
                0,
                vec![test_me()],
                None,
                host::BASELINE_VERSION,
                None,
                0,
                1,
            )
            .expect("base manifest");

            let mut expected = mixed_durability_host(TestDiskStore::default(), 0);
            let served = served_mixed_frame(&mut expected, &signer, 1, 0, 7).await;
            let target = statesync::Manifest {
                height: 1,
                app_hash: served.app_hash,
                epoch: 0,
                view_base: 0,
                participants: vec![test_me()],
                floor_cert: Some(vec![1, 2, 3]),
                current_version: host::BASELINE_VERSION,
                pending_upgrade: None,
                required_min_version: host::BASELINE_VERSION,
                entries: vec![],
            };

            let mut host = mixed_durability_host(durable_store.clone(), 0);
            let mut recovery = Recovery::open(context.child("post_catchup_mixed"))
                .await
                .expect("open recovery");
            recovery
                .write_manifest(&base_manifest)
                .await
                .expect("write base manifest");
            let applied =
                apply_post_reboot_catchup_frames(&mut recovery, &mut host, 0, 1, vec![served], None)
                    .await
                    .expect("catch up");

            assert_eq!(applied.applied, 1);
            assert_eq!(
                durable_store.get(),
                7,
                "disk cohort committed the catch-up block durably"
            );

            // an old-base replay reconciles the torn sealed block via selective
            // replay (the still-at-pre memory cohort recommits, the already-
            // durable disk cohort aborts) rather than fail-stopping; the
            // checkpoint's value below is recovering WITHOUT that replay.
            let mut torn_host =
                restore_mixed_durability_host(durable_store.clone(), &base_manifest);
            let healed = recovery
                .recover(&mut torn_host, &base_manifest)
                .await
                .expect("old base replay heals the torn sealed block selectively");
            assert_eq!(healed.height, Some(1));
            assert_eq!(healed.app_hash, target.app_hash);
            assert_eq!(healed.applied, 1, "the torn suffix frame was replayed");
            assert_eq!(
                durable_store.get(),
                7,
                "disk cohort stays at its durable post-state"
            );

            let ckpt = write_post_reboot_catchup_checkpoint(
                &mut recovery,
                &host,
                Some(&base_manifest),
                &target,
                &applied.blocks,
                1,
            )
            .await
            .expect("write catch-up checkpoint");
            assert_eq!(ckpt.height, Some(1));
            assert_eq!(ckpt.app_hash, target.app_hash);
            assert_eq!(ckpt.snapshot("memory"), Some([7u8].as_slice()));

            let mut restored = restore_mixed_durability_host(durable_store, &ckpt);
            let recovered = recovery
                .recover(&mut restored, &ckpt)
                .await
                .expect("T checkpoint must recover without replaying the torn suffix");
            assert_eq!(recovered.height, Some(1));
            assert_eq!(recovered.app_hash, target.app_hash);
            assert_eq!(recovered.applied, 0);
        });
    }

    #[test]
    fn post_reboot_catchup_aborts_on_mismatched_served_seal() {
        let executor = commonware_runtime::deterministic::Runner::default();
        executor.start(|context| async move {
            let signer = ed25519::PrivateKey::from_seed(79);
            let mut expected = fresh_directory_host();
            let mut served =
                served_directory_frame(&mut expected, &signer, 1, 0, dir_set("a", "1")).await;
            served.app_hash = test_root(0xA5);

            let mut host = fresh_directory_host();
            let mut recovery = Recovery::open(context.child("post_catchup_mismatch"))
                .await
                .expect("open recovery");
            let err =
                apply_post_reboot_catchup_frames(&mut recovery, &mut host, 0, 1, vec![served], None)
                    .await
                    .expect_err("seal mismatch must abort");

            assert!(
                err.contains("served seal"),
                "unexpected mismatch error: {err}"
            );
        });
    }

    #[test]
    fn post_reboot_catchup_is_noop_when_there_is_no_gap() {
        let executor = commonware_runtime::deterministic::Runner::default();
        executor.start(|context| async move {
            let mut host = fresh_directory_host();
            let before = host.app_hash();
            let mut recovery = Recovery::open(context.child("post_catchup_noop"))
                .await
                .expect("open recovery");
            let applied =
                apply_post_reboot_catchup_frames(&mut recovery, &mut host, 5, 5, Vec::new(), None)
                    .await
                    .expect("noop catch up");

            assert_eq!(applied.applied, 0);
            assert_eq!(host.app_hash(), before);
            assert!(
                recovery
                    .read_finalized_frames(5, 5)
                    .await
                    .expect("empty range")
                    .is_empty()
            );
        });
    }

    fn status(
        pending: Option<(&str, u64, u32)>,
        members: &[&[u8]],
        ready: &[&[u8]],
    ) -> UpgradeStatus {
        let members: Vec<Vec<u8>> = members.iter().map(|m| m.to_vec()).collect();
        let ready: Vec<Vec<u8>> = ready.iter().map(|m| m.to_vec()).collect();
        UpgradeStatus {
            current_version: 0,
            pending: pending.map(|(name, activation_height, to_version)| Upgrade {
                name: name.into(),
                activation_height,
                to_version,
            }),
            member_count: members.len() as u64,
            ready_count: ready.len() as u64,
            armed: false,
            members,
            ready,
        }
    }

    // ReadinessSignaller emits EXACTLY ONCE for a pending upgrade this binary can
    // execute when self is a current boundary member, and stays silent thereafter
    // (the latch), matching the module's own idempotence.
    #[test]
    fn readiness_signaller_emits_exactly_once_when_member_and_supported() {
        let me = vec![7u8; 32];
        let mut s = ReadinessSignaller::new(MAX_PROTOCOL_VERSION, me.clone());
        // to_version == MAX (<= MAX): supported, self a member, not yet in ready.
        let st = status(Some(("forge-v2", 100, MAX_PROTOCOL_VERSION)), &[&me], &[]);
        assert_eq!(
            s.decide(&st),
            Some(("forge-v2".to_string(), MAX_PROTOCOL_VERSION)),
            "the first tick signals"
        );
        // a second identical tick is a no-op (in-flight latch) — never spam.
        assert_eq!(s.decide(&st), None, "the latch suppresses re-emission");
        // once the module records our signal, still silent even if the latch reset.
        s.signaled = None;
        let st_ready = status(Some(("forge-v2", 100, MAX_PROTOCOL_VERSION)), &[&me], &[&me]);
        assert_eq!(s.decide(&st_ready), None, "module already holds our signal");
    }

    #[test]
    fn readiness_signaller_silent_when_under_versioned() {
        let me = vec![7u8; 32];
        let mut s = ReadinessSignaller::new(MAX_PROTOCOL_VERSION, me.clone());
        // to_version beyond what this binary can execute: never lie about readiness.
        let st = status(Some(("forge-v3", 100, MAX_PROTOCOL_VERSION + 1)), &[&me], &[]);
        assert_eq!(s.decide(&st), None);
    }

    #[test]
    fn readiness_signaller_silent_when_not_a_member() {
        let me = vec![7u8; 32];
        let other = vec![9u8; 32];
        let mut s = ReadinessSignaller::new(MAX_PROTOCOL_VERSION, me);
        // self is not in the boundary member set (not in the R = n denominator).
        let st = status(Some(("forge-v2", 100, MAX_PROTOCOL_VERSION)), &[&other], &[]);
        assert_eq!(s.decide(&st), None);
    }

    #[test]
    fn readiness_signaller_silent_when_no_pending() {
        let me = vec![7u8; 32];
        let mut s = ReadinessSignaller::new(MAX_PROTOCOL_VERSION, me.clone());
        assert_eq!(s.decide(&status(None, &[&me], &[])), None);
    }

    // the boot preflight gate: a boundary whose required_min_version exceeds this
    // binary's MAX_PROTOCOL_VERSION is refused (both recovery-resume and
    // state-sync-join call `Manifest::preflight`, which delegates here); an equal
    // or lower requirement boots. this is the fail-loud-early contract Task 7.3
    // wires onto both boot paths.
    #[test]
    fn boot_preflight_refuses_under_versioned_binary() {
        // a boundary needing one version beyond this build must be refused.
        assert!(sdk::check_required_version(MAX_PROTOCOL_VERSION + 1, MAX_PROTOCOL_VERSION).is_err());
        // exactly at the build ceiling, and below it, boots.
        assert!(sdk::check_required_version(MAX_PROTOCOL_VERSION, MAX_PROTOCOL_VERSION).is_ok());
        if MAX_PROTOCOL_VERSION > 0 {
            assert!(
                sdk::check_required_version(MAX_PROTOCOL_VERSION - 1, MAX_PROTOCOL_VERSION).is_ok()
            );
        }
    }
}
