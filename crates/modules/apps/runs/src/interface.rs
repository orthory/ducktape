use std::collections::BTreeMap;

use crate::{AgentAction, DelegationRequest, ModelRecord, ReplyBlock, ResourceCaps};
use sdk::Origin as RunOrigin;
use serde::{Deserialize, Serialize};

// ---- pending runs ---------------------------------------------------------------

/// one in-flight run's correlation entry: everything the module needs to act
/// on the dispatch plane's eventual `ResultEvent` — and NOTHING more. this is
/// not a lifecycle record: the entry exists exactly while the dispatch is
/// outstanding and is pruned when the result delivers. status and outcome
/// live in the dispatch module (`DispatchQuery::Dispatch`), keyed by
/// `dispatch_id` under receiver "runs".
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PendingRun {
    /// `"chat\x1f{channel_id}\x1f{anchor_seq}\x1f{agent_id}"` for chat runs
    /// and `"job\x1f{job_id}\x1f{agent_id}\x1f{job_claim_height}"` for job
    /// runs — the turn-claim key; the first creation in consensus order wins.
    pub run_id: String,
    /// hex sha256 of `run_id` — the dispatch-plane id this entry correlates.
    pub dispatch_id: String,
    pub agent_id: String,
    /// empty for job-backed runs.
    pub channel_id: String,
    /// 0 for job-backed runs.
    pub anchor_seq: u64,
    /// the anchor's thread root, if the anchor was a thread reply — the reply
    /// posts into the same thread.
    pub thread_root: Option<u64>,
    /// present for jobs-board runs. chat-triggered runs leave this `None`.
    pub job_id: Option<String>,
    /// the jobs claim height a job-backed run is bound to; chat runs use 0.
    pub job_claim_height: u64,
    /// The origin that called runs to create this work; it may cancel the
    /// run alongside the program's current controller.
    pub requester: RunOrigin,
    pub created_at: u64,
}

// ---- delivered-run history --------------------------------------------------

/// how a run ended: the result delivered, or it failed (worker error,
/// timeout, cancellation, failed validation). a degraded-but-delivered run is
/// `Delivered` with [`RunRecord::degraded`] set.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum RunOutcome {
    Delivered,
    Failed,
}

/// one terminal run in the delivered-runs ring — DERIVED observability state,
/// recorded at delivery (the moment the pending entry prunes). never part of
/// `root()`/snapshot: replay rebuilds it deterministically, and a
/// snapshot-joined node starts with an empty ring.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunRecord {
    pub run_id: String,
    pub agent_id: String,
    /// empty for job-backed runs.
    pub channel_id: String,
    /// the anchor message seq; 0 for job-backed runs. the envelope's
    /// thread key is `"{channel_id}#{seq}"`.
    pub anchor_seq: u64,
    pub outcome: RunOutcome,
    /// the host observed the run as degraded but still delivered it.
    pub degraded: bool,
    /// consensus counters (creation block / delivery block) — counter DIFFS
    /// are meaningful, the raw values are whatever the lane stamps.
    pub created_at: u64,
    pub delivered_at: u64,
    /// lowercase key hex of the node whose attempt settled the run (the DONE
    /// saga's assignee), or `"unknown"`.
    pub executing_node: String,
    /// what the run produced: forge `branch@output_commit` when a push
    /// landed, else the duckfs output snapshot, else `None`.
    pub output_ref: Option<String>,
    /// the forge PR this run opened or updated, when the PR sink applied.
    pub pr_number: Option<u64>,
}

// ---- run-scoped agent calls -------------------------------------------------

/// Maximum caller-chosen idempotency-key size for one agent call.
pub const MAX_DELEGATION_REQUEST_ID_BYTES: usize = 64;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum DelegationStatus {
    Pending,
    Delivered,
    Failed,
    Cancelled,
}

/// The bounded result routed back to a caller run. It is queryable only while
/// that root run remains live; Runs removes the whole call tree with the root.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DelegationResult {
    pub reply_blocks: Vec<ReplyBlock>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// One ephemeral caller/callee edge. This is run state, not an ModelRecord
