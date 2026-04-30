use super::{Driver, DriverError, DriverResult};
use std::path::{Path, PathBuf};

pub struct Stdfs;

impl Driver for Stdfs {
    fn load(&self, path: &Path) -> Result<DriverResult, DriverError> {
        if path
            .iter()
            .last()
            .unwrap()
            .to_string_lossy()
            .starts_with(".")
        {
            return Ok(DriverResult::Skip);
        }

        let path_metadata = std::fs::metadata(&path)?;
        if path_metadata.is_dir() {
            let descendants = std::fs::read_dir(&path)?
                .map(|f| Ok(f?.path()))
                .collect::<Result<Vec<PathBuf>, DriverError>>()?;

            return Ok(DriverResult::Directory(path.into(), descendants));
        }

        if path_metadata.is_file() {
            let file = std::fs::File::options()
                .read(true)
                .write(true)
                .open(&path)?;
            return Ok(DriverResult::File(path.into(), Box::new(file)));
        }

        Err(DriverError::Unreachable)
    }
}
