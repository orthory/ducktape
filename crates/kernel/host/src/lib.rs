//! the host — the deterministic state-machine spine.
//!
//! a [`Host`] owns a registry of [`Module`]s and turns an inbound [`Msg`] into a
//! block: it routes the message to its target module, awaits the (deterministic)
//! `execute`, then drains the intents that execute emitted. emitted [`Msg`]s are
//! re-dispatched as LOCAL-ONLY follow-up ops (never re-broadcast); emitted
//! [`Event`]s/[`Effect`]s are collected and handed back for the effectful node
//! layer (out of scope this slice). after the drain, the root-hash is recomposed
//! over the registry via [`global_root`].
//!
//! ## determinism
//!
//! `submit` is a pure function of `(registry state, msg, env)`:
//! - the registry is a [`BTreeMap`], so snapshot + root-hash iteration is sorted
//!   and order-stable across nodes;
//! - the follow-up queue is FIFO and dispatched purely locally;
//! - the drain is hard-capped at [`MAX_DISPATCHES`], so it always terminates
//!   (a self-emitting or A↔B-ping-pong module hits [`Error::BudgetExceeded`]
//!   rather than looping forever).
//!
//! ## the borrow seam (remove-execute-reinsert)
//!
//! executing module X needs `&mut X` while the [`Ctx`] must read the *other*
//! modules (for `query` routing). a `BTreeMap` can't hand out "one `&mut` + rest
//! `&`", so the host `remove`s the target — yielding an OWNED `Box<dyn Module>`
//! fully decoupled from the map — then borrows the remaining map into the ctx.
//! the owned module and the `&rest` borrow are disjoint, so they compose across
//! the `.await`. the module is reinserted before any error propagates, so it can
//! never vanish from the registry.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::time::Duration;

use sdk::{
    Ctx, Env, Error, Event, Module, ModuleId, Msg, Origin, ResolverSyncTarget, StateRoot,
    StateSyncHandle,
};
use sha2::{Digest, Sha256};

pub mod worker;

/// compute the global root-hash over `modules` — the composition consensus
/// commits to: every module's id, state root, and deployment hash. Because
/// a module's own [`StateRoot`] already commits to its children (a qmdb merkle
/// root commits to its keys; a git HEAD oid commits to the whole repo tree),
/// this one level on top yields the full two-level authentication tree.
///
/// determinism is the whole job: every validator must produce a byte-identical
/// global root or the chain forks — so modules are sorted by id, and each id is
/// length-prefixed before hashing (otherwise ("ab", r) and ("a", "b"||r) would
/// collide). deliberately a plain sorted hash, NOT a qmdb-of-heads: qmdb's root
/// is an order-dependent HISTORY commitment, while a root-hash must be
/// `f(current state)` — order-independent + idempotent — so a state-synced node
/// computes the same root. upgrade to a small merkle tree only when a light
/// client needs log-n membership proofs.
pub fn global_root(modules: &[&dyn Module]) -> StateRoot {
    let pairs: Vec<(ModuleId, StateRoot)> = modules
        .iter()
        .map(|m| {
            (
                m.id(),
                module_commitment(m.root(), m.code_hash().as_deref()),
            )
        })
        .collect();
    global_root_of(&pairs)
}

/// Bind executable code to its state. A manifest can reconstruct every module,
/// including the code registry itself, without trusting code chosen by a peer.
/// Native modules occur only in library harnesses and have no deployable code.
pub fn module_commitment(root: StateRoot, code_hash: Option<&[u8]>) -> StateRoot {
    let Some(code_hash) = code_hash else {
        return root;
    };
    let mut hash = Sha256::new();
    hash.update(b"ducktape.module");
    hash.update((code_hash.len() as u64).to_le_bytes());
    hash.update(code_hash);
    hash.update(root.as_bytes());
    StateRoot(hash.finalize().into())
}

/// Hash already-bound module commitments (see [`module_commitment`]).
/// `root()` is a full state
/// serialization + hash for a map-backed module, so a caller holding every
/// module's root must never pay for it twice — a checkpoint capture computed
/// each root four times before this seam existed (#1018).
pub fn global_root_of(pairs: &[(ModuleId, StateRoot)]) -> StateRoot {
    let mut sorted: Vec<&(ModuleId, StateRoot)> = pairs.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));

    let mut h = Sha256::new();
    h.update((sorted.len() as u64).to_le_bytes());
    for (id, root) in sorted {
        h.update((id.len() as u64).to_le_bytes());
        h.update(id.as_bytes());
        h.update(root.0);
    }
    StateRoot(h.finalize().into())
}

/// hard cap on dispatches per `submit` (the root op plus all follow-ups). a
/// consensus/genesis constant — identical on every node — so the local re-entry
/// loop is guaranteed to terminate regardless of module behavior.
pub const MAX_DISPATCHES: u32 = 1024;

/// hard cap on rollback+replay cycles per BLOCK — [`MAX_DISPATCHES`] is per
/// `drain_queue` CALL and so bounds one member, never the batch. Per-op
/// isolation replays every already-accepted member when a member that STAGED
/// then fails, which is quadratic in the member count; this bounds the block's
/// re-execution to `members * (1 + MAX_BLOCK_REPLAYS)`. Past the budget the
/// remaining members are rejected unexecuted — a function of the block alone,
/// so every validator produces the identical verdict set.
pub const MAX_BLOCK_REPLAYS: u32 = 8;

/// the genesis-constant module id the `modules` registry registers under. read
/// by the boundary code-swap realization ([`Host::realize_module_swaps`]) and by
/// the drain's [`Host::pending_modules_advance`] injection; absent on a host
/// without the module (e.g. a test registry), in which case no swap is ever
/// realized or injected.
pub const MODULES_ID: &str = modules::DEFAULT_MODULES_ID;

/// the genesis-constant module id the `dispatch` module registers under. read
/// by the drain's delivery injection ([`Host::pending_deliveries`]); absent on
/// a net without the module, in which case nothing is ever injected.
const DISPATCH_MODULE_ID: &str = dispatch::DEFAULT_DISPATCH_TARGET;

/// the genesis-constant module id the `acl` module registers under. read by
/// the drain's dispatch gate ([`Host::require_submit_standing`]); absent on a
/// net without the module, in which case every external submit is admitted
/// (the allow-all shape an empty policy table also produces).
const ACL_MODULE_ID: &str = acl::DEFAULT_ACL_ID;

/// the genesis-constant module id the `valset` module registers under —
/// the sibling read resolving the acl gate's validator/node standings.
const VALSET_MODULE_ID: &str = "valset";

/// the genesis-constant module id the `identity` module registers under —
/// the sibling read resolving the acl gate's user standing.
const IDENTITY_MODULE_ID: &str = "identity";

/// the out-of-band source of component BYTES for a code swap.
///
/// the code registry commits only the 32-byte content hash of each module's
/// active code; the BYTES are content-addressed and distributed off-band
/// (blobstore, state-sync). the host is HANDED one of these at a swap boundary
/// rather than owning a byte cache — so it stays registry-only, and a test can
/// inject a trivial in-memory map while a node injects a blobstore-backed one.
/// `fetch` returns `None` when this node does not (yet) hold the bytes for a
/// hash — a fail-closed miss that stops the boundary, never a fork.
/// `Send + Sync` because the ordered lane holds its source across an executor.
/// `fetch` is async (`?Send`, like every host-side future — the host itself is
/// `!Send`): a node-side source may go to the mesh for bytes its local store
/// lacks before answering, and only a still-missing digest is a `None`.
#[async_trait::async_trait(?Send)]
pub trait CodeSource: Send + Sync {
    /// component bytes for a content hash, or `None` if absent on this node.
    async fn fetch(&self, code_hash: &[u8]) -> Option<Vec<u8>>;

    /// a stable snake_case name for WHERE this source looks — logged with a
    /// fail-closed miss so an operator reads "we asked the mesh and no peer
    /// served it" apart from "this node never asked anyone".
    fn origin(&self) -> &'static str;
}

/// the no-source default: a node wired without any code source. every fetch
/// misses, so a boundary that actually arms a swap FAILS CLOSED (loudly) rather
/// than silently running stale code — and a net with no swaps never notices.
pub struct NoCodeSource;

#[async_trait::async_trait(?Send)]
impl CodeSource for NoCodeSource {
    async fn fetch(&self, _code_hash: &[u8]) -> Option<Vec<u8>> {
        None
    }

    fn origin(&self) -> &'static str {
        "none"
    }
}

/// instantiates a freshly-ADMITTED module from its verified component bytes at
/// the activation boundary — the constructor twin of [`CodeSource`]. the node
/// wires its module composer here (the one path every wasm module enters a
/// host through); the host itself stays wasm-runtime-agnostic. a host without
/// a factory FAILS CLOSED (loudly) when an admission arms, and a net that
/// never admits modules never notices. async like [`CodeSource::fetch`]: a
/// store-backed admission opens its store, and stores open asynchronously.
#[async_trait::async_trait(?Send)]
pub trait ModuleFactory: Send + Sync {
    /// a module instance for `id` from component bytes already verified
    /// against the committed code hash — or [`Admitted::ForeignAbi`] for bytes
    /// that are no module at all.
    async fn instantiate(&self, id: &str, component_bytes: &[u8]) -> Result<Admitted, Error>;
}

/// what a [`ModuleFactory`] made of one admission's verified bytes.
///
/// The registry is id-generic bookkeeping: any id may carry a hash-pinned
/// artifact, and the reachability plane's `ducktape:netstack` guest is
/// delivered through exactly that record. Only the factory can tell the two
/// apart, and only it may say so — a refusal that is really "this build cannot
/// run a genuine module" MUST stay fail-closed, or a node seats a different
/// registry set than its peers and forks in silence.
pub enum Admitted {
    /// a `ducktape:module` component, instantiated and ready to seat.
    Module(Box<dyn Module>),
    /// the bytes speak another world entirely: the registry entry is another
    /// plane's commitment record, not an admission. The module boundary skips
    /// it — see [`Host::realize_module_swaps`].
    ForeignAbi,
}

