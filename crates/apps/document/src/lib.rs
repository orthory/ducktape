//! qmdb-backed document module — ducktape's founding product, reborn simple.
//!
//! a document is an ORDERED LIST of [`Block`]s (no markdown), keyed by `doc_id`.
//! many documents live in ONE qmdb `any/unordered/variable` database: the qmdb
//! key is `sha256(doc_id)` and the value is the whole document serialized as a
//! json `Vec<Block>` (whole-doc-per-key — the simple MVP; a per-block ordered-KV
//! is a later optimization). the module's authenticated [`StateRoot`] IS the
//! qmdb merkle root, refreshed on every committed write, so it folds straight
//! into the global app-hash next to a git HEAD oid or another qmdb root.
//!
//! ## keys are hashed to a fixed width
//!
//! the logical key is the `doc_id` string at the interface seam, but the qmdb
//! key is `sha256(doc_id)` — a fixed 32-byte [`commonware_utils`] `Array`. this
//! mirrors the kv module and is load-bearing: commonware's state-sync resolvers
//! for the overwriteable variable db are bounded on `K: Array`. a hashed key
//! commits to `hash(doc_id) -> doc` and so can't be walked to recover the doc
//! ids themselves.
//!
//! ## enumeration via a reserved index entry
//!
//! to make the store BROWSABLE (a filesystem-like reader over `/`-delimited
//! path ids) without breaking the fixed-width-key contract, one extra qmdb entry
//! is reserved: a sentinel logical key [`DOC_INDEX_KEY`] whose value is the
//! serialized SORTED set of every known `doc_id`. its qmdb key is still
//! `sha256(sentinel)` — a 32-byte digest, indistinguishable in width from any
//! doc slot — so it rides through state-sync exactly like a document. a real
//! `doc_id` can never collide with the sentinel (it carries a leading NUL that
//! the id slugger can't produce) and every doc op that names the sentinel is
//! rejected. [`DocMsg::CreateDoc`] adds the id to the index (idempotent);
//! [`DocQuery::ListDocs`] reads it back. block edits never touch the index —
//! only doc creation grows it — so per-edit cost is unchanged.
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
//!
//! ## state-sync
//!
//! a joiner (dynamic-valset catch-up, a fresh full node, crash recovery) rebuilds
//! this store from a peer via [`Document::sync_target`] / [`Document::sync_from`],
//! delegating to commonware's qmdb `sync` engine: the reconstructed store's root
//! equals the source root, and every fetched batch is merkle-verified against
//! that root, so the source is untrusted — the root is the trust anchor.

use std::collections::BTreeMap;
use std::num::{NonZeroU16, NonZeroU64, NonZeroUsize};
use std::sync::Arc;

use commonware_codec::RangeCfg;
use commonware_cryptography::{Hasher, Sha256};
use commonware_parallel::Sequential;
use commonware_runtime::{BufferPooler, buffer::paged::CacheRef};
use commonware_storage::{
    Context, journal, mmr,
    qmdb::{
        any::{VariableConfig, unordered::variable::Db},
        sync::{self, DbResolver, Target, engine::Config as SyncConfig},
    },
    translator::TwoCap,
};
use commonware_utils::range::NonEmptyRange;

use document_interface::{
    Block, DocMsg, DocQuery, DocReply, decode_msg, decode_query, encode_reply,
};
use sdk::{Ctx, Error, Module, ModuleId, Msg, ResolverSyncTarget, StateRoot, StateSyncHandle};

/// write-time cap on a SERIALIZED document. [`doc_config`]'s codec [`RangeCfg`]
/// bounds a stored value at 1 MiB AT DECODE TIME only — an oversized doc would
/// stage fine, sail through the expect()-panicking commit path, and then panic
/// every later read of that doc (and any log replay / sync batch decode) on
/// every validator: a poison pill. rejecting at write time keeps it out of the
/// log entirely. 768 KiB leaves a 256 KiB margin under the codec bound so the
/// whole serialized operation (hashed key, varint length prefix, framing) — and
/// the NEXT small edit to an at-cap doc — stays comfortably under 1 MiB.
pub const MAX_DOC_LEN: usize = 768 * 1024;

/// the reserved logical key under which the enumeration INDEX rides in the same
/// qmdb. its value is a serialized sorted `Vec<String>` of every known `doc_id`.
///
/// the leading NUL makes it UNCOLLIDABLE with a real `doc_id`: the client id
/// slugger emits only `[a-z0-9/-]`, and every doc op that names this key is
/// rejected ([`DocError::ReservedDocId`]) before it can reach storage — so the
/// index can never be clobbered by a document write, on any validator.
///
/// GROWTH BOUND: the index value is capped by [`MAX_DOC_LEN`] like any doc
/// value (it stages through the same [`Document::stage`] guard). a `doc_id`
/// costs its own bytes plus json framing (two quotes + a comma ≈ 3 bytes), so a
/// 768 KiB index holds on the order of ten-thousand-plus ids before a
/// `CreateDoc` starts failing with [`DocError::DocTooLarge`]. a store that needs
/// more would shard the index across several sentinel keys — out of scope for
/// the whole-doc-per-key MVP.
const DOC_INDEX_KEY: &str = "\u{0}doc-index";

