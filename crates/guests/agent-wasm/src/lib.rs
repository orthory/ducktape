//! `agent-wasm` — the wasm port of the `agent` module, built the ADAPTER way:
//! the NATIVE `agent` crate is compiled to wasm32 unmodified and adapted to
//! the `ducktape:module` world through `guest-adapter`, so the module's logic
//! is single-sourced (a behavior change in the native crate IS the wasm
//! change).
//!
//! ## the whole-state dispatch model, and why it is equivalent
//!
//! the guest is re-instantiated per dispatch, so the native module inside it
//! must FULLY apply per dispatch. each `execute`:
//!
//! 1. loads the persisted snapshot through the host's staged-overlay reads
//!    (`__state`/`__root`, verify-then-adopt via the native `install`),
//! 2. runs the native `execute` over a [`WitCtx`] — the registry is
//!    self-contained (no sibling queries), but its two follow-up lanes cross
//!    the seam: the registry HOOK (`AgentEvent` msgs to the runs module, so
//!    an agent and its dispatch recipe stay one atomic unit) leaves through
//!    the wit `emit-msg` import, and the saga DEAD-LETTER arm (a foreign
//!    trigger's `reply_to` callback) swallows with an `emit-event` breadcrumb
//!    exactly as natively — never an abort,
//! 3. on success calls the INNER module's `commit_block`, and
//! 4. saves the new canonical snapshot back as STAGED host writes.
//!
//! the fold is safe for agent SPECIFICALLY because every decision in its
//! execute paths reads staged-over-committed (`AgentModule::agent` shadows
//! the committed map with `pending_agents`, `visible_ids` unions both), so a
//! reloaded module that sees earlier same-block writes as committed decides
//! byte-identically — no frozen-committed read anywhere (pinned by
//! `wasm_agent_parity.rs`).
//!
//! the persisted encoding is the native module's canonical snapshot stored as
//! ONE host-KV value, so the wasm root is the host-KV encoding over the two
//! reserved keys — a STATE-SCHEMA BREAK versus the native root (revision 2 in
//! `MODULE_STATE_SCHEMAS`; beta networks re-genesis, no back-compat shim).

use agent::AgentModule;
use guest_adapter::{block_on, host, load_state, save_state, Guest, WitCtx};
use sdk::{Error, Module as _, Msg, StateRoot};

/// the genesis-constant id this module registers under (the native twin's id:
/// `Env::me` and follow-up routing must read identically to ported logic).
const MODULE_ID: &str = "agent";
/// the sibling ids compiled into this instance — EXACTLY the production
/// wiring (`bin/node/src/host_state.rs`): saga is the dead-letter origin
/// router, runs the registry hook that keeps each agent's dispatch recipe in
/// lockstep.
const SAGA_ID: &str = "saga";
const HOOK_ID: &str = "runs";

struct Component;

/// the native module at THIS dispatch's state: genesis shape when nothing was
/// ever persisted, else the persisted snapshot verify-then-adopted against its
/// persisted root. an install failure is host-store corruption surfaced as a
/// deterministic rejection, never a silent re-genesis.
fn loaded_module() -> Result<AgentModule, host::Error> {
    let mut module = AgentModule::new(MODULE_ID, SAGA_ID, Some(HOOK_ID.into()));
    if let Some((bytes, root)) = load_state() {
        module
            .install(&bytes, StateRoot(root))
            .map_err(|e| host::Error::Rejected(format!("agent state reload: {e}")))?;
    }
    Ok(module)
}

/// map an inner sdk error onto the wit surface. `Module` is the native
/// rejection verbatim; anything else a native agent never surfaces from
/// its own execute, so the debug rendering is purely diagnostic.
fn to_wit_error(e: Error) -> host::Error {
    match e {
        Error::Module(m) => host::Error::Rejected(m),
        other => host::Error::Rejected(other.to_string()),
    }
}

impl Guest for Component {
    fn execute(payload: Vec<u8>) -> Result<(), host::Error> {
        let mut module = loaded_module()?;
        let mut ctx = WitCtx::new();
        block_on(module.execute(
            &mut ctx,
            &Msg {
                target: MODULE_ID.into(),
                payload,
            },
        ))
        .map_err(to_wit_error)?;
        // fully apply per dispatch: publish the inner per-op staging, then
        // persist the canonical snapshot as OUTER staged writes — the host
        // owns the real commit/abort boundary (see the crate doc).
        block_on(module.commit_block()).map_err(to_wit_error)?;
        save_state(&module.snapshot(), module.root().as_bytes());
        Ok(())
    }

    fn query(req: Vec<u8>) -> Result<Vec<u8>, host::Error> {
        // the loaded snapshot was saved post-inner-commit, so the native
        // query's committed+pending merge serves it with an empty pending —
        // the live (staged-overlay) projection the runtime hands this round
        // is already folded into `__state`. agent queries are pure registry
        // reads (no sibling access), so the ctx-less native `query` is the
        // whole surface.
        let module = loaded_module()?;
        block_on(module.query(&req)).map_err(to_wit_error)
    }
}

guest_adapter::export_module!(Component);
