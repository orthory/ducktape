//! qmdb-backed pages module — a notion-like block tree, one block per key.
//!
//! a page is a TREE of [`Block`]s: the page itself is the root block (kind
//! `Page`, text == title), every block carries an ordered `children` list, and
//! every block id is GLOBALLY UNIQUE within the module. unlike the document
//! module's whole-doc-per-key layout, the qmdb key here is `sha256(block_id)`
//! and the value is ONE serialized block — so the merkle root commits to every
//! block individually, and a single block is readable (and one day provable)
//! by id alone with no page context. that is the addressability contract that
//! lets other modules hold a [`crate::BlockRef`] today and resolve
//! it via `Ctx::query(pages, GetBlock { block_id })`.
//!
//! ## keys are hashed to a fixed width
//!
//! the logical key is the `block_id` string at the interface seam, but the
//! qmdb key is `sha256(block_id)` — a fixed 32-byte [`commonware_utils`]
//! `Array`, mirroring the kv/document modules. this is load-bearing:
//! commonware's state-sync resolvers for the overwriteable variable db are
//! bounded on `K: Array`.
//!
//! ## enumeration via a reserved index entry
//!
//! one extra qmdb entry is reserved: the sentinel logical key
//! [`PAGE_INDEX_KEY`] whose value is the serialized SORTED set of every page
//! (root block) id. its leading NUL makes it uncollidable with a client-minted
//! block id, and every op that names it is rejected before any storage touch.
//! only [`PageMsg::CreatePage`] grows the index; block edits never touch it —
//! and because block ops can neither insert nor convert to kind `Page`,
//! removal of a subtree can never orphan an index entry.
//!
//! ## host-lent staging (the kv/document pattern, plus deletes)
//!
//! writes made during a block are STAGED in an in-memory `pending` overlay and
//! flushed to qmdb in ONE batch by `commit_block`; `abort_block` drops the
//! overlay. the pages twist: `RemoveBlock` deletes a whole subtree, so the
//! overlay value is an `Option<Vec<u8>>` — `Some` stages a write, `None`
//! stages a DELETE (qmdb's `batch.write(key, None)`), and reads through the
//! overlay see a staged delete as absence.
//!
//! ## state-sync
//!
//! identical to the document module: [`Pages::sync_target`] /
//! [`Pages::sync_from`] delegate to commonware's qmdb sync engine, so a joiner
//! rebuilds a byte-identical root from an untrusted peer, merkle-verified
//! against the target root.

// the wire surface: this module's shared types, flattened at the crate root.
mod interface;
pub use interface::*;
// the derived-tier materialized view; registered only by serving binaries.
pub mod index;

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

use sdk::{
    Ctx, Error, Module, ModuleId, Msg, Origin, ResolverSyncTarget, StateRoot, StateSyncHandle,
};

/// reserved logical-key prefixes for comment records + the per-target thread
/// index. all lead with NUL, so they can never collide with a client-minted
/// block/page id (which the reserved-id guard forbids from starting with NUL).
const THREAD_PREFIX: &str = "\u{0}ct:";
const COMMENT_PREFIX: &str = "\u{0}cc:";
const TARGET_INDEX_PREFIX: &str = "\u{0}ci:";

fn thread_key(id: &str) -> String {
    format!("{THREAD_PREFIX}{id}")
}
fn comment_key(id: &str) -> String {
    format!("{COMMENT_PREFIX}{id}")
}
fn target_index_key(target: &str) -> String {
    format!("{TARGET_INDEX_PREFIX}{target}")
}

/// derive the comment author from the dispatch origin (mirrors chat). the
/// pre-consensus default `Origin::External(vec![])` must never pass as a real
/// user.
fn author_from_origin(origin: &Origin) -> Result<AuthorRef, PageError> {
    match origin {
        Origin::External(bytes) if bytes.is_empty() => Err(PageError::EmptyOrigin),
        Origin::External(bytes) => Ok(AuthorRef::User(bytes.clone())),
        Origin::Module(id) => Ok(AuthorRef::Module(id.to_string())),
        Origin::System => Ok(AuthorRef::System),
    }
}

/// write-time cap on ONE serialized block record (and on the enumeration
/// index value — both stage through the same guard). the codec [`RangeCfg`]
/// bounds a stored value at 1 MiB AT DECODE TIME only, so an oversized value
/// that staged fine would panic every later read on every validator: a poison
/// pill. 768 KiB leaves the same 256 KiB framing margin the document module
/// keeps. a block record carries its text plus its ordered child-id list, so
/// this also bounds a single parent to tens of thousands of children.
pub const MAX_BLOCK_LEN: usize = 768 * 1024;

/// the reserved logical key under which the page-enumeration INDEX rides in
/// the same qmdb. its value is a serialized sorted `Vec<String>` of every page
/// (root block) id. the leading NUL makes it UNCOLLIDABLE with a real block id
/// (clients mint uuids), and every op that names it is rejected
/// ([`PageError::ReservedId`]) before it can reach storage.
const PAGE_INDEX_KEY: &str = "\u{0}page-index";

/// how many parent hops a MoveBlock ancestry walk will follow before declaring
/// the stored tree corrupt. committed state is acyclic by construction (every
/// move re-checks), so a walk this deep can only mean a broken parent chain —
/// the cap turns a would-be infinite loop into a loud deterministic error.
const MAX_DEPTH: usize = 10_000;

/// the qmdb key: a fixed 32-byte sha256 digest of the `block_id`. fixed width
/// is what lets a store be state-synced (resolvers require `K: Array`).
type PageKey = <Sha256 as Hasher>::Digest;

/// the concrete qmdb store — identical params to the kv/document modules, so
/// all qmdb plumbing is shared verbatim.
type PagesDb<E> = Db<mmr::Family, E, PageKey, Vec<u8>, Sha256, TwoCap, Sequential>;

/// the qmdb configuration — shared by [`Pages::init`] (fresh open) and
/// [`Pages::sync_from`] (state-sync target) so a synced store's storage layout
/// is byte-identical to a freshly-opened one.
type PagesConfig = VariableConfig<TwoCap, ((), (RangeCfg<usize>, ())), Sequential>;

/// a state-sync target: a qmdb merkle root plus the operation range a joiner
/// must pull to reconstruct a store with an identical root.
pub type PagesTarget = Target<mmr::Family, PageKey>;

/// hash a `block_id` to its fixed-width qmdb key. deterministic, so every
/// validator maps a given block to the same store slot.
fn hash_key(block_id: &[u8]) -> PageKey {
    let mut h = Sha256::new();
    h.update(block_id);
    h.finalize()
}

/// build the qmdb [`VariableConfig`] for module `id` on `context`. partitions
/// are namespaced by `id` so several qmdb-backed modules share one runtime
/// context without colliding on storage.
fn pages_config<E>(context: &E, id: &str) -> PagesConfig
where
    E: Context + BufferPooler,
{
    // a single page-cache handle shared by both sub-configs (cheap to clone).
    let page_cache = CacheRef::from_pooler(
        context,
        NonZeroU16::new(128).unwrap(),
        NonZeroUsize::new(64).unwrap(),
    );

    // codec config for Operation<.., PageKey, Vec<u8>>: (key_cfg, value_cfg).
    // the key is a fixed-width digest so its cfg is `()`; the value is a
    // Vec<u8> whose <Vec<u8> as Read>::Cfg == (RangeCfg<usize>, ()).
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
enum PageError {
    /// insert/create of a block id already present ANYWHERE in the module —
    /// block ids are globally unique, that is the addressability contract.
    DuplicateBlock,
    /// update/move/remove/check of a block id not in the store.
    BlockNotFound,
    /// an insert/move named a parent block that does not exist.
    ParentNotFound,
    /// an `after` anchor that is not a child of the named parent.
    AnchorNotFound,
    /// a move whose new parent sits inside the moved block's own subtree.
    CycleMove,
    /// a move whose new parent belongs to a different page.
    CrossPageMove,
    /// move/remove/convert targeted a page root — roots are managed solely by
    /// `CreatePage` (and renames via `UpdateText`).
    PageRootImmutable,
    /// a block op tried to insert or convert to kind `Page` — pages come only
    /// from `CreatePage`, which is what keeps the enumeration index exact.
    PageViaBlockOp,
    /// `SetChecked` on a non-`Todo` block.
    NotTodo,
    /// the op would grow a serialized block (or the index) past
    /// [`MAX_BLOCK_LEN`] — rejected at write time so the oversized bytes never
    /// reach the panicking commit/read paths (the codec bound is decode-only).
    BlockTooLarge,
    /// stored state failed to decode or a tree invariant is broken (a listed
    /// child missing, a parent chain looping). distinct from absence:
    /// corruption must surface loudly, never masquerade as "not found".
    Corrupt,
    /// an op named the reserved [`PAGE_INDEX_KEY`] sentinel.
    ReservedId,
    /// a create/set-parent named a `parent` that is not an existing page root.
    ParentPageNotFound,
    /// set-parent/delete targeted an id that is not an existing page root.
    NotAPage,
    /// a set-parent would nest a page inside its own folder subtree.
    PageCycle,
    // ── comments ──
    /// a comment op arrived with an empty (pre-consensus) origin.
    EmptyOrigin,
    /// resolve/append named a thread id not in the store.
    ThreadNotFound,
    /// edit/delete named a comment id not in the store (or a tombstone).
    CommentNotFound,
    /// AddComment reused a comment id already present.
    DuplicateComment,
    /// an append named a target that differs from the thread's.
    TargetMismatch,
    /// edit/delete by someone other than the stored author.
    NotAuthor,
    /// comment text over [`MAX_COMMENT_TEXT_BYTES`].
    TextTooLarge,
    /// a thread already holds [`MAX_COMMENTS_PER_THREAD`] comments.
    TooManyComments,
    /// a target already holds [`MAX_THREADS_PER_TARGET`] threads.
    TooManyThreads,
    /// a ThreadsForTargets query named more than [`MAX_QUERY_TARGETS`] targets.
    TooManyTargets,
}

impl core::fmt::Display for PageError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let s = match self {
            PageError::DuplicateBlock => "duplicate block id",
            PageError::BlockNotFound => "block not found",
            PageError::ParentNotFound => "parent block not found",
            PageError::AnchorNotFound => "after-anchor not found",
            PageError::CycleMove => "move target is inside the moved subtree",
            PageError::CrossPageMove => "cross-page move",
            PageError::PageRootImmutable => "page roots cannot be moved, removed, or converted",
            PageError::PageViaBlockOp => "a page block can only be created by CreatePage",
            PageError::NotTodo => "checked applies only to todo blocks",
            PageError::BlockTooLarge => "block too large",
            PageError::Corrupt => "stored page state is corrupt",
            PageError::ReservedId => "reserved block id",
            PageError::ParentPageNotFound => "parent page not found",
            PageError::NotAPage => "not a page",
            PageError::PageCycle => "page cycle",
            PageError::EmptyOrigin => "empty origin",
            PageError::ThreadNotFound => "thread not found",
            PageError::CommentNotFound => "comment not found",
            PageError::DuplicateComment => "duplicate comment id",
            PageError::TargetMismatch => "target mismatch",
            PageError::NotAuthor => "not the comment author",
            PageError::TextTooLarge => "comment text too large",
            PageError::TooManyComments => "too many comments in thread",
            PageError::TooManyThreads => "too many threads on target",
            PageError::TooManyTargets => "too many query targets",
        };
        f.write_str(s)
    }
}

