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
use runs::RunsModule;
use commonware_codec::{DecodeExt as _, Encode as _};
use commonware_consensus::simplex::scheme::ed25519 as simplex_ed25519;
use commonware_consensus::types::Epoch;
use commonware_cryptography::{Signer, ed25519};
use commonware_p2p::authenticated::discovery::{self, Network};
use commonware_p2p::{Ingress, Manager, Receiver as P2pReceiver, Recipients, Sender as P2pSender};
use commonware_runtime::{Clock, IoBuf, Metrics, Quota, Runner, Spawner, Supervisor};
use commonware_utils::{NZU32, ordered::Set};
use dispatch::DispatchModule;
use tagging::TaggingModule;
use futures::{FutureExt as _, StreamExt as _};
use tracing_subscriber::prelude::*;

use consensus::{ConsensusScheme, ContentStore, Digest, SimplexOrderer, digest_of};

mod config;
mod first_contact_join;
mod lobby;
mod oracle_pool;
mod relay;
mod relay_runtime;
mod replica;
mod resource_limits;
mod resident_announce;
mod resident_dispatch;
mod statesync_plane;
mod userkey;
mod voice;
mod voice_plane;
use config::{Resolved, WireGuardEffectKind, hex_bytes, unhex};

fn run_output_sink(registry: noded::RunOutputRegistry) -> capability_host::OutputSink {
    std::sync::Arc::new(move |ctx, line| {
        let Some(run_key) = ctx.run_key.as_deref() else {
            return;
        };
        let stream = match line.stream {
            capability_host::OutputStream::Stdout => noded::RunStream::Stdout,
            capability_host::OutputStream::Stderr => noded::RunStream::Stderr,
        };
        registry.append(run_key, stream, line.line);
    })
}

/// the consensus signature scheme this build runs — a genesis-wide constant. today only
/// V1 (ed25519); see [`ConsensusScheme`]'s rekey/respawn contract for the BLS/V2 path.
const CONSENSUS_SCHEME: ConsensusScheme = ConsensusScheme::V1Ed25519;
/// the highest protocol version THIS binary's dual-path modules can execute — a
/// per-node BUILD constant, NEVER consensus state (a lying value can only
/// refuse-to-boot or halt this one node, never fork the network). the
/// `ReadinessSignaller` truthfully signals readiness for a pending upgrade iff
/// `MAX_PROTOCOL_VERSION >= to_version`, and the boot preflight refuses a boundary
/// whose `required_min_version` exceeds it. Phase 9 raised this to 2 when the
/// forge v2 dual path landed; the staged-admission resident tier raised it to
/// 3 — this binary can execute a scheduled `to_version=3` (valset/governance
/// resident ops, gated below 3) and truthfully `SignalReady`.
const MAX_PROTOCOL_VERSION: u32 = 3;
use automations::Automations;
use capability::CapabilityRegistry;
use chat::Chat;
use directory::Directory;
use directory::{DirMsg, DirQuery, DirReply, decode_reply, encode_msg, encode_query};
use duckfs_disk::SyncScratch;
use duckdns::DuckDns;
use files::Files;
use forge::Forge;
use governance::Governance;
use host::Host;
use identity::Identity;
use inbox::Inbox;
use jobs::Jobs;
use kv::Kv;
use node::OrderedNode;
use pages::Pages;
use profiles::Profiles;
use recovery::{Manifest, Recovery};
use saga::{LeasePolicy, SagaModule};
use sdk::{ModuleId, Msg, StateRoot};
use statesync::p2p::P2pSyncClient;
use statesync::qmdb::RemoteQmdbResolver;
use statesync::{SyncServer, fetch_frames, fetch_manifest, fetch_snapshot, fetch_tip_coords};
use tasks::Tasks;
use upgrade::Upgrade;
use valset::Valset;
use vaults::Vaults;

/// the peer-set index a node WITHOUT consensus coordinates tracks (a parked
/// joiner, a sync-only resident): the genesis mesh at index 0. a VALIDATOR
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
// block cadence is a single knob: `consensus::BLOCK_TIME` (1s). the idle
// heartbeat beats one nop block per BLOCK_TIME so an idle chain still finalizes
// (its height keeps ticking) and any pending cutover still crosses — paced to
// the same interval the leader's idle-propose holds a view open, so the beat
// never outpaces the intended block time.
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
/// how many times the promoted-validator boot retries an unavailable catch-up
/// source before failing over to the supervisor. sized (with the escalating
/// beat at the retry site) to ride out an overlay-only source whose tunnels
/// are still assembling after the reboot — several minutes, not seconds: an
/// exec-restart would only redo the plane restore from zero.
const POST_REBOOT_CATCHUP_MAX_ATTEMPTS: usize = 60;
/// max wire message size we accept on a channel (2 MiB). the tallest honest
/// messages are (a) an op frame carrying a full 1 MiB duckfs chunk — capped at
/// `node::MAX_FRAME_BYTES` by the submit-boundary guard, then gossiped raw on
/// the payload channel and served (plus a small rpc envelope) on the fetch and
/// statesync lanes — and (b) a `GetObjects` sync reply page, capped at
/// `duckfs_core::MAX_SYNC_REPLY_BYTES` (base64 wraps each 1 MiB object ~4/3x). the
/// asserts below pin both caps under this one, envelope headroom included:
/// commonware's sender ASSERTS on this cap, so "over" is a panic, not an error.
const MAX_MESSAGE_SIZE: u32 = 1 << 21;
const _: () = assert!(MAX_MESSAGE_SIZE as usize >= node::MAX_FRAME_BYTES + 1024);
const _: () = assert!(MAX_MESSAGE_SIZE as usize >= duckfs_core::MAX_SYNC_REPLY_BYTES + 1024);
/// inbound backlog before a channel applies receive backpressure.
const MAX_BACKLOG: usize = 128;
/// pump drain cadence: how often the pump applies finalized frames (and runs
/// everything that rides the drain arm — checkpoints, valset orchestration,
/// the epoch cutover, the heartbeat). enforced as a FLOOR via an absolute
/// deadline in the pump loop: ingress load can delay one drain by one
/// request's service time, but can never starve the arm.
const DRAIN_TICK: Duration = Duration::from_millis(100);
/// the submit-relay channel: a resident-standing node ships a frame it
/// SIGNED (its own identity key is the frame origin — authorship) to one
/// current validator, which takes consensus custody (`submit_frame`) and
/// answers with the frame's fate when it drains. the last free static slot
/// below CHANNEL_STATE_SYNC; engine banks start at 9 (statics run 3–8).
/// registered in EVERY
/// mode like the lanes above — validators serve, residents speak, sync-only
/// black-holes.
const CHANNEL_SUBMIT_RELAY: u64 = 3;
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
/// the voice channel: huddle audio datagrams between members (`chat::voice`
/// media frames inside data-plane datagrams — the `voice` module's mesh
/// transport arm). registered in EVERY mode like the lanes above; only the
/// validator path runs the hub that consumes it, every other mode black-holes.
const CHANNEL_VOICE: u64 = 7;
/// the video channel: camera-frame fragments between huddle members
/// (`chat::video` fragments inside data-plane datagrams). its own lane so
/// keyframe bursts never queue ahead of voice, with its own per-peer quota
/// sized for the top of the rate ladder plus keyframe bursts.
const CHANNEL_VIDEO: u64 = 8;
/// while parked and un-admitted, re-announce every N park-loop attempts
/// (attempts tick ~2s apart, so this is roughly every 10s) — often enough to
/// survive member restarts (the request queue is in-memory), quiet enough to
/// stay out of the members' way.
const LOBBY_ANNOUNCE_EVERY: usize = 5;
/// the park loop's poll cadence while the joiner still knocks for standing or
/// has no served boundary yet: fast, because this tick is all that paces the
/// first sync and the `LOBBY_ANNOUNCE_EVERY` knock counter.
const JOINER_POLL: Duration = Duration::from_secs(2);
/// a standing, SERVING resident's fallback poll. head-following is wake-driven
/// (cert-lane traffic nudges the park loop the moment a boundary seals), so
/// this tick only covers a missed wake — a mesh hiccup swallowing a
/// certificate burst, or an idle stretch with nothing to follow.
const RESIDENT_FALLBACK_POLL: Duration = Duration::from_secs(12);
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
const MODULE_IDS: [&str; 23] = [
    "kv",
    "pages",
    "chat",
    "forge",
    "valset",
    "governance",
    "upgrade",
    "saga",
    "capability",
    "dispatch",
    "tagging",
    "tasks",
    "vaults",
    "profiles",
    "identity",
    "duckdns",
    "inbox",
    "directory",
    "automations",
    "files",
    "jobs",
    "agent",
    "runs",
];
/// how long an app-surface submit reply may be held awaiting finalization
/// before it errors out (the op may still land later; clients re-query on
/// block events). mirrors the rpc bridge's stuck-node budget.
const SUBMIT_HOLD: Duration = Duration::from_secs(10);

