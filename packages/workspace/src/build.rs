use std::{path::Path};
use document::Document;

use crate::{
    drivers::{Driver, DriverResult}, entry::Entry
};

#[derive(thiserror::Error, Debug)]
pub(crate) enum Error {
    #[error("driver failed: {0}")]
    DriverError(crate::drivers::Error),

    #[error("document error: {0}")]
    DocumentError(document::Error),

    #[error("invalid path segment: {0}")]
    InvalidPathSegment(String)
}

/// Builds a `Tree` by recursively loading from `driver`, starting at
/// `basedir`. The bridge between the storage primitives (the `drivers`
/// module) and the in-memory tree. `basedir` is consumed only to walk the
/// driver — it isn't baked into the resulting tree, since `Tree` is a pure
/// in-memory structure with no path concept. Callers that need to remember
/// where on disk the tree came from (e.g. `Persister` for write paths) keep
/// the basedir alongside the tree, not inside it.
pub(crate) fn build_from_basedir(driver: &dyn Driver, basedir: &Path) -> Result<Option<Entry>, Error> {
    let basedir_str = basedir.to_string_lossy().to_string();
    build_in_recursion(driver, &basedir_str, &basedir_str)
}

// Returns `Ok(None)` when the driver chose to skip the path — the caller is
// expected to drop the entry from its parent's listing rather than treat it
// as a present-but-empty child.
fn build_in_recursion(
    driver: &dyn Driver,
    base_path: &str,
    load_path: &str,
) -> Result<Option<Entry>, Error> {
    let load_result = driver
        .load(Path::new(load_path))
        .map_err(|e| Error::DriverError(e))?;
    let next_entry = match load_result {
        DriverResult::Skip => return Ok(None),
        DriverResult::File(_, reader) => Entry::File(
            Document::from_reader(reader)
                .map_err(|e| Error::DocumentError(e))?,
        ),
        DriverResult::Directory(_, path_bufs) => {
            let descendants: Result<Vec<(String, Entry)>, Error> = path_bufs
                .iter()
                .filter_map(|descendant_path| {
                    let descendant_str = descendant_path.to_string_lossy().to_string();
                    let entry = match build_in_recursion(driver, base_path, &descendant_str) {
                        Ok(Some(e)) => e,
                        Ok(None) => return None,
                        Err(e) => return Some(Err(e)),
                    };
                    let relative_path: Vec<&str> =
                        match Path::new(&descendant_str).strip_prefix(base_path) {
                            Ok(p) => p.iter().map(|x| x.to_str().unwrap()).collect(),
                            Err(e) => {
                                return Some(Err(Error::InvalidPathSegment(e.to_string())));
                            }
                        };
                    let first_segment = relative_path[relative_path.len() - 1];
                    Some(Ok((first_segment.to_string(), entry)))
                })
                .collect();

            Entry::Directory(descendants?)
        }
    };

    Ok(Some(next_entry))
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::Vfs;

//     #[test]
//     fn build_tree_walks_vfs_fixture() {
//         let mut fs = Vfs::new();
//         fs.write_file("/root/a/aa/aaa.md", b"hello".to_vec());
//         fs.write_file("/root/b/bb/bbb.md", b"world".to_vec());

//         let tree = build_tree(&fs, Path::new("/root")).unwrap();

//         let aaa = tree.get_entries("a/aa/aaa.md".into()).unwrap();
//         assert!(matches!(aaa, Entry::File(_)));

//         let bbb = tree.get_entries("b/bb/bbb.md".into()).unwrap();
//         assert!(matches!(bbb, Entry::File(_)));
//     }

//     #[test]
//     fn build_tree_skips_dotfile_children() {
//         let mut fs = Vfs::new();
//         fs.write_file("/root/visible.md", b"".to_vec());
//         fs.write_file("/root/.hidden", b"".to_vec());

//         let tree = build_tree(&fs, Path::new("/root")).unwrap();

//         // The visible file is reachable.
//         let visible = tree.get_entries("visible.md".into()).unwrap();
//         assert!(matches!(visible, Entry::File(_)));

//         // The dotfile is not in the parent's listing at all (no None placeholder).
//         let Entry::Directory(items) = tree.root().as_ref() else {
//             panic!("root should be a Directory");
//         };
//         assert_eq!(items.len(), 1);
//         assert_eq!(items[0].0, "visible.md");
//     }
// }
