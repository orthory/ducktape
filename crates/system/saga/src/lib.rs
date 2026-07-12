//! the saga ledger — the DETERMINISTIC half of the async engine.
//!
//! a pure state-machine module (in the app-hash) that records async work in
//! flight: one effect, one agreed result, domain-agnostic. it keeps
//! `directory`'s shape — an in-memory `BTreeMap` with a `pending` overlay
//! staged during a block and merged at the boundary, and a state-based
//! `root()` — and implements three of the platform's ordering-contract
//! promises (docs/records/architecture/agent-collaboration-design.md §2, §4):
//!
//! - **P5 — result singularity.** exactly one `OracleResult` transitions a
//!   given attempt: the `(saga_id, attempt)` pair is the idempotency key, so
//!   duplicate results, results for terminal sagas, and stale-attempt results
//!   are all deterministic no-ops. an `Err` outcome consumes the attempt and
//!   re-leases while attempts remain; the last one lands `Failed`.
//! - **P6 — callback adjacency.** EVERY terminal transition (`Done`,
//!   `Failed`, `TimedOut`, `Cancelled`) with a `reply_to` emits a
//!   `SagaCallback` msg to the requester, which the host drains as a
//!   follow-up in the SAME block — the requester learns the outcome in the
//!   block the result lands, atomically with it.
//! - **P7 — deterministic deadlines.** expiry is never node-local: a
//!   permissionless `Crank` op sweeps pending sagas in id order (bounded by
//!   [`CRANK_BUDGET`]) and fires past-deadline timeouts and expired leases
//!   against the agreed `consensus_time` (a view number). given the same op
//!   sequence, every validator times out identically; liveness comes from
//!   anyone cranking, safety never depends on who does.
//!
//! ## the callback-poison rule (design §4)
//!
//! the terminal transition and the requester callback commit in one block; a
//! callback that ERRORS aborts that finalized block, which replays as a
//! deterministic no-op — wedging the saga at `Pending` forever. two defenses:
//! `reply_to` is validated against `ctx.module_root` at trigger time (an
//! unknown or self-targeting callback is rejected before a saga exists), and
//! requester callback arms MUST be no-fail by construction — treat a decode
//! failure as a staged no-op plus an event, never an `Err`.
//!
//! ## leases
//!
//! [`SagaModule::new`] runs [`LeasePolicy::Open`]: no assignee, any
//! submitter's result accepted (first agreed one wins), lease windows still
//! tracked when the trigger asks (so `Crank` can retry a silent worker).
//! [`SagaModule::with_assignment`] additionally rendezvous-assigns each
//! attempt to `pool[H(saga_id ‖ attempt ‖ height) % n]` over the valset
//! module's membership, with a capability registry on the side: a trigger
//! that names a capability then draws its pool from that tag's
//! ANNOUNCED PROVIDERS instead — only nodes that can execute the work ever
//! hold its lease, and a tag nobody provides assigns nobody (never the raw
//! valset). under [`LeasePolicy::Strict`] a result is accepted only from the
//! assignee's external origin. when the pool is empty or unavailable the
//! assignee is `None` and the emitted [`WorkerRequest`] is an ANNOUNCEMENT:
//! no result can land for it under strict — a capable node claims it with
//! `Accept` (first in consensus order wins the lease, and the re-emitted
//! request names the winner), so N capable nodes never each pay for the
//! same execution.
//!
//! ## GC
//!
//! retention is explicit: `Prune` removes terminal sagas, gated to the
//! recorded trigger origin per id. there is no lazy retention sweep — a
//! terminal saga stays in the root preimage until its owner prunes it.
//!
//! `root()` folds in every field, so any transition moves the app-hash. a
//! joiner rebuilds this module from a peer via [`SagaModule::snapshot`] /
//! [`SagaModule::install`]: the snapshot ships the committed map in the exact
//! canonical encoding `root()` hashes, and install re-derives the root from
//! the decoded temporaries before adopting them — the consensus-agreed root,
//! not the peer, is the trust anchor.

// the wire surface: this module's shared types, flattened at the crate root.
mod interface;
pub use interface::*;

// the usage ledger: a node-local derived index over this module's op stream.
pub mod index;

use std::collections::{BTreeMap, BTreeSet};

use capability::{
    CapabilityQuery, CapabilityReply, decode_reply as capability_decode_reply,
    encode_query as capability_encode_query,
};
use sdk::codec;
use sdk::{Ctx, Effect, Error, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};
use sha2::{Digest, Sha256};
use valset::{
    ValsetQuery, ValsetReply, decode_reply as valset_decode_reply,
    encode_query as valset_encode_query,
};

/// hard cap on state transitions per `Crank` op — a consensus constant, so a
/// backlog of expired sagas is worked off in deterministic, bounded slices.
pub const CRANK_BUDGET: u32 = 32;

/// lease window (in views) granted when a trigger leaves `lease_views` unset
/// but an assignee exists — an assigned attempt must always be reclaimable.
pub const DEFAULT_LEASE_VIEWS: u64 = 64;

/// who may complete an assigned attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeasePolicy {
    /// the assignee is advisory: any submitter's result is accepted (the
    /// first agreed one wins). the honest default until frames are
    /// signature-verified.
    Open,
    /// a result is accepted only from the assignee's external origin; a
    /// non-assignee result is a deterministic no-op. an attempt whose
    /// assignee is `None` (empty/unavailable set) accepts NO result until a
    /// node claims it via `Accept` — the announcement lane.
    Strict,
}

/// one tracked saga. the id is the map key, so it isn't repeated here.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Saga {
    /// the trigger's origin — the cancel/prune capability.
    origin: SagaOrigin,
    /// callback target, validated at trigger time (callback-poison rule).
    reply_to: Option<ModuleId>,
    /// opaque requester correlation, echoed back in the callback.
    reply_payload: Vec<u8>,
    /// opaque work spec, echoed to the worker on every attempt.
    spec: Vec<u8>,
    /// the capability the work requires, when the trigger named one: each
    /// attempt is then rendezvous-assigned over the tag's announced providers
    /// instead of the raw validator set. opaque to this module.
    capability: Option<String>,
    status: SagaStatus,
    /// the current attempt (0-based); the half of the idempotency key that
    /// makes retried work distinguishable from stale results.
    attempt: u32,
    /// total attempts allowed (>= 1).
    max_attempts: u32,
    /// the current attempt's lease holder, if assignment is configured.
    assignee: Option<Vec<u8>>,
    /// the trigger's static binding: when set, every attempt's assignee IS
    /// this key — no pool query, no rendezvous.
    pinned_assignee: Option<Vec<u8>>,
    /// the trigger's requested lease window in views, echoed onto every
    /// retry so re-leases reproduce the original grant deterministically.
    lease_views: Option<u64>,
    /// absolute view at which the current lease expires.
    lease_expires_at: Option<u64>,
    /// absolute view bounding the WHOLE saga.
    deadline: Option<u64>,
    /// the agreed oracle output, once `Done`.
    result: Option<Vec<u8>>,
    /// the final failure, once `Failed`.
    error: Option<String>,
    created_at: u64,
    updated_at: u64,
}

/// canonical byte encoding of a committed saga map: u64-le count, then per
/// saga in sorted-id order every field in declaration order — u64-le length
/// prefixes for byte strings, single-byte discriminants for enums, a 0/1 tag
/// byte for options, u32/u64-le for integers. this is the exact preimage
/// [`Module::root`] hashes, so a snapshot and the root that must authenticate
/// it cannot drift.
fn encode_committed(sagas: &BTreeMap<String, Saga>) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(sagas.len() as u64).to_le_bytes());
    for (id, s) in sagas {
        codec::push_str(&mut out, id);
        put_origin(&mut out, &s.origin);
        codec::push_opt_str(&mut out, s.reply_to.as_deref());
        codec::push_bytes(&mut out, &s.reply_payload);
        codec::push_bytes(&mut out, &s.spec);
        codec::push_opt_str(&mut out, s.capability.as_deref());
        out.push(match s.status {
            SagaStatus::Pending => 0,
            SagaStatus::Done => 1,
            SagaStatus::Failed => 2,
            SagaStatus::TimedOut => 3,
            SagaStatus::Cancelled => 4,
        });
        out.extend_from_slice(&s.attempt.to_le_bytes());
        out.extend_from_slice(&s.max_attempts.to_le_bytes());
        codec::push_opt_bytes(&mut out, s.assignee.as_deref());
        codec::push_opt_bytes(&mut out, s.pinned_assignee.as_deref());
        codec::push_opt_u64(&mut out, s.lease_views);
        codec::push_opt_u64(&mut out, s.lease_expires_at);
        codec::push_opt_u64(&mut out, s.deadline);
        codec::push_opt_bytes(&mut out, s.result.as_deref());
        codec::push_opt_str(&mut out, s.error.as_deref());
        out.extend_from_slice(&s.created_at.to_le_bytes());
        out.extend_from_slice(&s.updated_at.to_le_bytes());
    }
    out
}

/// the state-based commitment over a committed saga map — shared by `root()`
/// and `install()` so the verification a snapshot must pass is definitionally
/// the same algorithm the live module answers with.
fn committed_root(sagas: &BTreeMap<String, Saga>) -> StateRoot {
    StateRoot(Sha256::digest(encode_committed(sagas)).into())
}

/// an optional utf-8 string in the plain option layout — [`codec::Cursor`]
/// keeps only the byte primitive; the utf-8 check layers here (opt_str's
/// non-empty/bounded rules would reject valid states, e.g. an empty stored
/// error string).
fn take_opt_string(cur: &mut codec::Cursor, what: &str) -> Result<Option<String>, Error> {
    cur.opt_bytes(what)?
        .map(|raw| {
            std::str::from_utf8(raw)
                .map(str::to_string)
                .map_err(|e| Error::Module(format!("{what} is not utf-8: {e}")))
        })
        .transpose()
}

/// strict decode of an [`encode_committed`] snapshot. the input is UNTRUSTED —
/// it arrives from an arbitrary peer — so every count and length is bounded by
/// the remaining input before allocation, ids must be strictly ascending (one
/// byte encoding per state, and uniqueness for free), unknown discriminants
/// and option tags are rejected, and trailing bytes are rejected. never panics
/// on malformed input.
fn decode_committed(buf: &[u8]) -> Result<BTreeMap<String, Saga>, Error> {
    let mut cur = codec::Cursor::new(buf);
    let count = cur.u64("snapshot saga count")?;
    // every saga costs at least its fixed-width fields — the id length prefix,
    // one origin discriminant, nine option tags, three length prefixes,
    // status, two u32s, and two u64s — so a count the input cannot possibly
    // hold is rejected before the loop builds anything.
    const MIN_SAGA_BYTES: u64 =
        8 + 1 + 1 + 8 + 8 + 1 + 1 + 4 + 4 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 8 + 8;
    cur.bound(count, MIN_SAGA_BYTES, "snapshot saga")?;
    let mut sagas: BTreeMap<String, Saga> = BTreeMap::new();
    for _ in 0..count {
        let id = cur.string("snapshot saga id")?;
        if let Some((last, _)) = sagas.iter().next_back()
            && last.as_str() >= id.as_str()
        {
            return Err(Error::Module(
                "snapshot saga ids not strictly ascending".into(),
            ));
        }
        let origin = take_origin(&mut cur)?;
        let reply_to = take_opt_string(&mut cur, "snapshot reply_to")?;
        let reply_payload = cur.bytes("snapshot reply_payload")?.to_vec();
        let spec = cur.bytes("snapshot spec")?.to_vec();
        let capability = take_opt_string(&mut cur, "snapshot capability")?;
        let status = match cur.byte("snapshot status")? {
            0 => SagaStatus::Pending,
            1 => SagaStatus::Done,
            2 => SagaStatus::Failed,
            3 => SagaStatus::TimedOut,
            4 => SagaStatus::Cancelled,
            d => {
                return Err(Error::Module(format!(
                    "snapshot has unknown status discriminant {d}"
                )));
            }
        };
        let attempt = cur.u32("snapshot attempt")?;
        let max_attempts = cur.u32("snapshot max_attempts")?;
        let assignee = cur.opt_bytes("snapshot assignee")?.map(<[u8]>::to_vec);
        let pinned_assignee = cur
            .opt_bytes("snapshot pinned_assignee")?
            .map(<[u8]>::to_vec);
        let lease_views = cur.opt_u64("snapshot lease_views")?;
        let lease_expires_at = cur.opt_u64("snapshot lease_expires_at")?;
        let deadline = cur.opt_u64("snapshot deadline")?;
        let result = cur.opt_bytes("snapshot result")?.map(<[u8]>::to_vec);
        let error = take_opt_string(&mut cur, "snapshot error")?;
        let created_at = cur.u64("snapshot created_at")?;
        let updated_at = cur.u64("snapshot updated_at")?;
        sagas.insert(
            id,
            Saga {
                origin,
                reply_to,
                reply_payload,
                spec,
                capability,
                status,
                attempt,
                max_attempts,
                assignee,
                pinned_assignee,
                lease_views,
                lease_expires_at,
                deadline,
                result,
                error,
                created_at,
                updated_at,
            },
        );
    }
    cur.finish("snapshot")?;
    Ok(sagas)
}

