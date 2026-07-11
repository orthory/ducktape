//! the read-write mount: front a REAL phase-3 working copy through FUSE.
//!
//! `--rw` checks the subtree out into a hidden `<mountpoint>.duckfs-backing`
//! directory (resuming an existing one from a prior session) and serves the mount
//! as a straight passthrough over that backing dir — reads and writes hit real
//! files on the local disk. writes become cluster truth only on COMMIT, run
//! through the same phase-3 engine the CLI uses:
//!
//! - explicit by default: nothing auto-lands; on unmount we print how to commit
//!   (or note the copy is clean);
//! - `--auto-commit N`: a background worker commits every N seconds while the
//!   working copy is dirty, auto-rebasing disjoint upstream work; a genuine
//!   conflict is logged LOUDLY and the mount KEEPS serving — writes are never
//!   silently merged and never dropped.
//!
//! the backing dir's `.duckfs` index is the engine's private bookkeeping, so it is
//! hidden from the mount (it never appears in a listing and cannot be opened
//! through the mountpoint). uid/gid are synthetic (the mounter owns everything);
//! size, mtime, and the exec bit pass through from the real backing file.

use std::collections::HashMap;

/// one opendir stream's sorted listing: (child ino, kind, name) rows.
type DirSnapshot = Vec<(u64, FileType, String)>;
use std::ffi::OsStr;
use std::fs::{File, OpenOptions, Permissions};
use std::os::unix::fs::{FileExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use duckfs_client::api::ConflictReport;
use duckfs_client::checkout::{CheckoutOptions, checkout_with};
use duckfs_client::commit::{CommitError, commit};
use duckfs_client::http::HttpNode;
use duckfs_client::index::Index;
use fuser::{
    Errno, FileType, Filesystem, Generation, INodeNo, ReplyAttr, ReplyCreate, ReplyData,
    ReplyDirectory, ReplyEmpty, ReplyEntry, ReplyOpen, ReplyStatfs, ReplyWrite, Request,
};

use super::MountArgs;
use super::attr::passthrough_attr;
use super::inode::{Inodes, ROOT_INO};
use crate::args::CliError;

/// the `.duckfs` bookkeeping dir hidden from the mount at its root.
const DUCKFS_DIR: &str = ".duckfs";

/// mount `prefix` read-write over a backing checkout and serve until a
/// SIGINT/SIGTERM unmount, then run the final commit / how-to-commit message.
pub fn mount_rw(m: &MountArgs) -> Result<(), CliError> {
    let backing = backing_dir(&m.dir);
    let api = HttpNode::new(m.node_url.clone());
    ensure_checkout(&api, &backing, m)?;

    let (uid, gid) = super::caller_ids();
    let fs = WorkingCopyFs::new(backing.clone(), uid, gid);

    // opt-in background commits; the flag stops the worker on unmount.
    let stop = Arc::new(AtomicBool::new(false));
    let worker = m.auto_commit.map(|interval| {
        spawn_auto_commit(m.node_url.clone(), backing.clone(), interval, stop.clone())
    });

    let banner = format!(
        "ducktape-fs: mounted {} read-write at {} (backing: {})\n\
         ducktape-fs: {}; Ctrl-C (or SIGTERM) to unmount",
        m.prefix,
        m.dir.display(),
        backing.display(),
        match m.auto_commit {
            Some(d) => format!("auto-committing every {}s while dirty", d.as_secs()),
            None => "writes commit only on demand — nothing auto-lands".to_string(),
        },
    );

    let served = super::serve_until_signal(fs, &m.dir, false, &banner);

    // stop the worker BEFORE finalizing so two commits never race.
    stop.store(true, Ordering::SeqCst);
    if let Some(w) = worker {
        let _ = w.join();
    }
    served?;

    finalize(&m.node_url, &backing, m.auto_commit.is_some());
    Ok(())
}

/// the sibling backing dir for a mountpoint: `<mountpoint>.duckfs-backing`.
fn backing_dir(mountpoint: &Path) -> PathBuf {
    let mut s = mountpoint.as_os_str().to_os_string();
    s.push(".duckfs-backing");
    PathBuf::from(s)
}

/// check the subtree out into `backing`, or resume a matching existing checkout.
/// a leftover backing dir from a crashed session (same prefix) is reused so its
/// uncommitted writes are not lost; a mismatched or non-checkout dir is refused
/// loudly rather than clobbered.
fn ensure_checkout(api: &HttpNode, backing: &Path, m: &MountArgs) -> Result<(), CliError> {
    if let Ok(index) = Index::load(backing) {
        if index.prefix == m.prefix {
            eprintln!(
                "ducktape-fs: resuming the existing working copy at {}",
                backing.display()
            );
            return Ok(());
        }
        return Err(CliError::failed(format!(
            "backing dir {} already holds a checkout of {} (not {}); \
             remove it or choose another mountpoint",
            backing.display(),
            index.prefix,
            m.prefix
        )));
    }
    let non_empty = backing.exists()
        && std::fs::read_dir(backing)
            .map(|mut d| d.next().is_some())
            .unwrap_or(false);
    if non_empty {
        return Err(CliError::failed(format!(
            "backing dir {} exists and is not a duckfs checkout; remove it first",
            backing.display()
        )));
    }
    let opts = CheckoutOptions {
        node_url: m.node_url.clone(),
        ..Default::default()
    };
    checkout_with(api, backing, &m.prefix, m.snapshot.as_deref(), &opts)
        .map_err(|e| CliError::failed(e.to_string()))?;
    Ok(())
}

/// the background auto-commit worker: every `interval` (polled in small steps so
/// unmount stops it promptly), commit the working copy if it is dirty.
fn spawn_auto_commit(
    node_url: String,
    backing: PathBuf,
    interval: Duration,
    stop: Arc<AtomicBool>,
) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("duckfs-autocommit".into())
        .spawn(move || {
            let api = HttpNode::new(node_url);
            let step = Duration::from_millis(200);
            while !stop.load(Ordering::SeqCst) {
                let mut waited = Duration::ZERO;
                while waited < interval && !stop.load(Ordering::SeqCst) {
                    std::thread::sleep(step);
                    waited += step;
                }
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                try_commit(&api, &backing, "duckfs auto-commit");
            }
        })
        .expect("spawn the auto-commit worker")
}

