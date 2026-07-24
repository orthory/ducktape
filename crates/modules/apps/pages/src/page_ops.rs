use super::{Block, BlockKind, PageError, PageMsg, Pages, to_page_err};

impl Pages {
    pub(super) async fn apply_page_op(&mut self, msg: PageMsg) -> Result<(), PageError> {
        match msg {
            PageMsg::CreatePage { page_id, title } => {
                match self.load_block(&page_id).await.map_err(to_page_err)? {
                    // idempotent: re-creating an existing page is a benign
                    // no-op that does NOT clobber the live title or position.
                    Some(b) if b.kind == BlockKind::Page => Ok(()),
                    // the id is already a NON-page block somewhere — page ids
                    // are block ids, so this is a global-uniqueness violation.
                    Some(_) => Err(PageError::DuplicateBlock),
                    None => {
                        self.index_add(&page_id, None).await?;
                        self.store_block(&Block {
                            id: page_id.clone(),
                            parent: None,
                            page: page_id,
                            kind: BlockKind::Page,
                            text: title,
                            marks: Vec::new(),
                            checked: false,
                            children: Vec::new(),
                        })
                    }
                }
            }
            _ => unreachable!("non-page op routed to apply_page_op"),
        }
    }
}
