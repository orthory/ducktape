//! the git layer (p1.3) — the real [`VcsApply`](crate::VcsApply) handler.
//!
//! [`GitVcs`] is the engine's first non-noop handler seam: it binds the
//! `vcs` crate's local git primitive ([`vcs::apply::apply`]) to a single working
//! `repo` path, so every `Op::Vcs(..)` that reaches [`Engine::apply`](crate::Engine::apply)
//! actually runs `git` in that repo. inject it builder-style:
//!
//! ```ignore
//! let engine = Engine::new(workspace).with_vcs(Box::new(GitVcs::new("/path/to/repo")));
//! ```
//!
//! ## what it carries
//!
//! the full [`vcs::op::Op`] taxonomy — `Init`, `Add`, `Commit`, `Branch`,
//! `Checkout`, `Merge`, `Tag` — i.e. every major local git operation. it does
//! NOT carry `push`/`pull`/`fetch`, and that's deliberate: in this model nodes
//! never sync git *state* over the network (replaying a `git commit` yields a
//! different sha, so peers would never converge on it). file *content* syncs via
//! workspace ops instead; the git layer is a purely-local side-effect log. see
//! [`vcs::apply`] for the full rationale.
//!
//! ## why this is engine-local and NOT yet wired into [`Node`](crate::Node)
//!
//! that same non-replication constraint is a live conflict against the wire
//! routing: `op::Op::Vcs` currently rides the *consensus* (broadcast) lane and
//! the node's inbound task applies every decoded op through `Engine::apply`. so
//! the instant a real `GitVcs` is wired into a `Node`, a peer's `Commit` would
//! replay `git commit` on every receiver — exactly what the model forbids.
//! resolving that (skip vcs ops on the inbound path, or stop broadcasting them)
//! is a separate step; until then `GitVcs` is constructed and driven directly
//! against an engine, never minted inside `Node::new`.

use std::path::{Path, PathBuf};

use crate::{EngineError, VcsApply};

/// a [`VcsApply`] handler that performs git ops as local side-effects against a
/// single working repo. holds only the repo path; the actual git invocation
/// lives in [`vcs::apply::apply`].
pub struct GitVcs {
    repo: PathBuf,
}

impl GitVcs {
    /// bind a handler to the working repo at `repo`. does not create or `init`
    /// the directory — drive a [`vcs::op::Op::Init`] through the engine for
    /// that, exactly as a real caller would.
    pub fn new(repo: impl Into<PathBuf>) -> Self {
        Self { repo: repo.into() }
    }

    /// the working repo this handler operates on.
    pub fn repo(&self) -> &Path {
        &self.repo
    }
}

impl VcsApply for GitVcs {
    /// run `op` against the bound repo. the git invocation's captured output is
    /// discarded — callers that need repo state read it back out of the repo —
    /// and any non-zero git exit (or spawn failure) is surfaced as
    /// [`EngineError::Vcs`] rather than swallowed, so a failed commit can't pass
    /// for a successful one.
    fn apply(&mut self, op: &vcs::op::Op) -> Result<(), EngineError> {
        vcs::apply::apply(op, &self.repo)
            .map(|_output| ())
            .map_err(|e| EngineError::Vcs(e.to_string()))
    }
}
