//! the wasm port of this module, built the ADAPTER way: the native crate is
//! compiled to wasm32 unmodified and adapted to the `ducktape:module` world
//! through `guest-adapter`, so the module's logic is single-sourced (a behavior
//! change in the native crate IS the wasm change). the packaging cdylib around
//! this port is synthesized by `guest-builder` — this module is the whole of
//! the guest's hand-written surface.
//!
//! the guest takes the crate with `default-features = false`: the `native`
//! feature carries only the node-local derived index (whose `indexer` dep is
//! unix-only IO), never consensus state — so the ported state machine is the
//! FULL consensus surface.
//!
//! the whole-state dispatch model and its equivalence argument are spelled out
//! in `agent`'s guest port (a whole-state tenant) and `guest-adapter`; tasks is
//! the same shape: a pure `SnapshotBytes` module whose canonical snapshot is
//! persisted as ONE host-KV value per dispatch. that host-KV encoding is a
//! STATE-SCHEMA BREAK versus the native root (revision 3 — the tasks+jobs merge
//! folded the former `jobs` module's job board into this one, so the canonical
//! snapshot is now the task board and job board concatenated; beta networks
//! re-genesis, no back-compat shim).

use crate::Tasks;

/// the genesis-constant id this module registers under (the native twin's id:
/// `Env::me` and follow-up routing must read identically to ported logic).
const MODULE_ID: &str = "tasks";

// whole-state port: the shell loads/saves the canonical snapshot and runs the
// native module per dispatch (see `guest_adapter::snapshot_guest!`).
guest_adapter::snapshot_guest! {
    id: MODULE_ID,
    module: Tasks,
    new: Tasks::new(MODULE_ID),
}
