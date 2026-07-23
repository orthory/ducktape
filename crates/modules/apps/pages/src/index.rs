//! pages' materialized view: full-text search over the block tree.
//!
//! canonical pages state serves whole pages (preorder) and blocks by id; it
//! cannot search. this mapper folds applied [`PageMsg`] ops into a token
//! index and serves `search` as pages' endpoint on the derived tier.
//!
//! block ids are globally unique (the module's addressability contract), so
//! rows key on the id alone. the fold mirrors just enough of the tree to stay
//! correct under `RemoveBlock` — which removes a whole SUBTREE — by keeping
//! each row's child-id set (membership only; sibling ORDER is not mirrored,
//! search never needs it).
//!
//! key spaces (inside pages' per-module index database):
//! - `blk/{block_id}`         — the block's current [`PageBlockRow`].
//! - `tok/{token}/{block_id}` — one posting per (token, block), value =
//!   [`TokRef`]; rewritten whole on every text change.
//!
//! this file is the DECISION core — pure functions over [`StateRead`],
//! compiled natively and unit-tested against a plain map. the wasm shell
//! (`src/index_guest.rs`, feature `index-guest`) wires it into the engine.
//! within one op a read never sees that op's own writes (they apply after
//! the decision); across ops in one feed batch it sees everything earlier —
//! identical in the engine transaction and the native test harness.

use index_guest::search::{self, DEFAULT_POSTING_CAP};
use index_guest::{Fail, OpRow, StateRead, Writes};
use serde::{Deserialize, Serialize};

use crate::{BlockKind, PageMsg, decode_msg};

const DEFAULT_SEARCH_LIMIT: usize = 20;
const MAX_SEARCH_LIMIT: usize = 100;

/// [`Fail`] code: an applied op's payload did not decode.
const FAIL_OP_DECODE: i32 = 2;
/// [`Fail`] code: a stored row did not decode — a damaged read model.
const FAIL_ROW_DECODE: i32 = 3;
/// [`Fail`] code: a view request this mapper does not speak.
const FAIL_BAD_REQUEST: i32 = 4;

/// the stored row of one page block, as search results return it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageBlockRow {
    pub block_id: String,
    /// the page (root block id) this block belongs to; a root names itself.
    pub page_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    pub kind: BlockKind,
    pub text: String,
    /// child block ids, membership only (order lives in canonical state).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<String>,
    pub height: u64,
    pub time: u64,
}

/// a token posting's value: rank (time) plus the row address and its page.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct TokRef {
    block_id: String,
    page_id: String,
    time: u64,
}

/// pages' view requests, externally tagged:
/// `{"search": {"text": "...", "page_id": "...", "limit": 20}}`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PagesViewQuery {
    Search {
        text: String,
        #[serde(default)]
        page_id: Option<String>,
        #[serde(default)]
        limit: Option<usize>,
    },
}

/// pages' view replies: `{"hits": [<PageBlockRow>…]}`, newest first.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PagesViewReply {
    Hits(Vec<PageBlockRow>),
}

fn blk_key(id: &str) -> String {
    format!("blk/{id}")
}

fn tok_key(token: &str, id: &str) -> String {
    format!("tok/{token}/{id}")
}

fn read_row(read: &impl StateRead, id: &str) -> Result<Option<PageBlockRow>, Fail> {
    let Some(bytes) = read.get(blk_key(id).as_bytes()) else {
        return Ok(None);
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|e| Fail::new(FAIL_ROW_DECODE, e.to_string()))
}

fn encode_row(row: &PageBlockRow) -> Result<Vec<u8>, Fail> {
    serde_json::to_vec(row).map_err(|e| Fail::new(FAIL_ROW_DECODE, e.to_string()))
}

fn put_row(out: &mut Writes, row: &PageBlockRow) -> Result<(), Fail> {
    index_guest::put(out, blk_key(&row.block_id), encode_row(row)?);
    Ok(())
}

/// stage a row plus one posting per token, so every write path produces
/// byte-identical entries.
fn put_row_and_toks(out: &mut Writes, row: &PageBlockRow) -> Result<(), Fail> {
    put_row(out, row)?;
    let tok_ref = serde_json::to_vec(&TokRef {
        block_id: row.block_id.clone(),
        page_id: row.page_id.clone(),
        time: row.time,
    })
    .map_err(|e| Fail::new(FAIL_ROW_DECODE, e.to_string()))?;
    for token in search::tokens(&row.text) {
        index_guest::put(out, tok_key(&token, &row.block_id), tok_ref.clone());
    }
    Ok(())
}

