//! diff the working copy against the `.duckfs` index — git's status discipline.
//!
//! the fast path is mtime(+nanos)+size: an entry equal to the index is clean
//! WITHOUT reading its bytes. the load-bearing exception is git's racy-clean
//! rule: on a coarse-granularity filesystem a file rewritten in the same tick the
//! index was saved keeps its mtime, so any entry whose mtime is not strictly
//! older than the index file's own mtime is re-hashed rather than trusted. a
//! rehash of unchanged content still matches, so the rule only ever costs a read
//! — it never false-positives; it only closes the same-tick false-negative.

use std::collections::BTreeSet;
use std::os::unix::fs::MetadataExt as _;
use std::path::Path;

use duckfs_core::to_hex;

use crate::chunk::{chunk_ids, file_object_id};
use crate::index::{EntryKind, Index, IndexEntry, IndexError};
use crate::scan::{Ignore, ScanEntry, ScanError, ScanKind, disk_path, is_builtin_skip, scan};

/// the working-copy delta: files/symlinks/empty-dirs added or modified relative
/// to the index base, and index paths removed from disk. `clean` iff all empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub added: Vec<ScanEntry>,
    pub modified: Vec<ScanEntry>,
    pub removed: Vec<String>,
    pub clean: bool,
}

impl Status {
    /// keep only the changes at or under one of `specs` — the `ducktape fs`
    /// pathspec. a spec is a path relative to the checkout root (`src/lib.rs`,
    /// `.`) or an absolute duckfs path (`/shared/ws/src`); an EMPTY list selects
    /// everything. this is what makes the `MAX_CHANGES_PER_COMMIT` refusal
    /// actionable: a tree past the wire cap is committed a subtree at a time,
    /// each commit still one atomic unit.
    pub fn select(&self, specs: &[String], prefix: &str) -> Status {
        if specs.is_empty() {
            return self.clone();
        }
        let selected: Vec<String> = specs.iter().map(|s| duckfs_spec(s, prefix)).collect();
        let keep = |path: &String| {
            selected
                .iter()
                .any(|sel| path == sel || path.starts_with(&format!("{sel}/")))
        };
        let added: Vec<ScanEntry> = self
            .added
            .iter()
            .filter(|e| keep(&e.path))
            .cloned()
            .collect();
        let modified: Vec<ScanEntry> = self
            .modified
            .iter()
            .filter(|e| keep(&e.path))
            .cloned()
            .collect();
        let removed: Vec<String> = self.removed.iter().filter(|p| keep(p)).cloned().collect();
        let clean = added.is_empty() && modified.is_empty() && removed.is_empty();
        Status {
            added,
            modified,
            removed,
            clean,
        }
    }
}

/// a pathspec as an absolute duckfs path: an absolute spec is taken as written,
/// anything else is relative to the checkout root (and `.` is the whole tree).
fn duckfs_spec(spec: &str, prefix: &str) -> String {
    let root = prefix.trim_end_matches('/');
    let trimmed = spec.trim_end_matches('/');
    if trimmed.starts_with('/') {
        return trimmed.to_string();
    }
    let rel = trimmed.strip_prefix("./").unwrap_or(trimmed);
    let is_whole_tree = rel.is_empty() || rel == ".";
    if is_whole_tree {
        return root.to_string();
    }
    format!("{root}/{rel}")
}

