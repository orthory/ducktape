//! `automations-wasm` — the wasm port of the `automations` module, built the
//! ADAPTER way: the NATIVE `automations` crate is compiled to wasm32
//! unmodified and adapted to the `ducktape:module` world through
//! `guest-adapter`, so the module's logic is single-sourced (a behavior
//! change in the native crate IS the wasm change).
//!
//! ## the whole-state dispatch model, and why it is equivalent
//!
//! the guest is re-instantiated per dispatch, so the native module inside it
//! must FULLY apply per dispatch. each `execute`:
//!
//! 1. loads the persisted snapshot through the host's staged-overlay reads
//!    (`__state`/`__root`, verify-then-adopt via the native `install`),
//! 2. runs the native `execute` over a [`WitCtx`] — INCLUDING the chat-hook
//!    intake's pre-emit PROBES (channel-exists, message-id-unused,
//!    task-id-unused), which are host-routed `query-module` reads the runtime
//!    resolves through memoized replay against the SIBLINGS' live
//!    staged-over-committed state — exactly the view the native probes read,
//! 3. on success calls the INNER module's `commit_block`, and
//! 4. saves the new canonical snapshot back as STAGED host writes.
//!
//! the fold is safe for automations SPECIFICALLY because every decision in
//! its execute paths reads staged-over-committed (`rule` /`effective_rules`
//! shadow the committed map with `pending_rules`; the history ring appends
//! through `pending_history`), so a reloaded module that sees earlier
//! same-block writes as committed decides byte-identically. the run-history
//! ring cap is fold-invariant too: trimming to the newest
//! [`automations::MAX_RUN_HISTORY`] records once per block (native) and once
//! per dispatch (this guest) keeps the same suffix. the NO-FAIL hook-arm
//! contract crosses the seam unchanged:
//! an undecodable event, a failed text fetch, or a probe rejection stages a
//! `RunRecord` and returns Ok — never a trap — so a user's posting block can
//! never be aborted by this module on either runtime (pinned by
//! `wasm_automations_parity.rs`).
//!
//! the persisted encoding is the native module's canonical snapshot stored as
//! ONE host-KV value, so the wasm root is the host-KV encoding over the two
//! reserved keys — a STATE-SCHEMA BREAK versus the native root (revision 2 in
//! `MODULE_STATE_SCHEMAS`; beta networks re-genesis, no back-compat shim).

use automations::Automations;
use guest_adapter::{block_on, host, load_state, save_state, Guest, WitCtx};
use sdk::{Error, Module as _, Msg, StateRoot};

/// the genesis-constant id this module registers under (the native twin's id:
/// `Env::me` and follow-up routing must read identically to ported logic).
const MODULE_ID: &str = "automations";
/// the sibling ids compiled into this instance — EXACTLY the production
/// wiring (`bin/node/src/host_state.rs`): chat is both the trusted hook
/// origin and the PostMessage target, tasks the CreateTask target, inbox the
/// DeliverInbox target.
const CHAT_ID: &str = "chat";
const TASKS_ID: &str = "tasks";
const INBOX_ID: &str = "inbox";

struct Component;

/// the native module at THIS dispatch's state: genesis shape when nothing was
/// ever persisted, else the persisted snapshot verify-then-adopted against its
/// persisted root. an install failure is host-store corruption surfaced as a
/// deterministic rejection, never a silent re-genesis.
fn loaded_module() -> Result<Automations, host::Error> {
    let mut module = Automations::new(MODULE_ID, CHAT_ID, TASKS_ID, INBOX_ID);
    if let Some((bytes, root)) = load_state() {
        module
            .install(&bytes, StateRoot(root))
            .map_err(|e| host::Error::Rejected(format!("automations state reload: {e}")))?;
    }
    Ok(module)
}

/// map an inner sdk error onto the wit surface. `Module` is the native
/// rejection verbatim; anything else a native automations never surfaces from
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
        // query's committed+pending merge serves it with an empty pending —
        // the live (staged-overlay) projection the runtime hands this round
        // is already folded into `__state`. automations queries are pure
        // rule/history reads (no sibling access), so the ctx-less native
        // `query` is the whole surface.
        let module = loaded_module()?;
        block_on(module.query(&req)).map_err(to_wit_error)
    }
}

guest_adapter::export_module!(Component);
