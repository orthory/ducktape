//! the REAL FUSE smoke — gated behind `--features fuse` (the whole file compiles
//! to nothing otherwise). it stands the in-process files-only node (see
//! `support/mod.rs`), seeds a subtree through the phase-3 engine, then drives the
//! `ducktape-fs mount` verb as a spawned child process and reads/writes back
//! THROUGH THE KERNEL (`std::fs` on the mountpoint), proving the mount is a real
//! filesystem end to end.
//!
//! it SKIP-GATES cleanly (prints why and returns success) when `/dev/fuse` is
//! absent, so a CI box without FUSE stays green.
#![cfg(feature = "fuse")]

mod support;

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use duckfs_client::api::NodeApi;
use duckfs_client::http::HttpNode;
use support::Harness;

/// the seeded big file: > 1 MiB so a read crosses chunk/cache-block boundaries.
const BIG_LEN: usize = 2 * 1024 * 1024 + 3;

fn big_bytes() -> Vec<u8> {
    (0..BIG_LEN).map(|i| (i % 251) as u8).collect()
}

/// is unprivileged FUSE usable here? the smoke needs the device node.
fn fuse_available() -> bool {
    Path::new("/dev/fuse").exists()
}

/// seed `/shared` with the spread the mount must handle: a small file, a > 1 MiB
/// file, a nested file, an executable, a symlink, and an empty directory.
fn seed(node_url: &str) {
    use duckfs_client::checkout::{CheckoutOptions, checkout_with};
    use duckfs_client::commit::commit;
    use std::os::unix::fs::PermissionsExt as _;

    let node = HttpNode::new(node_url.to_string());
    let dir = tempfile::tempdir().expect("seed dir");
    let opts = CheckoutOptions {
        node_url: node_url.to_string(),
        ..Default::default()
    };
    checkout_with(&node, dir.path(), "/shared", None, &opts).expect("seed checkout");

    std::fs::write(dir.path().join("note.txt"), b"hello").expect("write note");
    std::fs::write(dir.path().join("big.bin"), big_bytes()).expect("write big");
    std::fs::create_dir_all(dir.path().join("sub")).expect("mkdir sub");
    std::fs::write(dir.path().join("sub/child.txt"), b"child").expect("write child");

    // an executable file — the exec bit must survive into the mount's stat.
    let exec = dir.path().join("run.sh");
    std::fs::write(&exec, b"#!/bin/sh\necho hi\n").expect("write exec");
    std::fs::set_permissions(&exec, std::fs::Permissions::from_mode(0o755)).expect("chmod exec");

    // a symlink whose target is a sibling name.
    std::os::unix::fs::symlink("note.txt", dir.path().join("link.txt")).expect("symlink");

    // an empty directory (exists in the tree with no children).
    std::fs::create_dir_all(dir.path().join("empty")).expect("mkdir empty");

    commit(&node, dir.path(), "seed the fuse surface").expect("seed commit");
}

/// spawn `ducktape-fs mount ...` against `node_url`, inheriting stderr so the
/// banner/errors show up in the test output.
fn spawn_mount(node_url: &str, mnt: &Path, extra: &[&str]) -> Child {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ducktape-fs"));
    cmd.arg("mount")
        .arg("/shared")
        .arg(mnt)
        .arg("--node")
        .arg(node_url)
        .args(extra)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    cmd.spawn().expect("spawn ducktape-fs mount")
}

/// poll `cond` until it holds or the deadline passes.
fn wait_until(timeout: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    cond()
}

/// is `mnt` currently a mountpoint (per /proc/mounts)?
fn is_mounted(mnt: &Path) -> bool {
    let want = std::fs::canonicalize(mnt).unwrap_or_else(|_| mnt.to_path_buf());
    let Ok(mounts) = std::fs::read_to_string("/proc/mounts") else {
        return false;
    };
    mounts.lines().any(|line| {
        line.split_whitespace()
            .nth(1)
            .map(|p| Path::new(p) == want)
            .unwrap_or(false)
    })
}

/// clean shutdown: SIGTERM the child (its handler unmounts), then wait for exit.
fn sigterm_and_wait(mut child: Child) {
    let pid = child.id();
    let _ = Command::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .status();
    // give the unmount + process exit some room.
    let _ = wait_until(Duration::from_secs(20), || {
        matches!(child.try_wait(), Ok(Some(_)))
    });
    let _ = child.wait();
}

/// backing dir a `--rw` mount creates beside the mountpoint (cleaned by the test).
fn backing_of(mnt: &Path) -> PathBuf {
    let mut s = mnt.as_os_str().to_os_string();
    s.push(".duckfs-backing");
    PathBuf::from(s)
}

