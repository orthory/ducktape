//! the wasm port of this module, built the ADAPTER way: the native crate is
//! compiled to wasm32 unmodified (minus the `native`-feature off-consensus
//! submodules — the derived index and the voice/video media engines, which
//! never touch the app-hash) and adapted to the `ducktape:module` world
//! through `guest-adapter`, so the module's logic is single-sourced (a
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
//! IS the state, the wasm root IS the store's merkle root, and the cutover is
//! ROOT-CONTINUOUS (state schema revision stays 1; pre-cutover workspaces
//! reopen unchanged). see `pages-wasm` for the staging-contract argument
//! spelled out point by point — chat rides the identical seams:
//!
//! * the guest rebuilds the module FRESH per dispatch over the exact
//!   production builder chain (`Chat::new("chat", store).with_tagging
//!   ("tagging")`); its inner `pending` overlay is per-dispatch, and
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
//!   replay; hook fan-out and the tagging report ride `emit-msg` follow-ups
//!   exactly like native.
//!
//! equivalence is pinned block-by-block (roots, replies, aborts,
//! multi-dispatch blocks) by `wasm_chat_parity`.

use crate::Chat;

/// the genesis-constant id this module registers under (the native twin's id:
/// `Env::me` and follow-up routing must read identically to ported logic).
const MODULE_ID: &str = "chat";
/// the engagement plane every post is reported to — EXACTLY the production
/// wiring the host builder chain used pre-cutover
/// (`bin/node/src/host_state.rs`): `.with_tagging("tagging")`. the wiring is
/// genesis config compiled into the guest; drift here would be a consensus
/// fork.
const TAGGING_ID: &str = "tagging";

use guest_adapter::WitStore;

// store-backed port: no snapshot — the host owns the real qmdb store and the
// module is rebuilt fresh per dispatch (see `guest_adapter::store_guest!`).
guest_adapter::store_guest! {
    id: MODULE_ID,
    module: Chat,
    new: Chat::new(MODULE_ID, Box::new(WitStore)).with_tagging(TAGGING_ID),
}
