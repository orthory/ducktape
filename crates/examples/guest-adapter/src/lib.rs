//! `guest-adapter` — the port harness for running NATIVE module crates as wasm
//! guests without rewriting their logic.
//!
//! a ported guest crate (`vaults-wasm`, …) compiles the native module crate to
//! `wasm32-unknown-unknown` and ADAPTS it to the `ducktape:module` world using
//! the pieces here, so the module's logic stays single-sourced in the native
//! crate — drift between the native and wasm builds is a compile error, not a
//! consensus bug. the adapter owns the three seams every port needs:
//!
//! * [`WitCtx`] — an [`sdk::Ctx`] over the host imports, so the native
//!   module's `execute(&mut dyn Ctx, ..)` runs unmodified inside the guest.
//! * [`block_on`] — the guest's executor. every await in a module resolves
//!   immediately (the sdk contract: awaits are on deterministic resources, and
//!   inside a guest they are all synchronous host calls underneath), so this
//!   is a noop-waker poll loop that NEVER parks.
//! * [`load_state`] / [`save_state`] — whole-state persistence for pure
//!   (`SnapshotBytes`) modules: the module's canonical snapshot is persisted
//!   as ONE value in the host-owned store each dispatch (under [`STATE_KEY`],
//!   with its 32-byte root under [`ROOT_KEY`] so reloads verify-then-adopt).
//!   the `WasmModule` root is therefore the host-KV encoding over these two
//!   keys — a STATE-SCHEMA BREAK versus the native module's root, declared by
//!   bumping the module's state-schema revision in the same change. there is
//!   no back-compat constraint (beta networks re-genesis on a schema break).
//! * [`load_config`] — the GENESIS-CONFIG read for tenants whose native
//!   constructor takes per-network parameters (a chain id, an invite binding):
//!   the host installs an `sdk::genesis_config`-encoded `__config` store entry
//!   at genesis construction, and the guest decodes it per dispatch.
//!
//! the dispatch shape of a whole-state port: load the snapshot through the
//! host's staged-overlay reads (read-your-writes across the dispatches of one
//! block), run the native module, commit the INNER module per dispatch, and
//! save the new snapshot back as a staged host write. the OUTER staging —
//! what the host publishes at the real block boundary or discards on abort —
//! is the only durable seam, so multi-dispatch blocks and aborts behave
//! exactly like the native module (see `vaults-wasm` for the argument spelled
//! out against a concrete module).

/// the raw generated bindings. a named module (not the crate root) because the
/// generated `#[macro_export]` machinery re-imports its own crate-root macro
/// names — at the root those two bindings collide. public because the export
/// macro's expansion names `guest_adapter::bindings` from the DOWNSTREAM crate.
#[doc(hidden)]
pub mod bindings {
    wit_bindgen::generate!({
        world: "module",
        path: "../../kernel/module-guest/wit",
        // downstream guest crates invoke the export macro and reuse the
        // generated types from THIS crate (bindings are generated once, here,
        // never per guest).
        pub_export_macro: true,
        export_macro_name: "export_module",
        default_bindings_module: "guest_adapter::bindings",
    });
}

// the host import surface, the world trait, and the export macro, re-exported
// so a guest crate needs only `use guest_adapter::{Guest, host, ...}` plus one
// `guest_adapter::export_module!(Component)`.
pub use bindings::ducktape::module::host;
pub use bindings::{export_module, Guest};

use sdk::{Ctx, Env, Error, Event, Msg, Origin, StateRoot, ROOT_LEN};

use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

// ============================================================================
// WitCtx — sdk::Ctx over the host imports
// ============================================================================

/// the [`sdk::Ctx`] a ported native module sees inside the guest: every method
/// forwards to the corresponding `ducktape:module/host` import. construct one
/// per dispatch (the env is read once per instantiation — it is dispatch-
/// constant, so caching it is free and keeps repeated `env()` calls cheap).
pub struct WitCtx {
    env: Env,
}

impl WitCtx {
    pub fn new() -> Self {
        Self {
            env: env_from_wit(host::get_env()),
        }
    }
}

impl Default for WitCtx {
    fn default() -> Self {
        Self::new()
    }
}

fn env_from_wit(env: host::Env) -> Env {
    Env {
        height: env.height,
        consensus_time: env.consensus_time,
        origin: match env.origin {
            host::Origin::External(id) => Origin::External(id),
            host::Origin::FromModule(id) => Origin::Module(id),
            host::Origin::System => Origin::System,
        },
        me: env.me,
        protocol_version: env.protocol_version,
    }
}

/// map a wit host error back onto the sdk surface a native module expects —
/// the exact INVERSE of wasm-host's `to_wit_error`, so an error that crossed
/// the boundary out and back reads the same to the ported logic as it would
/// have natively.
fn error_from_wit(target: &str, e: host::Error) -> Error {
    match e {
        host::Error::Rejected(m) => Error::Module(m),
        host::Error::NotFound => Error::UnknownModule(target.to_string()),
        host::Error::Unsupported => Error::QueryUnsupported,
    }
}

