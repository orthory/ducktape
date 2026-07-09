//! the runs module — the collaboration loop's actor.
//!
//! a pure state-machine module (in the app-hash) holding channel watches and
//! the correlation entries for still-pending dispatches. the agents it runs
//! are NOT its state: the agent registry (`crates/apps/agent`) is the record
//! book, and this module reads it by query — staged same-block registrations
//! included, through the host's live query routing. run LIFECYCLE is not here
//! either: a run is a dispatched task, and its status, outcome, and history
//! live in the dispatch module (and its saga) — per-dispatched-task, never
//! agent-owned. what this module keeps per run is exactly what acting on the
//! eventual `ResultEvent` needs (where the reply goes, which job to finalize,
//! who may cancel), pruned when the result delivers.
//!
//! the module implements the platform's ordering-contract promises where they
//! touch agents (docs/agent-collaboration-design.md §2, §3, §5):
//!
//! - **P2 — atomic causal cascades.** a user post, the tagging plane's
//!   engagement delivery, the pending entry, and the dispatch commit in ONE
//!   block; a watch and its plane subscription commit in one block; a
//!   validated response's chat reply and task writes commit in the delivery
//!   block. the registry hook extends this to registration: an agent's
//!   registry record and its dispatch recipe land (or abort) as one unit.
//! - **P4 — anchored generation.** the ENTIRE model input is composed in
//!   consensus — transcript window, prompt framing, output contract — and
//!   rides the dispatch as committed payload data (the structured envelope in
//!   [`envelope`]; the agent's prompt rides as its committed hash, resolved
//!   from the content-addressed blob store by the host), so any validator
//!   holds the exact prompt input as ordered state, and the reply is never
//!   presented as ordered before its anchor.
//! - **P6 — callback adjacency.** on the dispatch plane this becomes
//!   next-block delivery: the ResultEvent, the validated reply, the task
//!   writes, and a job-backed run's finalize all commit in the one delivery
//!   block.
//!
//! ## execute routing — payload namespaces, keyed by ORIGIN
//!
//! the dispatch origin is host-assigned and cannot be chosen by a submitter,
//! so routing on it makes every privileged intake spoof-proof by
//! construction:
//!
//! - `Origin::Module(tagging)` → an `EngagementEvent` (the engagement
//!   intake): the tagging plane's routed report of a user post in a watched
//!   channel, tags included;
//! - `Origin::Module(dispatch)` → a `ResultEvent` (the dispatch plane's
//!   next-block delivery — the ONLY result intake);
//! - `Origin::Module(jobs)` → a [`JobsEvent`] (the jobs-board intake);
//! - `Origin::Module(agent)` → an [`AgentEvent`] (the registry hook): the
//!   registry's same-block notification that an agent landed or changed
//!   capability, answered here by registering/retuning the agent's
//!   dispatch-plane recipe. unlike every other module intake this one MAY
//!   error — it rides the registry write's own block, and aborting that
//!   block is exactly the atomicity the recipe seam needs;
//! - `Origin::Module(saga)` → a dead-letter no-op. nothing here rides the
//!   saga directly, but any submitter can point a saga trigger's `reply_to`
//!   at this module — the tombstone keeps that callback from ever aborting
//!   the saga's terminal block (the callback-poison rule);
//! - `Origin::Module(chat)` → a dead-letter no-op (chat never notifies this
//!   module directly; the tombstone keeps a stray follow-up from aborting a
//!   posting block);
//! - anything else → a [`RunsMsg`] (admin ops and explicit runs). an
//!   external submitter shipping intake-shaped bytes lands HERE and fails the
//!   `RunsMsg` decode — it can never fake an intake.
//!
//! ## the NO-FAIL arms (design §4)
//!
//! every privileged intake except the registry hook MUST NEVER return `Err`:
//!
//! - the result intake runs inside the delivery block; an `Err` would abort
//!   it, the committed mailbox would re-inject next block, and every
//!   subsequent block would abort (the permanent-abort loop the dispatch
//!   module documents). malformed events and unknown dispatch ids are staged
//!   no-ops (plus an observability event), and a response that fails
//!   validation FAILS THE RUN, never the block. anything the emitted
//!   follow-ups could make chat or tasks reject (a squatted reply message id,
//!   an oversized reply, a duplicate task id, a full thread) is probed
//!   deterministically first — an emitted follow-up must be valid by
//!   construction.
//! - the engagement intake runs in the same block as the user's post. an
//!   `Err` here would abort the post (and every other subscriber's delivery),
//!   so a malformed event or a failed context pin is equally a staged no-op.
//! - the jobs intake runs in the same block as the job submit. jobs queries
//!   are committed-only, so the just-staged job is invisible to
//!   `JobsQuery::Get`; this path skips that blind probe and relies on the
//!   documented single claiming-worker cascade rule before emitting its
//!   `Claim`.
//!
//! ## agent identity
//!
//! this module posts replies `as_agent`, so chat's origin-derived authorship
//! makes every agent's wire identity `{runs}/{agent_id}` — the module that
//! ACTS for agents, not the registry that records them. mentions and
//! engagement tags use the same ref (`EntityRef { module: runs, entity }`),
//! so mentioning a reply's author round-trips into an engagement.
//!
//! ## the turn claim
//!
//! chat run ids and job run ids use disjoint `0x1f`-delimited keyspaces.
//! creating a run that already exists (staged or committed) is a
//! deterministic no-op, so however many paths race to claim a turn — the
//! engagement and an explicit `RequestRun`, or two identical requests — the
//! first in consensus order wins and the rest fall through silently.
//!
//! `root()` folds in every field of both maps, so any transition moves the
//! app-hash. a joiner rebuilds this module from a peer via
//! [`RunsModule::snapshot`] / [`RunsModule::install`]: the snapshot ships the
//! committed maps in the exact canonical encoding `root()` hashes, and
//! install re-derives the root from the decoded temporaries before adopting
//! them — the consensus-agreed root, not the peer, is the trust anchor.

// the wire surface: this module's shared types, flattened at the crate root.
mod interface;
pub use interface::*;

// dispatch payload composition: the structured run envelope.
mod envelope;
pub use envelope::RUN_ENVELOPE_VERSION;

use std::collections::{BTreeMap, BTreeSet};

use agent::{
    ACTION_CHAT_POST, ACTION_TASKS_CREATE, ACTION_TASKS_UPDATE_STATUS, AgentAction, AgentEvent,
    AgentQuery, AgentRecord, AgentReply, AgentResponse, AgentStatus, CapRequest,
    MAX_ACTIONS_PER_RUN, MAX_REPLY_BLOCKS_BYTES, RESERVED_ID_SEPARATOR, ReplyBlock,
    decode_event as agent_decode_event, decode_reply as agent_decode_reply,
    encode_query as agent_encode_query,
};
use chat::{
    Block, ChatMsg, ChatQuery, ChatReply, MAX_THREAD_REPLIES, MessageView,
    decode_reply as chat_decode_reply, encode_msg as chat_encode_msg,
    encode_query as chat_encode_query,
};
use dispatch::{
    DispatchMsg, DispatchQuery, DispatchReply, MAX_PAYLOAD_BYTES, OutputContract, ResultEvent,
    Routing, decode_reply as dispatch_decode_reply, decode_result_event,
    encode_msg as dispatch_encode_msg, encode_query as dispatch_encode_query,
};
use duckfs_core::{
    FilesQuery, FilesReply, decode_reply as files_decode_reply,
    encode_query as files_encode_query,
};
use jobs::{
    JobStatus, JobsEvent, JobsMsg, JobsQuery, JobsReply, decode_event as jobs_decode_event,
    decode_reply as jobs_decode_reply, encode_msg as jobs_encode_msg,
    encode_query as jobs_encode_query,
};
use saga::SagaOrigin;
use sdk::{Ctx, Error, Event, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tagging::{
    EngagementEvent, EntityRef, TaggingMsg, decode_event as tagging_decode_event,
    encode_msg as tagging_encode_msg,
};
use tasks::{
    TaskMsg, TaskQuery, TaskReply, TaskStatus, decode_reply as tasks_decode_reply,
    encode_msg as tasks_encode_msg, encode_query as tasks_encode_query,
};

/// how many transcript messages (newest-first, ending at the anchor) one run
/// embeds into its composed payload — the bounded prompt window (P4).
pub const CONTEXT_WINDOW: u64 = 64;

/// whole-dispatch deadline granted to a run's LLM work, in views past the
/// dispatching block.
pub const RUN_DEADLINE_VIEWS: u64 = 1024;

/// oracle attempts per run: one retry after a failed or expired attempt.
pub const RUN_MAX_ATTEMPTS: u32 = 2;

/// jobs-board claims created by the runs worker use a view-denominated lease.
pub const JOB_RUN_LEASE_VIEWS: u64 = 1000;

/// jobs finalization payloads must fit the jobs module's 64 KiB cap.
const JOB_FINALIZE_PAYLOAD_BYTES: usize = 64 * 1024;
/// reserved delimiter separating run-key fields — the registry rejects agent
/// ids carrying it ([`RESERVED_ID_SEPARATOR`]), so run keys stay unambiguous.
const RUN_KEY_SEPARATOR: char = RESERVED_ID_SEPARATOR;

/// the turn-claim key: first creation in consensus order wins.
pub fn run_id_for(channel_id: &str, anchor_seq: u64, agent_id: &str) -> String {
    format!(
        "chat{RUN_KEY_SEPARATOR}{channel_id}{RUN_KEY_SEPARATOR}{anchor_seq}{RUN_KEY_SEPARATOR}{agent_id}"
    )
}

/// the turn-claim key for a job-backed run.
pub fn job_run_id_for(job_id: &str, agent_id: &str, claim_height: u64) -> String {
    format!(
        "job{RUN_KEY_SEPARATOR}{job_id}{RUN_KEY_SEPARATOR}{agent_id}{RUN_KEY_SEPARATOR}{claim_height}"
    )
}

/// canonical pin over submitted job-spec bytes — the jobs event's `spec_hash`.
pub fn job_spec_hash(spec: &[u8]) -> Vec<u8> {
    Sha256::digest(spec).to_vec()
}

/// the chat message id of a run's reply — one run posts at most one reply.
pub fn reply_message_id(run_id: &str) -> String {
    format!("agent/{run_id}")
}

/// the dispatch-plane recipe an agent's runs execute under — registered
/// (module-owned) by the registry hook in the same block as the agent itself.
pub fn recipe_id_for(agent_id: &str) -> String {
    format!("agent/{agent_id}")
}

/// the dispatch-plane id of a run's dispatch. run ids carry the reserved
/// `\x1f` separator the dispatch module rejects in caller-chosen ids, so the
/// dispatch id is the run id's hex sha256 — fixed-width, always within the
/// dispatch id cap; the pending map is keyed by it.
pub fn dispatch_id_for(run_id: &str) -> String {
    hex(&Sha256::digest(run_id.as_bytes()))
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ---- response normalization ---------------------------------------------------------
// the dispatch-plane oracle returns the model's RAW text (opinion-free, Text
// contract); shaping it into an [`AgentResponse`] is deterministic string
// processing and therefore consensus work, done here in the result intake.

/// the reply-block kinds normalization keeps — the closed vocabulary the
/// strict-output instruction names.
const REPLY_KIND_PARAGRAPH: &str = "paragraph";
const REPLY_KIND_HEADING: &str = "heading";
const REPLY_KIND_CODE: &str = "code";
const RUNNER_RESULT_VERSION: u32 = envelope::RUNNER_RESULT_VERSION;

/// the R5 typed-data facet ceiling — data larger than this degrades to null.
const MAX_DATA_BYTES: usize = 32 * 1024;
/// the faceted job-finalize payload envelope version (the `ducktape_delivery`
/// wrapper every run's finalize carries).
const DELIVERY_RECEIPT_VERSION: u32 = 1;

/// the wrapper a portable (`v3`) provider returns instead of the bare response
/// text: the model prose plus the host-assembled facets — message
/// (`response_text`) / data / effects / artifact (`workspace_receipt`) / sink /
/// status. the five facet fields are ADDITIVE `#[serde(default)]`s (this struct
/// is deliberately NOT `deny_unknown_fields`), so a minimal
/// `{ducktape_runner_result, response_text, workspace_receipt}` wrapper still
/// decodes and a bytes-with-no-marker result becomes a message-only result. the
/// single delivery path ([`RunsModule::deliver_run_result`]) applies whatever
/// facets are present; a plain (message-only) result carries none.
#[derive(Deserialize, Debug)]
struct RunnerResult {
    ducktape_runner_result: u32,
    response_text: String,
    workspace_receipt: WorkspaceReceipt,
    /// R5 typed-data facet: an already-serialized JSON text or `None`.
    #[serde(default)]
    data: Option<String>,
    /// R2 declarative effects, host-assembled (lifted from the model's actions).
    #[serde(default)]
    effects: Vec<WireEffect>,
    /// O1/O2 output sink; default [`WireSink::Chain`].
    #[serde(default)]
    sink: WireSink,
    /// the host's terminal observation; default [`WireStatus::Ok`].
    #[serde(default)]
    status: WireStatus,
}

#[derive(Deserialize, Default, Debug)]
struct WorkspaceReceipt {
    source_prefix: String,
    #[allow(dead_code, reason = "audit metadata retained in dispatch history")]
    source_snapshot: Option<String>,
    output_snapshot: Option<String>,
    commit_height: Option<u64>,
    rebased: bool,
    no_changes: bool,
}

/// one host-assembled declarative effect (R2). `kind` is a run-effect wire name
/// (`tasks.create` / `tasks.update_status`); the remaining fields carry the
/// action's payload. mapped to an [`AgentAction`] by [`effects_to_actions`],
/// where an unknown `kind` fails the run deterministically (R4).
#[derive(Deserialize, Debug)]
struct WireEffect {
    kind: String,
    #[serde(default)]
    task_id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    status: String,
}

/// the O1/O2 output sink. internally tagged on `mode`; a MISSING sink field
/// defaults to `Chain` via the `#[serde(default)]` on [`RunnerResult::sink`], and
/// a present `{"mode":"pr",...}` decodes to `Pr`. `Merge` is DEFINED-BUT-INERT in
/// v1 (validated on the wire, treated like `Chain` with a breadcrumb).
#[derive(Deserialize, Default, Debug)]
#[serde(tag = "mode", rename_all = "snake_case")]
enum WireSink {
    #[default]
    Chain,
    Pr {
        #[serde(default)]
        repo: String,
        source_branch: String,
        #[serde(default)]
        target_branch: String,
        title: String,
        #[serde(default)]
        body: String,
    },
    Merge {
        #[serde(default)]
        repo: String,
        number: u64,
        #[allow(dead_code, reason = "merge sink wire is forward-compatible but inert in v1")]
        prev_target_oid: String,
        #[allow(dead_code, reason = "merge sink wire is forward-compatible but inert in v1")]
        expected_source_oid: String,
        #[allow(dead_code, reason = "merge sink wire is forward-compatible but inert in v1")]
        merge_oid: String,
        #[allow(dead_code, reason = "merge sink wire is forward-compatible but inert in v1")]
        pack_digest: String,
    },
}

/// the host's terminal observation of a run. `Failed` fails the run even with a
/// present message facet; `Degraded` still delivers (surfaced in the receipt).
#[derive(Deserialize, Default, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
enum WireStatus {
    #[default]
    Ok,
    Degraded,
    Failed,
}

// ---- faceted delivery --------------------------------------------------------
// the single delivery path decodes the runner result below and applies whatever
// facets it carries; a plain (message-only) result carries none.

/// decode the full faceted [`RunnerResult`]. marker + version strict (R4): a
/// wrapper that claims the marker but is malformed or an unsupported version
/// fails the run. bytes with NO marker — every legacy raw-text result — become a
/// message-only result (response_text = the lossy-decoded bytes, no facets).
fn decode_run_result_v1(bytes: &[u8]) -> Result<RunnerResult, String> {
    match serde_json::from_slice::<serde_json::Value>(bytes) {
        Ok(serde_json::Value::Object(map)) if map.contains_key("ducktape_runner_result") => {
            let result: RunnerResult = serde_json::from_value(serde_json::Value::Object(map))
                .map_err(|e| format!("runner result is malformed: {e}"))?;
            if result.ducktape_runner_result != RUNNER_RESULT_VERSION {
                return Err(format!(
                    "runner result version {} is not supported (understands {RUNNER_RESULT_VERSION})",
                    result.ducktape_runner_result
                ));
            }
            Ok(result)
        }
        _ => Ok(RunnerResult {
            ducktape_runner_result: RUNNER_RESULT_VERSION,
            response_text: String::from_utf8_lossy(bytes).into_owned(),
            workspace_receipt: WorkspaceReceipt::default(),
            data: None,
            effects: Vec::new(),
            sink: WireSink::Chain,
            status: WireStatus::Ok,
        }),
    }
}

/// map host-assembled declarative effects into the validated [`AgentAction`]
/// vocabulary. v1 vocab == today's two task verbs (chat.post is the message
/// facet, not an effect). an UNKNOWN kind fails the run deterministically (R4)
/// — this is the concrete gate for any verb beyond the 3-verb set.
fn effects_to_actions(effects: &[WireEffect]) -> Result<Vec<AgentAction>, String> {
    if effects.len() > MAX_ACTIONS_PER_RUN {
        return Err(format!(
            "{} effects exceed the cap of {MAX_ACTIONS_PER_RUN}",
            effects.len()
        ));
    }
    effects
        .iter()
        .map(|e| match e.kind.as_str() {
            ACTION_TASKS_CREATE => Ok(AgentAction::CreateTask {
                task_id: e.task_id.clone(),
                title: e.title.clone(),
            }),
            ACTION_TASKS_UPDATE_STATUS => Ok(AgentAction::UpdateTaskStatus {
                task_id: e.task_id.clone(),
                status: e.status.clone(),
            }),
            other => Err(format!("unknown effect kind: {other}")),
        })
        .collect()
}

/// the R5 data facet, valid only when it is within the size ceiling AND parses
/// as JSON; anything else degrades to null (never fails the run).
fn valid_data(data: &Option<String>) -> Option<&str> {
    data.as_deref()
        .filter(|s| s.len() <= MAX_DATA_BYTES && serde_json::from_str::<serde_json::Value>(s).is_ok())
}

/// the faceted job-finalize payload: the validated response plus the data
/// facet, the derived output_ref (O1), and the status. deterministic — fixed
/// serde field order, data embedded verbatim as already-validated JSON.
#[derive(Serialize)]
struct DeliveryReceipt<'a> {
    ducktape_delivery: u32,
    response: &'a AgentResponse,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_ref: Option<OutputRef<'a>>,
    status: &'static str,
}

/// the artifact facet distilled to a chainable reference (O1): a downstream run
/// can set `workspace.source = prior.output_snapshot`.
#[derive(Serialize, Clone)]
struct OutputRef<'a> {
    source_prefix: &'a str,
    output_snapshot: &'a str,
    commit_height: Option<u64>,
    rebased: bool,
    no_changes: bool,
}

fn encode_delivery_receipt(
    response: &AgentResponse,
    data: Option<&str>,
    receipt: &WorkspaceReceipt,
    status: WireStatus,
) -> String {
    let output_ref = receipt.output_snapshot.as_deref().map(|snap| OutputRef {
        source_prefix: &receipt.source_prefix,
        output_snapshot: snap,
        commit_height: receipt.commit_height,
        rebased: receipt.rebased,
        no_changes: receipt.no_changes,
    });
    let status = match status {
        WireStatus::Ok => "ok",
        WireStatus::Degraded => "degraded",
        WireStatus::Failed => "failed",
    };
    let encode = |data: Option<&str>| {
        serde_json::to_string(&DeliveryReceipt {
            ducktape_delivery: DELIVERY_RECEIPT_VERSION,
            response,
            data,
            output_ref: output_ref.clone(),
            status,
        })
        .expect("delivery receipt serializes")
    };
    // the finalize payload MUST stay valid JSON within the jobs cap: the naive
    // byte-truncation the jobs board applies would corrupt it. the response is
    // already capped (MAX_REPLY_BLOCKS_BYTES) and output_ref/status are tiny, so
    // only the optional `data` facet is unbounded — embed it only if the whole
    // receipt still fits, else DROP it here (the full data facet stays in the
    // dispatch-history audit lane, R6, so nothing durable is lost). guarantees a
    // valid, bounded finalize payload with the O1 output_ref always intact.
    let full = encode(data);
    if data.is_some() && full.len() > JOB_FINALIZE_PAYLOAD_BYTES {
        encode(None)
    } else {
        full
    }
}

// ---- forge sink wire (local mirrors) -----------------------------------------
// runs does NOT take a production dependency on the heavy `forge` crate (it
// pulls vendored libgit2). instead it mirrors the exact JSON shape forge decodes
// for the sink op it emits, and a dev-only conformance test pins the mirror
// against `forge::decode_msg` so the wire can't silently drift.

/// the exact `ForgeMsg::OpenPr` JSON the forge module decodes. only the PR sink
/// is emitted in v1 (the merge sink is inert), so only `OpenPr` is mirrored.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum ForgeSinkMsg<'a> {
    OpenPr {
        repo: &'a str,
        title: &'a str,
        body: &'a str,
        source_branch: &'a str,
        target_branch: &'a str,
    },
}

fn forge_open_pr_bytes(repo: &str, title: &str, body: &str, src: &str, tgt: &str) -> Vec<u8> {
    serde_json::to_vec(&ForgeSinkMsg::OpenPr {
        repo,
        title,
        body,
        source_branch: src,
        target_branch: tgt,
    })
    .expect("forge sink msg serializes")
}

/// the `ForgeQuery::ListRefs` mirror the branch-born probe encodes.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum ForgeSinkQuery<'a> {
    ListRefs { repo: &'a str },
}

/// the model's raw answer as a NORMALIZED [`AgentResponse`]: the wire shape
/// when it parses (unknown kinds and empty texts drop), a plain paragraph
/// reply as the fallback for prose. job runs never carry reply blocks — there
/// is no channel to deliver them to.
fn agent_response_from_text(text: &str, job_run: bool) -> AgentResponse {
    let parsed = parse_strict_response(text).unwrap_or_else(|| AgentResponse {
        reply_blocks: if job_run {
            Vec::new()
        } else {
            vec![paragraph_block(non_empty_text(text))]
        },
        actions: Vec::new(),
    });
    normalize_response(parsed, text, job_run)
}

/// decode the strict-output contract's [`AgentResponse`] from a provider's
/// final message. the contract asks for a bare JSON object, but LLMs routinely
/// wrap it in a ```` ```json ```` markdown fence (agentic multi-turn CLIs
/// especially) or pad it with a line of prose — so parse tolerantly: bare
/// first, then de-fenced, then the outermost `{…}` span. without this a
/// perfectly well-formed reply reaches chat as a raw ```` ```json ```` code
/// block instead of the prose the model actually wrote.
fn parse_strict_response(text: &str) -> Option<AgentResponse> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    [
        Some(trimmed),
        strip_code_fence(trimmed),
        outermost_json_object(trimmed),
    ]
    .into_iter()
    .flatten()
    .find_map(|candidate| serde_json::from_str::<AgentResponse>(candidate.trim()).ok())
}

/// strip a single surrounding markdown code fence, returning the inner body.
/// tolerant of an info string (```` ```json ````) and of a missing close.
fn strip_code_fence(text: &str) -> Option<&str> {
    // the opening fence's info string runs to the first newline (```json\n…).
    let body = text.strip_prefix("```")?.split_once('\n').map(|(_, b)| b)?;
    let body = body.trim();
    Some(body.strip_suffix("```").unwrap_or(body).trim())
}

/// the span from the first `{` to the last `}` — JSON the model buried in
/// prose. required fields keep a non-object span from parsing; an object with
/// no known fields decodes empty and degrades to the raw-text paragraph, so
/// over-matching is harmless.
fn outermost_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    // lazily — a `}` before the first `{` gives start > end, and slicing that
    // range panics; `then` must not evaluate the slice unless the range holds.
    (start < end).then(|| &text[start..=end])
}

fn paragraph_block(text: String) -> ReplyBlock {
    ReplyBlock {
        kind: REPLY_KIND_PARAGRAPH.into(),
        text,
        lang: None,
    }
}

/// map a NORMALIZED response's reply blocks into chat blocks — the only place
/// the response vocabulary meets chat's. normalization guarantees only known
/// kinds and non-empty texts remain.
fn to_chat_blocks(blocks: &[ReplyBlock]) -> Vec<Block> {
    blocks
        .iter()
        .map(|block| match block.kind.as_str() {
            REPLY_KIND_CODE => Block::Code {
                lang: block.lang.clone().filter(|l| !l.is_empty()),
                text: block.text.clone(),
            },
            _ => Block::paragraph(block.text.clone()),
        })
        .collect()
}

fn normalize_response(mut response: AgentResponse, raw_text: &str, job_run: bool) -> AgentResponse {
    response.actions.truncate(MAX_ACTIONS_PER_RUN);
    response.reply_blocks = response
        .reply_blocks
        .into_iter()
        .filter_map(|block| {
            let text = block.text.trim().to_string();
            if text.is_empty() {
                return None;
            }
            match block.kind.as_str() {
                REPLY_KIND_PARAGRAPH | REPLY_KIND_HEADING => Some(paragraph_block(text)),
                REPLY_KIND_CODE => Some(ReplyBlock {
                    kind: REPLY_KIND_CODE.into(),
                    text,
                    lang: block.lang.filter(|l| !l.is_empty()),
                }),
                _ => None,
            }
        })
        .collect();
    if job_run {
        response.reply_blocks.clear();
        return response;
    }
    if response.reply_blocks.is_empty() {
        response
            .reply_blocks
            .push(paragraph_block(non_empty_text(raw_text)));
    }
    let bytes = serde_json::to_vec(&to_chat_blocks(&response.reply_blocks)).expect("blocks serialize");
    if bytes.len() > MAX_REPLY_BLOCKS_BYTES {
        response.reply_blocks = vec![paragraph_block(truncate_utf8(
            &non_empty_text(raw_text),
            MAX_REPLY_BLOCKS_BYTES / 4,
        ))];
    }
    response
}

fn non_empty_text(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        "Done.".into()
    } else {
        trimmed.into()
    }
}

