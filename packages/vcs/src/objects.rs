//! the git <-> bytes bridge: snapshot a worktree to a commit, move refs, and
//! export/import an object closure as a packfile.
//!
//! these are the plumbing primitives the ref-replication path (A4) composes: a
//! node snapshots its worktree to a commit, ships that commit's reachable object
//! closure as pack bytes over the blob transport (A3), the receiver `import`s the
//! pack, then moves its ref via [`update_ref`] — never by replaying a `git
//! commit` (which would produce a different sha and never converge).

use std::path::Path;

use crate::git::{self, Error};
use crate::object::ObjectId;

/// stage everything in the worktree and snapshot it as a commit; returns the new
/// commit's oid.
///
/// this is plumbing — `add -A` -> `write-tree` -> `commit-tree` — not porcelain
/// `git commit`: it runs headless, returns the oid directly, and does NOT move
/// any ref ([`update_ref`] does that). the commit parents the current `HEAD`
/// when one exists, so successive snapshots chain into history.
///
/// identity is fixed (`ducktape`) and the timestamp is real. the substrate's
/// convergence comes from single-writer-per-branch (stable hashes), not from
/// deterministic-pinned commits — so we deliberately don't pin the date.
pub fn snapshot_worktree(repo: &Path, message: &str) -> Result<ObjectId, Error> {
    git::run(repo, &["add", "-A"], None)?;
    let tree = git::parse_oid(&git::run(repo, &["write-tree"], None)?.stdout)?;
    let tree_hex = tree.to_hex();

    let id = git::identity_args("ducktape");
    let mut args: Vec<&str> = vec![
        id[0].as_str(),
        id[1].as_str(),
        id[2].as_str(),
        id[3].as_str(),
        "commit-tree",
        tree_hex.as_str(),
    ];

    // parent on HEAD when it resolves; an unborn HEAD -> a root (parentless)
    // commit. head_hex must outlive `args`, so it's declared in fn scope.
    let head_hex;
    if git::run_ok(repo, &["rev-parse", "--verify", "--quiet", "HEAD"])? {
        head_hex =
            String::from_utf8_lossy(&git::run(repo, &["rev-parse", "HEAD"], None)?.stdout)
                .trim()
                .to_string();
        args.push("-p");
        args.push(head_hex.as_str());
    }
    args.push("-m");
    args.push(message);

    git::parse_oid(&git::run(repo, &args, None)?.stdout)
}

/// point ref `name` (e.g. `"refs/heads/main"`) at `target`. the inbound side of
/// replication: a receiver moves its ref here once the target's objects are
/// present, instead of re-running the originating verb.
pub fn update_ref(repo: &Path, name: &str, target: &ObjectId) -> Result<(), Error> {
    git::run(repo, &["update-ref", name, &target.to_hex()], None)?;
    Ok(())
}

/// is the entire object closure reachable from `tip` present in this repo? this
/// is the gate on applying a finalized ref move: the ref advances only once the
/// commit, every tree under it, and every blob it names are all stored locally —
/// so the move can never land a ref pointing into a hole.
///
/// there are three independent ways the closure can be incomplete, each with its
/// own probe:
/// - the tip commit isn't here at all (`cat-file -e`) — `Ok(false)`;
/// - a commit/tree the walk must read is unreadable, so `rev-list --objects`
///   can't enumerate the closure and exits non-zero — `Ok(false)`;
/// - `rev-list` named a blob oid straight from a tree entry without proving it's
///   stored; `cat-file --batch-check` probes each oid and reports any absent one
///   as `missing` — `Ok(false)`.
///
/// (some git builds make `rev-list` itself verify blobs and fail at the second
/// probe; others list the oid and leave the catch to `batch-check`. both holes
/// are covered either way.) only when all three pass is the closure whole
/// (`Ok(true)`). a spawn/io fault still propagates as `Err`.
pub fn closure_complete(repo: &Path, tip: &ObjectId) -> Result<bool, Error> {
    // the tip commit must be present before anything else is worth checking.
    if !git::run_ok(repo, &["cat-file", "-e", &tip.to_hex()])? {
        return Ok(false);
    }

    // enumerate the closure. a missing object the walk has to read makes
    // rev-list exit non-zero — that's "incomplete", not a fault to propagate.
    let revs = match git::run(
        repo,
        &["rev-list", "--objects", "--no-object-names", &tip.to_hex()],
        None,
    ) {
        Ok(out) => out.stdout,
        Err(Error::NonZeroExit(..)) => return Ok(false),
        Err(e) => return Err(e),
    };

    // rev-list can list a blob oid from a tree entry without opening the blob;
    // batch-check is what actually probes presence. a present object's line ends
    // in its type/size; an absent one's line ends in "missing".
    let check = git::run(repo, &["cat-file", "--batch-check"], Some(&revs))?.stdout;
    let text = String::from_utf8_lossy(&check);
    if text.lines().any(|line| line.ends_with("missing")) {
        return Ok(false);
    }

    Ok(true)
}