/// sha256 content hash of component bytes — the verify side of a code swap.
fn sha256(bytes: &[u8]) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).to_vec()
}

/// lowercase hex of a hash, for fail-closed error messages.
fn hex32(bytes: &[u8]) -> String {
    sdk::hash::hex_lower(bytes)
}

/// the block-constant consensus context for one [`Host::submit_at`]: the agreed
/// `height` / `consensus_time` (identical on every validator — sourced from the
/// finalized view) and the ROOT op's `origin`. these are constant across every
/// dispatch in the block; per-follow-up origin is set by the drain.
pub struct BlockContext {
    /// the finalized block height (the agreed simplex view).
    pub height: u64,
    /// the agreed logical clock (the finalized view) — NOT wall clock.
    pub consensus_time: u64,
    /// the root op's real submitter. follow-ups override with `Origin::Module`.
    pub origin: Origin,
}

impl Default for BlockContext {
    /// the pre-consensus default: height/time 0 and an empty external origin,
    /// so [`Host::submit`] is byte-for-byte the old hardcoded behavior.
    fn default() -> Self {
        Self {
            height: 0,
            consensus_time: 0,
            origin: Origin::External(Vec::new()),
        }
    }
}

/// one dispatch in a block's drain: the module that ran, what triggered it, and
/// how many intents it emitted. a DETERMINISTIC structural trace — pure function
/// of `(registry state, msg, env)`, identical on every honest validator — so it
/// is safe to expose for observability (it carries NO wall-clock; timing lives
/// in the effectful node layer). recorded in dispatch (drain FIFO) order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DispatchRecord {
    /// the module dispatched this step (`msg.target`).
    pub module: ModuleId,
    /// what triggered this dispatch: the root op's real `origin`, or
    /// `Origin::Module(emitter)` for a follow-up.
    pub origin: Origin,
    /// the op bytes this dispatch applied (`msg.payload`) — a consensus input,
    /// so the trace stays deterministic. carrying it here makes the outcome the
    /// block's complete per-module op stream (root op AND follow-ups), which is
    /// what a derived read-model tier consumes; the payload of a follow-up is
    /// otherwise visible to no one outside the drain.
    pub payload: Vec<u8>,
    /// count of follow-up `Msg`s this dispatch emitted (the causal fan-out).
    pub emitted_msgs: usize,
    /// count of observability `Event`s this dispatch emitted.
    pub emitted_events: usize,
    /// the module-assigned stamp of this dispatch ([`sdk::Ctx::set_assigned`]):
    /// values the module assigned while applying the op (a sequence, a
    /// revision) that the payload cannot carry. module-encoded, host-opaque —
    /// a consensus-deterministic function of `(state, msg)`, so the trace
    /// stays a pure structural record. empty when the dispatch assigned
    /// nothing.
    pub assigned: Vec<u8>,
}

/// the result of applying one block (`submit`).
#[derive(Debug)]
pub struct BlockOutcome {
    /// the root-hash over the registry after the drain settled.
    pub root_hash: StateRoot,
    /// events emitted during the block, in dispatch order — observability, and
    /// the lane the host-owned worker seam claims off-consensus work from.
    pub events: Vec<Event>,
    /// the deterministic dispatch trace: one entry per module dispatched this
    /// block, in drain order. the "what happened" spine the node layer tags with
    /// node-local timing for its metrics.
    pub dispatches: Vec<DispatchRecord>,
}

/// the result of applying a BATCH of ops as ONE block ([`Host::submit_block`]).
///
/// per-op isolation with a SINGLE commit boundary: each input op is drained on
/// top of the prior accepted ops' staged writes (read-your-writes across
/// members); an op that rejects DETERMINISTICALLY is isolated — its stage rolled
/// back and the accepted ops replayed — so the committed state is exactly the
/// accepted subset applied in input order. every applied member shares the ONE
/// post-batch [`root_hash`](BatchOutcome::root_hash).
#[derive(Debug)]
pub struct BatchOutcome {
    /// the one post-batch root-hash, shared by every applied member.
    pub root_hash: StateRoot,
    /// one outcome per input op, in input order.
    pub members: Vec<MemberOutcome>,
    /// aggregate events, in drain order: every applied member's trace in
    /// input order, then the once-per-block injections.
    pub events: Vec<Event>,
    /// the dispatch trace from the once-per-block System injections
    /// (`pending_modules_advance` / `pending_deliveries`),
    /// drained once after the members.
    pub system_dispatches: Vec<DispatchRecord>,
}

impl BatchOutcome {
    /// flatten this outcome into the block-level facts the replay paths seal
    /// and fold from: whether the block RAN REAL WORK (any member applied, or
    /// a once-per-block System injection dispatched — the live drain's
    /// seal-disposition rule), and the aggregate dispatch trace in the live
    /// index order — each applied member in input order, then the System
    /// injections. recovery replay and suffix catch-up fold THIS exact order,
    /// so a re-derived per-module op index matches the live one row for row.
    pub fn into_trace(self) -> (bool, Vec<DispatchRecord>) {
        let mut dispatches = Vec::new();
        let mut ran = !self.system_dispatches.is_empty();
        for member in self.members {
            if let MemberOutcome::Applied { dispatches: d } = member {
                ran = true;
                dispatches.extend(d);
            }
        }
        dispatches.extend(self.system_dispatches);
        (ran, dispatches)
    }
}

/// the outcome of one member op in a [`BatchOutcome`].
#[derive(Debug)]
pub enum MemberOutcome {
    /// the op applied; carries its OWN dispatch trace (root op + follow-ups).
    Applied { dispatches: Vec<DispatchRecord> },
    /// the op rejected deterministically; its staged writes were rolled back and
    /// the accepted members replayed, so it left no trace on committed state. the
    /// reason is the drain [`Error`] rendered to a string.
    Rejected { reason: String },
}

/// one submitted op at the batch seam: the frame's verified origin, its root
/// msg, and its content id. [`Host::submit_block`] wraps bare pairs; the
/// node's frame drain builds these directly.
///
/// An op carries EXACTLY ONE dispatch. There is no envelope continuation: a
/// frame cannot append a second op that runs under a caller-chosen
/// `Origin::Module`. See `no_continuation_lane.rs`.
#[derive(Clone, Debug)]
pub struct BlockOp {
    /// the frame's verified authorship.
    pub origin: Origin,
    /// the op's msg.
    pub msg: Msg,
    /// the frame's content id. all-zero for callers that have no frame.
    pub frame: [u8; 32],
}

impl BlockOp {
    /// an op with no frame id — the [`Host::submit_block`] wrapping.
    pub fn bare(origin: Origin, msg: Msg) -> Self {
        Self {
            origin,
            msg,
            frame: [0; 32],
        }
    }
}

/// one accepted member op inside [`Host::apply_block`]'s per-op isolation: the
/// inputs needed to REPLAY it verbatim after an isolation rollback, plus its
/// authoritative trace.
struct AcceptedUnit {
    origin: Origin,
    msg: Msg,
    /// which `members[i]` this unit's outcome lands in.
    member: usize,
    events: Vec<Event>,
    dispatches: Vec<DispatchRecord>,
}

/// a finalized consensus boundary the host is allowed to serve from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FinalizedBlock {
    /// finalized block height.
    pub height: u64,
    /// root-hash consensus committed at `height`.
    pub root_hash: StateRoot,
}

/// one module's committed root plus the sync surface it can currently serve.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleSnapshot {
    pub id: ModuleId,
    pub root: StateRoot,
    pub code_hash: Option<Vec<u8>>,
    pub state_sync: StateSyncHandle,
}

/// what [`CapturePayloads::InMemoryCohort`] reports for the disk cohort.
const SELF_DURABLE_NO_PAYLOAD: &str =
    "per-block durable on its own disk: this capture materializes no payload for it";

/// which modules a capture MATERIALIZES payload bytes for. every module's
/// root and the composed root-hash are captured either way — this decides only
/// whose `state_sync_handle` is asked, and asking is what costs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapturePayloads {
    /// every module's: the boundary a JOINER installs the whole registry from.
    All,
    /// the in-memory cohort's only. a [`Module::block_durable`] module commits
    /// to its OWN disk every block and reopens from it at boot — a checkpoint
    /// restore installs nothing for one — so materializing its container is a
    /// full re-encode of state the manifest never reads back. forge's is its
    /// whole git pack closure, built on the consensus select loop and fsync'd
    /// into every checkpoint (#1308). such a module is reported
    /// [`StateSyncHandle::Unsupported`]: this capture holds no payload for it,
    /// which is what the field means to every reader of a capture.
    InMemoryCohort,
}

/// one module that could NOT prepare a sync surface at this boundary. its
/// committed root is still known — `root()` is pure and cannot fail — so the
/// boundary stays describable; only this module's transfer surface is missing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DegradedModule {
    pub id: ModuleId,
    pub root: StateRoot,
    pub code_hash: Option<Vec<u8>>,
    pub reason: Error,
}

/// a consistent registry view captured at a finalized root-hash boundary.
///
/// a module that fails to prepare its handle lands in `degraded` instead of
/// aborting the capture: one module's bad state must not discard the rest of
/// the registry's perfectly good snapshots, and every failure of the boundary
/// is reported at once rather than only the first in registry order.
/// what that means for the caller is the CALLER's policy, and the two differ —
/// a joiner is told per-module (`statesync` serves the degraded module as
/// `Unsupported`), while a recovery checkpoint refuses outright, because a
/// checkpoint that cannot restore is worse than no checkpoint at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FinalizedSnapshot {
    pub height: u64,
    pub root_hash: StateRoot,
    pub modules: Vec<ModuleSnapshot>,
    pub degraded: Vec<DegradedModule>,
}

impl FinalizedSnapshot {
    /// find a module entry by id. degraded modules are NOT entries — they have
    /// no `state_sync` to return; they are listed in `degraded` instead.
    pub fn module(&self, id: &str) -> Option<&ModuleSnapshot> {
        self.modules.iter().find(|m| m.id == id)
    }

