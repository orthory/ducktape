//! the dispatch module — the network's task plane.
//!
//! a [`Recipe`] is a consensus-registered what-to-run manifest: required
//! capability, routing mode, output contract. a module runs one with
//! [`DispatchMsg::Dispatch`], carrying the ENTIRE input as opaque payload
//! data. this module stages a saga trigger for the work (rendezvous over the
//! capability's providers, or statically pinned to one node), validates the
//! agreed result against the recipe's [`OutputContract`], and delivers a
//! [`ResultEvent`] back to the dispatching module.
//!
//! ## the never-pop-stack rule (result delivery)
//!
//! a result is NEVER handed back inside the block that agreed on it. the saga
//! callback lands here, the checked outcome is staged into a MAILBOX, and the
//! block commits. the host drain notices the committed non-empty mailbox at
//! the start of a LATER block and injects one System-origin
//! [`DispatchMsg::DeliverPending`]; only that dispatch emits the events (at
//! most [`MAX_DELIVERIES_PER_BLOCK`] per block, FIFO) to their receivers. the
//! receiver consumes the result in its own block, its own failure domain.
//!
//! a receiver's `ResultEvent` intake is an intake like chat's hook intake:
//! it runs inside the delivery block, so it MUST NOT error on event content —
//! a decode/shape problem is the receiver's to swallow (log, mark, move on),
//! never to bubble. an erroring receiver would abort the delivery block, the
//! mailbox would stay committed-non-empty, and the host would re-inject next
//! block: a permanent abort loop. same discipline the platform already
//! demands of chat hook subscribers.
//!
//! ## self-containment
//!
//! this module imports no app module and no app interface. its collaborators
//! are saga (async work lifecycle) and the capability registry (indirectly,
//! via saga assignment). the receiver of a delivery is always the module
//! that dispatched — `Dispatch` is module-origin-only — so results route by
//! construction, never by configuration.

// the wire surface: this module's shared types, flattened at the crate root.
mod interface;
pub use interface::*;

use std::collections::BTreeMap;

use capability::{validate_resources, validate_tag};
use saga::{
    SagaCallback, SagaMsg, SagaOrigin, SagaOutcome, decode_callback, encode_msg as saga_encode_msg,
    put_origin, take_origin,
};
use sdk::codec;
use sdk::{Ctx, Error, Event, Module, ModuleId, Msg, Origin, StateRoot};
use sha2::{Digest as _, Sha256};

/// the field separator inside composite dispatch keys and saga ids (the shared
/// [`sdk::KEY_SEP`]). rejected inside caller-chosen ids by [`sdk::validate_id`]
/// so a crafted id can never forge another receiver's key.
const SEP: char = sdk::KEY_SEP;

/// the composite state key: dispatches are namespaced PER RECEIVER, so two
/// modules choosing the same local `dispatch_id` can never collide.
fn dispatch_key(receiver: &str, dispatch_id: &str) -> String {
    format!("{receiver}{SEP}{dispatch_id}")
}

/// the saga id a dispatch runs under — derived, collision-free, and stable so
/// a duplicate `Dispatch` maps onto the same (deduped) saga.
fn saga_id_for(key: &str) -> String {
    format!("dispatch{SEP}{key}")
}

// ---- state ---------------------------------------------------------------------

/// one recipe's committed state — exactly the wire [`Recipe`] (it is already
/// the canonical record; this module adds no private fields).
type RecipeState = Recipe;

/// one dispatch's committed state. the output contract is CAPTURED at
/// dispatch time, so removing or retuning the recipe never changes how an
/// in-flight run's result is judged.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DispatchState {
    receiver: ModuleId,
    dispatch_id: String,
    recipe_id: String,
    contract: OutputContract,
    saga_id: String,
    status: Status,
    outcome: Option<Result<Vec<u8>, String>>,
    created_at: u64,
    updated_at: u64,
}

/// the stored lifecycle discriminant (the wire shape carries `saga_id` inside
/// the variant; state keeps it as its own field).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    AwaitingResult,
    AwaitingDelivery,
    Delivered,
}

/// everything `root()` commits to, as one struct so the canonical encoding,
/// the root, and snapshot/install stay definitionally aligned.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Committed {
    recipes: BTreeMap<String, RecipeState>,
    dispatches: BTreeMap<String, DispatchState>,
    /// delivery FIFO: enqueue seq -> dispatch key. drained in seq order.
    mailbox: BTreeMap<u64, String>,
    /// the next mailbox seq — part of state so replay and snapshots agree.
    next_seq: u64,
}

// ---- canonical encoding ----------------------------------------------------------

/// canonical byte encoding of the committed state: u64-le counts, sorted-key
/// order, u64-le length prefixes, single-byte discriminants, 0/1 option tags.
/// this is the exact preimage `root()` hashes AND the snapshot format.
fn encode_committed(c: &Committed) -> Vec<u8> {
    let mut out = Vec::new();

    out.extend_from_slice(&(c.recipes.len() as u64).to_le_bytes());
    for (id, r) in &c.recipes {
        codec::push_str(&mut out, id);
        put_origin(&mut out, &r.owner);
        codec::push_str(&mut out, &r.description);
        codec::push_str(&mut out, &r.capability);
        match &r.routing {
            Routing::Rendezvous => out.push(0),
            Routing::Pinned(key) => {
                out.push(1);
                codec::push_bytes(&mut out, key);
            }
        }
        out.push(match r.output_contract {
            OutputContract::Text => 0,
            OutputContract::Json => 1,
        });
        out.extend_from_slice(&r.max_attempts.to_le_bytes());
        codec::push_opt_u64(&mut out, r.deadline_views);
        codec::push_opt_u64(&mut out, r.lease_views);
        out.extend_from_slice(&r.created_at.to_le_bytes());
        out.extend_from_slice(&r.updated_at.to_le_bytes());
    }

    out.extend_from_slice(&(c.dispatches.len() as u64).to_le_bytes());
    for (key, d) in &c.dispatches {
        codec::push_str(&mut out, key);
        codec::push_str(&mut out, &d.receiver);
        codec::push_str(&mut out, &d.dispatch_id);
        codec::push_str(&mut out, &d.recipe_id);
        out.push(match d.contract {
            OutputContract::Text => 0,
            OutputContract::Json => 1,
        });
        codec::push_str(&mut out, &d.saga_id);
        out.push(match d.status {
            Status::AwaitingResult => 0,
            Status::AwaitingDelivery => 1,
            Status::Delivered => 2,
        });
        match &d.outcome {
            None => out.push(0),
            Some(Ok(bytes)) => {
                out.push(1);
                codec::push_bytes(&mut out, bytes);
            }
            Some(Err(error)) => {
                out.push(2);
                codec::push_str(&mut out, error);
            }
        }
        out.extend_from_slice(&d.created_at.to_le_bytes());
        out.extend_from_slice(&d.updated_at.to_le_bytes());
    }

    out.extend_from_slice(&(c.mailbox.len() as u64).to_le_bytes());
    for (seq, key) in &c.mailbox {
        out.extend_from_slice(&seq.to_le_bytes());
        codec::push_str(&mut out, key);
    }
    out.extend_from_slice(&c.next_seq.to_le_bytes());

    out
}

fn committed_root(c: &Committed) -> StateRoot {
    StateRoot(Sha256::digest(encode_committed(c)).into())
}

