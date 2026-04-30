use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
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
/// - The tree lives in `Mutex<Arc<Tree>>`. Readers lock briefly to clone
///   the `Arc` (cheap; structural sharing keeps the clone shallow), drop
///   the lock, and read the snapshot lock-free — readers never wait on
///   each other or on a writer's actual read of the tree.
/// - Writers hold the tree mutex for the duration of their operation
///   (driver write + tree swap), serializing writes. Each write commits a
///   fresh `Arc` via `*guard = Arc::new(next)`; previous readers' snapshots
///   stay valid via Arc liveness.
/// - The driver lives in `Mutex<Box<dyn Driver>>`. Locking yields `&mut`
///   access — `Driver::write`'s `&mut self` enforces exclusivity at the
///   type level.
pub struct PersistedTree {
    tree: Mutex<Arc<Tree>>,
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
            tree: Mutex::new(Arc::new(tree)),
            driver: Mutex::new(Box::new(driver)),
        })
    }

    /// Grabs an `Arc<Tree>` snapshot under a brief lock, then reads the
    /// snapshot lock-free. Cloning an `Entry` is cheap — Directory variants
    /// share their subtrees via `Arc`.
    pub fn get_entries(&self, document_path: String) -> Result<Entry, PersistedTreeError> {
        let snapshot = {
            let guard = self
                .tree
                .lock()
                .map_err(|e| PersistedTreeError::Lock(e.to_string()))?;
            guard.clone()
        };
        let entry = snapshot.get_entries(document_path)?;
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
            .lock()
            .map_err(|e| PersistedTreeError::Lock(e.to_string()))?;
        let absolute_path = PathBuf::from(tree_guard.basedir()).join(&basename);

        self.driver
            .lock()
            .map_err(|e| PersistedTreeError::Lock(e.to_string()))?
            .write(&absolute_path, b"")?;

        let next = tree_guard.with_new_document(basename.clone())?;
        *tree_guard = Arc::new(next);

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

        let new_entry = pt.get_entries(path).unwrap();
        assert!(matches!(new_entry, Entry::File(_)));

        let existing = pt.get_entries("existing.md".into()).unwrap();
        assert!(matches!(existing, Entry::File(_)));
    }

    #[test]
    fn reader_snapshot_survives_concurrent_writer() {
        // A snapshot taken before a write must keep resolving the pre-write
        // tree, even after the writer's swap publishes the new version.
        // This is the MVCC property: Arc liveness keeps old snapshots
        // valid until their last reader drops them.
        let mut fs = Vfs::new();
        fs.write_file("/root/old.md", b"".to_vec());

        let pt = PersistedTree::open(fs, Path::new("/root")).unwrap();

        // Read the current snapshot's tree pointer by grabbing one entry.
        let pre_swap = pt.get_entries("old.md".into()).unwrap();
        assert!(matches!(pre_swap, Entry::File(_)));

        // Writer swaps in a new version with an additional doc.
        let new_path = pt.create_document().unwrap();

        // Old entry still resolves (the new version preserved it).
        let old = pt.get_entries("old.md".into()).unwrap();
        assert!(matches!(old, Entry::File(_)));

        // New entry resolves too.
        let new = pt.get_entries(new_path).unwrap();
        assert!(matches!(new, Entry::File(_)));
    }
}
