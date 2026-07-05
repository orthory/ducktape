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
//! key spaces:
//! - `blk/{block_id}`         — the block's current [`PageBlockRow`].
//! - `tok/{token}/{block_id}` — one posting per (token, block), value =
//!   [`TokRef`]; rewritten whole on every text change.

use indexer::search::{self, DEFAULT_POSTING_CAP};
use indexer::{ApplyCtx, Derived, Error, ModuleIndexer, OpMeta, Result, ViewReader};
use pages_interface::{BlockKind, PageMsg, decode_msg};
use serde::{Deserialize, Serialize};

const DEFAULT_SEARCH_LIMIT: usize = 20;
const MAX_SEARCH_LIMIT: usize = 100;

/// the stored row of one page block, as search results return it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
struct TokRef {
    block_id: String,
    page_id: String,
    time: u64,
}

/// pages' view requests, externally tagged:
/// `{"search": {"text": "...", "pageId": "...", "limit": 20}}`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PagesViewQuery {
    #[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub enum PagesViewReply {
    Hits(Vec<PageBlockRow>),
}

pub struct PagesIndex {
    module: String,
}

impl PagesIndex {
    pub fn new(module: impl Into<String>) -> Self {
        Self {
            module: module.into(),
        }
    }
}

fn blk_key(id: &str) -> String {
    format!("blk/{id}")
}

fn tok_key(token: &str, id: &str) -> String {
    format!("tok/{token}/{id}")
}

fn read_row(ctx: &ApplyCtx, id: &str) -> Result<Option<PageBlockRow>> {
    match ctx.get(blk_key(id).as_bytes())? {
        Some(bytes) => Ok(Some(
            serde_json::from_slice(&bytes).map_err(|e| Error::Mapper(e.to_string()))?,
        )),
        None => Ok(None),
    }
}

fn put_row(out: &mut Derived, row: &PageBlockRow) -> Result<()> {
    out.put(
        blk_key(&row.block_id),
        serde_json::to_vec(row).map_err(|e| Error::Mapper(e.to_string()))?,
    );
    Ok(())
}

fn put_toks(out: &mut Derived, row: &PageBlockRow) -> Result<()> {
    let tok_ref = serde_json::to_vec(&TokRef {
        block_id: row.block_id.clone(),
        page_id: row.page_id.clone(),
        time: row.time,
    })
    .map_err(|e| Error::Mapper(e.to_string()))?;
    for token in search::tokens(&row.text) {
        out.put(tok_key(&token, &row.block_id), tok_ref.clone());
    }
    Ok(())
}

fn delete_toks(out: &mut Derived, row: &PageBlockRow) {
    for token in search::tokens(&row.text) {
        out.delete(tok_key(&token, &row.block_id));
    }
}

impl ModuleIndexer for PagesIndex {
    fn module(&self) -> &str {
        &self.module
    }

