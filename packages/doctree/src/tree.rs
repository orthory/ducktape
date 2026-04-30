use std::{path::Path, sync::Arc};

use document::Document;

use crate::{
    drivers::{Driver, DriverResult},
    entry::Entry,
};

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

// `Tree` is structurally shared and cheap to clone: `root` is an `Arc<Entry>`
// (subtrees are also Arc'd inside the Directory variant) and `driver` is an
// `Arc<dyn Driver>`. Mutations produce a new `Tree` rather than modifying in
// place, so versions don't interfere with concurrent readers — the API surface
// is shaped for MVCC even though the current swap is just a write-locked
// pointer replacement at the api layer.
#[derive(Clone)]
pub struct Tree {
    basedir: String,
    root: Arc<Entry>,
    driver: Arc<dyn Driver>,
}

impl Tree {
    pub fn new(basedir: &Path, driver: impl Driver + 'static) -> Result<Self, TreeError> {
        let basedir_as_string = basedir.to_string_lossy().to_string();
        let driver: Arc<dyn Driver> = Arc::new(driver);

        Ok(Self {
            root: build_in_recursion(&basedir_as_string, &basedir_as_string, &*driver, 0, 10)?,
            basedir: basedir_as_string,
            driver,
        })
    }

    pub fn basedir(&self) -> String {
        self.basedir.clone()
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
    // swap is published. Cost is roughly O(direct_root_children) — only the
    // root Vec is reallocated; sibling subtrees are ref-bumped, not copied.
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
                driver: self.driver.clone(),
            },
            temp_path,
        ))
    }
}

fn build_in_recursion(
    base_path: &String,
    load_path: &String,
    driver: &dyn Driver,
    current_depth: usize,
    max_depth: usize,
) -> Result<Arc<Entry>, TreeError> {
    let load_result = driver
        .load(Path::new(load_path))
        .map_err(|e| TreeError::Invariant(e.into()))?;
    let next_entry = match load_result {
        DriverResult::Skip => Entry::None,
        DriverResult::File(_, reader) => Entry::File(
            Document::from_reader(reader)
                .map_err(|e| TreeError::DocBuilder(anyhow::Error::msg(e.to_string())))?,
        ),
        DriverResult::Directory(_, path_bufs) => {
            let descendants: Result<Vec<(String, Arc<Entry>)>, TreeError> = path_bufs
                .iter()
                .map(|descendant_path| {
                    let descendant_path_as_string = descendant_path.to_string_lossy().to_string();
                    match build_in_recursion(
                        base_path,
                        &descendant_path_as_string,
                        driver,
                        current_depth + 1,
                        max_depth,
                    ) {
                        Ok(entry) => {
                            let relative_path: Vec<&str> = Path::new(&descendant_path_as_string)
                                .strip_prefix(base_path)
                                .map_err(|e| TreeError::InvalidPathSegment(e.to_string()))?
                                .iter()
                                .map(|x| x.to_str().unwrap())
                                .collect();

                            let first_segment = relative_path[relative_path.len() - 1];

                            Ok((first_segment.to_string(), entry))
                        }
                        Err(e) => Err(e),
                    }
                })
                .collect();

            Entry::Directory(descendants?)
        }
    };

    Ok(Arc::new(next_entry))
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
    use crate::drivers::vfs::Vfs;

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
            driver: Arc::new(Vfs::new()),
        }
    }

    #[test]
    fn tree_builds_from_vfs_fixture() {
        let fs = Vfs::new();
        fs.write_file("/root/a/aa/aaa.md", b"hello".to_vec());
        fs.write_file("/root/b/bb/bbb.md", b"world".to_vec());

        let tree = Tree::new(Path::new("/root"), fs).unwrap();

        let aaa = tree.get_entries("a/aa/aaa.md".into()).unwrap();
        assert!(matches!(aaa, Entry::File(_)));

        let bbb = tree.get_entries("b/bb/bbb.md".into()).unwrap();
        assert!(matches!(bbb, Entry::File(_)));
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
        let fs = Vfs::new();
        fs.write_file("/root/existing.md", b"".to_vec());
        let original = Tree::new(Path::new("/root"), fs).unwrap();

        // Snapshot the original's root pointer; after create_document, the
        // original must still see the same root (no in-place mutation).
        let original_root_ptr = Arc::as_ptr(&original.root);

        let (next, path) = original.create_document().unwrap();

        assert_eq!(path, "/tmp");
        assert_eq!(Arc::as_ptr(&original.root), original_root_ptr);
        assert!(!Arc::ptr_eq(&original.root, &next.root));

        // The new version contains the new entry; the original does not.
        let Entry::Directory(orig_items) = original.root.as_ref() else {
            panic!("original root should be a Directory");
        };
        let Entry::Directory(next_items) = next.root.as_ref() else {
            panic!("next root should be a Directory");
        };
        assert_eq!(orig_items.len() + 1, next_items.len());
    }
}
