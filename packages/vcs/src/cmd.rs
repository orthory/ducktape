//! local git commands — the imperative surface, never replicated.
//!
//! [`Command`] is the old git-verb taxonomy (`Init`, `Add`, `Commit`, …) and
//! [`run_local`] performs one as a *local* side-effect against a working repo by
//! shelling out to `git`. these do NOT cross the wire: replaying `git commit` on
//! another node yields a different sha, so nodes would never converge on git
//! state. what syncs is the *result* — a [`crate::op::Op::RefUpdate`] naming the
//! resulting object, applied on receivers via `update-ref` — plus file content
//! over workspace ops. so `run_local` just runs the git action and reports it.
//!
//! [`Command`] deliberately derives **no** `Serialize`/`Deserialize`: the
//! "never on the wire" invariant is enforced by the type system — a `Command`
//! cannot be wrapped in `op::Op` or sent over a lane, only run here.

use std::path::Path;
// our `Command` enum below is the imperative verb; the std process builder is
// aliased so the names don't collide.
use std::process::Command as Process;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to spawn git: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("git exited with status {0}: {1}")]
    NonZeroExit(i32, String),
}

/// the captured result of a successful `git` invocation.
#[derive(Debug, Clone)]
pub struct Output {
    pub stdout: String,
    pub stderr: String,
}

/// the local git verbs. NOT replicated — see the module docs (no serde derive,
/// on purpose). the wire counterpart is [`crate::op::Op`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Init,
    Add { paths: Vec<String> },
    Commit { message: String, author: String },
    Branch { name: String },
    Checkout { reference: String },
    Merge { from: String },
    Tag { name: String, message: Option<String> },
}

/// fallback identity used for object-creating git actions (commit, annotated
/// tag, non-ff merge) so they work on hosts without a global git identity (CI).
/// injected via `-c` per-invocation; never touches global config.
fn identity_args(name: &str) -> [String; 4] {
    [
        "-c".to_string(),
        format!("user.name={name}"),
        "-c".to_string(),
        format!("user.email={name}@localhost"),
    ]
}

/// run `git` with `args` in `repo`, capturing stdout/stderr. non-zero exit maps
/// to [`Error::NonZeroExit`].
fn run_git(repo: &Path, args: &[&str]) -> Result<Output, Error> {
    let output = Process::new("git").current_dir(repo).args(args).output()?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        return Err(Error::NonZeroExit(code, stderr));
    }
    Ok(Output { stdout, stderr })
}

