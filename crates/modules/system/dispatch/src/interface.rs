//! the dispatch module's public wire surface — types only.
//!
//! dispatch is the network's task plane: a [`Recipe`] is a registered
//! what-to-run manifest (required capability, routing mode, output contract),
//! and a [`DispatchMsg::Dispatch`] runs one under it, carrying the entire
//! prompt/input as opaque payload data. HOW an executor runs is a host-side
//! capability spec; WHAT ran, on whose behalf, and what came back is
//! consensus state here. no prompt text, no executor name, and no domain
//! vocabulary (chat, tasks, …) exists in this surface — the module is
//! deliberately 100% self-contained.
//!
//! ## the never-pop-stack rule
//!
//! a dispatch result is never returned into the requester's call path. the
//! worker's result lands as an ordered op; the dispatch module validates it
//! against the recipe's [`OutputContract`] and stages a [`ResultEvent`] into
//! its mailbox; the host injects a System-origin [`DispatchMsg::DeliverPending`]
//! at the start of a LATER block's drain, and only that dispatch emits the
//! event to the receiver. the receiver consumes the result in its own block,
//! its own failure domain — at least one block after the result committed.

use saga::SagaOrigin;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// the conventional module id dispatch registers under — shared by the host's
/// delivery injection and node wiring.
pub const DEFAULT_DISPATCH_TARGET: &str = "dispatch";

// ---- consensus constants ----------------------------------------------------

/// hard cap on one dispatch's inline payload. the payload rides the saga work
/// spec (module-origin derived state, never a wire message), so this bounds
/// consensus state while a dispatch is pending — saga's `MAX_SPEC_BYTES` is
/// sized above this plus the [`WorkSpec`] envelope.
pub const MAX_PAYLOAD_BYTES: usize = 10 * 1024 * 1024;

/// hard cap on an accepted result — saga's own result cap, restated here so
/// contract validation and the mailbox agree with what saga can carry.
pub const MAX_RESULT_BYTES: usize = saga::MAX_RESULT_BYTES;

/// Exact successful result for a fail-fast attempt that finds occupied capacity.
/// Kept on the shared wire surface so host producers and consensus receivers
/// compare the same bytes.
pub const RESOURCE_UNAVAILABLE_RESULT: &[u8] = br#"{"code":"RESOURCE_UNAVAILABLE"}"#;

/// hard cap on a recipe / dispatch id.
pub const MAX_ID_BYTES: usize = 128;

/// hard cap on a recipe's human-facing description.
pub const MAX_DESCRIPTION_BYTES: usize = 1024;

/// mailbox events delivered per block. bounds the injected delivery
/// dispatch's fan-out so a full mailbox can never blow the host's dispatch
/// budget and poison every subsequent block; the remainder stays pending and
/// the host re-injects next block.
pub const MAX_DELIVERIES_PER_BLOCK: usize = 32;

/// the self-description [`WorkSpec::kind`] must carry — how a host worker
/// recognizes dispatch work without ever guessing at foreign spec bytes.
pub const WORK_SPEC_KIND: &str = "dispatch-work-v1";

// ---- the recipe manifest ------------------------------------------------------

/// where a recipe's runs are assigned.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Routing {
    /// each attempt is rendezvous-assigned over the capability's announced
    /// providers (saga's default capability assignment).
    Rendezvous,
    /// static binding: every attempt leases to exactly this node key.
    Pinned(Vec<u8>),
}

/// the recipe's promise about what a run's output looks like — validated
/// DETERMINISTICALLY by the dispatch module before any delivery. a closed set
/// on purpose: each name is a checkable rule, not a config-described guess.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutputContract {
    /// any byte string (size-capped). the receiver interprets.
    Text,
    /// the output must parse as one JSON value.
    Json,
}

/// one registered what-to-run manifest — an ordered-op registration, so which
/// capability and contract a recipe binds is part of the app-hash. `owner` is
/// the registration origin and gates every mutation.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Recipe {
    pub recipe_id: String,
    pub owner: SagaOrigin,
    pub description: String,
    /// the capability registry tag runs are dispatched on.
    pub capability: String,
    pub routing: Routing,
    pub output_contract: OutputContract,
    /// total saga attempts per dispatch (>= 1).
    pub max_attempts: u32,
    /// optional whole-run deadline, in views RELATIVE to the dispatching
    /// block; turned absolute at dispatch time.
    pub deadline_views: Option<u64>,
    /// optional per-attempt lease window, passed through to the saga.
    pub lease_views: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

// ---- runs ----------------------------------------------------------------------

/// where a dispatch is in its lifecycle. every transition is an ordered op;
/// `Delivered` is terminal.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DispatchStatus {
    /// the saga carrying the work, awaited for its callback.
    AwaitingResult { saga_id: String },
    /// outcome recorded and contract-checked, sitting in the mailbox for the
    /// host's next-block delivery injection.
    AwaitingDelivery,
    /// the [`ResultEvent`] was emitted to the receiver.
    Delivered,
}

