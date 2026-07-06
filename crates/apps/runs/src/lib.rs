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
//!   rides the dispatch as committed payload data, so any validator holds the
//!   exact prompt input as ordered state, and the reply is never presented as
//!   ordered before its anchor.
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

use std::collections::{BTreeMap, BTreeSet};

use agent::{
    ACTION_CHAT_POST, AgentAction, AgentEvent, AgentQuery, AgentRecord, AgentReply, AgentResponse,
    AgentStatus, MAX_ACTIONS_PER_RUN, MAX_REPLY_BLOCKS_BYTES, RENDERER_MEMORY_GENERATION,
    RESERVED_ID_SEPARATOR, ReplyBlock, decode_event as agent_decode_event,
    decode_reply as agent_decode_reply, encode_query as agent_encode_query, encode_response,
};
use chat::{
    AuthorRef, Block, ChatMsg, ChatQuery, ChatReply, MAX_THREAD_REPLIES, MessageView,
    decode_reply as chat_decode_reply, encode_msg as chat_encode_msg,
    encode_query as chat_encode_query,
};
use dispatch::{
    DispatchMsg, DispatchQuery, DispatchReply, MAX_PAYLOAD_BYTES, OutputContract, ResultEvent,
    Routing, decode_reply as dispatch_decode_reply, decode_result_event,
    encode_msg as dispatch_encode_msg, encode_query as dispatch_encode_query,
};
use jobs::{
    JobStatus, JobsEvent, JobsMsg, JobsQuery, JobsReply, decode_event as jobs_decode_event,
    decode_reply as jobs_decode_reply, encode_msg as jobs_encode_msg,
    encode_query as jobs_encode_query,
};
use memory::{
    Body, MemoryQuery, MemoryReply, decode_reply as memory_decode_reply,
    encode_query as memory_encode_query,
};
use saga::SagaOrigin;
use sdk::{Ctx, Error, Event, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};
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

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ---- dispatch payload composition ------------------------------------------------
// the dispatch plane's rule: the DISPATCHER composes the entire model input,
// in consensus, and the host-side worker feeds it to the provider verbatim.

/// generic instructions when the agent has no consensus-resident prompt.
const DEFAULT_PROMPT: &str =
    "You are a Ducktape agent. Reply helpfully and return only the requested JSON output.";

/// the strict output contract appended to every composed payload — exactly
/// the [`AgentResponse`] wire shape.
const STRICT_OUTPUT_INSTRUCTION: &str = r#"Return ONLY a JSON object with this shape:
{"reply_blocks":[{"id":"<uuid>","kind":"paragraph","text":"..."}],"actions":[]}
Allowed reply block kinds are paragraph, heading, and code. heading is rendered as a paragraph in Ducktape chat. code may include an optional "lang". Actions are optional and must use only actions allowed by the agent registry. Do not include markdown fences around the JSON."#;

/// the shared payload head: the agent's RESOLVED prompt (pin-verified at
/// compose time) when it has one, then the generic instructions, then the
/// strict-output contract — all committed consensus data (P4).
fn render_payload_head(prompt: Option<&str>) -> String {
    let mut out = String::new();
    if let Some(prompt) = prompt {
        out.push_str(prompt);
        out.push_str("\n\n");
    }
    out.push_str(DEFAULT_PROMPT);
    out.push_str("\n\n");
    out.push_str(STRICT_OUTPUT_INSTRUCTION);
    out
}

/// flatten a chat run into the single payload a non-interactive CLI takes:
/// the agent's resolved prompt (if registered), the system instructions, the
/// strict-output contract, then the conversation.
fn render_payload(
    module_id: &str,
    agent_id: &str,
    prompt: Option<&str>,
    transcript: &[MessageView],
) -> String {
    let mut out = render_payload_head(prompt);
    out.push_str("\n\n");
    if transcript.is_empty() {
        out.push_str("No transcript was embedded for this run. Answer the user helpfully.");
        return out;
    }
    out.push_str("Conversation so far:\n");
    for message in transcript {
        let speaker = match &message.head.author {
            AuthorRef::Agent {
                module,
                agent_id: author,
            } if module == module_id && author == agent_id => "you",
            _ => "them",
        };
        out.push_str(&format!("[{speaker}] {}\n", render_message(message)));
    }
    out.push_str("\nReply as the agent.");
    out
}

/// flatten a job-backed run the same way: prompt (if registered),
/// instructions, contract, then the job's coordinates and its FULL submitted
/// spec.
fn render_job_payload(job_id: &str, spec: &str, prompt: Option<&str>) -> String {
    let mut out = render_payload_head(prompt);
    out.push_str(&format!(
        "\n\nJob {job_id} — chat replies are not delivered for job runs; respond with actions only.\n\nJob spec:\n{spec}"
    ));
    out
}

fn render_message(message: &MessageView) -> String {
    format!(
        "{} @{}: {}",
        render_author(&message.head.author),
        message.seq,
        message
            .head
            .blocks
            .iter()
            .map(render_block)
            .collect::<Vec<_>>()
            .join("\n")
    )
}

