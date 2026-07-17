//! Private current-user filesystem boundary for desktop-owned state.

use std::fs::{self, File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};

const PRIVATE_DIR_MODE: u32 = 0o700;
const PRIVATE_FILE_MODE: u32 = 0o600;

pub(crate) fn ensure_private_dir(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => verify_directory(path, &metadata)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt as _;
                builder.mode(PRIVATE_DIR_MODE);
            }
            builder
                .create(path)
                .map_err(|error| format!("create private directory {}: {error}", path.display()))?;
            let metadata = fs::symlink_metadata(path).map_err(|error| {
                format!("inspect private directory {}: {error}", path.display())
            })?;
            verify_directory(path, &metadata)?;
        }
        Err(error) => {
            return Err(format!(
                "inspect private directory {}: {error}",
                path.display()
            ));
        }
    }
    set_private_dir_permissions(path)
}

pub(super) fn harden_private_dir(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("inspect private directory {}: {error}", path.display()))?;
    verify_directory(path, &metadata)?;
    set_private_dir_permissions(path)
}

pub(super) fn create_private_dir(path: &Path) -> Result<(), String> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(PRIVATE_DIR_MODE);
    }
    builder
        .create(path)
        .map_err(|error| format!("create private directory {}: {error}", path.display()))?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("inspect private directory {}: {error}", path.display()))?;
    verify_directory(path, &metadata)?;
    set_private_dir_permissions(path)
}

fn verify_directory(path: &Path, metadata: &fs::Metadata) -> Result<(), String> {
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "private state path is not a real directory: {}",
            path.display()
        ));
    }
    Ok(())
}

fn set_private_dir_permissions(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(PRIVATE_DIR_MODE)).map_err(
            |error| {
                format!(
                    "set private directory permissions on {}: {error}",
                    path.display()
                )
            },
        )?;
    }
    Ok(())
}

pub(super) fn open_private_append(path: &Path) -> Result<File, String> {
    refuse_symlink(path)?;
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    private_open_options(&mut options);
    let file = options
        .open(path)
        .map_err(|error| format!("open private file {}: {error}", path.display()))?;
    verify_and_harden_file(path, &file)?;
    Ok(file)
}

pub(super) fn read(path: &Path) -> Result<Option<Vec<u8>>, String> {
    let Some(mut file) = open_private_read(path)? else {
        return Ok(None);
    };
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|error| format!("read private file {}: {error}", path.display()))?;
    Ok(Some(bytes))
}

pub(crate) fn open_private_read(path: &Path) -> Result<Option<File>, String> {
    refuse_symlink(path)?;
    let mut options = OpenOptions::new();
    options.read(true);
    private_open_options(&mut options);
    let file = match options.open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!("open private file {}: {error}", path.display()));
        }
    };
    verify_and_harden_file(path, &file)?;
    Ok(Some(file))
}

pub(crate) fn read_to_string(path: &Path) -> Result<Option<String>, String> {
    read(path)?
        .map(|bytes| {
            String::from_utf8(bytes)
                .map_err(|_| format!("private file is not UTF-8: {}", path.display()))
        })
        .transpose()
}

pub(super) fn harden_private_file(path: &Path) -> Result<(), String> {
    refuse_symlink(path)?;
    let mut options = OpenOptions::new();
    options.read(true);
    private_open_options(&mut options);
    let file = options
        .open(path)
        .map_err(|error| format!("open private file {}: {error}", path.display()))?;
    verify_and_harden_file(path, &file)
}

pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("private file has no parent: {}", path.display()))?;
    ensure_private_dir(parent)?;
    refuse_symlink(path)?;

    let (temporary, mut file) = create_temporary(path)?;
    let result = (|| {
        file.write_all(bytes)
            .map_err(|error| format!("write private temporary {temporary:?}: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("fsync private temporary {temporary:?}: {error}"))?;
        drop(file);
        fs::rename(&temporary, path)
            .map_err(|error| format!("rename {temporary:?} -> {path:?}: {error}"))?;
        sync_directory(parent);
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn create_temporary(path: &Path) -> Result<(PathBuf, File), String> {
    let parent = path.parent().expect("write_atomic checked the parent");
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("private file name is not UTF-8: {}", path.display()))?;
    for _ in 0..16 {
        let mut random = [0u8; 8];
        getrandom::getrandom(&mut random)
            .map_err(|_| "could not randomize a private temporary file".to_string())?;
        let temporary = parent.join(format!(
            ".{name}.{}.{:016x}.tmp",
            std::process::id(),
            u64::from_ne_bytes(random)
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        private_open_options(&mut options);
        match options.open(&temporary) {
            Ok(file) => {
                if let Err(error) = verify_and_harden_file(&temporary, &file) {
                    drop(file);
                    let _ = fs::remove_file(&temporary);
                    return Err(error);
                }
                return Ok((temporary, file));
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "create private temporary {}: {error}",
                    temporary.display()
                ));
            }
        }
    }
    Err("could not allocate a unique private temporary file".into())
}

fn refuse_symlink(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(format!(
            "refusing symbolic link for private state: {}",
            path.display()
        )),
        Ok(metadata) if !metadata.is_file() => Err(format!(
            "private state path is not a regular file: {}",
            path.display()
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("inspect private file {}: {error}", path.display())),
    }
}

fn private_open_options(options: &mut OpenOptions) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options
            .mode(PRIVATE_FILE_MODE)
            .custom_flags(libc::O_NOFOLLOW);
    }
}

fn verify_and_harden_file(path: &Path, file: &File) -> Result<(), String> {
    let metadata = file
        .metadata()
        .map_err(|error| format!("inspect private file {}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!(
            "private state path is not a regular file: {}",
            path.display()
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        file.set_permissions(fs::Permissions::from_mode(PRIVATE_FILE_MODE))
            .map_err(|error| {
                format!(
                    "set private file permissions on {}: {error}",
                    path.display()
                )
            })?;
    }
    Ok(())
}

fn sync_directory(path: &Path) {
    #[cfg(unix)]
    if let Ok(directory) = File::open(path) {
        let _ = directory.sync_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn directories_and_atomic_files_are_private() {
        use std::os::unix::fs::PermissionsExt as _;

        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("state");
        ensure_private_dir(&directory).unwrap();
        assert_eq!(
            fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
            PRIVATE_DIR_MODE
        );

        let path = directory.join("registry.json");
        write_atomic(&path, b"one").unwrap();
        write_atomic(&path, b"two").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"two");
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            PRIVATE_FILE_MODE
        );
        assert!(fs::read_dir(&directory).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")
        }));

        let log = directory.join("daemon.log");
        writeln!(open_private_append(&log).unwrap(), "line").unwrap();
        assert_eq!(
            fs::metadata(log).unwrap().permissions().mode() & 0o777,
            PRIVATE_FILE_MODE
        );
    }

    #[cfg(unix)]
    #[test]
    fn final_symbolic_links_are_never_followed() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("target");
        fs::write(&target, b"unchanged").unwrap();
        let link = root.path().join("state");
        symlink(&target, &link).unwrap();
        assert!(ensure_private_dir(&link).is_err());
        assert!(write_atomic(&link, b"replaced").is_err());
        assert_eq!(fs::read(target).unwrap(), b"unchanged");
    }
}
