use serde::Serialize;

#[derive(thiserror::Error, Debug)]
pub enum EntryError {
    #[error("EntryError: {0}")]
    Invariant(anyhow::Error),
}

#[derive(Debug, Clone, Serialize)]
pub enum Entry<Doc> {
    None,
    File(Doc),
    Directory(Vec<(/*absolute*/ String, Entry<Doc>)>),
}
