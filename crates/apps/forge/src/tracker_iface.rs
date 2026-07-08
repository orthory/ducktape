//! the tracker's public wire surface — issue / pull-request / review types.
//!
//! forge's tracker is GitHub-shaped: issues and PRs share ONE per-repo number
//! space (so `#42` is unambiguous), a PR's source is a real branch in the SAME
//! repo (no forks), and a review is the batched GitHub flow — one verdict, an
//! optional body, and zero or more line-anchored diff comments submitted
//! together. every item owns a hidden chat channel (`forge:<repo>:<number>`)
//! that carries its free-form discussion; the item's BODY lives HERE, on the
//! record, so authorship stays origin-derived (a chat follow-up would be
//! attributed to the forge module, not the opening user).
//!
//! authorship reuses chat's [`AuthorRef`] so the app renders forge authors
//! through the exact same display-name path as chat messages.

use chat::AuthorRef;
use serde::{Deserialize, Serialize};

// ---- write-time caps (consensus constants) ---------------------------------
// enforced deterministically BEFORE staging, so an oversized op rejects
// identically on every validator. shared here so clients can pre-validate.

/// item / review titles.
pub const MAX_TITLE_BYTES: usize = 256;
/// item bodies and review bodies (markdown).
pub const MAX_BODY_BYTES: usize = 64 * 1024;
/// a branch short name ("feature/x") — also a consensus-visible key.
pub const MAX_BRANCH_BYTES: usize = 128;
/// line comments per submitted review.
pub const MAX_REVIEW_COMMENTS: usize = 64;
/// one line comment's body.
pub const MAX_REVIEW_COMMENT_BYTES: usize = 16 * 1024;
/// a diff comment's file path.
pub const MAX_PATH_BYTES: usize = 512;
/// ref updates in one atomic push op.
pub const MAX_REFS_PER_PUSH: usize = 32;
/// reviews per PR; further submissions are rejected.
pub const MAX_REVIEWS_PER_ITEM: usize = 256;

/// an item's lifecycle state. `Merged` is PR-only and terminal.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ItemState {
    Open,
    Closed,
    Merged,
}

/// what an item IS — issues and PRs share the number space, so listings carry
/// the kind explicitly.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Issue,
    Pr,
}

/// the reviewer's overall verdict — GitHub's three review outcomes. approvals
/// are ADVISORY: they render in the merge box but never gate `MergePr` (branch
/// protection is future work).
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewVerdict {
    Approve,
    RequestChanges,
    Comment,
}

/// which side of the diff a line comment anchors to.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiffSide {
    /// the deletion (base) side.
    Old,
    /// the addition (head) side.
    New,
}

/// one line-anchored diff comment inside a review. anchors are (path, line,
/// side) against the diff at the review's `commit_oid` — if the branch moves
/// past that commit the comment renders as "outdated" (no position tracking
/// across force-pushes; early-GitHub semantics).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ReviewComment {
    pub path: String,
    pub line: u32,
    pub side: DiffSide,
    pub body: String,
}

/// one submitted review, immutable once staged.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ReviewView {
    pub author: AuthorRef,
    pub verdict: ReviewVerdict,
    pub body: String,
    /// the PR source head hex (40-char sha1) the review was made against —
    /// the outdated-detection anchor.
    pub commit_oid: String,
    pub comments: Vec<ReviewComment>,
    pub created_at: u64,
}

/// one item row in a listing.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ItemSummary {
    pub number: u64,
    pub kind: ItemKind,
    pub title: String,
    pub state: ItemState,
    pub author: AuthorRef,
    pub created_at: u64,
    pub updated_at: u64,
}

/// the full item: the summary row plus body, discussion channel, and — for a
/// PR — branches, merge oid, and reviews.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ItemDetail {
    #[serde(flatten)]
    pub summary: ItemSummary,
    pub body: String,
    /// the item's hidden discussion channel id (`forge:<repo>:<number>`).
    pub channel_id: String,
    /// PR-only: the source branch short name.
    pub source_branch: Option<String>,
    /// PR-only: the target branch short name (normally "main").
    pub target_branch: Option<String>,
    /// PR-only: the merge commit hex once merged.
    pub merge_oid: Option<String>,
    pub reviews: Vec<ReviewView>,
}

/// one born branch in a [`crate::ForgeReply::Refs`] listing.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RefHead {
    /// the branch SHORT name ("main", "feature/x").
    pub name: String,
    /// the branch head oid as 40-char sha1 hex.
    pub head: String,
}

/// one ref command inside an atomic [`crate::ForgeMsg::PushRefs`]: a per-ref
/// compare-and-swap. `new_oid: None` deletes the branch (never "main");
/// `prev_oid: None` requires the branch to be unborn. raw 20-byte sha1 oids,
/// like [`crate::ForgeMsg::Push`].
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RefUpdate {
    /// the branch SHORT name ("main", "feature/x") — never a full refname.
    pub ref_name: String,
    pub prev_oid: Option<Vec<u8>>,
    pub new_oid: Option<Vec<u8>>,
}

/// the hidden discussion channel id every item owns. the `:` separator is the
/// chat module's reserved-namespace marker (external users cannot create such
/// ids; a module may only create ids under its own `"{module}:"` prefix), so
/// forge can rely on the id being unsquattable.
pub fn channel_id_for(repo: &str, number: u64) -> String {
    format!("forge:{repo}:{number}")
}
