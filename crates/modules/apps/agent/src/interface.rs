//! the agent module's public wire surface — types only.
//!
//! the agent module is the platform's agent REGISTRY and nothing more: a
//! self-contained record book of which agents exist — owner, capability tag,
//! curated skill set, granted actions, status. it consumes no other module's events
//! and emits exactly one follow-up shape ([`AgentEvent`], to a
//! genesis-configured hook target) so the module that runs agents can keep
//! each agent's dispatch-plane recipe in lockstep, atomically with the
//! registration that changed it. engagement, run orchestration, and response
//! delivery live in the runs module (`runs`). three payload
//! families cross this surface:
//!
//! - [`AgentMsg`] — writes: registry admin only.
//! - [`AgentResponse`] — the formal response wire spec: what a model's raw
//!   answer is normalized into, and what the strict-output instruction asks
//!   for. reply blocks are this surface's OWN vocabulary (kind + text), not
//!   another module's — the consuming module maps them to chat blocks when it
//!   emits the reply. it is DATA until deterministically validated against
//!   the agent's allowed-action set.
//! - [`AgentQuery`] -> [`AgentReply`] — reads over the registry.

use borsh::{BorshDeserialize, BorshSerialize};
use saga::SagaOrigin;
use serde::{Deserialize, Serialize};

// ---- consensus constants ----------------------------------------------------

/// hard cap on the actions one run's response may carry — the blast-radius
/// bound on the follow-up fan-out a single delivery can cause.
pub const MAX_ACTIONS_PER_RUN: usize = 8;

/// hard cap on the SERIALIZED bytes of a response's actions — the byte peer of
/// [`MAX_ACTIONS_PER_RUN`]'s count cap. action payloads (task ids, titles,
/// statuses) are otherwise unbounded strings, and the delivering module embeds
/// the validated response in a bounded job-finalize payload — so, like
/// [`MAX_REPLY_BLOCKS_BYTES`], it must be able to prove the size BEFORE
/// emitting.
pub const MAX_ACTIONS_BYTES: usize = 8 * 1024;

/// hard cap on one live peer call's instruction. The text is injected into the
/// callee's composed payload, so it needs its own trust-boundary bound before
/// composition.
pub const MAX_DELEGATION_INSTRUCTION_BYTES: usize = 4 * 1024;

/// hard cap on one serialized live peer-call request. `subagent_budget` plus
/// the fixed concurrent-call cap bound compute; this independently bounds
/// replicated input bytes.
pub const MAX_DELEGATIONS_BYTES: usize = 8 * 1024;

/// hard cap on concurrent live peer calls in one root run tree, independent of
/// the owner's potentially larger budget grant.
pub const MAX_DELEGATIONS_PER_RUN: usize = 8;

/// hard cap on a serialized [`AgentRecord`] — registry entries are replicated
/// consensus state, so registration is size-gated up front (at stage time).
pub const MAX_AGENT_RECORD_BYTES: usize = 4 * 1024;

/// hard cap on the COUNT of registered agents. the registry's roster — the
/// enumeration record consensus itself consumes (runs' `All`/`RoundRobin`
/// engagement domain reads EVERY active agent) — is one replicated record,
/// and every id in it also costs a dispatch fan-out under `All` engagement;
/// both must stay bounded. deliberately generous: a thousand agents is a
/// fleet, and each still costs [`MAX_AGENT_RECORD_BYTES`] of replicated state.
pub const MAX_REGISTERED_AGENTS: usize = 1024;

/// hard cap on the COUNT of agents ONE owner may register. without this, a
/// single account (one external key, or one module) fills the whole registry
/// (`MAX_REGISTERED_AGENTS`) alone and locks out every other account — the
/// global cap bounds total state, this one bounds one account's SHARE of it.
/// small on purpose: a legitimate fleet operator still needs dozens, not
/// hundreds, of distinct agent identities.
pub const MAX_AGENTS_PER_OWNER: usize = 32;

