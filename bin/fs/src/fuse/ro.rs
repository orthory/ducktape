//! the read-only mount: serve a duckfs subtree at a PINNED snapshot directly over
//! the phase-3 [`NodeApi`].
//!
//! the snapshot is resolved once at mount time and never moves, so every byte and
//! every directory listing is immutable for the mount's lifetime — the whole
//! surface caches hard. the inode table is built lazily from `ls`/`stat`; a
//! `readdir` also seeds the per-path stat cache for the children it lists, so a
//! subsequent `lookup` is a cache hit. file bytes go through a bounded
//! read-through block cache (see [`cache`]). all writes are refused by the kernel
//! (the mount carries `MountOption::RO`), so this filesystem implements only the
//! read side; unimplemented ops fall through to fuser's `ENOSYS`.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::sync::Mutex;

use duckfs_client::api::{ApiError, NodeApi};
use duckfs_client::http::HttpNode;
use duckfs_core::{EntryInfo, EntryKindWire, MAX_PAGE};
use fuser::{
    Errno, FileType, Filesystem, Generation, INodeNo, ReplyAttr, ReplyData, ReplyDirectory,
    ReplyEntry, ReplyStatfs, Request,
};

use super::MountArgs;
use super::attr::{NodeKind, synth_attr};
use super::cache::{BLOCK, BlockCache};
use super::inode::{Inodes, ROOT_INO};
use crate::args::CliError;

/// how much file data the read-through cache may hold before evicting oldest
/// blocks. 128 MiB is a generous ceiling that still bounds a huge-file stream.
const READ_CACHE_BUDGET: usize = 128 * 1024 * 1024;

/// mount `prefix` read-only at a resolved, pinned snapshot and serve until a
/// SIGINT/SIGTERM unmount.
pub fn mount_ro(m: &MountArgs) -> Result<(), CliError> {
    let api = HttpNode::new(m.node_url.clone());

    // resolve the snapshot ONCE and pin it (explicit, else head at mount time).
    let snapshot = match &m.snapshot {
        Some(s) => Some(s.clone()),
        None => api.refs().map_err(api_fail)?.head,
    };

    // validate the prefix names a directory at that snapshot. an absent prefix
    // (None) is a legal empty mount (mirrors checking out the empty tree).
    match api.stat(&m.prefix, snapshot.as_deref()).map_err(api_fail)? {
        Some(e) if matches!(e.kind, EntryKindWire::Dir) => {}
        Some(_) => {
            return Err(CliError::usage(format!(
                "{} is not a directory at that snapshot",
                m.prefix
            )));
        }
        None => {}
    }

    let (uid, gid) = super::caller_ids();
    let fs = ReadOnlyFs::new(api, snapshot.clone(), m.prefix.clone(), uid, gid);
    let banner = format!(
        "ducktape-fs: mounted {} read-only (snapshot {}) at {}\n\
         ducktape-fs: reads are pinned to this snapshot — remount to see newer commits; \
         Ctrl-C (or SIGTERM) to unmount",
        m.prefix,
        snapshot.as_deref().unwrap_or("(empty tree)"),
        m.dir.display(),
    );
    super::serve_until_signal(fs, &m.dir, true, &banner)
}

fn api_fail(e: ApiError) -> CliError {
    match e {
        ApiError::NotFound => CliError::failed("not found"),
        ApiError::Rejected(m) => CliError::failed(m),
        ApiError::Transport(m) => CliError::failed(format!("cannot reach the node: {m}")),
    }
}

/// the mutable, cache-heavy state behind one `Mutex` (the session is
/// single-threaded, so the lock never contends; it exists because fuser's
/// callbacks take `&self`).
struct Inner {
    inodes: Inodes,
    /// path -> committed entry at the pinned snapshot (`None` = negative cache).
    stat: HashMap<String, Option<EntryInfo>>,
    /// directory path -> its full child listing (cloned on read).
    listed: HashMap<String, Vec<EntryInfo>>,
    blocks: BlockCache,
}

/// a duckfs subtree served read-only at a fixed snapshot.
pub struct ReadOnlyFs {
    api: HttpNode,
    snapshot: Option<String>,
    /// the mount root's duckfs path (canonical, no trailing slash).
    prefix: String,
    uid: u32,
    gid: u32,
    inner: Mutex<Inner>,
}

