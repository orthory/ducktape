use super::{
    Arc, BTreeMap, Block, BlockKind, BufferPooler, CacheRef, Context, DbResolver, Error, Hasher,
    MAX_BLOCK_LEN, MAX_DEPTH, ModuleId, NonEmptyRange, NonZeroU16, NonZeroU64, NonZeroUsize,
    PAGE_INDEX_KEY, PageError, PageKey, Pages, PagesConfig, PagesDb, PagesTarget, RangeCfg,
    Sequential, Sha256, SyncConfig, Target, TwoCap, VariableConfig, journal, mmr, sync,
    to_page_err,
};

/// hash a `block_id` to its fixed-width qmdb key. deterministic, so every
/// validator maps a given block to the same store slot.
pub(super) fn hash_key(block_id: &[u8]) -> PageKey {
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
    pub(super) async fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        if let Some(staged) = self.pending.get(key) {
            return staged.clone();
        }
        self.db.get(&hash_key(key)).await.expect("get failed")
    }

    /// load one block (`None` == absent), through the staged-over-committed
    /// overlay. a decode failure is corruption, surfaced as an error.
    pub(super) async fn load_block(&self, block_id: &str) -> Result<Option<Block>, Error> {
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
    pub(super) fn stage(&mut self, key: &str, bytes: Vec<u8>) -> Result<(), PageError> {
        if bytes.len() > MAX_BLOCK_LEN {
            return Err(PageError::BlockTooLarge);
        }
        self.pending.insert(key.as_bytes().to_vec(), Some(bytes));
        Ok(())
    }

    /// stage one block record.
    pub(super) fn store_block(&mut self, block: &Block) -> Result<(), PageError> {
        let bytes = serde_json::to_vec(block).expect("Block is always serializable");
        self.stage(&block.id, bytes)
    }

    /// stage a DELETE of `block_id` — reads see absence at once; the key is
    /// dropped from qmdb (and the root) at `commit_block`.
    pub(super) fn delete_block(&mut self, block_id: &str) {
        self.pending.insert(block_id.as_bytes().to_vec(), None);
    }

    /// load the enumeration index — page id → folder parent — through the
    /// staged-over-committed overlay. absent reads as the empty map; a decode
    /// failure is corruption. `BTreeMap` serializes with SORTED keys, so the
    /// bytes are canonical and every validator commits the same index root.
    pub(super) async fn load_index(&self) -> Result<BTreeMap<String, Option<String>>, Error> {
        match self.get(PAGE_INDEX_KEY.as_bytes()).await {
            Some(b) => serde_json::from_slice(&b).map_err(|e| Error::Module(e.to_string())),
            None => Ok(BTreeMap::new()),
        }
    }

    /// re-stage the whole index map (canonical serialization).
    pub(super) fn stage_index(
        &mut self,
        index: &BTreeMap<String, Option<String>>,
    ) -> Result<(), PageError> {
        let bytes = serde_json::to_vec(index).expect("index is always serializable");
        self.stage(PAGE_INDEX_KEY, bytes)
    }

    /// add `page_id -> parent` to the index if absent (idempotent create keeps
    /// the existing entry, so re-create never re-nests).
    pub(super) async fn index_add(
        &mut self,
        page_id: &str,
        parent: Option<String>,
    ) -> Result<(), PageError> {
        let mut index = self.load_index().await.map_err(to_page_err)?;
        if !index.contains_key(page_id) {
            index.insert(page_id.to_string(), parent);
            self.stage_index(&index)?;
        }
        Ok(())
    }

    /// load a block that MUST exist (`missing` names the error when it does
    /// not) — the shared shape of every "look up then edit" op.
    pub(super) async fn require_block(
        &self,
        block_id: &str,
        missing: PageError,
    ) -> Result<Block, PageError> {
        self.load_block(block_id)
            .await
            .map_err(to_page_err)?
            .ok_or(missing)
    }

    /// walk parent pointers from `start` to the page root, erroring with
    /// [`PageError::CycleMove`] if `forbidden` appears on the path (that would
    /// reparent a block inside its own subtree). the [`MAX_DEPTH`] cap turns a
    /// corrupt (looping) parent chain into a loud error instead of a hang.
    pub(super) async fn ancestry_excludes(
        &self,
        start: &str,
        forbidden: &str,
    ) -> Result<(), PageError> {
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
    pub(super) async fn folder_ancestry_excludes(
        &self,
        start: &str,
        forbidden: &str,
    ) -> Result<(), PageError> {
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
    /// assemble a whole page in PREORDER (root first, each block's subtree
    /// before its next sibling), through the staged overlay. `None` when no
    /// PAGE lives at `page_id` (a non-root block id reads as absent here —
    /// `GetBlock` is the by-id surface).
    pub(super) async fn load_page(&self, page_id: &str) -> Result<Option<Vec<Block>>, Error> {
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
