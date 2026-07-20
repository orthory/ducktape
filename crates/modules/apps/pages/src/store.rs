use super::{
    BTreeMap, Block, BlockKind, Error, MAX_BLOCK_LEN, MAX_DEPTH, MerkleStore, ModuleId,
    PAGE_INDEX_KEY, PageError, Pages, to_page_err,
};
use sha2::Digest as _;

/// hash a `block_id` to its fixed-width store key. deterministic, so every
/// validator maps a given block to the same store slot.
pub(super) fn hash_key(block_id: &[u8]) -> [u8; 32] {
    sha2::Sha256::digest(block_id).into()
}

impl Pages {
    /// wrap the host-constructed store under module identity `id`. sync — the
    /// store arrives already opened (or already synced to a verified root).
    pub fn new(id: impl Into<ModuleId>, store: Box<dyn MerkleStore>) -> Self {
        Self {
            id: id.into(),
            store,
            pending: BTreeMap::new(),
            tagging: None,
        }
    }

    /// Report newly-added comments to the shared engagement router.
    pub fn with_tagging(mut self, tagging: impl Into<ModuleId>) -> Self {
        self.tagging = Some(tagging.into());
        self
    }

    /// read raw bytes for `key` through the staged overlay: a staged write
    /// shadows committed state, and a staged DELETE reads as absence.
    pub(super) async fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        if let Some(staged) = self.pending.get(key) {
            return staged.clone();
        }
        self.store.get(&hash_key(key)).await.expect("get failed")
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
    /// dropped from the store (and the root) at `commit_block`.
    pub(super) fn delete_block(&mut self, block_id: &str) {
        self.pending.insert(block_id.as_bytes().to_vec(), None);
    }

    /// delete a whole subtree depth-first, purging each block's comments and
    /// staging its delete (the shared RemoveBlock/DeletePage walk). a child
    /// listed but absent from the store is a broken invariant, surfaced loudly.
    pub(super) async fn delete_subtree(&mut self, root: Block) -> Result<(), PageError> {
        let mut stack = vec![root];
        while let Some(cur) = stack.pop() {
            for child in &cur.children {
                stack.push(self.require_block(child, PageError::Corrupt).await?);
            }
            self.purge_comments_for_target(&cur.id).await?;
            self.delete_block(&cur.id);
        }
        Ok(())
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
}
