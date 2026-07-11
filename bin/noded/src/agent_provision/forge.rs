//! the forge lane: a per-run git WORKTREE of a node-local forge repo at the
//! run's pinned commit, committed with agent authorship and pushed back
//! through this node's own loopback smart-HTTP lane (receive-pack → blob →
//! `PushRefs`) so the branch move settles through consensus, CAS included
//! (wire contract §4).
//!
//! host `git` is a REAL runtime dependency of this lane — the first in the
//! tree (forge itself is vendored libgit2 and never shells out). it is probed
//! ONCE at provisioner construction; no worktree-capable `git` ⇒ the lane is
//! permanently unavailable with a loud reason while the duckfs lane keeps
//! working. forge repos are git-default **SHA-1** (see forge/src/git.rs —
//! only pack digests / blob addressing are sha256): never pass
//! `--object-format` anywhere.
//!
//! DETACHED by construction: the worktree checks out the PINNED commit with
//! `--detach` and the provisioner never creates or moves a shared-repo ref.
//! this is load-bearing, not style — the consensus module's committed-ref
//! catch-up FORCE-MOVES `refs/heads/<branch>` in the shared repo while runs
//! execute; a worktree HOLDING that branch would silently reparent its
//! commit onto the moved tip with the worktree's old tree (reverting the
//! interloper's content, fast-forward push, no reject ever fired). a
//! detached HEAD is immune: the base stays the pin, and the remote tip is
//! read ONLY via `git fetch` (never the untrustworthy local ref).
//!
//! ordering over CAS-degrade: the push is a plain (never forced)
//! `HEAD:refs/heads/<branch>` update. a concurrently advanced branch rejects
//! at the fork base (production's receive-pack/`PushRefs` CAS, stock git's
//! fetch-first refusal elsewhere), and the reject is an internal RETRY
//! trigger, not a failure: fetch the new tip, rebase the run's commits onto
//! FETCH_HEAD, push again (bounded — [`PUSH_ATTEMPTS`]). the CAS stays the
//! consensus linearizer underneath; both commits land as linear history.
//! ONLY a genuine rebase conflict degrades the receipt (`commit_error` +
//! `Status::Degraded` via the pool), never the reply (R4).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use dispatch_oracle::{
    ProvisionedWorkspace, WorkspaceReceipt, WorkspaceSource, WorkspaceSpec,
};

use crate::NodeHandle;

/// every run commit's committer email — the committer NAME is the node's
/// stable identity (D2: author is the agent, committer is the executing node).
const NODE_COMMITTER_EMAIL: &str = "node@ducktape.local";
/// the synthetic domain agent author emails are minted under.
const AGENT_EMAIL_DOMAIN: &str = "agents.ducktape.local";

/// one node's configured forge lane: where the materialized repos live, where
/// pushes rendezvous, and who the committer is. built by
/// [`ForgeLane::configure`] exactly once, at provisioner construction.
pub(super) struct ForgeLane {
    /// the forge module's on-disk repo base — `<repo_base>/<repo>` is a real
    /// (non-bare, never-checked-out) libgit2 repo the module materializes.
    repo_base: PathBuf,
    /// the push base URL — production `http://127.0.0.1:<port>/forge`
    /// (loopback smart-HTTP receive-pack); `<push_base>/<repo>` is the remote.
    /// tests inject a `file://` base at bare rendezvous repos — the CAS
    /// behavior asserted there is stock git's, the http lane is Task 6's e2e.
    push_base: String,
    /// the committer identity on every run commit (the node, never the agent).
    committer_name: String,
}

impl ForgeLane {
    /// decide the lane ONCE: all three legs (repo base on the handle, a push
    /// base, a worktree-capable host git) must hold, else the lane is
    /// `Err(reason)` — permanent for this provisioner's lifetime. the probe
    /// runs only when the config legs are present (no point probing git on a
    /// node that serves no http surface).
    pub(super) fn configure(
        handle: &NodeHandle,
        push_base: Option<String>,
        committer_name: String,
        probe: impl FnOnce() -> Result<(), String>,
    ) -> Result<Self, String> {
        let Some(repo_base) = handle.forge_repo.clone() else {
            return Err(
                "this node handle carries no forge repo base — the forge module's \
                 materialized repos are not reachable here"
                    .into(),
            );
        };
        let Some(push_base) = push_base else {
            return Err(
                "this node serves no http surface, so the loopback smart-HTTP push lane \
                 (receive-pack → PushRefs) that lands agent branches is unavailable"
                    .into(),
            );
        };
        probe()?;
        Ok(Self {
            repo_base,
            push_base,
            committer_name,
        })
    }
}