impl ReadOnlyFs {
    fn new(api: HttpNode, snapshot: Option<String>, prefix: String, uid: u32, gid: u32) -> Self {
        ReadOnlyFs {
            api,
            snapshot,
            prefix,
            uid,
            gid,
            inner: Mutex::new(Inner {
                inodes: Inodes::new(),
                stat: HashMap::new(),
                listed: HashMap::new(),
                blocks: BlockCache::new(READ_CACHE_BUDGET),
            }),
        }
    }

    fn snap(&self) -> Option<&str> {
        self.snapshot.as_deref()
    }

    /// the duckfs path for a node's root-to-leaf name segments.
    fn join(&self, segs: &[String]) -> String {
        if segs.is_empty() {
            self.prefix.clone()
        } else {
            format!("{}/{}", self.prefix, segs.join("/"))
        }
    }

    /// stat a path at the pinned snapshot, caching the answer (including a
    /// negative one — the snapshot is immutable, so a miss stays a miss).
    fn stat_path(&self, inner: &mut Inner, path: &str) -> Result<Option<EntryInfo>, ApiError> {
        if let Some(cached) = inner.stat.get(path) {
            return Ok(cached.clone());
        }
        let info = self.api.stat(path, self.snap())?;
        inner.stat.insert(path.to_string(), info.clone());
        Ok(info)
    }

    /// list a directory at the pinned snapshot, paging to completion, caching the
    /// listing and seeding the stat cache for every child.
    fn list_dir(&self, inner: &mut Inner, dir: &str) -> Result<Vec<EntryInfo>, ApiError> {
        if let Some(cached) = inner.listed.get(dir) {
            return Ok(cached.clone());
        }
        let mut all = Vec::new();
        let mut after: Option<String> = None;
        loop {
            let (page, next) = self.api.ls(dir, self.snap(), after.as_deref(), MAX_PAGE)?;
            for e in &page {
                inner.stat.insert(e.path.clone(), Some(e.clone()));
            }
            all.extend(page);
            match next {
                Some(cursor) => after = Some(cursor),
                None => break,
            }
        }
        inner.listed.insert(dir.to_string(), all.clone());
        Ok(all)
    }
}

