//! the wasm port of this module, built the ADAPTER way:
//! the NATIVE `runs` crate is compiled to wasm32 unmodified and adapted to
//! the `ducktape:module` world through `guest-adapter`, so the module's logic
//! is single-sourced (a behavior change in the native crate IS the wasm
//! change).
//!
//! ## the whole-state dispatch model, and why it is equivalent
//!
//! the guest is re-instantiated per dispatch, so the native module inside it
//! must FULLY apply per dispatch. each `execute`:
//!
//! 1. loads the persisted snapshot through the host's staged-overlay reads
//!    (`__state`/`__root`, verify-then-adopt via the native `install`),
//! 2. runs the native `execute` over a [`WitCtx`] — INCLUDING every sibling
//!    read the collaboration loop makes (registry records off `agent`, the
//!    transcript window and post probes off `chat`, page threads off `pages`,
//!    the committed duckfs head off `files`, task ids off `tasks`, board items
//!    off `jobs`, and the dispatch plane's COMMITTED-ONLY read facade for
//!    `turn_taken` / `lease_holder` — dispatch stays NATIVE host-side and
//!    answers committed-only regardless of caller), all host-routed
//!    `query-module` reads the runtime resolves through memoized replay,
//! 3. on success calls the INNER module's `commit_block` — publishing the
//!    three staged overlays (watches, pending runs, agent sessions) into
//!    their committed maps — and
//! 4. saves the new canonical snapshot back as STAGED host writes.
//!
//! the fold is safe for runs SPECIFICALLY because every decision in its
//! handle paths reads staged-over-committed: the `watch` / `pending_entry` /
//! `session` accessors shadow the committed maps with this block's overlays,
//! and `visible_ids` unions both — so a reloaded module that sees earlier
//! same-block writes as committed decides byte-identically. the turn claim
//! (`turn_taken`) short-circuits on the staged pending entry before falling
//! through to dispatch's PERMANENT committed record; there is no
//! frozen-committed self-read anywhere in its handle paths (contrast
//! lifecycle's `Advance` and dispatch's read facade, which stay native for
//! exactly that reason). the ordering-contract surfaces cross the seam
//! unchanged: the engagement/result/jobs intakes stay NO-FAIL (a bad event
//! degrades to a breadcrumb `emit_event`, never a trap), the registry hook
//! still MAY error (aborting the registration block is the atomicity the
//! recipe seam needs), and every emitted follow-up (the dispatch, the reply,
//! the task writes, the jobs claim/finalize) leaves through the wit
//! `emit-msg` import and lands in the same block it always did (P2/P6).
//!
//! ## the delivered-runs ring rides its OWN key
//!
//! the native module's `RunsQuery::RecentRuns` ring is DERIVED state —
//! deliberately outside `root()`/`snapshot()`, rebuilt by replay on a native
//! node. the guest has no per-node memory to rebuild into (it is
//! re-instantiated per dispatch), and the ring has real consumers (the app's
//! runs client, the dogfood receipt lane), so this port persists the
//! committed ring as a THIRD host-KV value ([`HISTORY_KEY`]) beside the
//! canonical snapshot. that folds the ring into the wasm module's root —
//! safe, because every `RunRecord` field is already a deterministic
//! consensus derivation (the executing-node attribution feeds PR-body
//! breadcrumbs, committed forge state, today) — and it makes the ring ride
//! snapshots/state-sync, where a native joiner started empty. the ring cap
//! is fold-invariant: trimming to the newest 100 once per block (native) and
//! once per dispatch (this guest) keeps the same suffix (pinned by
//! `wasm_runs_parity.rs`).
//!
//! the persisted encoding is the native module's canonical snapshot stored as
//! ONE host-KV value (plus the ring under its own key), so the wasm root is
//! the host-KV encoding over the three reserved keys.

use crate::RunsModule;
use guest_adapter::{Guest, WitCtx, block_on, host, load_state, save_state};
use sdk::{Error, Module as _, Msg, StateRoot};

