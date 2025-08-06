use std::{fs::File, path::PathBuf};

use crate::{
    drivers::{DriverError, DriverResult},
    entry::Entry,
};

#[derive(thiserror::Error, Debug)]
pub enum TreeError<DocBuilderError> {
    #[error("TreeError: {0}")]
    Invariant(anyhow::Error),

    #[error("TreeError: error during docbuild: {0}")]
    DocBuilder(DocBuilderError),
}

pub struct Tree<Doc> {
    basedir: PathBuf,
    root: Entry<Doc>,
}

type Loader = fn(&PathBuf) -> Result<DriverResult, DriverError>;
type DocBuild<Doc, DocBuilderError> = fn(File) -> Result<Doc, DocBuilderError>;

impl<Doc> Tree<Doc> {
    pub fn new<DocBuilderError: std::error::Error>(
        basedir: &PathBuf,
        load: Loader,
        doc_builder: DocBuild<Doc, DocBuilderError>,
    ) -> Result<Self, TreeError<DocBuilderError>> {
        Ok(Self {
            basedir: basedir.clone(),
            root: build_in_recursion(basedir, load, doc_builder, 0, 10)?,
        })
    }
}

pub fn build_in_recursion<Doc, DocBuilderError>(
    path: &PathBuf,
    load: Loader,
    doc_builder: DocBuild<Doc, DocBuilderError>,
    current_depth: usize,
    max_depth: usize,
) -> Result<Entry<Doc>, TreeError<DocBuilderError>> {
    eprintln!("building tree ({})", path.to_string_lossy().to_string());
    let load_result = load(path).map_err(|e| TreeError::Invariant(e.into()))?;
    let next_entry = match load_result {
        DriverResult::File(_, file) => {
            Entry::File(doc_builder(file).map_err(|e| TreeError::DocBuilder(e))?)
        }
        DriverResult::Directory(_, path_bufs) => {
            let k: Result<Vec<(PathBuf, Entry<Doc>)>, TreeError<DocBuilderError>> = path_bufs
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

            Entry::Directory(k?)
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
        let docbuilder: DocBuild<String, anyhow::Error> = |f| {
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