/// hard cap on the COUNT of skills one agent curates. an unbounded skill list
/// is unbounded replicated state (it rides the record, hence every snapshot)
/// AND an unbounded run context — every one of them costs at least an index
/// line in the assembled context document, and an `Always` one costs its whole
/// body. [`MAX_AGENT_RECORD_BYTES`] bounds the BYTES and usually bites first;
/// this bounds the SHAPE, and it is the same number the host-side assembler
/// checks (`compute_service::assemble_context_doc`) — one rule, not two that
/// could drift into a record consensus accepts but no run can load.
///
/// deliberately generous, because curation is not the only door: an uncurated
/// skill belongs in the global library at `/shared/skills/`, which every run is
/// told about and which costs a run nothing until it reads one.
pub const MAX_SKILLS_PER_AGENT: usize = 64;

/// hard cap on a skill's `name` length in bytes. the name is not a label — it
/// becomes a run's host mount directory name verbatim — so it rides the same
/// bound an ordinary filename would.
pub const MAX_SKILL_NAME_BYTES: usize = 64;

/// the ONE rule for a skill's `name`: a bounded charset, `.`/`..` refused
/// outright (both pass the charset alone), and a byte cap. a `SkillRef::name`
/// becomes a run's host directory name verbatim
/// (`compute_service::envelope` copies it into `mount_subpath`), so this is
/// the single predicate BOTH sides of that trust boundary must agree on:
/// [`AgentModule::validate_skills`] calls it at consensus time, and
/// `noded::agent_provision::mount_dir_name` calls this same function at
/// provision time — one rule, never two that could drift into a record
/// consensus accepts but no run can load.
pub fn is_skill_mount_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_SKILL_NAME_BYTES
        && name != "."
        && name != ".."
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// the duckfs directory the GLOBAL SKILL LIBRARY lives under, one subdirectory
/// per skill (`<name>/SKILL.md`). a CONVENTION, not a consensus-enforced
/// namespace: the library is ordinary duckfs state, and an agent reaches it
/// through the same `duckfs_read` cap as any other path
/// ([`AgentRecord::library_readable`]).
///
/// it lives HERE, beside the caps that gate it, because three surfaces have to
/// agree on one string: the cap the app grants, the check the run assembler
/// makes before telling an agent the library exists, and the prefix the MCP
/// tool plane is asked to read. NO trailing slash — a cap entry is a path
/// prefix, and `permits` grants children by `"{prefix}/"`, so a trailing slash
/// would grant the directory and none of its contents.
pub const SKILL_LIBRARY_PREFIX: &str = "/shared/skills";

/// hard cap on the serialized CHAT blocks a response's `reply_blocks` map to.
/// deliberately well under chat's `MAX_MESSAGE_HEAD_BYTES`: the reply is
/// emitted as a chat post inside the delivery block, and a post that chat
/// rejects would abort that block (the no-fail rule) — so the delivering
/// module must be able to prove the post will fit BEFORE emitting.
pub const MAX_REPLY_BLOCKS_BYTES: usize = 32 * 1024;

/// required byte length of a recipe content-address (a sha256 digest).
pub const RECIPE_HASH_LEN: usize = 32;

/// the reserved unit separator agent ids must never contain: the runs module
/// keys its run records with `\x1f`-delimited fields, and an agent id
/// carrying the delimiter would make those keys ambiguous. the registry
/// rejects it at registration; downstream modules rely on that.
pub const RESERVED_ID_SEPARATOR: char = '\u{1f}';

// ---- the action vocabulary ---------------------------------------------------

