//! `tasks-wasm` — the wasm port of the `tasks` module, built the ADAPTER way:
//! the NATIVE `tasks` crate is compiled to wasm32 unmodified and adapted to
//! the `ducktape:module` world through `guest-adapter`, so the module's logic
//! is single-sourced (a behavior change in the native crate IS the wasm change).
//!
//! the guest takes `tasks` with `default-features = false`: the `native`
//! feature carries only the node-local derived index (whose `indexer` dep is
//! unix-only IO), never consensus state — so the ported state machine is the
//! FULL consensus surface.
//!
//! the whole-state dispatch model and its equivalence argument are spelled out
//! in `vaults-wasm` (the first adapter tenant) and `guest-adapter`; tasks is
//! the same shape: a pure `SnapshotBytes` module whose canonical snapshot is
//! persisted as ONE host-KV value per dispatch. that host-KV encoding is a
//! STATE-SCHEMA BREAK versus the native root (revision 3 — the tasks+jobs merge
//! folded the former `jobs` module's job board into this one, so the canonical
//! snapshot is now the task board and job board concatenated; beta networks
//! re-genesis, no back-compat shim).

use guest_adapter::{block_on, host, load_state, save_state, Guest, WitCtx};
use sdk::{Error, Module as _, Msg, StateRoot};
use tasks::Tasks;

/// the genesis-constant id this module registers under (the native twin's id:
/// `Env::me` and follow-up routing must read identically to ported logic).
const MODULE_ID: &str = "tasks";

struct Component;

/// the native module at THIS dispatch's state: genesis shape when nothing was
/// ever persisted, else the persisted snapshot verify-then-adopted against its
/// persisted root. an install failure is host-store corruption surfaced as a
/// deterministic rejection, never a silent re-genesis.
fn loaded_module() -> Result<Tasks, host::Error> {
    let mut module = Tasks::new(MODULE_ID);
    if let Some((bytes, root)) = load_state() {
        module
            .install(&bytes, StateRoot(root))
            .map_err(|e| host::Error::Rejected(format!("tasks state reload: {e}")))?;
    }
    Ok(module)
}

/// map an inner sdk error onto the wit surface. `Module` is the native
/// rejection verbatim; anything else a native tasks never surfaces from
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
        // owns the real commit/abort boundary (see the vaults-wasm crate doc).
        block_on(module.commit_block()).map_err(to_wit_error)?;
        save_state(&module.snapshot(), module.root().as_bytes());
        Ok(())
    }

    fn query(req: Vec<u8>) -> Result<Vec<u8>, host::Error> {
        // the loaded snapshot was saved post-inner-commit, so the native query's
        // committed+pending merge serves it with an empty pending — the live
        // (staged-overlay) projection the runtime hands this round is already
        // folded into `__state`.
        let module = loaded_module()?;
        block_on(module.query(&req)).map_err(to_wit_error)
    }
}

guest_adapter::export_module!(Component);
