use std::{fs::File, path::Path};

use crate::{
    drivers::{DriverError, DriverResult},
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

pub struct Tree<Doc> {
    basedir: String,
    root: Entry<Doc>,
}

type Loader = fn(&Path) -> Result<DriverResult, DriverError>;
type DocBuild<Doc> = fn(File) -> Result<Doc, anyhow::Error>;

impl<Doc: Default> Tree<Doc> {
    pub fn new(
        basedir: &Path,
        load: Loader,
        doc_builder: DocBuild<Doc>,
    ) -> Result<Self, TreeError> {
        let basedir_as_string = basedir.to_string_lossy().to_string();

        Ok(Self {
            root: build_in_recursion(
                &basedir_as_string,
                &basedir_as_string,
                load,
                doc_builder,
                0,
                10,
            )?,
            basedir: basedir_as_string,
        })
    }

    pub fn basedir(&self) -> String {
        self.basedir.clone()
    }

    pub fn get_entries(&self, document_path: String) -> Result<&Entry<Doc>, TreeError> {
        search_in_recursion(
            document_path
                .split("/")
                .filter(|seg| !seg.is_empty())
                .collect(),
            &self.root,
        )
    }

    // create_document creates new document at the root,
    // and returns its identifier
    pub fn create_document(&mut self) -> Result<String, TreeError> {
        let Entry::Directory(root) = &mut self.root else {
            return Err(TreeError::InvalidEntry(
                "tried to create a new document, but root isn't a directory".to_string(),
            ));
        };

        let temp_path: String = "/tmp".into();
        let temp_doc: Doc = Default::default();
        let temp_entry: Entry<Doc> = Entry::File(temp_doc);

        root.push((temp_path.clone(), temp_entry));

        Ok(temp_path)
    }
}

fn build_in_recursion<'build, Doc>(
    base_path: &String,
    load_path: &String,
    load: Loader,
    doc_builder: DocBuild<Doc>,
    current_depth: usize,
    max_depth: usize,
) -> Result<Entry<Doc>, TreeError> {
    let load_result = load(Path::new(load_path)).map_err(|e| TreeError::Invariant(e.into()))?;
    let next_entry = match load_result {
        // todo: what is this?
        DriverResult::Skip => Entry::None,
        DriverResult::File(_, file) => {
            Entry::File(doc_builder(file).map_err(|e| TreeError::DocBuilder(e))?)
        }
        DriverResult::Directory(_, path_bufs) => {
            let descendants: Result<Vec<(String, Entry<Doc>)>, TreeError> = path_bufs
                .iter()
                .map(|descendant_path| {
                    // in case of directory, recursively all the way to children
                    // while adjusting base_dir to the current path
                    let descendant_path_as_string = descendant_path.to_string_lossy().to_string();
                    match build_in_recursion(
                        base_path,
                        &descendant_path_as_string,
                        load,
                        doc_builder,
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

    eprintln!("building tree done");

    Ok(next_entry)
}

fn search_in_recursion<'search, Doc>(
    path_components: Vec<&str>,
    current: &'search Entry<Doc>,
) -> Result<&'search Entry<Doc>, TreeError> {
    dbg!(&path_components);
    match current {
        Entry::None => return Err(TreeError::InvalidEntry("???".to_string())),
        Entry::File(_) => Ok(current),
        Entry::Directory(items) => {
            let (_, next_current) = items
                .iter()
                .find(|(pb, _)| pb == path_components[0])
                .ok_or_else(|| TreeError::NotFound(path_components.join("/").to_string()))?;

            let next_path: Vec<&str> = path_components.into_iter().skip(1).collect();
            if next_path.len() == 0 {
                return Ok(next_current);
            }
            search_in_recursion(next_path, next_current)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Read};

    use crate::drivers;

    use super::*;

    fn create_test_doctree() -> Tree<String> {
        Tree {
            basedir: "/".into(),
            root: Entry::Directory(vec![
                (
                    "a".into(),
                    Entry::Directory(vec![(
                        "aa".into(),
                        Entry::Directory(vec![("aaa".into(), Entry::File("hello".to_string()))]),
                    )]),
                ),
                (
                    "b".into(),
                    Entry::Directory(vec![(
                        "bb".into(),
                        Entry::Directory(vec![("bbb".into(), Entry::File("world".to_string()))]),
                    )]),
                ),
            ]),
        }
    }

    #[test]
    fn build_in_recursion_works() {
        let cwd = std::env::current_dir()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let docbuilder: DocBuild<String> = |f| {
            let mut res: String = Default::default();
            let mut rb = BufReader::new(f);
            rb.read_to_string(&mut res)
                .map_err(|e| anyhow::anyhow!(e))?;
            Ok(res)
        };

        let yee = build_in_recursion(&cwd, drivers::stdfs::load, docbuilder, 0, 20).unwrap();

        dbg!(yee);
    }

    #[test]
    fn search_in_recursion_works() {
        let test_doc_tree = create_test_doctree();

        {
            // case found
            let found = search_in_recursion(vec!["a", "aa", "aaa"], &test_doc_tree.root).unwrap();
            match found {
                Entry::File(f) => assert_eq!(*f, "hello".to_string()),
                _ => panic!("expected found"),
            }
        }
    }
}
