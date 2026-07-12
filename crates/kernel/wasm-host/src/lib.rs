//! `wasm-host` — the wasmtime embedding that runs a `ducktape:module` component
//! as a native [`sdk::Module`].
//!
//! Design-B (host-owned state): the guest is PURE logic. It holds no durable
//! memory across dispatches — the host re-instantiates a fresh component
//! instance per call — so all durable state lives here, behind the `state`
//! capability. Consequences:
//!   * `root()` is host-computed from the owned store (not the guest), so a
//!     code swap keeps the store: that is the live-update primitive.
//!   * writes are STAGED in an overlay during `execute` and published at
//!     `commit_block` / discarded at `abort_block` — byte-for-byte the native
//!     module staging contract.
//!   * determinism is by construction: fresh instance (no memory carryover),
//!     fuel-metered termination, no ambient host imports, integer/bytes ABI.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};
use wasmtime::component::{Component, HasSelf, Linker};
use wasmtime::{Config, Engine, Store};

use sdk::{
    Ctx, Env as SdkEnv, Error as SdkError, Event, Module, ModuleId, Msg, Origin as SdkOrigin,
    StateRoot, StateSyncHandle,
};

mod bindings {
    wasmtime::component::bindgen!({
        world: "module",
        path: "../module-guest/wit",
    });
}

use bindings::Module as ModuleWorld;
use bindings::ducktape::module::host::{self, Env as WitEnv, Error as WitError, Origin as WitOrigin};

/// default per-dispatch fuel budget: the deterministic termination bound. It is
/// identical on every validator, so a runaway guest traps at the same point on
/// all of them — a trap is a deterministic rejection, not a per-node fork.
pub const DEFAULT_FUEL: u64 = 2_000_000_000;

// ============================================================================
// per-dispatch host state — owned (no borrows), so `Store<T>` stays `'static`.
// The committed/staged maps are MOVED in before a call and MOVED back out
// after, mirroring the host's own remove-execute-reinsert dispatch trick.
// ============================================================================

#[derive(Default)]
struct HostData {
    env: Option<WitEnv>,
    committed: BTreeMap<Vec<u8>, Vec<u8>>,
    staged: BTreeMap<Vec<u8>, Option<Vec<u8>>>,
    out_msgs: Vec<(String, Vec<u8>)>,
    out_events: Vec<(String, Vec<u8>)>,
}

impl HostData {
    /// overlay-over-committed read: a staged `Some` is a set, staged `None` a
    /// delete, absence falls through to committed.
    fn effective_get(&self, key: &[u8]) -> Option<Vec<u8>> {
        match self.staged.get(key) {
            Some(overlay) => overlay.clone(),
            None => self.committed.get(key).cloned(),
        }
    }
}

impl host::Host for HostData {
    fn get_env(&mut self) -> WitEnv {
        self.env.clone().expect("env is set before every dispatch")
    }
    fn state_get(&mut self, key: Vec<u8>) -> Option<Vec<u8>> {
        self.effective_get(&key)
    }
    fn state_set(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.staged.insert(key, Some(value));
    }
    fn state_delete(&mut self, key: Vec<u8>) {
        self.staged.insert(key, None);
    }
    fn module_root(&mut self, _target: String) -> Option<Vec<u8>> {
        // TODO(v2): cross-module snapshot root, threaded from the host `Ctx`.
        None
    }
    fn query_module(&mut self, _target: String, _req: Vec<u8>) -> Result<Vec<u8>, WitError> {
        // TODO(v2): host-routed sibling read, threaded from the host `Ctx`.
        Err(WitError::Unsupported)
    }
    fn emit_msg(&mut self, target: String, payload: Vec<u8>) {
        self.out_msgs.push((target, payload));
    }
    fn emit_event(&mut self, source: String, payload: Vec<u8>) {
        self.out_events.push((source, payload));
    }
}

// ============================================================================
// the wasm module
// ============================================================================

/// A wasm module: a `ducktape:module` component plus its host-owned
/// authenticated state. Presented to the host as an ordinary [`sdk::Module`].
pub struct WasmModule {
    id: ModuleId,
    engine: Engine,
    linker: Linker<HostData>,
    component: Component,
    /// sha256 of the component bytes currently loaded — the CODE identity the
    /// host reconciles against the registry's committed active hash. NOT part of
    /// `root()` (code is invisible to the app-hash); per-node realization only.
    code_hash: Vec<u8>,
    committed: BTreeMap<Vec<u8>, Vec<u8>>,
    staged: BTreeMap<Vec<u8>, Option<Vec<u8>>>,
    fuel: u64,
}

