//! the ONE log-file opener: append, and rotate at open when the file has
//! outgrown [`ROTATE_BYTES`].
//!
//! it lives in this crate because it is the smallest host-side crate BOTH
//! writers link — the daemon library (`noded::log`, for `daemon.log` and every
//! `service run <kind>.log`) and the desktop app (its `app.log`). the app does
//! not link `noded`, and the alternatives both depend on are module crates that
//! compile to wasm guests. one rotation mechanism in the tree, not two.
//!
//! rename-if-big, at open only: no size checks on the write path (a `Mutex<File>`
//! writer stays a plain unbuffered write, so a crash keeps its last line) and
//! exactly one rotated generation — `<name>.1`, replaced each time. a node that
//! outgrows the cap between restarts is a node whose filter was turned up and
//! left; the cap bounds the disk, the previous generation keeps the context.

use std::fs::File;
use std::path::{Path, PathBuf};

/// the size past which the file is rotated at the next open. at the default
/// filter a node writes ~a line per block, so this is weeks; under a
/// `debug` plane it is hours — either way the disk stays bounded at two of
/// these per log.
pub const ROTATE_BYTES: u64 = 64 * 1024 * 1024;

/// append-open `path`, creating its parent; when the file is already larger
/// than [`ROTATE_BYTES`] it is first renamed to `<name>.1` (replacing any
/// previous `.1`) and a fresh file is opened. append (never truncate): the
/// record across restarts is exactly what a crash post-mortem reads.
pub fn open_rotating(path: &Path) -> std::io::Result<File> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let oversize = std::fs::metadata(path).is_ok_and(|meta| meta.len() > ROTATE_BYTES);
    if oversize {
        // best-effort: a rename that fails (a `.1` some other process holds
        // open on windows, say) must not cost the node its log — the open
        // below still appends, and the next restart tries again.
        let _ = std::fs::rename(path, rotated_path(path));
    }
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
}

/// `<path>.1` — the one rotated generation.
fn rotated_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".1");
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    #[test]
    fn a_small_log_is_appended_and_never_rotated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("daemon.log");
        open_rotating(&path).unwrap().write_all(b"first\n").unwrap();
        open_rotating(&path)
            .unwrap()
            .write_all(b"second\n")
            .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"first\nsecond\n");
        assert!(!rotated_path(&path).exists(), "nothing to rotate");
    }

    #[test]
    fn an_oversize_log_rotates_once_and_the_next_rotation_replaces_the_copy() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("daemon.log");
        // the SIZE is what rotates, not the bytes: a sparse file past the cap
        // costs the test nothing and reads back zero-filled.
        let mut gen1 = open_rotating(&path).unwrap();
        gen1.write_all(b"gen1").unwrap();
        gen1.set_len(ROTATE_BYTES + 1).unwrap();
        drop(gen1);

        let mut gen2 = open_rotating(&path).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().len(), 0, "a fresh file");
        let rotated = std::fs::read(rotated_path(&path)).unwrap();
        assert!(rotated.starts_with(b"gen1"), "the old file is the .1");
        assert_eq!(rotated.len() as u64, ROTATE_BYTES + 1);

        gen2.write_all(b"gen2").unwrap();
        gen2.set_len(ROTATE_BYTES + 1).unwrap();
        drop(gen2);
        let _gen3 = open_rotating(&path).unwrap();
        assert!(
            std::fs::read(rotated_path(&path))
                .unwrap()
                .starts_with(b"gen2"),
            "the second rotation REPLACES .1"
        );
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            2,
            "daemon.log + daemon.log.1 — never a .2"
        );
    }

    #[test]
    fn a_file_exactly_at_the_cap_is_not_rotated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("compute.log");
        open_rotating(&path).unwrap().set_len(ROTATE_BYTES).unwrap();
        let _ = open_rotating(&path).unwrap();
        assert!(!rotated_path(&path).exists(), "larger THAN, not at");
    }
}
