//! the pages module's public wire surface — types only, no logic, no sdk dep.
//!
//! a page is a TREE of [`Block`]s (notion's model, simplified): the page itself
//! is the root block, every block carries an ordered `children` list, and every
//! block id is GLOBALLY UNIQUE within the module — not merely unique inside its
//! page. that global uniqueness is the addressability contract: a block is
//! resolvable by id alone ([`PageQuery::GetBlock`] takes no page context), so a
//! reference to a block can be held by anything that can later ask the pages
//! module about it. a consumer that writes pages depends on THIS crate, never
//! on the pages impl.

use serde::{Deserialize, Serialize};

/// the kind of a block. `Page` is a kind like any other (a page IS a block),
/// but only [`PageMsg::CreatePage`] may mint one — block ops that try to
/// insert or convert to `Page` are rejected.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BlockKind {
    Page,
    Paragraph,
    Heading1,
    Heading2,
    Heading3,
    Bulleted,
    Numbered,
    Todo,
    Toggle,
    Quote,
    Code,
    Callout,
    Divider,
}

/// one block of a page, as stored and as returned by queries.
///
/// the tree shape lives here: `parent` points up (None only for a page root),
/// `children` is the ordered list of ids below, and `page` names the root
/// block of the page this block belongs to (a root names itself). `page` and
/// `parent` are DERIVED by the module on insert/move — writers never supply
/// them (see [`NewBlock`]).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// globally unique within the module — the addressable handle.
    pub id: String,
    /// the parent block id; `None` only for a page root.
    pub parent: Option<String>,
    /// the page (root block id) this block belongs to; a root names itself.
    pub page: String,
    pub kind: BlockKind,
    /// the text payload — the page title for `Page`, empty for `Divider`.
    pub text: String,
    /// only meaningful for `Todo` (false everywhere else).
    pub checked: bool,
    /// ordered child block ids.
    pub children: Vec<String>,
}

/// the insert payload: a client-minted globally-unique id plus kind and text.
/// `parent`/`page`/`children` are derived by the module from the insert
/// position; `checked` starts false ([`PageMsg::SetChecked`] flips it).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct NewBlock {
    pub id: String,
    pub kind: BlockKind,
    pub text: String,
}

/// a stable pointer to one block in one pages module — the shape a FUTURE
/// cross-module reference carries. resolution is already possible today:
/// `Ctx::query(module, PageQuery::GetBlock { block_id: block })` answers with
/// the live block (or `None` once it was removed — a ref can dangle, exactly
/// like a hyperlink). serializable so other modules can embed it in their own
/// state now, before any shared reference machinery exists.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct BlockRef {
    /// the ModuleId of the pages module instance (e.g. "pages").
    pub module: String,
    /// the globally-unique block id inside that module.
    pub block: String,
}

