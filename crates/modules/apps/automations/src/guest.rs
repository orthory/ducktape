//! the wasm port of this module, built the ADAPTER way: the NATIVE
//! `automations` crate is compiled to wasm32 unmodified and adapted to the
//! `ducktape:module` world through `guest-adapter`, so the module's logic is
//! single-sourced (a behavior change in the native crate IS the wasm change).
//! the packaging cdylib around this port is synthesized by `guest-builder` —
//! this module is the whole of the guest's hand-written surface.
//!
//! ## the STORE-BACKED dispatch model, and why it is equivalent
//!
//! automations is pure logic over a host-injected [`sdk::MerkleStore`] — so
//! the port injects [`WitStore`], the adapter's `MerkleStore` over the wit
//! `state-*` imports, and the REAL qmdb store stays host-side
//! (`WasmModule::with_store`). there is NO per-dispatch snapshot: the store
//! IS the state and the wasm root is the store's Merkle root. See the
//! `pages` guest port for the staging-contract argument spelled out point by
//! point — automations rides the identical seams:
//!
//! * the guest rebuilds the module FRESH per dispatch over the exact
//!   production builder chain (`Automations::new` with the chat/tasks/inbox
//!   lane ids below); its inner `StagedStore` overlay is per-dispatch, and
//!   cross-dispatch read-your-writes comes from the host's outer staged
//!   overlay via `WitStore::get` (staged-over-committed) — every decision in
//!   the execute paths reads through it (the roster, the rule records, the
//!   run-history cursor), so a create-then-fire cascade inside one block
//!   decides byte-identically to native.
//! * each successful `execute` flushes the inner staging with the inner
//!   `commit_block` — `state-set`/`state-delete` OUTER staging the host
//!   publishes into the real store in ONE `commit_batch` at the true block
//!   boundary. the idempotent no-ops (a same-state SetEnabled) stage NOTHING
//!   on either side, so the op log — and the root — stays byte-identical
//!   there too.
//! * the chat-hook intake crosses the seam unchanged — INCLUDING the
//!   pre-emit PROBES (channel-exists, message-id-unused, task-id-unused),
//!   which are host-routed `query-module` reads the runtime resolves through
//!   memoized replay against the SIBLINGS' live staged-over-committed state —
//!   and the NO-FAIL contract holds: an undecodable event, a failed text
//!   fetch, or a probe rejection stages a `RunRecord` and returns Ok — never
//!   a trap — so a user's posting block can never be aborted by this module
//!   on either runtime (pinned by `wasm_automations_parity.rs`).

use crate::Automations;

/// the genesis-constant id this module registers under (the native twin's id:
/// `Env::me` and follow-up routing must read identically to ported logic).
const MODULE_ID: &str = "automations";
/// the sibling ids compiled into this instance — EXACTLY the production
/// wiring (`bin/node/src/host_state.rs`): chat is both the trusted hook
/// origin and the PostMessage target, tasks the CreateTask target, inbox the
/// DeliverInbox target.
const CHAT_ID: &str = "chat";
const TASKS_ID: &str = "tasks";
const INBOX_ID: &str = "inbox";

use guest_adapter::WitStore;

// store-backed port: no snapshot — the host owns the real qmdb store and the
// module is rebuilt fresh per dispatch (see `guest_adapter::store_guest!`).
guest_adapter::store_guest! {
    id: MODULE_ID,
    module: Automations,
    shape: guest_adapter::store_shape(),
    new: Automations::new(MODULE_ID, Box::new(WitStore), CHAT_ID, TASKS_ID, INBOX_ID),
}