// ---- strict decode (untrusted snapshot bytes) -------------------------------------

fn take_contract(cur: &mut codec::Cursor) -> Result<OutputContract, Error> {
    Ok(match cur.byte("snapshot contract")? {
        0 => OutputContract::Text,
        1 => OutputContract::Json,
        d => {
            return Err(Error::Module(format!(
                "snapshot has unknown contract discriminant {d}"
            )));
        }
    })
}

/// keys must be strictly ascending: one byte encoding per state, uniqueness
/// for free.
fn check_ascending<V>(map: &BTreeMap<String, V>, key: &str) -> Result<(), Error> {
    match map.iter().next_back() {
        Some((last, _)) if last.as_str() >= key => {
            Err(Error::Module("snapshot keys not strictly ascending".into()))
        }
        _ => Ok(()),
    }
}

fn decode_committed(buf: &[u8]) -> Result<Committed, Error> {
    let mut cur = codec::Cursor::new(buf);
    let mut c = Committed::default();

    let recipes = cur.u64("snapshot recipe count")?;
    // every recipe costs at least its fixed-width frame; a count the input
    // cannot possibly hold is rejected before the loop builds anything.
    const MIN_RECIPE_BYTES: u64 = 8 + 1 + 8 + 8 + 1 + 1 + 4 + 1 + 1 + 8 + 8;
    cur.bound(recipes, MIN_RECIPE_BYTES, "snapshot recipe")?;
    for _ in 0..recipes {
        let recipe_id = cur.string("snapshot recipe id")?;
        check_ascending(&c.recipes, &recipe_id)?;
        let owner = take_origin(&mut cur)?;
        let description = cur.string("snapshot description")?;
        let capability = cur.string("snapshot capability")?;
        let routing = match cur.byte("snapshot routing")? {
            0 => Routing::Rendezvous,
            1 => Routing::Pinned(cur.bytes("snapshot routing pin")?.to_vec()),
            d => {
                return Err(Error::Module(format!(
                    "snapshot has unknown routing discriminant {d}"
                )));
            }
        };
        let output_contract = take_contract(&mut cur)?;
        let max_attempts = cur.u32("snapshot max_attempts")?;
        let deadline_views = cur.opt_u64("snapshot deadline_views")?;
        let lease_views = cur.opt_u64("snapshot lease_views")?;
        let created_at = cur.u64("snapshot created_at")?;
        let updated_at = cur.u64("snapshot updated_at")?;
        c.recipes.insert(
            recipe_id.clone(),
            Recipe {
                recipe_id,
                owner,
                description,
                capability,
                routing,
                output_contract,
                max_attempts,
                deadline_views,
                lease_views,
                created_at,
                updated_at,
            },
        );
    }

    let dispatches = cur.u64("snapshot dispatch count")?;
    const MIN_DISPATCH_BYTES: u64 = 8 + 8 + 8 + 8 + 1 + 8 + 1 + 1 + 8 + 8;
    cur.bound(dispatches, MIN_DISPATCH_BYTES, "snapshot dispatch")?;
    for _ in 0..dispatches {
        let key = cur.string("snapshot dispatch key")?;
        check_ascending(&c.dispatches, &key)?;
        let receiver = cur.string("snapshot receiver")?;
        let dispatch_id = cur.string("snapshot dispatch id")?;
        let recipe_id = cur.string("snapshot recipe id")?;
        let contract = take_contract(&mut cur)?;
        let saga_id = cur.string("snapshot saga id")?;
        let status = match cur.byte("snapshot status")? {
            0 => Status::AwaitingResult,
            1 => Status::AwaitingDelivery,
            2 => Status::Delivered,
            d => {
                return Err(Error::Module(format!(
                    "snapshot has unknown status discriminant {d}"
                )));
            }
        };
        let outcome = match cur.byte("snapshot outcome tag")? {
            0 => None,
            1 => Some(Ok(cur.bytes("snapshot outcome")?.to_vec())),
            2 => Some(Err(cur.string("snapshot outcome error")?)),
            t => {
                return Err(Error::Module(format!(
                    "snapshot has unknown outcome tag {t}"
                )));
            }
        };
        let created_at = cur.u64("snapshot created_at")?;
        let updated_at = cur.u64("snapshot updated_at")?;
        c.dispatches.insert(
            key,
            DispatchState {
                receiver,
                dispatch_id,
                recipe_id,
                contract,
                saga_id,
                status,
                outcome,
                created_at,
                updated_at,
            },
        );
    }

    let mailbox = cur.u64("snapshot mailbox count")?;
    const MIN_MAILBOX_BYTES: u64 = 8 + 8;
    cur.bound(mailbox, MIN_MAILBOX_BYTES, "snapshot mailbox")?;
    for _ in 0..mailbox {
        let seq = cur.u64("snapshot mailbox seq")?;
        if let Some((last, _)) = c.mailbox.iter().next_back()
            && *last >= seq
        {
            return Err(Error::Module(
                "snapshot mailbox seqs not strictly ascending".into(),
            ));
        }
        let key = cur.string("snapshot mailbox key")?;
        c.mailbox.insert(seq, key);
    }
    c.next_seq = cur.u64("snapshot next_seq")?;

    cur.finish("snapshot")?;
    Ok(c)
}

// ---- the module -----------------------------------------------------------------

pub struct DispatchModule {
    id: ModuleId,
    /// the saga module dispatches trigger through — genesis config, not state.
    saga: ModuleId,
    /// committed state — what `root()` and the root-hash commit to.
    committed: Committed,
    /// this block's staged writes, read ahead of committed state
    /// (read-your-writes) but merged in only at `commit_block`.
    staged_recipes: BTreeMap<String, Option<RecipeState>>,
    staged_dispatches: BTreeMap<String, DispatchState>,
    /// staged mailbox overlay: whole-queue copy-on-first-write — the queue is
    /// small (delivery-bounded) and a full copy keeps abort trivially correct.
    staged_queue: Option<(BTreeMap<u64, String>, u64)>,
}

impl DispatchModule {
    pub fn new(id: impl Into<ModuleId>, saga: impl Into<ModuleId>) -> Self {
        Self {
            id: id.into(),
            saga: saga.into(),
            committed: Committed::default(),
            staged_recipes: BTreeMap::new(),
            staged_dispatches: BTreeMap::new(),
            staged_queue: None,
        }
    }

    // ---- staged-overlay reads ----------------------------------------------------

    fn recipe(&self, recipe_id: &str) -> Option<&RecipeState> {
        match self.staged_recipes.get(recipe_id) {
            Some(staged) => staged.as_ref(),
            None => self.committed.recipes.get(recipe_id),
        }
    }

    fn dispatch(&self, key: &str) -> Option<&DispatchState> {
        self.staged_dispatches
            .get(key)
            .or_else(|| self.committed.dispatches.get(key))
    }

    fn queue(&self) -> (&BTreeMap<u64, String>, u64) {
        match &self.staged_queue {
            Some((mailbox, next_seq)) => (mailbox, *next_seq),
            None => (&self.committed.mailbox, self.committed.next_seq),
        }
    }

    fn queue_mut(&mut self) -> &mut (BTreeMap<u64, String>, u64) {
        if self.staged_queue.is_none() {
            self.staged_queue = Some((self.committed.mailbox.clone(), self.committed.next_seq));
        }
        self.staged_queue.as_mut().expect("staged above")
    }

    // ---- validation helpers --------------------------------------------------------