impl WasmModule {
    /// Load a module from component bytes with an empty state store.
    pub fn from_bytes(id: impl Into<ModuleId>, component_bytes: &[u8]) -> Result<Self, SdkError> {
        let engine = Engine::new(&deterministic_config()).map_err(module_err)?;
        let component = Component::from_binary(&engine, component_bytes).map_err(module_err)?;
        let mut linker = Linker::new(&engine);
        ModuleWorld::add_to_linker::<HostData, HasSelf<HostData>>(&mut linker, |d| d)
            .map_err(module_err)?;
        Ok(Self {
            id: id.into(),
            engine,
            linker,
            component,
            code_hash: sha256(component_bytes),
            committed: BTreeMap::new(),
            staged: BTreeMap::new(),
            fuel: DEFAULT_FUEL,
        })
    }

    /// Canonical bytes of a store: count + length-prefixed sorted `(key, value)`
    /// pairs — the exact preimage of [`WasmModule::root_of`], and therefore the
    /// snapshot format (verify-then-adopt against the root, like modreg).
    fn encode_state(committed: &BTreeMap<Vec<u8>, Vec<u8>>) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(committed.len() as u64).to_le_bytes());
        for (k, v) in committed {
            out.extend_from_slice(&(k.len() as u64).to_le_bytes());
            out.extend_from_slice(k);
            out.extend_from_slice(&(v.len() as u64).to_le_bytes());
            out.extend_from_slice(v);
        }
        out
    }

    /// The authenticated root of the committed store: SHA-256 over
    /// [`WasmModule::encode_state`]. Deterministic and idempotent — the same
    /// scheme the native map-backed modules use, so it composes into the global
    /// app-hash exactly like any other module root.
    fn root_of(committed: &BTreeMap<Vec<u8>, Vec<u8>>) -> StateRoot {
        let mut h = Sha256::new();
        h.update(Self::encode_state(committed));
        StateRoot(h.finalize().into())
    }

    /// canonical bytes of COMMITTED state — the exact preimage of `root()`.
    /// this is what checkpoint capture ships (see [`Module::state_sync_handle`]).
    pub fn snapshot(&self) -> Vec<u8> {
        Self::encode_state(&self.committed)
    }

    /// verify-then-adopt a peer/checkpoint snapshot: strict-decode, recompute
    /// the root, refuse on mismatch — committed state and stage untouched on any
    /// error. code is NOT part of the snapshot (the registry owns the code
    /// commitment; the host reconciles running code separately), so install
    /// never touches the loaded component.
    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), SdkError> {
        let decoded = decode_state(bytes)?;
        if Self::root_of(&decoded) != expected {
            return Err(SdkError::Module("snapshot root mismatch".into()));
        }
        self.committed = decoded;
        self.staged.clear();
        Ok(())
    }
}

// ---- strict snapshot decode (untrusted bytes) -------------------------------

fn take_u64(buf: &mut &[u8]) -> Result<u64, SdkError> {
    let Some((head, rest)) = buf.split_first_chunk::<8>() else {
        return Err(SdkError::Module("snapshot truncated".into()));
    };
    *buf = rest;
    Ok(u64::from_le_bytes(*head))
}

fn take_vec(buf: &mut &[u8]) -> Result<Vec<u8>, SdkError> {
    let len = take_u64(buf)?;
    if len > buf.len() as u64 {
        return Err(SdkError::Module("snapshot length exceeds buffer".into()));
    }
    let (head, rest) = buf.split_at(len as usize);
    *buf = rest;
    Ok(head.to_vec())
}

fn decode_state(bytes: &[u8]) -> Result<BTreeMap<Vec<u8>, Vec<u8>>, SdkError> {
    let mut buf = bytes;
    let count = take_u64(&mut buf)?;
    // each entry costs at least two 8-byte length prefixes — a forged count can
    // never over-allocate.
    if count > (buf.len() / 16) as u64 {
        return Err(SdkError::Module("snapshot entry count exceeds buffer".into()));
    }
    let mut committed = BTreeMap::new();
    let mut prev: Option<Vec<u8>> = None;
    for _ in 0..count {
        let key = take_vec(&mut buf)?;
        // strictly increasing keys: one state has exactly one encoding.
        if prev.as_deref().is_some_and(|p| p >= key.as_slice()) {
            return Err(SdkError::Module(
                "snapshot keys must be strictly increasing".into(),
            ));
        }
        let value = take_vec(&mut buf)?;
        prev = Some(key.clone());
        committed.insert(key, value);
    }
    if !buf.is_empty() {
        return Err(SdkError::Module("snapshot carries trailing bytes".into()));
    }
    Ok(committed)
}

/// The determinism envelope for module execution. Fuel-metered termination, no
/// SIMD nondeterminism, no ambient imports. (Feature-trimming and NaN
/// canonicalization are tracked determinism-hardening follow-ups.)
fn deterministic_config() -> Config {
    let mut c = Config::new();
    c.wasm_component_model(true);
    c.consume_fuel(true);
    c.wasm_simd(false);
    c.wasm_relaxed_simd(false);
    c
}

