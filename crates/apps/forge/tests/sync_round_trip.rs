//! pack state-sync: a fresh forge reconstructs a source repo's committed state
//! from SELF-CONTAINED snapshot bytes — a 20-byte head oid plus the full
//! object closure as a packfile — and lands on the identical root() with the
//! real commit content intact. the bytes are the whole story: nothing here
//! assumes the two repos share a filesystem, a remote, or a `git` binary.
//!
//! the snapshot is also the module's byzantine surface: install() consumes
//! bytes a malicious peer chose. the negative tests pin the two layers of the
//! defense — the oid prefix must rehash to the expected root BEFORE any byte
//! is written, and the pack must hash-verify object by object BEFORE the ref
//! moves — by asserting the rejected module's root AND its on-disk repo stay
//! untouched.

use std::path::PathBuf;

use forge::Forge;
use forge_interface::{ForgeMsg, encode_msg};
use sdk::{Ctx, Error, Module, Msg, StateRoot};

/// the module's canonical branch — the ref install must (and may only) move.
const MAIN_REF: &str = "refs/heads/main";

/// the snapshot's head-oid header width (a raw sha1 oid).
const OID_LEN: usize = 20;

// a minimal Ctx so execute can read consensus_time without a full host.
struct TestCtx {
    env: sdk::Env,
}
impl TestCtx {
    fn at(consensus_time: u64) -> Self {
        Self {
            env: sdk::Env {
                height: 0,
                consensus_time,
                origin: sdk::Origin::System,
                me: "forge".into(),
            },
        }
    }
}
#[async_trait::async_trait(?Send)]
impl Ctx for TestCtx {
    fn env(&self) -> &sdk::Env {
        &self.env
    }
    fn module_root(&self, _t: &str) -> Option<StateRoot> {
        None
    }
    async fn query(&self, _t: &str, _r: &[u8]) -> Result<Vec<u8>, Error> {
        Err(Error::QueryUnsupported)
    }
    fn emit_msg(&mut self, _m: Msg) {}
    fn emit_event(&mut self, _e: sdk::Event) {}
    fn request_effect(&mut self, _e: sdk::Effect) {}
}

