//! the git layer driven through the [`Engine`] dispatcher (p1.3).
//!
//! proves the engine *carries* the full local-git lifecycle: every major
//! `vcs::op::Op` routed through `Engine::apply(Op::Vcs(..))` lands real state in
//! a real on-disk repo. the [`GitVcs`] handler is injected via `with_vcs`; the
//! engine's workspace is an irrelevant empty tree here (vcs ops don't touch it).
//!
//! the assertions are deliberately non-tautological: they don't trust `apply`'s
//! `Ok` — they shell out to `git` independently and assert the resulting repo
//! state (`git log`, `git tag -l`, a real merge commit, files on disk). a no-op
//! vcs handler (the old default) would make every one of these fail.
//!
//! NOTE: this is the *direct-engine* path on purpose. a real `git commit`
//! produces a host-specific sha, so vcs ops are local-only and must NOT replay
//! across peers; wiring [`GitVcs`] into a `Node` (where the inbound task applies
//! every decoded op) is a separate step gated on resolving that lane conflict.
//! see `engine/src/git.rs`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use engine::{Engine, EngineError, GitVcs};
use op::Op;
use vcs::op::Op as VcsOp;
use workspace::{Entry, Workspace};

/// a unique scratch dir per test invocation (pid + nanos), mirroring the
/// `vcs::apply` tests so parallel runs don't collide.
fn tmpdir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "engine-vcs-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// an engine over a throwaway empty workspace, wired to a real [`GitVcs`] on
/// `repo`.
fn engine_on(repo: &Path) -> Engine {
    let ws = Workspace::new_from_entry(Entry::Directory(Vec::new()));
    Engine::new(ws).with_vcs(Box::new(GitVcs::new(repo)))
}

/// drive one vcs op through the dispatcher and assert it emitted no follow-ups
/// (the vcs arm never does — only control can).
fn drive(engine: &mut Engine, op: VcsOp) {
    let follow_ups = engine
        .apply(Op::Vcs(op))
        .expect("engine applies the vcs op");
    assert!(follow_ups.is_empty(), "a vcs op emits no follow-up ops");
}

/// run `git` in `repo` for an *independent* assertion (not via the engine),
/// returning trimmed stdout. fails loudly on a non-zero git exit.
fn git(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .expect("spawn git");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// the full lifecycle: init → add/commit on the default branch → branch +
/// checkout → commit on the feature branch → checkout back → divergent commit →
/// non-ff merge → annotated tag. driven entirely through `Engine::apply`.
#[test]
fn engine_carries_full_git_lifecycle() {
    let repo = tmpdir("lifecycle");
    let mut engine = engine_on(&repo);

    // --- init -----------------------------------------------------------
    drive(&mut engine, VcsOp::Init);
    assert!(repo.join(".git").exists(), "Init created a real repo");
    // capture the default branch name (master vs main is host-config-dependent)
    // so we can return to it by name after working on the feature branch.
    let default_branch = git(&repo, &["symbolic-ref", "--short", "HEAD"]);

    // --- a base commit on the default branch ----------------------------
    fs::write(repo.join("base.md"), b"base\n").unwrap();
    drive(
        &mut engine,
        VcsOp::Add {
            paths: vec!["base.md".into()],
        },
    );
    drive(
        &mut engine,
        VcsOp::Commit {
            message: "base".into(),
            author: "alice".into(),
        },
    );

    // --- branch off, switch to it, commit there -------------------------
    drive(
        &mut engine,
        VcsOp::Branch {
            name: "feature".into(),
        },
    );
    drive(
        &mut engine,
        VcsOp::Checkout {
            reference: "feature".into(),
        },
    );
    assert_eq!(git(&repo, &["symbolic-ref", "--short", "HEAD"]), "feature");
    fs::write(repo.join("feature.md"), b"feature\n").unwrap();
    drive(
        &mut engine,
        VcsOp::Add {
            paths: vec!["feature.md".into()],
        },
    );
    drive(
        &mut engine,
        VcsOp::Commit {
            message: "feature work".into(),
            author: "bob".into(),
        },
    );

    // --- back to default, make a divergent commit so the merge is non-ff -
    drive(
        &mut engine,
        VcsOp::Checkout {
            reference: default_branch.clone(),
        },
    );
    // feature.md belongs to the feature branch only — not on the default branch yet.
    assert!(
        !repo.join("feature.md").exists(),
        "feature.md is not on {default_branch} before the merge"
    );
    fs::write(repo.join("main.md"), b"main\n").unwrap();
    drive(
        &mut engine,
        VcsOp::Add {
            paths: vec!["main.md".into()],
        },
    );
    drive(
        &mut engine,
        VcsOp::Commit {
            message: "main work".into(),
            author: "alice".into(),
        },
    );

    // --- merge feature in (non-ff -> a real merge commit) ---------------
    drive(
        &mut engine,
        VcsOp::Merge {
            from: "feature".into(),
        },
    );

    // --- annotate the merge with a tag ----------------------------------
    drive(
        &mut engine,
        VcsOp::Tag {
            name: "v1".into(),
            message: Some("release one".into()),
        },
    );

    // --- independent assertions on real repo state ----------------------
    // every commit is reachable from the merged default-branch HEAD.
    let log = git(&repo, &["log", "--oneline"]);
    for msg in ["base", "feature work", "main work"] {
        assert!(log.contains(msg), "git log missing {msg:?}:\n{log}");
    }
    // the merge produced an actual merge commit (two parents).
    let merges = git(&repo, &["rev-list", "--merges", "HEAD"]);
    assert!(
        !merges.is_empty(),
        "Merge produced a merge commit; rev-list --merges was empty"
    );
    // the feature file crossed the merge onto the default branch's worktree.
    assert!(
        repo.join("feature.md").exists(),
        "merge brought feature.md onto {default_branch}"
    );
    // the annotated tag exists and points at something.
    assert_eq!(git(&repo, &["tag", "-l"]), "v1", "tag v1 is present");
    // we ended back on the default branch.
    assert_eq!(
        git(&repo, &["symbolic-ref", "--short", "HEAD"]),
        default_branch,
        "HEAD is back on the default branch after the merge"
    );

    fs::remove_dir_all(&repo).ok();
}

/// a failing git action (checkout in a dir that was never `init`ed) must surface
/// as [`EngineError::Vcs`] — the engine doesn't swallow git failures.
#[test]
fn engine_surfaces_git_failure_as_vcs_error() {
    let repo = tmpdir("failure");
    let mut engine = engine_on(&repo);

    let result = engine.apply(Op::Vcs(VcsOp::Checkout {
        reference: "main".into(),
    }));
    assert!(
        matches!(result, Err(EngineError::Vcs(_))),
        "a non-zero git exit maps to EngineError::Vcs, got: {result:?}"
    );

    fs::remove_dir_all(&repo).ok();
}
