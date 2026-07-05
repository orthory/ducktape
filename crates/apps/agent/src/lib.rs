//! the agent module — the collaboration loop's dispatch-plane consumer.
//!
//! a pure state-machine module (in the app-hash) holding the agent registry,
//! channel watches, and the correlation entries for still-pending dispatches.
//! run LIFECYCLE is deliberately NOT here: a run is a dispatched task, and
//! its status, outcome, and history live in the dispatch module (and its
//! saga) — per-dispatched-task, never agent-owned. what this module keeps per
//! run is exactly what acting on the eventual `ResultEvent` needs (where the
//! reply goes, which job to finalize, who may cancel), pruned when the result
//! delivers.
//!
//! the module implements the platform's ordering-contract promises where they
//! touch agents (docs/agent-collaboration-design.md §2, §3, §5):
//!
//! - **P2 — atomic causal cascades.** a user post, the tagging plane's
//!   engagement delivery, the pending entry, and the dispatch commit in ONE
//!   block; a watch and its plane subscription commit in one block; a
//!   validated response's chat reply and task writes commit in the delivery
//!   block.
//! - **P4 — anchored generation.** the ENTIRE model input is composed in
//!   consensus — transcript window, prompt document, output contract — and
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
//! - `Origin::Module(saga)` → a dead-letter no-op. nothing here rides the
//!   saga directly anymore, but any submitter can point a saga trigger's
//!   `reply_to` at this module — the tombstone keeps that callback from ever
//!   aborting the saga's terminal block (the callback-poison rule);
//! - `Origin::Module(chat)` → a dead-letter no-op (chat never notifies this
//!   module directly; the tombstone keeps a stray follow-up from aborting a
//!   posting block);
//! - anything else → an [`AgentMsg`] (admin ops and explicit runs). an
//!   external submitter shipping intake-shaped bytes lands HERE and fails the
//!   `AgentMsg` decode — it can never fake an intake.
//!
//! ## the NO-FAIL arms (design §4)
//!
//! every privileged intake MUST NEVER return `Err`:
//!
//! - the result intake runs inside the delivery block; an `Err` would abort
//!   it, the committed mailbox would re-inject next block, and every
//!   subsequent block would abort (the permanent-abort loop the dispatch
//!   module documents). malformed events and unknown dispatch ids are staged
//!   no-ops (plus an observability event); a response that fails validation
//!   FAILS THE RUN — a breadcrumb and a pruned entry — never the block.
//!   anything the emitted follow-ups could make chat, tasks, or jobs reject
//!   (a squatted reply message id, a full thread, a duplicate task id, a
//!   stale job claim) is probed deterministically first — an emitted
//!   follow-up must be valid by construction.
//! - the engagement intake runs in the same block as the user's post. an
//!   `Err` would abort the post (and every other subscriber's delivery), so
//!   a malformed event, a failed context pin, a broken prompt document, or
//!   an oversized payload is equally a staged no-op.
//! - the jobs intake runs in the same block as the job submit. the event
//!   carries the full spec (committed-only jobs queries cannot see the
//!   just-staged job), and the documented single claiming-worker cascade
//!   rule is what makes the emitted `Claim` safe for this slice.
//!
//! ## loop prevention
//!
//! lives in the TAGGING PLANE, stated once for every module: only
//! user-authored content fires engagement events, so an agent's reply (or
//! any module-attributed post) never re-engages. this module trusts the
//! plane's rule — the events it receives are user-authored by construction.
//!
//! ## the turn claim
//!
//! chat run ids and job run ids use disjoint `0x1f`-delimited keyspaces, and
//! a run's dispatch id is the hex sha256 of its run id — deterministic on
//! every path that could race to claim a turn. the claim itself is settled in
//! TWO layers, both first-in-consensus-order-wins: the staged pending map
//! settles same-block races, and the dispatch module's permanent dispatch
//! record (probed committed-only) settles everything after — a turn whose
//! dispatch exists is taken forever, even after its pending entry pruned.
//!
//! `root()` folds in every field of the three maps, so any transition moves
//! the app-hash. a joiner rebuilds this module from a peer via
//! [`AgentModule::snapshot`] / [`AgentModule::install`]: the snapshot ships
//! the committed maps in the exact canonical encoding `root()` hashes, and
//! install re-derives the root from the decoded temporaries before adopting
//! them — the consensus-agreed root, not the peer, is the trust anchor.

use std::collections::{BTreeMap, BTreeSet};

