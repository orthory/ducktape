use super::{Driver, DriverError, DriverResult};
use std::{
    collections::HashMap,
    io::Cursor,
    path::{Path, PathBuf},
};

#[derive(Default, Debug)]
pub struct Vfs {
    files: HashMap<PathBuf, Vec<u8>>,
    dirs: HashMap<PathBuf, Vec<PathBuf>>,
}

impl Vfs {
    pub fn new() -> Self {
        let mut vfs = Vfs::default();
        vfs.dirs.insert(PathBuf::from("/"), Vec::new());
        vfs
    }

    pub fn write_file(&mut self, path: impl Into<PathBuf>, contents: impl Into<Vec<u8>>) {
        let path = path.into();
        self.ensure_parents(&path);
        self.register_in_parent(&path);
        self.files.insert(path, contents.into());
    }

    pub fn mkdir(&mut self, path: impl Into<PathBuf>) {
        let path = path.into();
        self.ensure_parents(&path);
        self.register_in_parent(&path);
        self.dirs.entry(path).or_default();
    }

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

        let p = path.to_path_buf();

        if let Some(contents) = self.files.get(&p) {
            let cursor = Cursor::new(contents.clone());
            return Ok(DriverResult::File(p, Box::new(cursor)));
        }

        if let Some(children) = self.dirs.get(&p) {
            return Ok(DriverResult::Directory(p, children.clone()));
        }

        Err(DriverError::IOError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("vfs: path not found: {}", p.display()),
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn write_file_creates_intermediate_dirs() {
        let mut vfs = Vfs::new();
        vfs.write_file("/a/b/c.md", b"hello".to_vec());

        assert!(vfs.dirs.contains_key(Path::new("/a")));
        assert!(vfs.dirs.contains_key(Path::new("/a/b")));
        assert!(vfs.files.contains_key(Path::new("/a/b/c.md")));

        let root_children = vfs.dirs.get(Path::new("/")).unwrap();
        assert!(root_children.contains(&PathBuf::from("/a")));
    }

    #[test]
    fn load_returns_file_with_contents() {
        let mut vfs = Vfs::new();
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
        let mut vfs = Vfs::new();
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
        let mut vfs = Vfs::new();
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
}
