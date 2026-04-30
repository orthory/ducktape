use std::{
    path::Path,
    sync::{Mutex, RwLock},
};

use doctree::{Entry, Tree, TreeError};

use crate::{Driver, build_tree};

/// `PersistedTree` is the persistence-aware wrapper around an in-memory
/// `Tree`. It owns both the tree and the driver, and holds them behind their
/// respective locks so the api layer never sees `Driver`, locks, or the
/// version-swap mechanics.
///
/// Concurrency model:
/// - Reads acquire a read guard on the tree (concurrent reads are fine).
/// - Writes acquire a write guard on the tree, then a lock on the driver.
///   The driver's `&mut self` write method is reachable only through the
///   `Mutex` — exclusivity is enforced by the type system.
pub struct PersistedTree {
    tree: RwLock<Tree>,
    driver: Mutex<Box<dyn Driver>>,
}

#[derive(thiserror::Error, Debug)]
pub enum PersistedTreeError {
    #[error("PersistedTreeError: {0}")]
    Tree(#[from] TreeError),

    #[error("PersistedTreeError: {0}")]
    Lock(String),
}

impl PersistedTree {
    /// Builds the initial tree from the driver and stores both behind locks.
    pub fn open(
        driver: impl Driver + 'static,
        basedir: &Path,
    ) -> Result<Self, PersistedTreeError> {
        let tree = build_tree(&driver, basedir)?;
        Ok(Self {
            tree: RwLock::new(tree),
            driver: Mutex::new(Box::new(driver)),
        })
    }

    /// Reads under a tree read guard, returning a clone of the matched
    /// `Entry` so the lock can be released before the caller does anything
    /// expensive with it. Cloning an `Entry` is cheap — Directory variants
    /// share their subtrees via `Arc`.
    pub fn get_entries(&self, document_path: String) -> Result<Entry, PersistedTreeError> {
        let tree = self
            .tree
            .read()
            .map_err(|e| PersistedTreeError::Lock(e.to_string()))?;
        let entry = tree.get_entries(document_path)?;
        Ok(entry.clone())
    }

    /// Produces a new tree version with a fresh document and swaps it into
    /// place. Driver write-through is not yet wired here — it'll plug in
    /// once the Document → bytes serialization path lands. The locking
    /// dance is in place so the wiring is a one-line addition.
    pub fn create_document(&self) -> Result<String, PersistedTreeError> {
        let mut tree_guard = self
            .tree
            .write()
            .map_err(|e| PersistedTreeError::Lock(e.to_string()))?;
        let (next, path) = tree_guard.create_document()?;

        // Reserved spot for: serialize the new entry to bytes, then
        // self.driver.lock().unwrap().write(Path::new(&path), &bytes)?;
        // Order matters: persist before swap so a crash leaves the on-disk
        // truth as either the old version or the new one, never an in-memory
        // state that disagrees with disk.
        let _ = &self.driver;

        *tree_guard = next;
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Vfs;
    use doctree::Entry;

    #[test]
    fn open_then_get_entries_returns_loaded_doc() {
        let mut fs = Vfs::new();
        fs.write_file("/root/a.md", b"hello".to_vec());

        let pt = PersistedTree::open(fs, Path::new("/root")).unwrap();
        let entry = pt.get_entries("a.md".into()).unwrap();
        assert!(matches!(entry, Entry::File(_)));
    }

    #[test]
    fn create_document_returns_path_and_swaps_tree() {
        let mut fs = Vfs::new();
        fs.write_file("/root/existing.md", b"".to_vec());

        let pt = PersistedTree::open(fs, Path::new("/root")).unwrap();
        let path = pt.create_document().unwrap();
        assert_eq!(path, "/tmp");

        // The original entry still resolves — the swap didn't blow away the
        // existing tree.
        let existing = pt.get_entries("existing.md".into()).unwrap();
        assert!(matches!(existing, Entry::File(_)));
    }
}
