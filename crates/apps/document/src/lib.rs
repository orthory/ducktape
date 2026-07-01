//! qmdb-backed document module — ducktape's founding product, reborn simple.
//!
//! a document is an ORDERED LIST of [`Block`]s (no markdown), keyed by `doc_id`.
//! many documents live in ONE qmdb `any/unordered/variable` database: the qmdb
//! key is the `doc_id` bytes and the value is the whole document serialized as a
//! json `Vec<Block>` (whole-doc-per-key — the simple MVP; a per-block ordered-KV
//! is a later optimization). the module's authenticated [`StateRoot`] IS the
//! qmdb merkle root, refreshed on every committed write, so it folds straight
//! into the global app-hash next to a git HEAD oid or another qmdb root.
//!
//! ## host-lent staging (mirrors the `kv` module)
//!
//! writes made during a block are STAGED in an in-memory `pending` overlay:
//! `execute` load-mutates-stores into `pending`, later ops in the same block
//! read their own writes back through it (read-your-writes), and `commit_block`
//! flushes every staged doc to qmdb in ONE batch — only then does `root()` move.
//! `abort_block` drops the overlay so a failed block leaves no trace. this is
//! kv's staging pattern verbatim; the only document-specific code is the
//! `Vec<Block>` (de)serialization and the per-op list surgery.

use std::collections::BTreeMap;
use std::num::{NonZeroU16, NonZeroU64, NonZeroUsize};

use commonware_codec::RangeCfg;
use commonware_cryptography::Sha256;
use commonware_parallel::Sequential;
use commonware_runtime::{buffer::paged::CacheRef, BufferPooler};
use commonware_storage::{
    journal, mmr,
    qmdb::any::{unordered::variable::Db, VariableConfig},
    translator::TwoCap,
    Context,
};

use document_interface::{
    decode_msg, decode_query, encode_reply, Block, DocMsg, DocQuery, DocReply,
};
use sdk::{Ctx, Error, Module, ModuleId, Msg, StateRoot};

/// the concrete qmdb store — byte keys/values, sha256 hasher, two-byte
/// translator, sequential (deterministic) merkle strategy. identical params to
/// the kv module's `KvDb`, so all qmdb plumbing is shared verbatim.
type DocDb<E> = Db<mmr::Family, E, Vec<u8>, Vec<u8>, Sha256, TwoCap, Sequential>;

/// per-op failures. mapped to [`Error::Module`] so any error aborts the whole
/// block (the sdk `abort_block` contract), rolling back the staged overlay.
#[derive(Debug, Clone, PartialEq, Eq)]
enum DocError {
    /// a block op targeted a doc that was never created.
    DocNotFound,
    /// insert of a block id already present (ids must be unique per doc).
    DuplicateBlock,
    /// update/remove/move of a block id not in the doc.
    BlockNotFound,
    /// an `after` anchor id that isn't in the doc.
    AnchorNotFound,
}

impl core::fmt::Display for DocError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let s = match self {
            DocError::DocNotFound => "doc not found",
            DocError::DuplicateBlock => "duplicate block id",
            DocError::BlockNotFound => "block not found",
            DocError::AnchorNotFound => "after-anchor not found",
        };
        f.write_str(s)
    }
}

/// resolve an `after` anchor to the insert index: `None` -> front (0);
/// `Some(id)` -> one past the anchor's position, else [`DocError::AnchorNotFound`].
fn idx_after(blocks: &[Block], after: &Option<String>) -> Result<usize, DocError> {
    match after {
        None => Ok(0),
        Some(a) => blocks
            .iter()
            .position(|b| &b.id == a)
            .map(|p| p + 1)
            .ok_or(DocError::AnchorNotFound),
    }
}

