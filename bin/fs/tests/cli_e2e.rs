//! e2e for `ducktape-fs` against an in-process files-only node (see
//! `support/mod.rs`). the read verbs are driven as a real subprocess over the
//! node's http surface; seeding rides the `duckfs-client` engine (checkout →
//! write → commit), the same path the CLI's own working-copy verbs use.

mod support;

use support::Harness;

/// seed `/shared` with a small file, a >1 MiB file, and a nested file. returns
/// the big file's bytes so `cat` can be checked byte-exact.
fn seed(node_url: &str) -> Vec<u8> {
    use duckfs_client::checkout::{CheckoutOptions, checkout_with};
    use duckfs_client::commit::commit;
    use duckfs_client::http::HttpNode;

    let node = HttpNode::new(node_url.to_string());
    let dir = tempfile::tempdir().expect("seed dir");
    let opts = CheckoutOptions {
        node_url: node_url.to_string(),
        ..Default::default()
    };
    checkout_with(&node, dir.path(), "/shared", None, &opts).expect("seed checkout");

    std::fs::write(dir.path().join("note.txt"), b"hello").expect("write note");
    let big: Vec<u8> = (0..(2 * 1024 * 1024 + 3))
        .map(|i| (i % 251) as u8)
        .collect();
    std::fs::write(dir.path().join("big.bin"), &big).expect("write big");
    std::fs::create_dir_all(dir.path().join("sub")).expect("mkdir sub");
    std::fs::write(dir.path().join("sub/child.txt"), b"child").expect("write child");

    commit(&node, dir.path(), "seed the read surface").expect("seed commit");
    big
}

#[test]
fn ls_lists_the_seeded_entries() {
    let h = Harness::start();
    seed(&h.node_url());

    let out = h.cli(&["ls", "/shared"]).output().expect("run ls");
    assert!(out.status.success(), "ls exits 0: {:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("note.txt"), "ls names note.txt: {stdout}");
    assert!(stdout.contains("big.bin"), "ls names big.bin: {stdout}");
    assert!(stdout.contains("sub"), "ls names the sub dir: {stdout}");
}

#[test]
fn cat_streams_bytes_exactly_including_a_large_file() {
    let h = Harness::start();
    let big = seed(&h.node_url());

    let small = h
        .cli(&["cat", "/shared/note.txt"])
        .output()
        .expect("cat small");
    assert!(small.status.success());
    assert_eq!(small.stdout, b"hello", "small file bytes match");

    let large = h
        .cli(&["cat", "/shared/big.bin"])
        .output()
        .expect("cat big");
    assert!(large.status.success());
    assert_eq!(large.stdout, big, ">1 MiB file streams byte-exact");
}

#[test]
fn stat_reports_the_entry_facts() {
    let h = Harness::start();
    seed(&h.node_url());

    let out = h.cli(&["stat", "/shared/note.txt"]).output().expect("stat");
    assert!(out.status.success(), "stat exits 0: {:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("file"), "stat names the kind: {stdout}");
    assert!(stdout.contains('5'), "stat reports the size 5: {stdout}");
}

#[test]
fn history_lists_the_commit() {
    let h = Harness::start();
    seed(&h.node_url());

    let out = h.cli(&["history"]).output().expect("history");
    assert!(out.status.success(), "history exits 0: {:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("seed the read surface"),
        "history shows the commit message: {stdout}"
    );
}

#[test]
fn missing_node_address_is_a_clear_error() {
    let h = Harness::start();

    let out = h
        .cli_bare(&["ls", "/shared"])
        .env_remove("DUCKTAPE_NODE")
        .output()
        .expect("run ls without a node");
    assert!(!out.status.success(), "no node address must fail");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--node") || stderr.contains("DUCKTAPE_NODE"),
        "the error names the resolution options: {stderr}"
    );
}

#[test]
fn mount_is_a_reserved_phase_4_stub() {
    let h = Harness::start();

    let out = h
        .cli_bare(&["mount", "x", "y"])
        .output()
        .expect("run mount");
    assert!(!out.status.success(), "the mount stub fails");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.to_lowercase().contains("phase 4"),
        "mount points at phase 4: {stderr}"
    );
}