/// commit the working copy, tolerating a clean tree and — critically — a genuine
/// conflict: on conflict we log loudly and RETURN, leaving the mount serving and
/// the writes intact (never a silent merge, never a dropped write).
fn try_commit(api: &HttpNode, backing: &Path, message: &str) {
    match commit(api, backing, message) {
        Ok(summary) => eprintln!(
            "ducktape-fs: auto-committed {} (height {}){}",
            summary.snapshot,
            summary.height,
            if summary.rebased { " [rebased]" } else { "" }
        ),
        Err(CommitError::Nothing) => {}
        Err(CommitError::Conflict(report)) => log_conflict(&report),
        Err(e) => eprintln!("ducktape-fs: auto-commit error (mount keeps serving): {e}"),
    }
}

fn log_conflict(r: &ConflictReport) {
    eprintln!(
        "ducktape-fs: AUTO-COMMIT CONFLICT — the mount keeps serving; your writes are \
         NOT lost and NOT merged"
    );
    eprintln!("  base: {}", r.base.as_deref().unwrap_or("(none)"));
    eprintln!("  head: {}", r.head.as_deref().unwrap_or("(none)"));
    for p in &r.clashing {
        eprintln!("  clashing: {p}");
    }
    if !r.remedy.is_empty() {
        eprintln!("  remedy: {}", r.remedy);
    }
}