/// the canonical state form of a dispatch origin (see [`SagaOrigin`]).
fn saga_origin(origin: &Origin) -> SagaOrigin {
    match origin {
        Origin::External(key) => SagaOrigin::External(key.clone()),
        Origin::Module(module) => SagaOrigin::Module(module.clone()),
        Origin::System => SagaOrigin::System,
    }
}

/// the absolute view a lease granted now expires at: an explicit window wins,
/// an assigned attempt without one gets [`DEFAULT_LEASE_VIEWS`], and an
/// unassigned attempt without one carries no lease at all.
fn lease_expiry(height: u64, assignee: &Option<Vec<u8>>, lease_views: Option<u64>) -> Option<u64> {
    match (assignee, lease_views) {
        (_, Some(views)) => Some(height.saturating_add(views)),
        (Some(_), None) => Some(height.saturating_add(DEFAULT_LEASE_VIEWS)),
        (None, None) => None,
    }
}

fn bounded_lease_expiry(
    height: u64,
    assignee: &Option<Vec<u8>>,
    lease_views: Option<u64>,
    deadline: Option<u64>,
) -> Option<u64> {
    lease_expiry(height, assignee, lease_views)
        .map(|expiry| deadline.map_or(expiry, |deadline| expiry.min(deadline)))
}

pub struct SagaModule {
    id: ModuleId,
    /// the valset module rendezvous assignment queries — `None` disables
    /// assignment entirely. genesis config, not state.
    valset: Option<ModuleId>,
    /// the capability registry consulted when a trigger names a capability:
    /// assignment then draws from the tag's announced providers instead of
    /// the raw validator set. `None` = capability-tagged sagas assign nobody
    /// (accept-any). genesis config, not state.
    capability_registry: Option<ModuleId>,
    /// genesis config, not state: identical on every node by construction.
    policy: LeasePolicy,
    /// committed state — what `root()` and the app-hash commit to.
    sagas: BTreeMap<String, Saga>,
    /// this block's staged writes, read ahead of `sagas` (read-your-writes)
    /// but merged in — and reflected in `root()` — only at `commit_block`.
    /// `Some` stages an upsert, `None` stages a removal (prune).
    pending: BTreeMap<String, Option<Saga>>,
}

impl SagaModule {
    /// an unassigned ledger under [`LeasePolicy::Open`] — no valset, no
    /// assignee, any submitter's result accepted.
    pub fn new(id: impl Into<ModuleId>) -> Self {
        Self {
            id: id.into(),
            valset: None,
            capability_registry: None,
            policy: LeasePolicy::Open,
            sagas: BTreeMap::new(),
            pending: BTreeMap::new(),
        }
    }

    /// a ledger that rendezvous-assigns each attempt over `valset`'s
    /// committed membership, gated by `policy` — the shared base of
    /// [`SagaModule::with_assignment`], which is the constructor real
    /// deployments use.
    fn with_valset(
        id: impl Into<ModuleId>,
        valset: impl Into<ModuleId>,
        policy: LeasePolicy,
    ) -> Self {
        Self {
            id: id.into(),
            valset: Some(valset.into()),
            capability_registry: None,
            policy,
            sagas: BTreeMap::new(),
            pending: BTreeMap::new(),
        }
    }

    /// valset rendezvous assignment plus capability-aware assignment: an
    /// attempt of a saga whose trigger named a capability is
    /// rendezvous-assigned over `capability_registry`'s announced providers
    /// of that tag; untagged sagas keep valset assignment.
    pub fn with_assignment(
        id: impl Into<ModuleId>,
        valset: impl Into<ModuleId>,
        capability_registry: impl Into<ModuleId>,
        policy: LeasePolicy,
    ) -> Self {
        Self {
            capability_registry: Some(capability_registry.into()),
            ..Self::with_valset(id, valset, policy)
        }
    }

    /// read a saga: a STAGED (this-block) write shadows committed state, and a
    /// staged removal shadows it as absent.
    fn get(&self, saga_id: &str) -> Option<&Saga> {
        match self.pending.get(saga_id) {
            Some(staged) => staged.as_ref(),
            None => self.sagas.get(saga_id),
        }
    }

    /// stage a whole saga for this block without committing.
    fn stage(&mut self, saga_id: String, saga: Saga) {
        self.pending.insert(saga_id, Some(saga));
    }

    /// stage a removal for this block without committing.
    fn stage_remove(&mut self, saga_id: String) {
        self.pending.insert(saga_id, None);
    }

    /// every saga id visible this dispatch — committed plus staged, sorted —
    /// the deterministic iteration domain for `Crank`.
    fn visible_ids(&self) -> Vec<String> {
        self.pending
            .keys()
            .chain(self.sagas.keys())
            .cloned()
            .collect::<BTreeSet<String>>()
            .into_iter()
            .collect()
    }

    /// project a saga to its wire view.
    fn view(saga: &Saga) -> SagaView {
        SagaView {
            origin: saga.origin.clone(),
            reply_to: saga.reply_to.clone(),
            reply_payload: saga.reply_payload.clone(),
            spec: saga.spec.clone(),
            capability: saga.capability.clone(),
            status: saga.status,
            attempt: saga.attempt,
            max_attempts: saga.max_attempts,
            assignee: saga.assignee.clone(),
            pinned_assignee: saga.pinned_assignee.clone(),
            lease_views: saga.lease_views,
            lease_expires_at: saga.lease_expires_at,
            deadline: saga.deadline,
            result: saga.result.clone(),
            error: saga.error.clone(),
            created_at: saga.created_at,
            updated_at: saga.updated_at,
        }
    }

    /// the candidate pool one attempt is assigned from. a saga that names a
    /// capability draws from that tag's ANNOUNCED PROVIDERS (the capability
    /// registry's sorted committed view) — never from the raw valset, so a
    /// node that cannot execute the work never holds its lease. an untagged
    /// saga draws from the valset as before. every failure path — module not
    /// configured, query unavailable, empty set — yields `None`: no
    /// assignment, and strict degrades to accept-any for the attempt.
    async fn assignment_pool(
        &self,
        ctx: &dyn Ctx,
        capability: Option<&str>,
    ) -> Option<Vec<Vec<u8>>> {
        let pool = match capability {
            Some(tag) => {
                let registry = self.capability_registry.as_deref()?;
                let reply = ctx
                    .query(
                        registry,
                        &capability_encode_query(&CapabilityQuery::Providers {
                            capability: tag.to_string(),
                        }),
                    )
                    .await
                    .ok()?;
                match capability_decode_reply(&reply).ok()? {
                    CapabilityReply::Providers(providers) => providers,
                    _ => return None,
                }
            }
            None => {
                let valset = self.valset.as_deref()?;
                let reply = ctx
                    .query(valset, &valset_encode_query(&ValsetQuery::Validators))
                    .await
                    .ok()?;
                match valset_decode_reply(&reply).ok()? {
                    ValsetReply::Validators(validators) => validators,
                    // the module answered a different query — no pool.
                    _ => return None,
                }
            }
        };
        (!pool.is_empty()).then_some(pool)
    }

    fn pick_assignee(
        pool: &[Vec<u8>],
        saga_id: &str,
        attempt: u32,
        height: u64,
    ) -> Option<Vec<u8>> {
        if pool.is_empty() {
            return None;
        }
        let mut hasher = Sha256::new();
        hasher.update(saga_id.as_bytes());
        hasher.update(attempt.to_le_bytes());
        hasher.update(height.to_le_bytes());
        let digest = hasher.finalize();
        let pick = u64::from_le_bytes(digest[..8].try_into().expect("8 bytes"));
        Some(pool[(pick % pool.len() as u64) as usize].clone())
    }

    /// rendezvous-assign one attempt over the sorted assignment pool. every
    /// input is agreed, so every validator derives the same assignee.
    async fn compute_assignee(
        &self,
        ctx: &dyn Ctx,
        saga_id: &str,
        capability: Option<&str>,
        attempt: u32,
        height: u64,
    ) -> Option<Vec<u8>> {
        let pool = self.assignment_pool(ctx, capability).await?;
        Self::pick_assignee(&pool, saga_id, attempt, height)
    }

    async fn compute_assignee_excluding(
        &self,
        ctx: &dyn Ctx,
        saga_id: &str,
        capability: Option<&str>,
        attempt: u32,
        height: u64,
        excluded: Option<&[u8]>,
    ) -> Option<Vec<u8>> {
        let mut pool = self.assignment_pool(ctx, capability).await?;
        if let Some(excluded) = excluded {
            pool.retain(|candidate| candidate.as_slice() != excluded);
        }
        Self::pick_assignee(&pool, saga_id, attempt, height)
    }

    /// the P6 promise: on a terminal transition, hand the requester its
    /// callback as a same-block follow-up msg. no-op without a `reply_to`.
    fn emit_callback(ctx: &mut dyn Ctx, saga_id: &str, saga: &Saga, outcome: SagaOutcome) {
        if let Some(target) = &saga.reply_to {
            ctx.emit_msg(Msg {
                target: target.clone(),
                payload: encode_callback(&SagaCallback {
                    saga_id: saga_id.to_string(),
                    payload: saga.reply_payload.clone(),
                    outcome,
                }),
            });
        }
    }

    /// grant the current attempt's lease and ask the worker to run it: the
    /// shared tail of trigger, error-retry, and lease-expiry-retry. a pinned
    /// saga leases every attempt to its pinned key; everything else is
    /// rendezvous-assigned from the pool.
    fn request_assigned(
        &mut self,
        ctx: &mut dyn Ctx,
        saga_id: String,
        mut saga: Saga,
        assignee: Option<Vec<u8>>,
    ) {
        let height = ctx.env().height;
        saga.assignee = assignee;
        saga.lease_expires_at =
            bounded_lease_expiry(height, &saga.assignee, saga.lease_views, saga.deadline);
        ctx.request_effect(Effect(encode_worker_request(&WorkerRequest {
            saga_id: saga_id.clone(),
            attempt: saga.attempt,
            spec: saga.spec.clone(),
            deadline: saga.deadline,
            assignee: saga.assignee.clone(),
        })));
        self.stage(saga_id, saga);
    }

    async fn lease_and_request(&mut self, ctx: &mut dyn Ctx, saga_id: String, saga: Saga) {
        let height = ctx.env().height;
        let assignee = match &saga.pinned_assignee {
            Some(key) => Some(key.clone()),
            None => {
                self.compute_assignee(
                    ctx,
                    &saga_id,
                    saga.capability.as_deref(),
                    saga.attempt,
                    height,
                )
                .await
            }
        };
        self.request_assigned(ctx, saga_id, saga, assignee);
    }

    // ---- state-sync ---------------------------------------------------------
    // hand a joiner the committed continuation state as canonical bytes; the
    // consensus-agreed root — never the serving peer — decides whether they land.

    /// serialize the COMMITTED continuation state (never the staged overlay) into
    /// the canonical encoding `root()` commits to: sorted ids, fixed-width length
    /// prefixes, single-byte enum discriminants. deterministic across nodes.
    pub fn snapshot(&self) -> Vec<u8> {
        encode_committed(&self.sagas)
    }

