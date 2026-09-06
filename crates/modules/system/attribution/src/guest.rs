//! the wasm port of this module, built the ADAPTER way: the NATIVE
//! `attribution` crate is compiled to wasm32 unmodified and adapted to the
//! `ducktape:module` world through `guest-adapter`, so the module's logic is
//! single-sourced (a behavior change in the native crate IS the wasm change).
//!
//! store-backed port: the host owns the real qmdb store and the module is
//! rebuilt fresh per dispatch over [`WitStore`] (see
//! `guest_adapter::store_guest!`), which also exports the delivery seam
//! (`pending_items` / `acknowledge`) the host drives between blocks.

use guest_adapter::WitStore;

use crate::AttributionModule;

/// the genesis-constant id this module registers under (the native twin's id:
/// `Env::me`, the `ItemRef.source` of every delivery, and follow-up routing
/// must read identically to ported logic).
const MODULE_ID: &str = "attribution";

/// the modules subscribed from genesis on — wiring, not user setup. the same
/// genesis-constant collaborator ids the native topology builder wires.
const GENESIS_SUBSCRIBERS: [&str; 2] = ["inbox", "agent"];

guest_adapter::store_guest! {
    id: MODULE_ID,
    module: AttributionModule,
    shape: guest_adapter::store_shape(),
    new: AttributionModule::new(MODULE_ID, Box::new(WitStore)).with_subscribers(GENESIS_SUBSCRIBERS),
}