/// derive the forge push base URL from the node's http listen address:
/// `http://<host>:<port>/forge`, with a wildcard bind rewritten to the SAME
/// family's loopback (`0.0.0.0` → `127.0.0.1`, `[::]` → `[::1]` — a
/// bindv6only `[::]` listener refuses v4 loopback dials) — the push must dial
/// a CONNECTABLE host, and the lane is loopback by design (the node pushes to
/// ITSELF; the bridge submits the ref move to consensus). `None` in = no http
/// surface = no push lane.
pub fn forge_push_base(http_listen: Option<&str>) -> Option<String> {
    let listen = http_listen?;
    let base = match listen.parse::<std::net::SocketAddr>() {
        Ok(addr) if addr.ip().is_unspecified() => {
            let loopback = if addr.is_ipv6() { "[::1]" } else { "127.0.0.1" };
            format!("{loopback}:{}", addr.port())
        }
        Ok(addr) => addr.to_string(),
        // not a socket address (hostname:port) — trust the operator's string.
        Err(_) => listen.to_string(),
    };
    Some(format!("http://{base}/forge"))
}

/// the construction-time probe: host `git` exists AND supports worktrees,
/// proven functionally (`git init` + `git worktree list` in a scratch dir)
/// rather than by version-string parsing. failure is permanent for the lane.
pub(super) fn probe_host_git() -> Result<(), String> {
    probe_host_git_with("git")
}

