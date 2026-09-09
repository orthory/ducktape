//! Model configuration, resource grants, curated skills and external-run actions.
//! A model is compute configuration for a program account; its keyless identity and
//! reaction program are owned by identity and agent.

use borsh::{BorshDeserialize, BorshSerialize};
pub use sdk::Origin as RunOrigin;
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

/// hard cap on a serialized [`ModelRecord`] — registry entries are replicated
/// consensus state, so registration is size-gated up front (at stage time).
pub const MAX_AGENT_RECORD_BYTES: usize = 4 * 1024;

/// Maximum registered model configurations. The roster is replicated state;
/// this count and [`MAX_AGENT_RECORD_BYTES`] bound its size. Programs decide
/// when to request model work through their own attribution handlers.
pub const MAX_REGISTERED_AGENTS: usize = 1024;

/// Maximum model configurations registered by one canonical origin. The
/// allocation follows the stored registration origin, independently of later
/// program-controller transfers. [`MAX_REGISTERED_AGENTS`] bounds the total.
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
/// [`RunsModule::validate_skills`] calls it at consensus time, and
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
/// ([`ModelRecord::library_readable`]).
///
/// it lives HERE, beside the caps that gate it, because three surfaces have to
/// agree on one string: the cap the app grants, the check the run assembler
/// makes before telling an agent the library exists, and the prefix the MCP
/// tool plane is asked to read. NO trailing slash — a cap entry is a path
/// prefix, and `permits` grants children by `"{prefix}/"`, so a trailing slash
/// would grant the directory and none of its contents.
pub const SKILL_LIBRARY_PREFIX: &str = "/shared/skills";

/// Maximum serialized Chat blocks proposed by a model response. Admission
/// bounds the reply before creating its action request; the program's later
/// Chat call still has its own authorization, size checks and target outcome.
pub const MAX_REPLY_BLOCKS_BYTES: usize = 32 * 1024;

/// required byte length of a recipe content-address (a sha256 digest).
pub const RECIPE_HASH_LEN: usize = 32;

/// the reserved unit separator agent ids must never contain: the runs module
/// keys its run records with `\x1f`-delimited fields, and an agent id
/// carrying the delimiter would make those keys ambiguous. the registry
/// rejects it at registration; downstream modules rely on that.
pub const RESERVED_ID_SEPARATOR: char = '\u{1f}';

// ---- the action vocabulary ---------------------------------------------------

/// Permission to post live and final replies in the channel and thread
/// where the agent was engaged. Does not permit posting
/// wherever it likes: see [`ACTION_CHAT_POST_MESSAGE`].
pub const ACTION_CHAT_POST: &str = "chat.post";
/// permission to post a message to an ARBITRARY channel
/// (the `chat.post_message` operation) — a strictly wider grant than
/// [`ACTION_CHAT_POST`], which only ever lets an agent answer where it was
/// spoken to.
///
/// The controller grants arbitrary-channel posting separately from replies.
pub const ACTION_CHAT_POST_MESSAGE: &str = "chat.post_message";
/// permission to create a task (the `tasks.create` operation).
pub const ACTION_TASKS_CREATE: &str = "tasks.create";
/// permission to move a task (the `tasks.update_status` operation).
pub const ACTION_TASKS_UPDATE_STATUS: &str = "tasks.update_status";
/// permission to anchor a comment to a page or block
/// (the `pages.comment` operation).
pub const ACTION_PAGES_COMMENT: &str = "pages.comment";
/// Permission to add a comment to a job.
pub const ACTION_JOBS_COMMENT: &str = "jobs.comment";
/// Permission to flip a todo block's checked state (the `pages.set_checked` operation).
pub const ACTION_PAGES_SET_CHECKED: &str = "pages.set_checked";
/// Permission to publish a new top-level page with its body (the
/// `pages.post` operation). A fresh page has an id no owner could have
/// listed, so the cap side of the gate is the [`EVERY`] entry in
/// `pages_write`.
pub const ACTION_PAGES_POST: &str = "pages.post";
/// permission to write a small UTF-8 text file under a granted duckfs prefix
/// (the `duckfs.write_text` operation).
pub const ACTION_DUCKFS_WRITE_TEXT: &str = "duckfs.write_text";

/// Deploy the component committed by this run after its program accepts the request.
pub const ACTION_MODULES_UPDATE: &str = "modules.update";
/// maximum UTF-8 text payload accepted by the `duckfs.write_text` operation.
pub const MAX_DUCKFS_WRITE_TEXT_BYTES: usize = 4 * 1024;

