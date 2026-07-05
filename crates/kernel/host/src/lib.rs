//! the host — the deterministic state-machine spine.
//!
//! a [`Host`] owns a registry of [`Module`]s and turns an inbound [`Msg`] into a
//! block: it routes the message to its target module, awaits the (deterministic)
//! `execute`, then drains the intents that execute emitted. emitted [`Msg`]s are
//! re-dispatched as LOCAL-ONLY follow-up ops (never re-broadcast); emitted
//! [`Event`]s/[`Effect`]s are collected and handed back for the effectful node
//! layer (out of scope this slice). after the drain, the app-hash is recomposed
//! over the registry via [`state::global_root`].
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
    Ctx, Effect, Env, Error, Event, Module, ModuleId, Msg, Origin, ResolverSyncTarget, StateRoot,
    StateSyncHandle,
};

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
pub const DISPATCH_MODULE_ID: &str = dispatch_interface::DEFAULT_DISPATCH_TARGET;

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
/// registered (pre-retrofit nets). Matches the `upgrade` module's uninitialized
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
    /// observability events emitted during the block, in dispatch order.
    pub events: Vec<Event>,
    /// effect intents emitted during the block — stub sink this slice.
    pub effects: Vec<Effect>,
    /// the deterministic dispatch trace: one entry per module dispatched this
    /// block, in drain order. the "what happened" spine the node layer tags with
    /// node-local timing for telemetry.
    pub dispatches: Vec<DispatchRecord>,
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
}

impl Host {
    pub fn new() -> Self {
        Self {
            registry: BTreeMap::new(),
        }
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
        let req = upgrade_interface::encode_query(&upgrade_interface::UpgradeQuery::Status);
        match self.query(UPGRADE_MODULE_ID, &req).await {
            Ok(bytes) => match upgrade_interface::decode_reply(&bytes) {
                Ok(upgrade_interface::UpgradeReply::Status(s)) => {
                    // the ONE shared predicate — the host stamps EXACTLY what the
                    // module's Advance arm check computes (both route through
                    // upgrade_interface::effective_version), so dispatch and hashing
                    // can never diverge at the boundary (risk R4).
                    let ready: std::collections::BTreeMap<Vec<u8>, ()> =
                        s.ready.iter().map(|k| (k.clone(), ())).collect();
                    upgrade_interface::effective_version(
                        height,
                        s.current_version,
                        s.pending.as_ref(),
                        &s.members,
                        &ready,
                    )
                }
                // module present but reply unreadable — never fork on a decode slip.
                Err(_) => BASELINE_VERSION,
            },
            // module absent (pre-retrofit) or unreadable — baseline, never error.
            Err(_) => BASELINE_VERSION,
        }
    }