/// write intents the pages module accepts (its `execute` payload).
///
/// `after` positioning rule (SAME in `InsertBlock` and `MoveBlock`): `None` ==
/// "first child of `parent`"; `Some(id)` == "immediately after that sibling"
/// (the anchor must be a child of `parent`, else the op errors).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PageMsg {
    /// create a page: a root block of kind `Page` whose text is `title`.
    /// `parent`, when `Some`, nests this page under another page (a folder
    /// relation stored only in the enumeration index — content blocks are
    /// untouched). idempotent: re-creating an existing page is a benign no-op
    /// that changes neither the title NOR the parent. `page_id` is a block id
    /// and shares the global-uniqueness rule.
    CreatePage {
        page_id: String,
        title: String,
        parent: Option<String>,
    },
    /// insert `block` under `parent` after the given sibling anchor (see the
    /// `after` rule). the parent may be the page root or any block — nesting
    /// is what makes toggles/indent work. rejected when `block.kind` is
    /// `Page` (pages come only from `CreatePage`).
    InsertBlock {
        parent: String,
        after: Option<String>,
        block: NewBlock,
    },
    /// replace a block's text. on a page root this renames the page.
    UpdateText { block_id: String, text: String },
    /// convert a block to another kind (markdown-shortcut conversions). both
    /// converting TO `Page` and converting a page root away are rejected.
    SetKind { block_id: String, kind: BlockKind },
    /// flip a `Todo` block's checked state. rejected on any other kind.
    SetChecked { block_id: String, checked: bool },
    /// move a block under a (possibly new) parent within the SAME page (see
    /// the `after` rule). rejected on page roots, across pages, and when the
    /// new parent sits inside the moved block's own subtree.
    MoveBlock {
        block_id: String,
        parent: String,
        after: Option<String>,
    },
    /// remove a block AND its whole subtree. rejected on page roots.
    RemoveBlock { block_id: String },
    /// re-nest a page under a (possibly new) parent page, or to top level with
    /// `None`. rejected when the target is not a page root, the parent is not a
    /// page, or the move would form a cycle in the folder forest.
    SetPageParent {
        page_id: String,
        parent: Option<String>,
    },
    /// delete a page: remove its root and whole block subtree, and PROMOTE its
    /// direct child pages to the deleted page's parent (no cascade). rejected
    /// when the id is not a page root.
    DeletePage { page_id: String },

    // ── comments ──
    // a comment thread anchors to a `target` (a block id or a page id in THIS
    // module). authorship is derived from the dispatch origin, never a payload
    // (mirrors the chat module). ids are client-minted like block ids.
    /// open a thread (when `thread_id` is new) anchored to `target` with this
    /// first comment, or append `comment_id` to an existing thread (whose
    /// target must match). author = origin.
    AddComment {
        thread_id: String,
        comment_id: String,
        target: String,
        text: String,
    },
    /// replace a comment's text; stored-author-only. rejected on a tombstone.
    EditComment { comment_id: String, text: String },
    /// tombstone a comment; stored-author-only. when it was the thread's last
    /// live comment, the whole thread record is removed.
    DeleteComment { comment_id: String },
    /// toggle a thread's resolved flag; records the resolver as origin.
    ResolveThread { thread_id: String, resolved: bool },

    // ── hooks ──
    // subscribe/unsubscribe a module to this module's [`PageEvent`] fan-out.
    // the payloads are EMPTY on purpose: the subscriber derives from the
    // EMITTING module's origin (spoof-proof, the tagging idiom), so only a
    // module can subscribe — and only ITSELF.
    /// subscribe the emitting module. module-origin only; idempotent; capped
    /// at [`MAX_PAGE_HOOKS`]; the pages module itself is rejected.
    RegisterHook {},
    /// unsubscribe the emitting module. module-origin only; absent is a
    /// deterministic no-op.
    UnregisterHook {},
}

// write-time caps for comments (consensus constants) — enforced before staging.
pub const MAX_COMMENT_TEXT_BYTES: usize = 64 * 1024;
pub const MAX_COMMENTS_PER_THREAD: usize = 4096;
pub const MAX_THREADS_PER_TARGET: usize = 1024;
pub const MAX_QUERY_TARGETS: usize = 512;

/// hard cap on every externally-supplied pages id: [`PageMsg::CreatePage`]'s
/// `page_id`, [`NewBlock::id`], and [`PageMsg::AddComment`]'s `thread_id` /
/// `comment_id` / `target` — enforced at creation/first-use (see
/// `crate::limits::check_id_len`), before any storage touch. this is a
/// flag-day change to an unmerged module: no migration, no pre-cap state to
/// grandfather. matches the platform-wide `MAX_ID_BYTES` convention already
/// used by `dispatch` and `tagging` (both 128).
///
/// ## the arithmetic
///
/// this repo's reference packaged module, `docs-harness`, embeds pages ids
/// VERBATIM in a job id shaped `docs:<agent_id>:<comment_id>`
/// (`docs_harness::engagement_job_id`), submitted to the jobs board capped at
/// `jobs::MAX_JOB_ID` = 256 bytes. the literal framing (`"docs:"` + `":"`) is
/// 6 bytes.
///
/// `agent_id` is NOT bounded by `agent::MAX_AGENT_RECORD_BYTES` in any tight
/// way (that caps the WHOLE registration record, 4 KiB, of which `agent_id`
/// is only one field) — but `runs`' registry hook re-derives the agent's
/// dispatch recipe id as `format!("agent/{agent_id}")` and REJECTS the
/// registration outright when that exceeds `dispatch::MAX_ID_BYTES` (128).
/// that is a hard, already-enforced consensus invariant: no agent can ever
/// exist with `agent_id.len() > 128 - "agent/".len() == 122`.
///
/// so the worst case is: `6 ("docs:" + ":") + 122 (max agent_id) +
/// comment_id.len() <= 256`, i.e. `comment_id.len() <= 128`. capping every
/// pages id at exactly 128 makes the WORST-CASE job id exactly 256 bytes —
/// fits `jobs::MAX_JOB_ID` with zero slack to spare, for any agent id the
/// network could ever register. (the docs-harness intake pre-check is the
/// belt to this suspenders: it re-measures the ASSEMBLED job id / spec
/// against the jobs caps at submit time, which also covers state from before
/// this cap existed, or a foreign module that skips it.)
pub const MAX_ID_BYTES: usize = 128;