/// relation: both agents remain peers before and after the call.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DelegationView {
    pub delegation_id: String,
    pub request_id: String,
    pub caller_run_id: String,
    pub root_run_id: String,
    pub callee_run_id: String,
    pub callee_agent_id: String,
    pub status: DelegationStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<DelegationResult>,
    pub created_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<u64>,
}

// ---- ops ----------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
// Messages are decoded one at a time; keeping configuration inline preserves
// the same direct construction API as the other module operations.
#[allow(clippy::large_enum_variant)]
pub enum RunsMsg {
    /// Configure model work for an existing keyless program account.
    ConfigureModel {
        operation: crate::ModelMsg,
    },
    RequestJobRun {
        agent_id: String,
        job_id: String,
    },
    RequestAttributedRun {
        agent_id: String,
        change_seq: u64,
    },
    ClaimActionRequest {
        request_id: String,
        target_step: u64,
    },
    CompleteActionRequest {
        request_id: String,
        call: sdk::CallId,
        /// The program's decoded call binding. Output-derived links are
        /// accepted only when their bytes match dispatch's committed digest.
        result: agent::CallResult,
    },
    RejectActionRequest {
        request_id: String,
        reason: String,
    },
    /// Source-owned deferred publication; authenticated against the host delivery.
    PublishActionRequest {
        request_id: String,
    },
    /// opt the runs module into or out of jobs-board submit notifications.
    /// the jobs module derives the worker id from this module's follow-up origin.
    EnableJobWorker {
        enabled: bool,
    },
    /// Ask the configured program to run this model against a channel anchor.
    /// The authenticated requester retains cancellation/reassignment authority.
    /// Calls for an already dispatched model/anchor are deterministic no-ops.
    /// External callers must have chat post standing; the eventual program
    /// execution also passes the target's own account authorization.
    RequestRun {
        agent_id: String,
        channel_id: String,
        anchor_seq: u64,
        /// numeric resource demands, threaded verbatim onto the emitted
        /// `DispatchMsg::Dispatch` — the only demand surface in this phase;
        /// every other intake (chat mention, page comment, jobs) dispatches
        /// demandless. empty = a demandless run (every non-RequestRun intake
        /// dispatches demandless).
        #[serde(default)]
        demands: BTreeMap<String, u64>,
        /// library skill NAMES curated for THIS run, on top of the agent's own
        /// curation — the explicit-request surface only (mention, page, forge,
        /// and jobs intakes have no requester to choose them). each resolves to
        /// `/shared/skills/<name>`, loaded on demand. skills are NOT part of the
        /// run's identity: the turn is claimed on (channel, anchor, agent), so a
        /// second request at one anchor with a different curation is the same
        /// turn and no-ops. empty = the agent's curation verbatim.
        #[serde(default)]
        skills: Vec<String>,
    },
    /// cancel a PENDING run — only the run-creating origin or the agent's
    /// owner. cancels the underlying dispatch in the same block; the plane's
    /// Err("cancelled") delivery then prunes the entry (and finalizes a
    /// job-backed run's job) through the one result path.
    CancelRun {
        run_id: String,
    },
    /// fence the current attempt and move the run to another provider. gated
    /// exactly like cancellation; `attempt` prevents a delayed click from
    /// revoking a newer assignment.
    ReassignRun {
        run_id: String,
        attempt: u32,
    },

