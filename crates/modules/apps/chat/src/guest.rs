//! the wasm port of this module, built the ADAPTER way: the native crate is
//! compiled to wasm32 unmodified (minus the `native`-feature off-consensus
//! submodules — the derived index and the voice/video media engines, which
//! never touch the root-hash) and adapted to the `ducktape:module` world
//! through `ducktape-module-sdk`, so the module's logic is single-sourced (a
//! behavior change in the native crate IS the wasm change). the packaging
//! cdylib around this port is synthesized by `guest-builder` — this module is
//! the whole of the guest's hand-written surface.
//!
//! ## the STORE-BACKED dispatch model, and why it is equivalent
//!
//! chat is pure logic over a host-injected [`sdk::MerkleStore`] — so the port
//! injects [`WitStore`], the adapter's `MerkleStore` over the wit `state-*`
//! imports, and the REAL qmdb store stays host-side
//! (`WasmModule::with_store`). there is NO per-dispatch snapshot: the store
//! IS the state and the wasm root is the store's Merkle root. See the `pages`
//! guest port for the staging-contract argument
//! spelled out point by point — chat rides the identical seams:
//!
//! * the guest rebuilds the module FRESH per dispatch over the exact
//!   production builder chain (`Chat::new("chat", store).with_attribution
//!   ("attribution").with_identity("identity")`); its inner `pending`
//!   overlay is per-dispatch, and
//!   cross-dispatch read-your-writes comes from the host's outer staged
//!   overlay via `WitStore::get` (staged-over-committed).
//! * each successful `execute` flushes the inner staging with the inner
//!   `commit_block` — `state-set`/`state-delete` OUTER staging the host
//!   publishes into the real store in ONE `commit_batch` at the true block
//!   boundary (qmdb's batch canonicalizes mutations by key, so the hashed-key
//!   drain order commits the same op log the native logical-key order did).
//!   note the idempotent no-ops (duplicate reaction add, exact-remove miss)
//!   stage NOTHING on either side, so the op log — and the root — stays
//!   byte-identical there too.
//! * `RegisterHook`'s registry check (`ctx.module_root`) is a host-routed
//!   SIBLING read inside the guest, resolved by the runtime's memoized
//!   replay, and so is every identity resolution (`OfKey`/`Get`) a write
//!   makes; hook fan-out and the attribution reports ride `emit-msg`
//!   follow-ups exactly like native.
//!
//! equivalence is pinned block-by-block (roots, replies, aborts,
//! multi-dispatch blocks) by `wasm_chat_parity`.

use crate::Chat;

/// the genesis-constant id this module registers under (the native twin's id:
/// `Env::me` and follow-up routing must read identically to ported logic).
const MODULE_ID: &str = "chat";
/// the attribution plane every channel and message revision is reported to.
/// the wiring is genesis config compiled into the guest; drift here would be
/// a consensus fork.
const ATTRIBUTION_ID: &str = "attribution";
/// the sibling every external key resolves through (`OfKey`) and every named
/// account is validated against (`Get`). genesis config compiled into the
/// guest, like `ATTRIBUTION_ID`.
const IDENTITY_ID: &str = "identity";

use ducktape_module_sdk::WitStore;

// store-backed port: no snapshot — the host owns the real qmdb store and the
// module is rebuilt fresh per dispatch (see `ducktape_module_sdk::store_guest!`).
ducktape_module_sdk::store_guest! {
    id: MODULE_ID,
    module: Chat,
    shape: ducktape_module_sdk::store_shape(),
    new: Chat::new(MODULE_ID, Box::new(WitStore))
        .with_attribution(ATTRIBUTION_ID)
        .with_identity(IDENTITY_ID),
}
