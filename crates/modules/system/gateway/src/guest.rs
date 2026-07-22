//! the wasm port of the merged `gateway` module, built the
//! ADAPTER way: the NATIVE `gateway` crate is compiled to wasm32 unmodified and
//! adapted to the `ducktape:module` world through `guest-adapter`, so the
//! module's logic is single-sourced (a behavior change in the native crate IS
//! the wasm change).
//!
//! `gateway` now owns the WHOLE `.duck` name → AccountId → route pipeline: the
//! route plane AND the `.duck` handle plane absorbed from the retired
//! `duckdns` module. So this single guest replaces the old gateway + duckdns
//! guest pair; its persisted snapshot is the merged canonical state
//! (both planes under one root — a STATE-SCHEMA BREAK, revision 3).
//!
//! like the `identity` guest port, the constructor takes a PER-NETWORK parameter — the
//! chain id every route statement is scoped to — which arrives as GENESIS
//! CONFIG: the host installs an `sdk::genesis_config`-encoded `__config` entry
//! into this module's consensus store at genesis construction, and every
//! dispatch decodes it and constructs the native module with it. the config is
//! consensus state and rides checkpoint snapshots like any other store key.
//!
//! every gateway execute depends on SIBLING reads — the valset standing gate
//! (validators ∪ residents) and the identity `OfNode` account derivation plus
//! current-member check — which resolve through the runtime's memoized replay.
//! the whole-state dispatch model (load `__state`/`__root` through the host's
//! staged overlay, run the native `execute`, commit the INNER module, save the
//! canonical snapshot back as OUTER staged writes) is the `agent` guest port verbatim;
//! see that crate for the equivalence argument.
//! the persisted encoding is the native canonical snapshot as ONE host-KV
//! value: a STATE-SCHEMA BREAK versus the native root (revision 3 — the merge
//! of the handle plane into the route plane's snapshot; beta networks
//! re-genesis, no back-compat shim).

use crate::Gateway;

/// the genesis-constant id this module registers under (the native twin's id:
/// `Env::me` and follow-up routing must read identically to ported logic).
const MODULE_ID: &str = "gateway";
/// the sibling ids this instance reads through host-routed queries — EXACTLY
/// the production wiring (`bin/node/src/host_state.rs`): the identity module
/// that resolves the publisher node to its account, and the valset module that
/// gates mutations to members.
const IDENTITY_ID: &str = "identity";
const VALSET_ID: &str = "valset";

// whole-state port with a per-network chain id from genesis config (the
// `genesis_chain_id` hook); the shell loads/saves the canonical snapshot and
// runs the native module per dispatch (see `guest_adapter::snapshot_guest!`).
guest_adapter::snapshot_guest! {
    id: MODULE_ID,
    module: Gateway,
    new: Gateway::new(MODULE_ID, IDENTITY_ID, Some(VALSET_ID.into()), guest_adapter::genesis_chain_id("gateway")?),
}
