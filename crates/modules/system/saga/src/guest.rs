//! the wasm port of this module, built the ADAPTER way:
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

use crate::{LeasePolicy, SagaModule};

/// the genesis-constant id this module registers under (the native twin's id:
/// `Env::me` and follow-up routing must read identically to ported logic).
const MODULE_ID: &str = "saga";
/// the sibling ids this instance reads through host-routed queries — EXACTLY
/// the production wiring (`bin/node/src/host_state.rs`): the valset module
/// whose membership assigns untagged sagas, and the capability registry whose
/// announced providers assign capability-tagged ones.
const VALSET_ID: &str = "valset";
const CAPABILITY_ID: &str = "capability";

// whole-state port: the shell loads/saves the canonical snapshot and runs the
// native module per dispatch (see `guest_adapter::snapshot_guest!`).
guest_adapter::snapshot_guest! {
    id: MODULE_ID,
    module: SagaModule,
    new: SagaModule::with_assignment(MODULE_ID, VALSET_ID, CAPABILITY_ID, LeasePolicy::Strict),
}
