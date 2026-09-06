use std::time::Duration;

// the consensus signature scheme is ed25519 — see the rekey/respawn contract
// in `crates/kernel/consensus/src/lib.rs` for a scheme change (an epoch
// teardown-respawn, not a constant flip).
/// the module-code fetch cap: the largest content-addressed code artifact (a
/// wasm component today, a quack capsule tomorrow) this node will pull over
/// the ranged blob lane or accept on the code plane. a policy bound, not a
/// frame size — transfers are ranged/streamed, so no single message ever
/// approaches it.
pub(crate) const MAX_MODULE_CODE_BYTES: u64 = 1024 * 1024 * 1024;
/// how many source conversations a code-blob fetch tries before reporting
/// the miss (each conversation resumes the staged prefix, so retries only
/// ever pay for bytes not yet landed).
pub(crate) const BLOB_FETCH_ATTEMPTS: usize = 3;
/// a deliberately-unregistered module target. validators submit empty frames
/// against it to advance the chain when there are no user ops: finalized views
/// only move with ops, so an idle network would otherwise freeze — parking AT an
/// armed cutover boundary forever, and never ticking the height the console
/// shows. the frame finalizes, rejects deterministically on every node (unknown
/// module), advances the engine clock, and leaves no state.
// single source: the block projection filters this exact target to hide nop-only
// blocks, and this heartbeat must submit the same string — so the value lives in
// `noded::projection` and both sides read it here.
pub(crate) const NOP_TARGET: &str = noded::projection::NOP_TARGET;
// block cadence: BUSY is event-driven with NO interval — the run loop flushes
// pending ops the moment nothing of ours is in flight (`pump_eager_flush`),
// so the network's own agreement speed paces blocks and ops aggregate behind
// the one batch in flight. IDLE is the only timed cadence: the heartbeat
// beats one nop block per block time (network.toml `block_time_ms`) so an idle
// chain still finalizes (its height keeps ticking) and any pending cutover
// still crosses — paced to the same interval the leader's idle-propose holds
// a view open, so the idle beat never outpaces the view hold.
/// how often the reachability plane re-offers whatever it still waits on —
/// un-acked gossip while assembling, stalled handshake messages after
/// verification (`ReachabilityCommand::Nudge`). fast enough that a lost
/// message costs one beat of mesh convergence, slow enough to be noise-free
/// — the nudge is a no-op once the epoch's handshakes have all completed.
pub(crate) const NUDGE_INTERVAL: Duration = Duration::from_secs(2);
/// suffix catch-up should close one boundary gap, not chase a live chain
/// forever. any tiny lag left after this cap is handled by the follower.
pub(crate) const SUFFIX_CATCHUP_MAX_ITERS: usize = 8;
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
// (c) a qmdb module's op-batch reply (`SyncResponse::Module`), trimmed to
// `statesync::qmdb::MAX_MODULE_REPLY_BYTES` by the serve side. the serve task
// additionally refuses ANY over-cap response with an `Error` reply
// (`sync::serve::encode_bounded_response`) — that is the last line; this pin
// keeps the honest path from ever needing it.
const _: () = assert!(MAX_MESSAGE_SIZE as usize >= statesync::qmdb::MAX_MODULE_REPLY_BYTES + 1024);
/// inbound backlog per channel. NOT backpressure: commonware's peer actor
/// DROPS an inbound message when the application buffer is full (it never
/// blocks a peer), so this is a drop boundary — `relay::MAX_RELAY_BLOB_BYTES`
/// is pinned so one offer plus every chunk of a max-size pack fits inside it.
pub(crate) const MAX_BACKLOG: usize = 128;
/// per-read/write deadline for every mesh socket — the OS arm gets it via
/// `with_read_write_timeout` at boot, and it IS the overlay seam's own
/// `IO_TIMEOUT` (aliased, not copied, so the arms cannot drift). see the
/// seam const's doc for the full rationale: this deadline is the only
/// half-open-connection detector on the block-delivery path, and a slept /
/// roamed / NAT-rebound laptop freezes for exactly this long before the
/// dialer heals the mesh.
pub(crate) const MESH_IO_TIMEOUT: Duration = overlay_net::userspace::seam::IO_TIMEOUT;
/// pump drain cadence: how often the pump runs the drain arm — checkpoints,
/// valset orchestration, the epoch cutover, the heartbeat — when no
/// finalization delivery wake turned the loop first. enforced as a FLOOR via
/// an absolute deadline in the pump loop: ingress load can delay one drain by
/// one request's service time, but can never starve the arm. it is a BACKSTOP
/// for block handling, not a pacer: finalized blocks drain (and pending ops
/// flush) event-driven the moment they land.
pub(crate) const DRAIN_TICK: Duration = Duration::from_millis(100);
/// the pace of `refresh_operations` (the /metrics exposition parse feeding
/// status' consensus/storage sections): the status cell publishes boundary
/// facts per drain pass, but the exposition parse is the pricey part and one
/// per second bounds its cost — and the staleness — at once.
pub(crate) const OPS_REFRESH_INTERVAL: Duration = Duration::from_secs(1);
/// how often the drain re-checks that this node's workspace directory is still
/// the one it booted on (`WorkspaceMark`). One `stat` per second is free next
/// to a block, and a second is far inside the window between a workspace being
/// deleted and the next journal write panicking somewhere in consensus.
pub(crate) const WORKSPACE_CHECK_INTERVAL: Duration = Duration::from_secs(1);
/// the submit-relay channel: a resident-standing node ships a frame it
/// SIGNED (its own identity key is the frame origin — authorship) to one
/// current validator, which takes consensus custody (`submit_frame`) and
/// answers with the frame's fate when it drains. the static mesh lanes run
/// 3–5; engine banks start right after them. registered in EVERY mode like
/// the lanes above — validators serve, residents speak, sync-only
/// black-holes.
pub(crate) const CHANNEL_SUBMIT_RELAY: u64 = 3;
/// the statesync rpc channel: joiners request manifests / snapshot chunks /
/// qmdb op-ranges here; validators answer between drains.
pub(crate) const CHANNEL_STATE_SYNC: u64 = 4;
/// the reachability channel: members gossip WireGuard endpoint records and
/// signed advertisements and run the tunnel-upgrade handshake here (the
/// `reachability` crate's staged node-driven WireGuard plane). registered in
/// EVERY mode — an unregistered channel is a protocol violation that kills
/// the sender's connection — and black-holed where the plane does not run.
pub(crate) const CHANNEL_REACHABILITY: u64 = 5;
/// the park loop's poll cadence while the joiner has standing but no served
/// boundary yet, and the join gate's per-candidate re-send tick:
/// fast, because this tick paces the first sync and the gate's warm-up resend.
pub(crate) const JOINER_POLL: Duration = Duration::from_secs(2);
/// a standing, SERVING resident's fallback poll. head-following is wake-driven
/// (cert-lane traffic nudges the park loop the moment a boundary seals), so
/// this tick only covers a missed wake — a mesh hiccup swallowing a
/// certificate burst, or an idle stretch with nothing to follow.
pub(crate) const RESIDENT_FALLBACK_POLL: Duration = Duration::from_secs(12);
/// how many epochs of engine channels are PRE-REGISTERED. mesh channels
/// can only be registered before `network.start()`, and every epoch's respawned
/// engine needs FRESH channels (an aborted old engine must never collide with
/// its successor) — so a bank is reserved up front. the bank bounds membership
/// changes per process RUN, not per network lifetime: a restart re-banks from
/// the checkpoint epoch, and the systemd unit's `Restart=always`
/// (`ops/node/ducktape-node@.service`) is the recovery for the fail-stop exit.
///
/// the cost is registrations held open for the life of the process: five per
/// slot on a validator (`validator/wiring.rs`, vote/certificate/resolver/
/// payload/fetch), five per slot again for the parked replica bank
/// (`replica/wiring.rs`) and for the sync-only joiner's blackholes
/// (`boot/sync_only.rs`) — so 64 slots is 320 channels per role.
pub(crate) const EPOCH_CHANNEL_BANK: u64 = 64;
/// finalized views between OBSERVING a membership change and CUTTING OVER —
/// the grace window in which every honest node sees the same change and arms
/// the same deterministic discard ceiling. small for the demo network; a
/// production mesh would size this in minutes of views.
pub(crate) const CUTOVER_DELAY: u64 = 3;
/// how long an app-surface submit reply may be held awaiting finalization
/// before it errors out (the op may still land later; clients re-query on
/// block events). mirrors the rpc bridge's stuck-node budget.
pub(crate) const SUBMIT_HOLD: Duration = Duration::from_secs(10);