fn render_author(author: &AuthorRef) -> String {
    match author {
        AuthorRef::User(bytes) => format!("user:{}", hex(bytes)),
        AuthorRef::Agent { module, agent_id } => format!("agent:{module}/{agent_id}"),
        AuthorRef::Module(module) => format!("module:{module}"),
        AuthorRef::System => "system".into(),
    }
}

fn render_block(block: &Block) -> String {
    match block {
        Block::Paragraph(spans) => spans.iter().map(|s| s.text.as_str()).collect(),
        Block::Code { lang, text } => match lang {
            Some(lang) if !lang.is_empty() => format!("```{lang}\n{text}\n```"),
            _ => format!("```\n{text}\n```"),
        },
        Block::Quote(spans) => {
            let text: String = spans.iter().map(|s| s.text.as_str()).collect();
            text.lines()
                .map(|line| format!("> {line}"))
                .collect::<Vec<_>>()
                .join("\n")
        }
        Block::Divider => "---".into(),
    }
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

/// the model's raw answer as a NORMALIZED [`AgentResponse`]: the wire shape
/// when it parses (unknown kinds and empty texts drop), a plain paragraph
/// reply as the fallback for prose. job runs never carry reply blocks — there
/// is no channel to deliver them to.
fn agent_response_from_text(text: &str, job_run: bool) -> AgentResponse {
    let parsed = serde_json::from_str::<AgentResponse>(text).unwrap_or_else(|_| AgentResponse {
        reply_blocks: if job_run {
            Vec::new()
        } else {
            vec![paragraph_block(non_empty_text(text))]
        },
        actions: Vec::new(),
    });
    normalize_response(parsed, text, job_run)
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
    let bytes =
        serde_json::to_vec(&to_chat_blocks(&response.reply_blocks)).expect("blocks serialize");
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
    if let Some((last, _)) = map.iter().next_back() {
        if last.as_str() >= key.as_str() {
            return Err("snapshot keys not strictly ascending".into());
        }
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

type Committed = (BTreeMap<String, TurnPolicy>, BTreeMap<String, PendingState>);

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
        for optional in [&tasks, &jobs] {
            if let Some(module) = optional {
                ids.insert(module.clone());
                expected += 1;
            }
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
            watches: BTreeMap::new(),
            pending: BTreeMap::new(),
            pending_watches: BTreeMap::new(),
            pending_overlay: BTreeMap::new(),
        }
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

    /// resolve the agent's registered prompt at compose time: query the
    /// source module the ref names, require inline content, and verify it
    /// hashes to the registered pin. `None` (no registered prompt) keeps the
    /// generic default. any failure is a deterministic compose failure — the
    /// run fails or skips (breadcrumb, no dispatch), never the block (the ADR
    /// rule: content that no longer hashes to the pin never reaches a model).
    async fn resolve_prompt(
        &self,
        ctx: &dyn Ctx,
        agent: &AgentRecord,
    ) -> Result<Option<String>, String> {
        let Some(prompt) = &agent.prompt else {
            return Ok(None);
        };
        if prompt.renderer != RENDERER_MEMORY_GENERATION {
            // registration validates the renderer set, so this is belt and
            // braces against a set that shrank across a flag day.
            return Err(format!("unknown prompt renderer: {}", prompt.renderer));
        }
        let (path, generation) = prompt
            .target
            .rsplit_once('@')
            .ok_or_else(|| format!("malformed prompt target: {}", prompt.target))?;
        let generation: u64 = generation
            .parse()
            .map_err(|_| format!("malformed prompt target: {}", prompt.target))?;
        let reply = ctx
            .query(
                &prompt.module,
                &memory_encode_query(&MemoryQuery::Read {
                    path: path.to_string(),
                    generation: Some(generation),
                    snapshot: None,
                }),
            )
            .await
            .map_err(|e| format!("prompt source query failed: {e}"))?;
        let resolved = match memory_decode_reply(&reply) {
            Ok(MemoryReply::Read(Some(resolved))) => resolved,
            Ok(MemoryReply::Read(None)) => {
                return Err(format!("prompt source is missing: {}", prompt.target));
            }
            _ => return Err("unexpected memory reply for a prompt read".into()),
        };
        let Body::Inline(text) = resolved.body else {
            return Err(format!("prompt source is not inline: {}", prompt.target));
        };
        if Sha256::digest(text.as_bytes()).as_slice() != prompt.sha256.as_slice() {
            return Err(format!(
                "prompt content does not hash to the registered pin: {}",
                prompt.target
            ));
        }
        Ok(Some(text))
    }

    /// everything a chat run's dispatch needs, prepared read-only: the pinned
    /// context (P4), the pin-verified prompt, and the fully composed payload.
    /// any failure here is a clean skip for the no-fail engagement intake and
    /// a clean error for an explicit `RequestRun`.
    async fn prepare_dispatch(
        &self,
        ctx: &dyn Ctx,
        agent: &AgentRecord,
        channel_id: &str,
        anchor_seq: u64,
    ) -> Result<PreparedDispatch, String> {
        let prompt = self.resolve_prompt(ctx, agent).await?;
        let (thread_root, transcript) = self.pin_context(ctx, channel_id, anchor_seq).await?;
        let payload =
            render_payload(&self.id, &agent.agent_id, prompt.as_deref(), &transcript).into_bytes();
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
            AgentEvent::Tombstoned { agent_id } => {
                // retire the agent's dispatch recipe atomically with the
                // registry tombstone: no recipe, no new dispatches — ever.
                // in-flight dispatches finish against the manifest values
                // captured at dispatch time (the dispatch module's rule).
                ctx.emit_msg(Msg {
                    target: self.dispatch.clone(),
                    payload: dispatch_encode_msg(&DispatchMsg::RemoveRecipe {
                        recipe_id: recipe_id_for(&agent_id),
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
        let agent = match self.active_agent(&*ctx, agent_id).await {
            Ok(Some(agent)) => agent,
            // an unknown, paused, or tombstoned agent leaves the job on the
            // board.
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
        // (an oversized spec, a prompt that no longer hashes to its pin) is
        // left unclaimed on the board, not claimed into a run that could
        // never execute.
        let prompt = match self.resolve_prompt(&*ctx, &agent).await {
            Ok(prompt) => prompt,
            Err(reason) => {
                self.note(ctx, format!("job run skipped for {run_id}: {reason}"));
                return Ok(());
            }
        };
        let payload = render_job_payload(&job_id, &spec, prompt.as_deref()).into_bytes();
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
                self.note(
                    ctx,
                    format!("engagement skipped for {channel_id}: {reason}"),
                );
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
            match self.prepare_dispatch(&*ctx, &agent, &channel_id, seq).await {
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
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes).into_owned();
                let response = agent_response_from_text(&text, entry.job_id.is_some());
                match self
                    .validate_response(&*ctx, &run_id, &entry, response)
                    .await
                {
                    Ok(response) => {
                        let payload = String::from_utf8(encode_response(&response))
                            .expect("AgentResponse JSON is utf-8");
                        self.emit_response(ctx, &run_id, &entry, response);
                        self.emit_job_finalize_if_current_claimant(ctx, &entry, true, payload)
                            .await;
                    }
                    // deterministically invalid response: the run fails —
                    // breadcrumb, job finalize, pruned entry — the delivery
                    // block commits.
                    Err(reason) => {
                        self.note(ctx, format!("run {run_id} failed: {reason}"));
                        self.emit_job_finalize_if_current_claimant(ctx, &entry, false, reason)
                            .await;
                    }
                }
            }
            Err(reason) => {
                self.note(ctx, format!("run {run_id} failed: {reason}"));
                self.emit_job_finalize_if_current_claimant(ctx, &entry, false, reason)
                    .await;
            }
        }
        Ok(())
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
                if let TurnPolicy::Assigned(assignee) = &policy {
                    if self
                        .agent_record(&*ctx, assignee)
                        .await
                        .map_err(Error::Module)?
                        .is_none()
                    {
                        return Err(Error::Module(format!(
                            "assigned agent is not registered: {assignee}"
                        )));
                    }
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
                match agent.status {
                    AgentStatus::Active => {}
                    AgentStatus::Paused => {
                        return Err(Error::Module(format!("agent is paused: {agent_id}")));
                    }
                    AgentStatus::Tombstoned => {
                        return Err(Error::Module(format!("agent is tombstoned: {agent_id}")));
                    }
                }
                // unlike the engagement intake, an explicit request REJECTS
                // on a failed preparation: this is the root op of its own
                // block, so an error poisons nothing but the request itself.
                let prepared = self
                    .prepare_dispatch(&*ctx, &agent, &channel_id, anchor_seq)
                    .await
                    .map_err(Error::Module)?;
                self.stage_dispatch_run(
                    ctx, &run_id, agent_id, channel_id, anchor_seq, requester, prepared,
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
    use crate::{decode_reply as runs_decode_reply, encode_msg, encode_query};
    use agent::{
        ACTION_TASKS_CREATE, ACTION_TASKS_UPDATE_STATUS, PromptRef,
        encode_event as agent_encode_event, encode_reply as agent_encode_reply,
    };
    use chat::{MessageHead, decode_msg as chat_decode_msg};
    use dispatch::{
        DispatchStatus, DispatchView, decode_msg as dispatch_decode_msg,
        encode_reply as dispatch_encode_reply, encode_result_event,
    };
    use futures::executor::block_on;
    use jobs::{
        Claim as JobClaim, Job, encode_event as jobs_encode_event,
        encode_reply as jobs_encode_reply,
    };
    use sdk::{Effect, Env};
    use tagging::{Author, encode_event as tagging_encode_event};
    use tasks::{Task, decode_msg as tasks_decode_msg, encode_reply as tasks_encode_reply};

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
        /// path -> one canned generation served by the "memory" arm (the
        /// prompt-resolution source).
        memory_files: BTreeMap<String, memory::Generation>,
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
                memory_files: BTreeMap::new(),
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
        fn from_origin(mut self, origin: Origin) -> Self {
            self.env.origin = origin;
            self
        }
        fn from_tagging(self) -> Self {
            self.from_origin(Origin::Module("tagging".into()))
        }
        fn from_dispatch(self) -> Self {
            self.from_origin(Origin::Module("dispatch".into()))
        }
        fn from_jobs(self) -> Self {
            self.from_origin(Origin::Module("jobs".into()))
        }
        fn from_agent(self) -> Self {
            self.from_origin(Origin::Module("agent".into()))
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
        /// a canned memory file: one inline generation at `generation`.
        fn with_memory_file(mut self, path: &str, generation: u64, text: &str) -> Self {
            self.memory_files.insert(
                path.into(),
                memory::Generation {
                    generation,
                    body: Body::Inline(text.into()),
                    meta: BTreeMap::new(),
                    author: "ext:09".into(),
                    published_at_height: 1,
                },
            );
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
                        let view =
                            self.taken_dispatches
                                .contains(&dispatch_id)
                                .then(|| DispatchView {
                                    dispatch_id,
                                    recipe_id: "agent/x".into(),
                                    receiver: "runs".into(),
                                    status: DispatchStatus::Delivered,
                                    outcome: Some(Ok(Vec::new())),
                                    assignee: None,
                                    created_at: 0,
                                    updated_at: 0,
                                });
                        Ok(dispatch_encode_reply(&DispatchReply::Dispatch(view)))
                    }
                    _ => Err(Error::QueryUnsupported),
                },
                "memory" => match memory::decode_query(req).map_err(Error::Module)? {
                    MemoryQuery::Read {
                        path,
                        generation,
                        snapshot: None,
                    } => {
                        let hit = self
                            .memory_files
                            .get(&path)
                            .filter(|g| generation.is_none_or(|n| n == g.generation))
                            .cloned();
                        Ok(memory::encode_reply(&MemoryReply::Read(hit)))
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
            prompt: None,
            allowed_actions: actions.iter().map(|s| s.to_string()).collect(),
            status: AgentStatus::Active,
            created_at: 0,
            updated_at: 0,
        }
    }

    /// the sha256 pin over `text` — what a registered PromptRef commits to.
    fn pin(text: &str) -> Vec<u8> {
        Sha256::digest(text.as_bytes()).to_vec()
    }

    /// pin `agent_id`'s prompt to `<path>@<generation>` under the
    /// memory-generation renderer.
    fn pin_prompt(
        registry: &mut Registry,
        agent_id: &str,
        path: &str,
        generation: u64,
        sha256: Vec<u8>,
    ) {
        registry.get_mut(agent_id).expect("registered").prompt = Some(PromptRef {
            module: "memory".into(),
            target: format!("{path}@{generation}"),
            renderer: RENDERER_MEMORY_GENERATION.into(),
            sha256,
        });
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
            .from_origin(user(9))
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
            .from_tagging()
            .with_registry(registry)
            .with_transcript("general", transcript(seq));
        let tags = mentioned.iter().map(|a| agent_tag(a)).collect();
        exec(m, &mut ctx, &engagement("general", seq, tags)).unwrap();
        ctx
    }

    fn response(reply: &[&str], actions: Vec<AgentAction>) -> Vec<u8> {
        encode_response(&AgentResponse {
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

    // ---- the registry hook ------------------------------------------------------

    #[test]
    fn a_registered_agent_event_registers_the_dispatch_recipe() {
        let mut m = module();
        let mut ctx = CaptureCtx::new().from_agent();
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
        let mut ctx = CaptureCtx::new().from_agent();
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
    fn a_tombstoned_agent_event_removes_the_dispatch_recipe() {
        let mut m = module();
        let mut ctx = CaptureCtx::new().from_agent();
        exec(
            &mut m,
            &mut ctx,
            &agent_event(&AgentEvent::Tombstoned {
                agent_id: "bot".into(),
            }),
        )
        .unwrap();
        assert_eq!(
            ctx.dispatch_msgs(),
            vec![DispatchMsg::RemoveRecipe {
                recipe_id: recipe_id_for("bot"),
            }],
            "the recipe teardown rides the tombstone block"
        );
    }

    #[test]
    fn the_registry_hook_may_error_to_abort_the_registration_block() {
        let mut m = module();

        // an agent id whose recipe id would blow the dispatch id cap: the
        // hook ERRORS, aborting the registration block — the atomic recipe
        // seam (the registry record must never land without its recipe).
        let oversized = "x".repeat(dispatch::MAX_ID_BYTES);
        let mut ctx = CaptureCtx::new().from_agent();
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
        let mut ctx = CaptureCtx::new().from_agent();
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
            .from_origin(user(9))
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
            .from_origin(user(9))
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
        let mut ctx = CaptureCtx::new().from_origin(user(9));
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
        let mut ctx = CaptureCtx::new().from_origin(user(9));
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

        let mut intruder = CaptureCtx::new().from_origin(Origin::System);
        let err = exec(
            &mut m,
            &mut intruder,
            &admin(&RunsMsg::EnableJobWorker { enabled: true }),
        )
        .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        abort(&mut m);

        let mut ctx = CaptureCtx::new().from_origin(user(9));
        exec(
            &mut m,
            &mut ctx,
            &admin(&RunsMsg::EnableJobWorker { enabled: true }),
        )
        .unwrap();
        assert_eq!(ctx.job_msgs(), vec![JobsMsg::RegisterWorker {}]);
        commit(&mut m);

        let mut ctx = CaptureCtx::new().from_origin(user(9));
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
        let mut ctx = CaptureCtx::new().from_origin(user(9));
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
            .from_tagging()
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
        // fully composed payload — prompt framing, contract, transcript.
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
        let text = String::from_utf8(payload.clone()).unwrap();
        assert!(
            text.contains("Return ONLY a JSON object"),
            "the strict output contract rides the payload"
        );
        assert!(
            text.contains("msg 3"),
            "the pinned transcript rides the payload verbatim"
        );
        assert!(
            text.starts_with("You are a Ducktape agent."),
            "the generic instructions lead the deterministic payload"
        );
    }

    // ---- prompt resolution (P4: the ENTIRE model input is consensus data) -----

    /// the single dispatched payload this ctx captured, as text.
    fn dispatched_payload(ctx: &CaptureCtx) -> String {
        let dispatches = ctx.dispatch_msgs();
        assert_eq!(dispatches.len(), 1, "exactly one dispatch expected");
        let DispatchMsg::Dispatch { payload, .. } = &dispatches[0] else {
            panic!("expected a dispatch");
        };
        String::from_utf8(payload.clone()).unwrap()
    }

    #[test]
    fn a_registered_prompt_is_resolved_pin_verified_and_prepended() {
        let text = "You are QUACKBOT. Be terse, be kind, cite your sources.";
        let mut registry = registry(&[("bot1", &[ACTION_CHAT_POST])]);
        pin_prompt(&mut registry, "bot1", "/agents/prompts/bot1", 2, pin(text));
        let mut m = watched(TurnPolicy::Mention, &registry);

        let mut ctx = CaptureCtx::new()
            .at(3)
            .from_tagging()
            .with_registry(&registry)
            .with_transcript("general", transcript(3))
            .with_memory_file("/agents/prompts/bot1", 2, text);
        exec(
            &mut m,
            &mut ctx,
            &engagement("general", 3, vec![agent_tag("bot1")]),
        )
        .unwrap();
        commit(&mut m);

        let payload = dispatched_payload(&ctx);
        assert!(
            payload.starts_with(text),
            "the resolved prompt leads the payload"
        );
        assert!(
            payload.contains("You are a Ducktape agent."),
            "the generic instructions still follow the resolved prompt"
        );
        assert!(
            payload.find(text).unwrap() < payload.find("You are a Ducktape agent.").unwrap(),
            "prompt first, default instructions after"
        );
        assert!(
            get_pending(&m, &run_id_for("general", 3, "bot1")).is_some(),
            "the run dispatched normally"
        );
    }

    #[test]
    fn a_prompt_pin_mismatch_or_missing_source_skips_the_run_never_the_block() {
        let text = "You are QUACKBOT.";
        let mut registry = registry(&[("bot1", &[ACTION_CHAT_POST])]);
        // the pin commits to DIFFERENT content than what memory now serves.
        pin_prompt(
            &mut registry,
            "bot1",
            "/agents/prompts/bot1",
            2,
            pin("something else entirely"),
        );
        let mut m = watched(TurnPolicy::Mention, &registry);

        // pin mismatch: the engagement block COMMITS (no-fail), the run is
        // skipped with a breadcrumb, nothing is staged or dispatched.
        let mut ctx = CaptureCtx::new()
            .at(3)
            .from_tagging()
            .with_registry(&registry)
            .with_transcript("general", transcript(3))
            .with_memory_file("/agents/prompts/bot1", 2, text);
        exec(
            &mut m,
            &mut ctx,
            &engagement("general", 3, vec![agent_tag("bot1")]),
        )
        .unwrap();
        commit(&mut m);
        assert!(ctx.dispatch_msgs().is_empty(), "no dispatch on a bad pin");
        assert_eq!(get_pending(&m, &run_id_for("general", 3, "bot1")), None);
        assert!(
            ctx.events.iter().any(|e| {
                String::from_utf8_lossy(&e.payload).contains("does not hash to the registered pin")
            }),
            "the failed run leaves a breadcrumb"
        );

        // a missing generation skips the same way.
        let mut ctx = CaptureCtx::new()
            .at(4)
            .from_tagging()
            .with_registry(&registry)
            .with_transcript("general", transcript(4));
        exec(
            &mut m,
            &mut ctx,
            &engagement("general", 4, vec![agent_tag("bot1")]),
        )
        .unwrap();
        commit(&mut m);
        assert!(ctx.dispatch_msgs().is_empty());
        assert_eq!(get_pending(&m, &run_id_for("general", 4, "bot1")), None);

        // an explicit RequestRun is the root op of its own block, so the
        // same compose failure REJECTS instead of skipping.
        let mut ctx = CaptureCtx::new()
            .at(5)
            .from_origin(user(9))
            .with_registry(&registry)
            .with_transcript("general", transcript(5))
            .with_memory_file("/agents/prompts/bot1", 2, text);
        let err = exec(
            &mut m,
            &mut ctx,
            &admin(&RunsMsg::RequestRun {
                agent_id: "bot1".into(),
                channel_id: "general".into(),
                anchor_seq: 5,
            }),
        )
        .unwrap_err();
        assert!(
            matches!(err, Error::Module(reason) if reason.contains("pin")),
            "an explicit request surfaces the pin mismatch"
        );
        abort(&mut m);
    }

    #[test]
    fn a_job_run_prepends_the_registered_prompt_and_skips_on_a_bad_pin() {
        let text = "You are DUCK, the job worker.";
        let mut registry = registry(&[("duck", &[ACTION_TASKS_CREATE])]);
        pin_prompt(&mut registry, "duck", "/agents/prompts/duck", 1, pin(text));
        let mut m = module();

        // the pin matches: the claim + dispatch cascade fires and the prompt
        // leads the composed job payload.
        let mut ctx = CaptureCtx::new()
            .at(3)
            .from_jobs()
            .with_registry(&registry)
            .with_memory_file("/agents/prompts/duck", 1, text);
        exec(&mut m, &mut ctx, &jobs_event("job-1", "agent/duck", "spec")).unwrap();
        commit(&mut m);
        assert_eq!(ctx.job_msgs().len(), 1, "the job was claimed");
        let payload = dispatched_payload(&ctx);
        assert!(payload.starts_with(text));
        assert!(payload.contains("Job job-1"));

        // the pin no longer matches: the job is left unclaimed on the board
        // (compose-before-claim), with a breadcrumb, and the block commits.
        let mut ctx = CaptureCtx::new()
            .at(4)
            .from_jobs()
            .with_registry(&registry)
            .with_memory_file("/agents/prompts/duck", 1, "rewritten content");
        exec(&mut m, &mut ctx, &jobs_event("job-2", "agent/duck", "spec")).unwrap();
        commit(&mut m);
        assert!(ctx.job_msgs().is_empty(), "no claim on a bad pin");
        assert!(ctx.dispatch_msgs().is_empty(), "no dispatch on a bad pin");
        assert!(
            ctx.events.iter().any(|e| {
                String::from_utf8_lossy(&e.payload).contains("does not hash to the registered pin")
            }),
            "the skipped job run leaves a breadcrumb"
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
        let mut ctx = CaptureCtx::new()
            .at(2)
            .from_tagging()
            .with_registry(&registry);
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
        let mut ctx = CaptureCtx::new().from_origin(Origin::Module("chat".into()));
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
        let mut ctx = CaptureCtx::new().from_origin(Origin::Module("saga".into()));
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
            .from_tagging()
            .with_registry(&registry)
            .with_transcript("random", transcript(2));
        exec(&mut m, &mut ctx, &engagement("random", 2, vec![])).unwrap();
        assert!(ctx.msgs.is_empty());

        // a failing context pin (the ctx serves NO transcript at all — the
        // chat query errors) must not poison the posting block: Ok, no run.
        let mut ctx = CaptureCtx::new()
            .at(2)
            .from_tagging()
            .with_registry(&registry);
        exec(&mut m, &mut ctx, &engagement("general", 2, vec![])).unwrap();
        assert!(
            ctx.dispatch_msgs().is_empty(),
            "no dispatch on a failed pin"
        );
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
            .from_origin(user(5))
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
            .from_tagging()
            .with_registry(&registry)
            .with_transcript("general", transcript(2))
            .with_taken_dispatch(&taken);
        exec(&mut m, &mut ctx, &engagement("general", 2, vec![])).unwrap();
        assert!(ctx.msgs.is_empty(), "a taken turn re-fires nothing");

        let mut ctx = CaptureCtx::new()
            .at(9)
            .from_origin(user(5))
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

        let mut ctx = CaptureCtx::new()
            .from_origin(user(9))
            .with_registry(&registry);
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
            .from_origin(user(1))
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

        let mut ctx = CaptureCtx::new().from_jobs().with_registry(&registry);
        exec(
            &mut m,
            &mut ctx,
            &jobs_event("bad\u{1f}job", "agent/bot", "spec"),
        )
        .expect("separator in a no-fail jobs event is a no-op");
        assert!(ctx.msgs.is_empty(), "no claim emitted for a bad job id");

        // a spec that does not hash to spec_hash is dropped the same way.
        let mut ctx = CaptureCtx::new().from_jobs().with_registry(&registry);
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
        let mut ctx = CaptureCtx::new().from_tagging();
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
        let mut ctx = CaptureCtx::new().from_dispatch();
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
        let mut ctx = CaptureCtx::new().from_jobs();
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
        let mut ctx = CaptureCtx::new().from_dispatch();
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
            .from_origin(user(1))
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
        let mut ctx = CaptureCtx::new().from_origin(user(1));
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
            .from_dispatch()
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
            .from_tagging()
            .with_registry(&registry)
            .with_transcript("general", thread_transcript.clone());
        exec(&mut m, &mut ctx, &engagement("general", 3, vec![])).unwrap();
        commit(&mut m);
        let run_id = run_id_for("general", 3, "bot");
        assert_eq!(get_pending(&m, &run_id).unwrap().thread_root, Some(1));

        let mut ctx = CaptureCtx::new()
            .at(9)
            .from_dispatch()
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
    fn invalid_responses_fail_the_run_and_emit_no_follow_ups() {
        // normalization already absorbed shape problems (prose, fences,
        // oversize); what remains failable is POLICY: task validity and
        // grants. every case still emits NOTHING, leaves a breadcrumb, and
        // prunes the entry — never the block.
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
                .from_dispatch()
                .with_registry(&registry)
                .with_transcript("general", transcript(2))
                .with_task("t0");
            exec(&mut m, &mut ctx, &result_event(&run_id, Ok(bytes))).unwrap();
            assert!(
                ctx.msgs.is_empty(),
                "an invalid response must emit NOTHING ({fragment})"
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
                .from_dispatch()
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
            .from_dispatch()
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
    fn responses_beyond_the_agents_grants_fail_the_run() {
        // an agent granted ONLY chat.post must not create tasks...
        let (mut m, registry, run_id) = awaiting_run(&[ACTION_CHAT_POST]);
        let mut ctx = CaptureCtx::new()
            .from_dispatch()
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
        assert!(ctx.msgs.is_empty(), "a disallowed action emits NOTHING");
        commit(&mut m);
        assert_eq!(get_pending(&m, &run_id), None);

        // ...and an agent granted only tasks.create must not post replies.
        let (mut m, registry, run_id) = awaiting_run(&[ACTION_TASKS_CREATE]);
        let mut ctx = CaptureCtx::new()
            .from_dispatch()
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
            "runs", "chat", "saga", "tagging", "dispatch", "agent", None, None,
        );
        let mut ctx = CaptureCtx::new()
            .from_origin(user(9))
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
            .from_dispatch()
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
        assert!(ctx.msgs.is_empty());
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
            .from_dispatch()
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
            .from_tagging()
            .with_registry(&registry)
            .with_transcript("general", full.clone());
        exec(&mut m, &mut ctx, &engagement("general", 2, vec![])).unwrap();
        commit(&mut m);
        let run_id = run_id_for("general", 2, "bot");

        let mut ctx = CaptureCtx::new()
            .from_dispatch()
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
    fn failed_dispatch_outcomes_prune_the_entry_without_follow_ups() {
        let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
        let mut m = watched(TurnPolicy::All, &registry);
        for seq in [2, 3] {
            engage_post(&mut m, &registry, seq, &[]);
        }
        commit(&mut m);

        // the dispatch plane already folded saga failures, timeouts, and
        // contract violations into the Err lane — one shape lands here.
        for (seq, reason) in [(2u64, "worker exploded"), (3, "timed out")] {
            let run_id = run_id_for("general", seq, "bot");
            let mut ctx = CaptureCtx::new()
                .at(20)
                .from_dispatch()
                .with_registry(&registry);
            exec(&mut m, &mut ctx, &result_event(&run_id, Err(reason.into()))).unwrap();
            assert!(ctx.msgs.is_empty(), "terminal failures emit nothing");
            commit(&mut m);
            assert_eq!(get_pending(&m, &run_id), None, "the entry pruned");
        }
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
        let mut ctx = CaptureCtx::new().at(3).from_jobs().with_registry(&registry);
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
        let text = String::from_utf8(payload.clone()).unwrap();
        assert!(
            text.contains("summarize this work item"),
            "the FULL job spec rides the payload"
        );
        assert!(text.contains("Return ONLY a JSON object"));
        assert!(text.contains("actions only"), "job framing rides along");

        let entry = get_pending(&m, &run_id).expect("job entry staged");
        assert_eq!(entry.job_id, Some("job-1".into()));
        assert_eq!(entry.job_claim_height, 3);
        assert_eq!(entry.agent_id, "duck");
        assert_eq!(entry.requester, SagaOrigin::Module("jobs".into()));
    }

    #[test]
    fn unknown_paused_or_foreign_kind_jobs_are_left_unclaimed() {
        let mut registry = job_registry();
        let mut m = module();
        let root = m.root();

        // an unregistered agent kind: no claim, no dispatch, no entry.
        let mut ctx = CaptureCtx::new().at(2).from_jobs().with_registry(&registry);
        exec(&mut m, &mut ctx, &jobs_event("j", "agent/ghost", "s")).unwrap();
        assert!(ctx.msgs.is_empty());

        // a non-agent kind is somebody else's job.
        let mut ctx = CaptureCtx::new().at(2).from_jobs().with_registry(&registry);
        exec(&mut m, &mut ctx, &jobs_event("j", "render/video", "s")).unwrap();
        assert!(ctx.msgs.is_empty());

        // a paused agent never claims.
        pause(&mut registry, "duck");
        let mut ctx = CaptureCtx::new().at(2).from_jobs().with_registry(&registry);
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
        let mut ctx = CaptureCtx::new().at(3).from_jobs().with_registry(&registry);
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
            .from_dispatch()
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
        assert_eq!(
            payload.as_bytes(),
            bytes.as_slice(),
            "the normalized response JSON is the finalize payload"
        );
        assert!(ctx.chat_msgs().is_empty(), "job runs never post to chat");
    }

    #[test]
    fn a_failed_job_result_finalizes_with_error_detail() {
        let registry = job_registry();
        let mut m = module();
        let mut ctx = CaptureCtx::new().at(3).from_jobs().with_registry(&registry);
        exec(&mut m, &mut ctx, &jobs_event("job-1", "agent/duck", "spec")).unwrap();
        commit(&mut m);
        let run_id = job_run_id_for("job-1", "duck", 3);

        let mut ctx = CaptureCtx::new()
            .at(10)
            .from_dispatch()
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
        let mut ctx = CaptureCtx::new().at(3).from_jobs().with_registry(&registry);
        exec(&mut m, &mut ctx, &jobs_event("job-1", "agent/duck", "spec")).unwrap();
        commit(&mut m);
        let run_id = job_run_id_for("job-1", "duck", 3);

        let mut ctx = CaptureCtx::new()
            .at(10)
            .from_dispatch()
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
        let mut ctx = CaptureCtx::new().at(3).from_jobs().with_registry(&registry);
        exec(&mut m, &mut ctx, &jobs_event("job-1", "agent/duck", "spec")).unwrap();
        commit(&mut m);
        let run_id = job_run_id_for("job-1", "duck", 3);

        let mut ctx = CaptureCtx::new()
            .at(2000)
            .from_dispatch()
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
        assert_eq!(
            get_pending(&m, &run_id),
            None,
            "the stale entry still prunes"
        );
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
            .from_origin(user(1))
            .with_registry(&registry)
            .with_transcript("general", transcript(3));
        assert!(exec(&mut m, &mut ctx, &request("ghost", 3)).is_err());
        abort(&mut m);
        let mut ctx = CaptureCtx::new()
            .from_origin(Origin::External(Vec::new()))
            .with_registry(&registry)
            .with_transcript("general", transcript(3));
        assert!(exec(&mut m, &mut ctx, &request("bot", 3)).is_err());
        abort(&mut m);
        let mut ctx = CaptureCtx::new()
            .from_origin(user(1))
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
            .from_origin(user(1))
            .with_registry(&registry)
            .with_transcript("general", transcript(3));
        assert!(exec(&mut m, &mut ctx, &request("bot", 3)).is_err());
        abort(&mut m);

        // resumed, the request lands: entry staged + dispatch emitted,
        // requester recorded as the submitting user.
        registry.get_mut("bot").unwrap().status = AgentStatus::Active;
        let mut ctx = CaptureCtx::new()
            .at(6)
            .from_origin(user(1))
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
            .from_origin(user(1))
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
        let mut ctx = CaptureCtx::new()
            .from_origin(user(2))
            .with_registry(&registry);
        assert!(exec(&mut m, &mut ctx, &cancel).is_err());
        abort(&mut m);
        // an unknown run is an error too.
        let mut ctx = CaptureCtx::new()
            .from_origin(user(1))
            .with_registry(&registry);
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
        let mut ctx = CaptureCtx::new()
            .at(7)
            .from_origin(user(1))
            .with_registry(&registry);
        exec(&mut m, &mut ctx, &cancel).unwrap();
        assert_eq!(
            ctx.dispatch_msgs(),
            vec![DispatchMsg::CancelDispatch {
                dispatch_id: dispatch_id_for(&run_id),
            }]
        );
        commit(&mut m);
        assert!(get_pending(&m, &run_id).is_some(), "still pending delivery");

        // the plane's Err("cancelled") delivery prunes the entry.
        let mut ctx = CaptureCtx::new().from_dispatch().with_registry(&registry);
        exec(
            &mut m,
            &mut ctx,
            &result_event(&run_id, Err("cancelled".into())),
        )
        .unwrap();
        assert!(ctx.msgs.is_empty());
        commit(&mut m);
        assert_eq!(get_pending(&m, &run_id), None);

        // cancelling the now-delivered run is an idempotent no-op (the
        // dispatch record proves it existed); a truly unknown one errors.
        let mut ctx = CaptureCtx::new()
            .from_origin(user(1))
            .with_registry(&registry)
            .with_taken_dispatch(&dispatch_id_for(&run_id));
        exec(&mut m, &mut ctx, &cancel).unwrap();
        assert!(ctx.msgs.is_empty());

        // the OWNER may cancel an engagement-created run (requester = the
        // tagging plane).
        engage_post(&mut m, &registry, 2, &["bot"]);
        commit(&mut m);
        let engaged_run = run_id_for("general", 2, "bot");
        let mut ctx = CaptureCtx::new()
            .from_origin(user(9))
            .with_registry(&registry);
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
                .from_origin(user(9))
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
                .from_tagging()
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
                .from_dispatch()
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
                .from_origin(user(9))
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
        let mut ctx = CaptureCtx::new()
            .from_origin(user(9))
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
        engage_post(&mut m, &registry, 2, &[]);
        let mut ctx = CaptureCtx::new()
            .at(3)
            .from_tagging()
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
