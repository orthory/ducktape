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
//! ## qmdb-backed
//!
//! the plane is pure logic over a host-injected [`sdk::MerkleStore`] with the
//! shared [`StagedStore`] overlay in front of it. every recipe, every dispatch
//! and every mailbox entry is its OWN store key (see `records`), so an op
//! touches only the keys it names, `root()` is the store's cached merkle root,
//! and state-sync rides the store's resolver lane rather than a byte snapshot
//! whose preimage was re-serialized on every single `root()` call.
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
//! ## retention
//!
//! the RECORD is permanent, the PAYLOAD is not. a dispatch record is the
//! network's turn-claim key — `runs` asks "does this dispatch id exist?" to
//! refuse a second run for a `run_id` it already ran, long after its own
//! pending entry is gone — so a record is never evicted, ever. what is
//! unbounded is the outcome: up to [`MAX_RESULT_BYTES`] per dispatch. delivery
//! therefore hands the bytes to the receiver and DROPS this module's copy, in
//! the same transition, leaving a fixed-size receipt. state then grows with the
//! number of dispatches, not with the size of their results — and because each
//! dispatch is its own record, that permanent count costs nothing per op.
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

// the store key space and the per-record codecs.
mod records;

use std::collections::BTreeMap;

use capability::{validate_resources, validate_tag};
use records::{
    Mailbox, committed_dispatch, committed_mailbox, committed_recipe, dispatch_key_of,
    encode_dispatch, encode_recipe, mailbox_key, recipe_key, stage_mailbox, staged_dispatch,
    staged_mailbox, staged_recipe,
};
use saga::{
    SagaCallback, SagaMsg, SagaOrigin, SagaOutcome, decode_callback, encode_msg as saga_encode_msg,
};
use sdk::{
    Ctx, Error, Event, MerkleStore, Module, ModuleId, Msg, Origin, ResolverSyncTarget, StagedStore,
    StateRoot, StateSyncHandle,
};

/// the field separator inside composite dispatch keys and saga ids (the shared
/// [`sdk::KEY_SEP`]). rejected inside caller-chosen ids by [`sdk::validate_id`]
/// so a crafted id can never forge another receiver's key.
const SEP: char = sdk::KEY_SEP;

/// write-time cap on ONE stored record. the concrete store's codec bounds a
/// stored value at 1 MiB AT DECODE TIME (`statesync::qmdb::store_config`): an
/// oversized value would COMMIT fine and then panic every later read on every
/// validator — a poison pill. the 4 KiB margin below the codec bound covers the
/// serialized operation's framing (32-byte hashed key, varint length prefix,
/// operation tag), exactly as `kv::MAX_VALUE_LEN` reasons.
///
/// this is the ONE guard the storage swap adds, and exactly one field made it
/// reachable: `Routing::Pinned(key)` was checked for non-emptiness and nothing
/// else, so a ~1 MiB pin off a single op frame (`node::MAX_FRAME_BYTES` is
/// 1 MiB + 16 KiB) would have poisoned that recipe's record. every other field
/// is already capped upstream — ids by [`MAX_ID_BYTES`], `description` by
/// [`MAX_DESCRIPTION_BYTES`], `capability` by `capability::MAX_TAG_LEN`, and an
/// outcome by saga's own `MAX_RESULT_BYTES` / `MAX_ERROR_BYTES`.
pub const MAX_RECORD_BYTES: usize = (1 << 20) - 4 * 1024;

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

/// refuse a value the store's codec would later panic decoding. `what` names
/// the record in the rejection. an op that writes SEVERAL records checks them
/// all before staging any, so a refused op leaves no overlay entry at all.
///
/// that ordering — CHECK everything, THEN stage everything — is a root
/// invariant, not a style preference. natively this module keeps `staged`
/// across every dispatch in a block; the wasm guest rebuilds the module per
/// dispatch and flushes its overlay only on a SUCCESSFUL execute. so a path
/// that stages a write and then returns `Err` leaves residue on one side and
/// none on the other, and the two ports diverge on the root. no path does that
/// today (every transition checks first and stages last); keep it that way when
/// adding one.
fn check_record(value: &[u8], what: &str) -> Result<(), Error> {
    if value.len() > MAX_RECORD_BYTES {
        return Err(Error::Module(format!(
            "{what} is {} bytes, over the {MAX_RECORD_BYTES}-byte store record cap",
            value.len()
        )));
    }
    Ok(())
}