use agent_interface::{
    ACTION_CHAT_POST, AgentAction, AgentMsg, AgentQuery, AgentRecord, AgentReply, AgentResponse,
    AgentStatus, KNOWN_ACTIONS, MAX_ACTIONS_PER_RUN, MAX_AGENT_RECORD_BYTES,
    MAX_REPLY_BLOCKS_BYTES, PROMPT_HASH_LEN, PendingRun, ReplyBlock, TurnPolicy, WatchView,
    decode_msg, decode_query, encode_reply, encode_response,
};
use capability_interface::validate_tag;
use chat_interface::{
    AuthorRef, Block, ChatMsg, ChatQuery, ChatReply, MAX_THREAD_REPLIES, MessageView,
    decode_reply as chat_decode_reply, encode_msg as chat_encode_msg,
    encode_query as chat_encode_query,
};
use dispatch_interface::{
    DispatchMsg, DispatchQuery, DispatchReply, MAX_PAYLOAD_BYTES, OutputContract, ResultEvent,
    Routing, decode_reply as dispatch_decode_reply, decode_result_event,
    encode_msg as dispatch_encode_msg, encode_query as dispatch_encode_query,
};
use document_interface::{
    DocQuery, DocReply, decode_reply as doc_decode_reply, encode_query as doc_encode_query,
};
use jobs_interface::{
    JobStatus, JobsEvent, JobsMsg, JobsQuery, JobsReply, decode_event as jobs_decode_event,
    decode_reply as jobs_decode_reply, encode_msg as jobs_encode_msg,
    encode_query as jobs_encode_query,
};
use saga_interface::SagaOrigin;
use sdk::{Ctx, Error, Event, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};
use sha2::{Digest, Sha256};
use tagging_interface::{
    EngagementEvent, EntityRef, TaggingMsg, decode_event as tagging_decode_event,
    encode_msg as tagging_encode_msg,
};
use tasks_interface::{
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

/// jobs-board claims created by the agent worker use a view-denominated lease.
pub const JOB_RUN_LEASE_VIEWS: u64 = 1000;

/// jobs finalization payloads must fit the jobs module's 64 KiB cap.
const JOB_FINALIZE_PAYLOAD_BYTES: usize = 64 * 1024;
/// reserved delimiter separating run-key fields.
const RUN_KEY_SEPARATOR: char = '\u{1f}';

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
/// (module-owned) in the same block as the agent itself.
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
{"reply_blocks":[{"id":"<uuid>","kind":"Paragraph","text":"..."}],"actions":[]}
Allowed reply block kinds are Paragraph, Heading, and Code. Heading is rendered as a paragraph in Ducktape chat. Code may include an optional "lang". Actions are optional and must use only actions allowed by the agent registry. Do not include markdown fences around the JSON."#;

/// the canonical rendering of a prompt document: block texts joined by blank
/// lines, kind-agnostic. `AgentRecord::prompt_hash` pins sha256 of exactly
/// this string, so registrant and validator agree byte-for-byte.
pub fn render_prompt_doc(blocks: &[document_interface::Block]) -> String {
    blocks
        .iter()
        .map(|b| b.text.as_str())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// flatten a chat run into the single payload a non-interactive CLI takes:
/// the system instructions, the strict-output contract, then the conversation.
fn render_payload(
    system: Option<&str>,
    module_id: &str,
    agent_id: &str,
    transcript: &[MessageView],
) -> String {
    let mut out = String::new();
    out.push_str(system.unwrap_or(DEFAULT_PROMPT));
    out.push_str("\n\n");
    out.push_str(STRICT_OUTPUT_INSTRUCTION);
    out.push_str("\n\n");
    if transcript.is_empty() {
        out.push_str("No transcript was embedded for this run. Answer the user helpfully.");
        return out;
    }
    out.push_str("Conversation so far:\n");
    for message in transcript {
        let speaker = match &message.head.author {
            AuthorRef::Agent { module, agent_id: author }
                if module == module_id && author == agent_id =>
            {
                "you"
            }
            _ => "them",
        };
        out.push_str(&format!("[{speaker}] {}\n", render_message(message)));
    }
    out.push_str("\nReply as the agent.");
    out
}

/// flatten a job-backed run the same way: instructions, contract, then the
/// job's coordinates and its FULL submitted spec.
fn render_job_payload(system: Option<&str>, job_id: &str, spec: &str) -> String {
    let mut out = String::new();
    out.push_str(system.unwrap_or(DEFAULT_PROMPT));
    out.push_str("\n\n");
    out.push_str(STRICT_OUTPUT_INSTRUCTION);
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
const REPLY_KIND_PARAGRAPH: &str = "Paragraph";
const REPLY_KIND_HEADING: &str = "Heading";
const REPLY_KIND_CODE: &str = "Code";

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

/// one registered agent. the id is the map key, so it isn't repeated here.
#[derive(Clone, Debug, PartialEq, Eq)]
struct AgentState {
    /// the registration origin — the owner capability for every mutation.
    owner: SagaOrigin,
    display_name: String,
    /// the capability registry tag this agent's runs are dispatched on —
    /// WHAT the run needs; how it executes (binary, flags, model) is host
    /// policy in each provider's spec, invisible to consensus.
    capability: String,
    /// sha256 of the prompt content (exactly [`PROMPT_HASH_LEN`] bytes).
    prompt_hash: Vec<u8>,
    /// the document module doc holding the prompt content; its canonical
    /// rendering must hash to `prompt_hash` (verified at dispatch time).
    prompt_doc: Option<String>,
    /// granted action names from the known vocabulary, deduped and sorted.
    allowed_actions: BTreeSet<String>,
    /// false = paused: the agent never engages new runs.
    active: bool,
    created_at: u64,
    updated_at: u64,
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
    agents: &BTreeMap<String, AgentState>,
    watches: &BTreeMap<String, TurnPolicy>,
    pending: &BTreeMap<String, PendingState>,
) -> Vec<u8> {
    let mut out = Vec::new();

    out.extend_from_slice(&(agents.len() as u64).to_le_bytes());
    for (id, a) in agents {
        put_bytes(&mut out, id.as_bytes());
        put_origin(&mut out, &a.owner);
        put_bytes(&mut out, a.display_name.as_bytes());
        put_bytes(&mut out, a.capability.as_bytes());
        put_bytes(&mut out, &a.prompt_hash);
        put_opt_string(&mut out, &a.prompt_doc);
        out.extend_from_slice(&(a.allowed_actions.len() as u64).to_le_bytes());
        for action in &a.allowed_actions {
            put_bytes(&mut out, action.as_bytes());
        }
        out.push(if a.active { 0 } else { 1 });
        out.extend_from_slice(&a.created_at.to_le_bytes());
        out.extend_from_slice(&a.updated_at.to_le_bytes());
    }

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
    agents: &BTreeMap<String, AgentState>,
    watches: &BTreeMap<String, TurnPolicy>,
    pending: &BTreeMap<String, PendingState>,
) -> StateRoot {
    StateRoot(Sha256::digest(encode_committed(agents, watches, pending)).into())
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

type Committed = (
    BTreeMap<String, AgentState>,
    BTreeMap<String, TurnPolicy>,
    BTreeMap<String, PendingState>,
);

fn decode_committed(mut buf: &[u8]) -> Result<Committed, String> {
    // per-entry minimum sizes: an agent costs its id prefix, one origin
    // discriminant, three length prefixes, a prompt-doc tag, an action
    // count, a status byte, and two u64s; a watch its id prefix and a policy
    // discriminant; a pending entry its three length prefixes, anchor, two
    // option tags, claim height, origin discriminant, and created_at.
    const MIN_AGENT_BYTES: u64 = 8 + 1 + 8 + 8 + 8 + 1 + 8 + 1 + 8 + 8;
    const MIN_WATCH_BYTES: u64 = 8 + 1;
    const MIN_PENDING_BYTES: u64 = 8 + 8 + 8 + 8 + 1 + 1 + 8 + 1 + 8;

    let mut agents: BTreeMap<String, AgentState> = BTreeMap::new();
    let count = take_count(&mut buf, MIN_AGENT_BYTES, "agent")?;
    for _ in 0..count {
        let id = take_lp_string(&mut buf)?;
        if contains_run_separator(&id) {
            return Err("snapshot agent_id contains reserved unit separator".into());
        }
        let owner = take_origin(&mut buf)?;
        let display_name = take_lp_string(&mut buf)?;
        let capability = take_lp_string(&mut buf)?;
        let prompt_hash = take_lp_bytes(&mut buf)?;
        let prompt_doc = take_opt_string(&mut buf)?;
        let mut allowed_actions: BTreeSet<String> = BTreeSet::new();
        let actions = take_count(&mut buf, 8, "action")?;
        for _ in 0..actions {
            let action = take_lp_string(&mut buf)?;
            if let Some(last) = allowed_actions.iter().next_back() {
                if last.as_str() >= action.as_str() {
                    return Err("snapshot actions not strictly ascending".into());
                }
            }
            allowed_actions.insert(action);
        }
        let active = match take(&mut buf, 1)?[0] {
            0 => true,
            1 => false,
            d => return Err(format!("snapshot has unknown agent status {d}")),
        };
        let created_at = take_u64(&mut buf)?;
        let updated_at = take_u64(&mut buf)?;
        insert_ascending(
            &mut agents,
            id,
            AgentState {
                owner,
                display_name,
                capability,
                prompt_hash,
                prompt_doc,
                allowed_actions,
                active,
                created_at,
                updated_at,
            },
        )?;
    }

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
    Ok((agents, watches, pending))
}

// ---- the module -----------------------------------------------------------

pub struct AgentModule {
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
    tasks: Option<ModuleId>,
    jobs: Option<ModuleId>,
    /// the document module prompt content is read from (`prompt_doc`).
    document: Option<ModuleId>,
    /// committed state — what `root()` and the app-hash commit to.
    agents: BTreeMap<String, AgentState>,
    watches: BTreeMap<String, TurnPolicy>,
    /// in-flight correlation entries keyed by dispatch id — pruned on
    /// delivery; the dispatch module owns lifecycle and history.
    pending: BTreeMap<String, PendingState>,
    /// this block's staged writes, read ahead of committed state
    /// (read-your-writes) but merged in — and reflected in `root()` — only at
    /// `commit_block`. agents are upsert-only; a watch stages `None` for
    /// removal (unwatch); a pending entry stages `None` for its prune.
    pending_agents: BTreeMap<String, AgentState>,
    pending_watches: BTreeMap<String, Option<TurnPolicy>>,
    pending_overlay: BTreeMap<String, Option<PendingState>>,
}

impl AgentModule {
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
        tasks: Option<ModuleId>,
        jobs: Option<ModuleId>,
        document: Option<ModuleId>,
    ) -> Self {
        let id = id.into();
        let chat = chat.into();
        let saga = saga.into();
        let tagging = tagging.into();
        let dispatch = dispatch.into();
        let mut ids = BTreeSet::from([
            id.clone(),
            chat.clone(),
            saga.clone(),
            tagging.clone(),
            dispatch.clone(),
        ]);
        let mut expected = 5;
        for optional in [&tasks, &jobs, &document] {
            if let Some(module) = optional {
                ids.insert(module.clone());
                expected += 1;
            }
        }
        assert_eq!(
            ids.len(),
            expected,
            "agent collaborator module ids must be pairwise distinct"
        );
        Self {
            id,
            chat,
            saga,
            tagging,
            dispatch,
            tasks,
            jobs,
            document,
            agents: BTreeMap::new(),
            watches: BTreeMap::new(),
            pending: BTreeMap::new(),
            pending_agents: BTreeMap::new(),
            pending_watches: BTreeMap::new(),
            pending_overlay: BTreeMap::new(),
        }
    }

    // ---- staged-over-committed reads ---------------------------------------

    fn agent(&self, agent_id: &str) -> Option<&AgentState> {
        self.pending_agents
            .get(agent_id)
            .or_else(|| self.agents.get(agent_id))
    }

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

    /// every ACTIVE registered agent visible this dispatch, sorted — the
    /// deterministic engagement domain for `All` and `RoundRobin`.
    fn active_agent_ids(&self) -> Vec<String> {
        Self::visible_ids(&self.agents, &self.pending_agents)
            .into_iter()
            .filter(|id| self.agent(id).is_some_and(|a| a.active))
            .collect()
    }

    // ---- views ---------------------------------------------------------------

    fn agent_view(agent_id: &str, a: &AgentState) -> AgentRecord {
        AgentRecord {
            agent_id: agent_id.to_string(),
            owner: a.owner.clone(),
            display_name: a.display_name.clone(),
            capability: a.capability.clone(),
            prompt_hash: a.prompt_hash.clone(),
            prompt_doc: a.prompt_doc.clone(),
            allowed_actions: a.allowed_actions.iter().cloned().collect(),
            status: if a.active {
                AgentStatus::Active
            } else {
                AgentStatus::Paused
            },
            created_at: a.created_at,
            updated_at: a.updated_at,
        }
    }

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

    fn validate_prompt_hash(prompt_hash: &[u8]) -> Result<(), Error> {
        if prompt_hash.len() != PROMPT_HASH_LEN {
            return Err(Error::Module(format!(
                "prompt_hash must be exactly {PROMPT_HASH_LEN} bytes, got {}",
                prompt_hash.len()
            )));
        }
        Ok(())
    }

    /// every granted action must come from the known vocabulary, so a grant
    /// always means something; duplicates collapse into the set.
    fn validate_actions(actions: Vec<String>) -> Result<BTreeSet<String>, Error> {
        let mut set = BTreeSet::new();
        for action in actions {
            if !KNOWN_ACTIONS.contains(&action.as_str()) {
                return Err(Error::Module(format!("unknown action: {action}")));
            }
            set.insert(action);
        }
        Ok(set)
    }

    /// registry entries live in the root preimage and every snapshot —
    /// size-gate them up front (the write-time-caps lesson).
    fn validate_record_size(agent_id: &str, state: &AgentState) -> Result<(), Error> {
        let bytes =
            serde_json::to_vec(&Self::agent_view(agent_id, state)).expect("record is serializable");
        if bytes.len() > MAX_AGENT_RECORD_BYTES {
            return Err(Error::Module(format!(
                "agent record too large: {} > {MAX_AGENT_RECORD_BYTES} bytes",
                bytes.len()
            )));
        }
        Ok(())
    }

    /// the owner capability: registration takes a non-empty external key or a
    /// module as the owner. the pre-consensus empty external default and the
    /// system origin (which any genesis path could wear) never own agents.
    fn admin_origin(origin: &Origin) -> Result<SagaOrigin, Error> {
        match origin {
            Origin::External(key) if key.is_empty() => Err(Error::Module(
                "agent admin ops require a non-empty submitter id".into(),
            )),
            Origin::System => Err(Error::Module(
                "agent admin ops require an external or module origin".into(),
            )),
            other => Ok(canonical_origin(other)),
        }
    }

    fn owned_agent(&self, ctx: &dyn Ctx, agent_id: &str) -> Result<&AgentState, Error> {
        let agent = self
            .agent(agent_id)
            .ok_or_else(|| Error::Module(format!("unknown agent: {agent_id}")))?;
        if agent.owner != canonical_origin(&ctx.env().origin) {
            return Err(Error::Module(format!(
                "only the owner may modify agent {agent_id}"
            )));
        }
        Ok(agent)
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

    // ---- prompt + payload preparation (the dispatch plane's composition rule) -----

    /// the agent's consensus-resident prompt text, when it has one: read from
    /// the document module and verified against the registered
    /// `prompt_hash`, so which prompt ran is part of the app-hash — a
    /// drifted document is an error the caller turns into a skipped run.
    async fn prompt_text(&self, ctx: &dyn Ctx, agent: &AgentState) -> Result<Option<String>, String> {
        let Some(doc_id) = &agent.prompt_doc else {
            return Ok(None);
        };
        let Some(document) = &self.document else {
            return Err("agent has a prompt_doc but no document module is configured".into());
        };
        let reply = ctx
            .query(
                document,
                &doc_encode_query(&DocQuery::GetDoc {
                    doc_id: doc_id.clone(),
                }),
            )
            .await
            .map_err(|e| format!("document query failed: {e}"))?;
        let blocks = match doc_decode_reply(&reply) {
            Ok(DocReply::Doc(Some(blocks))) => blocks,
            Ok(DocReply::Doc(None)) => return Err(format!("prompt document is missing: {doc_id}")),
            _ => return Err("unexpected document reply for a prompt query".into()),
        };
        let text = render_prompt_doc(&blocks);
        if Sha256::digest(text.as_bytes()).as_slice() != agent.prompt_hash.as_slice() {
            return Err(format!(
                "prompt document {doc_id} does not hash to the registered prompt_hash"
            ));
        }
        Ok(Some(text))
    }

    /// everything a chat run's dispatch needs, prepared read-only: the pinned
    /// context (P4) and the fully composed payload. any failure here is a
    /// clean skip for the no-fail engagement intake and a clean error for an
    /// explicit `RequestRun`.
    async fn prepare_dispatch(
        &self,
        ctx: &dyn Ctx,
        agent_id: &str,
        channel_id: &str,
        anchor_seq: u64,
    ) -> Result<PreparedDispatch, String> {
        let agent = self
            .agent(agent_id)
            .ok_or_else(|| format!("agent is not registered: {agent_id}"))?;
        let (thread_root, transcript) = self.pin_context(ctx, channel_id, anchor_seq).await?;
        let prompt = self.prompt_text(ctx, agent).await?;
        let payload =
            render_payload(prompt.as_deref(), &self.id, agent_id, &transcript).into_bytes();
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
    /// with the agent); the result lands as a next-block `ResultEvent` keyed
    /// by the dispatch id, which prunes the entry staged here.
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
        let Some(agent) = self.agent(agent_id) else {
            return Ok(());
        };
        if !agent.active {
            return Ok(());
        }
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
        // (drifted prompt doc, oversized spec) is left unclaimed on the
        // board, not claimed into a run that could never execute.
        let prompt = match self.prompt_text(&*ctx, agent).await {
            Ok(prompt) => prompt,
            Err(reason) => {
                self.note(ctx, format!("job run skipped for {run_id}: {reason}"));
                return Ok(());
            }
        };
        let payload = render_job_payload(prompt.as_deref(), &job_id, &spec).into_bytes();
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
    /// registered agents ever engage; every branch reads agreed state only.
    fn engaged_agents(&self, policy: &TurnPolicy, tags: &[EntityRef], seq: u64) -> Vec<String> {
        match policy {
            // entity tags naming THIS module's agents, in content order (the
            // content module dedupes them).
            TurnPolicy::Mention => tags
                .iter()
                .filter_map(|tag| {
                    (tag.module == self.id
                        && self.agent(&tag.entity).is_some_and(|a| a.active))
                    .then(|| tag.entity.clone())
                })
                .collect(),
            TurnPolicy::All => self.active_agent_ids(),
            TurnPolicy::Assigned(agent_id) => {
                if self.agent(agent_id).is_some_and(|a| a.active) {
                    vec![agent_id.clone()]
                } else {
                    Vec::new()
                }
            }
            TurnPolicy::RoundRobin => {
                let active = self.active_agent_ids();
                if active.is_empty() {
                    Vec::new()
                } else {
                    vec![active[(seq % active.len() as u64) as usize].clone()]
                }
            }
        }
    }

    /// NO-FAIL ARM. the tagging plane routes a user post here in the same
    /// block as the post itself — an `Err` would abort the post (and every
    /// other subscriber's delivery), so malformed events, unwatched
    /// channels, failed context pins, broken prompt documents, and oversized
    /// payloads are all staged no-ops. the plane's loop rule guarantees the
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

        let requester = canonical_origin(&ctx.env().origin);
        for agent_id in self.engaged_agents(&policy, &tags, seq) {
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
            match self
                .prepare_dispatch(&*ctx, &agent_id, &channel_id, seq)
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
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes).into_owned();
                let response = agent_response_from_text(&text, entry.job_id.is_some());
                match self.validate_response(&*ctx, &run_id, &entry, response).await {
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
            .agent(&entry.agent_id)
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
            if !agent.allowed_actions.contains(ACTION_CHAT_POST) {
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
                if !agent.allowed_actions.contains(name) {
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
        let marker = "\n[truncated by agent to fit jobs payload cap]";
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
                format!("job {job_id} finalize skipped: agent is not current claimant"),
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
        let now = ctx.env().consensus_time;
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            AgentMsg::RegisterAgent {
                agent_id,
                display_name,
                capability,
                prompt_hash,
                prompt_doc,
                allowed_actions,
            } => {
                let owner = Self::admin_origin(&ctx.env().origin)?;
                Self::validate_non_empty("agent_id", &agent_id)?;
                reject_run_separator("agent_id", &agent_id)?;
                // the agent's recipe id must fit the dispatch plane's id cap,
                // or the atomic recipe registration below could never land.
                if recipe_id_for(&agent_id).len() > dispatch_interface::MAX_ID_BYTES {
                    return Err(Error::Module(format!(
                        "agent_id is too long for its dispatch recipe id (cap {})",
                        dispatch_interface::MAX_ID_BYTES - recipe_id_for("").len()
                    )));
                }
                Self::validate_non_empty("display_name", &display_name)?;
                validate_tag(&capability).map_err(Error::Module)?;
                Self::validate_prompt_hash(&prompt_hash)?;
                if let Some(doc_id) = &prompt_doc {
                    Self::validate_non_empty("prompt_doc", doc_id)?;
                }
                let allowed_actions = Self::validate_actions(allowed_actions)?;
                if self.agent(&agent_id).is_some() {
                    return Err(Error::Module(format!("agent already exists: {agent_id}")));
                }
                // the agent and its dispatch recipe are ONE atomic unit: if
                // the dispatch module rejects the recipe (a squatted id), the
                // whole block aborts and the staged agent vanishes with it.
                ctx.emit_msg(Msg {
                    target: self.dispatch.clone(),
                    payload: dispatch_encode_msg(&DispatchMsg::RegisterRecipe {
                        recipe_id: recipe_id_for(&agent_id),
                        description: format!("runs for agent {agent_id}"),
                        capability: capability.clone(),
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
                let state = AgentState {
                    owner,
                    display_name,
                    capability,
                    prompt_hash,
                    prompt_doc,
                    allowed_actions,
                    active: true,
                    created_at: now,
                    updated_at: now,
                };
                Self::validate_record_size(&agent_id, &state)?;
                self.pending_agents.insert(agent_id, state);
                Ok(())
            }
            AgentMsg::UpdateAgent {
                agent_id,
                display_name,
                capability,
                prompt_hash,
                prompt_doc,
                allowed_actions,
            } => {
                let mut state = self.owned_agent(&*ctx, &agent_id)?.clone();
                if let Some(display_name) = display_name {
                    Self::validate_non_empty("display_name", &display_name)?;
                    state.display_name = display_name;
                }
                if let Some(capability) = capability {
                    validate_tag(&capability).map_err(Error::Module)?;
                    if capability != state.capability {
                        // keep the agent's dispatch recipe on the same tag,
                        // atomically with the record change.
                        ctx.emit_msg(Msg {
                            target: self.dispatch.clone(),
                            payload: dispatch_encode_msg(&DispatchMsg::UpdateRecipe {
                                recipe_id: recipe_id_for(&agent_id),
                                description: None,
                                capability: Some(capability.clone()),
                                routing: None,
                                output_contract: None,
                                max_attempts: None,
                            }),
                        });
                    }
                    state.capability = capability;
                }
                if let Some(prompt_hash) = prompt_hash {
                    Self::validate_prompt_hash(&prompt_hash)?;
                    state.prompt_hash = prompt_hash;
                }
                if let Some(doc_id) = prompt_doc {
                    Self::validate_non_empty("prompt_doc", &doc_id)?;
                    state.prompt_doc = Some(doc_id);
                }
                if let Some(allowed_actions) = allowed_actions {
                    state.allowed_actions = Self::validate_actions(allowed_actions)?;
                }
                state.updated_at = now;
                Self::validate_record_size(&agent_id, &state)?;
                self.pending_agents.insert(agent_id, state);
                Ok(())
            }
            AgentMsg::PauseAgent { agent_id } => self.stage_active(ctx, agent_id, false, now),
            AgentMsg::ResumeAgent { agent_id } => self.stage_active(ctx, agent_id, true, now),
            AgentMsg::WatchChannel { channel_id, policy } => {
                Self::admin_origin(&ctx.env().origin)?;
                Self::validate_non_empty("channel_id", &channel_id)?;
                reject_run_separator("channel_id", &channel_id)?;
                if let TurnPolicy::Assigned(assignee) = &policy {
                    if self.agent(assignee).is_none() {
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
            AgentMsg::UnwatchChannel { channel_id } => {
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
            AgentMsg::EnableJobWorker { enabled } => {
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
            AgentMsg::RequestRun {
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
                let Some(agent) = self.agent(&agent_id) else {
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
                if !agent.active {
                    return Err(Error::Module(format!("agent is paused: {agent_id}")));
                }
                // unlike the engagement intake, an explicit request REJECTS
                // on a failed preparation: this is the root op of its own
                // block, so an error poisons nothing but the request itself.
                let prepared = self
                    .prepare_dispatch(&*ctx, &agent_id, &channel_id, anchor_seq)
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
            AgentMsg::CancelRun { run_id } => {
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
                let owner = self.agent(&entry.agent_id).map(|a| a.owner.clone());
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

    fn stage_active(
        &mut self,
        ctx: &dyn Ctx,
        agent_id: String,
        active: bool,
        now: u64,
    ) -> Result<(), Error> {
        let state = self.owned_agent(ctx, &agent_id)?;
        if state.active == active {
            // idempotent: staging nothing keeps the root byte-identical.
            return Ok(());
        }
        let mut state = state.clone();
        state.active = active;
        state.updated_at = now;
        self.pending_agents.insert(agent_id, state);
        Ok(())
    }

    // ---- state-sync ---------------------------------------------------------
    // hand a joiner the committed continuation state as canonical bytes; the
    // consensus-agreed root — never the serving peer — decides whether they land.

    /// serialize the COMMITTED continuation state (never the staged overlay)
    /// into the canonical encoding `root()` commits to. deterministic across
    /// nodes.
    pub fn snapshot(&self) -> Vec<u8> {
        encode_committed(&self.agents, &self.watches, &self.pending)
    }

    /// adopt a peer's snapshot as own committed state — but only after the
    /// decoded temporaries re-derive `expected` via the exact `root()`
    /// algorithm, so a byzantine snapshot cannot land under an agreed root it
    /// doesn't match. all-or-nothing: on any Err this module (and its root)
    /// is byte-identical to before the call. on success the staged overlay is
    /// dropped — a snapshot describes a block boundary, and nothing
    /// half-applied may shadow it.
    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        let (agents, watches, pending) = decode_committed(bytes).map_err(Error::Module)?;
        if committed_root(&agents, &watches, &pending) != expected {
            return Err(Error::Module(
                "snapshot does not match expected root".into(),
            ));
        }
        self.agents = agents;
        self.watches = watches;
        self.pending = pending;
        self.pending_agents.clear();
        self.pending_watches.clear();
        self.pending_overlay.clear();
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl Module for AgentModule {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// state-based commitment: sha256 over the canonical committed encoding —
    /// a length-prefixed fold of every agent, watch, and pending-entry field
    /// in sorted-key order. sensitive to every field, so any transition moves
    /// the root. the preimage IS the snapshot encoding.
    fn root(&self) -> StateRoot {
        committed_root(&self.agents, &self.watches, &self.pending)
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(self.snapshot()))
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        // payload namespaces routed by the HOST-ASSIGNED origin — spoof-proof
        // by construction: only the tagging plane's follow-ups reach the
        // engagement intake, only dispatch's reach the result intake, and
        // everything else (external submitters included) must decode as an
        // AgentMsg. saga and chat origins are dead-lettered: neither may ever
        // abort the block that carried them.
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
            Origin::Module(module) if module == self.saga => {
                // dead letter: nothing here rides the saga anymore, but any
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
            AgentQuery::Agents => {
                let agents = Self::visible_ids(&self.agents, &self.pending_agents)
                    .into_iter()
                    .filter_map(|id| self.agent(&id).map(|a| Self::agent_view(&id, a)))
                    .collect();
                Ok(encode_reply(&AgentReply::Agents(agents)))
            }
            AgentQuery::Agent { agent_id } => Ok(encode_reply(&AgentReply::Agent(
                self.agent(&agent_id)
                    .map(|a| Self::agent_view(&agent_id, a)),
            ))),
            AgentQuery::PendingRuns => {
                let runs = Self::visible_ids(&self.pending, &self.pending_overlay)
                    .into_iter()
                    .filter_map(|dispatch_id| {
                        self.pending_entry(&dispatch_id)
                            .map(|p| Self::pending_view(&dispatch_id, p))
                    })
                    .collect();
                Ok(encode_reply(&AgentReply::PendingRuns(runs)))
            }
            AgentQuery::Watches => {
                let watches = Self::visible_ids(&self.watches, &self.pending_watches)
                    .into_iter()
                    .filter_map(|channel_id| {
                        self.watch(&channel_id).map(|policy| WatchView {
                            channel_id: channel_id.clone(),
                            policy: policy.clone(),
                        })
                    })
                    .collect();
                Ok(encode_reply(&AgentReply::Watches(watches)))
            }
        }
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        for (id, agent) in std::mem::take(&mut self.pending_agents) {
            self.agents.insert(id, agent);
        }
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
        self.pending_agents.clear();
        self.pending_watches.clear();
        self.pending_overlay.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_interface::{
        ACTION_TASKS_CREATE, ACTION_TASKS_UPDATE_STATUS, decode_reply, encode_msg, encode_query,
    };
    use chat_interface::{MessageHead, decode_msg as chat_decode_msg};
    use dispatch_interface::{
        DispatchStatus, DispatchView, decode_msg as dispatch_decode_msg,
        encode_reply as dispatch_encode_reply, encode_result_event,
    };
    use document_interface::{BlockKind, decode_query as doc_decode_query, encode_reply as doc_encode_reply};
    use futures::executor::block_on;
    use jobs_interface::{
        Claim as JobClaim, Job, encode_event as jobs_encode_event,
        encode_reply as jobs_encode_reply,
    };
    use sdk::{Effect, Env};
    use tagging_interface::{Author, encode_event as tagging_encode_event};
    use tasks_interface::{
        Task, decode_msg as tasks_decode_msg, encode_reply as tasks_encode_reply,
    };

    /// a minimal `Ctx` that captures emitted msgs/effects/events and serves
    /// canned chat transcripts, task lists, job records, and dispatch
    /// records — enough to unit-test `execute` in isolation (the host
    /// provides the real routing in integration).
    struct CaptureCtx {
        env: Env,
        /// channel -> messages with contiguous seqs starting at 1.
        transcripts: BTreeMap<String, Vec<MessageView>>,
        tasks: Vec<Task>,
        /// doc_id -> prompt document blocks served by the document arm.
        docs: BTreeMap<String, Vec<document_interface::Block>>,
        /// dispatch ids the dispatch module already has a record for — the
        /// committed turn-claim layer the module probes.
        taken_dispatches: BTreeSet<String>,
        /// job_id -> board record served by the jobs arm (finalize guard).
        jobs: BTreeMap<String, Job>,
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
                    me: "agent".into(),
                },
                transcripts: BTreeMap::new(),
                tasks: Vec::new(),
                docs: BTreeMap::new(),
                taken_dispatches: BTreeSet::new(),
                jobs: BTreeMap::new(),
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
        fn with_doc(mut self, doc_id: &str, text: &str) -> Self {
            self.docs.insert(
                doc_id.into(),
                vec![document_interface::Block {
                    id: "b1".into(),
                    kind: BlockKind::Paragraph,
                    text: text.into(),
                }],
            );
            self
        }
        fn with_taken_dispatch(mut self, dispatch_id: &str) -> Self {
            self.taken_dispatches.insert(dispatch_id.into());
            self
        }
        /// a job the board holds as Processing, claimed by "agent" at `height`.
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
                        worker: "agent".into(),
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
                .map(|m| jobs_interface::decode_msg(&m.payload).expect("jobs msg"))
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
                .map(|m| tagging_interface::decode_msg(&m.payload).expect("tagging msg"))
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
                "document" => match doc_decode_query(req).map_err(Error::Module)? {
                    DocQuery::GetDoc { doc_id } => Ok(doc_encode_reply(&DocReply::Doc(
                        self.docs.get(&doc_id).cloned(),
                    ))),
                    _ => Err(Error::QueryUnsupported),
                },
                "chat" => match chat_interface::decode_query(req).map_err(Error::Module)? {
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
                        Ok(chat_interface::encode_reply(&ChatReply::Messages(window)))
                    }
                    ChatQuery::Message { message_id } => {
                        Ok(chat_interface::encode_reply(&ChatReply::Message(
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
                "jobs" => match jobs_interface::decode_query(req).map_err(Error::Module)? {
                    JobsQuery::Get { job_id } => Ok(jobs_encode_reply(&JobsReply::Job(
                        self.jobs.get(&job_id).cloned(),
                    ))),
                    _ => Err(Error::QueryUnsupported),
                },
                "dispatch" => match dispatch_interface::decode_query(req).map_err(Error::Module)? {
                    DispatchQuery::Dispatch { dispatch_id, .. } => {
                        let view = self.taken_dispatches.contains(&dispatch_id).then(|| {
                            DispatchView {
                                dispatch_id,
                                recipe_id: "agent/x".into(),
                                receiver: "agent".into(),
                                status: DispatchStatus::Delivered,
                                outcome: Some(Ok(Vec::new())),
                                created_at: 0,
                                updated_at: 0,
                            }
                        });
                        Ok(dispatch_encode_reply(&DispatchReply::Dispatch(view)))
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

    fn module() -> AgentModule {
        AgentModule::new(
            "agent",
            "chat",
            "saga",
            "tagging",
            "dispatch",
            Some("tasks".into()),
            Some("jobs".into()),
            Some("document".into()),
        )
    }

    fn user(byte: u8) -> Origin {
        Origin::External(vec![byte; 32])
    }

    fn agent_tag(agent_id: &str) -> EntityRef {
        EntityRef {
            module: "agent".into(),
            entity: agent_id.into(),
        }
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

    fn register(agent_id: &str, actions: &[&str]) -> AgentMsg {
        AgentMsg::RegisterAgent {
            agent_id: agent_id.into(),
            display_name: agent_id.to_uppercase(),
            capability: "model-1".into(),
            prompt_hash: vec![7u8; PROMPT_HASH_LEN],
            prompt_doc: None,
            allowed_actions: actions.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn admin(m: &AgentMsg) -> Msg {
        Msg {
            target: "agent".into(),
            payload: encode_msg(m),
        }
    }

    /// the tagging plane's routed report of a user post — the engagement
    /// intake's payload. the plane's loop rule means these are always
    /// user-authored in practice.
    fn engagement(channel: &str, seq: u64, tags: Vec<EntityRef>) -> Msg {
        Msg {
            target: "agent".into(),
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
            target: "agent".into(),
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
            target: "agent".into(),
            payload: jobs_encode_event(&JobsEvent::Submitted {
                job_id: job_id.into(),
                kind: kind.into(),
                submitter: "ext:01".into(),
                spec: spec.into(),
                spec_hash: job_spec_hash(spec.as_bytes()),
            }),
        }
    }

    fn exec(m: &mut AgentModule, ctx: &mut CaptureCtx, op: &Msg) -> Result<(), Error> {
        block_on(m.execute(ctx, op))
    }

    fn commit(m: &mut AgentModule) {
        block_on(m.commit_block()).unwrap();
    }

    fn abort(m: &mut AgentModule) {
        block_on(m.abort_block()).unwrap();
    }

    fn pending_runs(m: &AgentModule) -> Vec<PendingRun> {
        let reply = block_on(m.query(&encode_query(&AgentQuery::PendingRuns))).unwrap();
        match decode_reply(&reply).unwrap() {
            AgentReply::PendingRuns(runs) => runs,
            other => panic!("unexpected reply: {other:?}"),
        }
    }

    fn get_pending(m: &AgentModule, run_id: &str) -> Option<PendingRun> {
        pending_runs(m).into_iter().find(|p| p.run_id == run_id)
    }

    fn get_agent(m: &AgentModule, agent_id: &str) -> Option<AgentRecord> {
        let reply = block_on(m.query(&encode_query(&AgentQuery::Agent {
            agent_id: agent_id.into(),
        })))
        .unwrap();
        match decode_reply(&reply).unwrap() {
            AgentReply::Agent(record) => record,
            other => panic!("unexpected reply: {other:?}"),
        }
    }

    /// a committed module with `agents` registered by user(9) and one watch
    /// on "general" under `policy`.
    fn watched(policy: TurnPolicy, agents: &[(&str, &[&str])]) -> AgentModule {
        let mut m = module();
        let mut ctx = CaptureCtx::new().from_origin(user(9));
        for (agent_id, actions) in agents {
            exec(&mut m, &mut ctx, &admin(&register(agent_id, actions))).unwrap();
        }
        exec(
            &mut m,
            &mut ctx,
            &admin(&AgentMsg::WatchChannel {
                channel_id: "general".into(),
                policy,
            }),
        )
        .unwrap();
        commit(&mut m);
        m
    }

    /// drive an engagement at `seq` (author user(1)) tagging `mentioned`.
    fn engage_post(m: &mut AgentModule, seq: u64, mentioned: &[&str]) -> CaptureCtx {
        let mut ctx = CaptureCtx::new()
            .at(seq)
            .from_tagging()
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
                    kind: "Paragraph".into(),
                    text: (*t).into(),
                    lang: None,
                })
                .collect(),
            actions,
        })
    }

    // ---- registry admin -------------------------------------------------------

    #[test]
    fn register_validates_and_stages_an_active_agent() {
        let mut m = module();
        let mut ctx = CaptureCtx::new().at(3).from_origin(user(9));
        exec(
            &mut m,
            &mut ctx,
            &admin(&register("bot", &[ACTION_CHAT_POST])),
        )
        .unwrap();
        commit(&mut m);

        let record = get_agent(&m, "bot").unwrap();
        assert_eq!(record.owner, SagaOrigin::External(vec![9; 32]));
        assert_eq!(record.status, AgentStatus::Active);
        assert_eq!(record.allowed_actions, vec![ACTION_CHAT_POST.to_string()]);
        assert_eq!(record.created_at, 3);

        // the agent's dispatch recipe registers atomically with the record.
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

        // duplicate registration is an error, even from the same owner.
        let err = exec(
            &mut m,
            &mut ctx,
            &admin(&register("bot", &[ACTION_CHAT_POST])),
        )
        .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        abort(&mut m);
    }

    #[test]
    fn register_rejects_bad_shapes_and_bad_origins() {
        let mut m = module();
        let root0 = m.root();
        let cases: Vec<(Origin, AgentMsg)> = vec![
            // the pre-consensus empty external default never owns an agent.
            (Origin::External(Vec::new()), register("a", &[])),
            // system is not an ownable origin (spec: external or module).
            (Origin::System, register("a", &[])),
            // a prompt hash that is not exactly 32 bytes.
            (
                user(9),
                AgentMsg::RegisterAgent {
                    agent_id: "a".into(),
                    display_name: "A".into(),
                    capability: "m".into(),
                    prompt_hash: vec![7u8; 31],
                    prompt_doc: None,
                    allowed_actions: Vec::new(),
                },
            ),
            // an action outside the known vocabulary.
            (
                user(9),
                AgentMsg::RegisterAgent {
                    agent_id: "a".into(),
                    display_name: "A".into(),
                    capability: "m".into(),
                    prompt_hash: vec![7u8; 32],
                    prompt_doc: None,
                    allowed_actions: vec!["forge.push".into()],
                },
            ),
            // empty required fields.
            (
                user(9),
                AgentMsg::RegisterAgent {
                    agent_id: String::new(),
                    display_name: "A".into(),
                    capability: "m".into(),
                    prompt_hash: vec![7u8; 32],
                    prompt_doc: None,
                    allowed_actions: Vec::new(),
                },
            ),
            (
                user(9),
                AgentMsg::RegisterAgent {
                    agent_id: "bad\u{1f}id".into(),
                    display_name: "A".into(),
                    capability: "m".into(),
                    prompt_hash: vec![7u8; 32],
                    prompt_doc: None,
                    allowed_actions: Vec::new(),
                },
            ),
            (
                user(9),
                AgentMsg::RegisterAgent {
                    agent_id: "a".into(),
                    display_name: "A".into(),
                    capability: String::new(),
                    prompt_hash: vec![7u8; 32],
                    prompt_doc: None,
                    allowed_actions: Vec::new(),
                },
            ),
            // an oversized record is rejected before staging.
            (
                user(9),
                AgentMsg::RegisterAgent {
                    agent_id: "a".into(),
                    display_name: "x".repeat(MAX_AGENT_RECORD_BYTES),
                    capability: "m".into(),
                    prompt_hash: vec![7u8; 32],
                    prompt_doc: None,
                    allowed_actions: Vec::new(),
                },
            ),
        ];
        for (origin, op) in cases {
            let mut ctx = CaptureCtx::new().from_origin(origin.clone());
            let err = exec(&mut m, &mut ctx, &admin(&op)).unwrap_err();
            assert!(matches!(err, Error::Module(_)), "{origin:?} / {op:?}");
            abort(&mut m);
            assert_eq!(m.root(), root0, "a rejected register leaves no trace");
        }
    }

    #[test]
    fn enable_job_worker_is_admin_gated_and_emits_self_registration() {
        let mut m = module();

        let mut intruder = CaptureCtx::new().from_origin(Origin::System);
        let err = exec(
            &mut m,
            &mut intruder,
            &admin(&AgentMsg::EnableJobWorker { enabled: true }),
        )
        .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        abort(&mut m);

        let mut ctx = CaptureCtx::new().from_origin(user(9));
        exec(
            &mut m,
            &mut ctx,
            &admin(&AgentMsg::EnableJobWorker { enabled: true }),
        )
        .unwrap();
        assert_eq!(ctx.job_msgs(), vec![JobsMsg::RegisterWorker {}]);
        commit(&mut m);

        let mut ctx = CaptureCtx::new().from_origin(user(9));
        exec(
            &mut m,
            &mut ctx,
            &admin(&AgentMsg::EnableJobWorker { enabled: false }),
        )
        .unwrap();
        assert_eq!(ctx.job_msgs(), vec![JobsMsg::UnregisterWorker {}]);
        commit(&mut m);

        let mut without_jobs = AgentModule::new(
            "agent",
            "chat",
            "saga",
            "tagging",
            "dispatch",
            Some("tasks".into()),
            None,
            None,
        );
        let mut ctx = CaptureCtx::new().from_origin(user(9));
        let err = exec(
            &mut without_jobs,
            &mut ctx,
            &admin(&AgentMsg::EnableJobWorker { enabled: true }),
        )
        .unwrap_err();
        assert!(matches!(err, Error::Module(m) if m.contains("jobs module")));
    }

    #[test]
    fn update_pause_resume_are_owner_gated() {
        let mut m = module();
        let mut ctx = CaptureCtx::new().from_origin(user(9));
        exec(&mut m, &mut ctx, &admin(&register("bot", &[]))).unwrap();
        commit(&mut m);

        // a foreign origin can neither update nor pause.
        for op in [
            AgentMsg::UpdateAgent {
                agent_id: "bot".into(),
                display_name: Some("Stolen".into()),
                capability: None,
                prompt_hash: None,
                prompt_doc: None,
                allowed_actions: None,
            },
            AgentMsg::PauseAgent {
                agent_id: "bot".into(),
            },
            AgentMsg::ResumeAgent {
                agent_id: "bot".into(),
            },
        ] {
            let mut ctx = CaptureCtx::new().from_origin(user(2));
            let err = exec(&mut m, &mut ctx, &admin(&op)).unwrap_err();
            assert!(matches!(err, Error::Module(_)));
            abort(&mut m);
        }

        // the owner updates fields selectively and toggles status.
        let mut ctx = CaptureCtx::new().at(5).from_origin(user(9));
        exec(
            &mut m,
            &mut ctx,
            &admin(&AgentMsg::UpdateAgent {
                agent_id: "bot".into(),
                display_name: None,
                capability: Some("model-2".into()),
                prompt_hash: None,
                prompt_doc: None,
                allowed_actions: Some(vec![ACTION_TASKS_CREATE.into()]),
            }),
        )
        .unwrap();
        // a capability change retunes the agent's dispatch recipe atomically.
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
        exec(
            &mut m,
            &mut ctx,
            &admin(&AgentMsg::PauseAgent {
                agent_id: "bot".into(),
            }),
        )
        .unwrap();
        commit(&mut m);
        let record = get_agent(&m, "bot").unwrap();
        assert_eq!(record.capability, "model-2");
        assert_eq!(record.display_name, "BOT", "unset fields keep their value");
        assert_eq!(
            record.allowed_actions,
            vec![ACTION_TASKS_CREATE.to_string()]
        );
        assert_eq!(record.status, AgentStatus::Paused);
        assert_eq!(record.updated_at, 5);

        // pausing a paused agent stages nothing: root byte-identical.
        let paused_root = m.root();
        let mut ctx = CaptureCtx::new().at(6).from_origin(user(9));
        exec(
            &mut m,
            &mut ctx,
            &admin(&AgentMsg::PauseAgent {
                agent_id: "bot".into(),
            }),
        )
        .unwrap();
        commit(&mut m);
        assert_eq!(m.root(), paused_root, "an idempotent pause is a no-op");

        let mut ctx = CaptureCtx::new().at(7).from_origin(user(9));
        exec(
            &mut m,
            &mut ctx,
            &admin(&AgentMsg::ResumeAgent {
                agent_id: "bot".into(),
            }),
        )
        .unwrap();
        commit(&mut m);
        assert_eq!(get_agent(&m, "bot").unwrap().status, AgentStatus::Active);
    }

    #[test]
    fn watch_and_unwatch_stage_the_policy_and_emit_the_plane_subscription_atomically() {
        let mut m = module();
        let mut ctx = CaptureCtx::new().from_origin(user(9));
        exec(
            &mut m,
            &mut ctx,
            &admin(&AgentMsg::WatchChannel {
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
        let mut ctx = CaptureCtx::new().from_origin(user(9));
        let err = exec(
            &mut m,
            &mut ctx,
            &admin(&AgentMsg::WatchChannel {
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
            &admin(&AgentMsg::UnwatchChannel {
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
            &admin(&AgentMsg::UnwatchChannel {
                channel_id: "general".into(),
            }),
        )
        .unwrap();
        assert!(ctx.msgs.is_empty(), "an idempotent unwatch emits nothing");
        commit(&mut m);
        assert_eq!(m.root(), before);
    }

    // ---- the engagement intake: turn policies ----------------------------------

    #[test]
    fn mention_policy_engages_only_this_modules_tagged_active_agents() {
        let mut m = watched(
            TurnPolicy::Mention,
            &[("bot1", &[ACTION_CHAT_POST]), ("bot2", &[ACTION_CHAT_POST])],
        );

        // the post tags bot1, an entity of a FOREIGN module, and an
        // unregistered agent — only bot1 engages.
        let mut ctx = CaptureCtx::new()
            .at(3)
            .from_tagging()
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
            "no prompt_doc: the generic instructions lead the payload"
        );
    }

    #[test]
    fn all_policy_engages_every_active_agent_and_paused_agents_never_engage() {
        let mut m = watched(TurnPolicy::All, &[("a", &[]), ("b", &[]), ("c", &[])]);
        let mut ctx = CaptureCtx::new().from_origin(user(9));
        exec(
            &mut m,
            &mut ctx,
            &admin(&AgentMsg::PauseAgent {
                agent_id: "b".into(),
            }),
        )
        .unwrap();
        commit(&mut m);

        let ctx = engage_post(&mut m, 2, &[]);
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
        let mut m = watched(
            TurnPolicy::RoundRobin,
            &[("a", &[]), ("b", &[]), ("c", &[])],
        );

        // seq 4 over [a, b, c]: 4 % 3 = 1 -> "b".
        engage_post(&mut m, 4, &[]);
        commit(&mut m);
        assert!(get_pending(&m, &run_id_for("general", 4, "b")).is_some());
        assert_eq!(get_pending(&m, &run_id_for("general", 4, "a")), None);
        assert_eq!(get_pending(&m, &run_id_for("general", 4, "c")), None);

        // pause "b": the domain shrinks to [a, c]; seq 5 % 2 = 1 -> "c".
        let mut ctx = CaptureCtx::new().from_origin(user(9));
        exec(
            &mut m,
            &mut ctx,
            &admin(&AgentMsg::PauseAgent {
                agent_id: "b".into(),
            }),
        )
        .unwrap();
        commit(&mut m);
        engage_post(&mut m, 5, &[]);
        commit(&mut m);
        assert!(get_pending(&m, &run_id_for("general", 5, "c")).is_some());
        assert_eq!(get_pending(&m, &run_id_for("general", 5, "b")), None);
    }

    #[test]
    fn assigned_policy_engages_exactly_its_agent_and_respects_pause() {
        let mut m = watched(TurnPolicy::Assigned("b".into()), &[("a", &[]), ("b", &[])]);
        engage_post(&mut m, 2, &[]);
        commit(&mut m);
        assert!(get_pending(&m, &run_id_for("general", 2, "b")).is_some());
        assert_eq!(get_pending(&m, &run_id_for("general", 2, "a")), None);

        // paused assignee: nothing engages, the block still commits.
        let mut ctx = CaptureCtx::new().from_origin(user(9));
        exec(
            &mut m,
            &mut ctx,
            &admin(&AgentMsg::PauseAgent {
                agent_id: "b".into(),
            }),
        )
        .unwrap();
        commit(&mut m);
        let ctx = engage_post(&mut m, 3, &[]);
        commit(&mut m);
        assert!(ctx.dispatch_msgs().is_empty());
        assert_eq!(get_pending(&m, &run_id_for("general", 3, "b")), None);
    }

    #[test]
    fn foreign_sources_and_direct_chat_or_saga_follow_ups_are_dead_letters() {
        // the LOOP RULE itself lives in the tagging plane (only user posts
        // fire) and is tested there; this module's job is to survive the
        // events it should not act on.
        let mut m = watched(TurnPolicy::All, &[("bot", &[ACTION_CHAT_POST])]);
        let before = m.root();

        // an engagement whose source is not chat: dropped with a breadcrumb.
        let mut ctx = CaptureCtx::new().at(2).from_tagging();
        exec(
            &mut m,
            &mut ctx,
            &Msg {
                target: "agent".into(),
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
                target: "agent".into(),
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
                target: "agent".into(),
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
        let mut m = watched(TurnPolicy::All, &[("bot", &[])]);
        let before = m.root();

        // an engagement for a channel we do not watch (subscription drift
        // within a block): no-op, never an error.
        let mut ctx = CaptureCtx::new()
            .at(2)
            .from_tagging()
            .with_transcript("random", transcript(2));
        exec(&mut m, &mut ctx, &engagement("random", 2, vec![])).unwrap();
        assert!(ctx.msgs.is_empty());

        // a failing context pin (the ctx serves NO transcript at all — the
        // chat query errors) must not poison the posting block: Ok, no run.
        let mut ctx = CaptureCtx::new().at(2).from_tagging();
        exec(&mut m, &mut ctx, &engagement("general", 2, vec![])).unwrap();
        assert!(ctx.dispatch_msgs().is_empty(), "no dispatch on a failed pin");
        assert!(!ctx.events.is_empty(), "the skip leaves a breadcrumb event");
        commit(&mut m);
        assert_eq!(m.root(), before, "nothing was staged");
    }

    // ---- the turn claim ----------------------------------------------------------

    #[test]
    fn duplicate_turn_claims_are_deterministic_no_ops() {
        let mut m = watched(TurnPolicy::All, &[("bot", &[])]);

        // the engagement claims the turn in the posting block...
        let ctx = engage_post(&mut m, 2, &[]);
        assert_eq!(ctx.dispatch_msgs().len(), 1);
        let run_id = run_id_for("general", 2, "bot");
        let created = get_pending(&m, &run_id).unwrap();

        // ...an explicit RequestRun for the SAME turn in the same block is a
        // staged no-op (first in consensus order won)...
        let mut ctx = CaptureCtx::new()
            .at(2)
            .from_origin(user(5))
            .with_transcript("general", transcript(2));
        exec(
            &mut m,
            &mut ctx,
            &admin(&AgentMsg::RequestRun {
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
        let ctx = engage_post(&mut m, 2, &[]);
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
        let mut m = watched(TurnPolicy::All, &[("bot", &[])]);
        let run_id = run_id_for("general", 2, "bot");
        let taken = dispatch_id_for(&run_id);

        let mut ctx = CaptureCtx::new()
            .at(9)
            .from_tagging()
            .with_transcript("general", transcript(2))
            .with_taken_dispatch(&taken);
        exec(&mut m, &mut ctx, &engagement("general", 2, vec![])).unwrap();
        assert!(ctx.msgs.is_empty(), "a taken turn re-fires nothing");

        let mut ctx = CaptureCtx::new()
            .at(9)
            .from_origin(user(5))
            .with_transcript("general", transcript(2))
            .with_taken_dispatch(&taken);
        exec(
            &mut m,
            &mut ctx,
            &admin(&AgentMsg::RequestRun {
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

        let mut m = watched(TurnPolicy::All, &[("bot", &[])]);
        let root = m.root();

        let mut ctx = CaptureCtx::new().from_origin(user(9));
        let err = exec(
            &mut m,
            &mut ctx,
            &admin(&AgentMsg::WatchChannel {
                channel_id: "bad\u{1f}channel".into(),
                policy: TurnPolicy::All,
            }),
        )
        .unwrap_err();
        assert!(matches!(err, Error::Module(message) if message.contains("unit separator")));
        abort(&mut m);

        let mut ctx = CaptureCtx::new()
            .from_origin(user(1))
            .with_transcript("bad\u{1f}channel", transcript(1));
        let err = exec(
            &mut m,
            &mut ctx,
            &admin(&AgentMsg::RequestRun {
                agent_id: "bot".into(),
                channel_id: "bad\u{1f}channel".into(),
                anchor_seq: 1,
            }),
        )
        .unwrap_err();
        assert!(matches!(err, Error::Module(message) if message.contains("unit separator")));
        abort(&mut m);

        let mut ctx = CaptureCtx::new().from_jobs();
        exec(
            &mut m,
            &mut ctx,
            &jobs_event("bad\u{1f}job", "agent/bot", "spec"),
        )
        .expect("separator in a no-fail jobs event is a no-op");
        assert!(ctx.msgs.is_empty(), "no claim emitted for a bad job id");

        // a spec that does not hash to spec_hash is dropped the same way.
        let mut ctx = CaptureCtx::new().from_jobs();
        exec(
            &mut m,
            &mut ctx,
            &Msg {
                target: "agent".into(),
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
        let mut m = watched(TurnPolicy::All, &[("bot", &[])]);
        let before = m.root();

        // garbage from the tagging origin: the posting block must survive.
        let mut ctx = CaptureCtx::new().from_tagging();
        exec(
            &mut m,
            &mut ctx,
            &Msg {
                target: "agent".into(),
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
                target: "agent".into(),
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
                target: "agent".into(),
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
        let mut m = watched(TurnPolicy::All, &[("bot", &[ACTION_CHAT_POST])]);

        // engagement-shaped bytes from an EXTERNAL origin route to the
        // AgentMsg decoder and fail there — no run is ever created.
        let mut ctx = CaptureCtx::new()
            .at(2)
            .from_origin(user(1))
            .with_transcript("general", transcript(2));
        let err = exec(&mut m, &mut ctx, &engagement("general", 2, vec![])).unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        abort(&mut m);
        assert_eq!(get_pending(&m, &run_id_for("general", 2, "bot")), None);

        // result-shaped bytes from an EXTERNAL origin: same story.
        engage_post(&mut m, 2, &[]);
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
    /// `actions`) at general/2, plus the run id.
    fn awaiting_run(actions: &[&str]) -> (AgentModule, String) {
        let mut m = watched(TurnPolicy::All, &[("bot", actions)]);
        engage_post(&mut m, 2, &[]);
        commit(&mut m);
        (m, run_id_for("general", 2, "bot"))
    }

    #[test]
    fn a_valid_response_emits_the_reply_and_actions_and_prunes_the_entry() {
        let (mut m, run_id) = awaiting_run(&[
            ACTION_CHAT_POST,
            ACTION_TASKS_CREATE,
            ACTION_TASKS_UPDATE_STATUS,
        ]);
        let mut ctx = CaptureCtx::new()
            .at(8)
            .from_dispatch()
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
        let mut m = watched(TurnPolicy::All, &[("bot", &[ACTION_CHAT_POST])]);
        // seq 3 is a reply to root 1; the pin records thread_root = 1.
        let mut transcript = transcript(2);
        transcript.push(message_in(
            "general",
            3,
            AuthorRef::User(vec![1; 32]),
            "in thread",
            Some(1),
        ));
        let mut ctx = CaptureCtx::new()
            .at(3)
            .from_tagging()
            .with_transcript("general", transcript.clone());
        exec(&mut m, &mut ctx, &engagement("general", 3, vec![])).unwrap();
        commit(&mut m);
        let run_id = run_id_for("general", 3, "bot");
        assert_eq!(get_pending(&m, &run_id).unwrap().thread_root, Some(1));

        let mut ctx = CaptureCtx::new()
            .at(9)
            .from_dispatch()
            .with_transcript("general", transcript);
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
            let (mut m, run_id) = awaiting_run(&[
                ACTION_CHAT_POST,
                ACTION_TASKS_CREATE,
                ACTION_TASKS_UPDATE_STATUS,
            ]);
            let mut ctx = CaptureCtx::new()
                .at(8)
                .from_dispatch()
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
            let (mut m, run_id) = awaiting_run(&[ACTION_CHAT_POST]);
            let mut ctx = CaptureCtx::new()
                .at(8)
                .from_dispatch()
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
        let (mut m, run_id) = awaiting_run(&[ACTION_CHAT_POST]);
        let raw = r#"{"reply_blocks":[{"id":"b1","kind":"Paragraph","text":"hello"},{"kind":"Code","lang":"rust","text":"fn main() {}"},{"kind":"Alien","text":"dropped"},{"kind":"Paragraph","text":"  "}],"actions":[]}"#;
        let mut ctx = CaptureCtx::new()
            .at(8)
            .from_dispatch()
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
        let (mut m, run_id) = awaiting_run(&[ACTION_CHAT_POST]);
        let mut ctx = CaptureCtx::new()
            .from_dispatch()
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
        let (mut m, run_id) = awaiting_run(&[ACTION_TASKS_CREATE]);
        let mut ctx = CaptureCtx::new()
            .from_dispatch()
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
        let mut m = AgentModule::new(
            "agent",
            "chat",
            "saga",
            "tagging",
            "dispatch",
            None,
            None,
            None,
        );
        let mut ctx = CaptureCtx::new().from_origin(user(9));
        exec(
            &mut m,
            &mut ctx,
            &admin(&register("bot", &[ACTION_CHAT_POST, ACTION_TASKS_CREATE])),
        )
        .unwrap();
        exec(
            &mut m,
            &mut ctx,
            &admin(&AgentMsg::WatchChannel {
                channel_id: "general".into(),
                policy: TurnPolicy::All,
            }),
        )
        .unwrap();
        commit(&mut m);
        engage_post(&mut m, 2, &[]);
        commit(&mut m);
        let run_id = run_id_for("general", 2, "bot");

        let mut ctx = CaptureCtx::new()
            .from_dispatch()
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
        let (mut m, run_id) = awaiting_run(&[ACTION_CHAT_POST]);
        // someone posted a message whose id IS the run's reply id.
        let mut transcript = transcript(2);
        transcript[1].head.message_id = reply_message_id(&run_id);
        let mut ctx = CaptureCtx::new()
            .from_dispatch()
            .with_transcript("general", transcript);
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
        let mut m = watched(TurnPolicy::All, &[("bot", &[ACTION_CHAT_POST])]);
        // the anchor replies to a root that has hit the reply cap.
        let mut root = message(1, "root");
        root.head.reply_count = MAX_THREAD_REPLIES as u64;
        let anchor = message_in("general", 2, AuthorRef::User(vec![1; 32]), "reply", Some(1));
        let full = vec![root, anchor];
        let mut ctx = CaptureCtx::new()
            .at(2)
            .from_tagging()
            .with_transcript("general", full.clone());
        exec(&mut m, &mut ctx, &engagement("general", 2, vec![])).unwrap();
        commit(&mut m);
        let run_id = run_id_for("general", 2, "bot");

        let mut ctx = CaptureCtx::new()
            .from_dispatch()
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
        let mut m = watched(TurnPolicy::All, &[("bot", &[ACTION_CHAT_POST])]);
        for seq in [2, 3] {
            engage_post(&mut m, seq, &[]);
        }
        commit(&mut m);

        // the dispatch plane already folded saga failures, timeouts, and
        // contract violations into the Err lane — one shape lands here.
        for (seq, reason) in [(2u64, "worker exploded"), (3, "timed out")] {
            let run_id = run_id_for("general", seq, "bot");
            let mut ctx = CaptureCtx::new().at(20).from_dispatch();
            exec(
                &mut m,
                &mut ctx,
                &result_event(&run_id, Err(reason.into())),
            )
            .unwrap();
            assert!(ctx.msgs.is_empty(), "terminal failures emit nothing");
            commit(&mut m);
            assert_eq!(get_pending(&m, &run_id), None, "the entry pruned");
        }
    }

    // ---- the jobs lane ----------------------------------------------------------

    /// a committed module with "duck" registered (task grants) and the jobs
    /// worker semantics of the collaboration loop.
    fn job_ready() -> AgentModule {
        let mut m = module();
        let mut ctx = CaptureCtx::new().from_origin(user(9));
        exec(
            &mut m,
            &mut ctx,
            &admin(&register("duck", &[ACTION_TASKS_CREATE])),
        )
        .unwrap();
        commit(&mut m);
        m
    }

    #[test]
    fn a_job_submit_claims_and_dispatches_with_the_spec_payload() {
        let mut m = job_ready();
        let mut ctx = CaptureCtx::new().at(3).from_jobs();
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
        let mut m = job_ready();
        let root = m.root();

        // an unregistered agent kind: no claim, no dispatch, no entry.
        let mut ctx = CaptureCtx::new().at(2).from_jobs();
        exec(&mut m, &mut ctx, &jobs_event("j", "agent/ghost", "s")).unwrap();
        assert!(ctx.msgs.is_empty());

        // a non-agent kind is somebody else's job.
        let mut ctx = CaptureCtx::new().at(2).from_jobs();
        exec(&mut m, &mut ctx, &jobs_event("j", "render/video", "s")).unwrap();
        assert!(ctx.msgs.is_empty());

        // a paused agent never claims.
        let mut pause = CaptureCtx::new().from_origin(user(9));
        exec(
            &mut m,
            &mut pause,
            &admin(&AgentMsg::PauseAgent {
                agent_id: "duck".into(),
            }),
        )
        .unwrap();
        commit(&mut m);
        let mut ctx = CaptureCtx::new().at(2).from_jobs();
        exec(&mut m, &mut ctx, &jobs_event("j", "agent/duck", "s")).unwrap();
        assert!(ctx.msgs.is_empty());
        commit(&mut m);
        assert_ne!(m.root(), root, "only the pause moved the root");
        assert!(pending_runs(&m).is_empty());
    }

    #[test]
    fn a_job_result_finalizes_the_board_and_emits_actions() {
        let mut m = job_ready();
        let mut ctx = CaptureCtx::new().at(3).from_jobs();
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
        let mut m = job_ready();
        let mut ctx = CaptureCtx::new().at(3).from_jobs();
        exec(&mut m, &mut ctx, &jobs_event("job-1", "agent/duck", "spec")).unwrap();
        commit(&mut m);
        let run_id = job_run_id_for("job-1", "duck", 3);

        let mut ctx = CaptureCtx::new()
            .at(10)
            .from_dispatch()
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
        let mut m = job_ready();
        let mut ctx = CaptureCtx::new().at(3).from_jobs();
        exec(&mut m, &mut ctx, &jobs_event("job-1", "agent/duck", "spec")).unwrap();
        commit(&mut m);
        let run_id = job_run_id_for("job-1", "duck", 3);

        let mut ctx = CaptureCtx::new()
            .at(10)
            .from_dispatch()
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
        let mut m = job_ready();
        let mut ctx = CaptureCtx::new().at(3).from_jobs();
        exec(&mut m, &mut ctx, &jobs_event("job-1", "agent/duck", "spec")).unwrap();
        commit(&mut m);
        let run_id = job_run_id_for("job-1", "duck", 3);

        let mut ctx = CaptureCtx::new()
            .at(2000)
            .from_dispatch()
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
        let mut m = watched(TurnPolicy::Mention, &[("bot", &[ACTION_CHAT_POST])]);
        let request = |agent: &str, seq: u64| {
            admin(&AgentMsg::RequestRun {
                agent_id: agent.into(),
                channel_id: "general".into(),
                anchor_seq: seq,
            })
        };

        // unknown agent, empty origin, missing anchor, anchor 0: all errors.
        let mut ctx = CaptureCtx::new()
            .from_origin(user(1))
            .with_transcript("general", transcript(3));
        assert!(exec(&mut m, &mut ctx, &request("ghost", 3)).is_err());
        abort(&mut m);
        let mut ctx = CaptureCtx::new()
            .from_origin(Origin::External(Vec::new()))
            .with_transcript("general", transcript(3));
        assert!(exec(&mut m, &mut ctx, &request("bot", 3)).is_err());
        abort(&mut m);
        let mut ctx = CaptureCtx::new()
            .from_origin(user(1))
            .with_transcript("general", transcript(3));
        assert!(
            exec(&mut m, &mut ctx, &request("bot", 9)).is_err(),
            "an anchor past the head does not exist"
        );
        abort(&mut m);
        assert!(exec(&mut m, &mut ctx, &request("bot", 0)).is_err());
        abort(&mut m);

        // a paused agent cannot be explicitly run either.
        let mut pause_ctx = CaptureCtx::new().from_origin(user(9));
        exec(
            &mut m,
            &mut pause_ctx,
            &admin(&AgentMsg::PauseAgent {
                agent_id: "bot".into(),
            }),
        )
        .unwrap();
        commit(&mut m);
        let mut ctx = CaptureCtx::new()
            .from_origin(user(1))
            .with_transcript("general", transcript(3));
        assert!(exec(&mut m, &mut ctx, &request("bot", 3)).is_err());
        abort(&mut m);

        // resumed, the request lands: entry staged + dispatch emitted,
        // requester recorded as the submitting user.
        let mut resume_ctx = CaptureCtx::new().from_origin(user(9));
        exec(
            &mut m,
            &mut resume_ctx,
            &admin(&AgentMsg::ResumeAgent {
                agent_id: "bot".into(),
            }),
        )
        .unwrap();
        commit(&mut m);
        let mut ctx = CaptureCtx::new()
            .at(6)
            .from_origin(user(1))
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
        let mut m = watched(TurnPolicy::Mention, &[("bot", &[ACTION_CHAT_POST])]);
        let mut ctx = CaptureCtx::new()
            .from_origin(user(1))
            .with_transcript("general", transcript(3));
        exec(
            &mut m,
            &mut ctx,
            &admin(&AgentMsg::RequestRun {
                agent_id: "bot".into(),
                channel_id: "general".into(),
                anchor_seq: 3,
            }),
        )
        .unwrap();
        commit(&mut m);
        let run_id = run_id_for("general", 3, "bot");
        let cancel = admin(&AgentMsg::CancelRun {
            run_id: run_id.clone(),
        });

        // a foreign origin (neither requester user(1) nor owner user(9)).
        let mut ctx = CaptureCtx::new().from_origin(user(2));
        assert!(exec(&mut m, &mut ctx, &cancel).is_err());
        abort(&mut m);
        // an unknown run is an error too.
        let mut ctx = CaptureCtx::new().from_origin(user(1));
        assert!(
            exec(
                &mut m,
                &mut ctx,
                &admin(&AgentMsg::CancelRun {
                    run_id: "nope".into(),
                }),
            )
            .is_err()
        );
        abort(&mut m);

        // the REQUESTER cancels: the dispatch plane is told; the entry STAYS
        // pending — the plane's Err("cancelled") delivery is the one result
        // path that prunes it.
        let mut ctx = CaptureCtx::new().at(7).from_origin(user(1));
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
        let mut ctx = CaptureCtx::new().from_dispatch();
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
            .with_taken_dispatch(&dispatch_id_for(&run_id));
        exec(&mut m, &mut ctx, &cancel).unwrap();
        assert!(ctx.msgs.is_empty());

        // the OWNER may cancel an engagement-created run (requester = the
        // tagging plane).
        engage_post(&mut m, 2, &["bot"]);
        commit(&mut m);
        let engaged_run = run_id_for("general", 2, "bot");
        let mut ctx = CaptureCtx::new().from_origin(user(9));
        exec(
            &mut m,
            &mut ctx,
            &admin(&AgentMsg::CancelRun {
                run_id: engaged_run.clone(),
            }),
        )
        .unwrap();
        assert_eq!(ctx.dispatch_msgs().len(), 1);
    }

    // ---- consensus-resident prompts ---------------------------------------------

    #[test]
    fn a_prompt_doc_rides_the_payload_only_when_it_hashes_to_the_pin() {
        let prompt = "You are DUCK-1, a terse reviewer.";
        let register_with_doc = |prompt_hash: Vec<u8>| {
            let mut m = module();
            let mut ctx = CaptureCtx::new().from_origin(user(9));
            exec(
                &mut m,
                &mut ctx,
                &admin(&AgentMsg::RegisterAgent {
                    agent_id: "bot".into(),
                    display_name: "BOT".into(),
                    capability: "model-1".into(),
                    prompt_hash,
                    prompt_doc: Some("prompts/bot".into()),
                    allowed_actions: vec![ACTION_CHAT_POST.into()],
                }),
            )
            .unwrap();
            exec(
                &mut m,
                &mut ctx,
                &admin(&AgentMsg::WatchChannel {
                    channel_id: "general".into(),
                    policy: TurnPolicy::All,
                }),
            )
            .unwrap();
            commit(&mut m);
            m
        };

        // the doc's canonical rendering hashes to the pin: it LEADS the
        // composed payload — for chat runs AND job runs.
        let good_hash = Sha256::digest(prompt.as_bytes()).to_vec();
        let mut m = register_with_doc(good_hash.clone());
        let mut ctx = CaptureCtx::new()
            .at(2)
            .from_tagging()
            .with_transcript("general", transcript(2))
            .with_doc("prompts/bot", prompt);
        exec(&mut m, &mut ctx, &engagement("general", 2, vec![])).unwrap();
        let dispatches = ctx.dispatch_msgs();
        assert_eq!(dispatches.len(), 1);
        let DispatchMsg::Dispatch { payload, .. } = &dispatches[0] else {
            panic!("expected a dispatch");
        };
        assert!(String::from_utf8(payload.clone()).unwrap().starts_with(prompt));
        abort(&mut m);

        let mut ctx = CaptureCtx::new()
            .at(3)
            .from_jobs()
            .with_doc("prompts/bot", prompt);
        exec(&mut m, &mut ctx, &jobs_event("job-1", "agent/bot", "spec")).unwrap();
        let dispatches = ctx.dispatch_msgs();
        assert_eq!(dispatches.len(), 1);
        let DispatchMsg::Dispatch { payload, .. } = &dispatches[0] else {
            panic!("expected a dispatch");
        };
        assert!(
            String::from_utf8(payload.clone()).unwrap().starts_with(prompt),
            "the job payload leads with the prompt doc too"
        );

        // a drifted document (hash mismatch) skips the run — a breadcrumb,
        // never a block abort; and an explicit request errors cleanly. a
        // job submit is left UNCLAIMED the same way.
        let mut m = register_with_doc(vec![7u8; PROMPT_HASH_LEN]);
        let mut ctx = CaptureCtx::new()
            .at(2)
            .from_tagging()
            .with_transcript("general", transcript(2))
            .with_doc("prompts/bot", prompt);
        exec(&mut m, &mut ctx, &engagement("general", 2, vec![])).unwrap();
        assert!(ctx.msgs.is_empty(), "a drifted prompt doc dispatches nothing");
        assert!(!ctx.events.is_empty());
        let mut ctx = CaptureCtx::new()
            .at(2)
            .from_jobs()
            .with_doc("prompts/bot", prompt);
        exec(&mut m, &mut ctx, &jobs_event("job-2", "agent/bot", "spec")).unwrap();
        assert!(ctx.msgs.is_empty(), "no claim on an uncomposable job");
        let mut ctx = CaptureCtx::new()
            .from_origin(user(1))
            .with_transcript("general", transcript(2))
            .with_doc("prompts/bot", prompt);
        let err = exec(
            &mut m,
            &mut ctx,
            &admin(&AgentMsg::RequestRun {
                agent_id: "bot".into(),
                channel_id: "general".into(),
                anchor_seq: 2,
            }),
        )
        .unwrap_err();
        assert!(matches!(err, Error::Module(reason) if reason.contains("prompt")));

        // a MISSING document skips the same way.
        let mut m = register_with_doc(good_hash);
        let mut ctx = CaptureCtx::new()
            .at(2)
            .from_tagging()
            .with_transcript("general", transcript(2));
        exec(&mut m, &mut ctx, &engagement("general", 2, vec![])).unwrap();
        assert!(ctx.msgs.is_empty());
    }

    // ---- determinism + queries + state-sync -------------------------------------------

    #[test]
    fn two_instances_replaying_the_same_ops_produce_identical_roots() {
        let run_id = run_id_for("general", 2, "bot");
        let build = || {
            let mut m = module();
            let mut roots = Vec::new();
            // block 1: register two agents + watch.
            let mut ctx = CaptureCtx::new().at(1).from_origin(user(9));
            exec(
                &mut m,
                &mut ctx,
                &admin(&register("bot", &[ACTION_CHAT_POST])),
            )
            .unwrap();
            exec(&mut m, &mut ctx, &admin(&register("z", &[]))).unwrap();
            exec(
                &mut m,
                &mut ctx,
                &admin(&AgentMsg::WatchChannel {
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
                .with_transcript("general", transcript(2));
            exec(
                &mut m,
                &mut ctx,
                &result_event(&run_id, Ok(response(&["done"], vec![]))),
            )
            .unwrap();
            commit(&mut m);
            roots.push(m.root());
            // block 4: pause an agent.
            let mut ctx = CaptureCtx::new().at(4).from_origin(user(9));
            exec(
                &mut m,
                &mut ctx,
                &admin(&AgentMsg::PauseAgent {
                    agent_id: "z".into(),
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
    fn queries_list_pending_watches_and_agents() {
        let mut m = watched(TurnPolicy::All, &[("a", &[]), ("b", &[])]);
        // watch a second channel and create runs in both.
        let mut ctx = CaptureCtx::new().from_origin(user(9));
        exec(
            &mut m,
            &mut ctx,
            &admin(&AgentMsg::WatchChannel {
                channel_id: "dev".into(),
                policy: TurnPolicy::All,
            }),
        )
        .unwrap();
        commit(&mut m);
        engage_post(&mut m, 2, &[]);
        let mut ctx = CaptureCtx::new().at(3).from_tagging().with_transcript(
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

        let reply = block_on(m.query(&encode_query(&AgentQuery::Watches))).unwrap();
        let AgentReply::Watches(watches) = decode_reply(&reply).unwrap() else {
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

        let reply = block_on(m.query(&encode_query(&AgentQuery::Agents))).unwrap();
        let AgentReply::Agents(agents) = decode_reply(&reply).unwrap() else {
            panic!("agents reply expected");
        };
        assert_eq!(
            agents
                .iter()
                .map(|a| a.agent_id.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "b"]
        );
    }

    #[test]
    fn state_sync_handle_exposes_the_snapshot_bytes() {
        let mut m = watched(TurnPolicy::All, &[("bot", &[ACTION_CHAT_POST])]);
        engage_post(&mut m, 2, &[]);
        commit(&mut m);
        assert_eq!(
            m.state_sync_handle().unwrap(),
            StateSyncHandle::SnapshotBytes(m.snapshot()),
            "the handle IS the canonical snapshot"
        );
    }
}
