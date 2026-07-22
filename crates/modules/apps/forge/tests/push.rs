//! the push lane (`PushRefs`): a git-faithful ref update over consensus.
//!
//! forge's committed HEAD is decoupled from the on-disk libgit2 objects —
//! `root() = sha256(<HEAD oid>)`, a pure function of the oid alone. a push
//! exploits exactly that: the ONLY consensus effect is a compare-and-swap on
//! the committed HEAD; the git objects ride out-of-band in a NODE-LOCAL
//! packfile (fetched from the files blob store by digest) and are installed
//! lazily by `materialize`, never influencing root/accept-reject.
//!
//! the load-bearing test is
//! [`determinism_a_pushed_root_is_identical_without_the_pack`]: a forge whose
//! blob store LACKS the pack reaches the SAME root as one that has it. that is
//! the fork-safety invariant — pack possession is per-node, root is not.

use std::path::{Path, PathBuf};

use forge::Forge;
use forge::{ForgeMsg, ForgeQuery, ForgeReply, RefUpdate, decode_reply, encode_msg, encode_query};
use sdk::{Error, Module, Msg, StateRoot};

/// the module's canonical branch — the ref a materialized push moves.
const MAIN_REF: &str = "refs/heads/main";
/// the raw width of a sha1 oid (a push `prev_oid`/`new_oid` field).
const OID_LEN: usize = 20;
/// every op here targets the default repo, whose git dir is `base/default`.
const DEFAULT_REPO: &str = "default";

use sdk_testkit::TestCtx;

// forge's execute reads only env (consensus_time); me/height are cosmetic.
fn at(consensus_time: u64) -> TestCtx {
    TestCtx::with_env(sdk::Env {
        height: 0,
        consensus_time,
        origin: sdk::Origin::System,
        me: "forge".into(),
    })
}

