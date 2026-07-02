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

use sdk::{Ctx, Effect, Env, Error, Event, Module, ModuleId, Msg, Origin, StateRoot};

/// hard cap on dispatches per `submit` (the root op plus all follow-ups). a
/// consensus/genesis constant — identical on every node — so the local re-entry
/// loop is guaranteed to terminate regardless of module behavior.
pub const MAX_DISPATCHES: u32 = 1024;

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
    /// the pre-consensus default: height/time 0 and an empty external origin, so
    /// [`Host::submit`] is byte-for-byte the old hardcoded behavior.
    fn default() -> Self {
        Self { height: 0, consensus_time: 0, origin: Origin::External(Vec::new()) }
    }
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
}

/// the deterministic state machine: a module registry + dispatch + drain.
#[derive(Default)]
pub struct Host {
    /// deterministic iteration order is load-bearing for snapshot + app-hash.
    registry: BTreeMap<ModuleId, Box<dyn Module>>,
}

impl Host {
    pub fn new() -> Self {
        Self { registry: BTreeMap::new() }
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

    /// the current app-hash: [`state::global_root`] over the registered modules.
    pub fn app_hash(&self) -> StateRoot {
        let mods: Vec<&dyn Module> = self.registry.values().map(|b| b.as_ref()).collect();
        state::global_root(&mods)
    }

    /// the live root of a single registered module (test/inspection accessor).
    pub fn module_root(&self, id: &str) -> Option<StateRoot> {
        self.registry.get(id).map(|m| m.root())
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
    pub async fn submit(&mut self, msg: Msg) -> Result<BlockOutcome, Error> {
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
    ) -> Result<BlockOutcome, Error> {
        // every module dispatched this block, in deterministic order — the set
        // the host commits or aborts at the boundary.
        let mut touched: BTreeSet<ModuleId> = BTreeSet::new();

        match self.drain(ctx, msg, &mut touched).await {
            Ok((events, effects)) => {
                // clean drain: publish every touched module's staged writes. this
                // is the ONLY place a module's state advances, so recompose the
                // app-hash AFTER.
                for id in &touched {
                    if let Some(m) = self.registry.get_mut(id) {
                        m.commit_block().await?;
                    }
                }
                Ok(BlockOutcome { app_hash: self.app_hash(), events, effects })
            }
            Err(e) => {
                // failure anywhere in the drain: discard every touched module's
                // staged writes. no root moves — the block leaves no trace.
                for id in &touched {
                    if let Some(m) = self.registry.get_mut(id) {
                        let _ = m.abort_block().await;
                    }
                }
                Err(e)
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
    ) -> Result<(Vec<Event>, Vec<Effect>), Error> {
        // block-constant across every dispatch this block — the agreed values.
        let height = ctx.height;
        let consensus_time = ctx.consensus_time;

        // the root op carries the real submitter's origin; follow-ups override.
        let mut queue: VecDeque<(Origin, Msg)> = VecDeque::from([(ctx.origin, msg)]);
        let mut events: Vec<Event> = Vec::new();
        let mut effects: Vec<Effect> = Vec::new();
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
            let mut snapshot: BTreeMap<ModuleId, StateRoot> =
                self.registry.iter().map(|(k, m)| (k.clone(), m.root())).collect();
            snapshot.insert(msg.target.clone(), me.root());

            let mut ctx = HostCtx {
                env: Env {
                    height,
                    consensus_time,
                    origin,
                    me: msg.target.clone(),
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
            let HostCtx { out_msgs, out_events, out_effects, .. } = ctx;

            // reinsert BEFORE propagating any error — a module never vanishes.
            self.registry.insert(msg.target.clone(), me);
            res?;

            // local-only re-entry: emitted msgs become follow-up ops, never
            // re-broadcast. events/effects leave the state machine.
            for m in out_msgs {
                queue.push_back((Origin::Module(msg.target.clone()), m));
            }
            events.extend(out_events);
            effects.extend(out_effects);
        }

        Ok((events, effects))
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
