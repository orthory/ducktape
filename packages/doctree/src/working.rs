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
///   `Persister`'s job. `Persister` doesn't hold a `WorkingTree` handle —
///   the api caller owns the canonical `Arc<WorkingTree>` and passes it in
///   to `Persister` operations that need it (e.g. `commit`).
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

    /// Mints a fresh basename, joins it with `document_path` to form the
    /// canonical entry key, applies the addition via `Tree::with_new_document`,
    /// and publishes the swap. Returns the minted basename so callers
    /// (e.g. `Persister`) can record it as dirty for later flush.
    ///
    /// This is a pure in-memory mutation — no persistence happens here.
    /// Callers that need disk durability should go through `Persister`.
    pub fn create_document(&self, document_path: String) -> Result<String, WorkingTreeError> {
        let basename = format!("untitled-{}.md", utils::time::now_micros());

        let mut guard = self
            .inner
            .lock()
            .map_err(|e| WorkingTreeError::Lock(e.to_string()))?;
        let canonical_file_path = Path::new(&basename).join(document_path).display().to_string();
        let next = guard.with_new_document(canonical_file_path)?;
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

    fn root_child_count(wt: &WorkingTree) -> usize {
        match wt.snapshot().unwrap().root().as_ref() {
            Entry::Directory(items) => items.len(),
            _ => panic!("root should be a Directory"),
        }
    }

    #[test]
    fn create_document_publishes_new_version() {
        let wt = working_with_seed();
        let pre_count = root_child_count(&wt);

        let basename = wt.create_document(String::new()).unwrap();
        assert!(basename.starts_with("untitled-"));

        // The new entry is published to the working tree.
        assert_eq!(root_child_count(&wt), pre_count + 1);
    }

    #[test]
    fn snapshot_survives_concurrent_mutation() {
        // The MVCC property: a snapshot taken before a mutation keeps
        // resolving the pre-mutation shape, even after the mutation
        // publishes its new version.
        let wt = working_with_seed();
        let pre = wt.snapshot().unwrap();

        wt.create_document(String::new()).unwrap();

        // Pre-mutation snapshot still resolves the seed and only knows about
        // pre-mutation children.
        assert!(matches!(
            pre.get_entries("seed.md".into()).unwrap(),
            Entry::File(_)
        ));
        let Entry::Directory(pre_items) = pre.root().as_ref() else {
            panic!("root should be a Directory");
        };
        assert_eq!(pre_items.len(), 1);

        // The live working tree sees the new entry.
        assert_eq!(root_child_count(&wt), 2);
    }
}
