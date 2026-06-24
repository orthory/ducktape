//! local git op handlers.
//!
//! [`apply`] performs a [`vcs::op::Op`](crate::op::Op) as a *local* side-effect
//! against a working repo by shelling out to `git`. these are NOT replicated:
//! replaying `git commit` on another node yields a different sha, so nodes would
//! never converge on git state. what syncs across the network is file *content*
//! (via workspace ops); workers commit locally and content propagates. so
//! `apply` just runs the git action in `repo` and reports success/failure.

use std::path::Path;
use std::process::Command;

use crate::op::Op;

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
    let output = Command::new("git").current_dir(repo).args(args).output()?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        return Err(Error::NonZeroExit(code, stderr));
    }
    Ok(Output { stdout, stderr })
}

/// perform `op` as a local side-effect against the working repo at `repo`.
pub fn apply(op: &Op, repo: &Path) -> Result<Output, Error> {
    match op {
        Op::Init => run_git(repo, &["init"]),
        Op::Add { paths } => {
            let mut args = vec!["add"];
            args.extend(paths.iter().map(String::as_str));
            run_git(repo, &args)
        }
        // configure identity per-invocation via `-c` so commit works even when
        // the repo/host has no global git identity (e.g. CI).
        Op::Commit { message, author } => {
            let id = identity_args(author);
            run_git(
                repo,
                &[&id[0], &id[1], &id[2], &id[3], "commit", "-m", message],
            )
        }
        Op::Branch { name } => run_git(repo, &["branch", name]),
        Op::Checkout { reference } => run_git(repo, &["checkout", reference]),
        // non-ff merges create a merge commit -> need a committer identity too.
        Op::Merge { from } => {
            let id = identity_args("ducktape");
            run_git(repo, &[&id[0], &id[1], &id[2], &id[3], "merge", from])
        }
        // annotated tags (`-m`) create a tag object -> need a tagger identity.
        // lightweight tags (no message) don't.
        Op::Tag { name, message } => match message {
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
            "vcs-apply-test-{}-{}",
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

        apply(&Op::Init, &repo).expect("init");
        assert!(repo.join(".git").exists(), ".git should exist after init");

        fs::write(repo.join("hello.txt"), b"hi").unwrap();
        apply(
            &Op::Add {
                paths: vec!["hello.txt".to_string()],
            },
            &repo,
        )
        .expect("add");

        apply(
            &Op::Commit {
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
        apply(&Op::Init, &repo).expect("init");
        fs::write(repo.join("f.txt"), b"x").unwrap();
        apply(
            &Op::Add {
                paths: vec!["f.txt".to_string()],
            },
            &repo,
        )
        .expect("add");
        apply(
            &Op::Commit {
                message: "c".to_string(),
                author: "t".to_string(),
            },
            &repo,
        )
        .expect("commit");

        apply(
            &Op::Tag {
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
        let err = apply(
            &Op::Checkout {
                reference: "main".to_string(),
            },
            &repo,
        );
        assert!(matches!(err, Err(Error::NonZeroExit(..))));
        fs::remove_dir_all(&repo).ok();
    }
}
