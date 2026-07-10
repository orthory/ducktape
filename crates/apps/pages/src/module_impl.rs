use super::{
    BlockKind, BufferPooler, Context, Ctx, Error, MAX_QUERY_TARGETS, Module, ModuleId, Msg,
    PAGE_INDEX_KEY, PageError, PageMeta, PageQuery, PageReply, Pages, ResolverSyncTarget,
    StateRoot, StateSyncHandle, TargetThreads, decode_msg, decode_query, encode_reply, hash_key,
};

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

    /// decode a [`crate::PageMsg`] and apply it to the staged overlay. the only
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
            PageQuery::GetComment { comment_id } => {
                let comment = self
                    .load_comment(&comment_id)
                    .await
                    .map_err(|e| Error::Module(e.to_string()))?;
                Ok(encode_reply(&PageReply::Comment(comment)))
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
