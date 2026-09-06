use super::{
    AuthorRef, Comment, MAX_COMMENT_AGENT_ID_BYTES, MAX_COMMENT_ID_BYTES, MAX_COMMENT_TARGET_BYTES,
    MAX_COMMENT_TEXT_BYTES, MAX_COMMENT_WORK_PER_TARGET, MAX_COMMENTS_PER_THREAD,
    MAX_THREAD_ID_BYTES, MAX_THREADS_PER_TARGET, Origin, PageError, PageMsg, Pages, Thread,
    ThreadView, author_from_origin, id_is_index_safe,
};
use crate::text_ranges::{TextEdit, rebase_anchor, valid_range};

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

impl Pages {
    // ── comment storage (reserved NUL-prefixed keys) ──

    pub(super) async fn load_thread(&self, id: &str) -> Result<Option<Thread>, PageError> {
        match self.get(thread_key(id).as_bytes()).await {
            Some(b) => Ok(Some(
                serde_json::from_slice(&b).map_err(|_| PageError::Corrupt)?,
            )),
            None => Ok(None),
        }
    }

    pub(super) async fn load_comment(&self, id: &str) -> Result<Option<Comment>, PageError> {
        match self.get(comment_key(id).as_bytes()).await {
            Some(b) => Ok(Some(
                serde_json::from_slice(&b).map_err(|_| PageError::Corrupt)?,
            )),
            None => Ok(None),
        }
    }

    fn store_thread(&mut self, t: &Thread) -> Result<(), PageError> {
        self.stage(
            &thread_key(&t.id),
            serde_json::to_vec(t).expect("thread serializable"),
        )
    }

    fn store_comment(&mut self, c: &Comment) -> Result<(), PageError> {
        self.stage(
            &comment_key(&c.id),
            serde_json::to_vec(c).expect("comment serializable"),
        )
    }

    pub(super) async fn load_target_index(&self, target: &str) -> Result<Vec<String>, PageError> {
        match self.get(target_index_key(target).as_bytes()).await {
            Some(b) => serde_json::from_slice(&b).map_err(|_| PageError::Corrupt),
            None => Ok(Vec::new()),
        }
    }

    /// the aggregate thread+comment work `preflight_subtree_removal` (store.rs)
    /// would charge THIS target's owning block: one unit per thread plus one
    /// per comment across every thread anchored here. `AddComment` caps this
    /// at [`MAX_COMMENT_WORK_PER_TARGET`] before staging a new thread or reply,
    /// so the removal budget can never be exhausted by comments alone.
    async fn comment_work_for_target(&self, target: &str) -> Result<usize, PageError> {
        let thread_ids = self.load_target_index(target).await?;
        let mut work = thread_ids.len();
        for thread_id in &thread_ids {
            if let Some(thread) = self.load_thread(thread_id).await? {
                work += thread.comment_ids.len();
            }
        }
        Ok(work)
    }

    fn stage_target_index(&mut self, target: &str, ids: &[String]) -> Result<(), PageError> {
        if ids.is_empty() {
            self.delete_block(&target_index_key(target));
            Ok(())
        } else {
            self.stage(
                &target_index_key(target),
                serde_json::to_vec(ids).expect("ids serializable"),
            )
        }
    }