    /// true only when every module supplied self-contained snapshot bytes.
    pub fn has_all_snapshot_bytes(&self) -> bool {
        self.degraded.is_empty()
            && self
                .modules
                .iter()
                .all(|m| m.state_sync.has_snapshot_bytes())
    }

    /// true when every module can be rebuilt without an external resolver.
    pub fn is_self_contained(&self) -> bool {
        self.degraded.is_empty()
            && self
                .modules
                .iter()
                .all(|m| m.state_sync.is_self_contained())
    }
}

/// which block-boundary phase a module failed in. the two phases have opposite
/// damage profiles: a COMMIT failure may leave the block half-published (earlier
/// modules in registry order committed, this one did not), an ABORT failure may
/// leave staged writes that leak into a later block. either way this node's
/// registry no longer matches what every other honest validator computed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryPhase {
    Commit,
    Abort,
}

/// a NON-DETERMINISTIC, node-local fault at the block boundary. this is NOT a
/// rejected op: a rejected op errors identically on every honest validator and
/// is safely treated as a deterministic no-op, while a boundary fault (a disk
/// error inside `commit_block`, a module that could not discard its stage) hit
/// only THIS node — its registry state is now indeterminate relative to its
/// peers. the only sound response is fail-stop: surface the fault and stop
/// applying blocks; continuing would silently fork this node's root-hash.
#[derive(Debug, PartialEq, Eq)]
pub struct FatalError {
    /// the module whose boundary hook failed.
    pub module: ModuleId,
    /// the boundary phase that failed.
    pub phase: BoundaryPhase,
    /// the module's own error.
    pub source: Error,
}

impl core::fmt::Display for FatalError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let phase = match self.phase {
            BoundaryPhase::Commit => "commit_block",
            BoundaryPhase::Abort => "abort_block",
        };
        write!(
            f,
            "fatal block-boundary fault: module {} failed {phase}: {} — registry state is \
             indeterminate, this node must stop applying blocks",
            self.module, self.source
        )
    }
}

impl std::error::Error for FatalError {}

/// the two ways a `submit` can fail, with OPPOSITE handling contracts:
///
/// - [`SubmitError::Rejected`] is DETERMINISTIC: every honest validator computes
///   the identical rejection for this op (a module error, an unknown target, a
///   blown dispatch budget) and the abort path verifiably rolled every touched
///   module back. an ordered lane treats it as a no-op and keeps draining.
/// - [`SubmitError::Fatal`] is NODE-LOCAL: a block-boundary hook failed on this
///   node only, and the registry may be half-committed or carrying a leaked
///   stage. the caller MUST stop applying blocks (fail-stop) — continuing forks
///   this node against its peers.
#[derive(Debug, PartialEq, Eq)]
pub enum SubmitError {
    /// the op was rejected deterministically; the block rolled back cleanly.
    Rejected(Error),
    /// a block-boundary hook failed on this node; state is indeterminate.
    Fatal(FatalError),
}

impl SubmitError {
    /// the deterministic rejection, if that is what this is. `None` for a fatal
    /// boundary fault — callers that only expect rejections should not silently
    /// discard fatality.
    pub fn rejected(&self) -> Option<&Error> {
        match self {
            Self::Rejected(e) => Some(e),
            Self::Fatal(_) => None,
        }
    }
}

impl core::fmt::Display for SubmitError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Rejected(e) => write!(f, "op rejected: {e}"),
            Self::Fatal(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for SubmitError {}

impl From<Error> for SubmitError {
    fn from(e: Error) -> Self {
        Self::Rejected(e)
    }
}

/// the one failure that aborts a whole capture: the boundary itself is wrong.
/// a MODULE failure is not one of these — it is per-module and reported in
/// [`FinalizedSnapshot::degraded`], so it can never take the boundary down.
#[derive(Debug, PartialEq, Eq)]
pub enum SnapshotError {
    /// the caller asked for a boundary that no longer matches the host state.
    RootHashMismatch {
        expected: StateRoot,
        actual: StateRoot,
    },
}

impl core::fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::RootHashMismatch { expected, actual } => write!(
                f,
                "finalized root-hash mismatch: expected {expected:?}, actual {actual:?}",
            ),
        }
    }
}

impl std::error::Error for SnapshotError {}

/// one roster entry's decided realization, resolved by
/// [`Host::realize_module_swaps`]'s first phase and applied by its second — the
/// staging that makes the boundary all-or-nothing.
enum Realization {
    /// a running module moves to already-fetched, hash-verified bytes.
    Swap { module_id: ModuleId, bytes: Vec<u8> },
    /// an ADMISSION: the instantiated module takes its registry seat.
    Seat(Box<dyn Module>),
    /// another plane's component — latch the decision, register nothing.
    Foreign {
        module_id: ModuleId,
        code_hash: Vec<u8>,
    },
}

/// the deterministic state machine: a module registry + dispatch + drain.
#[derive(Default)]
pub struct Host {
    /// deterministic iteration order is load-bearing for snapshot + root-hash.
    registry: BTreeMap<ModuleId, Box<dyn Module>>,
    /// instantiates post-genesis ADMISSIONS at the activation boundary.
    /// `None` fails closed the moment an admission arms — never before.
    module_factory: Option<Box<dyn ModuleFactory>>,
    /// `(module id, code hash)` pairs this boundary has already decided are
    /// [`Admitted::ForeignAbi`] — THE LATCH. Deciding costs a component
    /// compile and this boundary runs before EVERY block, so the answer (which
    /// cannot change for a fixed pair) is paid, reported, and skipped from
    /// then on. Per-node bookkeeping, never part of `root()`.
    foreign_admissions: BTreeSet<(ModuleId, Vec<u8>)>,
}

impl Host {
    pub fn new() -> Self {
        Self {
            registry: BTreeMap::new(),
            module_factory: None,
            foreign_admissions: BTreeSet::new(),
        }
    }

    /// wire the constructor for post-genesis module admissions (the node
    /// injects its module composer; same shape as `CodeSource`).
    pub fn set_module_factory(&mut self, factory: Box<dyn ModuleFactory>) {
        self.module_factory = Some(factory);
    }

    /// register a module under its own [`Module::id`]. genesis-time wiring.
    pub fn register(&mut self, module: Box<dyn Module>) {
        self.registry.insert(module.id(), module);
    }

    /// build a host from a declared module set (registry-as-genesis-state). errors
    /// on a duplicate module id, since dispatch addresses modules by id.
    pub fn genesis(modules: Vec<Box<dyn Module>>) -> Result<Self, Error> {
        let mut host = Self::new();
        for m in modules {
            let id = m.id();
            if host.registry.contains_key(&id) {
                return Err(Error::Module(format!("duplicate module id: {id}")));
            }
            host.registry.insert(id, m);
        }
        Ok(host)
    }

    /// external read-only query of a registered module (sync, like [`Ctx::query`]
    /// but from outside a dispatch). routes to [`Module::query_with`].
    pub async fn query(&self, target: &str, req: &[u8]) -> Result<Vec<u8>, Error> {
        match self.registry.get(target) {
            Some(m) => {
                let snapshot: BTreeMap<ModuleId, StateRoot> = self
                    .registry
                    .iter()
                    .map(|(k, m)| (k.clone(), m.root()))
                    .collect();
                let target = target.to_string();
                let ctx = ReadOnlyQueryCtx {
                    env: Env {
                        height: 0,
                        consensus_time: 0,
                        origin: Origin::System,
                        me: target.clone(),
                    },
                    snapshot: &snapshot,
                    registry: &self.registry,
                    active: BTreeSet::from([target]),
                };
                m.query_with(&ctx, req).await
            }
            None => Err(Error::UnknownModule(target.to_string())),
        }
    }

    /// route a byte-level state-sync serve request to a registered module (see
    /// [`Module::serve_sync`]). read-only against committed state; a network
    /// state-sync service calls this between blocks so responses are always
    /// boundary-consistent.
    pub async fn serve_sync(&self, target: &str, req: &[u8]) -> Result<Vec<u8>, Error> {
        match self.registry.get(target) {
            Some(m) => m.serve_sync(req).await,
            None => Err(Error::UnknownModule(target.to_string())),
        }
    }

    /// return a registered resolver-backed module's committed sync target.
    pub async fn resolver_sync_target(&self, target: &str) -> Result<ResolverSyncTarget, Error> {
        match self.registry.get(target) {
            Some(m) => m.resolver_sync_target().await,
            None => Err(Error::UnknownModule(target.to_string())),
        }
    }

    /// the SYSTEM-ORIGIN modules registry `Advance` the drain injects in-block at a
    /// finalized code-swap boundary, or `None` when there is nothing to
    /// reconcile.
    ///
    /// this is the replay-safe realization of "arm/abort is a deterministic
    /// self-transition evaluated exactly once at `H`": because it rides the
    /// SAME [`Host::submit_at`] drain that recovery-replay and
    /// state-sync-install also run, the committed active-hash flip +
    /// pending-swap clear reconstruct byte-for-byte on every node (never a
    /// respawn side-effect, invisible to replay). keyed purely on committed
    /// registry state + `height`: injected iff the committed module holds an
    /// armed swap ([`modules::ScheduledSwap::armed_at`] — readiness latched
    /// before `height`, floor reached). idempotent — the first block
    /// at/after `H` clears it, so later blocks inject nothing. ABSENT until
    /// the module is registered (the query errors → `None`), so the drain is
    /// byte-identical on a net without the module.
    async fn pending_modules_advance(&self, height: u64) -> Option<Msg> {
        let swap_armed = self.module_status().await.is_some_and(|modules| {
            modules
                .iter()
                .any(|m| m.pending.as_ref().is_some_and(|p| p.armed_at(height)))
        });
        swap_armed.then(|| Msg {
            target: MODULES_ID.into(),
            payload: modules::encode_msg(&modules::ModulesMsg::Advance),
        })
    }

