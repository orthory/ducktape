//! FileAttr synthesis for both mounts.
//!
//! duckfs consensus metadata is deliberately minimal — size, an exec bit, a
//! content hash — and never OS uid/gid/mode/mtime (the determinism boundary). So
//! the POSIX attributes a mount reports are SYNTHETIC: every entry is owned by the
//! mounting user, permissions are derived from kind + the exec bit, and timestamps
//! that the model does not carry read as the epoch. This is a documented non-goal
//! (no permission fidelity), not a bug.
//!
//! the RO mount synthesizes from duckfs facts ([`synth_attr`]); the RW mount
//! passes through the backing file's real metadata ([`passthrough_attr`]) so
//! sizes, mtimes, and the exec bit round-trip through the working copy — but still
//! stamps the mounter's uid/gid.

use std::fs::Metadata;
use std::os::unix::fs::MetadataExt as _;
use std::time::{Duration, SystemTime};

use fuser::{FileAttr, FileType, INodeNo};

/// the three entry kinds duckfs materializes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Dir,
    File,
    Symlink,
}

/// synthesize a FileAttr for a duckfs entry (the read-only mount). timestamps are
/// the epoch (the model carries none); uid/gid are the mounter's; the mode is
/// `kind + exec` only.
pub fn synth_attr(ino: u64, kind: NodeKind, size: u64, exec: bool, uid: u32, gid: u32) -> FileAttr {
    let (ftype, perm) = match kind {
        NodeKind::Dir => (FileType::Directory, 0o755),
        NodeKind::File => (FileType::RegularFile, if exec { 0o755 } else { 0o644 }),
        NodeKind::Symlink => (FileType::Symlink, 0o777),
    };
    let epoch = SystemTime::UNIX_EPOCH;
    FileAttr {
        ino: INodeNo(ino),
        size,
        blocks: size.div_ceil(512),
        atime: epoch,
        mtime: epoch,
        ctime: epoch,
        crtime: epoch,
        kind: ftype,
        perm,
        nlink: if matches!(kind, NodeKind::Dir) { 2 } else { 1 },
        uid,
        gid,
        rdev: 0,
        blksize: 512,
        flags: 0,
    }
}

/// build a FileAttr from a backing file's real metadata (the read-write mount).
/// real size/mtime/mode carry through so tools and the exec bit behave, but the
/// owner is always stamped to the mounter (uid/gid fidelity is a non-goal).
pub fn passthrough_attr(ino: u64, meta: &Metadata, uid: u32, gid: u32) -> FileAttr {
    let kind = if meta.file_type().is_dir() {
        FileType::Directory
    } else if meta.file_type().is_symlink() {
        FileType::Symlink
    } else {
        FileType::RegularFile
    };
    let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    let atime = meta.accessed().unwrap_or(mtime);
    let ctime = SystemTime::UNIX_EPOCH
        .checked_add(Duration::new(
            meta.ctime().max(0) as u64,
            meta.ctime_nsec().max(0) as u32,
        ))
        .unwrap_or(mtime);
    FileAttr {
        ino: INodeNo(ino),
        size: meta.size(),
        blocks: meta.blocks(),
        atime,
        mtime,
        ctime,
        crtime: mtime,
        kind,
        perm: (meta.mode() & 0o7777) as u16,
        nlink: meta.nlink() as u32,
        uid,
        gid,
        rdev: meta.rdev() as u32,
        blksize: 512,
        flags: 0,
    }
}
