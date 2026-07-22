//! the wasm port of this module, built the ADAPTER
//! way: the NATIVE `dispatch` crate is compiled to wasm32 unmodified and adapted
//! to the `ducktape:module` world through `guest-adapter`, so the module's logic
//! is single-sourced (a behavior change in the native crate IS the wasm change).
//!
//! dispatch is a SELF-CONTAINED consensus plane: its `execute` reads only
//! `ctx.env()` (the caller origin and consensus time) and EMITS follow-ups
//! (`emit_msg` to saga, `emit_event` breadcrumbs) through the wit imports. it
//! makes no cross-module `query-module` reads inside execute, so — unlike the
//! `tagging`/`runs` tenants — the fold below needs no memoized sibling replay on
//! the accept path. the one ctx-routed sibling read the native module has,
//! `saga_view`, lives in `query_with` (the single-dispatch assignee enrichment),
//! which the ctx-less guest query surface does not exercise; the host's
//! committed-only `PendingDeliveries` injection reads the plain `query` path,
//! which this guest serves faithfully.
//!
//! ## the whole-state dispatch model, and why it is equivalent
//!
//! the guest is re-instantiated per dispatch, so the native module inside it
//! must FULLY apply per dispatch. each `execute`:
//!
//! 1. loads the persisted snapshot through the host's staged-overlay reads
//!    (`__state`/`__root`, verify-then-adopt via the native `install`),
//! 2. runs the native `execute` over a [`WitCtx`],
//! 3. on success calls the INNER module's `commit_block` — publishing its three
//!    staged overlays (recipes, dispatches, the mailbox queue) into the
//!    committed maps — and
//! 4. saves the new canonical snapshot back as STAGED host writes.
//!
//! the inner commit does NOT publish anything durably: the OUTER staging (the
//! host-owned `state-set` overlay) is the only durable seam, published at the
//! real block boundary and discarded on abort. the native staging contract is
//! preserved point by point:
//!
//! * read-your-writes across a block's dispatches: dispatch N+1's `load_state`
//!   reads `__state` through the host overlay, so it sees dispatch N's saved
//!   snapshot — exactly like the native module's staged-over-committed accessors
//!   (`recipe()` / `dispatch()` shadow the committed maps with this block's
//!   overlays), so a reloaded module that sees an earlier same-block write as
//!   committed decides byte-identically.
//! * root() mid-block: the host computes the wasm root over COMMITTED store
//!   only, excluding the staged `__state` — as the native `root()` hashes only
//!   `self.committed`, never the staged overlays. roots move at commit, never
//!   inside a block.
//! * abort: the host discards the outer overlay, so `__state` reverts to the
//!   pre-block snapshot and the next dispatch reloads pre-block state — as the
//!   native `abort_block` clears the three staged overlays. (batch replay after
//!   a rejected member re-runs accepted ops from the reverted snapshot, same as
//!   native.)
//! * a rejected op: nothing was saved (step 4 never ran), and the runtime
//!   restores the pre-dispatch overlay — the native execute's error paths
//!   likewise leave the staged overlays untouched.
//!
//! the persisted encoding is the native module's canonical snapshot stored as
//! ONE host-KV value, so the wasm root is the host-KV encoding over the two
//! reserved keys — a STATE-SCHEMA BREAK versus the native root (declared at
//! cutover in `MODULE_STATE_SCHEMAS`; beta networks re-genesis, no back-compat
//! shim).

use crate::DispatchModule;

/// the genesis-constant id this module registers under (the native twin's id:
/// `Env::me` and follow-up routing must read identically to ported logic).
const MODULE_ID: &str = "dispatch";

// whole-state port: the shell loads/saves the canonical snapshot and runs the
// native module per dispatch (see `guest_adapter::snapshot_guest!`).
guest_adapter::snapshot_guest! {
    id: MODULE_ID,
    module: DispatchModule,
    new: DispatchModule::new(MODULE_ID, "saga"),
}
