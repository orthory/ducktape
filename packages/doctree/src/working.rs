use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use crate::{Entry, Tree, TreeError, build_tree, drivers::Driver};

/// `WorkingTree` is the canonical shared in-memory view: a `Mutex<Arc<Tree>>`
/// that every participant in a workspace observes and mutates. There's no
/// per-session fork — when one mutates, every other reader sees the change
/// as soon as the swap is published.
///
/// This is the layer where the MVCC machinery on `Tree` actually pays off:
/// readers grab the `Arc` under a brief lock, drop it, then read lock-free.
/// Writers compute the next `Tree` version, take the lock, and publish via
/// pointer swap. In-flight readers holding the previous `Arc` keep seeing a
/// consistent shape until they're done.
///
/// What lives here:
/// - the canonical `Mutex<Arc<Tree>>` and the read/mutate primitives over it.
///
/// What does *not* live here:
/// - persistence (driver i/o, dirty tracking, commit boundary). That's
///   `Persister`'s job, and it wraps an `Arc<WorkingTree>` to add those
///   concerns without tangling them into the canonical-state type.
pub struct WorkingTree {
    inner: Mutex<Arc<Tree>>,
}

#[derive(thiserror::Error, Debug)]
pub enum WorkingTreeError {
    #[error("WorkingTreeError: {0}")]
    Tree(#[from] TreeError),

    #[error("WorkingTreeError: {0}")]
    Lock(String),
}

impl WorkingTree {
    pub fn new(tree: Tree) -> Self {
        Self {
            inner: Mutex::new(Arc::new(tree)),
        }
    }

    /// Loads a tree from `driver` rooted at `basedir` and wraps it in a
    /// `WorkingTree`. The driver is borrowed only for the duration of the
    /// load — the caller can hand it off afterwards (e.g. into a `Persister`
    /// for writes). `basedir` is similarly only used to walk the driver; the
    /// resulting tree has no path concept and won't remember it.
    pub fn from_persisted(
        driver: &dyn Driver,
        basedir: &Path,
    ) -> Result<Self, WorkingTreeError> {
        let tree = build_tree(driver, basedir)?;
        Ok(Self::new(tree))
    }

    /// Briefly locks to clone the `Arc<Tree>`, then returns it for lock-free
    /// reads. Cheap — clones a refcount, not the tree.
    pub fn snapshot(&self) -> Result<Arc<Tree>, WorkingTreeError> {
        let guard = self
            .inner
            .lock()
            .map_err(|e| WorkingTreeError::Lock(e.to_string()))?;
        Ok(guard.clone())
    }

    /// Resolves an entry by path against the current snapshot. Returns an
    /// owned `Entry` (cloning is cheap — Directory variants share their
    /// children via `Arc`).
    pub fn get_entries(&self, document_path: String) -> Result<Entry, WorkingTreeError> {
        let snapshot = self.snapshot()?;
        let entry = snapshot.get_entries(document_path)?;
        Ok(entry.clone())
    }

    /// Mints a fresh basename, applies it via `Tree::with_new_document`, and
    /// publishes the swap. Returns the minted basename so callers (e.g.
    /// `Persister`) can record it as dirty for later flush.
    ///
    /// This is a pure in-memory mutation — no persistence happens here.
    /// Callers that need disk durability should go through `Persister`.
    pub fn create_document(&self) -> Result<String, WorkingTreeError> {
        let basename = format!("untitled-{}.md", utils::time::now_micros());

        let mut guard = self
            .inner
            .lock()
            .map_err(|e| WorkingTreeError::Lock(e.to_string()))?;
        let next = guard.with_new_document(basename.clone())?;
        *guard = Arc::new(next);

        Ok(basename)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Vfs;
    use std::path::Path;

    fn working_with_seed() -> WorkingTree {
        let mut fs = Vfs::new();
        fs.write_file("/root/seed.md", b"".to_vec());
        WorkingTree::from_persisted(&fs, Path::new("/root")).unwrap()
    }

    #[test]
    fn get_entries_returns_loaded_doc() {
        let wt = working_with_seed();
        let entry = wt.get_entries("seed.md".into()).unwrap();
        assert!(matches!(entry, Entry::File(_)));
    }

    #[test]
    fn create_document_publishes_new_version() {
        let wt = working_with_seed();
        let basename = wt.create_document().unwrap();
        assert!(basename.starts_with("untitled-"));

        let entry = wt.get_entries(basename).unwrap();
        assert!(matches!(entry, Entry::File(_)));
    }

    #[test]
    fn snapshot_survives_concurrent_mutation() {
        // The MVCC property: a snapshot taken before a mutation keeps
        // resolving the pre-mutation shape, even after the mutation
        // publishes its new version.
        let wt = working_with_seed();
        let pre = wt.snapshot().unwrap();

        let new_basename = wt.create_document().unwrap();

        // Pre-mutation snapshot still resolves the seed and does not see
        // the new document.
        assert!(matches!(
            pre.get_entries("seed.md".into()).unwrap(),
            Entry::File(_)
        ));
        assert!(pre.get_entries(new_basename.clone()).is_err());

        // Post-mutation, both are reachable through the live working tree.
        assert!(matches!(
            wt.get_entries("seed.md".into()).unwrap(),
            Entry::File(_)
        ));
        assert!(matches!(
            wt.get_entries(new_basename).unwrap(),
            Entry::File(_)
        ));
    }
}