    /// drive the agreed boundary protocol version into EVERY registered module at
    /// the finalized activation boundary (design §4). the version is read from
    /// frozen boundary state (the orchestrator's `RespawnPlan::boundary_version`),
    /// identical on every honest node, so this is a deterministic self-transition —
    /// never a wall-clock/IO/RNG input. a no-op for modules that don't override
    /// [`Module::set_active_version`] (only dual-path modules like forge do), and
    /// `version` is a NON-hashed branch selector — it never enters any `root()`
    /// preimage, so `app_hash()` is unmoved by this call alone (the app-hashed
    /// reconciliation of the upgrade module's own `current_version` rides the
    /// in-block System `Advance` the drain injects at the same height).
    pub fn set_active_version(&mut self, version: u32) {
        for m in self.registry.values_mut() {
            m.set_active_version(version);
        }
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
    /// byte-identical on a pre-retrofit net.
    async fn pending_advance(&self, height: u64) -> Option<Msg> {
        let req = upgrade_interface::encode_query(&upgrade_interface::UpgradeQuery::Status);
        let bytes = self.query(UPGRADE_MODULE_ID, &req).await.ok()?;
        let upgrade_interface::UpgradeReply::Status(status) =
            upgrade_interface::decode_reply(&bytes).ok()?;
        let pending = status.pending?;
        if height >= pending.activation_height {
            Some(Msg {
                target: UPGRADE_MODULE_ID.into(),
                payload: upgrade_interface::encode_msg(&upgrade_interface::UpgradeMsg::Advance),
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
            dispatch_interface::encode_query(&dispatch_interface::DispatchQuery::PendingDeliveries);
        let bytes = self.query(DISPATCH_MODULE_ID, &req).await.ok()?;
        let dispatch_interface::DispatchReply::PendingDeliveries(pending) =
            dispatch_interface::decode_reply(&bytes).ok()?
        else {
            return None;
        };
        (pending > 0).then(|| Msg {
            target: DISPATCH_MODULE_ID.into(),
            payload: dispatch_interface::encode_msg(
                &dispatch_interface::DispatchMsg::DeliverPending {},
            ),
        })
    }

    /// the current app-hash: [`state::global_root`] over the registered modules.
    pub fn app_hash(&self) -> StateRoot {
        let mods: Vec<&dyn Module> = self.registry.values().map(|b| b.as_ref()).collect();
        state::global_root(&mods)
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
        // every module dispatched this block, in deterministic order — the set
        // the host commits or aborts at the boundary.
        let mut touched: BTreeSet<ModuleId> = BTreeSet::new();

        match self.drain(ctx, msg, &mut touched).await {
            Ok((events, effects, dispatches)) => {
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
                    effects,
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
                    if let Some(m) = self.registry.get_mut(id) {
                        if let Err(source) = m.abort_block().await {
                            fatal.get_or_insert(FatalError {
                                module: id.clone(),
                                phase: BoundaryPhase::Abort,
                                source,
                            });
                        }
                    }
                }
                match fatal {
                    Some(f) => Err(SubmitError::Fatal(f)),
                    None => Err(SubmitError::Rejected(e)),
                }
            }
        }
    }

    /// RECOVERY-ONLY selective-commit replay of one block. identical to
    /// [`Host::submit_at`] — same [`drain`](Self::drain), same deterministic
    /// execution — except at the boundary it partitions the touched set: a
    /// module in `commit_only` is committed (its staged writes published),
    /// every other touched module is ABORTED (its stage discarded).
    ///
    /// this exists to heal a TORN block at boot: a block that committed a
    /// per-block-durable disk substrate (already at its sealed post-root on
    /// disk) but whose in-memory cohort was rolled back to the checkpoint
    /// (still at its pre-root). replay re-runs the sealed frame and commits
    /// ONLY the in-memory cohort — the modules still at pre — while ABORTING
    /// the disk substrates, because re-committing an already-durable qmdb store
    /// would MOVE its op-log root and fork this node. the caller (`recovery`)
    /// computes `commit_only` by per-module root compare and verifies every
    /// touched module lands on its sealed root afterward.
    ///
    /// NOT for the live consensus path: on a live block every touched module
    /// commits together (that is [`Host::submit_at`]); this reconstructs an
    /// outcome consensus already sealed and never manufactures new live state.
    pub async fn submit_at_committing(
        &mut self,
        ctx: BlockContext,
        msg: Msg,
        commit_only: &BTreeSet<ModuleId>,
    ) -> Result<BlockOutcome, SubmitError> {
        let mut touched: BTreeSet<ModuleId> = BTreeSet::new();

        match self.drain(ctx, msg, &mut touched).await {
            Ok((events, effects, dispatches)) => {
                // partition the touched set at the boundary: commit the modules
                // the caller marked (the in-memory cohort still at pre), abort
                // the rest (disk substrates already durable at post — a
                // re-commit would move their op-log root and fork). both hooks
                // run in deterministic registry order; either failing is FATAL.
                for id in &touched {
                    if let Some(m) = self.registry.get_mut(id) {
                        if commit_only.contains(id) {
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
                Ok(BlockOutcome {
                    app_hash: self.app_hash(),
                    events,
                    effects,
                    dispatches,
                })
            }
            Err(e) => {
                // drain failure: identical to submit_at — abort every touched
                // module, report the first abort fault as fatal else the
                // deterministic rejection.
                let mut fatal: Option<FatalError> = None;
                for id in &touched {
                    if let Some(m) = self.registry.get_mut(id) {
                        if let Err(source) = m.abort_block().await {
                            fatal.get_or_insert(FatalError {
                                module: id.clone(),
                                phase: BoundaryPhase::Abort,
                                source,
                            });
                        }
                    }
                }
                match fatal {
                    Some(f) => Err(SubmitError::Fatal(f)),
                    None => Err(SubmitError::Rejected(e)),
                }
            }
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
    ) -> Result<(Vec<Event>, Vec<Effect>, Vec<DispatchRecord>), Error> {
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
        let mut events: Vec<Event> = Vec::new();
        let mut effects: Vec<Effect> = Vec::new();
        let mut dispatches: Vec<DispatchRecord> = Vec::new();
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
                out_effects: Vec::new(),
            };

            // owned `me` (&mut) and `ctx` (holding &rest) are disjoint borrows,
            // so they compose across this await. deterministic awaits only.
            let res = me.execute(&mut ctx, &msg).await;

            // destructure releases the &registry borrow → map is mutable again.
            let HostCtx {
                out_msgs,
                out_events,
                out_effects,
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
            // re-broadcast. events/effects leave the state machine.
            for m in out_msgs {
                queue.push_back((Origin::Module(msg.target.clone()), m));
            }
            events.extend(out_events);
            effects.extend(out_effects);
        }

        Ok((events, effects, dispatches))
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
    out_effects: Vec<Effect>,
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

    fn request_effect(&mut self, eff: Effect) {
        self.out_effects.push(eff);
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

    fn request_effect(&mut self, _eff: Effect) {}
}