/// the last `/`-separated segment of a duckfs path (the child's own name).
fn last_segment(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn node_kind(kind: &EntryKindWire) -> NodeKind {
    match kind {
        EntryKindWire::Dir => NodeKind::Dir,
        EntryKindWire::File => NodeKind::File,
        EntryKindWire::Symlink => NodeKind::Symlink,
    }
}

fn file_type(kind: &EntryKindWire) -> FileType {
    match kind {
        EntryKindWire::Dir => FileType::Directory,
        EntryKindWire::File => FileType::RegularFile,
        EntryKindWire::Symlink => FileType::Symlink,
    }
}

fn errno(e: &ApiError) -> Errno {
    match e {
        ApiError::NotFound => Errno::ENOENT,
        _ => Errno::EIO,
    }
}

impl Filesystem for ReadOnlyFs {
    fn lookup(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
        let Some(name) = name.to_str() else {
            return reply.error(Errno::ENOENT);
        };
        let mut g = self.inner.lock().unwrap();
        let Some(parent_segs) = g.inodes.segments(parent.0) else {
            return reply.error(Errno::ENOENT);
        };
        let mut segs = parent_segs;
        segs.push(name.to_string());
        let path = self.join(&segs);
        match self.stat_path(&mut g, &path) {
            Ok(Some(info)) => {
                let ino = g.inodes.intern(parent.0, name);
                let attr = synth_attr(
                    ino,
                    node_kind(&info.kind),
                    info.size,
                    info.exec,
                    self.uid,
                    self.gid,
                );
                reply.entry(&super::TTL, &attr, Generation(0));
            }
            Ok(None) => reply.error(Errno::ENOENT),
            Err(e) => reply.error(errno(&e)),
        }
    }

    fn getattr(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: Option<fuser::FileHandle>,
        reply: ReplyAttr,
    ) {
        // the root is always a directory (validated at mount time).
        if ino.0 == ROOT_INO {
            let attr = synth_attr(ROOT_INO, NodeKind::Dir, 0, false, self.uid, self.gid);
            return reply.attr(&super::TTL, &attr);
        }
        let mut g = self.inner.lock().unwrap();
        let Some(segs) = g.inodes.segments(ino.0) else {
            return reply.error(Errno::ENOENT);
        };
        let path = self.join(&segs);
        match self.stat_path(&mut g, &path) {
            Ok(Some(info)) => {
                let attr = synth_attr(
                    ino.0,
                    node_kind(&info.kind),
                    info.size,
                    info.exec,
                    self.uid,
                    self.gid,
                );
                reply.attr(&super::TTL, &attr);
            }
            Ok(None) => reply.error(Errno::ENOENT),
            Err(e) => reply.error(errno(&e)),
        }
    }

    fn readlink(&self, _req: &Request, ino: INodeNo, reply: ReplyData) {
        let mut g = self.inner.lock().unwrap();
        let Some(segs) = g.inodes.segments(ino.0) else {
            return reply.error(Errno::ENOENT);
        };
        let path = self.join(&segs);
        let info = match self.stat_path(&mut g, &path) {
            Ok(Some(i)) if matches!(i.kind, EntryKindWire::Symlink) => i,
            Ok(Some(_)) => return reply.error(Errno::EINVAL),
            Ok(None) => return reply.error(Errno::ENOENT),
            Err(e) => return reply.error(errno(&e)),
        };
        // a symlink's content IS its target string (the module stores it as a
        // file); one read covers it (targets are ≤ the 4 KiB path cap).
        drop(g);
        match self.api.read(&path, self.snap(), 0, info.size) {
            Ok((bytes, _eof)) => reply.data(&bytes),
            Err(e) => reply.error(errno(&e)),
        }
    }

    fn read(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: fuser::FileHandle,
        offset: u64,
        size: u32,
        _flags: fuser::OpenFlags,
        _lock: Option<fuser::LockOwner>,
        reply: ReplyData,
    ) {
        let mut g = self.inner.lock().unwrap();
        let Some(segs) = g.inodes.segments(ino.0) else {
            return reply.error(Errno::ENOENT);
        };
        let path = self.join(&segs);
        let info = match self.stat_path(&mut g, &path) {
            Ok(Some(i)) => i,
            Ok(None) => return reply.error(Errno::ENOENT),
            Err(e) => return reply.error(errno(&e)),
        };
        match info.kind {
            EntryKindWire::File | EntryKindWire::Symlink => {}
            EntryKindWire::Dir => return reply.error(Errno::EISDIR),
        }
        let snap = self.snapshot.clone();
        let api = &self.api;
        let path_ref = path.clone();
        let result = g
            .blocks
            .read_range(ino.0, offset, size as u64, info.size, |o, l| {
                // one block window = one node Read (the module caps a read at BLOCK).
                debug_assert!(l <= BLOCK);
                api.read(&path_ref, snap.as_deref(), o, l)
                    .map(|(b, _eof)| b)
            });
        match result {
            Ok(bytes) => reply.data(&bytes),
            Err(e) => reply.error(errno(&e)),
        }
    }

    fn readdir(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: fuser::FileHandle,
        offset: u64,
        mut reply: ReplyDirectory,
    ) {
        let mut g = self.inner.lock().unwrap();
        let Some(segs) = g.inodes.segments(ino.0) else {
            return reply.error(Errno::ENOENT);
        };
        let dir = self.join(&segs);
        let children = match self.list_dir(&mut g, &dir) {
            Ok(c) => c,
            Err(e) => return reply.error(errno(&e)),
        };
        let parent_ino = g.inodes.parent_of(ino.0).unwrap_or(ROOT_INO);

        // "." and ".." then the children, each interned so its ino is stable.
        let mut entries: Vec<(u64, FileType, String)> = Vec::with_capacity(children.len() + 2);
        entries.push((ino.0, FileType::Directory, ".".to_string()));
        entries.push((parent_ino, FileType::Directory, "..".to_string()));
        for info in &children {
            let name = last_segment(&info.path).to_string();
            let cino = g.inodes.intern(ino.0, &name);
            entries.push((cino, file_type(&info.kind), name));
        }

        // the offset is the cookie of the last entry already sent (0 first call);
        // resume at that index and hand each entry a 1-based cookie.
        for (idx, (cino, kind, name)) in entries.iter().enumerate().skip(offset as usize) {
            if reply.add(INodeNo(*cino), (idx + 1) as u64, *kind, name) {
                break; // the reply buffer is full; the kernel will ask again.
            }
        }
        reply.ok();
    }

    fn statfs(&self, _req: &Request, _ino: INodeNo, reply: ReplyStatfs) {
        // a synthetic, read-only volume: report a namelen and block size, zero
        // free space (nothing is writable here).
        reply.statfs(0, 0, 0, 0, 0, BLOCK as u32, 255, BLOCK as u32);
    }
}
