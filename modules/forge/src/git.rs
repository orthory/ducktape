//! a small, binary-safe shell-out to real `git` — forge's private substrate.
//!
//! lifted in spirit from the legacy `vcs` crate's `run` primitive: every verb
//! goes through one `run`/`run_ok` pair so stdout stays raw `Vec<u8>` (git emits
//! non-utf8 object bodies) and stdin (unused here, but kept faithful) can't
//! deadlock. only the small set of plumbing verbs forge needs is exposed.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// errors from shelling out to git.
#[derive(Debug)]
pub enum GitError {
    /// the `git` process could not be spawned / io failed.
    Io(String),
    /// git ran and exited non-zero: `(code, stderr)`.
    NonZeroExit(i32, String),
    /// git emitted an oid we couldn't parse as hex.
    BadOid(String),
}

impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GitError::Io(e) => write!(f, "git io: {e}"),
            GitError::NonZeroExit(c, e) => write!(f, "git exit {c}: {e}"),
            GitError::BadOid(s) => write!(f, "bad oid: {s}"),
        }
    }
}

impl std::error::Error for GitError {}

/// which object format the repo was initialized in. drives how a HEAD oid maps
/// into a fixed-width [`sdk::StateRoot`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectFormat {
    /// 32-byte oids: the HEAD oid IS the state root, verbatim.
    Sha256,
    /// 20-byte oids (fallback where host git lacks sha256): the root is
    /// `sha256(oid_bytes)`, a stable commitment one indirection removed.
    Sha1,
}

/// run a git command; return raw stdout on success (exit 0), else `NonZeroExit`.
pub fn run(repo: &Path, args: &[&str]) -> Result<Vec<u8>, GitError> {
    let out = Command::new("git")
        .current_dir(repo)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| GitError::Io(e.to_string()))?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(GitError::NonZeroExit(
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ))
    }
}

/// run a git command feeding `stdin`; return raw stdout on success.
pub fn run_stdin(repo: &Path, args: &[&str], stdin: &[u8]) -> Result<Vec<u8>, GitError> {
    use std::io::Write;
    let mut child = Command::new("git")
        .current_dir(repo)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| GitError::Io(e.to_string()))?;
    child
        .stdin
        .take()
        .expect("stdin piped")
        .write_all(stdin)
        .map_err(|e| GitError::Io(e.to_string()))?;
    let out = child.wait_with_output().map_err(|e| GitError::Io(e.to_string()))?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(GitError::NonZeroExit(
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ))
    }
}

/// run a git command for its exit status only (an existence probe). a non-zero
/// exit is `Ok(false)`, NOT an error — that's how git signals "absent".
pub fn run_ok(repo: &Path, args: &[&str]) -> Result<bool, GitError> {
    let status = Command::new("git")
        .current_dir(repo)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| GitError::Io(e.to_string()))?;
    Ok(status.success())
}

fn oid_hex(out: Vec<u8>) -> String {
    String::from_utf8_lossy(&out).trim().to_string()
}

/// `git init` a fresh repo in `dir`, preferring sha256 (32-byte oids). falls
/// back to sha1 only if the host git rejects `--object-format=sha256`. returns
/// the format the repo ended up in. hermetic (`--template=`) and pins the
/// canonical branch name so init doesn't depend on the host's `init.defaultBranch`.
pub fn init(dir: &Path) -> Result<ObjectFormat, GitError> {
    std::fs::create_dir_all(dir).map_err(|e| GitError::Io(e.to_string()))?;
    let sha256 = run(
        dir,
        &["init", "--object-format=sha256", "--initial-branch=main", "--template=", "--quiet"],
    );
    if sha256.is_ok() {
        return Ok(ObjectFormat::Sha256);
    }
    // fallback: default (sha1) object format. documented contingency — mixed-
    // format nodes would fork the app-hash, so format is a genesis-uniform param.
    run(dir, &["init", "--initial-branch=main", "--template=", "--quiet"])?;
    Ok(ObjectFormat::Sha1)
}

/// detect the repo's object format after init (defensive — matches what `init`
/// returned, but reads it from git rather than trusting the flag took).
pub fn object_format(repo: &Path) -> Result<ObjectFormat, GitError> {
    let fmt = oid_hex(run(repo, &["rev-parse", "--show-object-format"])?);
    match fmt.as_str() {
        "sha256" => Ok(ObjectFormat::Sha256),
        _ => Ok(ObjectFormat::Sha1),
    }
}