    /// the SYSTEM-ORIGIN `DeliverPending` the drain injects when the
    /// committed dispatch mailbox is non-empty, or `None` when there is
    /// nothing to deliver.
    ///
    /// this is the never-pop-stack rule's other half: a dispatch result
    /// commits into the mailbox in one block, and THIS injection — keyed
    /// purely on that committed state — hands it to the receiver in a later
    /// block. it rides the same drain as recovery-replay and
    /// state-sync-install, so delivery reconstructs byte-for-byte on every
    /// node. idempotent: the injected dispatch drains (a bounded batch of)
    /// the mailbox, so blocks after the last delivery inject nothing. ABSENT
    /// until the module is registered — the query errors → `None`, keeping
    /// the drain byte-identical on a net without dispatch.
    async fn pending_deliveries(&self) -> Option<Msg> {
        let req = dispatch::encode_query(&dispatch::DispatchQuery::PendingDeliveries);
        let bytes = self.query(DISPATCH_MODULE_ID, &req).await.ok()?;
        let dispatch::DispatchReply::PendingDeliveries(pending) =
            dispatch::decode_reply(&bytes).ok()?
        else {
            return None;
        };
        (pending > 0).then(|| Msg {
            target: DISPATCH_MODULE_ID.into(),
            payload: dispatch::encode_msg(&dispatch::DispatchMsg::DeliverPending {}),
        })
    }

    /// whether the committed dispatch mailbox holds undelivered results —
    /// the drain will inject `DeliverPending` into the NEXT successful block.
    /// drivers with no other block flow (a reactor fixpoint, a block-per-op
    /// daemon, a quiet validator) read this to know a flush block is needed.
    pub async fn has_pending_deliveries(&self) -> bool {
        self.pending_deliveries().await.is_some()
    }

    /// the modules registry's committed per-module code state, or `None` when the
    /// module is absent / its reply is unreadable — the shared out-of-block
    /// committed read behind [`Host::pending_modules_advance`] and
    /// [`Host::realize_module_swaps`] (a missing registry is never an error,
    /// just nothing to do). `pub` for the node's restore/sync composers, which
    /// adopt the modules the registry admitted after genesis.
    pub async fn module_status(&self) -> Option<Vec<modules::ModuleCode>> {
        let req = modules::encode_query(&modules::ModulesQuery::ModuleStatus);
        let bytes = self.query(MODULES_ID, &req).await.ok()?;
        match modules::decode_reply(&bytes).ok()? {
            modules::ModulesReply::ModuleStatus { modules } => Some(modules),
            _ => None,
        }
    }

    /// realize a verified code swap against a single registered module: route to
    /// its [`Module::swap_code`], keeping its host-owned state. errors if the
    /// module is not registered, or is native (no swappable code —
    /// [`Error::SwapUnsupported`]). the LOW-LEVEL seam;
    /// [`Host::realize_module_swaps`] is the boundary driver that fetches +
    /// verifies bytes against the committed hash before calling this.
    pub fn swap_module_code(&mut self, id: &str, component_bytes: &[u8]) -> Result<(), Error> {
        match self.registry.get_mut(id) {
            Some(m) => m.swap_code(component_bytes),
            None => Err(Error::UnknownModule(id.to_string())),
        }
    }

    /// reconcile every hot-swappable module's RUNNING code against the code
    /// registry's committed decision for block `height`, realizing any swap that
    /// has armed. this is the per-node, NON-consensus half of a live code update;
    /// the consensus half is the in-block [`Host::pending_modules_advance`] tick
    /// that records the active hash. A swap preserves the module's state root;
    /// the global root also binds its running deployment hash, so it changes at
    /// activation even when the module's state is untouched.
    ///
    /// keyed PURELY on committed registry state + `height` — the code the
    /// registry designates FOR `height` ([`modules::code_at`]) — so it
    /// reconstructs identically on every path that advances a node to a
    /// committed state: live drain, recovery replay, state-sync catch-up. it
    /// is idempotent: it compares [`Module::code_hash`] and re-instantiates a
    /// component only on an actual change. run it BEFORE applying block
    /// `height`, so that block's dispatches execute on the code the registry
    /// designates for `height`.
    ///
    /// the target hash for a module at `height` is a pending hash that has
    /// armed (`ScheduledSwap::armed_at` — the SAME predicate
    /// the modules registry's `Advance` applies, so this out-of-block realization and the
    /// in-block flip never disagree on the arm set; on the live drain the
    /// registry sits at `height - 1` and this is the read that precedes the
    /// flip), else the latest ACTIVATION at or before `height`. the registry is
    /// disk-durable and reopens AHEAD of a crash-restart replay — active =
    /// whatever landed last, pending gone — so the tip cannot say which code
    /// sealed a replayed block; the activation history can, and a replay that
    /// spans a swap moves the module back to the pre-swap code and forward
    /// again at the swap block. a state-sync joiner (post-activation state,
    /// nothing pending) reads the same last activation and reconciles its
    /// genesis code to the live one instead of forking. a module whose first
    /// activation is past `height` seats its first code; one registered but
    /// never activated is nothing to realize.
    ///
    /// FAIL-CLOSED and ALL-OR-NOTHING: a designated hash whose bytes this node
    /// lacks, or bytes whose sha256 does not match the committed hash, is a hard
    /// error (the node cannot honestly apply `height` without the agreed code) —
    /// it returns `Err` with no partial swap applied. That is why realization is
    /// two phases: the whole roster RESOLVES first (fetch, verify, instantiate —
    /// every fallible step, touching nothing), and only a fully-resolved roster
    /// APPLIES. A half-applied roster would leave this node running code the
    /// registry designates for `height` over state still at `height - 1` — and,
    /// for an admission, publishing a root-hash no block ever sealed — while the
    /// drain turns the `Err` into a retryable code stall that never applies
    /// `height`. ABSENT registry → nothing to reconcile, `Ok(())`.
    ///
    /// The one thing that is NOT an admission is code that speaks another
    /// world ([`Admitted::ForeignAbi`]): the registry is id-generic and other
    /// planes commit their hash-pinned components through it, so such an entry
    /// is skipped and latched ([`Host::skip_foreign_admission`]) rather than
    /// halting every block on every node forever.
    pub async fn realize_module_swaps(
        &mut self,
        height: u64,
        src: &dyn CodeSource,
    ) -> Result<(), Error> {
        let Some(modules) = self.module_status().await else {
            return Ok(());
        };
        // PHASE 1 — RESOLVE. every fallible step happens here, against an
        // untouched registry: a miss anywhere returns Err having mutated nothing.
        let mut realizations: Vec<Realization> = Vec::new();
        for m in modules {
            let Some(target) = modules::code_at(&m, height) else {
                continue; // registered, never activated — nothing to realize.
            };
            // only reconcile a module this node actually runs AS a hot-swappable
            // component: a native test module (no `code_hash`) cannot swap.
            // An id absent from the
            // registry is a post-genesis ADMISSION this boundary must realize by
            // instantiating the module from its verified bytes.
            let current = match self.registry.get(&m.module_id) {
                Some(module) => match module.code_hash() {
                    Some(current) => Some(current),
                    None => continue, // native test module.
                },
                None => None, // admission to realize below.
            };
            if current.as_deref() == Some(target) {
                continue; // already on the designated code — idempotent no-op.
            }
            let decided_foreign = self
                .foreign_admissions
                .contains(&(m.module_id.clone(), target.to_vec()));
            if decided_foreign {
                continue; // another plane's record, already answered — see the latch.
            }
            let Some(bytes) = src.fetch(target).await else {
                // the ONE line that precedes the fatal: a fail-closed miss is
                // terminal, so this fires at most once per node lifetime and
                // names both the hash and the source that could not serve it.
                tracing::error!(
                    target: "ducktape::modules",
                    event = "module_code_unresolved",
                    reason = "code_bytes_absent",
                    module = %m.module_id,
                    code_hash = %hex32(target),
                    source = src.origin(),
                    "committed module code is unavailable — the boundary fails closed"
                );
                return Err(Error::Module(format!(
                    "code bytes absent for module {} (hash {}) — fail-closed",
                    m.module_id,
                    hex32(target),
                )));
            };
            if sha256(&bytes) != target {
                return Err(Error::Module(format!(
                    "code bytes for module {} do not match committed hash {} — fail-closed",
                    m.module_id,
                    hex32(target),
                )));
            }
            match current {
                Some(_) => realizations.push(Realization::Swap {
                    module_id: m.module_id.clone(),
                    bytes,
                }),
                None => {
                    // the admission path: registration changes root-hash by
                    // construction (the registry set is what `root_hash`
                    // composes over), which is exactly why it rides the same
                    // readiness/height gate as a swap and realizes at one
                    // deterministic boundary on every validator.
                    let Some(factory) = &self.module_factory else {
                        return Err(Error::Module(format!(
                            "module {} admitted but no module factory is wired — fail-closed",
                            m.module_id,
                        )));
                    };
                    let Admitted::Module(module) = factory.instantiate(&m.module_id, &bytes).await?
                    else {
                        realizations.push(Realization::Foreign {
                            module_id: m.module_id.clone(),
                            code_hash: target.to_vec(),
                        });
                        continue;
                    };
                    if module.id() != m.module_id {
                        return Err(Error::Module(format!(
                            "module factory instantiated `{}` for admission `{}` — fail-closed",
                            module.id(),
                            m.module_id,
                        )));
                    }
                    realizations.push(Realization::Seat(module));
                }
            }
        }

        // Prepare every replacement while the running roster is untouched.
        // Holding each module borrow in its action makes installation infallible;
        // a later compile failure simply drops all the prepared actions.
        let mut swaps = BTreeMap::new();
        let mut seats = Vec::new();
        for realization in realizations {
            match realization {
                Realization::Swap { module_id, bytes } => {
                    swaps.insert(module_id, bytes);
                }
                other => seats.push(other),
            }
        }
        let mut prepared = Vec::new();
        for (id, module) in &mut self.registry {
            let Some(bytes) = swaps.remove(id) else {
                continue;
            };
            prepared.push(module.prepare_swap(&bytes)?);
        }
        for apply in prepared {
            apply();
        }
        for realization in seats {
            match realization {
                Realization::Seat(module) => {
                    self.registry.insert(module.id(), module);
                }
                Realization::Foreign {
                    module_id,
                    code_hash,
                } => {
                    self.skip_foreign_admission(&module_id, &code_hash);
                }
                Realization::Swap { .. } => unreachable!("swaps were prepared above"),
            }
        }
        Ok(())
    }

