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
//! ## A0 status: a stub
//!
//! the real inbound path lands in **A4**: ensure the target's object closure is
//! present locally (fetched by address), then move the ref with `git update-ref`
//! — never by replaying the originating commit, which is what keeps every node's
//! git hash identical. until then [`GitVcs::apply`] surfaces an explicit
//! "unimplemented" [`EngineError::Vcs`] rather than silently no-op'ing, so a
//! premature `Node` wiring can't quietly drop ref updates.
//!
//! ## what A0 resolved
//!
//! before A0 the git verbs rode `op::Op::Vcs` on the consensus lane, so wiring a
//! real handler into a [`Node`](crate::Node) would replay `git commit` on every
//! receiver (host-specific shas → divergence). A0 removed that conflict at the
//! root: the verbs are local-only [`vcs::cmd::Command`]s, and the only vcs ops on
//! the wire are `RefUpdate`/`Announce`, applied by ref-move — not by replay.

use std::path::{Path, PathBuf};

use crate::{EngineError, VcsApply};

/// a [`VcsApply`] handler that applies vcs wire ops against a single working
/// repo. holds the repo path; the real ref-move logic lands in A4 (module docs).
pub struct GitVcs {
    repo: PathBuf,
}

impl GitVcs {
    /// bind a handler to the working repo at `repo`. does not create the repo —
    /// a worker `init`s it locally via [`vcs::cmd::Command::Init`]; the engine
    /// only ever applies ref-moves here.
    pub fn new(repo: impl Into<PathBuf>) -> Self {
        Self { repo: repo.into() }
    }

    /// the working repo this handler operates on (the A4 `git update-ref` target).
    pub fn repo(&self) -> &Path {
        &self.repo
    }
}

impl VcsApply for GitVcs {
    /// apply a vcs wire op against the bound repo.
    ///
    /// **A0 stub.** the real inbound path — ensure the object closure is present,
    /// then `git update-ref` — lands in A4. until then both arms surface
    /// [`EngineError::Vcs`] rather than succeeding, so a premature `Node` wiring
    /// can't quietly drop ref updates (the engine's "surface, don't swallow"
    /// stance).
    fn apply(&mut self, op: &vcs::op::Op) -> Result<(), EngineError> {
        match op {
            vcs::op::Op::RefUpdate { name, .. } => Err(EngineError::Vcs(format!(
                "RefUpdate({name}) apply (git update-ref) is unimplemented until A4"
            ))),
            vcs::op::Op::Announce { .. } => Err(EngineError::Vcs(
                "Announce handling (blob fetch by address) is unimplemented until A3/A4".into(),
            )),
        }
    }
}
