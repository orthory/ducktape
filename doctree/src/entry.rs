use serde::Serialize;
use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum EntryError {
    #[error("EntryError: {0}")]
    Invariant(anyhow::Error),
}

#[derive(Debug)]
pub enum Entry<Doc> {
    None,
    File(Doc),
    Directory(Vec<(/*absolute*/ PathBuf, Entry<Doc>)>),
}
