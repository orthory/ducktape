use std::time::Duration;

// the consensus signature scheme is V1 ed25519, the only wired variant — see
// `consensus::ConsensusScheme`'s rekey/respawn contract for what a scheme
// migration would take (an epoch teardown-respawn, not a constant flip).
/// the highest protocol version THIS binary's dual-path modules can execute — a
/// per-node BUILD constant, NEVER consensus state (a lying value can only
/// refuse-to-boot or halt this one node, never fork the network). the
/// `ReadinessSignaller` truthfully signals readiness for a pending upgrade iff
/// `MAX_PROTOCOL_VERSION >= to_version`, and the boot preflight refuses a boundary
/// whose `required_min_version` exceeds it. Phase 9 raised this to 2 when the
/// forge v2 dual path landed; the staged-admission resident tier raised it to
/// 3 — this binary can execute a scheduled `to_version=3` (valset/governance
/// resident ops, gated below 3) and truthfully `SignalReady`.
pub(crate) const MAX_PROTOCOL_VERSION: u32 = 3;
/// the peer-set index a node WITHOUT consensus coordinates tracks (a parked
/// joiner, a sync-only resident): the genesis mesh at index 0. a VALIDATOR
/// tracks its epoch's mesh at index = epoch instead — discovery requires
/// strictly increasing indexes per `track`, ignores indexes a peer does not
/// know, but KILLS a peer whose set at a SHARED index has a different
/// length, so the set tracked at a given index must be identical on every
/// node that tracks it (epoch participant sets are; instantaneous valset
/// projections are not).
pub(crate) const PEER_SET: u64 = 0;
/// a deliberately-unregistered module target. validators submit empty frames
/// against it to advance the chain when there are no user ops: finalized views
/// only move with ops, so an idle network would otherwise freeze — parking AT an
/// armed cutover boundary forever, and never ticking the height the console
/// shows. the frame finalizes, rejects deterministically on every node (unknown
/// module), advances the engine clock, and leaves no state.
pub(crate) const NOP_TARGET: &str = "consensus.nop";
// block cadence is a single knob: `consensus::BLOCK_TIME` (1s). the idle
// heartbeat beats one nop block per BLOCK_TIME so an idle chain still finalizes
// (its height keeps ticking) and any pending cutover still crosses — paced to
// the same interval the leader's idle-propose holds a view open, so the beat
// never outpaces the intended block time.
/// request timeout for the promoted-validator boot catch-up client. it is long
/// enough to let discovery links warm, but bounded so boot cannot hang forever
/// before the statesync server bridge is installed.
pub(crate) const BOOT_SYNC_REQUEST_TIMEOUT: Duration = Duration::from_secs(6);
/// how often the reachability plane re-offers whatever it still waits on —
/// un-acked gossip while assembling, stalled handshake messages after
/// verification (`ReachabilityCommand::Nudge`). fast enough that a lost
/// message costs one beat of mesh convergence, slow enough to be noise-free
/// — the nudge is a no-op once the epoch's handshakes have all completed.
pub(crate) const NUDGE_INTERVAL: Duration = Duration::from_secs(2);
/// post-reboot catch-up should close the reboot gap, not chase a live chain
/// forever. any tiny lag left after this cap is handled by the normal engine.
pub(crate) const POST_REBOOT_CATCHUP_MAX_ITERS: usize = 8;
/// how many times the promoted-validator boot retries an unavailable catch-up
/// source before failing over to the supervisor. sized (with the escalating
/// beat at the retry site) to ride out an overlay-only source whose tunnels
/// are still assembling after the reboot — several minutes, not seconds: an
/// exec-restart would only redo the plane restore from zero.
pub(crate) const POST_REBOOT_CATCHUP_MAX_ATTEMPTS: usize = 60;
/// max wire message size we accept on a channel (2 MiB). the tallest honest
/// messages are (a) an op frame carrying a full 1 MiB duckfs chunk — capped at
/// `node::MAX_FRAME_BYTES` by the submit-boundary guard, then gossiped raw on
/// the payload channel and served (plus a small rpc envelope) on the fetch and
/// statesync lanes — and (b) a `GetObjects` sync reply page, capped at
/// `duckfs_core::MAX_SYNC_REPLY_BYTES` (base64 wraps each 1 MiB object ~4/3x). the
/// asserts below pin both caps under this one, envelope headroom included:
/// commonware's sender ASSERTS on this cap, so "over" is a panic, not an error.
pub(crate) const MAX_MESSAGE_SIZE: u32 = 1 << 21;
const _: () = assert!(MAX_MESSAGE_SIZE as usize >= node::MAX_FRAME_BYTES + 1024);
const _: () = assert!(MAX_MESSAGE_SIZE as usize >= duckfs_core::MAX_SYNC_REPLY_BYTES + 1024);
/// inbound backlog before a channel applies receive backpressure.
pub(crate) const MAX_BACKLOG: usize = 128;
/// pump drain cadence: how often the pump applies finalized frames (and runs
/// everything that rides the drain arm — checkpoints, valset orchestration,
/// the epoch cutover, the heartbeat). enforced as a FLOOR via an absolute
/// deadline in the pump loop: ingress load can delay one drain by one
/// request's service time, but can never starve the arm.
pub(crate) const DRAIN_TICK: Duration = Duration::from_millis(100);
/// the submit-relay channel: a resident-standing node ships a frame it
/// SIGNED (its own identity key is the frame origin — authorship) to one
/// current validator, which takes consensus custody (`submit_frame`) and
/// answers with the frame's fate when it drains. the last free static slot
/// below CHANNEL_STATE_SYNC; engine banks start at 9 (statics run 3–6).
/// registered in EVERY
/// mode like the lanes above — validators serve, residents speak, sync-only
/// black-holes.
pub(crate) const CHANNEL_SUBMIT_RELAY: u64 = 3;
/// the statesync rpc channel: joiners request manifests / snapshot chunks /
/// qmdb op-ranges here; validators answer between drains.
pub(crate) const CHANNEL_STATE_SYNC: u64 = 4;
/// the lobby channel: a not-yet-admitted joiner (connected as the derived
/// lobby identity) announces `{invite token, pubkey, proof}` here; members
/// verify and RECORD the join request for manual approval, and answer with an
/// informational reply. see the `lobby` module.
pub(crate) const CHANNEL_LOBBY: u64 = 5;
/// the reachability channel: members gossip WireGuard endpoint records and
/// signed advertisements and run the tunnel-upgrade handshake here (the
/// `reachability` crate's staged node-driven WireGuard plane). registered in
/// EVERY mode — an unregistered channel is a protocol violation that kills
/// the sender's connection — and black-holed where the plane does not run.
pub(crate) const CHANNEL_REACHABILITY: u64 = 6;
/// while parked and un-admitted, re-announce every N park-loop attempts
/// (attempts tick ~2s apart, so this is roughly every 10s) — often enough to
/// survive member restarts (the request queue is in-memory), quiet enough to
/// stay out of the members' way.
pub(crate) const LOBBY_ANNOUNCE_EVERY: usize = 5;
/// the park loop's poll cadence while the joiner still knocks for standing or
/// has no served boundary yet: fast, because this tick is all that paces the
/// first sync and the `LOBBY_ANNOUNCE_EVERY` knock counter.
pub(crate) const JOINER_POLL: Duration = Duration::from_secs(2);
/// a standing, SERVING resident's fallback poll. head-following is wake-driven
/// (cert-lane traffic nudges the park loop the moment a boundary seals), so
/// this tick only covers a missed wake — a mesh hiccup swallowing a
/// certificate burst, or an idle stretch with nothing to follow.
pub(crate) const RESIDENT_FALLBACK_POLL: Duration = Duration::from_secs(12);
/// how many epochs of engine channels are PRE-REGISTERED. discovery channels
/// can only be registered before `network.start()`, and every epoch's respawned
/// engine needs FRESH channels (an aborted old engine must never collide with
/// its successor) — so a bank is reserved up front. exhausting it is a
/// fail-stop: restart the mesh with a wider bank (a config/build constant, not
/// consensus state).
pub(crate) const EPOCH_CHANNEL_BANK: u64 = 16;
/// finalized views between OBSERVING a membership change and CUTTING OVER —
/// the grace window in which every honest node sees the same change and arms
/// the same deterministic discard ceiling. small for the demo network; a
/// production mesh would size this in minutes of views.
pub(crate) const CUTOVER_DELAY: u64 = 3;
/// every module in the production genesis set, in status-report order. pinned
/// to the `host_state::ProductionModules` registry by the parity test in
/// `host_state` — status endpoints report exactly these roots.
///
/// A module here is in the app-hash: every node must run it, agree on its root
/// at every height, and keep doing so forever (the height-gated upgrade path
/// flips `protocol_version` only — it cannot change the module SET). Experiments
/// therefore live unwired in `crates/labs` and appear in no genesis set.
pub(crate) const MODULE_IDS: [&str; 25] = [
    "kv",
    "pages",
    "chat",
    "forge",
    "valset",
    "governance",
    "upgrade",
    "modreg",
    "hello",
    "saga",
    "capability",
    "dispatch",
    "tagging",
    "tasks",
    "vaults",
    "identity",
    "duckdns",
    "gateway",
    "inbox",
    "directory",
    "automations",
    "files",
    "jobs",
    "agent",
    "runs",
];

