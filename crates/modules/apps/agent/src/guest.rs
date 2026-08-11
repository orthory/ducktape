//! the wasm port of this module, built the ADAPTER way: the NATIVE `agent`
//! crate is compiled to wasm32 unmodified and adapted to the
//! `ducktape:module` world through `guest-adapter`, so the module's logic is
//! single-sourced (a behavior change in the native crate IS the wasm change).
//! the packaging cdylib around this port is synthesized by `guest-builder` —
//! this module is the whole of the guest's hand-written surface.
//!
//! ## the STORE-BACKED dispatch model, and why it is equivalent
//!
//! agent is pure logic over a host-injected [`sdk::MerkleStore`] — so the
//! port injects [`WitStore`], the adapter's `MerkleStore` over the wit
//! `state-*` imports, and the REAL qmdb store stays host-side
//! (`WasmModule::with_store`). there is NO per-dispatch snapshot: the store
//! IS the state and the wasm root is the store's Merkle root. See the
//! `pages` guest port for the staging-contract argument spelled out point by
//! point — agent rides the identical seams:
//!
//! * the guest rebuilds the module FRESH per dispatch over the exact
//!   production builder chain (`AgentModule::new` with the saga dead-letter
//!   and runs hook ids below); its inner `StagedStore` overlay is
//!   per-dispatch, and cross-dispatch read-your-writes comes from the host's
//!   outer staged overlay via `WitStore::get` (staged-over-committed) — every
//!   decision in the execute paths reads through it, so a register-then-read
//!   cascade inside one block decides byte-identically to native.
//! * each successful `execute` flushes the inner staging with the inner
//!   `commit_block` — `state-set`/`state-delete` OUTER staging the host
//!   publishes into the real store in ONE `commit_batch` at the true block
//!   boundary. the idempotent no-ops (a same-status pause/resume) stage
//!   NOTHING on either side, so the op log — and the root — stays
//!   byte-identical there too.
//! * both follow-up lanes cross the seam exactly as natively: the registry
//!   HOOK (`AgentEvent` msgs to the runs module, so an agent and its dispatch
//!   recipe stay one atomic unit) leaves through the wit `emit-msg` import,
//!   and the saga DEAD-LETTER arm (a foreign trigger's `reply_to` callback)
//!   swallows with an `emit-event` breadcrumb — never an abort.
//!
//! equivalence is pinned block-by-block (roots, replies, aborts,
//! multi-dispatch blocks) by `wasm_agent_parity`.

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

use guest_adapter::WitStore;

// store-backed port: no snapshot — the host owns the real qmdb store and the
// module is rebuilt fresh per dispatch (see `guest_adapter::store_guest!`).
guest_adapter::store_guest! {
    id: MODULE_ID,
    module: AgentModule,
    new: AgentModule::new(MODULE_ID, Box::new(WitStore), SAGA_ID, Some(HOOK_ID.into())),
}