    /// latch one `(id, code hash)` pair as another plane's record and say so
    /// ONCE. The registry admits any id: the reachability plane's
    /// `ducktape:netstack` guest is delivered through the very same record,
    /// and a boundary that treated it as a module admission would fail closed
    /// on every node, on every block, forever — a halted chain, not a refused
    /// swap. The code it commits is realized by whatever plane owns that id
    /// (netstack: its own non-blocking reconciler), never here.
    fn skip_foreign_admission(&mut self, module_id: &str, code_hash: &[u8]) {
        let newly_decided = self
            .foreign_admissions
            .insert((module_id.to_string(), code_hash.to_vec()));
        if !newly_decided {
            return;
        }
        tracing::warn!(
            target: "ducktape::modules",
            reason = "foreign_module_abi",
            module = %module_id,
            code_hash = %hex32(code_hash),
            "the code this registry entry commits is not a `ducktape:module` — the module \
             boundary skips it and keeps sealing"
        );
    }

    /// the current root-hash: [`global_root`] over the registered modules.
    pub fn root_hash(&self) -> StateRoot {
        let mods: Vec<&dyn Module> = self.registry.values().map(|b| b.as_ref()).collect();
        global_root(&mods)
    }

    /// the live root of a single registered module (test/inspection accessor).
    pub fn module_root(&self, id: &str) -> Option<StateRoot> {
        self.registry.get(id).map(|m| m.root())
    }

    /// the sha256 of the component a registered module currently RUNS, or
    /// `None` for a native module (no swappable code) and for an unknown id.
    /// per-node realization state, never part of `root()`.
    pub fn module_code_hash(&self, id: &str) -> Option<Vec<u8>> {
        self.registry.get(id).and_then(|m| m.code_hash())
    }

    /// Mappers belonging to the currently running deployments, including
    /// explicit absence when a deployment removes its previous mapper.
    pub fn module_index_guests(&self) -> impl Iterator<Item = (&str, Option<&[u8]>)> {
        self.registry
            .iter()
            .map(|(id, module)| (id.as_str(), module.index_guest()))
    }

    /// every registered module's `(id, root)`, in registry (sorted-id) order —
    /// the exact input [`Host::root_hash`] composes over. a recovery journal
    /// seals each applied block with these so a restarted node can locate every
    /// module's replay position by root equality.
    pub fn module_roots(&self) -> Vec<(ModuleId, StateRoot)> {
        self.registry
            .iter()
            .map(|(id, m)| (id.clone(), m.root()))
            .collect()
    }