/// perform `cmd` as a local side-effect against the working repo at `repo`.
pub fn run_local(cmd: &Command, repo: &Path) -> Result<Output, Error> {
    match cmd {
        Command::Init => run_git(repo, &["init"]),
        Command::Add { paths } => {
            let mut args = vec!["add"];
            args.extend(paths.iter().map(String::as_str));
            run_git(repo, &args)
        }
        // configure identity per-invocation via `-c` so commit works even when
        // the repo/host has no global git identity (e.g. CI).
        Command::Commit { message, author } => {
            let id = identity_args(author);
            run_git(
                repo,
                &[&id[0], &id[1], &id[2], &id[3], "commit", "-m", message],
            )
        }
        Command::Branch { name } => run_git(repo, &["branch", name]),
        Command::Checkout { reference } => run_git(repo, &["checkout", reference]),
        // non-ff merges create a merge commit -> need a committer identity too.
        Command::Merge { from } => {
            let id = identity_args("ducktape");
            run_git(repo, &[&id[0], &id[1], &id[2], &id[3], "merge", from])
        }
        // annotated tags (`-m`) create a tag object -> need a tagger identity.
        // lightweight tags (no message) don't.
        Command::Tag { name, message } => match message {
            Some(msg) => {
                let id = identity_args("ducktape");
                run_git(repo, &[&id[0], &id[1], &id[2], &id[3], "tag", "-m", msg, name])
            }
            None => run_git(repo, &["tag", name]),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmpdir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "vcs-cmd-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn init_add_commit_lands_a_commit() {
        let repo = tmpdir();

        run_local(&Command::Init, &repo).expect("init");
        assert!(repo.join(".git").exists(), ".git should exist after init");

        fs::write(repo.join("hello.txt"), b"hi").unwrap();
        run_local(
            &Command::Add {
                paths: vec!["hello.txt".to_string()],
            },
            &repo,
        )
        .expect("add");

        run_local(
            &Command::Commit {
                message: "first commit".to_string(),
                author: "tester".to_string(),
            },
            &repo,
        )
        .expect("commit");

        let log = run_git(&repo, &["log", "--oneline"]).expect("log");
        assert!(
            log.stdout.contains("first commit"),
            "git log should show the commit, got: {:?}",
            log.stdout
        );

        fs::remove_dir_all(&repo).ok();
    }

    #[test]
    fn annotated_tag_works_without_global_identity() {
        let repo = tmpdir();
        run_local(&Command::Init, &repo).expect("init");
        fs::write(repo.join("f.txt"), b"x").unwrap();
        run_local(
            &Command::Add {
                paths: vec!["f.txt".to_string()],
            },
            &repo,
        )
        .expect("add");
        run_local(
            &Command::Commit {
                message: "c".to_string(),
                author: "t".to_string(),
            },
            &repo,
        )
        .expect("commit");

        run_local(
            &Command::Tag {
                name: "v1".to_string(),
                message: Some("release one".to_string()),
            },
            &repo,
        )
        .expect("annotated tag");

        let tags = run_git(&repo, &["tag", "-l"]).expect("tag -l");
        assert!(tags.stdout.contains("v1"), "got: {:?}", tags.stdout);
        fs::remove_dir_all(&repo).ok();
    }

    #[test]
    fn nonzero_exit_is_an_error() {
        let repo = tmpdir();
        // checkout in a non-repo dir fails.
        let err = run_local(
            &Command::Checkout {
                reference: "main".to_string(),
            },
            &repo,
        );
        assert!(matches!(err, Err(Error::NonZeroExit(..))));
        fs::remove_dir_all(&repo).ok();
    }

    /// the full local lifecycle — init → commit → branch+checkout → divergent
    /// commit → non-ff merge → annotated tag — driven through `run_local` alone.
    /// ported from the old engine `vcs_lane` integration test (which drove these
    /// verbs through `Engine::apply`, the model A0 removes): branch / checkout /
    /// non-ff merge / tag-on-merge coverage the focused unit tests above lack.
    /// assertions shell out to `git` independently (`run_git`), so they don't
    /// trust `run_local`'s `Ok`.
    #[test]
    fn full_git_lifecycle() {
        let repo = tmpdir();

        // --- init ---------------------------------------------------------
        run_local(&Command::Init, &repo).expect("init");
        assert!(repo.join(".git").exists(), "Init created a real repo");
        // default branch (master vs main) is host-config-dependent — capture it
        // so we can return by name after working on the feature branch.
        let default_branch = run_git(&repo, &["symbolic-ref", "--short", "HEAD"])
            .expect("HEAD ref")
            .stdout
            .trim()
            .to_owned();

        // --- a base commit on the default branch --------------------------
        fs::write(repo.join("base.md"), b"base\n").unwrap();
        run_local(&Command::Add { paths: vec!["base.md".into()] }, &repo).expect("add base");
        run_local(
            &Command::Commit { message: "base".into(), author: "alice".into() },
            &repo,
        )
        .expect("commit base");

        // --- branch off, switch to it, commit there -----------------------
        run_local(&Command::Branch { name: "feature".into() }, &repo).expect("branch");
        run_local(&Command::Checkout { reference: "feature".into() }, &repo).expect("checkout feature");
        assert_eq!(
            run_git(&repo, &["symbolic-ref", "--short", "HEAD"]).unwrap().stdout.trim(),
            "feature"
        );
        fs::write(repo.join("feature.md"), b"feature\n").unwrap();
        run_local(&Command::Add { paths: vec!["feature.md".into()] }, &repo).expect("add feature");
        run_local(
            &Command::Commit { message: "feature work".into(), author: "bob".into() },
            &repo,
        )
        .expect("commit feature");

        // --- back to default, divergent commit so the merge is non-ff -----
        run_local(&Command::Checkout { reference: default_branch.clone() }, &repo)
            .expect("checkout default");
        assert!(
            !repo.join("feature.md").exists(),
            "feature.md is not on {default_branch} before the merge"
        );
        fs::write(repo.join("main.md"), b"main\n").unwrap();
        run_local(&Command::Add { paths: vec!["main.md".into()] }, &repo).expect("add main");
        run_local(
            &Command::Commit { message: "main work".into(), author: "alice".into() },
            &repo,
        )
        .expect("commit main");

        // --- merge feature in (non-ff -> a real merge commit) -------------
        run_local(&Command::Merge { from: "feature".into() }, &repo).expect("merge");

        // --- annotate the merge with a tag --------------------------------
        run_local(
            &Command::Tag { name: "v1".into(), message: Some("release one".into()) },
            &repo,
        )
        .expect("tag");

        // --- independent assertions on real repo state --------------------
        let log = run_git(&repo, &["log", "--oneline"]).unwrap().stdout;
        for msg in ["base", "feature work", "main work"] {
            assert!(log.contains(msg), "git log missing {msg:?}:\n{log}");
        }
        let merges = run_git(&repo, &["rev-list", "--merges", "HEAD"]).unwrap().stdout;
        assert!(!merges.trim().is_empty(), "Merge produced a merge commit");
        assert!(
            repo.join("feature.md").exists(),
            "merge brought feature.md onto {default_branch}"
        );
        assert_eq!(run_git(&repo, &["tag", "-l"]).unwrap().stdout.trim(), "v1");
        assert_eq!(
            run_git(&repo, &["symbolic-ref", "--short", "HEAD"]).unwrap().stdout.trim(),
            default_branch,
            "HEAD is back on the default branch after the merge"
        );

        fs::remove_dir_all(&repo).ok();
    }
}
