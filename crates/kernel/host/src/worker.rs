//! the worker seam — the host-owned, NON-DETERMINISTIC lane beside the drain.
//!
//! the host's within-block drain is the DETERMINISTIC re-entry lane: `emit_msg`
//! follow-ups re-dispatch inside ONE block, one app-hash. this seam is the
//! non-deterministic lane: a block may emit [`Event`]s, and each event that a
//! [`Worker`] claims produces a follow-up op submitted as a SEPARATE block —
//! because on a real node it is a separate consensus transaction (the
//! oracle-as-op). that split — deterministic hops stay local and free, only
//! genuine external edges pay a round — is the whole design.
//!
//! this seam is domain-agnostic: it knows `Event` and `Msg`, nothing about
//! sagas. a [`Worker`] try-decodes an event it recognizes and, off to the
//! side (non-deterministically, in the real world), computes a result and
//! returns the op that carries it back. modules stay pure; only the worker is
//! impure, and the seam lives HERE on the host side, never inside a module
//! crate — so a module never depends on it. the drive loop itself lives with
//! each binary (validator drain, noded, simnode), which all follow the same
//! shape: submit, offer each emitted event to every worker, submit each
//! claimed follow-up as its own block. events a worker does NOT claim are the
//! plain observability stream — one lane, two consumer classes.

use sdk::{Event, Msg};

/// outer-loop non-termination guard — the async sibling of the host's
/// `MAX_DISPATCHES`. bounds how many worker rounds one settle loop may drive
/// before giving up, so a worker that keeps re-triggering itself can't spin
/// forever.
pub const MAX_WORKER_ROUNDS: u32 = 256;

/// errors from driving workers.
#[derive(Debug)]
pub enum Error {
    /// the host rejected a submitted op (deterministic, block rolled back).
    Host(sdk::Error),
    /// a node-local block-boundary fault — the host registry is indeterminate
    /// and the caller must fail-stop (see [`crate::FatalError`]).
    Fatal(crate::FatalError),
    /// a worker failed to produce its result.
    Worker(String),
    /// the outer worker loop exceeded [`MAX_WORKER_ROUNDS`].
    BudgetExceeded,
}

impl From<sdk::Error> for Error {
    fn from(e: sdk::Error) -> Self {
        Error::Host(e)
    }
}

impl From<crate::SubmitError> for Error {
    fn from(e: crate::SubmitError) -> Self {
        match e {
            crate::SubmitError::Rejected(e) => Error::Host(e),
            crate::SubmitError::Fatal(f) => Error::Fatal(f),
        }
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::Host(e) => write!(f, "host error: {e}"),
            Error::Fatal(e) => write!(f, "{e}"),
            Error::Worker(m) => write!(f, "worker error: {m}"),
            Error::BudgetExceeded => write!(f, "worker-round budget exceeded"),
        }
    }
}

impl std::error::Error for Error {}

/// what a [`Worker`] did with an offered event. three-way on purpose:
/// "not my event" (keep offering) and "mine, deliberately not run" (stop
/// offering, nothing to submit) are different outcomes — collapsing them
/// would misreport an assignment-skipped request as an unclaimed drop.
#[derive(Debug)]
pub enum WorkOutcome {
    /// try-decode failed: not this worker's event — offer it to the next.
    NotMine,
    /// this worker's event, handled: submit the follow-up op if present.
    /// `None` is a deliberate no-op — e.g. the work is leased to another
    /// node's key and this host must not spawn it.
    Handled(Option<Msg>),
}

/// a host-owned, NON-DETERMINISTIC worker behind the event seam. given an
/// [`Event`] it recognizes, it does the off-consensus work (an LLM call, a fetch,
/// a commit) and returns the follow-up op that carries the result back through the
/// NORMAL submit path — the oracle-as-transaction. [`WorkOutcome::NotMine`] means
/// try-decode routing failed, so the caller can offer each event to every
/// worker until one claims it.
///
/// the worker never gets a handle to any module: it CANNOT mutate state directly.
/// its only channel back into the state machine is the `Msg` it returns, which the
/// caller submits as an ordinary op. that is the oracle pattern enforced by type.
#[async_trait::async_trait(?Send)]
pub trait Worker {
    async fn run(&self, event: &Event) -> Result<WorkOutcome, Error>;
}
