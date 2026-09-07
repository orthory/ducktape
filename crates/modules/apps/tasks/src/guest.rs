//! the wasm port of this module, built the ADAPTER way: the native crate is
//! compiled to wasm32 unmodified and adapted to the `ducktape:module` world
//! through `ducktape-module-sdk`, so the module's logic is single-sourced (a behavior
//! change in the native crate IS the wasm change). the packaging cdylib around
//! this port is synthesized by `guest-builder` — this module is the whole of
//! the guest's hand-written surface.
//!
//! the guest takes the crate with `default-features = false`: the `native`
//! feature carries only the node-local derived index (whose `indexer` dep is
//! unix-only IO), never consensus state — so the ported state machine is the
//! FULL consensus surface.
//!
//! ## the STORE-BACKED dispatch model
//!
//! tasks is pure logic over a host-injected [`sdk::MerkleStore`] — so the port
//! injects [`WitStore`], the adapter's `MerkleStore` over the wit `state-*`
//! imports, and the REAL qmdb store stays host-side
//! (`WasmModule::with_store`). there is NO per-dispatch snapshot: the store IS
//! the state and the wasm root is the store's Merkle root, so this port is
//! ROOT-CONTINUOUS with the native module (pinned block-by-block by
//! `wasm_tasks_parity`). see the `pages` guest port for the staging contract
//! spelled out point by point — tasks rides the identical seams:
//!
//! * the guest rebuilds the module FRESH per dispatch over the production
//!   constructor (`Tasks::new("tasks", "identity", "attribution", store)`); its inner `pending` overlay is
//!   per-dispatch, and cross-dispatch read-your-writes comes from the host's
//!   outer staged overlay via `WitStore::get` (staged-over-committed).
//! * each successful `execute` flushes the inner staging with the inner
//!   `commit_block` — `state-set`/`state-delete` OUTER staging the host
//!   publishes into the real store in ONE `commit_batch` at the true block
//!   boundary. the accepted no-ops (a same-status task update, an already-
//!   registered worker) stage NOTHING on either side, so the op log — and the
//!   root — stays byte-identical there too.

use crate::Tasks;

/// the genesis-constant id this module registers under (the native twin's id:
/// `Env::me` and follow-up routing must read identically to ported logic).
const MODULE_ID: &str = "tasks";

use ducktape_module_sdk::WitStore;

// store-backed port: no snapshot — the host owns the real qmdb store and the
// module is rebuilt fresh per dispatch (see `ducktape_module_sdk::store_guest!`).
ducktape_module_sdk::store_guest! {
    id: MODULE_ID,
    module: Tasks,
    shape: ducktape_module_sdk::store_shape(),
    new: Tasks::new(MODULE_ID, "identity", "attribution", Box::new(WitStore)),
}
