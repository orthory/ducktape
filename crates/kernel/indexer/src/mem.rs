//! the sim arm of the [`IndexDisk`] seam: an in-memory filesystem for driving
//! the shipping lane (stage/commit/adopt/discard) with no tempdir.
//!
//! the model is a flat `PathBuf -> bytes` map of FILES; directories are
//! implicit (a directory "exists" when a file sits under it). that is exactly
//! enough for the staging lane, whose only structure is `_staging/<db>/<file>`
//! plus the `.complete` marker. `create_dir_all`/`sync_dir` are no-ops — RAM
//! has no directory entries to fsync and no torn-write window — which is why
//! the mem arm satisfies the durability contract trivially rather than
//! honoring it.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::disk::{DiskEntry, IndexDisk};

/// an in-memory [`IndexDisk`]. `Clone` shares the same backing store, so a
/// clone sees the other's writes — handy for a test that hands the disk to
/// several call sites.
#[derive(Clone, Default)]
pub struct MemDisk {
    files: Arc<Mutex<BTreeMap<PathBuf, Vec<u8>>>>,
}

impl IndexDisk for MemDisk {
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        self.files
            .lock()
            .unwrap()
            .get(path)
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no such file"))
    }

    fn write(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        self.files
            .lock()
            .unwrap()
            .insert(path.to_path_buf(), bytes.to_vec());
        Ok(())
    }

    fn read_dir(&self, dir: &Path) -> io::Result<Vec<DiskEntry>> {
        let files = self.files.lock().unwrap();
        // fold every key under `dir` to its immediate child; a child with
        // further components below it is a directory.
        let mut children: BTreeMap<String, bool> = BTreeMap::new();
        for key in files.keys() {
            let Ok(rel) = key.strip_prefix(dir) else {
                continue;
            };
            let mut comps = rel.components();
            let Some(first) = comps.next() else {
                continue; // `key == dir`: a file at the dir path, no child
            };
            let Some(name) = first.as_os_str().to_str().map(str::to_string) else {
                continue;
            };
            let is_dir = comps.next().is_some();
            children
                .entry(name)
                .and_modify(|d| *d |= is_dir)
                .or_insert(is_dir);
        }
        Ok(children
            .into_iter()
            .map(|(name, is_dir)| DiskEntry { name, is_dir })
            .collect())
    }

    fn create_dir_all(&self, _dir: &Path) -> io::Result<()> {
        Ok(()) // directories are implicit in the flat map
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        let mut files = self.files.lock().unwrap();
        let keys: Vec<PathBuf> = files
            .keys()
            .filter(|k| k.starts_with(from))
            .cloned()
            .collect();
        if keys.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "rename source missing",
            ));
        }
        for key in keys {
            let rel = key.strip_prefix(from).expect("filtered by prefix");
            let dest = if rel.as_os_str().is_empty() {
                to.to_path_buf()
            } else {
                to.join(rel)
            };
            let bytes = files.remove(&key).expect("key just enumerated");
            files.insert(dest, bytes);
        }
        Ok(())
    }

    fn remove_dir_all(&self, dir: &Path) -> io::Result<()> {
        self.files.lock().unwrap().retain(|k, _| !k.starts_with(dir));
        Ok(())
    }

    fn sync_dir(&self, _dir: &Path) -> io::Result<()> {
        Ok(()) // nothing to fsync in RAM
    }

    fn exists(&self, path: &Path) -> bool {
        self.files.lock().unwrap().keys().any(|k| k.starts_with(path))
    }
}
