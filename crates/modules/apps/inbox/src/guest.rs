//! the wasm port of this module, built the ADAPTER way: the NATIVE `inbox`
//! crate is compiled to wasm32 unmodified and adapted to the `ducktape:module`
//! world through `ducktape-module-sdk`, so the module's logic is single-sourced (a
//! behavior change in the native crate IS the wasm change).
//!
//! store-backed port: the host owns the real qmdb store and the module is
//! rebuilt fresh per dispatch over [`WitStore`] (see
//! `ducktape_module_sdk::store_guest!`).

use ducktape_module_sdk::WitStore;

use crate::Inbox;

/// the genesis-constant id this module registers under (the native twin's id:
/// `Env::me` and follow-up routing must read identically to ported logic).
const MODULE_ID: &str = "inbox";

/// the collaborators, by their genesis-constant ids: the attribution module
/// whose deliveries this inbox ingests, and the identity module that resolves
/// recipients and admin keys. the native topology builder wires the same two.
const ATTRIBUTION_ID: &str = "attribution";
const IDENTITY_ID: &str = "identity";

ducktape_module_sdk::store_guest! {
    id: MODULE_ID,
    module: Inbox,
    shape: ducktape_module_sdk::store_shape(),
    new: Inbox::new(MODULE_ID, Box::new(WitStore), ATTRIBUTION_ID, IDENTITY_ID),
}
