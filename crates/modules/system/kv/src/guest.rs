//! the wasm port of this module, built the ADAPTER
//! way: the NATIVE `kv` crate is compiled to wasm32 unmodified and adapted
//! to the `ducktape:module` world through `ducktape-module-sdk`, so the module's
//! logic is single-sourced (a behavior change in the native crate IS the wasm
//! change).
//!
//! kv is the minimal store-backed tenant: one op (`Set`), one query (`Get`),
//! no env reads, no sibling reads — pure logic over the host-owned store, so
//! the port is the `store_guest!` shell verbatim. the write-time size caps
//! reject inside the guest exactly as they do natively, keeping the
//! poison-pill value out of the log on both runtimes identically.

use crate::Kv;

/// the genesis-constant id this module registers under (the native twin's id:
/// `Env::me` and follow-up routing must read identically to ported logic).
const MODULE_ID: &str = "kv";

use ducktape_module_sdk::WitStore;

// store-backed port: no snapshot — the host owns the real qmdb store and the
// module is rebuilt fresh per dispatch (see `ducktape_module_sdk::store_guest!`).
// no genesis config: kv carries no per-network parameter.
ducktape_module_sdk::store_guest! {
    id: MODULE_ID,
    module: Kv,
    shape: ducktape_module_sdk::store_shape(),
    new: Kv::new(MODULE_ID, Box::new(WitStore)),
}
