use std::{fs::File, path::PathBuf};

use identifier::{Identifiable, Identifier};

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
}

pub struct Tree<Doc> {
    basedir: PathBuf,
    root: Entry<Doc>,
}

type Loader = fn(&PathBuf) -> Result<DriverResult, DriverError>;
type DocBuild<Doc> = fn(File) -> Result<Doc, anyhow::Error>;

impl<Doc: Default + Identifiable> Tree<Doc> {
    pub fn new(
        basedir: &PathBuf,
        load: Loader,
        doc_builder: DocBuild<Doc>,
    ) -> Result<Self, TreeError> {
        Ok(Self {
            basedir: basedir.clone(),
            root: build_in_recursion(basedir, load, doc_builder, 0, 10)?,
        })
    }

    pub fn get_document(&self, document_path: PathBuf) {}

    // create_document creates new document at the root,
    // and returns its identifier
    pub fn create_document(&mut self) -> Result<Identifier, TreeError> {
        let Entry::Directory(root) = &mut self.root else {
            return Err(TreeError::InvalidEntry(
                "tried to create a new document, but root isn't a directory".to_string(),
            ));
        };

        let temp_path: PathBuf = "/..".into();
        let temp_doc: Doc = Default::default();
        let Some(document_id) = temp_doc.identifier() else {
            return Err(TreeError::InvalidEntry(
                "tried to create a new document, but got a wrong identifier".to_string(),
            ));
        };
        let temp_entry: Entry<Doc> = Entry::File(temp_doc);

        root.push((temp_path, temp_entry));

        Ok(document_id)
    }
}

fn search_in_recursion<'app, Doc>(
    path: &PathBuf,
    load: Loader,
    root: &'app Entry<Doc>,
) -> Result<&'app Entry<Doc>, TreeError> {
    let mut current = root;
    for segment in path {
        match &current {
            &Entry::Directory(d) => {
                let matching_descendant = d.iter().find(|(pb, _)| pb.starts_with(segment));
                match matching_descendant {
                    Some(matching_descendant) => current = &matching_descendant.1,
                    None => return Err(TreeError::NotFound(path.to_string_lossy().to_string())),
                }
            }
            &Entry::File(f) => current = &Entry::File(*f),
            _ => return Err(TreeError::InvalidEntry("???".to_string())),
        };
    }

    Ok(current)
}

pub fn build_in_recursion<Doc>(
    path: &PathBuf,
    load: Loader,
    doc_builder: DocBuild<Doc>,
    current_depth: usize,
    max_depth: usize,
) -> Result<Entry<Doc>, TreeError> {
    eprintln!("building tree ({})", path.to_string_lossy().to_string());
    let load_result = load(path).map_err(|e| TreeError::Invariant(e.into()))?;
    let next_entry = match load_result {
        DriverResult::File(_, file) => {
            Entry::File(doc_builder(file).map_err(|e| TreeError::DocBuilder(e))?)
        }
        DriverResult::Directory(_, path_bufs) => {
            let descendants: Result<Vec<(PathBuf, Entry<Doc>)>, TreeError> = path_bufs
                .iter()
                .map(|descendant_path| {
                    match build_in_recursion(
                        descendant_path,
                        load,
                        doc_builder,
                        current_depth + 1,
                        max_depth,
                    ) {
                        Ok(entry) => Ok((descendant_path.clone(), entry)),
                        Err(e) => Err(e),
                    }
                })
                .collect();

            Entry::Directory(descendants?)
        }

        // todo: what is this?
        DriverResult::Skip => Entry::None,
    };

    Ok(next_entry)
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Read};

    use crate::drivers;

    use super::*;

    #[test]
    fn build_in_recursion_works() {
        let cwd = std::env::current_dir().unwrap();
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
}
