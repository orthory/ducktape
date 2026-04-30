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

// `Tree` is a pure in-memory data structure — no persistence concerns. It's
// structurally shared and cheap to clone: `root` is an `Arc<Entry>` whose
// Directory variants also Arc their children. Mutations produce a new `Tree`
// version; the persistence layer (driver crate) wraps Tree in `PersistedTree`
// to coordinate with a `Driver` backend.
#[derive(Clone)]
pub struct Tree {
    basedir: String,
    root: Arc<Entry>,
}

impl Tree {
    pub fn new(basedir: String, root: Arc<Entry>) -> Self {
        Self { basedir, root }
    }

    pub fn basedir(&self) -> String {
        self.basedir.clone()
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

    // Returns a new `Tree` containing the new document, plus its identifier.
    // Takes `&self` rather than `&mut self`: the caller owns the version-swap,
    // and concurrent readers continue to see the previous snapshot until the
    // swap is published. Cost is O(direct_root_children) — only the root Vec
    // is reallocated; sibling subtrees are ref-bumped, not copied.
    pub fn create_document(&self) -> Result<(Self, String), TreeError> {
        // Bump the refcount on the existing root, then `Arc::make_mut` clones
        // it (because `self` still holds a reference, count is >1). The clone
        // is shallow: the Vec is freshly allocated, but each child `Arc<Entry>`
        // inside it is just a refcount bump — untouched subtrees are shared
        // with the previous version.
        let mut next_root_arc = self.root.clone();
        let next_root = Arc::make_mut(&mut next_root_arc);

        let Entry::Directory(items) = next_root else {
            return Err(TreeError::InvalidEntry(
                "tried to create a new document, but root isn't a directory".to_string(),
            ));
        };

        let temp_path: String = "/tmp".into();
        items.push((
            temp_path.clone(),
            Arc::new(Entry::File(Document::default())),
        ));

        Ok((
            Self {
                basedir: self.basedir.clone(),
                root: next_root_arc,
            },
            temp_path,
        ))
    }
}

fn search_in_recursion<'search>(
    path_components: Vec<&str>,
    current: &'search Entry,
) -> Result<&'search Entry, TreeError> {
    match current {
        Entry::None => Err(TreeError::InvalidEntry("???".to_string())),
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
            basedir: "/".into(),
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
    fn create_document_returns_new_version_without_mutating_original() {
        let original = create_test_doctree();
        let original_root_ptr = Arc::as_ptr(&original.root);

        let (next, path) = original.create_document().unwrap();

        assert_eq!(path, "/tmp");
        assert_eq!(Arc::as_ptr(&original.root), original_root_ptr);
        assert!(!Arc::ptr_eq(&original.root, &next.root));

        let Entry::Directory(orig_items) = original.root.as_ref() else {
            panic!("original root should be a Directory");
        };
        let Entry::Directory(next_items) = next.root.as_ref() else {
            panic!("next root should be a Directory");
        };
        assert_eq!(orig_items.len() + 1, next_items.len());
    }
}
