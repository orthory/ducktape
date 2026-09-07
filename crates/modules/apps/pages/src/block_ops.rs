use super::{
    Block, BlockKind, MAX_BLOCK_ID_BYTES, MAX_PAGE_DEPTH, PageError, PageMsg, Pages,
    id_is_index_safe, to_page_err,
};
use crate::text_ranges::{edit_between, rebase_marks, set_span_mark, utf16_len, validate_marks};

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

impl Pages {
    pub(super) async fn apply_block_op(
        &mut self,
        msg: PageMsg,
        authority: &super::Authority,
    ) -> Result<(), PageError> {
        match msg {
            PageMsg::InsertBlock {
                parent,
                after,
                block,
            } => {
                if block.id.len() > MAX_BLOCK_ID_BYTES || !id_is_index_safe(&block.id) {
                    return Err(PageError::IdTooLarge);
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
                let marks = validate_marks(&block.text, block.marks)?;
                let mut parent_blk = self
                    .require_block(&parent, PageError::ParentNotFound)
                    .await?;
                if !self.may_edit(&parent_blk.page, authority).await? {
                    return Err(PageError::NotPageAuthor);
                }
                let i = idx_after(&parent_blk.children, &after)?;
                let parent_depth = self.page_depth(&parent_blk).await?;
                if parent_depth >= MAX_PAGE_DEPTH {
                    return Err(PageError::PageTooDeep);
                }
                parent_blk.children.insert(i, block.id.clone());
                let creates_page = block.kind == BlockKind::Page;
                let page = if creates_page {
                    block.id.clone()
                } else {
                    parent_blk.page.clone()
                };
                if creates_page {
                    self.index_add(&block.id, Some(parent_blk.page.clone()))
                        .await?;
                }
                self.store_block(&Block {
                    author: authority.actor.clone(),
                    id: block.id,
                    parent: Some(parent_blk.id.clone()),
                    page,
                    kind: block.kind,
                    text: block.text,
                    marks,
                    checked: false,
                    children: Vec::new(),
                })?;
                self.store_block(&parent_blk)
            }
            PageMsg::UpdateText {
                block_id,
                text,
                marks,
            } => {
                // works on any block INCLUDING a Page — that is the rename
                // path (the title is the Page block's text).
                let mut blk = self
                    .require_block(&block_id, PageError::BlockNotFound)
                    .await?;
                if !self.may_edit(&blk.page, authority).await? {
                    return Err(PageError::NotPageAuthor);
                }
                // Validate the client-supplied atomic replacement before
                // staging any rebased comment records.
                let marks = marks
                    .map(|marks| validate_marks(&text, marks))
                    .transpose()?;
                if let Some(edit) = edit_between(&blk.text, &text) {
                    if marks.is_none() {
                        rebase_marks(&mut blk.marks, edit, utf16_len(&text));
                    }
                    self.rebase_comment_anchors(&block_id, edit, utf16_len(&text))
                        .await?;
                }
                if let Some(marks) = marks {
                    blk.marks = marks;
                }
                blk.text = text;
                self.store_block(&blk)
            }
            PageMsg::SetSpanMark {
                block_id,
                start,
                end,
                kind,
                active,
            } => {
                let mut blk = self
                    .require_block(&block_id, PageError::BlockNotFound)
                    .await?;
                if !self.may_edit(&blk.page, authority).await? {
                    return Err(PageError::NotPageAuthor);
                }
                set_span_mark(&mut blk.marks, &blk.text, start, end, kind, active)?;
                self.store_block(&blk)
            }
            PageMsg::SetKind { block_id, kind } => {
                if kind == BlockKind::Page {
                    return Err(PageError::PageKindImmutable);
                }
                let mut blk = self
                    .require_block(&block_id, PageError::BlockNotFound)
                    .await?;
                if blk.kind == BlockKind::Page {
                    return Err(PageError::PageKindImmutable);
                }
                if !self.may_edit(&blk.page, authority).await? {
                    return Err(PageError::NotPageAuthor);
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
                if !self.may_edit(&blk.page, authority).await? {
                    return Err(PageError::NotPageAuthor);
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
                if !self.may_edit(&blk.page, authority).await? {
                    return Err(PageError::NotPageAuthor);
                }
                let moves_page = blk.kind == BlockKind::Page;
                let old_parent_id = blk.parent.clone();
                match parent {
                    None => {
                        if !moves_page {
                            return Err(PageError::TopLevelNonPage);
                        }
                        if after.is_some() {
                            return Err(PageError::AnchorNotFound);
                        }
                        let Some(old_parent_id) = old_parent_id else {
                            return Ok(());
                        };
                        let mut old_parent = self
                            .require_block(&old_parent_id, PageError::Corrupt)
                            .await?;
                        let position = old_parent
                            .children
                            .iter()
                            .position(|child| child == &block_id)
                            .ok_or(PageError::Corrupt)?;
                        old_parent.children.remove(position);
                        blk.parent = None;
                        self.index_set_parent(&block_id, None).await?;
                        self.store_block(&old_parent)?;
                        self.store_block(&blk)
                    }
                    Some(parent_id) => {
                        let new_parent = self
                            .require_block(&parent_id, PageError::ParentNotFound)
                            .await?;
                        if !moves_page && new_parent.page != blk.page {
                            return Err(PageError::CrossPageMove);
                        }
                        // grafting under `new_parent` mutates ITS page's tree
                        // (its children list), so a page block moving under a
                        // different page needs that page's authority too —
                        // same-page moves recheck the source's own page.
                        if !self.may_edit(&new_parent.page, authority).await? {
                            return Err(PageError::NotPageAuthor);
                        }
                        let new_parent_depth = if moves_page {
                            self.ancestry_excludes(&parent_id, &block_id).await?;
                            self.page_depth(&new_parent).await?
                        } else {
                            self.page_depth_excluding(&new_parent, Some(&block_id))
                                .await?
                        };
                        let new_depth = new_parent_depth + 1;
                        if new_depth > MAX_PAGE_DEPTH {
                            return Err(PageError::PageTooDeep);
                        }
                        if !moves_page {
                            let old_depth = self.page_depth(&blk).await?;
                            let deepens_subtree = new_depth > old_depth;
                            if deepens_subtree {
                                self.ensure_subtree_fits(&blk, MAX_PAGE_DEPTH - new_depth)
                                    .await?;
                            }
                        }
                        if old_parent_id.as_deref() == Some(parent_id.as_str()) {
                            let mut parent = new_parent;
                            let position = parent
                                .children
                                .iter()
                                .position(|child| child == &block_id)
                                .ok_or(PageError::Corrupt)?;
                            parent.children.remove(position);
                            let index = idx_after(&parent.children, &after)?;
                            parent.children.insert(index, block_id);
                            return self.store_block(&parent);
                        }
                        if !moves_page && old_parent_id.is_none() {
                            return Err(PageError::Corrupt);
                        }
                        if let Some(old_parent_id) = old_parent_id {
                            let mut old_parent = self
                                .require_block(&old_parent_id, PageError::Corrupt)
                                .await?;
                            let position = old_parent
                                .children
                                .iter()
                                .position(|child| child == &block_id)
                                .ok_or(PageError::Corrupt)?;
                            old_parent.children.remove(position);
                            self.store_block(&old_parent)?;
                        }
                        let containing_page = new_parent.page.clone();
                        let mut new_parent = new_parent;
                        let index = idx_after(&new_parent.children, &after)?;
                        new_parent.children.insert(index, block_id.clone());
                        blk.parent = Some(parent_id);
                        if moves_page {
                            self.index_set_parent(&block_id, Some(containing_page))
                                .await?;
                        }
                        self.store_block(&new_parent)?;
                        self.store_block(&blk)
                    }
                }
            }
            PageMsg::RemoveBlock { block_id } => {
                let blk = self
                    .require_block(&block_id, PageError::BlockNotFound)
                    .await?;
                if !self.may_edit(&blk.page, authority).await? {
                    return Err(PageError::NotPageAuthor);
                }
                let invalid_top_level = blk.parent.is_none() && blk.kind != BlockKind::Page;
                if invalid_top_level {
                    return Err(PageError::Corrupt);
                }
                let removal = self.preflight_subtree_removal(blk.clone()).await?;
                if let Some(parent_id) = &blk.parent {
                    let mut parent = self.require_block(parent_id, PageError::Corrupt).await?;
                    // a nested subpage's parent can belong to a DIFFERENT
                    // page than the subpage itself; removing it also
                    // mutates that page's children list, so it needs that
                    // page's authority too.
                    if parent.page != blk.page && !self.may_edit(&parent.page, authority).await? {
                        return Err(PageError::NotPageAuthor);
                    }
                    let position = parent
                        .children
                        .iter()
                        .position(|child| child == &block_id)
                        .ok_or(PageError::Corrupt)?;
                    parent.children.remove(position);
                    self.store_block(&parent)?;
                }
                self.delete_subtree(removal).await
            }
            _ => unreachable!("non-block op routed to apply_block_op"),
        }
    }
}