    fn validate_recipe_shape(
        capability: &str,
        routing: &Routing,
        max_attempts: u32,
        description: &str,
    ) -> Result<(), Error> {
        validate_tag(capability).map_err(Error::Module)?;
        if let Routing::Pinned(key) = routing
            && key.is_empty()
        {
            return Err(Error::Module("routing Pinned key must be non-empty".into()));
        }
        if max_attempts == 0 {
            return Err(Error::Module("max_attempts must be >= 1".into()));
        }
        if description.len() > MAX_DESCRIPTION_BYTES {
            return Err(Error::Module(format!(
                "description is {} bytes; the cap is {MAX_DESCRIPTION_BYTES}",
                description.len()
            )));
        }
        Ok(())
    }

    /// the canonical state form of the acting origin — recipe ownership.
    fn acting_origin(origin: &Origin) -> Result<SagaOrigin, Error> {
        match origin {
            Origin::External(key) if key.is_empty() => {
                Err(Error::Module("external origin key is empty".into()))
            }
            Origin::External(key) => Ok(SagaOrigin::External(key.clone())),
            Origin::Module(module) => Ok(SagaOrigin::Module(module.clone())),
            Origin::System => Ok(SagaOrigin::System),
        }
    }

    fn owned_recipe(&self, ctx: &dyn Ctx, recipe_id: &str) -> Result<&RecipeState, Error> {
        let recipe = self
            .recipe(recipe_id)
            .ok_or_else(|| Error::Module(format!("unknown recipe {recipe_id:?}")))?;
        if recipe.owner != Self::acting_origin(&ctx.env().origin)? {
            return Err(Error::Module(format!(
                "recipe {recipe_id:?} is not owned by this origin"
            )));
        }
        Ok(recipe)
    }

    // ---- contract validation ---------------------------------------------------------

    /// judge a terminal saga outcome against the dispatch's captured
    /// contract. deterministic: bytes in, verdict out.
    fn judged_outcome(contract: OutputContract, outcome: SagaOutcome) -> Result<Vec<u8>, String> {
        match outcome {
            SagaOutcome::Done(result) => match contract {
                OutputContract::Text => Ok(result),
                OutputContract::Json => {
                    match serde_json::from_slice::<serde_json::Value>(&result) {
                        Ok(_) => Ok(result),
                        Err(e) => Err(format!("output contract violation: not JSON: {e}")),
                    }
                }
            },
            SagaOutcome::Failed(error) => Err(error),
            SagaOutcome::TimedOut => Err("timed out".into()),
            SagaOutcome::Cancelled => Err("cancelled".into()),
        }
    }

    // ---- intakes ----------------------------------------------------------------------

    /// an observability breadcrumb for the no-fail callback/intake arms: they
    /// swallow poison instead of aborting the block, and this is the trace
    /// the swallow leaves behind.
    fn note(&self, ctx: &mut dyn Ctx, what: String) {
        ctx.emit_event(Event {
            source: self.id.clone(),
            payload: what.into_bytes(),
        });
    }

    /// the saga callback intake: judge the outcome, stage it, enqueue the
    /// delivery. correlation is the composite dispatch key in the callback's
    /// echoed `reply_payload`. every mismatch is a deterministic no-op — a
    /// finalized callback must never abort its block (an abort would replay
    /// as a no-op and re-abort forever), so an undecodable payload is
    /// swallowed as a staged no-op with a diagnostic event, never an `Err`.
    fn on_saga_callback(&mut self, ctx: &mut dyn Ctx, payload: &[u8]) -> Result<(), Error> {
        let callback: SagaCallback = match decode_callback(payload) {
            Ok(callback) => callback,
            Err(e) => {
                self.note(ctx, format!("dropped undecodable saga callback: {e}"));
                return Ok(());
            }
        };
        let key = String::from_utf8_lossy(&callback.payload).into_owned();
        let Some(current) = self.dispatch(&key) else {
            return Ok(());
        };
        if current.status != Status::AwaitingResult || current.saga_id != callback.saga_id {
            return Ok(());
        }
        let mut dispatch = current.clone();
        dispatch.outcome = Some(Self::judged_outcome(dispatch.contract, callback.outcome));
        dispatch.status = Status::AwaitingDelivery;
        dispatch.updated_at = ctx.env().consensus_time;
        self.staged_dispatches.insert(key.clone(), dispatch);
        let (mailbox, next_seq) = self.queue_mut();
        mailbox.insert(*next_seq, key);
        *next_seq += 1;
        Ok(())
    }

    fn on_dispatch(
        &mut self,
        ctx: &mut dyn Ctx,
        dispatch_id: String,
        recipe_id: String,
        payload: Vec<u8>,
        demands: BTreeMap<String, u64>,
        admission: AdmissionPolicy,
    ) -> Result<(), Error> {
        // module-origin only: the dispatching module IS the receiver, so
        // results always have somewhere to land. an external submitter has
        // no execute intake — nothing could ever receive its result.
        let Origin::Module(receiver) = &ctx.env().origin else {
            return Err(Error::Module(
                "Dispatch is module-origin only (the dispatching module receives the result)"
                    .into(),
            ));
        };
        let receiver = receiver.clone();
        sdk::validate_id("dispatch_id", &dispatch_id, MAX_ID_BYTES)?;
        if payload.len() > MAX_PAYLOAD_BYTES {
            return Err(Error::Module(format!(
                "payload is {} bytes; the cap is {MAX_PAYLOAD_BYTES}",
                payload.len()
            )));
        }
        // the same validate_resources invariant saga holds at trigger time —
        // checked here too so a malformed demand set is attributed to THIS
        // dispatch, not a downstream saga message.
        validate_resources(&demands).map_err(Error::Module)?;
        let recipe = self
            .recipe(&recipe_id)
            .ok_or_else(|| Error::Module(format!("unknown recipe {recipe_id:?}")))?
            .clone();
        let key = dispatch_key(&receiver, &dispatch_id);
        // the receiver's idempotency: the first dispatch under a key wins,
        // a duplicate is a deterministic no-op (mirrors the saga trigger).
        if self.dispatch(&key).is_some() {
            return Ok(());
        }
        let env = ctx.env();
        let now = env.consensus_time;
        let saga_id = saga_id_for(&key);
        let (capability, pinned_assignee) = match &recipe.routing {
            Routing::Rendezvous => (Some(recipe.capability.clone()), None),
            Routing::Pinned(node) => (Some(recipe.capability.clone()), Some(node.clone())),
        };
        ctx.emit_msg(Msg {
            target: self.saga.clone(),
            payload: saga_encode_msg(&SagaMsg::Trigger {
                saga_id: saga_id.clone(),
                spec: encode_work_spec(&WorkSpec {
                    kind: WORK_SPEC_KIND.into(),
                    dispatch_id: dispatch_id.clone(),
                    capability: recipe.capability.clone(),
                    payload,
                    demands: demands.clone(),
                    admission,
                }),
                reply_to: Some(self.id.clone()),
                reply_payload: key.clone().into_bytes(),
                deadline: recipe.deadline_views.map(|v| env.height.saturating_add(v)),
                max_attempts: recipe.max_attempts,
                lease_views: recipe.lease_views,
                capability,
                // the ONE source: the same `demands` value just cloned into
                // the WorkSpec above, so the trigger and the spec can never
                // drift apart.
                demands,
                pinned_assignee,
            }),
        });
        self.staged_dispatches.insert(
            key,
            DispatchState {
                receiver,
                dispatch_id,
                recipe_id,
                contract: recipe.output_contract,
                saga_id,
                status: Status::AwaitingResult,
                outcome: None,
                created_at: now,
                updated_at: now,
            },
        );
        Ok(())
    }