fn tmp_repo(tag: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("ducktape-forge-sync-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    p
}

/// drive one file change through the REAL execute path and publish it — the
/// snapshot source is a normally-operated module, not a hand-built repo.
fn commit_one(forge: &mut Forge, t: u64, path: &str, content: &str, message: &str) {
    let msg = Msg {
        target: "forge".into(),
        payload: encode_msg(&ForgeMsg::Commit {
            path: path.into(),
            content: content.into(),
            message: message.into(),
        }),
    };
    futures::executor::block_on(forge.execute(&mut TestCtx::at(t), &msg)).unwrap();
    futures::executor::block_on(forge.commit_block()).unwrap();
}

/// a two-commit source module — history matters (the pack must ship the whole
/// closure, not just the tip).
fn source(tag: &str) -> (PathBuf, Forge) {
    let dir = tmp_repo(tag);
    let mut forge = Forge::init("forge", dir.clone()).unwrap();
    commit_one(&mut forge, 1, "a.txt", "one", "c1");
    commit_one(&mut forge, 2, "b.txt", "two", "c2");
    (dir, forge)
}

/// a repo dir that is still pristine: no born ref AND no indexed pack — the
/// on-disk oracle that a rejected install wrote NOTHING (not just that the
/// module's cache looks clean).
fn assert_repo_untouched(dir: &PathBuf) {
    let repo = git2::Repository::open(dir).unwrap();
    assert!(
        repo.refname_to_id(MAIN_REF).is_err(),
        "ref must not have moved"
    );
    let packs = std::fs::read_dir(dir.join(".git/objects/pack"))
        .map(|d| d.count())
        .unwrap_or(0);
    assert_eq!(packs, 0, "no pack may have been indexed");
}

#[test]
fn snapshot_reconstructs_root_and_content_on_a_fresh_module() {
    let (src_dir, src) = source("rt-src");
    let root = src.root();
    assert_ne!(root, StateRoot::ZERO, "source must have real state");
    let bytes = src.snapshot().unwrap();

    let dst_dir = tmp_repo("rt-dst");
    let mut dst = Forge::init("forge", dst_dir.clone()).unwrap();
    assert_eq!(dst.root(), StateRoot::ZERO);
    dst.install(&bytes, root).unwrap();

    // THE PROPERTY: identical root — the app-hash linkage a joiner needs.
    assert_eq!(
        dst.root(),
        root,
        "installed root must equal the source root"
    );

    // content oracle: read the installed repo with git2 directly — the head
    // commit, its message, both blobs, and the parent link all came through.
    let repo = git2::Repository::open(&dst_dir).unwrap();
    let head = repo.refname_to_id(MAIN_REF).unwrap();
    let commit = repo.find_commit(head).unwrap();
    assert_eq!(commit.message(), Some("c2"));
    let tree = commit.tree().unwrap();
    let blob = |name: &str| {
        let entry = tree
            .get_name(name)
            .unwrap_or_else(|| panic!("{name} missing from installed tree"));
        repo.find_blob(entry.id()).unwrap().content().to_vec()
    };
    assert_eq!(blob("a.txt"), b"one".to_vec());
    assert_eq!(blob("b.txt"), b"two".to_vec());
    assert_eq!(
        commit.parent(0).unwrap().message(),
        Some("c1"),
        "history must ship, not just the tip"
    );

    let _ = std::fs::remove_dir_all(&src_dir);
    let _ = std::fs::remove_dir_all(&dst_dir);
}

#[test]
fn tampered_oid_prefix_is_rejected_before_anything_is_written() {
    let (src_dir, src) = source("tamper-src");
    let root = src.root();
    let mut bytes = src.snapshot().unwrap();
    bytes[0] ^= 0xff; // the oid no longer rehashes to `root`

    let dst_dir = tmp_repo("tamper-dst");
    let mut dst = Forge::init("forge", dst_dir.clone()).unwrap();
    let err = dst.install(&bytes, root).unwrap_err();
    assert!(matches!(err, Error::Module(_)));
    assert_eq!(
        dst.root(),
        StateRoot::ZERO,
        "a rejected snapshot must leave the root untouched"
    );
    assert_repo_untouched(&dst_dir);

    let _ = std::fs::remove_dir_all(&src_dir);
    let _ = std::fs::remove_dir_all(&dst_dir);
}

#[test]
fn corrupt_or_garbage_pack_bytes_are_rejected() {
    let (src_dir, src) = source("garbage-src");
    let root = src.root();
    let bytes = src.snapshot().unwrap();

    let dst_dir = tmp_repo("garbage-dst");
    let mut dst = Forge::init("forge", dst_dir.clone()).unwrap();

    // an honest oid prefix over pure garbage instead of a pack.
    let mut garbage = bytes[..OID_LEN].to_vec();
    garbage.extend(std::iter::repeat_n(0xab, 256));
    assert!(
        dst.install(&garbage, root).is_err(),
        "garbage pack must be rejected"
    );

    // a real pack with its trailer checksum flipped.
    let mut flipped = bytes.clone();
    *flipped.last_mut().unwrap() ^= 0xff;
    assert!(
        dst.install(&flipped, root).is_err(),
        "corrupted pack must be rejected"
    );

    // truncation below the oid header.
    assert!(
        dst.install(&bytes[..10], root).is_err(),
        "truncated snapshot must be rejected"
    );

    // failed packs may strand junk in the odb, but the ref never moved and
    // the root is byte-identical to before every attempt.
    assert_eq!(dst.root(), StateRoot::ZERO);
    let repo = git2::Repository::open(&dst_dir).unwrap();
    assert!(
        repo.refname_to_id(MAIN_REF).is_err(),
        "ref must not have moved"
    );

    let _ = std::fs::remove_dir_all(&src_dir);
    let _ = std::fs::remove_dir_all(&dst_dir);
}

#[test]
fn empty_snapshot_round_trips_the_unborn_state() {
    let src_dir = tmp_repo("empty-src");
    let src = Forge::init("forge", src_dir.clone()).unwrap();
    let bytes = src.snapshot().unwrap();
    assert_eq!(
        bytes,
        vec![0u8; OID_LEN],
        "unborn state must serialize as the zero-oid marker"
    );

    let dst_dir = tmp_repo("empty-dst");
    let mut dst = Forge::init("forge", dst_dir.clone()).unwrap();
    dst.install(&bytes, StateRoot::ZERO).unwrap();
    assert_eq!(dst.root(), StateRoot::ZERO);

    // the marker binds to ZERO the way root() does — any other expectation
    // fails…
    assert!(
        dst.install(&bytes, StateRoot([1u8; sdk::ROOT_LEN]))
            .is_err()
    );
    // …and it must not smuggle trailing bytes.
    let mut padded = bytes.clone();
    padded.push(0);
    assert!(dst.install(&padded, StateRoot::ZERO).is_err());

    let _ = std::fs::remove_dir_all(&src_dir);
    let _ = std::fs::remove_dir_all(&dst_dir);
}

#[test]
fn install_replaces_a_divergent_head() {
    let (src_dir, src) = source("replace-src");
    let root = src.root();
    let bytes = src.snapshot().unwrap();

    let dst_dir = tmp_repo("replace-dst");
    let mut dst = Forge::init("forge", dst_dir.clone()).unwrap();
    commit_one(&mut dst, 9, "z.txt", "other", "unrelated");
    assert_ne!(
        dst.root(),
        root,
        "the destination starts on an unrelated head"
    );

    // install is a replacement, not a merge: the unrelated (non-fast-forward)
    // head is overwritten.
    dst.install(&bytes, root).unwrap();
    assert_eq!(dst.root(), root);

    let _ = std::fs::remove_dir_all(&src_dir);
    let _ = std::fs::remove_dir_all(&dst_dir);
}

#[test]
fn a_partial_closure_pack_is_rejected_before_the_ref_moves() {
    let (src_dir, src) = source("partial-src");
    let expected = src.root();
    let full = src.snapshot().unwrap();
    let head = git2::Oid::from_bytes(&full[..20]).unwrap();

    // byzantine pack: the GENUINE head commit and its root tree, nothing else.
    // every carried object hash-checks and the oid rehashes to `expected`, but
    // the blobs and the parent commit are missing — only a closure walk between
    // pack indexing and the ref move can catch it.
    let repo = git2::Repository::open(&src_dir).unwrap();
    let head_commit = repo.find_commit(head).unwrap();
    let mut pb = repo.packbuilder().unwrap();
    pb.insert_object(head, None).unwrap();
    pb.insert_object(head_commit.tree_id(), None).unwrap();
    let mut buf = git2::Buf::new();
    pb.write_buf(&mut buf).unwrap();
    let mut bytes = head.as_bytes().to_vec();
    bytes.extend_from_slice(&buf);

    let dst_dir = tmp_repo("partial-dst");
    let mut dst = Forge::init("forge", dst_dir).unwrap();
    let err = dst.install(&bytes, expected).unwrap_err();
    assert!(
        matches!(err, Error::Module(_)),
        "incomplete closure errs with Module"
    );
    assert_eq!(dst.root(), StateRoot::ZERO, "the ref never moved");
}

#[test]
fn empty_state_installed_over_a_born_repo_unbinds_it_durably() {
    // an empty source: its snapshot is the zero-oid marker.
    let empty_dir = tmp_repo("unbind-empty-src");
    let empty = Forge::init("forge", empty_dir).unwrap();
    let marker = empty.snapshot().unwrap();

    // a BORN destination with real history.
    let (dst_dir, mut dst) = source("born-dst");
    assert_ne!(dst.root(), StateRoot::ZERO, "destination starts born");

    dst.install(&marker, StateRoot::ZERO).unwrap();
    assert_eq!(
        dst.root(),
        StateRoot::ZERO,
        "in-memory root returns to ZERO"
    );

    // durability: a re-init re-reads the ref from DISK — if install had only
    // cleared the cached head and left the ref, the old root would resurrect
    // here and the app-hash would diverge from the consensus-expected ZERO.
    drop(dst);
    let reopened = Forge::init("forge", dst_dir).unwrap();
    assert_eq!(reopened.root(), StateRoot::ZERO, "the on-disk ref is gone");
}
