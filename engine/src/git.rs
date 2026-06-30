//! the git layer — the inbound [`VcsApply`](crate::VcsApply) handler.
//!
//! [`GitVcs`] is the engine's seam for applying the vcs WIRE ops
//! ([`vcs::op::Op`]) — `RefUpdate` and `Announce` — against a single working
//! `repo`. it does NOT run git verbs: those are [`vcs::cmd::Command`]s a worker
//! runs locally ([`vcs::cmd::run_local`]) and they never reach the engine. what
//! reaches here is the replicated *result* of a verb. inject it builder-style:
//!
//! ```ignore
//! let engine = Engine::new(workspace).with_vcs(Box::new(GitVcs::new("/path/to/repo")));
//! ```
//!
//! a finalized `RefUpdate` moves a ref with `git update-ref` — never by replaying
//! the originating commit, which is what keeps every node's git hash identical.
//! the move is gated on the target's whole object closure being present locally
//! ([`vcs::objects::closure_complete`]): the ref advances only once the commit,
//! every tree under it, and every blob it names are stored here, so a move can
//! never land a ref pointing into a hole. a target whose closure is incomplete
//! is PARKED rather than failed — recorded as a pending move and re-fired by
//! [`VcsApply::poll`] once the objects land locally — so `apply` succeeds and
//! the ref simply waits. fetching a missing closure by address is a separate
//! seam. `Announce` (an availability hint) isn't wired into a live node yet.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::{EngineError, VcsApply};

/// a [`VcsApply`] handler that applies vcs wire ops against a single working
/// repo. holds the repo path; moves a ref with `git update-ref` once the
/// target's object closure is present locally.
///
/// a move whose closure isn't present yet is parked in `pending` (ref name ->
/// the move's `(target, prev)`) instead of failing, and re-fired by [`poll`]
/// once the objects land. only one move per ref is kept — a newer move for a
/// ref supersedes any older parked one (last-write-wins; no compare-and-set).
///
/// [`poll`]: VcsApply::poll
pub struct GitVcs {
    repo: PathBuf,
    /// ref moves parked because their object closure wasn't present locally
    /// when applied: ref name -> `(target, prev)`. drained by [`poll`] as the
    /// objects land.
    ///
    /// [`poll`]: VcsApply::poll
    pending: HashMap<vcs::op::RefName, (vcs::ObjectId, Option<vcs::ObjectId>)>,
}

impl GitVcs {
    /// bind a handler to the working repo at `repo`. does not create the repo —
    /// a worker `init`s it locally via [`vcs::cmd::Command::Init`]; the engine
    /// only ever applies ref-moves here.
    pub fn new(repo: impl Into<PathBuf>) -> Self {
        Self {
            repo: repo.into(),
            pending: HashMap::new(),
        }
    }

    /// the working repo this handler operates on (the `git update-ref` target).
    pub fn repo(&self) -> &Path {
        &self.repo
    }
}

impl VcsApply for GitVcs {
    /// apply a vcs wire op against the bound repo.
    ///
    /// `RefUpdate` advances `name` to `target` with `git update-ref`, but only
    /// once `target`'s whole object closure is present locally. an incomplete
    /// closure PARKS the move — it's recorded in `pending` and `apply` returns
    /// `Ok` — rather than moving the ref into a hole or failing; [`VcsApply::poll`]
    /// re-fires it once the objects land. a newer move for a ref supersedes any
    /// older parked one (last-write-wins; no compare-and-set). `Announce` isn't
    /// wired into a live node yet and surfaces an error.
    fn apply(&mut self, op: &vcs::op::Op) -> Result<(), EngineError> {
        match op {
            vcs::op::Op::RefUpdate { name, target, prev } => {
                // a newer move for this ref supersedes an older parked one —
                // last-write-wins, matching the no-compare-and-set decision.
                self.pending.remove(name);
                if vcs::objects::closure_complete(&self.repo, target)
                    .map_err(|e| EngineError::Vcs(e.to_string()))?
                {
                    vcs::objects::update_ref(&self.repo, name, target)
                        .map_err(|e| EngineError::Vcs(e.to_string()))?;
                } else {
                    // closure isn't here yet — park the move; poll re-fires it
                    // once the objects land locally.
                    self.pending.insert(name.clone(), (*target, *prev));
                }
                Ok(())
            }
            vcs::op::Op::Announce { .. } => Err(EngineError::Vcs(
                "Announce handling (object fetch by address) is not yet wired".into(),
            )),
        }
    }