    // ---- the agent session lane (mid-run writes) --------------------------
    /// the EXECUTING node binds an ephemeral session key to a live run, so the
    /// agent it is running can act DURING the run instead of only in the JSON
    /// it returns at the end.
    ///
    /// authorization is the run's own committed lease: the origin must be the
    /// node that holds it (the `assignee` on the saga the run's dispatch names,
    /// read directly from saga's committed lease). that is
    /// SELF-AUTHORIZING — no owner signature, so an automated issue-mention run
    /// works with nobody at a keyboard — and it is correct cross-node, because
    /// the lease names the node actually executing the run.
    ///
    /// the OWNER's authority is deliberately not asked for here, because
    /// consensus already holds it: `ModelRecord { owner, allowed_actions, caps }`
    /// IS the capability grant — registering an agent with `chat.post` is the
    /// act of authorizing it. what this op adds is not authority but PROOF: that
    /// an op came from this agent's run and no other.
    OpenAgentSession {
        run_id: String,
        /// the session's ed25519 PUBLIC key, exactly [`SESSION_KEY_LEN`] bytes.
        /// its private half never leaves the executing host and reaches only the
        /// agent's tool server — never the node key, which can sign anything.
        session_key: Vec<u8>,
    },
    /// one agent action, applied MID-RUN, signed by the bound session key.
    ///
    /// the origin must BE that session key. a frame's origin is its VERIFIED
    /// public key (`node::decode_frame` binds `(origin, seq, target, payload)`,
    /// and every honest validator rejects a forged frame identically), so this
    /// is authorship consensus can trust — unlike the frameless `/v1/submit`
    /// lane, whose caller-supplied origin `bin/node` discards outright.
    ///
    /// the action is then validated against the SAME `allowed_actions` + caps
    /// the response path validates, by the same code: the tool plane must never
    /// become a second, wider permission vocabulary.
    AgentAction {
        run_id: String,
        action: AgentAction,
    },
    /// Start one peer agent call while the caller is still running. The bound
    /// session key authorizes the request; `subagent_budget` bounds concurrent
    /// live calls across the root tree, and the callee executes with caller ∩
    /// callee authority.
    ExecuteDelegation {
        run_id: String,
        request_id: String,
        request: DelegationRequest,
    },
    DelegateRun {
        run_id: String,
        request_id: String,
        request: DelegationRequest,
    },
}

// ---- the agent session lane ---------------------------------------------------

/// required byte length of a session key (an ed25519 public key).
pub const SESSION_KEY_LEN: usize = 32;

/// hard cap on writes and peer calls ONE session may apply — the mid-run peer of
/// [`crate::MAX_ACTIONS_PER_RUN`]'s blast-radius bound. a session that has burned
/// its budget can still RETURN a response; it just cannot keep writing.
pub const MAX_ACTIONS_PER_SESSION: u32 = 32;

/// one live agent session: an ephemeral key bound to a run, plus the audit
/// record of what it did with it.
///
/// `actions` is BOTH the spent budget and the deterministic id salt — the nth
/// action of a session mints its chat/task/pages ids from `(run_id, n)`, so every
/// replaying validator derives byte-identical ids and no host randomness ever
/// reaches consensus.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AgentSession {
    pub run_id: String,
    pub agent_id: String,
    /// the ed25519 public key that must sign every [`RunsMsg::AgentAction`] for
    /// this run.
    pub session_key: Vec<u8>,
    /// the node key that held the run's execution lease when the session was
    /// opened. the lease can MOVE mid-run (a reassignment, or an expiry that
    /// re-leases the saga), and the session's authority is the lease — so every
    /// acting op re-reads the live holder and refuses once it stops matching
    /// this, and the new holder may open a session of its own.
    pub holder: Vec<u8>,
    pub opened_at: u64,
    /// how many actions this session has applied — the audit counter.
    pub actions: u32,
}

/// the ADMISSION CEILING a delegated run carries: the CALLER's own grant,
/// frozen at the moment of the call. it is never authority of its own — every
/// read intersects it with the callee's LIVE record ([`Self::apply`]), so a
/// later owner revocation narrows the run immediately while a later widening
/// cannot escape what the caller originally granted.
///
/// consensus applies it on every write (`agent_for_run`); the read plane must
/// apply the SAME ceiling, which is what [`RunsQuery::RunAuthority`] is for —
/// a peer's MCP server re-fetching the callee's standing record would gate its
/// reads on the callee's full caps, i.e. on a grant this run never had.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunAuthority {
    pub allowed_actions: Vec<String>,
    pub caps: ResourceCaps,
}

impl RunAuthority {
    pub(crate) fn from_record(record: &ModelRecord) -> Self {
        Self {
            allowed_actions: record.allowed_actions.clone(),
            caps: record.caps.clone(),
        }
    }

