//! the host — the deterministic state-machine spine.
//!
//! a [`Host`] owns a registry of [`Module`]s and turns an inbound [`Msg`] into a
//! block: it routes the message to its target module, awaits the (deterministic)
//! `execute`, then drains the intents that execute emitted. emitted [`Msg`]s are
//! re-dispatched as LOCAL-ONLY follow-up ops (never re-broadcast); emitted
//! [`Event`]s/[`Effect`]s are collected and handed back for the effectful node
//! layer (out of scope this slice). after the drain, the app-hash is recomposed
//! over the registry via [`global_root`].
//!
//! ## determinism
//!
//! `submit` is a pure function of `(registry state, msg, env)`:
//! - the registry is a [`BTreeMap`], so snapshot + app-hash iteration is sorted
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

use sdk::{
    Ctx, Env, Error, Event, Module, ModuleId, Msg, Origin, ResolverSyncTarget, StateRoot,
    StateSyncHandle,
};
use sha2::{Digest, Sha256};

pub mod worker;

/// compute the global app-hash over `modules` — the composition consensus
/// commits to: a deterministic hash over every module's `(id, root)`. because
/// a module's own [`StateRoot`] already commits to its children (a qmdb merkle
/// root commits to its keys; a git HEAD oid commits to the whole repo tree),
/// this one level on top yields the full two-level authentication tree.
///
/// determinism is the whole job: every validator must produce a byte-identical
/// global root or the chain forks — so modules are sorted by id, and each id is
/// length-prefixed before hashing (otherwise ("ab", r) and ("a", "b"||r) would
/// collide). deliberately a plain sorted hash, NOT a qmdb-of-heads: qmdb's root
/// is an order-dependent HISTORY commitment, while an app-hash must be
/// `f(current state)` — order-independent + idempotent — so a state-synced node
/// computes the same root. upgrade to a small merkle tree only when a light
/// client needs log-n membership proofs.
pub fn global_root(modules: &[&dyn Module]) -> StateRoot {
    let mut pairs: Vec<(ModuleId, StateRoot)> =
        modules.iter().map(|m| (m.id(), m.root())).collect();
    pairs.sort_by(|a, b| a.0.cmp(&b.0));

    let mut h = Sha256::new();
    h.update((pairs.len() as u64).to_le_bytes());
    for (id, root) in &pairs {
        h.update((id.len() as u64).to_le_bytes());
        h.update(id.as_bytes());
        h.update(root.0);
    }
    StateRoot(h.finalize().into())
}

/// Canonical fingerprint of a module set's committed-state schemas.
///
/// Entries are sorted by module id before hashing, so registry construction
/// order is irrelevant. Length-prefixing keeps ids unambiguous; the domain tag
/// prevents this digest from being confused with an app hash.
pub fn state_schema_fingerprint<'a>(modules: impl IntoIterator<Item = (&'a str, u32)>) -> [u8; 32] {
    let mut modules: Vec<(&str, u32)> = modules.into_iter().collect();
    modules.sort_unstable_by(|a, b| a.0.cmp(b.0));
    let mut h = Sha256::new();
    h.update(b"ducktape-state-schema-v1");
    h.update((modules.len() as u64).to_le_bytes());
    for (id, revision) in modules {
        h.update((id.len() as u64).to_le_bytes());
        h.update(id.as_bytes());
        h.update(revision.to_le_bytes());
    }
    h.finalize().into()
}

/// hard cap on dispatches per `submit` (the root op plus all follow-ups). a
/// consensus/genesis constant — identical on every node — so the local re-entry
/// loop is guaranteed to terminate regardless of module behavior.
pub const MAX_DISPATCHES: u32 = 1024;

/// the genesis-constant module id the `upgrade` module registers under. read by
/// [`Host::effective_version`] to derive the block's protocol version; absent
/// before the coordinated retrofit, in which case the derivation falls back to
/// [`BASELINE_VERSION`].
pub const UPGRADE_MODULE_ID: &str = "upgrade";

/// the genesis-constant module id the `dispatch` module registers under. read
/// by the drain's delivery injection ([`Host::pending_deliveries`]); absent on
/// a net without the module, in which case nothing is ever injected.
const DISPATCH_MODULE_ID: &str = dispatch::DEFAULT_DISPATCH_TARGET;

/// the genesis-constant module id the `modreg` code registry registers under.
/// read by the boundary code-swap realization ([`Host::realize_module_swaps`])
/// and the drain's [`Host::pending_modreg_advance`] injection; absent on a net
/// without the module, in which case no swap is ever realized or injected.
pub const MODREG_MODULE_ID: &str = modreg::DEFAULT_MODREG_ID;

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
}

