use super::{BufferPooler, Context, Origin, PageError, PageMsg, Pages};

impl<E> Pages<E>
where
    E: Context + BufferPooler,
{
    /// apply one decoded [`PageMsg`] to the staged overlay. pure tree surgery
    /// over per-block/-comment records, re-staged on success. errors abort the
    /// block. `origin`/`now` are only consulted by the comment ops (author +
    /// timestamp); block ops ignore them.
    pub(super) async fn apply(
        &mut self,
        msg: PageMsg,
        origin: &Origin,
        now: u64,
    ) -> Result<(), PageError> {
        // no client-minted id may live in the reserved (NUL-prefixed) keyspace:
        // the enumeration index (`\0page-index`) and every comment record/index
        // key lead with NUL, so rejecting NUL-prefixed ids here — BEFORE any
        // storage touch — keeps a block/comment write from ever clobbering them.
        let named: Vec<&str> = match &msg {
            PageMsg::CreatePage {
                page_id, parent, ..
            } => {
                let mut v = vec![page_id.as_str()];
                if let Some(p) = parent {
                    v.push(p.as_str());
                }
                v
            }
            PageMsg::InsertBlock { parent, block, .. } => vec![parent.as_str(), block.id.as_str()],
            PageMsg::UpdateText { block_id, .. }
            | PageMsg::SetSpanMark { block_id, .. }
            | PageMsg::SetKind { block_id, .. }
            | PageMsg::SetChecked { block_id, .. }
            | PageMsg::RemoveBlock { block_id } => vec![block_id.as_str()],
            PageMsg::MoveBlock {
                block_id, parent, ..
            } => vec![block_id.as_str(), parent.as_str()],
            PageMsg::SetPageParent { page_id, parent } => {
                let mut v = vec![page_id.as_str()];
                if let Some(p) = parent {
                    v.push(p.as_str());
                }
                v
            }
            PageMsg::DeletePage { page_id } => vec![page_id.as_str()],
            PageMsg::AddComment {
                thread_id,
                comment_id,
                target,
                ..
            } => {
                vec![thread_id.as_str(), comment_id.as_str(), target.as_str()]
            }
            PageMsg::MoveCommentThread {
                thread_id, target, ..
            } => vec![thread_id.as_str(), target.as_str()],
            PageMsg::EditComment { comment_id, .. } => vec![comment_id.as_str()],
            PageMsg::DeleteComment { comment_id } => vec![comment_id.as_str()],
            PageMsg::ResolveThread { thread_id, .. } => vec![thread_id.as_str()],
        };
        if named.iter().any(|id| id.starts_with('\u{0}')) {
            return Err(PageError::ReservedId);
        }

        if matches!(
            &msg,
            PageMsg::AddComment { .. }
                | PageMsg::MoveCommentThread { .. }
                | PageMsg::EditComment { .. }
                | PageMsg::DeleteComment { .. }
                | PageMsg::ResolveThread { .. }
        ) {
            return self.apply_comment_op(msg, origin, now).await;
        }
        if matches!(
            &msg,
            PageMsg::CreatePage { .. } | PageMsg::DeletePage { .. } | PageMsg::SetPageParent { .. }
        ) {
            return self.apply_page_op(msg).await;
        }
        self.apply_block_op(msg).await
    }
}