#[test]
fn ro_mount_reads_back_through_the_kernel() {
    if !fuse_available() {
        eprintln!("SKIP ro_mount: /dev/fuse absent (no unprivileged FUSE here)");
        return;
    }
    let h = Harness::start();
    seed(&h.node_url());

    let mnt = tempfile::tempdir().expect("mountpoint");
    let child = spawn_mount(&h.node_url(), mnt.path(), &[]);

    // ready when the mount serves a known file.
    let note = mnt.path().join("note.txt");
    let ready = wait_until(Duration::from_secs(30), || note.exists());
    assert!(ready, "the RO mount never became ready");

    // small file: byte-exact.
    assert_eq!(std::fs::read(&note).expect("read note"), b"hello");

    // > 1 MiB file: byte-exact across chunk/cache-block boundaries.
    let big = std::fs::read(mnt.path().join("big.bin")).expect("read big");
    assert_eq!(big.len(), BIG_LEN, "big file size");
    assert_eq!(
        big,
        big_bytes(),
        "big file bytes are identical through the kernel"
    );

    // nested file.
    assert_eq!(
        std::fs::read(mnt.path().join("sub/child.txt")).expect("read child"),
        b"child"
    );

    // exec bit survives into stat.
    use std::os::unix::fs::PermissionsExt as _;
    let run_mode = std::fs::metadata(mnt.path().join("run.sh"))
        .expect("stat run.sh")
        .permissions()
        .mode();
    assert!(
        run_mode & 0o111 != 0,
        "run.sh is executable in the mount: {run_mode:o}"
    );
    let note_mode = std::fs::metadata(&note)
        .expect("stat note")
        .permissions()
        .mode();
    assert!(
        note_mode & 0o111 == 0,
        "note.txt is not executable: {note_mode:o}"
    );

    // symlink: is a symlink and points at the target string.
    let link_meta = std::fs::symlink_metadata(mnt.path().join("link.txt")).expect("lstat link");
    assert!(link_meta.file_type().is_symlink(), "link.txt is a symlink");
    assert_eq!(
        std::fs::read_link(mnt.path().join("link.txt")).expect("readlink"),
        Path::new("note.txt")
    );

    // readdir lists every seeded entry including the empty dir.
    let names: Vec<String> = std::fs::read_dir(mnt.path())
        .expect("readdir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    for want in ["note.txt", "big.bin", "sub", "run.sh", "link.txt", "empty"] {
        assert!(
            names.contains(&want.to_string()),
            "readdir has {want}: {names:?}"
        );
    }
    // the empty dir is a real, empty directory.
    let empty = mnt.path().join("empty");
    assert!(empty.is_dir(), "empty is a directory");
    assert_eq!(std::fs::read_dir(&empty).expect("readdir empty").count(), 0);

    // unmount cleanly on SIGTERM.
    assert!(is_mounted(mnt.path()), "mounted before SIGTERM");
    sigterm_and_wait(child);
    let unmounted = wait_until(Duration::from_secs(10), || !is_mounted(mnt.path()));
    assert!(unmounted, "the mountpoint is unmounted after SIGTERM");
}

#[test]
fn rw_mount_write_auto_commits_into_the_module() {
    if !fuse_available() {
        eprintln!("SKIP rw_mount: /dev/fuse absent (no unprivileged FUSE here)");
        return;
    }
    let h = Harness::start();
    seed(&h.node_url());

    let mnt = tempfile::tempdir().expect("mountpoint");
    let backing = backing_of(mnt.path());
    let child = spawn_mount(&h.node_url(), mnt.path(), &["--rw", "--auto-commit", "1"]);

    // ready when the checked-out working copy is served through the mount.
    let ready = wait_until(Duration::from_secs(30), || {
        mnt.path().join("note.txt").exists()
    });
    assert!(ready, "the RW mount never became ready");

    // write a NEW file through the mountpoint.
    let payload = b"written through the fuse mount".to_vec();
    std::fs::write(mnt.path().join("rw_new.txt"), &payload).expect("write through mount");
    assert_eq!(
        std::fs::read(mnt.path().join("rw_new.txt")).unwrap(),
        payload
    );

    // the auto-commit (1s) lands the bytes in the module — verify via the NodeApi.
    let node = HttpNode::new(h.node_url());
    let landed = wait_until(Duration::from_secs(20), || {
        node.stat("/shared/rw_new.txt", None)
            .ok()
            .flatten()
            .is_some()
    });
    assert!(landed, "the write never auto-committed into the module");

    // read the committed bytes straight from the node — byte-identical.
    let (bytes, _eof) = node
        .read("/shared/rw_new.txt", None, 0, 1024 * 1024)
        .expect("node read");
    assert_eq!(
        bytes, payload,
        "committed bytes match what was written through the mount"
    );

    // unmount cleanly.
    sigterm_and_wait(child);
    let unmounted = wait_until(Duration::from_secs(10), || !is_mounted(mnt.path()));
    assert!(unmounted, "the RW mountpoint is unmounted after SIGTERM");

    // the backing checkout is the test's to clean up (it survives unmount so an
    // uncommitted change is never lost — here everything committed).
    let _ = std::fs::remove_dir_all(&backing);
}
