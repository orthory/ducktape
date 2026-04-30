use std::{path::Path, sync::Arc};

use doctree::{Entry, Tree, TreeError};
use document::Document;

use crate::{Driver, DriverResult};

/// Builds a `Tree` by recursively loading from `driver`, starting at
/// `basedir`. This is the bridge between the storage layer (Driver) and the
/// in-memory tree (Tree); it lives in the driver crate so doctree itself
/// stays free of any persistence knowledge.
pub fn build_tree(driver: &dyn Driver, basedir: &Path) -> Result<Tree, TreeError> {
    let basedir_as_string = basedir.to_string_lossy().to_string();
    let root = build_in_recursion(driver, &basedir_as_string, &basedir_as_string, 0, 10)?;
    Ok(Tree::new(basedir_as_string, root))
}

fn build_in_recursion(
    driver: &dyn Driver,
    base_path: &String,
    load_path: &String,
    current_depth: usize,
    max_depth: usize,
) -> Result<Arc<Entry>, TreeError> {
    let load_result = driver
        .load(Path::new(load_path))
        .map_err(|e| TreeError::Invariant(anyhow::Error::msg(e.to_string())))?;
    let next_entry = match load_result {
        DriverResult::Skip => Entry::None,
        DriverResult::File(_, reader) => Entry::File(
            Document::from_reader(reader)
                .map_err(|e| TreeError::DocBuilder(anyhow::Error::msg(e.to_string())))?,
        ),
        DriverResult::Directory(_, path_bufs) => {
            let descendants: Result<Vec<(String, Arc<Entry>)>, TreeError> = path_bufs
                .iter()
                .map(|descendant_path| {
                    let descendant_path_as_string = descendant_path.to_string_lossy().to_string();
                    let entry = build_in_recursion(
                        driver,
                        base_path,
                        &descendant_path_as_string,
                        current_depth + 1,
                        max_depth,
                    )?;
                    let relative_path: Vec<&str> = Path::new(&descendant_path_as_string)
                        .strip_prefix(base_path)
                        .map_err(|e| TreeError::InvalidPathSegment(e.to_string()))?
                        .iter()
                        .map(|x| x.to_str().unwrap())
                        .collect();
                    let first_segment = relative_path[relative_path.len() - 1];
                    Ok((first_segment.to_string(), entry))
                })
                .collect();

            Entry::Directory(descendants?)
        }
    };

    Ok(Arc::new(next_entry))
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
}