/// permission to post reply blocks into chat — the run's ANSWER, in the channel
/// and thread it was engaged from. deliberately NOT the permission to post
/// wherever it likes: see [`ACTION_CHAT_POST_MESSAGE`].
pub const ACTION_CHAT_POST: &str = "chat.post";
/// permission to post a message to an ARBITRARY channel
/// ([`AgentAction::PostMessage`]) — a strictly wider grant than
/// [`ACTION_CHAT_POST`], which only ever lets an agent answer where it was
/// spoken to.
///
/// a separate name on purpose. folding "post anywhere, unprompted" into
/// `chat.post` would have SILENTLY widened every agent already registered with
/// it — an owner who granted "may answer me" would have been giving "may post
/// in any channel at any time" without ever being asked. a new action name means
/// the wider power can only arrive by an owner deliberately granting it.
pub const ACTION_CHAT_POST_MESSAGE: &str = "chat.post_message";
/// permission to create a task ([`AgentAction::CreateTask`]).
pub const ACTION_TASKS_CREATE: &str = "tasks.create";
/// permission to move a task ([`AgentAction::UpdateTaskStatus`]).
pub const ACTION_TASKS_UPDATE_STATUS: &str = "tasks.update_status";
/// permission to anchor a comment to a page or block
/// ([`AgentAction::AddPageComment`]).
pub const ACTION_PAGES_COMMENT: &str = "pages.comment";
/// permission to flip a todo block's checked state
/// ([`AgentAction::SetPageChecked`]).
pub const ACTION_PAGES_SET_CHECKED: &str = "pages.set_checked";
/// permission to write a small UTF-8 text file under a granted duckfs prefix
/// ([`AgentAction::DuckfsWriteText`]).
pub const ACTION_DUCKFS_WRITE_TEXT: &str = "duckfs.write_text";
/// maximum UTF-8 text payload accepted by [`AgentAction::DuckfsWriteText`].
pub const MAX_DUCKFS_WRITE_TEXT_BYTES: usize = 4 * 1024;

/// every action name the platform knows. `RegisterAgent`/`UpdateAgent` reject
/// an `allowed_actions` entry outside this vocabulary, so a granted permission
/// always means something.
///
/// ADDITIVE only: an existing record's `allowed_actions` is a set of these
/// names, so a NEW name grants nothing to an agent already registered — it can
/// only arrive through an owner-gated `UpdateAgent`. removing or renaming one,
/// by contrast, would strand every record that holds it.
pub const KNOWN_ACTIONS: [&str; 7] = [
    ACTION_CHAT_POST,
    ACTION_CHAT_POST_MESSAGE,
    ACTION_TASKS_CREATE,
    ACTION_TASKS_UPDATE_STATUS,
    ACTION_PAGES_COMMENT,
    ACTION_PAGES_SET_CHECKED,
    ACTION_DUCKFS_WRITE_TEXT,
];

// ---- runtime identity ---------------------------------------------------------