fn truncate_utf8(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let mut keep = max;
    while keep > 0 && !text.is_char_boundary(keep) {
        keep -= 1;
    }
    format!("{}…", &text[..keep])
}

/// byte bound on the error excerpt a failure reply carries — same order as
/// the host's diagnostic excerpts (capability-host bounds stderr to 400).
const FAILURE_EXCERPT_BYTES: usize = 400;

/// a failed run's error as ONE bounded chat line: whitespace runs (newlines
/// included) collapse to single spaces, then the excerpt bound applies.
fn failure_excerpt(reason: &str) -> String {
    let line = reason.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.is_empty() {
        return "no error detail".into();
    }
    truncate_utf8(&line, FAILURE_EXCERPT_BYTES)
}

/// the canonical state form of a dispatch origin (see [`SagaOrigin`]).
fn canonical_origin(origin: &Origin) -> SagaOrigin {
    match origin {
        Origin::External(key) => SagaOrigin::External(key.clone()),
        Origin::Module(module) => SagaOrigin::Module(module.clone()),
        Origin::System => SagaOrigin::System,
    }
}

/// the wire name of a task status an [`AgentAction::UpdateTaskStatus`] carries.
fn task_status(name: &str) -> Option<TaskStatus> {
    match name {
        "open" => Some(TaskStatus::Open),
        "in_progress" => Some(TaskStatus::InProgress),
        "done" => Some(TaskStatus::Done),
        _ => None,
    }
}

/// whether the registry granted this agent an action name.
fn allows(agent: &AgentRecord, action: &str) -> bool {
    agent.allowed_actions.iter().any(|a| a == action)
}

/// one in-flight dispatch's correlation entry. the dispatch id is the map
/// key; the run id is derivable from the fields. NOT a lifecycle record: it
/// exists exactly while the dispatch is outstanding and is pruned when the
/// result delivers.
#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingState {
    agent_id: String,
    /// empty for job-backed runs.
    channel_id: String,
    /// 0 for job-backed runs.
    anchor_seq: u64,
    /// the anchor's thread root, if the anchor was a thread reply.
    thread_root: Option<u64>,
    /// the jobs-board item this run owns, when created from a JobsEvent.
    job_id: Option<String>,
    /// the claim height this job-backed run is bound to; chat runs use 0.
    job_claim_height: u64,
    /// the run-creating origin — a cancel capability alongside the owner.
    requester: SagaOrigin,
    created_at: u64,
}

impl PendingState {
    /// the run id these fields derive — chat- or job-keyed.
    fn run_id(&self) -> String {
        match &self.job_id {
            Some(job_id) => job_run_id_for(job_id, &self.agent_id, self.job_claim_height),
            None => run_id_for(&self.channel_id, self.anchor_seq, &self.agent_id),
        }
    }
}

/// a chat run's read-only dispatch preparation: the pinned context plus the
/// fully composed payload, gathered before anything is staged.
#[derive(Debug)]
struct PreparedDispatch {
    thread_root: Option<u64>,
    payload: Vec<u8>,
}

// ---- canonical encoding -------------------------------------------------------
// u64-le counts, sorted keys, every field in declaration order: u64-le length
// prefixes for byte strings, single-byte discriminants for enums, a 0/1 tag
// byte for options, u64-le integers. this is the exact preimage
// [`Module::root`] hashes, so a snapshot and the root that must authenticate
// it cannot drift. no version byte — encoding changes are flag-day (design
// principle: no backwards compatibility).

fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}

fn put_opt_u64(out: &mut Vec<u8>, opt: Option<u64>) {
    match opt {
        None => out.push(0),
        Some(v) => {
            out.push(1);
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
}

fn put_opt_string(out: &mut Vec<u8>, opt: &Option<String>) {
    match opt {
        None => out.push(0),
        Some(value) => {
            out.push(1);
            put_bytes(out, value.as_bytes());
        }
    }
}

fn put_origin(out: &mut Vec<u8>, origin: &SagaOrigin) {
    match origin {
        SagaOrigin::External(key) => {
            out.push(0);
            put_bytes(out, key);
        }
        SagaOrigin::Module(module) => {
            out.push(1);
            put_bytes(out, module.as_bytes());
        }
        SagaOrigin::System => out.push(2),
    }
}

fn encode_committed(
    watches: &BTreeMap<String, TurnPolicy>,
    pending: &BTreeMap<String, PendingState>,
) -> Vec<u8> {
    let mut out = Vec::new();

    out.extend_from_slice(&(watches.len() as u64).to_le_bytes());
    for (channel, policy) in watches {
        put_bytes(&mut out, channel.as_bytes());
        match policy {
            TurnPolicy::Mention => out.push(0),
            TurnPolicy::All => out.push(1),
            TurnPolicy::Assigned(agent_id) => {
                out.push(2);
                put_bytes(&mut out, agent_id.as_bytes());
            }
            TurnPolicy::RoundRobin => out.push(3),
        }
    }

    out.extend_from_slice(&(pending.len() as u64).to_le_bytes());
    for (dispatch_id, p) in pending {
        put_bytes(&mut out, dispatch_id.as_bytes());
        put_bytes(&mut out, p.agent_id.as_bytes());
        put_bytes(&mut out, p.channel_id.as_bytes());
        out.extend_from_slice(&p.anchor_seq.to_le_bytes());
        put_opt_u64(&mut out, p.thread_root);
        put_opt_string(&mut out, &p.job_id);
        out.extend_from_slice(&p.job_claim_height.to_le_bytes());
        put_origin(&mut out, &p.requester);
        out.extend_from_slice(&p.created_at.to_le_bytes());
    }

    out
}

/// the state-based commitment over the committed maps — shared by `root()`
/// and `install()` so the verification a snapshot must pass is definitionally
/// the same algorithm the live module answers with.
fn committed_root(
    watches: &BTreeMap<String, TurnPolicy>,
    pending: &BTreeMap<String, PendingState>,
) -> StateRoot {
    StateRoot(Sha256::digest(encode_committed(watches, pending)).into())
}

// ---- canonical decoding (UNTRUSTED input) ---------------------------------
// bounds are validated against the remaining input BEFORE any allocation,
// keys must be strictly ascending (one encoding per state, uniqueness for
// free), unknown discriminants/tags and trailing bytes are rejected. never
// panics on malformed input.

/// pull `n` bytes off the front of `buf`, checked before any slicing.
fn take<'a>(buf: &mut &'a [u8], n: usize) -> Result<&'a [u8], String> {
    if n > buf.len() {
        return Err("snapshot truncated".into());
    }
    let (head, tail) = buf.split_at(n);
    *buf = tail;
    Ok(head)
}

fn take_u64(buf: &mut &[u8]) -> Result<u64, String> {
    Ok(u64::from_le_bytes(
        take(buf, 8)?.try_into().expect("8 bytes"),
    ))
}

/// a length prefix, validated against the remaining input before the caller
/// allocates anything of that size.
fn take_len(buf: &mut &[u8]) -> Result<usize, String> {
    let n = take_u64(buf)?;
    if n > buf.len() as u64 {
        return Err("snapshot length prefix exceeds input".into());
    }
    Ok(n as usize)
}

fn take_lp_bytes(buf: &mut &[u8]) -> Result<Vec<u8>, String> {
    let len = take_len(buf)?;
    Ok(take(buf, len)?.to_vec())
}

fn take_lp_string(buf: &mut &[u8]) -> Result<String, String> {
    let len = take_len(buf)?;
    Ok(std::str::from_utf8(take(buf, len)?)
        .map_err(|_| "snapshot string is not utf-8".to_string())?
        .to_owned())
}

fn take_opt_u64(buf: &mut &[u8]) -> Result<Option<u64>, String> {
    match take(buf, 1)?[0] {
        0 => Ok(None),
        1 => Ok(Some(take_u64(buf)?)),
        t => Err(format!("snapshot has unknown option tag {t}")),
    }
}

fn take_opt_string(buf: &mut &[u8]) -> Result<Option<String>, String> {
    match take(buf, 1)?[0] {
        0 => Ok(None),
        1 => Ok(Some(take_lp_string(buf)?)),
        t => Err(format!("snapshot has unknown option tag {t}")),
    }
}

fn take_origin(buf: &mut &[u8]) -> Result<SagaOrigin, String> {
    match take(buf, 1)?[0] {
        0 => Ok(SagaOrigin::External(take_lp_bytes(buf)?)),
        1 => Ok(SagaOrigin::Module(take_lp_string(buf)?)),
        2 => Ok(SagaOrigin::System),
        d => Err(format!("snapshot has unknown origin discriminant {d}")),
    }
}

/// a section count, bounded by what the remaining input could possibly hold
/// given each entry's minimum encoded size — rejected before the loop builds
/// anything.
fn take_count(buf: &mut &[u8], min_entry_bytes: u64, what: &str) -> Result<u64, String> {
    let count = take_u64(buf)?;
    if count
        .checked_mul(min_entry_bytes)
        .is_none_or(|need| need > buf.len() as u64)
    {
        return Err(format!("snapshot {what} count exceeds input"));
    }
    Ok(count)
}

/// enforce strictly-ascending map keys while inserting.
fn insert_ascending<V>(map: &mut BTreeMap<String, V>, key: String, value: V) -> Result<(), String> {
    if let Some((last, _)) = map.iter().next_back()
        && last.as_str() >= key.as_str()
    {
        return Err("snapshot keys not strictly ascending".into());
    }
    map.insert(key, value);
    Ok(())
}

fn contains_run_separator(value: &str) -> bool {
    value.contains(RUN_KEY_SEPARATOR)
}

fn reject_run_separator(field: &str, value: &str) -> Result<(), Error> {
    if contains_run_separator(value) {
        return Err(Error::Module(format!(
            "{field} must not contain the reserved unit separator"
        )));
    }
    Ok(())
}

/// a decoded pending entry must derive exactly its own key: the dispatch id
/// is the hex sha256 of the run id its fields produce, and the field shapes
/// must match the chat/job keyspace they claim.
fn validate_decoded_pending(dispatch_id: &str, p: &PendingState) -> Result<(), String> {
    if contains_run_separator(&p.agent_id) {
        return Err("snapshot agent_id contains reserved unit separator".into());
    }
    match &p.job_id {
        Some(job_id) => {
            if contains_run_separator(job_id) {
                return Err("snapshot job_id contains reserved unit separator".into());
            }
            if !p.channel_id.is_empty() || p.anchor_seq != 0 || p.thread_root.is_some() {
                return Err("snapshot job entry carries chat coordinates".into());
            }
        }
        None => {
            if p.job_claim_height != 0 {
                return Err("snapshot chat entry has non-zero job claim height".into());
            }
            if contains_run_separator(&p.channel_id) {
                return Err("snapshot channel_id contains reserved unit separator".into());
            }
        }
    }
    if dispatch_id != dispatch_id_for(&p.run_id()) {
        return Err("snapshot dispatch id does not match its run fields".into());
    }
    Ok(())
}

type Committed = (
    BTreeMap<String, TurnPolicy>,
    BTreeMap<String, PendingState>,
);

fn decode_committed(mut buf: &[u8]) -> Result<Committed, String> {
    // per-entry minimum sizes: a watch costs its id prefix and a policy
    // discriminant; a pending entry its three length prefixes, anchor, two
    // option tags, claim height, origin discriminant, and created_at.
    const MIN_WATCH_BYTES: u64 = 8 + 1;
    const MIN_PENDING_BYTES: u64 = 8 + 8 + 8 + 8 + 1 + 1 + 8 + 1 + 8;

    let mut watches: BTreeMap<String, TurnPolicy> = BTreeMap::new();
    let count = take_count(&mut buf, MIN_WATCH_BYTES, "watch")?;
    for _ in 0..count {
        let channel = take_lp_string(&mut buf)?;
        if contains_run_separator(&channel) {
            return Err("snapshot channel_id contains reserved unit separator".into());
        }
        let policy = match take(&mut buf, 1)?[0] {
            0 => TurnPolicy::Mention,
            1 => TurnPolicy::All,
            2 => TurnPolicy::Assigned(take_lp_string(&mut buf)?),
            3 => TurnPolicy::RoundRobin,
            d => return Err(format!("snapshot has unknown turn policy {d}")),
        };
        insert_ascending(&mut watches, channel, policy)?;
    }

    let mut pending: BTreeMap<String, PendingState> = BTreeMap::new();
    let count = take_count(&mut buf, MIN_PENDING_BYTES, "pending")?;
    for _ in 0..count {
        let dispatch_id = take_lp_string(&mut buf)?;
        let agent_id = take_lp_string(&mut buf)?;
        let channel_id = take_lp_string(&mut buf)?;
        let anchor_seq = take_u64(&mut buf)?;
        let thread_root = take_opt_u64(&mut buf)?;
        let job_id = take_opt_string(&mut buf)?;
        let job_claim_height = take_u64(&mut buf)?;
        let requester = take_origin(&mut buf)?;
        let created_at = take_u64(&mut buf)?;
        let entry = PendingState {
            agent_id,
            channel_id,
            anchor_seq,
            thread_root,
            job_id,
            job_claim_height,
            requester,
            created_at,
        };
        validate_decoded_pending(&dispatch_id, &entry)?;
        insert_ascending(&mut pending, dispatch_id, entry)?;
    }

    if !buf.is_empty() {
        return Err("snapshot has trailing bytes".into());
    }
    Ok((watches, pending))
}

// ---- the module -----------------------------------------------------------

pub struct RunsModule {
    id: ModuleId,
    /// genesis config, not state: which module ids the origin router trusts.
    chat: ModuleId,
    /// dead-letter routing only: a saga callback pointed here by a foreign
    /// trigger's `reply_to` must be swallowed, never abort its block.
    saga: ModuleId,
    /// the tagging plane — the engagement intake's trusted origin and the
    /// target of watch subscriptions.
    tagging: ModuleId,
    /// the dispatch plane — every run's recipe registry, executor, and
    /// lifecycle ledger.
    dispatch: ModuleId,
    /// the agent registry — the record book this module reads by query, and
    /// the registry hook's trusted origin.
    agent: ModuleId,
    tasks: Option<ModuleId>,
    jobs: Option<ModuleId>,
    /// the forge module id — the PR/merge sink target (O2). genesis config, NOT
    /// committed state (it never enters `root()`), so it adds no consensus
    /// surface. `None` on nodes not wired for the sink; the sink then degrades
    /// to a breadcrumb.
    forge: Option<ModuleId>,
    /// the duckfs/files module id — queried for the committed head that a
    /// portable (v3) envelope pins as `source_snapshot` (W2). genesis config,
    /// NOT committed state (never in `root()`), so it adds no consensus surface.
    /// its PRESENCE is what selects the portable v3 composer: `Some` composes v3
    /// (pins the committed head), `None` composes the v2 wire.
    files: Option<ModuleId>,
    /// committed state — what `root()` and the app-hash commit to.
    watches: BTreeMap<String, TurnPolicy>,
    /// in-flight correlation entries keyed by dispatch id — pruned on
    /// delivery; the dispatch module owns lifecycle and history.
    pending: BTreeMap<String, PendingState>,
    /// this block's staged writes, read ahead of committed state
    /// (read-your-writes) but merged in — and reflected in `root()` — only at
    /// `commit_block`. a watch stages `None` for removal (unwatch); a pending
    /// entry stages `None` for its prune.
    pending_watches: BTreeMap<String, Option<TurnPolicy>>,
    pending_overlay: BTreeMap<String, Option<PendingState>>,
}

