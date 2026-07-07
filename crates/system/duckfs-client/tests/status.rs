//! worktree status with git's index discipline — the mtime+size fast path, the
//! racy-clean rehash rule, and the A/M/D/exec/symlink cases.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use duckfs_client::chunk::{chunk_ids, file_object_id};
use duckfs_client::index::{EntryKind, Index, IndexEntry};
use duckfs_client::status::status;
use files::to_hex;

const PREFIX: &str = "/shared/ws";

fn dpath(rel: &str) -> String {
    format!("{PREFIX}/{rel}")
}

/// the file object id (hex) for `bytes` with empty meta — the recorded `object`.
fn file_id_hex(bytes: &[u8]) -> String {
    to_hex(&file_object_id(
        bytes.len() as u64,
        &chunk_ids(bytes),
        &BTreeMap::new(),
    ))
}

fn set_mtime(path: &Path, t: SystemTime) {
    let f = fs::File::options().write(true).open(path).unwrap();
    f.set_times(fs::FileTimes::new().set_modified(t)).unwrap();
}

fn record_file(idx: &mut Index, root: &Path, rel: &str, exec: bool) {
    let disk = root.join(rel);
    let bytes = fs::read(&disk).unwrap();
    let meta = fs::metadata(&disk).unwrap();
    use std::os::unix::fs::MetadataExt as _;
    idx.entries.insert(
        dpath(rel),
        IndexEntry {
            object: file_id_hex(&bytes),
            size: bytes.len() as u64,
            mtime_secs: meta.mtime(),
            mtime_nanos: meta.mtime_nsec() as u32,
            exec,
            kind: EntryKind::File,
            meta: BTreeMap::new(),
        },
    );
}

fn paths(entries: &[duckfs_client::scan::ScanEntry]) -> Vec<String> {
    entries.iter().map(|e| e.path.clone()).collect()
}

// ---- the racy-clean trap ----------------------------------------------------

#[test]
fn racily_clean_same_size_edit_is_reported_modified() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // write a file and record its ORIGINAL content id + mtime in the index.
    let file = root.join("a.txt");
    fs::write(&file, b"aaaaa").unwrap();
    let mut idx = Index::new(PREFIX, "http://node", None);
    record_file(&mut idx, root, "a.txt", false);
    idx.save(root).unwrap();

    // rewrite SAME-SIZE different content, then backdate the file's mtime to the
    // recorded value AND make the index file appear no newer (same instant) — the
    // classic racy-clean setup: a naive mtime+size fast path sees "unchanged".
    fs::write(&file, b"bbbbb").unwrap();
    let recorded = &idx.entries[&dpath("a.txt")];
    let t = UNIX_EPOCH + Duration::new(recorded.mtime_secs as u64, recorded.mtime_nanos);
    set_mtime(&file, t);
    set_mtime(&Index::path(root), t);

    let st = status(root).unwrap();
    assert!(
        paths(&st.modified).contains(&dpath("a.txt")),
        "a same-size edit at the recorded mtime must rehash and report modified: {:?}",
        paths(&st.modified)
    );
    assert!(!st.clean, "the worktree is dirty");
}

// ---- A / M / D / exec / symlink + the untouched-is-clean fast path ----------

#[test]
fn added_removed_exec_and_symlink_cases() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // a big (2 MiB + 1) file that stays untouched — the clean fast path.
    let big = (0..(2 * 1024 * 1024 + 1))
        .map(|i| (i % 251) as u8)
        .collect::<Vec<u8>>();
    fs::write(root.join("big.bin"), &big).unwrap();
    // an exec file whose bit we will flip.
    let ex = root.join("run.sh");
    fs::write(&ex, b"#!/bin/sh\n").unwrap();
    fs::set_permissions(&ex, fs::Permissions::from_mode(0o755)).unwrap();
    // a symlink we will retarget.
    symlink("target-one", root.join("link")).unwrap();
    // a file we will delete.
    fs::write(root.join("gone.txt"), b"delete me").unwrap();

    let mut idx = Index::new(PREFIX, "http://node", None);
    record_file(&mut idx, root, "big.bin", false);
    record_file(&mut idx, root, "run.sh", true);
    record_file(&mut idx, root, "gone.txt", false);
    // record the symlink (object = file id over the target bytes).
    {
        use std::os::unix::fs::MetadataExt as _;
        let meta = fs::symlink_metadata(root.join("link")).unwrap();
        idx.entries.insert(
            dpath("link"),
            IndexEntry {
                object: file_id_hex(b"target-one"),
                size: "target-one".len() as u64,
                mtime_secs: meta.mtime(),
                mtime_nanos: meta.mtime_nsec() as u32,
                exec: false,
                kind: EntryKind::Symlink,
                meta: BTreeMap::new(),
            },
        );
    }
    idx.save(root).unwrap();

    // mutate: add a new file, flip the exec bit off, retarget the symlink, delete
    // gone.txt. leave big.bin untouched.
    fs::write(root.join("new.txt"), b"fresh").unwrap();
    fs::set_permissions(&ex, fs::Permissions::from_mode(0o644)).unwrap();
    fs::remove_file(root.join("link")).unwrap();
    symlink("target-two", root.join("link")).unwrap();
    fs::remove_file(root.join("gone.txt")).unwrap();

    let st = status(root).unwrap();
    assert!(
        paths(&st.added).contains(&dpath("new.txt")),
        "new file is added"
    );
    assert!(
        !paths(&st.modified).contains(&dpath("big.bin")),
        "the untouched big file is clean (fast path): {:?}",
        paths(&st.modified)
    );
    assert!(
        paths(&st.modified).contains(&dpath("run.sh")),
        "an exec-bit-only flip is modified"
    );
    assert!(
        paths(&st.modified).contains(&dpath("link")),
        "a retargeted symlink is modified"
    );
    assert!(
        st.removed.contains(&dpath("gone.txt")),
        "the deleted file is removed"
    );
    assert!(!st.clean);
}

#[test]
fn a_new_empty_dir_is_added_but_a_recorded_one_is_clean() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // one empty dir already in the base (recorded), one created after checkout.
    fs::create_dir(root.join("kept")).unwrap();
    let mut idx = Index::new(PREFIX, "http://node", None);
    {
        use std::os::unix::fs::MetadataExt as _;
        let meta = fs::metadata(root.join("kept")).unwrap();
        idx.entries.insert(
            dpath("kept"),
            IndexEntry {
                object: String::new(),
                size: 0,
                mtime_secs: meta.mtime(),
                mtime_nanos: meta.mtime_nsec() as u32,
                exec: false,
                kind: EntryKind::Dir,
                meta: BTreeMap::new(),
            },
        );
    }
    idx.save(root).unwrap();

    fs::create_dir(root.join("fresh")).unwrap();

    let st = status(root).unwrap();
    assert!(
        paths(&st.added).contains(&dpath("fresh")),
        "a new empty dir is added"
    );
    assert!(
        !paths(&st.added).contains(&dpath("kept")),
        "a recorded empty dir is clean, not re-added: {:?}",
        paths(&st.added)
    );
}