/// bridge the only sdk error `load_block` can raise — a stored-block json
/// decode failure — back into `PageError` so `apply` stays single-error-typed.
/// if it ever fires it MUST surface as corruption, not absence: mapping a
/// decode failure to "not found" would let `CreatePage` silently re-seed a
/// root over the corrupt bytes, destroying the evidence AND the data.
fn to_page_err(_e: Error) -> PageError {
    PageError::Corrupt
}

/// resolve an `after` sibling anchor to the insert index within `children`:
/// `None` -> first child (0); `Some(id)` -> one past the anchor's position,
/// else [`PageError::AnchorNotFound`].
fn idx_after(children: &[String], after: &Option<String>) -> Result<usize, PageError> {
    match after {
        None => Ok(0),
        Some(a) => children
            .iter()
            .position(|c| c == a)
            .map(|p| p + 1)
            .ok_or(PageError::AnchorNotFound),
    }
}

/// a qmdb-backed, block-tree pages module.
pub struct Pages<E>
where
    E: Context + BufferPooler,
{
    id: ModuleId,
    db: PagesDb<E>,
    /// blocks touched this block-height, keyed by LOGICAL `block_id` bytes.
    /// `Some(bytes)` stages a write, `None` stages a DELETE (subtree removal).
    /// read ahead of committed state by `get` (read-your-writes) and flushed
    /// to qmdb in one batch by `commit_block`; NOT in `root()` until then.
    pending: BTreeMap<Vec<u8>, Option<Vec<u8>>>,
}