/// the D3 resource-capability grant an agent carries. every list is a
/// canonical SORTED + DEDUPED set (the write path canonicalizes, the committed
/// decoder rejects a non-ascending list) so two logically-equal grants hash
/// identically. `secrets` are OPAQUE vault references (D6) — never a
/// materialized value, never key material (D1). an empty `ResourceCaps` is the
/// default and denies every request except a zero budget check.
#[derive(
    BorshSerialize, BorshDeserialize, Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq,
)]
#[serde(deny_unknown_fields)]
pub struct ResourceCaps {
    /// forge repos this agent may READ.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forge_read: Vec<String>,
    /// forge repos this agent may PUSH to (implies read).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forge_push: Vec<String>,
    /// duckfs workspace-relative path prefixes this agent may READ (ro).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub duckfs_read: Vec<String>,
    /// duckfs workspace-relative path prefixes this agent may WRITE (rw).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub duckfs_write: Vec<String>,
    /// tool / mcp ids this agent may invoke.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<String>,
    /// D6 vault references (scoped, opaque). refs only — the value is resolved
    /// host-side and NEVER crosses consensus.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secrets: Vec<String>,
    /// page ids this agent may WRITE (comment on / check off). page ids are
    /// opaque, so matching is exact — no prefix containment — with the one
    /// literal entry `"*"` granting every page.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pages_write: Vec<String>,
    /// concurrent peer-call ceiling; 0 = none. completed calls release their
    /// slot, and the runtime applies a smaller hard cap.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub subagent_budget: u32,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// whether a [`ResourceCaps`] is the empty default — used to keep the empty
/// record's serialized JSON (and its `MAX_AGENT_RECORD_BYTES` size check)
/// byte-lean.
pub(crate) fn caps_is_default(c: &ResourceCaps) -> bool {
    *c == ResourceCaps::default()
}

/// how a curated skill reaches the model. the agent's SOUL is its `Always`
/// skills: the host assembles their full bodies into the one context document
/// the executor auto-loads, in curation order — this is where the old
/// `prompt_hash` blob went. an `OnDemand` skill is listed by name and
/// description in that document's index and read from its read-only mount only
/// when the task calls for it.
///
/// the mode rides the agent's skill REFERENCE, not the skill document, because
/// curation is per-agent: the same skill is one agent's persona and another's
/// reference material. it lives in consensus (rather than in the document's own
/// frontmatter) so "what does this agent always load" is visible to the root-hash
/// and to the UI.
#[derive(
    BorshSerialize,
    BorshDeserialize,
    Serialize,
    Deserialize,
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum LoadMode {
    /// inlined verbatim into the assembled context document — the persona.
    Always,
    /// indexed by name + description; the body is read from the mount on demand.
    #[default]
    OnDemand,
}

/// a C4 skill reference an agent's runs mount. this pins the REF, never the
/// content: `source_prefix` is a duckfs read-only subtree and
/// `source_snapshot` is its optional consensus pin — `Some` is a PINNED skill
/// (immutable), `None` is a TRACKING skill (the phase-5 composer resolves the
/// committed head at compose time). the list is ORDERED (later entries override
/// earlier, and `Always` bodies assemble in this order), so it is a `Vec`, not a
/// set — order is significant to the hash.
///
/// deliberately a struct (not an enum): the phase-5 envelope composer reads
/// every field straight through into a skill mount, so the same fields live here.
#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SkillRef {
    pub name: String,
    pub source_prefix: String,
    /// `Some` = pinned (immutable) snapshot id; `None` = tracking (resolved at
    /// compose time).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_snapshot: Option<String>,
    /// `default` so a submitter's JSON that omits it decodes as `OnDemand` —
    /// the conservative mode: an unstated skill never silently becomes persona.
    #[serde(default)]
    pub load: LoadMode,
}

