//! the wasm port of this module, built the ADAPTER
//! way: the NATIVE `vaults` crate is compiled to wasm32 unmodified and
//! adapted to the `ducktape:module` world through `guest-adapter`, so the
//! module's logic is single-sourced (a behavior change in the native crate IS
//! the wasm change).
//!
//! vaults is a WHOLE-STATE (`SnapshotBytes`) module — `root()` is sha256 over
//! the canonical encoding of committed vaults — so the port is the
//! `snapshot_guest!` shell: each dispatch verify-then-adopts the persisted
//! snapshot from the host's staged-overlay reads, runs the native execute,
//! commits the inner per-op overlay, and saves the canonical bytes back as
//! STAGED host writes. the host owns the real commit/abort boundary, so
//! mid-block roots, read-your-writes across a block's dispatches, and the
//! abort path match the native staging contract point by point (see the
//! shell's docs in `guest-adapter`).

use crate::Vaults;

/// the genesis-constant id this module registers under (the native twin's id:
/// `Env::me` and follow-up routing must read identically to ported logic).
const MODULE_ID: &str = "vaults";

// whole-state port: no store — the canonical snapshot is the single host-KV
// value under the adapter's reserved keys. no genesis config: vaults start
// empty and every vault is created by an authenticated external op.
guest_adapter::snapshot_guest! {
    id: MODULE_ID,
    module: Vaults,
    new: Vaults::new(MODULE_ID),
}
