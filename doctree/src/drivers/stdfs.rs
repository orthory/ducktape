use super::{DriverError, DriverResult};
use std::path::PathBuf;

pub fn load(path: &PathBuf) -> Result<DriverResult, DriverError> {
    // skip if this file starts with "."
    if path
        .iter()
        .last()
        .unwrap()
        .to_string_lossy()
        .starts_with(".")
    {
        return Ok(DriverResult::Skip);
    }

    // otherwise let's go
    let path_metadata = std::fs::metadata(&path).map_err(|e| DriverError::IOError(e))?;
    if path_metadata.is_dir() {
        let descendants = std::fs::read_dir(&path)
            .map_err(|e| DriverError::IOError(e))?
            .map(|f| Ok(f?.path()))
            .collect::<Result<Vec<PathBuf>, DriverError>>()?;

        return Ok(DriverResult::Directory(path.clone(), descendants));
    }

    if path_metadata.is_file() {
        let file = std::fs::File::options()
            .read(true)
            .write(true)
            .open(&path)?;
        return Ok(DriverResult::File(path.clone(), file));
    }

    // don't handle other cases
    return Err(DriverError::Unreachable);
}