    /// adopt a peer's snapshot as own committed state — but only after the
    /// decoded temporaries re-derive `expected` via the exact `root()` algorithm,
    /// so a byzantine snapshot cannot land under an agreed root it doesn't match.
    /// all-or-nothing: on any Err this module (and its root) is byte-identical to
    /// before the call. on success the staged overlay is dropped — a snapshot
    /// describes a block boundary, and nothing half-applied may shadow it.
    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        let sagas = decode_committed(bytes)?;
        if committed_root(&sagas) != expected {
            return Err(Error::Module(
                "snapshot does not match expected root".into(),
            ));
        }
        self.sagas = sagas;
        self.pending.clear();
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl Module for SagaModule {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// state-based commitment: sha256 over the canonical committed encoding —
    /// a length-prefixed fold of every saga field in sorted-id order.
    /// insertion-order-independent and idempotent — and sensitive to every
    /// field, so any transition (status, attempt, lease, result) yields a
    /// distinct root. the preimage IS the snapshot encoding (see
    /// [`SagaModule::snapshot`]).
    fn root(&self) -> StateRoot {
        committed_root(&self.sagas)
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(self.snapshot()))
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            SagaMsg::Trigger {
                saga_id,
                spec,
                reply_to,
                reply_payload,
                deadline,
                max_attempts,
                lease_views,
                capability,
                pinned_assignee,
            } => {
                // a duplicate saga_id — staged this block or already committed
                // — is a DETERMINISTIC NO-OP. (v1 silently reset the saga and
                // re-fired the worker, letting any later trigger clobber an
                // in-flight or finished saga.)
                if self.get(&saga_id).is_some() {
                    return Ok(());
                }
                if max_attempts == 0 {
                    return Err(Error::Module("trigger max_attempts must be >= 1".into()));
                }
                // the same commit-into-the-root-preimage class as an oversized
                // result: the spec is stored AND re-emitted per retry, the
                // reply_payload is stored and echoed in the callback.
                if spec.len() > MAX_SPEC_BYTES {
                    return Err(Error::Module(format!(
                        "trigger spec is {} bytes; the cap is {MAX_SPEC_BYTES}",
                        spec.len()
                    )));
                }
                if reply_payload.len() > MAX_REPLY_PAYLOAD_BYTES {
                    return Err(Error::Module(format!(
                        "trigger reply_payload is {} bytes; the cap is {MAX_REPLY_PAYLOAD_BYTES}",
                        reply_payload.len()
                    )));
                }
                // the tag is opaque here (no charset rules — an unannounced
                // tag simply assigns nobody) but its SIZE is bounded like
                // every other stored field; an empty Some is a caller bug,
                // rejected rather than silently read as "no capability".
                if let Some(tag) = &capability {
                    if tag.is_empty() {
                        return Err(Error::Module(
                            "trigger capability must be non-empty when set".into(),
                        ));
                    }
                    if tag.len() > MAX_CAPABILITY_BYTES {
                        return Err(Error::Module(format!(
                            "trigger capability is {} bytes; the cap is {MAX_CAPABILITY_BYTES}",
                            tag.len()
                        )));
                    }
                }
                // an empty pinned key is a caller bug, rejected rather than
                // silently read as "no binding" (the same rule as an empty
                // capability tag).
                if let Some(key) = &pinned_assignee
                    && key.is_empty()
                {
                    return Err(Error::Module(
                        "trigger pinned_assignee must be non-empty when set".into(),
                    ));
                }
                // the callback-poison rule (design §4): a callback aimed at an
                // unknown module — or at this module itself, which cannot
                // decode its own callback — would abort every future terminal
                // block and wedge the saga at Pending forever. reject at
                // trigger time, while rejection is still cheap and local.
                if let Some(target) = &reply_to {
                    if *target == ctx.env().me {
                        return Err(Error::Module(
                            "trigger reply_to must not target the saga module itself".into(),
                        ));
                    }
                    if ctx.module_root(target).is_none() {
                        return Err(Error::Module(format!(
                            "trigger reply_to targets unknown module {target}"
                        )));
                    }
                }
                let now = ctx.env().consensus_time;
                let saga = Saga {
                    origin: saga_origin(&ctx.env().origin),
                    reply_to,
                    reply_payload,
                    spec,
                    capability,
                    status: SagaStatus::Pending,
                    attempt: 0,
                    max_attempts,
                    assignee: None,
                    pinned_assignee,
                    lease_views,
                    lease_expires_at: None,
                    deadline,
                    result: None,
                    error: None,
                    created_at: now,
                    updated_at: now,
                };
                self.lease_and_request(ctx, saga_id, saga).await;
            }
            SagaMsg::OracleResult {
                saga_id,
                attempt,
                outcome,
                ..
            } => {
                // P5 gates, all deterministic no-ops: unknown saga (never
                // triggered, or pruned), terminal saga (a duplicate — the
                // first agreed result won), stale attempt (an executor
                // answering work that was already re-leased).
                let Some(current) = self.get(&saga_id) else {
                    return Ok(());
                };
                if current.status.is_terminal() || attempt != current.attempt {
                    return Ok(());
                }
                // the lease gate: under Strict a result lands only from the
                // assignee's external origin; anyone else is a no-op (never
                // an error — a finalized foreign result must not abort the
                // block). an UNASSIGNED attempt accepts no result at all
                // under Strict: its request was an announcement, and the
                // work is claimed via Accept first.
                if self.policy == LeasePolicy::Strict {
                    match &current.assignee {
                        Some(assignee) => {
                            let held = matches!(
                                &ctx.env().origin,
                                Origin::External(key) if key == assignee
                            );
                            if !held {
                                return Ok(());
                            }
                        }
                        None => return Ok(()),
                    }
                }
                // an oversized error string is the same abort-don't-commit
                // case as an oversized result: the Failed arm stores it in the
                // root preimage and echoes it in the callback.
                if let Err(error) = &outcome
                    && error.len() > MAX_ERROR_BYTES
                {
                    return Err(Error::Module(format!(
                        "oracle error is {} bytes; the cap is {MAX_ERROR_BYTES}",
                        error.len()
                    )));
                }
                let mut saga = current.clone();
                saga.updated_at = ctx.env().consensus_time;
                match outcome {
                    Ok(result) => {
                        // a finalized oversized result must not commit: abort
                        // the block rather than bloat the root preimage.
                        if result.len() > MAX_RESULT_BYTES {
                            return Err(Error::Module(format!(
                                "oracle result is {} bytes; the cap is {MAX_RESULT_BYTES}",
                                result.len()
                            )));
                        }
                        saga.status = SagaStatus::Done;
                        saga.result = Some(result.clone());
                        Self::emit_callback(ctx, &saga_id, &saga, SagaOutcome::Done(result));
                        self.stage(saga_id, saga);
                    }
                    // an Err consumes the attempt: re-lease while attempts
                    // remain, else the saga is terminally Failed.
                    Err(_) if saga.attempt + 1 < saga.max_attempts => {
                        saga.attempt += 1;
                        self.lease_and_request(ctx, saga_id, saga).await;
                    }
                    Err(error) => {
                        saga.status = SagaStatus::Failed;
                        saga.error = Some(error.clone());
                        Self::emit_callback(ctx, &saga_id, &saga, SagaOutcome::Failed(error));
                        self.stage(saga_id, saga);
                    }
                }
            }
            SagaMsg::RenewLease { saga_id, attempt } => {
                let Some(current) = self.get(&saga_id) else {
                    return Ok(());
                };
                if current.status.is_terminal() || attempt != current.attempt {
                    return Ok(());
                }
                let held = matches!(
                    (&ctx.env().origin, &current.assignee),
                    (Origin::External(key), Some(assignee)) if key == assignee
                );
                if !held {
                    return Ok(());
                }
                let height = ctx.env().height;
                let Some(expiry) = current.lease_expires_at else {
                    return Ok(());
                };
                if height >= expiry {
                    return Ok(());
                }
                let window = current.lease_views.unwrap_or(DEFAULT_LEASE_VIEWS);
                let mut saga = current.clone();
                if height >= expiry.saturating_sub(window / 2) {
                    let next = bounded_lease_expiry(
                        height,
                        &current.assignee,
                        current.lease_views,
                        current.deadline,
                    );
                    if next.is_some_and(|next| next > expiry) {
                        saga.lease_expires_at = next;
                    }
                }
                saga.updated_at = ctx.env().consensus_time;
                self.stage(saga_id, saga);
            }
            SagaMsg::Reassign { saga_id, attempt } => {
                let Some(current) = self.get(&saga_id) else {
                    return Ok(());
                };
                if current.status.is_terminal()
                    || attempt != current.attempt
                    || current.origin != saga_origin(&ctx.env().origin)
                {
                    return Ok(());
                }
                let mut saga = current.clone();
                saga.updated_at = ctx.env().consensus_time;
                if saga.pinned_assignee.is_some() {
                    return Err(Error::Module("pinned saga cannot be reassigned".into()));
                }
                if saga.attempt + 1 >= saga.max_attempts {
                    return Err(Error::Module("reassignment attempts exhausted".into()));
                }

                let old_assignee = saga.assignee.clone();
                saga.attempt += 1;
                let next = self
                    .compute_assignee_excluding(
                        ctx,
                        &saga_id,
                        saga.capability.as_deref(),
                        saga.attempt,
                        ctx.env().height,
                        old_assignee.as_deref(),
                    )
                    .await;
                let Some(next) = next else {
                    return Err(Error::Module("no alternate assignee is available".into()));
                };
                self.request_assigned(ctx, saga_id, saga, Some(next));
            }
            SagaMsg::Accept { saga_id, attempt } => {
                // the claim lane for UNASSIGNED attempts: first accept in
                // consensus order wins the lease; everything else — unknown
                // or terminal saga, stale attempt, an attempt someone (or
                // rendezvous) already assigned — is a deterministic no-op,
                // never an error (a finalized late accept must not abort
                // the block).
                let Origin::External(key) = &ctx.env().origin else {
                    return Err(Error::Module(
                        "Accept requires an external origin (the accepting node's key)".into(),
                    ));
                };
                if key.is_empty() {
                    return Err(Error::Module(
                        "Accept requires a non-empty submitter id".into(),
                    ));
                }
                let Some(current) = self.get(&saga_id) else {
                    return Ok(());
                };
                if current.status.is_terminal()
                    || attempt != current.attempt
                    || current.assignee.is_some()
                {
                    return Ok(());
                }
                let height = ctx.env().height;
                let mut saga = current.clone();
                saga.assignee = Some(key.clone());
                saga.lease_expires_at =
                    bounded_lease_expiry(height, &saga.assignee, saga.lease_views, saga.deadline);
                saga.updated_at = ctx.env().consensus_time;
                // the actual work order: the announcement's request, re-emitted
                // naming the winner — every other node's worker skips it.
                ctx.request_effect(Effect(encode_worker_request(&WorkerRequest {
                    saga_id: saga_id.clone(),
                    attempt: saga.attempt,
                    spec: saga.spec.clone(),
                    deadline: saga.deadline,
                    assignee: saga.assignee.clone(),
                })));
                self.stage(saga_id, saga);
            }
            SagaMsg::Crank {} => {
                // PERMISSIONLESS: any origin may crank — P7's liveness comes
                // from anyone submitting this op, and its safety from every
                // check reading only agreed values. bounded sweep in id order;
                // when nothing has expired, nothing is staged and the root is
                // untouched.
                let now = ctx.env().consensus_time;
                let mut transitions: u32 = 0;
                for saga_id in self.visible_ids() {
                    if transitions == CRANK_BUDGET {
                        break;
                    }
                    let Some(current) = self.get(&saga_id) else {
                        continue;
                    };
                    if current.status.is_terminal() {
                        continue;
                    }
                    let deadline_hit = current.deadline.is_some_and(|d| now >= d);
                    let lease_hit = current.lease_expires_at.is_some_and(|l| now >= l);
                    if !deadline_hit && !lease_hit {
                        continue;
                    }
                    let mut saga = current.clone();
                    saga.updated_at = now;
                    if deadline_hit {
                        // the whole-saga deadline dominates the lease: no
                        // retry may outlive it.
                        saga.status = SagaStatus::TimedOut;
                        Self::emit_callback(ctx, &saga_id, &saga, SagaOutcome::TimedOut);
                        self.stage(saga_id, saga);
                    } else if saga.attempt + 1 < saga.max_attempts {
                        // an expired lease consumes the attempt and re-leases.
                        saga.attempt += 1;
                        self.lease_and_request(ctx, saga_id, saga).await;
                    } else {
                        let error = "lease attempts exhausted".to_string();
                        saga.status = SagaStatus::Failed;
                        saga.error = Some(error.clone());
                        Self::emit_callback(ctx, &saga_id, &saga, SagaOutcome::Failed(error));
                        self.stage(saga_id, saga);
                    }
                    transitions += 1;
                }
            }
            SagaMsg::Cancel { saga_id } => {
                // only the recorded trigger origin may cancel, and only a
                // pending saga; everything else — terminal, unknown, foreign
                // origin — is a deterministic no-op, never an error (a
                // finalized foreign cancel must not abort the block).
                let Some(current) = self.get(&saga_id) else {
                    return Ok(());
                };
                if current.status.is_terminal() || current.origin != saga_origin(&ctx.env().origin)
                {
                    return Ok(());
                }
                let mut saga = current.clone();
                saga.status = SagaStatus::Cancelled;
                saga.updated_at = ctx.env().consensus_time;
                Self::emit_callback(ctx, &saga_id, &saga, SagaOutcome::Cancelled);
                self.stage(saga_id, saga);
            }
            SagaMsg::Prune { saga_ids } => {
                // explicit GC: remove TERMINAL sagas whose recorded trigger
                // origin matches the submitter. non-terminal, foreign, and
                // unknown ids are skipped as no-ops. no lazy retention sweep
                // exists — retention is always an owner's explicit choice.
                let origin = saga_origin(&ctx.env().origin);
                for saga_id in saga_ids {
                    let Some(current) = self.get(&saga_id) else {
                        continue;
                    };
                    if !current.status.is_terminal() || current.origin != origin {
                        continue;
                    }
                    self.stage_remove(saga_id);
                }
            }
        }
        Ok(())
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            SagaQuery::Get { saga_id } => Ok(encode_reply(&SagaReply::Saga(
                self.get(&saga_id).map(Self::view),
            ))),
            SagaQuery::NextExpiry => {
                // the crank pump's read: the earliest lease-expiry or
                // deadline over PENDING sagas — once the current view reaches
                // it, a Crank is guaranteed to transition something.
                let next = self
                    .visible_ids()
                    .into_iter()
                    .filter_map(|id| self.get(&id))
                    .filter(|saga| !saga.status.is_terminal())
                    .flat_map(|saga| [saga.lease_expires_at, saga.deadline])
                    .flatten()
                    .min();
                Ok(encode_reply(&SagaReply::NextExpiry(next)))
            }
            SagaQuery::AssignedPending { assignee } => {
                // the resident worker pump's read: reconstruct exactly the
                // WorkerRequest the effect lane carried for every pending
                // attempt leased to `assignee`. a node that installs synced
                // boundaries (and so never observes effects) discovers its
                // own assigned work here; visible_ids is sorted, so the
                // projection is deterministic.
                let requests = self
                    .visible_ids()
                    .into_iter()
                    .filter_map(|id| self.get(&id).map(|saga| (id, saga)))
                    .filter(|(_, saga)| {
                        !saga.status.is_terminal()
                            && saga.assignee.as_deref() == Some(assignee.as_slice())
                    })
                    .map(|(id, saga)| WorkerRequest {
                        saga_id: id,
                        attempt: saga.attempt,
                        spec: saga.spec.clone(),
                        deadline: saga.deadline,
                        assignee: saga.assignee.clone(),
                    })
                    .collect();
                Ok(encode_reply(&SagaReply::AssignedPending(requests)))
            }
        }
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        for (id, staged) in std::mem::take(&mut self.pending) {
            match staged {
                Some(saga) => {
                    self.sagas.insert(id, saga);
                }
                None => {
                    self.sagas.remove(&id);
                }
            }
        }
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{decode_callback, decode_reply, decode_worker_request, encode_msg, encode_query};
    use futures::executor::block_on;
    use sdk::{Env, Event};

    /// a minimal `Ctx` that captures emitted msgs/effects and serves a canned
    /// valset — enough to unit-test `execute` in isolation (the host provides
    /// the real one in integration).
    struct CaptureCtx {
        env: Env,
        /// module ids `module_root` resolves (reply_to validation).
        known_modules: BTreeSet<String>,
        /// a canned validator set served for a "valset" query when present.
        validators: Option<Vec<Vec<u8>>>,
        /// canned capability providers served for a "capability" query.
        providers: Option<Vec<Vec<u8>>>,
        msgs: Vec<Msg>,
        effects: Vec<Effect>,
    }
    impl CaptureCtx {
        fn new() -> Self {
            Self {
                env: Env {
                    protocol_version: 0,
                    height: 0,
                    consensus_time: 0,
                    origin: Origin::System,
                    me: "saga".into(),
                },
                known_modules: BTreeSet::new(),
                validators: None,
                providers: None,
                msgs: Vec::new(),
                effects: Vec::new(),
            }
        }
        fn at(mut self, height: u64) -> Self {
            self.env.height = height;
            self.env.consensus_time = height;
            self
        }
        fn with_origin(mut self, origin: Origin) -> Self {
            self.env.origin = origin;
            self
        }
        fn knowing(mut self, module: &str) -> Self {
            self.known_modules.insert(module.into());
            self
        }
        fn with_validators(mut self, validators: Vec<Vec<u8>>) -> Self {
            self.validators = Some(validators);
            self
        }
        fn with_providers(mut self, providers: Vec<Vec<u8>>) -> Self {
            self.providers = Some(providers);
            self
        }
        fn callbacks(&self) -> Vec<SagaCallback> {
            self.msgs
                .iter()
                .map(|m| decode_callback(&m.payload).expect("callback payload"))
                .collect()
        }
        fn worker_requests(&self) -> Vec<WorkerRequest> {
            self.effects
                .iter()
                .map(|e| decode_worker_request(&e.0).expect("worker request payload"))
                .collect()
        }
    }
    #[async_trait::async_trait(?Send)]
    impl Ctx for CaptureCtx {
        fn env(&self) -> &Env {
            &self.env
        }
        fn module_root(&self, target: &str) -> Option<StateRoot> {
            self.known_modules
                .contains(target)
                .then_some(StateRoot::ZERO)
        }
        async fn query(&self, target: &str, _req: &[u8]) -> Result<Vec<u8>, Error> {
            match target {
                "valset" => match &self.validators {
                    Some(v) => Ok(valset::encode_reply(&ValsetReply::Validators(v.clone()))),
                    None => Err(Error::QueryUnsupported),
                },
                "capability" => match &self.providers {
                    Some(p) => Ok(capability::encode_reply(&CapabilityReply::Providers(
                        p.clone(),
                    ))),
                    None => Err(Error::QueryUnsupported),
                },
                _ => Err(Error::QueryUnsupported),
            }
        }
        fn emit_msg(&mut self, msg: Msg) {
            self.msgs.push(msg);
        }
        fn emit_event(&mut self, _ev: Event) {}
        fn request_effect(&mut self, eff: Effect) {
            self.effects.push(eff);
        }
    }

    /// a trigger with fire-and-forget defaults; tests override fields inline.
    fn trigger_msg(id: &str, spec: &[u8]) -> SagaMsg {
        SagaMsg::Trigger {
            pinned_assignee: None,
            saga_id: id.into(),
            spec: spec.to_vec(),
            reply_to: None,
            reply_payload: Vec::new(),
            deadline: None,
            max_attempts: 1,
            lease_views: None,
            capability: None,
        }
    }
    fn msg(m: &SagaMsg) -> Msg {
        Msg {
            target: "saga".into(),
            payload: encode_msg(m),
        }
    }
    fn trigger(id: &str, spec: &[u8]) -> Msg {
        msg(&trigger_msg(id, spec))
    }
    fn oracle(id: &str, attempt: u32, outcome: Result<Vec<u8>, String>) -> Msg {
        msg(&SagaMsg::OracleResult {
            saga_id: id.into(),
            attempt,
            outcome,
            usage: None,
        })
    }
    fn crank() -> Msg {
        msg(&SagaMsg::Crank {})
    }
    fn get(m: &SagaModule, id: &str) -> Option<SagaView> {
        let reply =
            block_on(m.query(&encode_query(&SagaQuery::Get { saga_id: id.into() }))).unwrap();
        match decode_reply(&reply).unwrap() {
            SagaReply::Saga(v) => v,
            other => panic!("expected Saga reply, got {other:?}"),
        }
    }
    fn next_expiry(m: &SagaModule) -> Option<u64> {
        let reply = block_on(m.query(&encode_query(&SagaQuery::NextExpiry))).unwrap();
        match decode_reply(&reply).unwrap() {
            SagaReply::NextExpiry(v) => v,
            other => panic!("expected NextExpiry reply, got {other:?}"),
        }
    }
    fn assigned_pending(m: &SagaModule, assignee: &[u8]) -> Vec<WorkerRequest> {
        let reply = block_on(m.query(&encode_query(&SagaQuery::AssignedPending {
            assignee: assignee.to_vec(),
        })))
        .unwrap();
        match decode_reply(&reply).unwrap() {
            SagaReply::AssignedPending(v) => v,
            other => panic!("expected AssignedPending reply, got {other:?}"),
        }
    }
    fn exec(m: &mut SagaModule, ctx: &mut CaptureCtx, op: &Msg) -> Result<(), Error> {
        block_on(m.execute(ctx, op))
    }
    fn commit(m: &mut SagaModule) {
        block_on(m.commit_block()).unwrap();
    }

    #[test]
    fn trigger_stages_pending_and_emits_one_worker_request() {
        let mut m = SagaModule::new("saga");
        let r0 = m.root();
        let mut ctx = CaptureCtx::new();
        exec(
            &mut m,
            &mut ctx,
            &msg(&SagaMsg::Trigger {
                pinned_assignee: None,
                saga_id: "s1".into(),
                spec: b"hello".to_vec(),
                reply_to: None,
                reply_payload: Vec::new(),
                deadline: Some(99),
                max_attempts: 3,
                lease_views: None,
                capability: None,
            }),
        )
        .unwrap();

        // exactly one worker-request effect, keyed (saga_id, attempt 0).
        assert_eq!(
            ctx.worker_requests(),
            vec![WorkerRequest {
                saga_id: "s1".into(),
                attempt: 0,
                spec: b"hello".to_vec(),
                deadline: Some(99),
                assignee: None,
            }],
            "trigger emits exactly one WorkerRequest effect"
        );

        // read-your-writes shows Pending before commit; root only moves on commit.
        let v = get(&m, "s1").unwrap();
        assert_eq!(v.status, SagaStatus::Pending);
        assert_eq!(v.attempt, 0);
        assert_eq!(v.max_attempts, 3);
        assert_eq!(v.deadline, Some(99));
        assert_eq!(v.origin, SagaOrigin::System);
        assert_eq!(
            m.root(),
            r0,
            "staged-but-uncommitted work does not move root"
        );
        commit(&mut m);
        assert_ne!(m.root(), r0, "committing the pending saga moves the root");
    }

    #[test]
    fn duplicate_trigger_is_a_deterministic_no_op() {
        let mut m = SagaModule::new("saga");
        let mut ctx = CaptureCtx::new();
        exec(&mut m, &mut ctx, &trigger("s1", b"first")).unwrap();

        // a STAGED duplicate in the same block: no reset, no second effect.
        exec(&mut m, &mut ctx, &trigger("s1", b"second")).unwrap();
        assert_eq!(
            ctx.effects.len(),
            1,
            "a staged duplicate re-fires no worker"
        );
        assert_eq!(get(&m, "s1").unwrap().spec, b"first".to_vec());
        commit(&mut m);
        let committed_root = m.root();

        // a COMMITTED duplicate in a later block: root unchanged, no effect.
        let mut ctx2 = CaptureCtx::new().at(7);
        exec(&mut m, &mut ctx2, &trigger("s1", b"third")).unwrap();
        assert!(
            ctx2.effects.is_empty(),
            "a committed duplicate re-fires no worker"
        );
        commit(&mut m);
        assert_eq!(
            m.root(),
            committed_root,
            "a duplicate trigger is a no-op — root unchanged"
        );
        assert_eq!(
            get(&m, "s1").unwrap().spec,
            b"first".to_vec(),
            "the original spec survives"
        );
    }

    #[test]
    fn zero_max_attempts_is_rejected() {
        let mut m = SagaModule::new("saga");
        let mut ctx = CaptureCtx::new();
        let err = exec(
            &mut m,
            &mut ctx,
            &msg(&SagaMsg::Trigger {
                pinned_assignee: None,
                saga_id: "s1".into(),
                spec: Vec::new(),
                reply_to: None,
                reply_payload: Vec::new(),
                deadline: None,
                max_attempts: 0,
                lease_views: None,
                capability: None,
            }),
        )
        .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        assert!(ctx.effects.is_empty(), "a rejected trigger fires no worker");
        assert_eq!(get(&m, "s1"), None);
    }

    #[test]
    fn unknown_or_self_reply_to_is_rejected_at_trigger_time() {
        // the callback-poison pin, half (a): an unknown callback target would
        // abort every future terminal block, so it never becomes a saga.
        let mut m = SagaModule::new("saga");
        let mut ctx = CaptureCtx::new().knowing("agent");
        let err = exec(
            &mut m,
            &mut ctx,
            &msg(&SagaMsg::Trigger {
                pinned_assignee: None,
                saga_id: "s1".into(),
                spec: Vec::new(),
                reply_to: Some("nope".into()),
                reply_payload: Vec::new(),
                deadline: None,
                max_attempts: 1,
                lease_views: None,
                capability: None,
            }),
        )
        .unwrap_err();
        assert!(
            matches!(err, Error::Module(_)),
            "unknown reply_to rejects at trigger"
        );
        assert_eq!(get(&m, "s1"), None, "no saga was staged");

        // a self-targeting callback can never decode: equally poison.
        let err = exec(
            &mut m,
            &mut ctx,
            &msg(&SagaMsg::Trigger {
                pinned_assignee: None,
                saga_id: "s2".into(),
                spec: Vec::new(),
                reply_to: Some("saga".into()),
                reply_payload: Vec::new(),
                deadline: None,
                max_attempts: 1,
                lease_views: None,
                capability: None,
            }),
        )
        .unwrap_err();
        assert!(
            matches!(err, Error::Module(_)),
            "self reply_to rejects at trigger"
        );

        // a KNOWN reply_to passes the same gate.
        exec(
            &mut m,
            &mut ctx,
            &msg(&SagaMsg::Trigger {
                pinned_assignee: None,
                saga_id: "s3".into(),
                spec: Vec::new(),
                reply_to: Some("agent".into()),
                reply_payload: Vec::new(),
                deadline: None,
                max_attempts: 1,
                lease_views: None,
                capability: None,
            }),
        )
        .unwrap();
        assert_eq!(get(&m, "s3").unwrap().reply_to, Some("agent".to_string()));
    }

    #[test]
    fn ok_result_lands_done_and_emits_the_callback() {
        let mut m = SagaModule::new("saga");
        let mut ctx = CaptureCtx::new().knowing("agent");
        exec(
            &mut m,
            &mut ctx,
            &msg(&SagaMsg::Trigger {
                pinned_assignee: None,
                saga_id: "s1".into(),
                spec: b"work".to_vec(),
                reply_to: Some("agent".into()),
                reply_payload: b"corr-7".to_vec(),
                deadline: None,
                max_attempts: 1,
                lease_views: None,
                capability: None,
            }),
        )
        .unwrap();
        commit(&mut m);
        let pending_root = m.root();

        let mut ctx = CaptureCtx::new().at(5).knowing("agent");
        exec(&mut m, &mut ctx, &oracle("s1", 0, Ok(b"answer".to_vec()))).unwrap();
        commit(&mut m);

        let v = get(&m, "s1").unwrap();
        assert_eq!(v.status, SagaStatus::Done);
        assert_eq!(v.result, Some(b"answer".to_vec()));
        assert_eq!(v.updated_at, 5);
        assert_ne!(m.root(), pending_root, "Pending -> Done moves the root");

        // the P6 callback: correlation payload echoed, outcome carried.
        assert_eq!(ctx.msgs.len(), 1, "exactly one callback msg");
        assert_eq!(ctx.msgs[0].target, "agent");
        assert_eq!(
            ctx.callbacks(),
            vec![SagaCallback {
                saga_id: "s1".into(),
                payload: b"corr-7".to_vec(),
                outcome: SagaOutcome::Done(b"answer".to_vec()),
            }]
        );
    }

    #[test]
    fn err_result_retries_then_lands_done() {
        let mut m = SagaModule::new("saga");
        let mut ctx = CaptureCtx::new();
        exec(
            &mut m,
            &mut ctx,
            &msg(&SagaMsg::Trigger {
                pinned_assignee: None,
                saga_id: "s1".into(),
                spec: b"work".to_vec(),
                reply_to: None,
                reply_payload: Vec::new(),
                deadline: None,
                max_attempts: 2,
                lease_views: None,
                capability: None,
            }),
        )
        .unwrap();
        commit(&mut m);

        // attempt 0 fails: the attempt increments and the worker is re-asked
        // under the NEW idempotency key (saga_id, attempt 1).
        let mut ctx = CaptureCtx::new().at(3);
        exec(
            &mut m,
            &mut ctx,
            &oracle("s1", 0, Err("worker crashed".into())),
        )
        .unwrap();
        commit(&mut m);
        let v = get(&m, "s1").unwrap();
        assert_eq!(
            v.status,
            SagaStatus::Pending,
            "attempts remain -> still pending"
        );
        assert_eq!(v.attempt, 1, "the Err consumed attempt 0");
        assert_eq!(v.error, None, "a retried attempt stores no terminal error");
        let requests = ctx.worker_requests();
        assert_eq!(
            requests.len(),
            1,
            "the retry re-emits exactly one WorkerRequest"
        );
        assert_eq!(requests[0].attempt, 1);
        assert_eq!(requests[0].spec, b"work".to_vec());

        // attempt 1 succeeds.
        let mut ctx = CaptureCtx::new().at(4);
        exec(
            &mut m,
            &mut ctx,
            &oracle("s1", 1, Ok(b"recovered".to_vec())),
        )
        .unwrap();
        commit(&mut m);
        let v = get(&m, "s1").unwrap();
        assert_eq!(v.status, SagaStatus::Done);
        assert_eq!(v.result, Some(b"recovered".to_vec()));
        assert_eq!(v.attempt, 1);
    }

    #[test]
    fn err_result_with_attempts_exhausted_lands_failed() {
        let mut m = SagaModule::new("saga");
        let mut ctx = CaptureCtx::new().knowing("agent");
        exec(
            &mut m,
            &mut ctx,
            &msg(&SagaMsg::Trigger {
                pinned_assignee: None,
                saga_id: "s1".into(),
                spec: Vec::new(),
                reply_to: Some("agent".into()),
                reply_payload: b"c".to_vec(),
                deadline: None,
                max_attempts: 1,
                lease_views: None,
                capability: None,
            }),
        )
        .unwrap();
        commit(&mut m);

        let mut ctx = CaptureCtx::new().knowing("agent");
        exec(&mut m, &mut ctx, &oracle("s1", 0, Err("boom".into()))).unwrap();
        commit(&mut m);
        let v = get(&m, "s1").unwrap();
        assert_eq!(v.status, SagaStatus::Failed);
        assert_eq!(v.error, Some("boom".to_string()));
        assert!(
            ctx.effects.is_empty(),
            "no attempts remain -> no retry effect"
        );
        assert_eq!(
            ctx.callbacks(),
            vec![SagaCallback {
                saga_id: "s1".into(),
                payload: b"c".to_vec(),
                outcome: SagaOutcome::Failed("boom".into()),
            }],
            "the terminal failure still fires the callback"
        );
    }

    #[test]
    fn duplicate_and_stale_results_are_no_ops() {
        let mut m = SagaModule::new("saga");
        let mut ctx = CaptureCtx::new();
        exec(
            &mut m,
            &mut ctx,
            &msg(&SagaMsg::Trigger {
                pinned_assignee: None,
                saga_id: "s1".into(),
                spec: Vec::new(),
                reply_to: None,
                reply_payload: Vec::new(),
                deadline: None,
                max_attempts: 3,
                lease_views: None,
                capability: None,
            }),
        )
        .unwrap();
        commit(&mut m);

        // fail attempt 0 -> now on attempt 1. a STALE result for attempt 0
        // (an executor that lost its lease) must be a no-op.
        let mut ctx = CaptureCtx::new();
        exec(&mut m, &mut ctx, &oracle("s1", 0, Err("slow".into()))).unwrap();
        commit(&mut m);
        let retry_root = m.root();
        let mut ctx = CaptureCtx::new();
        exec(&mut m, &mut ctx, &oracle("s1", 0, Ok(b"stale".to_vec()))).unwrap();
        commit(&mut m);
        assert_eq!(
            m.root(),
            retry_root,
            "a stale-attempt result is a no-op — root unchanged"
        );
        assert_eq!(get(&m, "s1").unwrap().status, SagaStatus::Pending);

        // land attempt 1, then a DUPLICATE result must not overwrite it.
        let mut ctx = CaptureCtx::new();
        exec(&mut m, &mut ctx, &oracle("s1", 1, Ok(b"first".to_vec()))).unwrap();
        commit(&mut m);
        let done_root = m.root();
        let mut ctx = CaptureCtx::new();
        exec(&mut m, &mut ctx, &oracle("s1", 1, Ok(b"second".to_vec()))).unwrap();
        commit(&mut m);
        assert_eq!(
            get(&m, "s1").unwrap().result,
            Some(b"first".to_vec()),
            "first agreed result wins"
        );
        assert_eq!(
            m.root(),
            done_root,
            "a duplicate OracleResult is a no-op — root unchanged"
        );

        // and a result for an UNKNOWN saga is equally a no-op.
        let mut ctx = CaptureCtx::new();
        exec(&mut m, &mut ctx, &oracle("ghost", 0, Ok(b"x".to_vec()))).unwrap();
        commit(&mut m);
        assert_eq!(m.root(), done_root);
    }

    #[test]
    fn oversized_spec_reply_payload_and_error_abort_like_results() {
        // the symmetric caps: spec and reply_payload at trigger time, the Err
        // string at result time — all the same commit-into-the-root-preimage
        // class as an oversized Ok result.
        let mut m = SagaModule::new("saga");
        let genesis_root = m.root();

        let mut ctx = CaptureCtx::new();
        let err = exec(
            &mut m,
            &mut ctx,
            &msg(&SagaMsg::Trigger {
                pinned_assignee: None,
                saga_id: "s1".into(),
                spec: vec![0u8; MAX_SPEC_BYTES + 1],
                reply_to: None,
                reply_payload: Vec::new(),
                deadline: None,
                max_attempts: 1,
                lease_views: None,
                capability: None,
            }),
        )
        .unwrap_err();
        assert!(matches!(err, Error::Module(_)), "oversized spec errs");
        assert!(
            ctx.effects.is_empty(),
            "no WorkerRequest for a rejected trigger"
        );

        let mut ctx = CaptureCtx::new();
        let err = exec(
            &mut m,
            &mut ctx,
            &msg(&SagaMsg::Trigger {
                pinned_assignee: None,
                saga_id: "s1".into(),
                spec: b"w".to_vec(),
                reply_to: None,
                reply_payload: vec![0u8; MAX_REPLY_PAYLOAD_BYTES + 1],
                deadline: None,
                max_attempts: 1,
                lease_views: None,
                capability: None,
            }),
        )
        .unwrap_err();
        assert!(
            matches!(err, Error::Module(_)),
            "oversized reply_payload errs"
        );

        block_on(m.abort_block()).unwrap();
        assert_eq!(m.root(), genesis_root, "rejected triggers left no trace");

        // an oversized Err string aborts instead of committing into the root.
        let mut ctx = CaptureCtx::new();
        exec(&mut m, &mut ctx, &trigger("s1", b"w")).unwrap();
        commit(&mut m);
        let pending_root = m.root();
        let mut ctx = CaptureCtx::new();
        let huge = "e".repeat(MAX_ERROR_BYTES + 1);
        let err = exec(&mut m, &mut ctx, &oracle("s1", 0, Err(huge))).unwrap_err();
        assert!(matches!(err, Error::Module(_)), "oversized error errs");
        block_on(m.abort_block()).unwrap();
        assert_eq!(m.root(), pending_root, "the aborted block left no trace");
        assert_eq!(get(&m, "s1").unwrap().status, SagaStatus::Pending);

        // boundary sizes are accepted: an at-cap Err lands as Failed.
        let mut ctx = CaptureCtx::new();
        let at_cap = "e".repeat(MAX_ERROR_BYTES);
        exec(&mut m, &mut ctx, &oracle("s1", 0, Err(at_cap.clone()))).unwrap();
        commit(&mut m);
        let v = get(&m, "s1").unwrap();
        assert_eq!(v.status, SagaStatus::Failed);
        assert_eq!(v.error, Some(at_cap));
    }

    #[test]
    fn oversized_result_aborts_and_the_boundary_is_accepted() {
        let mut m = SagaModule::new("saga");
        let mut ctx = CaptureCtx::new();
        exec(&mut m, &mut ctx, &trigger("s1", b"w")).unwrap();
        commit(&mut m);
        let pending_root = m.root();

        // one byte over the cap: the op errs (the host aborts the block) and
        // the staged overlay is dropped — root byte-identical.
        let mut ctx = CaptureCtx::new();
        let err = exec(
            &mut m,
            &mut ctx,
            &oracle("s1", 0, Ok(vec![0u8; MAX_RESULT_BYTES + 1])),
        )
        .unwrap_err();
        assert!(
            matches!(err, Error::Module(_)),
            "oversized result errs with Module"
        );
        block_on(m.abort_block()).unwrap();
        assert_eq!(m.root(), pending_root, "the aborted block left no trace");
        assert_eq!(get(&m, "s1").unwrap().status, SagaStatus::Pending);

        // exactly the cap is accepted.
        let mut ctx = CaptureCtx::new();
        exec(
            &mut m,
            &mut ctx,
            &oracle("s1", 0, Ok(vec![0u8; MAX_RESULT_BYTES])),
        )
        .unwrap();
        commit(&mut m);
        let v = get(&m, "s1").unwrap();
        assert_eq!(v.status, SagaStatus::Done);
        assert_eq!(v.result.unwrap().len(), MAX_RESULT_BYTES);
    }

    #[test]
    fn crank_times_out_a_past_deadline_saga_and_deadline_dominates_lease() {
        let mut m = SagaModule::new("saga");
        let mut ctx = CaptureCtx::new().knowing("agent");
        exec(
            &mut m,
            &mut ctx,
            &msg(&SagaMsg::Trigger {
                pinned_assignee: None,
                saga_id: "s1".into(),
                spec: Vec::new(),
                reply_to: Some("agent".into()),
                reply_payload: Vec::new(),
                deadline: Some(10),
                // a live lease window AND spare attempts: if the lease were
                // checked first this would retry — the deadline must win.
                max_attempts: 5,
                lease_views: Some(4),
                capability: None,
            }),
        )
        .unwrap();
        commit(&mut m);

        // before the deadline (and before the lease expires) a crank is a
        // strict no-op: root byte-identical.
        let before = m.root();
        let mut ctx = CaptureCtx::new().at(3).knowing("agent");
        exec(&mut m, &mut ctx, &crank()).unwrap();
        commit(&mut m);
        assert_eq!(
            m.root(),
            before,
            "an unexpired crank leaves the root byte-identical"
        );
        assert!(ctx.msgs.is_empty() && ctx.effects.is_empty());

        // at the deadline: TimedOut, callback fired, no retry despite the
        // spare attempts and the (also expired) lease.
        let mut ctx = CaptureCtx::new().at(10).knowing("agent");
        exec(&mut m, &mut ctx, &crank()).unwrap();
        commit(&mut m);
        let v = get(&m, "s1").unwrap();
        assert_eq!(
            v.status,
            SagaStatus::TimedOut,
            "deadline dominates the lease"
        );
        assert_eq!(v.attempt, 0, "a timeout consumes no attempt");
        assert!(ctx.effects.is_empty(), "no retry past the deadline");
        assert_eq!(
            ctx.callbacks(),
            vec![SagaCallback {
                saga_id: "s1".into(),
                payload: Vec::new(),
                outcome: SagaOutcome::TimedOut,
            }]
        );
    }

    #[test]
    fn crank_expires_a_lease_into_a_retry_then_a_failure() {
        let mut m = SagaModule::new("saga");
        let mut ctx = CaptureCtx::new();
        exec(
            &mut m,
            &mut ctx,
            &msg(&SagaMsg::Trigger {
                pinned_assignee: None,
                saga_id: "s1".into(),
                spec: b"w".to_vec(),
                reply_to: None,
                reply_payload: Vec::new(),
                deadline: None,
                max_attempts: 2,
                lease_views: Some(5),
                capability: None,
            }),
        )
        .unwrap();
        commit(&mut m);
        assert_eq!(get(&m, "s1").unwrap().lease_expires_at, Some(5));

        // first expiry: attempts remain, so the crank re-leases and re-asks
        // the worker under attempt 1.
        let mut ctx = CaptureCtx::new().at(5);
        exec(&mut m, &mut ctx, &crank()).unwrap();
        commit(&mut m);
        let v = get(&m, "s1").unwrap();
        assert_eq!(v.status, SagaStatus::Pending);
        assert_eq!(v.attempt, 1);
        assert_eq!(
            v.lease_expires_at,
            Some(10),
            "the new lease reuses the trigger's window"
        );
        let requests = ctx.worker_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].attempt, 1);

        // second expiry: no attempts remain — terminally Failed.
        let mut ctx = CaptureCtx::new().at(10);
        exec(&mut m, &mut ctx, &crank()).unwrap();
        commit(&mut m);
        let v = get(&m, "s1").unwrap();
        assert_eq!(v.status, SagaStatus::Failed);
        assert_eq!(v.error, Some("lease attempts exhausted".to_string()));
        assert!(ctx.effects.is_empty());
    }

    #[test]
    fn assignee_renews_and_requester_reassigns_with_attempt_fencing() {
        let validators = vec![b"node-a".to_vec(), b"node-b".to_vec()];
        let mut m = SagaModule::with_valset("saga", "valset", LeasePolicy::Strict);
        let mut ctx = CaptureCtx::new()
            .with_origin(Origin::Module("dispatch".into()))
            .with_validators(validators.clone());
        exec(
            &mut m,
            &mut ctx,
            &msg(&SagaMsg::Trigger {
                saga_id: "s1".into(),
                spec: b"w".to_vec(),
                reply_to: None,
                reply_payload: Vec::new(),
                deadline: Some(100),
                max_attempts: 3,
                lease_views: Some(10),
                capability: None,
                pinned_assignee: None,
            }),
        )
        .unwrap();
        commit(&mut m);
        let first = get(&m, "s1").unwrap().assignee.unwrap();

        let mut ctx = CaptureCtx::new()
            .at(4)
            .with_origin(Origin::External(first.clone()))
            .with_validators(validators.clone());
        exec(
            &mut m,
            &mut ctx,
            &msg(&SagaMsg::RenewLease {
                saga_id: "s1".into(),
                attempt: 0,
            }),
        )
        .unwrap();
        commit(&mut m);
        let view = get(&m, "s1").unwrap();
        assert_eq!(view.lease_expires_at, Some(10));
        assert_eq!(view.updated_at, 4, "every valid heartbeat is observable");

        let mut ctx = CaptureCtx::new()
            .at(5)
            .with_origin(Origin::External(first.clone()))
            .with_validators(validators.clone());
        exec(
            &mut m,
            &mut ctx,
            &msg(&SagaMsg::RenewLease {
                saga_id: "s1".into(),
                attempt: 0,
            }),
        )
        .unwrap();
        commit(&mut m);
        assert_eq!(get(&m, "s1").unwrap().lease_expires_at, Some(15));

        let before = m.root();
        let mut ctx = CaptureCtx::new()
            .at(6)
            .with_origin(Origin::External(first.clone()))
            .with_validators(validators.clone());
        exec(
            &mut m,
            &mut ctx,
            &msg(&SagaMsg::Reassign {
                saga_id: "s1".into(),
                attempt: 0,
            }),
        )
        .unwrap();
        commit(&mut m);
        assert_eq!(m.root(), before, "the assignee cannot reassign itself");

        let mut ctx = CaptureCtx::new()
            .at(7)
            .with_origin(Origin::Module("dispatch".into()))
            .with_validators(validators.clone());
        exec(
            &mut m,
            &mut ctx,
            &msg(&SagaMsg::Reassign {
                saga_id: "s1".into(),
                attempt: 0,
            }),
        )
        .unwrap();
        commit(&mut m);
        let view = get(&m, "s1").unwrap();
        assert_eq!(view.attempt, 1);
        assert_ne!(view.assignee.as_deref(), Some(first.as_slice()));
        assert_eq!(ctx.worker_requests()[0].attempt, 1);

        let fenced_root = m.root();
        let mut ctx = CaptureCtx::new()
            .at(8)
            .with_origin(Origin::External(first))
            .with_validators(validators);
        exec(&mut m, &mut ctx, &oracle("s1", 0, Ok(b"stale".to_vec()))).unwrap();
        commit(&mut m);
        assert_eq!(m.root(), fenced_root, "the revoked attempt cannot finish");
    }

    #[test]
    fn crank_budget_bounds_one_sweep_and_the_next_crank_finishes() {
        let mut m = SagaModule::new("saga");
        let mut ctx = CaptureCtx::new();
        // 33 sagas, every one past its deadline at view 10. zero-padded ids
        // pin the sweep order.
        for i in 0..33 {
            exec(
                &mut m,
                &mut ctx,
                &msg(&SagaMsg::Trigger {
                    pinned_assignee: None,
                    saga_id: format!("s{i:02}"),
                    spec: Vec::new(),
                    reply_to: None,
                    reply_payload: Vec::new(),
                    deadline: Some(10),
                    max_attempts: 1,
                    lease_views: None,
                    capability: None,
                }),
            )
            .unwrap();
        }
        commit(&mut m);

        // one crank transitions exactly CRANK_BUDGET sagas, in id order — the
        // 33rd (lexicographically last) is still pending.
        let mut ctx = CaptureCtx::new().at(10);
        exec(&mut m, &mut ctx, &crank()).unwrap();
        commit(&mut m);
        let timed_out = (0..33)
            .filter(|i| get(&m, &format!("s{i:02}")).unwrap().status == SagaStatus::TimedOut)
            .count();
        assert_eq!(
            timed_out as u32, CRANK_BUDGET,
            "one crank does exactly its budget"
        );
        assert_eq!(
            get(&m, "s32").unwrap().status,
            SagaStatus::Pending,
            "the overflow saga waits"
        );

        // the next crank finishes the backlog.
        let mut ctx = CaptureCtx::new().at(11);
        exec(&mut m, &mut ctx, &crank()).unwrap();
        commit(&mut m);
        assert_eq!(get(&m, "s32").unwrap().status, SagaStatus::TimedOut);
    }

    #[test]
    fn cancel_is_gated_to_the_trigger_origin() {
        let alice = Origin::External(b"alice".to_vec());
        let mallory = Origin::External(b"mallory".to_vec());

        let mut m = SagaModule::new("saga");
        let mut ctx = CaptureCtx::new()
            .with_origin(alice.clone())
            .knowing("agent");
        exec(
            &mut m,
            &mut ctx,
            &msg(&SagaMsg::Trigger {
                pinned_assignee: None,
                saga_id: "s1".into(),
                spec: Vec::new(),
                reply_to: Some("agent".into()),
                reply_payload: Vec::new(),
                deadline: None,
                max_attempts: 1,
                lease_views: None,
                capability: None,
            }),
        )
        .unwrap();
        commit(&mut m);
        let pending_root = m.root();

        // a FOREIGN cancel is a no-op, not an error — a finalized foreign
        // cancel must not abort blocks.
        let mut ctx = CaptureCtx::new().with_origin(mallory).knowing("agent");
        exec(
            &mut m,
            &mut ctx,
            &msg(&SagaMsg::Cancel {
                saga_id: "s1".into(),
            }),
        )
        .unwrap();
        commit(&mut m);
        assert_eq!(m.root(), pending_root, "a foreign cancel is a no-op");
        assert_eq!(get(&m, "s1").unwrap().status, SagaStatus::Pending);

        // the trigger origin cancels: terminal + callback.
        let mut ctx = CaptureCtx::new()
            .at(9)
            .with_origin(alice.clone())
            .knowing("agent");
        exec(
            &mut m,
            &mut ctx,
            &msg(&SagaMsg::Cancel {
                saga_id: "s1".into(),
            }),
        )
        .unwrap();
        commit(&mut m);
        assert_eq!(get(&m, "s1").unwrap().status, SagaStatus::Cancelled);
        assert_eq!(
            ctx.callbacks(),
            vec![SagaCallback {
                saga_id: "s1".into(),
                payload: Vec::new(),
                outcome: SagaOutcome::Cancelled,
            }]
        );
        let cancelled_root = m.root();

        // cancelling a TERMINAL saga (and an unknown one) is a no-op.
        let mut ctx = CaptureCtx::new().with_origin(alice).knowing("agent");
        exec(
            &mut m,
            &mut ctx,
            &msg(&SagaMsg::Cancel {
                saga_id: "s1".into(),
            }),
        )
        .unwrap();
        exec(
            &mut m,
            &mut ctx,
            &msg(&SagaMsg::Cancel {
                saga_id: "ghost".into(),
            }),
        )
        .unwrap();
        commit(&mut m);
        assert_eq!(m.root(), cancelled_root);
        assert!(ctx.msgs.is_empty(), "no second callback");
    }

    #[test]
    fn prune_removes_terminal_sagas_only_and_is_origin_gated() {
        let alice = Origin::External(b"alice".to_vec());
        let mallory = Origin::External(b"mallory".to_vec());

        let mut m = SagaModule::new("saga");
        // "done" and "open" belong to alice; "theirs" to mallory.
        let mut ctx = CaptureCtx::new().with_origin(alice.clone());
        exec(&mut m, &mut ctx, &trigger("done", b"a")).unwrap();
        exec(&mut m, &mut ctx, &trigger("open", b"b")).unwrap();
        let mut ctx = CaptureCtx::new().with_origin(mallory);
        exec(&mut m, &mut ctx, &trigger("theirs", b"c")).unwrap();
        commit(&mut m);
        let mut ctx = CaptureCtx::new();
        exec(&mut m, &mut ctx, &oracle("done", 0, Ok(b"r".to_vec()))).unwrap();
        exec(&mut m, &mut ctx, &oracle("theirs", 0, Ok(b"r".to_vec()))).unwrap();
        commit(&mut m);

        // alice prunes everything she can name: only HER TERMINAL saga goes.
        // "open" (non-terminal), "theirs" (foreign), "ghost" (unknown) are
        // skipped as no-ops.
        let mut ctx = CaptureCtx::new().with_origin(alice);
        exec(
            &mut m,
            &mut ctx,
            &msg(&SagaMsg::Prune {
                saga_ids: vec![
                    "done".into(),
                    "open".into(),
                    "theirs".into(),
                    "ghost".into(),
                ],
            }),
        )
        .unwrap();
        commit(&mut m);
        assert_eq!(get(&m, "done"), None, "own terminal saga pruned");
        assert_eq!(
            get(&m, "open").unwrap().status,
            SagaStatus::Pending,
            "non-terminal survives"
        );
        assert_eq!(
            get(&m, "theirs").unwrap().status,
            SagaStatus::Done,
            "foreign survives"
        );

        // a pruned id may be re-triggered: GC really removed it.
        let mut ctx = CaptureCtx::new();
        exec(&mut m, &mut ctx, &trigger("done", b"again")).unwrap();
        assert_eq!(ctx.effects.len(), 1, "a pruned id triggers as new work");
    }

    #[test]
    fn open_policy_with_valset_assigns_but_accepts_any_submitter() {
        let validators = vec![vec![1u8; 32], vec![2u8; 32], vec![3u8; 32]];
        let mut m = SagaModule::with_valset("saga", "valset", LeasePolicy::Open);
        let mut ctx = CaptureCtx::new().at(4).with_validators(validators.clone());
        exec(&mut m, &mut ctx, &trigger("s1", b"w")).unwrap();
        commit(&mut m);

        // the trigger assigned a lease-holder from the set with the default
        // window, and advertised it in the WorkerRequest.
        let v = get(&m, "s1").unwrap();
        let assignee = v.assignee.clone().expect("an assignee was computed");
        assert!(
            validators.contains(&assignee),
            "assignee comes from the validator set"
        );
        assert_eq!(v.lease_expires_at, Some(4 + DEFAULT_LEASE_VIEWS));
        assert_eq!(ctx.worker_requests()[0].assignee, Some(assignee.clone()));

        // open policy: a NON-assignee's result still lands.
        let outsider = Origin::External(b"outsider".to_vec());
        let mut ctx = CaptureCtx::new()
            .with_origin(outsider)
            .with_validators(validators);
        exec(&mut m, &mut ctx, &oracle("s1", 0, Ok(b"r".to_vec()))).unwrap();
        commit(&mut m);
        assert_eq!(get(&m, "s1").unwrap().status, SagaStatus::Done);
    }

    #[test]
    fn strict_policy_gates_results_to_the_assignee() {
        let validators = vec![vec![1u8; 32], vec![2u8; 32], vec![3u8; 32]];
        let mut m = SagaModule::with_valset("saga", "valset", LeasePolicy::Strict);
        let mut ctx = CaptureCtx::new().with_validators(validators.clone());
        exec(&mut m, &mut ctx, &trigger("s1", b"w")).unwrap();
        commit(&mut m);
        let assignee = get(&m, "s1").unwrap().assignee.expect("assigned");
        let non_assignee = validators
            .iter()
            .find(|v| **v != assignee)
            .expect("another validator")
            .clone();
        let pending_root = m.root();

        // a non-assignee result is a deterministic no-op under strict.
        let mut ctx = CaptureCtx::new()
            .with_origin(Origin::External(non_assignee))
            .with_validators(validators.clone());
        exec(&mut m, &mut ctx, &oracle("s1", 0, Ok(b"intruder".to_vec()))).unwrap();
        commit(&mut m);
        assert_eq!(
            m.root(),
            pending_root,
            "a non-assignee result is a no-op under strict"
        );
        assert_eq!(get(&m, "s1").unwrap().status, SagaStatus::Pending);

        // the assignee's result lands.
        let mut ctx = CaptureCtx::new()
            .with_origin(Origin::External(assignee))
            .with_validators(validators);
        exec(&mut m, &mut ctx, &oracle("s1", 0, Ok(b"legit".to_vec()))).unwrap();
        commit(&mut m);
        let v = get(&m, "s1").unwrap();
        assert_eq!(v.status, SagaStatus::Done);
        assert_eq!(v.result, Some(b"legit".to_vec()));
    }

    #[test]
    fn strict_unassigned_attempts_are_announcements_claimed_by_accept() {
        // valset configured but EMPTY: assignee is None. under strict the
        // emitted request is an ANNOUNCEMENT — no result lands until a node
        // claims the attempt, first accept in consensus order wins, and only
        // the winner's result counts.
        let mut m = SagaModule::with_valset("saga", "valset", LeasePolicy::Strict);
        let mut ctx = CaptureCtx::new().with_validators(Vec::new());
        exec(&mut m, &mut ctx, &trigger("s1", b"w")).unwrap();
        commit(&mut m);
        let v = get(&m, "s1").unwrap();
        assert_eq!(v.assignee, None, "an empty set assigns no one");
        assert_eq!(
            v.lease_expires_at, None,
            "no assignee and no window -> no lease"
        );

        // an unclaimed result is a no-op — the accept-any hole is closed.
        let pending_root = m.root();
        let mut ctx = CaptureCtx::new()
            .with_origin(Origin::External(b"anyone".to_vec()))
            .with_validators(Vec::new());
        exec(&mut m, &mut ctx, &oracle("s1", 0, Ok(b"r".to_vec()))).unwrap();
        commit(&mut m);
        assert_eq!(m.root(), pending_root, "no result lands unclaimed");
        assert_eq!(get(&m, "s1").unwrap().status, SagaStatus::Pending);

        // the FIRST accept claims the attempt: assignee + lease + the actual
        // work order re-emitted naming the winner.
        let mut ctx = CaptureCtx::new()
            .at(7)
            .with_origin(Origin::External(b"node-a".to_vec()))
            .with_validators(Vec::new());
        exec(
            &mut m,
            &mut ctx,
            &msg(&SagaMsg::Accept {
                saga_id: "s1".into(),
                attempt: 0,
            }),
        )
        .unwrap();
        let requests = ctx.worker_requests();
        assert_eq!(requests.len(), 1, "the accept re-emits the work order");
        assert_eq!(requests[0].assignee, Some(b"node-a".to_vec()));
        assert_eq!(requests[0].attempt, 0);
        commit(&mut m);
        let v = get(&m, "s1").unwrap();
        assert_eq!(v.assignee, Some(b"node-a".to_vec()));
        assert_eq!(
            v.lease_expires_at,
            Some(7 + DEFAULT_LEASE_VIEWS),
            "the claim starts the lease clock"
        );

        // a late accept loses quietly: nothing staged, no second work order.
        let claimed_root = m.root();
        let mut ctx = CaptureCtx::new()
            .with_origin(Origin::External(b"node-b".to_vec()))
            .with_validators(Vec::new());
        exec(
            &mut m,
            &mut ctx,
            &msg(&SagaMsg::Accept {
                saga_id: "s1".into(),
                attempt: 0,
            }),
        )
        .unwrap();
        assert!(ctx.worker_requests().is_empty(), "a late accept is a no-op");
        commit(&mut m);
        assert_eq!(m.root(), claimed_root);

        // the loser's result is a no-op; the winner's lands.
        let mut ctx = CaptureCtx::new()
            .with_origin(Origin::External(b"node-b".to_vec()))
            .with_validators(Vec::new());
        exec(&mut m, &mut ctx, &oracle("s1", 0, Ok(b"stolen".to_vec()))).unwrap();
        commit(&mut m);
        assert_eq!(get(&m, "s1").unwrap().status, SagaStatus::Pending);
        let mut ctx = CaptureCtx::new()
            .with_origin(Origin::External(b"node-a".to_vec()))
            .with_validators(Vec::new());
        exec(&mut m, &mut ctx, &oracle("s1", 0, Ok(b"legit".to_vec()))).unwrap();
        commit(&mut m);
        let v = get(&m, "s1").unwrap();
        assert_eq!(v.status, SagaStatus::Done);
        assert_eq!(v.result, Some(b"legit".to_vec()));
    }

    #[test]
    fn accept_rejects_bad_origins_and_no_ops_on_assigned_or_stale_targets() {
        let validators = vec![vec![1u8; 32]];
        let mut m = SagaModule::with_valset("saga", "valset", LeasePolicy::Strict);
        let mut ctx = CaptureCtx::new().with_validators(validators.clone());
        exec(&mut m, &mut ctx, &trigger("assigned", b"w")).unwrap();
        commit(&mut m);
        assert_eq!(
            get(&m, "assigned").unwrap().assignee,
            Some(validators[0].clone()),
            "a one-node pool rendezvous-assigns that node"
        );

        // module / system / empty-key origins have no claim surface.
        for origin in [
            Origin::Module("dispatch".into()),
            Origin::System,
            Origin::External(Vec::new()),
        ] {
            let mut ctx = CaptureCtx::new()
                .with_origin(origin)
                .with_validators(validators.clone());
            assert!(
                exec(
                    &mut m,
                    &mut ctx,
                    &msg(&SagaMsg::Accept {
                        saga_id: "assigned".into(),
                        attempt: 0,
                    }),
                )
                .is_err()
            );
            block_on(m.abort_block()).unwrap();
        }

        // an already-assigned attempt, an unknown saga, and a stale attempt
        // are all quiet no-ops.
        let before = m.root();
        for (saga_id, attempt) in [("assigned", 0u32), ("ghost", 0), ("assigned", 9)] {
            let mut ctx = CaptureCtx::new()
                .with_origin(Origin::External(b"node-x".to_vec()))
                .with_validators(validators.clone());
            exec(
                &mut m,
                &mut ctx,
                &msg(&SagaMsg::Accept {
                    saga_id: saga_id.into(),
                    attempt,
                }),
            )
            .unwrap();
            assert!(ctx.worker_requests().is_empty(), "{saga_id}/{attempt}");
            commit(&mut m);
            assert_eq!(m.root(), before, "{saga_id}/{attempt} staged nothing");
        }
    }

    /// a trigger that names a capability; assignment must draw from the
    /// capability registry's providers, never the valset.
    fn capability_trigger(id: &str, tag: &str) -> Msg {
        msg(&SagaMsg::Trigger {
            pinned_assignee: None,
            saga_id: id.into(),
            spec: b"w".to_vec(),
            reply_to: None,
            reply_payload: Vec::new(),
            deadline: None,
            max_attempts: 1,
            lease_views: None,
            capability: Some(tag.into()),
        })
    }

    #[test]
    fn capability_tagged_sagas_assign_over_providers_not_the_valset() {
        let validators = vec![vec![1u8; 32], vec![2u8; 32], vec![3u8; 32]];
        // the sole provider is DISJOINT from the valset, so any valset leak
        // in pool selection fails the assertion.
        let provider = vec![9u8; 32];
        let mut m =
            SagaModule::with_assignment("saga", "valset", "capability", LeasePolicy::Strict);
        let mut ctx = CaptureCtx::new()
            .at(4)
            .with_validators(validators.clone())
            .with_providers(vec![provider.clone()]);
        exec(&mut m, &mut ctx, &capability_trigger("s1", "alpha")).unwrap();
        commit(&mut m);

        let v = get(&m, "s1").unwrap();
        assert_eq!(v.capability.as_deref(), Some("alpha"));
        assert_eq!(
            v.assignee,
            Some(provider.clone()),
            "the provider pool decides the lease holder"
        );
        assert_eq!(v.lease_expires_at, Some(4 + DEFAULT_LEASE_VIEWS));
        assert_eq!(ctx.worker_requests()[0].assignee, Some(provider.clone()));

        // strict: a validator that is NOT a provider cannot land the result...
        let pending_root = m.root();
        let mut ctx = CaptureCtx::new()
            .with_origin(Origin::External(validators[0].clone()))
            .with_validators(validators.clone())
            .with_providers(vec![provider.clone()]);
        exec(&mut m, &mut ctx, &oracle("s1", 0, Ok(b"intruder".to_vec()))).unwrap();
        commit(&mut m);
        assert_eq!(m.root(), pending_root, "a non-provider result is a no-op");

        // ... the provider can.
        let mut ctx = CaptureCtx::new()
            .with_origin(Origin::External(provider))
            .with_validators(validators)
            .with_providers(vec![vec![9u8; 32]]);
        exec(&mut m, &mut ctx, &oracle("s1", 0, Ok(b"legit".to_vec()))).unwrap();
        commit(&mut m);
        let v = get(&m, "s1").unwrap();
        assert_eq!(v.status, SagaStatus::Done);
        assert_eq!(v.result, Some(b"legit".to_vec()));
    }

    #[test]
    fn a_capability_nobody_provides_assigns_nobody_and_waits_for_a_claim() {
        let mut m =
            SagaModule::with_assignment("saga", "valset", "capability", LeasePolicy::Strict);
        let mut ctx = CaptureCtx::new()
            .with_validators(vec![vec![1u8; 32]])
            .with_providers(Vec::new());
        exec(&mut m, &mut ctx, &capability_trigger("s1", "alpha")).unwrap();
        commit(&mut m);
        assert_eq!(
            get(&m, "s1").unwrap().assignee,
            None,
            "no providers -> no assignee (the valset is NOT a fallback pool)"
        );

        // unclaimed: no result lands under strict.
        let mut ctx = CaptureCtx::new()
            .with_origin(Origin::External(b"anyone".to_vec()))
            .with_validators(vec![vec![1u8; 32]])
            .with_providers(Vec::new());
        exec(&mut m, &mut ctx, &oracle("s1", 0, Ok(b"r".to_vec()))).unwrap();
        commit(&mut m);
        assert_eq!(get(&m, "s1").unwrap().status, SagaStatus::Pending);

        // a node that CAN run the capability claims it, then its result lands.
        let mut ctx = CaptureCtx::new()
            .with_origin(Origin::External(b"provider".to_vec()))
            .with_validators(vec![vec![1u8; 32]])
            .with_providers(Vec::new());
        exec(
            &mut m,
            &mut ctx,
            &msg(&SagaMsg::Accept {
                saga_id: "s1".into(),
                attempt: 0,
            }),
        )
        .unwrap();
        commit(&mut m);
        let mut ctx = CaptureCtx::new()
            .with_origin(Origin::External(b"provider".to_vec()))
            .with_validators(vec![vec![1u8; 32]])
            .with_providers(Vec::new());
        exec(&mut m, &mut ctx, &oracle("s1", 0, Ok(b"r".to_vec()))).unwrap();
        commit(&mut m);
        assert_eq!(get(&m, "s1").unwrap().status, SagaStatus::Done);
    }

    #[test]
    fn untagged_sagas_keep_valset_assignment_under_with_assignment() {
        let validators = vec![vec![1u8; 32], vec![2u8; 32]];
        let mut m = SagaModule::with_assignment("saga", "valset", "capability", LeasePolicy::Open);
        let mut ctx = CaptureCtx::new()
            .with_validators(validators.clone())
            .with_providers(vec![vec![9u8; 32]]);
        exec(&mut m, &mut ctx, &trigger("s1", b"w")).unwrap();
        commit(&mut m);
        let assignee = get(&m, "s1").unwrap().assignee.expect("assigned");
        assert!(
            validators.contains(&assignee),
            "untagged work stays on the valset"
        );
    }

    #[test]
    fn a_pinned_trigger_leases_every_attempt_to_the_pinned_key() {
        // the pinned key is disjoint from the valset AND the provider pool,
        // so any rendezvous leak in assignment fails the assertions.
        let pinned = vec![7u8; 32];
        let mut m =
            SagaModule::with_assignment("saga", "valset", "capability", LeasePolicy::Strict);
        let mut ctx = CaptureCtx::new()
            .at(4)
            .with_validators(vec![vec![1u8; 32]])
            .with_providers(vec![vec![9u8; 32]]);
        exec(
            &mut m,
            &mut ctx,
            &msg(&SagaMsg::Trigger {
                saga_id: "s1".into(),
                spec: b"w".to_vec(),
                reply_to: None,
                reply_payload: Vec::new(),
                deadline: None,
                max_attempts: 2,
                lease_views: None,
                capability: Some("alpha".into()),
                pinned_assignee: Some(pinned.clone()),
            }),
        )
        .unwrap();
        commit(&mut m);

        let v = get(&m, "s1").unwrap();
        assert_eq!(v.assignee, Some(pinned.clone()), "attempt 0 leases pinned");
        assert_eq!(v.pinned_assignee, Some(pinned.clone()));
        assert_eq!(v.lease_expires_at, Some(4 + DEFAULT_LEASE_VIEWS));

        // strict: the announced provider does NOT hold this lease...
        let pending_root = m.root();
        let mut ctx = CaptureCtx::new().with_origin(Origin::External(vec![9u8; 32]));
        exec(&mut m, &mut ctx, &oracle("s1", 0, Ok(b"foreign".to_vec()))).unwrap();
        commit(&mut m);
        assert_eq!(m.root(), pending_root, "a non-pinned result is a no-op");

        // ... and the pinned key's Err consumes the attempt: the RETRY is
        // leased to the pinned key again, never rendezvous-reassigned.
        let mut ctx = CaptureCtx::new()
            .at(5)
            .with_origin(Origin::External(pinned.clone()))
            .with_validators(vec![vec![1u8; 32]])
            .with_providers(vec![vec![9u8; 32]]);
        exec(&mut m, &mut ctx, &oracle("s1", 0, Err("transient".into()))).unwrap();
        commit(&mut m);
        let v = get(&m, "s1").unwrap();
        assert_eq!(v.attempt, 1);
        assert_eq!(v.assignee, Some(pinned.clone()), "the retry stays pinned");
        assert_eq!(ctx.worker_requests()[0].assignee, Some(pinned));
    }

    #[test]
    fn an_empty_pinned_assignee_is_rejected() {
        let mut m = SagaModule::new("saga");
        let mut ctx = CaptureCtx::new();
        let err = exec(
            &mut m,
            &mut ctx,
            &msg(&SagaMsg::Trigger {
                saga_id: "s1".into(),
                spec: Vec::new(),
                reply_to: None,
                reply_payload: Vec::new(),
                deadline: None,
                max_attempts: 1,
                lease_views: None,
                capability: None,
                pinned_assignee: Some(Vec::new()),
            }),
        )
        .unwrap_err();
        assert!(err.to_string().contains("pinned_assignee"), "got: {err}");
    }

    #[test]
    fn empty_and_oversized_capability_tags_are_rejected() {
        let mut m = SagaModule::new("saga");
        let mut ctx = CaptureCtx::new();
        let oversized = "x".repeat(MAX_CAPABILITY_BYTES + 1);
        for bad in ["", oversized.as_str()] {
            let err = exec(
                &mut m,
                &mut ctx,
                &msg(&SagaMsg::Trigger {
                    pinned_assignee: None,
                    saga_id: "s1".into(),
                    spec: Vec::new(),
                    reply_to: None,
                    reply_payload: Vec::new(),
                    deadline: None,
                    max_attempts: 1,
                    lease_views: None,
                    capability: Some(bad.to_string()),
                }),
            )
            .unwrap_err();
            assert!(matches!(err, Error::Module(_)), "got {err:?} for {bad:?}");
        }
        assert!(ctx.effects.is_empty(), "rejected triggers fire no worker");
        assert_eq!(get(&m, "s1"), None, "nothing was staged");
    }

    #[test]
    fn next_expiry_reports_the_earliest_pending_expiry() {
        let mut m = SagaModule::new("saga");
        assert_eq!(next_expiry(&m), None, "an empty ledger has no expiry");

        let mut ctx = CaptureCtx::new();
        // a deadline at 50, a lease at 7, and one saga with neither.
        exec(
            &mut m,
            &mut ctx,
            &msg(&SagaMsg::Trigger {
                pinned_assignee: None,
                saga_id: "a".into(),
                spec: Vec::new(),
                reply_to: None,
                reply_payload: Vec::new(),
                deadline: Some(50),
                max_attempts: 1,
                lease_views: None,
                capability: None,
            }),
        )
        .unwrap();
        exec(
            &mut m,
            &mut ctx,
            &msg(&SagaMsg::Trigger {
                pinned_assignee: None,
                saga_id: "b".into(),
                spec: Vec::new(),
                reply_to: None,
                reply_payload: Vec::new(),
                deadline: None,
                max_attempts: 1,
                lease_views: Some(7),
                capability: None,
            }),
        )
        .unwrap();
        exec(&mut m, &mut ctx, &trigger("c", b"w")).unwrap();
        commit(&mut m);
        assert_eq!(next_expiry(&m), Some(7), "the lease at view 7 is earliest");

        // resolving the leased saga drops it out; the deadline remains.
        let mut ctx = CaptureCtx::new();
        exec(&mut m, &mut ctx, &oracle("b", 0, Ok(b"r".to_vec()))).unwrap();
        commit(&mut m);
        assert_eq!(next_expiry(&m), Some(50), "terminal sagas carry no expiry");
    }

    #[test]
    fn assigned_pending_projects_own_leases_as_worker_requests() {
        // the resident worker pump's read: a capability-tagged saga leased to
        // `me` surfaces as exactly the WorkerRequest the effect carried;
        // other keys see nothing, and a landed result retires it.
        let me = b"resident-key".to_vec();
        let other = b"someone-else".to_vec();
        let mut m =
            SagaModule::with_assignment("saga", "valset", "capability", LeasePolicy::Strict);
        assert!(
            assigned_pending(&m, &me).is_empty(),
            "an empty ledger assigns nothing"
        );

        // a single-provider pool makes the rendezvous pick deterministic.
        let mut ctx = CaptureCtx::new().at(4).with_providers(vec![me.clone()]);
        exec(
            &mut m,
            &mut ctx,
            &msg(&SagaMsg::Trigger {
                pinned_assignee: None,
                saga_id: "job".into(),
                spec: b"the work spec".to_vec(),
                reply_to: None,
                reply_payload: Vec::new(),
                deadline: Some(90),
                max_attempts: 3,
                lease_views: Some(10),
                capability: Some("codex".into()),
            }),
        )
        .unwrap();
        commit(&mut m);

        // the projection IS the effect's work order, field for field.
        let emitted = ctx.worker_requests();
        assert_eq!(emitted.len(), 1, "the trigger emitted one request");
        assert_eq!(
            assigned_pending(&m, &me),
            emitted,
            "the state projection matches the effect lane's request"
        );
        assert!(
            assigned_pending(&m, &other).is_empty(),
            "another key's read excludes foreign leases"
        );

        // the assignee's result settles the saga: nothing pending remains.
        let mut ctx = CaptureCtx::new()
            .at(5)
            .with_origin(Origin::External(me.clone()));
        exec(&mut m, &mut ctx, &oracle("job", 0, Ok(b"done".to_vec()))).unwrap();
        commit(&mut m);
        assert!(
            assigned_pending(&m, &me).is_empty(),
            "a terminal saga is no longer assigned work"
        );
    }

    #[test]
    fn two_instances_replaying_one_script_land_on_byte_identical_roots() {
        // the determinism pin: the same op script, replayed on two fresh
        // instances, must produce byte-identical snapshots (and thus roots)
        // after every block.
        fn script() -> Vec<Vec<Msg>> {
            let alice = |saga_id: &str, max_attempts: u32, deadline: Option<u64>| {
                msg(&SagaMsg::Trigger {
                    pinned_assignee: None,
                    saga_id: saga_id.into(),
                    spec: b"spec".to_vec(),
                    reply_to: None,
                    reply_payload: b"corr".to_vec(),
                    deadline,
                    max_attempts,
                    lease_views: Some(3),
                    capability: None,
                })
            };
            vec![
                vec![
                    alice("a", 2, None),
                    alice("b", 1, Some(6)),
                    alice("c", 1, None),
                    // a capability-tagged saga: the tag rides the committed
                    // encoding, so it must replay byte-identically too.
                    msg(&SagaMsg::Trigger {
                        pinned_assignee: None,
                        saga_id: "d".into(),
                        spec: b"spec".to_vec(),
                        reply_to: None,
                        reply_payload: Vec::new(),
                        deadline: None,
                        max_attempts: 1,
                        lease_views: None,
                        capability: Some("alpha".into()),
                    }),
                ],
                vec![
                    oracle("a", 0, Err("retry me".into())),
                    oracle("c", 0, Ok(b"done".to_vec())),
                ],
                vec![crank()],
                vec![msg(&SagaMsg::Cancel {
                    saga_id: "a".into(),
                })],
                vec![msg(&SagaMsg::Prune {
                    saga_ids: vec!["a".into(), "b".into()],
                })],
            ]
        }

        let run = || {
            let mut m = SagaModule::new("saga");
            let mut roots = Vec::new();
            for (height, block) in script().into_iter().enumerate() {
                let mut ctx = CaptureCtx::new()
                    .at(height as u64 * 10)
                    .with_origin(Origin::External(b"alice".to_vec()));
                for op in &block {
                    exec(&mut m, &mut ctx, op).unwrap();
                }
                commit(&mut m);
                roots.push(m.root());
            }
            (roots, m.snapshot())
        };

        let (roots_a, snapshot_a) = run();
        let (roots_b, snapshot_b) = run();
        assert_eq!(roots_a, roots_b, "identical roots after every block");
        assert_eq!(snapshot_a, snapshot_b, "byte-identical final snapshots");
    }
}