/// a D3 capability request the runtime probes an [`AgentRecord`] with before
/// applying an effect or opening a sink (the delivery path calls
/// [`AgentRecord::permits`]). the record carries the grant; this is the ask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapRequest<'a> {
    /// read the named forge repo.
    ForgeRead(&'a str),
    /// push to the named forge repo.
    ForgePush(&'a str),
    /// write the named duckfs workspace-relative path.
    DuckfsWrite(&'a str),
    /// read the named duckfs workspace-relative path.
    DuckfsRead(&'a str),
    /// invoke the named tool / mcp.
    Tool(&'a str),
    /// resolve the named vault secret ref.
    Secret(&'a str),
    /// write (comment on / check off) the named page.
    PagesWrite(&'a str),
    /// spawn a sub-agent (checked against the budget ceiling).
    SpawnSubagent,
}

// ---- registry ----------------------------------------------------------------

/// whether an agent may engage new runs. a paused agent never engages — but
/// pausing does not cancel work already dispatched.
#[derive(
    BorshSerialize, BorshDeserialize, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentStatus {
    Active,
    Paused,
}

/// Owner-assigned semantic role. General is the default; a record that omits
/// the role is an ordinary (General) agent.
#[derive(
    BorshSerialize,
    BorshDeserialize,
    Serialize,
    Deserialize,
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentRole {
    #[default]
    General,
}

/// one registered agent — an ordered-op registration, so which capability and
/// which SKILLS an agent runs is part of the root-hash and auditable. `owner` is
/// the registration origin and gates every mutation of the record.
///
/// `capability` names WHAT the run needs (an open-set registry tag like
/// "codex" — dispatch selects providers of that tag); HOW it runs — binary,
/// flags, model — is host policy in each provider's capability spec, and
/// consensus never sees it. the record is a recipe, not an executor config.
///
/// there is no prompt pin: an agent is DEFINED by its curated `skills`, and its
/// persona is simply a skill loaded [`LoadMode::Always`]. determinism is
/// unchanged in kind — consensus used to commit which prompt bytes ran (a
/// hash), and now commits which skill snapshots ran (pins). both are content
/// addresses; the skill one is also editable, diffable, and reviewable.
#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AgentRecord {
    pub agent_id: String,
    pub owner: SagaOrigin,
    pub display_name: String,
    /// the capability registry tag this agent's runs are dispatched on.
    pub capability: String,
    /// granted action names, each from [`KNOWN_ACTIONS`], sorted and deduped.
    pub allowed_actions: Vec<String>,
    pub status: AgentStatus,
    #[serde(default, skip_serializing_if = "role_is_default")]
    pub role: AgentRole,
    pub created_at: u64,
    pub updated_at: u64,
    /// W4 recipe content-address: empty (unset) or exactly [`RECIPE_HASH_LEN`]
    /// bytes. the committed encoding always carries it (empty when unset).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recipe_hash: Vec<u8>,
    /// D3 resource caps. the committed encoding always carries it (default-empty
    /// when unset).
    #[serde(default, skip_serializing_if = "caps_is_default")]
    pub caps: ResourceCaps,
    /// C4 ordered skill refs. the committed encoding always carries it (empty
    /// when unset).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<SkillRef>,
}

fn role_is_default(role: &AgentRole) -> bool {
    *role == AgentRole::General
}

impl AgentRecord {
    /// the pure D3 cap gate. the runtime calls this before applying an effect
    /// or opening a sink; a record with empty caps denies every request except a
    /// positive budget check. forge/tool/
    /// secret use exact membership; duckfs uses path-PREFIX containment (a
    /// prefix grants itself and any child path, but never a sibling that merely
    /// shares a textual prefix — `src` does not grant `srcx`); pages use exact
    /// membership with the literal `"*"` entry granting every page (ids are
    /// opaque — never a prefix). budget
    /// CONSUMPTION is the runtime's concern; this only reads the ceiling.
    pub fn permits(&self, req: &CapRequest) -> bool {
        let c = &self.caps;
        let has = |v: &[String], x: &str| v.iter().any(|s| s == x);
        let under = |v: &[String], p: &str| {
            v.iter().any(|pre| {
                p == pre || pre == "/" && p.starts_with('/') || p.starts_with(&format!("{pre}/"))
            })
        };
        match req {
            CapRequest::ForgeRead(r) => has(&c.forge_read, r) || has(&c.forge_push, r),
            CapRequest::ForgePush(r) => has(&c.forge_push, r),
            CapRequest::DuckfsWrite(p) => under(&c.duckfs_write, p),
            CapRequest::DuckfsRead(p) => under(&c.duckfs_read, p) || under(&c.duckfs_write, p),
            CapRequest::Tool(t) => has(&c.tools, t),
            CapRequest::Secret(s) => has(&c.secrets, s),
            CapRequest::PagesWrite(p) => has(&c.pages_write, "*") || has(&c.pages_write, p),
            CapRequest::SpawnSubagent => c.subagent_budget > 0,
        }
    }

    /// whether this agent may READ the global skill library
    /// ([`SKILL_LIBRARY_PREFIX`]) — the one question the host-side run assembler
    /// asks before telling the agent the library is there.
    ///
    /// deliberately [`Self::permits`] and nothing else: the assembled document
    /// tells the agent to call the MCP tool plane's `ducktape_files_grep` /
    /// `ducktape_files_read`, and those tools gate on exactly this call. a
    /// second, hand-rolled prefix rule here could drift from the one that
    /// enforces — and the drift would show up as a document that promises a door
    /// the tool plane then refuses to open.
    pub fn library_readable(&self) -> bool {
        self.permits(&CapRequest::DuckfsRead(SKILL_LIBRARY_PREFIX))
    }

    /// The callee as it may execute for this caller. Agents remain peers: a
    /// call does not require matching owners, providers, or a permanent
    /// parent/child relation. Authority is instead narrowed for this run to
    /// the intersection of both agents' grants.
    ///
    /// The callee keeps its standing curated skills only where the caller can
    /// also read the source. Curation itself is the callee's standing access;
    /// the caller check prevents a call from widening that access.
    pub fn scoped_for_call(&self, callee: &AgentRecord) -> AgentRecord {
        let mut scoped = callee.clone();
        scoped
            .allowed_actions
            .retain(|action| self.allowed_actions.binary_search(action).is_ok());
        scoped.caps = self.caps.intersection(&callee.caps);
        let caller = self;
        scoped
            .skills
            .retain(|skill| caller.permits(&CapRequest::DuckfsRead(&skill.source_prefix)));
        scoped
    }
}

impl ResourceCaps {
    /// Intersection used by one run-scoped agent call. Exact-name grants use
    /// set intersection. DuckFS prefixes use containment and keep the narrower
    /// prefix. Read authority includes write authority, matching [`AgentRecord::permits`].
    pub fn intersection(&self, other: &Self) -> Self {
        fn exact(left: &[String], right: &[String]) -> Vec<String> {
            left.iter()
                .filter(|value| right.binary_search(value).is_ok())
                .cloned()
                .collect()
        }

        fn under(prefix: &str, path: &str) -> bool {
            path == prefix
                || prefix == "/" && path.starts_with('/')
                || path.starts_with(&format!("{prefix}/"))
        }

        fn prefixes(left: &[String], right: &[String]) -> Vec<String> {
            let mut out = Vec::new();
            for a in left {
                for b in right {
                    if under(a, b) {
                        out.push(b.clone());
                    } else if under(b, a) {
                        out.push(a.clone());
                    }
                }
            }
            out.sort();
            out.dedup();
            out
        }

        fn readable(caps: &ResourceCaps) -> Vec<String> {
            let mut values = caps.duckfs_read.clone();
            values.extend(caps.duckfs_write.iter().cloned());
            values.sort();
            values.dedup();
            values
        }

        fn forge_readable(caps: &ResourceCaps) -> Vec<String> {
            let mut values = caps.forge_read.clone();
            values.extend(caps.forge_push.iter().cloned());
            values.sort();
            values.dedup();
            values
        }

        let pages_write = if self.pages_write.iter().any(|page| page == "*") {
            other.pages_write.clone()
        } else if other.pages_write.iter().any(|page| page == "*") {
            self.pages_write.clone()
        } else {
            exact(&self.pages_write, &other.pages_write)
        };
        Self {
            forge_read: exact(&forge_readable(self), &forge_readable(other)),
            forge_push: exact(&self.forge_push, &other.forge_push),
            duckfs_read: prefixes(&readable(self), &readable(other)),
            duckfs_write: prefixes(&self.duckfs_write, &other.duckfs_write),
            tools: exact(&self.tools, &other.tools),
            secrets: exact(&self.secrets, &other.secrets),
            pages_write,
            subagent_budget: self.subagent_budget.min(other.subagent_budget),
        }
    }
}

// ---- the response wire spec ----------------------------------------------------
// The MODEL output boundary. Every container here is lenient on purpose:
// unknown JSON fields are ignored and every field defaults, so a model answer
// either IS this shape or the consumer wraps it as one. Strictness lives in
// the separate validation step (grants, caps, probes) — never in the decode.

/// one reply block in this surface's OWN vocabulary — exactly the shape the
/// strict-output instruction asks the model for. `kind` is one of
/// "paragraph", "heading", or "code" (lowercase; anything else drops in
/// normalization); the consuming module maps these to chat blocks at emission.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ReplyBlock {
    pub kind: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
}

/// One run-scoped call to a registered peer agent. Runs derives
/// identity/authority from the caller and accepts only an existing agent plus a
/// bounded instruction.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DelegationRequest {
    pub agent_id: String,
    pub instruction: String,
    /// Library skill names curated for this call, on top of the callee's own
    /// curation — the whole point of curating at call time: a peer keeps
    /// its persona and gains what this one task needs. each name resolves to
    /// `/shared/skills/<name>`, loaded on demand; a caller offers a peer a
    /// library skill, it never authors a path or a persona-inlining body. empty
    /// = the callee's own curation verbatim.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<String>,
}

/// the formal agent response: reply blocks, a bounded list of [`AgentAction`]s,
/// and an optional workspace commit message.
/// lenient by construction — all fields default, unknown JSON fields are
/// ignored — so a model answer either IS this shape or the consumer wraps it
/// as one; validation (grants, caps, probes) is a separate, strict step.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentResponse {
    #[serde(default)]
    pub reply_blocks: Vec<ReplyBlock>,
    #[serde(default)]
    pub actions: Vec<AgentAction>,
    /// complete Git commit message authored by the agent for uncommitted
    /// workspace changes. Optional; a clean response (no workspace changes) omits
    /// it; existing agent commits keep their own messages. The host owns only
    /// safety validation, Git identity, and Forge-title recovery.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_message: Option<String>,
}

/// one validated cross-module write an agent's response may request.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentAction {
    /// post a message to a named channel ([`ACTION_CHAT_POST_MESSAGE`]) — the
    /// agent SPEAKING, as opposed to `reply_blocks`, which is the agent
    /// ANSWERING where it was engaged. `thread` makes it a reply under that
    /// root sequence.
    ///
    /// this is what lets an agent report progress while it works instead of
    /// saving everything for the end. it is a genuinely wider power than a
    /// reply, which is exactly why it carries its own action name.
    PostMessage {
        channel_id: String,
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        thread: Option<u64>,
    },
    CreateTask {
        task_id: String,
        title: String,
    },
    /// `status` is the wire name of a `tasks::TaskStatus`:
    /// `"open"`, `"in_progress"`, or `"done"`.
    UpdateTaskStatus {
        task_id: String,
        status: String,
    },
    /// anchor a comment to `target` — a page id or a block id in the pages
    /// module ([`ACTION_PAGES_COMMENT`]).
    AddPageComment {
        target: String,
        body: String,
    },
    /// flip a todo block's checked state ([`ACTION_PAGES_SET_CHECKED`]).
    SetPageChecked {
        block: String,
        checked: bool,
    },
    /// write a small UTF-8 text file through the files module's commit wire
    /// ([`ACTION_DUCKFS_WRITE_TEXT`]). `base_snapshot` feeds files' own
    /// per-path CAS (`FilesMsg::Commit`), never a global-head check: `Some`
    /// names the snapshot the write was staged against, `None` is files' own
    /// create-only sense (the empty tree) — the path must not already exist.
    /// omitted by an action the model authors directly (not through the
    /// `ducktape_duckfs_write_text` tool, which always fills it from a live
    /// refs query), so `None` on a non-empty filesystem is ordinary, not an
    /// error: it just means the write only succeeds if that path is new.
    DuckfsWriteText {
        path: String,
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base_snapshot: Option<String>,
    },
}

impl AgentAction {
    /// the vocabulary name this action needs in the agent's `allowed_actions`.
    pub fn vocabulary_name(&self) -> &'static str {
        match self {
            AgentAction::PostMessage { .. } => ACTION_CHAT_POST_MESSAGE,
            AgentAction::CreateTask { .. } => ACTION_TASKS_CREATE,
            AgentAction::UpdateTaskStatus { .. } => ACTION_TASKS_UPDATE_STATUS,
            AgentAction::AddPageComment { .. } => ACTION_PAGES_COMMENT,
            AgentAction::SetPageChecked { .. } => ACTION_PAGES_SET_CHECKED,
            AgentAction::DuckfsWriteText { .. } => ACTION_DUCKFS_WRITE_TEXT,
        }
    }
}

