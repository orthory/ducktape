//! the vcs WIRE ops — what actually replicates across peers.
//!
//! these are NOT git verbs. a git verb (commit, branch, merge) runs locally and
//! produces a host-specific sha, so replaying it on a peer diverges; the verbs
//! live in [`crate::cmd::Command`] (local-only). what crosses the wire is the
//! *result* of a verb: a ref now points at an object whose id every node agrees
//! on. that fact is [`Op::RefUpdate`], applied on a receiver via `git
//! update-ref` — never by replaying the verb.

use crate::object::ObjectId;

/// a fully-qualified git ref name, e.g. `"refs/heads/main"`. a plain `String`
/// for now; a newtype enforcing the `refs/...` shape can come later if needed.
pub type RefName = String;

/// the canonical branch — the ONE ref whose advances consensus total-orders;
/// every other ref gossips over broadcast (the canonical-head inversion). lives
/// here so `op::lane()`, the integrator, and the lane tests share one definition
/// instead of scattering the literal.
pub const MAIN_REF: &str = "refs/heads/main";

/// vcs ops that replicate. every git verb that moves a ref collapses into
/// [`Op::RefUpdate`]; object availability is hinted by [`Op::Announce`].
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Op {
    /// "`name` now points at `target`" — the replicated fact behind every
    /// commit / branch / merge / tag. `prev` is the expected current target for
    /// a compare-and-set (fork detection); `None` means the ref is being
    /// created. applied on a receiver via `git update-ref`, never by replaying
    /// the originating verb.
    RefUpdate {
        name: RefName,
        target: ObjectId,
        prev: Option<ObjectId>,
    },

    /// "i hold these objects" — a broadcast hint so peers can fetch a ref's
    /// object closure by address. carries no authority; purely an availability
    /// advertisement.
    Announce { objects: Vec<ObjectId> },
}
