//! the wasm port of this module, built the ADAPTER way: the native crate is
//! compiled to wasm32 unmodified and adapted to the `ducktape:module` world
//! through `ducktape-module-sdk`, so the module's logic is single-sourced (a
//! behaviour change in the native crate IS the wasm change). The packaging
//! cdylib around this port is synthesized by `guest-builder` — this file is
//! the whole of the guest's hand-written surface.
//!
//! ## the STORE-BACKED dispatch model
//!
//! collaboration is pure logic over a host-injected [`sdk::MerkleStore`], so
//! the port injects [`WitStore`], the adapter's `MerkleStore` over the wit
//! `state-*` imports, and the real qmdb store stays host-side. There is no
//! per-dispatch snapshot: the store IS the state and the wasm root is the
//! store's merkle root, so this port is root-continuous with the native
//! module. Each successful `execute` flushes the inner staging; a refused op
//! stages nothing on either side, which is why every write path here checks
//! all of its records before staging any of them.
//!
//! ## the delivery-deadline ceiling
//!
//! [`Collaboration::new`] takes the ceiling in `consensus_time` units, and a
//! wasm-composed genesis runs the height lane, so the port passes
//! [`crate::HEIGHT_LANE_MAX_DELIVERY_TTL`]. A network whose `consensus_time`
//! is a millisecond epoch clock needs that number scaled, which means a
//! genesis-config key this module does not yet declare: until one exists such
//! a network gets a ceiling that is too TIGHT — deadlines are refused, never
//! silently honoured for longer than promised.

use crate::{Collaboration, HEIGHT_LANE_MAX_DELIVERY_TTL};

/// the id this module registers under (the native twin's id: `Env::me` and
/// follow-up routing must read identically to ported logic).
const MODULE_ID: &str = "collaboration";

use ducktape_module_sdk::WitStore;

ducktape_module_sdk::store_guest! {
    id: MODULE_ID,
    module: Collaboration,
    shape: ducktape_module_sdk::store_shape(),
    new: Collaboration::new(
        MODULE_ID,
        "identity",
        "tasks",
        Box::new(WitStore),
        HEIGHT_LANE_MAX_DELIVERY_TTL,
    ),
}