// ---- ops ----------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentMsg {
    /// register an agent under the submitter's origin (a non-empty external
    /// key or a module — the owner capability). a duplicate `agent_id` is an
    /// error. registration also notifies the configured hook target
    /// ([`AgentEvent::Registered`]) in the same block, so the agent's
    /// dispatch-plane recipe lands (or aborts) atomically with the record.
    RegisterAgent {
        agent_id: String,
        display_name: String,
        capability: String,
        allowed_actions: Vec<String>,
        /// runtime-identity fields, all optional — a registration that sets none
        /// omits them; the module accepts them unconditionally.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        recipe_hash: Option<Vec<u8>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        caps: Option<ResourceCaps>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        skills: Option<Vec<SkillRef>>,
    },
    /// owner-gated partial update; `None` fields keep their current value. a
    /// capability change also notifies the hook target
    /// ([`AgentEvent::CapabilityChanged`]) in the same block.
    UpdateAgent {
        agent_id: String,
        display_name: Option<String>,
        capability: Option<String>,
        allowed_actions: Option<Vec<String>>,
        /// runtime-identity fields; `None` keeps the current value.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        recipe_hash: Option<Vec<u8>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        caps: Option<ResourceCaps>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        skills: Option<Vec<SkillRef>>,
    },
    /// owner-gated: stop the agent from engaging new runs.
    PauseAgent { agent_id: String },
    /// owner-gated: resume engagement.
    ResumeAgent { agent_id: String },
    /// owner- or governance-gated: remove the agent from the registry and
    /// free its roster slot. notifies the hook target
    /// ([`AgentEvent::Deregistered`]) in the same block, so the agent's
    /// dispatch-plane recipe is retired atomically with the record.
    DeregisterAgent { agent_id: String },
}

