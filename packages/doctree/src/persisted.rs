use std::{
    path::{Path, PathBuf},
    sync::{Mutex, RwLock},
};

use crate::{
    Entry, Tree, TreeError, build_tree,
    drivers::{Driver, DriverError},
};

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
    Driver(#[from] DriverError),

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

    /// Mints a new document, persists an empty buffer for it via the driver,
    /// and swaps in a tree version that includes the new entry. Returns the
    /// generated identifier (a basename within the tree's basedir).
    ///
    /// Order: write to driver first, then swap the tree. If the write fails,
    /// the in-memory tree stays consistent with disk; if the swap somehow
    /// failed afterwards, disk would be ahead but readers would converge on
    /// the next reload. Never the other direction.
    pub fn create_document(&self) -> Result<String, PersistedTreeError> {
        let basename = format!("untitled-{}.md", utils::time::now_micros());

        let mut tree_guard = self
            .tree
            .write()
            .map_err(|e| PersistedTreeError::Lock(e.to_string()))?;
        let absolute_path = PathBuf::from(tree_guard.basedir()).join(&basename);

        self.driver
            .lock()
            .map_err(|e| PersistedTreeError::Lock(e.to_string()))?
            .write(&absolute_path, b"")?;

        let next = tree_guard.with_new_document(basename.clone())?;
        *tree_guard = next;

        Ok(basename)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Vfs;

    #[test]
    fn open_then_get_entries_returns_loaded_doc() {
        let mut fs = Vfs::new();
        fs.write_file("/root/a.md", b"hello".to_vec());

        let pt = PersistedTree::open(fs, Path::new("/root")).unwrap();
        let entry = pt.get_entries("a.md".into()).unwrap();
        assert!(matches!(entry, Entry::File(_)));
    }

    #[test]
    fn create_document_persists_and_returns_resolvable_path() {
        let mut fs = Vfs::new();
        fs.write_file("/root/existing.md", b"".to_vec());

        let pt = PersistedTree::open(fs, Path::new("/root")).unwrap();
        let path = pt.create_document().unwrap();

        assert!(path.starts_with("untitled-"));
        assert!(path.ends_with(".md"));

        // The new doc resolves through the api surface — it's a real entry,
        // not just a stub.
        let new_entry = pt.get_entries(path).unwrap();
        assert!(matches!(new_entry, Entry::File(_)));

        // The original entry still resolves — swap didn't blow it away.
        let existing = pt.get_entries("existing.md".into()).unwrap();
        assert!(matches!(existing, Entry::File(_)));
    }
}