/// sha256 content hash of component bytes — the verify side of a code swap.
fn sha256(bytes: &[u8]) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).to_vec()
}

/// lowercase hex of a hash, for fail-closed error messages.
fn hex32(bytes: &[u8]) -> String {
    use core::fmt::Write;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
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
    /// the effective protocol version for this block — `effective_version(height)`
    /// derived from committed upgrade-module state and stamped by the node layer
    /// (see [`Host::effective_version`]). copied verbatim into every dispatch's
    /// [`Env::protocol_version`]. a read-only dispatch input: it is NEVER folded
    /// into any module `root()` preimage, op/wire encoding, or the app-hash
    /// composition. defaults to [`BASELINE_VERSION`].
    pub protocol_version: u32,
}

/// the baseline protocol version — the version every node runs before any upgrade
/// activates, and the graceful fallback when the `upgrade` module is not yet
/// registered (a host without the module, e.g. a test registry). Matches the `upgrade` module's uninitialized
/// `current_version == 0`, so a fresh module and a module-absent host agree.
pub const BASELINE_VERSION: u32 = 0;

impl Default for BlockContext {
    /// the pre-consensus default: height/time 0, an empty external origin, and the
    /// baseline protocol version, so [`Host::submit`] is byte-for-byte the old
    /// hardcoded behavior.
    fn default() -> Self {
        Self {
            height: 0,
            consensus_time: 0,
            origin: Origin::External(Vec::new()),
            protocol_version: BASELINE_VERSION,
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
}

/// the result of applying one block (`submit`).
#[derive(Debug)]
pub struct BlockOutcome {
    /// the app-hash over the registry after the drain settled.
    pub app_hash: StateRoot,
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
/// post-batch [`app_hash`](BatchOutcome::app_hash).
#[derive(Debug)]
pub struct BatchOutcome {
    /// the one post-batch app-hash, shared by every applied member.
    pub app_hash: StateRoot,
    /// one outcome per input op, in input order.
    pub members: Vec<MemberOutcome>,
    /// aggregate events, in drain order: every applied member's trace in input
    /// order, then the once-per-block injections.
    pub events: Vec<Event>,
    /// the dispatch trace from the once-per-block System injections
    /// (`pending_advance` / `pending_modreg_advance` / `pending_deliveries`),
    /// drained once after the members.
    pub system_dispatches: Vec<DispatchRecord>,
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

/// a finalized consensus boundary the host is allowed to serve from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FinalizedBlock {
    /// finalized block height.
    pub height: u64,
    /// app-hash consensus committed at `height`.
    pub app_hash: StateRoot,
}

/// one module's committed root plus the sync surface it can currently serve.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleSnapshot {
    pub id: ModuleId,
    pub root: StateRoot,
    pub state_sync: StateSyncHandle,
}

/// a consistent registry view captured at a finalized app-hash boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FinalizedSnapshot {
    pub height: u64,
    pub app_hash: StateRoot,
    pub modules: Vec<ModuleSnapshot>,
}

impl FinalizedSnapshot {
    /// find a module entry by id.
    pub fn module(&self, id: &str) -> Option<&ModuleSnapshot> {
        self.modules.iter().find(|m| m.id == id)
    }

    /// true only when every module supplied self-contained snapshot bytes.
    pub fn has_all_snapshot_bytes(&self) -> bool {
        self.modules
            .iter()
            .all(|m| m.state_sync.has_snapshot_bytes())
    }

    /// true when every module can be rebuilt without an external resolver.
    pub fn is_self_contained(&self) -> bool {
        self.modules
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
/// applying blocks; continuing would silently fork this node's app-hash.
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

/// failures while capturing a finalized snapshot.
#[derive(Debug, PartialEq, Eq)]
pub enum SnapshotError {
    /// the caller asked for a boundary that no longer matches the host state.
    AppHashMismatch {
        expected: StateRoot,
        actual: StateRoot,
    },
    /// a module failed while preparing its state-sync handle.
    Module { id: ModuleId, source: Error },
}

impl core::fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::AppHashMismatch { expected, actual } => write!(
                f,
                "finalized app-hash mismatch: expected {expected:?}, actual {actual:?}",
            ),
            Self::Module { id, source } => {
                write!(
                    f,
                    "module {id} failed to prepare state-sync handle: {source}"
                )
            }
        }
    }
}

impl std::error::Error for SnapshotError {}