    /// receiver-scoped cancellation: the emitting module may cancel its own
    /// in-flight dispatch. only the saga is told — the terminal `Cancelled`
    /// callback then drives the normal AwaitingResult → AwaitingDelivery →
    /// delivery path, so there is exactly one result state machine.
    fn on_cancel(&mut self, ctx: &mut dyn Ctx, dispatch_id: String) -> Result<(), Error> {
        let Origin::Module(receiver) = &ctx.env().origin else {
            return Err(Error::Module(
                "CancelDispatch is module-origin only (a module cancels its own dispatch)".into(),
            ));
        };
        let key = dispatch_key(receiver, &dispatch_id);
        // unknown or already-terminal: a deterministic no-op, so double
        // cancels and cancel/result races are all safe.
        let Some(dispatch) = self.dispatch(&key) else {
            return Ok(());
        };
        if dispatch.status != Status::AwaitingResult {
            return Ok(());
        }
        ctx.emit_msg(Msg {
            target: self.saga.clone(),
            payload: saga_encode_msg(&SagaMsg::Cancel {
                saga_id: dispatch.saga_id.clone(),
            }),
        });
        Ok(())
    }

    fn on_reassign(
        &mut self,
        ctx: &mut dyn Ctx,
        dispatch_id: String,
        attempt: u32,
    ) -> Result<(), Error> {
        let Origin::Module(receiver) = &ctx.env().origin else {
            return Err(Error::Module(
                "ReassignDispatch is module-origin only (a module reassigns its own dispatch)"
                    .into(),
            ));
        };
        let key = dispatch_key(receiver, &dispatch_id);
        let Some(dispatch) = self.dispatch(&key) else {
            return Ok(());
        };
        if dispatch.status != Status::AwaitingResult {
            return Ok(());
        }
        ctx.emit_msg(Msg {
            target: self.saga.clone(),
            payload: saga_encode_msg(&SagaMsg::Reassign {
                saga_id: dispatch.saga_id.clone(),
                attempt,
            }),
        });
        Ok(())
    }

    /// the host-injected delivery sweep: emit up to
    /// [`MAX_DELIVERIES_PER_BLOCK`] mailbox events, FIFO, each as one
    /// follow-up `Msg` to its receiver.
    fn on_deliver_pending(&mut self, ctx: &mut dyn Ctx) -> Result<(), Error> {
        if !matches!(ctx.env().origin, Origin::System) {
            return Err(Error::Module(
                "DeliverPending is System-origin only (host-injected)".into(),
            ));
        }
        let now = ctx.env().consensus_time;
        let batch: Vec<(u64, String)> = self
            .queue()
            .0
            .iter()
            .take(MAX_DELIVERIES_PER_BLOCK)
            .map(|(seq, key)| (*seq, key.clone()))
            .collect();
        for (seq, key) in batch {
            // a mailbox entry without a dispatch record (or without a
            // recorded outcome) cannot be built by this module's own
            // transitions — but this arm is host-injected every block while
            // the mailbox is non-empty, so erroring here would abort every
            // future block (the poison loop the module header forbids).
            // stage-drop the orphan and leave a diagnostic event instead.
            let Some(current) = self.dispatch(&key).cloned() else {
                self.note(ctx, format!("dropped orphaned mailbox entry {key:?}"));
                self.queue_mut().0.remove(&seq);
                continue;
            };
            let Some(outcome) = current.outcome.clone() else {
                self.note(
                    ctx,
                    format!("dropped mailbox entry {key:?} with no recorded outcome"),
                );
                self.queue_mut().0.remove(&seq);
                continue;
            };
            let mut dispatch = current;
            dispatch.status = Status::Delivered;
            dispatch.updated_at = now;
            ctx.emit_msg(Msg {
                target: dispatch.receiver.clone(),
                payload: encode_result_event(&ResultEvent {
                    dispatch_id: dispatch.dispatch_id.clone(),
                    recipe_id: dispatch.recipe_id.clone(),
                    outcome,
                }),
            });
            self.staged_dispatches.insert(key, dispatch);
            self.queue_mut().0.remove(&seq);
        }
        Ok(())
    }