/// the genesis-constant id this module registers under (the native twin's id:
/// `Env::me` and follow-up routing must read identically to ported logic).
const MODULE_ID: &str = "runs";
/// the sibling ids compiled into this instance — EXACTLY the production
/// wiring (`bin/node/src/host_state.rs`): chat is the transcript/probe/reply
/// surface, saga the dead-letter origin, tagging the engagement intake's
/// trusted origin, dispatch every run's recipe registry + executor +
/// lifecycle ledger, agent the registry hook's trusted origin, tasks/jobs the
/// action and board lanes, files the envelope's source-snapshot pin, forge
/// the PR/merge sink target, pages the `[[page:]]` context + effects lane.
const CHAT_ID: &str = "chat";
const SAGA_ID: &str = "saga";
const TAGGING_ID: &str = "tagging";
const DISPATCH_ID: &str = "dispatch";
const AGENT_ID: &str = "agent";
const TASKS_ID: &str = "tasks";
// the job board merged into the `tasks` work module: both roles resolve here.
const JOBS_ID: &str = "tasks";
const FILES_ID: &str = "files";
const FORGE_ID: &str = "forge";
const PAGES_ID: &str = "pages";

/// reserved host-store key for the delivered-runs ring — the derived
/// observability state the native module keeps OUTSIDE its canonical
/// snapshot, persisted here so it survives the per-dispatch re-instantiation
/// (see the crate doc). written on every clean execute, exactly like
/// `__state`/`__root`, so the three keys can never tear apart.
const HISTORY_KEY: &[u8] = b"__history";

struct Component;

/// the native module at THIS dispatch's state: genesis shape when nothing was
/// ever persisted, else the persisted snapshot verify-then-adopted against its
/// persisted root. an install failure is host-store corruption surfaced as a
/// deterministic rejection.
fn loaded_module() -> Result<RunsModule, host::Error> {
    let mut module = RunsModule::new(
        MODULE_ID,
        CHAT_ID,
        SAGA_ID,
        TAGGING_ID,
        DISPATCH_ID,
        AGENT_ID,
        Some(TASKS_ID.into()),
        Some(JOBS_ID.into()),
    )
    .with_files_module(FILES_ID)
    .with_sink_forge(FORGE_ID)
    .with_pages_module(PAGES_ID)
    // the per-network parameter a fixed component cannot compile in: the host
    // installed it as this Map tenant's genesis state (`__config`), and every
    // `duck://` link the injector renders stamps its `?net=` half from it. a
    // missing or malformed record is host wiring corruption, refused
    // deterministically rather than silently producing network-less links.
    .with_chain_id(guest_adapter::genesis_chain_id(MODULE_ID)?);
    if let Some((bytes, root)) = load_state() {
        module
            .install(&bytes, StateRoot(root))
            .map_err(|e| host::Error::Rejected(format!("runs state reload: {e}")))?;
    }
    // AFTER install (which clears the in-memory ring): adopt the persisted
    // delivered-runs ring. absent means the module never persisted — the
    // genesis-empty ring install left behind.
    if let Some(bytes) = host::state_get(HISTORY_KEY) {
        module
            .install_history(&bytes)
            .map_err(|e| host::Error::Rejected(format!("runs history reload: {e}")))?;
    }
    Ok(module)
}

/// map an inner sdk error onto the wit surface. `Module` is the native
/// rejection verbatim; anything else a native runs never surfaces from
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
        // persist the canonical snapshot — and the delivered-runs ring, under
        // its own key — as OUTER staged writes. the host owns the real
        // commit/abort boundary (see the crate doc), so an aborted block
        // discards the ring append exactly like the native `abort_block`.
        block_on(module.commit_block()).map_err(to_wit_error)?;
        save_state(&module.snapshot(), module.root().as_bytes());
        host::state_set(HISTORY_KEY, &module.history_snapshot());
        Ok(())
    }

    fn query(req: Vec<u8>) -> Result<Vec<u8>, host::Error> {
        // the loaded snapshot was saved post-inner-commit, so the native
        // query's committed+pending union serves it with an empty overlay —
        // the live (staged-overlay) projection the runtime hands this round
        // is already folded into `__state`. runs queries (PendingRuns /
        // Watches / AgentSessions) are pure self reads, and RecentRuns serves
        // the ring reloaded off `__history` — so the ctx-less native `query`
        // is the whole surface.
        let module = loaded_module()?;
        block_on(module.query(&req)).map_err(to_wit_error)
    }
}

guest_adapter::export_module!(Component);
