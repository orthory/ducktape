//! the wasm port of this module, built the ADAPTER
//! way: the NATIVE `dispatch` crate is compiled to wasm32 unmodified and adapted
//! to the `ducktape:module` world through `guest-adapter`, so the module's logic
//! is single-sourced (a behavior change in the native crate IS the wasm change).
//!
//! dispatch is a SELF-CONTAINED consensus plane: its `execute` reads only
//! `ctx.env()` (the caller origin and consensus time) and EMITS follow-ups
//! (`emit_msg` to saga, `emit_event` breadcrumbs) through the wit imports. it
//! makes no cross-module `query-module` reads inside execute, so — unlike the
//! `tagging`/`runs` tenants — it needs no memoized sibling replay on the accept
//! path. its query surface is COMMITTED-ONLY regardless of caller, which this
//! component declares (`shape().committed_queries`): the host drops the outer
//! staged overlay for a query round, so `WitStore` answers the native module's
//! `get_committed` reads from committed state exactly as the native store does.
//!
//! ## the STORE-BACKED dispatch model
//!
//! dispatch is pure logic over a host-injected [`sdk::MerkleStore`] — so the
//! port injects [`WitStore`], the adapter's `MerkleStore` over the wit `state-*`
//! imports, and the REAL qmdb store stays host-side
//! (`WasmModule::with_store`). there is NO per-dispatch snapshot: the store IS
//! the state and the wasm root is the store's Merkle root, so this port is
//! ROOT-CONTINUOUS with the native module (pinned block-by-block by
//! `wasm_dispatch_parity`). the staging contract is the one `pages`/`tasks`
//! ride:
//!
//! * the guest rebuilds the module FRESH per dispatch over the production
//!   constructor (`DispatchModule::new("dispatch", "saga", store)`); its inner
//!   `pending` overlay is per-dispatch, and cross-dispatch read-your-writes
//!   comes from the host's outer staged overlay via `WitStore::get`
//!   (staged-over-committed).
//! * each successful `execute` flushes the inner staging with the inner
//!   `commit_block` — `state-set`/`state-delete` OUTER staging the host
//!   publishes into the real store in ONE `commit_batch` at the true block
//!   boundary. the accepted no-ops (a duplicate `Dispatch`, a `Nudge`, a
//!   correlation-mismatched callback) stage NOTHING on either side, so the op
//!   log — and the root — stays byte-identical there too.
//! * a rejected op stages nothing (every write path CHECKS its records before
//!   staging any of them) and the runtime restores the pre-dispatch overlay.
//!
//! the saga collaborator id is genesis-constant, compiled in below.

use crate::DispatchModule;

/// the genesis-constant id this module registers under (the native twin's id:
/// `Env::me` and follow-up routing must read identically to ported logic).
const MODULE_ID: &str = "dispatch";

/// the saga module every dispatch triggers its work through — genesis config,
/// not committed state, so the guest carries the production wiring verbatim.
const SAGA_MODULE_ID: &str = "saga";

use guest_adapter::WitStore;

// store-backed port: no snapshot — the host owns the real qmdb store and the
// module is rebuilt fresh per dispatch (see `guest_adapter::store_guest!`).
guest_adapter::store_guest! {
    id: MODULE_ID,
    module: DispatchModule,
    // the query lane is committed-only regardless of caller: the between-block
    // delivery injection must never observe a same-block staged write.
    shape: guest_adapter::host::ModuleShape {
        committed_queries: true,
        ..guest_adapter::store_shape()
    },
    new: DispatchModule::new(MODULE_ID, SAGA_MODULE_ID, Box::new(WitStore)),
}