    /// the per-block-durable disk cohort: every registered module that declares
    /// [`Module::block_durable`] — a substrate that commits to its OWN disk each
    /// block and recovers itself at boot rather than riding a checkpoint
    /// snapshot. recovery uses this to tell a disk substrate that legitimately
    /// raced N blocks ahead of the last checkpoint apart from a rolled-back
    /// in-memory cohort module: only a disk-cohort module may be trusted at a
    /// self-durable root ABOVE the checkpoint.
    ///
    /// the question is DURABILITY, not sync surface. this used to read
    /// [`StateSyncHandle::ResolverBacked`] off `state_sync_handle`, which dropped
    /// forge — per-block durable on its own disk, but shipping one
    /// self-contained container — out of the cohort and bricked any restart with
    /// two forge blocks above the last checkpoint.
    pub fn block_durable_ids(&self) -> BTreeSet<ModuleId> {
        self.registry
            .iter()
            .filter(|(_, m)| m.block_durable())
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// a registered module's per-commit durable height cursor (see
    /// [`Module::durable_commit_height`]): the block height its last durable
    /// commit was written for, persisted atomically with that commit. `None`
    /// for unregistered ids and for modules that track no cursor. recovery
    /// reads this to bound-and-verify a disk module's TRAILING durable commit
    /// whose journal seal was lost to a power cut.
    pub fn durable_commit_height(&self, id: &str) -> Option<u64> {
        self.registry
            .get(id)
            .and_then(|m| m.durable_commit_height())
    }

    /// capture the committed registry view for a finalized block.
    ///
    /// The caller supplies the finalized root-hash from consensus. The host
    /// recomputes its current root-hash first and refuses to serve if it has
    /// already advanced, preventing a node from labeling current module state as
    /// an older height. Because this borrows `&self`, it can only run outside the
    /// mutable `submit_at` block lifecycle; module roots and state-sync handles
    /// therefore come from committed state, not an in-flight staged overlay.
    pub fn capture_finalized_snapshot(
        &self,
        finalized: FinalizedBlock,
    ) -> Result<FinalizedSnapshot, SnapshotError> {
        self.snapshot_at(
            finalized.height,
            Some(finalized.root_hash),
            CapturePayloads::All,
            || Duration::ZERO,
        )
        .map(|(snapshot, _)| snapshot)
    }

    /// capture the committed registry view at the host's CURRENT root, which
    /// this computes rather than verifying. For a caller that has no
    /// independently-known finalized hash to check against, passing
    /// [`Host::root_hash`] into [`Host::capture_finalized_snapshot`] is a
    /// tautology that costs a SECOND full pass over every module root — and
    /// `root()` is a whole state serialization + hash for a map-backed module.
    ///
    /// `now` is the CALLER's clock (the host owns none, by design): it is read
    /// around each module's capture and the deltas are returned ALONGSIDE the
    /// snapshot — what a module cost is wall-clock, so it is not part of the
    /// boundary's identity and must stay out of its `Eq`. Registry order,
    /// degraded modules included. Pass `|| Duration::ZERO` to opt out.
    pub fn capture_current_snapshot(
        &self,
        height: u64,
        payloads: CapturePayloads,
        now: impl FnMut() -> Duration,
    ) -> (FinalizedSnapshot, Vec<(ModuleId, Duration)>) {
        self.snapshot_at(height, None, payloads, now)
            .expect("an unverified capture has no root to mismatch")
    }

    /// the one capture body: every module root computed EXACTLY ONCE, the
    /// composite root derived from those, and the state-sync handles taken only
    /// after the root check passes (a mismatched capture must not pay to
    /// materialize snapshot bytes it will throw away).
    fn snapshot_at(
        &self,
        height: u64,
        expected: Option<StateRoot>,
        payloads: CapturePayloads,
        mut now: impl FnMut() -> Duration,
    ) -> Result<(FinalizedSnapshot, Vec<(ModuleId, Duration)>), SnapshotError> {
        let mut roots: Vec<(ModuleId, StateRoot)> = Vec::with_capacity(self.registry.len());
        let mut capture_cost: Vec<(ModuleId, Duration)> = Vec::with_capacity(self.registry.len());
        for (id, module) in self.registry.iter() {
            let started = now();
            let root = module.root();
            roots.push((id.clone(), root));
            capture_cost.push((id.clone(), now().saturating_sub(started)));
        }

        let commitments = roots
            .iter()
            .map(|(id, root)| {
                (
                    id.clone(),
                    module_commitment(*root, self.module_code_hash(id).as_deref()),
                )
            })
            .collect::<Vec<_>>();
        let actual = global_root_of(&commitments);
        if let Some(expected) = expected
            && actual != expected
        {
            return Err(SnapshotError::RootHashMismatch { expected, actual });
        }

        let mut modules = Vec::with_capacity(self.registry.len());
        let mut degraded = Vec::new();
        for ((module, (id, root)), cost) in self
            .registry
            .values()
            .zip(roots.iter())
            .zip(capture_cost.iter_mut())
        {
            let started = now();
            // `block_durable` is only consulted where it can change the
            // answer: its default impl reads `state_sync_handle`, and under
            // `All` that would be a second (for forge, very expensive) call.
            let reopens_from_own_disk =
                matches!(payloads, CapturePayloads::InMemoryCohort) && module.block_durable();
            let handle = if reopens_from_own_disk {
                Ok(StateSyncHandle::Unsupported {
                    reason: SELF_DURABLE_NO_PAYLOAD.into(),
                })
            } else {
                module.state_sync_handle()
            };
            cost.1 += now().saturating_sub(started);
            match handle {
                Ok(state_sync) => modules.push(ModuleSnapshot {
                    id: id.clone(),
                    root: *root,
                    code_hash: module.code_hash(),
                    state_sync,
                }),
                Err(reason) => degraded.push(DegradedModule {
                    id: id.clone(),
                    root: *root,
                    code_hash: module.code_hash(),
                    reason,
                }),
            }
        }

        Ok((
            FinalizedSnapshot {
                height,
                root_hash: actual,
                modules,
                degraded,
            },
            capture_cost,
        ))
    }

    /// apply one inbound message as a block: route, execute, drain follow-ups,
    /// then COMMIT the block at its boundary. `height`/`consensus_time` are
    /// block-constant; the root op's origin is `External`, follow-ups carry
    /// `Origin::Module(emitter)`.
    ///
    /// ## per-block atomicity
    ///
    /// a module STAGES its writes during the drain and never commits mid-block.
    /// the host owns the commit lifecycle: on a clean drain it calls
    /// [`Module::commit_block`] on every touched module (deterministic registry
    /// order) to publish their staged writes together; on ANY drain failure (a
    /// later `execute` erroring, or [`Error::BudgetExceeded`]) it calls
    /// [`Module::abort_block`] on every touched module, so a half-applied block
    /// leaves NO trace — every module root is byte-identical to its pre-block
    /// value. the root-hash is recomposed AFTER the commit, so it reflects exactly
    /// the committed state.
    ///
    /// ## the two failure modes
    ///
    /// a [`SubmitError::Rejected`] means the drain failed DETERMINISTICALLY and
    /// the abort path rolled every touched module back — same on every honest
    /// validator, safe to treat as a no-op. a [`SubmitError::Fatal`] means a
    /// boundary hook itself failed on THIS node: a commit fault leaves the block
    /// half-published (modules earlier in registry order already committed), an
    /// abort fault leaves a stage that may leak into a later block. no cleanup
    /// is attempted for either — any further boundary calls would run against a
    /// registry already known to be inconsistent, manufacturing a THIRD state no
    /// validator agreed on. the caller must fail-stop.
    pub async fn submit(&mut self, msg: Msg) -> Result<BlockOutcome, SubmitError> {
        self.submit_at(BlockContext::default(), msg).await
    }

    /// apply one inbound message as a block with an EXPLICIT [`BlockContext`] —
    /// the agreed `height` / `consensus_time` and the root op's `origin`, sourced
    /// from the finalized view by the ordered lane. otherwise identical to
    /// [`Host::submit`] (which is just `submit_at(BlockContext::default(), msg)`).
    pub async fn submit_at(
        &mut self,
        ctx: BlockContext,
        msg: Msg,
    ) -> Result<BlockOutcome, SubmitError> {
        // every module dispatched this block, in deterministic order — the set
        // the host commits or aborts at the boundary.
        let mut touched: BTreeSet<ModuleId> = BTreeSet::new();

        match self.drain(ctx, msg, &mut touched).await {
            Ok((events, dispatches)) => {
                // clean drain: publish every touched module's staged writes. this
                // is the ONLY place a module's state advances, so recompose the
                // root-hash AFTER. a commit failure is FATAL, not a rejection: the
                // modules before this one in registry order already published,
                // so the block is half-committed on this node alone.
                for id in &touched {
                    if let Some(m) = self.registry.get_mut(id) {
                        m.commit_block().await.map_err(|source| {
                            SubmitError::Fatal(FatalError {
                                module: id.clone(),
                                phase: BoundaryPhase::Commit,
                                source,
                            })
                        })?;
                    }
                }
                Ok(BlockOutcome {
                    root_hash: self.root_hash(),
                    events,
                    dispatches,
                })
            }
            Err(e) => {
                // failure anywhere in the drain: discard every touched module's
                // staged writes. no root moves — the block leaves no trace. an
                // abort failure is FATAL, not a rejection: that module's stage
                // may leak into a later block's commit. keep aborting the rest
                // (each un-aborted stage is one more leak) but report the FIRST
                // fault — the node is stopping either way.
                let mut fatal: Option<FatalError> = None;
                for id in &touched {
                    if let Some(m) = self.registry.get_mut(id)
                        && let Err(source) = m.abort_block().await
                    {
                        fatal.get_or_insert(FatalError {
                            module: id.clone(),
                            phase: BoundaryPhase::Abort,
                            source,
                        });
                    }
                }
                match fatal {
                    Some(f) => Err(SubmitError::Fatal(f)),
                    None => Err(SubmitError::Rejected(e)),
                }
            }
        }
    }

    /// apply a BATCH of ops as ONE block: per-op isolation, a SINGLE commit
    /// boundary, and ONE post-batch root-hash shared by every applied member.
    ///
    /// each op is drained in input order on top of the prior accepted ops' staged
    /// writes (read-your-writes across members). an op that rejects
    /// DETERMINISTICALLY is ISOLATED — its stage is rolled back and every already-
    /// accepted op is replayed — so the committed state equals exactly the accepted
    /// subset applied in order (applying `[A, B]` where `B` rejects lands the same
    /// state as applying `[A]` alone). the once-per-block System injections
    /// (`Advance` / `DeliverPending`), computed against PRE-batch committed state,
    /// drain once after the members; then the whole touched set commits together.
    ///
    /// the two failure modes match [`Host::submit_at`]: a boundary hook failing is
    /// a node-local [`SubmitError::Fatal`] (fail-stop); a member rejecting is
    /// folded into that member's [`MemberOutcome::Rejected`], never a whole-batch
    /// error. an empty `ops` is a valid empty block — no members, injections drain
    /// once and the touched set commits (a no-op when nothing was pending).
    pub async fn submit_block(
        &mut self,
        ctx: BlockContext,
        ops: Vec<(Origin, Msg)>,
    ) -> Result<BatchOutcome, SubmitError> {
        let ops = ops
            .into_iter()
            .map(|(origin, msg)| BlockOp::bare(origin, msg))
            .collect();
        self.apply_block(ctx, ops, None).await
    }

    /// [`Host::submit_block`] over full [`BlockOp`]s — the entry the node's
    /// frame drain uses, so each op carries its frame's content id.
    pub async fn submit_block_ops(
        &mut self,
        ctx: BlockContext,
        ops: Vec<BlockOp>,
    ) -> Result<BatchOutcome, SubmitError> {
        self.apply_block(ctx, ops, None).await
    }

    /// RECOVERY-ONLY selective-commit variant of [`Host::submit_block`]: identical
    /// per-op isolation and single-root-hash composition, but at the boundary it
    /// partitions the touched set — commit the modules in `commit_only`, abort the
    /// rest. this heals a TORN block at boot: a block that committed a
    /// per-block-durable disk substrate (already at its sealed post-root on disk)
    /// but whose in-memory cohort was rolled back to the checkpoint; replay re-runs
    /// the frame and commits ONLY the at-pre cohort, aborting the durable substrate
    /// (re-committing it would move its op-log root and fork). NOT the live path.
    /// takes full [`BlockOp`]s: a journaled frame must re-run on the heal
    /// exactly as it ran live, or the sealed roots cannot reproduce.
    pub async fn submit_block_committing(
        &mut self,
        ctx: BlockContext,
        ops: Vec<BlockOp>,
        commit_only: &BTreeSet<ModuleId>,
    ) -> Result<BatchOutcome, SubmitError> {
        self.apply_block(ctx, ops, Some(commit_only)).await
    }

    /// the shared batch engine behind [`Host::submit_block`] /
    /// [`Host::submit_block_committing`]. `commit_only == None` commits every
    /// touched module (the live path); `Some(set)` partitions the boundary
    /// (recovery). see [`Host::submit_block`] for the algorithm and invariants.
    async fn apply_block(
        &mut self,
        ctx: BlockContext,
        ops: Vec<BlockOp>,
        commit_only: Option<&BTreeSet<ModuleId>>,
    ) -> Result<BatchOutcome, SubmitError> {
        // block-constant across every dispatch this block — the agreed values.
        let height = ctx.height;
        let consensus_time = ctx.consensus_time;

        // 1. the once-per-block System injections, computed ONCE against PRE-batch
        // committed state — the "results staged by this very block are invisible
        // here" invariant, evaluated BEFORE any member stages. same order as the
        // single-op drain: the registry `Advance` (one tick reconciling every
        // armed code swap), then `DeliverPending`. drained once, after every
        // member, below (step 4).
        let mut injections: VecDeque<(Origin, Msg)> = VecDeque::new();
        if let Some(advance) = self.pending_modules_advance(height).await {
            injections.push_back((Origin::System, advance));
        }
        if let Some(deliver) = self.pending_deliveries().await {
            injections.push_back((Origin::System, deliver));
        }

        // 2. per-op isolation: each input op is one unit, drained against its OWN
        // touched set (merged into the block's on the way out). the modules'
        // staging accumulates ACROSS units (never committed mid-batch), so a unit
        // that STAGED and then failed is entangled with the accepted units and
        // costs a rollback + replay; a unit whose own set is EMPTY reached no
        // module at all and costs nothing. the replay budget bounds the rest.
        let mut touched: BTreeSet<ModuleId> = BTreeSet::new();
        let mut accepted: Vec<AcceptedUnit> = Vec::new();
        let mut results: Vec<Option<MemberOutcome>> = (0..ops.len()).map(|_| None).collect();
        let mut replays: u32 = 0;

        for (i, op) in ops.into_iter().enumerate() {
            let BlockOp {
                origin,
                msg,
                frame: _frame,
            } = op;

            let mut ev: Vec<Event> = Vec::new();
            let mut di: Vec<DispatchRecord> = Vec::new();
            let mut unit_touched: BTreeSet<ModuleId> = BTreeSet::new();
            let queue: VecDeque<(Origin, Msg)> = VecDeque::from([(origin.clone(), msg.clone())]);
            let verdict = self
                .drain_queue(
                    height,
                    consensus_time,
                    queue,
                    &mut unit_touched,
                    &mut ev,
                    &mut di,
                )
                .await;
            // whatever this unit reached is part of the block's stage from here
            // (aborted below on a rejection, committed at the boundary otherwise).
            let unit_staged = !unit_touched.is_empty();
            touched.append(&mut unit_touched);
            match verdict {
                Ok(()) => {
                    accepted.push(AcceptedUnit {
                        origin,
                        msg,
                        member: i,
                        events: ev,
                        dispatches: di,
                    });
                    // authoritative trace is written after the loop (step 3);
                    // this placeholder is overwritten there.
                    results[i] = Some(MemberOutcome::Applied {
                        dispatches: Vec::new(),
                    });
                }
                Err(reason) => {
                    results[i] = Some(MemberOutcome::Rejected {
                        reason: reason.to_string(),
                    });
                    // an unknown target or an acl refusal is rejected BEFORE any
                    // module is reached, so it staged nothing and the accepted
                    // units' stage already IS the state a rollback would rebuild
                    // — the whole point of the per-unit set.
                    if !unit_staged {
                        continue;
                    }
                    // ISOLATE: this unit's partial stage is entangled with the
                    // accepted units' stage (one shared per-module stage), so
                    // roll the WHOLE stage back, then replay only the accepted
                    // units to rebuild their writes without this one.
                    self.abort_all(&mut touched).await?;
                    self.replay_accepted(height, consensus_time, &mut accepted, &mut touched)
                        .await?;
                    replays += 1;
                    if replays > MAX_BLOCK_REPLAYS {
                        break;
                    }
                }
            }
        }

        // the replay budget ran out: every member the loop did not reach is
        // rejected UNEXECUTED. the budget is a function of the block alone
        // (member order and their verdicts), so every validator rejects exactly
        // the same suffix — deterministic, like any other rejection.
        for slot in results.iter_mut().filter(|s| s.is_none()) {
            *slot = Some(MemberOutcome::Rejected {
                reason: format!("block replay budget exhausted ({MAX_BLOCK_REPLAYS})"),
            });
        }

        // 3. write each accepted unit's authoritative trace into its slot and
        // accumulate the aggregate events in execution order (input order).
        let mut events: Vec<Event> = Vec::new();
        for unit in accepted {
            events.extend(unit.events);
            results[unit.member] = Some(MemberOutcome::Applied {
                dispatches: unit.dispatches,
            });
        }

        // 4. drain the once-per-block injections ONCE, on top of the accepted
        // members' staged writes. an injection drain error is handled exactly like
        // submit_at's drain failure: abort the whole touched set — fatal on an
        // abort fault, else the deterministic rejection.
        let mut system_dispatches: Vec<DispatchRecord> = Vec::new();
        let mut sys_events: Vec<Event> = Vec::new();
        if let Err(reason) = self
            .drain_queue(
                height,
                consensus_time,
                injections,
                &mut touched,
                &mut sys_events,
                &mut system_dispatches,
            )
            .await
        {
            self.abort_all(&mut touched).await?;
            return Err(SubmitError::Rejected(reason));
        }
        events.extend(sys_events);

        // 5. COMMIT once — the single boundary for the whole batch. the live path
        // commits every touched module; recovery partitions on `commit_only`
        // (commit those in the set, abort the rest). either hook failing is FATAL.
        match commit_only {
            None => {
                for id in touched.iter() {
                    if let Some(m) = self.registry.get_mut(id) {
                        m.commit_block().await.map_err(|source| {
                            SubmitError::Fatal(FatalError {
                                module: id.clone(),
                                phase: BoundaryPhase::Commit,
                                source,
                            })
                        })?;
                    }
                }
            }
            Some(set) => {
                for id in touched.iter() {
                    if let Some(m) = self.registry.get_mut(id) {
                        if set.contains(id) {
                            m.commit_block().await.map_err(|source| {
                                SubmitError::Fatal(FatalError {
                                    module: id.clone(),
                                    phase: BoundaryPhase::Commit,
                                    source,
                                })
                            })?;
                        } else {
                            m.abort_block().await.map_err(|source| {
                                SubmitError::Fatal(FatalError {
                                    module: id.clone(),
                                    phase: BoundaryPhase::Abort,
                                    source,
                                })
                            })?;
                        }
                    }
                }
            }
        }

        // 6. ONE root-hash over the committed registry, shared by every member.
        Ok(BatchOutcome {
            root_hash: self.root_hash(),
            members: results.into_iter().map(Option::unwrap).collect(),
            events,
            system_dispatches,
        })
    }

    /// abort every module in `touched` (deterministic registry order), then clear
    /// the set. best-effort: keep aborting after a fault (each un-aborted stage is
    /// one more leak) but return the FIRST fault as a fatal boundary error. shared
    /// by [`Host::apply_block`]'s isolation and injection-failure paths.
    async fn abort_all(&mut self, touched: &mut BTreeSet<ModuleId>) -> Result<(), SubmitError> {
        let mut fatal: Option<FatalError> = None;
        for id in touched.iter() {
            if let Some(m) = self.registry.get_mut(id)
                && let Err(source) = m.abort_block().await
            {
                fatal.get_or_insert(FatalError {
                    module: id.clone(),
                    phase: BoundaryPhase::Abort,
                    source,
                });
            }
        }
        touched.clear();
        match fatal {
            Some(f) => Err(SubmitError::Fatal(f)),
            None => Ok(()),
        }
    }

    /// replay every accepted unit after an isolation rollback, rebuilding
    /// their staged writes and overwriting their authoritative traces. an accepted unit drained Ok in this same
    /// context before, so a reject on replay is NON-DETERMINISM → fatal.
    async fn replay_accepted(
        &mut self,
        height: u64,
        consensus_time: u64,
        accepted: &mut [AcceptedUnit],
        touched: &mut BTreeSet<ModuleId>,
    ) -> Result<(), SubmitError> {
        for unit in accepted.iter_mut() {
            let mut rev: Vec<Event> = Vec::new();
            let mut rdi: Vec<DispatchRecord> = Vec::new();
            let rq: VecDeque<(Origin, Msg)> =
                VecDeque::from([(unit.origin.clone(), unit.msg.clone())]);
            self.drain_queue(height, consensus_time, rq, touched, &mut rev, &mut rdi)
                .await
                .map_err(|re| {
                    // the kernel's ONLY in-band detector of module
                    // non-determinism, and the most fork-relevant event that
                    // can occur — a module that rejects on replay what it
                    // accepted on first execution. it was being wrapped into
                    // a FatalError mislabelled as an Abort-phase boundary
                    // fault and returned in SILENCE.
                    tracing::error!(
                        target: "ducktape::consensus",
                        module = %unit.msg.target,
                        error = %re,
                        "NON-DETERMINISTIC module: rejected on replay what it \
                         accepted during per-op isolation — this node's state \
                         may diverge from its peers"
                    );
                    SubmitError::Fatal(FatalError {
                        module: unit.msg.target.clone(),
                        phase: BoundaryPhase::Abort,
                        source: Error::Module(format!(
                            "non-deterministic reject replaying accepted batch \
                         member during per-op isolation: {re}"
                        )),
                    })
                })?;
            unit.events = rev;
            unit.dispatches = rdi;
        }
        Ok(())
    }

    /// the block's dispatch DRAIN: route the root op, run its `execute`, and
    /// re-dispatch every emitted follow-up FIFO until the queue empties or the
    /// dispatch budget is hit. modules only STAGE here — nothing is committed;
    /// [`submit`](Self::submit) commits (or aborts) the touched set at the block
    /// boundary. every dispatched target is recorded in `touched` so the boundary
    /// can reach exactly the modules that may hold staged writes.
    async fn drain(
        &mut self,
        ctx: BlockContext,
        msg: Msg,
        touched: &mut BTreeSet<ModuleId>,
    ) -> Result<(Vec<Event>, Vec<DispatchRecord>), Error> {
        // block-constant across every dispatch this block — the agreed values.
        let height = ctx.height;
        let consensus_time = ctx.consensus_time;

        // the root op carries the real submitter's origin; follow-ups override.
        let mut queue: VecDeque<(Origin, Msg)> = VecDeque::from([(ctx.origin, msg)]);

        // DETERMINISTIC ACTIVATION INJECTION. at a finalized boundary where the
        // committed `modules` registry holds an armed code swap, append EXACTLY
        // ONE System-origin `Advance` so the module reconciles its own
        // root-hashed state in-block (flip every armed active hash — the
        // consensus commitment to the new code; the actual component swap is
        // realized out-of-block by `realize_module_swaps`). it rides this drain
        // (not the respawn side-path), so live, recovery-replay, and state-sync
        // nodes all reconstruct it byte-for-byte, and it frees the
        // at-most-one-pending slot after activation. INERT until the module is
        // registered — `pending_modules_advance` returns `None` when absent.
        if let Some(advance) = self.pending_modules_advance(height).await {
            queue.push_back((Origin::System, advance));
        }
        // DETERMINISTIC DELIVERY INJECTION: when the committed dispatch
        // mailbox holds results, append EXACTLY ONE System-origin
        // `DeliverPending` so the dispatch module hands them to their
        // receivers THIS block — at least one block after each result
        // committed (the mailbox read is committed state, so results staged
        // by this very block are invisible here). INERT until the module is
        // registered — `pending_deliveries` returns `None` when absent.
        if let Some(deliver) = self.pending_deliveries().await {
            queue.push_back((Origin::System, deliver));
        }

        // run the whole queue (root op + the once-per-block injections) as ONE
        // drain into fresh trace vecs. the extracted queue-runner is what
        // submit_block reuses — once per member, then once for the injections.
        let mut events: Vec<Event> = Vec::new();
        let mut dispatches: Vec<DispatchRecord> = Vec::new();
        self.drain_queue(height, consensus_time, queue, touched, &mut events, &mut dispatches)
            .await?;
        Ok((events, dispatches))
    }

    /// the extracted dispatch-loop queue-runner: pop `(origin, msg)` FIFO, run
    /// each target's `execute` (remove-execute-reinsert), record the deterministic
    /// [`DispatchRecord`], and push emitted follow-ups back as `Origin::Module`
    /// ops until the queue empties or [`MAX_DISPATCHES`] is hit. modules only
    /// STAGE; the caller owns the commit/abort boundary. staged writes and
    /// `touched` accumulate across calls, so `submit_block` can drain members one
    /// at a time on top of one another. `events` / `dispatches` are appended to
    /// (never cleared), so a caller can thread one set of sinks across several
    /// calls or hand in fresh ones per call. the dispatch budget is per-call:
    /// each queue-run gets a fresh [`MAX_DISPATCHES`].
    /// the acl dispatch gate: does `submitter` (a verified external origin)
    /// hold the standing `target` requires? consults the acl module's
    /// staged-over-committed policy and resolves the principal against the
    /// valset/identity siblings — deterministic on every node, because the
    /// drain order and the sibling state are. FAIL-OPEN on an ABSENT acl
    /// module (a net without the module is an open network, byte-identical to
    /// an empty table); FAIL-CLOSED on a set policy whose standing set cannot
    /// be read (a net that demands validator standing but composes no valset
    /// grants nobody that standing).
    async fn require_submit_standing(&self, submitter: &[u8], target: &str) -> Result<(), Error> {
        let Ok(reply) = self
            .query(
                ACL_MODULE_ID,
                &acl::encode_query(&acl::AclQuery::PolicyFor {
                    target: target.into(),
                }),
            )
            .await
        else {
            return Ok(()); // no acl module composed — open network.
        };
        let policy = match acl::decode_reply(&reply) {
            Ok(acl::AclReply::PolicyFor(policy)) => policy,
            Ok(_) | Err(_) => return Ok(()),
        };
        let Some(required) = policy else {
            return Ok(()); // no entry, no "*" fallback — open by default.
        };
        let holds = match required {
            acl::Standing::Open => true,
            acl::Standing::Validator => self.valset_tier_holds(submitter, false).await,
            acl::Standing::Node => self.valset_tier_holds(submitter, true).await,
            acl::Standing::User => self.identity_account_holds(submitter).await,
        };
        if holds {
            return Ok(());
        }
        Err(Error::Module(format!(
            "acl: target {target} requires {} standing — the submitting origin holds none",
            required.as_str()
        )))
    }

    /// is `submitter` in valset's validator tier (`with_residents: false`) or
    /// in validators ∪ residents (`true`)? an unreadable tier is an empty
    /// tier — fail-closed for a policy that names it.
    async fn valset_tier_holds(&self, submitter: &[u8], with_residents: bool) -> bool {
        let tier = |q: valset::ValsetQuery| async move {
            let bytes = self.query(VALSET_MODULE_ID, &valset::encode_query(&q)).await;
            match bytes.map(|b| valset::decode_reply(&b)) {
                Ok(Ok(valset::ValsetReply::Validators(keys)))
                | Ok(Ok(valset::ValsetReply::Residents(keys))) => keys,
                _ => Vec::new(),
            }
        };
        let in_validators = tier(valset::ValsetQuery::Validators)
            .await
            .iter()
            .any(|k| k.as_slice() == submitter);
        if in_validators {
            return true;
        }
        with_residents
            && tier(valset::ValsetQuery::Residents)
                .await
                .iter()
                .any(|k| k.as_slice() == submitter)
    }

    /// does `submitter` belong to an identity account? an unreadable reply is
    /// "no" — fail-closed for a policy that names user standing. a node key
    /// is never an account member by itself: only a user-signed origin holds
    /// user standing.
    async fn identity_account_holds(&self, submitter: &[u8]) -> bool {
        let query = identity::IdentityQuery::OfKey {
            key: submitter.to_vec(),
        };
        let bytes = self
            .query(IDENTITY_MODULE_ID, &identity::encode_query(&query))
            .await;
        matches!(
            bytes.map(|b| identity::decode_reply(&b)),
            Ok(Ok(identity::IdentityReply::Account(Some(_))))
        )
    }

    async fn drain_queue(
        &mut self,
        height: u64,
        consensus_time: u64,
        mut queue: VecDeque<(Origin, Msg)>,
        touched: &mut BTreeSet<ModuleId>,
        events: &mut Vec<Event>,
        dispatches: &mut Vec<DispatchRecord>,
    ) -> Result<(), Error> {
        let mut n: u32 = 0;

        while let Some((origin, msg)) = queue.pop_front() {
            n += 1;
            if n > MAX_DISPATCHES {
                return Err(Error::BudgetExceeded);
            }

            // the acl dispatch gate: an EXTERNAL submitter must hold the
            // target's required standing (allow-all when no policy is set).
            // module follow-ups and system injections are the host's own
            // machinery and bypass policy. a refusal is a deterministic
            // rejection — the identical no-op every honest validator makes,
            // exactly like a module rejection.
            if let Origin::External(submitter) = &origin {
                self.require_submit_standing(submitter, &msg.target).await?;
            }

            // remove → owned module, decoupled from the map's borrow.
            let mut me = self
                .registry
                .remove(&msg.target)
                .ok_or_else(|| Error::UnknownModule(msg.target.clone()))?;
            // record it as touched only after a successful remove: an unknown
            // target never staged anything, but everything dispatched before it
            // did and must still be aborted.
            touched.insert(msg.target.clone());

            // dispatch-start snapshot: the rest of the registry, plus self.
            let mut snapshot: BTreeMap<ModuleId, StateRoot> = self
                .registry
                .iter()
                .map(|(k, m)| (k.clone(), m.root()))
                .collect();
            snapshot.insert(msg.target.clone(), me.root());

            // keep the trigger origin for the dispatch record; the env takes a clone.
            let trigger = origin;
            let mut ctx = HostCtx {
                env: Env {
                    height,
                    consensus_time,
                    origin: trigger.clone(),
                    me: msg.target.clone(),
                },
                snapshot,
                registry: &self.registry, // the rest — for query routing
                out_msgs: Vec::new(),
                out_events: Vec::new(),
                out_output: None,
                out_assigned: Vec::new(),
            };

            // owned `me` (&mut) and `ctx` (holding &rest) are disjoint borrows,
            // so they compose across this await. deterministic awaits only.
            let res = me.execute(&mut ctx, &msg).await;

            // destructure releases the &registry borrow → map is mutable again.
            let HostCtx {
                out_msgs,
                out_events,
                out_output,
                out_assigned,
                ..
            } = ctx;

            // reinsert BEFORE propagating any error — a module never vanishes.
            self.registry.insert(msg.target.clone(), me);
            res?;

            // an oversized declared output is a deterministic REJECTION of the
            // op, never a truncation (the saga oversize discipline: bytes that
            // would ride a consensus lane are capped loudly at the source).
            if let Some(out) = &out_output
                && out.len() > sdk::MAX_OUTPUT_BYTES
            {
                return Err(Error::Module(format!(
                    "op output exceeds cap ({} > {})",
                    out.len(),
                    sdk::MAX_OUTPUT_BYTES
                )));
            }
            if out_assigned.len() > sdk::MAX_ASSIGNED_BYTES {
                return Err(Error::Module(format!(
                    "op assigned stamp exceeds cap ({} > {})",
                    out_assigned.len(),
                    sdk::MAX_ASSIGNED_BYTES
                )));
            }

            // record this (successful) dispatch for the deterministic trace. only
            // committed blocks yield a BlockOutcome, so a later abort discards the
            // whole trace with the block — it never reports a rolled-back dispatch.
            dispatches.push(DispatchRecord {
                module: msg.target.clone(),
                origin: trigger,
                // partial move: only `msg.target` is read below.
                payload: msg.payload,
                emitted_msgs: out_msgs.len(),
                emitted_events: out_events.len(),
                assigned: out_assigned,
            });

            // local-only re-entry: emitted msgs become follow-up ops, never
            // re-broadcast. events leave the state machine.
            for m in out_msgs {
                queue.push_back((Origin::Module(msg.target.clone()), m));
            }
            events.extend(out_events);
        }

        Ok(())
    }
}

/// the host's `Ctx` impl, rebuilt per dispatch. `snapshot` is owned (so
/// `module_root` works for self too, with no map borrow); `registry` is the rest
/// of the modules, borrowed only for live `query` routing.
struct HostCtx<'a> {
    env: Env,
    snapshot: BTreeMap<ModuleId, StateRoot>,
    registry: &'a BTreeMap<ModuleId, Box<dyn Module>>,
    out_msgs: Vec<Msg>,
    out_events: Vec<Event>,
    /// the op's declared output ([`Ctx::set_output`]), staged with the
    /// dispatch; the drain caps it. DEAD: nothing reads it — its only consumer
    /// was the deleted continuation relay.
    out_output: Option<Vec<u8>>,
    /// the dispatch's assigned stamp ([`Ctx::set_assigned`]), staged with the
    /// dispatch; the drain caps it and records it on the trace.
    out_assigned: Vec<u8>,
}

