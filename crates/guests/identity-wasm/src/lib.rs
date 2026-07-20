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
//! save the canonical snapshot back as OUTER staged writes — is `vaults-wasm`
//! / `duckdns-wasm` verbatim; see those crates for the equivalence argument
//! spelled out. the persisted encoding is the native canonical snapshot as ONE
//! host-KV value: a STATE-SCHEMA BREAK versus the native root (revision 2;
//! beta networks re-genesis, no back-compat shim). the WebAuthn / P-256 member
//! verifies run IN the guest — pure-Rust p256, deterministic on wasm32.

use guest_adapter::{block_on, host, load_config, load_state, save_state, Guest, WitCtx};
use identity::Identity;
use sdk::{genesis_config, Error, Module as _, Msg, StateRoot};

/// the genesis-constant id this module registers under (the native twin's id:
/// `Env::me` and follow-up routing must read identically to ported logic).
const MODULE_ID: &str = "identity";
/// the sibling id this instance gates binds through — EXACTLY the production
/// wiring (`bin/node/src/host_state.rs`): the valset module whose validators ∪
/// residents union admits a bind origin.
const VALSET_ID: &str = "valset";
/// the genesis-config key carrying this network's chain id.
const CHAIN_ID_PARAM: &str = "chain_id";

struct Component;

/// this network's chain id, decoded from the host-installed genesis config. a
/// missing or malformed config is host wiring corruption surfaced as a
/// deterministic rejection — never a guessed default (a wrong chain id would
/// silently refuse every certificate).
fn chain_id() -> Result<String, host::Error> {
    let raw = load_config().ok_or_else(|| {
        host::Error::Rejected("identity genesis config missing (__config)".into())
    })?;
    let params = genesis_config::decode_config(&raw)
        .map_err(|e| host::Error::Rejected(format!("identity genesis config: {e}")))?;
    let chain_id = genesis_config::find(&params, CHAIN_ID_PARAM).ok_or_else(|| {
        host::Error::Rejected("identity genesis config carries no chain_id".into())
    })?;
    String::from_utf8(chain_id.to_vec())
        .map_err(|e| host::Error::Rejected(format!("identity chain_id is not utf-8: {e}")))
}

/// the native module at THIS dispatch's state: genesis shape (under the
/// configured chain id) when nothing was ever persisted, else the persisted
/// snapshot verify-then-adopted against its persisted root. an install failure
/// is host-store corruption surfaced as a deterministic rejection, never a
/// silent re-genesis.
fn loaded_module() -> Result<Identity, host::Error> {
    let mut module = Identity::new(MODULE_ID, Some(VALSET_ID.into()), chain_id()?);
    if let Some((bytes, root)) = load_state() {
        module
            .install(&bytes, StateRoot(root))
            .map_err(|e| host::Error::Rejected(format!("identity state reload: {e}")))?;
    }
    Ok(module)
}

/// map an inner sdk error onto the wit surface. `Module` is the native
/// rejection verbatim; anything else a native identity never surfaces from
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
        // the loaded snapshot was saved post-inner-commit, so the native
        // query's merged (pending-over-committed) view serves it with an empty
        // pending — the live (staged-overlay) projection the runtime hands
        // this round is already folded into `__state`. identity queries are
        // pure registry reads (no sibling access), so the ctx-less native
        // `query` is the whole surface.
        let module = loaded_module()?;
        block_on(module.query(&req)).map_err(to_wit_error)
    }
}

guest_adapter::export_module!(Component);
