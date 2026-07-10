//! the commit engine over the module-backed mock: the checkout→edit→commit→
//! re-checkout round-trip, HasChunks-probed dedup + resume (stage-call counters),
//! and the atomicity guards (MAX_CHANGES_PER_COMMIT, local NFD names) that fail
//! before any submit.

mod support;

use std::collections::BTreeMap;
use std::fs;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use duckfs_client::checkout::checkout;
use duckfs_client::commit::{CommitError, commit};
use duckfs_client::index::Index;
use duckfs_core::{CHUNK_SIZE, Change, Content};
use support::ModuleNode;

const PREFIX: &str = "/shared/ws";

fn pattern(len: usize, seed: u8) -> Vec<u8> {
    (0..len)
        .map(|i| ((i + seed as usize) % 251) as u8)
        .collect()
}

fn put_inline(path: &str, bytes: &[u8]) -> Change {
    Change::Put {
        path: path.into(),
        exec: false,
        meta: BTreeMap::new(),
        content: Content::Inline {
            b64: STANDARD.encode(bytes),
        },
    }
}

fn put_chunks(node: &ModuleNode, path: &str, bytes: &[u8]) -> Change {
    let digests: Vec<String> = bytes
        .chunks(CHUNK_SIZE as usize)
        .map(|s| node.seed_stage(s).expect("seed stage"))
        .collect();
    Change::Put {
        path: path.into(),
        exec: false,
        meta: BTreeMap::new(),
        content: Content::Chunks {
            size: bytes.len() as u64,
            chunks: digests,
        },
    }
}

// ---- the round trip ----------------------------------------------------------

#[test]
fn checkout_edit_commit_and_re_checkout_round_trip() {
    let node = ModuleNode::new();
    let big0 = pattern(2 * CHUNK_SIZE as usize + 1, 0);
    node.seed_commit(
        None,
        "seed",
        vec![
            put_inline(&format!("{PREFIX}/readme.txt"), b"hello"),
            put_inline(&format!("{PREFIX}/gone.txt"), b"delete me"),
            put_chunks(&node, &format!("{PREFIX}/big.bin"), &big0),
        ],
    )
    .expect("seed");

    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    checkout(&node, root, PREFIX, None).expect("checkout");

    // edit a small file, rewrite the 2 MiB file, delete one, add an empty dir.
    let big1 = pattern(2 * CHUNK_SIZE as usize + 1, 9);
    fs::write(root.join("readme.txt"), b"hello again").unwrap();
    fs::write(root.join("big.bin"), &big1).unwrap();
    fs::remove_file(root.join("gone.txt")).unwrap();
    fs::create_dir(root.join("newdir")).unwrap();

    let summary = commit(&node, root, "edit").expect("commit");
    assert_eq!(
        node.head().as_deref(),
        Some(summary.snapshot.as_str()),
        "head advanced"
    );
    // the index base is the resolved new snapshot (matched by receipt height).
    assert_eq!(
        Index::load(root).unwrap().base_snapshot.as_deref(),
        Some(summary.snapshot.as_str())
    );
    assert!(
        duckfs_client::status::status(root).unwrap().clean,
        "clean right after commit"
    );

    // a fresh checkout into a second dir is byte-identical to the working copy.
    let dir2 = tempfile::tempdir().unwrap();
    checkout(&node, dir2.path(), PREFIX, None).expect("re-checkout");
    assert_eq!(
        fs::read(dir2.path().join("readme.txt")).unwrap(),
        b"hello again"
    );
    assert_eq!(fs::read(dir2.path().join("big.bin")).unwrap(), big1);
    assert!(
        !dir2.path().join("gone.txt").exists(),
        "deletion propagated"
    );
    assert!(dir2.path().join("newdir").is_dir(), "empty dir propagated");
}

// ---- dedup + resume (stage counters) ----------------------------------------

