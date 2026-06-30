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

    use vcs::objects::{export_reachable, import, resolve_ref, snapshot_worktree};
    use vcs::op::{Op, MAIN_REF};
    use vcs::{GitOdb, ObjectId};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    /// a unique temp dir under a process-wide counter — same collision-proofing
    /// as [`repo_with_commit`], so parallel tests in one binary can't share a path.
    fn temp_repo_dir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "ducktape-git-apply-{tag}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed),
        ))
    }

    /// a fresh empty sha256 repo with no objects and no refs — the receiving end
    /// of a transfer, before any closure has landed.
    fn empty_repo(tag: &str) -> PathBuf {
        GitOdb::init(&temp_repo_dir(tag))
            .expect("init sha256 odb")
            .repo()
            .to_path_buf()
    }

    /// a fresh sha256 repo holding one committed file, returned with the tip
    /// commit oid — its whole closure (commit, root tree, blob) is present and
    /// loose. the commit exists but no ref points at it yet.
    fn repo_with_commit(tag: &str) -> (PathBuf, ObjectId) {
        let odb = GitOdb::init(&temp_repo_dir(tag)).expect("init sha256 odb");
        let repo = odb.repo().to_path_buf();
        std::fs::write(repo.join("doc.md"), b"payload\n").unwrap();
        let commit = snapshot_worktree(&repo, "snap").expect("snapshot");
        (repo, commit)
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
    fn parks_a_ref_move_whose_closure_is_missing_then_advances_it_when_the_closure_lands() {
        // SOURCE holds a full closure -> commit C. a SEPARATE empty target repo R
        // has none of C's objects, mirroring a receiver before the transport has
        // delivered them.
        let (source, commit) = repo_with_commit("park-source");
        let target = empty_repo("park-target");

        // apply on the missing closure PARKS the move: it succeeds (no longer an
        // Err) and leaves the ref unmoved — the move can't land into a hole.
        let mut vcs = GitVcs::new(&target);
        let op = Op::RefUpdate { name: MAIN_REF.to_string(), target: commit, prev: None };
        vcs.apply(&op).expect("apply parks the move when the closure is missing");
        assert_eq!(
            resolve_ref(&target, MAIN_REF).unwrap(),
            None,
            "the ref must stay unset while the closure is absent",
        );

        // land C's whole closure into R the way the real transport will: export
        // the reachable pack from source, unpack it into the target.
        let pack = export_reachable(&source, &commit).expect("export closure from source");
        import(&target, &pack).expect("import closure into target");

        // poll re-fires the parked move now that the objects are present.
        vcs.poll().expect("poll advances the parked move once the closure lands");
        assert_eq!(
            resolve_ref(&target, MAIN_REF).unwrap(),
            Some(commit),
            "the ref advances to the finalized commit after its closure lands",
        );

        std::fs::remove_dir_all(&source).ok();
        std::fs::remove_dir_all(&target).ok();
    }

    #[test]
    fn poll_is_a_noop_with_nothing_parked() {
        let repo = empty_repo("poll-noop");
        let mut vcs = GitVcs::new(&repo);
        vcs.poll().expect("poll on a fresh handler is Ok");
        assert_eq!(
            resolve_ref(&repo, MAIN_REF).unwrap(),
            None,
            "poll moves no ref when nothing is parked",
        );
        std::fs::remove_dir_all(&repo).ok();
    }

    #[test]
    fn a_newer_parked_move_supersedes_an_older_one_for_the_same_ref() {
        // SOURCE holds two distinct parentless commits, C1 then C2, each with its
        // own full (independent) closure.
        let (source, c1) = repo_with_commit("supersede-source");
        std::fs::write(source.join("doc.md"), b"newer payload\n").unwrap();
        let c2 = snapshot_worktree(&source, "newer").expect("second snapshot");
        assert_ne!(c1, c2, "the two snapshots are distinct commits");

        // SEPARATE empty target: neither closure is present yet.
        let target = empty_repo("supersede-target");
        let mut vcs = GitVcs::new(&target);

        // park C1, then park C2 for the same ref — C2 supersedes C1.
        vcs.apply(&Op::RefUpdate { name: MAIN_REF.to_string(), target: c1, prev: None })
            .expect("park C1");
        vcs.apply(&Op::RefUpdate { name: MAIN_REF.to_string(), target: c2, prev: None })
            .expect("park C2 supersedes C1");

        // land ONLY C1's closure. if C1 were still parked it would advance the
        // ref here; because C2 superseded it and C2's closure is still absent,
        // poll fires nothing and the ref stays unset.
        import(&target, &export_reachable(&source, &c1).expect("export C1")).expect("import C1");
        vcs.poll().expect("poll with only the superseded closure present");
        assert_eq!(
            resolve_ref(&target, MAIN_REF).unwrap(),
            None,
            "the superseded older move must not advance the ref",
        );

        // land C2's closure: now the surviving parked move fires.
        import(&target, &export_reachable(&source, &c2).expect("export C2")).expect("import C2");
        vcs.poll().expect("poll once C2's closure lands");
        assert_eq!(
            resolve_ref(&target, MAIN_REF).unwrap(),
            Some(c2),
            "the newer move is the one that advances the ref",
        );

        std::fs::remove_dir_all(&source).ok();
        std::fs::remove_dir_all(&target).ok();
    }
}