    /// `record` narrowed to this ceiling — the ONE definition, shared by the
    /// consensus write path and the host-side read plane.
    pub fn apply(&self, record: &ModelRecord) -> ModelRecord {
        let mut ceiling = record.clone();
        ceiling.allowed_actions = self.allowed_actions.clone();
        ceiling.caps = self.caps.clone();
        ceiling.scoped_for_call(record)
    }
}

/// one in-flight run's ceiling, as [`RunsQuery::RunAuthority`] answers it.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunAuthorityView {
    pub run_id: String,
    pub agent_id: String,
    /// `None` for an ordinary run — nothing narrows it but the agent's own
    /// committed record.
    pub authority: Option<RunAuthority>,
}

// ---- queries ------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum RunsQuery {
    Model {
        query: crate::ModelQuery,
    },
    /// Stored proposal data, without querying its program's status.
    ActionPlan {
        request_id: String,
    },
    ActionRequest {
        request_id: String,
    },
    /// every in-flight correlation entry, ascending by dispatch id. bounded:
    /// entries prune on delivery, and every dispatch has a deadline.
    PendingRuns,
    /// the delivered-runs ring, newest first (last 100). derived state — see
    /// [`RunRecord`].
    RecentRuns,
    /// every LIVE agent session, ascending by run id — the audit surface: which
    /// agents hold a key right now, and how much of their action budget each has
    /// spent. sessions prune when their run settles, so this is bounded by the
    /// in-flight runs.
    AgentSessions,
    /// Calls made by one live caller, including completed results waiting for
    /// that caller to collect them.
    Delegations {
        caller_run_id: String,
    },
    /// one IN-FLIGHT run's admission ceiling — the read plane's half of
    /// [`RunAuthority`]. `None` when the run is not in flight, which a caller
    /// must treat as a refusal: an unknown run proves no ceiling.
    RunAuthority {
        run_id: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum RunsReply {
    Model(crate::ModelReply),
    ActionRequest(Option<ActionRequestView>),
    PendingRuns(Vec<PendingRun>),
    RecentRuns(Vec<RunRecord>),
    AgentSessions(Vec<AgentSession>),
    Delegations(Vec<DelegationView>),
    /// boxed only to keep the reply enum small — serde is transparent over
    /// `Box`, so the wire shape is the bare view or `null`.
    RunAuthority(Option<Box<RunAuthorityView>>),
}

// ---- codecs -------------------------------------------------------------------

pub fn encode_msg(m: &RunsMsg) -> Vec<u8> {
    sdk::wire::encode(m)
}
pub fn decode_msg(b: &[u8]) -> Result<RunsMsg, String> {
    sdk::wire::decode(b)
}
pub fn encode_query(q: &RunsQuery) -> Vec<u8> {
    sdk::wire::encode(q)
}
pub fn decode_query(b: &[u8]) -> Result<RunsQuery, String> {
    sdk::wire::decode(b)
}
pub fn encode_reply(r: &RunsReply) -> Vec<u8> {
    sdk::wire::encode(r)
}
pub fn decode_reply(b: &[u8]) -> Result<RunsReply, String> {
    sdk::wire::decode(b)
}

/// A run tool request is proposed work until a program's real target call completes.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ActionStatus {
    AwaitingProgram,
    Claimed {
        call: sdk::CallId,
    },
    Completed {
        call: sdk::CallId,
        outcome: dispatch::CallOutcomeSummary,
    },
    Rejected {
        reason: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionRequestView {
    pub request_id: String,
    pub account: sdk::AccountNumber,
    pub generation: u64,
    pub run_id: String,
    pub target: String,
    pub payload: serde_json::Value,
    pub status: ActionStatus,
}

/// A caller knows its session slot before admission and can await this exact receipt.
pub fn action_request_id(run_id: &str, slot: u32) -> String {
    format!("session/{}/{slot}", crate::dispatch_id_for(run_id))
}

/// Stable correlation for a named peer call, including an exact retry.
pub fn delegation_action_id(run_id: &str, request_id: &str) -> String {
    format!(
        "delegate/{}/{}",
        crate::dispatch_id_for(run_id),
        crate::dispatch_id_for(request_id)
    )
}
