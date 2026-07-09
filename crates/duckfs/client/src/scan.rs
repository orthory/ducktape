//! walk a checkout directory into per-path facts (size, mtime, exec, kind).
//!
//! the walk is the raw OS observation; `status` diffs it against the index. paths
//! are the absolute duckfs paths (the index `prefix` joined with the on-disk
//! relative path), so a scan entry keys directly into the index. the `.duckfs`
//! state dir at the checkout root is skipped — it is client bookkeeping, not
//! replicated content.

use std::fs;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

/// what an on-disk entry is. only these three kinds materialize; anything else
/// (fifo, socket, device) is out of scope for a duckfs checkout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanKind {
    File,
    Symlink,
    Dir,
}

/// one observed path. `mtime` is split into whole seconds and sub-second nanos so
/// the racy-clean rule can compare at whatever granularity the filesystem offers.
/// `size` is the file byte length, a symlink target's byte length, or 0 for a dir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanEntry {
    /// the absolute duckfs path (index `prefix` + on-disk relative path).
    pub path: String,
    pub kind: ScanKind,
    pub size: u64,
    pub mtime_secs: i64,
    pub mtime_nanos: u32,
    pub exec: bool,
    /// the symlink target, present only for [`ScanKind::Symlink`].
    pub target: Option<String>,
    /// true only for a directory with no children — the case `status` tracks so a
    /// fresh empty dir can be told from a recorded one.
    pub empty_dir: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("duckfs: scan io: {0}")]
    Io(String),
    #[error("duckfs: non-utf8 path under the checkout: {0}")]
    NonUtf8(String),
}

impl From<std::io::Error> for ScanError {
    fn from(e: std::io::Error) -> Self {
        ScanError::Io(e.to_string())
    }
}

/// scan `root` (a checkout directory) under duckfs `prefix`, returning every
/// entry sorted by path. directories are emitted (with the `empty_dir` flag) as
/// well as files and symlinks, so `status` can both track empty dirs and treat a
/// non-empty dir as "seen" (never a spurious removal).
pub fn scan(root: &Path, prefix: &str) -> Result<Vec<ScanEntry>, ScanError> {
    let mut out = Vec::new();
    scan_dir(root, root, prefix, &mut out)?;
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// the disk path for a duckfs path under this checkout: strip the prefix and
/// re-root. shared with `status` so a rehash reads the right file.
pub fn disk_path(root: &Path, prefix: &str, duckfs_path: &str) -> PathBuf {
    let rel = duckfs_path
        .strip_prefix(prefix)
        .unwrap_or(duckfs_path)
        .trim_start_matches('/');
    root.join(rel)
}

fn scan_dir(
    root: &Path,
    dir: &Path,
    prefix: &str,
    out: &mut Vec<ScanEntry>,
) -> Result<(), ScanError> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        // the `.duckfs` bookkeeping dir at the checkout root is not content.
        if dir == root && entry.file_name() == std::ffi::OsStr::new(".duckfs") {
            continue;
        }

        let duckfs_path = duckfs_join(root, &path, prefix)?;
        // lstat, so a symlink is observed as a symlink (never followed).
        let meta = fs::symlink_metadata(&path)?;
        let ft = meta.file_type();

        if ft.is_symlink() {
            let target = fs::read_link(&path)?;
            let target = target
                .to_str()
                .ok_or_else(|| ScanError::NonUtf8(path.display().to_string()))?
                .to_string();
            out.push(ScanEntry {
                path: duckfs_path,
                kind: ScanKind::Symlink,
                size: target.len() as u64,
                mtime_secs: meta.mtime(),
                mtime_nanos: meta.mtime_nsec() as u32,
                exec: false,
                target: Some(target),
                empty_dir: false,
            });
        } else if ft.is_dir() {
            let empty = fs::read_dir(&path)?.next().is_none();
            out.push(ScanEntry {
                path: duckfs_path,
                kind: ScanKind::Dir,
                size: 0,
                mtime_secs: meta.mtime(),
                mtime_nanos: meta.mtime_nsec() as u32,
                exec: false,
                target: None,
                empty_dir: empty,
            });
            scan_dir(root, &path, prefix, out)?;
        } else {
            // a regular file: exec is the owner/group/other execute bits (the
            // module tracks one exec bit; any set bit means executable).
            let exec = meta.permissions().mode() & 0o111 != 0;
            out.push(ScanEntry {
                path: duckfs_path,
                kind: ScanKind::File,
                size: meta.len(),
                mtime_secs: meta.mtime(),
                mtime_nanos: meta.mtime_nsec() as u32,
                exec,
                target: None,
                empty_dir: false,
            });
        }
    }
    Ok(())
}

/// join the duckfs `prefix` with the on-disk path relative to the checkout root.
fn duckfs_join(root: &Path, path: &Path, prefix: &str) -> Result<String, ScanError> {
    let rel = path
        .strip_prefix(root)
        .map_err(|_| ScanError::Io("path escaped the checkout root".into()))?;
    let mut joined = prefix.trim_end_matches('/').to_string();
    for comp in rel.components() {
        let seg = comp
            .as_os_str()
            .to_str()
            .ok_or_else(|| ScanError::NonUtf8(path.display().to_string()))?;
        joined.push('/');
        joined.push_str(seg);
    }
    Ok(joined)
}