    /// a thread plus its LIVE (non-tombstoned) comments in order. `None` when
    /// the thread is absent; a listed comment missing from the store is
    /// corruption, surfaced loudly.
    pub(super) async fn thread_view(
        &self,
        thread_id: &str,
    ) -> Result<Option<ThreadView>, PageError> {
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

    /// remove every comment thread (its comments + the target index) anchored
    /// to `target` — called when the target block/page is deleted so comment
    /// records never dangle in the reserved keyspace with no reachable target.
    ///
    /// deliberately NOT author-gated on its own, and that is the module's
    /// rule rather than an omission: this is an IMPLICIT mutation, a
    /// consequence of removing the block, and it rides that `RemoveBlock`
    /// op's OWN [`Pages::may_edit`] check, already passed by the time this
    /// runs — a per-comment check on top would only make a block undeletable
    /// once anyone else commented on it, while adding no authority the module
    /// does not already have. What bounds the purge is aim, not permission: it
    /// reaches
    /// exactly the threads anchored to the subtree being removed, which is why
    /// [`Pages::apply_comment_op`]'s `MoveCommentThread` must stay
    /// opener-gated — that op is the only way to aim it at a thread that was
    /// never on your block. Same rule as [`Self::rebase_comment_anchors`].
    pub(super) async fn purge_comments_for_target(
        &mut self,
        target: &str,
    ) -> Result<(), PageError> {
        let thread_ids = self.load_target_index(target).await?;
        if thread_ids.is_empty() {
            return Ok(());
        }
        for tid in &thread_ids {
            if let Some(thread) = self.load_thread(tid).await? {
                for cid in &thread.comment_ids {
                    self.delete_block(&comment_key(cid));
                }
            }
            self.delete_block(&thread_key(tid));
        }
        self.delete_block(&target_index_key(target));
        Ok(())
    }

    /// Keep selection anchors attached while a block's text shifts. Implicit
    /// like the purge above, so ungated for the same reason: it is a
    /// consequence of an edit to the block and rides that block op's
    /// authority. This is linear in threads on one target (hard-capped at
    /// 1024); shard the target index only if real documents make that hotspot
    /// measurable.
    pub(super) async fn rebase_comment_anchors(
        &mut self,
        target: &str,
        edit: TextEdit,
        new_len: u32,
    ) -> Result<(), PageError> {
        for thread_id in self.load_target_index(target).await? {
            let mut thread = self
                .load_thread(&thread_id)
                .await?
                .ok_or(PageError::Corrupt)?;
            if let Some(anchor) = &mut thread.anchor {
                rebase_anchor(anchor, edit, new_len);
                self.store_thread(&thread)?;
            }
        }
        Ok(())
    }

    // ── comments ──

    pub(super) async fn apply_comment_op(
        &mut self,
        msg: PageMsg,
        origin: &Origin,
        now: u64,
    ) -> Result<(), PageError> {
        match msg {
            PageMsg::AddComment {
                thread_id,
                comment_id,
                target,
                text,
                anchor,
                mentions: _,
                as_agent,
            } => {
                // bound the client-minted ids BEFORE staging: they drive the
                // size of the shared derived blocks (the target index and the
                // thread record). the length cap alone is NOT enough — an id
                // of escaping chars serializes to 2–6 B each, so it must ALSO
                // be index-safe for `len()` to bound the serialized cost and
                // the count × length margins to hold.
                if thread_id.len() > MAX_THREAD_ID_BYTES
                    || comment_id.len() > MAX_COMMENT_ID_BYTES
                    || target.len() > MAX_COMMENT_TARGET_BYTES
                    || !id_is_index_safe(&thread_id)
                    || !id_is_index_safe(&comment_id)
                    || !id_is_index_safe(&target)
                {
                    return Err(PageError::IdTooLarge);
                }
                if text.len() > MAX_COMMENT_TEXT_BYTES {
                    return Err(PageError::TextTooLarge);
                }
                // `as_agent` refines a MODULE origin into an individual agent
                // author (chat's refine pattern): modules are genesis-trusted
                // code, so the module half stays origin-derived and
                // spoof-proof; an external or system submitter claiming an
                // agent identity is rejected outright.
                let author = match as_agent {
                    None => author_from_origin(origin)?,
                    Some(agent_id) => {
                        if agent_id.is_empty() {
                            return Err(PageError::EmptyAgent);
                        }
                        if agent_id.len() > MAX_COMMENT_AGENT_ID_BYTES {
                            return Err(PageError::AgentIdTooLarge);
                        }
                        match author_from_origin(origin)? {
                            AuthorRef::Module(module) => AuthorRef::Agent { module, agent_id },
                            _ => return Err(PageError::AgentNeedsModuleOrigin),
                        }
                    }
                };
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
                        if self.comment_work_for_target(&target).await? + 1
                            > MAX_COMMENT_WORK_PER_TARGET
                        {
                            return Err(PageError::TooMuchCommentWork);
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
                        if let Some(anchor) = &anchor {
                            let block = self
                                .load_block(&target)
                                .await
                                .map_err(|_| PageError::Corrupt)?
                                .ok_or(PageError::BlockNotFound)?;
                            if !valid_range(&block.text, anchor.start, anchor.end) {
                                return Err(PageError::InvalidTextRange);
                            }
                        }
                        let mut ids = self.load_target_index(&target).await?;
                        if ids.len() >= MAX_THREADS_PER_TARGET {
                            return Err(PageError::TooManyThreads);
                        }
                        if self.comment_work_for_target(&target).await? + 2
                            > MAX_COMMENT_WORK_PER_TARGET
                        {
                            return Err(PageError::TooMuchCommentWork);
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
                            anchor,
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
            PageMsg::MoveCommentThread {
                thread_id,
                target,
                anchor,
            } => {
                // WHO first, then WHAT: an explicit re-home rewrites the
                // anchor its OPENER placed, so it carries the same
                // stored-author rule as `EditComment`/`DeleteComment` — and
                // `author_from_origin` refuses the empty (pre-consensus)
                // origin here exactly as it does on its four siblings.
                // Ungated, this was also the aiming device for the comment
                // purge: re-home a stranger's thread onto a throwaway block,
                // `RemoveBlock` it, and their comments are hard-deleted past
                // the very author check `DeleteComment` enforces.
                let author = author_from_origin(origin)?;
                let mut thread = self
                    .load_thread(&thread_id)
                    .await?
                    .ok_or(PageError::ThreadNotFound)?;
                if thread.opener != author {
                    return Err(PageError::NotAuthor);
                }
                if target.len() > MAX_COMMENT_TARGET_BYTES || !id_is_index_safe(&target) {
                    return Err(PageError::IdTooLarge);
                }
                let block = self
                    .load_block(&target)
                    .await
                    .map_err(|_| PageError::Corrupt)?
                    .ok_or(PageError::BlockNotFound)?;
                if let Some(anchor) = &anchor
                    && !valid_range(&block.text, anchor.start, anchor.end)
                {
                    return Err(PageError::InvalidTextRange);
                }
                if thread.target != target {
                    let mut next = self.load_target_index(&target).await?;
                    if !next.contains(&thread_id) && next.len() >= MAX_THREADS_PER_TARGET {
                        return Err(PageError::TooManyThreads);
                    }
                    let mut previous = self.load_target_index(&thread.target).await?;
                    previous.retain(|id| id != &thread_id);
                    self.stage_target_index(&thread.target, &previous)?;
                    if !next.contains(&thread_id) {
                        next.push(thread_id.clone());
                        next.sort();
                        self.stage_target_index(&target, &next)?;
                    }
                    thread.target = target;
                }
                thread.anchor = anchor;
                self.store_thread(&thread)
            }
            PageMsg::EditComment {
                comment_id, text, ..
            } => {
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
            PageMsg::ResolveThread {
                thread_id,
                resolved,
            } => {
                let author = author_from_origin(origin)?;
                let mut thread = self
                    .load_thread(&thread_id)
                    .await?
                    .ok_or(PageError::ThreadNotFound)?;
                thread.resolved = resolved;
                thread.resolved_by = if resolved { Some(author) } else { None };
                self.store_thread(&thread)
            }
            _ => unreachable!("non-comment op routed to apply_comment_op"),
        }
    }
}