/// the qmdb key: a fixed 32-byte sha256 digest of the `doc_id`. fixed width is
/// what lets a store be state-synced (commonware's resolvers require `K: Array`).
type DocKey = <Sha256 as Hasher>::Digest;

/// the concrete qmdb store — 32-byte hashed doc-id keys, variable byte values,
/// sha256 hasher, two-byte translator, sequential (deterministic) merkle
/// strategy. identical params to the kv module's `KvDb`, so all qmdb plumbing is
/// shared verbatim.
type DocDb<E> = Db<mmr::Family, E, DocKey, Vec<u8>, Sha256, TwoCap, Sequential>;

/// the qmdb configuration for a document store — shared by [`Document::init`]
/// (fresh open) and [`Document::sync_from`] (state-sync target) so a synced
/// store's storage layout is byte-identical to a freshly-opened one. the key
/// codec cfg is `()` (fixed width); only the variable value carries a
/// [`RangeCfg`].
type DocConfig = VariableConfig<TwoCap, ((), (RangeCfg<usize>, ())), Sequential>;

/// a state-sync target: a qmdb merkle root plus the operation range a joiner
/// must pull to reconstruct a store with an identical root. produced by
/// [`Document::sync_target`], consumed by [`Document::sync_from`].
pub type DocTarget = Target<mmr::Family, DocKey>;

/// hash a `doc_id` to its fixed-width qmdb key. deterministic, so every
/// validator maps a given doc to the same store slot.
fn hash_key(doc_id: &[u8]) -> DocKey {
    let mut h = Sha256::new();
    h.update(doc_id);
    h.finalize()
}

/// build the qmdb [`VariableConfig`] for module `id` on `context`. partitions
/// are namespaced by `id` so several qmdb-backed modules can share one runtime
/// context without colliding on storage. the single source of truth for a
/// document store's storage layout, so [`Document::init`] and
/// [`Document::sync_from`] can never drift apart.
fn doc_config<E>(context: &E, id: &str) -> DocConfig
where
    E: Context + BufferPooler,
{
    // a single page-cache handle shared by both sub-configs (cheap to clone).
    let page_cache = CacheRef::from_pooler(
        context,
        NonZeroU16::new(128).unwrap(),
        NonZeroUsize::new(64).unwrap(),
    );

    // codec config for Operation<.., DocKey, Vec<u8>>: (key_cfg, value_cfg). the
    // key is a fixed-width digest so its cfg is `()`; the value is a Vec<u8>
    // whose <Vec<u8> as Read>::Cfg == (RangeCfg<usize>, ()). bound generously;
    // a whole serialized doc rides in one value.
    let codec_config = ((), (RangeCfg::from(0..=1 << 20), ()));

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
    }
}

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
    /// the op would grow the serialized doc past [`MAX_DOC_LEN`] — rejected at
    /// write time so the oversized bytes never reach the panicking commit/read
    /// paths (the codec bound is decode-only).
    DocTooLarge,
    /// a stored doc failed to decode. distinct from [`DocError::DocNotFound`]:
    /// corruption must surface loudly, never masquerade as an absent doc.
    DocCorrupt,
    /// a doc op named the reserved [`DOC_INDEX_KEY`] sentinel. rejected so the
    /// enumeration index can never be overwritten by a document write.
    ReservedDocId,
}

