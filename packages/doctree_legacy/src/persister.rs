use std::{
    collections::HashSet,
    path::PathBuf,
    sync::Mutex,
};

use crate::{
    WorkingTree, WorkingTreeError,
    drivers::{Driver, DriverError},
    tree::TreeError,
};

/// `Persister` is the fs-side service for a `WorkingTree`. It owns the disk
/// driver, the basedir (where on disk the tree lives), and the dirty set.
/// It does *not* own the canonical tree state, and it doesn't even hold a
/// handle to one — operations that need the working tree take it as a
/// parameter. This keeps `Persister` a fully passive sink that can be
/// constructed and dropped independently of any `WorkingTree` it happens to
/// service.
///
/// Roles:
/// - Wraps a `Driver` and serializes write access to it.
/// - Holds the `basedir`. The in-memory tree itself has no path concept —
///   path-on-disk is purely a persistence concern, so it lives here.
/// - Tracks the **dirty set**: basenames whose in-memory state has diverged
///   from disk. Conceptually this is the `git status` "modified" list for
///   the working copy.
/// - Exposes a `commit` boundary that flushes the dirty set through the
///   driver. Future hook point for invoking an actual git commit.
///
/// Mutation flow:
/// - The api caller holds both an `Arc<WorkingTree>` and a `Persister`.
/// - For mutations, it can either go through `Persister`'s convenience
///   wrappers (e.g. `create_document`) which apply the working-tree
///   mutation and register the resulting basename as dirty in one call, or
///   mutate the working tree directly and call `mark_dirty` itself.
/// - `commit` drains the dirty set; the working tree is borrowed only for
///   future content serialization.
///
/// Read consumers don't need a `Persister` at all: hold an `Arc<WorkingTree>`
/// directly and you can resolve entries lock-freely without dragging a
/// driver along.
///
/// Explicit non-goals:
/// - No MVCC isolation between editors. The whole point of the split is
///   that there's exactly one canonical view; `Persister` doesn't change
///   that.
/// - Commit is currently all-or-nothing across the dirty set. Per-basename
///   staged commits aren't modeled yet.
/// - No discard / rollback path for dirty entries. When that's needed, the
///   shape will be: keep `last_committed: Arc<Tree>` alongside the working
///   tree's live version and rebuild on discard.
///
/// Concurrency primitives:
/// - `driver: Mutex<Box<dyn Driver>>` — `Driver::write`'s `&mut self`
///   enforces exclusive access at the type level; the mutex is what mints
///   that `&mut`.
/// - `pending: Mutex<HashSet<String>>` — guarded set of basenames awaiting
///   commit; locked briefly when adding entries and when draining at commit.
pub struct Persister {
    driver: Mutex<Box<dyn Driver>>,
    basedir: PathBuf,
    pending: Mutex<HashSet<String>>,
}