/// on unmount: in `--auto-commit` mode land a final commit of anything the last
/// interval missed; otherwise (explicit mode) print how to commit the still-dirty
/// working copy — nothing auto-lands.
fn finalize(node_url: &str, backing: &Path, auto: bool) {
    let api = HttpNode::new(node_url.to_string());
    if auto {
        try_commit(&api, backing, "duckfs auto-commit (final)");
        return;
    }
    let dirty = duckfs_client::status::status(backing)
        .map(|s| !s.clean)
        .unwrap_or(false);
    if dirty {
        eprintln!(
            "ducktape-fs: uncommitted changes remain in the working copy at {}",
            backing.display()
        );
        eprintln!(
            "ducktape-fs: commit them with:  ducktape-fs commit {} --message <m>",
            backing.display()
        );
        eprintln!("ducktape-fs: or discard them by removing that directory");
    } else {
        eprintln!(
            "ducktape-fs: working copy clean — nothing to commit (backing: {})",
            backing.display()
        );
    }
}

/// map a std io error to an errno, falling back to EIO.
fn io_errno(e: &std::io::Error) -> Errno {
    e.raw_os_error().map(Errno::from_i32).unwrap_or(Errno::EIO)
}

/// the mutable passthrough state behind one `Mutex` (single-threaded session).
struct Inner {
    inodes: Inodes,
    /// open backing files keyed by the file handle we hand the kernel.
    handles: HashMap<u64, File>,
    /// per-directory listing snapshot for paged `readdir`: taken at offset 0,
    /// continuations page over it (re-reading the live dir mid-stream could
    /// reorder entries). the latest snapshot per dir is kept until the next
    /// offset-0 pass overwrites it.
    // keyed by (dir ino, directory file handle): each opendir stream pages
    // its own snapshot, so concurrent listers of one directory never clobber
    // each other's cookies. releasedir drops the stream's snapshot.
    dir_pages: HashMap<(u64, u64), DirSnapshot>,
}

/// a phase-3 working copy fronted as a passthrough FUSE filesystem.
pub struct WorkingCopyFs {
    backing: PathBuf,
    uid: u32,
    gid: u32,
    next_fh: AtomicU64,
    inner: Mutex<Inner>,
}

impl WorkingCopyFs {
    fn new(backing: PathBuf, uid: u32, gid: u32) -> Self {
        WorkingCopyFs {
            backing,
            uid,
            gid,
            next_fh: AtomicU64::new(1),
            inner: Mutex::new(Inner {
                inodes: Inodes::new(),
                handles: HashMap::new(),
                dir_pages: HashMap::new(),
            }),
        }
    }

    /// the backing-dir path for an inode's segments (root → the backing dir).
    fn real(&self, inner: &Inner, ino: u64) -> Option<PathBuf> {
        let segs = inner.inodes.segments(ino)?;
        let mut p = self.backing.clone();
        for s in &segs {
            p.push(s);
        }
        Some(p)
    }

    /// the backing-dir path of a child `name` under directory `parent`.
    fn child_real(&self, inner: &Inner, parent: u64, name: &str) -> Option<PathBuf> {
        self.real(inner, parent).map(|p| p.join(name))
    }

    fn next_handle(&self) -> u64 {
        self.next_fh.fetch_add(1, Ordering::SeqCst)
    }

    /// build an attr from a backing path's real (lstat) metadata.
    fn attr_of(&self, ino: u64, path: &Path) -> std::io::Result<fuser::FileAttr> {
        let meta = std::fs::symlink_metadata(path)?;
        Ok(passthrough_attr(ino, &meta, self.uid, self.gid))
    }
}

/// `.duckfs` under the mount root is engine-private — hide it from the mount.
fn hidden_at_root(parent: u64, name: &str) -> bool {
    parent == ROOT_INO && name == DUCKFS_DIR
}

/// the name-SORTED `(file type, name)` listing of a backing dir. sorting pins a
/// stable order: `read_dir` order is not guaranteed stable across calls, so a
/// paged `readdir` that re-derived it could skip or duplicate entries.
fn sorted_dir_entries(dir: &Path) -> std::io::Result<Vec<(FileType, String)>> {
    let mut out = Vec::new();
    for dirent in std::fs::read_dir(dir)?.flatten() {
        let Ok(name) = dirent.file_name().into_string() else {
            continue;
        };
        let ftype = match dirent.file_type() {
            Ok(ft) if ft.is_dir() => FileType::Directory,
            Ok(ft) if ft.is_symlink() => FileType::Symlink,
            Ok(_) => FileType::RegularFile,
            Err(_) => continue,
        };
        out.push((ftype, name));
    }
    out.sort_by(|a, b| a.1.cmp(&b.1));
    Ok(out)
}

