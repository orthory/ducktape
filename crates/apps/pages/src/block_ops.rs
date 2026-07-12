use super::{Block, BlockKind, PageError, PageMsg, Pages, to_page_err};

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
    pub(super) async fn apply_block_op(&mut self, msg: PageMsg) -> Result<(), PageError> {
        match msg {
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
                // delete the WHOLE subtree, depth-first. no page root can
                // live below a non-root block (block ops can't mint kind
                // Page), so the index never needs updating.
                self.delete_subtree(blk).await
            }
            _ => unreachable!("non-block op routed to apply_block_op"),
        }
    }
}
