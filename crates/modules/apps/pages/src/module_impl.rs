use super::{
    AuthorRef, BlockKind, Ctx, Error, MAX_QUERY_TARGETS, Module, ModuleId, Msg, PAGE_INDEX_KEY,
    PageError, PageMeta, PageQuery, PageReply, Pages, ResolverSyncTarget, StateRoot,
    StateSyncHandle, TagEvent, TaggingMsg, TargetThreads, decode_msg, decode_query, encode_reply,
};

fn tag_author(author: &AuthorRef) -> tagging::Author {
    match author {
        AuthorRef::User(key) => tagging::Author::User(key.clone()),
        AuthorRef::Agent { module, agent_id } => tagging::Author::Entity(tagging::EntityRef {
            module: module.clone(),
            entity: agent_id.clone(),
        }),
        AuthorRef::Module(module) => tagging::Author::Module(module.clone()),
        AuthorRef::System => tagging::Author::System,
    }
}

fn tag_ref(author: &AuthorRef) -> Option<tagging::EntityRef> {
    match author {
        AuthorRef::Agent { module, agent_id } => Some(tagging::EntityRef {
            module: module.clone(),
            entity: agent_id.clone(),
        }),
        _ => None,
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Pages {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// the store's REAL merkle root over all blocks, as a 32-byte state root.
    fn root(&self) -> StateRoot {
        self.staged.root()
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        self.staged.state_sync_handle()
    }

    /// the network state-sync serve lane: answers the shared qmdb wire
    /// requests from committed state. read-only.
    async fn serve_sync(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        self.staged.serve_sync(req).await
    }

    async fn resolver_sync_target(&self) -> Result<ResolverSyncTarget, Error> {
        self.staged.sync_target().await
    }

    /// decode a [`crate::PageMsg`] and apply it to the staged overlay. the only
    /// `.await` is on own store state — deterministic, so replay-safe.
    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let m = decode_msg(&msg.payload).map_err(Error::Module)?;
        let tagged_comment = match &m {
            super::PageMsg::AddComment {
                comment_id,
                mentions,
                ..
            } => Some((comment_id.clone(), mentions.clone())),
            super::PageMsg::EditComment {
                comment_id,
                mentions,
                ..
            } if !mentions.is_empty() => Some((comment_id.clone(), mentions.clone())),
            _ => None,
        };
        // origin + consensus time feed the comment ops (author + timestamp);
        // block ops ignore them. clone off `ctx` so `self` can borrow mutably.
        let origin = ctx.env().origin.clone();
        let now = ctx.env().consensus_time;
        self.apply(m, &origin, now)
            .await
            .map_err(|e| Error::Module(e.to_string()))?;
        if let (Some(tagging), Some((comment_id, mentions))) = (&self.tagging, tagged_comment) {
            let comment = self
                .load_comment(&comment_id)
                .await
                .map_err(|e| Error::Module(e.to_string()))?
                .ok_or_else(|| Error::Module("staged comment is missing".into()))?;
            let thread_id = comment.thread_id.clone();
            let thread = self
                .load_thread(&thread_id)
                .await
                .map_err(|e| Error::Module(e.to_string()))?
                .ok_or_else(|| Error::Module("staged comment thread is missing".into()))?;
            let ordinal = thread
                .comment_ids
                .iter()
                .position(|id| id == &comment_id)
                .map(|index| index as u64 + 1)
                .ok_or_else(|| Error::Module("staged comment is absent from its thread".into()))?;
            ctx.emit_msg(Msg {
                target: tagging.clone(),
                payload: tagging::encode_msg(&TaggingMsg::Tag(TagEvent {
                    container: thread_id,
                    content_seq: ordinal,
                    author: tag_author(&comment.author),
                    tags: mentions.iter().filter_map(tag_ref).collect(),
                })),
            });
        }
        Ok(())
    }

    /// real async read of own store state, serving STAGED-over-committed via
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

    /// publish the block-height's staged records in ONE store batch: writes
    /// AND deletes (a `None` value drops a key). no-op (and no root movement)
    /// if nothing was staged. BTreeMap iteration keeps the write order
    /// deterministic across validators.
    async fn commit_block(&mut self) -> Result<(), Error> {
        self.staged.commit().await
    }

    /// discard the staged records — nothing reached the store, so `root()` is
    /// unchanged.
    async fn abort_block(&mut self) -> Result<(), Error> {
        self.staged.abort();
        Ok(())
    }
}
