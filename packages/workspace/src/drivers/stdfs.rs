use super::{Driver, Error, DriverResult};
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct Stdfs;

impl Driver for Stdfs {
    fn load(&self, path: &Path) -> Result<DriverResult, Error> {
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
                .collect::<Result<Vec<PathBuf>, Error>>()?;

            return Ok(DriverResult::Directory(path.into(), descendants));
        }

        if path_metadata.is_file() {
            let file = std::fs::File::options()
                .read(true)
                .write(true)
                .open(&path)?;
            return Ok(DriverResult::File(path.into(), Box::new(file)));
        }

        Err(Error::Unreachable)
    }

    fn write(&mut self, path: &Path, content: &[u8]) -> Result<(), Error> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // crash-safe write: stage to a temp file in the SAME directory (so the
        // final rename is atomic — cross-filesystem renames are not), fsync the
        // bytes to disk, then atomically rename over the target. a crash before
        // the rename leaves the original intact and only the temp file dangling;
        // a crash after sees the fully-written new file. on any failure we best-
        // effort remove the temp file so we don't litter the dir.
        let tmp = tmp_sibling(path);

        let staged = (|| -> Result<(), Error> {
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(content)?;
            f.sync_all()?;
            Ok(())
        })();
        if let Err(e) = staged {
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }

        if let Err(e) = std::fs::rename(&tmp, path) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.into());
        }
        Ok(())
    }
}

/// derive a temp-file path sibling to `path` (same parent dir, so the rename is
/// atomic). a pid + nanosecond suffix keeps concurrent writers from colliding.
fn tmp_sibling(path: &Path) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let suffix = format!(".tmp.{}.{}", std::process::id(), nanos);

    let name = path
        .file_name()
        .map(|n| {
            let mut s = n.to_os_string();
            s.push(&suffix);
            s
        })
        .unwrap_or_else(|| suffix.clone().into());

    match path.parent() {
        Some(parent) => parent.join(name),
        None => PathBuf::from(name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// a unique scratch dir under the OS temp dir, removed on drop.
    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let dir = std::env::temp_dir()
                .join(format!("ducktape-stdfs-{}-{}", std::process::id(), nanos));
            std::fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn write_then_read_back_leaves_no_temp_file() {
        let dir = TempDir::new();
        let target = dir.0.join("note.md");
        let bytes = b"hello crash-safe world";

        let mut driver = Stdfs;
        driver.write(&target, bytes).expect("write ok");

        // bytes round-trip.
        let read = std::fs::read(&target).expect("read back");
        assert_eq!(read, bytes);

        // the only thing in the dir is the target — no dangling temp file.
        let names: Vec<String> = std::fs::read_dir(&dir.0)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["note.md".to_string()]);
    }

    #[test]
    fn write_overwrites_existing_file_atomically() {
        let dir = TempDir::new();
        let target = dir.0.join("note.md");

        let mut driver = Stdfs;
        driver.write(&target, b"first").expect("first write");
        driver.write(&target, b"second").expect("second write");

        assert_eq!(std::fs::read(&target).unwrap(), b"second");
        let count = std::fs::read_dir(&dir.0).unwrap().count();
        assert_eq!(count, 1, "no temp file left behind after overwrite");
    }
}
