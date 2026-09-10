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
//! ## the delivery-deadline ceiling, on EITHER lane
//!
//! A component's bytes are fixed and a deadline is a duration, so the ceiling
//! cannot be compiled in: `consensus_time` is the block height on the
//! validator and replica lanes and a millisecond epoch clock on the sim lane,
//! and one number cannot mean seven days on both. The network states which it
//! is as the `time_unit` genesis parameter — seeded into this module's own
//! store at genesis construction and read back each dispatch — and
//! [`crate::max_delivery_ttl`] scales [`crate::MAX_DELIVERY_TTL_SECONDS`] into
//! that lane's units. A missing or malformed value REJECTS: guessing a scale
//! would silently mean the wrong duration, which is the one failure a
//! deadline must not have.
//!
//! ## the network binding
//!
//! The other genesis parameter is the `chain_id` every op names. A submitted
//! frame's signature covers no chain id, so the binding is the payload's and
//! this is where the network's value reaches a component whose bytes are the
//! same everywhere.

use crate::{Collaboration, max_delivery_ttl};

/// the id this module registers under (the native twin's id: `Env::me` and
/// follow-up routing must read identically to ported logic).
const MODULE_ID: &str = "collaboration";

use ducktape_module_sdk::{WitStore, store_genesis_chain_id, store_genesis_time_unit};

ducktape_module_sdk::store_guest! {
    id: MODULE_ID,
    module: Collaboration,
    // the config keys must be strictly increasing — `encode_config` asserts it.
    shape: ducktape_module_sdk::host::ModuleShape {
        config: vec![
            sdk::genesis_config::CHAIN_ID.into(),
            sdk::genesis_config::TIME_UNIT.into(),
        ],
        ..ducktape_module_sdk::store_shape()
    },
    new: Collaboration::new(
        MODULE_ID,
        "identity",
        "tasks",
        Box::new(WitStore),
        max_delivery_ttl(store_genesis_time_unit(MODULE_ID)?),
        store_genesis_chain_id(MODULE_ID)?,
    ),
}
