//! `pages-wasm` — the wasm port of the `pages` module, built the ADAPTER way:
//! the NATIVE `pages` crate is compiled to wasm32 unmodified and adapted to
//! the `ducktape:module` world through `guest-adapter`, so the module's logic
//! is single-sourced (a behavior change in the native crate IS the wasm
//! change).
//!
//! ## the STORE-BACKED dispatch model, and why it is equivalent
//!
//! unlike the whole-state (`load_state`/`save_state`) tenants, pages is pure
//! logic over a host-injected [`sdk::MerkleStore`] — so the port injects
//! [`WitStore`], the adapter's `MerkleStore` over the wit `state-*` imports,
//! and the REAL qmdb store stays host-side (`WasmModule::with_store`). there
//! is NO per-dispatch snapshot: the store IS the state, the wasm root IS the
//! store's merkle root, and the cutover is ROOT-CONTINUOUS (state schema
//! revision stays 1; pre-cutover workspaces reopen unchanged).
//!
//! the guest is re-instantiated per dispatch, so the native module inside it
//! must fully FLUSH per dispatch. each `execute`:
//!
//! 1. constructs the native module FRESH over `WitStore` — the exact
//!    production builder chain (`Pages::new("pages", store).with_tagging
//!    ("tagging")`), so `Env::me`, follow-up routing, and the tag-report edge
//!    read identically to ported logic,
//! 2. runs the native `execute` over a [`WitCtx`] — its own-store reads go
//!    `pending` → `WitStore::get` → host `state-get` (staged-over-committed;
//!    a committed miss is resolved against the real store by memoized
//!    replay), and
//! 3. on success calls the INNER module's `commit_block`, whose
//!    `store.commit_batch(writes)` lands as `state-set`/`state-delete` OUTER
//!    staging — the host publishes the accumulated block batch into the real
//!    store in ONE `commit_batch` at the true block boundary, or discards it
//!    on abort.
//!
//! the native staging contract is preserved point by point:
//!
//! * read-your-writes across a block's dispatches: the native module keeps
//!   ONE `pending` overlay alive across the whole block, so dispatch N+1's
//!   `store.get` fallthrough happens only for keys `pending` misses. here the
//!   inner `pending` dies with each dispatch, but its flushed writes sit in
//!   the host's outer staged overlay, which `state-get` consults FIRST — the
//!   union view (this dispatch's pending, over earlier dispatches' staging,
//!   over committed) is byte-identical to the native overlay-over-committed
//!   read. mid-block queries (ctx-less and host-routed alike) run over the
//!   same overlay (`wasm-host::query_round`), exactly like the native `query`
//!   served from committed + `pending`.
//! * root() mid-block: the host answers `root()` from the real store, which
//!   only moves at `commit_block` — as the native `root()` excludes `pending`.
//! * commit: the host drains the outer overlay into ONE `commit_batch`. the
//!   native module batched by logical key, this path batches by hashed key,
//!   and qmdb's batch canonicalizes mutations by key before merkleizing — so
//!   the committed op log, and therefore the root, is IDENTICAL (pinned by
//!   `wasm_pages_parity`).
//! * abort: the host discards the outer overlay; nothing reached the store —
//!   as the native `abort_block` clears `pending`. (batch replay after a
//!   rejected member re-runs accepted ops over the untouched store, same as
//!   native.)
//! * a rejected op: step 3 never ran, so nothing was flushed, and the runtime
//!   restores the pre-dispatch overlay — the native execute's error paths
//!   leave partial staging behind only until the host aborts the block, which
//!   both sides answer identically.

use guest_adapter::{Guest, WitCtx, WitStore, block_on, host};
use pages::Pages;
use sdk::{Error, Module as _, Msg};

/// the genesis-constant id this module registers under (the native twin's id:
/// `Env::me` and follow-up routing must read identically to ported logic).
const MODULE_ID: &str = "pages";
/// the engagement plane every newly-added comment is reported to — EXACTLY
/// the production wiring the host builder chain used pre-cutover
/// (`bin/node/src/host_state.rs`): `.with_tagging("tagging")`. the wiring is
/// genesis config compiled into the guest; drift here would be a consensus
/// fork.
const TAGGING_ID: &str = "tagging";

struct Component;

/// the native module over the host's real store, rebuilt fresh per dispatch.
/// no state load: the store IS the state, and the module's own `pending`
/// overlay is per-dispatch by design (cross-dispatch read-your-writes comes
/// from the host's outer staged overlay via `WitStore::get`).
fn module() -> Pages {
    Pages::new(MODULE_ID, Box::new(WitStore)).with_tagging(TAGGING_ID)
}

/// map an inner sdk error onto the wit surface. `Module` is the native
/// rejection verbatim; anything else a native pages never surfaces from
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
        // merge. pages queries are pure own-store reads (no sibling access),
        // so the ctx-less native `query` is the whole surface.
        let module = module();
        block_on(module.query(&req)).map_err(to_wit_error)
    }
}

guest_adapter::export_module!(Component);