#[test]
fn duplicate_bytes_of_a_committed_file_restage() {
    let node = ModuleNode::new();
    let big = pattern(2 * CHUNK_SIZE as usize + 1, 3); // three distinct chunks
    node.seed_commit(
        None,
        "seed",
        vec![put_chunks(&node, &format!("{PREFIX}/orig.bin"), &big)],
    )
    .expect("seed");

    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    checkout(&node, root, PREFIX, None).expect("checkout");

    // a NEW path whose bytes duplicate the already-committed file. those chunks
    // are durable on disk but NO LONGER staged (a commit consumes the stage), and
    // HasChunks now reports STAGING ONLY — odb presence is per-node (orphan sets
    // diverge across the set), so it can't gate a consensus availability decision
    // (finding #1). the client therefore RE-STAGES all three chunks. this drops
    // the old cross-commit zero-byte dedup, but it is consensus-safe: the bytes
    // ride the block, so every validator lands the identical staging entry — and
    // dedup against the CURRENT staging table still holds (see the resume test).
    fs::write(root.join("dup.bin"), &big).unwrap();

    let before = node.stage_calls.get();
    commit(&node, root, "dup").expect("commit");
    assert_eq!(
        node.stage_calls.get() - before,
        3,
        "a committed-but-unstaged file's bytes re-stage (all three chunks)"
    );
    // and the duplicate really landed.
    let dir2 = tempfile::tempdir().unwrap();
    checkout(&node, dir2.path(), PREFIX, None).unwrap();
    assert_eq!(fs::read(dir2.path().join("dup.bin")).unwrap(), big);
}

#[test]
fn an_interrupted_upload_resumes_with_exactly_the_missing_stages() {
    let node = ModuleNode::new();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    // empty base checkout.
    checkout(&node, root, PREFIX, None).expect("checkout");

    // a fresh 3-chunk file; pre-stage the MIDDLE chunk out-of-band (as if a prior
    // upload attempt got that far).
    let big = pattern(2 * CHUNK_SIZE as usize + 1, 7);
    let slices: Vec<&[u8]> = big.chunks(CHUNK_SIZE as usize).collect();
    assert_eq!(slices.len(), 3);
    node.seed_stage(slices[1])
        .expect("pre-stage the middle chunk");

    fs::write(root.join("big.bin"), &big).unwrap();

    let before = node.stage_calls.get();
    commit(&node, root, "resume").expect("commit");
    assert_eq!(
        node.stage_calls.get() - before,
        2,
        "only the two missing chunks are staged (the third was already present)"
    );
}

// ---- atomicity guards (nothing submitted) -----------------------------------

#[test]
fn over_the_change_cap_fails_before_any_submit() {
    let node = ModuleNode::new();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    checkout(&node, root, PREFIX, None).expect("checkout");

    // 4097 new files — one past MAX_CHANGES_PER_COMMIT (4096).
    for i in 0..4097u32 {
        fs::write(root.join(format!("f{i:05}")), b"x").unwrap();
    }

    let err = commit(&node, root, "flood").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("MAX_CHANGES_PER_COMMIT"),
        "names the cap: {msg}"
    );
    assert!(msg.contains("4097"), "names the count: {msg}");
    assert_eq!(node.commit_calls.get(), 0, "nothing submitted");
    assert_eq!(node.stage_calls.get(), 0, "nothing staged");
}

#[test]
fn a_local_nfd_filename_fails_before_any_submit() {
    let node = ModuleNode::new();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    checkout(&node, root, PREFIX, None).expect("checkout");

    // "café" in NFD (e + combining acute) — a non-canonical name the module would
    // reject; the plan catches it locally first.
    fs::write(root.join("cafe\u{301}.txt"), b"x").unwrap();

    let err = commit(&node, root, "nfd").unwrap_err();
    assert!(
        matches!(err, CommitError::Plan(_)),
        "a plan-time rejection: {err}"
    );
    assert!(
        err.to_string().contains("cafe"),
        "names the offending path: {err}"
    );
    assert_eq!(node.commit_calls.get(), 0, "nothing submitted");
    assert_eq!(node.stage_calls.get(), 0, "nothing staged");
}
