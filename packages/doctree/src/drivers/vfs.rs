use super::{Driver, DriverError, DriverResult};
use std::{
    collections::HashMap,
    io::Cursor,
    path::{Path, PathBuf},
    sync::RwLock,
};

// Vfs's mutable state lives behind a RwLock because the Driver trait's methods
// take `&self` (the tree holds `Arc<dyn Driver>` and can't get exclusive
// access). Reads and writes both go through the lock.
pub struct Vfs {
    inner: RwLock<VfsInner>,
}

#[derive(Default, Debug)]
struct VfsInner {
    files: HashMap<PathBuf, Vec<u8>>,
    dirs: HashMap<PathBuf, Vec<PathBuf>>,
}

impl Vfs {
    pub fn new() -> Self {
        let mut inner = VfsInner::default();
        inner.dirs.insert(PathBuf::from("/"), Vec::new());
        Self {
            inner: RwLock::new(inner),
        }
    }

    pub fn write_file(&self, path: impl Into<PathBuf>, contents: impl Into<Vec<u8>>) {
        let path = path.into();
        let mut inner = self.inner.write().unwrap();
        inner.ensure_parents(&path);
        inner.register_in_parent(&path);
        inner.files.insert(path, contents.into());
    }

    pub fn mkdir(&self, path: impl Into<PathBuf>) {
        let path = path.into();
        let mut inner = self.inner.write().unwrap();
        inner.ensure_parents(&path);
        inner.register_in_parent(&path);
        inner.dirs.entry(path).or_default();
    }
}

impl VfsInner {
    fn ensure_parents(&mut self, path: &Path) {
        let Some(parent) = path.parent() else {
            return;
        };

        let mut current = PathBuf::from("/");
        for comp in parent.components().skip(1) {
            let next = current.join(comp);
            let children = self.dirs.entry(current.clone()).or_default();
            if !children.contains(&next) {
                children.push(next.clone());
            }
            self.dirs.entry(next.clone()).or_default();
            current = next;
        }
    }

    fn register_in_parent(&mut self, path: &Path) {
        let parent = path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/"));
        let children = self.dirs.entry(parent).or_default();
        if !children.contains(&path.to_path_buf()) {
            children.push(path.to_path_buf());
        }
    }
}

impl Driver for Vfs {
    fn load(&self, path: &Path) -> Result<DriverResult, DriverError> {
        if let Some(name) = path.file_name() {
            if name.to_string_lossy().starts_with(".") {
                return Ok(DriverResult::Skip);
            }
        }

        let inner = self.inner.read().unwrap();
        let p = path.to_path_buf();

        if let Some(contents) = inner.files.get(&p) {
            let cursor = Cursor::new(contents.clone());
            return Ok(DriverResult::File(p, Box::new(cursor)));
        }

        if let Some(children) = inner.dirs.get(&p) {
            return Ok(DriverResult::Directory(p, children.clone()));
        }

        Err(DriverError::IOError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("vfs: path not found: {}", p.display()),
        )))
    }

    fn write(&self, path: &Path, content: &[u8]) -> Result<(), DriverError> {
        self.write_file(path.to_path_buf(), content.to_vec());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn write_file_creates_intermediate_dirs() {
        let vfs = Vfs::new();
        vfs.write_file("/a/b/c.md", b"hello".to_vec());

        // Verify dir structure via load() — we don't reach into private state.
        match vfs.load(Path::new("/a")).unwrap() {
            DriverResult::Directory(_, children) => {
                assert!(children.contains(&PathBuf::from("/a/b")));
            }
            _ => panic!("expected /a to be a Directory"),
        }
        match vfs.load(Path::new("/a/b")).unwrap() {
            DriverResult::Directory(_, children) => {
                assert!(children.contains(&PathBuf::from("/a/b/c.md")));
            }
            _ => panic!("expected /a/b to be a Directory"),
        }
        assert!(matches!(
            vfs.load(Path::new("/a/b/c.md")).unwrap(),
            DriverResult::File(_, _)
        ));
    }

    #[test]
    fn load_returns_file_with_contents() {
        let vfs = Vfs::new();
        vfs.write_file("/foo.txt", b"hello world".to_vec());

        match vfs.load(Path::new("/foo.txt")).unwrap() {
            DriverResult::File(path, mut reader) => {
                assert_eq!(path, PathBuf::from("/foo.txt"));
                let mut buf = String::new();
                reader.read_to_string(&mut buf).unwrap();
                assert_eq!(buf, "hello world");
            }
            _ => panic!("expected File variant"),
        }
    }

    #[test]
    fn load_returns_directory_with_children() {
        let vfs = Vfs::new();
        vfs.write_file("/dir/a.md", b"a".to_vec());
        vfs.write_file("/dir/b.md", b"b".to_vec());

        match vfs.load(Path::new("/dir")).unwrap() {
            DriverResult::Directory(path, children) => {
                assert_eq!(path, PathBuf::from("/dir"));
                assert_eq!(children.len(), 2);
                assert!(children.contains(&PathBuf::from("/dir/a.md")));
                assert!(children.contains(&PathBuf::from("/dir/b.md")));
            }
            _ => panic!("expected Directory variant"),
        }
    }

    #[test]
    fn load_skips_dotfiles() {
        let vfs = Vfs::new();
        vfs.write_file("/.hidden", b"secret".to_vec());

        assert!(matches!(
            vfs.load(Path::new("/.hidden")).unwrap(),
            DriverResult::Skip
        ));
    }

    #[test]
    fn load_missing_path_errors() {
        let vfs = Vfs::new();
        match vfs.load(Path::new("/nope")) {
            Err(DriverError::IOError(_)) => (),
            Err(other) => panic!("expected IOError, got {:?}", other),
            Ok(_) => panic!("expected error for missing path"),
        }
    }

    #[test]
    fn driver_write_then_load_roundtrips() {
        let vfs = Vfs::new();

        // Call through the Driver trait, not the convenience method, to exercise
        // the trait surface a real consumer would use.
        Driver::write(&vfs, Path::new("/round/trip.md"), b"persisted").unwrap();

        match vfs.load(Path::new("/round/trip.md")).unwrap() {
            DriverResult::File(_, mut reader) => {
                let mut buf = String::new();
                reader.read_to_string(&mut buf).unwrap();
                assert_eq!(buf, "persisted");
            }
            _ => panic!("expected File after write"),
        }

        // Parent directory listing should reflect the new entry.
        match vfs.load(Path::new("/round")).unwrap() {
            DriverResult::Directory(_, children) => {
                assert!(children.contains(&PathBuf::from("/round/trip.md")));
            }
            _ => panic!("expected /round to be a Directory"),
        }
    }
}