#[async_trait::async_trait(?Send)]
impl Ctx for HostCtx<'_> {
    fn env(&self) -> &Env {
        &self.env
    }

    fn module_root(&self, target: &str) -> Option<StateRoot> {
        self.snapshot.get(target).copied()
    }

    async fn query(&self, target: &str, req: &[u8]) -> Result<Vec<u8>, Error> {
        if target == self.env.me {
            return Err(Error::SelfQuery);
        }
        match self.registry.get(target) {
            Some(m) => {
                let target = target.to_string();
                let ctx = ReadOnlyQueryCtx {
                    env: Env {
                        height: self.env.height,
                        consensus_time: self.env.consensus_time,
                        origin: self.env.origin.clone(),
                        me: target.clone(),
                    },
                    snapshot: &self.snapshot,
                    registry: self.registry,
                    active: BTreeSet::from([self.env.me.clone(), target]),
                };
                m.query_with(&ctx, req).await
            }
            None => Err(Error::UnknownModule(target.to_string())),
        }
    }

    fn emit_msg(&mut self, msg: Msg) {
        self.out_msgs.push(msg);
    }

    fn set_output(&mut self, bytes: Vec<u8>) {
        self.out_output = Some(bytes);
    }

    fn set_assigned(&mut self, bytes: Vec<u8>) {
        self.out_assigned = bytes;
    }

    fn emit_event(&mut self, ev: Event) {
        self.out_events.push(ev);
    }
}

