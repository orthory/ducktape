//! the versioned `.duckfs/index.json` — git's index discipline for duckfs.
//!
//! a checkout writes this beside the materialized tree; status/commit read it as
//! the base state (the snapshot the working copy descends from, plus a per-path
//! hash/size/mtime record). it is a private, OS-side artifact — never consensus
//! state — so the format is a plain versioned json document, saved atomically
//! (`tmp` → rename) so a crash mid-write never leaves a half-written index.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// the on-disk index format version. bumped only on a breaking layout change; a
/// client refuses any other version with a re-checkout remedy rather than
/// misread a schema it was not built for.
pub const INDEX_VERSION: u32 = 1;

/// the per-checkout state directory, at the root of the materialized tree.
pub const DUCKFS_DIR: &str = ".duckfs";
/// the index file inside [`DUCKFS_DIR`].
pub const INDEX_FILE: &str = "index.json";

/// what a recorded entry is. files and symlinks carry content status must track;
/// a symlink's `object` is the file id over its target bytes, exactly as the
/// module stores it. only EMPTY directories are recorded (`Dir`, object empty):
/// non-empty dirs are implied by their entries and rematerialized structurally,
/// but an empty dir has nothing to imply it, so status tracks it explicitly to
/// tell a fresh empty dir (needs a `Mkdir`) from one already in the base snapshot
/// (`mkdir` on an existing dir rejects, so re-emitting it would break the commit).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    File,
    Symlink,
    Dir,
}

/// one recorded path: the committed file object id, plus the size/mtime/exec the
/// fast path compares against and the meta needed to recompute the file id on a
/// rehash (meta is part of the file-id preimage — see [`crate::chunk`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexEntry {
    /// the file object id (hex) as of the base snapshot.
    pub object: String,
    pub size: u64,
    /// mtime split into whole seconds and the sub-second nanos — coarse-mtime
    /// filesystems only fill `mtime_secs`, which the racy-clean rule accounts for.
    pub mtime_secs: i64,
    pub mtime_nanos: u32,
    pub exec: bool,
    pub kind: EntryKind,
    pub meta: BTreeMap<String, String>,
}

/// the whole index: the base snapshot the working copy descends from, the duckfs
/// prefix it was checked out under, the node url it talks to, and every recorded
/// path keyed by its absolute duckfs path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Index {
    pub version: u32,
    /// the snapshot per-path CAS commits against; `None` on a checkout of the
    /// empty filesystem (first commit will pass `base_snapshot: None`).
    pub base_snapshot: Option<String>,
    /// the duckfs subtree this checkout covers, e.g. `/shared/ws`.
    pub prefix: String,
    /// the node http base url worktree verbs default to (a `--node` flag overrides).
    pub node: String,
    pub entries: BTreeMap<String, IndexEntry>,
}

/// index errors are plain (no `files:` prefix — that prefix is the module's; this
/// is client-side). the version mismatch names the re-checkout remedy the spec
/// prescribes ("the client re-checks out").
#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("duckfs: index io: {0}")]
    Io(String),
    #[error("duckfs: index is not valid json: {0}")]
    Parse(String),
    #[error(
        "duckfs: index version {found} is unsupported (this client writes v{expected}); \
         re-checkout the directory to rebuild it"
    )]
    Version { found: u32, expected: u32 },
}

impl Index {
    /// a fresh index for a checkout under `prefix` talking to `node`, descending
    /// from `base_snapshot` (`None` = the empty filesystem).
    pub fn new(
        prefix: impl Into<String>,
        node: impl Into<String>,
        base_snapshot: Option<String>,
    ) -> Self {
        Index {
            version: INDEX_VERSION,
            base_snapshot,
            prefix: prefix.into(),
            node: node.into(),
            entries: BTreeMap::new(),
        }
    }

    /// the `.duckfs` directory under a checkout root.
    pub fn dir(root: &Path) -> PathBuf {
        root.join(DUCKFS_DIR)
    }

    /// the `index.json` path under a checkout root.
    pub fn path(root: &Path) -> PathBuf {
        Index::dir(root).join(INDEX_FILE)
    }

    /// load and validate the index at `<root>/.duckfs/index.json`. the version is
    /// checked BEFORE a full parse so a future schema fails with the re-checkout
    /// remedy instead of an opaque deserialize error over unfamiliar fields.
    pub fn load(root: &Path) -> Result<Index, IndexError> {
        let bytes = std::fs::read(Index::path(root)).map_err(|e| IndexError::Io(e.to_string()))?;

        // version-probe first: `{"version":N}` alone must route to the version
        // error, never a missing-field parse error.
        #[derive(Deserialize)]
        struct VersionProbe {
            version: u32,
        }
        let probe: VersionProbe =
            serde_json::from_slice(&bytes).map_err(|e| IndexError::Parse(e.to_string()))?;
        if probe.version != INDEX_VERSION {
            return Err(IndexError::Version {
                found: probe.version,
                expected: INDEX_VERSION,
            });
        }

        serde_json::from_slice(&bytes).map_err(|e| IndexError::Parse(e.to_string()))
    }

    /// save atomically: write `index.json.tmp` then rename over `index.json`, so a
    /// crash mid-write leaves the previous index intact (rename is atomic on the
    /// same filesystem, and `.duckfs` always sits beside the file).
    pub fn save(&self, root: &Path) -> Result<(), IndexError> {
        let dir = Index::dir(root);
        std::fs::create_dir_all(&dir).map_err(|e| IndexError::Io(e.to_string()))?;
        let body = serde_json::to_vec_pretty(self).map_err(|e| IndexError::Parse(e.to_string()))?;
        let tmp = dir.join(format!("{INDEX_FILE}.tmp"));
        std::fs::write(&tmp, &body).map_err(|e| IndexError::Io(e.to_string()))?;
        std::fs::rename(&tmp, Index::path(root)).map_err(|e| IndexError::Io(e.to_string()))?;
        Ok(())
    }
}