#[derive(Debug, thiserror::Error)]
pub enum StatusError {
    #[error(transparent)]
    Index(#[from] IndexError),
    #[error(transparent)]
    Scan(#[from] ScanError),
    #[error("duckfs: status io: {0}")]
    Io(String),
}

/// compute the status of the checkout rooted at `root` (which must hold a
/// `.duckfs/index.json`).
pub fn status(root: &Path) -> Result<Status, StatusError> {
    let index = Index::load(root)?;
    let scanned = scan(root, &index.prefix)?;
    let index_mtime = index_file_mtime(root)?;

    // every ancestor directory of an indexed path exists in the base snapshot
    // (implied by its children — only EMPTY dirs get their own index entry). a
    // dir emptied on disk by deleting its last indexed child is still one of
    // these: the module's Rm removes the entry, never its parent tree, so
    // planning a Mkdir for it would reject ("target already exists").
    let base_dirs = ancestor_dirs(&index);

    let mut added = Vec::new();
    let mut modified = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();

    for entry in &scanned {
        seen.insert(entry.path.clone());
        match entry.kind {
            // only EMPTY dirs are tracked; a non-empty dir is implied by its
            // entries, but it stays "seen" so it is never reported removed.
            ScanKind::Dir => {
                if entry.empty_dir {
                    match index.entries.get(&entry.path) {
                        Some(e) if e.kind == EntryKind::Dir => {}
                        Some(_) => modified.push(entry.clone()),
                        None if base_dirs.contains(&entry.path) => {}
                        None => added.push(entry.clone()),
                    }
                }
            }
            ScanKind::File | ScanKind::Symlink => match index.entries.get(&entry.path) {
                None => added.push(entry.clone()),
                Some(recorded) => {
                    if is_modified(entry, recorded, root, &index.prefix, index_mtime)? {
                        modified.push(entry.clone());
                    }
                }
            },
        }
    }

    // removed: any recorded path no longer on disk. a recorded empty dir that
    // became non-empty is still "seen" (the non-empty dir scanned), so it is not
    // spuriously removed — its new children show up as added instead.
    //
    // an INDEXED path the ignore file now excludes is NOT one of these: the walk
    // pruned it, so it is absent for a reason that is not deletion. it freezes at
    // its recorded state instead — writing `target/` into `.duckfsignore` after
    // committing it must never turn into a 100k-path deletion commit.
    let ignore = Ignore::load(root)?;
    let removed: Vec<String> = index
        .entries
        .keys()
        .filter(|path| !seen.contains(*path))
        .filter(|path| !is_ignored(&ignore, path, &index))
        .cloned()
        .collect();

    let clean = added.is_empty() && modified.is_empty() && removed.is_empty();
    Ok(Status {
        added,
        modified,
        removed,
        clean,
    })
}

/// is this INDEXED duckfs path one the walk never reaches — a built-in skip, or
/// excluded by the ignore file? the rules are written against the
/// checkout-relative path, so the index `prefix` comes off first.
fn is_ignored(ignore: &Ignore, path: &str, index: &Index) -> bool {
    let rel = path
        .strip_prefix(&index.prefix)
        .unwrap_or(path)
        .trim_start_matches('/');
    if is_builtin_skip(rel) {
        return true;
    }
    let is_dir = index
        .entries
        .get(path)
        .is_some_and(|e| e.kind == EntryKind::Dir);
    ignore.ignored_path(rel, is_dir)
}

/// is a tracked file/symlink modified relative to `recorded`? cheap checks first
/// (kind flip, exec flip, size), then the mtime fast path with the racy-clean
/// exception, and only on suspicion a rehash + object-id compare.
fn is_modified(
    entry: &ScanEntry,
    recorded: &IndexEntry,
    root: &Path,
    prefix: &str,
    index_mtime: (i64, u32),
) -> Result<bool, StatusError> {
    // a kind flip (file <-> symlink) is always a modification.
    let entry_kind = match entry.kind {
        ScanKind::File => EntryKind::File,
        ScanKind::Symlink => EntryKind::Symlink,
        ScanKind::Dir => return Ok(true),
    };
    if entry_kind != recorded.kind {
        return Ok(true);
    }
    // an exec-bit-only change is a modification (files carry the exec bit; a
    // symlink's is always false, so this never fires for one).
    if entry.kind == ScanKind::File && entry.exec != recorded.exec {
        return Ok(true);
    }
    // a size change is a modification without reading a byte.
    if entry.size != recorded.size {
        return Ok(true);
    }

    // a symlink's content IS its target string; size-equal means compare the
    // target by recomputing its file id (cheap — the target is already in hand).
    if entry.kind == ScanKind::Symlink {
        let target = entry.target.as_deref().unwrap_or_default();
        let recomputed = to_hex(&file_object_id(
            target.len() as u64,
            &chunk_ids(target.as_bytes()),
            &recorded.meta,
        ));
        return Ok(recomputed != recorded.object);
    }

    // file, size-equal: trust the mtime fast path only when the entry is strictly
    // older than the index (not racily clean) AND the mtime matches exactly.
    let mtime_equal =
        entry.mtime_secs == recorded.mtime_secs && entry.mtime_nanos == recorded.mtime_nanos;
    let racily_clean = (entry.mtime_secs, entry.mtime_nanos) >= index_mtime;
    if mtime_equal && !racily_clean {
        return Ok(false);
    }

    // suspicion → rehash and compare the file object id (meta from the record, so
    // the preimage matches exactly).
    let disk = disk_path(root, prefix, &entry.path);
    let bytes = std::fs::read(&disk).map_err(|e| StatusError::Io(e.to_string()))?;
    let recomputed = to_hex(&file_object_id(
        bytes.len() as u64,
        &chunk_ids(&bytes),
        &recorded.meta,
    ));
    Ok(recomputed != recorded.object)
}

/// every strict-ancestor directory of every indexed path — the directories the
/// base snapshot holds implicitly (a non-empty dir never gets its own entry).
fn ancestor_dirs(index: &Index) -> BTreeSet<String> {
    let mut dirs = BTreeSet::new();
    for path in index.entries.keys() {
        let mut end = path.len();
        while let Some(slash) = path[..end].rfind('/') {
            if slash == 0 {
                break;
            }
            dirs.insert(path[..slash].to_string());
            end = slash;
        }
    }
    dirs
}

/// the index file's own mtime, the reference the racy-clean rule compares against.
fn index_file_mtime(root: &Path) -> Result<(i64, u32), StatusError> {
    let meta = std::fs::metadata(Index::path(root)).map_err(|e| StatusError::Io(e.to_string()))?;
    Ok((meta.mtime(), meta.mtime_nsec() as u32))
}