/// resolve a ref to its oid hex, or `None` if it doesn't exist (unborn HEAD).
pub fn resolve_ref(repo: &Path, name: &str) -> Result<Option<String>, GitError> {
    if !run_ok(repo, &["rev-parse", "--verify", "--quiet", name])? {
        return Ok(None);
    }
    Ok(Some(oid_hex(run(repo, &["rev-parse", name])?)))
}

/// hash `content` into a blob object (`-w` writes it to the odb), returning its oid.
pub fn hash_blob(repo: &Path, content: &[u8]) -> Result<String, GitError> {
    Ok(oid_hex(run_stdin(repo, &["hash-object", "-w", "-t", "blob", "--stdin"], content)?))
}

/// build a tree from `parent_tree` (if any) with a single `path -> blob` staged,
/// using an ISOLATED index so the result is a pure function of its inputs — no
/// worktree cruft can leak in. returns the written tree oid.
pub fn write_tree_with(
    repo: &Path,
    index_file: &Path,
    parent_tree: Option<&str>,
    path: &str,
    blob: &str,
) -> Result<String, GitError> {
    // GIT_INDEX_FILE isolates the staging area; wipe any stale index first.
    let _ = std::fs::remove_file(index_file);
    let idx = index_file.to_string_lossy().to_string();
    let with_idx = |args: &[&str]| -> Result<Vec<u8>, GitError> {
        let out = Command::new("git")
            .current_dir(repo)
            .env("GIT_INDEX_FILE", &idx)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| GitError::Io(e.to_string()))?;
        if out.status.success() {
            Ok(out.stdout)
        } else {
            Err(GitError::NonZeroExit(
                out.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&out.stderr).trim().to_string(),
            ))
        }
    };
    if let Some(t) = parent_tree {
        with_idx(&["read-tree", t])?;
    }
    let cacheinfo = format!("100644,{blob},{path}");
    with_idx(&["update-index", "--add", "--cacheinfo", &cacheinfo])?;
    Ok(oid_hex(with_idx(&["write-tree"])?))
}

/// create a commit object over `tree`, chained on `parent` if present, with a
/// FIXED identity and a date derived from `consensus_time` (NOT wall clock) — so
/// the commit oid is byte-reproducible across nodes. returns the commit oid.
pub fn commit_tree(
    repo: &Path,
    tree: &str,
    parent: Option<&str>,
    message: &str,
    consensus_time: u64,
) -> Result<String, GitError> {
    let date = format!("@{consensus_time} +0000");
    let mut args: Vec<&str> = vec!["commit-tree", tree];
    if let Some(p) = parent {
        args.push("-p");
        args.push(p);
    }
    args.push("-m");
    args.push(message);
    let out = Command::new("git")
        .current_dir(repo)
        .env("GIT_AUTHOR_NAME", "ducktape")
        .env("GIT_AUTHOR_EMAIL", "ducktape@localhost")
        .env("GIT_COMMITTER_NAME", "ducktape")
        .env("GIT_COMMITTER_EMAIL", "ducktape@localhost")
        .env("GIT_AUTHOR_DATE", &date)
        .env("GIT_COMMITTER_DATE", &date)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| GitError::Io(e.to_string()))?;
    if !out.status.success() {
        return Err(GitError::NonZeroExit(
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ));
    }
    Ok(oid_hex(out.stdout))
}

/// move a ref to `target`. single-node: the LOCAL ref move at the commit origin.
/// (faithful multi-node applies this same primitive on receipt of a wire
/// RefUpdate — never a `git commit` — see the module docstring.)
pub fn update_ref(repo: &Path, name: &str, target: &str) -> Result<(), GitError> {
    run(repo, &["update-ref", name, target])?;
    Ok(())
}

/// resolve `<commit>^{tree}` — the tree oid a commit points at.
pub fn commit_tree_oid(repo: &Path, commit: &str) -> Result<String, GitError> {
    let spec = format!("{commit}^{{tree}}");
    Ok(oid_hex(run(repo, &["rev-parse", &spec])?))
}

/// parse a 32-byte oid from its hex; errors on wrong length / non-hex.
pub fn oid_bytes32(hex: &str) -> Result<[u8; 32], GitError> {
    if hex.len() != 64 {
        return Err(GitError::BadOid(format!("expected 64 hex chars, got {}", hex.len())));
    }
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
            .map_err(|_| GitError::BadOid(hex.to_string()))?;
    }
    Ok(out)
}

/// derive a repo path helper for callers wanting a sibling index file.
pub fn index_path(repo: &Path) -> PathBuf {
    repo.join(".git").join("ducktape-index")
}