fn tmp_repo(tag: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("ducktape-forge-push-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    p
}

/// the default repo's git dir under a forge base.
fn repo_dir(base: &Path) -> PathBuf {
    base.join(DEFAULT_REPO)
}

/// parse forge's multi-branch snapshot container into `(name, main_oid, pack)`
/// entries — the test-side inverse of `Forge::snapshot` (per repo: name,
/// ref_count, per-ref branch+oid, pack; the trailing tracker section is
/// irrelevant here).
fn parse_container(bytes: &[u8]) -> Vec<(String, Vec<u8>, Vec<u8>)> {
    fn u32_at(bytes: &[u8], p: &mut usize) -> usize {
        let v = u32::from_le_bytes(bytes[*p..*p + 4].try_into().unwrap()) as usize;
        *p += 4;
        v
    }
    let mut p = 4; // skip the 4-byte "FGv1" container magic
    let count = u32_at(bytes, &mut p);
    let mut out = Vec::new();
    for _ in 0..count {
        let nl = u32_at(bytes, &mut p);
        let name = String::from_utf8(bytes[p..p + nl].to_vec()).unwrap();
        p += nl;
        let ref_count = u32_at(bytes, &mut p);
        let mut main_oid = Vec::new();
        for _ in 0..ref_count {
            let bl = u32_at(bytes, &mut p);
            let branch = String::from_utf8(bytes[p..p + bl].to_vec()).unwrap();
            p += bl;
            let oid = bytes[p..p + OID_LEN].to_vec();
            p += OID_LEN;
            if branch == "main" {
                main_oid = oid;
            }
        }
        let pl = u32_at(bytes, &mut p);
        let pack = bytes[p..p + pl].to_vec();
        p += pl;
        out.push((name, main_oid, pack));
    }
    out
}

/// drive one file change through the REAL commit path (execute + publish) on the
/// default repo (empty `repo` -> back-compat default).
fn commit_one(forge: &mut Forge, t: u64, path: &str, content: &str, message: &str) {
    let msg = Msg {
        target: "forge".into(),
        payload: encode_msg(&ForgeMsg::Commit {
            repo: String::new(),
            path: path.into(),
            content: content.into(),
            message: message.into(),
        }),
    };
    futures::executor::block_on(forge.execute(&mut at(t), &msg)).unwrap();
    futures::executor::block_on(forge.commit_block()).unwrap();
}

/// a captured push input: a real commit's head oid, the packfile of its full
/// object closure (exactly what `snapshot()` carries after its 20-byte oid
/// header — the same closure `install` consumes, so materialize's git plumbing
/// is proven the way state sync is), and the source `root()` that head produced
/// (== `sha256(head oid)`, the value a faithful push must reproduce).
struct Captured {
    head: Vec<u8>,
    pack: Vec<u8>,
    root: StateRoot,
}
impl Captured {
    /// the head oid as a git2 `Oid`.
    fn oid(&self) -> git2::Oid {
        git2::Oid::from_bytes(&self.head).unwrap()
    }
    /// store the pack in a blob handle and return the digest to push with.
    fn stash(&self, blobs: &blobstore::BlobHandle) -> Vec<u8> {
        blobs.put_chunk(self.pack.clone()).to_vec()
    }
}

/// capture the current committed head of `src` (oid + full-closure pack + root).
/// the head + pack come straight out of `snapshot()` (parsed from the single
/// default-repo container) so materialize's git plumbing is proven the same way
/// state sync is.
fn capture(src: &Forge) -> Captured {
    let snap = src.snapshot().unwrap();
    let mut entries = parse_container(&snap);
    assert_eq!(entries.len(), 1, "a single born default repo");
    let (name, head, pack) = entries.remove(0);
    assert_eq!(name, DEFAULT_REPO);
    assert!(!pack.is_empty(), "a born head must carry a pack");
    Captured {
        head,
        pack,
        root: src.root(),
    }
}

/// build a single-update `PushRefs` on `branch` of the default repo (empty `repo`).
fn push_branch_msg(branch: &str, prev: Option<&[u8]>, new: &[u8], digest: &[u8]) -> Msg {
    Msg {
        target: "forge".into(),
        payload: encode_msg(&ForgeMsg::PushRefs {
            repo: String::new(),
            updates: vec![RefUpdate {
                ref_name: branch.into(),
                prev_oid: prev.map(<[u8]>::to_vec),
                new_oid: Some(new.to_vec()),
            }],
            pack_digest: Some(digest.to_vec()),
        }),
    }
}

/// build a single-update `PushRefs` on `main` of the default repo (empty `repo`).
fn push_msg(prev: Option<&[u8]>, new: &[u8], digest: &[u8]) -> Msg {
    push_branch_msg("main", prev, new, digest)
}

/// execute a Push and publish the block — the happy path.
fn push(forge: &mut Forge, prev: Option<&[u8]>, new: &[u8], digest: &[u8]) {
    futures::executor::block_on(forge.execute(&mut at(0), &push_msg(prev, new, digest)))
        .unwrap();
    futures::executor::block_on(forge.commit_block()).unwrap();
}

/// execute a Push WITHOUT publishing — used to observe a rejection in isolation
/// (a rejected op never stages, so there is nothing to commit).
fn try_push(
    forge: &mut Forge,
    prev: Option<&[u8]>,
    new: &[u8],
    digest: &[u8],
) -> Result<(), Error> {
    futures::executor::block_on(forge.execute(&mut at(0), &push_msg(prev, new, digest)))
}

/// every `(name, head)` pair `query(ListRepos)` reports.
fn repo_heads(forge: &Forge) -> Vec<(String, Option<String>)> {
    let reply =
        futures::executor::block_on(forge.query(&encode_query(&ForgeQuery::ListRepos))).unwrap();
    match decode_reply(&reply).unwrap() {
        ForgeReply::Repos(repos) => repos.into_iter().map(|r| (r.name, r.head)).collect(),
        other => panic!("expected Repos, got {other:?}"),
    }
}

/// the current head hex reported by `query(Head)`.
fn head_query(forge: &Forge) -> Option<String> {
    let reply = futures::executor::block_on(forge.query(&encode_query(&ForgeQuery::Head))).unwrap();
    match decode_reply(&reply).unwrap() {
        ForgeReply::Head(h) => h,
        other => panic!("expected Head, got {other:?}"),
    }
}

/// the on-disk `MAIN_REF` oid of the default repo, or `None` if unborn (or the
/// repo dir doesn't exist yet) — the independent oracle that materialization
/// really moved (or did not move) the real ref.
fn on_disk_head(base: &Path) -> Option<git2::Oid> {
    git2::Repository::open(repo_dir(base))
        .ok()?
        .refname_to_id(MAIN_REF)
        .ok()
}

/// read a file's bytes out of the default repo's materialized head tree.
fn read_blob(base: &Path, name: &str) -> Vec<u8> {
    let repo = git2::Repository::open(repo_dir(base)).unwrap();
    let head = repo.refname_to_id(MAIN_REF).unwrap();
    let tree = repo.find_commit(head).unwrap().tree().unwrap();
    let entry = tree
        .get_name(name)
        .unwrap_or_else(|| panic!("{name} missing from materialized tree"));
    repo.find_blob(entry.id()).unwrap().content().to_vec()
}

/// a single-commit source module + its captured push input.
fn source_one(tag: &str) -> (PathBuf, Forge, Captured) {
    let dir = tmp_repo(tag);
    let mut src = Forge::init("forge", dir.clone()).unwrap();
    commit_one(&mut src, 1, "a.txt", "hello", "first");
    let cap = capture(&src);
    (dir, src, cap)
}

// ---------------------------------------------------------------------------

#[test]
fn push_to_unborn_moves_head_and_materializes_content() {
    let (src_dir, _src, cap) = source_one("unborn-src");
    assert_ne!(cap.root, StateRoot::ZERO, "source has real state");

    // a blob store that HOLDS the pack (the submitter's situation).
    let blobs = blobstore::BlobHandle::default();
    let digest = cap.stash(&blobs);

    let dst_dir = tmp_repo("unborn-dst");
    let mut dst = Forge::with_blobs("forge", dst_dir.clone(), blobs).unwrap();
    assert_eq!(dst.root(), StateRoot::ZERO, "unborn remote");

    push(&mut dst, None, &cap.head, &digest);

    // (a) root moved to sha256(new_oid) == the source root.
    assert_eq!(dst.root(), cap.root, "root becomes sha256(new_oid)");
    // (b) Head query returns the new oid hex.
    assert_eq!(head_query(&dst), Some(cap.oid().to_string()));
    // (c) content materialized: the REAL on-disk ref moved and the blob reads back.
    assert_eq!(on_disk_head(&dst_dir), Some(cap.oid()), "ref materialized");
    assert_eq!(read_blob(&dst_dir, "a.txt"), b"hello".to_vec());

    let _ = std::fs::remove_dir_all(&src_dir);
    let _ = std::fs::remove_dir_all(&dst_dir);
}

#[test]
fn fast_forward_push_advances_the_head() {
    // a source that commits twice, captured after each commit.
    let src_dir = tmp_repo("ff-src");
    let mut src = Forge::init("forge", src_dir.clone()).unwrap();
    commit_one(&mut src, 1, "a.txt", "one", "c1");
    let c1 = capture(&src);
    commit_one(&mut src, 2, "b.txt", "two", "c2");
    let c2 = capture(&src);

    let blobs = blobstore::BlobHandle::default();
    let d1 = c1.stash(&blobs);
    let d2 = c2.stash(&blobs);

    let dst_dir = tmp_repo("ff-dst");
    let mut dst = Forge::with_blobs("forge", dst_dir.clone(), blobs).unwrap();

    push(&mut dst, None, &c1.head, &d1);
    assert_eq!(dst.root(), c1.root);
    assert_eq!(on_disk_head(&dst_dir), Some(c1.oid()));

    // fast-forward: prev == the current committed head.
    push(&mut dst, Some(&c1.head), &c2.head, &d2);
    assert_eq!(dst.root(), c2.root, "head advanced to the second commit");
    assert_eq!(on_disk_head(&dst_dir), Some(c2.oid()), "ref fast-forwarded");
    assert_eq!(read_blob(&dst_dir, "b.txt"), b"two".to_vec());
    // history came through the closure pack, not just the tip.
    let repo = git2::Repository::open(repo_dir(&dst_dir)).unwrap();
    assert_eq!(
        repo.find_commit(c2.oid()).unwrap().parent(0).unwrap().id(),
        c1.oid(),
        "the pushed tip descends from the prior head"
    );

    let _ = std::fs::remove_dir_all(&src_dir);
    let _ = std::fs::remove_dir_all(&dst_dir);
}

#[test]
fn stale_prev_oid_is_rejected_and_head_is_unchanged() {
    let src_dir = tmp_repo("stale-src");
    let mut src = Forge::init("forge", src_dir.clone()).unwrap();
    commit_one(&mut src, 1, "a.txt", "one", "c1");
    let c1 = capture(&src);
    commit_one(&mut src, 2, "b.txt", "two", "c2");
    let c2 = capture(&src);

    let blobs = blobstore::BlobHandle::default();
    let d1 = c1.stash(&blobs);
    let d2 = c2.stash(&blobs);

    let dst_dir = tmp_repo("stale-dst");
    let mut dst = Forge::with_blobs("forge", dst_dir.clone(), blobs).unwrap();
    push(&mut dst, None, &c1.head, &d1);
    let pinned = dst.root();

    // (a) prev = None but the head is born -> non-fast-forward.
    let err = try_push(&mut dst, None, &c2.head, &d2).unwrap_err();
    assert!(matches!(err, Error::Module(m) if m.contains("non-fast-forward")));
    assert_eq!(dst.root(), pinned, "a rejected push must not move the head");

    // (b) prev = a wrong 20-byte oid -> also non-fast-forward.
    let bogus = [0x11u8; OID_LEN];
    let err = try_push(&mut dst, Some(&bogus), &c2.head, &d2).unwrap_err();
    assert!(matches!(err, Error::Module(m) if m.contains("non-fast-forward")));
    assert_eq!(dst.root(), pinned);
    assert_eq!(
        on_disk_head(&dst_dir),
        Some(c1.oid()),
        "on-disk ref pinned too"
    );

    let _ = std::fs::remove_dir_all(&src_dir);
    let _ = std::fs::remove_dir_all(&dst_dir);
}

#[test]
fn determinism_a_pushed_root_is_identical_without_the_pack() {
    // THE load-bearing invariant: a validator WITHOUT the pack reaches the same
    // root as one WITH it. pack possession is per-node; root is not.
    let (src_dir, _src, cap) = source_one("det-src");

    // node WITH the pack.
    let with_blobs = blobstore::BlobHandle::default();
    let digest = cap.stash(&with_blobs);
    let with_dir = tmp_repo("det-with");
    let mut with = Forge::with_blobs("forge", with_dir.clone(), with_blobs).unwrap();
    push(&mut with, None, &cap.head, &digest);

    // node WITHOUT the pack: an EMPTY blob store, but the SAME op (same digest).
    let without_blobs = blobstore::BlobHandle::default();
    assert!(
        !without_blobs.has_chunk(&<[u8; 32]>::try_from(digest.as_slice()).unwrap()),
        "this store must not hold the pack"
    );
    let without_dir = tmp_repo("det-without");
    let mut without = Forge::with_blobs("forge", without_dir.clone(), without_blobs).unwrap();
    push(&mut without, None, &cap.head, &digest);

    // identical roots — the fork-safety property.
    assert_eq!(
        without.root(),
        cap.root,
        "root is sha256(new_oid), pack-free"
    );
    assert_eq!(
        without.root(),
        with.root(),
        "the pack-less node MUST match the pack-holding node's root"
    );
    // and Head reports the new oid on both.
    assert_eq!(head_query(&without), Some(cap.oid().to_string()));
    assert_eq!(head_query(&without), head_query(&with));

    // the difference is ONLY node-local: content is not yet materialized on the
    // pack-less node (its on-disk ref stays unborn), while the pack-holder has it.
    assert_eq!(
        on_disk_head(&without_dir),
        None,
        "no pack -> ref stays behind"
    );
    assert_eq!(
        on_disk_head(&with_dir),
        Some(cap.oid()),
        "pack -> materialized"
    );

    // a later materialize with the pack STILL absent is a safe no-op: root stays
    // correct, ref stays behind, no error.
    without.materialize().unwrap();
    assert_eq!(without.root(), cap.root);
    assert_eq!(on_disk_head(&without_dir), None);

    // once the pack arrives, a retry catches the on-disk ref up — root unchanged.
    let arrived = blobstore::BlobHandle::default();
    let d2 = cap.stash(&arrived); // digest is identical (content-addressed)
    assert_eq!(d2, digest, "content-addressed digest is stable");
    let mut caught_up = Forge::with_blobs("forge", without_dir.clone(), arrived).unwrap();
    push(&mut caught_up, None, &cap.head, &digest);
    assert_eq!(caught_up.root(), cap.root, "root still sha256(new_oid)");
    assert_eq!(
        on_disk_head(&without_dir),
        Some(cap.oid()),
        "the pack now present -> ref materialized"
    );

    let _ = std::fs::remove_dir_all(&src_dir);
    let _ = std::fs::remove_dir_all(&with_dir);
    let _ = std::fs::remove_dir_all(&without_dir);
}

#[test]
fn malformed_push_fields_are_rejected_deterministically() {
    let dst_dir = tmp_repo("malformed-dst");
    let mut dst = Forge::init("forge", dst_dir.clone()).unwrap();

    let ok_oid = [0u8; OID_LEN];
    let ok_digest = [0u8; 32];

    // new_oid too short.
    let err = try_push(&mut dst, None, &[0u8; 19], &ok_digest).unwrap_err();
    assert!(matches!(err, Error::Module(m) if m.contains("new_oid")));

    // new_oid too long.
    let err = try_push(&mut dst, None, &[0u8; 21], &ok_digest).unwrap_err();
    assert!(matches!(err, Error::Module(m) if m.contains("new_oid")));

    // pack_digest not 32 bytes.
    let err = try_push(&mut dst, None, &ok_oid, &[0u8; 31]).unwrap_err();
    assert!(matches!(err, Error::Module(m) if m.contains("pack_digest")));

    // prev_oid present but wrong length.
    let err = try_push(&mut dst, Some(&[0u8; 10]), &ok_oid, &ok_digest).unwrap_err();
    assert!(matches!(err, Error::Module(m) if m.contains("prev_oid")));

    // every rejection left the module untouched (still unborn, never staged).
    assert_eq!(dst.root(), StateRoot::ZERO);
    assert_eq!(head_query(&dst), None);
    assert_eq!(on_disk_head(&dst_dir), None);

    let _ = std::fs::remove_dir_all(&dst_dir);
}

#[test]
fn commit_and_push_coexist_on_one_module() {
    // back-compat: the file-by-file Commit still works, and a Push can build on
    // a Commit-made head (prev = the committed commit oid). the pinned identity +
    // date make c1's oid identical across independent repos, so a second module
    // can replay c1 then push c2's closure onto it.
    let src_dir = tmp_repo("coexist-src");
    let mut src = Forge::init("forge", src_dir.clone()).unwrap();
    commit_one(&mut src, 1, "a.txt", "one", "c1");
    let c1_head = on_disk_head(&src_dir).unwrap();
    commit_one(&mut src, 2, "b.txt", "two", "c2");
    let c2 = capture(&src);

    let dst_dir = tmp_repo("coexist-dst");
    let blobs = blobstore::BlobHandle::default();
    let d2 = c2.stash(&blobs);
    let mut dst = Forge::with_blobs("forge", dst_dir.clone(), blobs).unwrap();
    commit_one(&mut dst, 1, "a.txt", "one", "c1");
    assert_eq!(
        on_disk_head(&dst_dir),
        Some(c1_head),
        "same deterministic c1 oid across repos"
    );

    push(&mut dst, Some(c1_head.as_bytes()), &c2.head, &d2);
    assert_eq!(dst.root(), c2.root, "push advanced the head off the commit");
    assert_eq!(
        on_disk_head(&dst_dir),
        Some(c2.oid()),
        "push materialized on top of a commit-made head"
    );
    assert_eq!(read_blob(&dst_dir, "a.txt"), b"one".to_vec());
    assert_eq!(read_blob(&dst_dir, "b.txt"), b"two".to_vec());

    let _ = std::fs::remove_dir_all(&src_dir);
    let _ = std::fs::remove_dir_all(&dst_dir);
}

/// `list_repos` answers the INTEGRATION head: a main-only repo falls back to
/// the main head, but once `dev` is born the listing reports dev's oid —
/// the branch every browse surface reads, so a dev-only repo must never list
/// as unborn to a remote client.
#[test]
fn list_repos_reports_the_integration_head() {
    let src_dir = tmp_repo("listrepos-src");
    let mut src = Forge::init("forge", src_dir.clone()).unwrap();
    commit_one(&mut src, 1, "a.txt", "one", "c1");
    let c1 = capture(&src);

    let dst_dir = tmp_repo("listrepos-dst");
    let blobs = blobstore::BlobHandle::default();
    let d1 = c1.stash(&blobs);
    let mut dst = Forge::with_blobs("forge", dst_dir.clone(), blobs).unwrap();
    commit_one(&mut dst, 1, "a.txt", "one", "c1");
    commit_one(&mut dst, 2, "b.txt", "two", "c2");
    let main_head = on_disk_head(&dst_dir).unwrap();

    // main-only: the listing falls back to the main head.
    assert_eq!(
        repo_heads(&dst),
        vec![("default".into(), Some(main_head.to_string()))]
    );

    // dev born at c1 (≠ main's c2): the integration branch owns the listing.
    futures::executor::block_on(dst.execute(
        &mut at(3),
        &push_branch_msg("dev", None, &c1.head, &d1),
    ))
    .unwrap();
    futures::executor::block_on(dst.commit_block()).unwrap();
    assert_ne!(c1.oid(), main_head, "the two branches must diverge");
    assert_eq!(
        repo_heads(&dst),
        vec![("default".into(), Some(c1.oid().to_string()))]
    );

    let _ = std::fs::remove_dir_all(&src_dir);
    let _ = std::fs::remove_dir_all(&dst_dir);
}