/// translate open flags (raw `O_*` bits) into std `OpenOptions`.
fn open_opts(flags: i32) -> OpenOptions {
    let mut o = OpenOptions::new();
    match flags & libc::O_ACCMODE {
        libc::O_WRONLY => {
            o.write(true);
        }
        libc::O_RDWR => {
            o.read(true).write(true);
        }
        _ => {
            o.read(true);
        }
    }
    if flags & libc::O_APPEND != 0 {
        o.append(true);
    }
    if flags & libc::O_CREAT != 0 {
        o.create(true);
    }
    // O_TRUNC needs write access; guard it so a bogus RDONLY|TRUNC can't error.
    if flags & libc::O_TRUNC != 0 && (flags & libc::O_ACCMODE) != libc::O_RDONLY {
        o.truncate(true);
    }
    o
}

impl Filesystem for WorkingCopyFs {
    fn lookup(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
        let Some(name) = name.to_str() else {
            return reply.error(Errno::ENOENT);
        };
        if hidden_at_root(parent.0, name) {
            return reply.error(Errno::ENOENT);
        }
        let mut g = self.inner.lock().unwrap();
        let Some(path) = self.child_real(&g, parent.0, name) else {
            return reply.error(Errno::ENOENT);
        };
        if path.symlink_metadata().is_err() {
            return reply.error(Errno::ENOENT);
        }
        let ino = g.inodes.intern(parent.0, name);
        match self.attr_of(ino, &path) {
            Ok(attr) => reply.entry(&super::TTL, &attr, Generation(0)),
            Err(e) => reply.error(io_errno(&e)),
        }
    }