impl<E> Pages<E>
where
    E: Context + BufferPooler,
{
    /// open (or recover) the store on `context` under module identity `id`.
    /// qmdb partitions are namespaced by `id`, so the pages module shares one
    /// runtime context with kv/document/other qmdb modules without colliding.
    pub async fn init(context: E, id: impl Into<ModuleId>) -> Self {
        let id = id.into();
        let cfg = pages_config(&context, &id);
        let db = PagesDb::<E>::init(context, cfg)
            .await
            .expect("qmdb init failed");
        Self {
            id,
            db,
            pending: BTreeMap::new(),
        }
    }

    /// read raw bytes for `key` through the staged overlay: a staged write
    /// shadows committed state, and a staged DELETE reads as absence.
    async fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        if let Some(staged) = self.pending.get(key) {
            return staged.clone();
        }
        self.db.get(&hash_key(key)).await.expect("get failed")
    }

    /// load one block (`None` == absent), through the staged-over-committed
    /// overlay. a decode failure is corruption, surfaced as an error.
    async fn load_block(&self, block_id: &str) -> Result<Option<Block>, Error> {
        match self.get(block_id.as_bytes()).await {
            Some(b) => Ok(Some(
                serde_json::from_slice(&b).map_err(|e| Error::Module(e.to_string()))?,
            )),
            None => Ok(None),
        }
    }

    /// stage serialized `bytes` under logical `key` for this block-height
    /// WITHOUT committing. rejects a value over [`MAX_BLOCK_LEN`] BEFORE
    /// staging — the shared poison-pill guard for block records AND the
    /// enumeration index value.
    fn stage(&mut self, key: &str, bytes: Vec<u8>) -> Result<(), PageError> {
        if bytes.len() > MAX_BLOCK_LEN {
            return Err(PageError::BlockTooLarge);
        }
        self.pending.insert(key.as_bytes().to_vec(), Some(bytes));
        Ok(())
    }

    /// stage one block record.
    fn store_block(&mut self, block: &Block) -> Result<(), PageError> {
        let bytes = serde_json::to_vec(block).expect("Block is always serializable");
        self.stage(&block.id, bytes)
    }

    /// stage a DELETE of `block_id` — reads see absence at once; the key is
    /// dropped from qmdb (and the root) at `commit_block`.
    fn delete_block(&mut self, block_id: &str) {
        self.pending.insert(block_id.as_bytes().to_vec(), None);
    }

    /// load the enumeration index — page id → folder parent — through the
    /// staged-over-committed overlay. absent reads as the empty map; a decode
    /// failure is corruption. `BTreeMap` serializes with SORTED keys, so the
    /// bytes are canonical and every validator commits the same index root.
    async fn load_index(&self) -> Result<BTreeMap<String, Option<String>>, Error> {
        match self.get(PAGE_INDEX_KEY.as_bytes()).await {
            Some(b) => serde_json::from_slice(&b).map_err(|e| Error::Module(e.to_string())),
            None => Ok(BTreeMap::new()),
        }
    }

    /// re-stage the whole index map (canonical serialization).
    fn stage_index(&mut self, index: &BTreeMap<String, Option<String>>) -> Result<(), PageError> {
        let bytes = serde_json::to_vec(index).expect("index is always serializable");
        self.stage(PAGE_INDEX_KEY, bytes)
    }

    /// add `page_id -> parent` to the index if absent (idempotent create keeps
    /// the existing entry, so re-create never re-nests).
    async fn index_add(&mut self, page_id: &str, parent: Option<String>) -> Result<(), PageError> {
        let mut index = self.load_index().await.map_err(to_page_err)?;
        if !index.contains_key(page_id) {
            index.insert(page_id.to_string(), parent);
            self.stage_index(&index)?;
        }
        Ok(())
    }

    // ── comment storage (reserved NUL-prefixed keys) ──

    async fn load_thread(&self, id: &str) -> Result<Option<Thread>, PageError> {
        match self.get(thread_key(id).as_bytes()).await {
            Some(b) => Ok(Some(
                serde_json::from_slice(&b).map_err(|_| PageError::Corrupt)?,
            )),
            None => Ok(None),
        }
    }

    async fn load_comment(&self, id: &str) -> Result<Option<Comment>, PageError> {
        match self.get(comment_key(id).as_bytes()).await {
            Some(b) => Ok(Some(
                serde_json::from_slice(&b).map_err(|_| PageError::Corrupt)?,
            )),
            None => Ok(None),
        }
    }

    fn store_thread(&mut self, t: &Thread) -> Result<(), PageError> {
        self.stage(&thread_key(&t.id), serde_json::to_vec(t).expect("thread serializable"))
    }

    fn store_comment(&mut self, c: &Comment) -> Result<(), PageError> {
        self.stage(&comment_key(&c.id), serde_json::to_vec(c).expect("comment serializable"))
    }

    async fn load_target_index(&self, target: &str) -> Result<Vec<String>, PageError> {
        match self.get(target_index_key(target).as_bytes()).await {
            Some(b) => serde_json::from_slice(&b).map_err(|_| PageError::Corrupt),
            None => Ok(Vec::new()),
        }
    }

    fn stage_target_index(&mut self, target: &str, ids: &[String]) -> Result<(), PageError> {
        if ids.is_empty() {
            self.delete_block(&target_index_key(target));
            Ok(())
        } else {
            self.stage(&target_index_key(target), serde_json::to_vec(ids).expect("ids serializable"))
        }
    }

    /// a thread plus its LIVE (non-tombstoned) comments in order. `None` when
    /// the thread is absent; a listed comment missing from the store is
    /// corruption, surfaced loudly.
    async fn thread_view(&self, thread_id: &str) -> Result<Option<ThreadView>, PageError> {
        let thread = match self.load_thread(thread_id).await? {
            Some(t) => t,
            None => return Ok(None),
        };
        let mut comments = Vec::new();
        for cid in &thread.comment_ids {
            let c = self.load_comment(cid).await?.ok_or(PageError::Corrupt)?;
            if !c.deleted {
                comments.push(c);
            }
        }
        Ok(Some(ThreadView { thread, comments }))
    }

    /// load a block that MUST exist (`missing` names the error when it does
    /// not) — the shared shape of every "look up then edit" op.
    async fn require_block(&self, block_id: &str, missing: PageError) -> Result<Block, PageError> {
        self.load_block(block_id)
            .await
            .map_err(to_page_err)?
            .ok_or(missing)
    }

    /// walk parent pointers from `start` to the page root, erroring with
    /// [`PageError::CycleMove`] if `forbidden` appears on the path (that would
    /// reparent a block inside its own subtree). the [`MAX_DEPTH`] cap turns a
    /// corrupt (looping) parent chain into a loud error instead of a hang.
    async fn ancestry_excludes(&self, start: &str, forbidden: &str) -> Result<(), PageError> {
        let mut cur = start.to_string();
        for _ in 0..MAX_DEPTH {
            if cur == forbidden {
                return Err(PageError::CycleMove);
            }
            let blk = self.require_block(&cur, PageError::Corrupt).await?;
            match blk.parent {
                Some(p) => cur = p,
                None => return Ok(()),
            }
        }
        Err(PageError::Corrupt)
    }

    /// walk FOLDER parents (the index map) up from `start`, erroring with
    /// [`PageError::PageCycle`] if `forbidden` is met — that would nest a page
    /// inside its own folder subtree. [`MAX_DEPTH`] turns a corrupt (looping)
    /// folder chain into a loud error instead of a hang.
    async fn folder_ancestry_excludes(&self, start: &str, forbidden: &str) -> Result<(), PageError> {
        let index = self.load_index().await.map_err(to_page_err)?;
        let mut cur = Some(start.to_string());
        for _ in 0..MAX_DEPTH {
            match cur {
                None => return Ok(()),
                Some(id) => {
                    if id == forbidden {
                        return Err(PageError::PageCycle);
                    }
                    cur = index.get(&id).cloned().flatten();
                }
            }
        }
        Err(PageError::Corrupt)
    }

    /// apply one decoded [`PageMsg`] to the staged overlay. pure tree surgery
    /// over per-block/-comment records, re-staged on success. errors abort the
    /// block. `origin`/`now` are only consulted by the comment ops (author +
    /// timestamp); block ops ignore them.
    async fn apply(&mut self, msg: PageMsg, origin: &Origin, now: u64) -> Result<(), PageError> {
        // no client-minted id may live in the reserved (NUL-prefixed) keyspace:
        // the enumeration index (`\0page-index`) and every comment record/index
        // key lead with NUL, so rejecting NUL-prefixed ids here — BEFORE any
        // storage touch — keeps a block/comment write from ever clobbering them.
        let named: Vec<&str> = match &msg {
            PageMsg::CreatePage { page_id, parent, .. } => {
                let mut v = vec![page_id.as_str()];
                if let Some(p) = parent {
                    v.push(p.as_str());
                }
                v
            }
            PageMsg::InsertBlock { parent, block, .. } => vec![parent.as_str(), block.id.as_str()],
            PageMsg::UpdateText { block_id, .. }
            | PageMsg::SetKind { block_id, .. }
            | PageMsg::SetChecked { block_id, .. }
            | PageMsg::RemoveBlock { block_id } => vec![block_id.as_str()],
            PageMsg::MoveBlock { block_id, parent, .. } => vec![block_id.as_str(), parent.as_str()],
            PageMsg::SetPageParent { page_id, parent } => {
                let mut v = vec![page_id.as_str()];
                if let Some(p) = parent {
                    v.push(p.as_str());
                }
                v
            }
            PageMsg::DeletePage { page_id } => vec![page_id.as_str()],
            PageMsg::AddComment { thread_id, comment_id, target, .. } => {
                vec![thread_id.as_str(), comment_id.as_str(), target.as_str()]
            }
            PageMsg::EditComment { comment_id, .. } => vec![comment_id.as_str()],
            PageMsg::DeleteComment { comment_id } => vec![comment_id.as_str()],
            PageMsg::ResolveThread { thread_id, .. } => vec![thread_id.as_str()],
        };
        if named.iter().any(|id| id.starts_with('\u{0}')) {
            return Err(PageError::ReservedId);
        }

        match msg {
            PageMsg::CreatePage { page_id, title, parent } => {
                match self.load_block(&page_id).await.map_err(to_page_err)? {
                    // idempotent: re-creating an existing page is a benign
                    // no-op that does NOT clobber the live title OR re-nest it.
                    Some(b) if b.kind == BlockKind::Page => Ok(()),
                    // the id is already a NON-page block somewhere — page ids
                    // are block ids, so this is a global-uniqueness violation.
                    Some(_) => Err(PageError::DuplicateBlock),
                    None => {
                        // a named folder parent must exist AND be a page root.
                        if let Some(par) = &parent {
                            match self.load_block(par).await.map_err(to_page_err)? {
                                Some(b) if b.kind == BlockKind::Page => {}
                                _ => return Err(PageError::ParentPageNotFound),
                            }
                        }
                        self.index_add(&page_id, parent).await?;
                        self.store_block(&Block {
                            id: page_id.clone(),
                            // block-parent stays None; the folder parent lives
                            // only in the enumeration index.
                            parent: None,
                            page: page_id,
                            kind: BlockKind::Page,
                            text: title,
                            checked: false,
                            children: Vec::new(),
                        })
                    }
                }
            }
            PageMsg::InsertBlock {
                parent,
                after,
                block,
            } => {
                if block.kind == BlockKind::Page {
                    return Err(PageError::PageViaBlockOp);
                }
                // global uniqueness: the id must be absent from the WHOLE
                // store, not just this page — that is what makes a bare block
                // id addressable (and referenceable) without page context.
                if self
                    .load_block(&block.id)
                    .await
                    .map_err(to_page_err)?
                    .is_some()
                {
                    return Err(PageError::DuplicateBlock);
                }
                let mut parent_blk = self
                    .require_block(&parent, PageError::ParentNotFound)
                    .await?;
                let i = idx_after(&parent_blk.children, &after)?;
                parent_blk.children.insert(i, block.id.clone());
                self.store_block(&Block {
                    id: block.id,
                    parent: Some(parent_blk.id.clone()),
                    page: parent_blk.page.clone(),
                    kind: block.kind,
                    text: block.text,
                    checked: false,
                    children: Vec::new(),
                })?;
                self.store_block(&parent_blk)
            }
            PageMsg::UpdateText { block_id, text } => {
                // works on any block INCLUDING a page root — that is the
                // rename path (the title is the root's text).
                let mut blk = self
                    .require_block(&block_id, PageError::BlockNotFound)
                    .await?;
                blk.text = text;
                self.store_block(&blk)
            }
            PageMsg::SetKind { block_id, kind } => {
                if kind == BlockKind::Page {
                    return Err(PageError::PageViaBlockOp);
                }
                let mut blk = self
                    .require_block(&block_id, PageError::BlockNotFound)
                    .await?;
                if blk.kind == BlockKind::Page {
                    return Err(PageError::PageRootImmutable);
                }
                blk.kind = kind;
                self.store_block(&blk)
            }
            PageMsg::SetChecked { block_id, checked } => {
                let mut blk = self
                    .require_block(&block_id, PageError::BlockNotFound)
                    .await?;
                if blk.kind != BlockKind::Todo {
                    return Err(PageError::NotTodo);
                }
                blk.checked = checked;
                self.store_block(&blk)
            }
            PageMsg::MoveBlock {
                block_id,
                parent,
                after,
            } => {
                // self-anchor is a benign no-op (the document module's rule):
                // "after myself" describes the position the block already
                // holds relative to itself, wherever it is.
                if after.as_deref() == Some(block_id.as_str()) {
                    return Ok(());
                }
                let mut blk = self
                    .require_block(&block_id, PageError::BlockNotFound)
                    .await?;
                if blk.kind == BlockKind::Page {
                    return Err(PageError::PageRootImmutable);
                }
                let new_parent = self
                    .require_block(&parent, PageError::ParentNotFound)
                    .await?;
                if new_parent.page != blk.page {
                    return Err(PageError::CrossPageMove);
                }
                // reparenting under one's own descendant would detach the
                // subtree from the page into an unreachable cycle.
                self.ancestry_excludes(&parent, &block_id).await?;

                // a non-root block always has a parent; a missing pointer or a
                // child entry the parent does not carry is a broken invariant.
                let old_parent_id = blk.parent.clone().ok_or(PageError::Corrupt)?;
                if old_parent_id == parent {
                    // reorder among current siblings: one record changes.
                    let mut p = self
                        .require_block(&old_parent_id, PageError::Corrupt)
                        .await?;
                    let pos = p
                        .children
                        .iter()
                        .position(|c| c == &block_id)
                        .ok_or(PageError::Corrupt)?;
                    p.children.remove(pos);
                    // anchor index is computed in the now-shortened list.
                    let i = idx_after(&p.children, &after)?;
                    p.children.insert(i, block_id);
                    self.store_block(&p)
                } else {
                    let mut old_p = self
                        .require_block(&old_parent_id, PageError::Corrupt)
                        .await?;
                    let pos = old_p
                        .children
                        .iter()
                        .position(|c| c == &block_id)
                        .ok_or(PageError::Corrupt)?;
                    old_p.children.remove(pos);
                    // reload the new parent so this read is not stale ordering
                    // (require_block above ran before the old parent restage;
                    // distinct ids, so the record is unchanged — but keep one
                    // authoritative copy to mutate).
                    let mut new_p = new_parent;
                    let i = idx_after(&new_p.children, &after)?;
                    new_p.children.insert(i, block_id.clone());
                    blk.parent = Some(parent);
                    self.store_block(&old_p)?;
                    self.store_block(&new_p)?;
                    self.store_block(&blk)
                }
            }
            PageMsg::RemoveBlock { block_id } => {
                let blk = self
                    .require_block(&block_id, PageError::BlockNotFound)
                    .await?;
                if blk.kind == BlockKind::Page {
                    return Err(PageError::PageRootImmutable);
                }
                let parent_id = blk.parent.clone().ok_or(PageError::Corrupt)?;
                let mut p = self.require_block(&parent_id, PageError::Corrupt).await?;
                let pos = p
                    .children
                    .iter()
                    .position(|c| c == &block_id)
                    .ok_or(PageError::Corrupt)?;
                p.children.remove(pos);
                self.store_block(&p)?;
                // delete the WHOLE subtree, depth-first. a child listed but
                // absent from the store is a broken invariant, surfaced loudly.
                // no page root can live below a non-root block (block ops
                // can't mint kind Page), so the index never needs updating.
                let mut stack = vec![blk];
                while let Some(cur) = stack.pop() {
                    for child in &cur.children {
                        stack.push(self.require_block(child, PageError::Corrupt).await?);
                    }
                    self.delete_block(&cur.id);
                }
                Ok(())
            }
            PageMsg::DeletePage { page_id } => {
                let root = self.require_block(&page_id, PageError::NotAPage).await?;
                if root.kind != BlockKind::Page {
                    return Err(PageError::NotAPage);
                }
                // promote direct child pages to the deleted page's parent, then
                // drop the deleted page's own index entry.
                let mut index = self.load_index().await.map_err(to_page_err)?;
                let promoted_to = index.get(&page_id).cloned().flatten();
                for parent in index.values_mut() {
                    if parent.as_deref() == Some(page_id.as_str()) {
                        *parent = promoted_to.clone();
                    }
                }
                index.remove(&page_id);
                self.stage_index(&index)?;
                // delete the whole block subtree, root included (depth-first).
                // child PAGES are separate roots (folder relation is in the
                // index, not the block children), so they are untouched.
                let mut stack = vec![root];
                while let Some(cur) = stack.pop() {
                    for child in &cur.children {
                        stack.push(self.require_block(child, PageError::Corrupt).await?);
                    }
                    self.delete_block(&cur.id);
                }
                Ok(())
            }
            PageMsg::SetPageParent { page_id, parent } => {
                // the target must be an existing page root.
                let root = self.require_block(&page_id, PageError::NotAPage).await?;
                if root.kind != BlockKind::Page {
                    return Err(PageError::NotAPage);
                }
                if let Some(par) = &parent {
                    // parent must exist and be a page …
                    match self.load_block(par).await.map_err(to_page_err)? {
                        Some(b) if b.kind == BlockKind::Page => {}
                        _ => return Err(PageError::ParentPageNotFound),
                    }
                    // … and nesting under self or a descendant would cycle.
                    self.folder_ancestry_excludes(par, &page_id).await?;
                }
                let mut index = self.load_index().await.map_err(to_page_err)?;
                index.insert(page_id, parent);
                self.stage_index(&index)
            }

            // ── comments ──
            PageMsg::AddComment { thread_id, comment_id, target, text } => {
                if text.len() > MAX_COMMENT_TEXT_BYTES {
                    return Err(PageError::TextTooLarge);
                }
                let author = author_from_origin(origin)?;
                if self.load_comment(&comment_id).await?.is_some() {
                    return Err(PageError::DuplicateComment);
                }
                match self.load_thread(&thread_id).await? {
                    Some(mut thread) => {
                        if thread.target != target {
                            return Err(PageError::TargetMismatch);
                        }
                        if thread.comment_ids.len() >= MAX_COMMENTS_PER_THREAD {
                            return Err(PageError::TooManyComments);
                        }
                        let comment = Comment {
                            id: comment_id.clone(),
                            thread_id: thread_id.clone(),
                            author,
                            text,
                            created_at: now,
                            edited_at: None,
                            deleted: false,
                        };
                        thread.comment_ids.push(comment_id);
                        self.store_comment(&comment)?;
                        self.store_thread(&thread)
                    }
                    None => {
                        let mut ids = self.load_target_index(&target).await?;
                        if ids.len() >= MAX_THREADS_PER_TARGET {
                            return Err(PageError::TooManyThreads);
                        }
                        let comment = Comment {
                            id: comment_id.clone(),
                            thread_id: thread_id.clone(),
                            author: author.clone(),
                            text,
                            created_at: now,
                            edited_at: None,
                            deleted: false,
                        };
                        let thread = Thread {
                            id: thread_id.clone(),
                            target: target.clone(),
                            opener: author,
                            created_at: now,
                            resolved: false,
                            resolved_by: None,
                            comment_ids: vec![comment_id],
                        };
                        if !ids.contains(&thread_id) {
                            ids.push(thread_id);
                            ids.sort();
                            self.stage_target_index(&target, &ids)?;
                        }
                        self.store_comment(&comment)?;
                        self.store_thread(&thread)
                    }
                }
            }
            PageMsg::EditComment { comment_id, text } => {
                if text.len() > MAX_COMMENT_TEXT_BYTES {
                    return Err(PageError::TextTooLarge);
                }
                let author = author_from_origin(origin)?;
                let mut c = self
                    .load_comment(&comment_id)
                    .await?
                    .ok_or(PageError::CommentNotFound)?;
                if c.deleted {
                    return Err(PageError::CommentNotFound);
                }
                if c.author != author {
                    return Err(PageError::NotAuthor);
                }
                c.text = text;
                c.edited_at = Some(now);
                self.store_comment(&c)
            }
            PageMsg::DeleteComment { comment_id } => {
                let author = author_from_origin(origin)?;
                let mut c = self
                    .load_comment(&comment_id)
                    .await?
                    .ok_or(PageError::CommentNotFound)?;
                if c.deleted {
                    return Ok(()); // idempotent
                }
                if c.author != author {
                    return Err(PageError::NotAuthor);
                }
                c.deleted = true;
                c.text = String::new();
                let thread_id = c.thread_id.clone();
                self.store_comment(&c)?;
                // if no live comments remain, remove the whole thread.
                let thread = self
                    .load_thread(&thread_id)
                    .await?
                    .ok_or(PageError::Corrupt)?;
                let mut any_live = false;
                for cid in &thread.comment_ids {
                    let cc = self.load_comment(cid).await?.ok_or(PageError::Corrupt)?;
                    if !cc.deleted {
                        any_live = true;
                        break;
                    }
                }
                if !any_live {
                    for cid in &thread.comment_ids {
                        self.delete_block(&comment_key(cid));
                    }
                    self.delete_block(&thread_key(&thread.id));
                    let mut ids = self.load_target_index(&thread.target).await?;
                    ids.retain(|t| t != &thread.id);
                    self.stage_target_index(&thread.target, &ids)?;
                }
                Ok(())
            }
            PageMsg::ResolveThread { thread_id, resolved } => {
                let author = author_from_origin(origin)?;
                let mut thread = self
                    .load_thread(&thread_id)
                    .await?
                    .ok_or(PageError::ThreadNotFound)?;
                thread.resolved = resolved;
                thread.resolved_by = if resolved { Some(author) } else { None };
                self.store_thread(&thread)
            }
        }
    }

    /// assemble a whole page in PREORDER (root first, each block's subtree
    /// before its next sibling), through the staged overlay. `None` when no
    /// PAGE lives at `page_id` (a non-root block id reads as absent here —
    /// `GetBlock` is the by-id surface).
    async fn load_page(&self, page_id: &str) -> Result<Option<Vec<Block>>, Error> {
        let corrupt = || Error::Module(PageError::Corrupt.to_string());
        let root = match self.load_block(page_id).await? {
            Some(b) if b.kind == BlockKind::Page => b,
            _ => return Ok(None),
        };
        let mut out = Vec::new();
        let mut stack = vec![root];
        while let Some(cur) = stack.pop() {
            // push children reversed so the leftmost child is popped next —
            // that is what makes the flat output preorder.
            for child in cur.children.iter().rev() {
                stack.push(self.load_block(child).await?.ok_or_else(corrupt)?);
            }
            out.push(cur);
        }
        Ok(Some(out))
    }

    // ---- state-sync ---------------------------------------------------------
    // reconstruct a byte-identical-rooted store from a peer WITHOUT replaying
    // the op history in application order — commonware's qmdb sync ships the
    // live op range and merkle-verifies every batch against the target root.

    /// the sync [`PagesTarget`] for this store: its qmdb merkle root plus the
    /// LIVE operation range `[sync_boundary, end)`. hand it to
    /// [`Pages::sync_from`] to rebuild a store with an identical root.
    pub async fn sync_target(&self) -> PagesTarget {
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
    /// batches.
    pub fn into_resolver(self) -> Arc<PagesDb<E>> {
        Arc::new(self.db)
    }

    /// reconstruct a `Pages` at `id` on `context` whose qmdb root EQUALS
    /// `target.root`, by pulling `target`'s op range from `resolver`. every
    /// fetched batch is merkle-verified against `target.root`, so a byzantine
    /// source cannot forge contents — the root is the trust anchor.
    pub async fn sync_from<R>(
        context: E,
        id: impl Into<ModuleId>,
        target: PagesTarget,
        resolver: R,
    ) -> Result<Self, String>
    where
        R: DbResolver<PagesDb<E>>,
    {
        let id = id.into();
        let db_config = pages_config(&context, &id);
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

#[async_trait::async_trait(?Send)]
impl<E> Module for Pages<E>
where
    E: Context + BufferPooler,
{
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// the REAL qmdb merkle root over all blocks, as a 32-byte state root.
    fn root(&self) -> StateRoot {
        StateRoot(self.db.root().0)
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::ResolverBacked {
            backend: "qmdb".into(),
            detail: "serve_sync answers qmdb op-range requests (statesync wire)".into(),
        })
    }

    /// the network state-sync serve lane: answers the shared qmdb wire
    /// requests from committed state. read-only.
    async fn serve_sync(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        statesync::qmdb::serve_bytes(&self.db, req).await
    }

    async fn resolver_sync_target(&self) -> Result<ResolverSyncTarget, Error> {
        statesync::qmdb::resolver_sync_target(&self.db).await
    }

    /// decode a [`PageMsg`] and apply it to the staged overlay. the only
    /// `.await` is on own qmdb state — deterministic, so replay-safe.
    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let m = decode_msg(&msg.payload).map_err(Error::Module)?;
        // origin + consensus time feed the comment ops (author + timestamp);
        // block ops ignore them. clone off `ctx` so `self` can borrow mutably.
        let origin = ctx.env().origin.clone();
        let now = ctx.env().consensus_time;
        self.apply(m, &origin, now)
            .await
            .map_err(|e| Error::Module(e.to_string()))
    }

    /// real async read of own qmdb state, serving STAGED-over-committed via
    /// the overlay, so reads within a block observe this block's writes. the
    /// reserved sentinel reads as absence (it is not a block).
    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            PageQuery::GetPage { page_id } => {
                let page = if page_id == PAGE_INDEX_KEY {
                    None
                } else {
                    self.load_page(&page_id).await?
                };
                Ok(encode_reply(&PageReply::Page(page)))
            }
            PageQuery::GetBlock { block_id } => {
                let block = if block_id == PAGE_INDEX_KEY {
                    None
                } else {
                    self.load_block(&block_id).await?
                };
                Ok(encode_reply(&PageReply::Block(block)))
            }
            PageQuery::ListPages => {
                // id -> folder parent straight from the reserved index entry;
                // titles read from the live roots so a rename shows without
                // touching the index.
                let index = self.load_index().await?;
                let mut pages = Vec::with_capacity(index.len());
                for (id, parent) in index {
                    let root = self
                        .load_block(&id)
                        .await?
                        .filter(|b| b.kind == BlockKind::Page)
                        .ok_or_else(|| Error::Module(PageError::Corrupt.to_string()))?;
                    pages.push(PageMeta {
                        id,
                        title: root.text,
                        parent,
                    });
                }
                Ok(encode_reply(&PageReply::PageList(pages)))
            }
            PageQuery::ThreadsForTargets { targets } => {
                if targets.len() > MAX_QUERY_TARGETS {
                    return Err(Error::Module(PageError::TooManyTargets.to_string()));
                }
                let err = |e: PageError| Error::Module(e.to_string());
                let mut out = Vec::with_capacity(targets.len());
                for target in targets {
                    let ids = self.load_target_index(&target).await.map_err(err)?;
                    let mut threads = Vec::new();
                    for tid in ids {
                        if let Some(view) = self.thread_view(&tid).await.map_err(err)? {
                            threads.push(view);
                        }
                    }
                    out.push(TargetThreads { target, threads });
                }
                Ok(encode_reply(&PageReply::CommentThreads(out)))
            }
            PageQuery::CommentThread { thread_id } => {
                let view = self
                    .thread_view(&thread_id)
                    .await
                    .map_err(|e| Error::Module(e.to_string()))?;
                Ok(encode_reply(&PageReply::CommentThread(view)))
            }
        }
    }

    /// publish the block-height's staged records in ONE qmdb batch: writes AND
    /// deletes (`batch.write(key, None)` drops a key). no-op (and no root
    /// movement) if nothing was staged.
    async fn commit_block(&mut self) -> Result<(), Error> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let mut batch = self.db.new_batch();
        for (key, value) in &self.pending {
            batch = batch.write(hash_key(key), value.clone());
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

    /// discard the staged records — nothing reached qmdb, so `root()` is
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
    use crate::{NewBlock, decode_reply, encode_msg, encode_query};
    use state::global_root;

    fn nb(id: &str, kind: BlockKind, text: &str) -> NewBlock {
        NewBlock {
            id: id.into(),
            kind,
            text: text.into(),
        }
    }

    fn para(id: &str, text: &str) -> NewBlock {
        nb(id, BlockKind::Paragraph, text)
    }

    fn msg(m: &PageMsg) -> Msg {
        Msg {
            target: "pages".into(),
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
                env: sdk::Env {
                    protocol_version: 0,
                    height: 0,
                    consensus_time: 0,
                    origin: sdk::Origin::System,
                    me: "pages".into(),
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

    // drive one op through execute + commit_block (one op per block-height).
    async fn apply_commit<E: Context + BufferPooler>(p: &mut Pages<E>, m: &PageMsg) {
        p.execute(&mut TestCtx::new(), &msg(m)).await.unwrap();
        p.commit_block().await.unwrap();
    }

    // an op that must FAIL, followed by the host's abort.
    async fn apply_expect_err<E: Context + BufferPooler>(
        p: &mut Pages<E>,
        m: &PageMsg,
        needle: &str,
    ) {
        let err = p
            .execute(&mut TestCtx::new(), &msg(m))
            .await
            .expect_err("op must be rejected");
        assert!(
            matches!(err, Error::Module(ref s) if s.contains(needle)),
            "unexpected error: {err:?}"
        );
        p.abort_block().await.unwrap();
    }

    async fn get_page<E: Context + BufferPooler>(
        p: &Pages<E>,
        page_id: &str,
    ) -> Option<Vec<Block>> {
        let reply = p
            .query(&encode_query(&PageQuery::GetPage {
                page_id: page_id.into(),
            }))
            .await
            .unwrap();
        match decode_reply(&reply).unwrap() {
            PageReply::Page(v) => v,
            _ => panic!("expected Page"),
        }
    }

    async fn get_block<E: Context + BufferPooler>(p: &Pages<E>, block_id: &str) -> Option<Block> {
        let reply = p
            .query(&encode_query(&PageQuery::GetBlock {
                block_id: block_id.into(),
            }))
            .await
            .unwrap();
        match decode_reply(&reply).unwrap() {
            PageReply::Block(b) => b,
            _ => panic!("expected Block"),
        }
    }

    async fn list_pages<E: Context + BufferPooler>(p: &Pages<E>) -> Vec<PageMeta> {
        let reply = p.query(&encode_query(&PageQuery::ListPages)).await.unwrap();
        match decode_reply(&reply).unwrap() {
            PageReply::PageList(l) => l,
            _ => panic!("expected PageList"),
        }
    }

    fn ids(blocks: &[Block]) -> Vec<&str> {
        blocks.iter().map(|b| b.id.as_str()).collect()
    }

    // ── comment test helpers (author-carrying origins) ──

    fn user(name: &str) -> sdk::Origin {
        sdk::Origin::External(name.as_bytes().to_vec())
    }
    fn ctx_as(origin: sdk::Origin) -> TestCtx {
        TestCtx {
            env: sdk::Env {
                protocol_version: 0,
                height: 0,
                consensus_time: 7,
                origin,
                me: "pages".into(),
            },
        }
    }
    async fn apply_commit_as<E: Context + BufferPooler>(
        p: &mut Pages<E>,
        m: &PageMsg,
        origin: sdk::Origin,
    ) {
        p.execute(&mut ctx_as(origin), &msg(m)).await.unwrap();
        p.commit_block().await.unwrap();
    }
    async fn apply_err_as<E: Context + BufferPooler>(
        p: &mut Pages<E>,
        m: &PageMsg,
        origin: sdk::Origin,
        needle: &str,
    ) {
        let err = p
            .execute(&mut ctx_as(origin), &msg(m))
            .await
            .expect_err("op must be rejected");
        assert!(
            matches!(err, Error::Module(ref s) if s.contains(needle)),
            "unexpected error: {err:?}"
        );
        p.abort_block().await.unwrap();
    }
    async fn query_threads<E: Context + BufferPooler>(
        p: &Pages<E>,
        targets: &[&str],
    ) -> Vec<TargetThreads> {
        let q = PageQuery::ThreadsForTargets {
            targets: targets.iter().map(|s| s.to_string()).collect(),
        };
        match decode_reply(&p.query(&encode_query(&q)).await.unwrap()).unwrap() {
            PageReply::CommentThreads(v) => v,
            _ => panic!("expected CommentThreads"),
        }
    }
    async fn query_thread<E: Context + BufferPooler>(
        p: &Pages<E>,
        thread_id: &str,
    ) -> Option<ThreadView> {
        let q = PageQuery::CommentThread { thread_id: thread_id.into() };
        match decode_reply(&p.query(&encode_query(&q)).await.unwrap()).unwrap() {
            PageReply::CommentThread(v) => v,
            _ => panic!("expected CommentThread"),
        }
    }

    // seed one page with three top-level paragraphs b1, b2, b3.
    async fn seed_page<E: Context + BufferPooler>(p: &mut Pages<E>, page: &str) {
        apply_commit(
            p,
            &PageMsg::CreatePage {
                page_id: page.into(),
                title: format!("{page} title"),
                parent: None,
            },
        )
        .await;
        let mut after = None;
        for id in ["b1", "b2", "b3"] {
            apply_commit(
                p,
                &PageMsg::InsertBlock {
                    parent: page.into(),
                    after,
                    block: para(id, id),
                },
            )
            .await;
            after = Some(id.to_string());
        }
    }

    #[test]
    fn create_page_and_insert_blocks_in_order() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            seed_page(&mut p, "p1").await;
            // after None -> first child, so b0 lands before b1.
            apply_commit(
                &mut p,
                &PageMsg::InsertBlock {
                    parent: "p1".into(),
                    after: None,
                    block: para("b0", "zero"),
                },
            )
            .await;

            let page = get_page(&p, "p1").await.unwrap();
            assert_eq!(ids(&page), ["p1", "b0", "b1", "b2", "b3"]);
            assert_eq!(page[0].kind, BlockKind::Page);
            assert_eq!(page[0].text, "p1 title");
            assert_eq!(page[0].children, ["b0", "b1", "b2", "b3"]);
        });
    }

    #[test]
    fn nested_children_come_back_in_preorder() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            seed_page(&mut p, "p1").await;
            // c1 under b1, d1 under c1: preorder puts b1's whole subtree
            // before its next sibling b2.
            apply_commit(
                &mut p,
                &PageMsg::InsertBlock {
                    parent: "b1".into(),
                    after: None,
                    block: para("c1", "child"),
                },
            )
            .await;
            apply_commit(
                &mut p,
                &PageMsg::InsertBlock {
                    parent: "c1".into(),
                    after: None,
                    block: para("d1", "grandchild"),
                },
            )
            .await;

            let page = get_page(&p, "p1").await.unwrap();
            assert_eq!(ids(&page), ["p1", "b1", "c1", "d1", "b2", "b3"]);
        });
    }

    // the addressability contract: a bare block id resolves with NO page
    // context, and the answer carries where the block lives — exactly what a
    // future cross-module BlockRef resolution needs.
    #[test]
    fn get_block_by_id_alone_carries_page_context() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            seed_page(&mut p, "p1").await;
            apply_commit(
                &mut p,
                &PageMsg::InsertBlock {
                    parent: "b1".into(),
                    after: None,
                    block: para("c1", "deep"),
                },
            )
            .await;

            let blk = get_block(&p, "c1").await.unwrap();
            assert_eq!(blk.parent.as_deref(), Some("b1"));
            assert_eq!(blk.page, "p1");
            assert_eq!(blk.text, "deep");
            // a non-page block id is NOT a page.
            assert!(get_page(&p, "c1").await.is_none());
        });
    }

    #[test]
    fn block_ids_are_globally_unique_across_pages() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            seed_page(&mut p, "p1").await;
            apply_commit(
                &mut p,
                &PageMsg::CreatePage {
                    page_id: "p2".into(),
                    title: "two".into(),
                    parent: None,
                },
            )
            .await;
            // b1 lives in p1 — inserting it into p2 must fail globally.
            apply_expect_err(
                &mut p,
                &PageMsg::InsertBlock {
                    parent: "p2".into(),
                    after: None,
                    block: para("b1", "dup"),
                },
                "duplicate block id",
            )
            .await;
            // a page id is a block id too: reusing one as a block id fails …
            apply_expect_err(
                &mut p,
                &PageMsg::InsertBlock {
                    parent: "p2".into(),
                    after: None,
                    block: para("p1", "dup"),
                },
                "duplicate block id",
            )
            .await;
            // … and creating a page over an existing NON-page block fails.
            apply_expect_err(
                &mut p,
                &PageMsg::CreatePage {
                    page_id: "b1".into(),
                    title: "steal".into(),
                    parent: None,
                },
                "duplicate block id",
            )
            .await;
        });
    }

    #[test]
    fn update_text_edits_blocks_and_renames_pages() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            seed_page(&mut p, "p1").await;
            apply_commit(
                &mut p,
                &PageMsg::UpdateText {
                    block_id: "b1".into(),
                    text: "edited".into(),
                },
            )
            .await;
            assert_eq!(get_block(&p, "b1").await.unwrap().text, "edited");

            // UpdateText on the root IS the rename; ListPages reads live roots.
            apply_commit(
                &mut p,
                &PageMsg::UpdateText {
                    block_id: "p1".into(),
                    text: "renamed".into(),
                },
            )
            .await;
            let pages = list_pages(&p).await;
            assert_eq!(pages.len(), 1);
            assert_eq!(pages[0].id, "p1");
            assert_eq!(pages[0].title, "renamed");
        });
    }

    #[test]
    fn set_kind_and_checked_enforce_their_domains() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            seed_page(&mut p, "p1").await;
            // paragraph -> todo, then check it off.
            apply_commit(
                &mut p,
                &PageMsg::SetKind {
                    block_id: "b1".into(),
                    kind: BlockKind::Todo,
                },
            )
            .await;
            apply_commit(
                &mut p,
                &PageMsg::SetChecked {
                    block_id: "b1".into(),
                    checked: true,
                },
            )
            .await;
            let b1 = get_block(&p, "b1").await.unwrap();
            assert_eq!(b1.kind, BlockKind::Todo);
            assert!(b1.checked);

            // checked is a todo-only surface.
            apply_expect_err(
                &mut p,
                &PageMsg::SetChecked {
                    block_id: "b2".into(),
                    checked: true,
                },
                "todo",
            )
            .await;
            // pages come only from CreatePage — no converting to Page …
            apply_expect_err(
                &mut p,
                &PageMsg::SetKind {
                    block_id: "b2".into(),
                    kind: BlockKind::Page,
                },
                "CreatePage",
            )
            .await;
            // … and no converting a root away from Page.
            apply_expect_err(
                &mut p,
                &PageMsg::SetKind {
                    block_id: "p1".into(),
                    kind: BlockKind::Paragraph,
                },
                "page roots",
            )
            .await;
        });
    }

    #[test]
    fn move_reorders_within_a_parent() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            seed_page(&mut p, "p1").await;
            // b1,b2,b3 -> move b1 after b3 -> b2,b3,b1.
            apply_commit(
                &mut p,
                &PageMsg::MoveBlock {
                    block_id: "b1".into(),
                    parent: "p1".into(),
                    after: Some("b3".into()),
                },
            )
            .await;
            assert_eq!(
                get_page(&p, "p1").await.unwrap()[0].children,
                ["b2", "b3", "b1"]
            );
            // back to the front (after None).
            apply_commit(
                &mut p,
                &PageMsg::MoveBlock {
                    block_id: "b1".into(),
                    parent: "p1".into(),
                    after: None,
                },
            )
            .await;
            assert_eq!(
                get_page(&p, "p1").await.unwrap()[0].children,
                ["b1", "b2", "b3"]
            );
            // self-anchor is a benign no-op.
            apply_commit(
                &mut p,
                &PageMsg::MoveBlock {
                    block_id: "b1".into(),
                    parent: "p1".into(),
                    after: Some("b1".into()),
                },
            )
            .await;
            assert_eq!(
                get_page(&p, "p1").await.unwrap()[0].children,
                ["b1", "b2", "b3"]
            );
        });
    }

    #[test]
    fn move_reparents_a_subtree() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            seed_page(&mut p, "p1").await;
            apply_commit(
                &mut p,
                &PageMsg::InsertBlock {
                    parent: "b2".into(),
                    after: None,
                    block: para("c1", "rides along"),
                },
            )
            .await;
            // b2 (with c1 below) becomes b1's child — the subtree rides along.
            apply_commit(
                &mut p,
                &PageMsg::MoveBlock {
                    block_id: "b2".into(),
                    parent: "b1".into(),
                    after: None,
                },
            )
            .await;
            let page = get_page(&p, "p1").await.unwrap();
            assert_eq!(ids(&page), ["p1", "b1", "b2", "c1", "b3"]);
            assert_eq!(
                get_block(&p, "b2").await.unwrap().parent.as_deref(),
                Some("b1")
            );
            // c1 still knows its page.
            assert_eq!(get_block(&p, "c1").await.unwrap().page, "p1");
        });
    }

    #[test]
    fn illegal_moves_are_rejected() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            seed_page(&mut p, "p1").await;
            apply_commit(
                &mut p,
                &PageMsg::CreatePage {
                    page_id: "p2".into(),
                    title: "two".into(),
                    parent: None,
                },
            )
            .await;
            apply_commit(
                &mut p,
                &PageMsg::InsertBlock {
                    parent: "b1".into(),
                    after: None,
                    block: para("c1", "child"),
                },
            )
            .await;
            // into one's own subtree: b1 under its child c1.
            apply_expect_err(
                &mut p,
                &PageMsg::MoveBlock {
                    block_id: "b1".into(),
                    parent: "c1".into(),
                    after: None,
                },
                "inside the moved subtree",
            )
            .await;
            // across pages.
            apply_expect_err(
                &mut p,
                &PageMsg::MoveBlock {
                    block_id: "b1".into(),
                    parent: "p2".into(),
                    after: None,
                },
                "cross-page",
            )
            .await;
            // a page root.
            apply_expect_err(
                &mut p,
                &PageMsg::MoveBlock {
                    block_id: "p2".into(),
                    parent: "b1".into(),
                    after: None,
                },
                "page roots",
            )
            .await;
            // a bad sibling anchor.
            apply_expect_err(
                &mut p,
                &PageMsg::MoveBlock {
                    block_id: "b1".into(),
                    parent: "p1".into(),
                    after: Some("ghost".into()),
                },
                "after-anchor",
            )
            .await;
        });
    }

    #[test]
    fn remove_deletes_the_whole_subtree() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            seed_page(&mut p, "p1").await;
            apply_commit(
                &mut p,
                &PageMsg::InsertBlock {
                    parent: "b1".into(),
                    after: None,
                    block: para("c1", "child"),
                },
            )
            .await;
            apply_commit(
                &mut p,
                &PageMsg::InsertBlock {
                    parent: "c1".into(),
                    after: None,
                    block: para("d1", "grandchild"),
                },
            )
            .await;
            apply_commit(&mut p, &PageMsg::RemoveBlock { block_id: "b1".into() }).await;

            // b1, c1, d1 all gone — by id and from the page.
            for gone in ["b1", "c1", "d1"] {
                assert!(get_block(&p, gone).await.is_none(), "{gone} must be gone");
            }
            let page = get_page(&p, "p1").await.unwrap();
            assert_eq!(ids(&page), ["p1", "b2", "b3"]);

            // roots are not removable.
            apply_expect_err(
                &mut p,
                &PageMsg::RemoveBlock { block_id: "p1".into() },
                "page roots",
            )
            .await;
        });
    }

    #[test]
    fn write_moves_root_and_composes_into_global_root() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            let r0 = p.root();
            seed_page(&mut p, "p1").await;
            let r1 = p.root();
            assert_ne!(r0, r1, "a write must move the root");
            assert_ne!(r1, StateRoot::ZERO, "root after write must be non-zero");

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
                let mods: [&dyn Module; 2] = [&p, &stub];
                global_root(&mods)
            };
            assert_ne!(g, state::global_root(&[&stub as &dyn Module]));
        });
    }

    // host-lent staging: a whole staged edit (including a staged DELETE) that
    // then ABORTS must leave no trace.
    #[test]
    fn staged_writes_and_deletes_roll_back_on_abort() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            seed_page(&mut p, "p1").await;
            let r_before = p.root();

            // stage a removal (a delete) AND an insert, then abort.
            p.execute(
                &mut TestCtx::new(),
                &msg(&PageMsg::RemoveBlock { block_id: "b2".into() }),
            )
            .await
            .unwrap();
            p.execute(
                &mut TestCtx::new(),
                &msg(&PageMsg::InsertBlock {
                    parent: "p1".into(),
                    after: None,
                    block: para("ghost", "should vanish"),
                }),
            )
            .await
            .unwrap();
            p.abort_block().await.unwrap();

            assert_eq!(p.root(), r_before, "aborted block must not move the root");
            let page = get_page(&p, "p1").await.unwrap();
            assert_eq!(ids(&page), ["p1", "b1", "b2", "b3"]);
        });
    }

    // mid-block read-your-writes across the overlay, INCLUDING staged deletes:
    // op2 parents on op1's staged insert; op4 sees op3's staged delete.
    #[test]
    fn staged_writes_are_visible_within_one_block() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            apply_commit(
                &mut p,
                &PageMsg::CreatePage {
                    page_id: "p1".into(),
                    title: "one".into(),
                    parent: None,
                },
            )
            .await;
            // two inserts, NO commit between: the child hangs off a parent
            // that exists only in the overlay.
            p.execute(
                &mut TestCtx::new(),
                &msg(&PageMsg::InsertBlock {
                    parent: "p1".into(),
                    after: None,
                    block: para("b1", "one"),
                }),
            )
            .await
            .unwrap();
            p.execute(
                &mut TestCtx::new(),
                &msg(&PageMsg::InsertBlock {
                    parent: "b1".into(),
                    after: None,
                    block: para("c1", "two"),
                }),
            )
            .await
            .unwrap();
            // a staged delete is visible too: re-inserting the removed id in
            // the SAME block-height succeeds (absence through the overlay).
            p.execute(
                &mut TestCtx::new(),
                &msg(&PageMsg::RemoveBlock { block_id: "c1".into() }),
            )
            .await
            .unwrap();
            p.execute(
                &mut TestCtx::new(),
                &msg(&PageMsg::InsertBlock {
                    parent: "b1".into(),
                    after: None,
                    block: para("c1", "again"),
                }),
            )
            .await
            .unwrap();
            p.commit_block().await.unwrap();

            let page = get_page(&p, "p1").await.unwrap();
            assert_eq!(ids(&page), ["p1", "b1", "c1"]);
            assert_eq!(page[2].text, "again");
        });
    }

    // the poison-pill guard: an op that would grow one serialized block past
    // MAX_BLOCK_LEN is rejected at WRITE time — never staged, never committed.
    #[test]
    fn oversized_block_is_rejected_before_staging() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            apply_commit(
                &mut p,
                &PageMsg::CreatePage {
                    page_id: "p1".into(),
                    title: "one".into(),
                    parent: None,
                },
            )
            .await;
            let r_before = p.root();

            let huge = "x".repeat(MAX_BLOCK_LEN + 1);
            apply_expect_err(
                &mut p,
                &PageMsg::InsertBlock {
                    parent: "p1".into(),
                    after: None,
                    block: para("big", &huge),
                },
                "block too large",
            )
            .await;
            assert!(p.pending.is_empty(), "a rejected write must not be staged");
            assert_eq!(p.root(), r_before, "a rejected write must not move the root");
            assert_eq!(get_page(&p, "p1").await.unwrap().len(), 1);
        });
    }

    // corruption must surface as a DISTINCT error, never absence: mapping it
    // to "not found" would let CreatePage re-seed a root over the corrupt
    // bytes, silently destroying the data.
    #[test]
    fn corrupt_stored_block_errors_as_corruption_not_absence() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            // commit bytes that are NOT valid Block json under blk1's key
            // (simulating on-disk corruption; unreachable through PageMsg ops).
            p.pending
                .insert(b"blk1".to_vec(), Some(b"not json".to_vec()));
            p.commit_block().await.unwrap();

            apply_expect_err(
                &mut p,
                &PageMsg::UpdateText {
                    block_id: "blk1".into(),
                    text: "x".into(),
                },
                "corrupt",
            )
            .await;
            apply_expect_err(
                &mut p,
                &PageMsg::CreatePage {
                    page_id: "blk1".into(),
                    title: "steal".into(),
                    parent: None,
                },
                "corrupt",
            )
            .await;
            // the read path surfaces the decode failure too (error, not None).
            assert!(
                p.query(&encode_query(&PageQuery::GetBlock {
                    block_id: "blk1".into()
                }))
                .await
                .is_err()
            );
        });
    }

    #[test]
    fn create_page_is_idempotent_and_preserves_the_title() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            seed_page(&mut p, "p1").await;
            apply_commit(
                &mut p,
                &PageMsg::UpdateText {
                    block_id: "p1".into(),
                    text: "renamed".into(),
                },
            )
            .await;
            // re-create must neither wipe blocks nor clobber the live title.
            apply_commit(
                &mut p,
                &PageMsg::CreatePage {
                    page_id: "p1".into(),
                    title: "stale title".into(),
                    parent: None,
                },
            )
            .await;
            let page = get_page(&p, "p1").await.unwrap();
            assert_eq!(ids(&page), ["p1", "b1", "b2", "b3"]);
            assert_eq!(page[0].text, "renamed");
            assert_eq!(list_pages(&p).await.len(), 1);
        });
    }

    #[test]
    fn list_pages_enumerates_sorted_with_live_titles() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            assert!(list_pages(&p).await.is_empty(), "a fresh store lists nothing");
            // create out of order; the index comes back sorted by id.
            for (id, title) in [("zebra", "Z"), ("alpha", "A"), ("mid", "M")] {
                apply_commit(
                    &mut p,
                    &PageMsg::CreatePage {
                        page_id: id.into(),
                        title: title.into(),
                        parent: None,
                    },
                )
                .await;
            }
            let pages = list_pages(&p).await;
            let got: Vec<(&str, &str)> = pages
                .iter()
                .map(|m| (m.id.as_str(), m.title.as_str()))
                .collect();
            assert_eq!(got, [("alpha", "A"), ("mid", "M"), ("zebra", "Z")]);
        });
    }

    // the reserved sentinel is UNREACHABLE by any op, so a block write can
    // never overwrite the enumeration index; and it reads as absence.
    #[test]
    fn reserved_index_id_is_rejected() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            seed_page(&mut p, "p1").await;
            let r_before = p.root();

            apply_expect_err(
                &mut p,
                &PageMsg::CreatePage {
                    page_id: PAGE_INDEX_KEY.into(),
                    title: "clobber".into(),
                    parent: None,
                },
                "reserved block id",
            )
            .await;
            apply_expect_err(
                &mut p,
                &PageMsg::InsertBlock {
                    parent: "p1".into(),
                    after: None,
                    block: para(PAGE_INDEX_KEY, "clobber"),
                },
                "reserved block id",
            )
            .await;
            assert!(p.pending.is_empty(), "a rejected op must stage nothing");
            assert_eq!(p.root(), r_before, "a rejected op must not move the root");
            // the sentinel reads as absence on the query surface.
            assert!(get_block(&p, PAGE_INDEX_KEY).await.is_none());
            assert!(get_page(&p, PAGE_INDEX_KEY).await.is_none());
            assert_eq!(list_pages(&p).await.len(), 1);
        });
    }

    // ── nested pages (folder relation in the index) ──

    #[test]
    fn create_with_parent_records_folder_edge() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            apply_commit(&mut p, &PageMsg::CreatePage {
                page_id: "root".into(), title: "Root".into(), parent: None,
            }).await;
            apply_commit(&mut p, &PageMsg::CreatePage {
                page_id: "child".into(), title: "Child".into(), parent: Some("root".into()),
            }).await;
            let pages = list_pages(&p).await;
            let child = pages.iter().find(|m| m.id == "child").unwrap();
            assert_eq!(child.parent.as_deref(), Some("root"));
            let root = pages.iter().find(|m| m.id == "root").unwrap();
            assert_eq!(root.parent, None);
        });
    }

    #[test]
    fn create_under_missing_or_nonpage_parent_is_rejected() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            seed_page(&mut p, "p1").await; // p1 + blocks b1,b2,b3
            // parent does not exist
            apply_expect_err(&mut p, &PageMsg::CreatePage {
                page_id: "x".into(), title: "x".into(), parent: Some("ghost".into()),
            }, "parent page not found").await;
            // parent exists but is a non-page block
            apply_expect_err(&mut p, &PageMsg::CreatePage {
                page_id: "y".into(), title: "y".into(), parent: Some("b1".into()),
            }, "parent page not found").await;
        });
    }

    #[test]
    fn set_page_parent_renests_and_rejects_cycles() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            for id in ["a", "b", "c"] {
                apply_commit(&mut p, &PageMsg::CreatePage {
                    page_id: id.into(), title: id.into(), parent: None,
                }).await;
            }
            // b under a, c under b.
            apply_commit(&mut p, &PageMsg::SetPageParent { page_id: "b".into(), parent: Some("a".into()) }).await;
            apply_commit(&mut p, &PageMsg::SetPageParent { page_id: "c".into(), parent: Some("b".into()) }).await;
            let parent_of = |pages: &[PageMeta], id: &str| pages.iter().find(|m| m.id == id).unwrap().parent.clone();
            let pages = list_pages(&p).await;
            assert_eq!(parent_of(&pages, "b"), Some("a".into()));
            assert_eq!(parent_of(&pages, "c"), Some("b".into()));
            // a under c would cycle (a -> c -> b -> a).
            apply_expect_err(&mut p, &PageMsg::SetPageParent { page_id: "a".into(), parent: Some("c".into()) }, "page cycle").await;
            // self-parent cycles too.
            apply_expect_err(&mut p, &PageMsg::SetPageParent { page_id: "a".into(), parent: Some("a".into()) }, "page cycle").await;
            // detach to top level.
            apply_commit(&mut p, &PageMsg::SetPageParent { page_id: "b".into(), parent: None }).await;
            assert_eq!(parent_of(&list_pages(&p).await, "b"), None);
            // target must be a page root.
            seed_page(&mut p, "pg").await;
            apply_expect_err(&mut p, &PageMsg::SetPageParent { page_id: "b1".into(), parent: None }, "not a page").await;
            // parent must be a page.
            apply_expect_err(&mut p, &PageMsg::SetPageParent { page_id: "a".into(), parent: Some("b1".into()) }, "parent page not found").await;
        });
    }

    #[test]
    fn delete_page_removes_subtree_and_promotes_children() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            // grand -> parent -> child ; parent also has a content block pb1.
            apply_commit(&mut p, &PageMsg::CreatePage { page_id: "grand".into(), title: "G".into(), parent: None }).await;
            apply_commit(&mut p, &PageMsg::CreatePage { page_id: "parent".into(), title: "P".into(), parent: Some("grand".into()) }).await;
            apply_commit(&mut p, &PageMsg::CreatePage { page_id: "child".into(), title: "C".into(), parent: Some("parent".into()) }).await;
            apply_commit(&mut p, &PageMsg::InsertBlock { parent: "parent".into(), after: None, block: para("pb1", "body") }).await;

            apply_commit(&mut p, &PageMsg::DeletePage { page_id: "parent".into() }).await;

            // parent's root + content block are gone …
            assert!(get_block(&p, "parent").await.is_none());
            assert!(get_block(&p, "pb1").await.is_none());
            assert!(get_page(&p, "parent").await.is_none());
            // … child was PROMOTED to grand (parent's parent), not deleted.
            let pages = list_pages(&p).await;
            assert!(pages.iter().all(|m| m.id != "parent"));
            let child = pages.iter().find(|m| m.id == "child").unwrap();
            assert_eq!(child.parent.as_deref(), Some("grand"));

            // deleting a non-page id is rejected.
            seed_page(&mut p, "pg").await;
            apply_expect_err(&mut p, &PageMsg::DeletePage { page_id: "b1".into() }, "not a page").await;
        });
    }

    // ── comments (folded into the pages module) ──

    fn add(thread: &str, comment: &str, target: &str, text: &str) -> PageMsg {
        PageMsg::AddComment {
            thread_id: thread.into(),
            comment_id: comment.into(),
            target: target.into(),
            text: text.into(),
        }
    }

    #[test]
    fn comment_add_opens_then_appends_and_batches_by_target() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            apply_commit_as(&mut p, &add("t1", "m1", "b1", "first"), user("alice")).await;
            apply_commit_as(&mut p, &add("t1", "m2", "b1", "second"), user("bob")).await;
            apply_commit_as(&mut p, &add("t2", "m3", "b1", "other"), user("alice")).await;
            apply_commit_as(&mut p, &add("t3", "m4", "b2", "elsewhere"), user("alice")).await;

            let groups = query_threads(&p, &["b1", "b2"]).await;
            let b1 = groups.iter().find(|g| g.target == "b1").unwrap();
            assert_eq!(b1.threads.len(), 2);
            let t1 = b1.threads.iter().find(|v| v.thread.id == "t1").unwrap();
            assert_eq!(t1.comments.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(), ["first", "second"]);
            assert_eq!(t1.thread.opener, AuthorRef::User(b"alice".to_vec()));
            assert_eq!(t1.comments[1].author, AuthorRef::User(b"bob".to_vec()));
            assert_eq!(groups.iter().find(|g| g.target == "b2").unwrap().threads.len(), 1);
        });
    }

    #[test]
    fn comment_append_rejects_target_mismatch_duplicate_and_empty_origin() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            apply_commit_as(&mut p, &add("t1", "m1", "b1", "x"), user("alice")).await;
            apply_err_as(&mut p, &add("t1", "m2", "b2", "y"), user("alice"), "target mismatch").await;
            apply_err_as(&mut p, &add("t1", "m1", "b1", "z"), user("alice"), "duplicate comment id").await;
            apply_err_as(&mut p, &add("t9", "m9", "b1", "z"), sdk::Origin::External(vec![]), "empty origin").await;
        });
    }

    #[test]
    fn comment_edit_and_delete_are_author_only() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            apply_commit_as(&mut p, &add("t1", "m1", "b1", "orig"), user("alice")).await;
            apply_err_as(&mut p, &PageMsg::EditComment { comment_id: "m1".into(), text: "hax".into() }, user("bob"), "not the comment author").await;
            apply_err_as(&mut p, &PageMsg::DeleteComment { comment_id: "m1".into() }, user("bob"), "not the comment author").await;
            apply_commit_as(&mut p, &PageMsg::EditComment { comment_id: "m1".into(), text: "edited".into() }, user("alice")).await;
            let v = query_thread(&p, "t1").await.unwrap();
            assert_eq!(v.comments[0].text, "edited");
            assert_eq!(v.comments[0].edited_at, Some(7));
        });
    }

    #[test]
    fn comment_deleting_last_live_removes_the_thread() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            apply_commit_as(&mut p, &add("t1", "m1", "b1", "a"), user("alice")).await;
            apply_commit_as(&mut p, &add("t1", "m2", "b1", "b"), user("alice")).await;
            apply_commit_as(&mut p, &PageMsg::DeleteComment { comment_id: "m1".into() }, user("alice")).await;
            let v = query_thread(&p, "t1").await.unwrap();
            assert_eq!(v.comments.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(), ["b"]);
            apply_commit_as(&mut p, &PageMsg::DeleteComment { comment_id: "m2".into() }, user("alice")).await;
            assert!(query_thread(&p, "t1").await.is_none());
            assert!(query_threads(&p, &["b1"]).await[0].threads.is_empty());
        });
    }

    #[test]
    fn comment_resolve_toggles_and_records_resolver() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            apply_commit_as(&mut p, &add("t1", "m1", "b1", "a"), user("alice")).await;
            apply_commit_as(&mut p, &PageMsg::ResolveThread { thread_id: "t1".into(), resolved: true }, user("bob")).await;
            let v = query_thread(&p, "t1").await.unwrap();
            assert!(v.thread.resolved);
            assert_eq!(v.thread.resolved_by, Some(AuthorRef::User(b"bob".to_vec())));
            apply_commit_as(&mut p, &PageMsg::ResolveThread { thread_id: "t1".into(), resolved: false }, user("alice")).await;
            assert_eq!(query_thread(&p, "t1").await.unwrap().thread.resolved_by, None);
            apply_err_as(&mut p, &PageMsg::ResolveThread { thread_id: "ghost".into(), resolved: true }, user("alice"), "thread not found").await;
        });
    }

    #[test]
    fn comment_caps_and_reserved_ids_reject() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            let huge = "x".repeat(MAX_COMMENT_TEXT_BYTES + 1);
            apply_err_as(&mut p, &add("t1", "m1", "b1", &huge), user("alice"), "comment text too large").await;
            assert!(p.pending.is_empty(), "a rejected comment op stages nothing");
            // a NUL-prefixed id lands in the reserved keyspace — rejected.
            apply_err_as(&mut p, &add("\u{0}evil", "m1", "b1", "x"), user("alice"), "reserved block id").await;
            // over-cap query rejected.
            let targets: Vec<String> = (0..=MAX_QUERY_TARGETS).map(|i| format!("t{i}")).collect();
            assert!(p.query(&encode_query(&PageQuery::ThreadsForTargets { targets })).await.is_err());
        });
    }

    // comments ride the same qmdb as blocks (reserved NUL-prefixed keys), so a
    // block edit and a comment op never collide, and both compose into root.
    #[test]
    fn comments_and_blocks_coexist_and_move_the_root() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            seed_page(&mut p, "p1").await;
            let before = p.root();
            apply_commit_as(&mut p, &add("t1", "m1", "b1", "note on b1"), user("alice")).await;
            assert_ne!(p.root(), before, "a comment write moves the root");
            // the block tree is untouched by the comment.
            assert_eq!(ids(&get_page(&p, "p1").await.unwrap()), ["p1", "b1", "b2", "b3"]);
            assert_eq!(query_threads(&p, &["b1"]).await[0].threads.len(), 1);
        });
    }
}