    fn index_op(
        &self,
        ctx: &ApplyCtx,
        meta: &OpMeta,
        payload: &[u8],
        out: &mut Derived,
    ) -> Result<()> {
        match decode_msg(payload).map_err(Error::Mapper)? {
            PageMsg::CreatePage { page_id, title } => {
                // idempotence mirror: re-creating an existing page is a no-op
                // that does NOT overwrite the title.
                if read_row(ctx, &page_id)?.is_some() {
                    return Ok(());
                }
                let row = PageBlockRow {
                    page_id: page_id.clone(),
                    block_id: page_id,
                    parent: None,
                    kind: BlockKind::Page,
                    text: title,
                    children: Vec::new(),
                    height: meta.height,
                    time: meta.time,
                };
                put_toks(out, &row)?;
                put_row(out, &row)
            }
            PageMsg::InsertBlock { parent, block, .. } => {
                // the page is derived from the parent — a parent this index
                // never saw (pre-index tree) leaves the whole insert out.
                let Some(mut parent_row) = read_row(ctx, &parent)? else {
                    return Ok(());
                };
                let row = PageBlockRow {
                    block_id: block.id.clone(),
                    page_id: parent_row.page_id.clone(),
                    parent: Some(parent.clone()),
                    kind: block.kind,
                    text: block.text,
                    children: Vec::new(),
                    height: meta.height,
                    time: meta.time,
                };
                put_toks(out, &row)?;
                put_row(out, &row)?;
                parent_row.children.push(block.id);
                put_row(out, &parent_row)
            }
            PageMsg::UpdateText { block_id, text } => {
                let Some(mut row) = read_row(ctx, &block_id)? else {
                    return Ok(());
                };
                delete_toks(out, &row);
                row.text = text;
                row.height = meta.height;
                row.time = meta.time;
                put_toks(out, &row)?;
                put_row(out, &row)
            }
            PageMsg::SetKind { block_id, kind } => {
                let Some(mut row) = read_row(ctx, &block_id)? else {
                    return Ok(());
                };
                row.kind = kind;
                put_row(out, &row)
            }
            PageMsg::MoveBlock {
                block_id, parent, ..
            } => {
                // same-page move (the module rejects the rest): re-home the
                // membership edge, page and text unchanged.
                let Some(mut row) = read_row(ctx, &block_id)? else {
                    return Ok(());
                };
                if let Some(old_parent) = &row.parent
                    && let Some(mut old) = read_row(ctx, old_parent)?
                {
                    old.children.retain(|c| c != &block_id);
                    put_row(out, &old)?;
                }
                if let Some(mut new_parent) = read_row(ctx, &parent)? {
                    new_parent.children.push(block_id.clone());
                    put_row(out, &new_parent)?;
                }
                row.parent = Some(parent);
                put_row(out, &row)
            }
            PageMsg::RemoveBlock { block_id } => {
                let Some(row) = read_row(ctx, &block_id)? else {
                    return Ok(());
                };
                // unhook from the parent's membership set…
                if let Some(parent) = &row.parent
                    && let Some(mut parent_row) = read_row(ctx, parent)?
                {
                    parent_row.children.retain(|c| c != &block_id);
                    put_row(out, &parent_row)?;
                }
                // …then drop the whole subtree, rows and postings both.
                let mut stack = vec![row];
                while let Some(row) = stack.pop() {
                    delete_toks(out, &row);
                    out.delete(blk_key(&row.block_id));
                    for child in &row.children {
                        if let Some(child_row) = read_row(ctx, child)? {
                            stack.push(child_row);
                        }
                    }
                }
                Ok(())
            }
            // checked state carries no searchable text.
            PageMsg::SetChecked { .. } => Ok(()),
        }
    }

