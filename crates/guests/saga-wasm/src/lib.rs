//! `saga-wasm` — the wasm port of the `saga` module, built the ADAPTER way:
//! the NATIVE `saga` crate is compiled to wasm32 unmodified and adapted to
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
//! 2. runs the native `execute` over a [`WitCtx`] — INCLUDING the assignment
//!    pool reads (the valset membership for untagged sagas, the capability
//!    registry's announced providers for tagged ones), both host-routed
//!    `query-module` reads the runtime resolves through memoized replay,
//! 3. on success calls the INNER module's `commit_block` — publishing the
//!    ledger's per-op `pending` overlay into its committed map — and
//! 4. saves the new canonical snapshot back as STAGED host writes.
//!
//! the fold is safe for saga SPECIFICALLY because every decision in its
//! execute paths reads staged-over-committed (`SagaModule::get` shadows the
//! committed map with this block's `pending`, `visible_ids` unions both), so
//! a reloaded module that sees earlier same-block writes as committed decides
//! byte-identically — there is no frozen-committed read anywhere in its
//! handle paths (contrast upgrade's `Advance`, which stays native for exactly
//! that reason). the ordering-contract surfaces cross the seam unchanged:
//!
//! * P6 callbacks: `emit_msg` follow-ups leave through the wit `emit-msg`
//!   import and the runtime republishes them on a clean execute — the
//!   requester callback still commits in the SAME block as the terminal
//!   transition.
//! * WORK-ORDER EVENTS: `emit_event` crosses the wit `emit-event` import and
//!   the runtime forwards it into the block's event trace — the host-side
//!   worker seam decodes the identical [`saga::WorkerRequest`] bytes from
//!   `BlockOutcome::events` on both runtimes (pinned by
//!   `wasm_saga_parity.rs`), so the reactor feeds workers identically.
//! * abort/rejection: nothing was saved (step 4 never ran), the runtime
//!   restores the pre-dispatch overlay, and the host discards the outer
//!   staging on a block abort — exactly the native `abort_block` story.
//!
//! the persisted encoding is the native module's canonical snapshot stored as
//! ONE host-KV value, so the wasm root is the host-KV encoding over the two
//! reserved keys — a STATE-SCHEMA BREAK versus the native root (revision 2 in
//! `MODULE_STATE_SCHEMAS`; beta networks re-genesis, no back-compat shim).

use guest_adapter::{block_on, host, load_state, save_state, Guest, WitCtx};
use saga::{LeasePolicy, SagaModule};
use sdk::{Error, Module as _, Msg, StateRoot};

/// the genesis-constant id this module registers under (the native twin's id:
/// `Env::me` and follow-up routing must read identically to ported logic).
const MODULE_ID: &str = "saga";
/// the sibling ids this instance reads through host-routed queries — EXACTLY
/// the production wiring (`bin/node/src/host_state.rs`): the valset module
/// whose membership assigns untagged sagas, and the capability registry whose
/// announced providers assign capability-tagged ones.
const VALSET_ID: &str = "valset";
const CAPABILITY_ID: &str = "capability";

struct Component;

/// the native module at THIS dispatch's state: genesis shape when nothing was
/// ever persisted, else the persisted snapshot verify-then-adopted against its
/// persisted root. an install failure is host-store corruption surfaced as a
/// deterministic rejection, never a silent re-genesis.
fn loaded_module() -> Result<SagaModule, host::Error> {
    let mut module = SagaModule::with_assignment(
        MODULE_ID,
        VALSET_ID,
        CAPABILITY_ID,
        LeasePolicy::Strict,
    );
    if let Some((bytes, root)) = load_state() {
        module
            .install(&bytes, StateRoot(root))
            .map_err(|e| host::Error::Rejected(format!("saga state reload: {e}")))?;
    }
    Ok(module)
}

/// map an inner sdk error onto the wit surface. `Module` is the native
/// rejection verbatim; anything else a native saga never surfaces from
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
        // is already folded into `__state`. saga queries (Get / NextExpiry /
        // AssignedPending) are pure ledger reads — no sibling access — so the
        // ctx-less native `query` is the whole surface.
        let module = loaded_module()?;
        block_on(module.query(&req)).map_err(to_wit_error)
    }
}

guest_adapter::export_module!(Component);