/// a dispatch's observable state.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DispatchView {
    pub dispatch_id: String,
    pub recipe_id: String,
    /// the module the result event is delivered to — always the dispatching
    /// module (`Dispatch` is module-origin-only).
    pub receiver: String,
    pub status: DispatchStatus,
    /// the contract-checked outcome, present from `AwaitingDelivery` on.
    /// `Err` carries the saga failure or the contract violation.
    pub outcome: Option<Result<Vec<u8>, String>>,
    /// the node key currently holding the run's execution lease (the saga
    /// assignee), resolved at QUERY TIME by the read facade. `None` unless the
    /// dispatch is `AwaitingResult` — a delivered run runs nowhere. VIEW-ONLY:
    /// never committed state, never part of the app-hash.
    pub assignee: Option<Vec<u8>>,
    /// live saga lease metadata, populated only while awaiting a result.
    #[serde(default)]
    pub attempt: Option<u32>,
    #[serde(default)]
    pub max_attempts: Option<u32>,
    #[serde(default)]
    pub lease_expires_at: Option<u64>,
    #[serde(default)]
    pub deadline: Option<u64>,
    #[serde(default)]
    pub lease_updated_at: Option<u64>,
    #[serde(default)]
    pub reassignable: Option<bool>,
    pub created_at: u64,
    pub updated_at: u64,
}

/// the saga work spec a dispatch stages — what the host-side worker decodes.
/// `kind` is a fixed self-description ([`WORK_SPEC_KIND`]) so this spec and
/// foreign spec shapes can never cross-decode.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct WorkSpec {
    pub kind: String,
    pub dispatch_id: String,
    /// the capability tag the executing host resolves to a local provider.
    pub capability: String,
    /// the ENTIRE model/tool input, verbatim. composed by the dispatcher —
    /// never by host code, never from static text.
    pub payload: Vec<u8>,
    /// numeric resource demands, validated by `capability::validate_resources`
    /// at dispatch time; empty = demandless job. the same value the
    /// dispatch handler threads onto the emitted `SagaMsg::Trigger` — the host
    /// worker reads demands from here, saga stays spec-opaque.
    pub demands: BTreeMap<String, u64>,
    /// host-local resource admission behavior. Omitted specs queue.
    #[serde(default, skip_serializing_if = "AdmissionPolicy::is_queue")]
    pub admission: AdmissionPolicy,
}

/// Host-local admission behavior for an assigned dispatch attempt.
#[derive(Serialize, Deserialize, Debug, Default, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdmissionPolicy {
    /// Wait for currently occupied capacity — the default: wait for capacity.
    #[default]
    Queue,
    /// Attempt one atomic reservation and settle immediately when occupied.
    FailFast,
}

impl AdmissionPolicy {
    fn is_queue(&self) -> bool {
        *self == Self::Queue
    }
}

/// the delivery envelope a receiver module gets as a follow-up `Msg` from the
/// dispatch module, one block (or more) after the outcome committed.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ResultEvent {
    pub dispatch_id: String,
    pub recipe_id: String,
    /// `Ok` passed the recipe's output contract; `Err` is the saga failure
    /// (worker error, timeout, cancellation) or the contract violation.
    pub outcome: Result<Vec<u8>, String>,
}

// ---- ops -----------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DispatchMsg {
    /// register a recipe under the submitter's origin. a duplicate
    /// `recipe_id` is an error.
    RegisterRecipe {
        recipe_id: String,
        description: String,
        capability: String,
        routing: Routing,
        output_contract: OutputContract,
        max_attempts: u32,
        deadline_views: Option<u64>,
        lease_views: Option<u64>,
    },
    /// owner-gated partial update; `None` fields keep their current value.
    /// (clearing an optional field means re-registering.)
    UpdateRecipe {
        recipe_id: String,
        description: Option<String>,
        capability: Option<String>,
        routing: Option<Routing>,
        output_contract: Option<OutputContract>,
        max_attempts: Option<u32>,
    },
    /// owner-gated removal. in-flight dispatches under the recipe finish
    /// against the manifest values captured at dispatch time.
    RemoveRecipe { recipe_id: String },
    /// run `recipe_id` once over `payload`. MODULE-ORIGIN ONLY — the
    /// dispatching module is the receiver of the eventual [`ResultEvent`].
    /// a duplicate `dispatch_id` is a deterministic no-op (first wins).
    Dispatch {
        dispatch_id: String,
        recipe_id: String,
        payload: Vec<u8>,
        /// numeric resource demands, validated by
        /// `capability::validate_resources` at dispatch time; empty =
        /// demandless job. threaded verbatim onto both the composed
        /// `WorkSpec` and the emitted `SagaMsg::Trigger` — one source, so the
        /// two can never drift.
        demands: BTreeMap<String, u64>,
        /// host-local admission behavior; omitted callers retain Queue.
        #[serde(default, skip_serializing_if = "AdmissionPolicy::is_queue")]
        admission: AdmissionPolicy,
    },
    /// MODULE-ORIGIN ONLY, receiver-scoped: cancel an in-flight dispatch the
    /// emitting module owns. the underlying saga is cancelled in the same
    /// block; its terminal callback then flows the normal path, so the
    /// receiver still gets a [`ResultEvent`] (`Err`) via next-block delivery.
    /// unknown, foreign, and already-terminal dispatches are deterministic
    /// no-ops — cancellation is idempotent.
    CancelDispatch { dispatch_id: String },
    /// MODULE-ORIGIN ONLY, receiver-scoped: fence `attempt` and move the
    /// in-flight dispatch to a different provider.
    ReassignDispatch { dispatch_id: String, attempt: u32 },
    /// SYSTEM-ORIGIN ONLY: emit up to [`MAX_DELIVERIES_PER_BLOCK`] pending
    /// [`ResultEvent`]s to their receivers. injected by the host drain when
    /// the committed mailbox is non-empty — never submitted by anyone.
    DeliverPending {},
    /// permissionless no-op: stages nothing, always applies. its only purpose
    /// is EXISTING as a successful block, which carries the host's
    /// `DeliverPending` injection — the liveness pump for a committed
    /// mailbox on a chain nothing else is ticking (the never-pop-stack
    /// rule's flush lane). duplicate nudges are free.
    Nudge {},
}

