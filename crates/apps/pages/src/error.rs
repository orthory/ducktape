use super::Error;

/// per-op failures. mapped to [`Error::Module`] so any error aborts the whole
/// block (the sdk `abort_block` contract), rolling back the staged overlay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PageError {
    /// insert/create of a block id already present ANYWHERE in the module —
    /// block ids are globally unique, that is the addressability contract.
    DuplicateBlock,
    /// update/move/remove/check of a block id not in the store.
    BlockNotFound,
    /// an insert/move named a parent block that does not exist.
    ParentNotFound,
    /// an `after` anchor that is not a child of the named parent.
    AnchorNotFound,
    /// a move whose new parent sits inside the moved block's own subtree.
    CycleMove,
    /// a move whose new parent belongs to a different page.
    CrossPageMove,
    /// move/remove/convert targeted a page root — roots are managed solely by
    /// `CreatePage` (and renames via `UpdateText`).
    PageRootImmutable,
    /// a block op tried to insert or convert to kind `Page` — pages come only
    /// from `CreatePage`, which is what keeps the enumeration index exact.
    PageViaBlockOp,
    /// `SetChecked` on a non-`Todo` block.
    NotTodo,
    /// the op would grow a serialized block (or the index) past
    /// [`MAX_BLOCK_LEN`] — rejected at write time so the oversized bytes never
    /// reach the panicking commit/read paths (the codec bound is decode-only).
    BlockTooLarge,
    /// stored state failed to decode or a tree invariant is broken (a listed
    /// child missing, a parent chain looping). distinct from absence:
    /// corruption must surface loudly, never masquerade as "not found".
    Corrupt,
    /// an op named the reserved [`PAGE_INDEX_KEY`] sentinel.
    ReservedId,
    /// a create/set-parent named a `parent` that is not an existing page root.
    ParentPageNotFound,
    /// set-parent/delete targeted an id that is not an existing page root.
    NotAPage,
    /// a set-parent would nest a page inside its own folder subtree.
    PageCycle,
    // ── comments ──
    /// a comment op arrived with an empty (pre-consensus) origin.
    EmptyOrigin,
    /// an AddComment carried an empty `as_agent` id.
    EmptyAgent,
    /// an AddComment carried `as_agent` under a non-module origin — only
    /// genesis-trusted module code may attribute a comment to an agent.
    AgentNeedsModuleOrigin,
    /// resolve/append named a thread id not in the store.
    ThreadNotFound,
    /// edit/delete named a comment id not in the store (or a tombstone).
    CommentNotFound,
    /// AddComment reused a comment id already present.
    DuplicateComment,
    /// an append named a target that differs from the thread's.
    TargetMismatch,
    /// edit/delete by someone other than the stored author.
    NotAuthor,
    /// comment text over [`MAX_COMMENT_TEXT_BYTES`].
    TextTooLarge,
    /// a thread already holds [`MAX_COMMENTS_PER_THREAD`] comments.
    TooManyComments,
    /// a target already holds [`MAX_THREADS_PER_TARGET`] threads.
    TooManyThreads,
    /// a ThreadsForTargets query named more than [`MAX_QUERY_TARGETS`] targets.
    TooManyTargets,
}

impl core::fmt::Display for PageError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let s = match self {
            PageError::DuplicateBlock => "duplicate block id",
            PageError::BlockNotFound => "block not found",
            PageError::ParentNotFound => "parent block not found",
            PageError::AnchorNotFound => "after-anchor not found",
            PageError::CycleMove => "move target is inside the moved subtree",
            PageError::CrossPageMove => "cross-page move",
            PageError::PageRootImmutable => "page roots cannot be moved, removed, or converted",
            PageError::PageViaBlockOp => "a page block can only be created by CreatePage",
            PageError::NotTodo => "checked applies only to todo blocks",
            PageError::BlockTooLarge => "block too large",
            PageError::Corrupt => "stored page state is corrupt",
            PageError::ReservedId => "reserved block id",
            PageError::ParentPageNotFound => "parent page not found",
            PageError::NotAPage => "not a page",
            PageError::PageCycle => "page cycle",
            PageError::EmptyOrigin => "empty origin",
            PageError::EmptyAgent => "empty as_agent",
            PageError::AgentNeedsModuleOrigin => "as_agent requires a module origin",
            PageError::ThreadNotFound => "thread not found",
            PageError::CommentNotFound => "comment not found",
            PageError::DuplicateComment => "duplicate comment id",
            PageError::TargetMismatch => "target mismatch",
            PageError::NotAuthor => "not the comment author",
            PageError::TextTooLarge => "comment text too large",
            PageError::TooManyComments => "too many comments in thread",
            PageError::TooManyThreads => "too many threads on target",
            PageError::TooManyTargets => "too many query targets",
        };
        f.write_str(s)
    }
}

/// bridge the only sdk error `load_block` can raise — a stored-block json
/// decode failure — back into `PageError` so `apply` stays single-error-typed.
/// if it ever fires it MUST surface as corruption, not absence: mapping a
/// decode failure to "not found" would let `CreatePage` silently re-seed a
/// root over the corrupt bytes, destroying the evidence AND the data.
pub(super) fn to_page_err(_e: Error) -> PageError {
    PageError::Corrupt
}