// ---- the registry hook ----------------------------------------------------------

/// the registry's follow-up shape, emitted to a genesis-configured hook
/// target (the runs module) in the same block as the registry write that
/// caused it. the hook keeps the agent's dispatch-plane recipe in lockstep:
/// if the recipe registration is rejected (a squatted id), the whole block
/// aborts and the staged registry write vanishes with it — the agent and its
/// recipe stay ONE atomic unit without the registry referencing the dispatch
/// plane.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentEvent {
    /// a new agent landed; the hook registers its recipe.
    Registered {
        agent_id: String,
        capability: String,
    },
    /// an existing agent's capability changed; the hook retunes its recipe.
    CapabilityChanged {
        agent_id: String,
        capability: String,
    },
    /// an agent left the registry; the hook retires its recipe.
    Deregistered { agent_id: String },
}

// ---- queries ------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentQuery {
    Agents,
    Agent { agent_id: String },
}

// the runtime-identity tail grew `AgentRecord` past clippy's 200-byte
// `large_enum_variant` threshold. this is a query REPLY, built rarely and moved
// once through a channel, not a hot per-op allocation — boxing the variant
// would ripple a wire/type change through every reader for no real benefit, so
// the size asymmetry is accepted (mirrors the saga/reachability reply enums).
#[allow(clippy::large_enum_variant)]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentReply {
    Agents(Vec<AgentRecord>),
    Agent(Option<AgentRecord>),
}

// ---- codecs -------------------------------------------------------------------

pub fn encode_msg(m: &AgentMsg) -> Vec<u8> {
    sdk::wire::encode(m)
}
pub fn decode_msg(b: &[u8]) -> Result<AgentMsg, String> {
    sdk::wire::decode(b)
}
pub fn encode_event(e: &AgentEvent) -> Vec<u8> {
    sdk::wire::encode(e)
}
pub fn decode_event(b: &[u8]) -> Result<AgentEvent, String> {
    sdk::wire::decode(b)
}
pub fn encode_response(r: &AgentResponse) -> Vec<u8> {
    sdk::wire::encode(r)
}
pub fn decode_response(b: &[u8]) -> Result<AgentResponse, String> {
    sdk::wire::decode(b)
}
pub fn encode_query(q: &AgentQuery) -> Vec<u8> {
    sdk::wire::encode(q)
}
pub fn decode_query(b: &[u8]) -> Result<AgentQuery, String> {
    sdk::wire::decode(b)
}
pub fn encode_reply(r: &AgentReply) -> Vec<u8> {
    sdk::wire::encode(r)
}
pub fn decode_reply(b: &[u8]) -> Result<AgentReply, String> {
    sdk::wire::decode(b)
}
