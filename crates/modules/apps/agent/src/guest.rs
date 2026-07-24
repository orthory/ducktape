//! the wasm port of this module, built the ADAPTER way:
//! the NATIVE `agent` crate is compiled to wasm32 unmodified and adapted to
//! the `ducktape:module` world through `guest-adapter`, so the module's logic
//! is single-sourced (a behavior change in the native crate IS the wasm
//! change).
//!
//! ## the whole-state dispatch model, and why it is equivalent
//!
//! the guest is re-instantiated per dispatch, so the native module inside it
//! must FULLY apply per dispatch. each `execute`:
//!
//! 1. loads the persisted snapshot through the host's staged-overlay reads
//!    (`__state`/`__root`, verify-then-adopt via the native `install`),
//! 2. runs the native `execute` over a [`WitCtx`] — the registry is
//!    self-contained (no sibling queries), but its two follow-up lanes cross
//!    the seam: the registry HOOK (`AgentEvent` msgs to the runs module, so
//!    an agent and its dispatch recipe stay one atomic unit) leaves through
//!    the wit `emit-msg` import, and the saga DEAD-LETTER arm (a foreign
//!    trigger's `reply_to` callback) swallows with an `emit-event` breadcrumb
//!    exactly as natively — never an abort,
//! 3. on success calls the INNER module's `commit_block`, and
//! 4. saves the new canonical snapshot back as STAGED host writes.
//!
//! the fold is safe for agent SPECIFICALLY because every decision in its
//! execute paths reads staged-over-committed (`AgentModule::agent` shadows
//! the committed map with `pending_agents`, `visible_ids` unions both), so a
//! reloaded module that sees earlier same-block writes as committed decides
//! byte-identically — no frozen-committed read anywhere (pinned by
//! `wasm_agent_parity.rs`).
//!
//! the canonical snapshot is stored as one host-KV value under the adapter's
//! reserved keys.

use crate::AgentModule;

/// the genesis-constant id this module registers under (the native twin's id:
/// `Env::me` and follow-up routing must read identically to ported logic).
const MODULE_ID: &str = "agent";
/// the sibling ids compiled into this instance — EXACTLY the production
/// wiring (`bin/node/src/host_state.rs`): saga is the dead-letter origin
/// router, runs the registry hook that keeps each agent's dispatch recipe in
/// lockstep.
const SAGA_ID: &str = "saga";
const HOOK_ID: &str = "runs";

// whole-state port: the shell loads/saves the canonical snapshot and runs the
// native module per dispatch (see `guest_adapter::snapshot_guest!`).
guest_adapter::snapshot_guest! {
    id: MODULE_ID,
    module: AgentModule,
    new: AgentModule::new(MODULE_ID, SAGA_ID, Some(HOOK_ID.into())),
}