/// a qmdb-backed, block-based document module.
pub struct Document<E>
where
    E: Context + BufferPooler,
{
    id: ModuleId,
    db: DocDb<E>,
    /// docs written this block, keyed by `doc_id` bytes -> serialized `Vec<Block>`.
    /// read ahead of committed state by `get` (read-your-writes) and flushed to
    /// qmdb in one batch by `commit_block`; NOT reflected in `root()` until then.
    pending: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl<E> Document<E>
where
    E: Context + BufferPooler,
{
    /// open (or recover) the store on `context` under module identity `id`.
    /// qmdb partitions are namespaced by `id`, so a document module shares one
    /// runtime context with kv/other qmdb modules without colliding — the demo
    /// hookup is purely additive. copied verbatim from the kv module.
    pub async fn init(context: E, id: impl Into<ModuleId>) -> Self {
        let id = id.into();
        let page_cache = CacheRef::from_pooler(
            &context,
            NonZeroU16::new(128).unwrap(),
            NonZeroUsize::new(64).unwrap(),
        );

        let codec_config = (
            (RangeCfg::from(0..=1 << 20), ()),
            (RangeCfg::from(0..=1 << 20), ()),
        );

        let cfg: VariableConfig<TwoCap, ((RangeCfg<usize>, ()), (RangeCfg<usize>, ())), Sequential> =
            VariableConfig {
                merkle_config: mmr::full::Config {
                    journal_partition: format!("{id}-merkle-journal"),
                    metadata_partition: format!("{id}-merkle-meta"),
                    items_per_blob: NonZeroU64::new(64).unwrap(),
                    write_buffer: NonZeroUsize::new(1024).unwrap(),
                    strategy: Sequential,
                    page_cache: page_cache.clone(),
                },
                journal_config: journal::contiguous::variable::Config {
                    partition: format!("{id}-log"),
                    items_per_section: NonZeroU64::new(64).unwrap(),
                    write_buffer: NonZeroUsize::new(1024).unwrap(),
                    compression: None,
                    codec_config,
                    page_cache,
                },
                translator: TwoCap,
            };

        let db = DocDb::<E>::init(context, cfg)
            .await
            .expect("qmdb init failed");

        Self { id, db, pending: BTreeMap::new() }
    }

    /// read raw bytes for `key`: a STAGED (this-block) write shadows committed
    /// qmdb state, so a later op in the same block sees an earlier staged write.
    async fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        if let Some(v) = self.pending.get(key) {
            return Some(v.clone());
        }
        self.db.get(&key.to_vec()).await.expect("get failed")
    }

    /// load a document's ordered blocks (`None` == doc absent), through the
    /// staged-over-committed overlay.
    async fn load(&self, doc_id: &str) -> Result<Option<Vec<Block>>, Error> {
        match self.get(doc_id.as_bytes()).await {
            Some(b) => Ok(Some(
                serde_json::from_slice(&b).map_err(|e| Error::Module(e.to_string()))?,
            )),
            None => Ok(None),
        }
    }

    /// stage a document's serialized blocks for this block WITHOUT committing.
    /// visible to `get`/`load` at once; folded into qmdb (and `root()`) only when
    /// the host calls `commit_block`.
    fn store(&mut self, doc_id: &str, blocks: &[Block]) {
        self.pending.insert(
            doc_id.as_bytes().to_vec(),
            serde_json::to_vec(blocks).expect("Vec<Block> is always serializable"),
        );
    }

    /// apply one decoded [`DocMsg`] to the staged overlay. pure list surgery over
    /// the loaded `Vec<Block>`, re-staged on success. errors abort the block.
    async fn apply(&mut self, msg: DocMsg) -> Result<(), DocError> {
        match msg {
            DocMsg::CreateDoc { doc_id } => {
                // idempotent: only seed an empty doc if absent. an empty doc is a
                // stored `[]`; ABSENT is `None` — that distinction is why CreateDoc
                // is its own op and why block ops require it first.
                if self.load(&doc_id).await.map_err(to_doc_err)?.is_none() {
                    self.store(&doc_id, &[]);
                }
                Ok(())
            }
            DocMsg::InsertBlock { doc_id, after, block } => {
                let mut d = self.load(&doc_id).await.map_err(to_doc_err)?.ok_or(DocError::DocNotFound)?;
                if d.iter().any(|b| b.id == block.id) {
                    return Err(DocError::DuplicateBlock);
                }
                let i = idx_after(&d, &after)?;
                d.insert(i, block);
                self.store(&doc_id, &d);
                Ok(())
            }
            DocMsg::UpdateBlock { doc_id, block_id, text } => {
                let mut d = self.load(&doc_id).await.map_err(to_doc_err)?.ok_or(DocError::DocNotFound)?;
                let b = d.iter_mut().find(|b| b.id == block_id).ok_or(DocError::BlockNotFound)?;
                b.text = text;
                self.store(&doc_id, &d);
                Ok(())
            }
            DocMsg::RemoveBlock { doc_id, block_id } => {
                let mut d = self.load(&doc_id).await.map_err(to_doc_err)?.ok_or(DocError::DocNotFound)?;
                let pos = d.iter().position(|b| b.id == block_id).ok_or(DocError::BlockNotFound)?;
                d.remove(pos);
                self.store(&doc_id, &d);
                Ok(())
            }
            DocMsg::MoveBlock { doc_id, block_id, after } => {
                // self-anchor is a no-op — resolve it BEFORE removal, else the
                // remove-then-lookup would fail with a bogus AnchorNotFound.
                if after.as_deref() == Some(block_id.as_str()) {
                    return Ok(());
                }
                let mut d = self.load(&doc_id).await.map_err(to_doc_err)?.ok_or(DocError::DocNotFound)?;
                let pos = d.iter().position(|b| b.id == block_id).ok_or(DocError::BlockNotFound)?;
                let blk = d.remove(pos);
                // anchor index is computed in the now-shortened list.
                let i = idx_after(&d, &after)?;
                d.insert(i, blk);
                self.store(&doc_id, &d);
                Ok(())
            }
        }
    }
}

