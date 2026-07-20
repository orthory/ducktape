//! the node transport seam — one small SYNCHRONOUS trait every engine operation
//! flows through.
//!
//! sync on purpose: phase-4 FUSE is callback-driven (sync), so a colocated-odb
//! fast path can implement this same trait with no async plumbing. phase 3 ships
//! exactly one implementation, `HttpNode` (reqwest blocking) over the noded http
//! surface; the tests drive a module-backed mock over the same trait. reads are
//! snapshot-addressable; writes are staging + one atomic commit.

use duckfs_core::{Change, DiffEntry, DigestHex, EntryInfo, RefsInfo, SnapshotInfo};
use serde::{Deserialize, Serialize};

/// the block a commit landed in. the engine resolves the new snapshot id by
/// matching this height against `history`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitReceipt {
    pub height: u64,
}

/// a structured, no-silent-merge conflict outcome. auto-rebase covers disjoint
/// upstream work only; anything overlapping surfaces here for the caller to
/// resolve. `clashing` is the intersection the rebase refused; `remedy` carries
/// human advice for the cases rebase cannot fix (a GC'd base → re-checkout).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictReport {
    pub base: Option<String>,
    pub head: Option<String>,
    pub ours: Vec<String>,
    pub theirs: Vec<String>,
    pub clashing: Vec<String>,
    pub remedy: String,
}

/// a node-side failure. the engine's conflict taxonomy keys on the exact module
/// string inside `Rejected` (the `"files: conflict:"` / `"files: base snapshot
/// not resolvable"` / `"files: chunk not available"` contracts), so it must pass
/// through verbatim — never reworded.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApiError {
    /// a module rejection — the verbatim `"files: ..."` string.
    #[error("{0}")]
    Rejected(String),
    /// a 404 (absent path / unresolvable snapshot over http).
    #[error("not found")]
    NotFound,
    /// a transport-layer failure (connection, decode, non-error non-2xx).
    #[error("transport: {0}")]
    Transport(String),
}

/// every node interaction the engine needs. all reads take an optional snapshot
/// (`None` = committed head) and are paged where the module pages. one commit is
/// atomic; staging is one chunk per call (one block).
pub trait NodeApi {
    /// the committed refs summary (`head`, pins, window length).
    fn refs(&self) -> Result<RefsInfo, ApiError>;

    /// the entry at `path`, or `None` when nothing is there.
    fn stat(&self, path: &str, snapshot: Option<&str>) -> Result<Option<EntryInfo>, ApiError>;

    /// one page of a directory listing plus the `next` cursor.
    fn ls(
        &self,
        path: &str,
        snapshot: Option<&str>,
        after: Option<&str>,
        limit: u64,
    ) -> Result<(Vec<EntryInfo>, Option<String>), ApiError>;

    /// one page of a raw string-prefix subtree walk plus the `next` cursor.
    fn find(
        &self,
        prefix: &str,
        snapshot: Option<&str>,
        after: Option<&str>,
        limit: u64,
    ) -> Result<(Vec<EntryInfo>, Option<String>), ApiError>;

    /// a byte range of a file (or a symlink's target); returns `(bytes, eof)`.
    fn read(
        &self,
        path: &str,
        snapshot: Option<&str>,
        offset: u64,
        len: u64,
    ) -> Result<(Vec<u8>, bool), ApiError>;

    /// the bounded commit window, newest-first.
    fn history(&self, limit: u64) -> Result<Vec<SnapshotInfo>, ApiError>;

    /// the Added/Removed/Modified leaves between two committed snapshots.
    fn diff(&self, from: &str, to: &str, prefix: &str) -> Result<Vec<DiffEntry>, ApiError>;

    /// the staging probe: which of these chunk ids the cluster already holds
    /// (advisory — the commit re-validates). reply order matches request order.
    fn has_chunks(&self, ids: &[String]) -> Result<Vec<bool>, ApiError>;

    /// stage one raw chunk (≤ 1 MiB); returns its digest. one block per call.
    fn stage_chunk(&self, bytes: &[u8]) -> Result<DigestHex, ApiError>;

    /// one atomic commit with per-path CAS against `base` (`None` = empty tree).
    fn commit(
        &self,
        base: Option<&str>,
        message: &str,
        changes: Vec<Change>,
    ) -> Result<CommitReceipt, ApiError>;

    /// pin a snapshot by name so gc keeps it reachable.
    fn pin(&self, snapshot: &str, name: &str) -> Result<(), ApiError>;
}