/// Canonical committed-state revisions for the production module set.
///
/// Keep this alphabetically ordered and bump a module's revision in the same
/// change that alters its canonical snapshot/root encoding. The registry
/// parity test compares these declarations with the live module trait values.
pub(crate) const MODULE_STATE_SCHEMAS: [(&str, u32); 25] = [
    ("agent", 1),
    ("automations", 1),
    ("capability", 1),
    ("chat", 1),
    ("directory", 1),
    ("dispatch", 1),
    ("duckdns", 1),
    ("files", 1),
    ("forge", 1),
    ("gateway", 1),
    ("governance", 1),
    ("hello", 1),
    ("identity", 1),
    ("inbox", 2),
    ("jobs", 2),
    ("kv", 1),
    ("modreg", 1),
    ("pages", 1),
    ("runs", 2),
    ("saga", 1),
    ("tagging", 1),
    ("tasks", 2),
    ("upgrade", 1),
    ("valset", 1),
    // 2: the wasm adapter port — the native canonical snapshot persisted as
    // one host-KV value, so root()/snapshot bytes changed shape at cutover.
    ("vaults", 2),
];

pub(crate) fn current_state_schema_fingerprint() -> [u8; 32] {
    host::state_schema_fingerprint(MODULE_STATE_SCHEMAS.iter().copied())
}

/// how long an app-surface submit reply may be held awaiting finalization
/// before it errors out (the op may still land later; clients re-query on
/// block events). mirrors the rpc bridge's stuck-node budget.
pub(crate) const SUBMIT_HOLD: Duration = Duration::from_secs(10);

/// the five channels epoch `e`'s engine uses: vote, certificate, resolver, the
/// eager payload-relay lane, and the payload FETCH lane (the lazy catch-up
/// backstop — a validator that missed the one-shot relay gossip for a
/// finalized op fetches its bytes by digest instead of wedging its apply
/// prefix forever). starts at 9, clear of the fixed discovery channels
/// (statesync 4, lobby 5, reachability 6).
pub(crate) fn engine_channels(epoch: u64) -> (u64, u64, u64, u64, u64) {
    let base = 9 + epoch * 5;
    (base, base + 1, base + 2, base + 3, base + 4)
}
