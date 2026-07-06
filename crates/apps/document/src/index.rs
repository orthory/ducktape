//! document's materialized view: full-text search over blocks.
//!
//! canonical document state serves whole docs and single blocks by id; it
//! cannot search. this mapper folds applied [`DocMsg`] ops into a token index
//! over block text and serves `search` as document's endpoint on the derived
//! tier. block ids are client-minted and stable, so the fold needs no
//! sequence mirroring: rows key directly on `(doc_id, block_id)`.
//!
//! key spaces:
//! - `blk/{doc_id}/{block_id}`          — the block's current [`BlockRow`].
//! - `tok/{token}/{doc_id}/{block_id}`  — one posting per (token, block),
//!   value = [`TokRef`]; rewritten whole on every text change.
//!
//! from-state rebuild: canonical `ListDocs`/`GetDoc` enumerate every block, so
//! rows and postings re-derive with an exact hit set. what canonical `Block`
//! does NOT carry is per-block coordinates — `height` and `time` collapse to
//! the boundary, so ranking among rebuilt rows degrades to id order.

use crate::{BlockKind, DocMsg, DocQuery, DocReply, decode_msg, decode_reply, encode_query};
use indexer::search::{self, DEFAULT_POSTING_CAP};
use indexer::{
    ApplyCtx, Backfill, Derived, Error, ModuleIndexer, OpMeta, RebuildMeta, Result, StateReader,
    ViewReader,
};
use serde::{Deserialize, Serialize};

const DEFAULT_SEARCH_LIMIT: usize = 20;
const MAX_SEARCH_LIMIT: usize = 100;

/// the stored row of one document block, as search results return it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BlockRow {
    pub doc_id: String,
    pub block_id: String,
    pub kind: BlockKind,
    pub text: String,
    pub height: u64,
    pub time: u64,
}

/// a token posting's value: rank (time) plus the row address.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TokRef {
    doc_id: String,
    block_id: String,
    time: u64,
}

/// document's view requests, externally tagged:
/// `{"search": {"text": "...", "docId": "...", "limit": 20}}`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DocViewQuery {
    #[serde(rename_all = "camelCase")]
    Search {
        text: String,
        #[serde(default)]
        doc_id: Option<String>,
        #[serde(default)]
        limit: Option<usize>,
    },
}

/// document's view replies: `{"hits": [<BlockRow>…]}`, newest first.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DocViewReply {
    Hits(Vec<BlockRow>),
}

pub struct DocumentIndex {
    module: String,
}

impl DocumentIndex {
    pub fn new(module: impl Into<String>) -> Self {
        Self {
            module: module.into(),
        }
    }
}

fn blk_key(doc: &str, block: &str) -> String {
    format!("blk/{doc}/{block}")
}

fn tok_key(token: &str, doc: &str, block: &str) -> String {
    format!("tok/{token}/{doc}/{block}")
}

fn read_row(ctx: &ApplyCtx, doc: &str, block: &str) -> Result<Option<BlockRow>> {
    match ctx.get(blk_key(doc, block).as_bytes())? {
        Some(bytes) => Ok(Some(
            serde_json::from_slice(&bytes).map_err(|e| Error::Mapper(e.to_string()))?,
        )),
        None => Ok(None),
    }
}

/// every entry one block row materializes to — the row itself plus one
/// posting per token. fold and rebuild both write THROUGH this, so the two
/// paths produce byte-identical rows.
fn row_entries(row: &BlockRow) -> Result<Vec<(String, Vec<u8>)>> {
    let mut entries = vec![(
        blk_key(&row.doc_id, &row.block_id),
        serde_json::to_vec(row).map_err(|e| Error::Mapper(e.to_string()))?,
    )];
    let tok_ref = serde_json::to_vec(&TokRef {
        doc_id: row.doc_id.clone(),
        block_id: row.block_id.clone(),
        time: row.time,
    })
    .map_err(|e| Error::Mapper(e.to_string()))?;
    for token in search::tokens(&row.text) {
        entries.push((
            tok_key(&token, &row.doc_id, &row.block_id),
            tok_ref.clone(),
        ));
    }
    Ok(entries)
}

fn put_row_and_toks(out: &mut Derived, row: &BlockRow) -> Result<()> {
    for (key, value) in row_entries(row)? {
        out.put(key, value);
    }
    Ok(())
}

fn delete_toks(out: &mut Derived, row: &BlockRow) {
    for token in search::tokens(&row.text) {
        out.delete(tok_key(&token, &row.doc_id, &row.block_id));
    }
}