// ---- queries --------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DispatchQuery {
    Recipes,
    Recipe {
        recipe_id: String,
    },
    /// one dispatch, addressed the way its creator knows it: the receiving
    /// module's id plus the receiver-local dispatch id.
    Dispatch {
        receiver: String,
        dispatch_id: String,
    },
    /// count of mailbox events awaiting delivery — the host injection's read.
    PendingDeliveries,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DispatchReply {
    Recipes(Vec<Recipe>),
    Recipe(Option<Recipe>),
    Dispatch(Option<DispatchView>),
    PendingDeliveries(u64),
}

// ---- codecs ---------------------------------------------------------------------

pub fn encode_msg(m: &DispatchMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}
pub fn decode_msg(b: &[u8]) -> Result<DispatchMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_work_spec(s: &WorkSpec) -> Vec<u8> {
    serde_json::to_vec(s).expect("serializable")
}
/// decode a [`WorkSpec`] and check its self-description: bytes whose `kind`
/// is not [`WORK_SPEC_KIND`] are somebody else's spec, reported as such.
pub fn decode_work_spec(b: &[u8]) -> Result<WorkSpec, String> {
    let spec: WorkSpec = serde_json::from_slice(b).map_err(|e| e.to_string())?;
    if spec.kind != WORK_SPEC_KIND {
        return Err(format!("not a dispatch work spec (kind {:?})", spec.kind));
    }
    Ok(spec)
}
pub fn encode_result_event(e: &ResultEvent) -> Vec<u8> {
    serde_json::to_vec(e).expect("serializable")
}
pub fn decode_result_event(b: &[u8]) -> Result<ResultEvent, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_query(q: &DispatchQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}
pub fn decode_query(b: &[u8]) -> Result<DispatchQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_reply(r: &DispatchReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}
pub fn decode_reply(b: &[u8]) -> Result<DispatchReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_spec_kind_gates_decode() {
        let spec = WorkSpec {
            kind: WORK_SPEC_KIND.into(),
            dispatch_id: "d1".into(),
            capability: "alpha".into(),
            payload: b"input".to_vec(),
            demands: BTreeMap::new(),
            admission: AdmissionPolicy::Queue,
        };
        let bytes = encode_work_spec(&spec);
        assert_eq!(decode_work_spec(&bytes).unwrap(), spec);

        let foreign = serde_json::to_vec(&WorkSpec {
            kind: "other".into(),
            ..spec
        })
        .unwrap();
        assert!(decode_work_spec(&foreign).unwrap_err().contains("kind"));

        // a shape without the kind field at all is not a work spec.
        assert!(decode_work_spec(br#"{"run_id":"r","agent_id":"a"}"#).is_err());
    }

    #[test]
    fn queue_admission_is_omitted_and_defaults_on_decode() {
        let queue_msg =
            br#"{"dispatch":{"dispatch_id":"d","recipe_id":"r","payload":[],"demands":{}}}"#;
        let msg = decode_msg(queue_msg).unwrap();
        assert_eq!(encode_msg(&msg), queue_msg);
        assert!(matches!(
            msg,
            DispatchMsg::Dispatch {
                admission: AdmissionPolicy::Queue,
                ..
            }
        ));

        let spec = decode_work_spec(
            br#"{"kind":"dispatch-work-v1","dispatch_id":"d","capability":"c","payload":[],"demands":{}}"#,
        )
        .unwrap();
        assert_eq!(spec.admission, AdmissionPolicy::Queue);
        assert_eq!(
            encode_work_spec(&spec),
            br#"{"kind":"dispatch-work-v1","dispatch_id":"d","capability":"c","payload":[],"demands":{}}"#
        );

        let mut fail_fast = spec;
        fail_fast.admission = AdmissionPolicy::FailFast;
        assert_eq!(
            decode_work_spec(&encode_work_spec(&fail_fast)).unwrap(),
            fail_fast
        );
    }
}
