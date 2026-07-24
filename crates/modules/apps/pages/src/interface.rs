//! the pages module's public wire surface — types plus thin `sdk::wire` codec
//! delegates.
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

/// Inline formatting applied to a UTF-16 span of a block's text. UTF-16 is
/// deliberate: browser selection offsets use UTF-16 code units, so the wire
/// range is exactly what the editor reports even around emoji.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum InlineMark {
    Bold,
    Italic,
    Underline,
    Strikethrough,
    Code,
}

/// One half-open inline mark range (`start..end`) in UTF-16 code units.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SpanMark {
    pub start: u32,
    pub end: u32,
    pub kind: InlineMark,
}

/// A half-open comment selection range in UTF-16 code units. The module
/// rebases both endpoints whenever the target block's text changes, so this
/// remains relative to the selected text instead of becoming a stale absolute
/// character offset.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RelativeAnchor {
    pub start: u32,
    pub end: u32,
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
    /// Persistent inline formatting. Omitted from the wire when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub marks: Vec<SpanMark>,
    /// only meaningful for `Todo` (false everywhere else).
    pub checked: bool,
    /// ordered child block ids.
    pub children: Vec<String>,
}

/// the insert payload: a client-minted globally-unique id plus content.
/// `parent`/`page`/`children` are derived by the module from the insert
/// position; `checked` starts false ([`PageMsg::SetChecked`] flips it).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct NewBlock {
    pub id: String,
    pub kind: BlockKind,
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub marks: Vec<SpanMark>,
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
    UpdateText {
        block_id: String,
        text: String,
        /// A split/merge can replace content + marks atomically. Omitted (the
        /// common plain edit) leaves the block's existing marks to rebase.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        marks: Option<Vec<SpanMark>>,
    },
    /// Apply or remove one inline mark over an exact UTF-16 range. Applying
    /// merges adjacent/overlapping spans of the same kind; removing splits
    /// spans when needed.
    SetSpanMark {
        block_id: String,
        start: u32,
        end: u32,
        kind: InlineMark,
        active: bool,
    },
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
    /// target must match). author = origin — except `as_agent`, which refines
    /// a MODULE origin into `AuthorRef::Agent { module, agent_id }` (the
    /// module half stays origin-derived and spoof-proof; mirrors chat's
    /// `as_agent`). `as_agent` with a non-module origin is rejected.
    AddComment {
        thread_id: String,
        comment_id: String,
        target: String,
        text: String,
        /// Exact selection for a new thread. Replies and block-level comments
        /// omit it (a comment with no text selection).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        anchor: Option<RelativeAnchor>,
        /// Structured mentions carried by this comment. Only agent refs are
        /// translated into tagging-plane entities; omitted when the comment
        /// mentions no one.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        mentions: Vec<AuthorRef>,
        /// Present only for an agent-authored comment; omitted for a human
        /// author (see `PostMessage::as_agent`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        as_agent: Option<String>,
    },
    /// Move a thread with text that crossed a block boundary during split or
    /// merge. The replacement anchor is validated against the new target.
    MoveCommentThread {
        thread_id: String,
        target: String,
        #[serde(default)]
        anchor: Option<RelativeAnchor>,
    },
    /// replace a comment's text; stored-author-only. rejected on a tombstone.
    /// `mentions` carries only refs newly introduced by this edit, so an
    /// unrelated wording change cannot re-engage everyone already mentioned.
    EditComment {
        comment_id: String,
        text: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        mentions: Vec<AuthorRef>,
    },
    /// tombstone a comment; stored-author-only. when it was the thread's last
    /// live comment, the whole thread record is removed.
    DeleteComment { comment_id: String },
    /// toggle a thread's resolved flag; records the resolver as origin.
    ResolveThread { thread_id: String, resolved: bool },
}

// write-time caps for comments (consensus constants) — enforced before staging.
pub const MAX_COMMENT_TEXT_BYTES: usize = 64 * 1024;
pub const MAX_COMMENTS_PER_THREAD: usize = 4096;
pub const MAX_THREADS_PER_TARGET: usize = 1024;
pub const MAX_QUERY_TARGETS: usize = 512;
pub const MAX_SPAN_MARKS_PER_BLOCK: usize = 4096;

// client-minted id length caps (consensus constants). the shared DERIVED
// blocks — the per-target thread index (a `Vec<thread_id>`, up to
// `MAX_THREADS_PER_TARGET`) and a thread record (a `Vec<comment_id>`, up to
// `MAX_COMMENTS_PER_THREAD`) — grow with these ids. WITHOUT a length cap a
// user (AddComment needs no capability) can pre-bloat a target's index with
// long ids until one more append trips `MAX_BLOCK_LEN` at stage time and
// ABORTS the block (a permanent-re-abort R4 wedge). these caps keep those
// derived blocks safely under `MAX_BLOCK_LEN` (768 KiB) at full count (JSON
// overhead ≈ 3 B/entry): 1024 × (512+3) ≈ 515 KiB and 4096 × (128+3) ≈ 524
// KiB, both a comfortable ~250 KiB clear.
pub const MAX_THREAD_ID_BYTES: usize = 512;
pub const MAX_COMMENT_ID_BYTES: usize = 128;
pub const MAX_COMMENT_TARGET_BYTES: usize = 512;