/// cap on the module-wide [`PageMsg::RegisterHook`] subscriber set (consensus
/// constant) — every extra hook is one more same-block follow-up per write.
pub const MAX_PAGE_HOOKS: usize = 8;

/// who authored a comment — derived from `Env.origin`, never a payload. own
/// copy of chat's shape (each module's interface is self-contained).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum AuthorRef {
    User(Vec<u8>),
    Agent { module: String, agent_id: String },
    Module(String),
    System,
}

/// a comment thread: a `target` (block or page id), its opener, resolve state,
/// and the ordered ids of its comments (tombstoned comments stay listed until
/// the whole thread is removed on last-live-delete).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Thread {
    pub id: String,
    pub target: String,
    pub opener: AuthorRef,
    pub created_at: u64,
    pub resolved: bool,
    pub resolved_by: Option<AuthorRef>,
    pub comment_ids: Vec<String>,
}

/// one comment. `deleted` tombstones content but keeps the record so ordering
/// and the thread skeleton survive until the thread is removed.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    pub id: String,
    pub thread_id: String,
    pub author: AuthorRef,
    pub text: String,
    pub created_at: u64,
    pub edited_at: Option<u64>,
    pub deleted: bool,
}

/// a thread plus its live (non-tombstoned) comments in order.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ThreadView {
    pub thread: Thread,
    pub comments: Vec<Comment>,
}

/// the threads anchored to one target.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct TargetThreads {
    pub target: String,
    pub threads: Vec<ThreadView>,
}

pub fn encode_msg(m: &PageMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}
pub fn decode_msg(b: &[u8]) -> Result<PageMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

/// what the pages module tells its registered hooks: one follow-up `Msg` per
/// subscriber, emitted in the SAME block as (and only after) the triggering
/// write, so the write and every notification commit or abort as one atomic
/// unit. receivers own no-fail handling — a hook that errors on decode poisons
/// the writer's block.
///
/// `page_id` is the page containing the touched target, derived by the module
/// (a page-anchored comment names the page itself); it is EMPTY when a comment
/// anchors to a target that does not resolve to a live block (a dangling
/// anchor, exactly like a hyperlink).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PageEvent {
    /// a comment was added — a new thread's opener or an append.
    CommentAdded {
        page_id: String,
        target: String,
        thread_id: String,
        comment_id: String,
        author: AuthorRef,
        text: String,
    },
    /// a thread's resolved flag was toggled.
    ThreadResolved {
        page_id: String,
        thread_id: String,
        resolved: bool,
    },
    /// a block's text was replaced (a page root's text is its title).
    BlockUpdated { page_id: String, block_id: String },
}

pub fn encode_page_event(e: &PageEvent) -> Vec<u8> {
    serde_json::to_vec(e).expect("serializable")
}
pub fn decode_page_event(b: &[u8]) -> Result<PageEvent, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

