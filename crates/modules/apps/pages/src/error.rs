use super::Error;

/// deterministic module failures. Operation errors abort the whole block (the
/// sdk `abort_block` contract); query errors leave state untouched.
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
    /// a page-block query cursor that is absent or outside the requested page.
    InvalidPageCursor,
    /// a page query exceeded its deterministic block-read budget before the
    /// wasm host's broader per-dispatch store-read ceiling.
    PageTraversalTooDeep,
    /// an insert or move would put a block below [`crate::MAX_PAGE_DEPTH`].
    PageTooDeep,
    /// a subtree-deepening move was too large to validate inside one wasm
    /// dispatch. Same-depth and shallower moves do not need this traversal.
    MoveSubtreeTooLarge,
    /// a page move's physical ancestry exceeded the local store-read budget.
    MoveAncestryTooDeep,
    /// a subtree removal exceeded the local traversal/work budget during preflight.
    RemoveSubtreeTooLarge,
    /// a move whose new parent sits inside the moved block's own subtree.
    CycleMove,
    /// a move whose new parent belongs to a different page.
    CrossPageMove,
    /// `SetKind` tried to convert to or from `Page`. Page membership changes
    /// only through insert/move/remove so the enumeration index stays exact.
    PageKindImmutable,
    /// a non-page block tried to move without a parent.
    TopLevelNonPage,
    /// `SetChecked` on a non-`Todo` block.
    NotTodo,
    /// an inline mark/comment anchor is empty, outside the target text, or
    /// splits a UTF-16 surrogate pair.
    InvalidTextRange,
    /// normalized inline formatting exceeded the per-block span cap.
    TooManySpanMarks,
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
    // ── comments ──
    /// a comment op arrived with an empty (pre-consensus) origin.
    EmptyOrigin,
    /// an external or module origin was too large for bounded comment replies.
    AuthorTooLarge,
    /// an AddComment carried an empty `as_agent` id.
    EmptyAgent,
    /// an AddComment carried an `as_agent` id too large for bounded comment
    /// query replies.
    AgentIdTooLarge,
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
    /// an AddComment thread_id/comment_id/target over its length cap —
    /// bounded so the derived index/thread blocks can never exceed
    /// [`MAX_BLOCK_LEN`] and abort a block.
    IdTooLarge,
    /// a thread already holds [`MAX_COMMENTS_PER_THREAD`] comments.
    TooManyComments,
    /// a target already holds [`MAX_THREADS_PER_TARGET`] threads.
    TooManyThreads,
}

impl core::fmt::Display for PageError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let s = match self {
            PageError::DuplicateBlock => "duplicate block id",
            PageError::BlockNotFound => "block not found",
            PageError::ParentNotFound => "parent block not found",
            PageError::AnchorNotFound => "after-anchor not found",
            PageError::InvalidPageCursor => "invalid page cursor",
            PageError::PageTraversalTooDeep => "page traversal too deep",
            PageError::PageTooDeep => "page nesting is too deep",
            PageError::MoveSubtreeTooLarge => "subtree is too large to move deeper",
            PageError::MoveAncestryTooDeep => "page ancestry is too deep to move",
            PageError::RemoveSubtreeTooLarge => "subtree is too large to remove",
            PageError::CycleMove => "move target is inside the moved subtree",
            PageError::CrossPageMove => "cross-page move",
            PageError::PageKindImmutable => "page blocks cannot be converted to another kind",
            PageError::TopLevelNonPage => "only page blocks may move to the top level",
            PageError::NotTodo => "checked applies only to todo blocks",
            PageError::InvalidTextRange => "invalid text range",
            PageError::TooManySpanMarks => "too many inline marks",
            PageError::BlockTooLarge => "block too large",
            PageError::Corrupt => "stored page state is corrupt",
            PageError::ReservedId => "reserved block id",
            PageError::EmptyOrigin => "empty origin",
            PageError::AuthorTooLarge => "comment author is too large",
            PageError::EmptyAgent => "empty as_agent",
            PageError::AgentIdTooLarge => "as_agent is too large",
            PageError::AgentNeedsModuleOrigin => "as_agent requires a module origin",
            PageError::ThreadNotFound => "thread not found",
            PageError::CommentNotFound => "comment not found",
            PageError::DuplicateComment => "duplicate comment id",
            PageError::TargetMismatch => "target mismatch",
            PageError::NotAuthor => "not the comment author",
            PageError::TextTooLarge => "comment text too large",
            PageError::IdTooLarge => "comment id or target too large",
            PageError::TooManyComments => "too many comments in thread",
            PageError::TooManyThreads => "too many threads on target",
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