/// the five channels epoch `e`'s engine uses: vote, certificate, resolver, the
/// eager payload-relay lane, and the payload FETCH lane (the lazy catch-up
/// backstop — a validator that missed the one-shot relay gossip for a
/// finalized op fetches its bytes by digest instead of wedging its apply
/// prefix forever). starts at 9, clear of the fixed discovery channels
/// (statesync 4, lobby 5, reachability 6, voice 7, video 8).
fn engine_channels(epoch: u64) -> (u64, u64, u64, u64, u64) {
    let base = 9 + epoch * 5;
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

fn resident_bytes(
    orchestrator: &consensus::ValsetOrchestrator<ed25519::PublicKey>,
) -> Vec<Vec<u8>> {
    orchestrator
        .current_residents()
        .iter()
        .map(|k| k.as_ref().to_vec())
        .collect()
}

/// read the valset module's current membership projection (committed state —
/// called between drains, outside any block).
async fn read_valset_members(host: &Host) -> Vec<Vec<u8>> {
    use valset::{ValsetQuery, ValsetReply, decode_reply, encode_query};
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

/// read the valset module's current RESIDENT projection (committed state —
/// called between drains, outside any block; same read point as
/// [`read_valset_members`], so a boundary read sees one frozen state).
async fn read_valset_residents(host: &Host) -> Vec<Vec<u8>> {
    use valset::{ValsetQuery, ValsetReply, decode_reply, encode_query};
    let Ok(reply) = host
        .query("valset", &encode_query(&ValsetQuery::Residents))
        .await
    else {
        return Vec::new();
    };
    match decode_reply(&reply) {
        Ok(ValsetReply::Residents(v)) => v,
        Ok(_) | Err(_) => Vec::new(),
    }
}

/// the transport-mesh set a parked joiner tracks at a manifest's epoch. it MUST
/// be the same set every member tracks at that epoch — a validator tracks
/// `descriptor_mesh ∪ members ∪ residents` (see the `mesh_at` closure in the
/// validator boot below) — because `authenticated::discovery` KILLS a peer
/// whose bit-vector length disagrees at a shared index. the manifest carries
/// members (`participants`) and residents (`residents`) as separate lists, so a
/// joiner that folds only `participants` tracks a SHORTER set than every member
/// the moment any resident is granted, and discovery tears the link down on
/// every gossip round (a resident redeeming its own grant is exactly this case:
/// the founder counts it, the joiner does not). the descriptor mesh already
/// carries the lobby key. undecodable keys are dropped (dead serving hints).
fn joiner_epoch_mesh(
    descriptor_mesh: &[ed25519::PublicKey],
    participants: &[Vec<u8>],
    residents: &[Vec<u8>],
) -> Set<ed25519::PublicKey> {
    let mut union: std::collections::BTreeSet<ed25519::PublicKey> =
        descriptor_mesh.iter().cloned().collect();
    // fold BOTH lists: members AND residents. every validator tracks the epoch
    // as `descriptor_mesh ∪ members ∪ residents`; omitting residents here would
    // leave the joiner one short and get it killed on every discovery round.
    for k in participants.iter().chain(residents.iter()) {
        if let Ok(pk) = ed25519::PublicKey::decode(k.as_slice()) {
            union.insert(pk);
        }
    }
    Set::try_from(union.into_iter().collect::<Vec<_>>())
        .expect("a btree-set union has no duplicates")
}

/// read the upgrade module's committed state as the boundary snapshot the
/// orchestrator reads at a finalized boundary (committed state — called between
/// drains, outside any block). the readiness keys are projected into decoded
/// ed25519 pubkeys (an undecodable key is dropped — dead weight, exactly like the
/// module). falls back to the baseline (no pending) when the module is absent
/// (pre-retrofit) or the reply is unreadable, so this never forks on a decode slip
/// — matching `Host::effective_version`'s graceful fallback.
async fn read_upgrade_state(host: &Host) -> consensus::BoundaryUpgrade<ed25519::PublicKey> {
    use upgrade::{UpgradeQuery, UpgradeReply, decode_reply, encode_query};
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
    use upgrade::{UpgradeQuery, UpgradeReply, decode_reply, encode_query};
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

/// the CURRENT member set from the valset module's committed+staged projection
/// (host-routed read, between drains). an unreadable reply degrades to empty —
/// callers treat that as "can't authorize anything right now", never a panic.
async fn read_members_from_host(host: &Host) -> Vec<Vec<u8>> {
    use valset::{ValsetQuery, ValsetReply, decode_reply, encode_query};
    let Ok(raw) = host
        .query("valset", &encode_query(&ValsetQuery::Validators))
        .await
    else {
        return Vec::new();
    };
    match decode_reply(&raw) {
        Ok(ValsetReply::Validators(v)) => v,
        Ok(_) | Err(_) => Vec::new(),
    }
}

/// read the upgrade module's raw committed [`UpgradeStatus`] (committed state,
/// between drains). `None` when the module is absent (pre-retrofit) or the reply
/// is unreadable — so the transition-marker latches degrade to silent on a
/// baseline net, never panicking.
async fn read_upgrade_status_raw(host: &Host) -> Option<upgrade::UpgradeStatus> {
    use upgrade::{UpgradeQuery, UpgradeReply, decode_reply, encode_query};
    let reply = host
        .query("upgrade", &encode_query(&UpgradeQuery::Status))
        .await
        .ok()?;
    let UpgradeReply::Status(status) = decode_reply(&reply).ok()?;
    Some(status)
}

/// read the governance module's committed invite redemptions — the
/// exactly-once nonce set (committed+staged projection, between drains). an
/// unreadable reply degrades to empty: the lobby then simply cannot pre-empt
/// a spent invite, and the in-consensus exactly-once check still holds.
async fn read_redemptions_from_host(host: &Host) -> Vec<governance::RedemptionView> {
    use governance::{GovQuery, GovReply, decode_reply, encode_query};
    let Ok(reply) = host
        .query("governance", &encode_query(&GovQuery::Redemptions))
        .await
    else {
        return Vec::new();
    };
    match decode_reply(&reply) {
        Ok(GovReply::Redemptions(v)) => v,
        Ok(_) | Err(_) => Vec::new(),
    }
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

/// hex-encode a state root for a stable, greppable log line.
fn hex(root: &StateRoot) -> String {
    hex_bytes(&root.0)
}

/// Consensus-visible network names that must be identical across every host
/// construction path. Keeping them together makes it harder for genesis,
/// restore, and state sync to drift as another chain-scoped module is added.
#[derive(Clone, Copy)]
struct NetworkBindings<'a> {
    invite: &'a [u8],
    identity_chain_id: &'a str,
    duckdns_chain_id: &'a str,
}

/// Node-local substrates needed while reconstructing a host from state sync.
/// Consensus-visible names stay in [`NetworkBindings`]; paths and blob handles
/// stay here so callers cannot accidentally blur the two kinds of input.
struct SyncSubstrates<'a> {
    forge_repo: &'a std::path::Path,
    duckfs_dir: &'a std::path::Path,
    blobs: blobstore::BlobHandle,
}

/// the PRODUCTION module set — genesis state, identical on every node (a
/// different set composes a different app-hash and the network forks at
/// genesis). system infrastructure (kv, valset seeded with the genesis
/// validators, saga) plus every product module. `forge_repo` is this node's
/// on-disk git substrate; wrapper modules run EMBEDDED substrates for now.
async fn genesis_host(
    context: &commonware_runtime::tokio::Context,
    forge_repo: &std::path::Path,
    duckfs_dir: &std::path::Path,
    genesis_validators: &[ed25519::PublicKey],
    bindings: NetworkBindings<'_>,
    blobs: blobstore::BlobHandle,
) -> Host {
    let kv = Kv::init(context.child("kv"), "kv").await;
    let pages = Pages::init(context.child("pages"), "pages").await;
    let chat = Chat::init(context.child("chat"), "chat")
        .await
        .with_tagging("tagging");
    // forge shares the blob plane so a Push's packfile (staged on the blob
    // lane before submit) can materialize locally; the pack never touches root.
    let forge = Forge::with_blobs("forge", forge_repo.to_path_buf(), blobs)
        .expect("forge init")
        .with_chat("chat");
    let mut valset = Valset::new("valset");
    // genesis-seed the validator set from config — deterministic and identical
    // on every node, so membership is IN consensus state from block zero (the
    // substrate epoch cutover + governance will drive).
    for v in genesis_validators {
        valset.insert(v.as_ref().to_vec());
    }
    Host::genesis(vec![
        Box::new(kv),
        Box::new(pages),
        Box::new(chat),
        Box::new(forge),
        Box::new(valset),
        // governance is the SOLE authorized author of valset changes: member
        // proposals + ballots, deterministic tally, follow-up membership ops.
        Box::new(
            Governance::new("governance", "valset", "upgrade")
                .with_invite_binding(bindings.invite),
        ),
        // the no-downtime upgrade coordinator: holds the at-most-one pending
        // upgrade + per-validator readiness set (valset-gated). its mere
        // presence in the registry is its genesis app-hash contribution.
        Box::new(Upgrade::new("upgrade", "valset")),
        // capability-aware strict leases: a saga whose trigger names a
        // capability is assigned over that tag's announced providers, and
        // only the assignee's result lands. an UNASSIGNED attempt (empty
        // provider pool) accepts no result at all: its WorkerRequest is an
        // announcement a capable node must first claim via `SagaMsg::Accept`.
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
        // the task plane: recipe manifests + capability-routed dispatch with
        // next-block result delivery (the host's DeliverPending injection).
        Box::new(DispatchModule::new("dispatch", "saga")),
        // the engagement plane: content modules report tags, subscriber
        // modules receive engagement events — router only, module-agnostic.
        Box::new(TaggingModule::new("tagging")),
        Box::new(Tasks::new("tasks")),
        Box::new(Vaults::new("vaults")),
        // the origin-gated display-name registry: each verified submit origin
        // may set its own name, so the ui can resolve authors to names.
        Box::new(Profiles::new("profiles")),
        // the deterministic user->nodes binding registry: certificates are
        // chain-scoped (this network's chain id), member-gated binds via valset.
        Box::new(Identity::new(
            "identity",
            Some("valset".into()),
            bindings.identity_chain_id.to_string(),
        )),
        // SDK adapter over the pure DuckDNS registry. Names and provider ids
        // replicate; loopback publication targets remain node-local config.
        Box::new(
            DuckDns::new(
                "duckdns",
                "identity",
                Some("valset".into()),
                bindings.duckdns_chain_id,
            )
            .expect("descriptor chain id has a DuckDNS label"),
        ),
        // per-member notification queues; other modules deliver via follow-up
        // ops so a notification commits atomically with the causing event (P2).
        Box::new(Inbox::new("inbox")),
        Box::new(Files::open("files", duckfs_dir.to_path_buf()).expect("duckfs open")),
        Box::new(Jobs::new("jobs")),
        // the agent registry: a self-contained record book; its hook keeps
        // each agent's dispatch recipe in lockstep via the runs module.
        Box::new(AgentModule::new("agent", "saga", Some("runs".into()))),
        // the collaboration loop's actor: watches, engagement, composition,
        // dispatch, and response delivery — reads the registry by query.
        Box::new(
            RunsModule::new(
                "runs",
                "chat",
                "saga",
                "tagging",
                "dispatch",
                "agent",
                Some("tasks".into()),
                Some("jobs".into()),
            )
            // the duckfs/files module the portable (v3) composer pins its source
            // head from (W2). its presence is what selects the v3 composer;
            // unwired, the composer emits the v2 wire.
            .with_files_module("files"),
        ),
        Box::new(Directory::new("directory")),
        // user-defined rules over chat posts: trusts the "chat" origin for hook
        // events and emits chat/tasks follow-ups.
        Box::new(Automations::new("automations", "chat", "tasks", "inbox")),
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
    duckfs_dir: &std::path::Path,
    manifest: &Manifest,
    bindings: NetworkBindings<'_>,
    blobs: blobstore::BlobHandle,
) -> Result<Host, String> {
    let kv = Kv::init(context.child("kv"), "kv").await;
    let pages = Pages::init(context.child("pages"), "pages").await;
    let chat = Chat::init(context.child("chat"), "chat")
        .await
        .with_tagging("tagging");
    // forge shares the blob plane (see genesis_host) for Push materialization.
    let mut forge = Forge::with_blobs("forge", forge_repo.to_path_buf(), blobs)
        .map_err(|e| format!("forge: {e}"))?
        .with_chat("chat");
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

    let mut governance =
        Governance::new("governance", "valset", "upgrade")
            .with_invite_binding(bindings.invite);
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

    let mut dispatch = DispatchModule::new("dispatch", "saga");
    let (bytes, root) = snapshot_of("dispatch")?;
    dispatch
        .install(bytes, root)
        .map_err(|e| format!("dispatch install: {e}"))?;

    let mut tagging = TaggingModule::new("tagging");
    let (bytes, root) = snapshot_of("tagging")?;
    tagging
        .install(bytes, root)
        .map_err(|e| format!("tagging install: {e}"))?;

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

    let mut identity = Identity::new(
        "identity",
        Some("valset".into()),
        bindings.identity_chain_id.to_string(),
    );
    let (bytes, root) = snapshot_of("identity")?;
    identity
        .install(bytes, root)
        .map_err(|e| format!("identity install: {e}"))?;

    let mut duckdns = DuckDns::new(
        "duckdns",
        "identity",
        Some("valset".into()),
        bindings.duckdns_chain_id,
    )
    .map_err(|e| format!("duckdns init: {e}"))?;
    let (bytes, root) = snapshot_of("duckdns")?;
    duckdns
        .install(bytes, root)
        .map_err(|e| format!("duckdns install: {e}"))?;

    let mut inbox = Inbox::new("inbox");
    let (bytes, root) = snapshot_of("inbox")?;
    inbox
        .install(bytes, root)
        .map_err(|e| format!("inbox install: {e}"))?;

    // files is a duckfs-odb resolver module — NOT in the checkpoint's snapshot
    // set (like the qmdb modules above, which `init` from their own on-disk
    // stores). `Files::open` already recovers its committed refs, durable height,
    // and objects from the on-disk odb/refs envelope, and recovery replays
    // forward from that height — so a reboot needs no checkpoint bytes and no
    // object fetch here.
    let files =
        Files::open("files", duckfs_dir.to_path_buf()).map_err(|e| format!("duckfs open: {e}"))?;

    let mut jobs = Jobs::new("jobs");
    let (bytes, root) = snapshot_of("jobs")?;
    jobs.install(bytes, root)
        .map_err(|e| format!("jobs install: {e}"))?;

    let mut agent = AgentModule::new("agent", "saga", Some("runs".into()));
    let (bytes, root) = snapshot_of("agent")?;
    agent
        .install(bytes, root)
        .map_err(|e| format!("agent install: {e}"))?;

    let mut runs = RunsModule::new(
        "runs",
        "chat",
        "saga",
        "tagging",
        "dispatch",
        "agent",
        Some("tasks".into()),
        Some("jobs".into()),
    )
    .with_files_module("files");
    let (bytes, root) = snapshot_of("runs")?;
    runs.install(bytes, root)
        .map_err(|e| format!("runs install: {e}"))?;

    let mut directory = Directory::new("directory");
    let (bytes, root) = snapshot_of("directory")?;
    directory
        .install(bytes, root)
        .map_err(|e| format!("directory install: {e}"))?;

    let mut automations = Automations::new("automations", "chat", "tasks", "inbox");
    let (bytes, root) = snapshot_of("automations")?;
    automations
        .install(bytes, root)
        .map_err(|e| format!("automations install: {e}"))?;

    Host::genesis(vec![
        Box::new(kv),
        Box::new(pages),
        Box::new(chat),
        Box::new(forge),
        Box::new(valset),
        Box::new(governance),
        Box::new(upgrade),
        Box::new(saga),
        Box::new(capability),
        Box::new(dispatch),
        Box::new(tagging),
        Box::new(tasks),
        Box::new(vaults),
        Box::new(profiles),
        Box::new(identity),
        Box::new(duckdns),
        Box::new(inbox),
        Box::new(files),
        Box::new(jobs),
        Box::new(agent),
        Box::new(runs),
        Box::new(directory),
        Box::new(automations),
    ])
    .map_err(|e| format!("restore host: {e}"))
}

/// the object-store ([`statesync::ObjectFetch`]) adapter over the live `files`
/// module: the statesync possession driver owns the loop + the full-possession
/// gate, this owns the duckfs `serve_sync` wire (refs image + `GetObjects`).
///
/// SCRATCH NAMESPACE (#219): like the qmdb modules — whose `sync_from` lands
/// under an ATTEMPT-scoped runtime child (`{name}_scratch_a{n}`) — the module
/// this adapter wraps is opened over `duckfs_disk::SyncScratch`'s attempt-scoped
/// scratch dir, NEVER the canonical `duckfs_dir`. the canonical dir is written
/// only by the verified promotion after `sync_all_modules`' composite app-hash
/// gate, so a failed join leaves it byte-untouched.
struct FilesOdb<'a>(&'a mut Files);

impl statesync::ObjectFetch for FilesOdb<'_> {
    fn refs_request(&self) -> Vec<u8> {
        duckfs_core::encode_get_refs()
    }

    fn install_refs(&mut self, reply: &[u8], root: StateRoot, height: u64) -> Result<(), String> {
        let bytes = duckfs_core::decode_refs_reply(reply)?;
        // persist the refs envelope at the SYNCED boundary height so a restart
        // right after the join resumes replay from the boundary, not genesis.
        self.0
            .install(&bytes, root, height)
            .map_err(|e| e.to_string())
    }

    fn missing_request(&self, limit: usize) -> Result<Option<Vec<u8>>, String> {
        let ids = self.0.missing_objects(limit).map_err(|e| e.to_string())?;
        if ids.is_empty() {
            return Ok(None);
        }
        Ok(Some(duckfs_core::encode_get_objects(&ids)))
    }

    fn ingest(&mut self, reply: &[u8]) -> Result<usize, String> {
        let batch = duckfs_core::decode_objects_reply(reply)?;
        let landed = batch.len();
        self.0.ingest_objects(&batch).map_err(|e| e.to_string())?;
        Ok(landed)
    }

    fn possession_complete(&self) -> Result<bool, String> {
        self.0.possession_complete().map_err(|e| e.to_string())
    }
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
    bindings: NetworkBindings<'_>,
    substrates: SyncSubstrates<'_>,
    attempt: usize,
) -> Result<Host, String> {
    let SyncSubstrates {
        forge_repo,
        duckfs_dir,
        blobs,
    } = substrates;
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
    .await?
    .with_tagging("tagging");

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

    let (bytes, root) = snapshot_of("dispatch").await?;
    let mut dispatch = DispatchModule::new("dispatch", "saga");
    dispatch
        .install(&bytes, root)
        .map_err(|e| format!("dispatch install: {e}"))?;

    let (bytes, root) = snapshot_of("tagging").await?;
    let mut tagging = TaggingModule::new("tagging");
    tagging
        .install(&bytes, root)
        .map_err(|e| format!("tagging install: {e}"))?;

    let (bytes, root) = snapshot_of("governance").await?;
    let mut governance = Governance::new("governance", "valset", "upgrade")
        .with_invite_binding(bindings.invite);
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

    let (bytes, root) = snapshot_of("identity").await?;
    let mut identity = Identity::new(
        "identity",
        Some("valset".into()),
        bindings.identity_chain_id.to_string(),
    );
    identity
        .install(&bytes, root)
        .map_err(|e| format!("identity install: {e}"))?;

    let (bytes, root) = snapshot_of("duckdns").await?;
    let mut duckdns = DuckDns::new(
        "duckdns",
        "identity",
        Some("valset".into()),
        bindings.duckdns_chain_id,
    )
    .map_err(|e| format!("duckdns init: {e}"))?;
    duckdns
        .install(&bytes, root)
        .map_err(|e| format!("duckdns install: {e}"))?;

    let (bytes, root) = snapshot_of("inbox").await?;
    let mut inbox = Inbox::new("inbox");
    inbox
        .install(&bytes, root)
        .map_err(|e| format!("inbox install: {e}"))?;

    // files is a duckfs-odb resolver module: its refs image AND its
    // content-addressed objects both ride the Module/`serve_sync` lane. a fresh
    // joiner's odb is EMPTY, so install the boundary refs (root-verified) at the
    // sync-target height and then loop GetObjects to full object possession —
    // the snapshot lane would leave this node refs-only (every file listed, not
    // one byte readable). the sync lands in an ATTEMPT-scoped scratch dir
    // (`duckfs_scratch_a{attempt}`, mirroring the qmdb scratch namespaces);
    // the canonical `duckfs_dir` is written only by the verified promotion
    // after the composite app-hash gate below (#219).
    let files_scratch = SyncScratch::prepare(duckfs_dir, attempt)
        .map_err(|e| format!("duckfs scratch: {e}"))?;
    let mut files = Files::open("files", files_scratch.dir().to_path_buf())
        .map_err(|e| format!("duckfs open: {e}"))?;
    let files_root = entry_root("files")?;
    let files_lane = statesync::ClientModuleLane::new(client.clone(), manifest.boundary_id());
    statesync::sync_object_possession(
        &files_lane,
        "files",
        files_root,
        manifest.height,
        &mut FilesOdb(&mut files),
        duckfs_core::MAX_SYNC_IDS,
    )
    .await
    .map_err(|e| format!("files sync: {e}"))?;

    let (bytes, root) = snapshot_of("jobs").await?;
    let mut jobs = Jobs::new("jobs");
    jobs.install(&bytes, root)
        .map_err(|e| format!("jobs install: {e}"))?;

    let (bytes, root) = snapshot_of("agent").await?;
    let mut agent = AgentModule::new("agent", "saga", Some("runs".into()));
    agent
        .install(&bytes, root)
        .map_err(|e| format!("agent install: {e}"))?;

    let (bytes, root) = snapshot_of("runs").await?;
    let mut runs = RunsModule::new(
        "runs",
        "chat",
        "saga",
        "tagging",
        "dispatch",
        "agent",
        Some("tasks".into()),
        Some("jobs".into()),
    )
    .with_files_module("files");
    runs.install(&bytes, root)
        .map_err(|e| format!("runs install: {e}"))?;

    let (bytes, root) = snapshot_of("automations").await?;
    let mut automations = Automations::new("automations", "chat", "tasks", "inbox");
    automations
        .install(&bytes, root)
        .map_err(|e| format!("automations install: {e}"))?;

    let (bytes, root) = snapshot_of("forge").await?;
    let mut forge = Forge::with_blobs("forge", forge_repo.to_path_buf(), blobs)
        .map_err(|e| format!("forge init: {e}"))?
        .with_chat("chat");
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
        Box::new(pages),
        Box::new(chat),
        Box::new(forge),
        Box::new(valset),
        Box::new(governance),
        Box::new(upgrade),
        Box::new(saga),
        Box::new(capability),
        Box::new(dispatch),
        Box::new(tagging),
        Box::new(tasks),
        Box::new(vaults),
        Box::new(profiles),
        Box::new(identity),
        Box::new(duckdns),
        Box::new(inbox),
        Box::new(files),
        Box::new(jobs),
        Box::new(agent),
        Box::new(runs),
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
    // the composite gate passed — promote files' scratch into the canonical
    // `duckfs_dir` (verify-then-replace refs + content-addressed object merge,
    // gated on the exact files root this composition certified) and swap the
    // registry onto a canonical-backed module. the returned host must run in
    // place over the canonical dir: the post-reboot full-sync fallback keeps
    // it live without a reboot, and a joiner's promotion reboot re-opens the
    // same dir. on any error the host is discarded and the retry re-syncs —
    // an already-promoted canonical dir is verified state, never damage.
    files_scratch
        .promote(files_root.0)
        .map_err(|e| format!("duckfs promote: {e}"))?;
    host.register(Box::new(
        Files::open("files", duckfs_dir.to_path_buf())
            .map_err(|e| format!("duckfs reopen: {e}"))?,
    ));
    // re-realize the boundary version over the swapped registry (idempotent),
    // then re-check THE property against the canonical-backed composition.
    host.set_active_version(manifest.current_version);
    if host.app_hash() != manifest.app_hash {
        return Err(format!(
            "canonical duckfs reopen composed {} != manifest {}",
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
    // a VERIFIER-only scheme: no signing key, no our-key-is-a-participant
    // requirement — any node (a not-yet-seated joiner included) can check a
    // served floor. and the check is now CRYPTOGRAPHIC (the quorum's
    // signatures), not the former structural decode: a server cannot mint a
    // floor its quorum never signed.
    let finalization = match CONSENSUS_SCHEME {
        ConsensusScheme::V1Ed25519 => {
            let scheme = simplex_ed25519::Scheme::verifier(namespace, participants);
            consensus::verify_finalization(&mut rand::rngs::OsRng, &scheme, &cert)
        }
        ConsensusScheme::V2Bls => {
            unimplemented!(
                "V2Bls joiner wiring lands with valset bls key registration — the manifest \
                 carries ed25519 transport identities only, and a bls verifier needs the \
                 committed (ed25519 -> bls) participant map"
            )
        }
    }
    .map_err(|e| {
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

/// reopen the recovery journal after a replica DESCEND (the node — which
/// owned the journal as its block sink — was dropped for an epoch cutover or
/// a promotion re-sync). a fresh metrics child label per reopen keeps the
/// runtime's registry collision-free. FATAL on failure: a node that lost its
/// journal handle must not continue as if it had one.
async fn reopen_recovery(
    context: &commonware_runtime::tokio::Context,
    reopens: &mut u32,
    label: &str,
) -> Recovery<commonware_runtime::tokio::Context> {
    *reopens += 1;
    let child: &'static str = Box::leak(format!("recovery_reopen_{reopens}").into_boxed_str());
    match Recovery::open(context.child(child)).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[node {label}] FATAL: cannot reopen the recovery store: {e}");
            std::process::exit(1);
        }
    }
}

/// a backfilled height's served seal, held for the post-fold cross-check:
/// `(disposition, app_hash, per-module roots)` as the quorum sealed them.
type ServedSeal = (node::Disposition, StateRoot, Vec<(sdk::ModuleId, StateRoot)>);

/// fold the committed views in `(after_view, up_to_view]` that never reached
/// this replica as certificates — lost gossip, or ancestors committed by
/// descent without their own finalization (the parent-linkage gap the fold
/// planner detected). the frames come from the statesync Frames lane (the
/// validators' journal: the authoritative FOLDED sequence) and enter through
/// the follower gate content-addressed; the served seals are stashed for the
/// post-fold cross-check — a mismatch there is divergence and fatal.
async fn replica_backfill<C>(
    client: &C,
    node_r: &mut node::OrderedNode<
        consensus::FollowerOrderer,
        Recovery<commonware_runtime::tokio::Context>,
    >,
    view_base: u64,
    views: (u64, u64),
    watermark: &mut Option<u64>,
    seal_checks: &mut std::collections::HashMap<u64, ServedSeal>,
    label: &str,
) -> Result<(), String>
where
    C: statesync::SyncClient,
{
    let (after_view, up_to_view) = views;
    let frames = fetch_frames(client, view_base + after_view, view_base + up_to_view)
        .await
        .map_err(|e| format!("{e}"))?;
    println!(
        "[node {label}] replica: backfilling {} committed frame(s) in views ({after_view}, \
         {up_to_view}]",
        frames.len()
    );
    for f in frames {
        let view = f.height.saturating_sub(view_base);
        seal_checks.insert(
            f.height,
            (to_node_disposition(f.disposition), f.app_hash, f.roots.clone()),
        );
        if node_r.orderer_mut().admit_backfilled(view, f.frame.clone()) {
            *watermark = Some(view);
        }
    }
    Ok(())
}

/// the verifier-only scheme for a boundary's epoch: what the replica fold
/// driver checks every observed finalization certificate against. mirrors
/// [`verify_manifest_floor`]'s construction (and shares its V2 gap: a bls
/// verifier needs the committed ed25519 -> bls participant map valset does
/// not carry yet). FATAL on undecodable participants — the boundary already
/// passed the floor verify, so garbage here is our own bug, not the wire's.
fn replica_verifier(namespace: &[u8], participant_keys: &[Vec<u8>]) -> simplex_ed25519::Scheme {
    let mut keys = Vec::with_capacity(participant_keys.len());
    for k in participant_keys {
        let pk = ed25519::PublicKey::decode(k.as_slice())
            .expect("participants already decoded for the floor verify");
        keys.push(pk);
    }
    let participants =
        Set::try_from(keys).expect("participant set already deduplicated for the floor verify");
    match CONSENSUS_SCHEME {
        ConsensusScheme::V1Ed25519 => simplex_ed25519::Scheme::verifier(namespace, participants),
        ConsensusScheme::V2Bls => {
            unimplemented!("V2Bls replica wiring lands with valset bls key registration")
        }
    }
}

/// the replica's valset orchestrator at (epoch, base): the same
/// deterministic observe → ceiling → cutover state machine the validator
/// drain runs. the pending-cutover slot resumes empty — the manifest-epoch
/// descend stays as the safety net for a cutover armed before this handle
/// existed (a restart into a pending window).
fn replica_orchestrator_at(
    epoch: u64,
    view_base: u64,
    participants: &[Vec<u8>],
    residents: &[Vec<u8>],
) -> consensus::ValsetOrchestrator<ed25519::PublicKey> {
    let decode = |keys: &[Vec<u8>]| -> Vec<ed25519::PublicKey> {
        keys.iter()
            .filter_map(|k| ed25519::PublicKey::decode(k.as_slice()).ok())
            .collect()
    };
    consensus::ValsetOrchestrator::resume(
        CUTOVER_DELAY,
        decode(participants),
        decode(residents),
        epoch,
        view_base,
        None,
    )
}

/// capture and persist the checkpoint (+ floor cert) that makes a synced
/// boundary a valid recovery-boot base — the journal's genesis for an
/// identity that never framed ops on this network (`next_seq = 1`). used by
/// the replica's join-time journal init, and (until the promotion collapse
/// lands) by the promotion path's pre-reboot fabrication. FATALs on
/// persistence failure: a node that cannot journal its base must not proceed
/// as if it had.
async fn write_boundary_checkpoint<E>(
    recovery: &mut Recovery<E>,
    host: &Host,
    boundary: &statesync::Manifest,
    floor: &Option<recovery::FloorCert>,
    label: &str,
    diag_tag: &str,
) -> u64
where
    E: recovery::Context + commonware_runtime::BufferPooler + commonware_runtime::Supervisor,
{
    let pos = recovery.oplog_pos().await;
    let floor_height = floor
        .as_ref()
        .map(|floor| floor.height.to_string())
        .unwrap_or_else(|| "none".to_string());
    diag_log(format!(
        "DIAG {diag_tag} checkpoint_height={} checkpoint_hash={} \
         floor_height={} floor_present={}",
        boundary.height,
        hex(&host.app_hash()),
        floor_height,
        floor.is_some()
    ));
    // stamp the real committed version fields so the captured checkpoint
    // carries the same `required_min_version` a live checkpoint would; the
    // next boot then preflights against them like any restart.
    let (cv, pu) = read_upgrade_version_fields(host).await;
    let ckpt = match Manifest::capture(
        host,
        Some(boundary.height),
        boundary.epoch,
        boundary.view_base,
        boundary.participants.clone(),
        boundary.residents.clone(),
        None,
        cv,
        pu,
        pos,
        1,
    ) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("[node {label}] FATAL: {diag_tag} capture: {e}");
            std::process::exit(1);
        }
    };
    if let Err(e) = recovery.write_manifest(&ckpt).await {
        eprintln!("[node {label}] FATAL: {diag_tag} write: {e}");
        std::process::exit(1);
    }
    if let Some(fc) = floor
        && let Err(e) = recovery.write_floor_cert(fc).await
    {
        eprintln!("[node {label}] FATAL: {diag_tag} floor-cert write: {e}");
        std::process::exit(1);
    }
    // this checkpoint IS the journal's new genesis: everything below its
    // oplog position must never roll into a boot at this base — a prior
    // life's replica-folded frames sit at earlier POSITIONS even when their
    // heights exceed the boundary, and recovery would roll a trailing one
    // forward past the checkpoint (observed: a promoted ex-replica booting
    // AHEAD of its source's serving window). the engine floor at `boundary`
    // suppresses replay at or below it, so no pruned frame is needed again.
    if let Err(e) = recovery.prune_oplog(pos).await {
        eprintln!("[node {label}] FATAL: {diag_tag} journal prune: {e}");
        std::process::exit(1);
    }
    // the checkpoint's oplog position — the caller's prune anchor when the
    // NEXT (periodic) checkpoint supersedes this one.
    pos
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
// the statesync serve seam: serving runs on its OWN task (captures, leases,
// chunk slicing, mesh/plane replies), so a joiner's sync never rides a drain
// beat of the consensus loop. only the four STATE TOUCHES below cross back to
// the loop — the one task that owns the host, the recovery journal, and the
// derived index — as bounded request/reply pairs, so a busy loop backpressures
// the serve lane instead of the reverse.
// ---------------------------------------------------------------------------

/// one state touch the statesync serve task asks of the consensus loop.
enum SyncStateRequest {
    /// capture (or re-coordinate) the current finalized boundary — the
    /// Manifest path. `known` names the boundaries the serve task already
    /// holds, so a known id round-trips fresh coordinates only, never
    /// payload bytes.
    Boundary {
        known: Vec<statesync::BoundaryId>,
        reply: tokio::sync::oneshot::Sender<Result<SyncBoundary, String>>,
    },
    /// route module-defined bytes to the live module's `serve_sync` (the
    /// resolver lanes: qmdb op ranges, duckfs refs/objects).
    ModuleServe {
        module_id: String,
        body: Vec<u8>,
        reply: tokio::sync::oneshot::Sender<Result<Vec<u8>, String>>,
    },
    /// read recovery-equivalent finalized frames in `(after, up_to]`.
    Frames {
        after_height: u64,
        up_to_height: u64,
        reply: tokio::sync::oneshot::Sender<Result<Vec<recovery::JournalFrame>, recovery::Error>>,
    },
    /// checkpoint the derived index databases for the shipped-index lane.
    IndexCut {
        reply: tokio::sync::oneshot::Sender<std::collections::BTreeMap<String, Vec<u8>>>,
    },
    /// read the tip's consensus coordinates — the DETECTION lane: answered
    /// straight from loop-owned state (no capture, no lease, no floor-cert
    /// alignment gate), so a resident fleet's routine polling never rides
    /// the Manifest path.
    TipCoords {
        reply: tokio::sync::oneshot::Sender<Result<statesync::TipCoords, String>>,
    },
}

/// the [`SyncStateRequest::Boundary`] answer: the served boundary's identity
/// and coordinates, with capture payload only when the serve task named the
/// id unknown.
struct SyncBoundary {
    id: statesync::BoundaryId,
    coords: statesync::BoundaryCoords,
    data: Option<statesync::CaptureData>,
}

/// drive one decoded statesync request against the serve-task-owned
/// [`SyncServer`], round-tripping the state touches to the consensus loop.
/// a closed loop (shutdown) answers as a plain serve error — clients retry
/// against the next source.
async fn drive_sync_request(
    server: &mut SyncServer,
    state_tx: &futures::channel::mpsc::Sender<SyncStateRequest>,
    req: statesync::SyncRequest,
) -> statesync::SyncResponse {
    const CLOSED: &str = "statesync state owner is shutting down";
    // a failed send drops the request (and its reply sender) on the floor, so
    // the paired `rx.await` below surfaces it as the CLOSED error — no
    // separate delivered/undelivered bookkeeping.
    let ask = |req: SyncStateRequest| {
        let mut tx = state_tx.clone();
        async move {
            let _ = futures::SinkExt::send(&mut tx, req).await;
        }
    };
    match server.serve(req) {
        statesync::ServeStep::Reply(resp) => resp,
        statesync::ServeStep::NeedBoundary => {
            let (tx, rx) = tokio::sync::oneshot::channel();
            ask(SyncStateRequest::Boundary {
                known: server.known_boundaries(),
                reply: tx,
            })
            .await;
            match rx.await {
                Ok(Ok(SyncBoundary { id, coords, data })) => {
                    match data {
                        Some(data) => server.install_capture(id, data),
                        None => server.refresh_coords(id, coords),
                    }
                    server
                        .manifest_for(id)
                        .unwrap_or_else(statesync::SyncResponse::Error)
                }
                Ok(Err(e)) => statesync::SyncResponse::Error(e),
                Err(_) => statesync::SyncResponse::Error(CLOSED.into()),
            }
        }
        statesync::ServeStep::NeedModuleServe { module_id, body } => {
            let (tx, rx) = tokio::sync::oneshot::channel();
            ask(SyncStateRequest::ModuleServe {
                module_id,
                body,
                reply: tx,
            })
            .await;
            match rx.await {
                Ok(Ok(bytes)) => statesync::SyncResponse::Module(bytes),
                Ok(Err(e)) => statesync::SyncResponse::Error(e),
                Err(_) => statesync::SyncResponse::Error(CLOSED.into()),
            }
        }
        statesync::ServeStep::NeedFrames {
            after_height,
            up_to_height,
        } => {
            let (tx, rx) = tokio::sync::oneshot::channel();
            ask(SyncStateRequest::Frames {
                after_height,
                up_to_height,
                reply: tx,
            })
            .await;
            match rx.await {
                Ok(Ok(frames)) => {
                    let mut out = Vec::new();
                    let mut err = None;
                    for frame in frames.into_iter().take(statesync::FRAME_BATCH_LEN) {
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
                Ok(Err(recovery::Error::RangePruned {
                    after_height,
                    retained_start,
                })) => statesync::SyncResponse::RangePruned {
                    requested_after: after_height,
                    retained_from: retained_start,
                },
                Ok(Err(e)) => {
                    statesync::SyncResponse::Error(format!("recovery frame range: {e}"))
                }
                Err(_) => statesync::SyncResponse::Error(CLOSED.into()),
            }
        }
        statesync::ServeStep::NeedIndexCut { boundary } => {
            // the shipped-index lane cuts lazily: the FIRST index request for
            // a boundary checkpoints the derived databases and attaches the
            // archives to that capture, so joiners that never opt in cost
            // nothing. the attach is unconditional, so the re-drive below
            // resolves — it cannot need a second cut.
            let (tx, rx) = tokio::sync::oneshot::channel();
            ask(SyncStateRequest::IndexCut { reply: tx }).await;
            let blobs = match rx.await {
                Ok(blobs) => blobs,
                Err(_) => return statesync::SyncResponse::Error(CLOSED.into()),
            };
            if let Err(e) = server.attach_index(boundary, blobs) {
                return statesync::SyncResponse::Error(e);
            }
            match server.serve(statesync::SyncRequest::IndexModules { boundary }) {
                statesync::ServeStep::Reply(resp) => resp,
                _ => statesync::SyncResponse::Error(
                    "index attach did not settle the request".into(),
                ),
            }
        }
        statesync::ServeStep::NeedCoords => {
            let (tx, rx) = tokio::sync::oneshot::channel();
            ask(SyncStateRequest::TipCoords { reply: tx }).await;
            match rx.await {
                Ok(Ok(coords)) => statesync::SyncResponse::TipCoords(coords),
                Ok(Err(e)) => statesync::SyncResponse::Error(e),
                Err(_) => statesync::SyncResponse::Error(CLOSED.into()),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// the derived-index boot fold. consensus never depends on it: fold errors
// poison the store and log, heal errors log — recovery and the drain proceed
// identically with or without the index.
// ---------------------------------------------------------------------------

/// build one explorer row ([`noded::BlockRecord`] json) from a block's
/// decoded parts — THE row construction seam, shared by the live drain and
/// the boot fold so both writers produce byte-identical rows. staging the
/// payload IS computing `op_hash` (put_chunk keys the blob by sha256), and
/// on the fold path the re-staging is load-bearing: the blob store is
/// in-memory, so the live drain's staging dies with the process and this is
/// what makes `GET /v1/files/blob/{op_hash}` answer again after a restart.
fn explorer_root_op(
    blobs: &blobstore::BlobHandle,
    origin: &sdk::Origin,
    target: &str,
    payload: &[u8],
    dispatches: &[host::DispatchRecord],
    disposition: noded::BlockDisposition,
) -> noded::RootOp {
    noded::RootOp {
        proposer: match origin {
            sdk::Origin::External(key) => noded::hex_bytes(key),
            // frames only carry verified External authorship; label the
            // impossible rest.
            sdk::Origin::Module(id) => format!("module:{id}"),
            sdk::Origin::System => "system".into(),
        },
        disposition,
        target: target.to_string(),
        operations: dispatches.iter().map(noded::DispatchInfo::from).collect(),
        payload: noded::payload_preview(payload),
        op_hash: noded::hex_bytes(&blobs.put_chunk(payload.to_vec())),
    }
}

/// rebuild the explorer row for a replayed sealed frame — the boot fold's
/// equivalent of the drain's row construction, fed from the journal instead
/// of the live decode. `None` mirrors the drain's gates exactly: an
/// undecodable frame never had a row (its drain `op` was `None`), the
/// heartbeat nop is the deliberately-empty block the explorer hides, and a
/// discarded frame is never journaled (the arm keeps this total anyway).
fn sealed_frame_block_row(
    blobs: &blobstore::BlobHandle,
    block: &recovery::FoldedBlock<'_>,
) -> Option<Vec<u8>> {
    // the sealed frame is a BATCH: decode its members and show each as a block
    // op. per-member dispositions/traces are not carried in the fold (recovery
    // folds the block-level disposition + aggregate trace), so a replayed op
    // shows the block disposition and an empty trace — the LIVE drain carries
    // the full per-op detail.
    let members = node::decode_batch(block.frame).ok()?;
    let disposition = match block.disposition {
        node::Disposition::Applied => noded::BlockDisposition::Applied,
        node::Disposition::Rejected => noded::BlockDisposition::Rejected,
        node::Disposition::Discarded => return None,
    };
    let mut ops = Vec::new();
    for member in &members {
        let Ok((origin, msg)) = node::decode_frame(member) else {
            continue;
        };
        if msg.target == NOP_TARGET {
            continue;
        }
        ops.push(explorer_root_op(
            blobs,
            &origin,
            &msg.target,
            &msg.payload,
            &[],
            disposition,
        ));
    }
    if ops.is_empty() {
        // a pure nop/idle block — the explorer hides it (same rule as live).
        return None;
    }
    Some(noded::block_row(&noded::BlockRecord {
        height: block.height,
        hash: noded::hex_bytes(&node::frame_id(block.frame)),
        commit_hash: hex(&block.app_hash),
        ops,
    }))
}

/// the resident's explorer row: a followed BOUNDARY, not a sealed frame. the
/// populated fields are verified truth — the boundary height and the
/// app-hash the manifest check passed — and every frame-derived field stays
/// honestly empty, because a resident never sees the frames between
/// boundaries (the same degradation rule that keeps the frameless daemon
/// lane's `hash` empty rather than fabricated).
fn boundary_block_row(height: u64, app_hash: &StateRoot) -> Vec<u8> {
    noded::block_row(&noded::BlockRecord {
        height,
        hash: String::new(),
        commit_hash: hex(app_hash),
        // a resident follows boundaries, not frames: no member ops to show.
        ops: Vec::new(),
    })
}

/// folds sealed blocks into the derived per-module index during boot (journal
/// replay + post-reboot frame catch-up), with the GAP DISCIPLINE: once one
/// sealed height's content is unreproducible (opaque) above some module's
/// watermark, folding stops for good. advancing watermarks past the hole
/// would hide it from the post-boot heal, which re-derives from verified
/// state exactly when a watermark trails the boot tip. a re-executed block
/// carries its sealed frame, so the fold also rebuilds the explorer row the
/// live drain wrote — the blocks database is the one derived tier a
/// from-state rebuild can NOT repair (rows are node-layer observations, not
/// canonical state), so the crash-window suffix must be re-derived here or
/// `GET /v1/blocks` loses those heights for good.
struct IndexFold<'a> {
    index: &'a indexer::IndexStore,
    blobs: blobstore::BlobHandle,
    stopped: bool,
}

impl<'a> IndexFold<'a> {
    fn new(index: &'a indexer::IndexStore, blobs: blobstore::BlobHandle) -> Self {
        Self {
            index,
            blobs,
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
    fn folded_block(&mut self, block: &recovery::FoldedBlock<'_>) {
        if self.stopped {
            return;
        }
        let height = block.height;
        let ops = indexer::BlockOps {
            record: sealed_frame_block_row(&self.blobs, block),
            // the validator's consensus time IS the height (see BlockContext).
            ..noded::index_block_ops(height, height, block.dispatches)
        };
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
            let files = statesync::decode_index_archive(&blob).map_err(|e| format!("{db}: {e}"))?;
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
        Ok(0) => println!("[node {label}] source ships no index — views heal from verified state"),
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
    let protocol_version = host.effective_version(served.height).await;
    host.set_active_version(protocol_version);
    // the served frame is a BATCH: decode its members and apply as ONE block,
    // exactly like the live drain and recovery replay, so the disposition,
    // roots, and app-hash reproduce what the peer served. disposition is
    // DRAIN-based (any member applied or a System injection ran), never
    // app-hash-based.
    let (outcome, dispatches) = match node::decode_batch(&served.frame) {
        Ok(members) => {
            let mut ops = Vec::new();
            for member in &members {
                if let Ok(pair) = node::decode_frame(member) {
                    ops.push(pair);
                }
            }
            let ctx = host::BlockContext {
                protocol_version,
                height: served.height,
                consensus_time: served.height,
                origin: sdk::Origin::System,
            };
            match host.submit_block(ctx, ops).await {
                Ok(batch) => {
                    let mut dispatches = Vec::new();
                    let mut any_applied = false;
                    for member in batch.members {
                        if let host::MemberOutcome::Applied { dispatches: d } = member {
                            any_applied = true;
                            dispatches.extend(d);
                        }
                    }
                    let has_system = !batch.system_dispatches.is_empty();
                    dispatches.extend(batch.system_dispatches);
                    let outcome = if any_applied || has_system {
                        node::Disposition::Applied
                    } else {
                        node::Disposition::Rejected
                    };
                    (outcome, dispatches)
                }
                Err(host::SubmitError::Rejected(_)) => (node::Disposition::Rejected, Vec::new()),
                Err(host::SubmitError::Fatal(f)) => {
                    return Err(format!("fatal host error applying suffix frame: {f}"));
                }
            }
        }
        Err(_) => (node::Disposition::Rejected, Vec::new()),
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
        fold.folded_block(&recovery::FoldedBlock {
            height: frame.height,
            frame: &frame.frame,
            disposition: seal.disposition,
            app_hash: seal.app_hash,
            dispatches: &dispatches,
        });
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
        target.residents.clone(),
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

/// the recovered epoch's RESIDENT keys — empty on a fresh boot (genesis has
/// no residents) and on checkpoints written before the staged-admission tier.
fn resume_resident_keys(
    resumed: Option<&recovery::Recovered>,
) -> Result<Vec<ed25519::PublicKey>, String> {
    let raw: Vec<Vec<u8>> = match resumed {
        Some(rec) => rec.residents.clone(),
        None => Vec::new(),
    };
    let mut keys = Vec::with_capacity(raw.len());
    for k in &raw {
        keys.push(
            ed25519::PublicKey::decode(k.as_slice())
                .map_err(|e| format!("recovered resident set holds a non-ed25519 key: {e}"))?,
        );
    }
    Ok(keys)
}

fn advance_next_seq_from_frames(next_seq: &mut u64, frames: &[Vec<u8>], me: &[u8]) {
    for frame in frames {
        if let Some((origin, seq)) = node::frame_origin_seq(frame)
            && origin == me
        {
            *next_seq = (*next_seq).max(seq + 1);
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
    match args.first().map(String::as_str) {
        Some("keygen") => return cmd_keygen(&args[1..]),
        Some("user-key") => {
            return cmd_user_key(&args[1..], &mut std::io::BufReader::new(std::io::stdin()));
        }
        Some("user-sign-bind") => {
            return cmd_user_sign_bind(&args[1..], &mut std::io::BufReader::new(std::io::stdin()));
        }
        Some("user-sign-unbind") => {
            return cmd_user_sign_unbind(
                &args[1..],
                &mut std::io::BufReader::new(std::io::stdin()),
            );
        }
        Some("user-sign-possession") => {
            return cmd_user_sign_possession(
                &args[1..],
                &mut std::io::BufReader::new(std::io::stdin()),
            );
        }
        Some("user-sign-add-member") => {
            return cmd_user_sign_add_member(
                &args[1..],
                &mut std::io::BufReader::new(std::io::stdin()),
            );
        }
        Some("user-sign-remove-member") => {
            return cmd_user_sign_remove_member(
                &args[1..],
                &mut std::io::BufReader::new(std::io::stdin()),
            );
        }
        Some("user-webauthn-challenge") => {
            return cmd_user_webauthn_challenge(&args[1..]);
        }
        Some("user-p256-payload") => {
            return cmd_user_p256_payload(&args[1..]);
        }
        Some("init") => return cmd_init(&args[1..]),
        Some("invite") => return cmd_invite(&args[1..]),
        Some("admit") => return cmd_admit(&args[1..]),
        Some("invite-accept") => return cmd_invite_accept(&args[1..]),
        Some("promote") => return cmd_promote(&args[1..]),
        Some("resident-remove") => return cmd_resident_remove(&args[1..]),
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

// ============================================================================
// user-key lifecycle verbs (init/restore/unlock/reveal/encrypt/status) — see
// docs/superpowers/specs/2026-07-07-identity-onboarding-design.md's "CLI
// verbs" section for the binding stdin/stdout contract. every secret
// (password, mnemonic) crosses the process boundary via STDIN ONLY, one
// newline-delimited field per line in the documented order — never argv/env,
// which would leak into shell history / `ps`. each verb below is split into
// a `user_key_*` core (takes the parsed stdin, returns the value to print —
// directly unit-testable without capturing stdout) and a thin `cmd_user_key_*`
// wrapper that prints it; the wrapper is what `run()`'s dispatch calls.
// ============================================================================

/// read one line from `stdin`, minus its trailing newline — the stdin-only
/// convention every secret field crosses the process boundary through.
/// errors only on true EOF (nothing at all, not even a newline): a caller
/// that doesn't pipe the expected field. an explicit empty line (just `\n`)
/// is NOT an error here — callers that need a non-empty value (passwords)
/// reject that on their own terms (`check_password_len`), with a clearer
/// message than a generic "missing" would give.
fn read_stdin_line(stdin: &mut impl std::io::BufRead, field: &str) -> Result<String, String> {
    let mut line = String::new();
    let n = stdin
        .read_line(&mut line)
        .map_err(|e| format!("read {field} from stdin: {e}"))?;
    if n == 0 {
        return Err(format!("missing {field} on stdin"));
    }
    while line.ends_with('\n') || line.ends_with('\r') {
        line.pop();
    }
    Ok(line)
}

/// the design spec's floor for NEW passwords (`init`/`restore`/`encrypt`),
/// enforced before any file is touched. counts scalar chars, not bytes, so a
/// multi-byte-but-short password isn't laundered past the floor.
const MIN_PASSWORD_LEN: usize = 8;

fn check_password_len(password: &str) -> Result<(), String> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(format!(
            "password must be at least {MIN_PASSWORD_LEN} characters"
        ));
    }
    Ok(())
}

/// `path`'s raw trimmed line — the exact text [`userkey::open_user_key`]
/// parses, as opposed to [`userkey::read_user_key_file`]'s already-decoded
/// shape. verbs that must hand a v2 line to `open_user_key` read it via this.
fn read_key_line(path: &std::path::Path) -> Result<String, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("read {path:?}: {e}"))?;
    let line = text.trim();
    if line.is_empty() {
        return Err(format!("{path:?} is empty"));
    }
    Ok(line.to_string())
}

/// resolve the USER signer at `key_path` for the sign verbs: a v2
/// (encrypted) file decrypts with a password read as the FIRST stdin line;
/// anything else (legacy plaintext, or absent — freshly generated) falls
/// through to [`config::load_or_generate_identity`] UNCHANGED, reading no
/// stdin at all — byte-identical to the pre-onboarding sign-verb behavior.
fn load_user_signer(
    key_path: &std::path::Path,
    stdin: &mut impl std::io::BufRead,
) -> Result<ed25519::PrivateKey, Box<dyn std::error::Error>> {
    if let Ok(text) = std::fs::read_to_string(key_path)
        && text.trim().starts_with(userkey::USER_KEY_V2_PREFIX)
    {
        let password = read_stdin_line(stdin, "password")?;
        return Ok(userkey::open_user_key(text.trim(), &password)?);
    }
    let (user, generated) = config::load_or_generate_identity(key_path)?;
    if generated {
        eprintln!("generated user identity at {}", key_path.display());
    }
    Ok(user)
}

/// `user-key init` core — see [`cmd_user_key_init`] for the print contract.
/// returns `(mnemonic, pubkey-hex)` so tests can assert both independently.
fn user_key_init(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<(String, String), Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let out = PathBuf::from(flags.get("out").map(String::as_str).unwrap_or("user.key"));
    let password = read_stdin_line(stdin, "password")?;
    check_password_len(&password)?;

    let mut seed = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut seed);
    let words = userkey::mnemonic_of_seed(&seed);
    let line = userkey::seal_user_key(&seed, &password)?;
    userkey::write_user_key_new(&out, &line)?;

    let key = ed25519::PrivateKey::decode(seed.as_slice()).expect("32 random bytes decode");
    Ok((words, hex_bytes(key.public_key().as_ref())))
}

/// `user-key init --out <path>` — stdin: password. Generates a fresh seed,
/// writes v2 (refuses to overwrite via `create_new`), and prints the 24-word
/// mnemonic line THEN the pubkey-hex line — pubkey is the LAST stdout line
/// (the `run_verb`/`last_line` contract), mnemonic is the line before it.
fn cmd_user_key_init(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<(), Box<dyn std::error::Error>> {
    let (words, pubkey_hex) = user_key_init(args, stdin)?;
    println!("{words}");
    println!("{pubkey_hex}");
    Ok(())
}

/// `user-key restore` core — see [`cmd_user_key_restore`].
fn user_key_restore(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let out = PathBuf::from(flags.get("out").map(String::as_str).unwrap_or("user.key"));
    let mnemonic = read_stdin_line(stdin, "mnemonic")?;
    let password = read_stdin_line(stdin, "password")?;
    check_password_len(&password)?;

    let seed = userkey::seed_of_mnemonic(&mnemonic)?;
    let line = userkey::seal_user_key(&seed, &password)?;
    userkey::write_user_key_new(&out, &line)?;

    let key = ed25519::PrivateKey::decode(seed.as_slice())
        .map_err(|e| format!("restored seed is not a valid ed25519 secret: {e}"))?;
    Ok(hex_bytes(key.public_key().as_ref()))
}

/// `user-key restore --out <path>` — stdin: mnemonic line, then password
/// line. Validates the BIP39 checksum, writes v2 (refuses to overwrite),
/// prints the pubkey (the only stdout line).
fn cmd_user_key_restore(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", user_key_restore(args, stdin)?);
    Ok(())
}

/// `user-key unlock` core — see [`cmd_user_key_unlock`].
fn user_key_unlock(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let key_path = PathBuf::from(flags.get("key").ok_or("user-key unlock needs --key <path>")?);
    let password = read_stdin_line(stdin, "password")?;

    let key = match userkey::read_user_key_file(&key_path)? {
        userkey::UserKeyFile::Plaintext(key) => key,
        userkey::UserKeyFile::Encrypted(_) => {
            let line = read_key_line(&key_path)?;
            userkey::open_user_key(&line, &password)?
        }
    };
    Ok(hex_bytes(key.public_key().as_ref()))
}

/// `user-key unlock --key <path>` — stdin: password. Pure verification
/// (nothing persists); prints the pubkey on success, a clean error + nonzero
/// exit on a wrong password or a corrupt file.
fn cmd_user_key_unlock(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", user_key_unlock(args, stdin)?);
    Ok(())
}

/// `user-key reveal` core — see [`cmd_user_key_reveal`].
fn user_key_reveal(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let key_path = PathBuf::from(flags.get("key").ok_or("user-key reveal needs --key <path>")?);

    // legacy plaintext tolerates an absent/empty password line; only an
    // encrypted file actually needs one. read leniently (empty on EOF) so a
    // caller revealing a legacy key doesn't have to pipe an unused line.
    let mut password = String::new();
    let _ = stdin.read_line(&mut password);
    while password.ends_with('\n') || password.ends_with('\r') {
        password.pop();
    }

    let key = match userkey::read_user_key_file(&key_path)? {
        userkey::UserKeyFile::Plaintext(key) => key,
        userkey::UserKeyFile::Encrypted(_) => {
            let line = read_key_line(&key_path)?;
            userkey::open_user_key(&line, &password)?
        }
    };
    let seed_bytes = key.encode();
    let seed: [u8; 32] = seed_bytes
        .as_ref()
        .try_into()
        .map_err(|_| "decoded key is not a 32-byte seed".to_string())?;
    Ok(userkey::mnemonic_of_seed(&seed))
}

/// `user-key reveal --key <path>` — stdin: password (empty/absent tolerated
/// for legacy plaintext, required to decrypt v2). Prints the 24-word
/// mnemonic — the SAME encoding `init`/`restore` use, so it round-trips
/// through `user-key restore` to the identical pubkey.
fn cmd_user_key_reveal(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", user_key_reveal(args, stdin)?);
    Ok(())
}

/// `user-key encrypt` core — see [`cmd_user_key_encrypt`].
fn user_key_encrypt(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let key_path = PathBuf::from(flags.get("key").ok_or("user-key encrypt needs --key <path>")?);
    let password = read_stdin_line(stdin, "password")?;
    check_password_len(&password)?;

    let key = match userkey::read_user_key_file(&key_path)? {
        userkey::UserKeyFile::Plaintext(key) => key,
        userkey::UserKeyFile::Encrypted(_) => {
            return Err(format!("{} is already encrypted", key_path.display()).into());
        }
    };
    let seed_bytes = key.encode();
    let seed: [u8; 32] = seed_bytes
        .as_ref()
        .try_into()
        .map_err(|_| "decoded key is not a 32-byte seed".to_string())?;
    let line = userkey::seal_user_key(&seed, &password)?;
    userkey::rewrite_user_key(&key_path, &line)?;
    Ok(hex_bytes(key.public_key().as_ref()))
}

/// `user-key encrypt --key <path>` — stdin: password. Migrates a legacy v1
/// plaintext file to v2 in place (temp file + rename, the same atomicity as
/// every other in-place rewrite); errors (no-op) if the file is already v2.
/// Prints the pubkey (unchanged by the migration).
fn cmd_user_key_encrypt(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", user_key_encrypt(args, stdin)?);
    Ok(())
}

/// `user-key status` core — see [`cmd_user_key_status`].
fn user_key_status(args: &[String]) -> Result<String, Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let key_path = PathBuf::from(flags.get("key").ok_or("user-key status needs --key <path>")?);
    if !key_path.exists() {
        return Ok("absent".to_string());
    }
    Ok(match userkey::read_user_key_file(&key_path)? {
        userkey::UserKeyFile::Plaintext(key) => {
            format!("plaintext {}", hex_bytes(key.public_key().as_ref()))
        }
        userkey::UserKeyFile::Encrypted(enc) => format!("encrypted {}", hex_bytes(&enc.pubkey)),
    })
}

/// `user-key status --key <path>` — no stdin. Prints exactly one of `absent`
/// | `plaintext <pubkey-hex>` | `encrypted <pubkey-hex>`; never touches a
/// password, so it's safe to poll from the app on every launch.
fn cmd_user_key_status(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", user_key_status(args)?);
    Ok(())
}

/// `user-key [init|restore|unlock|reveal|encrypt|status]` — dispatches to the
/// v2 lifecycle verbs (see the design spec's "CLI verbs" section); a bare
/// `user-key [--out <path>]` (no recognized subcommand) falls through to the
/// legacy v1 generate-or-reuse shape from #205, kept working unchanged for
/// the app/tests until the app migrates onto the v2 verbs.
fn cmd_user_key(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<(), Box<dyn std::error::Error>> {
    match args.first().map(String::as_str) {
        Some("init") => return cmd_user_key_init(&args[1..], stdin),
        Some("restore") => return cmd_user_key_restore(&args[1..], stdin),
        Some("unlock") => return cmd_user_key_unlock(&args[1..], stdin),
        Some("reveal") => return cmd_user_key_reveal(&args[1..], stdin),
        Some("encrypt") => return cmd_user_key_encrypt(&args[1..], stdin),
        Some("status") => return cmd_user_key_status(&args[1..]),
        Some(other) if !other.starts_with("--") => {
            return Err(format!(
                "unknown user-key subcommand {other:?} (want \
                 init|restore|unlock|reveal|encrypt|status, or a bare \
                 `user-key [--out <path>]` to generate/reuse a legacy key)"
            )
            .into());
        }
        _ => {}
    }
    cmd_user_key_generate_legacy(args)
}

/// `user-key [--out <path>]` — generate (or reuse) a persisted ed25519 USER
/// identity: the human's app-side keypair (distinct from `keygen`'s per-node
/// identity), a bare hex ed25519 seed file under the same load-or-generate
/// discipline. pubkey on stdout (scriptable — the desktop shell's `run_verb`
/// takes the LAST stdout line as the value), provenance on stderr. the
/// legacy v1 shape (#205), kept working verbatim; `init` is the v2
/// replacement `cmd_user_key` dispatches to instead.
fn cmd_user_key_generate_legacy(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let out = PathBuf::from(flags.get("out").map(String::as_str).unwrap_or("user.key"));
    let (key, generated) = config::load_or_generate_identity(&out)?;
    println!("{}", hex_bytes(key.public_key().as_ref()));
    eprintln!(
        "{} user identity at {}",
        if generated { "generated" } else { "reusing" },
        out.display()
    );
    Ok(())
}

/// `user-sign-bind` core — see [`cmd_user_sign_bind`].
fn user_sign_bind(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    use identity::{IdentityMsg, encode_msg};

    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let key_path = PathBuf::from(flags.get("key").ok_or("user-sign-bind needs --key <path>")?);
    let chain_id = flags
        .get("chain-id")
        .ok_or("user-sign-bind needs --chain-id <id>")?;
    let node_pub_hex = flags
        .get("node-pub")
        .ok_or("user-sign-bind needs --node-pub <hex>")?;
    let node_pub = config::decode_key(node_pub_hex)?;
    let nonce: u64 = flags
        .get("nonce")
        .ok_or("user-sign-bind needs --nonce <n>")?
        .parse()
        .map_err(|e| format!("--nonce is not a valid u64: {e}"))?;

    let user = load_user_signer(&key_path, stdin)?;
    let authorizer = config::ed25519_member_auth(
        &user,
        identity::IDENTITY_BIND_NS,
        &identity::bind_preimage(chain_id, node_pub.as_ref(), nonce),
    );
    let msg = IdentityMsg::BindNode { authorizer };
    Ok(String::from_utf8(encode_msg(&msg)).expect("json is utf-8"))
}

/// `user-sign-bind --key <path> --chain-id <id> --node-pub <hex> --nonce <n>`
/// — mint a bind certificate binding `node-pub` to the user identity at
/// `--key` (generated there if absent, or decrypted with stdin's password
/// line if it's a v2 file — see [`load_user_signer`]), at `chain-id`/`nonce`,
/// and print the ready-to-submit `IdentityMsg::BindNode` JSON as the last
/// (only) stdout line. `user_key` rides the payload — the node being bound is
/// the verified submit ORIGIN, never a payload field; the module resolves it
/// from the rpc transport, not from this CLI.
fn cmd_user_sign_bind(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", user_sign_bind(args, stdin)?);
    Ok(())
}

/// `user-sign-unbind` core — see [`cmd_user_sign_unbind`].
fn user_sign_unbind(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    use identity::{IdentityMsg, encode_msg};

    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let key_path = PathBuf::from(flags.get("key").ok_or("user-sign-unbind needs --key <path>")?);
    let chain_id = flags
        .get("chain-id")
        .ok_or("user-sign-unbind needs --chain-id <id>")?;
    let node_pub_hex = flags
        .get("node-pub")
        .ok_or("user-sign-unbind needs --node-pub <hex>")?;
    let node_pub = config::decode_key(node_pub_hex)?;
    let nonce: u64 = flags
        .get("nonce")
        .ok_or("user-sign-unbind needs --nonce <n>")?
        .parse()
        .map_err(|e| format!("--nonce is not a valid u64: {e}"))?;

    let user = load_user_signer(&key_path, stdin)?;
    let authorizer = config::ed25519_member_auth(
        &user,
        identity::IDENTITY_UNBIND_NS,
        &identity::unbind_preimage(chain_id, node_pub.as_ref(), nonce),
    );
    let msg = IdentityMsg::UnbindNode {
        node_key: node_pub.as_ref().to_vec(),
        authorizer,
    };
    Ok(String::from_utf8(encode_msg(&msg)).expect("json is utf-8"))
}

/// `user-sign-unbind --key <path> --chain-id <id> --node-pub <hex> --nonce <n>`
/// — mint an unbind certificate evicting `node-pub` from the user identity at
/// `--key`, and print the ready-to-submit `IdentityMsg::UnbindNode` JSON as
/// the last stdout line. `node_key` (not `user_key`) rides the payload:
/// unbind carries no origin restriction — a surviving device evicts a lost
/// one by naming it directly, identified via the existing binding.
fn cmd_user_sign_unbind(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", user_sign_unbind(args, stdin)?);
    Ok(())
}

/// parse a `--new-kind` flag value into a [`identity::KeyKind`]. the CLI's own
/// key is always ed25519; `p256`/`webauthn_p256` name the kind of a DIFFERENT
/// key being admitted (whose possession proof comes from that key's holder --
/// a native signer, or the FIDO2 transport for a passkey).
fn parse_kind(s: &str) -> Result<identity::KeyKind, Box<dyn std::error::Error>> {
    match s {
        "ed25519" => Ok(identity::KeyKind::Ed25519),
        "p256" => Ok(identity::KeyKind::P256),
        "webauthn_p256" | "webauthn-p256" | "passkey" => Ok(identity::KeyKind::WebauthnP256),
        other => {
            Err(format!("unknown key kind {other:?} (want ed25519|p256|webauthn_p256)").into())
        }
    }
}

/// `user-sign-possession` core — see [`cmd_user_sign_possession`].
fn user_sign_possession(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let key_path = PathBuf::from(flags.get("key").ok_or("user-sign-possession needs --key <path>")?);
    let chain_id = flags
        .get("chain-id")
        .ok_or("user-sign-possession needs --chain-id <id>")?;
    let account_id = config::unhex(
        flags
            .get("account-id")
            .ok_or("user-sign-possession needs --account-id <hex>")?,
    )?;
    let nonce: u64 = flags
        .get("nonce")
        .ok_or("user-sign-possession needs --nonce <n>")?
        .parse()
        .map_err(|e| format!("--nonce is not a valid u64: {e}"))?;

    // this key proves it holds itself over the add-member preimage; its own
    // pubkey is `new_key`, and the node's user key is ed25519.
    let user = load_user_signer(&key_path, stdin)?;
    let new_key = user.public_key().as_ref().to_vec();
    let preimage = identity::add_member_preimage(
        chain_id,
        &account_id,
        &new_key,
        identity::KeyKind::Ed25519,
        nonce,
    );
    let proof = config::ed25519_possession(&user, identity::IDENTITY_ADD_MEMBER_NS, &preimage);
    Ok(serde_json::to_string(&proof).expect("json is utf-8"))
}

/// `user-sign-possession --key <path> --chain-id <id> --account-id <hex> --nonce <n>`
/// — for a NEW ed25519 device joining an existing account: print the
/// possession-proof `MemberProof` JSON this device signs over the add-member
/// preimage (pair its `user-key status` pubkey with it). the existing member
/// then feeds both to `user-sign-add-member`.
fn cmd_user_sign_possession(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", user_sign_possession(args, stdin)?);
    Ok(())
}

/// `user-sign-add-member` core — see [`cmd_user_sign_add_member`].
fn user_sign_add_member(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    use identity::{IdentityMsg, encode_msg};

    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let key_path = PathBuf::from(flags.get("key").ok_or("user-sign-add-member needs --key <path>")?);
    let chain_id = flags
        .get("chain-id")
        .ok_or("user-sign-add-member needs --chain-id <id>")?;
    let account_id = config::unhex(
        flags
            .get("account-id")
            .ok_or("user-sign-add-member needs --account-id <hex>")?,
    )?;
    let new_key = config::unhex(
        flags
            .get("new-key")
            .ok_or("user-sign-add-member needs --new-key <hex>")?,
    )?;
    let new_kind = parse_kind(
        flags
            .get("new-kind")
            .ok_or("user-sign-add-member needs --new-kind <ed25519|p256|webauthn_p256>")?,
    )?;
    let nonce: u64 = flags
        .get("nonce")
        .ok_or("user-sign-add-member needs --nonce <n>")?
        .parse()
        .map_err(|e| format!("--nonce is not a valid u64: {e}"))?;
    let new_label = flags.get("label").cloned();
    let possession: identity::MemberProof = serde_json::from_str(
        flags
            .get("possession")
            .ok_or("user-sign-add-member needs --possession <MemberProof json>")?,
    )
    .map_err(|e| format!("--possession is not a MemberProof: {e}"))?;

    // the local user key is an existing member; it consents to admitting the
    // new key over the same preimage the new key proved possession of.
    let user = load_user_signer(&key_path, stdin)?;
    let preimage = identity::add_member_preimage(chain_id, &account_id, &new_key, new_kind, nonce);
    let authorizer = config::ed25519_member_auth(&user, identity::IDENTITY_ADD_MEMBER_NS, &preimage);
    let msg = IdentityMsg::AddMemberKey {
        new_key,
        new_kind,
        new_label,
        possession,
        authorizer,
    };
    Ok(String::from_utf8(encode_msg(&msg)).expect("json is utf-8"))
}

/// `user-sign-add-member --key <path> --chain-id <id> --account-id <hex>
/// --new-key <hex> --new-kind <ed25519|p256|webauthn_p256> --nonce <n>
/// --possession <json> [--label <s>]` — the LOCAL user key (an existing
/// member) consents to admitting `new-key`; `--possession` is that key's own
/// proof (from `user-sign-possession`, or the FIDO2 transport for a passkey).
/// prints the ready-to-submit `IdentityMsg::AddMemberKey` JSON.
fn cmd_user_sign_add_member(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", user_sign_add_member(args, stdin)?);
    Ok(())
}

/// `user-sign-remove-member` core — see [`cmd_user_sign_remove_member`].
fn user_sign_remove_member(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    use identity::{IdentityMsg, encode_msg};

    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let key_path =
        PathBuf::from(flags.get("key").ok_or("user-sign-remove-member needs --key <path>")?);
    let chain_id = flags
        .get("chain-id")
        .ok_or("user-sign-remove-member needs --chain-id <id>")?;
    let account_id = config::unhex(
        flags
            .get("account-id")
            .ok_or("user-sign-remove-member needs --account-id <hex>")?,
    )?;
    let target_key = config::unhex(
        flags
            .get("target-key")
            .ok_or("user-sign-remove-member needs --target-key <hex>")?,
    )?;
    let nonce: u64 = flags
        .get("nonce")
        .ok_or("user-sign-remove-member needs --nonce <n>")?
        .parse()
        .map_err(|e| format!("--nonce is not a valid u64: {e}"))?;

    let user = load_user_signer(&key_path, stdin)?;
    let preimage = identity::remove_member_preimage(chain_id, &account_id, &target_key, nonce);
    let authorizer =
        config::ed25519_member_auth(&user, identity::IDENTITY_REMOVE_MEMBER_NS, &preimage);
    let msg = IdentityMsg::RemoveMemberKey { target_key, authorizer };
    Ok(String::from_utf8(encode_msg(&msg)).expect("json is utf-8"))
}

/// `user-sign-remove-member --key <path> --chain-id <id> --account-id <hex>
/// --target-key <hex> --nonce <n>` — the LOCAL user key (a member) evicts
/// `target-key` from the account. prints the ready-to-submit
/// `IdentityMsg::RemoveMemberKey` JSON. any member may remove any member
/// except the last one.
fn cmd_user_sign_remove_member(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", user_sign_remove_member(args, stdin)?);
    Ok(())
}

/// `user-webauthn-challenge` core — see [`cmd_user_webauthn_challenge`].
fn user_webauthn_challenge(args: &[String]) -> Result<String, Box<dyn std::error::Error>> {
    use base64::Engine as _;

    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let chain_id = flags
        .get("chain-id")
        .ok_or("user-webauthn-challenge needs --chain-id <id>")?;
    let account_id = config::unhex(
        flags
            .get("account-id")
            .ok_or("user-webauthn-challenge needs --account-id <hex>")?,
    )?;
    let new_key = config::unhex(
        flags
            .get("new-key")
            .ok_or("user-webauthn-challenge needs --new-key <hex>")?,
    )?;
    let nonce: u64 = flags
        .get("nonce")
        .ok_or("user-webauthn-challenge needs --nonce <n>")?
        .parse()
        .map_err(|e| format!("--nonce is not a valid u64: {e}"))?;

    // the exact bytes the on-chain verifier will demand the passkey signed:
    // SHA256(ADD_MEMBER_NS ‖ add_member_preimage(...)). one source of truth
    // with `identity::verify_authority` — no drift between enroll and verify.
    let preimage = identity::add_member_preimage(
        chain_id,
        &account_id,
        &new_key,
        identity::KeyKind::WebauthnP256,
        nonce,
    );
    let challenge =
        identity::webauthn_challenge(identity::IDENTITY_ADD_MEMBER_NS, &preimage);
    // base64url (no pad) — WebAuthn's native challenge encoding, so the phone
    // page passes it straight into `navigator.credentials.get({ challenge })`.
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(challenge))
}

/// `user-webauthn-challenge --chain-id <id> --account-id <hex> --new-key <hex>
/// --nonce <n>` — print the base64url WebAuthn challenge a passkey must sign to
/// join `account-id` as `new-key` at `nonce`. Pure computation (no key, no
/// signing): the phone's `get()` signs this, and the resulting assertion feeds
/// `user-sign-add-member --possession`. Keeping the preimage math in the node
/// (not the web page) is why "core in node" — the page never reconstructs it.
fn cmd_user_webauthn_challenge(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", user_webauthn_challenge(args)?);
    Ok(())
}

/// `user-p256-payload` core — see [`cmd_user_p256_payload`].
fn user_p256_payload(args: &[String]) -> Result<String, Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let chain_id = flags
        .get("chain-id")
        .ok_or("user-p256-payload needs --chain-id <id>")?;
    let account_id = config::unhex(
        flags
            .get("account-id")
            .ok_or("user-p256-payload needs --account-id <hex>")?,
    )?;
    let new_key = config::unhex(
        flags
            .get("new-key")
            .ok_or("user-p256-payload needs --new-key <hex>")?,
    )?;
    let nonce: u64 = flags
        .get("nonce")
        .ok_or("user-p256-payload needs --nonce <n>")?
        .parse()
        .map_err(|e| format!("--nonce is not a valid u64: {e}"))?;

    // the exact bytes a P256 joiner must ECDSA-sign — union_unique(ADD_MEMBER_NS,
    // add_member_preimage(...)), what the on-chain verifier reconstructs. Hex so
    // the phone hex-decodes and signs them raw; no preimage math on the page.
    let payload = identity::add_member_signing_payload(
        chain_id,
        &account_id,
        &new_key,
        identity::KeyKind::P256,
        nonce,
    );
    Ok(payload.iter().map(|b| format!("{b:02x}")).collect())
}

/// `user-p256-payload --chain-id <id> --account-id <hex> --new-key <hex>
/// --nonce <n>` — print the hex bytes a software P256 key (a phone's pure-JS
/// signer, in the in-app LAN enrollment) must ECDSA-P256-SHA256-sign to join
/// `account-id` as `new-key` at `nonce`. Its raw R‖S signature feeds
/// `user-sign-add-member --new-kind p256 --possession`. Pure computation.
fn cmd_user_p256_payload(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", user_p256_payload(args)?);
    Ok(())
}

#[cfg(test)]
mod webauthn_challenge_tests {
    use super::*;

    fn challenge(chain: &str, account_hex: &str, new_hex: &str, nonce: &str) -> String {
        let args: Vec<String> = [
            "--chain-id",
            chain,
            "--account-id",
            account_hex,
            "--new-key",
            new_hex,
            "--nonce",
            nonce,
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        user_webauthn_challenge(&args).unwrap()
    }

    #[test]
    fn challenge_matches_the_on_chain_verifier_math() {
        use base64::Engine as _;
        let account_id = [0xabu8; 33];
        let new_key = [0xcdu8; 33];
        let account_hex: String = account_id.iter().map(|b| format!("{b:02x}")).collect();
        let new_hex: String = new_key.iter().map(|b| format!("{b:02x}")).collect();

        let got = challenge("team#abcd", &account_hex, &new_hex, "5");

        // recompute via identity's PUBLIC surface — the exact functions the
        // verifier uses. if the verb and the verifier ever diverge, an enrolled
        // passkey would sign a challenge the chain then rejects.
        let expected = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
            identity::webauthn_challenge(
                identity::IDENTITY_ADD_MEMBER_NS,
                &identity::add_member_preimage(
                    "team#abcd",
                    &account_id,
                    &new_key,
                    identity::KeyKind::WebauthnP256,
                    5,
                ),
            ),
        );
        assert_eq!(got, expected);
    }

    #[test]
    fn challenge_binds_chain_account_key_and_nonce() {
        let base = challenge("c", "aa", "bb", "0");
        assert_ne!(base, challenge("d", "aa", "bb", "0"), "chain must move it");
        assert_ne!(base, challenge("c", "cc", "bb", "0"), "account must move it");
        assert_ne!(base, challenge("c", "aa", "cc", "0"), "new key must move it");
        assert_ne!(base, challenge("c", "aa", "bb", "1"), "nonce must move it");
    }

    #[test]
    fn p256_payload_matches_identity_signing_payload() {
        let account_id = [0xabu8; 33];
        let new_key = [0xcdu8; 33];
        let account_hex: String = account_id.iter().map(|b| format!("{b:02x}")).collect();
        let new_hex: String = new_key.iter().map(|b| format!("{b:02x}")).collect();

        let args: Vec<String> = [
            "--chain-id",
            "team#abcd",
            "--account-id",
            &account_hex,
            "--new-key",
            &new_hex,
            "--nonce",
            "5",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let got = user_p256_payload(&args).unwrap();

        // the verb's hex must be exactly identity's signing payload — the bytes
        // the on-chain P256 verifier reconstructs.
        let expected: String = identity::add_member_signing_payload(
            "team#abcd",
            &account_id,
            &new_key,
            identity::KeyKind::P256,
            5,
        )
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
        assert_eq!(got, expected);
    }
}

#[cfg(test)]
mod userkey_verb_tests {
    use super::*;
    use std::io::Cursor;

    /// build the `&[String]` verb args from string-literal parts.
    fn args_of(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    /// a stdin double: one line per element, in order.
    fn stdin_of(lines: &[&str]) -> Cursor<Vec<u8>> {
        let mut s = String::new();
        for l in lines {
            s.push_str(l);
            s.push('\n');
        }
        Cursor::new(s.into_bytes())
    }

    fn empty_stdin() -> Cursor<Vec<u8>> {
        Cursor::new(Vec::new())
    }

    fn write_legacy(path: &std::path::Path, seed: &[u8; 32]) {
        userkey::write_user_key_new(path, &hex_bytes(seed)).unwrap();
    }

    fn pubkey_of(seed: &[u8; 32]) -> String {
        hex_bytes(
            ed25519::PrivateKey::decode(seed.as_slice())
                .unwrap()
                .public_key()
                .as_ref(),
        )
    }

    #[test]
    fn init_writes_v2_and_outputs_mnemonic_then_pubkey() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("user.key");
        let mut stdin = stdin_of(&["correct horse battery"]);

        let (words, pubkey_hex) = user_key_init(
            &args_of(&["--out", &path.to_string_lossy()]),
            &mut stdin,
        )
        .unwrap();

        assert_eq!(words.split_whitespace().count(), 24);
        assert_eq!(pubkey_hex.len(), 64);
        match userkey::read_user_key_file(&path).unwrap() {
            userkey::UserKeyFile::Encrypted(enc) => {
                assert_eq!(hex_bytes(&enc.pubkey), pubkey_hex);
            }
            userkey::UserKeyFile::Plaintext(_) => panic!("expected v2/Encrypted"),
        }
    }

    #[test]
    fn restore_round_trips_init_mnemonic_to_identical_pubkey() {
        let dir = tempfile::tempdir().unwrap();
        let init_path = dir.path().join("a.key");
        let mut init_stdin = stdin_of(&["password one"]);
        let (words, pubkey_hex) = user_key_init(
            &args_of(&["--out", &init_path.to_string_lossy()]),
            &mut init_stdin,
        )
        .unwrap();

        let restore_path = dir.path().join("b.key");
        let mut restore_stdin = stdin_of(&[&words, "password two"]);
        let restored_pubkey = user_key_restore(
            &args_of(&["--out", &restore_path.to_string_lossy()]),
            &mut restore_stdin,
        )
        .unwrap();

        assert_eq!(restored_pubkey, pubkey_hex);
    }

    #[test]
    fn unlock_verifies_and_rejects_wrong_password() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("user.key");
        let mut init_stdin = stdin_of(&["right password"]);
        let (_, pubkey_hex) = user_key_init(
            &args_of(&["--out", &path.to_string_lossy()]),
            &mut init_stdin,
        )
        .unwrap();

        let mut ok_stdin = stdin_of(&["right password"]);
        let unlocked =
            user_key_unlock(&args_of(&["--key", &path.to_string_lossy()]), &mut ok_stdin).unwrap();
        assert_eq!(unlocked, pubkey_hex);

        let mut bad_stdin = stdin_of(&["wrong password"]);
        assert!(
            user_key_unlock(&args_of(&["--key", &path.to_string_lossy()]), &mut bad_stdin)
                .is_err()
        );
    }

    #[test]
    fn reveal_returns_same_words_for_v2_and_legacy() {
        let dir = tempfile::tempdir().unwrap();

        // v2: reveal requires the password.
        let v2_path = dir.path().join("v2.key");
        let mut init_stdin = stdin_of(&["a password"]);
        let (words, _) = user_key_init(
            &args_of(&["--out", &v2_path.to_string_lossy()]),
            &mut init_stdin,
        )
        .unwrap();
        let mut reveal_stdin = stdin_of(&["a password"]);
        let revealed = user_key_reveal(
            &args_of(&["--key", &v2_path.to_string_lossy()]),
            &mut reveal_stdin,
        )
        .unwrap();
        assert_eq!(revealed, words);

        // legacy: reveal tolerates an absent password line entirely.
        let legacy_path = dir.path().join("legacy.key");
        let seed = [42u8; 32];
        write_legacy(&legacy_path, &seed);
        let legacy_words = userkey::mnemonic_of_seed(&seed);

        let mut stdin = empty_stdin();
        let revealed_legacy = user_key_reveal(
            &args_of(&["--key", &legacy_path.to_string_lossy()]),
            &mut stdin,
        )
        .unwrap();
        assert_eq!(revealed_legacy, legacy_words);
    }

    #[test]
    fn encrypt_migrates_legacy_to_v2_preserving_pubkey_and_mnemonic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("user.key");
        let seed = [9u8; 32];
        write_legacy(&path, &seed);
        let expected_pubkey = pubkey_of(&seed);

        let mut before_stdin = empty_stdin();
        let words_before =
            user_key_reveal(&args_of(&["--key", &path.to_string_lossy()]), &mut before_stdin)
                .unwrap();

        let mut encrypt_stdin = stdin_of(&["fresh password"]);
        let pubkey_after = user_key_encrypt(
            &args_of(&["--key", &path.to_string_lossy()]),
            &mut encrypt_stdin,
        )
        .unwrap();
        assert_eq!(pubkey_after, expected_pubkey);

        // already-v2 encrypt is a hard error, not a silent no-op.
        let mut second_stdin = stdin_of(&["another password"]);
        assert!(
            user_key_encrypt(&args_of(&["--key", &path.to_string_lossy()]), &mut second_stdin)
                .is_err()
        );

        let mut after_stdin = stdin_of(&["fresh password"]);
        let words_after =
            user_key_reveal(&args_of(&["--key", &path.to_string_lossy()]), &mut after_stdin)
                .unwrap();
        assert_eq!(words_after, words_before);
    }

    #[test]
    fn status_reports_all_three_shapes() {
        let dir = tempfile::tempdir().unwrap();

        let absent_path = dir.path().join("absent.key");
        assert_eq!(
            user_key_status(&args_of(&["--key", &absent_path.to_string_lossy()])).unwrap(),
            "absent"
        );

        let plaintext_path = dir.path().join("plaintext.key");
        let seed = [3u8; 32];
        write_legacy(&plaintext_path, &seed);
        assert_eq!(
            user_key_status(&args_of(&["--key", &plaintext_path.to_string_lossy()])).unwrap(),
            format!("plaintext {}", pubkey_of(&seed))
        );

        let encrypted_path = dir.path().join("encrypted.key");
        let mut stdin = stdin_of(&["a password"]);
        let (_, init_pubkey_hex) = user_key_init(
            &args_of(&["--out", &encrypted_path.to_string_lossy()]),
            &mut stdin,
        )
        .unwrap();
        assert_eq!(
            user_key_status(&args_of(&["--key", &encrypted_path.to_string_lossy()])).unwrap(),
            format!("encrypted {init_pubkey_hex}")
        );
    }

    #[test]
    fn short_password_rejected_in_init_restore_encrypt() {
        let dir = tempfile::tempdir().unwrap();

        let init_path = dir.path().join("init.key");
        let mut stdin = stdin_of(&["short1"]);
        assert!(
            user_key_init(&args_of(&["--out", &init_path.to_string_lossy()]), &mut stdin)
                .is_err()
        );
        assert!(
            !init_path.exists(),
            "a rejected password must not write a file"
        );

        let words = userkey::mnemonic_of_seed(&[1u8; 32]);
        let restore_path = dir.path().join("restore.key");
        let mut stdin = stdin_of(&[&words, "short1"]);
        assert!(
            user_key_restore(&args_of(&["--out", &restore_path.to_string_lossy()]), &mut stdin)
                .is_err()
        );
        assert!(!restore_path.exists());

        let legacy_path = dir.path().join("legacy.key");
        let seed = [5u8; 32];
        write_legacy(&legacy_path, &seed);
        let mut stdin = stdin_of(&["short1"]);
        assert!(
            user_key_encrypt(&args_of(&["--key", &legacy_path.to_string_lossy()]), &mut stdin)
                .is_err()
        );
        // a rejected password must not have migrated the file.
        match userkey::read_user_key_file(&legacy_path).unwrap() {
            userkey::UserKeyFile::Plaintext(_) => {}
            userkey::UserKeyFile::Encrypted(_) => panic!("still-plaintext expected"),
        }
    }

    /// same seed, two custody shapes (legacy plaintext vs v2+password) must
    /// mint byte-identical bind JSON (ed25519 signing is deterministic), and
    /// that JSON must decode via `identity::decode_msg`.
    #[test]
    fn sign_bind_v2_password_matches_legacy_and_decodes() {
        let dir = tempfile::tempdir().unwrap();
        let seed = [77u8; 32];
        // an arbitrary but VALID ed25519 point — a public key derived from a
        // seed, not raw bytes (not every 32-byte string is on-curve).
        let node_pub_hex = pubkey_of(&[100u8; 32]);

        let legacy_path = dir.path().join("legacy.key");
        write_legacy(&legacy_path, &seed);
        let mut stdin = empty_stdin();
        let legacy_json = user_sign_bind(
            &args_of(&[
                "--key",
                &legacy_path.to_string_lossy(),
                "--chain-id",
                "test-chain",
                "--node-pub",
                &node_pub_hex,
                "--nonce",
                "0",
            ]),
            &mut stdin,
        )
        .unwrap();

        let v2_path = dir.path().join("v2.key");
        let line = userkey::seal_user_key(&seed, "a password").unwrap();
        userkey::write_user_key_new(&v2_path, &line).unwrap();
        let mut stdin = stdin_of(&["a password"]);
        let v2_json = user_sign_bind(
            &args_of(&[
                "--key",
                &v2_path.to_string_lossy(),
                "--chain-id",
                "test-chain",
                "--node-pub",
                &node_pub_hex,
                "--nonce",
                "0",
            ]),
            &mut stdin,
        )
        .unwrap();

        assert_eq!(legacy_json, v2_json);

        match identity::decode_msg(legacy_json.as_bytes()).unwrap() {
            identity::IdentityMsg::BindNode { authorizer } => {
                assert_eq!(authorizer.key, pubkey_bytes(&seed));
                assert_eq!(authorizer.kind, identity::KeyKind::Ed25519);
            }
            other => panic!("expected BindNode, got {other:?}"),
        }

        // wrong password fails cleanly (and never silently falls back to
        // auto-generating a fresh legacy key underneath the v2 file).
        let mut bad_stdin = stdin_of(&["wrong password"]);
        assert!(
            user_sign_bind(
                &args_of(&[
                    "--key",
                    &v2_path.to_string_lossy(),
                    "--chain-id",
                    "test-chain",
                    "--node-pub",
                    &node_pub_hex,
                    "--nonce",
                    "0",
                ]),
                &mut bad_stdin,
            )
            .is_err()
        );
    }

    #[test]
    fn sign_unbind_v2_password_matches_legacy_and_decodes() {
        let dir = tempfile::tempdir().unwrap();
        let seed = [88u8; 32];
        let node_pub_hex = pubkey_of(&[101u8; 32]);

        let legacy_path = dir.path().join("legacy.key");
        write_legacy(&legacy_path, &seed);
        let mut stdin = empty_stdin();
        let legacy_json = user_sign_unbind(
            &args_of(&[
                "--key",
                &legacy_path.to_string_lossy(),
                "--chain-id",
                "test-chain",
                "--node-pub",
                &node_pub_hex,
                "--nonce",
                "1",
            ]),
            &mut stdin,
        )
        .unwrap();

        let v2_path = dir.path().join("v2.key");
        let line = userkey::seal_user_key(&seed, "a password").unwrap();
        userkey::write_user_key_new(&v2_path, &line).unwrap();
        let mut stdin = stdin_of(&["a password"]);
        let v2_json = user_sign_unbind(
            &args_of(&[
                "--key",
                &v2_path.to_string_lossy(),
                "--chain-id",
                "test-chain",
                "--node-pub",
                &node_pub_hex,
                "--nonce",
                "1",
            ]),
            &mut stdin,
        )
        .unwrap();

        assert_eq!(legacy_json, v2_json);
        assert!(identity::decode_msg(legacy_json.as_bytes()).is_ok());
    }

    fn pubkey_bytes(seed: &[u8; 32]) -> Vec<u8> {
        ed25519::PrivateKey::decode(seed.as_slice())
            .unwrap()
            .public_key()
            .as_ref()
            .to_vec()
    }

    #[test]
    fn legacy_bare_generate_verb_still_works() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("legacy.key");
        cmd_user_key_generate_legacy(&args_of(&["--out", &path.to_string_lossy()])).unwrap();
        match userkey::read_user_key_file(&path).unwrap() {
            userkey::UserKeyFile::Plaintext(_) => {}
            userkey::UserKeyFile::Encrypted(_) => panic!("legacy generate must write v1"),
        }
    }

    #[test]
    fn unknown_subcommand_errors_cleanly() {
        let mut stdin = empty_stdin();
        let err = cmd_user_key(&args_of(&["bogus"]), &mut stdin).unwrap_err();
        assert!(err.to_string().contains("unknown user-key subcommand"));
    }

    // regression: a parked joiner must track the SAME epoch mesh as every
    // member — `descriptor_mesh ∪ participants ∪ residents`. discovery kills a
    // peer whose bit-vector length disagrees at a shared index, so a joiner that
    // drops the manifest's residents (its own grant included) tracks a shorter
    // set and is torn down on every gossip round — the churn the sentry +
    // coordinator resident hit (`bit vector length mismatch expected=2 actual=3`).
    #[test]
    fn joiner_epoch_mesh_folds_members_and_residents() {
        let founder = ed25519::PrivateKey::decode([1u8; 32].as_slice())
            .unwrap()
            .public_key();
        let lobby = ed25519::PrivateKey::decode([2u8; 32].as_slice())
            .unwrap()
            .public_key();
        let resident = ed25519::PrivateKey::decode([3u8; 32].as_slice())
            .unwrap()
            .public_key();
        // the descriptor mesh every member carries: founder + derived lobby key.
        let descriptor_mesh = vec![founder, lobby];
        // the manifest a member serves once the resident's grant has committed:
        // participants = validators, residents = the granted resident (itself).
        let participants = vec![pubkey_bytes(&[1u8; 32])]; // founder
        let residents = vec![pubkey_bytes(&[3u8; 32])]; // the resident

        let set = joiner_epoch_mesh(&descriptor_mesh, &participants, &residents);

        assert!(
            set.position(&resident).is_some(),
            "joiner dropped the manifest resident — discovery will kill the link \
             on a bit-vector length mismatch"
        );
        assert_eq!(
            set.len(),
            3,
            "every member tracks 3 (founder, lobby, resident); a shorter joiner \
             set is torn down every discovery round"
        );
    }
}

/// `init --name <human name> [--dir .] [--listen a] [--advertised a] [--http a]
/// [--rpc a] [--primary-coordinator host:port|none]
/// [--wireguard-listen a] [--invite-listen a]
/// [--duckdns-ingress a]
/// [--wireguard-effect socket|tun|fake]` — found a network: mint the
/// chain-id, write the descriptor + node config, seed the genesis validator
/// set with this identity.
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
    let primary_coordinator =
        config::primary_coordinator_or_default(flags.get("primary-coordinator").map(String::as_str))?;
    let mut plumbing = config::merged_plumbing(
        &dir,
        flags.get("listen").map(String::as_str),
        flags.get("advertised").map(String::as_str),
        flags.get("http").map(String::as_str),
        flags.get("rpc").map(String::as_str),
        flags.get("wireguard-effect").map(String::as_str),
        flags.get("wireguard-listen").map(String::as_str),
        flags.get("invite-listen").map(String::as_str),
        flags.get("duckdns-ingress").map(String::as_str),
    )?;
    if primary_coordinator.is_some() {
        if plumbing.wireguard_listen.is_none() {
            plumbing.wireguard_listen = Some("0.0.0.0:51820".into());
        }
        if !flags.contains_key("listen") {
            let port: u16 = plumbing
                .listen
                .parse::<std::net::SocketAddr>()
                .map(|a| a.port())
                .unwrap_or(0);
            if port == 0 || !plumbing.listen.starts_with('[') {
                plumbing.listen = format!("[::]:{}", if port == 0 { 52200 } else { port });
            }
        }
        if plumbing.advertised.is_none() {
            plumbing.advertised = Some("overlay".into());
        }
    }

    let (key, generated) = config::load_or_generate_identity(&dir.join("identity.key"))?;
    let me = key.public_key();
    let chain_id = config::mint_chain_id(name, &me);
    let mut descriptor = config::NetworkDescriptor {
        chain_id: chain_id.clone(),
        scheme: config::SCHEME_ED25519.into(),
        validators: vec![hex_bytes(me.as_ref())],
        bootstrap: Vec::new(),
        reach: Vec::new(),
        coordination: None,
    };
    if let Some(addr) = config::dialable(plumbing.advertised.as_deref(), &plumbing.listen)? {
        descriptor.add_bootstrap(&me, &addr);
    }
    if let Some(coord) = &primary_coordinator {
        descriptor.apply_primary_coordinator(&me, coord)?;
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

/// `invite [--config node.toml] [--ttl-days N]` — emit the one-line paste
/// blob: the whole join credential. minting IS the admission decision — the
/// blob carries the descriptor with THIS member's dial hint folded in (and
/// persisted, so every future invite carries it), the inviter's WireGuard
/// bootstrap when the reachability plane is configured (`wireguard_listen`),
/// an expiry, and a single-use INVITE TOKEN, the whole envelope signed by
/// this member's identity. the joiner's node redeems the token automatically
/// (governance `Redeem`) — no member approval step follows.
fn cmd_invite(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let ttl_days: u64 = match flags.get("ttl-days") {
        Some(v) => v
            .parse()
            .map_err(|e| format!("--ttl-days {v:?}: {e}"))?,
        None => config::DEFAULT_INVITE_TTL_DAYS,
    };
    if ttl_days == 0 {
        return Err("--ttl-days must be at least 1".into());
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
    let dial_hint = config::dialable(raw.advertised.as_deref(), &raw.listen)?;
    let has_coordinated_reach = descriptor.has_coordinated_reach()?;
    match &dial_hint {
        Some(addr) => descriptor.add_bootstrap(&key.public_key(), addr),
        // an invite must carry SOME dialable member. a member that joined via
        // an invite holds its dial hints as `reach` (bootstrap is empty), so
        // check the union, not just bootstrap — else a reachable NAT'd member
        // is wrongly refused. reachability-plane inviters are exempt when
        // they carry either a direct WireGuard bootstrap or coordinated reach.
        None if raw.wireguard_listen.is_none()
            && !has_coordinated_reach
            && descriptor
                .reach_hints()
                .map(|h| h.is_empty())
                .unwrap_or(true) =>
        {
            return Err(
                "no dialable address: give node.toml a concrete `listen` port or an \
                        `advertised` addr, or configure a primary coordinator, so a joiner can \
                        reach the network"
                    .into(),
            );
        }
        None => {}
    }
    descriptor.save(&descriptor_path)?;

    // the WireGuard bootstrap: present iff this member runs the reachability
    // plane. endpoints are minted from the advertised host (the listen IP is
    // usually unspecified) + the plane's UDP ports; the mesh port is where
    // the joiner dials this member's overlay ULA once the tunnel routes.
    let wireguard = match config::resolved_wireguard_listen(raw.wireguard_listen.as_deref())? {
        Some(wg_listen) => {
            let (wg_keypair, _) =
                reachability::WireGuardKeypair::load_or_generate(&base.join("wireguard.key"))
                    .map_err(|e| format!("wireguard key: {e}"))?;
            let mesh_port: u16 = raw
                .listen
                .parse::<std::net::SocketAddr>()
                .map(|a| a.port())
                .map_err(|e| format!("listen {:?}: {e}", raw.listen))?;
            let host =
                match config::endpoint_host(raw.advertised.as_deref(), &raw.listen, wg_listen) {
                    Ok(host) => Some(host),
                    Err(_) if has_coordinated_reach => {
                        // Coordinated reach gives the joiner a rendezvous
                        // path; there is deliberately no inviter-hosted
                        // underlay endpoint to bake into the blob.
                        None
                    }
                    Err(err) => return Err(err.into()),
                };
            match host {
                Some(host) => {
                    let intro_port =
                        config::resolved_invite_listen(raw.invite_listen.as_deref(), wg_listen)?
                            .port();
                    Some(config::InviteWireGuard {
                        public_key: wg_keypair.public_key().0,
                        endpoint: Some(format!("{host}:{}", wg_listen.port())),
                        intro: Some(format!("{host}:{intro_port}")),
                        mesh_port,
                    })
                }
                None => Some(config::InviteWireGuard {
                    public_key: wg_keypair.public_key().0,
                    endpoint: None,
                    intro: None,
                    mesh_port,
                }),
            }
        }
        None => None,
    };

    // the fronts: every reachable member the inviter already meshes with, read
    // from the persisted mesh state so a joiner can bring its tunnel up against
    // ANY of them, not just the inviter (the unified all-paths invite). A
    // host-capable member rides as a direct front, a NAT'd-but-registered one
    // as a coordinated (by-identity) front. No mesh state yet → no fronts.
    let storage = base.join(raw.storage_dir.as_deref().unwrap_or("storage"));
    let mesh_state_file = storage.join("mesh-state.json");
    let chain_id = descriptor.genesis_namespace();
    let own: [u8; 32] = key
        .public_key()
        .as_ref()
        .try_into()
        .expect("ed25519 public key is 32 bytes");
    let fronts = match reachability::store::load(&mesh_state_file, &chain_id) {
        Ok(Some(mesh)) => {
            let fronts = config::fronts_from_adverts(&mesh.adverts, &own);
            if fronts.is_empty() {
                eprintln!(
                    "[invite] persisted mesh at {} holds no other members — the invite \
                     carries only the inviter's own paths",
                    mesh_state_file.display()
                );
            }
            fronts
        }
        Ok(None) => {
            eprintln!(
                "[invite] no persisted mesh state at {} — the invite carries no member \
                 fronts (only the inviter's own paths); mint again once the mesh has peers",
                mesh_state_file.display()
            );
            Vec::new()
        }
        Err(e) => {
            eprintln!(
                "[invite] mesh state at {} unreadable ({e}) — the invite carries no member \
                 fronts",
                mesh_state_file.display()
            );
            Vec::new()
        }
    };

    // stop embedding a coordinator address in the invite: the joiner reaches
    // every path through its OWN ambient coordinator (config/default), never a
    // coordinator baked into the blob. The inviter still registers with its own
    // coordinator via its own config; here we only strip Coordinated reach
    // hints from the ENCODED copy — the on-disk descriptor keeps its config.
    let mut invite_descriptor = descriptor.clone();
    invite_descriptor
        .reach
        .retain(|hint| !hint.trim_start().starts_with("coordinated:"));

    let expires = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock is past the epoch")
        .as_secs()
        + ttl_days * 24 * 60 * 60;
    let token = config::mint_invite_token(&key, descriptor.genesis_namespace().as_bytes());
    println!(
        "{}",
        config::encode_invite(
            &invite_descriptor,
            &token,
            wireguard.as_ref(),
            &fronts,
            expires,
            &key
        )?
    );
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
    use valset::{ValsetQuery, ValsetReply, decode_reply, encode_query};
    let raw = rpc_query(addr, "valset", &encode_query(&ValsetQuery::Validators))?;
    match decode_reply(&raw)? {
        ValsetReply::Validators(v) => Ok(v),
        other => Err(format!("expected Validators, got {other:?}")),
    }
}

fn read_residents(addr: &str) -> Result<Vec<Vec<u8>>, String> {
    use valset::{ValsetQuery, ValsetReply, decode_reply, encode_query};
    let raw = rpc_query(addr, "valset", &encode_query(&ValsetQuery::Residents))?;
    match decode_reply(&raw)? {
        ValsetReply::Residents(v) => Ok(v),
        other => Err(format!("expected Residents, got {other:?}")),
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
    use upgrade::{UpgradeQuery, UpgradeReply, decode_reply, encode_query};
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
) -> Result<Option<governance::ProposalView>, String> {
    use governance::{GovQuery, GovReply, decode_reply, encode_query};
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
    mut pred: impl FnMut(&Option<governance::ProposalView>) -> bool,
) -> Result<Option<governance::ProposalView>, String> {
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

/// how a driven membership ceremony left the proposal.
enum CeremonyOutcome {
    /// passed and executed — the set changes at the next epoch cutover.
    Passed,
    /// this ballot landed but a strict majority is still outstanding.
    AwaitingBallots,
}

/// drive a strict-majority governance membership ceremony for `wanted`
/// through this member's own running node: adopt an existing OPEN proposal
/// for exactly this action (else mint an unused `<id_prefix><key>:<n>` id and
/// propose), cast a yes ballot, and execute once decidable. idempotent across
/// members — each runs the same verb; the run landing the deciding ballot
/// executes. shared by `invite-accept` (AddResident), `promote`
/// (AddValidator), and `resident-remove` (RemoveResident).
fn drive_membership_ceremony(
    rpc_addr: &str,
    me_bytes: &[u8],
    pubkey_hex: &str,
    verb: &str,
    id_prefix: &str,
    wanted: governance::GovAction,
) -> Result<CeremonyOutcome, Box<dyn std::error::Error>> {
    use governance::{GovMsg, ProposalStatus, encode_msg};
    use governance::{GovQuery, GovReply, decode_reply, encode_query};
    let proposals = match decode_reply(&rpc_query(
        rpc_addr,
        "governance",
        &encode_query(&GovQuery::Proposals),
    )?)? {
        GovReply::Proposals(views) => views,
        other => return Err(format!("unexpected governance reply: {other:?}").into()),
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
                .map(|n| format!("{id_prefix}{prefix}:{n}"))
                .find(|id| !proposals.iter().any(|p| &p.proposal_id == id))
                .expect("the id space is unbounded");
            rpc_submit(
                rpc_addr,
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
            poll_proposal(rpc_addr, &id, "the proposal to finalize", |p| p.is_some())?;
            eprintln!("proposed {id}");
            id
        }
    };

    rpc_submit(
        rpc_addr,
        "governance",
        &encode_msg(&GovMsg::Vote {
            proposal_id: proposal_id.clone(),
            approve: true,
        }),
    )?;
    let after_vote = poll_proposal(rpc_addr, &proposal_id, "this ballot to finalize", |p| {
        p.as_ref().is_some_and(|v| {
            v.status != ProposalStatus::Open
                || v.votes
                    .iter()
                    .any(|(voter, yes)| voter == me_bytes && *yes)
        })
    })?
    .expect("the poll only accepts a present proposal");
    eprintln!("ballot cast as {}", hex_bytes(me_bytes));

    // execute only when decidable — a strict-majority shortfall is the
    // normal n>=2 intermediate state, not an error.
    let members = read_members(rpc_addr)?;
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
             ducktape-node {verb} {pubkey_hex} --config <their node.toml>"
        );
        return Ok(CeremonyOutcome::AwaitingBallots);
    }
    if after_vote.status == ProposalStatus::Open {
        rpc_submit(
            rpc_addr,
            "governance",
            &encode_msg(&GovMsg::Execute {
                proposal_id: proposal_id.clone(),
            }),
        )?;
    }
    let settled = poll_proposal(rpc_addr, &proposal_id, "the tally to settle", |p| {
        p.as_ref().is_some_and(|v| v.status != ProposalStatus::Open)
    })?
    .expect("the poll only accepts a present proposal");
    match settled.status {
        ProposalStatus::Passed => Ok(CeremonyOutcome::Passed),
        status => Err(format!("proposal {proposal_id} settled as {status:?}").into()),
    }
}

/// `invite-accept <hex pubkey> [--config node.toml]` — approve a join request
/// as RESIDENT standing (the staged-admission tier): drive a governance
/// AddResident proposal for `pubkey` through this member's own RUNNING node.
/// the passing proposal's valset Grant schedules the epoch cutover that
/// admits the key to the mesh, at which point its parked node PRE-SYNCS
/// state on a stride cadence. promotion into the quorum is the separate,
/// deliberate `promote` verb — run it once the resident is warm.
fn cmd_invite_accept(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use governance::GovAction;

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
    if read_residents(&rpc_addr)?.contains(&key_bytes) {
        eprintln!(
            "{pubkey_hex} already holds resident standing — promote with \
             `ducktape-node promote {pubkey_hex}` once it is synced"
        );
        return Ok(());
    }
    if !members.contains(&me_bytes) {
        return Err(
            "this node's identity is not a current member — only members admit \
                    residents"
                .into(),
        );
    }

    match drive_membership_ceremony(
        &rpc_addr,
        &me_bytes,
        pubkey_hex,
        "invite-accept",
        "resident:",
        GovAction::AddResident { key: key_bytes },
    )? {
        CeremonyOutcome::Passed => {
            eprintln!(
                "granted resident standing to {pubkey_hex}: the mesh admits it at the next \
                 epoch cutover and its parked node pre-syncs state. promote it into the \
                 quorum once warm:\n    ducktape-node promote {pubkey_hex}"
            );
            Ok(())
        }
        CeremonyOutcome::AwaitingBallots => Ok(()),
    }
}

/// `promote <hex pubkey> [--config node.toml]` — seat a key in the consensus
/// quorum: drive a governance AddValidator proposal through this member's own
/// RUNNING node. the passing proposal's valset Join clears any resident
/// standing in the same block and schedules the epoch cutover; a pre-synced
/// resident then catches up a small delta and reboots as a validator, so the
/// quorum only ever gains a warm member. also serves DIRECT (un-staged)
/// admission — exactly the pre-resident `invite-accept` semantics.
fn cmd_promote(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use governance::GovAction;

    let (pos, flags) = parse_flags(args)?;
    let [pubkey_hex] = pos.as_slice() else {
        return Err("promote needs exactly one <hex pubkey>".into());
    };
    let key = config::decode_key(pubkey_hex)?;
    let key_bytes = key.as_ref().to_vec();
    let cfg_path = config_path(&flags)?;
    let resolved = config::resolve(&cfg_path)?;
    let rpc_addr = resolved
        .rpc_listen
        .clone()
        .ok_or("promote drives the node's local rpc — set `rpc_listen` in node.toml")?;
    let me_bytes = resolved.signer.public_key().as_ref().to_vec();

    let members = read_members(&rpc_addr)?;
    if members.contains(&key_bytes) {
        eprintln!("{pubkey_hex} is already a validator — nothing to do");
        return Ok(());
    }
    if !members.contains(&me_bytes) {
        return Err(
            "this node's identity is not a current member — only members promote \
                    validators"
                .into(),
        );
    }

    match drive_membership_ceremony(
        &rpc_addr,
        &me_bytes,
        pubkey_hex,
        "promote",
        "admit:",
        GovAction::AddValidator { key: key_bytes },
    )? {
        CeremonyOutcome::Passed => {
            eprintln!(
                "admitted {pubkey_hex} as STANDBY: the joiner's parked node will verify a \
                 state sync, announce itself online, and join the consensus quorum at the \
                 activation cutover — no quorum slot is spent until the node is actually up"
            );
            Ok(())
        }
        CeremonyOutcome::AwaitingBallots => Ok(()),
    }
}

/// `resident-remove <hex pubkey> [--config node.toml]` — revoke resident
/// standing: drive a governance RemoveResident proposal through this member's
/// own RUNNING node. the mirror of `invite-accept` with inverted guards — a
/// no-op when the key holds no resident standing, and only members may drive
/// it. the passing proposal's valset Revoke schedules the epoch cutover that
/// drops the key from the mesh; its node falls back to a parked joiner, and
/// `invite-accept` re-grants. a seated validator is `member-remove`'s job —
/// standing never overlaps (Grant refuses validators, Join clears standing).
fn cmd_resident_remove(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use governance::GovAction;

    let (pos, flags) = parse_flags(args)?;
    let [pubkey_hex] = pos.as_slice() else {
        return Err("resident-remove needs exactly one <hex pubkey>".into());
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
        .ok_or("resident-remove drives the node's local rpc — set `rpc_listen` in node.toml")?;
    let me_bytes = resolved.signer.public_key().as_ref().to_vec();

    let members = read_members(&rpc_addr)?;
    if members.contains(&key_bytes) {
        eprintln!(
            "{pubkey_hex} is a seated validator, not a resident — remove it with \
             `ducktape-node member-remove {pubkey_hex}`"
        );
        return Ok(());
    }
    if !read_residents(&rpc_addr)?.contains(&key_bytes) {
        eprintln!("{pubkey_hex} holds no resident standing — nothing to do");
        return Ok(());
    }
    if !members.contains(&me_bytes) {
        return Err(
            "this node's identity is not a current member — only members remove \
                    residents"
                .into(),
        );
    }

    match drive_membership_ceremony(
        &rpc_addr,
        &me_bytes,
        pubkey_hex,
        "resident-remove",
        "revoke:",
        GovAction::RemoveResident { key: key_bytes },
    )? {
        CeremonyOutcome::Passed => {
            eprintln!(
                "revoked resident standing from {pubkey_hex}: the mesh drops it at the next \
                 epoch cutover and its node parks again. a member re-grants with:\n    \
                 ducktape-node invite-accept {pubkey_hex}"
            );
            Ok(())
        }
        CeremonyOutcome::AwaitingBallots => Ok(()),
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
    use governance::{GovAction, GovMsg, ProposalStatus, encode_msg};

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
    use governance::{GovQuery, GovReply, decode_reply, encode_query};
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
            eprintln!("removed {pubkey_hex}: the validator set changes at the next epoch cutover");
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
/// [--rpc a] [--wireguard-listen a] [--invite-listen a]
/// [--duckdns-ingress a]
/// [--wireguard-effect socket|tun|fake]` — materialize a workspace
/// from an invite: descriptor + identity (kept across re-joins) + node
/// config. prints this identity for the inviter's pre-genesis `admit`.
fn cmd_join(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    let [blob] = pos.as_slice() else {
        return Err("join needs exactly one <invite blob>".into());
    };
    let invite = config::decode_invite(blob)?;
    let mut descriptor = invite.descriptor.clone();
    let dir = PathBuf::from(flags.get("dir").map(String::as_str).unwrap_or("."));
    std::fs::create_dir_all(&dir)?;
    config::guard_join_descriptor(&dir, &descriptor)?;
    // plumbing merges: explicit flags win, an existing node.toml's values
    // (network- or dev-shape) survive, defaults fill the rest. computed
    // BEFORE anything lands on disk so a corrupt existing node.toml aborts
    // the join without leaving a half-migrated dir. the file is ALWAYS
    // rewritten in the network shape — a join must take effect even in a dir
    // holding the app's dev-shape solo config.
    let mut plumbing = config::merged_plumbing(
        &dir,
        flags.get("listen").map(String::as_str),
        flags.get("advertised").map(String::as_str),
        flags.get("http").map(String::as_str),
        flags.get("rpc").map(String::as_str),
        flags.get("wireguard-effect").map(String::as_str),
        flags.get("wireguard-listen").map(String::as_str),
        flags.get("invite-listen").map(String::as_str),
        flags.get("duckdns-ingress").map(String::as_str),
    )?;
    if config::invite_requires_reachability_defaults(&invite) {
        // a WireGuard or Coordinated invite makes the reachability plane the
        // dial path, so the joiner's defaults change shape: its own plane
        // comes up (wireguard_listen), its mesh listens dual-stack on a
        // CONCRETE port and advertises the overlay ULA (members reverse-dial
        // it over the tunnels). explicit flags and an existing node.toml
        // still win.
        if plumbing.wireguard_listen.is_none() {
            plumbing.wireguard_listen = Some("0.0.0.0:51820".into());
        }
        if !flags.contains_key("listen") {
            let port: u16 = plumbing
                .listen
                .parse::<std::net::SocketAddr>()
                .map(|a| a.port())
                .unwrap_or(0);
            if port == 0 || !plumbing.listen.starts_with('[') {
                plumbing.listen = format!("[::]:{}", if port == 0 { 52200 } else { port });
            }
        }
        if plumbing.advertised.is_none() {
            plumbing.advertised = Some("overlay".into());
        }
        if let Some(wg) = &invite.wireguard {
            let issuer_identity =
                wireguard_upgrade::ValidatorIdentity::try_from(invite.token.issuer.as_ref())
                    .map_err(|e| format!("inviter identity: {e:?}"))?;
            let inviter_ula = wireguard_upgrade::ula_v6_member_addr(
                &descriptor.genesis_namespace(),
                issuer_identity,
            );
            descriptor.add_reach_route(&config::ReachHint {
                expected_key: invite.token.issuer.clone(),
                reach: config::Reach::Direct(format!("[{inviter_ula}]:{}", wg.mesh_port)),
            });
        }
        // every offered front gets the same overlay-ULA Direct hint the inviter
        // does: once ANY candidate's tunnel comes up, the mesh dialer can reach
        // that member's overlay ULA and ride the mesh from there. A hint whose
        // tunnel never comes up simply fails to dial — harmless.
        for front in &invite.fronts {
            let Ok(member) = ed25519::PublicKey::decode(&front.member_key[..]) else {
                continue;
            };
            let Ok(identity) = wireguard_upgrade::ValidatorIdentity::try_from(&front.member_key[..])
            else {
                continue;
            };
            let ula =
                wireguard_upgrade::ula_v6_member_addr(&descriptor.genesis_namespace(), identity);
            descriptor.add_reach_route(&config::ReachHint {
                expected_key: member,
                reach: config::Reach::Direct(format!("[{ula}]:{}", front.mesh_port)),
            });
        }
    }
    descriptor.save(&dir.join("network.toml"))?;
    let (key, generated) = config::load_or_generate_identity(&dir.join("identity.key"))?;
    let me_hex = hex_bytes(key.public_key().as_ref());
    config::write_node_toml(&dir, &plumbing)?;
    // the capability the joining node redeems automatically; a re-join with a
    // fresh invite replaces a stale/spent one.
    config::save_invite_token(&dir, &invite.token)?;
    // the offered fronts, kept beside the token so `run_node` can race the
    // whole union of first-contact paths. Empty clears any stale set.
    config::save_invite_fronts(&dir, &invite.fronts)?;
    if let Some(wg) = &invite.wireguard {
        // the tunnel bootstrap the joining node dials BEFORE any p2p; kept
        // beside the token so `run_node` can bring the interface up first.
        config::save_invite_wireguard(&dir, &invite.token.issuer, wg)?;
        // mint the WireGuard identity NOW so the run's plane and intro
        // announcer read one settled key file instead of racing to create it.
        reachability::WireGuardKeypair::load_or_generate(&dir.join("wireguard.key"))
            .map_err(|e| format!("wireguard key: {e}"))?;
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
    } else {
        eprintln!(
            "NOT yet a member. start now — `ducktape-node --config {}/node.toml` redeems \
             this invite automatically: the node joins the network's VPN, syncs state, and \
             comes up as a full node. no approval step follows (minting the invite WAS the \
             approval); a member can later promote it into the quorum with `promote {me_hex}`.",
            dir.display()
        );
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
        context.child("reachability_in").spawn(move |_ctx| async move {
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
                let Some(cmds) = intro_cmds.upgrade() else { break };
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
                let Some(cmds) = intro_cmds.upgrade() else { break };
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
        duckdns_services,
        duckdns_ingress_listen,
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
    let duckdns_announcements: Vec<_> = duckdns_services
        .iter()
        .map(|service| service.announcement.clone())
        .collect();
    let duckdns_publications = std::sync::Arc::new(duckdns_services);
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
                        let pk = ed25519::PublicKey::decode(&record.validator_identity.0[..])
                            .ok()?;
                        if bootstrappers.iter().any(|(hinted, _)| *hinted == pk) {
                            return None;
                        }
                        Some((
                            pk,
                            Ingress::Socket(record.control_endpoint.socket_addr()),
                        ))
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
    let bootstrappers: Vec<(ed25519::PublicKey, _)> = bootstrappers
        .into_iter()
        .chain(mesh_dial_seeds)
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
            format!("endpoint-less on udp port {} (roaming: peers learn this node's address from its own initiations)", wg.port())
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
    let duckdns_plane_slot: duckdns_node::plane::PlaneSlot =
        std::sync::Arc::new(std::sync::OnceLock::new());
    let duckdns_ingress_listener = match duckdns_ingress_listen {
        Some(address) if !sync_only => {
            let listener = std::net::TcpListener::bind(address)?;
            listener.set_nonblocking(true)?;
            println!(
                "[node {label}] DuckDNS HTTP ingress listening on http://{}",
                listener.local_addr()?
            );
            Some(listener)
        }
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
    let agent_provisioner: Option<dispatch_oracle::SharedProvisioner> =
        Some(std::sync::Arc::new(noded::agent_provision::NodedProvisioner::new(
            http_handle.clone(),
            noded::agent_provision::agent_runs_root(&storage)
                .unwrap_or_else(|e| panic!("agent runs root failed D7 validation: {e}")),
        )));
    // Reuse noded's established no-self-dial files adapter for DuckFS-backed
    // DuckDNS sites. It holds only a clone of the actor command lane.
    let duckdns_files = noded::ActorNodeApi::new(http_handle.clone());
    if let Some(listener) = duckdns_ingress_listener {
        let commands = http_handle.command_sender();
        let plane = std::sync::Arc::clone(&duckdns_plane_slot);
        let publications = std::sync::Arc::clone(&duckdns_publications);
        let files = duckdns_files.clone();
        let me: [u8; 32] = signer
            .public_key()
            .as_ref()
            .try_into()
            .expect("ed25519 keys are 32 bytes");
        let thread_label = label.clone();
        std::thread::Builder::new()
            .name("duckdns-ingress".into())
            .spawn(move || {
                tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("DuckDNS ingress tokio runtime")
                    .block_on(async move {
                        if let Err(error) =
                            duckdns_node::ingress::serve(
                                listener,
                                commands,
                                plane,
                                me,
                                publications,
                                files,
                            )
                            .await
                        {
                            eprintln!("[node {thread_label}] DuckDNS ingress failed: {error}");
                        }
                    });
            })?;
    }
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
            let mut resident_duckdns_plane_book: Option<
                std::sync::Arc<duckdns_node::plane::WebPeers>,
            > = None;
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
                // A resident is a real DuckDNS requester/provider. Bring its
                // web plane up once standing appears, and refresh admission on
                // every later tip snapshot so revocation cuts inbound streams.
                let duckdns_transport_keys: Vec<ed25519::PublicKey> = tip
                    .participants
                    .iter()
                    .chain(tip.residents.iter())
                    .filter_map(|key| ed25519::PublicKey::decode(key.as_slice()).ok())
                    .collect();
                if let Some(book) = &resident_duckdns_plane_book {
                    book.set_peers(duckdns_transport_keys.iter());
                } else if wireguard_listen.is_some()
                    && tip.residents.iter().any(|key| key == &me_bytes)
                {
                    let book = duckdns_node::plane::WebPeers::new(
                        String::from_utf8(namespace.clone()).expect("namespace is utf-8"),
                    );
                    book.set_peers(duckdns_transport_keys.iter());
                    duckdns_node::plane::spawn_bring_up(
                        label.clone(),
                        std::sync::Arc::clone(&book),
                        signer.public_key(),
                        std::sync::Arc::clone(&duckdns_plane_slot),
                        statesync_plane::socket_factory(wireguard_effect, &overlay_slot),
                        std::sync::Arc::clone(&duckdns_publications),
                        duckdns_files.clone(),
                    );
                    resident_duckdns_plane_book = Some(book);
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
        // DuckDNS's per-use stream plane. Unlike statesync this is not env
        // gated: a configured overlay is the web transport. The slot is kept
        // for the requester-side local ingress; the accept loop always runs so
        // this node can provide its explicit local publications.
        let duckdns_plane_book = wireguard_listen.map(|_| {
            let book = duckdns_node::plane::WebPeers::new(
                String::from_utf8(namespace.clone()).expect("namespace is utf-8"),
            );
            book.set_peers(initial_member_keys.iter().chain(initial_resident_keys.iter()));
            duckdns_node::plane::spawn_bring_up(
                label.clone(),
                std::sync::Arc::clone(&book),
                signer.public_key(),
                std::sync::Arc::clone(&duckdns_plane_slot),
                statesync_plane::socket_factory(wireguard_effect, &overlay_slot),
                std::sync::Arc::clone(&duckdns_publications),
                duckdns_files.clone(),
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
            context
                .child("statesync_serve")
                .spawn(move |_ctx| async move {
                    let mut server = SyncServer::new();
                    while let Some(job) = ingress.next().await {
                        // both carriers land here: mesh frames ride an rpc
                        // envelope (multiplexed channel — the id correlates);
                        // a plane stream IS its own correlation and reply path.
                        let (reply_to, rpc_id, body) = match job {
                            statesync_plane::SyncJob::Mesh(peer, bytes) => {
                                let Ok((rpc_id, body)) = statesync::decode_rpc(&bytes) else {
                                    continue; // malformed rpc envelope: drop, never crash.
                                };
                                (
                                    statesync_plane::SyncReplyTo::Mesh(peer),
                                    rpc_id,
                                    body.to_vec(),
                                )
                            }
                            statesync_plane::SyncJob::Plane(stream, req) => {
                                (statesync_plane::SyncReplyTo::Plane(stream), 0, req)
                            }
                        };
                        let resp = match statesync::decode_request(&body) {
                            Ok(req) => drive_sync_request(&mut server, &state_tx, req).await,
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
                            if let Some(book) = &duckdns_plane_book {
                                book.set_peers(plan.valset().transport_members().iter());
                            }
                            // the media planes authenticate inbound by the same
                            // tracked set — follow the re-track too, so a
                            // just-added member's huddle media is admitted.
                            if let Some(peers) = &media_peers {
                                peers.set_peers(plan.valset().transport_members().iter());
                            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use sdk::{Ctx, Error, Module, StateSyncHandle};
    use std::sync::{Arc, Mutex};
    use upgrade::{ScheduledUpgrade, UpgradeStatus};

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
            residents: vec![],
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
                target: "mem".into(),
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
            "mem".into()
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
                manifest.snapshot("mem").expect("mem snapshot"),
                manifest.root("mem").expect("mem root"),
            )
            .expect("mem install");
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

    #[test]
    fn resident_manifest_fetch_retry_stays_resident_and_does_not_reannounce() {
        let retry = joiner_manifest_fetch_retry(
            "9f7bae44",
            true,
            "server error: no finalized boundary to serve yet",
        );

        assert!(
            !retry.announce,
            "a resident should not re-announce the invite after standing is known"
        );
        assert!(
            retry.log_line.contains("[node 9f7bae44] resident:"),
            "post-standing retry should be logged as resident follow noise: {}",
            retry.log_line
        );
        assert!(
            retry
                .log_line
                .contains("no finalized boundary to serve yet"),
            "the source fetch detail should remain visible: {}",
            retry.log_line
        );
        assert!(
            !retry.log_line.contains("redemption not landed")
                && !retry.log_line.contains("joining:"),
            "post-standing retry must not look like a pending invite: {}",
            retry.log_line
        );
    }

    #[test]
    fn parked_manifest_fetch_retry_keeps_join_announce() {
        let retry =
            joiner_manifest_fetch_retry("9f7bae44", false, "server error: bouncer rejected");

        assert!(retry.announce, "a parked joiner must keep re-announcing");
        assert!(
            retry.log_line.contains("[node 9f7bae44] joining:")
                && retry.log_line.contains("redemption not landed")
                && retry.log_line.contains("bouncer rejected"),
            "parked retry should keep the invite wording and source detail: {}",
            retry.log_line
        );
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
        // a block is a BATCH super-frame: apply the single member via the batch
        // API and serve the batch bytes, so the catch-up replay reproduces this.
        expected
            .submit_block(
                host::BlockContext {
                    protocol_version: host::BASELINE_VERSION,
                    height,
                    consensus_time: height,
                    origin: origin.clone(),
                },
                vec![(origin, msg)],
            )
            .await
            .expect("apply");
        statesync::FinalizedFrame {
            height,
            frame: node::encode_batch(&[frame]),
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
        // a block is a BATCH super-frame: apply the single member through the
        // batch API so the served app-hash matches what recovery reproduces on
        // replay (which decodes the frame as a batch), and serve the batch bytes.
        expected
            .submit_block(
                host::BlockContext {
                    protocol_version: host::BASELINE_VERSION,
                    height,
                    consensus_time: height,
                    origin: origin.clone(),
                },
                vec![(origin, msg)],
            )
            .await
            .expect("apply mixed frame");
        statesync::FinalizedFrame {
            height,
            frame: node::encode_batch(&[frame]),
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
            let applied = apply_post_reboot_catchup_frames(
                &mut recovery,
                &mut host,
                0,
                2,
                frames.clone(),
                None,
            )
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
                vec![],
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
                residents: vec![],
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
            let applied = apply_post_reboot_catchup_frames(
                &mut recovery,
                &mut host,
                0,
                1,
                vec![served],
                None,
            )
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
            assert_eq!(ckpt.snapshot("mem"), Some([7u8].as_slice()));

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
            let err = apply_post_reboot_catchup_frames(
                &mut recovery,
                &mut host,
                0,
                1,
                vec![served],
                None,
            )
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
            pending: pending.map(|(name, activation_height, to_version)| ScheduledUpgrade {
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
        let st_ready = status(
            Some(("forge-v2", 100, MAX_PROTOCOL_VERSION)),
            &[&me],
            &[&me],
        );
        assert_eq!(s.decide(&st_ready), None, "module already holds our signal");
    }

    #[test]
    fn readiness_signaller_silent_when_under_versioned() {
        let me = vec![7u8; 32];
        let mut s = ReadinessSignaller::new(MAX_PROTOCOL_VERSION, me.clone());
        // to_version beyond what this binary can execute: never lie about readiness.
        let st = status(
            Some(("forge-v3", 100, MAX_PROTOCOL_VERSION + 1)),
            &[&me],
            &[],
        );
        assert_eq!(s.decide(&st), None);
    }

    #[test]
    fn readiness_signaller_silent_when_not_a_member() {
        let me = vec![7u8; 32];
        let other = vec![9u8; 32];
        let mut s = ReadinessSignaller::new(MAX_PROTOCOL_VERSION, me);
        // self is not in the boundary member set (not in the R = n denominator).
        let st = status(
            Some(("forge-v2", 100, MAX_PROTOCOL_VERSION)),
            &[&other],
            &[],
        );
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
        assert!(
            sdk::check_required_version(MAX_PROTOCOL_VERSION + 1, MAX_PROTOCOL_VERSION).is_err()
        );
        // exactly at the build ceiling, and below it, boots.
        assert!(sdk::check_required_version(MAX_PROTOCOL_VERSION, MAX_PROTOCOL_VERSION).is_ok());
        if MAX_PROTOCOL_VERSION > 0 {
            assert!(
                sdk::check_required_version(MAX_PROTOCOL_VERSION - 1, MAX_PROTOCOL_VERSION).is_ok()
            );
        }
    }

    // ---- explorer-row rebuild (boot fold == live drain) ---------------------

    fn row_dispatches(payload: &[u8], origin: &sdk::Origin) -> Vec<host::DispatchRecord> {
        vec![host::DispatchRecord {
            module: "directory".into(),
            origin: origin.clone(),
            payload: payload.to_vec(),
            emitted_msgs: 0,
            emitted_events: 0,
        }]
    }

    /// the boot fold rebuilds a block's per-op rows from its sealed BATCH frame,
    /// re-staging each op's payload so `GET /v1/files/blob/{op_hash}` answers
    /// again after a restart. the block coordinates and every op's identity
    /// (proposer/target/payload/opHash) match the drain's live row; the only
    /// difference is the per-op dispatch TRACE — recovery folds the block-level
    /// aggregate, not per-member, so a replayed op carries an empty trace (a
    /// documented degradation visible only when the index is rebuilt).
    #[test]
    fn boot_fold_rebuilds_a_batch_block_ops() {
        let signer = ed25519::PrivateKey::from_seed(42);
        let payload = br#"{"set":{"key":"who","value":"ducktape"}}"#.to_vec();
        let msg = Msg {
            target: "directory".into(),
            payload: payload.clone(),
        };
        let frame = node::encode_frame(&signer, 1, &msg);
        let (origin, decoded) = node::decode_frame(&frame).expect("frame decodes");
        let dispatches = row_dispatches(&payload, &origin);
        let app_hash = test_root(9);

        // the drain's construction: one member op with its full dispatch trace.
        let drain_blobs = blobstore::BlobHandle::default();
        let drain_row = noded::block_row(&noded::BlockRecord {
            height: 7,
            hash: noded::hex_bytes(&node::frame_id(&frame)),
            commit_hash: hex(&app_hash),
            ops: vec![explorer_root_op(
                &drain_blobs,
                &origin,
                &decoded.target,
                &decoded.payload,
                &dispatches,
                noded::BlockDisposition::Applied,
            )],
        });
        let drain: serde_json::Value = serde_json::from_slice(&drain_row).unwrap();

        // the boot fold's construction: the sealed frame is a BATCH.
        let batch = node::encode_batch(std::slice::from_ref(&frame));
        let fold_blobs = blobstore::BlobHandle::default();
        let fold_row = sealed_frame_block_row(
            &fold_blobs,
            &recovery::FoldedBlock {
                height: 7,
                frame: &batch,
                disposition: node::Disposition::Applied,
                app_hash,
                dispatches: &dispatches,
            },
        )
        .expect("an applied non-nop batch rebuilds its row");
        let row: serde_json::Value = serde_json::from_slice(&fold_row).expect("row json");

        // block coordinates match the drain.
        assert_eq!(row["height"], 7);
        assert_eq!(row["hash"], noded::hex_bytes(&node::frame_id(&batch)));
        assert_eq!(row["commitHash"], hex(&app_hash));
        assert_eq!(row["ops"].as_array().unwrap().len(), 1);
        // the op's identity matches the drain byte-for-byte.
        assert_eq!(row["ops"][0]["proposer"], drain["ops"][0]["proposer"]);
        assert_eq!(row["ops"][0]["target"], "directory");
        assert_eq!(row["ops"][0]["payload"], drain["ops"][0]["payload"]);
        assert_eq!(row["ops"][0]["opHash"], drain["ops"][0]["opHash"]);
        // the fold carries an empty per-op trace (recovery folds the aggregate).
        assert_eq!(row["ops"][0]["operations"].as_array().unwrap().len(), 0);

        // the rebuild re-staged the payload: op_hash is dereferencable again
        // from the FOLD's (fresh, post-restart) blob store.
        let op_digest = drain_blobs.put_chunk(payload.clone());
        assert_eq!(row["ops"][0]["opHash"], noded::hex_bytes(&op_digest));
        assert!(fold_blobs.has_chunk(&op_digest));
    }

    /// the fold's `None` gates mirror the drain's: a heartbeat nop and an
    /// undecodable frame produce no explorer row (the drain's `op` is `None` /
    /// nop-filtered for exactly these).
    #[test]
    fn boot_fold_skips_nop_and_undecodable_frames() {
        let blobs = blobstore::BlobHandle::default();
        let signer = ed25519::PrivateKey::from_seed(43);
        let nop = node::encode_frame(
            &signer,
            1,
            &Msg {
                target: NOP_TARGET.into(),
                payload: Vec::new(),
            },
        );
        for frame in [nop.as_slice(), b"not a frame".as_slice()] {
            assert!(
                sealed_frame_block_row(
                    &blobs,
                    &recovery::FoldedBlock {
                        height: 3,
                        frame,
                        disposition: node::Disposition::Applied,
                        app_hash: test_root(1),
                        dispatches: &[],
                    },
                )
                .is_none()
            );
        }
    }

    /// a REJECTED sealed frame still gets its row (the drain writes one for a
    /// decoded non-nop reject), with an empty dispatch trace.
    #[test]
    fn boot_fold_rebuilds_rejected_rows_with_empty_trace() {
        let blobs = blobstore::BlobHandle::default();
        let signer = ed25519::PrivateKey::from_seed(44);
        let frame = node::encode_frame(
            &signer,
            2,
            &Msg {
                target: "directory".into(),
                payload: b"garbage-the-module-rejects".to_vec(),
            },
        );
        let batch = node::encode_batch(&[frame]);
        let row = sealed_frame_block_row(
            &blobs,
            &recovery::FoldedBlock {
                height: 5,
                frame: &batch,
                disposition: node::Disposition::Rejected,
                app_hash: test_root(2),
                dispatches: &[],
            },
        )
        .expect("a decoded non-nop reject still shows in the explorer");
        let row: serde_json::Value = serde_json::from_slice(&row).expect("row json");
        assert_eq!(row["ops"][0]["disposition"], "rejected");
        assert_eq!(row["ops"][0]["operations"].as_array().map(Vec::len), Some(0));
    }
}