fn to_wit_env(env: &SdkEnv) -> WitEnv {
    WitEnv {
        height: env.height,
        consensus_time: env.consensus_time,
        protocol_version: env.protocol_version,
        me: env.me.clone(),
        origin: match &env.origin {
            SdkOrigin::External(id) => WitOrigin::External(id.clone()),
            SdkOrigin::Module(id) => WitOrigin::FromModule(id.clone()),
            SdkOrigin::System => WitOrigin::System,
        },
    }
}

/// Any wasmtime/trap/instantiate failure is a DETERMINISTIC rejection: the same
/// code runs on every validator under the same fuel budget, so it traps at the
/// same point. Surfaced as [`SdkError::Module`] → the host rolls the op back.
fn module_err(e: impl std::fmt::Display) -> SdkError {
    SdkError::Module(e.to_string())
}

fn wit_err(e: WitError) -> SdkError {
    SdkError::Module(format!("{e:?}"))
}

/// the 32-byte content hash of a component — the code identity the registry
/// commits to and the host verifies before a swap.
fn sha256(bytes: &[u8]) -> Vec<u8> {
    Sha256::digest(bytes).to_vec()
}

#[async_trait::async_trait(?Send)]
impl Module for WasmModule {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        Self::root_of(&self.committed)
    }

    fn code_hash(&self) -> Option<Vec<u8>> {
        Some(self.code_hash.clone())
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, SdkError> {
        Ok(StateSyncHandle::SnapshotBytes(self.snapshot()))
    }

    /// Replace the component code IN PLACE, keeping the host-owned state store.
    /// This is the live-update primitive: same store, new logic, and the root is
    /// computed from the (untouched) store — so app-hash is continuous across the
    /// swap. Staged (yet uncommitted) writes are discarded: a swap is only ever
    /// driven at a clean block boundary, never mid-block.
    fn swap_code(&mut self, component_bytes: &[u8]) -> Result<(), SdkError> {
        let component = Component::from_binary(&self.engine, component_bytes).map_err(module_err)?;
        self.component = component;
        self.code_hash = sha256(component_bytes);
        self.staged.clear();
        Ok(())
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), SdkError> {
        // move committed + staged into owned per-dispatch data.
        let data = HostData {
            env: Some(to_wit_env(ctx.env())),
            committed: std::mem::take(&mut self.committed),
            staged: std::mem::take(&mut self.staged),
            out_msgs: Vec::new(),
            out_events: Vec::new(),
        };
        let mut store = Store::new(&self.engine, data);

        let call: Result<Result<(), WitError>, SdkError> = match store.set_fuel(self.fuel) {
            Err(e) => Err(module_err(e)),
            Ok(()) => match ModuleWorld::instantiate(&mut store, &self.component, &self.linker) {
                Err(e) => Err(module_err(e)),
                Ok(inst) => inst.call_execute(&mut store, &msg.payload).map_err(module_err),
            },
        };

        // reclaim state regardless of outcome (a trap leaves the moved-in state
        // in the store; take it back so the module is never left empty).
        let data = store.into_data();
        self.committed = data.committed;
        self.staged = data.staged;

        match call {
            Ok(Ok(())) => {
                // only a clean execute publishes its intents; a rejection leaks nothing.
                for (target, payload) in data.out_msgs {
                    ctx.emit_msg(Msg { target, payload });
                }
                for (source, payload) in data.out_events {
                    ctx.emit_event(Event { source, payload });
                }
                Ok(())
            }
            Ok(Err(e)) => Err(wit_err(e)),
            Err(e) => Err(e),
        }
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, SdkError> {
        // read-only projection over COMMITTED state (no staging overlay).
        let data = HostData {
            env: Some(WitEnv {
                height: 0,
                consensus_time: 0,
                protocol_version: 0,
                me: self.id.clone(),
                origin: WitOrigin::System,
            }),
            committed: self.committed.clone(),
            staged: BTreeMap::new(),
            out_msgs: Vec::new(),
            out_events: Vec::new(),
        };
        let mut store = Store::new(&self.engine, data);
        store.set_fuel(self.fuel).map_err(module_err)?;
        let inst =
            ModuleWorld::instantiate(&mut store, &self.component, &self.linker).map_err(module_err)?;
        let res = inst.call_query(&mut store, req).map_err(module_err)?;
        res.map_err(wit_err)
    }

    async fn commit_block(&mut self) -> Result<(), SdkError> {
        for (key, overlay) in std::mem::take(&mut self.staged) {
            match overlay {
                Some(value) => {
                    self.committed.insert(key, value);
                }
                None => {
                    self.committed.remove(&key);
                }
            }
        }
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), SdkError> {
        self.staged.clear();
        Ok(())
    }
}
