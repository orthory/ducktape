//! the wasm port of this module, built the ADAPTER way:
//! the NATIVE `inbox` crate is compiled to wasm32 unmodified and adapted to
//! the `ducktape:module` world through `guest-adapter`, so the module's logic
//! is single-sourced (a behavior change in the native crate IS the wasm change).
//!
//! ## the whole-state dispatch model, and why it is equivalent
//!
//! the guest is re-instantiated per dispatch, so the native module inside it
//! must FULLY apply per dispatch. each `execute`:
//!
//! 1. loads the persisted snapshot through the host's staged-overlay reads
//!    (`__state`/`__root`, verify-then-adopt via the native `install`),
//! 2. runs the native `execute` over a [`WitCtx`],
//! 3. on success calls the INNER module's `commit_block` — publishing its
//!    per-member `pending` overlay into its committed map — and
//! 4. saves the new canonical snapshot back as STAGED host writes.
//!
//! the inner commit does NOT publish anything durably: the OUTER staging (the
//! host-owned `state-set` overlay) is the only durable seam, published at the
//! real block boundary and discarded on abort. the native staging contract is
//! preserved point by point:
//!
//! * read-your-writes across a block's dispatches: dispatch N+1's `load_state`
//!   reads `__state` through the host overlay, so it sees dispatch N's saved
//!   snapshot — exactly like the native module's reads through its `pending`
//!   overlay. mid-block queries (ctx-less and host-routed alike) run over the
//!   same overlay (`wasm-host::query_round`), so they serve committed + this
//!   block's staged writes — byte-for-byte what the native `query` serves from
//!   committed + `pending`.
//! * root() mid-block: the host computes the wasm root over COMMITTED store
//!   only, excluding the staged `__state` — as the native `root()` excludes
//!   `pending`. roots move at commit, never inside a block.
//! * abort: the host discards the outer overlay, so `__state` reverts to the
//!   pre-block snapshot and the next dispatch reloads pre-block state — as the
//!   native `abort_block` clears `pending`. (batch replay after a rejected
//!   member re-runs accepted ops from the reverted snapshot, same as native.)
//! * a rejected op: nothing was saved (step 4 never ran), and the runtime
//!   restores the pre-dispatch overlay — the native execute's error paths
//!   likewise leave `pending` untouched.
//!
//! the canonical snapshot is stored as one host-KV value under the adapter's
//! reserved keys.

use crate::Inbox;

/// the genesis-constant id this module registers under (the native twin's id:
/// `Env::me` and follow-up routing must read identically to ported logic).
const MODULE_ID: &str = "inbox";

// whole-state port: the shell loads/saves the canonical snapshot and runs the
use guest_adapter::WitStore;

// store-backed port: no snapshot — the host owns the real qmdb store and the
// module is rebuilt fresh per dispatch (see `guest_adapter::store_guest!`).
guest_adapter::store_guest! {
    id: MODULE_ID,
    module: Inbox,
    new: Inbox::new(MODULE_ID, Box::new(WitStore)),
}