/// whether a client-minted id serializes 1:1 (byte-for-byte) under
/// `serde_json` — i.e. carries no escaping char. `serde_json` escapes `"`→
/// `\"` (2 B), `\`→`\\` (2 B), and control chars `< 0x20` → `\u00XX` (6 B),
/// so a length-capped id built from control chars could still balloon a
/// derived block past [`MAX_BLOCK_LEN`] and abort it. every OTHER char (incl.
/// non-ASCII UTF-8, `/`, `:`) serializes to exactly its UTF-8 byte length, so
/// with escaping chars rejected `String::len()` bounds the serialized cost
/// exactly and the count × length caps hold. legit ids (uuids, path/hex
/// forms) never contain these chars.
pub fn id_is_index_safe(s: &str) -> bool {
    !s.chars().any(|c| c == '"' || c == '\\' || (c as u32) < 0x20)
}

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
    /// `None` is a block/page-level thread; `Some` pins it to exact text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<RelativeAnchor>,
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

pub fn encode_msg(m: &PageMsg) -> Vec<u8> {
    sdk::wire::encode(m)
}
pub fn decode_msg(b: &[u8]) -> Result<PageMsg, String> {
    sdk::wire::decode(b)
}

/// the DISPATCH read surface — the point reads other modules' `execute()`
/// paths resolve through `Ctx::query` (runs' block/comment probes and page
/// context assembly). UI-shaped enumeration (the page list, per-target
/// thread panels, search) is served by pages' index guest on the derived
/// tier instead.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PageQuery {
    /// the whole page as its blocks in PREORDER (root first, each block's
    /// subtree before its next sibling). `None` == no page at that id.
    GetPage { page_id: String },
    /// a single block by id ALONE — no page context needed. this is the
    /// cross-module resolution surface; the returned block carries its `page`
    /// and `parent`, so a resolver learns where the block lives, not just
    /// what it says.
    GetBlock { block_id: String },
    /// one thread with its live comments.
    CommentThread { thread_id: String },
    /// one comment by id, tombstones included — the existence probe a module
    /// emitting `AddComment` follow-ups uses (comment ids are client-minted,
    /// so a squatted id would otherwise reject the follow-up and abort its
    /// block). `None` == no comment record at that id.
    GetComment { comment_id: String },
    /// how many threads anchor to one target — the [`MAX_THREADS_PER_TARGET`]
    /// cap probe a module staging `AddComment` follow-ups runs. a count off
    /// the target's thread-index record, deliberately NOT the thread views
    /// (those are the index guest's `threads_for_targets`).
    TargetThreadCount { target: String },
}

/// replies to a [`PageQuery`]. `Option` mirrors absence.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PageReply {
    Page(Option<Vec<Block>>),
    Block(Option<Block>),
    CommentThread(Option<ThreadView>),
    Comment(Option<Comment>),
    TargetThreadCount(u64),
}

pub fn encode_query(q: &PageQuery) -> Vec<u8> {
    sdk::wire::encode(q)
}
pub fn decode_query(b: &[u8]) -> Result<PageQuery, String> {
    sdk::wire::decode(b)
}
pub fn encode_reply(r: &PageReply) -> Vec<u8> {
    sdk::wire::encode(r)
}
pub fn decode_reply(b: &[u8]) -> Result<PageReply, String> {
    sdk::wire::decode(b)
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
        assert!(String::from_utf8(encode_msg(&top)).unwrap().contains("\"parent\":null"));
    }

    #[test]
    fn set_parent_and_delete_round_trip() {
        for m in [
            PageMsg::SetPageParent { page_id: "p2".into(), parent: None },
            PageMsg::DeletePage { page_id: "p2".into() },
        ] {
            assert_eq!(decode_msg(&encode_msg(&m)).unwrap(), m);
        }
    }

    #[test]
    fn target_thread_count_round_trips() {
        // the cap-probe read a module staging AddComment follow-ups runs.
        let q = PageQuery::TargetThreadCount { target: "b1".into() };
        assert_eq!(decode_query(&encode_query(&q)).unwrap(), q);
        let r = PageReply::TargetThreadCount(7);
        assert_eq!(decode_reply(&encode_reply(&r)).unwrap(), r);
    }

    #[test]
    fn edit_comment_without_mentions_decodes() {
        // the exact wire an edit that adds no mentions emits (skip_if_empty).
        let wire = br#"{"edit_comment":{"comment_id":"c1","text":"reworded"}}"#;
        assert_eq!(
            decode_msg(wire).unwrap(),
            PageMsg::EditComment {
                comment_id: "c1".into(),
                text: "reworded".into(),
                mentions: Vec::new(),
            }
        );
    }

    #[test]
    fn omitted_optional_fields_decode_to_defaults() {
        let block: Block = serde_json::from_slice(
            br#"{"id":"b1","parent":"p1","page":"p1","kind":"paragraph","text":"hello","checked":false,"children":[]}"#,
        )
        .unwrap();
        assert!(block.marks.is_empty());
        let thread: Thread = serde_json::from_slice(
            br#"{"id":"t1","target":"b1","opener":"system","created_at":1,"resolved":false,"resolved_by":null,"comment_ids":[]}"#,
        )
        .unwrap();
        assert!(thread.anchor.is_none());
        assert_eq!(
            decode_msg(br#"{"update_text":{"block_id":"b1","text":"next"}}"#).unwrap(),
            PageMsg::UpdateText {
                block_id: "b1".into(),
                text: "next".into(),
                marks: None,
            }
        );

        // a reply / block-level comment (the runs agent emits exactly this).
        let wire = br#"{"add_comment":{"thread_id":"t1","comment_id":"c1","target":"b1","text":"note"}}"#;
        let PageMsg::AddComment { anchor, mentions, as_agent, .. } = decode_msg(wire).unwrap()
        else {
            panic!("expected AddComment")
        };
        assert!(anchor.is_none());
        assert!(mentions.is_empty());
        assert!(as_agent.is_none());
    }
}