#[derive(thiserror::Error, Debug)]
pub enum PersisterError {
    #[error("PersisterError: {0}")]
    Tree(#[from] TreeError),

    #[error("PersisterError: {0}")]
    Working(#[from] WorkingTreeError),

    #[error("PersisterError: {0}")]
    Driver(#[from] DriverError),

    #[error("PersisterError: {0}")]
    Lock(String),
}

impl Persister {
    /// Constructs a fresh `Persister` for the given driver and basedir. The
    /// working tree it'll service is constructed separately; pass it in to
    /// the methods that need it.
    ///
    /// Typical wiring:
    /// ```ignore
    /// let driver = Stdfs;
    /// let basedir = PathBuf::from("/path/to/workspace");
    /// let working = Arc::new(WorkingTree::from_persisted(&driver, &basedir)?);
    /// let persister = Persister::new(driver, basedir);
    /// ```
    pub fn new(driver: impl Driver + 'static, basedir: PathBuf) -> Self {
        Self {
            driver: Mutex::new(Box::new(driver)),
            basedir,
            pending: Mutex::new(HashSet::new()),
        }
    }

    /// Records `basename` in the dirty set. Idempotent — re-marking is a
    /// no-op. Use this when mutating the working tree directly and you want
    /// the result included in the next `commit`.
    pub fn mark_dirty(&self, basename: String) -> Result<(), PersisterError> {
        self.pending
            .lock()
            .map_err(|e| PersisterError::Lock(e.to_string()))?
            .insert(basename);
        Ok(())
    }

    /// Convenience: applies `WorkingTree::create_document` to `working` and
    /// registers the resulting basename as dirty in one call. Returns the
    /// minted basename so callers can use it in subsequent operations.
    /// Nothing hits the driver until `commit`.
    pub fn create_document(
        &self,
        working: &WorkingTree,
        document_path: String,
    ) -> Result<String, PersisterError> {
        let basename = working.create_document(document_path)?;
        self.mark_dirty(basename.clone())?;
        Ok(basename)
    }

    /// Drains the pending set and writes each entry through the driver.
    /// On a partial failure (driver returns an error mid-drain) the already-
    /// written entries stay written and the failed-or-later ones remain in
    /// the pending set; the caller can retry `commit`. The in-memory tree
    /// is unchanged either way.
    ///
    /// Today every pending entry is a freshly-created empty document, so
    /// `working` isn't actually consulted and the content written is `b""`.
    /// Once Document → bytes serialization exists, this is where we'll
    /// serialize each pending basename's `Entry::File` by reading from
    /// `working`'s current snapshot.
    pub fn commit(&self, _working: &WorkingTree) -> Result<(), PersisterError> {
        let mut driver = self
            .driver
            .lock()
            .map_err(|e| PersisterError::Lock(e.to_string()))?;
        let mut pending = self
            .pending
            .lock()
            .map_err(|e| PersisterError::Lock(e.to_string()))?;

        // Drain by collecting first; lets us put back any unprocessed entries
        // if a write fails partway through.
        let to_flush: Vec<String> = pending.drain().collect();
        for basename in to_flush {
            let absolute_path = self.basedir.join(&basename);
            if let Err(e) = driver.write(&absolute_path, b"") {
                // Re-insert this basename so a retry will pick it up.
                pending.insert(basename);
                return Err(PersisterError::Driver(e));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Vfs;

    fn setup(seed_path: &str) -> (WorkingTree, Persister) {
        let mut fs = Vfs::new();
        fs.write_file(seed_path, b"".to_vec());
        let basedir = PathBuf::from("/root");
        let working = WorkingTree::from_persisted(&fs, &basedir).unwrap();
        let persister = Persister::new(fs, basedir);
        (working, persister)
    }

    #[test]
    fn create_document_is_in_memory_only_until_commit() {
        // After create_document, the basename is registered in the pending
        // set but the driver hasn't been written to yet. Commit drains.
        let (working, persister) = setup("/root/seed.md");

        let basename = persister
            .create_document(&working, String::new())
            .unwrap();
        assert!(basename.starts_with("untitled-"));

        // Pending: contains the new basename.
        assert!(persister.pending.lock().unwrap().contains(&basename));

        // Commit: pending drains, no error.
        persister.commit(&working).unwrap();
        assert!(persister.pending.lock().unwrap().is_empty());

        // Re-running commit is a no-op (nothing pending).
        persister.commit(&working).unwrap();
    }

    #[test]
    fn mark_dirty_is_idempotent() {
        let (_working, persister) = setup("/root/seed.md");
        persister.mark_dirty("foo.md".into()).unwrap();
        persister.mark_dirty("foo.md".into()).unwrap();
        assert_eq!(persister.pending.lock().unwrap().len(), 1);
    }

    #[test]
    fn direct_working_mutation_plus_mark_dirty_path() {
        // Caller can mutate the working tree directly and inform the
        // persister via `mark_dirty`. Equivalent to using `create_document`.
        let (working, persister) = setup("/root/seed.md");

        let basename = working.create_document(String::new()).unwrap();
        persister.mark_dirty(basename.clone()).unwrap();

        assert!(persister.pending.lock().unwrap().contains(&basename));
    }
}
