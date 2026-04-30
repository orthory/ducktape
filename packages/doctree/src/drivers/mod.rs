use std::{
    io::Read,
    path::{Path, PathBuf},
};

pub mod stdfs;
pub mod vfs;

pub enum DriverResult {
    File(PathBuf, Box<dyn Read + Send>),
    Directory(PathBuf, Vec<PathBuf>),
    Skip,
}

#[derive(thiserror::Error, Debug)]
pub enum DriverError {
    #[error("DriverError: {0}")]
    Invariant(anyhow::Error),

    #[error("DriverError: FileError: {0}")]
    IOError(#[from] std::io::Error),

    #[error("DriverError: code reached the unreachable")]
    Unreachable,
}

pub trait Driver: Send + Sync {
    /// Resolves an entry at `path`. Drivers may return `Skip` to filter out
    /// paths the caller shouldn't see (e.g. dotfiles) without raising an error.
    fn load(&self, path: &Path) -> Result<DriverResult, DriverError>;

    /// Persists `content` at `path` as a finalization step. The upper layer is
    /// responsible for accumulating edits/events and serializing them into the
    /// complete buffer passed here — drivers don't see the intermediate ops.
    fn write(&self, path: &Path, content: &[u8]) -> Result<(), DriverError>;
}