    fn serve_view(&self, reader: &ViewReader, req: &[u8]) -> Result<Vec<u8>> {
        let PagesViewQuery::Search {
            text,
            page_id,
            limit,
        } = serde_json::from_slice(req).map_err(|e| Error::View(e.to_string()))?;
        let tokens = search::tokens(&text);
        if tokens.is_empty() {
            return Err(Error::View("search text has no tokens".into()));
        }
        // block ids are global, so postings carry no page segment — the page
        // filter applies to the intersected refs instead.
        let prefixes: Vec<String> = tokens.iter().map(|t| format!("tok/{t}/")).collect();
        let mut refs: Vec<TokRef> = search::intersect(reader, &prefixes, DEFAULT_POSTING_CAP)?
            .into_iter()
            .filter_map(|hit| serde_json::from_slice(&hit.value).ok())
            .filter(|r: &TokRef| page_id.as_ref().is_none_or(|p| &r.page_id == p))
            .collect();
        refs.sort_by(|a, b| (b.time, &b.block_id).cmp(&(a.time, &a.block_id)));
        let limit = limit.unwrap_or(DEFAULT_SEARCH_LIMIT).clamp(1, MAX_SEARCH_LIMIT);
        let mut hits = Vec::new();
        for r in refs.into_iter().take(limit) {
            if let Some(bytes) = reader.get(blk_key(&r.block_id).as_bytes())? {
                hits.push(
                    serde_json::from_slice(&bytes).map_err(|e| Error::Mapper(e.to_string()))?,
                );
            }
        }
        serde_json::to_vec(&PagesViewReply::Hits(hits)).map_err(|e| Error::View(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexer::{AppliedOp, BlockOps, IndexStore, OriginTag};
    use pages_interface::{NewBlock, encode_msg};

    fn store(dir: &std::path::Path) -> IndexStore {
        IndexStore::open(dir, &["pages"])
            .expect("open store")
            .with_indexer(Box::new(PagesIndex::new("pages")))
    }

    fn op(msg: &PageMsg) -> AppliedOp {
        AppliedOp {
            module: "pages".into(),
            origin: OriginTag::external("jess"),
            payload: encode_msg(msg),
        }
    }

    fn insert(parent: &str, id: &str, text: &str) -> AppliedOp {
        op(&PageMsg::InsertBlock {
            parent: parent.into(),
            after: None,
            block: NewBlock {
                id: id.into(),
                kind: BlockKind::Paragraph,
                text: text.into(),
            },
        })
    }

    fn apply(store: &IndexStore, height: u64, ops: Vec<AppliedOp>) {
        store
            .apply_block(&BlockOps {
                height,
                time: 1_000 + height,
                ops,
                record: None,
            })
            .expect("apply");
    }

    fn search(store: &IndexStore, req: serde_json::Value) -> Vec<PageBlockRow> {
        let bytes = store
            .view("pages", &serde_json::to_vec(&req).unwrap())
            .expect("view");
        match serde_json::from_slice(&bytes).expect("reply decodes") {
            PagesViewReply::Hits(hits) => hits,
        }
    }

    #[test]
    fn page_titles_and_nested_blocks_are_searchable() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        apply(
            &store,
            1,
            vec![
                op(&PageMsg::CreatePage {
                    page_id: "p1".into(),
                    title: "roadmap draft".into(),
                }),
                insert("p1", "b1", "quarter goals"),
                insert("b1", "b2", "nested milestone detail"),
            ],
        );

        // the title, a child, and a grandchild all resolve to page p1.
        for term in ["roadmap", "goals", "milestone"] {
            let hits = search(&store, serde_json::json!({"search": {"text": term}}));
            assert_eq!(hits.len(), 1, "{term}");
            assert_eq!(hits[0].page_id, "p1", "{term}");
        }

        // recreate is a no-op: the title survives.
        apply(
            &store,
            2,
            vec![op(&PageMsg::CreatePage {
                page_id: "p1".into(),
                title: "usurper".into(),
            })],
        );
        assert!(search(&store, serde_json::json!({"search": {"text": "usurper"}})).is_empty());
        assert_eq!(
            search(&store, serde_json::json!({"search": {"text": "roadmap"}})).len(),
            1
        );
    }

    #[test]
    fn remove_block_unindexes_the_whole_subtree() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        apply(
            &store,
            1,
            vec![
                op(&PageMsg::CreatePage {
                    page_id: "p1".into(),
                    title: "home".into(),
                }),
                insert("p1", "b1", "toggle section"),
                insert("b1", "b2", "hidden inner text"),
                insert("p1", "b3", "sibling survivor"),
            ],
        );
        apply(
            &store,
            2,
            vec![op(&PageMsg::RemoveBlock {
                block_id: "b1".into(),
            })],
        );

        assert!(search(&store, serde_json::json!({"search": {"text": "toggle"}})).is_empty());
        assert!(search(&store, serde_json::json!({"search": {"text": "hidden"}})).is_empty());
        assert_eq!(
            search(&store, serde_json::json!({"search": {"text": "survivor"}})).len(),
            1
        );
    }

    #[test]
    fn update_text_renames_and_page_filter_applies() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        apply(
            &store,
            1,
            vec![
                op(&PageMsg::CreatePage {
                    page_id: "p1".into(),
                    title: "alpha".into(),
                }),
                op(&PageMsg::CreatePage {
                    page_id: "p2".into(),
                    title: "beta".into(),
                }),
                insert("p1", "b1", "shared term"),
                insert("p2", "b2", "shared term"),
            ],
        );

        let hits = search(&store, serde_json::json!({"search": {"text": "shared"}}));
        assert_eq!(hits.len(), 2);
        let hits = search(
            &store,
            serde_json::json!({"search": {"text": "shared", "pageId": "p2"}}),
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].block_id, "b2");

        // renaming the page root retokenizes the title.
        apply(
            &store,
            2,
            vec![op(&PageMsg::UpdateText {
                block_id: "p1".into(),
                text: "gamma".into(),
            })],
        );
        assert!(search(&store, serde_json::json!({"search": {"text": "alpha"}})).is_empty());
        assert_eq!(
            search(&store, serde_json::json!({"search": {"text": "gamma"}})).len(),
            1
        );
    }
}
