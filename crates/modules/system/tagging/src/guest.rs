//! the wasm port of this module, built the ADAPTER
//! way: the NATIVE `tagging` crate is compiled to wasm32 unmodified and adapted
//! to the `ducktape:module` world through `guest-adapter`, so the module's
//! logic is single-sourced (a behavior change in the native crate IS the wasm
//! change).
//!
//! ## sibling reads (the reason this tenant exists)
//!
//! tagging is a CROSS-MODULE plane: `on_subscribe` verifies the source module
//! against the registry (`ctx.module_root(source)`) and `on_tag` gates the
//! direct-owner route on `ctx.module_root(tag.module)`. inside the guest those
//! are host imports the sync world cannot await, so the runtime resolves them
//! by MEMOIZED REPLAY — the run pauses on an unanswered read, the host resolves
//! it through the real `Ctx`, and the pure guest re-runs with the answer
//! memoized. an op's ACCEPTANCE therefore depends on sibling reads resolving
//! through the wasm runtime, which is exactly what the parity proof pins.
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
//!    per-op staged scope writes into the committed subscription map — and
//! 4. saves the new canonical snapshot back as STAGED host writes.
//!
//! the inner commit does NOT publish anything durably: the OUTER staging (the
//! host-owned `state-set` overlay) is the only durable seam, published at the
//! real block boundary and discarded on abort. the native staging contract is
//! preserved point by point:
//!
//! * read-your-writes across a block's dispatches: dispatch N+1's `load_state`
//!   reads `__state` through the host overlay, so it sees dispatch N's saved
//!   snapshot — exactly like the native module's `subscribers()` reading its
//!   staged overlay (a tag in one block fans out to a subscription staged
//!   earlier in the same block).
//! * root() mid-block: the host computes the wasm root over COMMITTED store
//!   only, excluding the staged `__state` — as the native `root()` excludes
//!   the staged map. roots move at commit, never inside a block.
//! * abort: the host discards the outer overlay, so `__state` reverts to the
//!   pre-block snapshot and the next dispatch reloads pre-block state — as the
//!   native `abort_block` clears the staged map. (batch replay after a rejected
//!   member re-runs accepted ops from the reverted snapshot, same as native.)
//! * a rejected op: nothing was saved (step 4 never ran), and the runtime
//!   restores the pre-dispatch overlay — the native execute's error paths
//!   likewise leave the staged map untouched.
//!
//! the persisted encoding is the native module's canonical snapshot stored as
//! ONE host-KV value, so the wasm root is the host-KV encoding over the two
//! reserved keys — a STATE-SCHEMA BREAK versus the native root (revision 2 of
//! the tagging canonical state, declared at cutover in `MODULE_STATE_SCHEMAS`;
//! beta networks re-genesis, no back-compat shim).

use crate::TaggingModule;

/// the genesis-constant id this module registers under (the native twin's id:
/// `Env::me` and follow-up routing must read identically to ported logic).
const MODULE_ID: &str = "tagging";

// whole-state port: the shell loads/saves the canonical snapshot and runs the
// native module per dispatch (see `guest_adapter::snapshot_guest!`).
guest_adapter::snapshot_guest! {
    id: MODULE_ID,
    module: TaggingModule,
    new: TaggingModule::new(MODULE_ID).with_direct_owner("runs"),
}
