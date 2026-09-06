//! the wasm port of this module, built the ADAPTER way:
//! the NATIVE `saga` crate is compiled to wasm32 unmodified and adapted to
//! the `ducktape:module` world through `guest-adapter`, so the module's logic
//! is single-sourced (a behavior change in the native crate IS the wasm
//! change).
//!
//! ## the STORE-BACKED dispatch model
//!
//! saga is pure logic over a host-injected [`sdk::MerkleStore`] — so the port
//! injects [`WitStore`], the adapter's `MerkleStore` over the wit `state-*`
//! imports, and the REAL qmdb store stays host-side
//! (`WasmModule::with_store`). there is NO per-dispatch snapshot: the store IS
//! the state and the wasm root is the store's Merkle root, so this port is
//! ROOT-CONTINUOUS with the native module (pinned block-by-block by
//! `wasm_saga_parity`).
//!
//! * the guest rebuilds the module FRESH per dispatch over the production
//!   constructor; its inner staging overlay is per-dispatch, and cross-dispatch
//!   read-your-writes comes from the host's outer staged overlay via
//!   `WitStore::get` (staged-over-committed). the fold is safe for saga
//!   SPECIFICALLY because every decision in its execute paths reads
//!   staged-over-committed — there is no frozen-committed read anywhere in its
//!   handle paths (contrast the modules registry's `Advance`, which stays native for
//!   exactly that reason).
//! * each successful `execute` flushes the inner staging with the inner
//!   `commit_block` — `state-set`/`state-delete` OUTER staging the host
//!   publishes into the real store in ONE `commit_batch` at the true block
//!   boundary. the accepted no-ops (a duplicate trigger, a stale result, a
//!   crank that finds nothing expired) stage NOTHING on either side, so the op
//!   log — and the root — stays byte-identical there too.
//!
//! the ordering-contract surfaces cross the seam unchanged:
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
//! * SIBLING READS: the assignment pool (the valset membership for untagged
//!   sagas, the capability registry's announced providers for tagged ones) is
//!   a host-routed `query-module` read the runtime resolves through memoized
//!   replay.
//! * abort/rejection: nothing was flushed, the runtime restores the
//!   pre-dispatch overlay, and the host discards the outer staging on a block
//!   abort — exactly the native `abort_block` story.

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

use guest_adapter::WitStore;

// store-backed port: no snapshot — the host owns the real qmdb store and the
// module is rebuilt fresh per dispatch (see `guest_adapter::store_guest!`).
guest_adapter::store_guest! {
    id: MODULE_ID,
    module: SagaModule,
    shape: guest_adapter::store_shape(),
    new: SagaModule::with_assignment(
        MODULE_ID,
        Box::new(WitStore),
        VALSET_ID,
        CAPABILITY_ID,
        LeasePolicy::Strict,
    ),
}
