use super::{Block, BlockKind, BufferPooler, Context, PageError, PageMsg, Pages, to_page_err};

impl<E> Pages<E>
where
    E: Context + BufferPooler,
{
    pub(super) async fn apply_page_op(&mut self, msg: PageMsg) -> Result<(), PageError> {
        match msg {
            PageMsg::CreatePage {
                page_id,
                title,
                parent,
            } => {
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
                self.delete_subtree(root).await
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
            _ => unreachable!("non-page op routed to apply_page_op"),
        }
    }
}
