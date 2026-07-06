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
}

pub fn encode_msg(m: &PageMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}
pub fn decode_msg(b: &[u8]) -> Result<PageMsg, String> {
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
    fn page_meta_carries_parent() {
        let meta = PageMeta { id: "p2".into(), title: "t".into(), parent: Some("p1".into()) };
        let bytes = serde_json::to_vec(&meta).unwrap();
        let back: PageMeta = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(back, meta);
    }
}