/// read requests the pages module serves via `Module::query`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PageQuery {
    /// the whole page as its blocks in PREORDER (root first, each block's
    /// subtree before its next sibling). `None` == no page at that id.
    GetPage { page_id: String },
    /// a single block by id ALONE — no page context needed. this is the
    /// cross-module resolution surface a [`BlockRef`] points at; the returned
    /// block carries its `page` and `parent`, so a resolver learns where the
    /// block lives, not just what it says.
    GetBlock { block_id: String },
    /// enumerate every page, served from the module's reserved index entry
    /// (sorted by id), with titles read from the live roots.
    ListPages,
    /// every thread anchored to any of `targets` (block or page ids), grouped
    /// by target. a page render calls this once with all visible block ids +
    /// the page id. `targets` beyond [`MAX_QUERY_TARGETS`] are rejected.
    ThreadsForTargets { targets: Vec<String> },
    /// one thread with its live comments.
    CommentThread { thread_id: String },
    /// a single comment by id ALONE — comment ids are globally unique within
    /// the module (stored under one reserved keyspace, whatever thread they
    /// belong to), so this is the existence probe an action owner runs
    /// against a minted comment id before staging an `AddComment` follow-up.
    /// a TOMBSTONED comment is still returned (its id is still taken);
    /// `None` means the id is free.
    GetComment { comment_id: String },
}

/// one entry of [`PageReply::PageList`]: a page id and its current title.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct PageMeta {
    pub id: String,
    pub title: String,
    /// the containing page id (folder parent), or `None` for a top-level page.
    pub parent: Option<String>,
}

/// replies to a [`PageQuery`]. `Option` mirrors absence.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PageReply {
    Page(Option<Vec<Block>>),
    Block(Option<Block>),
    PageList(Vec<PageMeta>),
    CommentThreads(Vec<TargetThreads>),
    CommentThread(Option<ThreadView>),
    Comment(Option<Comment>),
}

pub fn encode_query(q: &PageQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}
pub fn decode_query(b: &[u8]) -> Result<PageQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_reply(r: &PageReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}
pub fn decode_reply(b: &[u8]) -> Result<PageReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

#[cfg(test)]
mod interface_tests {
    use super::*;

    #[test]
    fn create_page_carries_optional_parent() {
        let m = PageMsg::CreatePage {
            page_id: "p2".into(),
            title: "child".into(),
            parent: Some("p1".into()),
        };
        let round: PageMsg = decode_msg(&encode_msg(&m)).unwrap();
        assert_eq!(round, m);
        // top-level create serializes parent as null.
        let top = PageMsg::CreatePage {
            page_id: "p1".into(),
            title: "root".into(),
            parent: None,
        };
        assert!(
            String::from_utf8(encode_msg(&top))
                .unwrap()
                .contains("\"parent\":null")
        );
    }

    #[test]
    fn set_parent_and_delete_round_trip() {
        for m in [
            PageMsg::SetPageParent {
                page_id: "p2".into(),
                parent: None,
            },
            PageMsg::DeletePage {
                page_id: "p2".into(),
            },
        ] {
            assert_eq!(decode_msg(&encode_msg(&m)).unwrap(), m);
        }
    }

    #[test]
    fn page_meta_carries_parent() {
        let meta = PageMeta {
            id: "p2".into(),
            title: "t".into(),
            parent: Some("p1".into()),
        };
        let bytes = serde_json::to_vec(&meta).unwrap();
        let back: PageMeta = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(back, meta);
    }

    #[test]
    fn hook_msgs_and_page_events_round_trip() {
        // the registration payloads are EMPTY — the subscriber is the origin.
        for m in [PageMsg::RegisterHook {}, PageMsg::UnregisterHook {}] {
            assert_eq!(decode_msg(&encode_msg(&m)).unwrap(), m);
        }
        for e in [
            PageEvent::CommentAdded {
                page_id: "p1".into(),
                target: "b1".into(),
                thread_id: "t1".into(),
                comment_id: "c1".into(),
                author: AuthorRef::User(vec![7; 32]),
                text: "hi".into(),
            },
            PageEvent::ThreadResolved {
                page_id: "p1".into(),
                thread_id: "t1".into(),
                resolved: true,
            },
            PageEvent::BlockUpdated {
                page_id: "p1".into(),
                block_id: "b1".into(),
            },
        ] {
            assert_eq!(decode_page_event(&encode_page_event(&e)).unwrap(), e);
        }
    }
}
