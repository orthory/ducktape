//! `tagging-wasm` — the wasm port of the `tagging` module, built the ADAPTER
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

use guest_adapter::{block_on, host, load_state, save_state, Guest, WitCtx};
use sdk::{Error, Module as _, Msg, StateRoot};
use tagging::TaggingModule;

/// the genesis-constant id this module registers under (the native twin's id:
/// `Env::me` and follow-up routing must read identically to ported logic).
const MODULE_ID: &str = "tagging";

struct Component;

/// the native module at THIS dispatch's state: genesis shape when nothing was
/// ever persisted, else the persisted snapshot verify-then-adopted against its
/// persisted root. an install failure is host-store corruption surfaced as a
/// deterministic rejection, never a silent re-genesis.
///
/// the constructor is EXACTLY the production wiring in bin/node's host state:
/// `TaggingModule::new("tagging").with_direct_owner("runs")`. the direct-owner
/// set is genesis config, not committed state, so the guest must wire the same
/// set as every native host — drift here would be a consensus fork.
fn loaded_module() -> Result<TaggingModule, host::Error> {
    let mut module = TaggingModule::new(MODULE_ID).with_direct_owner("runs");
    if let Some((bytes, root)) = load_state() {
        module
            .install(&bytes, StateRoot(root))
            .map_err(|e| host::Error::Rejected(format!("tagging state reload: {e}")))?;
    }
    Ok(module)
}

/// map an inner sdk error onto the wit surface. `Module` is the native
/// rejection verbatim; anything else a native tagging never surfaces from
/// its own execute, so the debug rendering is purely diagnostic.
fn to_wit_error(e: Error) -> host::Error {
    match e {
        Error::Module(m) => host::Error::Rejected(m),
        other => host::Error::Rejected(other.to_string()),
    }
}

impl Guest for Component {
    fn execute(payload: Vec<u8>) -> Result<(), host::Error> {
        let mut module = loaded_module()?;
        let mut ctx = WitCtx::new();
        block_on(module.execute(
            &mut ctx,
            &Msg {
                target: MODULE_ID.into(),
                payload,
            },
        ))
        .map_err(to_wit_error)?;
        // fully apply per dispatch: publish the inner per-op staging, then
        // persist the canonical snapshot as OUTER staged writes — the host
        // owns the real commit/abort boundary (see the crate doc).
        block_on(module.commit_block()).map_err(to_wit_error)?;
        save_state(&module.snapshot(), module.root().as_bytes());
        Ok(())
    }

    fn query(req: Vec<u8>) -> Result<Vec<u8>, host::Error> {
        // tagging has no read surface (the native module keeps the sdk default
        // `QueryUnsupported`); loading state first keeps the corruption check
        // ahead of the refusal, matching a native host's dispatch order.
        let module = loaded_module()?;
        block_on(module.query(&req)).map_err(to_wit_error)
    }
}

guest_adapter::export_module!(Component);