/// Query projections can also be filtered views over other registered modules.
/// This context carries the host snapshot and rejects nested query cycles.
struct ReadOnlyQueryCtx<'a> {
    env: Env,
    snapshot: &'a BTreeMap<ModuleId, StateRoot>,
    registry: &'a BTreeMap<ModuleId, Box<dyn Module>>,
    active: BTreeSet<ModuleId>,
}

#[async_trait::async_trait(?Send)]
impl Ctx for ReadOnlyQueryCtx<'_> {
    fn env(&self) -> &Env {
        &self.env
    }

    fn module_root(&self, target: &str) -> Option<StateRoot> {
        self.snapshot.get(target).copied()
    }

    async fn query(&self, target: &str, req: &[u8]) -> Result<Vec<u8>, Error> {
        if target == self.env.me {
            return Err(Error::SelfQuery);
        }
        if self.active.contains(target) {
            return Err(Error::Module(format!("query cycle: {target}")));
        }
        match self.registry.get(target) {
            Some(m) => {
                let target = target.to_string();
                let mut active = self.active.clone();
                active.insert(target.clone());
                let ctx = ReadOnlyQueryCtx {
                    env: Env {
                        height: self.env.height,
                        consensus_time: self.env.consensus_time,
                        origin: self.env.origin.clone(),
                        me: target,
                    },
                    snapshot: self.snapshot,
                    registry: self.registry,
                    active,
                };
                m.query_with(&ctx, req).await
            }
            None => Err(Error::UnknownModule(target.to_string())),
        }
    }

    fn emit_msg(&mut self, _msg: Msg) {}

    fn emit_event(&mut self, _ev: Event) {}
}
