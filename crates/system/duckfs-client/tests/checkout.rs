//! checkout materialization over the module-backed mock: byte-exactness, exec
//! bits, symlinks, empty dirs, object-id verification, the case-collision guard,
//! and resumability.

mod support;

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use duckfs_client::checkout::{CheckoutError, CheckoutOptions, checkout, checkout_with};
use duckfs_client::index::Index;
use files::{CHUNK_SIZE, Change, Content};
use support::ModuleNode;

const PREFIX: &str = "/shared/ws";

fn pattern(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

fn put_inline(path: &str, bytes: &[u8], exec: bool) -> Change {
    Change::Put {
        path: path.into(),
        exec,
        meta: BTreeMap::new(),
        content: Content::Inline {
            b64: STANDARD.encode(bytes),
        },
    }
}

fn put_chunks(path: &str, size: u64, chunks: Vec<String>) -> Change {
    Change::Put {
        path: path.into(),
        exec: false,
        meta: BTreeMap::new(),
        content: Content::Chunks { size, chunks },
    }
}

/// seed the mock with a representative tree under /shared/ws and return the head
/// snapshot plus the 2 MiB+1 pattern bytes.
fn seed_tree(node: &ModuleNode) -> (String, Vec<u8>) {
    // a 2 MiB + 1 file, staged as three chunks through the (seed) putblob path.
    let big = pattern(2 * CHUNK_SIZE as usize + 1);
    let digests: Vec<String> = big
        .chunks(CHUNK_SIZE as usize)
        .map(|slice| node.seed_stage(slice).expect("stage"))
        .collect();

    let nfc_name = format!("{PREFIX}/caf\u{e9}.txt"); // precomposed é — canonical NFC
    let changes = vec![
        put_inline(&format!("{PREFIX}/readme.txt"), b"hello duckfs", false),
        put_inline(&format!("{PREFIX}/sub/nested.txt"), b"nested body", false),
        put_inline(&format!("{PREFIX}/run.sh"), b"#!/bin/sh\necho hi\n", true),
        put_inline(&nfc_name, b"unicode", false),
        Change::Mkdir {
            path: format!("{PREFIX}/emptydir"),
        },
        Change::Symlink {
            path: format!("{PREFIX}/link"),
            target: "readme.txt".into(),
        },
        put_chunks(&format!("{PREFIX}/big.bin"), big.len() as u64, digests),
    ];
    node.seed_commit(None, "seed", changes)
        .expect("seed commit");
    (node.head().expect("head after seed"), big)
}

#[test]
fn checkout_materializes_the_tree_byte_exact() {
    let node = ModuleNode::new();
    let (head, big) = seed_tree(&node);

    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    checkout(&node, root, PREFIX, None).expect("checkout");

    // files byte-exact.
    assert_eq!(fs::read(root.join("readme.txt")).unwrap(), b"hello duckfs");
    assert_eq!(
        fs::read(root.join("sub/nested.txt")).unwrap(),
        b"nested body"
    );
    assert_eq!(fs::read(root.join("caf\u{e9}.txt")).unwrap(), b"unicode");
    assert_eq!(
        fs::read(root.join("big.bin")).unwrap(),
        big,
        "2 MiB file round-trips"
    );

    // exec bit set on run.sh, clear on a normal file.
    let mode = fs::metadata(root.join("run.sh"))
        .unwrap()
        .permissions()
        .mode();
    assert!(mode & 0o111 != 0, "exec bit set: {mode:o}");
    let plain = fs::metadata(root.join("readme.txt"))
        .unwrap()
        .permissions()
        .mode();
    assert!(plain & 0o111 == 0, "normal file not exec: {plain:o}");

    // symlink target exact.
    assert_eq!(
        fs::read_link(root.join("link")).unwrap().to_str().unwrap(),
        "readme.txt"
    );

    // empty dir present.
    assert!(root.join("emptydir").is_dir(), "empty dir materialized");
    assert!(
        fs::read_dir(root.join("emptydir"))
            .unwrap()
            .next()
            .is_none(),
        "and it is empty"
    );

    // the index records the head as base.
    let idx = Index::load(root).expect("index loads");
    assert_eq!(
        idx.base_snapshot,
        Some(head),
        "index base is the checked-out head"
    );
    assert_eq!(idx.prefix, PREFIX);
    // and status is clean right after checkout (index written last -> fast path).
    assert!(
        duckfs_client::status::status(root).unwrap().clean,
        "a fresh checkout is clean"
    );
}

#[test]
fn case_colliding_siblings_fail_on_a_case_insensitive_fs() {
    let node = ModuleNode::new();
    // both are legal in consensus (case-sensitive) but collide when folded.
    node.seed_commit(
        None,
        "seed",
        vec![
            put_inline(&format!("{PREFIX}/Readme"), b"upper", false),
            put_inline(&format!("{PREFIX}/readme"), b"lower", false),
        ],
    )
    .expect("seed");

    let dir = tempfile::tempdir().unwrap();
    let opts = CheckoutOptions {
        force_case_insensitive: true,
        ..Default::default()
    };
    let err = checkout_with(&node, dir.path(), PREFIX, None, &opts).unwrap_err();
    match err {
        CheckoutError::CaseCollision(paths) => {
            assert!(
                paths.contains(&format!("{PREFIX}/Readme")),
                "lists Readme: {paths:?}"
            );
            assert!(
                paths.contains(&format!("{PREFIX}/readme")),
                "lists readme: {paths:?}"
            );
        }
        other => panic!("expected a case-collision error, got {other:?}"),
    }
}

#[test]
fn checkout_is_resumable_over_a_half_materialized_dir() {
    let node = ModuleNode::new();
    let (_head, _big) = seed_tree(&node);

    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    checkout(&node, root, PREFIX, None).expect("first checkout");

    // simulate a half-materialized dir: delete a file and the symlink.
    fs::remove_file(root.join("readme.txt")).unwrap();
    fs::remove_file(root.join("link")).unwrap();

    // a re-run converges: the deleted entries come back and status is clean.
    checkout(&node, root, PREFIX, None).expect("second checkout converges");
    assert_eq!(fs::read(root.join("readme.txt")).unwrap(), b"hello duckfs");
    assert_eq!(
        fs::read_link(root.join("link")).unwrap().to_str().unwrap(),
        "readme.txt"
    );
    assert!(
        duckfs_client::status::status(root).unwrap().clean,
        "converged clean"
    );
}
