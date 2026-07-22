//! the wasm port of this module, built the
//! ADAPTER way: the NATIVE `governance` crate is compiled to wasm32 unmodified
//! and adapted to the `ducktape:module` world through `guest-adapter`, so the
//! module's logic is single-sourced (a behavior change in the native crate IS
//! the wasm change).
//!
//! governance's per-network parameter is the INVITE BINDING — the genesis
//! namespace every invite token and join proof verify against. a wasm
//! component is fixed bytes, so the binding arrives as GENESIS CONFIG: the
//! host installs an `sdk::genesis_config`-encoded `__config` entry into this
//! module's consensus store at genesis construction, and every dispatch
//! decodes it and constructs the native module with it. the config is
//! consensus state and rides checkpoint snapshots like any other store key.
//! the valset / lifecycle / identity sibling ids are genesis-constant
//! wiring (identical on every network), so they stay compiled in like every
//! other port's sibling ids.
//!
//! governance exercises every seam the runtime offers at once: sibling reads
//! (valset membership, identity account resolution) resolve through the
//! memoized replay, and a passing proposal EMITS follow-up msgs (valset
//! membership ops, lifecycle upgrade schedules + code swaps) that the runtime
//! republishes through the host ctx only after a clean run — so a wasm
//! governance still drives the code registry that live-updates the other wasm
//! tenants. the whole-state dispatch model (load `__state`/`__root` through
//! the host's staged overlay, run the native `execute`, commit the INNER
//! module, save the canonical snapshot back as OUTER staged writes) is
//! the `agent` guest port verbatim; see that crate for the
//! equivalence argument. the persisted encoding is the native canonical
//! snapshot as ONE host-KV value: a STATE-SCHEMA BREAK versus the native root
//! (revision 2; beta networks re-genesis, no back-compat shim).

use crate::Governance;
use guest_adapter::{host, load_config};
use sdk::genesis_config;

/// the genesis-constant id this module registers under (the native twin's id:
/// `Env::me` and follow-up routing must read identically to ported logic).
const MODULE_ID: &str = "governance";
/// the sibling ids this instance reads/authorizes through — EXACTLY the
/// production wiring (`bin/node/src/host_state.rs`): valset for membership
/// (reads + emitted membership ops), the lifecycle module for wasm-module
/// code swaps ("lifecycle" == `host::LIFECYCLE_MODULE_ID`), and identity for
/// account-share resolution.
const VALSET_ID: &str = "valset";
const LIFECYCLE_ID: &str = "lifecycle";
const IDENTITY_ID: &str = "identity";
/// the genesis-config key carrying this network's invite binding.
const INVITE_PARAM: &str = "invite";

/// this network's invite binding, decoded from the host-installed genesis
/// config. a missing or malformed config is host wiring corruption surfaced
/// as a deterministic rejection — never a guessed default (an unwired binding
/// would refuse every `Redeem` its peers accept, which forks).
fn invite_binding() -> Result<Vec<u8>, host::Error> {
    let raw = load_config().ok_or_else(|| {
        host::Error::Rejected("governance genesis config missing (__config)".into())
    })?;
    let params = genesis_config::decode_config(&raw)
        .map_err(|e| host::Error::Rejected(format!("governance genesis config: {e}")))?;
    genesis_config::find(&params, INVITE_PARAM)
        .map(<[u8]>::to_vec)
        .ok_or_else(|| {
            host::Error::Rejected("governance genesis config carries no invite binding".into())
        })
}

// whole-state port; the per-network invite binding comes from genesis config
// via the bespoke `invite_binding` above (a bytes param, not the chain_id
// twin's string). the shell loads/saves the canonical snapshot and runs the
// native module per dispatch (see `guest_adapter::snapshot_guest!`).
guest_adapter::snapshot_guest! {
    id: MODULE_ID,
    module: Governance,
    new: Governance::new(MODULE_ID, VALSET_ID, IDENTITY_ID)
        .with_invite_binding(invite_binding()?)
        // redeem-time client grants ride an `IdentityMsg::GrantClient` follow-up
        // into identity (already wired for account-share). no separate module.
        .with_code_registry(LIFECYCLE_ID),
}