impl core::fmt::Display for DocError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let s = match self {
            DocError::DocNotFound => "doc not found",
            DocError::DuplicateBlock => "duplicate block id",
            DocError::BlockNotFound => "block not found",
            DocError::AnchorNotFound => "after-anchor not found",
            DocError::DocTooLarge => "doc too large",
            DocError::DocCorrupt => "stored doc is corrupt",
            DocError::ReservedDocId => "reserved doc id",
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
    /// docs written this block, keyed by LOGICAL `doc_id` bytes -> serialized
    /// `Vec<Block>`. read ahead of committed state by `get` (read-your-writes)
    /// and flushed to qmdb (under the hashed key) in one batch by
    /// `commit_block`; NOT reflected in `root()` until then.
    pending: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl<E> Document<E>
where
    E: Context + BufferPooler,
{
    /// open (or recover) the store on `context` under module identity `id`.
    /// qmdb partitions are namespaced by `id`, so a document module shares one
    /// runtime context with kv/other qmdb modules without colliding — the demo
    /// hookup is purely additive.
    pub async fn init(context: E, id: impl Into<ModuleId>) -> Self {
        let id = id.into();
        let cfg = doc_config(&context, &id);
        let db = DocDb::<E>::init(context, cfg)
            .await
            .expect("qmdb init failed");
        Self {
            id,
            db,
            pending: BTreeMap::new(),
        }
    }

    /// read raw bytes for `key`: a STAGED (this-block) write shadows committed
    /// qmdb state, so a later op in the same block sees an earlier staged write.
    /// committed reads go through the hashed key.
    async fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        if let Some(v) = self.pending.get(key) {
            return Some(v.clone());
        }
        self.db.get(&hash_key(key)).await.expect("get failed")
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

    /// stage arbitrary serialized `bytes` under logical `key` for this block
    /// WITHOUT committing. visible to `get`/`load` at once; folded into qmdb (and
    /// `root()`) only when the host calls `commit_block`. rejects a value over
    /// [`MAX_DOC_LEN`] BEFORE staging — the shared poison-pill guard for BOTH
    /// whole-doc values and the enumeration index value (the codec bound is
    /// decode-only, so an oversized value must never reach the panicking
    /// commit/read paths).
    fn stage(&mut self, key: &str, bytes: Vec<u8>) -> Result<(), DocError> {
        if bytes.len() > MAX_DOC_LEN {
            return Err(DocError::DocTooLarge);
        }
        self.pending.insert(key.as_bytes().to_vec(), bytes);
        Ok(())
    }

    /// stage a document's serialized blocks for this block. rejects a doc whose
    /// serialized form exceeds [`MAX_DOC_LEN`] BEFORE staging, so a failed op
    /// leaves no overlay entry.
    fn store(&mut self, doc_id: &str, blocks: &[Block]) -> Result<(), DocError> {
        let bytes = serde_json::to_vec(blocks).expect("Vec<Block> is always serializable");
        self.stage(doc_id, bytes)
    }

    /// load the enumeration index — the sorted set of known `doc_id`s — through
    /// the staged-over-committed overlay. an absent index (never written yet)
    /// reads as the empty set. a decode failure is corruption, surfaced by the
    /// caller, never masqueraded as "no docs".
    async fn load_index(&self) -> Result<Vec<String>, Error> {
        match self.get(DOC_INDEX_KEY.as_bytes()).await {
            Some(b) => serde_json::from_slice(&b).map_err(|e| Error::Module(e.to_string())),
            None => Ok(Vec::new()),
        }
    }

    /// add `doc_id` to the enumeration index if absent, re-staging it as a sorted
    /// set. idempotent: re-creating a known id restages an IDENTICAL value (a
    /// benign no-op write). the SORT makes the serialized bytes canonical, so
    /// every validator commits the same index bytes and lands on the same qmdb
    /// root. capped by [`MAX_DOC_LEN`] through [`Document::stage`] (see the
    /// growth bound on [`DOC_INDEX_KEY`]).
    async fn index_add(&mut self, doc_id: &str) -> Result<(), DocError> {
        let mut ids = self.load_index().await.map_err(to_doc_err)?;
        if !ids.iter().any(|id| id == doc_id) {
            ids.push(doc_id.to_string());
            ids.sort();
            let bytes = serde_json::to_vec(&ids).expect("Vec<String> is always serializable");
            self.stage(DOC_INDEX_KEY, bytes)?;
        }
        Ok(())
    }

    /// apply one decoded [`DocMsg`] to the staged overlay. pure list surgery over
    /// the loaded `Vec<Block>`, re-staged on success. errors abort the block.
    async fn apply(&mut self, msg: DocMsg) -> Result<(), DocError> {
        // every op carries a doc_id; none may target the reserved index sentinel.
        // reject deterministically on every validator BEFORE any storage touch,
        // so a document write can never clobber the enumeration index.
        let doc_id = match &msg {
            DocMsg::CreateDoc { doc_id }
            | DocMsg::InsertBlock { doc_id, .. }
            | DocMsg::UpdateBlock { doc_id, .. }
            | DocMsg::RemoveBlock { doc_id, .. }
            | DocMsg::MoveBlock { doc_id, .. } => doc_id,
        };
        if doc_id == DOC_INDEX_KEY {
            return Err(DocError::ReservedDocId);
        }

        match msg {
            DocMsg::CreateDoc { doc_id } => {
                // record the id in the enumeration index (idempotent) so the doc
                // becomes browsable. block ops require CreateDoc first, so every
                // doc that can hold blocks is already indexed by the time they run
                // — the index only ever grows on creation, never on an edit.
                self.index_add(&doc_id).await?;
                // idempotent: only seed an empty doc if absent. an empty doc is a
                // stored `[]`; ABSENT is `None` — that distinction is why CreateDoc
                // is its own op and why block ops require it first.
                if self.load(&doc_id).await.map_err(to_doc_err)?.is_none() {
                    self.store(&doc_id, &[])?;
                }
                Ok(())
            }
            DocMsg::InsertBlock {
                doc_id,
                after,
                block,
            } => {
                let mut d = self
                    .load(&doc_id)
                    .await
                    .map_err(to_doc_err)?
                    .ok_or(DocError::DocNotFound)?;
                if d.iter().any(|b| b.id == block.id) {
                    return Err(DocError::DuplicateBlock);
                }
                let i = idx_after(&d, &after)?;
                d.insert(i, block);
                self.store(&doc_id, &d)?;
                Ok(())
            }
            DocMsg::UpdateBlock {
                doc_id,
                block_id,
                text,
            } => {
                let mut d = self
                    .load(&doc_id)
                    .await
                    .map_err(to_doc_err)?
                    .ok_or(DocError::DocNotFound)?;
                let b = d
                    .iter_mut()
                    .find(|b| b.id == block_id)
                    .ok_or(DocError::BlockNotFound)?;
                b.text = text;
                self.store(&doc_id, &d)?;
                Ok(())
            }
            DocMsg::RemoveBlock { doc_id, block_id } => {
                let mut d = self
                    .load(&doc_id)
                    .await
                    .map_err(to_doc_err)?
                    .ok_or(DocError::DocNotFound)?;
                let pos = d
                    .iter()
                    .position(|b| b.id == block_id)
                    .ok_or(DocError::BlockNotFound)?;
                d.remove(pos);
                self.store(&doc_id, &d)?;
                Ok(())
            }
            DocMsg::MoveBlock {
                doc_id,
                block_id,
                after,
            } => {
                // self-anchor is a no-op — resolve it BEFORE removal, else the
                // remove-then-lookup would fail with a bogus AnchorNotFound.
                if after.as_deref() == Some(block_id.as_str()) {
                    return Ok(());
                }
                let mut d = self
                    .load(&doc_id)
                    .await
                    .map_err(to_doc_err)?
                    .ok_or(DocError::DocNotFound)?;
                let pos = d
                    .iter()
                    .position(|b| b.id == block_id)
                    .ok_or(DocError::BlockNotFound)?;
                let blk = d.remove(pos);
                // anchor index is computed in the now-shortened list.
                let i = idx_after(&d, &after)?;
                d.insert(i, blk);
                self.store(&doc_id, &d)?;
                Ok(())
            }
        }
    }

    // ---- state-sync ---------------------------------------------------------
    // reconstruct a byte-identical-rooted store from a peer WITHOUT replaying the
    // op history in application order — commonware's qmdb sync ships the live op
    // range and merkle-verifies every batch against the target root.

    /// the sync [`DocTarget`] for this store: its qmdb merkle root plus the LIVE
    /// operation range `[sync_boundary, end)`. hand it to [`Document::sync_from`]
    /// to rebuild a store with an identical root. async only because `bounds()`
    /// reads the committed log tail.
    ///
    /// the range starts at `sync_boundary()`, not `0`: qmdb compacts overwritten
    /// history below its inactivity floor, so only the active tail ships (pinned
    /// merkle nodes cover the pruned prefix). that IS checkpoint semantics — the
    /// snapshot half of snapshot-plus-replay-tail.
    pub async fn sync_target(&self) -> DocTarget {
        let end = self.db.bounds().await.end;
        let start = self.db.sync_boundary();
        Target {
            root: self.db.root(),
            range: NonEmptyRange::new(start..end)
                .expect("a committed store has a non-empty op range"),
        }
    }

    /// consume this store into an `Arc`-wrapped raw qmdb that serves as a sync
    /// resolver: it answers a joiner's op-range requests with proof-carrying
    /// batches. a LIVE source still taking writes would instead wrap
    /// `Arc<AsyncRwLock<..>>`; this consuming form is the handoff / test source.
    pub fn into_resolver(self) -> Arc<DocDb<E>> {
        Arc::new(self.db)
    }

    /// reconstruct a `Document` at `id` on `context` whose qmdb root EQUALS
    /// `target.root`, by pulling `target`'s op range from `resolver`.
    /// commonware's sync engine merkle-verifies every fetched batch against
    /// `target.root`, so a byzantine source cannot produce a store with a
    /// matching root but forged contents — the root is the trust anchor. reuses
    /// [`doc_config`] so the synced store's storage layout matches a
    /// freshly-opened one.
    pub async fn sync_from<R>(
        context: E,
        id: impl Into<ModuleId>,
        target: DocTarget,
        resolver: R,
    ) -> Result<Self, String>
    where
        R: DbResolver<DocDb<E>>,
    {
        let id = id.into();
        let db_config = doc_config(&context, &id);
        let config = SyncConfig {
            context,
            resolver,
            target,
            max_outstanding_requests: 1,
            fetch_batch_size: NonZeroU64::new(64).unwrap(),
            apply_batch_size: 1024,
            db_config,
            update_rx: None,
            finish_rx: None,
            reached_target_tx: None,
            max_retained_roots: 8,
        };
        // a sync failure (transport blip, dropped source) is the caller's
        // retry loop to own — never a process kill.
        let db = sync::sync(config)
            .await
            .map_err(|e| format!("qmdb sync: {e:?}"))?;
        Ok(Self {
            id,
            db,
            pending: BTreeMap::new(),
        })
    }
}

/// bridge the only sdk error `load` can raise — a stored-doc json decode failure
/// — back into `DocError` so `apply` stays single-error-typed. unreachable for
/// our own writes (we only ever store valid `Vec<Block>` json), but if it ever
/// fires it MUST surface as corruption, not [`DocError::DocNotFound`]: mapping a
/// decode failure to "absent" would let `CreateDoc` silently re-seed an empty
/// doc over the corrupt bytes, destroying the evidence AND the data.
fn to_doc_err(_e: Error) -> DocError {
    DocError::DocCorrupt
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

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::ResolverBacked {
            backend: "qmdb".into(),
            detail: "serve_sync answers qmdb op-range requests (statesync wire)".into(),
        })
    }

    /// the network state-sync serve lane: answers the shared qmdb wire requests
    /// (historical proof-carrying op ranges) from committed state. read-only;
    /// the joiner's sync engine merkle-verifies every batch.
    async fn serve_sync(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        statesync::qmdb::serve_bytes(&self.db, req).await
    }

    async fn resolver_sync_target(&self) -> Result<ResolverSyncTarget, Error> {
        statesync::qmdb::resolver_sync_target(&self.db).await
    }

    /// decode a [`DocMsg`] and apply it to the staged overlay. the only `.await`
    /// is on own qmdb state — deterministic, so replay-safe across validators.
    async fn execute(&mut self, _ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let m = decode_msg(&msg.payload).map_err(Error::Module)?;
        self.apply(m)
            .await
            .map_err(|e| Error::Module(e.to_string()))
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
            DocQuery::ListDocs => {
                // served straight from the reserved index entry — the sorted set
                // of every known doc_id. absent index reads as an empty list.
                Ok(encode_reply(&DocReply::DocList(self.load_index().await?)))
            }
        }
    }

    /// publish the block's staged docs in ONE qmdb batch: write every pending
    /// doc (under its hashed key), merkleize, apply, commit. no-op (and no root
    /// movement) if nothing was staged. byte-identical to the kv commit path.
    async fn commit_block(&mut self) -> Result<(), Error> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let mut batch = self.db.new_batch();
        for (key, value) in &self.pending {
            batch = batch.write(hash_key(key), Some(value.clone()));
        }
        let batch = batch
            .merkleize(&self.db, None::<Vec<u8>>)
            .await
            .expect("merkleize failed");
        self.db
            .apply_batch(batch)
            .await
            .expect("apply_batch failed");
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
    use commonware_runtime::{Runner as _, deterministic};
    use document_interface::{BlockKind, decode_reply, encode_msg, encode_query};
    use state::global_root;

    fn blk(id: &str, text: &str) -> Block {
        Block {
            id: id.into(),
            kind: BlockKind::Paragraph,
            text: text.into(),
        }
    }

    fn msg(m: &DocMsg) -> Msg {
        Msg {
            target: "document".into(),
            payload: encode_msg(m),
        }
    }

    // a minimal Ctx so execute can be driven without a full host.
    struct TestCtx {
        env: sdk::Env,
    }
    impl TestCtx {
        fn new() -> Self {
            Self {
                env: sdk::Env { protocol_version: 0,
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
        fn env(&self) -> &sdk::Env {
            &self.env
        }
        fn module_root(&self, _t: &str) -> Option<StateRoot> {
            None
        }
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

    async fn get_doc<E: Context + BufferPooler>(
        d: &Document<E>,
        doc_id: &str,
    ) -> Option<Vec<Block>> {
        let reply = d
            .query(&encode_query(&DocQuery::GetDoc {
                doc_id: doc_id.into(),
            }))
            .await
            .unwrap();
        match decode_reply(&reply).unwrap() {
            DocReply::Doc(v) => v,
            _ => panic!("expected Doc"),
        }
    }

    async fn list_docs<E: Context + BufferPooler>(d: &Document<E>) -> Vec<String> {
        let reply = d.query(&encode_query(&DocQuery::ListDocs)).await.unwrap();
        match decode_reply(&reply).unwrap() {
            DocReply::DocList(ids) => ids,
            _ => panic!("expected DocList"),
        }
    }

    #[test]
    fn create_insert_returns_blocks_in_order() {
        deterministic::Runner::default().start(|context| async move {
            let mut d = Document::init(context, "document").await;
            apply_commit(
                &mut d,
                &DocMsg::CreateDoc {
                    doc_id: "doc1".into(),
                },
            )
            .await;
            apply_commit(
                &mut d,
                &DocMsg::InsertBlock {
                    doc_id: "doc1".into(),
                    after: None,
                    block: blk("b1", "first"),
                },
            )
            .await;
            // after b1 -> b2 lands at the end.
            apply_commit(
                &mut d,
                &DocMsg::InsertBlock {
                    doc_id: "doc1".into(),
                    after: Some("b1".into()),
                    block: blk("b2", "second"),
                },
            )
            .await;
            // after None -> front, so b0 goes before b1.
            apply_commit(
                &mut d,
                &DocMsg::InsertBlock {
                    doc_id: "doc1".into(),
                    after: None,
                    block: blk("b0", "zero"),
                },
            )
            .await;

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
            apply_commit(
                &mut d,
                &DocMsg::CreateDoc {
                    doc_id: "doc1".into(),
                },
            )
            .await;
            apply_commit(
                &mut d,
                &DocMsg::InsertBlock {
                    doc_id: "doc1".into(),
                    after: None,
                    block: blk("b1", "old"),
                },
            )
            .await;
            apply_commit(
                &mut d,
                &DocMsg::UpdateBlock {
                    doc_id: "doc1".into(),
                    block_id: "b1".into(),
                    text: "new".into(),
                },
            )
            .await;
            let doc = get_doc(&d, "doc1").await.unwrap();
            assert_eq!(doc[0].text, "new");
        });
    }

    #[test]
    fn remove_drops_a_block() {
        deterministic::Runner::default().start(|context| async move {
            let mut d = Document::init(context, "document").await;
            apply_commit(
                &mut d,
                &DocMsg::CreateDoc {
                    doc_id: "doc1".into(),
                },
            )
            .await;
            apply_commit(
                &mut d,
                &DocMsg::InsertBlock {
                    doc_id: "doc1".into(),
                    after: None,
                    block: blk("b1", "one"),
                },
            )
            .await;
            apply_commit(
                &mut d,
                &DocMsg::InsertBlock {
                    doc_id: "doc1".into(),
                    after: Some("b1".into()),
                    block: blk("b2", "two"),
                },
            )
            .await;
            apply_commit(
                &mut d,
                &DocMsg::RemoveBlock {
                    doc_id: "doc1".into(),
                    block_id: "b1".into(),
                },
            )
            .await;
            let doc = get_doc(&d, "doc1").await.unwrap();
            let ids: Vec<&str> = doc.iter().map(|b| b.id.as_str()).collect();
            assert_eq!(ids, ["b2"]);
        });
    }

    #[test]
    fn move_reorders_blocks() {
        deterministic::Runner::default().start(|context| async move {
            let mut d = Document::init(context, "document").await;
            apply_commit(
                &mut d,
                &DocMsg::CreateDoc {
                    doc_id: "doc1".into(),
                },
            )
            .await;
            for (id, t) in [("b1", "one"), ("b2", "two"), ("b3", "three")] {
                let after = if id == "b1" {
                    None
                } else {
                    Some(format!(
                        "b{}",
                        id.chars().last().unwrap().to_digit(10).unwrap() - 1
                    ))
                };
                apply_commit(
                    &mut d,
                    &DocMsg::InsertBlock {
                        doc_id: "doc1".into(),
                        after,
                        block: blk(id, t),
                    },
                )
                .await;
            }
            // start: b1,b2,b3. move b1 after b3 -> b2,b3,b1.
            apply_commit(
                &mut d,
                &DocMsg::MoveBlock {
                    doc_id: "doc1".into(),
                    block_id: "b1".into(),
                    after: Some("b3".into()),
                },
            )
            .await;
            let doc = get_doc(&d, "doc1").await.unwrap();
            let ids: Vec<&str> = doc.iter().map(|b| b.id.as_str()).collect();
            assert_eq!(ids, ["b2", "b3", "b1"]);

            // move b1 to the front (after None) -> b1,b2,b3.
            apply_commit(
                &mut d,
                &DocMsg::MoveBlock {
                    doc_id: "doc1".into(),
                    block_id: "b1".into(),
                    after: None,
                },
            )
            .await;
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
            apply_commit(
                &mut d,
                &DocMsg::InsertBlock {
                    doc_id: "a".into(),
                    after: None,
                    block: blk("x", "in-a"),
                },
            )
            .await;
            apply_commit(
                &mut d,
                &DocMsg::InsertBlock {
                    doc_id: "b".into(),
                    after: None,
                    block: blk("y", "in-b"),
                },
            )
            .await;
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
            apply_commit(
                &mut d,
                &DocMsg::CreateDoc {
                    doc_id: "doc1".into(),
                },
            )
            .await;
            apply_commit(
                &mut d,
                &DocMsg::InsertBlock {
                    doc_id: "doc1".into(),
                    after: None,
                    block: blk("b1", "hi"),
                },
            )
            .await;
            let r1 = d.root();
            assert_ne!(r0, r1, "a write must move the root");
            assert_ne!(r1, StateRoot::ZERO, "root after write must be non-zero");

            // the document root genuinely composes into the global app-hash.
            struct Stub;
            #[async_trait::async_trait(?Send)]
            impl Module for Stub {
                fn id(&self) -> ModuleId {
                    "stub".into()
                }
                fn root(&self) -> StateRoot {
                    StateRoot([9u8; sdk::ROOT_LEN])
                }
                async fn execute(&mut self, _c: &mut dyn Ctx, _m: &Msg) -> Result<(), Error> {
                    Ok(())
                }
            }
            let stub = Stub;
            let g = {
                let mods: [&dyn Module; 2] = [&d, &stub];
                global_root(&mods)
            };
            assert_ne!(g, state::global_root(&[&stub as &dyn Module]));
        });
    }

    // host-lent staging: a write staged in a block that then ABORTS must leave no
    // trace — root() unchanged, and the doc invisible to a later query.
    #[test]
    fn staged_write_in_failing_block_rolls_back() {
        deterministic::Runner::default().start(|context| async move {
            let mut d = Document::init(context, "document").await;
            apply_commit(
                &mut d,
                &DocMsg::CreateDoc {
                    doc_id: "doc1".into(),
                },
            )
            .await;
            let r_before = d.root();

            // stage an insert, then abort instead of commit (as the host does when a
            // later op in the block errors).
            d.execute(
                &mut TestCtx::new(),
                &msg(&DocMsg::InsertBlock {
                    doc_id: "doc1".into(),
                    after: None,
                    block: blk("ghost", "should vanish"),
                }),
            )
            .await
            .unwrap();
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
            let e = d
                .execute(
                    &mut TestCtx::new(),
                    &msg(&DocMsg::InsertBlock {
                        doc_id: "nope".into(),
                        after: None,
                        block: blk("b1", "x"),
                    }),
                )
                .await;
            assert!(e.is_err());
            d.abort_block().await.unwrap();

            apply_commit(
                &mut d,
                &DocMsg::CreateDoc {
                    doc_id: "doc1".into(),
                },
            )
            .await;
            apply_commit(
                &mut d,
                &DocMsg::InsertBlock {
                    doc_id: "doc1".into(),
                    after: None,
                    block: blk("b1", "x"),
                },
            )
            .await;
            // duplicate id.
            let e = d
                .execute(
                    &mut TestCtx::new(),
                    &msg(&DocMsg::InsertBlock {
                        doc_id: "doc1".into(),
                        after: None,
                        block: blk("b1", "dup"),
                    }),
                )
                .await;
            assert!(e.is_err());
            d.abort_block().await.unwrap();
            // bad anchor.
            let e = d
                .execute(
                    &mut TestCtx::new(),
                    &msg(&DocMsg::InsertBlock {
                        doc_id: "doc1".into(),
                        after: Some("ghost".into()),
                        block: blk("b2", "x"),
                    }),
                )
                .await;
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
            apply_commit(
                &mut d,
                &DocMsg::CreateDoc {
                    doc_id: "doc1".into(),
                },
            )
            .await;
            // two inserts, NO commit between them: op2 anchors on b1, which only
            // exists in op1's staged (uncommitted) write.
            d.execute(
                &mut TestCtx::new(),
                &msg(&DocMsg::InsertBlock {
                    doc_id: "doc1".into(),
                    after: None,
                    block: blk("b1", "one"),
                }),
            )
            .await
            .unwrap();
            d.execute(
                &mut TestCtx::new(),
                &msg(&DocMsg::InsertBlock {
                    doc_id: "doc1".into(),
                    after: Some("b1".into()),
                    block: blk("b2", "two"),
                }),
            )
            .await
            .unwrap();
            d.commit_block().await.unwrap();
            let doc = get_doc(&d, "doc1").await.unwrap();
            let ids: Vec<&str> = doc.iter().map(|b| b.id.as_str()).collect();
            assert_eq!(ids, ["b1", "b2"], "op2 must have seen op1's staged write");
        });
    }

    // the poison-pill guard: an op that would grow the serialized doc past
    // MAX_DOC_LEN is rejected at WRITE time — never staged, never committed —
    // instead of committing fine and panicking every later decode of that doc.
    #[test]
    fn oversized_doc_is_rejected_before_staging() {
        deterministic::Runner::default().start(|context| async move {
            let mut d = Document::init(context, "document").await;
            apply_commit(
                &mut d,
                &DocMsg::CreateDoc {
                    doc_id: "doc1".into(),
                },
            )
            .await;
            let r_before = d.root();

            // one block whose text alone exceeds the cap -> the serialized doc
            // must exceed it too, whatever the json framing adds.
            let huge = "x".repeat(MAX_DOC_LEN + 1);
            let err = d
                .execute(
                    &mut TestCtx::new(),
                    &msg(&DocMsg::InsertBlock {
                        doc_id: "doc1".into(),
                        after: None,
                        block: blk("big", &huge),
                    }),
                )
                .await
                .expect_err("over-cap doc must be rejected");
            assert!(
                matches!(err, Error::Module(ref m) if m.contains("doc too large")),
                "unexpected error: {err:?}"
            );

            // rejected BEFORE staging: no overlay entry, commit is a no-op, and
            // the committed doc is untouched.
            assert!(d.pending.is_empty(), "a rejected write must not be staged");
            d.commit_block().await.unwrap();
            assert_eq!(
                d.root(),
                r_before,
                "a rejected write must not move the root"
            );
            assert!(get_doc(&d, "doc1").await.unwrap().is_empty());
        });
    }

    // corruption must surface as a DISTINCT error, never DocNotFound: mapping it
    // to "absent" would let CreateDoc re-seed an empty doc over the corrupt
    // bytes, silently destroying the data.
    #[test]
    fn corrupt_stored_doc_errors_as_corruption_not_absence() {
        deterministic::Runner::default().start(|context| async move {
            let mut d = Document::init(context, "document").await;
            // commit bytes that are NOT valid Vec<Block> json under doc1's key
            // (simulating on-disk corruption; unreachable through DocMsg ops).
            d.pending.insert(b"doc1".to_vec(), b"not json".to_vec());
            d.commit_block().await.unwrap();

            // a block op must report corruption, not DocNotFound.
            let err = d
                .execute(
                    &mut TestCtx::new(),
                    &msg(&DocMsg::InsertBlock {
                        doc_id: "doc1".into(),
                        after: None,
                        block: blk("b1", "x"),
                    }),
                )
                .await
                .expect_err("op on a corrupt doc must fail");
            assert!(
                matches!(err, Error::Module(ref m) if m.contains("corrupt")),
                "unexpected error: {err:?}"
            );
            d.abort_block().await.unwrap();

            // CreateDoc must NOT treat the corrupt doc as absent and re-seed it.
            let err = d
                .execute(
                    &mut TestCtx::new(),
                    &msg(&DocMsg::CreateDoc {
                        doc_id: "doc1".into(),
                    }),
                )
                .await
                .expect_err("create over a corrupt doc must fail");
            assert!(
                matches!(err, Error::Module(ref m) if m.contains("corrupt")),
                "unexpected error: {err:?}"
            );
            d.abort_block().await.unwrap();

            // the read path surfaces the decode failure too (as an error, not None).
            assert!(
                d.query(&encode_query(&DocQuery::GetDoc {
                    doc_id: "doc1".into()
                }))
                .await
                .is_err()
            );
        });
    }

    #[test]
    fn create_is_idempotent() {
        deterministic::Runner::default().start(|context| async move {
            let mut d = Document::init(context, "document").await;
            apply_commit(
                &mut d,
                &DocMsg::CreateDoc {
                    doc_id: "doc1".into(),
                },
            )
            .await;
            apply_commit(
                &mut d,
                &DocMsg::InsertBlock {
                    doc_id: "doc1".into(),
                    after: None,
                    block: blk("b1", "keep"),
                },
            )
            .await;
            // re-create must NOT wipe the existing block.
            apply_commit(
                &mut d,
                &DocMsg::CreateDoc {
                    doc_id: "doc1".into(),
                },
            )
            .await;
            let doc = get_doc(&d, "doc1").await.unwrap();
            assert_eq!(doc.len(), 1);
            assert_eq!(doc[0].id, "b1");
        });
    }

    // enumeration: ListDocs starts empty, then reflects every CreateDoc, sorted
    // and deduplicated. this is the browsable-store property the index adds.
    #[test]
    fn list_docs_enumerates_created_docs_sorted() {
        deterministic::Runner::default().start(|context| async move {
            let mut d = Document::init(context, "document").await;
            assert!(list_docs(&d).await.is_empty(), "a fresh store lists nothing");

            // create out of order; the index must come back sorted.
            for id in ["projects/retro", "notes", "projects/launch-plan"] {
                apply_commit(&mut d, &DocMsg::CreateDoc { doc_id: id.into() }).await;
            }
            assert_eq!(
                list_docs(&d).await,
                ["notes", "projects/launch-plan", "projects/retro"],
                "ListDocs is the sorted set of known doc ids"
            );

            // re-creating a known id is idempotent — no duplicate entry.
            apply_commit(
                &mut d,
                &DocMsg::CreateDoc {
                    doc_id: "notes".into(),
                },
            )
            .await;
            assert_eq!(
                list_docs(&d).await,
                ["notes", "projects/launch-plan", "projects/retro"],
                "re-creating a doc must not duplicate its index entry"
            );
        });
    }

    // block edits (insert/update/remove/move) must NOT touch the index — only
    // CreateDoc grows it. the index stays exactly the set of created docs.
    #[test]
    fn block_edits_leave_the_index_untouched() {
        deterministic::Runner::default().start(|context| async move {
            let mut d = Document::init(context, "document").await;
            apply_commit(
                &mut d,
                &DocMsg::CreateDoc {
                    doc_id: "doc1".into(),
                },
            )
            .await;
            for op in [
                DocMsg::InsertBlock {
                    doc_id: "doc1".into(),
                    after: None,
                    block: blk("b1", "one"),
                },
                DocMsg::UpdateBlock {
                    doc_id: "doc1".into(),
                    block_id: "b1".into(),
                    text: "two".into(),
                },
                DocMsg::RemoveBlock {
                    doc_id: "doc1".into(),
                    block_id: "b1".into(),
                },
            ] {
                apply_commit(&mut d, &op).await;
            }
            assert_eq!(
                list_docs(&d).await,
                ["doc1"],
                "block edits must not add or drop index entries"
            );
        });
    }

    // the reserved sentinel is UNREACHABLE by any doc op: a CreateDoc (or block
    // op) that names it is rejected, so a document write can never overwrite the
    // enumeration index. the rejected op stages nothing and moves no root.
    #[test]
    fn reserved_index_id_is_rejected() {
        deterministic::Runner::default().start(|context| async move {
            let mut d = Document::init(context, "document").await;
            apply_commit(
                &mut d,
                &DocMsg::CreateDoc {
                    doc_id: "real".into(),
                },
            )
            .await;
            let r_before = d.root();

            let err = d
                .execute(
                    &mut TestCtx::new(),
                    &msg(&DocMsg::CreateDoc {
                        doc_id: DOC_INDEX_KEY.into(),
                    }),
                )
                .await
                .expect_err("a doc op on the reserved id must be rejected");
            assert!(
                matches!(err, Error::Module(ref m) if m.contains("reserved doc id")),
                "unexpected error: {err:?}"
            );
            assert!(d.pending.is_empty(), "a rejected op must stage nothing");
            d.abort_block().await.unwrap();

            assert_eq!(d.root(), r_before, "a rejected op must not move the root");
            // the index is intact — still exactly the real doc.
            assert_eq!(list_docs(&d).await, ["real"]);
        });
    }

    // the index is committed state: a write that adds a doc to the index MOVES
    // the qmdb root, and the whole index survives a re-open (it IS qmdb state).
    #[test]
    fn create_moves_root_via_the_index() {
        deterministic::Runner::default().start(|context| async move {
            let mut d = Document::init(context, "document").await;
            let r0 = d.root();
            apply_commit(
                &mut d,
                &DocMsg::CreateDoc {
                    doc_id: "doc1".into(),
                },
            )
            .await;
            assert_ne!(r0, d.root(), "creating a doc (index + empty doc) moves root");
            assert_eq!(list_docs(&d).await, ["doc1"]);
        });
    }
}
