use std::sync::Arc;

use document::Document;

use crate::entry::Entry;

#[derive(thiserror::Error, Debug)]
pub enum TreeError {
    #[error("TreeError: {0}")]
    Invariant(anyhow::Error),

    #[error("TreeError: error during docbuild: {0}")]
    DocBuilder(anyhow::Error),

    #[error("TreeError: invalid entry: {0}")]
    InvalidEntry(String),

    #[error("TreeError: entry not found: {0}")]
    NotFound(String),

    #[error("TreeError: invalid path segment found: {0}")]
    InvalidPathSegment(String),
}

// `Tree` is a pure in-memory data structure — no persistence concerns, no
// path/basedir, no concurrency primitives. It's structurally shared and
// cheap to clone: `root` is an `Arc<Entry>` whose Directory variants also
// Arc their children. Mutations produce a new `Tree` version; `WorkingTree`
// wraps `Mutex<Arc<Tree>>` to coordinate canonical shared state, and
// `Persister` wraps `WorkingTree` to add disk-side concerns (driver, dirty
// set, basedir, commit boundary).
//
// The MVCC shape (immutability + `with_X` builders + structural sharing) is
// here for atomic version swap, lock-free reads, and in-flight read
// consistency across concurrent writes — *not* for writer isolation.
// `WorkingTree` deliberately exposes a single canonical view to all
// participants, so the new version produced by `with_new_document` is
// expected to be published immediately, not stashed as a private fork.
// Cheap snapshotting also makes future features (e.g. retaining a
// `last_committed` snapshot, time-travel over commit history) essentially
// free thanks to subtree sharing.
#[derive(Clone)]
pub struct Tree {
    root: Arc<Entry>,
}

impl Tree {
    pub fn new(root: Arc<Entry>) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Arc<Entry> {
        &self.root
    }

    pub fn get_entries(&self, document_path: String) -> Result<&Entry, TreeError> {
        search_in_recursion(
            document_path
                .split("/")
                .filter(|seg| !seg.is_empty())
                .collect(),
            self.root.as_ref(),
        )
    }

    // Returns a new `Tree` with `path` registered as a child of the root.
    // Takes `&self` rather than `&mut self`: the caller owns the version-swap,
    // and concurrent readers continue to see the previous snapshot until the
    // swap is published. Cost is O(direct_root_children) — only the root Vec
    // is reallocated; sibling subtrees are ref-bumped, not copied.
    //
    // The path here is the basename used as the lookup key (matches the
    // convention used at each level: entries are keyed by basename).
    pub fn with_new_document(&self, path: String) -> Result<Self, TreeError> {
        // Bump the refcount on the existing root, then `Arc::make_mut` clones
        // it (because `self` still holds a reference, count is >1). The clone
        // is shallow: the Vec is freshly allocated, but each child `Arc<Entry>`
        // inside it is just a refcount bump — untouched subtrees are shared
        // with the previous version.
        let mut next_root_arc = self.root.clone();
        let next_root = Arc::make_mut(&mut next_root_arc);

        let Entry::Directory(items) = next_root else {
            return Err(TreeError::InvalidEntry(
                "tried to add a document, but root isn't a directory".to_string(),
            ));
        };

        items.push((path, Arc::new(Entry::File(Document::default()))));

        Ok(Self {
            root: next_root_arc,
        })
    }
}

fn search_in_recursion<'search>(
    path_components: Vec<&str>,
    current: &'search Entry,
) -> Result<&'search Entry, TreeError> {
    match current {
        Entry::File(_) => Ok(current),
        Entry::Directory(items) => {
            let (_, next_current) = items
                .iter()
                .find(|(pb, _)| pb == path_components[0])
                .ok_or_else(|| TreeError::NotFound(path_components.join("/").to_string()))?;

            let next_path: Vec<&str> = path_components.into_iter().skip(1).collect();
            if next_path.is_empty() {
                return Ok(next_current.as_ref());
            }
            search_in_recursion(next_path, next_current.as_ref())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_doctree() -> Tree {
        Tree {
            root: Arc::new(Entry::Directory(vec![
                (
                    "a".into(),
                    Arc::new(Entry::Directory(vec![(
                        "aa".into(),
                        Arc::new(Entry::Directory(vec![(
                            "aaa".into(),
                            Arc::new(Entry::File(Document::default())),
                        )])),
                    )])),
                ),
                (
                    "b".into(),
                    Arc::new(Entry::Directory(vec![(
                        "bb".into(),
                        Arc::new(Entry::Directory(vec![(
                            "bbb".into(),
                            Arc::new(Entry::File(Document::default())),
                        )])),
                    )])),
                ),
            ])),
        }
    }

    #[test]
    fn search_in_recursion_works() {
        let test_doc_tree = create_test_doctree();

        let found =
            search_in_recursion(vec!["a", "aa", "aaa"], test_doc_tree.root.as_ref()).unwrap();
        assert!(matches!(found, Entry::File(_)));
    }

    #[test]
    fn with_new_document_returns_new_version_without_mutating_original() {
        let original = create_test_doctree();
        let original_root_ptr = Arc::as_ptr(&original.root);

        let next = original.with_new_document("c.md".into()).unwrap();

        assert_eq!(Arc::as_ptr(&original.root), original_root_ptr);
        assert!(!Arc::ptr_eq(&original.root, &next.root));

        let Entry::Directory(orig_items) = original.root.as_ref() else {
            panic!("original root should be a Directory");
        };
        let Entry::Directory(next_items) = next.root.as_ref() else {
            panic!("next root should be a Directory");
        };
        assert_eq!(orig_items.len() + 1, next_items.len());

        // The new doc is reachable by the basename it was added under.
        let found = next.get_entries("c.md".into()).unwrap();
        assert!(matches!(found, Entry::File(_)));
    }
}