/// the deterministic state machine: a module registry + dispatch + drain.
#[derive(Default)]
pub struct Host {
    /// deterministic iteration order is load-bearing for snapshot + app-hash.
    registry: BTreeMap<ModuleId, Box<dyn Module>>,
    /// Modules carried by a legacy workspace but excluded from every host
    /// surface until an agreed protocol boundary activates them. Keeping them
    /// outside `registry` makes pre-boundary query/dispatch/root/snapshot
    /// behavior byte-for-byte identical to the old binary.
    dormant: BTreeMap<ModuleId, Box<dyn Module>>,
    /// Per-module activation threshold for the one harder upgrade class that
    /// changes the registry itself. Empty for ordinary/fresh hosts.
    activation_versions: BTreeMap<ModuleId, u32>,
    active_version: u32,
}

impl Host {
    pub fn new() -> Self {
        Self {
            registry: BTreeMap::new(),
            dormant: BTreeMap::new(),
            activation_versions: BTreeMap::new(),
            active_version: BASELINE_VERSION,
        }
    }

    /// register a module under its own [`Module::id`]. genesis-time wiring.
    pub fn register(&mut self, module: Box<dyn Module>) {
        self.registry.insert(module.id(), module);
    }

    /// Keep one already-registered module invisible until `version` is active.
    /// This is intentionally explicit and narrow: unknown module-set changes
    /// remain incompatible instead of acquiring a generic migration escape
    /// hatch.
    pub fn defer_module_until(&mut self, id: &str, version: u32) -> Result<(), Error> {
        if version <= self.active_version {
            return Err(Error::Module(format!(
                "module {id} activation version {version} is not above active version {}",
                self.active_version
            )));
        }
        if self.activation_versions.contains_key(id) {
            return Err(Error::Module(format!(
                "module {id} already has an activation version"
            )));
        }
        let module = self
            .registry
            .remove(id)
            .ok_or_else(|| Error::UnknownModule(id.to_string()))?;
        self.activation_versions.insert(id.to_string(), version);
        self.dormant.insert(id.to_string(), module);
        Ok(())
    }

    /// Sorted module ids and their canonical-state revisions.
    pub fn state_schema(&self) -> Vec<(ModuleId, u32)> {
        self.registry
            .iter()
            .map(|(id, module)| (id.clone(), module.state_schema_revision()))
            .collect()
    }