/// every action name the platform knows. `RegisterModel`/`UpdateModel` reject
/// an `allowed_actions` entry outside this vocabulary, so a granted permission
/// always means something.
///
/// Each action requires an explicit grant in the model configuration.
pub const KNOWN_ACTIONS: [&str; 10] = [
    ACTION_CHAT_POST,
    ACTION_JOBS_COMMENT,
    ACTION_CHAT_POST_MESSAGE,
    ACTION_TASKS_CREATE,
    ACTION_TASKS_UPDATE_STATUS,
    ACTION_PAGES_COMMENT,
    ACTION_PAGES_SET_CHECKED,
    ACTION_PAGES_POST,
    ACTION_DUCKFS_WRITE_TEXT,
    ACTION_MODULES_UPDATE,
];

// ---- runtime identity ---------------------------------------------------------

/// The literal entry in an opaque-id cap list (forge repos, page ids) that
/// grants every id: an owner names "all" without enumerating a set that
/// grows after the grant was written.
pub const EVERY: &str = "*";

/// The resource-capability grant a model carries. Every list is a
/// canonical SORTED + DEDUPED set (the write path canonicalizes, the committed
/// decoder rejects a non-ascending list) so two logically-equal grants hash
/// identically. `secrets` are opaque vault references; their values remain
/// outside consensus. An empty `ResourceCaps` is the default and denies every
/// request, including peer calls.
#[derive(
    BorshSerialize, BorshDeserialize, Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq,
)]
#[serde(deny_unknown_fields)]
pub struct ResourceCaps {
    /// forge repos this agent may READ. repo names are opaque, so matching is
    /// exact, with the one literal entry [`EVERY`] granting every repo.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forge_read: Vec<String>,
    /// forge repos this agent may PUSH to (implies read); [`EVERY`] grants
    /// every repo.
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
    /// Vault references (scoped, opaque). refs only — the value is resolved
    /// host-side and NEVER crosses consensus.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secrets: Vec<String>,
    /// page ids this agent may WRITE (comment on / check off). page ids are
    /// opaque, so matching is exact — no prefix containment — with the one
    /// literal entry [`EVERY`] granting every page.
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
/// the executor auto-loads, in curation order. An `OnDemand` skill is listed by
/// name and description in that document's index and read from its read-only
/// mount when the task calls for it.
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

/// A skill reference the model's runs mount. This pins the reference, not the
/// content: `source_prefix` is a duckfs read-only subtree and
/// `source_snapshot` is its optional consensus pin — `Some` is a PINNED skill
/// (immutable), `None` is a TRACKING skill (the envelope composer resolves the
/// committed head at compose time). the list is ORDERED (later entries override
/// earlier, and `Always` bodies assemble in this order), so it is a `Vec`, not a
/// set — order is significant to the hash.
///
/// The envelope composer reads every field into a skill mount.
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

/// A capability request the runtime probes a [`ModelRecord`] with before
/// applying an effect or opening a sink (the delivery path calls
/// [`ModelRecord::permits`]). the record carries the grant; this is the ask.
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
pub enum ModelStatus {
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
pub enum ModelRole {
    #[default]
    General,
}

/// One model configuration for a program account. Capability and skill grants
/// are consensus state. `owner` records the registration origin for allocation
/// accounting; the program account's live control governs mutations.
///
/// `capability` names WHAT the run needs (an open-set registry tag like
/// "codex" — dispatch selects providers of that tag); HOW it runs — binary,
/// flags, model — is host policy in each provider's capability spec, and
/// consensus never sees it. the record is a recipe, not an executor config.
///
/// Curated `skills` describe model context. [`LoadMode::Always`] includes the
/// skill's body in every run; a snapshot pin fixes the source content, while an
/// unpinned reference follows the committed source head.
#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ModelRecord {
    /// The keyless user this model configuration serves.
    pub account: sdk::AccountNumber,
    pub agent_id: String,
    pub owner: RunOrigin,
    pub display_name: String,
    /// the capability registry tag this agent's runs are dispatched on.
    pub capability: String,
    /// granted action names, sorted and deduped: each from [`KNOWN_ACTIONS`],
    /// or the single [`EVERY`] entry naming all of them — the ones the
    /// vocabulary gains after the grant was written included.
    pub allowed_actions: Vec<String>,
    pub status: ModelStatus,
    #[serde(default, skip_serializing_if = "role_is_default")]
    pub role: ModelRole,
    pub created_at: u64,
    pub updated_at: u64,
    /// Recipe content-address: empty (unset) or exactly [`RECIPE_HASH_LEN`]
    /// bytes. the committed encoding always carries it (empty when unset).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recipe_hash: Vec<u8>,
    /// Resource caps. the committed encoding always carries it (default-empty
    /// when unset).
    #[serde(default, skip_serializing_if = "caps_is_default")]
    pub caps: ResourceCaps,
    /// Ordered skill refs. the committed encoding always carries it (empty
    /// when unset).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<SkillRef>,
}

fn role_is_default(role: &ModelRole) -> bool {
    *role == ModelRole::General
}

impl ModelRecord {
    /// whether this agent holds `action`: by name, or through the [`EVERY`]
    /// grant. THE grant predicate, shared by every lane that admits an
    /// operation.
    pub fn allows(&self, action: &str) -> bool {
        self.allowed_actions
            .iter()
            .any(|granted| granted == action || granted == EVERY)
    }