/// [`check_record`] then stage — the single-record writer's shape.
fn stage_record(
    staged: &mut StagedStore,
    key: Vec<u8>,
    value: Vec<u8>,
    what: &str,
) -> Result<(), Error> {
    check_record(&value, what)?;
    staged.stage(key, value);
    Ok(())
}

// ---- state ---------------------------------------------------------------------

/// one dispatch's committed state. the output contract is CAPTURED at
/// dispatch time, so removing or retuning the recipe never changes how an
/// in-flight run's result is judged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DispatchState {
    pub(crate) receiver: ModuleId,
    pub(crate) dispatch_id: String,
    pub(crate) recipe_id: String,
    pub(crate) contract: OutputContract,
    pub(crate) saga_id: String,
    pub(crate) status: Status,
    pub(crate) outcome: Option<Result<Vec<u8>, String>>,
    pub(crate) created_at: u64,
    pub(crate) updated_at: u64,
}

/// the stored lifecycle discriminant (the wire shape carries `saga_id` inside
/// the variant; state keeps it as its own field).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Status {
    AwaitingResult,
    AwaitingDelivery,
    Delivered,
}

// ---- the module -----------------------------------------------------------------

pub struct DispatchModule {
    id: ModuleId,
    /// the saga module dispatches trigger through — genesis config, not state.
    saga: ModuleId,
    /// the host-injected authenticated store plus this block's staging overlay
    /// (read-your-writes; folded into `root()` at `commit_block`).
    staged: StagedStore,
}