impl RunsModule {
    /// wire the module to its collaborators. the ids must be pairwise
    /// distinct — origin routing is what makes the privileged intakes
    /// spoof-proof, and colliding ids would collapse those namespaces.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<ModuleId>,
        chat: impl Into<ModuleId>,
        saga: impl Into<ModuleId>,
        tagging: impl Into<ModuleId>,
        dispatch: impl Into<ModuleId>,
        agent: impl Into<ModuleId>,
        tasks: Option<ModuleId>,
        jobs: Option<ModuleId>,
    ) -> Self {
        let id = id.into();
        let chat = chat.into();
        let saga = saga.into();
        let tagging = tagging.into();
        let dispatch = dispatch.into();
        let agent = agent.into();
        let mut ids = BTreeSet::from([
            id.clone(),
            chat.clone(),
            saga.clone(),
            tagging.clone(),
            dispatch.clone(),
            agent.clone(),
        ]);
        let mut expected = 6;
        for module in [&tasks, &jobs].into_iter().flatten() {
            ids.insert(module.clone());
            expected += 1;
        }
        assert_eq!(
            ids.len(),
            expected,
            "runs collaborator module ids must be pairwise distinct"
        );
        Self {
            id,
            chat,
            saga,
            tagging,
            dispatch,
            agent,
            tasks,
            jobs,
            forge: None,
            files: None,
            watches: BTreeMap::new(),
            pending: BTreeMap::new(),
            pending_watches: BTreeMap::new(),
            pending_overlay: BTreeMap::new(),
        }
    }

    /// wire the forge module as the PR/merge sink target (O2), after
    /// construction — mirrors the injected `Option<ModuleId>` collaborators so
    /// `new` and every existing call site stay untouched. the PR sink only fires
    /// under a D3 forge-push cap; without this wired the sink degrades to a
    /// breadcrumb.
    pub fn with_sink_forge(mut self, forge: impl Into<ModuleId>) -> Self {
        let forge = forge.into();
        assert!(
            forge != self.id
                && forge != self.chat
                && forge != self.saga
                && forge != self.tagging
                && forge != self.dispatch
                && forge != self.agent
                && Some(&forge) != self.tasks.as_ref()
                && Some(&forge) != self.jobs.as_ref(),
            "forge sink id must be distinct from every other collaborator"
        );
        self.forge = Some(forge);
        self
    }

    /// wire the duckfs/files module so a portable (v3) envelope can pin the
    /// committed head as `source_snapshot` (W2), after construction — mirrors
    /// the injected `Option<ModuleId>` collaborators so `new` and every existing
    /// call site stay untouched. wiring it is what makes the composer emit the
    /// portable v3 wire; unwired, the composer emits the v2 wire.
    pub fn with_files_module(mut self, files: impl Into<ModuleId>) -> Self {
        let files = files.into();
        assert!(
            files != self.id,
            "files module id must be distinct from the runs module id"
        );
        self.files = Some(files);
        self
    }

    // ---- staged-over-committed reads ---------------------------------------

    fn watch(&self, channel_id: &str) -> Option<&TurnPolicy> {
        match self.pending_watches.get(channel_id) {
            Some(staged) => staged.as_ref(),
            None => self.watches.get(channel_id),
        }
    }

    fn pending_entry(&self, dispatch_id: &str) -> Option<&PendingState> {
        match self.pending_overlay.get(dispatch_id) {
            Some(staged) => staged.as_ref(),
            None => self.pending.get(dispatch_id),
        }
    }

    fn visible_ids<'a, V, W>(
        committed: &'a BTreeMap<String, V>,
        pending: &'a BTreeMap<String, W>,
    ) -> Vec<String> {
        pending
            .keys()
            .chain(committed.keys())
            .cloned()
            .collect::<BTreeSet<String>>()
            .into_iter()
            .collect()
    }

    // ---- registry reads --------------------------------------------------------
    // the agent registry is another module's state; these queries see staged
    // same-block registrations through the host's live query routing, so a
    // register-watch-engage cascade works within one block — deterministically,
    // on every validator.

    /// one registry record, or `None` when the agent isn't registered.
    async fn agent_record(
        &self,
        ctx: &dyn Ctx,
        agent_id: &str,
    ) -> Result<Option<AgentRecord>, String> {
        let reply = ctx
            .query(
                &self.agent,
                &agent_encode_query(&AgentQuery::Agent {
                    agent_id: agent_id.to_string(),
                }),
            )
            .await
            .map_err(|e| format!("agent registry query failed: {e}"))?;
        match agent_decode_reply(&reply) {
            Ok(AgentReply::Agent(record)) => Ok(record),
            _ => Err("unexpected agent reply for an agent lookup".into()),
        }
    }

    /// the record, but only while the agent may engage new runs.
    async fn active_agent(
        &self,
        ctx: &dyn Ctx,
        agent_id: &str,
    ) -> Result<Option<AgentRecord>, String> {
        Ok(self
            .agent_record(ctx, agent_id)
            .await?
            .filter(|a| a.status == AgentStatus::Active))
    }

    /// every ACTIVE registered agent, sorted — the deterministic engagement
    /// domain for `All` and `RoundRobin`.
    async fn active_agent_ids(&self, ctx: &dyn Ctx) -> Result<Vec<String>, String> {
        let reply = ctx
            .query(&self.agent, &agent_encode_query(&AgentQuery::Agents))
            .await
            .map_err(|e| format!("agent registry query failed: {e}"))?;
        match agent_decode_reply(&reply) {
            Ok(AgentReply::Agents(records)) => Ok(records
                .into_iter()
                .filter(|a| a.status == AgentStatus::Active)
                .map(|a| a.agent_id)
                .collect()),
            _ => Err("unexpected agent reply for an agents listing".into()),
        }
    }

    // ---- views ---------------------------------------------------------------

    fn pending_view(dispatch_id: &str, p: &PendingState) -> PendingRun {
        PendingRun {
            run_id: p.run_id(),
            dispatch_id: dispatch_id.to_string(),
            agent_id: p.agent_id.clone(),
            channel_id: p.channel_id.clone(),
            anchor_seq: p.anchor_seq,
            thread_root: p.thread_root,
            job_id: p.job_id.clone(),
            job_claim_height: p.job_claim_height,
            requester: p.requester.clone(),
            created_at: p.created_at,
        }
    }

    // ---- shared validation ----------------------------------------------------

    fn validate_non_empty(field: &str, value: &str) -> Result<(), Error> {
        if value.is_empty() {
            return Err(Error::Module(format!("{field} must not be empty")));
        }
        Ok(())
    }

    /// admin ops take a non-empty external key or a module as the submitter.
    /// the pre-consensus empty external default and the system origin (which
    /// any genesis path could wear) never administer watches.
    fn admin_origin(origin: &Origin) -> Result<SagaOrigin, Error> {
        match origin {
            Origin::External(key) if key.is_empty() => Err(Error::Module(
                "runs admin ops require a non-empty submitter id".into(),
            )),
            Origin::System => Err(Error::Module(
                "runs admin ops require an external or module origin".into(),
            )),
            other => Ok(canonical_origin(other)),
        }
    }

    // ---- the turn claim --------------------------------------------------------

    /// whether this dispatch id's turn is already taken: a staged/committed
    /// pending entry settles same-block races; the dispatch module's
    /// PERMANENT dispatch record (committed-only query) settles everything
    /// after — including turns whose pending entry already pruned. without
    /// the second layer a repeat request would re-stage an entry no
    /// `ResultEvent` will ever prune (the dispatch module no-ops duplicate
    /// dispatch ids without telling the dispatcher).
    async fn turn_taken(&self, ctx: &dyn Ctx, dispatch_id: &str) -> Result<bool, String> {
        if self.pending_entry(dispatch_id).is_some() {
            return Ok(true);
        }
        let reply = ctx
            .query(
                &self.dispatch,
                &dispatch_encode_query(&DispatchQuery::Dispatch {
                    receiver: self.id.clone(),
                    dispatch_id: dispatch_id.to_string(),
                }),
            )
            .await
            .map_err(|e| format!("dispatch lookup failed: {e}"))?;
        match dispatch_decode_reply(&reply) {
            Ok(DispatchReply::Dispatch(view)) => Ok(view.is_some()),
            _ => Err("unexpected dispatch reply for a dispatch lookup".into()),
        }
    }

    // ---- context pinning (P4) --------------------------------------------------

    /// pin the transcript window ending at `anchor_seq`: query chat for the
    /// (at most [`CONTEXT_WINDOW`]) newest messages up to and including the
    /// anchor. staged same-block writes are visible through the host's live
    /// query routing, so an engagement fired by a post sees the post itself —
    /// deterministically, on every validator. also returns the anchor's
    /// thread root so the reply can join the same thread.
    async fn pin_context(
        &self,
        ctx: &dyn Ctx,
        channel_id: &str,
        anchor_seq: u64,
    ) -> Result<(Option<u64>, Vec<MessageView>), String> {
        if anchor_seq == 0 {
            return Err("anchor_seq must be >= 1".into());
        }
        let from = anchor_seq.saturating_sub(CONTEXT_WINDOW - 1).max(1);
        let reply = ctx
            .query(
                &self.chat,
                &chat_encode_query(&ChatQuery::MessagesRange {
                    channel_id: channel_id.to_string(),
                    from_seq: from,
                    limit: anchor_seq - from + 1,
                }),
            )
            .await
            .map_err(|e| format!("chat query failed: {e}"))?;
        let window = match chat_decode_reply(&reply) {
            Ok(ChatReply::Messages(window)) => window,
            _ => return Err("unexpected chat reply for a transcript query".into()),
        };
        // the sequence space is gap-free (P3), so the anchor exists exactly
        // when it is the window's last message.
        let Some(anchor) = window.last().filter(|m| m.seq == anchor_seq) else {
            return Err(format!("anchor does not exist: {channel_id}/{anchor_seq}"));
        };
        let thread_root = anchor.head.thread;
        Ok((thread_root, window))
    }

    // ---- payload preparation (the dispatch plane's composition rule) -----

    /// resolve the portable (v3) inputs, or `None` when the files module is
    /// unwired.
    ///
    /// the files module's PRESENCE is the composer's v2-vs-v3 selector — it is
    /// genesis-uniform infrastructure, not a version tag. unwired, no files query
    /// is issued and the composer takes its byte-identical v2 path. wired, every
    /// validator resolves the SAME committed head, so the composed v3 bytes are
    /// consensus-uniform; a head query that FAILS becomes the run's error (a
    /// loud skip at the callsite), never a silent unpinned source.
    async fn portable_inputs(
        &self,
        ctx: &dyn Ctx,
        agent: &AgentRecord,
    ) -> Result<Option<envelope::PortableInputs>, String> {
        let Some(files) = self.files.clone() else {
            return Ok(None);
        };
        let source_snapshot = self.duckfs_head(ctx, &files).await?;
        let skills = agent
            .skills
            .iter()
            .map(|s| envelope::SkillEnvelope {
                name: s.name.clone(),
                source_prefix: s.source_prefix.clone(),
                // a pinned skill passes its snapshot through; a tracking skill
                // (no pin) resolves to the SAME committed head this run pins its
                // workspace to (W2) — deterministic across validators.
                source_snapshot: s.source_snapshot.clone().or_else(|| source_snapshot.clone()),
            })
            .collect();
        Ok(Some(envelope::PortableInputs {
            source_snapshot,
            skills,
        }))
    }

    /// the committed duckfs head — a dispatch-start committed read, so the
    /// pinned id is consensus-uniform across validators (W2). errors become the
    /// run's (a clean skip/error at the callsite), never a silent unpinned
    /// compose. `RefsInfo.head` is `None` on a fresh network (a legitimate null
    /// pin), which is distinct from the files module being unwired.
    async fn duckfs_head(
        &self,
        ctx: &dyn Ctx,
        files: &ModuleId,
    ) -> Result<Option<String>, String> {
        let reply = ctx
            .query(files, &files_encode_query(&FilesQuery::Refs {}))
            .await
            .map_err(|e| format!("files refs query failed: {e}"))?;
        match files_decode_reply(&reply) {
            Ok(FilesReply::Refs(info)) => Ok(info.head),
            Ok(_) => Err("unexpected files reply for a refs query".into()),
            Err(e) => Err(format!("files refs reply failed to decode: {e}")),
        }
    }

    /// everything a chat run's dispatch needs, prepared read-only: the pinned
    /// context (P4) and the fully composed payload. any failure here is a
    /// clean skip for the no-fail engagement intake and a clean error for an
    /// explicit `RequestRun`.
    async fn prepare_dispatch(
        &self,
        ctx: &dyn Ctx,
        agent: &AgentRecord,
        channel_id: &str,
        anchor_seq: u64,
    ) -> Result<PreparedDispatch, String> {
        let (thread_root, transcript) = self.pin_context(ctx, channel_id, anchor_seq).await?;
        let portable = self.portable_inputs(ctx, agent).await?;
        let payload = envelope::render_payload(
            &self.id,
            agent,
            channel_id,
            anchor_seq,
            thread_root,
            &transcript,
            portable,
        )
        .into_bytes();
        if payload.len() > MAX_PAYLOAD_BYTES {
            return Err(format!(
                "composed payload is {} bytes; the dispatch cap is {MAX_PAYLOAD_BYTES}",
                payload.len()
            ));
        }
        Ok(PreparedDispatch {
            thread_root,
            payload,
        })
    }

    /// stage a chat run's dispatch — one atomic unit with whatever op caused
    /// it (P2). the recipe is the agent's own (`agent/{agent_id}`, registered
    /// by the registry hook); the result lands as a next-block `ResultEvent`
    /// keyed by the dispatch id, which prunes the entry staged here.
    #[allow(clippy::too_many_arguments)]
    fn stage_dispatch_run(
        &mut self,
        ctx: &mut dyn Ctx,
        run_id: &str,
        agent_id: String,
        channel_id: String,
        anchor_seq: u64,
        requester: SagaOrigin,
        prepared: PreparedDispatch,
    ) {
        let now = ctx.env().consensus_time;
        let dispatch_id = dispatch_id_for(run_id);
        ctx.emit_msg(Msg {
            target: self.dispatch.clone(),
            payload: dispatch_encode_msg(&DispatchMsg::Dispatch {
                dispatch_id: dispatch_id.clone(),
                recipe_id: recipe_id_for(&agent_id),
                payload: prepared.payload,
            }),
        });
        self.pending_overlay.insert(
            dispatch_id,
            Some(PendingState {
                agent_id,
                channel_id,
                anchor_seq,
                thread_root: prepared.thread_root,
                job_id: None,
                job_claim_height: 0,
                requester,
                created_at: now,
            }),
        );
    }

    /// an observability breadcrumb for the no-fail arms: dropped payloads,
    /// skipped engagements, and failed runs leave the state machine as
    /// events, never as errors.
    fn note(&self, ctx: &mut dyn Ctx, what: String) {
        ctx.emit_event(Event {
            source: self.id.clone(),
            payload: what.into_bytes(),
        });
    }

    // ---- the registry hook (origin == agent) ------------------------------------

    /// keep an agent's dispatch-plane recipe in lockstep with its registry
    /// record. this intake rides the registry write's own block and — unlike
    /// every other module intake — MAY error: an `Err` aborts that write,
    /// which is exactly the atomicity the recipe seam needs (a squatted or
    /// oversized recipe id must fail the registration, not land a record
    /// without a recipe).
    fn on_agent_event(&mut self, ctx: &mut dyn Ctx, payload: &[u8]) -> Result<(), Error> {
        match agent_decode_event(payload).map_err(Error::Module)? {
            AgentEvent::Registered {
                agent_id,
                capability,
            } => {
                // the agent's recipe id must fit the dispatch plane's id cap,
                // or the recipe registration below could never land.
                if recipe_id_for(&agent_id).len() > dispatch::MAX_ID_BYTES {
                    return Err(Error::Module(format!(
                        "agent_id is too long for its dispatch recipe id (cap {})",
                        dispatch::MAX_ID_BYTES - recipe_id_for("").len()
                    )));
                }
                ctx.emit_msg(Msg {
                    target: self.dispatch.clone(),
                    payload: dispatch_encode_msg(&DispatchMsg::RegisterRecipe {
                        recipe_id: recipe_id_for(&agent_id),
                        description: format!("runs for agent {agent_id}"),
                        capability,
                        routing: Routing::Rendezvous,
                        // Text on purpose: the oracle returns the model's raw
                        // answer and THIS module normalizes it — a strict
                        // Json contract would fail every prose reply.
                        output_contract: OutputContract::Text,
                        max_attempts: RUN_MAX_ATTEMPTS,
                        deadline_views: Some(RUN_DEADLINE_VIEWS),
                        lease_views: None,
                    }),
                });
                Ok(())
            }
            AgentEvent::CapabilityChanged {
                agent_id,
                capability,
            } => {
                // keep the agent's dispatch recipe on the same tag,
                // atomically with the record change.
                ctx.emit_msg(Msg {
                    target: self.dispatch.clone(),
                    payload: dispatch_encode_msg(&DispatchMsg::UpdateRecipe {
                        recipe_id: recipe_id_for(&agent_id),
                        description: None,
                        capability: Some(capability),
                        routing: None,
                        output_contract: None,
                        max_attempts: None,
                    }),
                });
                Ok(())
            }
        }
    }

    // ---- the jobs intake (origin == jobs) -----------------------------------------

    /// NO-FAIL ARM. jobs submits fan out in the submitter's block; the event
    /// carries the full spec because committed-only jobs queries cannot see
    /// the just-staged job. the claim and the dispatch are staged together:
    /// the single claiming-worker cascade rule makes the emitted claim safe,
    /// and the dispatch rides the agent's own recipe like every other run.
    async fn on_jobs_event(&mut self, ctx: &mut dyn Ctx, payload: &[u8]) -> Result<(), Error> {
        let Ok(event) = jobs_decode_event(payload) else {
            self.note(ctx, "dropped undecodable jobs event".into());
            return Ok(());
        };
        let JobsEvent::Submitted {
            job_id,
            kind,
            spec,
            spec_hash,
            ..
        } = event;
        if contains_run_separator(&job_id) {
            self.note(
                ctx,
                format!("dropped jobs event with invalid job id {job_id}"),
            );
            return Ok(());
        }
        let Some(agent_id) = kind.strip_prefix("agent/").filter(|id| !id.is_empty()) else {
            return Ok(());
        };
        if contains_run_separator(agent_id) {
            self.note(
                ctx,
                format!("dropped jobs event with invalid agent id {agent_id}"),
            );
            return Ok(());
        }
        // the record rides into the envelope below (agent id + prompt pin).
        let agent = match self.active_agent(&*ctx, agent_id).await {
            Ok(Some(agent)) => agent,
            // an unknown or paused agent leaves the job on the board.
            Ok(None) => return Ok(()),
            Err(reason) => {
                self.note(ctx, format!("dropped jobs event for {job_id}: {reason}"));
                return Ok(());
            }
        };
        let Some(jobs) = self.jobs.clone() else {
            self.note(
                ctx,
                "dropped jobs event without configured jobs module".into(),
            );
            return Ok(());
        };
        if job_spec_hash(spec.as_bytes()) != spec_hash {
            self.note(
                ctx,
                format!("dropped jobs event for {job_id}: spec does not hash to spec_hash"),
            );
            return Ok(());
        }

        let claim_height = ctx.env().height;
        let run_id = job_run_id_for(&job_id, agent_id, claim_height);
        let dispatch_id = dispatch_id_for(&run_id);
        match self.turn_taken(&*ctx, &dispatch_id).await {
            Ok(false) => {}
            Ok(true) => return Ok(()),
            Err(reason) => {
                self.note(ctx, format!("job run skipped for {run_id}: {reason}"));
                return Ok(());
            }
        }
        // compose BEFORE claiming: a job whose payload cannot be composed
        // (an oversized spec, or an unpinnable portable source) is left
        // unclaimed on the board, not claimed into a run that could never
        // execute. the portable resolve is a loud skip like the rest of this
        // no-fail arm.
        let portable = match self.portable_inputs(&*ctx, &agent).await {
            Ok(portable) => portable,
            Err(reason) => {
                self.note(ctx, format!("job run skipped for {run_id}: {reason}"));
                return Ok(());
            }
        };
        let payload = envelope::render_job_payload(&agent, &job_id, &spec, portable).into_bytes();
        if payload.len() > MAX_PAYLOAD_BYTES {
            self.note(
                ctx,
                format!("job run skipped for {run_id}: payload exceeds the dispatch cap"),
            );
            return Ok(());
        }

        let now = ctx.env().consensus_time;
        let requester = canonical_origin(&ctx.env().origin);
        ctx.emit_msg(Msg {
            target: jobs,
            payload: jobs_encode_msg(&JobsMsg::Claim {
                job_id: job_id.clone(),
                lease_views: JOB_RUN_LEASE_VIEWS,
            }),
        });
        ctx.emit_msg(Msg {
            target: self.dispatch.clone(),
            payload: dispatch_encode_msg(&DispatchMsg::Dispatch {
                dispatch_id: dispatch_id.clone(),
                recipe_id: recipe_id_for(agent_id),
                payload,
            }),
        });
        self.pending_overlay.insert(
            dispatch_id,
            Some(PendingState {
                agent_id: agent_id.to_string(),
                channel_id: String::new(),
                anchor_seq: 0,
                thread_root: None,
                job_id: Some(job_id),
                job_claim_height: claim_height,
                requester,
                created_at: now,
            }),
        );
        Ok(())
    }

    // ---- the engagement intake (origin == tagging) ----------------------------------

    /// which agents an engagement engages under `policy`. only ACTIVE
    /// registered agents ever engage; every branch reads agreed state only
    /// (registry queries included — they are consensus reads).
    async fn engaged_agents(
        &self,
        ctx: &dyn Ctx,
        policy: &TurnPolicy,
        tags: &[EntityRef],
        seq: u64,
    ) -> Result<Vec<String>, String> {
        match policy {
            // entity tags naming THIS module's agents, in content order (the
            // content module dedupes them).
            TurnPolicy::Mention => {
                let mut engaged = Vec::new();
                for tag in tags {
                    if tag.module != self.id {
                        continue;
                    }
                    if self.active_agent(ctx, &tag.entity).await?.is_some() {
                        engaged.push(tag.entity.clone());
                    }
                }
                Ok(engaged)
            }
            TurnPolicy::All => self.active_agent_ids(ctx).await,
            TurnPolicy::Assigned(agent_id) => {
                Ok(if self.active_agent(ctx, agent_id).await?.is_some() {
                    vec![agent_id.clone()]
                } else {
                    Vec::new()
                })
            }
            TurnPolicy::RoundRobin => {
                let active = self.active_agent_ids(ctx).await?;
                Ok(if active.is_empty() {
                    Vec::new()
                } else {
                    vec![active[(seq % active.len() as u64) as usize].clone()]
                })
            }
        }
    }

    /// NO-FAIL ARM. the tagging plane routes a user post here in the same
    /// block as the post itself — an `Err` would abort the post (and every
    /// other subscriber's delivery), so malformed events, unwatched
    /// channels, failed context pins, and oversized payloads are all staged
    /// no-ops. the plane's loop rule guarantees the
    /// event is user-authored.
    async fn on_engagement(&mut self, ctx: &mut dyn Ctx, payload: &[u8]) -> Result<(), Error> {
        let Ok(event) = tagging_decode_event(payload) else {
            self.note(ctx, "dropped undecodable engagement event".into());
            return Ok(());
        };
        let EngagementEvent {
            source,
            container: channel_id,
            content_seq: seq,
            author: _,
            tags,
        } = event;
        if source != self.chat {
            // this module only understands chat containers; a subscription
            // to another source would be a config bug, not a block abort.
            self.note(ctx, format!("dropped engagement from source {source}"));
            return Ok(());
        }
        let Some(policy) = self.watch(&channel_id).cloned() else {
            // an engagement for a channel we no longer watch (subscription
            // and watch drift within a block): a no-op, never an error.
            return Ok(());
        };

        let engaged = match self.engaged_agents(&*ctx, &policy, &tags, seq).await {
            Ok(engaged) => engaged,
            Err(reason) => {
                self.note(ctx, format!("engagement skipped for {channel_id}: {reason}"));
                return Ok(());
            }
        };
        let requester = canonical_origin(&ctx.env().origin);
        for agent_id in engaged {
            let run_id = run_id_for(&channel_id, seq, &agent_id);
            let dispatch_id = dispatch_id_for(&run_id);
            match self.turn_taken(&*ctx, &dispatch_id).await {
                // the turn claim: the first creation in consensus order won.
                Ok(true) => continue,
                Ok(false) => {}
                Err(reason) => {
                    self.note(ctx, format!("run skipped for {run_id}: {reason}"));
                    continue;
                }
            }
            let agent = match self.active_agent(&*ctx, &agent_id).await {
                Ok(Some(agent)) => agent,
                Ok(None) => continue,
                Err(reason) => {
                    self.note(ctx, format!("run skipped for {run_id}: {reason}"));
                    continue;
                }
            };
            match self
                .prepare_dispatch(&*ctx, &agent, &channel_id, seq)
                .await
            {
                Ok(prepared) => self.stage_dispatch_run(
                    ctx,
                    &run_id,
                    agent_id,
                    channel_id.clone(),
                    seq,
                    requester.clone(),
                    prepared,
                ),
                // a failed preparation must not poison the posting block —
                // same no-fail reasoning as the result intake.
                Err(reason) => self.note(ctx, format!("run skipped for {run_id}: {reason}")),
            }
        }
        Ok(())
    }

    // ---- the result intake (origin == dispatch) ------------------------------------

    /// NO-FAIL ARM. the dispatch plane delivers a run's outcome here inside
    /// its delivery block; an `Err` would abort that block, the committed
    /// mailbox would re-inject next block, and every block after would abort
    /// (the permanent-abort loop). unknown dispatch ids are staged no-ops;
    /// the model's raw text is normalized deterministically, and a response
    /// that fails validation FAILS THE RUN — breadcrumb + pruned entry —
    /// never the block. the entry prunes on EVERY matched delivery: the
    /// dispatch module is the lifecycle ledger, this map is only the
    /// correlation for work still in flight.
    async fn on_result_event(&mut self, ctx: &mut dyn Ctx, payload: &[u8]) -> Result<(), Error> {
        let Ok(event) = decode_result_event(payload) else {
            self.note(ctx, "dropped undecodable dispatch result event".into());
            return Ok(());
        };
        let Some(entry) = self.pending_entry(&event.dispatch_id).cloned() else {
            self.note(
                ctx,
                format!("dropped result for unknown dispatch {}", event.dispatch_id),
            );
            return Ok(());
        };
        let run_id = entry.run_id();
        let ResultEvent {
            dispatch_id,
            outcome,
            ..
        } = event;
        self.pending_overlay.insert(dispatch_id, None);

        match outcome {
            // THE single delivery path: decode the runner result and apply
            // whatever facets it carries. a plain (message-only) result carries
            // none — it delivers exactly the model prose + its parsed actions.
            Ok(bytes) => self.deliver_run_result(ctx, &run_id, &entry, &bytes).await,
            Err(reason) => self.fail_run(ctx, &run_id, &entry, reason).await,
        }
        Ok(())
    }

    /// the failure triple (breadcrumb note + threaded failure reply + job
    /// finalize false) — unchanged behavior, was inlined three times.
    async fn fail_run(&mut self, ctx: &mut dyn Ctx, run_id: &str, entry: &PendingState, reason: String) {
        self.note(ctx, format!("run {run_id} failed: {reason}"));
        self.emit_failure_reply(ctx, run_id, entry, &reason).await;
        self.emit_job_finalize_if_current_claimant(ctx, entry, false, reason)
            .await;
    }

    /// THE single delivery path. message facet + host-assembled effects → one
    /// [`AgentResponse`] (validate/emit reused); the sink is applied (cap-gated,
    /// probe-guarded, degrades to a breadcrumb, never aborts); data (R5) +
    /// artifact (O1) + status fold into the faceted finalize payload. a plain
    /// (message-only) result — raw text or an `AgentResponse` with no runner
    /// marker — decodes to a facet-free [`RunnerResult`] (Chain sink, Ok status,
    /// empty effects), so it delivers exactly the model prose + its parsed
    /// actions. idempotent by run_id — every effect applies once, here, from the
    /// winning attempt (X2); nothing is emitted mid-run.
    async fn deliver_run_result(
        &mut self,
        ctx: &mut dyn Ctx,
        run_id: &str,
        entry: &PendingState,
        bytes: &[u8],
    ) {
        let result = match decode_run_result_v1(bytes) {
            Ok(r) => r,
            Err(reason) => return self.fail_run(ctx, run_id, entry, reason).await,
        };
        // the host observation overrides a present message facet (R4).
        if result.status == WireStatus::Failed {
            return self
                .fail_run(ctx, run_id, entry, "run reported a failed status".into())
                .await;
        }
        let mut response = agent_response_from_text(&result.response_text, entry.job_id.is_some());
        // R1: host-assembled effects are authoritative. FALLBACK: only override
        // the response-parsed actions when the effects facet is non-empty, so a
        // model that emitted actions only in prose (an oracle that didn't lift
        // them) still gets them applied — never a silent drop. a message-only
        // result has empty effects, so it keeps its prose-parsed actions.
        if !result.effects.is_empty() {
            response.actions = match effects_to_actions(&result.effects) {
                Ok(actions) => actions,
                Err(reason) => return self.fail_run(ctx, run_id, entry, reason).await,
            };
        }
        let response = match self.validate_response(&*ctx, run_id, entry, response).await {
            Ok(r) => r,
            Err(reason) => return self.fail_run(ctx, run_id, entry, reason).await,
        };
        // build the faceted finalize payload BEFORE moving `response` into
        // emit_response; emission order is response → sink → finalize.
        let payload =
            encode_delivery_receipt(&response, valid_data(&result.data), &result.workspace_receipt, result.status);
        self.emit_response(ctx, run_id, entry, response);
        self.emit_sink(ctx, run_id, entry, &result.sink).await;
        self.emit_job_finalize_if_current_claimant(ctx, entry, true, payload)
            .await;
    }

    /// apply the O1/O2 sink. Chain is a breadcrumb/no-op in v1 (durable
    /// output_ref chaining is future work — the receipt already carries the
    /// output_ref for a downstream consumer). Pr emits a forge `OpenPr` gated on
    /// the agent's D3 `ForgePush` cap (Phase 4's `permits`, NOT a KNOWN_ACTIONS
    /// grant) and a committed-state branch-born probe (the no-fail rule: an
    /// OpenPr for an unborn branch would abort the block). Merge is inert in v1.
    /// any missing precondition degrades to a breadcrumb — the sink NEVER aborts
    /// the delivery block.
    async fn emit_sink(&self, ctx: &mut dyn Ctx, run_id: &str, entry: &PendingState, sink: &WireSink) {
        match sink {
            WireSink::Chain => {}
            WireSink::Pr {
                repo,
                source_branch,
                target_branch,
                title,
                body,
            } => {
                let Some(forge) = self.forge.clone() else {
                    return self.note(ctx, format!("run {run_id} pr sink skipped: no forge module wired"));
                };
                let agent = match self.agent_record(&*ctx, &entry.agent_id).await {
                    Ok(Some(a)) => a,
                    _ => {
                        return self
                            .note(ctx, format!("run {run_id} pr sink skipped: agent not registered"));
                    }
                };
                if !agent.permits(&CapRequest::ForgePush(repo.as_str())) {
                    return self.note(
                        ctx,
                        format!("run {run_id} pr sink skipped: agent lacks forge_push for {repo}"),
                    );
                }
                match self.forge_branch_born(&*ctx, &forge, repo, source_branch).await {
                    Ok(true) => {}
                    Ok(false) => {
                        return self.note(
                            ctx,
                            format!("run {run_id} pr sink skipped: source branch not present"),
                        );
                    }
                    Err(why) => {
                        return self.note(ctx, format!("run {run_id} pr sink skipped: {why}"));
                    }
                }
                ctx.emit_msg(Msg {
                    target: forge,
                    payload: forge_open_pr_bytes(repo, title, body, source_branch, target_branch),
                });
            }
            WireSink::Merge { repo, number, .. } => {
                // v1: the merge sink needs a host-computed merge pack (a phase-2
                // wrapper responsibility). validate the wire, breadcrumb, and
                // fall through like Chain — never emit a MergePr yet.
                self.note(
                    ctx,
                    format!("run {run_id} merge sink for {repo}#{number} is inert in v1 (treated as chain)"),
                );
            }
        }
    }

    /// deterministic committed-state probe: is `branch` a born ref of `repo`?
    /// reads COMMITTED forge state via a query (never node-local pending), so it
    /// is uniform across validators. decoded via `serde_json::Value` to avoid a
    /// production dependency on the forge crate.
    async fn forge_branch_born(
        &self,
        ctx: &dyn Ctx,
        forge: &str,
        repo: &str,
        branch: &str,
    ) -> Result<bool, String> {
        let reply = ctx
            .query(
                forge,
                &serde_json::to_vec(&ForgeSinkQuery::ListRefs { repo }).expect("query serializes"),
            )
            .await
            .map_err(|e| format!("forge refs lookup failed: {e}"))?;
        let value: serde_json::Value =
            serde_json::from_slice(&reply).map_err(|e| format!("undecodable forge reply: {e}"))?;
        let Some(refs) = value.get("refs").and_then(|r| r.as_array()) else {
            return Err("unexpected forge reply for a refs listing".into());
        };
        Ok(refs
            .iter()
            .any(|r| r.get("name").and_then(|n| n.as_str()) == Some(branch)))
    }

    /// deterministic response validation — THE safety boundary (design §5).
    /// the response is data until every check here passes; only then do its
    /// follow-ups exist. beyond grants and caps, this probes everything the
    /// emitted follow-ups could make chat or tasks REJECT (which would abort
    /// the delivery block — the no-fail rule): a squatted reply message id, a
    /// full thread, a duplicate or unknown task id.
    async fn validate_response(
        &self,
        ctx: &dyn Ctx,
        run_id: &str,
        entry: &PendingState,
        response: AgentResponse,
    ) -> Result<AgentResponse, String> {
        let agent = self
            .agent_record(ctx, &entry.agent_id)
            .await?
            .ok_or_else(|| format!("agent is not registered: {}", entry.agent_id))?;
        if response.reply_blocks.is_empty() && response.actions.is_empty() {
            return Err("response carries neither reply blocks nor actions".into());
        }
        if response.actions.len() > MAX_ACTIONS_PER_RUN {
            return Err(format!(
                "{} actions exceed the cap of {MAX_ACTIONS_PER_RUN}",
                response.actions.len()
            ));
        }

        if !response.reply_blocks.is_empty() {
            if !allows(&agent, ACTION_CHAT_POST) {
                return Err(format!(
                    "agent {} is not allowed to {ACTION_CHAT_POST}",
                    entry.agent_id
                ));
            }
            let reply_bytes = serde_json::to_vec(&to_chat_blocks(&response.reply_blocks))
                .expect("blocks are serializable");
            if reply_bytes.len() > MAX_REPLY_BLOCKS_BYTES {
                return Err(format!(
                    "reply blocks are {} bytes; the cap is {MAX_REPLY_BLOCKS_BYTES}",
                    reply_bytes.len()
                ));
            }
            self.probe_reply_postable(ctx, run_id, entry).await?;
        }

        if !response.actions.is_empty() {
            let Some(tasks) = self.tasks.clone() else {
                return Err("no tasks module is configured".into());
            };
            let existing = self.task_ids(ctx, &tasks).await?;
            let mut created: BTreeSet<&str> = BTreeSet::new();
            for action in &response.actions {
                let name = action.vocabulary_name();
                if !allows(&agent, name) {
                    return Err(format!("agent {} is not allowed to {name}", entry.agent_id));
                }
                match action {
                    AgentAction::CreateTask { task_id, title } => {
                        if task_id.is_empty() || title.is_empty() {
                            return Err("task actions require a non-empty task_id and title".into());
                        }
                        // duplicates — committed or earlier in this very
                        // response — would make tasks reject the follow-up.
                        if existing.contains(task_id) || !created.insert(task_id) {
                            return Err(format!("task already exists: {task_id}"));
                        }
                    }
                    AgentAction::UpdateTaskStatus { task_id, status } => {
                        if task_status(status).is_none() {
                            return Err(format!("unknown task status: {status}"));
                        }
                        if !existing.contains(task_id) && !created.contains(task_id.as_str()) {
                            return Err(format!("unknown task: {task_id}"));
                        }
                    }
                }
            }
        }

        Ok(response)
    }

    /// prove a reply under the run's message id could land in chat RIGHT NOW
    /// — the no-fail rule again: an emitted post must be valid by
    /// construction, so anything chat would reject is probed first.
    async fn probe_reply_postable(
        &self,
        ctx: &dyn Ctx,
        run_id: &str,
        entry: &PendingState,
    ) -> Result<(), String> {
        // message ids are client-chosen, so anyone could squat the reply
        // id before the result lands; chat would reject the duplicate and
        // abort the block. fail the run instead.
        let message_id = reply_message_id(run_id);
        let reply = ctx
            .query(
                &self.chat,
                &chat_encode_query(&ChatQuery::Message {
                    message_id: message_id.clone(),
                }),
            )
            .await
            .map_err(|e| format!("chat message lookup failed: {e}"))?;
        match chat_decode_reply(&reply) {
            Ok(ChatReply::Message(None)) => {}
            Ok(ChatReply::Message(Some(_))) => {
                return Err(format!("reply message id already taken: {message_id}"));
            }
            _ => return Err("unexpected chat reply for a message lookup".into()),
        }
        // a threaded reply must still fit under chat's thread cap.
        if let Some(root_seq) = entry.thread_root {
            let reply = ctx
                .query(
                    &self.chat,
                    &chat_encode_query(&ChatQuery::MessagesRange {
                        channel_id: entry.channel_id.clone(),
                        from_seq: root_seq,
                        limit: 1,
                    }),
                )
                .await
                .map_err(|e| format!("chat thread lookup failed: {e}"))?;
            let Ok(ChatReply::Messages(views)) = chat_decode_reply(&reply) else {
                return Err("unexpected chat reply for a thread lookup".into());
            };
            let root = views
                .first()
                .filter(|v| v.seq == root_seq)
                .ok_or_else(|| format!("thread root does not exist: {root_seq}"))?;
            if root.head.reply_count >= MAX_THREAD_REPLIES as u64 {
                return Err(format!(
                    "thread reply cap reached: {}/{root_seq}",
                    entry.channel_id
                ));
            }
        }
        Ok(())
    }

    /// surface a failed CHAT run as a threaded reply authored by the agent —
    /// same message id as a success reply would use, so the one-reply-per-run
    /// dedup holds and a redelivered result (entry already pruned) can never
    /// double-post. anything that keeps the post from being valid by
    /// construction (job run, unregistered agent, missing chat.post grant,
    /// squatted id, full thread) degrades to the pre-existing breadcrumb-only
    /// silence — never an error on this no-fail arm.
    async fn emit_failure_reply(
        &self,
        ctx: &mut dyn Ctx,
        run_id: &str,
        entry: &PendingState,
        reason: &str,
    ) {
        if entry.job_id.is_some() {
            // job runs have no channel; the finalize payload carries the error.
            return;
        }
        match self.failure_reply(&*ctx, run_id, entry, reason).await {
            Ok(msg) => ctx.emit_msg(msg),
            Err(why) => self.note(ctx, format!("run {run_id} failure not surfaced: {why}")),
        }
    }

    /// the failure post, or the reason it must stay unposted.
    async fn failure_reply(
        &self,
        ctx: &dyn Ctx,
        run_id: &str,
        entry: &PendingState,
        reason: &str,
    ) -> Result<Msg, String> {
        let agent = self
            .agent_record(ctx, &entry.agent_id)
            .await?
            .ok_or_else(|| format!("agent is not registered: {}", entry.agent_id))?;
        // posting the failure is a chat post like any reply — ungranted
        // agents keep the old silent-fail.
        if !allows(&agent, ACTION_CHAT_POST) {
            return Err(format!(
                "agent {} is not allowed to {ACTION_CHAT_POST}",
                entry.agent_id
            ));
        }
        self.probe_reply_postable(ctx, run_id, entry).await?;
        let name = if agent.display_name.is_empty() {
            agent.agent_id.as_str()
        } else {
            agent.display_name.as_str()
        };
        let text = format!("⚠ {name} failed: {}", failure_excerpt(reason));
        Ok(Msg {
            target: self.chat.clone(),
            payload: chat_encode_msg(&ChatMsg::PostMessage {
                channel_id: entry.channel_id.clone(),
                message_id: reply_message_id(run_id),
                blocks: vec![Block::paragraph(text)],
                thread: entry.thread_root,
                as_agent: Some(entry.agent_id.clone()),
            }),
        })
    }

    async fn task_ids(&self, ctx: &dyn Ctx, tasks: &str) -> Result<BTreeSet<String>, String> {
        let reply = ctx
            .query(tasks, &tasks_encode_query(&TaskQuery::List))
            .await
            .map_err(|e| format!("tasks lookup failed: {e}"))?;
        match tasks_decode_reply(&reply) {
            Ok(TaskReply::Tasks(list)) => Ok(list.into_iter().map(|t| t.id).collect()),
            Err(e) => Err(format!("undecodable tasks reply: {e}")),
        }
    }

    fn truncate_job_payload(mut payload: String) -> String {
        if payload.len() <= JOB_FINALIZE_PAYLOAD_BYTES {
            return payload;
        }
        let marker = "\n[truncated by runs to fit jobs payload cap]";
        let mut keep = JOB_FINALIZE_PAYLOAD_BYTES.saturating_sub(marker.len());
        while keep > 0 && !payload.is_char_boundary(keep) {
            keep -= 1;
        }
        payload.truncate(keep);
        payload.push_str(marker);
        payload
    }

    async fn job_claimed_by_run(
        &self,
        ctx: &dyn Ctx,
        job_id: &str,
        claim_height: u64,
    ) -> Result<bool, String> {
        let Some(jobs) = &self.jobs else {
            return Ok(false);
        };
        let reply = ctx
            .query(
                jobs,
                &jobs_encode_query(&JobsQuery::Get {
                    job_id: job_id.to_string(),
                }),
            )
            .await
            .map_err(|e| format!("jobs lookup failed: {e}"))?;
        let job = match jobs_decode_reply(&reply) {
            Ok(JobsReply::Job(job)) => job,
            Ok(_) => return Err("unexpected jobs reply for a job lookup".into()),
            Err(e) => return Err(format!("undecodable jobs reply: {e}")),
        };
        Ok(job.is_some_and(|job| {
            job.status == JobStatus::Processing
                && job.claim.as_ref().map(|claim| claim.worker.as_str()) == Some(self.id.as_str())
                && job.claim.as_ref().map(|claim| claim.claimed_at_height) == Some(claim_height)
        }))
    }

    /// finalize a job-backed run's board item — but only while this module is
    /// still the claimant of exactly the claim episode the run was bound to
    /// (a reclaimed/re-claimed job must not be finalized by a stale run).
    async fn emit_job_finalize_if_current_claimant(
        &self,
        ctx: &mut dyn Ctx,
        entry: &PendingState,
        ok: bool,
        payload: String,
    ) {
        let Some(job_id) = &entry.job_id else {
            return;
        };
        match self
            .job_claimed_by_run(&*ctx, job_id, entry.job_claim_height)
            .await
        {
            Ok(true) => {
                let Some(jobs) = &self.jobs else {
                    self.note(
                        ctx,
                        format!("job {job_id} finalize skipped: no jobs module"),
                    );
                    return;
                };
                ctx.emit_msg(Msg {
                    target: jobs.clone(),
                    payload: jobs_encode_msg(&JobsMsg::Finalize {
                        job_id: job_id.clone(),
                        ok,
                        payload: Self::truncate_job_payload(payload),
                    }),
                });
            }
            Ok(false) => self.note(
                ctx,
                format!("job {job_id} finalize skipped: runs is not current claimant"),
            ),
            Err(reason) => self.note(ctx, format!("job {job_id} finalize skipped: {reason}")),
        }
    }

    /// hand a VALIDATED response its follow-ups: the chat reply (authored as
    /// the agent, threaded like its anchor) and the task writes — all drained
    /// in this same delivery block (P2, P6).
    fn emit_response(
        &self,
        ctx: &mut dyn Ctx,
        run_id: &str,
        entry: &PendingState,
        response: AgentResponse,
    ) {
        if !response.reply_blocks.is_empty() {
            ctx.emit_msg(Msg {
                target: self.chat.clone(),
                payload: chat_encode_msg(&ChatMsg::PostMessage {
                    channel_id: entry.channel_id.clone(),
                    message_id: reply_message_id(run_id),
                    blocks: to_chat_blocks(&response.reply_blocks),
                    thread: entry.thread_root,
                    as_agent: Some(entry.agent_id.clone()),
                }),
            });
        }
        for action in response.actions {
            let target = self
                .tasks
                .clone()
                .expect("actions were validated against a configured tasks module");
            let payload = match action {
                AgentAction::CreateTask { task_id, title } => {
                    tasks_encode_msg(&TaskMsg::CreateTask { task_id, title })
                }
                AgentAction::UpdateTaskStatus { task_id, status } => {
                    tasks_encode_msg(&TaskMsg::UpdateStatus {
                        task_id,
                        status: task_status(&status).expect("status was validated"),
                    })
                }
            };
            ctx.emit_msg(Msg { target, payload });
        }
    }

    // ---- admin ops + explicit runs (any other origin) --------------------------------

    async fn on_admin(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            RunsMsg::WatchChannel { channel_id, policy } => {
                Self::admin_origin(&ctx.env().origin)?;
                Self::validate_non_empty("channel_id", &channel_id)?;
                reject_run_separator("channel_id", &channel_id)?;
                if let TurnPolicy::Assigned(assignee) = &policy
                    && self
                        .agent_record(&*ctx, assignee)
                        .await
                        .map_err(Error::Module)?
                        .is_none()
                {
                    return Err(Error::Module(format!(
                        "assigned agent is not registered: {assignee}"
                    )));
                }
                // the watch and the plane subscription are ONE atomic unit
                // (P2): if the tagging plane rejects the subscription (bad
                // container, subscriber cap), the whole block aborts and the
                // staged watch vanishes with it.
                self.pending_watches
                    .insert(channel_id.clone(), Some(policy));
                ctx.emit_msg(Msg {
                    target: self.tagging.clone(),
                    payload: tagging_encode_msg(&TaggingMsg::Subscribe {
                        source: self.chat.clone(),
                        container: channel_id,
                    }),
                });
                Ok(())
            }
            RunsMsg::UnwatchChannel { channel_id } => {
                Self::admin_origin(&ctx.env().origin)?;
                if self.watch(&channel_id).is_none() {
                    // idempotent: unwatching an unwatched channel stages (and
                    // emits) nothing.
                    return Ok(());
                }
                self.pending_watches.insert(channel_id.clone(), None);
                ctx.emit_msg(Msg {
                    target: self.tagging.clone(),
                    payload: tagging_encode_msg(&TaggingMsg::Unsubscribe {
                        source: self.chat.clone(),
                        container: channel_id,
                    }),
                });
                Ok(())
            }
            RunsMsg::EnableJobWorker { enabled } => {
                Self::admin_origin(&ctx.env().origin)?;
                let jobs = self
                    .jobs
                    .clone()
                    .ok_or_else(|| Error::Module("no jobs module is configured".into()))?;
                let payload = if enabled {
                    jobs_encode_msg(&JobsMsg::RegisterWorker {})
                } else {
                    jobs_encode_msg(&JobsMsg::UnregisterWorker {})
                };
                ctx.emit_msg(Msg {
                    target: jobs,
                    payload,
                });
                Ok(())
            }
            RunsMsg::RequestRun {
                agent_id,
                channel_id,
                anchor_seq,
            } => {
                // an explicit turn claim: same run id, same dedup as the
                // engagement path — first in consensus order wins, the loser
                // no-ops.
                let requester = match &ctx.env().origin {
                    Origin::External(key) if key.is_empty() => {
                        return Err(Error::Module(
                            "run requests require a non-empty submitter id".into(),
                        ));
                    }
                    other => canonical_origin(other),
                };
                let Some(agent) = self
                    .agent_record(&*ctx, &agent_id)
                    .await
                    .map_err(Error::Module)?
                else {
                    return Err(Error::Module(format!("unknown agent: {agent_id}")));
                };
                reject_run_separator("channel_id", &channel_id)?;
                let run_id = run_id_for(&channel_id, anchor_seq, &agent_id);
                if self
                    .turn_taken(&*ctx, &dispatch_id_for(&run_id))
                    .await
                    .map_err(Error::Module)?
                {
                    return Ok(());
                }
                if agent.status != AgentStatus::Active {
                    return Err(Error::Module(format!("agent is paused: {agent_id}")));
                }
                // unlike the engagement intake, an explicit request REJECTS
                // on a failed preparation: this is the root op of its own
                // block, so an error poisons nothing but the request itself.
                let prepared = self
                    .prepare_dispatch(&*ctx, &agent, &channel_id, anchor_seq)
                    .await
                    .map_err(Error::Module)?;
                self.stage_dispatch_run(
                    ctx,
                    &run_id,
                    agent_id,
                    channel_id,
                    anchor_seq,
                    requester,
                    prepared,
                );
                Ok(())
            }
            RunsMsg::CancelRun { run_id } => {
                let submitter = canonical_origin(&ctx.env().origin);
                let dispatch_id = dispatch_id_for(&run_id);
                let Some(entry) = self.pending_entry(&dispatch_id).cloned() else {
                    // not pending: a run whose dispatch already exists is
                    // terminal (delivered and pruned) — cancelling it is an
                    // idempotent no-op; anything else is unknown.
                    return match self.turn_taken(&*ctx, &dispatch_id).await {
                        Ok(true) => Ok(()),
                        Ok(false) => Err(Error::Module(format!("unknown run: {run_id}"))),
                        Err(reason) => Err(Error::Module(reason)),
                    };
                };
                let owner = self
                    .agent_record(&*ctx, &entry.agent_id)
                    .await
                    .map_err(Error::Module)?
                    .map(|a| a.owner);
                // the empty external default can never match: requesters and
                // owners are always non-empty by construction.
                if submitter != entry.requester && Some(&submitter) != owner.as_ref() {
                    return Err(Error::Module(
                        "only the run creator or the agent owner may cancel a run".into(),
                    ));
                }
                // cancel through the dispatch plane; the entry stays pending
                // and the plane's Err("cancelled") delivery prunes it (and
                // finalizes a job-backed run's job) through the ONE result
                // path — no second lifecycle machine here.
                ctx.emit_msg(Msg {
                    target: self.dispatch.clone(),
                    payload: dispatch_encode_msg(&DispatchMsg::CancelDispatch { dispatch_id }),
                });
                Ok(())
            }
        }
    }

    // ---- state-sync ---------------------------------------------------------
    // hand a joiner the committed continuation state as canonical bytes; the
    // consensus-agreed root — never the serving peer — decides whether they land.

    /// serialize the COMMITTED continuation state (never the staged overlay)
    /// into the canonical encoding `root()` commits to. deterministic across
    /// nodes.
    pub fn snapshot(&self) -> Vec<u8> {
        encode_committed(&self.watches, &self.pending)
    }

    /// adopt a peer's snapshot as own committed state — but only after the
    /// decoded temporaries re-derive `expected` via the exact `root()`
    /// algorithm, so a byzantine snapshot cannot land under an agreed root it
    /// doesn't match. all-or-nothing: on any Err this module (and its root)
    /// is byte-identical to before the call. on success the staged overlay is
    /// dropped — a snapshot describes a block boundary, and nothing
    /// half-applied may shadow it.
    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        let (watches, pending) = decode_committed(bytes).map_err(Error::Module)?;
        if committed_root(&watches, &pending) != expected {
            return Err(Error::Module(
                "snapshot does not match expected root".into(),
            ));
        }
        self.watches = watches;
        self.pending = pending;
        self.pending_watches.clear();
        self.pending_overlay.clear();
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl Module for RunsModule {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// state-based commitment: sha256 over the canonical committed encoding —
    /// a length-prefixed fold of every watch and pending-entry field in
    /// sorted-key order. sensitive to every field, so any transition moves
    /// the root. the preimage IS the snapshot encoding.
    fn root(&self) -> StateRoot {
        committed_root(&self.watches, &self.pending)
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(self.snapshot()))
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        // payload namespaces routed by the HOST-ASSIGNED origin — spoof-proof
        // by construction: only the tagging plane's follow-ups reach the
        // engagement intake, only dispatch's reach the result intake, only
        // the registry's reach the recipe hook, and everything else (external
        // submitters included) must decode as a RunsMsg. saga and chat
        // origins are dead-lettered: neither may ever abort the block that
        // carried them.
        let origin = ctx.env().origin.clone();
        match origin {
            Origin::Module(module) if module == self.tagging => {
                self.on_engagement(ctx, &msg.payload).await
            }
            Origin::Module(module) if module == self.dispatch => {
                self.on_result_event(ctx, &msg.payload).await
            }
            Origin::Module(module) if self.jobs.as_ref() == Some(&module) => {
                self.on_jobs_event(ctx, &msg.payload).await
            }
            Origin::Module(module) if module == self.agent => {
                self.on_agent_event(ctx, &msg.payload)
            }
            Origin::Module(module) if module == self.saga => {
                // dead letter: nothing here rides the saga directly, but any
                // trigger's reply_to can point a callback at this module —
                // it must never abort the saga's terminal block.
                self.note(ctx, "dropped a direct saga callback".into());
                Ok(())
            }
            Origin::Module(module) if module == self.chat => {
                // dead letter: chat never notifies this module directly. a
                // stray follow-up must never abort a posting block.
                self.note(ctx, "dropped a direct chat follow-up".into());
                Ok(())
            }
            _ => self.on_admin(ctx, msg).await,
        }
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            RunsQuery::PendingRuns => {
                let runs = Self::visible_ids(&self.pending, &self.pending_overlay)
                    .into_iter()
                    .filter_map(|dispatch_id| {
                        self.pending_entry(&dispatch_id)
                            .map(|p| Self::pending_view(&dispatch_id, p))
                    })
                    .collect();
                Ok(encode_reply(&RunsReply::PendingRuns(runs)))
            }
            RunsQuery::Watches => {
                let watches = Self::visible_ids(&self.watches, &self.pending_watches)
                    .into_iter()
                    .filter_map(|channel_id| {
                        self.watch(&channel_id).map(|policy| WatchView {
                            channel_id: channel_id.clone(),
                            policy: policy.clone(),
                        })
                    })
                    .collect();
                Ok(encode_reply(&RunsReply::Watches(watches)))
            }
        }
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        for (id, staged) in std::mem::take(&mut self.pending_watches) {
            match staged {
                Some(policy) => {
                    self.watches.insert(id, policy);
                }
                None => {
                    self.watches.remove(&id);
                }
            }
        }
        for (dispatch_id, staged) in std::mem::take(&mut self.pending_overlay) {
            match staged {
                Some(entry) => {
                    self.pending.insert(dispatch_id, entry);
                }
                None => {
                    self.pending.remove(&dispatch_id);
                }
            }
        }
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending_watches.clear();
        self.pending_overlay.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent::{
        ACTION_TASKS_CREATE, ACTION_TASKS_UPDATE_STATUS, PROMPT_HASH_LEN,
        encode_event as agent_encode_event, encode_reply as agent_encode_reply,
    };
    use chat::{AuthorRef, MessageHead, decode_msg as chat_decode_msg};
    use dispatch::{
        DispatchStatus, DispatchView, decode_msg as dispatch_decode_msg,
        encode_reply as dispatch_encode_reply, encode_result_event,
    };
    use duckfs_core::{decode_query as files_decode_query, encode_reply as files_encode_reply};
    use futures::executor::block_on;
    use jobs::{
        Claim as JobClaim, Job, encode_event as jobs_encode_event,
        encode_reply as jobs_encode_reply,
    };
    use crate::{decode_reply as runs_decode_reply, encode_msg, encode_query};
    use sdk::{Effect, Env};
    use tagging::{Author, encode_event as tagging_encode_event};
    use tasks::{
        Task, decode_msg as tasks_decode_msg, encode_reply as tasks_encode_reply,
    };

    /// a canned registry: agent id -> record, served by the ctx's "agent"
    /// query arm exactly like the live registry module would answer.
    type Registry = BTreeMap<String, AgentRecord>;

    /// a minimal `Ctx` that captures emitted msgs/effects/events and serves
    /// a canned agent registry, chat transcripts, task lists, job records,
    /// and dispatch records — enough to unit-test `execute` in isolation
    /// (the host provides the real routing in integration).
    struct CaptureCtx {
        env: Env,
        /// agent id -> registry record served by the "agent" arm.
        agents: Registry,
        /// channel -> messages with contiguous seqs starting at 1.
        transcripts: BTreeMap<String, Vec<MessageView>>,
        tasks: Vec<Task>,
        /// dispatch ids the dispatch module already has a record for — the
        /// committed turn-claim layer the module probes.
        taken_dispatches: BTreeSet<String>,
        /// job_id -> board record served by the jobs arm (finalize guard).
        jobs: BTreeMap<String, Job>,
        /// repo -> born branch names, served by the "forge" ListRefs arm (the
        /// sink's committed-state branch-born probe).
        forge_refs: BTreeMap<String, Vec<String>>,
        /// the committed duckfs head served by the "files" Refs arm — the v3
        /// composer's `source_snapshot` pin. `None` = a fresh network (null pin).
        files_head: Option<String>,
        msgs: Vec<Msg>,
        #[allow(dead_code)]
        effects: Vec<Effect>,
        events: Vec<Event>,
    }
    impl CaptureCtx {
        fn new() -> Self {
            Self {
                env: Env {
                    protocol_version: 0,
                    height: 0,
                    consensus_time: 0,
                    origin: Origin::System,
                    me: "runs".into(),
                },
                agents: Registry::new(),
                transcripts: BTreeMap::new(),
                tasks: Vec::new(),
                taken_dispatches: BTreeSet::new(),
                jobs: BTreeMap::new(),
                forge_refs: BTreeMap::new(),
                files_head: None,
                msgs: Vec::new(),
                effects: Vec::new(),
                events: Vec::new(),
            }
        }
        fn at(mut self, view: u64) -> Self {
            self.env.height = view;
            self.env.consensus_time = view;
            self
        }
        /// register a born branch under `repo` (the sink's branch-born probe).
        fn with_forge_ref(mut self, repo: &str, branch: &str) -> Self {
            self.forge_refs.entry(repo.into()).or_default().push(branch.into());
            self
        }
        /// set the committed duckfs head the "files" Refs arm serves (the v3
        /// composer's `source_snapshot`).
        fn with_files_head(mut self, head: &str) -> Self {
            self.files_head = Some(head.into());
            self
        }
        fn with_origin(mut self, origin: Origin) -> Self {
            self.env.origin = origin;
            self
        }
        fn with_tagging_origin(self) -> Self {
            self.with_origin(Origin::Module("tagging".into()))
        }
        fn with_dispatch_origin(self) -> Self {
            self.with_origin(Origin::Module("dispatch".into()))
        }
        fn with_jobs_origin(self) -> Self {
            self.with_origin(Origin::Module("jobs".into()))
        }
        fn with_agent_origin(self) -> Self {
            self.with_origin(Origin::Module("agent".into()))
        }
        fn with_registry(mut self, registry: &Registry) -> Self {
            self.agents = registry.clone();
            self
        }
        fn with_transcript(mut self, channel: &str, messages: Vec<MessageView>) -> Self {
            self.transcripts.insert(channel.into(), messages);
            self
        }
        fn with_task(mut self, id: &str) -> Self {
            self.tasks.push(Task {
                id: id.into(),
                title: id.into(),
                status: TaskStatus::Open,
                created_at: 0,
                updated_at: 0,
            });
            self
        }
        fn with_taken_dispatch(mut self, dispatch_id: &str) -> Self {
            self.taken_dispatches.insert(dispatch_id.into());
            self
        }
        /// a job the board holds as Processing, claimed by "runs" at `height`.
        fn with_claimed_job(mut self, job_id: &str, height: u64) -> Self {
            self.jobs.insert(
                job_id.into(),
                Job {
                    job_id: job_id.into(),
                    kind: "agent/duck".into(),
                    spec: "spec".into(),
                    submitter: "ext:01".into(),
                    status: JobStatus::Processing,
                    attempt: 1,
                    claim: Some(JobClaim {
                        worker: "runs".into(),
                        claimed_at_height: height,
                        lease_views: JOB_RUN_LEASE_VIEWS,
                    }),
                    result: None,
                    created_at_height: height,
                    updated_at_height: height,
                },
            );
            self
        }
        /// decoded chat msgs emitted this dispatch.
        fn chat_msgs(&self) -> Vec<ChatMsg> {
            self.msgs
                .iter()
                .filter(|m| m.target == "chat")
                .map(|m| chat_decode_msg(&m.payload).expect("chat msg"))
                .collect()
        }
        /// decoded task msgs emitted this dispatch.
        fn task_msgs(&self) -> Vec<TaskMsg> {
            self.msgs
                .iter()
                .filter(|m| m.target == "tasks")
                .map(|m| tasks_decode_msg(&m.payload).expect("task msg"))
                .collect()
        }
        /// decoded jobs msgs emitted this dispatch.
        fn job_msgs(&self) -> Vec<JobsMsg> {
            self.msgs
                .iter()
                .filter(|m| m.target == "jobs")
                .map(|m| jobs::decode_msg(&m.payload).expect("jobs msg"))
                .collect()
        }
        /// decoded dispatch-plane msgs emitted this dispatch.
        fn dispatch_msgs(&self) -> Vec<DispatchMsg> {
            self.msgs
                .iter()
                .filter(|m| m.target == "dispatch")
                .map(|m| dispatch_decode_msg(&m.payload).expect("dispatch msg"))
                .collect()
        }
        /// decoded tagging-plane msgs emitted this dispatch.
        fn tagging_msgs(&self) -> Vec<TaggingMsg> {
            self.msgs
                .iter()
                .filter(|m| m.target == "tagging")
                .map(|m| tagging::decode_msg(&m.payload).expect("tagging msg"))
                .collect()
        }
    }
    #[async_trait::async_trait(?Send)]
    impl Ctx for CaptureCtx {
        fn env(&self) -> &Env {
            &self.env
        }
        fn module_root(&self, _target: &str) -> Option<StateRoot> {
            None
        }
        async fn query(&self, target: &str, req: &[u8]) -> Result<Vec<u8>, Error> {
            match target {
                "agent" => match agent::decode_query(req).map_err(Error::Module)? {
                    AgentQuery::Agent { agent_id } => Ok(agent_encode_reply(&AgentReply::Agent(
                        self.agents.get(&agent_id).cloned(),
                    ))),
                    AgentQuery::Agents => Ok(agent_encode_reply(&AgentReply::Agents(
                        self.agents.values().cloned().collect(),
                    ))),
                },
                "chat" => match chat::decode_query(req).map_err(Error::Module)? {
                    ChatQuery::MessagesRange {
                        channel_id,
                        from_seq,
                        limit,
                    } => {
                        let transcript = self.transcripts.get(&channel_id).ok_or_else(|| {
                            Error::Module(format!("unknown channel: {channel_id}"))
                        })?;
                        let head = transcript.len() as u64;
                        let from = from_seq.max(1);
                        let mut window = Vec::new();
                        if limit > 0 && from <= head {
                            let to = head.min(from + limit - 1);
                            window = transcript[(from - 1) as usize..to as usize].to_vec();
                        }
                        Ok(chat::encode_reply(&ChatReply::Messages(window)))
                    }
                    ChatQuery::Message { message_id } => {
                        Ok(chat::encode_reply(&ChatReply::Message(
                            self.transcripts
                                .values()
                                .flatten()
                                .find(|v| v.head.message_id == message_id)
                                .cloned(),
                        )))
                    }
                    _ => Err(Error::QueryUnsupported),
                },
                "tasks" => Ok(tasks_encode_reply(&TaskReply::Tasks(self.tasks.clone()))),
                "jobs" => match jobs::decode_query(req).map_err(Error::Module)? {
                    JobsQuery::Get { job_id } => Ok(jobs_encode_reply(&JobsReply::Job(
                        self.jobs.get(&job_id).cloned(),
                    ))),
                    _ => Err(Error::QueryUnsupported),
                },
                "dispatch" => match dispatch::decode_query(req).map_err(Error::Module)? {
                    DispatchQuery::Dispatch { dispatch_id, .. } => {
                        let view = self.taken_dispatches.contains(&dispatch_id).then(|| {
                            DispatchView {
                                dispatch_id,
                                recipe_id: "agent/x".into(),
                                receiver: "runs".into(),
                                status: DispatchStatus::Delivered,
                                outcome: Some(Ok(Vec::new())),
                                assignee: None,
                                created_at: 0,
                                updated_at: 0,
                            }
                        });
                        Ok(dispatch_encode_reply(&DispatchReply::Dispatch(view)))
                    }
                    _ => Err(Error::QueryUnsupported),
                },
                "files" => match files_decode_query(req).map_err(Error::Module)? {
                    FilesQuery::Refs {} => Ok(files_encode_reply(&FilesReply::Refs(
                        duckfs_core::RefsInfo {
                            head: self.files_head.clone(),
                            pins: BTreeMap::new(),
                            window_len: 0,
                        },
                    ))),
                    _ => Err(Error::QueryUnsupported),
                },
                "forge" => match forge::decode_query(req).map_err(Error::Module)? {
                    forge::ForgeQuery::ListRefs { repo } => {
                        let refs = self
                            .forge_refs
                            .get(&repo)
                            .into_iter()
                            .flatten()
                            .map(|name| forge::RefHead {
                                name: name.clone(),
                                head: "00".repeat(20),
                            })
                            .collect();
                        Ok(forge::encode_reply(&forge::ForgeReply::Refs(refs)))
                    }
                    _ => Err(Error::QueryUnsupported),
                },
                other => Err(Error::UnknownModule(other.into())),
            }
        }
        fn emit_msg(&mut self, msg: Msg) {
            self.msgs.push(msg);
        }
        fn emit_event(&mut self, ev: Event) {
            self.events.push(ev);
        }
        fn request_effect(&mut self, eff: Effect) {
            self.effects.push(eff);
        }
    }

    // ---- fixtures -----------------------------------------------------------

    fn module() -> RunsModule {
        RunsModule::new(
            "runs",
            "chat",
            "saga",
            "tagging",
            "dispatch",
            "agent",
            Some("tasks".into()),
            Some("jobs".into()),
        )
    }

    fn user(byte: u8) -> Origin {
        Origin::External(vec![byte; 32])
    }

    /// entity tags carry the ACTING module's id — the unified agent identity.
    fn agent_tag(agent_id: &str) -> EntityRef {
        EntityRef {
            module: "runs".into(),
            entity: agent_id.into(),
        }
    }

    fn record(agent_id: &str, actions: &[&str]) -> AgentRecord {
        AgentRecord {
            agent_id: agent_id.into(),
            owner: SagaOrigin::External(vec![9; 32]),
            display_name: agent_id.to_uppercase(),
            capability: "model-1".into(),
            prompt_hash: vec![7u8; PROMPT_HASH_LEN],
            allowed_actions: actions.iter().map(|s| s.to_string()).collect(),
            status: AgentStatus::Active,
            created_at: 0,
            updated_at: 0,
            recipe_hash: Vec::new(),
            caps: agent::ResourceCaps::default(),
            skills: Vec::new(),
        }
    }

    fn registry(agents: &[(&str, &[&str])]) -> Registry {
        agents
            .iter()
            .map(|(id, actions)| ((*id).to_string(), record(id, actions)))
            .collect()
    }

    fn pause(registry: &mut Registry, agent_id: &str) {
        registry.get_mut(agent_id).expect("registered").status = AgentStatus::Paused;
    }

    fn message_in(
        channel: &str,
        seq: u64,
        author: AuthorRef,
        text: &str,
        thread: Option<u64>,
    ) -> MessageView {
        MessageView {
            channel_id: channel.into(),
            seq,
            head: MessageHead {
                message_id: format!("{channel}-m{seq}"),
                author,
                blocks: vec![Block::paragraph(text)],
                created_at: 0,
                rev: 0,
                edited_at: None,
                base_rev: None,
                deleted: false,
                thread,
                reply_count: 0,
                last_reply_seq: None,
            },
            reactions: Vec::new(),
            channel_head_seq: seq,
        }
    }

    fn message(seq: u64, text: &str) -> MessageView {
        message_in("general", seq, AuthorRef::User(vec![1; 32]), text, None)
    }

    fn transcript(n: u64) -> Vec<MessageView> {
        (1..=n).map(|i| message(i, &format!("msg {i}"))).collect()
    }

    fn admin(m: &RunsMsg) -> Msg {
        Msg {
            target: "runs".into(),
            payload: encode_msg(m),
        }
    }

    /// the tagging plane's routed report of a user post — the engagement
    /// intake's payload. the plane's loop rule means these are always
    /// user-authored in practice.
    fn engagement(channel: &str, seq: u64, tags: Vec<EntityRef>) -> Msg {
        Msg {
            target: "runs".into(),
            payload: tagging_encode_event(&EngagementEvent {
                source: "chat".into(),
                container: channel.into(),
                content_seq: seq,
                author: Author::User(vec![1; 32]),
                tags,
            }),
        }
    }

    /// the dispatch plane's next-block delivery for a run.
    fn result_event(run_id: &str, outcome: Result<Vec<u8>, String>) -> Msg {
        Msg {
            target: "runs".into(),
            payload: encode_result_event(&ResultEvent {
                dispatch_id: dispatch_id_for(run_id),
                recipe_id: recipe_id_for("bot"),
                outcome,
            }),
        }
    }

    /// a jobs-board submit notification, spec + matching hash included.
    fn jobs_event(job_id: &str, kind: &str, spec: &str) -> Msg {
        Msg {
            target: "runs".into(),
            payload: jobs_encode_event(&JobsEvent::Submitted {
                job_id: job_id.into(),
                kind: kind.into(),
                submitter: "ext:01".into(),
                spec: spec.into(),
                spec_hash: job_spec_hash(spec.as_bytes()),
            }),
        }
    }

    /// the registry hook's payload (origin == agent).
    fn agent_event(event: &AgentEvent) -> Msg {
        Msg {
            target: "runs".into(),
            payload: agent_encode_event(event),
        }
    }

    fn exec(m: &mut RunsModule, ctx: &mut CaptureCtx, op: &Msg) -> Result<(), Error> {
        block_on(m.execute(ctx, op))
    }

    fn commit(m: &mut RunsModule) {
        block_on(m.commit_block()).unwrap();
    }

    fn abort(m: &mut RunsModule) {
        block_on(m.abort_block()).unwrap();
    }

    fn pending_runs(m: &RunsModule) -> Vec<PendingRun> {
        let reply = block_on(m.query(&encode_query(&RunsQuery::PendingRuns))).unwrap();
        match runs_decode_reply(&reply).unwrap() {
            RunsReply::PendingRuns(runs) => runs,
            other => panic!("unexpected reply: {other:?}"),
        }
    }

    fn get_pending(m: &RunsModule, run_id: &str) -> Option<PendingRun> {
        pending_runs(m).into_iter().find(|p| p.run_id == run_id)
    }

    /// a committed module with one watch on "general" under `policy`. the
    /// registry itself lives in each ctx (`with_registry`), never here.
    fn watched(policy: TurnPolicy, registry: &Registry) -> RunsModule {
        let mut m = module();
        let mut ctx = CaptureCtx::new()
            .with_origin(user(9))
            .with_registry(registry);
        exec(
            &mut m,
            &mut ctx,
            &admin(&RunsMsg::WatchChannel {
                channel_id: "general".into(),
                policy,
            }),
        )
        .unwrap();
        commit(&mut m);
        m
    }

    /// drive an engagement at `seq` (author user(1)) tagging `mentioned`.
    fn engage_post(
        m: &mut RunsModule,
        registry: &Registry,
        seq: u64,
        mentioned: &[&str],
    ) -> CaptureCtx {
        let mut ctx = CaptureCtx::new()
            .at(seq)
            .with_tagging_origin()
            .with_registry(registry)
            .with_transcript("general", transcript(seq));
        let tags = mentioned.iter().map(|a| agent_tag(a)).collect();
        exec(m, &mut ctx, &engagement("general", seq, tags)).unwrap();
        ctx
    }

    fn response(reply: &[&str], actions: Vec<AgentAction>) -> Vec<u8> {
        agent::encode_response(&AgentResponse {
            reply_blocks: reply
                .iter()
                .map(|t| ReplyBlock {
                    kind: "paragraph".into(),
                    text: (*t).into(),
                    lang: None,
                })
                .collect(),
            actions,
        })
    }

    // ---- the composer's v2-vs-v3 selection (files presence) ---------------------

    #[test]
    fn a_run_composes_v2_without_files_and_v3_with_files_wired() {
        let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
        let agent = record("bot", &[ACTION_CHAT_POST]);
        let head = "aa".repeat(32);

        // no files module: the byte-identical v2 payload, no portable fields.
        let m0 = module();
        let ctx0 = CaptureCtx::new()
            .with_registry(&registry)
            .with_transcript("general", transcript(2));
        let prepared = block_on(m0.prepare_dispatch(&ctx0, &agent, "general", 2)).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&prepared.payload).unwrap();
        assert_eq!(v["ducktape_run"], 2, "no files module composes v2");
        assert!(v.get("workspace").is_none(), "no v3 workspace without files");
        assert!(v.get("skills").is_none());

        // files wired: the v3 payload pins the committed head.
        let m4 = module().with_files_module("files");
        let ctx4 = CaptureCtx::new()
            .with_registry(&registry)
            .with_transcript("general", transcript(2))
            .with_files_head(&head);
        let prepared = block_on(m4.prepare_dispatch(&ctx4, &agent, "general", 2)).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&prepared.payload).unwrap();
        assert_eq!(v["ducktape_run"], 3, "a wired files module composes v3");
        assert_eq!(v["workspace"]["source_prefix"], "/shared/agent-workspaces/bot");
        assert_eq!(
            v["workspace"]["source_snapshot"], head,
            "source_snapshot pins the committed duckfs head (W2)"
        );
        assert!(
            v["workspace"].get("mount_path").is_none(),
            "the composed v3 workspace carries NO mount_path (D7)"
        );
    }

    #[test]
    fn portable_inputs_gate_pin_and_skill_resolution() {
        let head = "aa".repeat(32);
        let mut agent = record("bot", &[ACTION_CHAT_POST]);
        agent.skills = vec![
            agent::SkillRef {
                name: "pinned".into(),
                source_prefix: "/shared/skills/pinned".into(),
                source_snapshot: Some("bb".repeat(32)),
            },
            agent::SkillRef {
                name: "tracking".into(),
                source_prefix: "/shared/skills/tracking".into(),
                source_snapshot: None,
            },
        ];

        // no files module: None (the composer takes its v2 path).
        let unwired = module();
        let ctx0 = CaptureCtx::new().with_files_head(&head);
        assert!(
            block_on(unwired.portable_inputs(&ctx0, &agent)).unwrap().is_none(),
            "no portable inputs without a wired files module"
        );

        let m = module().with_files_module("files");

        // files wired + a committed head: Some, head pinned, skills resolved.
        let ctx4 = CaptureCtx::new().with_files_head(&head);
        let inputs = block_on(m.portable_inputs(&ctx4, &agent)).unwrap().unwrap();
        assert_eq!(inputs.source_snapshot.as_deref(), Some(head.as_str()));
        // pinned skill passes its snapshot through; tracking resolves to the head.
        assert_eq!(inputs.skills[0].source_snapshot.as_deref(), Some("bb".repeat(32).as_str()));
        assert_eq!(
            inputs.skills[1].source_snapshot.as_deref(),
            Some(head.as_str()),
            "a tracking skill pins the same committed head (W2)"
        );

        // files wired + an unresolved head: Some with a null pin (fresh network).
        let ctx_empty = CaptureCtx::new();
        let inputs = block_on(m.portable_inputs(&ctx_empty, &agent)).unwrap().unwrap();
        assert!(
            inputs.source_snapshot.is_none(),
            "an unresolved head is a legitimate null pin, still Some"
        );
    }

    // ---- runner-result decode (facet-free + faceted) ----------------------------

    #[test]
    fn legacy_raw_text_results_decode_as_message_only() {
        // a raw-text result (or the AgentResponse JSON the model emits) carries
        // no runner marker, so it decodes to a facet-free message-only result:
        // response_text = the lossy-decoded bytes, no effects, Chain sink, Ok.
        for raw in [
            "just a prose answer",
            "",
            r#"{"reply_blocks":[{"id":"x","kind":"paragraph","text":"hi"}],"actions":[]}"#,
            // a JSON object WITHOUT the marker is not a runner wrapper.
            r#"{"response_text":"nope"}"#,
        ] {
            let result = decode_run_result_v1(raw.as_bytes()).unwrap();
            assert_eq!(result.response_text, raw);
            assert!(result.effects.is_empty());
            assert!(matches!(result.sink, WireSink::Chain));
            assert_eq!(result.status, WireStatus::Ok);
        }
        // invalid utf-8 still degrades lossily rather than erroring.
        assert_eq!(
            decode_run_result_v1(&[0xff, 0xfe]).unwrap().response_text,
            "\u{fffd}\u{fffd}"
        );
    }

    #[test]
    fn a_well_formed_runner_result_yields_its_response_text() {
        let wrapper = serde_json::json!({
            "ducktape_runner_result": 1,
            "response_text": "the deliverable prose",
            "workspace_receipt": {
                "source_prefix": "/shared/agent-workspaces/bot",
                "source_snapshot": null,
                "output_snapshot": null,
                "commit_height": null,
                "rebased": false,
                "no_changes": true
            }
        })
        .to_string();
        assert_eq!(
            decode_run_result_v1(wrapper.as_bytes()).unwrap().response_text,
            "the deliverable prose"
        );
    }

    #[test]
    fn a_broken_runner_wrapper_is_a_loud_error_not_raw_delivery() {
        // claims the marker but the version is unknown → fail the run.
        let bad_version = serde_json::json!({
            "ducktape_runner_result": 99,
            "response_text": "x",
            "workspace_receipt": {
                "source_prefix": "p", "source_snapshot": null, "output_snapshot": null,
                "commit_height": null, "rebased": false, "no_changes": false
            }
        })
        .to_string();
        let err = decode_run_result_v1(bad_version.as_bytes()).unwrap_err();
        assert!(err.contains("version 99"), "got {err:?}");

        // claims the marker but the shape is malformed → fail, never deliver
        // the raw JSON as if it were the model's prose.
        let malformed = r#"{"ducktape_runner_result":1,"response_text":42}"#;
        let err = decode_run_result_v1(malformed.as_bytes()).unwrap_err();
        assert!(err.contains("malformed"), "got {err:?}");
    }

    // ---- faceted delivery -------------------------------------------------------

    /// build a faceted RunnerResult wrapper: the three core fields plus whatever
    /// facet keys `facets` carries (data / effects / sink / status, and a
    /// `workspace_receipt` override when present).
    fn runner_wrapper(response_text: &str, facets: serde_json::Value) -> Vec<u8> {
        let mut obj = serde_json::json!({
            "ducktape_runner_result": 1,
            "response_text": response_text,
            "workspace_receipt": {
                "source_prefix": "/shared/agent-workspaces/bot",
                "source_snapshot": null,
                "output_snapshot": null,
                "commit_height": null,
                "rebased": false,
                "no_changes": true
            }
        });
        if let serde_json::Value::Object(extra) = facets {
            let base = obj.as_object_mut().expect("object");
            for (k, v) in extra {
                base.insert(k, v);
            }
        }
        serde_json::to_vec(&obj).expect("wrapper serializes")
    }

    /// a module wired with the forge sink, one watch on "general", one engaged
    /// run for agent "bot" at seq 2.
    fn awaiting_run_with_forge(registry: &Registry) -> (RunsModule, String) {
        let mut m = module().with_sink_forge("forge");
        let mut ctx = CaptureCtx::new().with_origin(user(9)).with_registry(registry);
        exec(
            &mut m,
            &mut ctx,
            &admin(&RunsMsg::WatchChannel {
                channel_id: "general".into(),
                policy: TurnPolicy::All,
            }),
        )
        .unwrap();
        commit(&mut m);
        engage_post(&mut m, registry, 2, &[]);
        commit(&mut m);
        (m, run_id_for("general", 2, "bot"))
    }

    #[test]
    fn a_plain_result_delivers_its_prose_and_parsed_actions() {
        // a bare response_text (no runner marker, no facets) flows through the
        // single delivery path: the message is delivered and the prose-parsed
        // action is applied — exactly as today's message-only delivery did.
        let response_text = String::from_utf8(response(
            &["on it"],
            vec![AgentAction::CreateTask {
                task_id: "from_prose".into(),
                title: "prose".into(),
            }],
        ))
        .unwrap();
        let (mut m, registry, run_id) = awaiting_run(&[ACTION_CHAT_POST, ACTION_TASKS_CREATE]);
        let mut ctx = CaptureCtx::new()
            .at(8)
            .with_dispatch_origin()
            .with_registry(&registry)
            .with_transcript("general", transcript(2));
        exec(
            &mut m,
            &mut ctx,
            &result_event(&run_id, Ok(response_text.into_bytes())),
        )
        .unwrap();
        assert_eq!(ctx.chat_msgs().len(), 1, "the run delivers its message");
        assert_eq!(
            ctx.task_msgs(),
            vec![TaskMsg::CreateTask {
                task_id: "from_prose".into(),
                title: "prose".into(),
            }],
            "the prose-parsed action is applied"
        );
        assert!(
            ctx.msgs.iter().all(|msg| msg.target != "forge"),
            "a message-only result opens no sink"
        );
        commit(&mut m);
        assert_eq!(get_pending(&m, &run_id), None);
    }

    #[test]
    fn effects_facet_applies_cap_checked() {
        // response_text is plain prose with NO action; the task write comes from
        // the host-assembled effects facet (R1).
        let (mut m, registry, run_id) = awaiting_run(&[ACTION_CHAT_POST, ACTION_TASKS_CREATE]);
        let facets = serde_json::json!({
            "effects": [{"kind":"tasks.create","task_id":"t1","title":"from effect"}]
        });
        let mut ctx = CaptureCtx::new()
            .at(8)            .with_dispatch_origin()
            .with_registry(&registry)
            .with_transcript("general", transcript(2));
        exec(
            &mut m,
            &mut ctx,
            &result_event(&run_id, Ok(runner_wrapper("done", facets))),
        )
        .unwrap();
        assert_eq!(
            ctx.task_msgs(),
            vec![TaskMsg::CreateTask {
                task_id: "t1".into(),
                title: "from effect".into(),
            }]
        );
        commit(&mut m);
        assert_eq!(get_pending(&m, &run_id), None);
    }

    #[test]
    fn unknown_effect_kind_fails_the_run() {
        let (mut m, registry, run_id) = awaiting_run(&[ACTION_CHAT_POST, ACTION_TASKS_CREATE]);
        let facets = serde_json::json!({
            "effects": [{"kind":"forge.delete_universe","task_id":"t1"}]
        });
        let mut ctx = CaptureCtx::new()
            .at(8)            .with_dispatch_origin()
            .with_registry(&registry)
            .with_transcript("general", transcript(2));
        exec(
            &mut m,
            &mut ctx,
            &result_event(&run_id, Ok(runner_wrapper("done", facets))),
        )
        .unwrap();
        assert!(ctx.task_msgs().is_empty(), "no task write escapes a failed run");
        assert!(
            ctx.events
                .iter()
                .any(|e| String::from_utf8_lossy(&e.payload).contains("unknown effect kind")),
            "the failure names the unknown effect kind"
        );
        commit(&mut m);
        assert_eq!(get_pending(&m, &run_id), None);
    }

    #[test]
    fn empty_effects_falls_back_to_response_parsed_actions() {
        // critic #4 fallback: with an EMPTY effects facet, a model that emitted
        // the action only in prose still gets it applied — never a silent drop.
        let (mut m, registry, run_id) = awaiting_run(&[ACTION_CHAT_POST, ACTION_TASKS_CREATE]);
        let response_text = String::from_utf8(response(
            &["on it"],
            vec![AgentAction::CreateTask {
                task_id: "t1".into(),
                title: "from prose".into(),
            }],
        ))
        .unwrap();
        let mut ctx = CaptureCtx::new()
            .at(8)            .with_dispatch_origin()
            .with_registry(&registry)
            .with_transcript("general", transcript(2));
        exec(
            &mut m,
            &mut ctx,
            &result_event(&run_id, Ok(runner_wrapper(&response_text, serde_json::json!({})))),
        )
        .unwrap();
        assert_eq!(
            ctx.task_msgs(),
            vec![TaskMsg::CreateTask {
                task_id: "t1".into(),
                title: "from prose".into(),
            }]
        );
    }

    #[test]
    fn pr_sink_emits_open_pr_only_with_the_forge_push_cap() {
        let sink = serde_json::json!({
            "sink": {"mode":"pr","repo":"app","source_branch":"agent/x","target_branch":"main","title":"My PR","body":"details"}
        });

        // (1) GRANTED forge_push (D3 cap) + branch born → OpenPr emitted.
        let mut granted = registry(&[("bot", &[ACTION_CHAT_POST])]);
        granted.get_mut("bot").unwrap().caps.forge_push = vec!["app".into()];
        let (mut m, run_id) = awaiting_run_with_forge(&granted);
        let mut ctx = CaptureCtx::new()
            .at(8)            .with_dispatch_origin()
            .with_registry(&granted)
            .with_transcript("general", transcript(2))
            .with_forge_ref("app", "agent/x");
        exec(
            &mut m,
            &mut ctx,
            &result_event(&run_id, Ok(runner_wrapper("done", sink.clone()))),
        )
        .unwrap();
        let forge_ops: Vec<_> = ctx.msgs.iter().filter(|m| m.target == "forge").collect();
        assert_eq!(forge_ops.len(), 1, "one OpenPr emitted");
        assert_eq!(
            forge::decode_msg(&forge_ops[0].payload).unwrap(),
            forge::ForgeMsg::OpenPr {
                repo: "app".into(),
                title: "My PR".into(),
                body: "details".into(),
                source_branch: "agent/x".into(),
                target_branch: "main".into(),
            }
        );

        // (2) NO forge_push cap → degrade to a breadcrumb, no forge op, no abort.
        let ungranted = registry(&[("bot", &[ACTION_CHAT_POST])]);
        let (mut m2, run_id2) = awaiting_run_with_forge(&ungranted);
        let mut ctx2 = CaptureCtx::new()
            .at(8)            .with_dispatch_origin()
            .with_registry(&ungranted)
            .with_transcript("general", transcript(2))
            .with_forge_ref("app", "agent/x");
        exec(
            &mut m2,
            &mut ctx2,
            &result_event(&run_id2, Ok(runner_wrapper("done", sink))),
        )
        .unwrap();
        assert!(
            ctx2.msgs.iter().all(|m| m.target != "forge"),
            "no cap → no forge op"
        );
        assert!(
            ctx2.events
                .iter()
                .any(|e| String::from_utf8_lossy(&e.payload).contains("lacks forge_push")),
            "the breadcrumb names the missing cap"
        );
        assert_eq!(ctx2.chat_msgs().len(), 1, "the run still delivers its message");
    }

    #[test]
    fn pr_sink_with_an_unborn_branch_degrades_without_aborting() {
        let mut granted = registry(&[("bot", &[ACTION_CHAT_POST])]);
        granted.get_mut("bot").unwrap().caps.forge_push = vec!["app".into()];
        let (mut m, run_id) = awaiting_run_with_forge(&granted);
        // no with_forge_ref → the source branch is NOT born in committed forge.
        let mut ctx = CaptureCtx::new()
            .at(8)            .with_dispatch_origin()
            .with_registry(&granted)
            .with_transcript("general", transcript(2));
        exec(
            &mut m,
            &mut ctx,
            &result_event(
                &run_id,
                Ok(runner_wrapper(
                    "done",
                    serde_json::json!({"sink":{"mode":"pr","repo":"app","source_branch":"agent/x","title":"PR"}}),
                )),
            ),
        )
        .unwrap();
        assert!(
            ctx.msgs.iter().all(|m| m.target != "forge"),
            "an unborn source branch must never emit an OpenPr (no-fail rule)"
        );
        assert!(
            ctx.events
                .iter()
                .any(|e| String::from_utf8_lossy(&e.payload).contains("source branch not present"))
        );
    }

    #[test]
    fn malformed_facet_fails_the_run_without_aborting() {
        // effects is not an array → decode_run_result_v1 fails → the run fails
        // deterministically (R4), never a delivery-block abort.
        let (mut m, registry, run_id) = awaiting_run(&[ACTION_CHAT_POST]);
        let bad = serde_json::json!({
            "ducktape_runner_result": 1,
            "response_text": "hi",
            "workspace_receipt": {"source_prefix":"p","source_snapshot":null,"output_snapshot":null,"commit_height":null,"rebased":false,"no_changes":false},
            "effects": "not-an-array"
        });
        let mut ctx = CaptureCtx::new()
            .at(8)            .with_dispatch_origin()
            .with_registry(&registry)
            .with_transcript("general", transcript(2));
        // exec returns Ok — the block commits — but the run FAILED.
        exec(
            &mut m,
            &mut ctx,
            &result_event(&run_id, Ok(serde_json::to_vec(&bad).unwrap())),
        )
        .unwrap();
        assert!(
            ctx.events
                .iter()
                .any(|e| String::from_utf8_lossy(&e.payload).contains("malformed")),
            "a malformed facet fails the run loudly"
        );
        assert_eq!(ctx.chat_msgs().len(), 1, "the failure surfaces as a threaded reply");
        commit(&mut m);
        assert_eq!(get_pending(&m, &run_id), None);
    }

    #[test]
    fn status_failed_overrides_a_present_message() {
        let (mut m, registry, run_id) = awaiting_run(&[ACTION_CHAT_POST]);
        let mut ctx = CaptureCtx::new()
            .at(8)            .with_dispatch_origin()
            .with_registry(&registry)
            .with_transcript("general", transcript(2));
        exec(
            &mut m,
            &mut ctx,
            &result_event(
                &run_id,
                Ok(runner_wrapper("a perfectly good message", serde_json::json!({"status":"failed"}))),
            ),
        )
        .unwrap();
        assert!(
            ctx.events
                .iter()
                .any(|e| String::from_utf8_lossy(&e.payload).contains("failed status")),
            "a failed status fails the run despite the present message"
        );
    }

    #[test]
    fn job_finalize_is_a_delivery_receipt_with_data_and_output_ref() {
        let registry = job_registry(); // agent "duck" with tasks.create
        let mut m = module();
        let mut ctx = CaptureCtx::new().at(3).with_jobs_origin().with_registry(&registry);
        exec(&mut m, &mut ctx, &jobs_event("job-1", "agent/duck", "spec")).unwrap();
        commit(&mut m);
        let run_id = job_run_id_for("job-1", "duck", 3);

        let facets = serde_json::json!({
            "workspace_receipt": {"source_prefix":"/ws/duck","source_snapshot":null,"output_snapshot":"deadbeef","commit_height":7,"rebased":false,"no_changes":false},
            "data": "{\"summary\":\"ok\"}",
            "effects": [{"kind":"tasks.create","task_id":"t1","title":"todo"}],
            "status": "ok"
        });
        let mut ctx = CaptureCtx::new()
            .at(10)            .with_dispatch_origin()
            .with_registry(&registry)
            .with_claimed_job("job-1", 3);
        exec(
            &mut m,
            &mut ctx,
            &result_event(&run_id, Ok(runner_wrapper("done", facets))),
        )
        .unwrap();

        // the effects facet applied a task write.
        assert_eq!(
            ctx.task_msgs(),
            vec![TaskMsg::CreateTask {
                task_id: "t1".into(),
                title: "todo".into(),
            }]
        );
        // the finalize payload is a faceted DeliveryReceipt (not a bare response).
        let finalize = ctx.job_msgs();
        assert_eq!(finalize.len(), 1);
        let JobsMsg::Finalize { ok, payload, .. } = &finalize[0] else {
            panic!("expected a finalize");
        };
        assert!(*ok);
        let v: serde_json::Value = serde_json::from_str(payload).unwrap();
        assert_eq!(v["ducktape_delivery"], 1);
        assert_eq!(v["status"], "ok");
        assert_eq!(v["data"], "{\"summary\":\"ok\"}");
        assert_eq!(v["output_ref"]["output_snapshot"], "deadbeef");
        assert_eq!(v["output_ref"]["commit_height"], 7);
        assert_eq!(v["output_ref"]["source_prefix"], "/ws/duck");
    }

    #[test]
    fn wire_sink_defaults_to_chain_and_decodes_a_present_pr() {
        // a MISSING sink field → Chain (internal-tag + serde default interplay).
        let no_sink = runner_wrapper("hi", serde_json::json!({}));
        assert!(matches!(
            decode_run_result_v1(&no_sink).unwrap().sink,
            WireSink::Chain
        ));
        // a present {"mode":"pr",...} → Pr.
        let pr = runner_wrapper(
            "hi",
            serde_json::json!({"sink":{"mode":"pr","repo":"a","source_branch":"s","title":"t"}}),
        );
        assert!(matches!(
            decode_run_result_v1(&pr).unwrap().sink,
            WireSink::Pr { .. }
        ));
        // an unsupported wrapper version fails to decode (R4).
        let badv = serde_json::json!({
            "ducktape_runner_result": 99,
            "response_text": "x",
            "workspace_receipt": {"source_prefix":"p","source_snapshot":null,"output_snapshot":null,"commit_height":null,"rebased":false,"no_changes":false}
        });
        assert!(decode_run_result_v1(&serde_json::to_vec(&badv).unwrap()).is_err());
    }

    #[test]
    fn forge_sink_mirror_matches_forge_decode_msg() {
        // pin the local ForgeSinkMsg mirror against the real forge decoder so the
        // wire cannot silently drift (the reason forge is a dev-dependency).
        let bytes = forge_open_pr_bytes("app", "T", "B", "agent/x", "main");
        assert_eq!(
            forge::decode_msg(&bytes).unwrap(),
            forge::ForgeMsg::OpenPr {
                repo: "app".into(),
                title: "T".into(),
                body: "B".into(),
                source_branch: "agent/x".into(),
                target_branch: "main".into(),
            }
        );
    }

    // ---- the registry hook ------------------------------------------------------

    #[test]
    fn a_registered_agent_event_registers_the_dispatch_recipe() {
        let mut m = module();
        let mut ctx = CaptureCtx::new().with_agent_origin();
        exec(
            &mut m,
            &mut ctx,
            &agent_event(&AgentEvent::Registered {
                agent_id: "bot".into(),
                capability: "model-1".into(),
            }),
        )
        .unwrap();

        let recipes = ctx.dispatch_msgs();
        assert_eq!(recipes.len(), 1);
        let DispatchMsg::RegisterRecipe {
            recipe_id,
            capability,
            routing,
            output_contract,
            max_attempts,
            deadline_views,
            ..
        } = &recipes[0]
        else {
            panic!("expected a recipe registration");
        };
        assert_eq!(*recipe_id, recipe_id_for("bot"));
        assert_eq!(*capability, "model-1");
        assert_eq!(*routing, Routing::Rendezvous);
        assert_eq!(
            *output_contract,
            OutputContract::Text,
            "raw model text back; THIS module normalizes"
        );
        assert_eq!(*max_attempts, RUN_MAX_ATTEMPTS);
        assert_eq!(*deadline_views, Some(RUN_DEADLINE_VIEWS));
    }

    #[test]
    fn a_capability_change_event_retunes_the_dispatch_recipe() {
        let mut m = module();
        let mut ctx = CaptureCtx::new().with_agent_origin();
        exec(
            &mut m,
            &mut ctx,
            &agent_event(&AgentEvent::CapabilityChanged {
                agent_id: "bot".into(),
                capability: "model-2".into(),
            }),
        )
        .unwrap();
        assert_eq!(
            ctx.dispatch_msgs(),
            vec![DispatchMsg::UpdateRecipe {
                recipe_id: recipe_id_for("bot"),
                description: None,
                capability: Some("model-2".into()),
                routing: None,
                output_contract: None,
                max_attempts: None,
            }]
        );
    }

    #[test]
    fn the_registry_hook_may_error_to_abort_the_registration_block() {
        let mut m = module();

        // an agent id whose recipe id would blow the dispatch id cap: the
        // hook ERRORS, aborting the registration block — the atomic recipe
        // seam (the registry record must never land without its recipe).
        let oversized = "x".repeat(dispatch::MAX_ID_BYTES);
        let mut ctx = CaptureCtx::new().with_agent_origin();
        let err = exec(
            &mut m,
            &mut ctx,
            &agent_event(&AgentEvent::Registered {
                agent_id: oversized,
                capability: "model-1".into(),
            }),
        )
        .unwrap_err();
        assert!(matches!(err, Error::Module(reason) if reason.contains("recipe id")));

        // malformed bytes from the registry origin error the same way — the
        // registry is genesis-trusted code, so this is a bug, not traffic.
        let mut ctx = CaptureCtx::new().with_agent_origin();
        let err = exec(
            &mut m,
            &mut ctx,
            &Msg {
                target: "runs".into(),
                payload: b"not an agent event".to_vec(),
            },
        )
        .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
    }

    // ---- watches -----------------------------------------------------------------

    #[test]
    fn watch_and_unwatch_stage_the_policy_and_emit_the_plane_subscription_atomically() {
        let registry = registry(&[]);
        let mut m = module();
        let mut ctx = CaptureCtx::new()
            .with_origin(user(9))
            .with_registry(&registry);
        exec(
            &mut m,
            &mut ctx,
            &admin(&RunsMsg::WatchChannel {
                channel_id: "general".into(),
                policy: TurnPolicy::Mention,
            }),
        )
        .unwrap();
        // the watch and the plane Subscribe follow-up are one atomic unit (P2).
        assert_eq!(
            ctx.tagging_msgs(),
            vec![TaggingMsg::Subscribe {
                source: "chat".into(),
                container: "general".into(),
            }]
        );
        commit(&mut m);

        // an Assigned policy must name a registered agent.
        let mut ctx = CaptureCtx::new()
            .with_origin(user(9))
            .with_registry(&registry);
        let err = exec(
            &mut m,
            &mut ctx,
            &admin(&RunsMsg::WatchChannel {
                channel_id: "other".into(),
                policy: TurnPolicy::Assigned("ghost".into()),
            }),
        )
        .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        abort(&mut m);

        // unwatch removes the watch and drops the plane subscription.
        let mut ctx = CaptureCtx::new().with_origin(user(9));
        exec(
            &mut m,
            &mut ctx,
            &admin(&RunsMsg::UnwatchChannel {
                channel_id: "general".into(),
            }),
        )
        .unwrap();
        assert_eq!(
            ctx.tagging_msgs(),
            vec![TaggingMsg::Unsubscribe {
                source: "chat".into(),
                container: "general".into(),
            }]
        );
        commit(&mut m);

        // unwatching an unwatched channel stages and emits NOTHING.
        let before = m.root();
        let mut ctx = CaptureCtx::new().with_origin(user(9));
        exec(
            &mut m,
            &mut ctx,
            &admin(&RunsMsg::UnwatchChannel {
                channel_id: "general".into(),
            }),
        )
        .unwrap();
        assert!(ctx.msgs.is_empty(), "an idempotent unwatch emits nothing");
        commit(&mut m);
        assert_eq!(m.root(), before);
    }

    #[test]
    fn enable_job_worker_is_admin_gated_and_emits_self_registration() {
        let mut m = module();

        let mut intruder = CaptureCtx::new().with_origin(Origin::System);
        let err = exec(
            &mut m,
            &mut intruder,
            &admin(&RunsMsg::EnableJobWorker { enabled: true }),
        )
        .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        abort(&mut m);

        let mut ctx = CaptureCtx::new().with_origin(user(9));
        exec(
            &mut m,
            &mut ctx,
            &admin(&RunsMsg::EnableJobWorker { enabled: true }),
        )
        .unwrap();
        assert_eq!(ctx.job_msgs(), vec![JobsMsg::RegisterWorker {}]);
        commit(&mut m);

        let mut ctx = CaptureCtx::new().with_origin(user(9));
        exec(
            &mut m,
            &mut ctx,
            &admin(&RunsMsg::EnableJobWorker { enabled: false }),
        )
        .unwrap();
        assert_eq!(ctx.job_msgs(), vec![JobsMsg::UnregisterWorker {}]);
        commit(&mut m);

        let mut without_jobs = RunsModule::new(
            "runs",
            "chat",
            "saga",
            "tagging",
            "dispatch",
            "agent",
            Some("tasks".into()),
            None,
        );
        let mut ctx = CaptureCtx::new().with_origin(user(9));
        let err = exec(
            &mut without_jobs,
            &mut ctx,
            &admin(&RunsMsg::EnableJobWorker { enabled: true }),
        )
        .unwrap_err();
        assert!(matches!(err, Error::Module(m) if m.contains("jobs module")));
    }

    // ---- the engagement intake: turn policies ----------------------------------

    #[test]
    fn mention_policy_engages_only_this_modules_tagged_active_agents() {
        let registry = registry(&[("bot1", &[ACTION_CHAT_POST]), ("bot2", &[ACTION_CHAT_POST])]);
        let mut m = watched(TurnPolicy::Mention, &registry);

        // the post tags bot1, an entity of a FOREIGN module, and an
        // unregistered agent — only bot1 engages.
        let mut ctx = CaptureCtx::new()
            .at(3)
            .with_tagging_origin()
            .with_registry(&registry)
            .with_transcript("general", transcript(3));
        exec(
            &mut m,
            &mut ctx,
            &engagement(
                "general",
                3,
                vec![
                    agent_tag("bot1"),
                    EntityRef {
                        module: "other-module".into(),
                        entity: "bot2".into(),
                    },
                    agent_tag("ghost"),
                ],
            ),
        )
        .unwrap();
        commit(&mut m);

        let run_id = run_id_for("general", 3, "bot1");
        let entry = get_pending(&m, &run_id).expect("bot1 engaged");
        assert_eq!(entry.dispatch_id, dispatch_id_for(&run_id));
        assert_eq!(entry.requester, SagaOrigin::Module("tagging".into()));
        assert_eq!(get_pending(&m, &run_id_for("general", 3, "bot2")), None);

        // exactly one dispatch, under the agent's own recipe, carrying the
        // fully composed envelope — prompt pin, thread key, contract,
        // transcript.
        let dispatches = ctx.dispatch_msgs();
        assert_eq!(dispatches.len(), 1);
        let DispatchMsg::Dispatch {
            dispatch_id,
            recipe_id,
            payload,
        } = &dispatches[0]
        else {
            panic!("expected a dispatch");
        };
        assert_eq!(*dispatch_id, dispatch_id_for(&run_id));
        assert_eq!(*recipe_id, recipe_id_for("bot1"));
        let envelope: serde_json::Value =
            serde_json::from_slice(payload).expect("the payload is a JSON envelope");
        assert_eq!(envelope["ducktape_run"], RUN_ENVELOPE_VERSION);
        assert_eq!(envelope["agent_id"], "bot1");
        assert_eq!(
            envelope["prompt_hash"],
            "07".repeat(PROMPT_HASH_LEN),
            "the registry's prompt pin rides the envelope"
        );
        assert_eq!(
            envelope["thread_key"], "general#3",
            "a non-thread anchor keys the thread by itself"
        );
        assert!(
            envelope["contract"]
                .as_str()
                .unwrap()
                .contains("Return ONLY a JSON object"),
            "the strict output contract rides the payload"
        );
        assert!(
            envelope["conversation"].as_str().unwrap().contains("msg 3"),
            "the pinned transcript rides the payload verbatim"
        );
        assert!(
            envelope["instructions"]
                .as_str()
                .unwrap()
                .starts_with("You are a Ducktape agent."),
            "the generic fallback instructions ride the envelope"
        );
    }

    #[test]
    fn the_envelope_tracks_the_registrys_live_prompt_pin() {
        // runs never mirrors the pin: composition queries the registry at
        // dispatch time (staged same-block registrations included), so an
        // UpdateAgent prompt rotation is picked up by the very next run
        // without any hook payload carrying it, and a capability retune
        // never disturbs it.
        let mut registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
        let mut m = watched(TurnPolicy::All, &registry);

        // the registration hook fires as it would in the record's block.
        let mut hook_ctx = CaptureCtx::new().with_agent_origin().with_registry(&registry);
        exec(
            &mut m,
            &mut hook_ctx,
            &agent_event(&AgentEvent::Registered {
                agent_id: "bot".into(),
                capability: "model-1".into(),
            }),
        )
        .unwrap();
        commit(&mut m);

        let ctx = engage_post(&mut m, &registry, 2, &[]);
        commit(&mut m);
        let DispatchMsg::Dispatch { payload, .. } = &ctx.dispatch_msgs()[0] else {
            panic!("expected a dispatch");
        };
        let envelope: serde_json::Value = serde_json::from_slice(payload).unwrap();
        assert_eq!(envelope["prompt_hash"], "07".repeat(PROMPT_HASH_LEN));

        // the owner rotates the prompt; the registry hook only ever carries
        // capability retunes — process one to show it is orthogonal.
        registry.get_mut("bot").unwrap().prompt_hash = vec![9u8; PROMPT_HASH_LEN];
        let mut hook_ctx = CaptureCtx::new().with_agent_origin().with_registry(&registry);
        exec(
            &mut m,
            &mut hook_ctx,
            &agent_event(&AgentEvent::CapabilityChanged {
                agent_id: "bot".into(),
                capability: "model-2".into(),
            }),
        )
        .unwrap();
        commit(&mut m);

        let ctx = engage_post(&mut m, &registry, 3, &[]);
        commit(&mut m);
        let DispatchMsg::Dispatch { payload, .. } = &ctx.dispatch_msgs()[0] else {
            panic!("expected a dispatch");
        };
        let envelope: serde_json::Value = serde_json::from_slice(payload).unwrap();
        assert_eq!(
            envelope["prompt_hash"],
            "09".repeat(PROMPT_HASH_LEN),
            "the next run composes from the updated record"
        );
        assert_eq!(envelope["agent_id"], "bot");
    }

    #[test]
    fn an_agent_without_a_prompt_pin_dispatches_a_null_prompt_hash() {
        let mut registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
        registry.get_mut("bot").unwrap().prompt_hash = Vec::new();
        let mut m = watched(TurnPolicy::All, &registry);
        let ctx = engage_post(&mut m, &registry, 2, &[]);
        commit(&mut m);
        let DispatchMsg::Dispatch { payload, .. } = &ctx.dispatch_msgs()[0] else {
            panic!("expected a dispatch");
        };
        let envelope: serde_json::Value = serde_json::from_slice(payload).unwrap();
        assert!(
            envelope["prompt_hash"].is_null(),
            "no pin composes as null — the host falls back to instructions"
        );
        assert!(
            envelope["instructions"]
                .as_str()
                .unwrap()
                .starts_with("You are a Ducktape agent.")
        );
    }

    #[test]
    fn all_policy_engages_every_active_agent_and_paused_agents_never_engage() {
        let mut registry = registry(&[("a", &[]), ("b", &[]), ("c", &[])]);
        let mut m = watched(TurnPolicy::All, &registry);
        pause(&mut registry, "b");

        let ctx = engage_post(&mut m, &registry, 2, &[]);
        commit(&mut m);
        assert_eq!(
            ctx.dispatch_msgs().len(),
            2,
            "two active agents, two dispatches"
        );
        assert!(get_pending(&m, &run_id_for("general", 2, "a")).is_some());
        assert_eq!(
            get_pending(&m, &run_id_for("general", 2, "b")),
            None,
            "a paused agent never engages"
        );
        assert!(get_pending(&m, &run_id_for("general", 2, "c")).is_some());
    }

    #[test]
    fn round_robin_picks_by_anchor_seq_over_the_sorted_active_agents() {
        let mut registry = registry(&[("a", &[]), ("b", &[]), ("c", &[])]);
        let mut m = watched(TurnPolicy::RoundRobin, &registry);

        // seq 4 over [a, b, c]: 4 % 3 = 1 -> "b".
        engage_post(&mut m, &registry, 4, &[]);
        commit(&mut m);
        assert!(get_pending(&m, &run_id_for("general", 4, "b")).is_some());
        assert_eq!(get_pending(&m, &run_id_for("general", 4, "a")), None);
        assert_eq!(get_pending(&m, &run_id_for("general", 4, "c")), None);

        // pause "b": the domain shrinks to [a, c]; seq 5 % 2 = 1 -> "c".
        pause(&mut registry, "b");
        engage_post(&mut m, &registry, 5, &[]);
        commit(&mut m);
        assert!(get_pending(&m, &run_id_for("general", 5, "c")).is_some());
        assert_eq!(get_pending(&m, &run_id_for("general", 5, "b")), None);
    }

    #[test]
    fn assigned_policy_engages_exactly_its_agent_and_respects_pause() {
        let mut registry = registry(&[("a", &[]), ("b", &[])]);
        let mut m = watched(TurnPolicy::Assigned("b".into()), &registry);
        engage_post(&mut m, &registry, 2, &[]);
        commit(&mut m);
        assert!(get_pending(&m, &run_id_for("general", 2, "b")).is_some());
        assert_eq!(get_pending(&m, &run_id_for("general", 2, "a")), None);

        // paused assignee: nothing engages, the block still commits.
        pause(&mut registry, "b");
        let ctx = engage_post(&mut m, &registry, 3, &[]);
        commit(&mut m);
        assert!(ctx.dispatch_msgs().is_empty());
        assert_eq!(get_pending(&m, &run_id_for("general", 3, "b")), None);
    }

    #[test]
    fn foreign_sources_and_direct_chat_or_saga_follow_ups_are_dead_letters() {
        // the LOOP RULE itself lives in the tagging plane (only user posts
        // fire) and is tested there; this module's job is to survive the
        // events it should not act on.
        let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
        let mut m = watched(TurnPolicy::All, &registry);
        let before = m.root();

        // an engagement whose source is not chat: dropped with a breadcrumb.
        let mut ctx = CaptureCtx::new().at(2).with_tagging_origin().with_registry(&registry);
        exec(
            &mut m,
            &mut ctx,
            &Msg {
                target: "runs".into(),
                payload: tagging_encode_event(&EngagementEvent {
                    source: "pages".into(),
                    container: "general".into(),
                    content_seq: 2,
                    author: Author::User(vec![1; 32]),
                    tags: vec![],
                }),
            },
        )
        .unwrap();
        assert!(ctx.msgs.is_empty());
        assert!(!ctx.events.is_empty(), "the drop leaves a breadcrumb");

        // a direct chat-origin follow-up (no hook is ever registered now):
        // dead-lettered, never an abort of the posting block.
        let mut ctx = CaptureCtx::new().with_origin(Origin::Module("chat".into()));
        exec(
            &mut m,
            &mut ctx,
            &Msg {
                target: "runs".into(),
                payload: b"anything at all".to_vec(),
            },
        )
        .unwrap();
        assert!(ctx.msgs.is_empty());

        // a saga-origin callback (a foreign trigger's reply_to pointed here):
        // dead-lettered — an Err would abort the saga's terminal block.
        let mut ctx = CaptureCtx::new().with_origin(Origin::Module("saga".into()));
        exec(
            &mut m,
            &mut ctx,
            &Msg {
                target: "runs".into(),
                payload: b"a saga callback of any shape".to_vec(),
            },
        )
        .unwrap();
        assert!(ctx.msgs.is_empty());
        assert!(!ctx.events.is_empty(), "the drop leaves a breadcrumb");
        commit(&mut m);
        assert_eq!(m.root(), before, "nothing was staged");
    }

    #[test]
    fn unwatched_channels_and_failed_pins_are_staged_no_ops_on_the_engagement_arm() {
        let registry = registry(&[("bot", &[])]);
        let mut m = watched(TurnPolicy::All, &registry);
        let before = m.root();

        // an engagement for a channel we do not watch (subscription drift
        // within a block): no-op, never an error.
        let mut ctx = CaptureCtx::new()
            .at(2)
            .with_tagging_origin()
            .with_registry(&registry)
            .with_transcript("random", transcript(2));
        exec(&mut m, &mut ctx, &engagement("random", 2, vec![])).unwrap();
        assert!(ctx.msgs.is_empty());

        // a failing context pin (the ctx serves NO transcript at all — the
        // chat query errors) must not poison the posting block: Ok, no run.
        let mut ctx = CaptureCtx::new().at(2).with_tagging_origin().with_registry(&registry);
        exec(&mut m, &mut ctx, &engagement("general", 2, vec![])).unwrap();
        assert!(ctx.dispatch_msgs().is_empty(), "no dispatch on a failed pin");
        assert!(!ctx.events.is_empty(), "the skip leaves a breadcrumb event");
        commit(&mut m);
        assert_eq!(m.root(), before, "nothing was staged");
    }

    // ---- the turn claim ----------------------------------------------------------

    #[test]
    fn duplicate_turn_claims_are_deterministic_no_ops() {
        let registry = registry(&[("bot", &[])]);
        let mut m = watched(TurnPolicy::All, &registry);

        // the engagement claims the turn in the posting block...
        let ctx = engage_post(&mut m, &registry, 2, &[]);
        assert_eq!(ctx.dispatch_msgs().len(), 1);
        let run_id = run_id_for("general", 2, "bot");
        let created = get_pending(&m, &run_id).unwrap();

        // ...an explicit RequestRun for the SAME turn in the same block is a
        // staged no-op (first in consensus order won)...
        let mut ctx = CaptureCtx::new()
            .at(2)
            .with_origin(user(5))
            .with_registry(&registry)
            .with_transcript("general", transcript(2));
        exec(
            &mut m,
            &mut ctx,
            &admin(&RunsMsg::RequestRun {
                agent_id: "bot".into(),
                channel_id: "general".into(),
                anchor_seq: 2,
            }),
        )
        .unwrap();
        assert!(ctx.msgs.is_empty(), "the losing claim re-fires nothing");
        commit(&mut m);
        assert_eq!(
            get_pending(&m, &run_id).unwrap(),
            created,
            "the first claim's entry survives untouched"
        );

        // ...and a COMMITTED duplicate (the same engagement replayed later)
        // is equally a no-op.
        let root = m.root();
        let ctx = engage_post(&mut m, &registry, 2, &[]);
        assert!(ctx.msgs.is_empty());
        commit(&mut m);
        assert_eq!(m.root(), root, "a duplicate claim moves nothing");
    }

    #[test]
    fn a_delivered_turn_stays_claimed_via_the_dispatch_record() {
        // after delivery the pending entry is pruned — the dispatch module's
        // permanent record is what keeps the turn claimed. re-staging an
        // entry here would orphan it forever (the dispatch module no-ops the
        // duplicate dispatch and no ResultEvent would ever prune it).
        let registry = registry(&[("bot", &[])]);
        let mut m = watched(TurnPolicy::All, &registry);
        let run_id = run_id_for("general", 2, "bot");
        let taken = dispatch_id_for(&run_id);

        let mut ctx = CaptureCtx::new()
            .at(9)
            .with_tagging_origin()
            .with_registry(&registry)
            .with_transcript("general", transcript(2))
            .with_taken_dispatch(&taken);
        exec(&mut m, &mut ctx, &engagement("general", 2, vec![])).unwrap();
        assert!(ctx.msgs.is_empty(), "a taken turn re-fires nothing");

        let mut ctx = CaptureCtx::new()
            .at(9)
            .with_origin(user(5))
            .with_registry(&registry)
            .with_transcript("general", transcript(2))
            .with_taken_dispatch(&taken);
        exec(
            &mut m,
            &mut ctx,
            &admin(&RunsMsg::RequestRun {
                agent_id: "bot".into(),
                channel_id: "general".into(),
                anchor_seq: 2,
            }),
        )
        .unwrap();
        assert!(ctx.msgs.is_empty());
        commit(&mut m);
        assert_eq!(get_pending(&m, &run_id), None, "nothing was re-staged");
    }

    #[test]
    fn chat_and_job_run_keys_are_structurally_disjoint_and_reject_separator_inputs() {
        assert_ne!(
            run_id_for("job", 7, "duck"),
            job_run_id_for("7", "duck", 3),
            "a channel literally named job must not collide with job runs"
        );

        let registry = registry(&[("bot", &[])]);
        let mut m = watched(TurnPolicy::All, &registry);
        let root = m.root();

        let mut ctx = CaptureCtx::new().with_origin(user(9)).with_registry(&registry);
        let err = exec(
            &mut m,
            &mut ctx,
            &admin(&RunsMsg::WatchChannel {
                channel_id: "bad\u{1f}channel".into(),
                policy: TurnPolicy::All,
            }),
        )
        .unwrap_err();
        assert!(matches!(err, Error::Module(message) if message.contains("unit separator")));
        abort(&mut m);

        let mut ctx = CaptureCtx::new()
            .with_origin(user(1))
            .with_registry(&registry)
            .with_transcript("bad\u{1f}channel", transcript(1));
        let err = exec(
            &mut m,
            &mut ctx,
            &admin(&RunsMsg::RequestRun {
                agent_id: "bot".into(),
                channel_id: "bad\u{1f}channel".into(),
                anchor_seq: 1,
            }),
        )
        .unwrap_err();
        assert!(matches!(err, Error::Module(message) if message.contains("unit separator")));
        abort(&mut m);

        let mut ctx = CaptureCtx::new().with_jobs_origin().with_registry(&registry);
        exec(
            &mut m,
            &mut ctx,
            &jobs_event("bad\u{1f}job", "agent/bot", "spec"),
        )
        .expect("separator in a no-fail jobs event is a no-op");
        assert!(ctx.msgs.is_empty(), "no claim emitted for a bad job id");

        // a spec that does not hash to spec_hash is dropped the same way.
        let mut ctx = CaptureCtx::new().with_jobs_origin().with_registry(&registry);
        exec(
            &mut m,
            &mut ctx,
            &Msg {
                target: "runs".into(),
                payload: jobs_encode_event(&JobsEvent::Submitted {
                    job_id: "job-x".into(),
                    kind: "agent/bot".into(),
                    submitter: "ext:01".into(),
                    spec: "actual".into(),
                    spec_hash: vec![9u8; 32],
                }),
            },
        )
        .expect("a mismatched spec hash is a no-op");
        assert!(ctx.msgs.is_empty());
        commit(&mut m);
        assert_eq!(m.root(), root, "bad jobs events staged nothing");
    }

    // ---- the no-fail arms ----------------------------------------------------------

    #[test]
    fn malformed_intake_payloads_are_staged_no_ops() {
        let registry = registry(&[("bot", &[])]);
        let mut m = watched(TurnPolicy::All, &registry);
        let before = m.root();

        // garbage from the tagging origin: the posting block must survive.
        let mut ctx = CaptureCtx::new().with_tagging_origin();
        exec(
            &mut m,
            &mut ctx,
            &Msg {
                target: "runs".into(),
                payload: b"not an engagement".to_vec(),
            },
        )
        .unwrap();
        assert!(ctx.msgs.is_empty());
        assert!(!ctx.events.is_empty(), "the drop leaves a breadcrumb");

        // garbage from the dispatch origin: the delivery block must survive.
        let mut ctx = CaptureCtx::new().with_dispatch_origin();
        exec(
            &mut m,
            &mut ctx,
            &Msg {
                target: "runs".into(),
                payload: b"not a result event".to_vec(),
            },
        )
        .unwrap();
        assert!(ctx.msgs.is_empty());

        // garbage from the jobs origin: the submit block must survive.
        let mut ctx = CaptureCtx::new().with_jobs_origin();
        exec(
            &mut m,
            &mut ctx,
            &Msg {
                target: "runs".into(),
                payload: b"not a jobs event".to_vec(),
            },
        )
        .unwrap();
        assert!(ctx.msgs.is_empty());

        // a well-formed result event for an UNKNOWN dispatch: staged no-op.
        let mut ctx = CaptureCtx::new().with_dispatch_origin();
        exec(&mut m, &mut ctx, &result_event("ghost-run", Ok(Vec::new()))).unwrap();

        commit(&mut m);
        assert_eq!(m.root(), before, "none of the drops staged anything");
    }

    #[test]
    fn external_submitters_cannot_fake_the_engagement_or_result_intakes() {
        let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
        let mut m = watched(TurnPolicy::All, &registry);

        // engagement-shaped bytes from an EXTERNAL origin route to the
        // RunsMsg decoder and fail there — no run is ever created.
        let mut ctx = CaptureCtx::new()
            .at(2)
            .with_origin(user(1))
            .with_registry(&registry)
            .with_transcript("general", transcript(2));
        let err = exec(&mut m, &mut ctx, &engagement("general", 2, vec![])).unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        abort(&mut m);
        assert_eq!(get_pending(&m, &run_id_for("general", 2, "bot")), None);

        // result-shaped bytes from an EXTERNAL origin: same story.
        engage_post(&mut m, &registry, 2, &[]);
        commit(&mut m);
        let run_id = run_id_for("general", 2, "bot");
        let mut ctx = CaptureCtx::new().with_origin(user(1));
        let err = exec(&mut m, &mut ctx, &result_event(&run_id, Ok(Vec::new()))).unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        abort(&mut m);
        assert!(
            get_pending(&m, &run_id).is_some(),
            "the forged delivery pruned nothing"
        );
    }

    // ---- the result intake ----------------------------------------------------------

    /// a committed module holding one pending run for "bot" (granted
    /// `actions`) at general/2, plus the registry and the run id.
    fn awaiting_run(actions: &[&str]) -> (RunsModule, Registry, String) {
        let registry = registry(&[("bot", actions)]);
        let mut m = watched(TurnPolicy::All, &registry);
        engage_post(&mut m, &registry, 2, &[]);
        commit(&mut m);
        (m, registry, run_id_for("general", 2, "bot"))
    }

    #[test]
    fn a_valid_response_emits_the_reply_and_actions_and_prunes_the_entry() {
        let (mut m, registry, run_id) = awaiting_run(&[
            ACTION_CHAT_POST,
            ACTION_TASKS_CREATE,
            ACTION_TASKS_UPDATE_STATUS,
        ]);
        let mut ctx = CaptureCtx::new()
            .at(8)
            .with_dispatch_origin()
            .with_registry(&registry)
            .with_transcript("general", transcript(2));
        exec(
            &mut m,
            &mut ctx,
            &result_event(
                &run_id,
                Ok(response(
                    &["on it"],
                    vec![
                        AgentAction::CreateTask {
                            task_id: "t1".into(),
                            title: "ship it".into(),
                        },
                        // updating a task created earlier in this SAME response
                        // is valid — tasks applies the follow-ups in order.
                        AgentAction::UpdateTaskStatus {
                            task_id: "t1".into(),
                            status: "in_progress".into(),
                        },
                    ],
                )),
            ),
        )
        .unwrap();
        commit(&mut m);

        assert_eq!(
            get_pending(&m, &run_id),
            None,
            "the delivered entry pruned — the dispatch module holds the history"
        );
        assert_eq!(
            ctx.chat_msgs(),
            vec![ChatMsg::PostMessage {
                channel_id: "general".into(),
                message_id: reply_message_id(&run_id),
                blocks: vec![Block::paragraph("on it")],
                thread: None,
                as_agent: Some("bot".into()),
            }],
            "the reply posts as the AGENT, under the run's message id"
        );
        assert_eq!(
            ctx.task_msgs(),
            vec![
                TaskMsg::CreateTask {
                    task_id: "t1".into(),
                    title: "ship it".into(),
                },
                TaskMsg::UpdateStatus {
                    task_id: "t1".into(),
                    status: TaskStatus::InProgress,
                },
            ]
        );
    }

    #[test]
    fn a_threaded_anchor_threads_the_reply() {
        let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
        let mut m = watched(TurnPolicy::All, &registry);
        // seq 3 is a reply to root 1; the pin records thread_root = 1.
        let mut thread_transcript = transcript(2);
        thread_transcript.push(message_in(
            "general",
            3,
            AuthorRef::User(vec![1; 32]),
            "in thread",
            Some(1),
        ));
        let mut ctx = CaptureCtx::new()
            .at(3)
            .with_tagging_origin()
            .with_registry(&registry)
            .with_transcript("general", thread_transcript.clone());
        exec(&mut m, &mut ctx, &engagement("general", 3, vec![])).unwrap();
        commit(&mut m);
        let run_id = run_id_for("general", 3, "bot");
        assert_eq!(get_pending(&m, &run_id).unwrap().thread_root, Some(1));
        // the envelope keys thread continuity by the ROOT, not the anchor.
        let dispatches = ctx.dispatch_msgs();
        let DispatchMsg::Dispatch { payload, .. } = &dispatches[0] else {
            panic!("expected a dispatch");
        };
        let envelope: serde_json::Value = serde_json::from_slice(payload).unwrap();
        assert_eq!(envelope["thread_key"], "general#1");

        let mut ctx = CaptureCtx::new()
            .at(9)
            .with_dispatch_origin()
            .with_registry(&registry)
            .with_transcript("general", thread_transcript);
        exec(
            &mut m,
            &mut ctx,
            &result_event(&run_id, Ok(response(&["answered"], vec![]))),
        )
        .unwrap();
        commit(&mut m);
        let posts = ctx.chat_msgs();
        assert_eq!(posts.len(), 1);
        let ChatMsg::PostMessage { thread, .. } = &posts[0] else {
            panic!("expected a post");
        };
        assert_eq!(*thread, Some(1), "the reply joins the anchor's thread");
    }

    #[test]
    fn invalid_responses_fail_the_run_and_surface_a_threaded_failure_reply() {
        // normalization already absorbed shape problems (prose, fences,
        // oversize); what remains failable is POLICY: task validity and
        // grants. every case emits NO follow-up except the ⚠ failure reply
        // (the agent here holds chat.post), leaves a breadcrumb, and prunes
        // the entry — never the block.
        let cases: Vec<(&str, Vec<u8>)> = vec![
            (
                "task already exists: t0",
                response(
                    &["ok"],
                    vec![AgentAction::CreateTask {
                        task_id: "t0".into(),
                        title: "dup of a committed task".into(),
                    }],
                ),
            ),
            (
                "task already exists: fresh",
                response(
                    &["ok"],
                    vec![
                        AgentAction::CreateTask {
                            task_id: "fresh".into(),
                            title: "one".into(),
                        },
                        AgentAction::CreateTask {
                            task_id: "fresh".into(),
                            title: "two".into(),
                        },
                    ],
                ),
            ),
            (
                "unknown task: ghost",
                response(
                    &["ok"],
                    vec![AgentAction::UpdateTaskStatus {
                        task_id: "ghost".into(),
                        status: "done".into(),
                    }],
                ),
            ),
            (
                "unknown task status",
                response(
                    &["ok"],
                    vec![AgentAction::UpdateTaskStatus {
                        task_id: "t0".into(),
                        status: "shipped".into(),
                    }],
                ),
            ),
            (
                "non-empty task_id",
                response(
                    &["ok"],
                    vec![AgentAction::CreateTask {
                        task_id: String::new(),
                        title: "x".into(),
                    }],
                ),
            ),
        ];
        for (fragment, bytes) in cases {
            let (mut m, registry, run_id) = awaiting_run(&[
                ACTION_CHAT_POST,
                ACTION_TASKS_CREATE,
                ACTION_TASKS_UPDATE_STATUS,
            ]);
            let mut ctx = CaptureCtx::new()
                .at(8)
                .with_dispatch_origin()
                .with_registry(&registry)
                .with_transcript("general", transcript(2))
                .with_task("t0");
            exec(&mut m, &mut ctx, &result_event(&run_id, Ok(bytes))).unwrap();
            assert!(
                ctx.task_msgs().is_empty(),
                "an invalid response must emit no task writes ({fragment})"
            );
            let posts = ctx.chat_msgs();
            assert_eq!(posts.len(), 1, "exactly one failure reply ({fragment})");
            let ChatMsg::PostMessage {
                message_id,
                blocks,
                as_agent,
                ..
            } = &posts[0]
            else {
                panic!("expected a post");
            };
            assert_eq!(
                *message_id,
                reply_message_id(&run_id),
                "the failure reply holds the run's one reply id ({fragment})"
            );
            assert_eq!(*as_agent, Some("bot".into()));
            assert_eq!(blocks.len(), 1, "one ⚠ paragraph ({fragment})");
            let Block::Paragraph(spans) = &blocks[0] else {
                panic!("expected a paragraph");
            };
            let text: String = spans.iter().map(|s| s.text.as_str()).collect();
            assert!(
                text.starts_with("⚠ BOT failed: "),
                "the reply names the agent's display name: {text}"
            );
            assert!(
                text.contains(fragment),
                "the reply carries the reason excerpt: {text}"
            );
            let breadcrumbs: Vec<String> = ctx
                .events
                .iter()
                .map(|e| String::from_utf8_lossy(&e.payload).into_owned())
                .collect();
            assert!(
                breadcrumbs.iter().any(|b| b.contains(fragment)),
                "breadcrumbs {breadcrumbs:?} must mention {fragment:?}"
            );
            commit(&mut m);
            assert_eq!(
                get_pending(&m, &run_id),
                None,
                "the failed run's entry pruned ({fragment})"
            );
        }
    }

    #[test]
    fn raw_model_text_normalizes_into_a_postable_reply() {
        // the dispatch oracle returns RAW text; the intake's deterministic
        // normalization turns prose, empty JSON, and oversized answers into
        // valid replies instead of failed runs.
        let cases: Vec<Vec<u8>> = vec![
            b"just prose, no JSON anywhere".to_vec(),
            response(&[], vec![]),
            "x".repeat(MAX_REPLY_BLOCKS_BYTES + 1).into_bytes(),
        ];
        for bytes in cases {
            let (mut m, registry, run_id) = awaiting_run(&[ACTION_CHAT_POST]);
            let mut ctx = CaptureCtx::new()
                .at(8)
                .with_dispatch_origin()
                .with_registry(&registry)
                .with_transcript("general", transcript(2));
            exec(&mut m, &mut ctx, &result_event(&run_id, Ok(bytes))).unwrap();
            commit(&mut m);
            assert_eq!(get_pending(&m, &run_id), None, "the entry pruned");
            let posts = ctx.chat_msgs();
            assert_eq!(posts.len(), 1, "exactly one normalized reply posts");
        }
    }

    #[test]
    fn code_blocks_survive_normalization_into_chat_blocks() {
        let (mut m, registry, run_id) = awaiting_run(&[ACTION_CHAT_POST]);
        let raw = r#"{"reply_blocks":[{"id":"b1","kind":"paragraph","text":"hello"},{"kind":"code","lang":"rust","text":"fn main() {}"},{"kind":"Alien","text":"dropped"},{"kind":"paragraph","text":"  "}],"actions":[]}"#;
        let mut ctx = CaptureCtx::new()
            .at(8)
            .with_dispatch_origin()
            .with_registry(&registry)
            .with_transcript("general", transcript(2));
        exec(
            &mut m,
            &mut ctx,
            &result_event(&run_id, Ok(raw.as_bytes().to_vec())),
        )
        .unwrap();
        let posts = ctx.chat_msgs();
        let ChatMsg::PostMessage { blocks, .. } = &posts[0] else {
            panic!("expected a post");
        };
        assert_eq!(
            *blocks,
            vec![
                Block::paragraph("hello"),
                Block::Code {
                    lang: Some("rust".into()),
                    text: "fn main() {}".into(),
                },
            ],
            "known kinds map to chat blocks; unknown kinds and blank texts drop"
        );
    }

    #[test]
    fn a_fenced_json_reply_is_parsed_into_prose_not_dumped_as_a_code_block() {
        // the observed failure: an agentic CLI wraps its AgentResponse in a
        // ```json fence despite the contract, the bare parse fails, and the
        // whole fenced string lands in chat as a raw code block. the tolerant
        // parser must recover the real prose.
        let (mut m, registry, run_id) = awaiting_run(&[ACTION_CHAT_POST]);
        let raw = "```json\n{\"reply_blocks\":[{\"kind\":\"paragraph\",\"text\":\"QUACKTEST! Hello there.\"}],\"actions\":[]}\n```";
        let mut ctx = CaptureCtx::new()
            .at(8)
            .with_dispatch_origin()
            .with_registry(&registry)
            .with_transcript("general", transcript(2));
        exec(
            &mut m,
            &mut ctx,
            &result_event(&run_id, Ok(raw.as_bytes().to_vec())),
        )
        .unwrap();
        let posts = ctx.chat_msgs();
        let ChatMsg::PostMessage { blocks, .. } = &posts[0] else {
            panic!("expected a post");
        };
        assert_eq!(
            *blocks,
            vec![Block::paragraph("QUACKTEST! Hello there.")],
            "a fenced AgentResponse is decoded to its prose, never posted as raw JSON"
        );
    }

    #[test]
    fn parse_strict_response_tolerates_the_shapes_llms_actually_emit() {
        let bare = r#"{"reply_blocks":[{"kind":"paragraph","text":"hi"}],"actions":[]}"#;
        assert_eq!(parse_strict_response(bare).unwrap().reply_blocks[0].text, "hi");

        // a fence with an info string (```json), the reproduced case.
        let fenced = format!("```json\n{bare}\n```");
        assert_eq!(
            parse_strict_response(&fenced).unwrap().reply_blocks[0].text,
            "hi"
        );

        // a bare fence (```), no info string.
        let bare_fence = format!("```\n{bare}\n```");
        assert_eq!(
            parse_strict_response(&bare_fence).unwrap().reply_blocks[0].text,
            "hi"
        );

        // JSON with a trailing line of prose the model tacked on.
        let trailing = format!("{fenced}\nHope that helps!");
        assert_eq!(
            parse_strict_response(&trailing).unwrap().reply_blocks[0].text,
            "hi"
        );

        // genuine prose (no JSON object) does NOT parse — it must fall back to
        // the raw-text paragraph, not be swallowed.
        assert!(parse_strict_response("just a plain hello, no json here").is_none());
        assert!(parse_strict_response("   ").is_none());

        // a `}` before the first `{` must not panic the outermost-object span.
        assert!(parse_strict_response("close } then open { please").is_none());
    }

    #[test]
    fn a_fenced_job_response_still_yields_actions_only() {
        // job runs drop reply_blocks; the fenced-parse path must still recover
        // the actions inside the fence.
        let raw = "```json\n{\"reply_blocks\":[{\"kind\":\"paragraph\",\"text\":\"noise\"}],\"actions\":[{\"create_task\":{\"task_id\":\"t1\",\"title\":\"did it\"}}]}\n```";
        let parsed = agent_response_from_text(raw, true);
        assert!(parsed.reply_blocks.is_empty(), "job runs post no chat reply");
        assert_eq!(parsed.actions.len(), 1, "the fenced action is recovered");
    }

    #[test]
    fn responses_beyond_the_agents_grants_fail_the_run() {
        // an agent granted ONLY chat.post must not create tasks...
        let (mut m, registry, run_id) = awaiting_run(&[ACTION_CHAT_POST]);
        let mut ctx = CaptureCtx::new()
            .with_dispatch_origin()
            .with_registry(&registry)
            .with_transcript("general", transcript(2));
        exec(
            &mut m,
            &mut ctx,
            &result_event(
                &run_id,
                Ok(response(
                    &["look what i did"],
                    vec![AgentAction::CreateTask {
                        task_id: "t1".into(),
                        title: "sneaky".into(),
                    }],
                )),
            ),
        )
        .unwrap();
        assert!(
            ctx.task_msgs().is_empty(),
            "a disallowed action emits no task writes"
        );
        // the agent holds chat.post, so the failure surfaces as the ⚠ reply.
        let posts = ctx.chat_msgs();
        assert_eq!(posts.len(), 1);
        let ChatMsg::PostMessage { blocks, .. } = &posts[0] else {
            panic!("expected a post");
        };
        assert_eq!(
            *blocks,
            vec![Block::paragraph(format!(
                "⚠ BOT failed: agent bot is not allowed to {ACTION_TASKS_CREATE}"
            ))]
        );
        commit(&mut m);
        assert_eq!(get_pending(&m, &run_id), None);

        // ...and an agent granted only tasks.create must not post replies —
        // and without chat.post the failure CANNOT surface in chat either:
        // the old breadcrumb-only silence holds.
        let (mut m, registry, run_id) = awaiting_run(&[ACTION_TASKS_CREATE]);
        let mut ctx = CaptureCtx::new()
            .with_dispatch_origin()
            .with_registry(&registry)
            .with_transcript("general", transcript(2));
        exec(
            &mut m,
            &mut ctx,
            &result_event(&run_id, Ok(response(&["hello"], vec![]))),
        )
        .unwrap();
        assert!(ctx.msgs.is_empty());
        let breadcrumbs: Vec<String> = ctx
            .events
            .iter()
            .map(|e| String::from_utf8_lossy(&e.payload).into_owned())
            .collect();
        assert!(
            breadcrumbs.iter().any(|b| b.contains(ACTION_CHAT_POST)),
            "the failure names the missing grant: {breadcrumbs:?}"
        );
        commit(&mut m);
        assert_eq!(get_pending(&m, &run_id), None);
    }

    #[test]
    fn task_actions_without_a_configured_tasks_module_fail_the_run() {
        let registry = registry(&[("bot", &[ACTION_CHAT_POST, ACTION_TASKS_CREATE])]);
        let mut m = RunsModule::new(
            "runs",
            "chat",
            "saga",
            "tagging",
            "dispatch",
            "agent",
            None,
            None,
        );
        let mut ctx = CaptureCtx::new()
            .with_origin(user(9))
            .with_registry(&registry);
        exec(
            &mut m,
            &mut ctx,
            &admin(&RunsMsg::WatchChannel {
                channel_id: "general".into(),
                policy: TurnPolicy::All,
            }),
        )
        .unwrap();
        commit(&mut m);
        engage_post(&mut m, &registry, 2, &[]);
        commit(&mut m);
        let run_id = run_id_for("general", 2, "bot");

        let mut ctx = CaptureCtx::new()
            .with_dispatch_origin()
            .with_registry(&registry)
            .with_transcript("general", transcript(2));
        exec(
            &mut m,
            &mut ctx,
            &result_event(
                &run_id,
                Ok(response(
                    &["ok"],
                    vec![AgentAction::CreateTask {
                        task_id: "t1".into(),
                        title: "x".into(),
                    }],
                )),
            ),
        )
        .unwrap();
        assert!(ctx.task_msgs().is_empty(), "no task write may escape");
        // the failure still surfaces in chat — the agent holds chat.post.
        assert_eq!(ctx.chat_msgs().len(), 1);
        let breadcrumbs: Vec<String> = ctx
            .events
            .iter()
            .map(|e| String::from_utf8_lossy(&e.payload).into_owned())
            .collect();
        assert!(breadcrumbs.iter().any(|b| b.contains("no tasks module")));
        commit(&mut m);
        assert_eq!(get_pending(&m, &run_id), None);
    }

    #[test]
    fn a_squatted_reply_message_id_fails_the_run_instead_of_the_block() {
        let (mut m, registry, run_id) = awaiting_run(&[ACTION_CHAT_POST]);
        // someone posted a message whose id IS the run's reply id.
        let mut squatted = transcript(2);
        squatted[1].head.message_id = reply_message_id(&run_id);
        let mut ctx = CaptureCtx::new()
            .with_dispatch_origin()
            .with_registry(&registry)
            .with_transcript("general", squatted);
        exec(
            &mut m,
            &mut ctx,
            &result_event(&run_id, Ok(response(&["hi"], vec![]))),
        )
        .unwrap();
        assert!(ctx.msgs.is_empty(), "the squatted id emits NOTHING");
        let breadcrumbs: Vec<String> = ctx
            .events
            .iter()
            .map(|e| String::from_utf8_lossy(&e.payload).into_owned())
            .collect();
        assert!(breadcrumbs.iter().any(|b| b.contains("already taken")));
        commit(&mut m);
        assert_eq!(get_pending(&m, &run_id), None);
    }

    #[test]
    fn a_full_thread_fails_the_run_instead_of_the_block() {
        let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
        let mut m = watched(TurnPolicy::All, &registry);
        // the anchor replies to a root that has hit the reply cap.
        let mut root = message(1, "root");
        root.head.reply_count = MAX_THREAD_REPLIES as u64;
        let anchor = message_in("general", 2, AuthorRef::User(vec![1; 32]), "reply", Some(1));
        let full = vec![root, anchor];
        let mut ctx = CaptureCtx::new()
            .at(2)
            .with_tagging_origin()
            .with_registry(&registry)
            .with_transcript("general", full.clone());
        exec(&mut m, &mut ctx, &engagement("general", 2, vec![])).unwrap();
        commit(&mut m);
        let run_id = run_id_for("general", 2, "bot");

        let mut ctx = CaptureCtx::new()
            .with_dispatch_origin()
            .with_registry(&registry)
            .with_transcript("general", full);
        exec(
            &mut m,
            &mut ctx,
            &result_event(&run_id, Ok(response(&["hi"], vec![]))),
        )
        .unwrap();
        assert!(ctx.msgs.is_empty());
        let breadcrumbs: Vec<String> = ctx
            .events
            .iter()
            .map(|e| String::from_utf8_lossy(&e.payload).into_owned())
            .collect();
        assert!(breadcrumbs.iter().any(|b| b.contains("thread reply cap")));
        commit(&mut m);
        assert_eq!(get_pending(&m, &run_id), None);
    }

    #[test]
    fn a_failed_dispatch_outcome_posts_a_threaded_failure_reply_and_prunes_the_entry() {
        // the anchor is a thread reply, so the failure reply must join the
        // same thread a success reply would have.
        let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
        let mut m = watched(TurnPolicy::All, &registry);
        let mut thread_transcript = transcript(1);
        thread_transcript.push(message_in(
            "general",
            2,
            AuthorRef::User(vec![1; 32]),
            "in thread",
            Some(1),
        ));
        let mut ctx = CaptureCtx::new()
            .at(2)
            .with_tagging_origin()
            .with_registry(&registry)
            .with_transcript("general", thread_transcript.clone());
        exec(&mut m, &mut ctx, &engagement("general", 2, vec![])).unwrap();
        commit(&mut m);
        let run_id = run_id_for("general", 2, "bot");

        // the dispatch plane already folded saga failures, timeouts, and
        // contract violations into the Err lane — one shape lands here. the
        // reason's newlines collapse into the single-paragraph excerpt.
        let mut ctx = CaptureCtx::new()
            .at(20)
            .with_dispatch_origin()
            .with_registry(&registry)
            .with_transcript("general", thread_transcript.clone());
        exec(
            &mut m,
            &mut ctx,
            &result_event(&run_id, Err("worker exploded\nstack line two".into())),
        )
        .unwrap();
        assert_eq!(
            ctx.chat_msgs(),
            vec![ChatMsg::PostMessage {
                channel_id: "general".into(),
                message_id: reply_message_id(&run_id),
                blocks: vec![Block::paragraph(
                    "⚠ BOT failed: worker exploded stack line two"
                )],
                thread: Some(1),
                as_agent: Some("bot".into()),
            }],
            "one threaded ⚠ reply, authored as the agent"
        );
        commit(&mut m);
        assert_eq!(get_pending(&m, &run_id), None, "the entry pruned");

        // a redelivered result finds no entry: no second post, breadcrumb
        // only — the one-reply-per-run dedup holds.
        let mut ctx = CaptureCtx::new()
            .at(21)
            .with_dispatch_origin()
            .with_registry(&registry)
            .with_transcript("general", thread_transcript);
        exec(
            &mut m,
            &mut ctx,
            &result_event(&run_id, Err("worker exploded\nstack line two".into())),
        )
        .unwrap();
        assert!(ctx.msgs.is_empty(), "a redelivery must never double-post");
        let breadcrumbs: Vec<String> = ctx
            .events
            .iter()
            .map(|e| String::from_utf8_lossy(&e.payload).into_owned())
            .collect();
        assert!(breadcrumbs.iter().any(|b| b.contains("unknown dispatch")));
    }

    #[test]
    fn a_failure_reply_requires_the_chat_post_grant() {
        // without chat.post the pre-existing silence holds: breadcrumbs only,
        // never a post the validator could not have proven postable.
        let (mut m, registry, run_id) = awaiting_run(&[]);
        let mut ctx = CaptureCtx::new()
            .at(20)
            .with_dispatch_origin()
            .with_registry(&registry);
        exec(
            &mut m,
            &mut ctx,
            &result_event(&run_id, Err("timed out".into())),
        )
        .unwrap();
        assert!(ctx.msgs.is_empty(), "no grant, no failure post");
        let breadcrumbs: Vec<String> = ctx
            .events
            .iter()
            .map(|e| String::from_utf8_lossy(&e.payload).into_owned())
            .collect();
        assert!(
            breadcrumbs
                .iter()
                .any(|b| b.contains("failure not surfaced") && b.contains(ACTION_CHAT_POST)),
            "the silence leaves its reason as a breadcrumb: {breadcrumbs:?}"
        );
        commit(&mut m);
        assert_eq!(get_pending(&m, &run_id), None, "the entry still pruned");
    }

    #[test]
    fn failure_excerpts_are_single_line_and_bounded() {
        assert_eq!(
            failure_excerpt("line one\n\n  line two\tend"),
            "line one line two end"
        );
        assert_eq!(failure_excerpt("  \n \t "), "no error detail");
        let long = "x".repeat(FAILURE_EXCERPT_BYTES * 2);
        let bounded = failure_excerpt(&long);
        assert!(bounded.len() <= FAILURE_EXCERPT_BYTES + '…'.len_utf8());
        assert!(bounded.ends_with('…'));
    }

    // ---- the jobs lane ----------------------------------------------------------

    /// the canned registry for the jobs lane: "duck" with task grants.
    fn job_registry() -> Registry {
        registry(&[("duck", &[ACTION_TASKS_CREATE])])
    }

    #[test]
    fn a_job_submit_claims_and_dispatches_with_the_spec_payload() {
        let registry = job_registry();
        let mut m = module();
        let mut ctx = CaptureCtx::new().at(3).with_jobs_origin().with_registry(&registry);
        exec(
            &mut m,
            &mut ctx,
            &jobs_event("job-1", "agent/duck", "summarize this work item"),
        )
        .unwrap();
        commit(&mut m);

        // the claim and the dispatch are staged together in the submit block.
        assert_eq!(
            ctx.job_msgs(),
            vec![JobsMsg::Claim {
                job_id: "job-1".into(),
                lease_views: JOB_RUN_LEASE_VIEWS,
            }]
        );
        let run_id = job_run_id_for("job-1", "duck", 3);
        let dispatches = ctx.dispatch_msgs();
        assert_eq!(dispatches.len(), 1);
        let DispatchMsg::Dispatch {
            dispatch_id,
            recipe_id,
            payload,
        } = &dispatches[0]
        else {
            panic!("expected a dispatch");
        };
        assert_eq!(*dispatch_id, dispatch_id_for(&run_id));
        assert_eq!(*recipe_id, recipe_id_for("duck"));
        let envelope: serde_json::Value =
            serde_json::from_slice(payload).expect("the payload is a JSON envelope");
        assert_eq!(envelope["ducktape_run"], RUN_ENVELOPE_VERSION);
        assert_eq!(envelope["agent_id"], "duck", "the claiming agent");
        assert_eq!(
            envelope["prompt_hash"],
            "07".repeat(PROMPT_HASH_LEN),
            "the claiming agent's prompt pin rides along"
        );
        assert!(
            envelope["thread_key"].is_null(),
            "job runs have no channel, so no thread key"
        );
        let conversation = envelope["conversation"].as_str().unwrap();
        assert!(
            conversation.contains("summarize this work item"),
            "the FULL job spec rides the payload"
        );
        assert!(
            envelope["contract"]
                .as_str()
                .unwrap()
                .contains("Return ONLY a JSON object")
        );
        assert!(
            conversation.contains("chat replies are not delivered for job runs"),
            "job framing rides along"
        );

        let entry = get_pending(&m, &run_id).expect("job entry staged");
        assert_eq!(entry.job_id, Some("job-1".into()));
        assert_eq!(entry.job_claim_height, 3);
        assert_eq!(entry.agent_id, "duck");
        assert_eq!(entry.requester, SagaOrigin::Module("jobs".into()));
    }

    #[test]
    fn an_oversized_job_spec_is_left_unclaimed_by_the_payload_cap() {
        // the envelope wraps the spec, so a spec at the dispatch cap must
        // overflow it — the job stays on the board, breadcrumb only.
        let registry = job_registry();
        let mut m = module();
        let spec = "x".repeat(MAX_PAYLOAD_BYTES);
        let mut ctx = CaptureCtx::new().at(3).with_jobs_origin().with_registry(&registry);
        exec(&mut m, &mut ctx, &jobs_event("job-1", "agent/duck", &spec)).unwrap();
        assert!(ctx.msgs.is_empty(), "no claim and no dispatch may land");
        let breadcrumbs: Vec<String> = ctx
            .events
            .iter()
            .map(|e| String::from_utf8_lossy(&e.payload).into_owned())
            .collect();
        assert!(
            breadcrumbs
                .iter()
                .any(|b| b.contains("payload exceeds the dispatch cap")),
            "the skip leaves a breadcrumb: {breadcrumbs:?}"
        );
        commit(&mut m);
        assert!(pending_runs(&m).is_empty());
    }

    #[test]
    fn unknown_paused_or_foreign_kind_jobs_are_left_unclaimed() {
        let mut registry = job_registry();
        let mut m = module();
        let root = m.root();

        // an unregistered agent kind: no claim, no dispatch, no entry.
        let mut ctx = CaptureCtx::new().at(2).with_jobs_origin().with_registry(&registry);
        exec(&mut m, &mut ctx, &jobs_event("j", "agent/ghost", "s")).unwrap();
        assert!(ctx.msgs.is_empty());

        // a non-agent kind is somebody else's job.
        let mut ctx = CaptureCtx::new().at(2).with_jobs_origin().with_registry(&registry);
        exec(&mut m, &mut ctx, &jobs_event("j", "render/video", "s")).unwrap();
        assert!(ctx.msgs.is_empty());

        // a paused agent never claims.
        pause(&mut registry, "duck");
        let mut ctx = CaptureCtx::new().at(2).with_jobs_origin().with_registry(&registry);
        exec(&mut m, &mut ctx, &jobs_event("j", "agent/duck", "s")).unwrap();
        assert!(ctx.msgs.is_empty());
        commit(&mut m);
        assert_eq!(m.root(), root, "nothing moved the root");
        assert!(pending_runs(&m).is_empty());
    }

    #[test]
    fn a_job_result_finalizes_the_board_and_emits_actions() {
        let registry = job_registry();
        let mut m = module();
        let mut ctx = CaptureCtx::new().at(3).with_jobs_origin().with_registry(&registry);
        exec(&mut m, &mut ctx, &jobs_event("job-1", "agent/duck", "spec")).unwrap();
        commit(&mut m);
        let run_id = job_run_id_for("job-1", "duck", 3);

        let bytes = response(
            &[],
            vec![AgentAction::CreateTask {
                task_id: "job-task".into(),
                title: "complete job".into(),
            }],
        );
        let mut ctx = CaptureCtx::new()
            .at(10)
            .with_dispatch_origin()
            .with_registry(&registry)
            .with_claimed_job("job-1", 3);
        exec(&mut m, &mut ctx, &result_event(&run_id, Ok(bytes.clone()))).unwrap();
        commit(&mut m);

        assert_eq!(get_pending(&m, &run_id), None, "the job entry pruned");
        assert_eq!(
            ctx.task_msgs(),
            vec![TaskMsg::CreateTask {
                task_id: "job-task".into(),
                title: "complete job".into(),
            }]
        );
        let finalize = ctx.job_msgs();
        assert_eq!(finalize.len(), 1);
        let JobsMsg::Finalize {
            job_id,
            ok,
            payload,
        } = &finalize[0]
        else {
            panic!("expected a finalize");
        };
        assert_eq!(job_id, "job-1");
        assert!(*ok);
        // a message-only job result finalizes as a faceted DeliveryReceipt whose
        // `response` is the normalized AgentResponse (no data / output_ref facets).
        let v: serde_json::Value = serde_json::from_str(payload).unwrap();
        assert_eq!(v["ducktape_delivery"], 1);
        assert_eq!(v["status"], "ok");
        assert!(v.get("data").is_none(), "no data facet on a message-only result");
        assert!(v.get("output_ref").is_none(), "no artifact facet");
        let expected: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["response"], expected, "response is the normalized AgentResponse");
        assert!(ctx.chat_msgs().is_empty(), "job runs never post to chat");
    }

    #[test]
    fn a_failed_job_result_finalizes_with_error_detail() {
        let registry = job_registry();
        let mut m = module();
        let mut ctx = CaptureCtx::new().at(3).with_jobs_origin().with_registry(&registry);
        exec(&mut m, &mut ctx, &jobs_event("job-1", "agent/duck", "spec")).unwrap();
        commit(&mut m);
        let run_id = job_run_id_for("job-1", "duck", 3);

        let mut ctx = CaptureCtx::new()
            .at(10)
            .with_dispatch_origin()
            .with_registry(&registry)
            .with_claimed_job("job-1", 3);
        exec(
            &mut m,
            &mut ctx,
            &result_event(&run_id, Err("model unavailable".into())),
        )
        .unwrap();
        commit(&mut m);

        assert_eq!(get_pending(&m, &run_id), None);
        assert!(
            ctx.chat_msgs().is_empty(),
            "a job run has no channel — failures never post to chat"
        );
        assert_eq!(
            ctx.job_msgs(),
            vec![JobsMsg::Finalize {
                job_id: "job-1".into(),
                ok: false,
                payload: "model unavailable".into(),
            }]
        );
    }

    #[test]
    fn a_job_response_with_reply_blocks_normalizes_to_actions_only() {
        // job runs have no channel: normalization CLEARS reply blocks. a
        // response left with neither blocks nor actions fails the run and
        // finalizes the job as failed.
        let registry = job_registry();
        let mut m = module();
        let mut ctx = CaptureCtx::new().at(3).with_jobs_origin().with_registry(&registry);
        exec(&mut m, &mut ctx, &jobs_event("job-1", "agent/duck", "spec")).unwrap();
        commit(&mut m);
        let run_id = job_run_id_for("job-1", "duck", 3);

        let mut ctx = CaptureCtx::new()
            .at(10)
            .with_dispatch_origin()
            .with_registry(&registry)
            .with_claimed_job("job-1", 3);
        exec(
            &mut m,
            &mut ctx,
            &result_event(&run_id, Ok(response(&["chatty"], vec![]))),
        )
        .unwrap();
        commit(&mut m);

        assert!(ctx.chat_msgs().is_empty(), "no chat post for a job run");
        let finalize = ctx.job_msgs();
        assert_eq!(finalize.len(), 1);
        let JobsMsg::Finalize { ok, payload, .. } = &finalize[0] else {
            panic!("expected a finalize");
        };
        assert!(!*ok, "an empty normalized response fails the job run");
        assert!(payload.contains("neither reply blocks nor actions"));
        assert_eq!(get_pending(&m, &run_id), None);
    }

    #[test]
    fn a_stale_job_run_does_not_finalize_a_reclaimed_episode() {
        // the board reclaimed and re-claimed the job at a LATER height: the
        // stale run's delivery must not finalize the new episode.
        let registry = job_registry();
        let mut m = module();
        let mut ctx = CaptureCtx::new().at(3).with_jobs_origin().with_registry(&registry);
        exec(&mut m, &mut ctx, &jobs_event("job-1", "agent/duck", "spec")).unwrap();
        commit(&mut m);
        let run_id = job_run_id_for("job-1", "duck", 3);

        let mut ctx = CaptureCtx::new()
            .at(2000)
            .with_dispatch_origin()
            .with_registry(&registry)
            .with_claimed_job("job-1", 1005);
        exec(
            &mut m,
            &mut ctx,
            &result_event(
                &run_id,
                Ok(response(
                    &[],
                    vec![AgentAction::CreateTask {
                        task_id: "stale".into(),
                        title: "late".into(),
                    }],
                )),
            ),
        )
        .unwrap();
        commit(&mut m);

        assert!(
            ctx.job_msgs().is_empty(),
            "a stale claim episode is never finalized"
        );
        assert_eq!(get_pending(&m, &run_id), None, "the stale entry still prunes");
    }

    // ---- explicit runs + cancellation ------------------------------------------------

    #[test]
    fn request_run_validates_agent_origin_and_anchor() {
        let mut registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
        let mut m = watched(TurnPolicy::Mention, &registry);
        let request = |agent: &str, seq: u64| {
            admin(&RunsMsg::RequestRun {
                agent_id: agent.into(),
                channel_id: "general".into(),
                anchor_seq: seq,
            })
        };

        // unknown agent, empty origin, missing anchor, anchor 0: all errors.
        let mut ctx = CaptureCtx::new()
            .with_origin(user(1))
            .with_registry(&registry)
            .with_transcript("general", transcript(3));
        assert!(exec(&mut m, &mut ctx, &request("ghost", 3)).is_err());
        abort(&mut m);
        let mut ctx = CaptureCtx::new()
            .with_origin(Origin::External(Vec::new()))
            .with_registry(&registry)
            .with_transcript("general", transcript(3));
        assert!(exec(&mut m, &mut ctx, &request("bot", 3)).is_err());
        abort(&mut m);
        let mut ctx = CaptureCtx::new()
            .with_origin(user(1))
            .with_registry(&registry)
            .with_transcript("general", transcript(3));
        assert!(
            exec(&mut m, &mut ctx, &request("bot", 9)).is_err(),
            "an anchor past the head does not exist"
        );
        abort(&mut m);
        assert!(exec(&mut m, &mut ctx, &request("bot", 0)).is_err());
        abort(&mut m);

        // a paused agent cannot be explicitly run either.
        pause(&mut registry, "bot");
        let mut ctx = CaptureCtx::new()
            .with_origin(user(1))
            .with_registry(&registry)
            .with_transcript("general", transcript(3));
        assert!(exec(&mut m, &mut ctx, &request("bot", 3)).is_err());
        abort(&mut m);

        // resumed, the request lands: entry staged + dispatch emitted,
        // requester recorded as the submitting user.
        registry.get_mut("bot").unwrap().status = AgentStatus::Active;
        let mut ctx = CaptureCtx::new()
            .at(6)
            .with_origin(user(1))
            .with_registry(&registry)
            .with_transcript("general", transcript(3));
        exec(&mut m, &mut ctx, &request("bot", 3)).unwrap();
        assert_eq!(ctx.dispatch_msgs().len(), 1);
        commit(&mut m);
        let entry = get_pending(&m, &run_id_for("general", 3, "bot")).unwrap();
        assert_eq!(entry.requester, SagaOrigin::External(vec![1; 32]));
        assert_eq!(entry.created_at, 6);
    }

    #[test]
    fn cancel_run_is_gated_to_the_requester_or_the_owner() {
        let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
        let mut m = watched(TurnPolicy::Mention, &registry);
        let mut ctx = CaptureCtx::new()
            .with_origin(user(1))
            .with_registry(&registry)
            .with_transcript("general", transcript(3));
        exec(
            &mut m,
            &mut ctx,
            &admin(&RunsMsg::RequestRun {
                agent_id: "bot".into(),
                channel_id: "general".into(),
                anchor_seq: 3,
            }),
        )
        .unwrap();
        commit(&mut m);
        let run_id = run_id_for("general", 3, "bot");
        let cancel = admin(&RunsMsg::CancelRun {
            run_id: run_id.clone(),
        });

        // a foreign origin (neither requester user(1) nor owner user(9)).
        let mut ctx = CaptureCtx::new().with_origin(user(2)).with_registry(&registry);
        assert!(exec(&mut m, &mut ctx, &cancel).is_err());
        abort(&mut m);
        // an unknown run is an error too.
        let mut ctx = CaptureCtx::new().with_origin(user(1)).with_registry(&registry);
        assert!(
            exec(
                &mut m,
                &mut ctx,
                &admin(&RunsMsg::CancelRun {
                    run_id: "nope".into(),
                }),
            )
            .is_err()
        );
        abort(&mut m);

        // the REQUESTER cancels: the dispatch plane is told; the entry STAYS
        // pending — the plane's Err("cancelled") delivery is the one result
        // path that prunes it.
        let mut ctx = CaptureCtx::new().at(7).with_origin(user(1)).with_registry(&registry);
        exec(&mut m, &mut ctx, &cancel).unwrap();
        assert_eq!(
            ctx.dispatch_msgs(),
            vec![DispatchMsg::CancelDispatch {
                dispatch_id: dispatch_id_for(&run_id),
            }]
        );
        commit(&mut m);
        assert!(get_pending(&m, &run_id).is_some(), "still pending delivery");

        // the plane's Err("cancelled") delivery prunes the entry. it rides
        // the ONE result path, so it surfaces like any failed run — a
        // threaded ⚠ reply, never silence.
        let mut ctx = CaptureCtx::new().with_dispatch_origin().with_registry(&registry);
        exec(
            &mut m,
            &mut ctx,
            &result_event(&run_id, Err("cancelled".into())),
        )
        .unwrap();
        assert_eq!(
            ctx.chat_msgs(),
            vec![ChatMsg::PostMessage {
                channel_id: "general".into(),
                message_id: reply_message_id(&run_id),
                blocks: vec![Block::paragraph("⚠ BOT failed: cancelled")],
                thread: None,
                as_agent: Some("bot".into()),
            }]
        );
        commit(&mut m);
        assert_eq!(get_pending(&m, &run_id), None);

        // cancelling the now-delivered run is an idempotent no-op (the
        // dispatch record proves it existed); a truly unknown one errors.
        let mut ctx = CaptureCtx::new()
            .with_origin(user(1))
            .with_registry(&registry)
            .with_taken_dispatch(&dispatch_id_for(&run_id));
        exec(&mut m, &mut ctx, &cancel).unwrap();
        assert!(ctx.msgs.is_empty());

        // the OWNER may cancel an engagement-created run (requester = the
        // tagging plane).
        engage_post(&mut m, &registry, 2, &["bot"]);
        commit(&mut m);
        let engaged_run = run_id_for("general", 2, "bot");
        let mut ctx = CaptureCtx::new().with_origin(user(9)).with_registry(&registry);
        exec(
            &mut m,
            &mut ctx,
            &admin(&RunsMsg::CancelRun {
                run_id: engaged_run.clone(),
            }),
        )
        .unwrap();
        assert_eq!(ctx.dispatch_msgs().len(), 1);
    }

    // ---- determinism + queries + state-sync -------------------------------------------

    #[test]
    fn two_instances_replaying_the_same_ops_produce_identical_roots() {
        let registry = registry(&[("bot", &[ACTION_CHAT_POST]), ("z", &[])]);
        let run_id = run_id_for("general", 2, "bot");
        let build = || {
            let mut m = module();
            let mut roots = Vec::new();
            // block 1: watch.
            let mut ctx = CaptureCtx::new()
                .at(1)
                .with_origin(user(9))
                .with_registry(&registry);
            exec(
                &mut m,
                &mut ctx,
                &admin(&RunsMsg::WatchChannel {
                    channel_id: "general".into(),
                    policy: TurnPolicy::Mention,
                }),
            )
            .unwrap();
            commit(&mut m);
            roots.push(m.root());
            // block 2: an engagement engages bot.
            let mut ctx = CaptureCtx::new()
                .at(2)
                .with_tagging_origin()
                .with_registry(&registry)
                .with_transcript("general", transcript(2));
            exec(
                &mut m,
                &mut ctx,
                &engagement("general", 2, vec![agent_tag("bot")]),
            )
            .unwrap();
            commit(&mut m);
            roots.push(m.root());
            // block 3: the dispatch result lands and prunes.
            let mut ctx = CaptureCtx::new()
                .at(3)
                .with_dispatch_origin()
                .with_registry(&registry)
                .with_transcript("general", transcript(2));
            exec(
                &mut m,
                &mut ctx,
                &result_event(&run_id, Ok(response(&["done"], vec![]))),
            )
            .unwrap();
            commit(&mut m);
            roots.push(m.root());
            // block 4: a second watch.
            let mut ctx = CaptureCtx::new()
                .at(4)
                .with_origin(user(9))
                .with_registry(&registry);
            exec(
                &mut m,
                &mut ctx,
                &admin(&RunsMsg::WatchChannel {
                    channel_id: "dev".into(),
                    policy: TurnPolicy::All,
                }),
            )
            .unwrap();
            commit(&mut m);
            roots.push(m.root());
            roots
        };

        let left = build();
        let right = build();
        assert_eq!(left, right, "same ops, same blocks -> identical roots");
        assert_ne!(*left.last().unwrap(), StateRoot::ZERO);
    }

    #[test]
    fn queries_list_pending_and_watches() {
        let registry = registry(&[("a", &[]), ("b", &[])]);
        let mut m = watched(TurnPolicy::All, &registry);
        // watch a second channel and create runs in both.
        let mut ctx = CaptureCtx::new().with_origin(user(9)).with_registry(&registry);
        exec(
            &mut m,
            &mut ctx,
            &admin(&RunsMsg::WatchChannel {
                channel_id: "dev".into(),
                policy: TurnPolicy::All,
            }),
        )
        .unwrap();
        commit(&mut m);
        engage_post(&mut m, &registry, 2, &[]);
        let mut ctx = CaptureCtx::new()
            .at(3)
            .with_tagging_origin()
            .with_registry(&registry)
            .with_transcript(
                "dev",
                vec![message_in(
                    "dev",
                    1,
                    AuthorRef::User(vec![1; 32]),
                    "hello dev",
                    None,
                )],
            );
        exec(&mut m, &mut ctx, &engagement("dev", 1, vec![])).unwrap();
        commit(&mut m);

        let runs = pending_runs(&m);
        assert_eq!(runs.len(), 4, "2 agents x 2 channels, all in flight");
        assert!(
            runs.iter()
                .all(|r| r.dispatch_id == dispatch_id_for(&r.run_id)),
            "every view carries its own dispatch id"
        );

        let reply = block_on(m.query(&encode_query(&RunsQuery::Watches))).unwrap();
        let RunsReply::Watches(watches) = runs_decode_reply(&reply).unwrap() else {
            panic!("watches reply expected");
        };
        assert_eq!(
            watches,
            vec![
                WatchView {
                    channel_id: "dev".into(),
                    policy: TurnPolicy::All,
                },
                WatchView {
                    channel_id: "general".into(),
                    policy: TurnPolicy::All,
                },
            ]
        );
    }

    #[test]
    fn state_sync_handle_exposes_the_snapshot_bytes() {
        let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
        let mut m = watched(TurnPolicy::All, &registry);
        engage_post(&mut m, &registry, 2, &[]);
        commit(&mut m);
        assert_eq!(
            m.state_sync_handle().unwrap(),
            StateSyncHandle::SnapshotBytes(m.snapshot()),
            "the handle IS the canonical snapshot"
        );
    }
}