    /// The capability gate for preparing actions and opening sinks. Empty caps
    /// deny every request; peer calls require a positive budget. Tool and
    /// secret use exact membership; forge repos and pages use exact
    /// membership with the literal [`EVERY`] entry granting every one (ids
    /// are opaque — never a prefix); duckfs uses path-PREFIX containment (a
    /// prefix grants itself and any child path, but never a sibling that
    /// merely shares a textual prefix — `src` does not grant `srcx`). budget
    /// CONSUMPTION is the runtime's concern; this only reads the ceiling.
    pub fn permits(&self, req: &CapRequest) -> bool {
        let c = &self.caps;
        let has = |v: &[String], x: &str| v.iter().any(|s| s == x);
        // an opaque-id list: the exact entry, or the literal `*` naming all.
        let names = |v: &[String], x: &str| has(v, EVERY) || has(v, x);
        let under = |v: &[String], p: &str| {
            v.iter().any(|pre| {
                p == pre || pre == "/" && p.starts_with('/') || p.starts_with(&format!("{pre}/"))
            })
        };
        match req {
            CapRequest::ForgeRead(r) => names(&c.forge_read, r) || names(&c.forge_push, r),
            CapRequest::ForgePush(r) => names(&c.forge_push, r),
            CapRequest::DuckfsWrite(p) => under(&c.duckfs_write, p),
            CapRequest::DuckfsRead(p) => under(&c.duckfs_read, p) || under(&c.duckfs_write, p),
            CapRequest::Tool(t) => has(&c.tools, t),
            CapRequest::Secret(s) => has(&c.secrets, s),
            CapRequest::PagesWrite(p) => names(&c.pages_write, p),
            CapRequest::SpawnSubagent => c.subagent_budget > 0,
        }
    }

    /// whether this agent may READ the global skill library
    /// ([`SKILL_LIBRARY_PREFIX`]) — the one question the host-side run assembler
    /// asks before telling the agent the library is there.
    ///
    /// deliberately [`Self::permits`] and nothing else: the assembled document
    /// tells the agent to run the MCP tool plane's `files.grep` / `files.read`
    /// queries, and those operations gate on exactly this call. a
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
    pub fn scoped_for_call(&self, callee: &ModelRecord) -> ModelRecord {
        let mut scoped = callee.clone();
        scoped.allowed_actions = every_or_exact(&self.allowed_actions, &callee.allowed_actions);
        scoped.caps = self.caps.intersection(&callee.caps);
        let caller = self;
        scoped
            .skills
            .retain(|skill| caller.permits(&CapRequest::DuckfsRead(&skill.source_prefix)));
        scoped
    }
}

/// the intersection of two sorted opaque-name grants, where the [`EVERY`]
/// entry on either side stands for the whole of the other side's list. an
/// action list and the forge and pages cap lists all narrow this way.
pub(crate) fn every_or_exact(left: &[String], right: &[String]) -> Vec<String> {
    let left_names_all = left.iter().any(|name| name == EVERY);
    let right_names_all = right.iter().any(|name| name == EVERY);
    if left_names_all {
        return right.to_vec();
    }
    if right_names_all {
        return left.to_vec();
    }
    left.iter()
        .filter(|value| right.binary_search(value).is_ok())
        .cloned()
        .collect()
}

impl ResourceCaps {
    /// Intersection used by one run-scoped agent call. Exact-name grants use
    /// set intersection. DuckFS prefixes use containment and keep the narrower
    /// prefix. Read authority includes write authority, matching [`ModelRecord::permits`].
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

