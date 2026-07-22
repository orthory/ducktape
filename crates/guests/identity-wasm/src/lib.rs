//! `identity-wasm` — the wasm port of the `identity` module, built the ADAPTER
//! way: the NATIVE `identity` crate is compiled to wasm32 unmodified and
//! adapted to the `ducktape:module` world through `guest-adapter`, so the
//! module's logic is single-sourced (a behavior change in the native crate IS
//! the wasm change).
//!
//! identity is the first ported module whose constructor takes a PER-NETWORK
//! parameter: the chain id every signed certificate preimage folds in. a wasm
//! component is fixed bytes, so the id cannot be compiled in — it arrives as
//! GENESIS CONFIG: the host installs an `sdk::genesis_config`-encoded
//! `__config` entry into this module's consensus store at genesis
//! construction, and every dispatch decodes it and constructs the native
//! module with it (see [`chain_id`]). the config is consensus state (identical
//! on every node, in the root from genesis) and rides checkpoint snapshots
//! like any other store key, so restore/state-sync need nothing special.
//!
//! the whole-state dispatch model — load `__state`/`__root` through the host's
//! staged-overlay reads, run the native `execute` (its valset member gate
//! resolves through the runtime's memoized replay), commit the INNER module,
//! save the canonical snapshot back as OUTER staged writes — is `agent-wasm`
//! verbatim; see that crate for the equivalence argument
//! spelled out. the persisted encoding is the native canonical snapshot as ONE
//! host-KV value: a STATE-SCHEMA BREAK versus the native root (revision 2;
//! beta networks re-genesis, no back-compat shim). the WebAuthn / P-256 member
//! verifies run IN the guest — pure-Rust p256, deterministic on wasm32.

use identity::Identity;

/// the genesis-constant id this module registers under (the native twin's id:
/// `Env::me` and follow-up routing must read identically to ported logic).
const MODULE_ID: &str = "identity";
/// the sibling id this instance gates binds through — EXACTLY the production
/// wiring (`bin/node/src/host_state.rs`): the valset module whose validators ∪
/// residents union admits a bind origin.
const VALSET_ID: &str = "valset";

// whole-state port with a per-network chain id from genesis config (the
// `genesis_chain_id` hook); the shell loads/saves the canonical snapshot and
// runs the native module per dispatch (see `guest_adapter::snapshot_guest!`).
guest_adapter::snapshot_guest! {
    id: MODULE_ID,
    module: Identity,
    new: Identity::new(MODULE_ID, Some(VALSET_ID.into()), guest_adapter::genesis_chain_id("identity")?),
}