    fn on_admin(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            DispatchMsg::RegisterRecipe {
                recipe_id,
                description,
                capability,
                routing,
                output_contract,
                max_attempts,
                deadline_views,
                lease_views,
            } => {
                sdk::validate_id("recipe_id", &recipe_id, MAX_ID_BYTES)?;
                Self::validate_recipe_shape(&capability, &routing, max_attempts, &description)?;
                if self.recipe(&recipe_id).is_some() {
                    return Err(Error::Module(format!(
                        "recipe {recipe_id:?} already exists"
                    )));
                }
                let now = ctx.env().consensus_time;
                let recipe = Recipe {
                    recipe_id: recipe_id.clone(),
                    owner: Self::acting_origin(&ctx.env().origin)?,
                    description,
                    capability,
                    routing,
                    output_contract,
                    max_attempts,
                    deadline_views,
                    lease_views,
                    created_at: now,
                    updated_at: now,
                };
                self.staged_recipes.insert(recipe_id, Some(recipe));
                Ok(())
            }
            DispatchMsg::UpdateRecipe {
                recipe_id,
                description,
                capability,
                routing,
                output_contract,
                max_attempts,
            } => {
                let mut recipe = self.owned_recipe(ctx, &recipe_id)?.clone();
                if let Some(description) = description {
                    recipe.description = description;
                }
                if let Some(capability) = capability {
                    recipe.capability = capability;
                }
                if let Some(routing) = routing {
                    recipe.routing = routing;
                }
                if let Some(output_contract) = output_contract {
                    recipe.output_contract = output_contract;
                }
                if let Some(max_attempts) = max_attempts {
                    recipe.max_attempts = max_attempts;
                }
                Self::validate_recipe_shape(
                    &recipe.capability,
                    &recipe.routing,
                    recipe.max_attempts,
                    &recipe.description,
                )?;
                recipe.updated_at = ctx.env().consensus_time;
                self.staged_recipes.insert(recipe_id, Some(recipe));
                Ok(())
            }
            DispatchMsg::RemoveRecipe { recipe_id } => {
                self.owned_recipe(ctx, &recipe_id)?;
                self.staged_recipes.insert(recipe_id, None);
                Ok(())
            }
            DispatchMsg::Dispatch {
                dispatch_id,
                recipe_id,
                payload,
                demands,
                admission,
            } => self.on_dispatch(ctx, dispatch_id, recipe_id, payload, demands, admission),
            DispatchMsg::CancelDispatch { dispatch_id } => self.on_cancel(ctx, dispatch_id),
            DispatchMsg::ReassignDispatch {
                dispatch_id,
                attempt,
            } => self.on_reassign(ctx, dispatch_id, attempt),
            DispatchMsg::DeliverPending {} => self.on_deliver_pending(ctx),
            // the block's EXISTENCE is the point (it carries the host's
            // delivery injection); the op itself stages nothing.
            DispatchMsg::Nudge {} => Ok(()),
        }
    }

    fn view(d: &DispatchState) -> DispatchView {
        DispatchView {
            dispatch_id: d.dispatch_id.clone(),
            recipe_id: d.recipe_id.clone(),
            receiver: d.receiver.clone(),
            status: match d.status {
                Status::AwaitingResult => DispatchStatus::AwaitingResult {
                    saga_id: d.saga_id.clone(),
                },
                Status::AwaitingDelivery => DispatchStatus::AwaitingDelivery,
                Status::Delivered => DispatchStatus::Delivered,
            },
            outcome: d.outcome.clone(),
            created_at: d.created_at,
            updated_at: d.updated_at,
        }
    }

    // ---- state-sync -----------------------------------------------------------------

    /// serialize the COMMITTED state (never the staged overlay) into the
    /// canonical encoding `root()` commits to.
    pub fn snapshot(&self) -> Vec<u8> {
        encode_committed(&self.committed)
    }

    /// adopt a peer's snapshot — only after the decoded temporaries re-derive
    /// `expected` via the exact `root()` algorithm. all-or-nothing.
    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        let committed = decode_committed(bytes)?;
        sdk::verify_snapshot_root(committed_root(&committed), expected)?;
        self.committed = committed;
        self.staged_recipes.clear();
        self.staged_dispatches.clear();
        self.staged_queue = None;
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl Module for DispatchModule {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        committed_root(&self.committed)
    }

    fn snapshot_bytes(&self) -> Option<Vec<u8>> {
        Some(self.snapshot())
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        // route by origin: the saga module sends exactly one payload family
        // (terminal callbacks); everything else is the admin/dispatch surface.
        match &ctx.env().origin {
            Origin::Module(m) if *m == self.saga => {
                let payload = msg.payload.clone();
                self.on_saga_callback(ctx, &payload)
            }
            _ => self.on_admin(ctx, msg),
        }
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        // COMMITTED state only: the host's delivery injection reads
        // PendingDeliveries between blocks, and a staged overlay must never
        // leak into that decision.
        match decode_query(req).map_err(Error::Module)? {
            DispatchQuery::Recipes => Ok(encode_reply(&DispatchReply::Recipes(
                self.committed.recipes.values().cloned().collect(),
            ))),
            DispatchQuery::Recipe { recipe_id } => Ok(encode_reply(&DispatchReply::Recipe(
                self.committed.recipes.get(&recipe_id).cloned(),
            ))),
            DispatchQuery::Dispatch {
                receiver,
                dispatch_id,
            } => Ok(encode_reply(&DispatchReply::Dispatch(
                self.committed
                    .dispatches
                    .get(&dispatch_key(&receiver, &dispatch_id))
                    .map(Self::view),
            ))),
            DispatchQuery::PendingDeliveries => Ok(encode_reply(
                &DispatchReply::PendingDeliveries(self.committed.mailbox.len() as u64),
            )),
        }
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        for (id, staged) in std::mem::take(&mut self.staged_recipes) {
            match staged {
                Some(recipe) => self.committed.recipes.insert(id, recipe),
                None => self.committed.recipes.remove(&id),
            };
        }
        for (key, dispatch) in std::mem::take(&mut self.staged_dispatches) {
            self.committed.dispatches.insert(key, dispatch);
        }
        if let Some((mailbox, next_seq)) = self.staged_queue.take() {
            self.committed.mailbox = mailbox;
            self.committed.next_seq = next_seq;
        }
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.staged_recipes.clear();
        self.staged_dispatches.clear();
        self.staged_queue = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;
    use saga::{decode_msg as saga_decode_msg, encode_callback};
    use sdk::Env;

    use sdk_testkit::TestCtx;

    // dispatch's execute reads only env; the shared TestCtx captures emitted
    // msgs/events (read via msgs()/events()). module_root is never consulted.
    fn mk_ctx(height: u64, origin: Origin) -> TestCtx {
        TestCtx::with_env(Env {
            height,
            consensus_time: height,
            origin,
            me: "dispatch".into(),
        })
    }

    fn module() -> DispatchModule {
        DispatchModule::new("dispatch", "saga")
    }
    fn exec(
        m: &mut DispatchModule,
        ctx: &mut TestCtx,
        payload: &DispatchMsg,
    ) -> Result<(), Error> {
        let msg = Msg {
            target: "dispatch".into(),
            payload: crate::encode_msg(payload),
        };
        block_on(m.execute(ctx, &msg))
    }
    fn commit(m: &mut DispatchModule) {
        block_on(m.commit_block()).unwrap();
    }
    fn register(kind: OutputContract, routing: Routing) -> DispatchMsg {
        DispatchMsg::RegisterRecipe {
            recipe_id: "summarize".into(),
            description: "test recipe".into(),
            capability: "alpha".into(),
            routing,
            output_contract: kind,
            max_attempts: 2,
            deadline_views: Some(100),
            lease_views: None,
        }
    }
    fn owner() -> Origin {
        Origin::External(b"owner".to_vec())
    }
    fn dispatch_op(id: &str, payload: &[u8]) -> DispatchMsg {
        DispatchMsg::Dispatch {
            dispatch_id: id.into(),
            recipe_id: "summarize".into(),
            payload: payload.to_vec(),
            demands: BTreeMap::new(),
            admission: AdmissionPolicy::Queue,
        }
    }
    /// run register (as the external owner) + dispatch (as module "caller"),
    /// committed — the shared preamble of the callback/delivery tests.
    fn registered_and_dispatched(m: &mut DispatchModule, kind: OutputContract) -> String {
        let mut ctx = mk_ctx(0, owner());
        exec(m, &mut ctx, &register(kind, Routing::Rendezvous)).unwrap();
        let mut ctx = mk_ctx(5, Origin::Module("caller".into()));
        exec(m, &mut ctx, &dispatch_op("d1", b"input")).unwrap();
        commit(m);
        dispatch_key("caller", "d1")
    }
    fn callback_for(m: &mut DispatchModule, key: &str, outcome: SagaOutcome) -> Result<(), Error> {
        let mut ctx = mk_ctx(9, Origin::Module("saga".into()));
        let msg = Msg {
            target: "dispatch".into(),
            payload: encode_callback(&SagaCallback {
                saga_id: saga_id_for(key),
                payload: key.as_bytes().to_vec(),
                outcome,
            }),
        };
        block_on(m.execute(&mut ctx, &msg))
    }
    fn get_dispatch(m: &DispatchModule, key: &str) -> Option<DispatchView> {
        // the tests address dispatches by their composite state key; split it
        // back into the wire query's (receiver, local id) coordinates.
        let (receiver, dispatch_id) = key.split_once(SEP).expect("composite key");
        let reply = block_on(m.query(&crate::encode_query(&DispatchQuery::Dispatch {
            receiver: receiver.into(),
            dispatch_id: dispatch_id.into(),
        })))
        .unwrap();
        match crate::decode_reply(&reply).unwrap() {
            DispatchReply::Dispatch(v) => v,
            other => panic!("expected Dispatch reply, got {other:?}"),
        }
    }
    fn pending_deliveries(m: &DispatchModule) -> u64 {
        let reply =
            block_on(m.query(&crate::encode_query(&DispatchQuery::PendingDeliveries))).unwrap();
        match crate::decode_reply(&reply).unwrap() {
            DispatchReply::PendingDeliveries(n) => n,
            other => panic!("expected PendingDeliveries reply, got {other:?}"),
        }
    }

    #[test]
    fn recipe_registration_validates_and_gates_mutation_by_owner() {
        let mut m = module();
        let mut ctx = mk_ctx(0, owner());
        exec(
            &mut m,
            &mut ctx,
            &register(OutputContract::Text, Routing::Rendezvous),
        )
        .unwrap();
        // duplicate id is an error.
        let err = exec(
            &mut m,
            &mut ctx,
            &register(OutputContract::Text, Routing::Rendezvous),
        )
        .unwrap_err();
        assert!(err.to_string().contains("already exists"), "got {err}");

        // shape rules: bad tag, empty pinned key, zero attempts.
        for (bad, needle) in [
            (
                DispatchMsg::RegisterRecipe {
                    recipe_id: "r2".into(),
                    description: String::new(),
                    capability: "NOT A TAG".into(),
                    routing: Routing::Rendezvous,
                    output_contract: OutputContract::Text,
                    max_attempts: 1,
                    deadline_views: None,
                    lease_views: None,
                },
                "invalid characters",
            ),
            (
                DispatchMsg::RegisterRecipe {
                    recipe_id: "r2".into(),
                    description: String::new(),
                    capability: "alpha".into(),
                    routing: Routing::Pinned(Vec::new()),
                    output_contract: OutputContract::Text,
                    max_attempts: 1,
                    deadline_views: None,
                    lease_views: None,
                },
                "Pinned key",
            ),
            (
                DispatchMsg::RegisterRecipe {
                    recipe_id: "r2".into(),
                    description: String::new(),
                    capability: "alpha".into(),
                    routing: Routing::Rendezvous,
                    output_contract: OutputContract::Text,
                    max_attempts: 0,
                    deadline_views: None,
                    lease_views: None,
                },
                "max_attempts",
            ),
        ] {
            let err = exec(&mut m, &mut ctx, &bad).unwrap_err();
            assert!(err.to_string().contains(needle), "wanted {needle} in {err}");
        }

        // a foreign origin cannot update or remove.
        let mut foreign = mk_ctx(0, Origin::External(b"other".to_vec()));
        let err = exec(
            &mut m,
            &mut foreign,
            &DispatchMsg::RemoveRecipe {
                recipe_id: "summarize".into(),
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("not owned"), "got {err}");

        // the owner can update; the update is validated too.
        let err = exec(
            &mut m,
            &mut ctx,
            &DispatchMsg::UpdateRecipe {
                recipe_id: "summarize".into(),
                description: None,
                capability: Some("ALSO NOT A TAG".into()),
                routing: None,
                output_contract: None,
                max_attempts: None,
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("invalid characters"), "got {err}");
        exec(
            &mut m,
            &mut ctx,
            &DispatchMsg::UpdateRecipe {
                recipe_id: "summarize".into(),
                description: None,
                capability: Some("beta".into()),
                routing: None,
                output_contract: None,
                max_attempts: Some(3),
            },
        )
        .unwrap();
        commit(&mut m);
        let reply = block_on(m.query(&crate::encode_query(&DispatchQuery::Recipe {
            recipe_id: "summarize".into(),
        })))
        .unwrap();
        let DispatchReply::Recipe(Some(recipe)) = crate::decode_reply(&reply).unwrap() else {
            panic!("recipe committed");
        };
        assert_eq!(recipe.capability, "beta");
        assert_eq!(recipe.max_attempts, 3);
    }

    #[test]
    fn dispatch_stages_trigger_with_recipe_routing_and_dedups_per_receiver() {
        let mut m = module();
        let mut ctx = mk_ctx(0, owner());
        exec(
            &mut m,
            &mut ctx,
            &register(OutputContract::Json, Routing::Pinned(vec![7u8; 32])),
        )
        .unwrap();
        commit(&mut m);

        // external dispatch is refused.
        let mut external = mk_ctx(0, owner());
        let err = exec(&mut m, &mut external, &dispatch_op("d1", b"x")).unwrap_err();
        assert!(err.to_string().contains("module-origin only"), "got {err}");

        // an unknown recipe is an error; an oversized payload is an error.
        let mut caller = mk_ctx(5, Origin::Module("caller".into()));
        let err = exec(
            &mut m,
            &mut caller,
            &DispatchMsg::Dispatch {
                dispatch_id: "d1".into(),
                recipe_id: "nope".into(),
                payload: b"x".to_vec(),
                demands: BTreeMap::new(),
                admission: AdmissionPolicy::Queue,
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("unknown recipe"), "got {err}");
        let err = exec(
            &mut m,
            &mut caller,
            &dispatch_op("d1", &vec![0u8; MAX_PAYLOAD_BYTES + 1]),
        )
        .unwrap_err();
        assert!(err.to_string().contains("payload is"), "got {err}");

        // the real dispatch stages exactly one trigger carrying the recipe's
        // capability, routing pin, deadline, and the work-spec payload.
        exec(&mut m, &mut caller, &dispatch_op("d1", b"input")).unwrap();
        assert_eq!(caller.msgs().len(), 1);
        assert_eq!(caller.msgs()[0].target, "saga");
        let SagaMsg::Trigger {
            saga_id,
            spec,
            reply_to,
            reply_payload,
            deadline,
            max_attempts,
            capability,
            pinned_assignee,
            ..
        } = saga_decode_msg(&caller.msgs()[0].payload).unwrap()
        else {
            panic!("expected a trigger");
        };
        let key = dispatch_key("caller", "d1");
        assert_eq!(saga_id, saga_id_for(&key));
        assert_eq!(reply_to.as_deref(), Some("dispatch"));
        assert_eq!(reply_payload, key.clone().into_bytes());
        assert_eq!(deadline, Some(5 + 100));
        assert_eq!(max_attempts, 2);
        assert_eq!(capability.as_deref(), Some("alpha"));
        assert_eq!(pinned_assignee, Some(vec![7u8; 32]));
        let work = crate::decode_work_spec(&spec).unwrap();
        assert_eq!(work.dispatch_id, "d1");
        assert_eq!(work.capability, "alpha");
        assert_eq!(work.payload, b"input");

        // a duplicate under the same receiver is a deterministic no-op...
        exec(&mut m, &mut caller, &dispatch_op("d1", b"other")).unwrap();
        assert_eq!(caller.msgs().len(), 1, "no second trigger");
        // ...but another receiver's identical id is a distinct dispatch.
        let mut other = mk_ctx(5, Origin::Module("other".into()));
        exec(&mut m, &mut other, &dispatch_op("d1", b"input")).unwrap();
        assert_eq!(other.msgs().len(), 1);
        commit(&mut m);
        assert!(get_dispatch(&m, &key).is_some());
        assert!(get_dispatch(&m, &dispatch_key("other", "d1")).is_some());
    }

    #[test]
    fn dispatch_demands_reach_both_the_trigger_and_the_work_spec() {
        let mut m = module();
        let mut ctx = mk_ctx(0, owner());
        exec(
            &mut m,
            &mut ctx,
            &register(OutputContract::Text, Routing::Rendezvous),
        )
        .unwrap();
        commit(&mut m);

        let demands = BTreeMap::from([("cores".to_string(), 4u64)]);
        let mut caller = mk_ctx(5, Origin::Module("caller".into()));
        exec(
            &mut m,
            &mut caller,
            &DispatchMsg::Dispatch {
                dispatch_id: "d1".into(),
                recipe_id: "summarize".into(),
                payload: b"input".to_vec(),
                demands: demands.clone(),
                admission: AdmissionPolicy::FailFast,
            },
        )
        .unwrap();
        assert_eq!(caller.msgs().len(), 1);
        let SagaMsg::Trigger {
            spec,
            demands: trigger_demands,
            ..
        } = saga_decode_msg(&caller.msgs()[0].payload).unwrap()
        else {
            panic!("expected a trigger");
        };
        // ONE source: the trigger's demands and the work spec's demands agree
        // exactly with what was dispatched — no drift possible.
        assert_eq!(trigger_demands, demands);
        let work = crate::decode_work_spec(&spec).unwrap();
        assert_eq!(work.demands, demands);
        assert_eq!(work.admission, AdmissionPolicy::FailFast);
    }

    #[test]
    fn work_spec_without_demands_field_is_rejected() {
        // FLAG DAY: demands is required — a spec that omits the key fails to
        // decode rather than silently defaulting to a demandless job.
        let no_demands = br#"{"kind":"dispatch-work-v1","dispatch_id":"d","capability":"c","payload":[]}"#;
        assert!(crate::decode_work_spec(no_demands).is_err());
    }

    #[test]
    fn callbacks_judge_contracts_and_enqueue_deliveries() {
        // Json contract, JSON result: Ok flows to the mailbox.
        let mut m = module();
        let key = registered_and_dispatched(&mut m, OutputContract::Json);
        callback_for(&mut m, &key, SagaOutcome::Done(br#"{"ok":true}"#.to_vec())).unwrap();
        commit(&mut m);
        let view = get_dispatch(&m, &key).unwrap();
        assert_eq!(view.status, DispatchStatus::AwaitingDelivery);
        assert_eq!(view.outcome, Some(Ok(br#"{"ok":true}"#.to_vec())));
        assert_eq!(pending_deliveries(&m), 1);

        // Json contract, non-JSON result: the VIOLATION is the outcome.
        let mut m = module();
        let key = registered_and_dispatched(&mut m, OutputContract::Json);
        callback_for(&mut m, &key, SagaOutcome::Done(b"not json".to_vec())).unwrap();
        commit(&mut m);
        let view = get_dispatch(&m, &key).unwrap();
        match view.outcome {
            Some(Err(e)) => assert!(e.contains("output contract violation"), "got {e}"),
            other => panic!("expected a contract violation, got {other:?}"),
        }
        assert_eq!(pending_deliveries(&m), 1, "violations are delivered too");

        // saga failure maps to the Err outcome verbatim.
        let mut m = module();
        let key = registered_and_dispatched(&mut m, OutputContract::Text);
        callback_for(&mut m, &key, SagaOutcome::Failed("provider died".into())).unwrap();
        commit(&mut m);
        assert_eq!(
            get_dispatch(&m, &key).unwrap().outcome,
            Some(Err("provider died".into()))
        );

        // correlation gates: unknown key and stale saga id are no-ops.
        let mut m = module();
        let key = registered_and_dispatched(&mut m, OutputContract::Text);
        let before = m.root();
        callback_for(&mut m, "caller\x1fnope", SagaOutcome::Done(b"x".to_vec())).unwrap();
        let mut ctx = mk_ctx(0, Origin::Module("saga".into()));
        let msg = Msg {
            target: "dispatch".into(),
            payload: encode_callback(&SagaCallback {
                saga_id: "some-other-saga".into(),
                payload: key.as_bytes().to_vec(),
                outcome: SagaOutcome::Done(b"x".to_vec()),
            }),
        };
        block_on(m.execute(&mut ctx, &msg)).unwrap();
        commit(&mut m);
        assert_eq!(m.root(), before, "mismatched callbacks stage nothing");
    }

    #[test]
    fn deliver_pending_is_system_only_bounded_and_fifo() {
        let mut m = module();
        let mut ctx = mk_ctx(0, owner());
        exec(
            &mut m,
            &mut ctx,
            &register(OutputContract::Text, Routing::Rendezvous),
        )
        .unwrap();
        // enqueue MAX + 1 outcomes across two receivers.
        for i in 0..=MAX_DELIVERIES_PER_BLOCK {
            let receiver = if i % 2 == 0 { "even" } else { "odd" };
            let mut caller = mk_ctx(5, Origin::Module(receiver.into()));
            exec(
                &mut m,
                &mut caller,
                &dispatch_op(&format!("d{i:03}"), b"in"),
            )
            .unwrap();
        }
        commit(&mut m);
        for i in 0..=MAX_DELIVERIES_PER_BLOCK {
            let receiver = if i % 2 == 0 { "even" } else { "odd" };
            let key = dispatch_key(receiver, &format!("d{i:03}"));
            callback_for(
                &mut m,
                &key,
                SagaOutcome::Done(format!("r{i}").into_bytes()),
            )
            .unwrap();
        }
        commit(&mut m);
        assert_eq!(
            pending_deliveries(&m) as usize,
            MAX_DELIVERIES_PER_BLOCK + 1
        );

        // non-System origins cannot force a delivery sweep.
        let mut foreign = mk_ctx(0, owner());
        let err = exec(&mut m, &mut foreign, &DispatchMsg::DeliverPending {}).unwrap_err();
        assert!(err.to_string().contains("System-origin"), "got {err}");

        // the System sweep drains FIFO, bounded per block.
        let mut sys = mk_ctx(9, Origin::System);
        exec(&mut m, &mut sys, &DispatchMsg::DeliverPending {}).unwrap();
        commit(&mut m);
        assert_eq!(sys.msgs().len(), MAX_DELIVERIES_PER_BLOCK);
        // FIFO: the first emitted event is the first enqueued dispatch.
        let first = crate::decode_result_event(&sys.msgs()[0].payload).unwrap();
        assert_eq!(first.dispatch_id, "d000");
        assert_eq!(first.outcome, Ok(b"r0".to_vec()));
        assert_eq!(sys.msgs()[0].target, "even");
        assert_eq!(pending_deliveries(&m), 1, "the remainder stays pending");
        assert_eq!(
            get_dispatch(&m, &dispatch_key("even", "d000"))
                .unwrap()
                .status,
            DispatchStatus::Delivered
        );

        // the next sweep drains the remainder.
        let mut sys = mk_ctx(10, Origin::System);
        exec(&mut m, &mut sys, &DispatchMsg::DeliverPending {}).unwrap();
        commit(&mut m);
        assert_eq!(sys.msgs().len(), 1);
        assert_eq!(pending_deliveries(&m), 0);
    }

    #[test]
    fn snapshot_round_trips_and_rejects_wrong_roots() {
        let mut m = module();
        let key = registered_and_dispatched(&mut m, OutputContract::Json);
        callback_for(&mut m, &key, SagaOutcome::Done(br#"[1,2]"#.to_vec())).unwrap();
        commit(&mut m);

        let snap = m.snapshot();
        let root = m.root();
        let mut dst = module();
        dst.install(&snap, root).unwrap();
        assert_eq!(dst.root(), root);
        assert_eq!(dst.snapshot(), snap);
        assert_eq!(pending_deliveries(&dst), 1);

        let mut dst = module();
        assert!(dst.install(&snap, StateRoot::ZERO).is_err());
        assert_eq!(
            dst.root(),
            module().root(),
            "failed install leaves no trace"
        );

        // truncated and trailing bytes are rejected.
        assert!(dst.install(&snap[..snap.len() - 1], root).is_err());
        let mut padded = snap.clone();
        padded.push(0);
        assert!(dst.install(&padded, root).is_err());
    }

    #[test]
    fn admission_policy_does_not_change_committed_snapshot_or_root() {
        fn dispatched(admission: AdmissionPolicy) -> DispatchModule {
            let mut m = module();
            let mut owner_ctx = mk_ctx(0, owner());
            exec(
                &mut m,
                &mut owner_ctx,
                &register(OutputContract::Json, Routing::Rendezvous),
            )
            .unwrap();
            commit(&mut m);
            let mut caller = mk_ctx(5, Origin::Module("caller".into()));
            exec(
                &mut m,
                &mut caller,
                &DispatchMsg::Dispatch {
                    dispatch_id: "d1".into(),
                    recipe_id: "summarize".into(),
                    payload: b"input".to_vec(),
                    demands: BTreeMap::new(),
                    admission,
                },
            )
            .unwrap();
            commit(&mut m);
            m
        }

        let queue = dispatched(AdmissionPolicy::Queue);
        let fail_fast = dispatched(AdmissionPolicy::FailFast);
        assert_eq!(queue.root(), fail_fast.root());
        assert_eq!(queue.snapshot(), fail_fast.snapshot());
    }

    #[test]
    fn abort_discards_every_staged_write() {
        let mut m = module();
        let before = m.root();
        let mut ctx = mk_ctx(0, owner());
        exec(
            &mut m,
            &mut ctx,
            &register(OutputContract::Text, Routing::Rendezvous),
        )
        .unwrap();
        let mut caller = mk_ctx(0, Origin::Module("caller".into()));
        exec(&mut m, &mut caller, &dispatch_op("d1", b"in")).unwrap();
        block_on(m.abort_block()).unwrap();
        assert_eq!(m.root(), before, "aborted block leaves no trace");
        assert_eq!(pending_deliveries(&m), 0);
    }

    #[test]
    fn cancel_is_receiver_scoped_and_idempotent() {
        let mut m = module();
        let key = registered_and_dispatched(&mut m, OutputContract::Text);
        let cancel = DispatchMsg::CancelDispatch {
            dispatch_id: "d1".into(),
        };

        // an external submitter has no cancel surface.
        let mut ctx = mk_ctx(0, owner());
        assert!(exec(&mut m, &mut ctx, &cancel).is_err());

        // a foreign module's cancel lands in its OWN receiver namespace —
        // an unknown key, a deterministic no-op.
        let mut ctx = mk_ctx(0, Origin::Module("other".into()));
        exec(&mut m, &mut ctx, &cancel).unwrap();
        assert!(ctx.msgs().is_empty());

        // the receiver's cancel tells exactly the dispatch's own saga.
        let mut ctx = mk_ctx(0, Origin::Module("caller".into()));
        exec(&mut m, &mut ctx, &cancel).unwrap();
        assert_eq!(ctx.msgs().len(), 1);
        assert_eq!(ctx.msgs()[0].target, "saga");
        match saga_decode_msg(&ctx.msgs()[0].payload).unwrap() {
            SagaMsg::Cancel { saga_id } => assert_eq!(saga_id, saga_id_for(&key)),
            other => panic!("expected a saga Cancel, got {other:?}"),
        }

        // once the Cancelled callback lands, the result flows the NORMAL
        // path (Err in the mailbox) and a repeat cancel is a no-op.
        callback_for(&mut m, &key, SagaOutcome::Cancelled).unwrap();
        commit(&mut m);
        assert_eq!(pending_deliveries(&m), 1);
        let mut ctx = mk_ctx(0, Origin::Module("caller".into()));
        exec(&mut m, &mut ctx, &cancel).unwrap();
        assert!(ctx.msgs().is_empty());
    }

    #[test]
    fn an_undecodable_saga_callback_is_swallowed_not_an_abort() {
        // the callback-poison rule: the callback intake runs inside a
        // finalized block, so a payload that fails to decode must be a
        // staged no-op plus a diagnostic event — an Err would abort the
        // block, which replays as a no-op and re-aborts forever.
        let mut m = module();
        let key = registered_and_dispatched(&mut m, OutputContract::Text);
        let before = m.root();

        let mut ctx = mk_ctx(0, Origin::Module("saga".into()));
        let msg = Msg {
            target: "dispatch".into(),
            payload: b"not a callback".to_vec(),
        };
        block_on(m.execute(&mut ctx, &msg)).expect("the poisoned callback must not abort");
        assert_eq!(ctx.events().len(), 1, "the swallow left a diagnostic event");
        assert!(
            String::from_utf8_lossy(&ctx.events()[0].payload).contains("undecodable"),
            "the event names the drop"
        );

        // the block applies and stages nothing: the dispatch still awaits its
        // result and the root is unmoved.
        commit(&mut m);
        assert_eq!(m.root(), before, "nothing was staged");
        assert!(matches!(
            get_dispatch(&m, &key).unwrap().status,
            DispatchStatus::AwaitingResult { .. }
        ));
    }

    #[test]
    fn an_orphaned_mailbox_entry_is_dropped_not_an_abort() {
        // the never-pop-stack rule's failure mode: a committed non-empty
        // mailbox is re-injected every block, so an entry the sweep cannot
        // deliver must be dropped (with an event), never an Err — an abort
        // here would poison every future block.
        let mut m = module();
        // an orphan cannot be built through the module's own transitions;
        // plant it directly in committed state (same crate, test-only).
        m.committed.mailbox.insert(0, "ghost\x1fentry".into());
        m.committed.next_seq = 1;
        assert_eq!(pending_deliveries(&m), 1);

        let mut sys = mk_ctx(0, Origin::System);
        exec(&mut m, &mut sys, &DispatchMsg::DeliverPending {})
            .expect("the orphan must not abort the delivery block");
        assert!(sys.msgs().is_empty(), "no delivery was invented for it");
        assert_eq!(sys.events().len(), 1, "the drop left a diagnostic event");
        assert!(
            String::from_utf8_lossy(&sys.events()[0].payload).contains("orphaned"),
            "the event names the orphan"
        );

        // the block applies and the mailbox drains — no re-injection loop.
        commit(&mut m);
        assert_eq!(pending_deliveries(&m), 0, "the orphan seq was dropped");
    }

    #[test]
    fn reassign_is_receiver_scoped_and_carries_the_expected_attempt() {
        let mut m = module();
        let key = registered_and_dispatched(&mut m, OutputContract::Text);
        let reassign = DispatchMsg::ReassignDispatch {
            dispatch_id: "d1".into(),
            attempt: 2,
        };
        let mut ctx = mk_ctx(0, Origin::Module("caller".into()));
        exec(&mut m, &mut ctx, &reassign).unwrap();
        assert_eq!(ctx.msgs().len(), 1);
        assert!(matches!(
            saga_decode_msg(&ctx.msgs()[0].payload).unwrap(),
            SagaMsg::Reassign { saga_id, attempt: 2 } if saga_id == saga_id_for(&key)
        ));
    }

}

// the wasm-guest port: the dispatch shell that adapts this module to the
// ducktape:module world. compiled only by the guest-builder's synthesized
// wasm32 cdylib workspace (feature `guest`), never by the native build.
#[cfg(feature = "guest")]
mod guest;
