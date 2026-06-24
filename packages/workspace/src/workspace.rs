use std::{path::Path, sync::Arc};

use document::Document;

use crate::{build, drivers::Driver, entry::Entry};

#[derive(Clone)]
pub struct Workspace {
    root: Arc<Entry>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("d: {0}")]
    BuildError(build::Error),

    #[error("TreeError: invalid entry: {0}")]
    InvalidEntry(String),

    #[error("TreeError: entry not found: {0}")]
    NotFound(String),

    #[error("TreeError: invalid path segment found: {0}")]
    InvalidPathSegment(String),
}

impl Workspace {
    pub fn new_from_path(driver: &dyn Driver, basedir: &Path) -> Result<Self, Error> {
        match build::build_from_basedir(driver, basedir).map_err(|e| Error::BuildError(e))? {
            Some(root) => Ok(Self { root: Arc::new(root) }),
            None => Ok(Self { root: Arc::new(Entry::Directory(Default::default())) }),
        }
    }

    pub fn new_from_entry(root: Entry) -> Self {
        Self { root: Arc::new(root) }
    }

    pub fn root(&self) -> &Arc<Entry> {
        &self.root
    }

    pub fn get_entries(&self, document_path: String) -> Result<&Entry, Error> {
        search_in_recursion(
            document_path
                .split("/")
                .filter(|seg| !seg.is_empty())
                .collect(),
            self.root.as_ref(),
        )
    }

    pub fn add_temporary_entry(&self, path: String) -> Result<Self, Error> {
        let mut next_root_arc = self.root.clone();
        let next_root = Arc::make_mut(&mut next_root_arc);

        let Entry::Directory(items) = next_root else {
            return Err(Error::InvalidEntry(
                "tried to add a document, but root isn't a directory".to_string(),
            ));
        };

        items.push((path, Entry::File(Document::default())));

        Ok(Self {
            root: next_root_arc,
        })
    }

    pub fn promote_temporary_entry(&self, uid: uid::Uid) {

    }
}

fn search_in_recursion<'search>(
    path_components: Vec<&str>,
    current: &'search Entry,
) -> Result<&'search Entry, Error> {
    match current {
        Entry::File(_) => Ok(current),
        Entry::Directory(items) => {
            let (_, next_current) = items
                .iter()
                .find(|(pb, _)| pb == path_components[0])
                .ok_or_else(|| Error::NotFound(path_components.join("/").to_string()))?;

            let next_path: Vec<&str> = path_components.into_iter().skip(1).collect();
            if next_path.is_empty() {
                return Ok(next_current);
            }
            search_in_recursion(next_path, next_current)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_doctree() -> Workspace {
        Workspace {
            root: Arc::new(Entry::Directory(vec![
                (
                    "a".into(),
                    Entry::Directory(vec![(
                        "aa".into(),
                        Entry::Directory(vec![(
                            "aaa".into(),
                            Entry::File(Document::default()),
                        )]),
                    )]),
                ),
                (
                    "b".into(),
                    Entry::Directory(vec![(
                        "bb".into(),
                        Entry::Directory(vec![(
                            "bbb".into(),
                            Entry::File(Document::default()),
                        )]),
                    )]),
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
    fn add_temporary_entry_returns_new_version_without_mutating_original() {
        let original = create_test_doctree();
        let original_root_ptr = Arc::as_ptr(&original.root);

        let next = original.add_temporary_entry("c.md".into()).unwrap();

        assert_eq!(Arc::as_ptr(&original.root), original_root_ptr);
        assert!(!Arc::ptr_eq(&original.root, &next.root));

        let Entry::Directory(orig_items) = original.root.as_ref() else {
            panic!("original root should be a Directory");
        };
        let Entry::Directory(next_items) = next.root.as_ref() else {
            panic!("next root should be a Directory");
        };
        assert_eq!(orig_items.len() + 1, next_items.len());

        let found = next.get_entries("c.md".into()).unwrap();
        assert!(matches!(found, Entry::File(_)));
    }
}
