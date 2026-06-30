//! the one top-level op dispatcher.
//!
//! [`Engine`] is the single seam every op (whatever lane it rode in on, whatever
//! node it landed at) flows through: `apply(op::Op)` matches the four-arm
//! taxonomy and routes each to its handler. document/workspace state lives
//! inline as a [`Workspace`] hydrated in place; vcs + control are injectable
//! handler traits so the layers behind them plug in without this file changing
//! shape. [`GitVcs`] is the inbound vcs (ref-move) handler — the wire ops
//! `RefUpdate`/`Announce`, with the real `git update-ref` path landing in A4;
//! the agentic supervisor (p2.1) still slots into the control seam later.
//!
//! handlers can emit follow-up ops: `apply` returns `Vec<op::Op>`. today only
//! [`ControlApply`] uses this (a control op may want a `Vcs(RefUpdate)` to
//! follow); the workspace/vcs arms return an empty vec.

use hydration::Hydratable;
use workspace::Workspace;

mod git;
mod node;
pub use git::GitVcs;
pub use node::{Config, Node, NoopWorker, run_loopback_demo};

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// a bare top-level `Op::Document` arrived with no entry context. document
    /// edits are expected to ride wrapped as `Workspace::EntryMut { entry_id, op }`
    /// so the dispatcher knows which file to apply them to; an unwrapped one has
    /// no target. we surface this rather than silently dropping it — in a
    /// convergence engine a quietly-discarded op is exactly how silent
    /// divergence hides.
    #[error("bare top-level Document op has no entry context; wrap it as Workspace::EntryMut")]
    UnroutableBareDocumentOp,

    /// a vcs handler failed.
    #[error("vcs: {0}")]
    Vcs(String),

    /// a control handler failed.
    #[error("control: {0}")]
    Control(String),
}

/// seam for the vcs (git) layer. the real impl is [`GitVcs`] (applies the
/// `RefUpdate`/`Announce` wire ops by ref-move; the `git update-ref` path lands
/// in A4); [`NoopVcs`] is the default no-side-effect handler.
pub trait VcsApply: Send {
    fn apply(&mut self, op: &vcs::op::Op) -> Result<(), EngineError>;

    /// re-fire any ref moves parked because their object closure wasn't present
    /// locally when first applied. the default is a no-op — a handler that never
    /// parks (e.g. [`NoopVcs`]) inherits it.
    fn poll(&mut self) -> Result<(), EngineError> {
        Ok(())
    }
}

/// seam for the agentic control/supervisor layer — implemented for real in p2.1.
/// a control op may emit follow-up ops (e.g. a `Vcs(RefUpdate)`) for the engine
/// to route, hence the `Vec<op::Op>` return.
pub trait ControlApply: Send {
    fn apply(&mut self, op: &control::op::Op) -> Result<Vec<op::Op>, EngineError>;
}

/// default vcs handler: accepts everything, does nothing. the no-side-effect
/// choice for engines that don't want a real repo (most tests); inject
/// [`GitVcs`] with [`Engine::with_vcs`] for the real git layer.
pub struct NoopVcs;

impl VcsApply for NoopVcs {
    fn apply(&mut self, _op: &vcs::op::Op) -> Result<(), EngineError> {
        Ok(())
    }
}

/// default control handler: accepts everything, emits no follow-up ops.
pub struct NoopControl;

impl ControlApply for NoopControl {
    fn apply(&mut self, _op: &control::op::Op) -> Result<Vec<op::Op>, EngineError> {
        Ok(vec![])
    }
}

/// the top-level dispatcher. owns the workspace state inline; vcs + control are
/// injectable handler boxes (defaulting to the noop impls).
pub struct Engine {
    workspace: Workspace,
    vcs: Box<dyn VcsApply>,
    control: Box<dyn ControlApply>,
}

impl Engine {
    /// new engine over `workspace`, wired to the noop handlers. inject real
    /// handlers with [`Engine::with_vcs`] / [`Engine::with_control`].
    pub fn new(workspace: Workspace) -> Self {
        Self {
            workspace,
            vcs: Box::new(NoopVcs),
            control: Box::new(NoopControl),
        }
    }

    /// inject the real vcs handler (builder style).
    pub fn with_vcs(mut self, vcs: Box<dyn VcsApply>) -> Self {
        self.vcs = vcs;
        self
    }

    /// inject the real control handler (builder style).
    pub fn with_control(mut self, control: Box<dyn ControlApply>) -> Self {
        self.control = control;
        self
    }

    /// apply one op, routing by its taxonomy arm. returns any follow-up ops the
    /// handlers emitted (only control does, today).
    pub fn apply(&mut self, op: op::Op) -> Result<Vec<op::Op>, EngineError> {
        match op {
            // workspace ops (structural AddEntry/RemoveEntry/MoveEntry, and
            // wrapped EntryMut document edits) fold straight into local state.
            op::Op::Workspace(w) => {
                self.workspace.hydrate(std::iter::once(w));
                Ok(vec![])
            }

            // a bare top-level document op has no entry context — document edits
            // normally arrive wrapped as Workspace::EntryMut. surface it rather
            // than silently dropping (see EngineError::UnroutableBareDocumentOp).
            op::Op::Document(_) => Err(EngineError::UnroutableBareDocumentOp),

            op::Op::Vcs(v) => {
                self.vcs.apply(&v)?;
                Ok(vec![])
            }

            op::Op::Control(c) => self.control.apply(&c),
        }
    }

    /// drive the vcs handler's retry of parked ref moves. a ref move whose
    /// object closure wasn't present when first applied is parked, not failed;
    /// this re-fires it once the objects land locally. a future node-tick calls
    /// this so parked moves eventually advance.
    pub fn poll_vcs(&mut self) -> Result<(), EngineError> {
        self.vcs.poll()
    }

    /// read-only view of the engine's workspace state.
    pub fn workspace(&self) -> &Workspace {
        &self.workspace
    }
}