impl DispatchModule {
    /// wrap the host-constructed store under module identity `id`. sync — the
    /// store arrives already opened (or already synced to a verified root).
    pub fn new(
        id: impl Into<ModuleId>,
        saga: impl Into<ModuleId>,
        store: Box<dyn MerkleStore>,
    ) -> Self {
        Self {
            id: id.into(),
            saga: saga.into(),
            staged: StagedStore::new(store),
        }
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

    async fn owned_recipe(&self, ctx: &dyn Ctx, recipe_id: &str) -> Result<Recipe, Error> {
        let recipe = staged_recipe(&self.staged, recipe_id)
            .await?
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
    async fn on_saga_callback(&mut self, ctx: &mut dyn Ctx, payload: &[u8]) -> Result<(), Error> {
        let callback: SagaCallback = match decode_callback(payload) {
            Ok(callback) => callback,
            Err(e) => {
                self.note(ctx, format!("dropped undecodable saga callback: {e}"));
                return Ok(());
            }
        };
        let key = String::from_utf8_lossy(&callback.payload).into_owned();
        let Some(mut dispatch) = staged_dispatch(&self.staged, &key).await? else {
            return Ok(());
        };
        if dispatch.status != Status::AwaitingResult || dispatch.saga_id != callback.saga_id {
            return Ok(());
        }
        dispatch.outcome = Some(Self::judged_outcome(dispatch.contract, callback.outcome));
        dispatch.status = Status::AwaitingDelivery;
        dispatch.updated_at = ctx.env().consensus_time;

        let record = encode_dispatch(&dispatch);
        // saga bounds both outcome shapes (MAX_RESULT_BYTES / MAX_ERROR_BYTES)
        // far below the store's record cap, so this cannot fire. if those caps
        // ever drift, DROP the result rather than return `Err`: this arm runs
        // inside a finalized block, where an error is a permanent abort loop.
        if let Err(e) = check_record(&record, "dispatch record") {
            self.note(
                ctx,
                format!("dropped oversized dispatch record {key:?}: {e}"),
            );
            return Ok(());
        }
        let mailbox = staged_mailbox(&self.staged).await?;
        self.staged.stage(dispatch_key_of(&key), record);
        self.staged
            .stage(mailbox_key(mailbox.next), key.into_bytes());
        stage_mailbox(
            &mut self.staged,
            Mailbox {
                head: mailbox.head,
                next: mailbox.next + 1,
            },
        );
        Ok(())
    }

    async fn on_dispatch(
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
        let recipe = staged_recipe(&self.staged, &recipe_id)
            .await?
            .ok_or_else(|| Error::Module(format!("unknown recipe {recipe_id:?}")))?;
        let key = dispatch_key(&receiver, &dispatch_id);
        // the receiver's idempotency: the first dispatch under a key wins,
        // a duplicate is a deterministic no-op (mirrors the saga trigger).
        if staged_dispatch(&self.staged, &key).await?.is_some() {
            return Ok(());
        }
        let env = ctx.env();
        let now = env.consensus_time;
        let height = env.height;
        let saga_id = saga_id_for(&key);
        let (capability, pinned_assignee) = match &recipe.routing {
            Routing::Rendezvous => (Some(recipe.capability.clone()), None),
            Routing::Pinned(node) => (Some(recipe.capability.clone()), Some(node.clone())),
        };
        // CHECK before any effect: the record is encoded and bounded first, so
        // a refusal emits no trigger and stages nothing.
        let record = encode_dispatch(&DispatchState {
            receiver: receiver.clone(),
            dispatch_id: dispatch_id.clone(),
            recipe_id,
            contract: recipe.output_contract,
            saga_id: saga_id.clone(),
            status: Status::AwaitingResult,
            outcome: None,
            created_at: now,
            updated_at: now,
        });
        check_record(&record, "dispatch record")?;
        ctx.emit_msg(Msg {
            target: self.saga.clone(),
            payload: saga_encode_msg(&SagaMsg::Trigger {
                saga_id,
                spec: encode_work_spec(&WorkSpec {
                    kind: WORK_SPEC_KIND.into(),
                    dispatch_id,
                    capability: recipe.capability.clone(),
                    payload,
                    demands: demands.clone(),
                    admission,
                }),
                reply_to: Some(self.id.clone()),
                reply_payload: key.clone().into_bytes(),
                deadline: recipe.deadline_views.map(|v| height.saturating_add(v)),
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
        self.staged.stage(dispatch_key_of(&key), record);
        Ok(())
    }

    /// receiver-scoped cancellation: the emitting module may cancel its own
    /// in-flight dispatch. only the saga is told — the terminal `Cancelled`
    /// callback then drives the normal AwaitingResult → AwaitingDelivery →
    /// delivery path, so there is exactly one result state machine.
    async fn on_cancel(&mut self, ctx: &mut dyn Ctx, dispatch_id: String) -> Result<(), Error> {
        let Origin::Module(receiver) = &ctx.env().origin else {
            return Err(Error::Module(
                "CancelDispatch is module-origin only (a module cancels its own dispatch)".into(),
            ));
        };
        let key = dispatch_key(receiver, &dispatch_id);
        // unknown or already-terminal: a deterministic no-op, so double
        // cancels and cancel/result races are all safe.
        let Some(dispatch) = staged_dispatch(&self.staged, &key).await? else {
            return Ok(());
        };
        if dispatch.status != Status::AwaitingResult {
            return Ok(());
        }
        ctx.emit_msg(Msg {
            target: self.saga.clone(),
            payload: saga_encode_msg(&SagaMsg::Cancel {
                saga_id: dispatch.saga_id,
            }),
        });
        Ok(())
    }

    async fn on_reassign(
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
        let Some(dispatch) = staged_dispatch(&self.staged, &key).await? else {
            return Ok(());
        };
        if dispatch.status != Status::AwaitingResult {
            return Ok(());
        }
        ctx.emit_msg(Msg {
            target: self.saga.clone(),
            payload: saga_encode_msg(&SagaMsg::Reassign {
                saga_id: dispatch.saga_id,
                attempt,
            }),
        });
        Ok(())
    }

    /// the host-injected delivery sweep: emit up to
    /// [`MAX_DELIVERIES_PER_BLOCK`] mailbox events, FIFO, each as one
    /// follow-up `Msg` to its receiver.
    ///
    /// the queue is the contiguous `head..next` range, so the batch is a plain
    /// seq walk: every arm below removes its entry, and `head` lands exactly on
    /// the first seq the sweep did not reach.
    async fn on_deliver_pending(&mut self, ctx: &mut dyn Ctx) -> Result<(), Error> {
        if !matches!(ctx.env().origin, Origin::System) {
            return Err(Error::Module(
                "DeliverPending is System-origin only (host-injected)".into(),
            ));
        }
        let now = ctx.env().consensus_time;
        let mailbox = staged_mailbox(&self.staged).await?;
        let end = mailbox
            .head
            .saturating_add(MAX_DELIVERIES_PER_BLOCK as u64)
            .min(mailbox.next);
        // an EMPTY mailbox is the common case (the host injects a sweep on the
        // block after every callback), and it must stage NOTHING: staging a
        // delete of an already-absent cursor key still appends an operation the
        // store hashes, so an idle sweep would move the root every block.
        if end == mailbox.head {
            return Ok(());
        }
        for seq in mailbox.head..end {
            let entry_key = mailbox_key(seq);
            // a mailbox entry without its record (or without a recorded
            // outcome) cannot be built by this module's own transitions — but
            // this arm is host-injected every block while the mailbox is
            // non-empty, so erroring here would abort every future block (the
            // poison loop the module header forbids). drop the orphan and leave
            // a diagnostic event instead.
            let Some(raw) = self.staged.get(&entry_key).await? else {
                self.note(ctx, format!("dropped empty mailbox slot {seq}"));
                continue;
            };
            self.staged.delete(entry_key);
            let key = String::from_utf8_lossy(&raw).into_owned();
            let Some(mut dispatch) = staged_dispatch(&self.staged, &key).await? else {
                self.note(ctx, format!("dropped orphaned mailbox entry {key:?}"));
                continue;
            };
            // TAKE, never clone: the receiver now owns the bytes, and a second
            // copy here would grow this record forever (crate header,
            // "retention"). the RECORD stays — it is `runs`' permanent turn
            // claim.
            let Some(outcome) = dispatch.outcome.take() else {
                self.note(
                    ctx,
                    format!("dropped mailbox entry {key:?} with no recorded outcome"),
                );
                continue;
            };
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
            // strictly smaller than the record that already passed the cap.
            self.staged
                .stage(dispatch_key_of(&key), encode_dispatch(&dispatch));
        }
        stage_mailbox(
            &mut self.staged,
            Mailbox {
                head: end,
                next: mailbox.next,
            },
        );
        Ok(())
    }

    async fn on_register_recipe(
        &mut self,
        ctx: &mut dyn Ctx,
        recipe_id: String,
        recipe: RecipeFields,
    ) -> Result<(), Error> {
        sdk::validate_id("recipe_id", &recipe_id, MAX_ID_BYTES)?;
        Self::validate_recipe_shape(
            &recipe.capability,
            &recipe.routing,
            recipe.max_attempts,
            &recipe.description,
        )?;
        if staged_recipe(&self.staged, &recipe_id).await?.is_some() {
            return Err(Error::Module(format!(
                "recipe {recipe_id:?} already exists"
            )));
        }
        let now = ctx.env().consensus_time;
        let record = encode_recipe(&Recipe {
            recipe_id: recipe_id.clone(),
            owner: Self::acting_origin(&ctx.env().origin)?,
            description: recipe.description,
            capability: recipe.capability,
            routing: recipe.routing,
            output_contract: recipe.output_contract,
            max_attempts: recipe.max_attempts,
            deadline_views: recipe.deadline_views,
            lease_views: recipe.lease_views,
            created_at: now,
            updated_at: now,
        });
        stage_record(
            &mut self.staged,
            recipe_key(&recipe_id),
            record,
            "recipe record",
        )
    }

    async fn on_remove_recipe(&mut self, ctx: &dyn Ctx, recipe_id: String) -> Result<(), Error> {
        self.owned_recipe(ctx, &recipe_id).await?;
        // the record is the recipe's whole footprint, so removing every recipe
        // returns the plane to the root it had before any registration.
        self.staged.delete(recipe_key(&recipe_id));
        Ok(())
    }

    async fn on_admin(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
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
                self.on_register_recipe(
                    ctx,
                    recipe_id,
                    RecipeFields {
                        description,
                        capability,
                        routing,
                        output_contract,
                        max_attempts,
                        deadline_views,
                        lease_views,
                    },
                )
                .await
            }
            DispatchMsg::UpdateRecipe {
                recipe_id,
                description,
                capability,
                routing,
                output_contract,
                max_attempts,
            } => {
                let mut recipe = self.owned_recipe(ctx, &recipe_id).await?;
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
                stage_record(
                    &mut self.staged,
                    recipe_key(&recipe_id),
                    encode_recipe(&recipe),
                    "recipe record",
                )
            }
            DispatchMsg::RemoveRecipe { recipe_id } => self.on_remove_recipe(ctx, recipe_id).await,
            DispatchMsg::Dispatch {
                dispatch_id,
                recipe_id,
                payload,
                demands,
                admission,
            } => {
                self.on_dispatch(ctx, dispatch_id, recipe_id, payload, demands, admission)
                    .await
            }
            DispatchMsg::CancelDispatch { dispatch_id } => self.on_cancel(ctx, dispatch_id).await,
            DispatchMsg::ReassignDispatch {
                dispatch_id,
                attempt,
            } => self.on_reassign(ctx, dispatch_id, attempt).await,
            DispatchMsg::DeliverPending {} => self.on_deliver_pending(ctx).await,
            // the block's EXISTENCE is the point (it carries the host's
            // delivery injection); the op itself stages nothing.
            DispatchMsg::Nudge {} => Ok(()),
        }
    }

    fn view(d: DispatchState) -> DispatchView {
        DispatchView {
            dispatch_id: d.dispatch_id,
            recipe_id: d.recipe_id,
            receiver: d.receiver,
            status: match d.status {
                Status::AwaitingResult => DispatchStatus::AwaitingResult { saga_id: d.saga_id },
                Status::AwaitingDelivery => DispatchStatus::AwaitingDelivery,
                Status::Delivered => DispatchStatus::Delivered,
            },
            outcome: d.outcome,
            created_at: d.created_at,
            updated_at: d.updated_at,
        }
    }
}

/// the [`DispatchMsg::RegisterRecipe`] payload minus its id — one named value
/// so the registration handler takes two arguments instead of eight.
struct RecipeFields {
    description: String,
    capability: String,
    routing: Routing,
    output_contract: OutputContract,
    max_attempts: u32,
    deadline_views: Option<u64>,
    lease_views: Option<u64>,
}

#[async_trait::async_trait(?Send)]
impl Module for DispatchModule {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// the REAL merkle root over all committed records, cached by the store —
    /// never a re-serialization of the plane.
    fn root(&self) -> StateRoot {
        self.staged.root()
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        self.staged.state_sync_handle()
    }

    /// the network state-sync serve lane: answers the shared qmdb wire requests
    /// (historical proof-carrying op ranges) from committed state. read-only.
    async fn serve_sync(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        self.staged.serve_sync(req).await
    }

    async fn resolver_sync_target(&self) -> Result<ResolverSyncTarget, Error> {
        self.staged.sync_target().await
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        // route by origin: the saga module sends exactly one payload family
        // (terminal callbacks); everything else is the admin/dispatch surface.
        match &ctx.env().origin {
            Origin::Module(m) if *m == self.saga => {
                let payload = msg.payload.clone();
                self.on_saga_callback(ctx, &payload).await
            }
            _ => self.on_admin(ctx, msg).await,
        }
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        // COMMITTED state only: the host's delivery injection reads
        // PendingDeliveries between blocks, and a staged overlay must never
        // leak into that decision.
        match decode_query(req).map_err(Error::Module)? {
            DispatchQuery::Recipe { recipe_id } => Ok(encode_reply(&DispatchReply::Recipe(
                committed_recipe(&self.staged, &recipe_id).await?,
            ))),
            DispatchQuery::Dispatch {
                receiver,
                dispatch_id,
            } => Ok(encode_reply(&DispatchReply::Dispatch(
                committed_dispatch(&self.staged, &dispatch_key(&receiver, &dispatch_id))
                    .await?
                    .map(Self::view),
            ))),
            DispatchQuery::PendingDeliveries => Ok(encode_reply(
                &DispatchReply::PendingDeliveries(committed_mailbox(&self.staged).await?.len()),
            )),
        }
    }

    /// publish the block's staged writes AND deletes in ONE store batch.
    async fn commit_block(&mut self) -> Result<(), Error> {
        self.staged.commit().await
    }

    /// discard the block's staged writes — nothing reached the store, so
    /// `root()` is unchanged.
    async fn abort_block(&mut self) -> Result<(), Error> {
        self.staged.abort();
        Ok(())
    }
}

#[cfg(test)]
mod tests;

// the wasm-guest port: the dispatch shell that adapts this module to the
// ducktape:module world. compiled only by the guest-builder's synthesized
// wasm32 cdylib workspace (feature `guest`), never by the native build.
#[cfg(feature = "guest")]
mod guest;