fn delete_toks(out: &mut Writes, row: &PageBlockRow) {
    for token in search::tokens(&row.text) {
        index_guest::delete(out, tok_key(&token, &row.block_id));
    }
}

/// drop a whole subtree depth-first — rows and postings both. a child this
/// index never saw is skipped (the mirror only holds what was folded).
fn delete_subtree(read: &impl StateRead, out: &mut Writes, root: PageBlockRow) -> Result<(), Fail> {
    let mut stack = vec![root];
    while let Some(row) = stack.pop() {
        delete_toks(out, &row);
        index_guest::delete(out, blk_key(&row.block_id));
        for child in &row.children {
            if let Some(child_row) = read_row(read, child)? {
                stack.push(child_row);
            }
        }
    }
    Ok(())
}

/// fold one applied op into derived writes.
pub fn fold_op(op: &OpRow, read: &impl StateRead) -> Result<Writes, Fail> {
    let msg = decode_msg(&op.payload).map_err(|e| Fail::new(FAIL_OP_DECODE, e))?;
    let mut out = Writes::new();
    match msg {
        PageMsg::CreatePage {
            page_id,
            title,
            parent: _,
        } => {
            // idempotence mirror: re-creating an existing page is a no-op
            // that does NOT overwrite the title. the folder parent is not
            // searchable, so the index ignores it.
            if read_row(read, &page_id)?.is_some() {
                return Ok(out);
            }
            let row = PageBlockRow {
                page_id: page_id.clone(),
                block_id: page_id,
                parent: None,
                kind: BlockKind::Page,
                text: title,
                children: Vec::new(),
                height: op.height,
                time: op.time,
            };
            put_row_and_toks(&mut out, &row)?;
        }
        PageMsg::InsertBlock { parent, block, .. } => {
            // the page is derived from the parent — a parent this index
            // never saw (pre-index tree) leaves the whole insert out.
            let Some(mut parent_row) = read_row(read, &parent)? else {
                return Ok(out);
            };
            let row = PageBlockRow {
                block_id: block.id.clone(),
                page_id: parent_row.page_id.clone(),
                parent: Some(parent.clone()),
                kind: block.kind,
                text: block.text,
                children: Vec::new(),
                height: op.height,
                time: op.time,
            };
            put_row_and_toks(&mut out, &row)?;
            parent_row.children.push(block.id);
            put_row(&mut out, &parent_row)?;
        }
        PageMsg::UpdateText { block_id, text, .. } => {
            let Some(mut row) = read_row(read, &block_id)? else {
                return Ok(out);
            };
            // delete BEFORE re-putting: tokens shared by the old and new
            // text stage a delete then a put, and the last command wins.
            delete_toks(&mut out, &row);
            row.text = text;
            row.height = op.height;
            row.time = op.time;
            put_row_and_toks(&mut out, &row)?;
        }
        PageMsg::SetKind { block_id, kind } => {
            let Some(mut row) = read_row(read, &block_id)? else {
                return Ok(out);
            };
            row.kind = kind;
            put_row(&mut out, &row)?;
        }
        PageMsg::MoveBlock {
            block_id, parent, ..
        } => {
            // same-page move (the module rejects the rest): re-home the
            // membership edge, page and text unchanged.
            let Some(mut row) = read_row(read, &block_id)? else {
                return Ok(out);
            };
            // a same-parent move is a sibling reorder: membership is
            // unchanged and this index does not mirror order (the module
            // special-cases it too). re-reading the parent below would see
            // the pre-op row (this op's writes apply after the decision)
            // and re-push a duplicate child — so: no-op.
            if row.parent.as_deref() == Some(parent.as_str()) {
                return Ok(out);
            }
            if let Some(old_parent) = &row.parent
                && let Some(mut old) = read_row(read, old_parent)?
            {
                old.children.retain(|c| c != &block_id);
                put_row(&mut out, &old)?;
            }
            if let Some(mut new_parent) = read_row(read, &parent)? {
                new_parent.children.push(block_id.clone());
                put_row(&mut out, &new_parent)?;
            }
            row.parent = Some(parent);
            put_row(&mut out, &row)?;
        }
        PageMsg::RemoveBlock { block_id } => {
            let Some(row) = read_row(read, &block_id)? else {
                return Ok(out);
            };
            // unhook from the parent's membership set…
            if let Some(parent) = &row.parent
                && let Some(mut parent_row) = read_row(read, parent)?
            {
                parent_row.children.retain(|c| c != &block_id);
                put_row(&mut out, &parent_row)?;
            }
            // …then drop the whole subtree, rows and postings both.
            delete_subtree(read, &mut out, row)?;
        }
        // checked state carries no searchable text.
        PageMsg::SetChecked { .. } | PageMsg::SetSpanMark { .. } => {}
        // comments live in a reserved keyspace, not the block tree — no
        // searchable block row changes (a future pass could index them).
        PageMsg::AddComment { .. }
        | PageMsg::MoveCommentThread { .. }
        | PageMsg::EditComment { .. }
        | PageMsg::DeleteComment { .. }
        | PageMsg::ResolveThread { .. } => {}
        // folder nesting carries no searchable text — the block tree (and
        // thus every row) is unchanged.
        PageMsg::SetPageParent { .. } => {}
        PageMsg::DeletePage { page_id } => {
            // drop the page root and its whole block subtree (rows +
            // postings), exactly like RemoveBlock but starting from a root.
            // child PAGES are separate roots (the folder relation is not
            // mirrored in this index's membership set), so they survive —
            // matching the module's promote-children semantics.
            let Some(row) = read_row(read, &page_id)? else {
                return Ok(out);
            };
            delete_subtree(read, &mut out, row)?;
        }
    }
    Ok(out)
}

