//! `chat-wasm` — the wasm port of the `chat` module, built the ADAPTER way:
//! the NATIVE `chat` crate is compiled to wasm32 unmodified (minus the
//! `native`-feature off-consensus submodules — the derived index and the
//! voice/video media engines, which never touch the app-hash) and adapted to
//! the `ducktape:module` world through `guest-adapter`, so the module's logic
//! is single-sourced (a behavior change in the native crate IS the wasm
//! change).
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

use chat::Chat;
use guest_adapter::{Guest, WitCtx, WitStore, block_on, host};
use sdk::{Error, Module as _, Msg};

/// the genesis-constant id this module registers under (the native twin's id:
/// `Env::me` and follow-up routing must read identically to ported logic).
const MODULE_ID: &str = "chat";
/// the engagement plane every post is reported to — EXACTLY the production
/// wiring the host builder chain used pre-cutover
/// (`bin/node/src/host_state.rs`): `.with_tagging("tagging")`. the wiring is
/// genesis config compiled into the guest; drift here would be a consensus
/// fork.
const TAGGING_ID: &str = "tagging";

struct Component;

/// the native module over the host's real store, rebuilt fresh per dispatch.
/// no state load: the store IS the state, and the module's own `pending`
/// overlay is per-dispatch by design (cross-dispatch read-your-writes comes
/// from the host's outer staged overlay via `WitStore::get`).
fn module() -> Chat {
    Chat::new(MODULE_ID, Box::new(WitStore)).with_tagging(TAGGING_ID)
}

/// map an inner sdk error onto the wit surface. `Module` is the native
/// rejection verbatim; anything else a native chat never surfaces from
/// its own execute, so the debug rendering is purely diagnostic.
fn to_wit_error(e: Error) -> host::Error {
    match e {
        Error::Module(m) => host::Error::Rejected(m),
        other => host::Error::Rejected(other.to_string()),
    }
}

impl Guest for Component {
    fn execute(payload: Vec<u8>) -> Result<(), host::Error> {
        let mut module = module();
        let mut ctx = WitCtx::new();
        block_on(module.execute(
            &mut ctx,
            &Msg {
                target: MODULE_ID.into(),
                payload,
            },
        ))
        .map_err(to_wit_error)?;
        // flush the inner per-dispatch staging into the host's OUTER overlay
        // (WitStore::commit_batch = state-set/state-delete per record). the
        // host owns the real store commit/abort boundary (see the crate doc).
        block_on(module.commit_block()).map_err(to_wit_error)?;
        Ok(())
    }

    fn query(req: Vec<u8>) -> Result<Vec<u8>, host::Error> {
        // a fresh module's `pending` is empty, so the native query reads
        // straight through `WitStore::get` — the staged-over-committed view
        // this round serves, byte-identical to the native committed+pending
        // merge. chat queries are pure own-store reads (no sibling access),
        // so the ctx-less native `query` is the whole surface.
        let module = module();
        block_on(module.query(&req)).map_err(to_wit_error)
    }
}

guest_adapter::export_module!(Component);