/// bridge the only sdk error `load` can raise — a stored-doc json decode failure
/// — back into `DocError` so `apply` stays single-error-typed. unreachable for
/// our own writes (we only ever store valid `Vec<Block>` json); a corrupt stored
/// doc is treated as absent.
fn to_doc_err(_e: Error) -> DocError {
    DocError::DocNotFound
}

#[async_trait::async_trait(?Send)]
impl<E> Module for Document<E>
where
    E: Context + BufferPooler,
{
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// the REAL qmdb merkle root over all documents, as a 32-byte state root.
    /// sync (qmdb caches its root); never a placeholder.
    fn root(&self) -> StateRoot {
        StateRoot(self.db.root().0)
    }

    /// decode a [`DocMsg`] and apply it to the staged overlay. the only `.await`
    /// is on own qmdb state — deterministic, so replay-safe across validators.
    async fn execute(&mut self, _ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let m = decode_msg(&msg.payload).map_err(Error::Module)?;
        self.apply(m).await.map_err(|e| Error::Module(e.to_string()))
    }

    /// real async read of own qmdb state, serving STAGED-over-committed via
    /// `load`, so reads within a block observe this block's staged writes.
    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            DocQuery::GetDoc { doc_id } => {
                Ok(encode_reply(&DocReply::Doc(self.load(&doc_id).await?)))
            }
            DocQuery::GetBlock { doc_id, block_id } => {
                let block = self
                    .load(&doc_id)
                    .await?
                    .and_then(|d| d.into_iter().find(|b| b.id == block_id));
                Ok(encode_reply(&DocReply::Block(block)))
            }
        }
    }

    /// publish the block's staged docs in ONE qmdb batch: write every pending
    /// doc, merkleize, apply, commit. no-op (and no root movement) if nothing was
    /// staged. byte-identical to the kv commit path.
    async fn commit_block(&mut self) -> Result<(), Error> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let mut batch = self.db.new_batch();
        for (key, value) in &self.pending {
            batch = batch.write(key.clone(), Some(value.clone()));
        }
        let batch = batch
            .merkleize(&self.db, None::<Vec<u8>>)
            .await
            .expect("merkleize failed");
        self.db.apply_batch(batch).await.expect("apply_batch failed");
        self.db.commit().await.expect("commit failed");
        self.pending.clear();
        Ok(())
    }

    /// discard the block's staged docs — nothing reached qmdb, so `root()` is
    /// unchanged.
    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use commonware_runtime::{deterministic, Runner as _};
    use document_interface::{decode_reply, encode_msg, encode_query, BlockKind};
    use state::global_root;

    fn blk(id: &str, text: &str) -> Block {
        Block { id: id.into(), kind: BlockKind::Paragraph, text: text.into() }
    }

    fn msg(m: &DocMsg) -> Msg {
        Msg { target: "document".into(), payload: encode_msg(m) }
    }

    // a minimal Ctx so execute can be driven without a full host.
    struct TestCtx {
        env: sdk::Env,
    }
    impl TestCtx {
        fn new() -> Self {
            Self {
                env: sdk::Env {
                    height: 0,
                    consensus_time: 0,
                    origin: sdk::Origin::System,
                    me: "document".into(),
                },
            }
        }
    }
    #[async_trait::async_trait(?Send)]
    impl Ctx for TestCtx {
        fn env(&self) -> &sdk::Env { &self.env }
        fn module_root(&self, _t: &str) -> Option<StateRoot> { None }
        async fn query(&self, _t: &str, _r: &[u8]) -> Result<Vec<u8>, Error> {
            Err(Error::QueryUnsupported)
        }
        fn emit_msg(&mut self, _m: Msg) {}
        fn emit_event(&mut self, _e: sdk::Event) {}
        fn request_effect(&mut self, _e: sdk::Effect) {}
    }

    // drive one op through execute + commit_block (one op per block).
    async fn apply_commit<E: Context + BufferPooler>(d: &mut Document<E>, m: &DocMsg) {
        d.execute(&mut TestCtx::new(), &msg(m)).await.unwrap();
        d.commit_block().await.unwrap();
    }

    async fn get_doc<E: Context + BufferPooler>(d: &Document<E>, doc_id: &str) -> Option<Vec<Block>> {
        let reply = d.query(&encode_query(&DocQuery::GetDoc { doc_id: doc_id.into() })).await.unwrap();
        match decode_reply(&reply).unwrap() {
            DocReply::Doc(v) => v,
            _ => panic!("expected Doc"),
        }
    }

    #[test]
    fn create_insert_returns_blocks_in_order() {
        deterministic::Runner::default().start(|context| async move {
            let mut d = Document::init(context, "document").await;
            apply_commit(&mut d, &DocMsg::CreateDoc { doc_id: "doc1".into() }).await;
            apply_commit(&mut d, &DocMsg::InsertBlock { doc_id: "doc1".into(), after: None, block: blk("b1", "first") }).await;
            // after b1 -> b2 lands at the end.
            apply_commit(&mut d, &DocMsg::InsertBlock { doc_id: "doc1".into(), after: Some("b1".into()), block: blk("b2", "second") }).await;
            // after None -> front, so b0 goes before b1.
            apply_commit(&mut d, &DocMsg::InsertBlock { doc_id: "doc1".into(), after: None, block: blk("b0", "zero") }).await;

            let doc = get_doc(&d, "doc1").await.unwrap();
            let ids: Vec<&str> = doc.iter().map(|b| b.id.as_str()).collect();
            assert_eq!(ids, ["b0", "b1", "b2"]);
            assert_eq!(doc[1].text, "first");
        });
    }

    #[test]
    fn update_changes_a_block() {
        deterministic::Runner::default().start(|context| async move {
            let mut d = Document::init(context, "document").await;
            apply_commit(&mut d, &DocMsg::CreateDoc { doc_id: "doc1".into() }).await;
            apply_commit(&mut d, &DocMsg::InsertBlock { doc_id: "doc1".into(), after: None, block: blk("b1", "old") }).await;
            apply_commit(&mut d, &DocMsg::UpdateBlock { doc_id: "doc1".into(), block_id: "b1".into(), text: "new".into() }).await;
            let doc = get_doc(&d, "doc1").await.unwrap();
            assert_eq!(doc[0].text, "new");
        });
    }

    #[test]
    fn remove_drops_a_block() {
        deterministic::Runner::default().start(|context| async move {
            let mut d = Document::init(context, "document").await;
            apply_commit(&mut d, &DocMsg::CreateDoc { doc_id: "doc1".into() }).await;
            apply_commit(&mut d, &DocMsg::InsertBlock { doc_id: "doc1".into(), after: None, block: blk("b1", "one") }).await;
            apply_commit(&mut d, &DocMsg::InsertBlock { doc_id: "doc1".into(), after: Some("b1".into()), block: blk("b2", "two") }).await;
            apply_commit(&mut d, &DocMsg::RemoveBlock { doc_id: "doc1".into(), block_id: "b1".into() }).await;
            let doc = get_doc(&d, "doc1").await.unwrap();
            let ids: Vec<&str> = doc.iter().map(|b| b.id.as_str()).collect();
            assert_eq!(ids, ["b2"]);
        });
    }

    #[test]
    fn move_reorders_blocks() {
        deterministic::Runner::default().start(|context| async move {
            let mut d = Document::init(context, "document").await;
            apply_commit(&mut d, &DocMsg::CreateDoc { doc_id: "doc1".into() }).await;
            for (id, t) in [("b1", "one"), ("b2", "two"), ("b3", "three")] {
                let after = if id == "b1" { None } else { Some(format!("b{}", id.chars().last().unwrap().to_digit(10).unwrap() - 1)) };
                apply_commit(&mut d, &DocMsg::InsertBlock { doc_id: "doc1".into(), after, block: blk(id, t) }).await;
            }
            // start: b1,b2,b3. move b1 after b3 -> b2,b3,b1.
            apply_commit(&mut d, &DocMsg::MoveBlock { doc_id: "doc1".into(), block_id: "b1".into(), after: Some("b3".into()) }).await;
            let doc = get_doc(&d, "doc1").await.unwrap();
            let ids: Vec<&str> = doc.iter().map(|b| b.id.as_str()).collect();
            assert_eq!(ids, ["b2", "b3", "b1"]);

            // move b1 to the front (after None) -> b1,b2,b3.
            apply_commit(&mut d, &DocMsg::MoveBlock { doc_id: "doc1".into(), block_id: "b1".into(), after: None }).await;
            let doc = get_doc(&d, "doc1").await.unwrap();
            let ids: Vec<&str> = doc.iter().map(|b| b.id.as_str()).collect();
            assert_eq!(ids, ["b1", "b2", "b3"]);
        });
    }

    #[test]
    fn two_docs_are_independent() {
        deterministic::Runner::default().start(|context| async move {
            let mut d = Document::init(context, "document").await;
            apply_commit(&mut d, &DocMsg::CreateDoc { doc_id: "a".into() }).await;
            apply_commit(&mut d, &DocMsg::CreateDoc { doc_id: "b".into() }).await;
            apply_commit(&mut d, &DocMsg::InsertBlock { doc_id: "a".into(), after: None, block: blk("x", "in-a") }).await;
            apply_commit(&mut d, &DocMsg::InsertBlock { doc_id: "b".into(), after: None, block: blk("y", "in-b") }).await;
            let da = get_doc(&d, "a").await.unwrap();
            let db = get_doc(&d, "b").await.unwrap();
            assert_eq!(da.len(), 1);
            assert_eq!(da[0].id, "x");
            assert_eq!(db.len(), 1);
            assert_eq!(db[0].id, "y");
        });
    }

    #[test]
    fn write_moves_root_and_is_real_qmdb_root() {
        deterministic::Runner::default().start(|context| async move {
            let mut d = Document::init(context, "document").await;
            let r0 = d.root();
            apply_commit(&mut d, &DocMsg::CreateDoc { doc_id: "doc1".into() }).await;
            apply_commit(&mut d, &DocMsg::InsertBlock { doc_id: "doc1".into(), after: None, block: blk("b1", "hi") }).await;
            let r1 = d.root();
            assert_ne!(r0, r1, "a write must move the root");
            assert_ne!(r1, StateRoot::ZERO, "root after write must be non-zero");

            // the document root genuinely composes into the global app-hash.
            struct Stub;
            #[async_trait::async_trait(?Send)]
            impl Module for Stub {
                fn id(&self) -> ModuleId { "stub".into() }
                fn root(&self) -> StateRoot { StateRoot([9u8; sdk::ROOT_LEN]) }
                async fn execute(&mut self, _c: &mut dyn Ctx, _m: &Msg) -> Result<(), Error> { Ok(()) }
            }
            let stub = Stub;
            let g = { let mods: [&dyn Module; 2] = [&d, &stub]; global_root(&mods) };
            assert_ne!(g, state::global_root(&[&stub as &dyn Module]));
        });
    }

    // host-lent staging: a write staged in a block that then ABORTS must leave no
    // trace — root() unchanged, and the doc invisible to a later query.
    #[test]
    fn staged_write_in_failing_block_rolls_back() {
        deterministic::Runner::default().start(|context| async move {
            let mut d = Document::init(context, "document").await;
            apply_commit(&mut d, &DocMsg::CreateDoc { doc_id: "doc1".into() }).await;
            let r_before = d.root();

            // stage an insert, then abort instead of commit (as the host does when a
            // later op in the block errors).
            d.execute(&mut TestCtx::new(), &msg(&DocMsg::InsertBlock {
                doc_id: "doc1".into(), after: None, block: blk("ghost", "should vanish"),
            })).await.unwrap();
            d.abort_block().await.unwrap();

            assert_eq!(d.root(), r_before, "aborted block must not move the root");
            let doc = get_doc(&d, "doc1").await.unwrap();
            assert!(doc.is_empty(), "the staged block must have rolled back");
        });
    }

    // errors: a block op on an absent doc, a dup insert, and a bad anchor all fail
    // (so the host aborts the block).
    #[test]
    fn error_paths() {
        deterministic::Runner::default().start(|context| async move {
            let mut d = Document::init(context, "document").await;
            // insert before create -> DocNotFound.
            let e = d.execute(&mut TestCtx::new(), &msg(&DocMsg::InsertBlock {
                doc_id: "nope".into(), after: None, block: blk("b1", "x"),
            })).await;
            assert!(e.is_err());
            d.abort_block().await.unwrap();

            apply_commit(&mut d, &DocMsg::CreateDoc { doc_id: "doc1".into() }).await;
            apply_commit(&mut d, &DocMsg::InsertBlock { doc_id: "doc1".into(), after: None, block: blk("b1", "x") }).await;
            // duplicate id.
            let e = d.execute(&mut TestCtx::new(), &msg(&DocMsg::InsertBlock {
                doc_id: "doc1".into(), after: None, block: blk("b1", "dup"),
            })).await;
            assert!(e.is_err());
            d.abort_block().await.unwrap();
            // bad anchor.
            let e = d.execute(&mut TestCtx::new(), &msg(&DocMsg::InsertBlock {
                doc_id: "doc1".into(), after: Some("ghost".into()), block: blk("b2", "x"),
            })).await;
            assert!(e.is_err());
            d.abort_block().await.unwrap();
        });
    }

    // mid-block read-your-writes: two inserts in ONE block (single commit) — op2's
    // `load` must see op1's STAGED bytes through the pending overlay, before any
    // commit reaches qmdb. this is the core of the host-lent staging pattern.
    #[test]
    fn staged_writes_are_visible_within_one_block() {
        deterministic::Runner::default().start(|context| async move {
            let mut d = Document::init(context, "document").await;
            apply_commit(&mut d, &DocMsg::CreateDoc { doc_id: "doc1".into() }).await;
            // two inserts, NO commit between them: op2 anchors on b1, which only
            // exists in op1's staged (uncommitted) write.
            d.execute(&mut TestCtx::new(), &msg(&DocMsg::InsertBlock {
                doc_id: "doc1".into(), after: None, block: blk("b1", "one"),
            })).await.unwrap();
            d.execute(&mut TestCtx::new(), &msg(&DocMsg::InsertBlock {
                doc_id: "doc1".into(), after: Some("b1".into()), block: blk("b2", "two"),
            })).await.unwrap();
            d.commit_block().await.unwrap();
            let doc = get_doc(&d, "doc1").await.unwrap();
            let ids: Vec<&str> = doc.iter().map(|b| b.id.as_str()).collect();
            assert_eq!(ids, ["b1", "b2"], "op2 must have seen op1's staged write");
        });
    }

    #[test]
    fn create_is_idempotent() {
        deterministic::Runner::default().start(|context| async move {
            let mut d = Document::init(context, "document").await;
            apply_commit(&mut d, &DocMsg::CreateDoc { doc_id: "doc1".into() }).await;
            apply_commit(&mut d, &DocMsg::InsertBlock { doc_id: "doc1".into(), after: None, block: blk("b1", "keep") }).await;
            // re-create must NOT wipe the existing block.
            apply_commit(&mut d, &DocMsg::CreateDoc { doc_id: "doc1".into() }).await;
            let doc = get_doc(&d, "doc1").await.unwrap();
            assert_eq!(doc.len(), 1);
            assert_eq!(doc[0].id, "b1");
        });
    }
}