    /// re-fire parked ref moves whose object closure is now present locally.
    ///
    /// each pending `(name, (target, _))` whose closure is complete advances its
    /// ref with `git update-ref` and is dropped from `pending`; still-incomplete
    /// ones stay parked for a later poll. a genuine `closure_complete` /
    /// `update_ref` io fault propagates as an [`EngineError::Vcs`].
    fn poll(&mut self) -> Result<(), EngineError> {
        // collect the refs that landed, then drop them — don't mutate `pending`
        // while iterating it.
        let mut landed = Vec::new();
        for (name, (target, _prev)) in &self.pending {
            if vcs::objects::closure_complete(&self.repo, target)
                .map_err(|e| EngineError::Vcs(e.to_string()))?
            {
                vcs::objects::update_ref(&self.repo, name, target)
                    .map_err(|e| EngineError::Vcs(e.to_string()))?;
                landed.push(name.clone());
            }
        }
        for name in landed {
            self.pending.remove(&name);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    use vcs::objects::{resolve_ref, snapshot_worktree};
    use vcs::op::{Op, MAIN_REF};
    use vcs::{GitOdb, ObjectId};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    /// a fresh sha256 repo holding one committed file, returned with the tip
    /// commit oid — its whole closure (commit, root tree, blob) is present and
    /// loose. the commit exists but no ref points at it yet.
    fn repo_with_commit(tag: &str) -> (PathBuf, ObjectId) {
        // process-wide counter (not just pid+nanos) so parallel tests in one
        // binary can't collide on a temp dir under coarse clock resolution.
        let dir = std::env::temp_dir().join(format!(
            "ducktape-git-apply-{tag}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed),
        ));
        let odb = GitOdb::init(&dir).expect("init sha256 odb");
        let repo = odb.repo().to_path_buf();
        std::fs::write(repo.join("doc.md"), b"payload\n").unwrap();
        let commit = snapshot_worktree(&repo, "snap").expect("snapshot");
        (repo, commit)
    }

    /// remove the loose object file for `oid` (`.git/objects/<2>/<62>`) so the
    /// object is genuinely gone — freshly-written objects are loose (no pack).
    fn delete_loose(repo: &Path, oid: &ObjectId) {
        let hex = oid.to_hex();
        let path = repo.join(".git/objects").join(&hex[..2]).join(&hex[2..]);
        std::fs::remove_file(&path).unwrap_or_else(|e| panic!("remove loose {hex}: {e}"));
    }

    #[test]
    fn advances_the_ref_when_the_whole_closure_is_present() {
        let (repo, commit) = repo_with_commit("present");
        let mut vcs = GitVcs::new(&repo);
        let op = Op::RefUpdate { name: MAIN_REF.to_string(), target: commit, prev: None };
        vcs.apply(&op).expect("apply moves the ref when the closure is whole");
        assert_eq!(resolve_ref(&repo, MAIN_REF).unwrap(), Some(commit));
        std::fs::remove_dir_all(&repo).ok();
    }

    #[test]
    fn refuses_and_leaves_the_ref_unmoved_when_a_blob_is_missing() {
        let (repo, commit) = repo_with_commit("missing-blob");
        // delete the one blob the root tree names: the closure is now incomplete
        // even though `cat-file -e <commit>` still succeeds. this proves apply
        // gates on the DEEP closure check, not a shallow tip-present probe — and
        // that a rejected move leaves the ref untouched (no half-applied state).
        let blob_hex = String::from_utf8(
            vcs::git::run(&repo, &["rev-parse", &format!("{}:doc.md", commit.to_hex())], None)
                .unwrap()
                .stdout,
        )
        .unwrap()
        .trim()
        .to_string();
        let blob = ObjectId::from_hex(&blob_hex).unwrap();
        delete_loose(&repo, &blob);

        let mut vcs = GitVcs::new(&repo);
        let op = Op::RefUpdate { name: MAIN_REF.to_string(), target: commit, prev: None };
        assert!(vcs.apply(&op).is_err(), "an incomplete closure must not move the ref");
        assert_eq!(resolve_ref(&repo, MAIN_REF).unwrap(), None, "the ref must stay unset");
        std::fs::remove_dir_all(&repo).ok();
    }
}
