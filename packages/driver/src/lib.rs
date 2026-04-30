use std::{
    io::Read,
    path::{Path, PathBuf},
};

mod build;
mod persisted;
pub mod stdfs;
pub mod vfs;

pub use build::build_tree;
pub use persisted::PersistedTree;
pub use stdfs::Stdfs;
pub use vfs::Vfs;

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
    /// paths the caller shouldn't see (e.g. dotfiles) without raising an
    /// error. Takes `&self` so concurrent reads can run behind a `RwLock`
    /// read guard.
    fn load(&self, path: &Path) -> Result<DriverResult, DriverError>;

    /// Persists `content` at `path` as a finalization step. Takes `&mut self`
    /// to make exclusive access a type-level requirement: callers must hold
    /// a write guard on the driver (mediated by `PersistedTree`) before they
    /// can write. The buffer is the complete new contents — drivers don't see
    /// the intermediate ops accumulated by the upper layer.
    fn write(&mut self, path: &Path, content: &[u8]) -> Result<(), DriverError>;
}
