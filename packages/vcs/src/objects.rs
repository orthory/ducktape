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
