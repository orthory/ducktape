use super::{PageError, PageMsg, Pages};

impl Pages {
    /// apply one decoded [`PageMsg`] to the staged overlay. pure tree surgery
    /// over per-block/-comment records. The caller restores its incoming staging
    /// on error. `actor` is already resolved: the comment ops gate on
    /// stored comment/thread authorship, `CreatePage` records the creating
    /// party as the page's author, and every other page/block op is gated by
    /// [`Pages::may_edit`] against that recorded author. `now` is consulted
    /// only by the comment ops (their stored timestamps).
    pub(super) async fn apply(
        &mut self,
        msg: PageMsg,
        authority: &super::Authority,
        now: u64,
    ) -> Result<(), PageError> {
        // no client-minted id may live in the reserved (NUL-prefixed) keyspace:
        // the enumeration index (`\0page-index`) and every comment record/index
        // key lead with NUL, so rejecting NUL-prefixed ids here — BEFORE any
        // storage touch — keeps a block/comment write from ever clobbering them.
        let named: Vec<&str> = match &msg {
            PageMsg::CreatePage { page_id, .. } => vec![page_id.as_str()],
            PageMsg::InsertBlock { parent, block, .. } => vec![parent.as_str(), block.id.as_str()],
            PageMsg::UpdateText { block_id, .. }
            | PageMsg::SetSpanMark { block_id, .. }
            | PageMsg::SetKind { block_id, .. }
            | PageMsg::SetChecked { block_id, .. }
            | PageMsg::RemoveBlock { block_id } => vec![block_id.as_str()],
            PageMsg::MoveBlock {
                block_id, parent, ..
            } => {
                let mut ids = vec![block_id.as_str()];
                if let Some(parent) = parent {
                    ids.push(parent.as_str());
                }
                ids
            }
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
            return self.apply_comment_op(msg, authority, now).await;
        }
        if matches!(&msg, PageMsg::CreatePage { .. }) {
            return self.apply_page_op(msg, authority).await;
        }
        self.apply_block_op(msg, authority).await
    }
}
