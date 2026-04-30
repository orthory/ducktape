use std::{path::Path, sync::Arc};

use document::Document;

use crate::{
    Entry, Tree, TreeError,
    drivers::{Driver, DriverResult},
};

/// Builds a `Tree` by recursively loading from `driver`, starting at
/// `basedir`. The bridge between the storage primitives (the `drivers`
/// module) and the in-memory tree.
pub fn build_tree(driver: &dyn Driver, basedir: &Path) -> Result<Tree, TreeError> {
    let basedir_as_string = basedir.to_string_lossy().to_string();
    let root = build_in_recursion(driver, &basedir_as_string, &basedir_as_string, 0, 10)?
        .ok_or_else(|| {
            TreeError::InvalidEntry(format!(
                "basedir was skipped by driver: {}",
                basedir_as_string
            ))
        })?;
    Ok(Tree::new(basedir_as_string, root))
}

// Returns `Ok(None)` when the driver chose to skip the path — the caller is
// expected to drop the entry from its parent's listing rather than treat it
// as a present-but-empty child.
fn build_in_recursion(
    driver: &dyn Driver,
    base_path: &String,
    load_path: &String,
    current_depth: usize,
    max_depth: usize,
) -> Result<Option<Arc<Entry>>, TreeError> {
    let load_result = driver
        .load(Path::new(load_path))
        .map_err(|e| TreeError::Invariant(anyhow::Error::msg(e.to_string())))?;
    let next_entry = match load_result {
        DriverResult::Skip => return Ok(None),
        DriverResult::File(_, reader) => Entry::File(
            Document::from_reader(reader)
                .map_err(|e| TreeError::DocBuilder(anyhow::Error::msg(e.to_string())))?,
        ),
        DriverResult::Directory(_, path_bufs) => {
            let descendants: Result<Vec<(String, Arc<Entry>)>, TreeError> = path_bufs
                .iter()
                .filter_map(|descendant_path| {
                    let descendant_path_as_string = descendant_path.to_string_lossy().to_string();
                    let entry = match build_in_recursion(
                        driver,
                        base_path,
                        &descendant_path_as_string,
                        current_depth + 1,
                        max_depth,
                    ) {
                        Ok(Some(e)) => e,
                        Ok(None) => return None,
                        Err(e) => return Some(Err(e)),
                    };
                    let relative_path: Vec<&str> = match Path::new(&descendant_path_as_string)
                        .strip_prefix(base_path)
                    {
                        Ok(p) => p.iter().map(|x| x.to_str().unwrap()).collect(),
                        Err(e) => return Some(Err(TreeError::InvalidPathSegment(e.to_string()))),
                    };
                    let first_segment = relative_path[relative_path.len() - 1];
                    Some(Ok((first_segment.to_string(), entry)))
                })
                .collect();

            Entry::Directory(descendants?)
        }
    };

    Ok(Some(Arc::new(next_entry)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Vfs;

    #[test]
    fn build_tree_walks_vfs_fixture() {
        let mut fs = Vfs::new();
        fs.write_file("/root/a/aa/aaa.md", b"hello".to_vec());
        fs.write_file("/root/b/bb/bbb.md", b"world".to_vec());

        let tree = build_tree(&fs, Path::new("/root")).unwrap();

        let aaa = tree.get_entries("a/aa/aaa.md".into()).unwrap();
        assert!(matches!(aaa, Entry::File(_)));

        let bbb = tree.get_entries("b/bb/bbb.md".into()).unwrap();
        assert!(matches!(bbb, Entry::File(_)));
    }

    #[test]
    fn build_tree_skips_dotfile_children() {
        let mut fs = Vfs::new();
        fs.write_file("/root/visible.md", b"".to_vec());
        fs.write_file("/root/.hidden", b"".to_vec());

        let tree = build_tree(&fs, Path::new("/root")).unwrap();

        // The visible file is reachable.
        let visible = tree.get_entries("visible.md".into()).unwrap();
        assert!(matches!(visible, Entry::File(_)));

        // The dotfile is not in the parent's listing at all (no None placeholder).
        let Entry::Directory(items) = tree.root().as_ref() else {
            panic!("root should be a Directory");
        };
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].0, "visible.md");
    }
}
