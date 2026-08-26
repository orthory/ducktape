//! the wasm port of this module, built the ADAPTER
//! way: the NATIVE `kv` crate is compiled to wasm32 unmodified and adapted
//! to the `ducktape:module` world through `guest-adapter`, so the module's
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

use guest_adapter::WitStore;

// store-backed port: no snapshot — the host owns the real qmdb store and the
// module is rebuilt fresh per dispatch (see `guest_adapter::store_guest!`).
// no genesis config: kv carries no per-network parameter.
guest_adapter::store_guest! {
    id: MODULE_ID,
    module: Kv,
    new: Kv::new(MODULE_ID, Box::new(WitStore)),
}
