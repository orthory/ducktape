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
//! IS the state and the wasm root is the store's Merkle root.
//!
//! * the guest rebuilds the module FRESH per dispatch over the exact
//!   production builder chain (`AgentModule::new` with the sibling ids
//!   below); its inner `StagedStore` overlay is per-dispatch, and
//!   cross-dispatch read-your-writes comes from the host's outer staged
//!   overlay via `WitStore::get` (staged-over-committed) — every decision
//!   reads through it, so a provision-then-bind unit (identity's answer
//!   arrives as a follow-up in the same unit) decides byte-identically to
//!   native.
//! * each successful `execute` flushes the inner staging with the inner
//!   `commit_block` — `state-set`/`state-delete` OUTER staging the host
//!   publishes into the real store in ONE `commit_batch` at the true unit
//!   boundary. an input retired without effect (an ignored delivery, an
//!   orphaned completion) stages NOTHING on either side, so the op log —
//!   and the root — stays byte-identical there too.
//! * every follow-up (identity's `CreateProgram`/`SetProgramStanding`, a
//!   program's reports to attribution, its calls and dispatches to the
//!   queue plane) leaves through the wit `emit-msg` import, and every
//!   sibling read a decision makes (identity's control records,
//!   attribution's change at a resumption, a program's query steps) comes
//!   back through the wit `query` import — the same host-routed reads the
//!   native module makes through its ctx.
//! * this module keeps no outbound queue: the `pending-items` and
//!   `acknowledge` exports are the trait defaults (nothing, and a refusal).

use crate::{AgentModule, Siblings};

/// the genesis-constant id this module registers under (the native twin's id:
/// `Env::me`, every `CallId` this module queues, and identity's executor
/// record must read identically to ported logic).
const MODULE_ID: &str = "agent";
/// the sibling ids compiled into this instance — EXACTLY the production
/// wiring (`crates/topology/src/lib.rs`): the account book, the change
/// ledger, and the queue plane.
const IDENTITY_ID: &str = "identity";
const ATTRIBUTION_ID: &str = "attribution";
const DISPATCH_ID: &str = "dispatch";

use guest_adapter::WitStore;

// store-backed port: no snapshot — the host owns the real qmdb store and the
// module is rebuilt fresh per dispatch (see `guest_adapter::store_guest!`).
guest_adapter::store_guest! {
    id: MODULE_ID,
    module: AgentModule,
    shape: guest_adapter::store_shape(),
    new: AgentModule::new(
        MODULE_ID,
        Box::new(WitStore),
        Siblings {
            identity: IDENTITY_ID.into(),
            attribution: ATTRIBUTION_ID.into(),
            dispatch: DISPATCH_ID.into(),
        },
    ),
}