        Self {
            forge_read: every_or_exact(&forge_readable(self), &forge_readable(other)),
            forge_push: every_or_exact(&self.forge_push, &other.forge_push),
            duckfs_read: prefixes(&readable(self), &readable(other)),
            duckfs_write: prefixes(&self.duckfs_write, &other.duckfs_write),
            tools: exact(&self.tools, &other.tools),
            secrets: exact(&self.secrets, &other.secrets),
            pages_write: every_or_exact(&self.pages_write, &other.pages_write),
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

/// the formal agent response: reply blocks, a bounded list of [`crate::ActionEnvelope`]s,
/// and an optional workspace commit message.
/// lenient by construction — all fields default, unknown JSON fields are
/// ignored — so a model answer either IS this shape or the consumer wraps it
/// as one; validation (grants, caps, probes) is a separate, strict step.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentResponse {
    #[serde(default)]
    pub reply_blocks: Vec<ReplyBlock>,
    #[serde(default)]
    pub actions: Vec<crate::ActionEnvelope>,
    /// complete Git commit message authored by the agent for uncommitted
    /// workspace changes. Optional; a clean response (no workspace changes) omits
    /// it; existing agent commits keep their own messages. The host owns only
    /// safety validation, Git identity, and Forge-title recovery.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_message: Option<String>,
}

/// A conversational destination, resolved from the run's committed source for
/// `reply` and built from an explicit operation's target otherwise.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum ReplyDestination {
    Chat {
        channel_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        thread: Option<u64>,
    },
    Page {
        target: String,
    },
    PageThread {
        thread_id: String,
    },
    Job {
        job_id: String,
    },
}

impl ReplyDestination {
    /// The grant an explicit destination needs. A source-resolved chat reply
    /// needs [`ACTION_CHAT_POST`] instead; the caller decides which applies.
    pub(crate) fn required_action(&self) -> &'static str {
        match self {
            Self::Chat { .. } => ACTION_CHAT_POST_MESSAGE,
            Self::Page { .. } | Self::PageThread { .. } => ACTION_PAGES_COMMENT,
            Self::Job { .. } => ACTION_JOBS_COMMENT,
        }
    }
}

// ---- ops ----------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ModelMsg {
    /// Configure model work for a keyless account, authorized by that account
    /// or its current controller. The model and its dispatch recipe commit
    /// atomically. A duplicate configuration slug is an error.
    RegisterModel {
        account: sdk::AccountNumber,
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
    /// Partial update authorized by the account or its current controller.
    /// None fields retain their values; capability and recipe change atomically.
    UpdateModel {
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
    /// Stop admitting new model work.
    PauseModel { agent_id: String },
    /// Resume model work.
    ResumeModel { agent_id: String },
    /// Remove the configuration and retire its dispatch recipe atomically.
    DeregisterModel { agent_id: String },
}

// ---- queries ------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ModelQuery {
    Agents,
    Agent { agent_id: String },
}

// the runtime-identity tail grew `ModelRecord` past clippy's 200-byte
// `large_enum_variant` threshold. this is a query REPLY, built rarely and moved
// once through a channel, not a hot per-op allocation — boxing the variant
// would ripple a wire/type change through every reader for no real benefit, so
// the size asymmetry is accepted (mirrors the saga/reachability reply enums).
#[allow(clippy::large_enum_variant)]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ModelReply {
    Agents(Vec<ModelRecord>),
    Agent(Option<ModelRecord>),
}

// ---- codecs -------------------------------------------------------------------

pub fn encode_model_msg(m: &ModelMsg) -> Vec<u8> {
    sdk::wire::encode(m)
}
pub fn decode_model_msg(b: &[u8]) -> Result<ModelMsg, String> {
    sdk::wire::decode(b)
}
pub fn encode_response(r: &AgentResponse) -> Vec<u8> {
    sdk::wire::encode(r)
}
pub fn decode_response(b: &[u8]) -> Result<AgentResponse, String> {
    sdk::wire::decode(b)
}
pub fn encode_model_query(q: &ModelQuery) -> Vec<u8> {
    sdk::wire::encode(q)
}
pub fn decode_model_query(b: &[u8]) -> Result<ModelQuery, String> {
    sdk::wire::decode(b)
}
pub fn encode_model_reply(r: &ModelReply) -> Vec<u8> {
    sdk::wire::encode(r)
}
pub fn decode_model_reply(b: &[u8]) -> Result<ModelReply, String> {
    sdk::wire::decode(b)
}