#[async_trait::async_trait(?Send)]
impl Ctx for WitCtx {
    fn env(&self) -> &Env {
        &self.env
    }

    fn module_root(&self, target: &str) -> Option<StateRoot> {
        host::module_root(target).map(|bytes| {
            // the host only ever hands out ROOT_LEN digests; anything else is a
            // host bug, and a panic here is a deterministic trap (a rejection),
            // never a fork.
            let root: [u8; ROOT_LEN] = bytes
                .try_into()
                .expect("host module-root answers are 32 bytes");
            StateRoot(root)
        })
    }

    async fn query(&self, target: &str, req: &[u8]) -> Result<Vec<u8>, Error> {
        host::query_module(target, req).map_err(|e| error_from_wit(target, e))
    }

    fn emit_msg(&mut self, msg: Msg) {
        host::emit_msg(&msg.target, &msg.payload);
    }

    fn emit_event(&mut self, ev: Event) {
        host::emit_event(&ev.source, &ev.payload);
    }
}

// ============================================================================
// block_on — the guest executor
// ============================================================================

/// polls before the executor declares the future stuck. one poll drives every
/// nested synchronous await to completion; the headroom only covers adapters
/// (e.g. boxed future chains) that legitimately need a re-poll after Pending,
/// and it is a constant so a stuck future fails identically on every validator.
const MAX_POLLS: usize = 64;

/// drive a module future to completion synchronously. every await in a guest
/// module resolves immediately — the sdk contract says awaits are on
/// deterministic resources, and in the guest they are all synchronous host
/// calls underneath — so a noop-waker poll suffices. a future still Pending
/// after [`MAX_POLLS`] rounds is waiting on something no guest may wait on
/// (io, timers, channels): fail loud with a deterministic panic (a trap, hence
/// a clean rejection) rather than park forever.
pub fn block_on<F: Future>(f: F) -> F::Output {
    let mut f = pin!(f);
    let mut cx = Context::from_waker(Waker::noop());
    for _ in 0..MAX_POLLS {
        match f.as_mut().poll(&mut cx) {
            Poll::Ready(out) => return out,
            Poll::Pending => continue,
        }
    }
    panic!(
        "guest future still pending after {MAX_POLLS} polls — a module future \
         must only await deterministic, immediately-ready resources"
    );
}

// ============================================================================
// whole-state persistence for pure (SnapshotBytes) modules
// ============================================================================

/// reserved host-store key for the module's canonical snapshot bytes.
pub const STATE_KEY: &[u8] = b"__state";
/// reserved host-store key for the snapshot's 32-byte root (the expected-root
/// argument reloads verify against — verify-then-adopt, like every install).
pub const ROOT_KEY: &[u8] = b"__root";

/// read the persisted snapshot + root through the host's staged-overlay view
/// (so a later dispatch in the same block sees an earlier dispatch's writes).
/// `None` means the module has never persisted — construct it at genesis
/// shape. a HALF-persisted pair or a malformed root is store corruption:
/// panic (a deterministic trap) rather than silently re-genesis the module.
pub fn load_state() -> Option<(Vec<u8>, [u8; ROOT_LEN])> {
    match (host::state_get(STATE_KEY), host::state_get(ROOT_KEY)) {
        (None, None) => None,
        (Some(bytes), Some(root)) => {
            let root: [u8; ROOT_LEN] = root
                .try_into()
                .expect("persisted __root must be 32 bytes");
            Some((bytes, root))
        }
        // one key without the other can only mean a torn write in the
        // host-owned store; treating it as "no state" would wipe the module.
        _ => panic!("half-persisted module state: __state/__root must land together"),
    }
}

/// stage the module's canonical snapshot (and its root) as the whole of this
/// module's durable state. the writes are OUTER staging: the host publishes
/// them at the block-commit boundary and discards them on abort, so the
/// persisted pair always describes a state every honest validator agreed on.
pub fn save_state(bytes: &[u8], root: &[u8; ROOT_LEN]) {
    host::state_set(STATE_KEY, bytes);
    host::state_set(ROOT_KEY, root);
}

// ============================================================================
// genesis config — per-network parameters for a fixed component
// ============================================================================

/// read this tenant's GENESIS-CONFIG bytes: the reserved
/// [`sdk::genesis_config::CONFIG_KEY`] (`__config`) entry the HOST installed
/// into the store at genesis construction, carrying the per-network parameters
/// (a chain id, an invite binding, …) that a fixed wasm component cannot
/// compile in. decode with [`sdk::genesis_config::decode_config`] and
/// construct the native module with the decoded parameters each dispatch.
///
/// the config is CONSENSUS STATE — installed identically on every node, part
/// of the module root from genesis, and carried by checkpoint snapshots like
/// any other store key — so restore and state-sync need nothing special.
/// `None` means the host never installed one: for a tenant whose constructor
/// NEEDS parameters that is wiring corruption, and the guest should reject
/// deterministically rather than guess defaults. the module never writes this
/// key; `save_state` touches only `__state`/`__root`, so the config persists
/// untouched across every dispatch.
pub fn load_config() -> Option<Vec<u8>> {
    host::state_get(sdk::genesis_config::CONFIG_KEY)
}