fn probe_host_git_with(program: &str) -> Result<(), String> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let scratch = std::env::temp_dir().join(format!(
        "ducktape-git-probe-{}-{nanos}",
        std::process::id()
    ));
    let result = (|| {
        std::fs::create_dir_all(&scratch)
            .map_err(|e| format!("probe scratch dir {}: {e}", scratch.display()))?;
        run_git_program(program, &scratch, &["init", "-q"], &[])
            .map_err(|e| format!("`git init` failed — is git installed? {e}"))?;
        run_git_program(program, &scratch, &["worktree", "list"], &[])
            .map_err(|e| format!("`git worktree list` failed — host git lacks worktree support: {e}"))?;
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    result.map_err(|e: String| {
        format!("host `git` probe failed (forge worktree provisioning needs a worktree-capable git on PATH): {e}")
    })
}

/// a hermetic git command: no host/global/system config, no prompts, no
/// gpg — identity comes ONLY from the env vars the caller sets (D2). the
/// same posture as the e2e suites' git helpers.
fn git(dir: &Path) -> Command {
    git_program("git", dir)
}

fn git_program(program: &str, dir: &Path) -> Command {
    let mut command = Command::new(program);
    command
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .args(["-c", "init.defaultBranch=main", "-c", "commit.gpgsign=false"]);
    command
}

/// run a git command to completion; non-zero exit becomes `Err` carrying the
/// command and its stderr (the operator-facing failure text).
fn run_git(dir: &Path, args: &[&str], envs: &[(&str, &str)]) -> Result<String, String> {
    run_git_program("git", dir, args, envs)
}

fn run_git_program(
    program: &str,
    dir: &Path,
    args: &[&str],
    envs: &[(&str, &str)],
) -> Result<String, String> {
    let mut command = git_program(program, dir);
    command.args(args);
    for (k, v) in envs {
        command.env(k, v);
    }
    let out = command
        .output()
        .map_err(|e| format!("host `git` failed to spawn: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {} failed ({}): {}",
            args.join(" "),
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// the coordinates feed a command line and a path join — refuse anything that
/// could traverse (`..`), read as a git flag (leading `-`), or escape the
/// repo base. consensus already normalized these (norm_repo/norm_branch/hex
/// oids), so a failure here means a corrupt envelope, not a policy gate.
fn validate_coords(repo: &str, commit: &str, branch: &str) -> Result<(), String> {
    let repo_ok = !repo.is_empty()
        && repo != "."
        && repo != ".."
        && !repo.starts_with('-')
        && repo
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if !repo_ok {
        return Err(format!("forge repo name {repo:?} is not a safe repo slug"));
    }
    if commit.len() != 40 || !commit.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!(
            "forge pinned commit {commit:?} is not a 40-hex sha1 oid"
        ));
    }
    let branch_ok = !branch.is_empty()
        && !branch.starts_with('-')
        && !branch.starts_with('/')
        && branch
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'));
    if !branch_ok {
        return Err(format!(
            "forge work branch {branch:?} is not a safe branch name"
        ));
    }
    Ok(())
}

/// provision one forge run: verify the repo + pinned commit are materialized
/// on THIS node, then `git worktree add` the work branch at the pinned commit
/// under the (already D7-validated) run dir.
pub(super) async fn provision(
    lane: &ForgeLane,
    run_dir: PathBuf,
    spec: &WorkspaceSpec,
) -> Result<Box<dyn ProvisionedWorkspace>, String> {
    let WorkspaceSource::Forge {
        repo,
        commit,
        branch,
        ..
    } = &spec.source
    else {
        return Err("forge provisioning invoked on a non-forge workspace spec".into());
    };
    validate_coords(repo, commit, branch)?;
    let repo_dir = lane.repo_base.join(repo);
    let push_url = format!("{}/{repo}", lane.push_base);

    // W6 parity with the duckfs lane: skill ro mounts are not materialized
    // yet — loud, never silent (the envelope pinned them on consensus).
    if !spec.ro_mounts.is_empty() {
        eprintln!(
            "[oracle] run {} requests {} skill mount(s) this provisioner \
             does not materialize yet — running without them",
            spec.run_id,
            spec.ro_mounts.len()
        );
    }

    let blocking = ProvisionArgs {
        repo_dir,
        run_dir,
        repo: repo.clone(),
        commit: commit.clone(),
    };
    let workspace_args = tokio::task::spawn_blocking(move || provision_blocking(blocking))
        .await
        .map_err(|_| "forge worktree provision task panicked".to_string())??;

    let mut env = BTreeMap::new();
    env.insert(
        "DUCKTAPE_RUN_WORKSPACE".into(),
        workspace_args.run_dir.display().to_string(),
    );
    Ok(Box::new(ForgeWorkspace {
        repo_dir: workspace_args.repo_dir,
        run_dir: workspace_args.run_dir,
        push_url,
        source: spec.source.clone(),
        agent_id: spec.agent_id.clone(),
        committer_name: lane.committer_name.clone(),
        env,
    }))
}

/// the plain-data bundle the blocking provision step consumes and returns.
struct ProvisionArgs {
    repo_dir: PathBuf,
    run_dir: PathBuf,
    repo: String,
    commit: String,
}

fn provision_blocking(args: ProvisionArgs) -> Result<ProvisionArgs, String> {
    let ProvisionArgs {
        repo_dir,
        run_dir,
        repo,
        commit,
    } = &args;
    if !repo_dir.join(".git").exists() {
        return Err(format!(
            "forge repo {repo:?} is not materialized on this node ({} is not a git repo)",
            repo_dir.display()
        ));
    }
    // the pinned commit must be ON DISK: committed refs can lead the local
    // pack (materialization lag — a missing/late pack blob leaves the on-disk
    // repo behind the committed head). a wrong-base worktree must never be
    // minted; the attempt fails and the saga's re-lease owns liveness.
    let present = git(repo_dir)
        .args(["cat-file", "-e", &format!("{commit}^{{commit}}")])
        .output()
        .map_err(|e| format!("host `git` failed to spawn: {e}"))?;
    if !present.status.success() {
        return Err(format!(
            "pinned commit {commit} is not present in forge repo {repo:?} on this node \
             (pack materialization can lag committed refs): {}",
            String::from_utf8_lossy(&present.stderr).trim()
        ));
    }
    // DETACHED checkout at the pinned commit — the provisioner never creates
    // or moves a shared-repo ref, so the consensus module's committed-ref
    // catch-up (which force-moves refs/heads/* in this repo while runs
    // execute) can never reparent the run's base under it. the work branch
    // exists only remotely; the push names it explicitly. concurrent
    // attempts of one item both provision (no branch to contend on) and the
    // push loop orders them.
    let add = git(repo_dir)
        .args(["worktree", "add", "--detach"])
        .arg(run_dir.as_os_str())
        .arg(commit)
        .output()
        .map_err(|e| format!("host `git` failed to spawn: {e}"))?;
    if !add.status.success() {
        // W5 applies to the error path too: a refused/partial add must not
        // strand a run dir or worktree metadata.
        cleanup_blocking(repo_dir, run_dir);
        return Err(format!(
            "git worktree add for forge repo {repo:?} at {commit} failed: {}",
            String::from_utf8_lossy(&add.stderr).trim()
        ));
    }
    Ok(args)
}

/// one live forge worktree: the shared repo it hangs off, its own checkout
/// dir, and everything the commit-and-push needs.
struct ForgeWorkspace {
    repo_dir: PathBuf,
    run_dir: PathBuf,
    push_url: String,
    source: WorkspaceSource,
    agent_id: Option<String>,
    committer_name: String,
    env: BTreeMap<String, String>,
}

impl ForgeWorkspace {
    /// a receipt-only spec, same trick as the duckfs lane: the constructors
    /// read only the source coords.
    fn receipt_spec(&self) -> WorkspaceSpec {
        WorkspaceSpec {
            run_id: String::new(),
            agent_id: None,
            source: self.source.clone(),
            base_tools: Vec::new(),
            ro_mounts: Vec::new(),
        }
    }

    /// the source's pinned base commit / work branch (forge variant by
    /// construction — provision refused anything else).
    fn coords(&self) -> (String, String) {
        match &self.source {
            WorkspaceSource::Forge { commit, branch, .. } => (commit.clone(), branch.clone()),
            WorkspaceSource::Duckfs { .. } => (String::new(), String::new()),
        }
    }
}

/// what the blocking commit step concluded.
enum CommitOutcome {
    /// clean tree AND the branch still sits at the pinned base — nothing to
    /// push, a true `no_changes`.
    NoChanges,
    /// the work landed: the branch was pushed and this is its new head oid.
    /// `rebased` = a concurrent advance was folded under it first (rides the
    /// receipt's EXISTING `rebased` field — the duckfs semantic twin).
    Pushed { oid: String, rebased: bool },
}

/// total push attempts before a rejection is a real failure. a concurrent
/// branch advance is an ORDERING problem, not a degrade: the consensus CAS
/// underneath stays the linearizer, a reject just triggers fetch → rebase →
/// push like a git user would. bounded so a hot/hostile remote can't spin
/// the commit bracket.
const PUSH_ATTEMPTS: u32 = 3;

fn commit_blocking(
    run_dir: &Path,
    pinned_commit: &str,
    branch: &str,
    push_url: &str,
    message: &str,
    author_name: &str,
    committer_name: &str,
) -> Result<CommitOutcome, String> {
    run_git(run_dir, &["add", "-A"], &[])?;
    let staged = staged_changes(run_dir)?;
    if staged {
        // D2 identities, env-only (NEVER global config): author = the agent
        // under the synthetic domain, committer = the executing node.
        let author_email = format!("{author_name}@{AGENT_EMAIL_DOMAIN}");
        run_git(
            run_dir,
            &["commit", "-m", message],
            &[
                ("GIT_AUTHOR_NAME", author_name),
                ("GIT_AUTHOR_EMAIL", &author_email),
                ("GIT_COMMITTER_NAME", committer_name),
                ("GIT_COMMITTER_EMAIL", NODE_COMMITTER_EMAIL),
            ],
        )?;
    }
    let head = run_git(run_dir, &["rev-parse", "HEAD"], &[])?;
    // a true no_changes needs BOTH: nothing to stage AND the branch still at
    // the pinned base — an agent that `git commit`ed its own work leaves a
    // clean tree that must still push.
    if !staged && head == pinned_commit {
        return Ok(CommitOutcome::NoChanges);
    }
    // plain push, NEVER --force. a rejection means the branch advanced while
    // the run executed — an ordering problem, so do what a git user does:
    // fetch the new tip, rebase the run's commits onto it (the author
    // survives a rebase natively; the committer stays the node via the same
    // env), push again. ONLY a genuine rebase conflict degrades (Err → the
    // pool's commit_failed + Degraded; the reply still delivers, R4), and
    // the interloper's tip stays branch head.
    let refspec = format!("HEAD:refs/heads/{branch}");
    let fetchspec = format!("refs/heads/{branch}");
    let committer_env = [
        ("GIT_COMMITTER_NAME", committer_name),
        ("GIT_COMMITTER_EMAIL", NODE_COMMITTER_EMAIL),
    ];
    let mut rebased = false;
    for attempt in 1..=PUSH_ATTEMPTS {
        match run_git(run_dir, &["push", push_url, &refspec], &[]) {
            Ok(_) => {
                // re-read AFTER any rebase: the pushed head is the output_commit.
                let oid = run_git(run_dir, &["rev-parse", "HEAD"], &[])?;
                return Ok(CommitOutcome::Pushed { oid, rebased });
            }
            Err(e) if attempt == PUSH_ATTEMPTS => {
                return Err(format!(
                    "push of branch {branch:?} to {push_url} was rejected after \
                     {PUSH_ATTEMPTS} attempts: {e}"
                ));
            }
            Err(_) => {
                // the remote tip comes ONLY from fetch — never the local
                // shared ref, which consensus catch-up moves/leaves stale. a
                // fetch miss means the branch is unborn remotely (a create
                // race that resolved away, or a non-tip reject like a hook):
                // skip the rebase and just push again.
                if run_git(run_dir, &["fetch", push_url, &fetchspec], &[]).is_ok() {
                    run_git(run_dir, &["rebase", "FETCH_HEAD"], &committer_env).map_err(|e| {
                        // never leave the worktree mid-rebase; the
                        // interloper's tip stays branch head (best-effort
                        // abort, then degrade).
                        let _ = git(run_dir).args(["rebase", "--abort"]).output();
                        format!("rebase conflict on {branch:?}: {e}")
                    })?;
                    rebased = true;
                }
            }
        }
    }
    unreachable!("the push loop returns on every arm of its final attempt")
}

/// whether `git add -A` staged anything (index vs HEAD) — the exit-code
/// trichotomy of `git diff --cached --quiet`.
fn staged_changes(run_dir: &Path) -> Result<bool, String> {
    let out = git(run_dir)
        .args(["diff", "--cached", "--quiet"])
        .output()
        .map_err(|e| format!("host `git` failed to spawn: {e}"))?;
    match out.status.code() {
        Some(0) => Ok(false),
        Some(1) => Ok(true),
        _ => Err(format!(
            "git diff --cached failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )),
    }
}

/// remove the run worktree AND its metadata in the parent repo — shared by
/// the W5 cleanup and provision's own error path (self-cleanup of debris).
/// idempotent, best-effort: every error is swallowed (cleanup must never
/// fail the run, and an already-gone dir is success).
fn cleanup_blocking(repo_dir: &Path, run_dir: &Path) {
    // `worktree remove --force` handles the registered case, dirty/untracked
    // trees included; the plain removal covers a half-made dir git no longer
    // recognizes; prune drops any leftover `.git/worktrees` metadata.
    let _ = git(repo_dir)
        .args(["worktree", "remove", "--force"])
        .arg(run_dir.as_os_str())
        .output();
    let _ = std::fs::remove_dir_all(run_dir);
    let _ = git(repo_dir).args(["worktree", "prune"]).output();
}

#[async_trait::async_trait]
impl ProvisionedWorkspace for ForgeWorkspace {
    fn workdir(&self) -> PathBuf {
        self.run_dir.clone()
    }

    fn env(&self) -> BTreeMap<String, String> {
        self.env.clone()
    }

    fn path_entries(&self) -> Vec<PathBuf> {
        Vec::new()
    }

    async fn commit(&self, message: &str) -> Result<WorkspaceReceipt, String> {
        let (pinned_commit, branch) = self.coords();
        let run_dir = self.run_dir.clone();
        let push_url = self.push_url.clone();
        let message = message.to_string();
        // D2: author = the agent (synthetic email), committer = the node.
        let author_name = self.agent_id.clone().unwrap_or_else(|| "agent".into());
        let committer_name = self.committer_name.clone();
        let outcome = tokio::task::spawn_blocking(move || {
            commit_blocking(
                &run_dir,
                &pinned_commit,
                &branch,
                &push_url,
                &message,
                &author_name,
                &committer_name,
            )
        })
        .await
        .map_err(|_| "forge workspace commit task panicked".to_string())??;
        let spec = self.receipt_spec();
        Ok(match outcome {
            CommitOutcome::NoChanges => WorkspaceReceipt::no_changes(&spec),
            CommitOutcome::Pushed { oid, rebased } => {
                let mut receipt = WorkspaceReceipt::pushed(&spec, oid);
                // the receipt's EXISTING field, the duckfs semantic twin
                // (work landed on a moved base) — no wire change.
                receipt.rebased = rebased;
                receipt
            }
        })
    }

    async fn cleanup(&self) {
        let repo_dir = self.repo_dir.clone();
        let run_dir = self.run_dir.clone();
        let _ =
            tokio::task::spawn_blocking(move || cleanup_blocking(&repo_dir, &run_dir)).await;
    }
}

#[cfg(test)]
#[path = "forge_tests.rs"]
mod tests;
