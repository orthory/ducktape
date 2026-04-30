use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crate::{
    WorkingTree, WorkingTreeError,
    drivers::{Driver, DriverError},
    tree::TreeError,
};

/// `Persister` is the fs-side service for a `WorkingTree`. It owns the disk
/// driver, the basedir (where on disk the tree lives), and the dirty set;
/// it does *not* own the canonical tree state — that lives in `WorkingTree`,
/// which is held behind an `Arc` and freely shareable with read-only
/// consumers.
///
/// `Persister` is glued to a `WorkingTree` externally: the api caller builds
/// a `WorkingTree` first (e.g. via `WorkingTree::from_persisted`), then
/// hands an `Arc` clone to `Persister::new` along with the driver and
/// basedir. `Persister` plays no role in tree construction.
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
/// Mutations flow through `Persister` so that the working-tree update and
/// the dirty-set registration happen together — callers that go straight to
/// `WorkingTree` get pure in-memory mutation with no persistence guarantee,
/// which is appropriate for tests and ephemeral views but not for the api
/// path.
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
/// - `working: Arc<WorkingTree>` — the canonical state. `WorkingTree` holds
///   its own `Mutex<Arc<Tree>>` for atomic version swaps and lock-free
///   reads.
/// - `driver: Mutex<Box<dyn Driver>>` — `Driver::write`'s `&mut self`
///   enforces exclusive access at the type level; the mutex is what mints
///   that `&mut`.
/// - `pending: Mutex<HashSet<String>>` — guarded set of basenames awaiting
///   commit; locked briefly when adding entries and when draining at commit.
pub struct Persister {
    working: Arc<WorkingTree>,
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
    /// Glues the fs side onto an already-constructed `WorkingTree`. Takes
    /// ownership of the driver (it'll be used for writes) and remembers
    /// `basedir` so `commit` can compute absolute write paths.
    ///
    /// Typical wiring:
    /// ```ignore
    /// let driver = Stdfs;
    /// let basedir = PathBuf::from("/path/to/workspace");
    /// let working = Arc::new(WorkingTree::from_persisted(&driver, &basedir)?);
    /// let persister = Persister::new(working.clone(), driver, basedir);
    /// ```
    pub fn new(
        working: Arc<WorkingTree>,
        driver: impl Driver + 'static,
        basedir: PathBuf,
    ) -> Self {
        Self {
            working,
            driver: Mutex::new(Box::new(driver)),
            basedir,
            pending: Mutex::new(HashSet::new()),
        }
    }

    /// Hands out the canonical `WorkingTree` for read consumers. Cloning the
    /// `Arc` is cheap; safe to share widely.
    pub fn working(&self) -> Arc<WorkingTree> {
        self.working.clone()
    }

    /// Mints a new document on the working tree and registers its basename
    /// as dirty. The new entry is observable via `working` immediately;
    /// nothing hits the driver until `commit`.
    pub fn create_document(&self) -> Result<String, PersisterError> {
        let basename = self.working.create_document()?;

        self.pending
            .lock()
            .map_err(|e| PersisterError::Lock(e.to_string()))?
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
    pub fn commit(&self) -> Result<(), PersisterError> {
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
    use crate::{Entry, Vfs};

    fn setup(seed_path: &str) -> Persister {
        let mut fs = Vfs::new();
        fs.write_file(seed_path, b"".to_vec());
        let basedir = PathBuf::from("/root");
        let working = Arc::new(WorkingTree::from_persisted(&fs, &basedir).unwrap());
        Persister::new(working, fs, basedir)
    }

    #[test]
    fn read_via_working_returns_loaded_doc() {
        let mut fs = Vfs::new();
        fs.write_file("/root/a.md", b"hello".to_vec());

        let basedir = PathBuf::from("/root");
        let working = Arc::new(WorkingTree::from_persisted(&fs, &basedir).unwrap());
        let p = Persister::new(working, fs, basedir);

        let entry = p.working().get_entries("a.md".into()).unwrap();
        assert!(matches!(entry, Entry::File(_)));
    }

    #[test]
    fn create_document_is_in_memory_only_until_commit() {
        // After create_document, the entry is reachable via the working tree
        // but the driver hasn't been written to yet — the basename sits in
        // the pending set until `commit` drains it.
        let p = setup("/root/seed.md");

        let basename = p.create_document().unwrap();
        assert!(basename.starts_with("untitled-"));

        // In-memory: reachable via working tree.
        let entry = p.working().get_entries(basename.clone()).unwrap();
        assert!(matches!(entry, Entry::File(_)));

        // Pending: contains the new basename.
        assert!(p.pending.lock().unwrap().contains(&basename));

        // Commit: pending drains, no error.
        p.commit().unwrap();
        assert!(p.pending.lock().unwrap().is_empty());

        // Re-running commit is a no-op (nothing pending).
        p.commit().unwrap();
    }

    #[test]
    fn working_arc_is_shared_with_persister() {
        // The Arc<WorkingTree> handed out by `working()` is the same one the
        // persister holds — mutations through the persister are visible to
        // anyone holding an external clone.
        let p = setup("/root/seed.md");
        let external = p.working();

        let new_basename = p.create_document().unwrap();
        let entry = external.get_entries(new_basename).unwrap();
        assert!(matches!(entry, Entry::File(_)));
    }

}