    /// Fingerprint persisted in recovery/state-sync manifests.
    pub fn state_schema_fingerprint(&self) -> [u8; 32] {
        let schema = self.state_schema();
        state_schema_fingerprint(schema.iter().map(|(id, revision)| (id.as_str(), *revision)))
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
                        // an out-of-block external read has no block version; it
                        // reads committed state under the baseline format.
                        protocol_version: BASELINE_VERSION,
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

    /// the effective protocol version governing block `height`, derived from the
    /// committed `upgrade`-module state (the pure `effective_version` derivation,
    /// not the raw stored `current_version`). the node layer stamps this onto
    /// [`BlockContext::protocol_version`] so dispatch and hashing agree that block
    /// `height` runs one version.
    ///
    /// this is a read-only, out-of-block committed read (routed like any external
    /// [`Host::query`]). the `upgrade` module is a genesis constant on an upgraded
    /// net, but is ABSENT before the coordinated retrofit — so a missing module,
    /// an undecodable reply, or any query error gracefully falls back to
    /// [`BASELINE_VERSION`] rather than panicking or erroring. behavior is
    /// therefore unchanged until the module is registered and a pending upgrade
    /// arms at its activation height.
    pub async fn effective_version(&self, height: u64) -> u32 {
        let req = upgrade::encode_query(&upgrade::UpgradeQuery::Status);
        match self.query(UPGRADE_MODULE_ID, &req).await {
            Ok(bytes) => match upgrade::decode_reply(&bytes) {
                Ok(upgrade::UpgradeReply::Status(s)) => {
                    // the ONE shared predicate — the host stamps EXACTLY what the
                    // module's Advance arm check computes (both route through
                    // upgrade::effective_version), so dispatch and hashing
                    // can never diverge at the boundary (risk R4).
                    upgrade::effective_version(
                        height,
                        s.current_version,
                        s.pending.as_ref(),
                        &s.members,
                        |member| {
                            s.ready
                                .binary_search_by(|ready| ready.as_slice().cmp(member))
                                .is_ok()
                        },
                    )
                }
                // module present but reply unreadable — never fork on a decode slip.
                Err(_) => BASELINE_VERSION,
            },
            // module absent or unreadable — baseline, never error.
            Err(_) => BASELINE_VERSION,
        }
    }

    /// drive the agreed boundary protocol version into EVERY registered module at
    /// the finalized activation boundary (design §4). the version is read from
    /// frozen boundary state (the orchestrator's `RespawnPlan::boundary_version`),
    /// identical on every honest node, so this is a deterministic self-transition —
    /// never a wall-clock/IO/RNG input. a no-op for modules that don't override
    /// [`Module::set_active_version`] (only dual-path modules do), and
    /// `version` itself is a NON-hashed branch selector — it never enters any
    /// `root()` preimage. Ordinary dual-path modules therefore leave the current
    /// root unmoved by this call alone. A module explicitly registered through
    /// [`Host::defer_module_until`] is the narrow exception: it joins/leaves the
    /// registry (and app-hash) exactly when its version threshold is crossed.
    /// The upgrade module's own `current_version` still reconciles through the
    /// in-block System `Advance` injected at the same height.
    pub fn set_active_version(&mut self, version: u32) {
        for m in self.registry.values_mut() {
            m.set_active_version(version);
        }
        for m in self.dormant.values_mut() {
            m.set_active_version(version);
        }
        let activate: Vec<ModuleId> = self
            .activation_versions
            .iter()
            .filter(|(id, threshold)| version >= **threshold && self.dormant.contains_key(*id))
            .map(|(id, _)| id.clone())
            .collect();
        let deactivate: Vec<ModuleId> = self
            .activation_versions
            .iter()
            .filter(|(id, threshold)| version < **threshold && self.registry.contains_key(*id))
            .map(|(id, _)| id.clone())
            .collect();
        for id in activate {
            let module = self.dormant.remove(&id).expect("activation source checked");
            self.registry.insert(id, module);
        }
        for id in deactivate {
            let module = self
                .registry
                .remove(&id)
                .expect("deactivation source checked");
            self.dormant.insert(id, module);
        }
        self.active_version = version;
    }

    /// the SYSTEM-ORIGIN `Advance` the drain injects in-block at a finalized
    /// activation boundary, or `None` when there is nothing to reconcile.
    ///
    /// this is the replay-safe realization of the design's "arm/abort is a
    /// deterministic self-transition evaluated exactly once at `H`": because it
    /// rides the SAME [`Host::submit_at`] drain that recovery-replay and
    /// state-sync-install also run, the `current_version` flip + pending clear
    /// reconstruct byte-for-byte on every node (never a respawn side-effect,
    /// invisible to replay). keyed purely on committed upgrade state + `height`:
    /// injected iff the committed `upgrade` module holds a pending upgrade whose
    /// `activation_height` has been reached. idempotent — the first block at/after
    /// `H` clears the pending, so later blocks inject nothing. ABSENT until the
    /// module is registered (the `Status` query errors → `None`), so the drain is
    /// byte-identical on a net without the module.
    async fn pending_advance(&self, height: u64) -> Option<Msg> {
        let req = upgrade::encode_query(&upgrade::UpgradeQuery::Status);
        let bytes = self.query(UPGRADE_MODULE_ID, &req).await.ok()?;
        let upgrade::UpgradeReply::Status(status) =
            upgrade::decode_reply(&bytes).ok()?;
        let pending = status.pending?;
        if height >= pending.activation_height {
            Some(Msg {
                target: UPGRADE_MODULE_ID.into(),
                payload: upgrade::encode_msg(&upgrade::UpgradeMsg::Advance),
            })
        } else {
            None
        }
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
        let req =
            dispatch::encode_query(&dispatch::DispatchQuery::PendingDeliveries);
        let bytes = self.query(DISPATCH_MODULE_ID, &req).await.ok()?;
        let dispatch::DispatchReply::PendingDeliveries(pending) =
            dispatch::decode_reply(&bytes).ok()?
        else {
            return None;
        };
        (pending > 0).then(|| Msg {
            target: DISPATCH_MODULE_ID.into(),
            payload: dispatch::encode_msg(
                &dispatch::DispatchMsg::DeliverPending {},
            ),
        })
    }

    /// whether the committed dispatch mailbox holds undelivered results —
    /// the drain will inject `DeliverPending` into the NEXT successful block.
    /// drivers with no other block flow (a reactor fixpoint, a block-per-op
    /// daemon, a quiet validator) read this to know a flush block is needed.
    pub async fn has_pending_deliveries(&self) -> bool {
        self.pending_deliveries().await.is_some()
    }

    /// the SYSTEM-ORIGIN modreg `Advance` the drain injects in-block, or `None`
    /// when the committed code registry has no armed swap to activate. mirrors
    /// [`Host::pending_advance`]: it rides the SAME drain that recovery-replay and
    /// state-sync-install also run, so the committed active-hash flip + pending
    /// clear reconstruct byte-for-byte on every node (folded into the app-hash —
    /// the consensus commitment to WHICH code is active). keyed purely on
    /// committed registry state + `height`: injected iff the committed registry
    /// holds a pending swap whose `activation_height` has been reached. idempotent
    /// — the first block at/after `H` clears the pending, so later blocks inject
    /// nothing. ABSENT until the module is registered (`Status` errors → `None`),
    /// so the drain is byte-identical on a net without the code registry.
    async fn pending_modreg_advance(&self, height: u64) -> Option<Msg> {
        let modules = self.modreg_status().await?;
        let any_armed = modules.iter().any(|m| {
            m.pending
                .as_ref()
                .is_some_and(|p| p.ready && height >= p.activation_height)
        });
        any_armed.then(|| Msg {
            target: MODREG_MODULE_ID.into(),
            payload: modreg::encode_msg(&modreg::ModregMsg::Advance),
        })
    }

    /// the code registry's committed per-module code state, or `None` when the
    /// module is absent / its reply is unreadable — the shared out-of-block
    /// committed read behind [`Host::pending_modreg_advance`] and
    /// [`Host::realize_module_swaps`] (mirrors [`Host::effective_version`]'s
    /// graceful fallback: a missing registry is never an error, just nothing to do).
    async fn modreg_status(&self) -> Option<Vec<modreg::ModuleCode>> {
        let req = modreg::encode_query(&modreg::ModregQuery::Status);
        let bytes = self.query(MODREG_MODULE_ID, &req).await.ok()?;
        match modreg::decode_reply(&bytes).ok()? {
            modreg::ModregReply::Status { modules } => Some(modules),
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
    /// the consensus half is the in-block [`Host::pending_modreg_advance`] tick
    /// that flips the committed active hash into the app-hash. code is invisible
    /// to `root()`, so a swap keeps the module's state and the app-hash is
    /// byte-continuous across it.
    ///
    /// keyed PURELY on committed registry state + `height`, so it reconstructs
    /// identically on every path that advances a node to a committed state — live
    /// drain, recovery replay, state-sync catch-up — and is idempotent: it
    /// compares [`Module::code_hash`] and re-instantiates a component only on an
    /// actual change. run it BEFORE applying block `height`, so that block's
    /// dispatches execute on the code the registry designates for `height`.
    ///
    /// the target hash for a module at `height` is a pending hash that has reached
    /// activation (`activation_height <= height`), else its committed ACTIVE hash
    /// — the SAME predicate modreg's `Advance` arm-check applies, so this
    /// out-of-block realization and the in-block commit never disagree on the arm
    /// set. reading ACTIVE (not only the armed pending) is what lets a state-sync
    /// joiner — which installs post-activation state with no pending left to arm —
    /// reconcile to the live code instead of forking on stale genesis code.
    ///
    /// FAIL-CLOSED: a designated hash whose bytes this node lacks, or bytes whose
    /// sha256 does not match the committed hash, is a hard error (the node cannot
    /// honestly apply `height` without the agreed code) — it returns `Err` with no
    /// partial swap applied. ABSENT registry → nothing to reconcile, `Ok(())`.
    pub async fn realize_module_swaps(
        &mut self,
        height: u64,
        src: &dyn CodeSource,
    ) -> Result<(), Error> {
        let Some(modules) = self.modreg_status().await else {
            return Ok(());
        };
        for m in modules {
            // the SAME arm predicate as modreg::handle_advance (height >=
            // activation_height): pending-if-armed, else the committed active hash.
            let target = match m.pending {
                // the SAME arm predicate as modreg: ready (full byte receipt,
                // latched in committed state) AND the height floor reached.
                Some(p) if p.ready && height >= p.activation_height => p.code_hash,
                _ => m.active_code_hash,
            };
            // only reconcile a module this node actually runs AS a hot-swappable
            // component: an absent id, or a native module (no `code_hash`), is
            // nothing to realize — its registry entry, if any, is a genesis concern.
            let Some(current) = self.registry.get(&m.module_id).and_then(|x| x.code_hash()) else {
                continue;
            };
            if current == target {
                continue; // already on the designated code — idempotent no-op.
            }
            let bytes = src.fetch(&target).await.ok_or_else(|| {
                Error::Module(format!(
                    "code bytes absent for module {} (hash {}) — fail-closed",
                    m.module_id,
                    hex32(&target),
                ))
            })?;
            if sha256(&bytes) != target {
                return Err(Error::Module(format!(
                    "code bytes for module {} do not match committed hash {} — fail-closed",
                    m.module_id,
                    hex32(&target),
                )));
            }
            self.swap_module_code(&m.module_id, &bytes)?;
        }
        Ok(())
    }

    /// the current app-hash: [`global_root`] over the registered modules.
    pub fn app_hash(&self) -> StateRoot {
        let mods: Vec<&dyn Module> = self.registry.values().map(|b| b.as_ref()).collect();
        global_root(&mods)
    }

    /// the live root of a single registered module (test/inspection accessor).
    pub fn module_root(&self, id: &str) -> Option<StateRoot> {
        self.registry.get(id).map(|m| m.root())
    }

    /// every registered module's `(id, root)`, in registry (sorted-id) order —
    /// the exact input [`Host::app_hash`] composes over. a recovery journal
    /// seals each applied block with these so a restarted node can locate every
    /// module's replay position by root equality.
    pub fn module_roots(&self) -> Vec<(ModuleId, StateRoot)> {
        self.registry
            .iter()
            .map(|(id, m)| (id.clone(), m.root()))
            .collect()
    }

    /// the per-block-durable disk cohort: every registered module whose sync
    /// surface is [`StateSyncHandle::ResolverBacked`] — a qmdb-like store that
    /// commits to its OWN disk each block and recovers itself at boot rather than
    /// riding a checkpoint snapshot. recovery uses this to tell a disk substrate
    /// that legitimately raced N blocks ahead of the last checkpoint apart from a
    /// rolled-back in-memory cohort module: only a disk-cohort module may be
    /// trusted at a self-durable root ABOVE the checkpoint. a module whose
    /// `state_sync_handle` errors is excluded (it cannot claim the disk cohort's
    /// self-durability), falling through to the ordinary root-equality path.
    pub fn resolver_backed_ids(&self) -> BTreeSet<ModuleId> {
        self.registry
            .iter()
            .filter(|(_, m)| {
                matches!(
                    m.state_sync_handle(),
                    Ok(StateSyncHandle::ResolverBacked { .. })
                )
            })
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
        self.registry.get(id).and_then(|m| m.durable_commit_height())
    }

    /// capture the committed registry view for a finalized block.
    ///
    /// The caller supplies the finalized app-hash from consensus. The host
    /// recomputes its current app-hash first and refuses to serve if it has
    /// already advanced, preventing a node from labeling current module state as
    /// an older height. Because this borrows `&self`, it can only run outside the
    /// mutable `submit_at` block lifecycle; module roots and state-sync handles
    /// therefore come from committed state, not an in-flight staged overlay.
    pub fn capture_finalized_snapshot(
        &self,
        finalized: FinalizedBlock,
    ) -> Result<FinalizedSnapshot, SnapshotError> {
        let actual = self.app_hash();
        if actual != finalized.app_hash {
            return Err(SnapshotError::AppHashMismatch {
                expected: finalized.app_hash,
                actual,
            });
        }

        let modules = self
            .registry
            .iter()
            .map(|(id, module)| {
                let state_sync =
                    module
                        .state_sync_handle()
                        .map_err(|source| SnapshotError::Module {
                            id: id.clone(),
                            source,
                        })?;
                Ok(ModuleSnapshot {
                    id: id.clone(),
                    root: module.root(),
                    state_sync,
                })
            })
            .collect::<Result<Vec<_>, SnapshotError>>()?;

        Ok(FinalizedSnapshot {
            height: finalized.height,
            app_hash: finalized.app_hash,
            modules,
        })
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
    /// value. the app-hash is recomposed AFTER the commit, so it reflects exactly
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
        self.set_active_version(ctx.protocol_version);
        // every module dispatched this block, in deterministic order — the set
        // the host commits or aborts at the boundary.
        let mut touched: BTreeSet<ModuleId> = BTreeSet::new();

        match self.drain(ctx, msg, &mut touched).await {
            Ok((events, dispatches)) => {
                // clean drain: publish every touched module's staged writes. this
                // is the ONLY place a module's state advances, so recompose the
                // app-hash AFTER. a commit failure is FATAL, not a rejection: the
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
                    app_hash: self.app_hash(),
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
    /// boundary, and ONE post-batch app-hash shared by every applied member.
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
        self.apply_block(ctx, ops, None).await
    }

    /// RECOVERY-ONLY selective-commit variant of [`Host::submit_block`]: identical
    /// per-op isolation and single-app-hash composition, but at the boundary it
    /// partitions the touched set — commit the modules in `commit_only`, abort the
    /// rest. this heals a TORN block at boot: a block that committed a
    /// per-block-durable disk substrate (already at its sealed post-root on disk)
    /// but whose in-memory cohort was rolled back to the checkpoint; replay re-runs
    /// the frame and commits ONLY the at-pre cohort, aborting the durable substrate
    /// (re-committing it would move its op-log root and fork). NOT the live path.
    pub async fn submit_block_committing(
        &mut self,
        ctx: BlockContext,
        ops: Vec<(Origin, Msg)>,
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
        ops: Vec<(Origin, Msg)>,
        commit_only: Option<&BTreeSet<ModuleId>>,
    ) -> Result<BatchOutcome, SubmitError> {
        // block-constant across every dispatch this block — the agreed values.
        let height = ctx.height;
        let consensus_time = ctx.consensus_time;
        let protocol_version = ctx.protocol_version;
        self.set_active_version(protocol_version);

        // 1. the once-per-block System injections, computed ONCE against PRE-batch
        // committed state — the "results staged by this very block are invisible
        // here" invariant, evaluated BEFORE any member stages. same order as the
        // single-op drain: upgrade `Advance`, then modreg `Advance`, then
        // `DeliverPending`. drained once, after every member, below (step 4).
        let mut injections: VecDeque<(Origin, Msg)> = VecDeque::new();
        if let Some(advance) = self.pending_advance(height).await {
            injections.push_back((Origin::System, advance));
        }
        // the code-registry boundary tick: flip every armed swap's committed
        // active hash so the app-hash commits to the new code (the per-node
        // realization of the actual swap is out-of-block, in realize_module_swaps).
        if let Some(advance) = self.pending_modreg_advance(height).await {
            injections.push_back((Origin::System, advance));
        }
        if let Some(deliver) = self.pending_deliveries().await {
            injections.push_back((Origin::System, deliver));
        }

        // 2. per-op isolation. `touched` and the modules' own staging accumulate
        // ACROSS members (never committed mid-batch); a member that rejects rolls
        // the whole stage back and replays the accepted members, so its rejection
        // leaves no trace on committed state.
        let mut touched: BTreeSet<ModuleId> = BTreeSet::new();
        // accepted members and their authoritative traces, parallel arrays in
        // input order; `acc_pos[k]` is the input index of accepted member `k`.
        let mut accepted: Vec<(Origin, Msg)> = Vec::new();
        let mut acc_traces: Vec<(Vec<Event>, Vec<DispatchRecord>)> = Vec::new();
        let mut acc_pos: Vec<usize> = Vec::new();
        let mut results: Vec<Option<MemberOutcome>> = (0..ops.len()).map(|_| None).collect();

        for (i, (origin, msg)) in ops.into_iter().enumerate() {
            let mut ev: Vec<Event> = Vec::new();
            let mut di: Vec<DispatchRecord> = Vec::new();
            let queue: VecDeque<(Origin, Msg)> = VecDeque::from([(origin.clone(), msg.clone())]);
            match self
                .drain_queue(
                    height,
                    consensus_time,
                    protocol_version,
                    queue,
                    &mut touched,
                    &mut ev,
                    &mut di,
                )
                .await
            {
                Ok(()) => {
                    accepted.push((origin, msg));
                    acc_traces.push((ev, di));
                    acc_pos.push(i);
                    // authoritative trace is written after the loop (step 3);
                    // this placeholder is overwritten there.
                    results[i] = Some(MemberOutcome::Applied {
                        dispatches: Vec::new(),
                    });
                }
                Err(reason) => {
                    // ISOLATE: this member's partial stage is entangled with the
                    // accepted members' stage (one shared per-module stage), so
                    // roll the WHOLE stage back, then replay only the accepted
                    // members to rebuild their writes without this one.
                    self.abort_all(&mut touched).await?;
                    for (k, (o, m)) in accepted.iter().enumerate() {
                        let mut rev: Vec<Event> = Vec::new();
                        let mut rdi: Vec<DispatchRecord> = Vec::new();
                        let rq: VecDeque<(Origin, Msg)> = VecDeque::from([(o.clone(), m.clone())]);
                        // an accepted member drained Ok in this same context
                        // before; a reject on replay is NON-DETERMINISM → fatal.
                        self.drain_queue(
                            height,
                            consensus_time,
                            protocol_version,
                            rq,
                            &mut touched,
                            &mut rev,
                            &mut rdi,
                        )
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
                                module = %m.target,
                                error = %re,
                                "NON-DETERMINISTIC module: rejected on replay what it \
                                 accepted during per-op isolation — this node's state \
                                 may diverge from its peers"
                            );
                            SubmitError::Fatal(FatalError {
                                module: m.target.clone(),
                                phase: BoundaryPhase::Abort,
                                source: Error::Module(format!(
                                    "non-deterministic reject replaying accepted batch \
                                     member during per-op isolation: {re}"
                                )),
                            })
                        })?;
                        acc_traces[k] = (rev, rdi);
                    }
                    results[i] = Some(MemberOutcome::Rejected {
                        reason: reason.to_string(),
                    });
                }
            }
        }

        // 3. write each accepted member's authoritative trace and accumulate the
        // aggregate events in input order (accepted / acc_traces / acc_pos are
        // all in input order).
        let mut events: Vec<Event> = Vec::new();
        for ((ev, di), pos) in acc_traces.into_iter().zip(acc_pos.iter()) {
            events.extend(ev);
            results[*pos] = Some(MemberOutcome::Applied { dispatches: di });
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
                protocol_version,
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

        // 6. ONE app-hash over the committed registry, shared by every member.
        Ok(BatchOutcome {
            app_hash: self.app_hash(),
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
        // effective_version(height) for this block — stamped constant across the
        // root op and every FIFO follow-up; a read-only dispatch input, NEVER hashed.
        let protocol_version = ctx.protocol_version;

        // the root op carries the real submitter's origin; follow-ups override.
        let mut queue: VecDeque<(Origin, Msg)> = VecDeque::from([(ctx.origin, msg)]);

        // DETERMINISTIC ACTIVATION INJECTION (design §4 / plan Task 6.3). at a
        // finalized boundary where the committed `upgrade` module holds a pending
        // upgrade that has reached its activation height, append EXACTLY ONE
        // System-origin `Advance` so the module reconciles its own app-hashed state
        // in-block (ARM: `current_version = to_version` + clear pending/readiness;
        // ABORT: clear only) at the SAME finalized view on every node. it rides this
        // drain (not the respawn side-path), so live, recovery-replay, and
        // state-sync nodes all reconstruct it byte-for-byte. this is what frees the
        // at-most-one-pending slot after activation. INERT until the module is
        // registered — `pending_advance` returns `None` when the module is absent.
        if let Some(advance) = self.pending_advance(height).await {
            queue.push_back((Origin::System, advance));
        }
        // DETERMINISTIC CODE-SWAP ACTIVATION INJECTION: when the committed code
        // registry holds a pending swap that has reached its activation height,
        // append EXACTLY ONE System-origin modreg `Advance` so the registry flips
        // the armed active hash in-block (folded into the app-hash) at the SAME
        // finalized view on every node — the consensus commitment to the new code.
        // the actual component swap is realized out-of-block by
        // `realize_module_swaps`. INERT until the module is registered.
        if let Some(advance) = self.pending_modreg_advance(height).await {
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
        self.drain_queue(
            height,
            consensus_time,
            protocol_version,
            queue,
            touched,
            &mut events,
            &mut dispatches,
        )
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
    #[allow(clippy::too_many_arguments)]
    async fn drain_queue(
        &mut self,
        height: u64,
        consensus_time: u64,
        protocol_version: u32,
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
                    protocol_version,
                },
                snapshot,
                registry: &self.registry, // the rest — for query routing
                out_msgs: Vec::new(),
                out_events: Vec::new(),
            };

            // owned `me` (&mut) and `ctx` (holding &rest) are disjoint borrows,
            // so they compose across this await. deterministic awaits only.
            let res = me.execute(&mut ctx, &msg).await;

            // destructure releases the &registry borrow → map is mutable again.
            let HostCtx {
                out_msgs,
                out_events,
                ..
            } = ctx;

            // reinsert BEFORE propagating any error — a module never vanishes.
            self.registry.insert(msg.target.clone(), me);
            res?;

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
                        protocol_version: self.env.protocol_version,
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
                        protocol_version: self.env.protocol_version,
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

#[cfg(test)]
mod state_schema_tests {
    use super::state_schema_fingerprint;

    #[test]
    fn fingerprint_is_canonically_sorted_and_revision_sensitive() {
        let first = state_schema_fingerprint([("runs", 2), ("chat", 1)]);
        let reordered = state_schema_fingerprint([("chat", 1), ("runs", 2)]);
        let old_runs = state_schema_fingerprint([("chat", 1), ("runs", 1)]);
        assert_eq!(
            first, reordered,
            "registry construction order is irrelevant"
        );
        assert_ne!(first, old_runs, "a canonical schema change requires a bump");
    }
}
