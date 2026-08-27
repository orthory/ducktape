//! the wasm port of this module, built the ADAPTER way: the NATIVE `identity`
//! crate is compiled to wasm32 unmodified and adapted to the
//! `ducktape:module` world through `guest-adapter`, so the module's logic is
//! single-sourced (a behavior change in the native crate IS the wasm change).
//! the packaging cdylib around this port is synthesized by `guest-builder` —
//! this module is the whole of the guest's hand-written surface.
//!
//! ## the STORE-BACKED dispatch model, and why it is equivalent
//!
//! identity is pure logic over a host-injected [`sdk::MerkleStore`] — so the
//! port injects [`WitStore`], the adapter's `MerkleStore` over the wit
//! `state-*` imports, and the REAL qmdb store stays host-side
//! (`WasmModule::with_store`). there is NO per-dispatch snapshot: the store
//! IS the state and the wasm root is the store's Merkle root. See the
//! `pages` guest port for the staging-contract argument spelled out point by
//! point — identity rides the identical seams: the guest rebuilds the module
//! FRESH per dispatch over the exact production builder chain, cross-dispatch
//! read-your-writes comes from the host's outer staged overlay via
//! `WitStore::get`, and each successful `execute` flushes the inner staging
//! with the inner `commit_block`. every add-key consent verifies IN the guest
//! through `keyscheme` — pure-Rust p256/k256 and commonware ed25519,
//! deterministic on wasm32. identity reads no sibling: admission is open, and
//! the ACL policy on the `identity` target is the operator's knob.
//!
//! ## the genesis-config chain id
//!
//! identity's per-network parameter is the CHAIN ID every signed certificate
//! preimage folds in. a wasm component is fixed bytes, so the id arrives as
//! GENESIS CONFIG: the host seeds an `sdk::genesis_config`-encoded `__config`
//! record into this module's qmdb store at genesis construction — under
//! [`sdk::store_key`], the store-backed twin of the host-KV `__config` entry
//! — and every dispatch reads it back through
//! [`guest_adapter::store_genesis_chain_id`] and constructs the native module
//! with it. the config is consensus state in the store's merkle root from
//! genesis, and it rides state-sync like any other record.

use crate::Identity;
use guest_adapter::WitStore;

/// the genesis-constant id this module registers under (the native twin's id:
/// `Env::me` and follow-up routing must read identically to ported logic).
const MODULE_ID: &str = "identity";

// store-backed port: no snapshot — the host owns the real qmdb store and the
// module is rebuilt fresh per dispatch (see `guest_adapter::store_guest!`).
// the per-network chain id comes from the store-seeded genesis config.
guest_adapter::store_guest! {
    id: MODULE_ID,
    module: Identity,
    new: Identity::new(
        MODULE_ID,
        Box::new(WitStore),
        guest_adapter::store_genesis_chain_id("identity")?,
    ),
}