#[async_trait::async_trait(?Send)]
impl ModuleIndexer for DocumentIndex {
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
            DocMsg::InsertBlock { doc_id, block, .. } => {
                let row = BlockRow {
                    doc_id,
                    block_id: block.id,
                    kind: block.kind,
                    text: block.text,
                    height: meta.height,
                    time: meta.time,
                };
                put_row_and_toks(out, &row)
            }
            DocMsg::UpdateBlock {
                doc_id,
                block_id,
                text,
            } => {
                // absent row == the block predates this index (pre-index doc).
                let Some(mut row) = read_row(ctx, &doc_id, &block_id)? else {
                    return Ok(());
                };
                // delete BEFORE re-putting: tokens shared by the old and new
                // text stage a delete then a put, and the last action wins.
                delete_toks(out, &row);
                row.text = text;
                row.height = meta.height;
                row.time = meta.time;
                put_row_and_toks(out, &row)
            }
            DocMsg::RemoveBlock { doc_id, block_id } => {
                let Some(row) = read_row(ctx, &doc_id, &block_id)? else {
                    return Ok(());
                };
                delete_toks(out, &row);
                out.delete(blk_key(&doc_id, &block_id));
                Ok(())
            }
            // no text changes: creating an empty doc / reordering blocks.
            DocMsg::CreateDoc { .. } | DocMsg::MoveBlock { .. } => Ok(()),
        }
    }

    fn serve_view(&self, reader: &ViewReader, req: &[u8]) -> Result<Vec<u8>> {
        let DocViewQuery::Search {
            text,
            doc_id,
            limit,
        } = serde_json::from_slice(req).map_err(|e| Error::View(e.to_string()))?;
        let tokens = search::tokens(&text);
        if tokens.is_empty() {
            return Err(Error::View("search text has no tokens".into()));
        }
        let prefixes: Vec<String> = tokens
            .iter()
            .map(|t| match &doc_id {
                Some(d) => format!("tok/{t}/{d}/"),
                None => format!("tok/{t}/"),
            })
            .collect();
        let mut refs: Vec<TokRef> = search::intersect(reader, &prefixes, DEFAULT_POSTING_CAP)?
            .into_iter()
            .filter_map(|hit| serde_json::from_slice(&hit.value).ok())
            .collect();
        refs.sort_by(|a, b| {
            (b.time, &b.doc_id, &b.block_id).cmp(&(a.time, &a.doc_id, &a.block_id))
        });
        let limit = limit.unwrap_or(DEFAULT_SEARCH_LIMIT).clamp(1, MAX_SEARCH_LIMIT);
        let mut hits = Vec::new();
        for r in refs.into_iter().take(limit) {
            if let Some(bytes) = reader.get(blk_key(&r.doc_id, &r.block_id).as_bytes())? {
                hits.push(
                    serde_json::from_slice(&bytes).map_err(|e| Error::Mapper(e.to_string()))?,
                );
            }
        }
        serde_json::to_vec(&DocViewReply::Hits(hits)).map_err(|e| Error::View(e.to_string()))
    }

    fn supports_rebuild(&self) -> bool {
        true
    }

    /// re-derive rows and postings from canonical `ListDocs`/`GetDoc`. the
    /// documented degradation: canonical `Block` keeps no per-block
    /// coordinates, so every rebuilt row is boundary-stamped — hit sets stay
    /// exact, ranking among rebuilt rows falls back to id order.
    async fn rebuild_from_state(
        &self,
        state: &dyn StateReader,
        meta: &RebuildMeta,
        out: &mut Backfill<'_>,
    ) -> Result<()> {
        let reply = state.query(&encode_query(&DocQuery::ListDocs)).await?;
        let doc_ids = match decode_reply(&reply).map_err(Error::State)? {
            DocReply::DocList(ids) => ids,
            other => return Err(Error::State(format!("ListDocs answered {other:?}"))),
        };
        for doc_id in doc_ids {
            let reply = state
                .query(&encode_query(&DocQuery::GetDoc {
                    doc_id: doc_id.clone(),
                }))
                .await?;
            let blocks = match decode_reply(&reply).map_err(Error::State)? {
                DocReply::Doc(blocks) => blocks,
                other => return Err(Error::State(format!("GetDoc answered {other:?}"))),
            };
            // a listed doc always answers Some — state cannot change under a
            // rebuild — but an empty doc is real (created, no blocks yet).
            for block in blocks.unwrap_or_default() {
                let row = BlockRow {
                    doc_id: doc_id.clone(),
                    block_id: block.id,
                    kind: block.kind,
                    text: block.text,
                    height: meta.height,
                    time: meta.time,
                };
                for (key, value) in row_entries(&row)? {
                    out.put(key, value)?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Block, encode_msg};
    use indexer::{AppliedOp, BlockOps, IndexStore, OriginTag};

    fn store(dir: &std::path::Path) -> IndexStore {
        IndexStore::open(dir, &["document"])
            .expect("open store")
            .with_indexer(Box::new(DocumentIndex::new("document")))
    }

    fn op(msg: &DocMsg) -> AppliedOp {
        AppliedOp {
            module: "document".into(),
            origin: OriginTag::external("jess"),
            payload: encode_msg(msg),
        }
    }

    fn insert(doc: &str, id: &str, text: &str) -> AppliedOp {
        op(&DocMsg::InsertBlock {
            doc_id: doc.into(),
            after: None,
            block: Block {
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

    fn search(store: &IndexStore, req: serde_json::Value) -> Vec<BlockRow> {
        let bytes = store
            .view("document", &serde_json::to_vec(&req).unwrap())
            .expect("view");
        match serde_json::from_slice(&bytes).expect("reply decodes") {
            DocViewReply::Hits(hits) => hits,
        }
    }

    #[test]
    fn blocks_are_searchable_with_doc_filter() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        apply(&store, 1, vec![insert("spec", "b1", "consensus design notes")]);
        apply(&store, 2, vec![insert("journal", "b2", "consensus meeting")]);

        let hits = search(&store, serde_json::json!({"search": {"text": "consensus"}}));
        assert_eq!(hits.len(), 2);

        let hits = search(
            &store,
            serde_json::json!({"search": {"text": "consensus", "docId": "spec"}}),
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].block_id, "b1");
    }

    #[test]
    fn update_retokenizes_and_remove_unindexes() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        apply(&store, 1, vec![insert("spec", "b1", "draft wording")]);
        apply(
            &store,
            2,
            vec![op(&DocMsg::UpdateBlock {
                doc_id: "spec".into(),
                block_id: "b1".into(),
                text: "polished wording".into(),
            })],
        );
        assert!(search(&store, serde_json::json!({"search": {"text": "draft"}})).is_empty());
        assert_eq!(
            search(&store, serde_json::json!({"search": {"text": "polished"}})).len(),
            1
        );
        // a token in BOTH the old and new text stages delete-then-put on one
        // key inside one op; the posting must survive the retokenize.
        assert_eq!(
            search(&store, serde_json::json!({"search": {"text": "wording"}})).len(),
            1,
            "retained token survives a retokenize"
        );

        apply(
            &store,
            3,
            vec![op(&DocMsg::RemoveBlock {
                doc_id: "spec".into(),
                block_id: "b1".into(),
            })],
        );
        assert!(search(&store, serde_json::json!({"search": {"text": "polished"}})).is_empty());
    }

    /// canonical document state standing in for the module's query surface:
    /// doc id → ordered blocks.
    struct CanonicalDocs(Vec<(String, Vec<Block>)>);

    #[async_trait::async_trait(?Send)]
    impl indexer::StateReader for CanonicalDocs {
        async fn query(&self, req: &[u8]) -> indexer::Result<Vec<u8>> {
            let reply = match crate::decode_query(req).map_err(Error::State)? {
                DocQuery::ListDocs => {
                    DocReply::DocList(self.0.iter().map(|(id, _)| id.clone()).collect())
                }
                DocQuery::GetDoc { doc_id } => DocReply::Doc(
                    self.0
                        .iter()
                        .find(|(id, _)| *id == doc_id)
                        .map(|(_, blocks)| blocks.clone()),
                ),
                other => return Err(Error::State(format!("unexpected query {other:?}"))),
            };
            Ok(crate::encode_reply(&reply))
        }
    }

    #[tokio::test]
    async fn rebuild_rederives_search_from_canonical_docs() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        // folded rows the rebuild must throw away.
        apply(&store, 1, vec![insert("stale", "b0", "vanishing text")]);

        let state = CanonicalDocs(vec![
            (
                "spec".into(),
                vec![
                    Block {
                        id: "b1".into(),
                        kind: BlockKind::Paragraph,
                        text: "consensus design notes".into(),
                    },
                    Block {
                        id: "b2".into(),
                        kind: BlockKind::Paragraph,
                        text: "derived tier rebuild".into(),
                    },
                ],
            ),
            ("empty".into(), vec![]),
        ]);
        let written = store
            .rebuild_module(
                "document",
                &state,
                indexer::RebuildMeta { height: 33, time: 0 },
            )
            .await
            .expect("rebuild");
        assert!(written >= 2, "two rows plus their postings");

        assert!(
            search(&store, serde_json::json!({"search": {"text": "vanishing"}})).is_empty(),
            "pre-rebuild rows do not survive"
        );
        let hits = search(&store, serde_json::json!({"search": {"text": "consensus"}}));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].block_id, "b1");
        // the documented degradation: coordinates collapse to the boundary.
        assert_eq!(hits[0].height, 33);

        assert_eq!(store.applied_height("document").unwrap(), 33);
        assert_eq!(store.backfill_height("document").unwrap(), Some(33));

        // rebuilt rows fold forward like originals: an update retokenizes.
        apply(
            &store,
            34,
            vec![op(&DocMsg::UpdateBlock {
                doc_id: "spec".into(),
                block_id: "b2".into(),
                text: "warm views".into(),
            })],
        );
        assert!(search(&store, serde_json::json!({"search": {"text": "rebuild"}})).is_empty());
        assert_eq!(
            search(&store, serde_json::json!({"search": {"text": "warm"}})).len(),
            1
        );
    }
}
