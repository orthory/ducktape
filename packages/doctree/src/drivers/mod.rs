use std::{fs::File, path::PathBuf};

pub mod stdfs;
pub mod vfs;

pub enum DriverResult {
    File(PathBuf, File),
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