    fn getattr(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: Option<fuser::FileHandle>,
        reply: ReplyAttr,
    ) {
        let g = self.inner.lock().unwrap();
        let Some(path) = self.real(&g, ino.0) else {
            return reply.error(Errno::ENOENT);
        };
        match self.attr_of(ino.0, &path) {
            Ok(attr) => reply.attr(&super::TTL, &attr),
            Err(e) => reply.error(io_errno(&e)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn setattr(
        &self,
        _req: &Request,
        ino: INodeNo,
        mode: Option<u32>,
        _uid: Option<u32>,
        _gid: Option<u32>,
        size: Option<u64>,
        _atime: Option<fuser::TimeOrNow>,
        _mtime: Option<fuser::TimeOrNow>,
        _ctime: Option<std::time::SystemTime>,
        _fh: Option<fuser::FileHandle>,
        _crtime: Option<std::time::SystemTime>,
        _chgtime: Option<std::time::SystemTime>,
        _bkuptime: Option<std::time::SystemTime>,
        _flags: Option<fuser::BsdFileFlags>,
        reply: ReplyAttr,
    ) {
        let g = self.inner.lock().unwrap();
        let Some(path) = self.real(&g, ino.0) else {
            return reply.error(Errno::ENOENT);
        };
        // mode → chmod (so `chmod +x` round-trips the exec bit into a commit).
        if let Some(mode) = mode
            && let Err(e) = std::fs::set_permissions(&path, Permissions::from_mode(mode))
        {
            return reply.error(io_errno(&e));
        }
        // size → truncate/extend. times are synthetic (a documented non-goal).
        if let Some(sz) = size {
            match OpenOptions::new().write(true).open(&path) {
                Ok(f) => {
                    if let Err(e) = f.set_len(sz) {
                        return reply.error(io_errno(&e));
                    }
                }
                Err(e) => return reply.error(io_errno(&e)),
            }
        }
        match self.attr_of(ino.0, &path) {
            Ok(attr) => reply.attr(&super::TTL, &attr),
            Err(e) => reply.error(io_errno(&e)),
        }
    }

    fn readlink(&self, _req: &Request, ino: INodeNo, reply: ReplyData) {
        let g = self.inner.lock().unwrap();
        let Some(path) = self.real(&g, ino.0) else {
            return reply.error(Errno::ENOENT);
        };
        match std::fs::read_link(&path) {
            Ok(target) => reply.data(target.as_os_str().as_encoded_bytes()),
            Err(e) => reply.error(io_errno(&e)),
        }
    }

    fn open(&self, _req: &Request, ino: INodeNo, flags: fuser::OpenFlags, reply: ReplyOpen) {
        let mut g = self.inner.lock().unwrap();
        let Some(path) = self.real(&g, ino.0) else {
            return reply.error(Errno::ENOENT);
        };
        match open_opts(flags.0).open(&path) {
            Ok(file) => {
                let fh = self.next_handle();
                g.handles.insert(fh, file);
                reply.opened(fuser::FileHandle(fh), fuser::FopenFlags::empty());
            }
            Err(e) => reply.error(io_errno(&e)),
        }
    }

    fn read(
        &self,
        _req: &Request,
        ino: INodeNo,
        fh: fuser::FileHandle,
        offset: u64,
        size: u32,
        _flags: fuser::OpenFlags,
        _lock: Option<fuser::LockOwner>,
        reply: ReplyData,
    ) {
        let g = self.inner.lock().unwrap();
        let mut buf = vec![0u8; size as usize];
        // prefer the open handle; fall back to opening the path (stateless read).
        let n = if let Some(file) = g.handles.get(&fh.0) {
            file.read_at(&mut buf, offset)
        } else {
            match self.real(&g, ino.0).map(File::open) {
                Some(Ok(file)) => file.read_at(&mut buf, offset),
                Some(Err(e)) => return reply.error(io_errno(&e)),
                None => return reply.error(Errno::ENOENT),
            }
        };
        match n {
            Ok(n) => {
                buf.truncate(n);
                reply.data(&buf);
            }
            Err(e) => reply.error(io_errno(&e)),
        }
    }

    fn write(
        &self,
        _req: &Request,
        ino: INodeNo,
        fh: fuser::FileHandle,
        offset: u64,
        data: &[u8],
        _write_flags: fuser::WriteFlags,
        _flags: fuser::OpenFlags,
        _lock: Option<fuser::LockOwner>,
        reply: ReplyWrite,
    ) {
        let g = self.inner.lock().unwrap();
        let res = if let Some(file) = g.handles.get(&fh.0) {
            file.write_at(data, offset)
        } else {
            match self
                .real(&g, ino.0)
                .map(|p| OpenOptions::new().write(true).open(p))
            {
                Some(Ok(file)) => file.write_at(data, offset),
                Some(Err(e)) => return reply.error(io_errno(&e)),
                None => return reply.error(Errno::ENOENT),
            }
        };
        match res {
            Ok(n) => reply.written(n as u32),
            Err(e) => reply.error(io_errno(&e)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn create(
        &self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        mode: u32,
        _umask: u32,
        flags: i32,
        reply: ReplyCreate,
    ) {
        let Some(name) = name.to_str() else {
            return reply.error(Errno::EINVAL);
        };
        if hidden_at_root(parent.0, name) {
            return reply.error(Errno::EACCES);
        }
        let mut g = self.inner.lock().unwrap();
        let Some(path) = self.child_real(&g, parent.0, name) else {
            return reply.error(Errno::ENOENT);
        };
        let mut opts = open_opts(flags);
        opts.create(true).mode(mode);
        match opts.open(&path) {
            Ok(file) => {
                let ino = g.inodes.intern(parent.0, name);
                let fh = self.next_handle();
                g.handles.insert(fh, file);
                match self.attr_of(ino, &path) {
                    Ok(attr) => reply.created(
                        &super::TTL,
                        &attr,
                        Generation(0),
                        fuser::FileHandle(fh),
                        fuser::FopenFlags::empty(),
                    ),
                    Err(e) => reply.error(io_errno(&e)),
                }
            }
            Err(e) => reply.error(io_errno(&e)),
        }
    }

    fn mkdir(
        &self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        mode: u32,
        _umask: u32,
        reply: ReplyEntry,
    ) {
        let Some(name) = name.to_str() else {
            return reply.error(Errno::EINVAL);
        };
        let mut g = self.inner.lock().unwrap();
        let Some(path) = self.child_real(&g, parent.0, name) else {
            return reply.error(Errno::ENOENT);
        };
        if let Err(e) = std::fs::create_dir(&path) {
            return reply.error(io_errno(&e));
        }
        let _ = std::fs::set_permissions(&path, Permissions::from_mode(mode));
        let ino = g.inodes.intern(parent.0, name);
        match self.attr_of(ino, &path) {
            Ok(attr) => reply.entry(&super::TTL, &attr, Generation(0)),
            Err(e) => reply.error(io_errno(&e)),
        }
    }

    fn unlink(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
        let Some(name) = name.to_str() else {
            return reply.error(Errno::EINVAL);
        };
        let g = self.inner.lock().unwrap();
        let Some(path) = self.child_real(&g, parent.0, name) else {
            return reply.error(Errno::ENOENT);
        };
        match std::fs::remove_file(&path) {
            Ok(()) => reply.ok(),
            Err(e) => reply.error(io_errno(&e)),
        }
    }

    fn rmdir(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
        let Some(name) = name.to_str() else {
            return reply.error(Errno::EINVAL);
        };
        let g = self.inner.lock().unwrap();
        let Some(path) = self.child_real(&g, parent.0, name) else {
            return reply.error(Errno::ENOENT);
        };
        match std::fs::remove_dir(&path) {
            Ok(()) => reply.ok(),
            Err(e) => reply.error(io_errno(&e)),
        }
    }

    fn symlink(
        &self,
        _req: &Request,
        parent: INodeNo,
        link_name: &OsStr,
        target: &Path,
        reply: ReplyEntry,
    ) {
        let Some(name) = link_name.to_str() else {
            return reply.error(Errno::EINVAL);
        };
        let mut g = self.inner.lock().unwrap();
        let Some(path) = self.child_real(&g, parent.0, name) else {
            return reply.error(Errno::ENOENT);
        };
        if let Err(e) = std::os::unix::fs::symlink(target, &path) {
            return reply.error(io_errno(&e));
        }
        let ino = g.inodes.intern(parent.0, name);
        match self.attr_of(ino, &path) {
            Ok(attr) => reply.entry(&super::TTL, &attr, Generation(0)),
            Err(e) => reply.error(io_errno(&e)),
        }
    }

    fn rename(
        &self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        newparent: INodeNo,
        newname: &OsStr,
        _flags: fuser::RenameFlags,
        reply: ReplyEmpty,
    ) {
        let (Some(name), Some(newname)) = (name.to_str(), newname.to_str()) else {
            return reply.error(Errno::EINVAL);
        };
        if hidden_at_root(parent.0, name) || hidden_at_root(newparent.0, newname) {
            return reply.error(Errno::EACCES);
        }
        let mut g = self.inner.lock().unwrap();
        let (Some(from), Some(to)) = (
            self.child_real(&g, parent.0, name),
            self.child_real(&g, newparent.0, newname),
        ) else {
            return reply.error(Errno::ENOENT);
        };
        if let Err(e) = std::fs::rename(&from, &to) {
            return reply.error(io_errno(&e));
        }
        // keep the inode table consistent so an already-cached ino (and its
        // subtree) follows the move.
        let moved = g.inodes.intern(parent.0, name);
        g.inodes.rename(moved, newparent.0, newname);
        reply.ok();
    }

    fn flush(
        &self,
        _req: &Request,
        _ino: INodeNo,
        fh: fuser::FileHandle,
        _lock_owner: fuser::LockOwner,
        reply: ReplyEmpty,
    ) {
        let g = self.inner.lock().unwrap();
        if let Some(file) = g.handles.get(&fh.0)
            && let Err(e) = file.sync_all()
        {
            return reply.error(io_errno(&e));
        }
        reply.ok();
    }

    fn fsync(
        &self,
        _req: &Request,
        _ino: INodeNo,
        fh: fuser::FileHandle,
        datasync: bool,
        reply: ReplyEmpty,
    ) {
        let g = self.inner.lock().unwrap();
        if let Some(file) = g.handles.get(&fh.0) {
            let r = if datasync {
                file.sync_data()
            } else {
                file.sync_all()
            };
            if let Err(e) = r {
                return reply.error(io_errno(&e));
            }
        }
        reply.ok();
    }

    fn release(
        &self,
        _req: &Request,
        _ino: INodeNo,
        fh: fuser::FileHandle,
        _flags: fuser::OpenFlags,
        _lock_owner: Option<fuser::LockOwner>,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        let mut g = self.inner.lock().unwrap();
        g.handles.remove(&fh.0);
        reply.ok();
    }

    fn readdir(
        &self,
        _req: &Request,
        ino: INodeNo,
        fh: fuser::FileHandle,
        offset: u64,
        mut reply: ReplyDirectory,
    ) {
        let mut g = self.inner.lock().unwrap();
        // offset 0 opens a new stream: snapshot the (sorted) listing. paged
        // continuations replay the snapshot rather than re-reading the live dir,
        // whose iteration order could shift between calls.
        if offset == 0 {
            let Some(dir) = self.real(&g, ino.0) else {
                return reply.error(Errno::ENOENT);
            };
            let listed = match sorted_dir_entries(&dir) {
                Ok(l) => l,
                Err(e) => return reply.error(io_errno(&e)),
            };
            let parent_ino = g.inodes.parent_of(ino.0).unwrap_or(ROOT_INO);
            let mut entries: DirSnapshot = vec![
                (ino.0, FileType::Directory, ".".to_string()),
                (parent_ino, FileType::Directory, "..".to_string()),
            ];
            for (ftype, name) in listed {
                if hidden_at_root(ino.0, &name) {
                    continue; // never expose the engine's `.duckfs` index
                }
                let cino = g.inodes.intern(ino.0, &name);
                entries.push((cino, ftype, name));
            }
            g.dir_pages.insert((ino.0, fh.0), entries);
        }
        // a continuation without a prior offset-0 pass has nothing to page over.
        let Some(entries) = g.dir_pages.get(&(ino.0, fh.0)) else {
            return reply.error(Errno::ENOENT);
        };
        // the offset is the cookie of the last entry already sent (0 first call);
        // resume at that index and hand each entry a 1-based cookie.
        for (idx, (cino, kind, name)) in entries.iter().enumerate().skip(offset as usize) {
            if reply.add(INodeNo(*cino), (idx + 1) as u64, *kind, name) {
                break; // the reply buffer is full; the kernel will ask again.
            }
        }
        reply.ok();
    }

    fn releasedir(
        &self,
        _req: &Request,
        ino: INodeNo,
        fh: fuser::FileHandle,
        _flags: fuser::OpenFlags,
        reply: ReplyEmpty,
    ) {
        let mut g = self.inner.lock().unwrap();
        g.dir_pages.remove(&(ino.0, fh.0));
        reply.ok();
    }

    fn access(&self, _req: &Request, _ino: INodeNo, _mask: fuser::AccessFlags, reply: ReplyEmpty) {
        reply.ok();
    }

    fn statfs(&self, _req: &Request, _ino: INodeNo, reply: ReplyStatfs) {
        reply.statfs(0, 0, 0, 0, 0, 512, 255, 512);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// paged readdir depends on a stable, name-sorted listing — pin it.
    #[test]
    fn sorted_dir_entries_is_name_sorted_with_kinds() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("b.txt"), b"x").unwrap();
        std::fs::create_dir(dir.path().join("a-dir")).unwrap();
        std::os::unix::fs::symlink("b.txt", dir.path().join("c-link")).unwrap();

        let got = sorted_dir_entries(dir.path()).unwrap();
        assert_eq!(
            got,
            vec![
                (FileType::Directory, "a-dir".to_string()),
                (FileType::RegularFile, "b.txt".to_string()),
                (FileType::Symlink, "c-link".to_string()),
            ]
        );
        // the order is deterministic across calls — the paging invariant.
        assert_eq!(sorted_dir_entries(dir.path()).unwrap(), got);
    }
}
