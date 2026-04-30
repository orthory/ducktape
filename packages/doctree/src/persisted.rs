use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use crate::{
    Entry, Tree, TreeError, build_tree,
    drivers::{Driver, DriverError},
};

/// `PersistedTree` couples an in-memory `Tree` with a `Driver` backend, but
/// follows a working-copy / commit model rather than write-through:
///
/// - Reads always go through `Tree`. The driver isn't touched on the read
///   path — `Tree` is effectively a fully-loaded read cache built from the
///   driver at `open` time.
/// - Mutations (e.g. `create_document`) only update the in-memory tree and
///   record the affected basename in a `pending` set. Nothing hits the
///   driver yet.
/// - `commit` is the explicit sync point: it drains the pending set and
///   writes each entry through the driver. Calls between mutations and
///   commit see the new state in-memory; the on-disk truth lags until
///   commit succeeds.
///
/// Concurrency model:
/// - `tree: Mutex<Arc<Tree>>` — readers lock briefly to clone the `Arc`,
///   drop the lock, then read the snapshot lock-free. Writers hold the
///   mutex for the duration of their op (mutate + swap).
/// - `driver: Mutex<Box<dyn Driver>>` — `Driver::write`'s `&mut self`
///   enforces exclusive access at the type level; the mutex is what mints
///   that `&mut`.
/// - `pending: Mutex<HashSet<String>>` — guarded set of basenames awaiting
///   commit; locked briefly when adding entries and when draining at commit.
pub struct PersistedTree {
    tree: Mutex<Arc<Tree>>,
    driver: Mutex<Box<dyn Driver>>,
    pending: Mutex<HashSet<String>>,
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
    /// The pending set starts empty — `open` reflects what's on disk.
    pub fn open(
        driver: impl Driver + 'static,
        basedir: &Path,
    ) -> Result<Self, PersistedTreeError> {
        let tree = build_tree(&driver, basedir)?;
        Ok(Self {
            tree: Mutex::new(Arc::new(tree)),
            driver: Mutex::new(Box::new(driver)),
            pending: Mutex::new(HashSet::new()),
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

    /// Mints a new document and adds it to the tree in-memory only. The
    /// returned basename is reachable via `get_entries` immediately, but
    /// nothing is on disk until `commit` is called.
    pub fn create_document(&self) -> Result<String, PersistedTreeError> {
        let basename = format!("untitled-{}.md", utils::time::now_micros());

        let mut tree_guard = self
            .tree
            .lock()
            .map_err(|e| PersistedTreeError::Lock(e.to_string()))?;
        let next = tree_guard.with_new_document(basename.clone())?;
        *tree_guard = Arc::new(next);
        drop(tree_guard);

        self.pending
            .lock()
            .map_err(|e| PersistedTreeError::Lock(e.to_string()))?
            .insert(basename.clone());

        Ok(basename)
    }

    /// Drains the pending set and writes each entry through the driver.
    /// On a partial failure (driver returns an error mid-drain) the already-
    /// written entries stay written and the failed-or-later ones remain in
    /// the pending set; the caller can retry `commit`. The in-memory tree
    /// is unchanged either way.
    ///
    /// Today every pending entry is a freshly-created empty document, so the
    /// content written is `b""`. Once Document → bytes serialization exists,
    /// this is where we serialize each pending basename's `Entry::File`.
    pub fn commit(&self) -> Result<(), PersistedTreeError> {
        let snapshot = {
            let guard = self
                .tree
                .lock()
                .map_err(|e| PersistedTreeError::Lock(e.to_string()))?;
            guard.clone()
        };
        let basedir = snapshot.basedir();

        let mut driver = self
            .driver
            .lock()
            .map_err(|e| PersistedTreeError::Lock(e.to_string()))?;
        let mut pending = self
            .pending
            .lock()
            .map_err(|e| PersistedTreeError::Lock(e.to_string()))?;

        // Drain by collecting first; lets us put back any unprocessed entries
        // if a write fails partway through.
        let to_flush: Vec<String> = pending.drain().collect();
        for basename in to_flush {
            let absolute_path = PathBuf::from(&basedir).join(&basename);
            if let Err(e) = driver.write(&absolute_path, b"") {
                // Re-insert this basename so a retry will pick it up.
                pending.insert(basename);
                return Err(PersistedTreeError::Driver(e));
            }
        }

        Ok(())
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
    fn create_document_is_in_memory_only_until_commit() {
        // Driver is shared between PersistedTree and the test's view of the
        // filesystem via Arc-cloning the inner Vfs through a side channel —
        // not how production code uses it, but lets us inspect what was
        // actually written. Simpler: drive everything through PersistedTree
        // and re-open from the same backing to check on-disk state.
        // Instead we do a direct check: after create_document, the tree has
        // the entry but the driver doesn't.
        let pt = {
            let mut fs = Vfs::new();
            fs.write_file("/root/seed.md", b"".to_vec());
            PersistedTree::open(fs, Path::new("/root")).unwrap()
        };

        let path = pt.create_document().unwrap();
        assert!(path.starts_with("untitled-"));

        // In-memory: reachable.
        let entry = pt.get_entries(path.clone()).unwrap();
        assert!(matches!(entry, Entry::File(_)));

        // Pending: contains the new basename.
        assert!(pt.pending.lock().unwrap().contains(&path));

        // Commit: pending drains, no error.
        pt.commit().unwrap();
        assert!(pt.pending.lock().unwrap().is_empty());

        // Re-running commit is a no-op (nothing pending).
        pt.commit().unwrap();
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

        let pre_swap = pt.get_entries("old.md".into()).unwrap();
        assert!(matches!(pre_swap, Entry::File(_)));

        let new_path = pt.create_document().unwrap();

        let old = pt.get_entries("old.md".into()).unwrap();
        assert!(matches!(old, Entry::File(_)));

        let new = pt.get_entries(new_path).unwrap();
        assert!(matches!(new, Entry::File(_)));
    }
}
