//! the wasm port of the merged `gateway` module, built the ADAPTER way: the
//! NATIVE `gateway` crate is compiled to wasm32 unmodified and adapted to the
//! `ducktape:module` world through `ducktape-module-sdk`, so the module's logic is
//! single-sourced (a behavior change in the native crate IS the wasm change).
//!
//! `gateway` owns the WHOLE `.duck` name → AccountId → route pipeline: the
//! route plane and the `.duck` handle plane, both as records in ONE
//! host-owned qmdb store.
//!
//! ## the STORE-BACKED dispatch model
//!
//! gateway is pure logic over a host-injected [`sdk::MerkleStore`] — so the
//! port injects [`WitStore`], the adapter's `MerkleStore` over the wit
//! `state-*` imports, and the REAL qmdb store stays host-side
//! (`WasmModule::with_store`). there is NO per-dispatch snapshot: the store
//! IS the state and the wasm root is the store's Merkle root. see the `pages`
//! guest port for the staging-contract argument; gateway rides the identical
//! seams, and every execute's SIBLING read — the identity `OfKey` account
//! derivation plus current-member check — resolves through the runtime's
//! memoized replay.
//!
//! ## the genesis-config chain id
//!
//! gateway's per-network parameter is the CHAIN ID every route statement is
//! scoped to. a wasm component is fixed bytes, so the id arrives as GENESIS
//! CONFIG: the host seeds an `sdk::genesis_config`-encoded `__config` record
//! into this module's qmdb store at genesis construction — under
//! [`sdk::store_key`] — and every dispatch reads it back through
//! [`ducktape_module_sdk::store_genesis_chain_id`] and constructs the native module
//! with it. the config is consensus state in the store's merkle root from
//! genesis, and it rides state-sync like any other record.

use crate::Gateway;
use ducktape_module_sdk::WitStore;

/// the genesis-constant id this module registers under (the native twin's id:
/// `Env::me` and follow-up routing must read identically to ported logic).
const MODULE_ID: &str = "gateway";
/// the sibling id this instance reads through host-routed queries — EXACTLY
/// the production wiring (`bin/node/src/host_state.rs`): the identity module
/// that resolves the origin key to its account.
const IDENTITY_ID: &str = "identity";

// store-backed port: no snapshot — the host owns the real qmdb store and the
// module is rebuilt fresh per dispatch (see `ducktape_module_sdk::store_guest!`).
// the per-network chain id comes from the store-seeded genesis config.
ducktape_module_sdk::store_guest! {
    id: MODULE_ID,
    module: Gateway,
    shape: ducktape_module_sdk::host::ModuleShape {
        config: vec![sdk::genesis_config::CHAIN_ID.into()],
        ..ducktape_module_sdk::store_shape()
    },
    new: Gateway::new(
        MODULE_ID,
        Box::new(WitStore),
        IDENTITY_ID,
        ducktape_module_sdk::store_genesis_chain_id("gateway")?,
    ),
}