/// resolve ref `name` to its current target, or `Ok(None)` if it doesn't exist.
pub fn resolve_ref(repo: &Path, name: &str) -> Result<Option<ObjectId>, Error> {
    if !git::run_ok(repo, &["rev-parse", "--verify", "--quiet", name])? {
        return Ok(None);
    }
    Ok(Some(git::parse_oid(
        &git::run(repo, &["rev-parse", name], None)?.stdout,
    )?))
}

/// export the full object closure reachable from `commit` as a packfile's bytes.
/// `rev-list --objects` enumerates the closure (commit + trees + blobs);
/// `pack-objects` packs it. the bytes are binary — fed to [`import`] on a
/// receiver over the A3 blob lane.
pub fn export_reachable(repo: &Path, commit: &ObjectId) -> Result<Vec<u8>, Error> {
    let revs = git::run(repo, &["rev-list", "--objects", &commit.to_hex()], None)?.stdout;
    let pack = git::run(repo, &["pack-objects", "--stdout"], Some(&revs))?.stdout;
    Ok(pack)
}

/// import a packfile (from [`export_reachable`]) into `repo`, exploding it into
/// loose objects. afterward the closure is present and readable via the ODB.
pub fn import(repo: &Path, pack: &[u8]) -> Result<(), Error> {
    git::run(repo, &["unpack-objects"], Some(pack))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::ObjectId;
    use crate::odb::GitOdb;
    use objstore::ObjectStore;

    fn fresh_repo(tag: &str) -> GitOdb {
        // process-wide counter for collision-proof temp dirs: see cmd.rs::tmpdir
        // — {pid}-{nanos} alone can collide under parallel init on macos.
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "vcs-objects-{}-{}-{}-{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            COUNTER.fetch_add(1, Ordering::Relaxed),
        ));
        GitOdb::init(&dir).expect("init sha256 odb")
    }

    fn cleanup(odb: &GitOdb) {
        std::fs::remove_dir_all(odb.repo()).ok();
    }

    /// snapshot a one-file worktree into a commit and return the repo plus the
    /// three oids of its closure: `(repo, commit, root_tree, blob)`. selectively
    /// deleting one of these loose objects is how the closure-hole tests below
    /// construct "commit present, but a tree/blob absent".
    fn build_closure(tag: &str) -> (GitOdb, std::path::PathBuf, ObjectId, ObjectId, ObjectId) {
        let odb = fresh_repo(tag);
        let repo = odb.repo().to_path_buf();
        std::fs::write(repo.join("doc.md"), b"payload\n").unwrap();
        let commit = snapshot_worktree(&repo, "snap").expect("snapshot");

        let tree_hex = String::from_utf8(
            git::run(&repo, &["rev-parse", &format!("{}^{{tree}}", commit.to_hex())], None)
                .unwrap()
                .stdout,
        )
        .unwrap()
        .trim()
        .to_string();
        let tree = ObjectId::from_hex(&tree_hex).unwrap();

        let blob_hex = String::from_utf8(
            git::run(&repo, &["rev-parse", &format!("{}:doc.md", commit.to_hex())], None)
                .unwrap()
                .stdout,
        )
        .unwrap()
        .trim()
        .to_string();
        let blob = ObjectId::from_hex(&blob_hex).unwrap();

        (odb, repo, commit, tree, blob)
    }

    /// delete the loose object file for `oid` (`.git/objects/<2>/<62>`), so the
    /// object is genuinely gone from the store. freshly-written objects are loose
    /// (no pack/gc), so this reliably removes them.
    fn delete_loose(repo: &Path, oid: &ObjectId) {
        let hex = oid.to_hex();
        let path = repo.join(".git/objects").join(&hex[..2]).join(&hex[2..]);
        std::fs::remove_file(&path).unwrap_or_else(|e| panic!("remove loose {}: {e}", oid.to_hex()));
    }

    #[test]
    fn closure_complete_when_whole_closure_present() {
        let (odb, repo, commit, _tree, _blob) = build_closure("closure-full");
        assert!(
            closure_complete(&repo, &commit).unwrap(),
            "commit + tree + blob all present -> complete"
        );
        cleanup(&odb);
    }

    #[test]
    fn closure_incomplete_when_tip_commit_absent() {
        // nothing about the tip is here. the cheapest probe (`cat-file -e`)
        // short-circuits before any walk.
        let odb = fresh_repo("closure-absent");
        let repo = odb.repo().to_path_buf();
        let absent = ObjectId::from_hex(&"0".repeat(64)).unwrap();
        assert!(
            !closure_complete(&repo, &absent).unwrap(),
            "absent tip commit -> incomplete"
        );
        cleanup(&odb);
    }

    #[test]
    fn closure_incomplete_when_root_tree_missing() {
        // dangling commit: the commit's loose bytes are present, but its root
        // tree is gone, so the walk can't enumerate the closure. a has(tip)-only
        // guard would wrongly pass this.
        let (odb, repo, commit, tree, _blob) = build_closure("closure-tree");
        delete_loose(&repo, &tree);
        assert!(odb.has(&commit).unwrap(), "commit still present");
        assert!(!odb.has(&tree).unwrap(), "root tree removed");
        assert!(
            !closure_complete(&repo, &commit).unwrap(),
            "missing root tree -> incomplete"
        );
        cleanup(&odb);
    }

    #[test]
    fn closure_incomplete_when_blob_missing() {
        // commit + root tree present, one referenced blob gone. the tree is
        // readable, so the walk reaches the blob entry; whether the rev-list walk
        // or the batch-check probe flags it depends on the git build, but the
        // closure is incomplete either way.
        let (odb, repo, commit, tree, blob) = build_closure("closure-blob");
        delete_loose(&repo, &blob);
        assert!(odb.has(&commit).unwrap(), "commit still present");
        assert!(odb.has(&tree).unwrap(), "root tree still present");
        assert!(!odb.has(&blob).unwrap(), "blob removed");
        assert!(
            !closure_complete(&repo, &commit).unwrap(),
            "missing blob -> incomplete"
        );
        cleanup(&odb);
    }

    #[test]
    fn snapshot_update_ref_resolve() {
        let odb = fresh_repo("snap");
        let repo = odb.repo().to_path_buf();
        std::fs::write(repo.join("doc.md"), b"hello\n").unwrap();

        let commit = snapshot_worktree(&repo, "snapshot").expect("snapshot");
        assert!(odb.has(&commit).unwrap(), "commit object exists");

        // no ref moved by snapshot itself.
        assert_eq!(resolve_ref(&repo, "refs/heads/main").unwrap(), None);
        update_ref(&repo, "refs/heads/main", &commit).expect("update-ref");
        assert_eq!(
            resolve_ref(&repo, "refs/heads/main").unwrap(),
            Some(commit)
        );

        cleanup(&odb);
    }

    #[test]
    fn snapshots_chain_on_head() {
        let odb = fresh_repo("chain");
        let repo = odb.repo().to_path_buf();

        std::fs::write(repo.join("a.md"), b"one\n").unwrap();
        let first = snapshot_worktree(&repo, "first").expect("first");
        // move HEAD's branch so the next snapshot parents on it.
        update_ref(&repo, "HEAD", &first).expect("update HEAD");

        std::fs::write(repo.join("b.md"), b"two\n").unwrap();
        let second = snapshot_worktree(&repo, "second").expect("second");

        // the second commit body names the first as its parent.
        let body = String::from_utf8(
            git::run(&repo, &["cat-file", "commit", &second.to_hex()], None)
                .unwrap()
                .stdout,
        )
        .unwrap();
        assert!(
            body.contains(&format!("parent {}", first.to_hex())),
            "second commit should parent on first:\n{body}"
        );

        cleanup(&odb);
    }

    #[test]
    fn export_import_transfers_the_whole_closure() {
        // src: build a commit; dst: a fresh empty repo that has none of it.
        let src = fresh_repo("src");
        let srepo = src.repo().to_path_buf();
        std::fs::write(srepo.join("doc.md"), b"payload\n").unwrap();
        let commit = snapshot_worktree(&srepo, "ship").expect("snapshot");

        let dst = fresh_repo("dst");
        let drepo = dst.repo().to_path_buf();
        assert!(!dst.has(&commit).unwrap(), "dst starts without the commit");

        // export the closure from src, import into dst.
        let pack = export_reachable(&srepo, &commit).expect("export");
        assert!(!pack.is_empty(), "pack has bytes");
        import(&drepo, &pack).expect("import");

        // commit is present on dst and its loose form round-trips bit-for-bit.
        assert!(dst.has(&commit).unwrap(), "commit landed on dst");
        assert_eq!(
            dst.get(&commit).unwrap(),
            src.get(&commit).unwrap(),
            "commit loose bytes identical across the transfer"
        );

        // and so is the blob it references (the full closure, not just the tip).
        let blob_hex = String::from_utf8(
            git::run(&srepo, &["rev-parse", &format!("{}:doc.md", commit.to_hex())], None)
                .unwrap()
                .stdout,
        )
        .unwrap()
        .trim()
        .to_string();
        let blob = ObjectId::from_hex(&blob_hex).unwrap();
        assert!(dst.has(&blob).unwrap(), "referenced blob landed on dst too");

        cleanup(&src);
        cleanup(&dst);
    }
}