/// the join gate's settle budget: a gating member holds the joiner's
/// pending `Admitted`/`Rejected` outcome against its submitted `Redeem` frame
/// for this long. if the frame has not drained by then the member writes
/// `Rejected{ Busy, terminal: false }` into the gate-outcome map and the joiner
/// fails over to another member. wider than `SUBMIT_HOLD` because a fresh
/// joiner's first block can wait on mesh warm-up.
pub(crate) const GATE_SETTLE_TIMEOUT: Duration = Duration::from_secs(30);

/// the five channels epoch `e`'s engine uses: vote, certificate, resolver, the
/// eager payload-relay lane, and the payload FETCH lane (the lazy catch-up
/// backstop — a validator that missed the one-shot relay gossip for a
/// finalized op fetches its bytes by digest instead of wedging its apply
/// prefix forever). starts at 6, right after the fixed mesh channels
/// (submit-relay 3, statesync 4, reachability 5).
pub(crate) fn engine_channels(epoch: u64) -> (u64, u64, u64, u64, u64) {
    let base = 6 + epoch * 5;
    (base, base + 1, base + 2, base + 3, base + 4)
}

/// how long a booting validator keeps re-asking peers for the frame above its
/// recovered floor while the mesh is still forming. a WALL-CLOCK budget, not
/// an attempt count: one attempt costs nothing when the link is not up yet
/// (the send fails immediately with no recipients) but a full request timeout
/// when it is up and the peer does not answer, so only a deadline bounds the
/// wait either way. it has to cover a returning node re-forming its p2p and
/// overlay links, which is seconds, not milliseconds.
///
/// bounded ON PURPOSE, unlike the resident's re-bootstrap loop: this runs
/// BEFORE the engine and before the loop that answers other nodes' probes, so
/// a whole cluster restarting at once has nobody to answer it. an unbounded
/// wait here would deadlock that restart forever; a budget makes it cost this
/// much, once, and the expiry says so at `warn`. the probe runs only for a
/// validator that recovered a real height and only when some OTHER key is in
/// its peer book, so a cold genesis start and a solo validator pay nothing.
pub(crate) const BOOT_PROBE_BUDGET: Duration = Duration::from_secs(30);

/// the pause between boot catch-up probes (see [`BOOT_PROBE_BUDGET`]).
pub(crate) const BOOT_PROBE_INTERVAL: Duration = Duration::from_millis(250);