/// serve one materialized-view request.
pub fn serve_view(read: &impl StateRead, req: &[u8]) -> Result<Vec<u8>, Fail> {
    let PagesViewQuery::Search {
        text,
        page_id,
        limit,
    } = serde_json::from_slice(req).map_err(|e| Fail::new(FAIL_BAD_REQUEST, e.to_string()))?;
    let tokens: Vec<String> = search::tokens(&text).into_iter().collect();
    if tokens.is_empty() {
        return Err(Fail::new(FAIL_BAD_REQUEST, "search text has no tokens"));
    }
    // each token matches as a prefix (search-as-you-type). block ids are
    // global, so postings carry no page segment — the page filter applies
    // to the intersected refs instead.
    let mut refs: Vec<TokRef> =
        search::intersect_prefix(read, "tok/", &tokens, DEFAULT_POSTING_CAP)
            .into_iter()
            .filter_map(|hit| serde_json::from_slice(&hit.value).ok())
            .filter(|r: &TokRef| page_id.as_ref().is_none_or(|p| &r.page_id == p))
            .collect();
    refs.sort_by(|a, b| (b.time, &b.block_id).cmp(&(a.time, &a.block_id)));
    let limit = limit
        .unwrap_or(DEFAULT_SEARCH_LIMIT)
        .clamp(1, MAX_SEARCH_LIMIT);
    let mut hits = Vec::new();
    for r in refs.into_iter().take(limit) {
        if let Some(bytes) = read.get(blk_key(&r.block_id).as_bytes()) {
            hits.push(
                serde_json::from_slice(&bytes)
                    .map_err(|e| Fail::new(FAIL_ROW_DECODE, e.to_string()))?,
            );
        }
    }
    serde_json::to_vec(&PagesViewReply::Hits(hits))
        .map_err(|e| Fail::new(FAIL_BAD_REQUEST, e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NewBlock, encode_msg};
    use index_guest::{OriginTag, apply_to_map};
    use std::collections::BTreeMap;

    type Map = BTreeMap<Vec<u8>, Vec<u8>>;

    fn op(height: u64, seq: u32, msg: &PageMsg) -> OpRow {
        OpRow {
            height,
            seq,
            time: 1_000 + height,
            origin: OriginTag::external("jess"),
            payload: encode_msg(msg),
        }
    }

    fn insert(parent: &str, id: &str, text: &str) -> PageMsg {
        PageMsg::InsertBlock {
            parent: parent.into(),
            after: None,
            block: NewBlock {
                id: id.into(),
                kind: BlockKind::Paragraph,
                text: text.into(),
                marks: Vec::new(),
            },
        }
    }

    fn apply(map: &mut Map, height: u64, msgs: &[PageMsg]) {
        for (seq, msg) in msgs.iter().enumerate() {
            let writes = fold_op(&op(height, seq as u32, msg), map).expect("fold");
            apply_to_map(map, writes);
        }
    }

    fn search(map: &Map, req: serde_json::Value) -> Vec<PageBlockRow> {
        let bytes = serve_view(map, &serde_json::to_vec(&req).unwrap()).expect("view");
        match serde_json::from_slice(&bytes).expect("reply decodes") {
            PagesViewReply::Hits(hits) => hits,
        }
    }

    #[test]
    fn page_titles_and_nested_blocks_are_searchable() {
        let mut map = Map::new();
        apply(
            &mut map,
            1,
            &[
                PageMsg::CreatePage {
                    page_id: "p1".into(),
                    title: "roadmap draft".into(),
                    parent: None,
                },
                insert("p1", "b1", "quarter goals"),
                insert("b1", "b2", "nested milestone detail"),
            ],
        );

        // the title, a child, and a grandchild all resolve to page p1.
        for term in ["roadmap", "goals", "milestone"] {
            let hits = search(&map, serde_json::json!({"search": {"text": term}}));
            assert_eq!(hits.len(), 1, "{term}");
            assert_eq!(hits[0].page_id, "p1", "{term}");
        }

        // recreate is a no-op: the title survives.
        apply(
            &mut map,
            2,
            &[PageMsg::CreatePage {
                page_id: "p1".into(),
                title: "usurper".into(),
                parent: None,
            }],
        );
        assert!(search(&map, serde_json::json!({"search": {"text": "usurper"}})).is_empty());
        assert_eq!(
            search(&map, serde_json::json!({"search": {"text": "roadmap"}})).len(),
            1
        );
    }

    #[test]
    fn remove_block_unindexes_the_whole_subtree() {
        let mut map = Map::new();
        apply(
            &mut map,
            1,
            &[
                PageMsg::CreatePage {
                    page_id: "p1".into(),
                    title: "home".into(),
                    parent: None,
                },
                insert("p1", "b1", "toggle section"),
                insert("b1", "b2", "hidden inner text"),
                insert("p1", "b3", "sibling survivor"),
            ],
        );
        apply(
            &mut map,
            2,
            &[PageMsg::RemoveBlock {
                block_id: "b1".into(),
            }],
        );

        assert!(search(&map, serde_json::json!({"search": {"text": "toggle"}})).is_empty());
        assert!(search(&map, serde_json::json!({"search": {"text": "hidden"}})).is_empty());
        assert_eq!(
            search(&map, serde_json::json!({"search": {"text": "survivor"}})).len(),
            1
        );
    }

    #[test]
    fn same_parent_move_does_not_duplicate_membership() {
        let mut map = Map::new();
        apply(
            &mut map,
            1,
            &[
                PageMsg::CreatePage {
                    page_id: "p1".into(),
                    title: "home".into(),
                    parent: None,
                },
                insert("p1", "b1", "first"),
                insert("p1", "b2", "second"),
            ],
        );
        // two sibling reorders under the same parent — each used to re-read
        // the stale parent row and re-push the child, duplicating membership.
        apply(
            &mut map,
            2,
            &[PageMsg::MoveBlock {
                block_id: "b1".into(),
                parent: "p1".into(),
                after: Some("b2".into()),
            }],
        );
        apply(
            &mut map,
            3,
            &[PageMsg::MoveBlock {
                block_id: "b1".into(),
                parent: "p1".into(),
                after: None,
            }],
        );

        let hits = search(&map, serde_json::json!({"search": {"text": "home"}}));
        assert_eq!(hits.len(), 1);
        assert_eq!(
            hits[0].children.iter().filter(|c| *c == "b1").count(),
            1,
            "sibling reorders must not duplicate membership: {:?}",
            hits[0].children
        );
    }

    #[test]
    fn update_text_renames_and_page_filter_applies() {
        let mut map = Map::new();
        apply(
            &mut map,
            1,
            &[
                PageMsg::CreatePage {
                    page_id: "p1".into(),
                    title: "alpha".into(),
                    parent: None,
                },
                PageMsg::CreatePage {
                    page_id: "p2".into(),
                    title: "beta".into(),
                    parent: None,
                },
                insert("p1", "b1", "shared term"),
                insert("p2", "b2", "shared term"),
            ],
        );

        let hits = search(&map, serde_json::json!({"search": {"text": "shared"}}));
        assert_eq!(hits.len(), 2);
        let hits = search(
            &map,
            serde_json::json!({"search": {"text": "shared", "page_id": "p2"}}),
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].block_id, "b2");

        // renaming the page root retokenizes the title.
        apply(
            &mut map,
            2,
            &[PageMsg::UpdateText {
                block_id: "p1".into(),
                text: "gamma".into(),
                marks: None,
            }],
        );
        assert!(search(&map, serde_json::json!({"search": {"text": "alpha"}})).is_empty());
        assert_eq!(
            search(&map, serde_json::json!({"search": {"text": "gamma"}})).len(),
            1
        );
    }
}
