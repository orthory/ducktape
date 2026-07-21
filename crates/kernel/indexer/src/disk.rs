//! the indexer's disk seam: the small filesystem contract the shipping lane
//! (checkpoint file sets + staged installs) needs, plus its real arm.
//!
//! only the SHIPPING LANE touches raw files — cutting a checkpoint's file set
//! and moving staged database directories into place. the per-module read
//! models themselves live in fluent31 `Db`s, which own their own IO; this trait
//! does NOT reach them (there is no mem-backed fluent31), so `apply_block` /
//! `scan` stay real-disk operations. what this seam buys is a mockable staging
//! lane: [`crate::MemDisk`] (feature `sim`) runs the whole
//! stage/commit/adopt/discard sequence with no tempdir.

use std::io;
use std::path::Path;

/// one entry from [`IndexDisk::read_dir`]: the child's name and whether it is a
/// directory. names that are not valid utf-8 are dropped by the real arm (a
/// checkpoint file set and a staging root only ever hold ascii names).
#[derive(Clone, Debug)]
pub struct DiskEntry {
    pub name: String,
    pub is_dir: bool,
}

/// the filesystem operations the indexer's shipping lane performs itself —
/// exactly the ops the former raw `std::fs` sites used. methods return
/// [`std::io::Result`] so the lane's error wrapping (`Error::Shipping`) is
/// unchanged. `write`/`sync_dir` carry a durability contract on the real arm
/// that a mem arm trivially satisfies (RAM has no torn-write window).
pub trait IndexDisk: Send + Sync {
    /// read a file whole.
    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;

    /// create (truncating) a file, write it, and fsync its DATA before return.
    fn write(&self, path: &Path, bytes: &[u8]) -> io::Result<()>;

    /// immediate children of a directory (files and subdirectories).
    fn read_dir(&self, dir: &Path) -> io::Result<Vec<DiskEntry>>;

    /// create a directory and every missing parent.
    fn create_dir_all(&self, dir: &Path) -> io::Result<()>;

    /// atomically move `from` onto `to` (rename semantics).
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;

    /// remove a directory and its whole subtree.
    fn remove_dir_all(&self, dir: &Path) -> io::Result<()>;

    /// fsync a directory so its entries (created/renamed files) are durable.
    fn sync_dir(&self, dir: &Path) -> io::Result<()>;

    /// whether a path exists (a file or a directory).
    fn exists(&self, path: &Path) -> bool;
}

/// the real arm: the moved raw `std::fs` code, verbatim in behavior. a
/// zero-sized unit — [`IndexStore`](crate::IndexStore) defaults to it and the
/// shipping-lane free functions take `&DiskFs` in production.
pub struct DiskFs;

impl IndexDisk for DiskFs {
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }

    fn write(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        use std::io::Write as _;
        let mut f = std::fs::File::create(path)?;
        f.write_all(bytes)?;
        f.sync_all()
    }

    fn read_dir(&self, dir: &Path) -> io::Result<Vec<DiskEntry>> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                continue; // a non-utf-8 name is never one this lane wrote
            };
            let is_dir = entry.file_type()?.is_dir();
            out.push(DiskEntry { name, is_dir });
        }
        Ok(out)
    }

    fn create_dir_all(&self, dir: &Path) -> io::Result<()> {
        std::fs::create_dir_all(dir)
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        std::fs::rename(from, to)
    }

    fn remove_dir_all(&self, dir: &Path) -> io::Result<()> {
        std::fs::remove_dir_all(dir)
    }

    fn sync_dir(&self, dir: &Path) -> io::Result<()> {
        std::fs::File::open(dir)?.sync_all()
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }
}
