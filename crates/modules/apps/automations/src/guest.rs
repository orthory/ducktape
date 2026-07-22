//! the wasm port of this module, built the
//! ADAPTER way: the NATIVE `automations` crate is compiled to wasm32
//! unmodified and adapted to the `ducktape:module` world through
//! `guest-adapter`, so the module's logic is single-sourced (a behavior
//! change in the native crate IS the wasm change).
//!
//! ## the whole-state dispatch model, and why it is equivalent
//!
//! the guest is re-instantiated per dispatch, so the native module inside it
//! must FULLY apply per dispatch. each `execute`:
//!
//! 1. loads the persisted snapshot through the host's staged-overlay reads
//!    (`__state`/`__root`, verify-then-adopt via the native `install`),
//! 2. runs the native `execute` over a [`WitCtx`] — INCLUDING the chat-hook
//!    intake's pre-emit PROBES (channel-exists, message-id-unused,
//!    task-id-unused), which are host-routed `query-module` reads the runtime
//!    resolves through memoized replay against the SIBLINGS' live
//!    staged-over-committed state — exactly the view the native probes read,
//! 3. on success calls the INNER module's `commit_block`, and
//! 4. saves the new canonical snapshot back as STAGED host writes.
//!
//! the fold is safe for automations SPECIFICALLY because every decision in
//! its execute paths reads staged-over-committed (`rule` /`effective_rules`
//! shadow the committed map with `pending_rules`; the history ring appends
//! through `pending_history`), so a reloaded module that sees earlier
//! same-block writes as committed decides byte-identically. the run-history
//! ring cap is fold-invariant too: trimming to the newest
//! [`automations::MAX_RUN_HISTORY`] records once per block (native) and once
//! per dispatch (this guest) keeps the same suffix. the NO-FAIL hook-arm
//! contract crosses the seam unchanged:
//! an undecodable event, a failed text fetch, or a probe rejection stages a
//! `RunRecord` and returns Ok — never a trap — so a user's posting block can
//! never be aborted by this module on either runtime (pinned by
//! `wasm_automations_parity.rs`).
//!
//! the persisted encoding is the native module's canonical snapshot stored as
//! ONE host-KV value, so the wasm root is the host-KV encoding over the two
//! reserved keys — a STATE-SCHEMA BREAK versus the native root (revision 2 in
//! `MODULE_STATE_SCHEMAS`; beta networks re-genesis, no back-compat shim).

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

// whole-state port: the shell loads/saves the canonical snapshot and runs the
// native module per dispatch (see `guest_adapter::snapshot_guest!`).
guest_adapter::snapshot_guest! {
    id: MODULE_ID,
    module: Automations,
    new: Automations::new(MODULE_ID, CHAT_ID, TASKS_ID, INBOX_ID),
}
