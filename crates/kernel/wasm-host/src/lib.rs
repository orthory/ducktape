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
//!   * cross-module reads (`module-root` / `query-module`) are MEMOIZED REPLAY:
//!     the sync guest world cannot await the host's async `Ctx`, so a read the
//!     per-dispatch memo can't answer pauses the run (a deterministic trap), the
//!     wrapper resolves it through `Ctx`, and the pure guest re-runs with the
//!     answer memoized — every round re-treads the identical prefix, so the
//!     replay converges in (distinct reads + 1) rounds, bounded by
//!     [`MAX_SIBLING_READS`].
//!
//! COMMITTED state has two backings ([`StateBacking`]):
//!   * `Map` — the original host-KV `BTreeMap`, whose root is sha256 over the
//!     canonical encoding and whose sync surface is installable snapshot bytes.
//!   * `Store` — a host-injected [`sdk::MerkleStore`] (qmdb in production, via
//!     [`WasmModule::with_store`]): the root IS the store's merkle root and
//!     sync is the store's resolver lane, so a native module already written
//!     over `Box<dyn MerkleStore>` ports with a ROOT-CONTINUOUS cutover — the
//!     same ops commit the same store ops and the app-hash never moves. store
//!     reads are async, so `state-get` misses ride the SAME memoized-replay
//!     machinery as sibling reads (bounded by [`MAX_STORE_READS`]); the staged
//!     overlay and the commit/abort boundary are identical in both backings.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};
use wasmtime::component::{Component, HasSelf, Linker};
use wasmtime::{Config, Engine, Store};

use sdk::{
    Ctx, Env as SdkEnv, Error as SdkError, Event, MerkleStore, Module, ModuleId, Msg,
    Origin as SdkOrigin, ROOT_LEN, ResolverSyncTarget, StateRoot, StateSyncHandle,
};

mod bindings {
    wasmtime::component::bindgen!({
        world: "module",
        path: "../module-guest/wit",
        // these imports may TRAP: a read the per-dispatch memo cannot answer
        // pauses the run (deterministically — same point on every validator),
        // the async wrapper resolves it (sibling reads through the host `Ctx`,
        // store-backed `state-get` through the injected store), and the pure
        // guest is replayed with the answer memoized. see `SiblingMemo`.
        imports: {
            "ducktape:module/host.state-get": trappable,
            "ducktape:module/host.module-root": trappable,
            "ducktape:module/host.query-module": trappable,
        },
    });
}

use bindings::Module as ModuleWorld;
use bindings::ducktape::module::host::{self, Env as WitEnv, Error as WitError, Origin as WitOrigin};

/// default per-dispatch fuel budget: the deterministic termination bound. It is
/// identical on every validator, so a runaway guest traps at the same point on
/// all of them — a trap is a deterministic rejection, not a per-node fork.
pub const DEFAULT_FUEL: u64 = 2_000_000_000;

/// per-dispatch bound on DISTINCT sibling reads (`module-root` + `query-module`).
/// each unresolved read replays the pure guest once with the answer memoized, so
/// this caps both the replay count and the memo size. a protocol constant: an op
/// that needs more is rejected identically on every validator.
pub const MAX_SIBLING_READS: usize = 64;

/// per-dispatch bound on DISTINCT committed-store reads (`state-get` misses in
/// [`StateBacking::Store`] mode). own-state reads are far cheaper than sibling
/// reads to answer but far more numerous (an op walks its own records), so they
/// carry their own, larger budget. a protocol constant, like
/// [`MAX_SIBLING_READS`]: an op that needs more is rejected identically on
/// every validator. map-backed modules never pause on state reads, so this
/// bound is store-mode-only.
pub const MAX_STORE_READS: usize = 4096;

/// trap message for a read the memo cannot answer yet. never surfaces to
/// consensus: the execute/query drivers intercept the run (via
/// [`HostData::pending`]) and replay with the answer resolved.
const PENDING_READ_TRAP: &str = "pending host read (host resolves and replays)";

// ============================================================================
// per-dispatch host state — owned (no borrows), so `Store<T>` stays `'static`.
// The committed/staged maps are MOVED in before a call and MOVED back out
// after, mirroring the host's own remove-execute-reinsert dispatch trick.
// ============================================================================

/// one host read the guest attempted that the memo could not answer yet.
enum PendingRead {
    Root(String),
    Query(String, Vec<u8>),
    /// a committed-store `state-get` miss ([`StateBacking::Store`] mode only):
    /// the driver resolves it against the injected [`MerkleStore`] — no ctx
    /// needed, so even the ctx-less [`Module::query`] path replays these.
    State(Vec<u8>),
}

/// resolved read answers, accumulated across the replay rounds of ONE
/// dispatch/query. the guest is pure and its inputs are fixed for the whole
/// dispatch, so each round re-treads the identical prefix; a memo hit returns
/// exactly what the earlier round saw, and each round discovers at most one new
/// read. answers are stable within a dispatch (nothing else runs in between —
/// the injected store only ever moves at `commit_block`, never mid-dispatch).
#[derive(Default)]
struct SiblingMemo {
    roots: BTreeMap<String, Option<Vec<u8>>>,
    queries: BTreeMap<(String, Vec<u8>), Result<Vec<u8>, WitError>>,
    /// committed-store answers for store-backed modules. staged writes shadow
    /// these at `state-get` (overlay first), so a stale entry is unreachable.
    states: BTreeMap<Vec<u8>, Option<Vec<u8>>>,
}

impl SiblingMemo {
    /// DISTINCT sibling reads so far (the [`MAX_SIBLING_READS`] budget); the
    /// store-read budget is tracked separately over `states`.
    fn len(&self) -> usize {
        self.roots.len() + self.queries.len()
    }

    /// both replay budgets still hold — the loop guard every driver shares.
    fn within_budgets(&self) -> bool {
        self.len() <= MAX_SIBLING_READS && self.states.len() <= MAX_STORE_READS
    }

    /// the deterministic rejection for a blown replay budget.
    fn budget_error(&self) -> SdkError {
        if self.len() > MAX_SIBLING_READS {
            SdkError::Module(format!(
                "sibling-read budget exceeded ({MAX_SIBLING_READS})"
            ))
        } else {
            SdkError::Module(format!("store-read budget exceeded ({MAX_STORE_READS})"))
        }
    }

    /// resolve one pending SIBLING read against the host ctx and memoize the
    /// answer. errors are answers too: a deterministic rejection (unknown
    /// module, unsupported query, cycle) memoizes as the wit error the guest
    /// will see. `State` reads never reach here — the drivers resolve them
    /// against the injected store (they need no ctx).
    async fn resolve(&mut self, ctx: &dyn Ctx, read: PendingRead) {
        match read {
            PendingRead::Root(target) => {
                let answer = ctx.module_root(&target).map(|r| r.as_bytes().to_vec());
                self.roots.insert(target, answer);
            }
            PendingRead::Query(target, req) => {
                let answer = ctx.query(&target, &req).await.map_err(to_wit_error);
                self.queries.insert((target, req), answer);
            }
            PendingRead::State(_) => {
                unreachable!("state reads resolve against the injected store, never the ctx")
            }
        }
    }
}

#[derive(Default)]
struct HostData {
    env: Option<WitEnv>,
    committed: BTreeMap<Vec<u8>, Vec<u8>>,
    staged: BTreeMap<Vec<u8>, Option<Vec<u8>>>,
    memo: SiblingMemo,
    /// set when the guest hits a read the memo can't answer: the run is paused
    /// (host trap) and the driver resolves + replays. always `None` when a run
    /// finishes cleanly.
    pending: Option<PendingRead>,
    /// a ctx-less run (plain [`Module::query`]) has no SIBLING resolver: a memo
    /// miss on module-root/query-module answers the pre-cutover stub surface
    /// (root `None`, query `unsupported`) instead of pausing the run. state
    /// reads are NOT sealed — the injected store is the module's own state and
    /// is always resolvable, ctx or not.
    sealed: bool,
    /// committed state lives in an injected [`MerkleStore`], not `committed`:
    /// a `state-get` miss (staged, then memo) pauses the run for the driver to
    /// resolve `store.get` and replay. `committed` stays empty in this mode.
    store_backed: bool,
    out_msgs: Vec<(String, Vec<u8>)>,
    out_events: Vec<(String, Vec<u8>)>,
}

impl host::Host for HostData {
    fn get_env(&mut self) -> WitEnv {
        self.env.clone().expect("env is set before every dispatch")
    }
    /// overlay-over-committed read: a staged `Some` is a set, staged `None` a
    /// delete, absence falls through to committed — the map directly, or the
    /// injected store via memoized replay.
    fn state_get(&mut self, key: Vec<u8>) -> wasmtime::Result<Option<Vec<u8>>> {
        if let Some(overlay) = self.staged.get(&key) {
            return Ok(overlay.clone());
        }
        if !self.store_backed {
            return Ok(self.committed.get(&key).cloned());
        }
        if let Some(answer) = self.memo.states.get(&key) {
            return Ok(answer.clone());
        }
        self.pending = Some(PendingRead::State(key));
        Err(wasmtime::Error::msg(PENDING_READ_TRAP))
    }
    fn state_set(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.staged.insert(key, Some(value));
    }
    fn state_delete(&mut self, key: Vec<u8>) {
        self.staged.insert(key, None);
    }
    fn module_root(&mut self, target: String) -> wasmtime::Result<Option<Vec<u8>>> {
        if let Some(answer) = self.memo.roots.get(&target) {
            return Ok(answer.clone());
        }
        if self.sealed {
            return Ok(None);
        }
        self.pending = Some(PendingRead::Root(target));
        Err(wasmtime::Error::msg(PENDING_READ_TRAP))
    }
    fn query_module(
        &mut self,
        target: String,
        req: Vec<u8>,
    ) -> wasmtime::Result<Result<Vec<u8>, WitError>> {
        let key = (target, req);
        if let Some(answer) = self.memo.queries.get(&key) {
            return Ok(answer.clone());
        }
        if self.sealed {
            return Ok(Err(WitError::Unsupported));
        }
        self.pending = Some(PendingRead::Query(key.0, key.1));
        Err(wasmtime::Error::msg(PENDING_READ_TRAP))
    }
    fn emit_msg(&mut self, target: String, payload: Vec<u8>) {
        self.out_msgs.push((target, payload));
    }
    fn emit_event(&mut self, source: String, payload: Vec<u8>) {
        self.out_events.push((source, payload));
    }
}

/// map a host-ctx read error onto the deterministic wit error surface a guest
/// sees. every arm is host-computed and identical on all validators.
fn to_wit_error(e: SdkError) -> WitError {
    match e {
        SdkError::UnknownModule(_) => WitError::NotFound,
        SdkError::QueryUnsupported | SdkError::SyncUnsupported | SdkError::SwapUnsupported => {
            WitError::Unsupported
        }
        SdkError::Module(m) => WitError::Rejected(m),
        other => WitError::Rejected(other.to_string()),
    }
}

// ============================================================================
// the wasm module
// ============================================================================

/// where a wasm tenant's COMMITTED state lives. the staged overlay, the
/// commit/abort boundary, and the whole dispatch machinery are identical in
/// both modes — only the committed substrate (and therefore `root()` and the
/// sync surface) differs.
enum StateBacking {
    /// the original host-KV map: root is sha256 over the canonical encoding,
    /// sync is installable snapshot bytes ([`WasmModule::snapshot`] /
    /// [`WasmModule::install`]).
    Map {
        committed: BTreeMap<Vec<u8>, Vec<u8>>,
    },
    /// a host-injected authenticated store (qmdb in production): root is the
    /// store's merkle root, sync is the store's resolver lane. snapshot/install
    /// do not apply — a joiner rebuilds the CONCRETE store (`sync_from`) and
    /// wraps a fresh module around it, exactly like a native store-backed
    /// module.
    Store { store: Box<dyn MerkleStore> },
}

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
    backing: StateBacking,
    staged: BTreeMap<Vec<u8>, Option<Vec<u8>>>,
    fuel: u64,
    /// the canonical committed-state revision this tenant declares (see
    /// [`Module::state_schema_revision`]). a byte-compatible port keeps the
    /// native module's revision; a port that changes the state layout bumps it.
    state_schema_revision: u32,
    /// serve QUERY rounds from committed state ALONE — the staged overlay is
    /// dropped for the read (execute rounds are untouched). opt-in, for a ported
    /// native module whose query surface was committed-only regardless of caller
    /// (e.g. dispatch, whose between-block delivery injection must never observe a
    /// same-block staged write). off by default: every other tenant keeps the
    /// read-your-writes query surface.
    committed_queries: bool,
}

impl WasmModule {
    fn load(id: ModuleId, component_bytes: &[u8], backing: StateBacking) -> Result<Self, SdkError> {
        let engine = Engine::new(&deterministic_config()).map_err(module_err)?;
        let component = Component::from_binary(&engine, component_bytes).map_err(module_err)?;
        let mut linker = Linker::new(&engine);
        ModuleWorld::add_to_linker::<HostData, HasSelf<HostData>>(&mut linker, |d| d)
            .map_err(module_err)?;
        Ok(Self {
            id,
            engine,
            linker,
            component,
            code_hash: sha256(component_bytes),
            backing,
            staged: BTreeMap::new(),
            fuel: DEFAULT_FUEL,
            state_schema_revision: 1,
            committed_queries: false,
        })
    }

    /// Load a module from component bytes with an empty host-KV state store.
    pub fn from_bytes(id: impl Into<ModuleId>, component_bytes: &[u8]) -> Result<Self, SdkError> {
        Self::load(
            id.into(),
            component_bytes,
            StateBacking::Map {
                committed: BTreeMap::new(),
            },
        )
    }

    /// Load a module from component bytes over a host-injected authenticated
    /// store — committed state lives in `store` (already opened, or already
    /// synced to a verified root), `root()` is the store's merkle root, and
    /// sync is the store's resolver lane. this is the STORE-BACKED port shape:
    /// a native module written over `Box<dyn MerkleStore>` compiles into the
    /// guest and drives the very same store through the wit `state-*` imports,
    /// so the cutover is root-continuous.
    pub fn with_store(
        id: impl Into<ModuleId>,
        component_bytes: &[u8],
        store: Box<dyn MerkleStore>,
    ) -> Result<Self, SdkError> {
        Self::load(id.into(), component_bytes, StateBacking::Store { store })
    }

    fn is_store_backed(&self) -> bool {
        matches!(self.backing, StateBacking::Store { .. })
    }

    /// resolve one paused committed-store read: 32-byte digests are the only
    /// key shape the store speaks (every store-backed module hashes its logical
    /// keys before the wit boundary), so anything else is rejected — a
    /// deterministic refusal, identical on every validator, never a fork.
    async fn resolve_state_read(&self, key: &[u8]) -> Result<Option<Vec<u8>>, SdkError> {
        let StateBacking::Store { store } = &self.backing else {
            unreachable!("map-backed runs never pause on state reads");
        };
        let digest: &[u8; ROOT_LEN] = key.try_into().map_err(|_| {
            SdkError::Module(format!(
                "store-backed state keys must be {ROOT_LEN}-byte digests, got {}",
                key.len()
            ))
        })?;
        store.get(digest).await
    }

    /// declare a non-default canonical-state revision (the recovery/state-sync
    /// schema fence). a ported module that KEPT its native predecessor's byte
    /// encoding keeps its revision (directory: 1); a port that changed the
    /// layout must bump it in the same change, exactly like a native module.
    pub fn with_state_schema_revision(mut self, revision: u32) -> Self {
        self.state_schema_revision = revision;
        self
    }

    /// serve this tenant's QUERY rounds from committed state ALONE — drop the
    /// staged overlay for the read (execute rounds keep their read-your-writes
    /// stage). the opt-in for a ported native module whose query surface was
    /// committed-only regardless of caller: dispatch answers `Module::query` from
    /// committed state so a same-block sibling read never observes an uncommitted
    /// write (the between-block delivery injection depends on it). every other
    /// tenant leaves this off and keeps the read-your-writes query surface.
    pub fn with_committed_queries(mut self) -> Self {
        self.committed_queries = true;
        self
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
    /// map-backed only: a store-backed tenant has no byte snapshot (its sync
    /// surface is the store's resolver lane), so asking for one is a host
    /// wiring bug and panics loud.
    pub fn snapshot(&self) -> Vec<u8> {
        match &self.backing {
            StateBacking::Map { committed } => Self::encode_state(committed),
            StateBacking::Store { .. } => {
                panic!("a store-backed wasm module has no byte snapshot — sync the store")
            }
        }
    }

    /// verify-then-adopt a peer/checkpoint snapshot: strict-decode, recompute
    /// the root, refuse on mismatch — committed state and stage untouched on any
    /// error. code is NOT part of the snapshot (the registry owns the code
    /// commitment; the host reconciles running code separately), so install
    /// never touches the loaded component. map-backed only: a store-backed
    /// tenant adopts state by rebuilding its CONCRETE store (`sync_from`) and
    /// wrapping a fresh module around it, so install refuses.
    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), SdkError> {
        let StateBacking::Map { committed } = &mut self.backing else {
            return Err(SdkError::Module(
                "a store-backed wasm module adopts state through its injected store, not install"
                    .into(),
            ));
        };
        let decoded = decode_state(bytes)?;
        if Self::root_of(&decoded) != expected {
            return Err(SdkError::Module("snapshot root mismatch".into()));
        }
        *committed = decoded;
        self.staged.clear();
        Ok(())
    }

    /// this round's copy of map-backed committed state. store-backed rounds
    /// carry an EMPTY map — their committed reads pause and resolve through
    /// the injected store instead.
    fn committed_for_round(&self) -> BTreeMap<Vec<u8>, Vec<u8>> {
        match &self.backing {
            StateBacking::Map { committed } => committed.clone(),
            StateBacking::Store { .. } => BTreeMap::new(),
        }
    }

    /// one round of the guest's `query` export over LIVE state — the staged
    /// overlay on committed, the same read-your-writes surface a native module's
    /// query serves from its live struct (out of block the overlay is empty, so
    /// this is the committed projection). writes a guest attempts here land in
    /// the round's own copy and are dropped: read-only by construction. returns
    /// the outcome plus the memo and any pending read the round paused on.
    fn query_round(
        &self,
        env: WitEnv,
        memo: SiblingMemo,
        sealed: bool,
        req: &[u8],
    ) -> (Result<Vec<u8>, SdkError>, SiblingMemo, Option<PendingRead>) {
        // committed-only tenants (opt-in) answer queries from committed state
        // alone: the staged overlay is dropped for this read. execute rounds are
        // untouched — and a query round never writes, so an empty stage is a pure
        // read-view change, not a loss of read-your-writes.
        let staged = if self.committed_queries {
            BTreeMap::new()
        } else {
            self.staged.clone()
        };
        let data = HostData {
            env: Some(env),
            committed: self.committed_for_round(),
            staged,
            memo,
            pending: None,
            sealed,
            store_backed: self.is_store_backed(),
            out_msgs: Vec::new(),
            out_events: Vec::new(),
        };
        let mut store = Store::new(&self.engine, data);
        let call: Result<Result<Vec<u8>, WitError>, SdkError> = match store.set_fuel(self.fuel) {
            Err(e) => Err(module_err(e)),
            Ok(()) => match ModuleWorld::instantiate(&mut store, &self.component, &self.linker) {
                Err(e) => Err(module_err(e)),
                Ok(inst) => inst.call_query(&mut store, req).map_err(module_err),
            },
        };
        let data = store.into_data();
        let outcome = call.and_then(|r| r.map_err(wit_err));
        (outcome, data.memo, data.pending)
    }
}

/// canonical `install`-able bytes (and their root) for a host-COMPUTED initial
/// store — how the host seeds a wasm tenant with state at construction, e.g.
/// the `sdk::genesis_config` `__config` entry carrying per-network genesis
/// parameters (a fixed component cannot compile them in). the encoding is the
/// exact [`WasmModule::encode_state`] shape (count + sorted len-prefixed
/// pairs), so `initial_state(entries)` feeds straight into
/// [`WasmModule::install`]: deterministic, sorted, one store per entry set.
/// duplicate keys are a wiring bug and panic rather than silently collapse.
pub fn initial_state(entries: &[(&[u8], &[u8])]) -> (Vec<u8>, StateRoot) {
    let mut committed = BTreeMap::new();
    for (key, value) in entries {
        assert!(
            committed.insert(key.to_vec(), value.to_vec()).is_none(),
            "initial_state entries must have unique keys"
        );
    }
    (
        WasmModule::encode_state(&committed),
        WasmModule::root_of(&committed),
    )
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

/// The determinism envelope for module execution: fuel-metered termination, no
/// ambient imports, canonical NaNs, and every wasm proposal the integer/bytes
/// component ABI does not need switched OFF — the envelope is identical on
/// every validator, so the same guest bytes behave identically everywhere.
///
/// Kept ON (the componentized-Rust baseline): bulk-memory, multi-value,
/// reference-types (LLVM output uses funcref tables), and multi-memory
/// (component adapters). All are deterministic.
fn deterministic_config() -> Config {
    let mut c = Config::new();
    c.wasm_component_model(true);
    c.consume_fuel(true);
    // float ops emit ONE canonical NaN bit pattern: a guest computing floats
    // can never leak host-hardware NaN payloads into state or the app-hash.
    c.cranelift_nan_canonicalization(true);
    c.wasm_simd(false);
    c.wasm_relaxed_simd(false);
    c.wasm_threads(false);
    c.wasm_shared_everything_threads(false);
    c.wasm_gc(false);
    c.wasm_function_references(false);
    c.wasm_memory64(false);
    c.wasm_tail_call(false);
    c.wasm_stack_switching(false);
    c.wasm_custom_page_sizes(false);
    c.wasm_wide_arithmetic(false);
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

    /// map mode: sha256 over the canonical host-KV encoding. store mode: the
    /// injected store's REAL merkle root, verbatim — the same value the native
    /// module computed pre-cutover, so the app-hash is continuous.
    fn root(&self) -> StateRoot {
        match &self.backing {
            StateBacking::Map { committed } => Self::root_of(committed),
            StateBacking::Store { store } => store.root(),
        }
    }

    fn state_schema_revision(&self) -> u32 {
        self.state_schema_revision
    }

    fn code_hash(&self) -> Option<Vec<u8>> {
        Some(self.code_hash.clone())
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, SdkError> {
        match &self.backing {
            StateBacking::Map { .. } => Ok(StateSyncHandle::SnapshotBytes(self.snapshot())),
            // verbatim what the native store-backed modules (pages, chat, kv)
            // declared: sync rides the store's resolver lane, not byte
            // snapshots.
            StateBacking::Store { .. } => Ok(StateSyncHandle::ResolverBacked {
                backend: "qmdb".into(),
                detail: "serve_sync answers qmdb op-range requests (statesync wire)".into(),
            }),
        }
    }

    /// the network state-sync serve lane of a store-backed tenant: answers the
    /// shared qmdb wire requests from committed state, read-only. map-backed
    /// tenants keep the default non-coverage (their sync surface is snapshot
    /// bytes).
    async fn serve_sync(&self, req: &[u8]) -> Result<Vec<u8>, SdkError> {
        match &self.backing {
            StateBacking::Map { .. } => Err(SdkError::SyncUnsupported),
            StateBacking::Store { store } => store.serve_sync(req).await,
        }
    }

    async fn resolver_sync_target(&self) -> Result<ResolverSyncTarget, SdkError> {
        match &self.backing {
            StateBacking::Map { .. } => Err(SdkError::SyncUnsupported),
            StateBacking::Store { store } => store.sync_target().await,
        }
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
        let env = to_wit_env(ctx.env());
        // every replay round re-runs the pure guest over the SAME pre-dispatch
        // stage: an aborted round's writes must not leak into the next, or a
        // replay could observe (e.g. double-apply) its own discarded effects.
        let staged0 = std::mem::take(&mut self.staged);
        let mut memo = SiblingMemo::default();
        while memo.within_budgets() {
            // move map-backed committed + memo into owned per-round data;
            // staged is a copy. store-backed rounds carry an empty map and
            // resolve committed reads through the injected store instead.
            let round_committed = match &mut self.backing {
                StateBacking::Map { committed } => std::mem::take(committed),
                StateBacking::Store { .. } => BTreeMap::new(),
            };
            let data = HostData {
                env: Some(env.clone()),
                committed: round_committed,
                staged: staged0.clone(),
                memo: std::mem::take(&mut memo),
                pending: None,
                sealed: false,
                store_backed: self.is_store_backed(),
                out_msgs: Vec::new(),
                out_events: Vec::new(),
            };
            let mut store = Store::new(&self.engine, data);

            let call: Result<Result<(), WitError>, SdkError> = match store.set_fuel(self.fuel) {
                Err(e) => Err(module_err(e)),
                Ok(()) => match ModuleWorld::instantiate(&mut store, &self.component, &self.linker)
                {
                    Err(e) => Err(module_err(e)),
                    Ok(inst) => inst.call_execute(&mut store, &msg.payload).map_err(module_err),
                },
            };

            // reclaim state regardless of outcome (a trap leaves the moved-in
            // state in the store; take it back so the module is never left empty).
            let data = store.into_data();
            if let StateBacking::Map { committed } = &mut self.backing {
                *committed = data.committed;
            }
            memo = data.memo;

            // a paused run: resolve the read (own store, or host ctx) and replay.
            if let Some(read) = data.pending {
                match read {
                    PendingRead::State(key) => match self.resolve_state_read(&key).await {
                        Ok(answer) => {
                            memo.states.insert(key, answer);
                        }
                        // a refused store read (bad key shape, store error) is
                        // a deterministic rejection of the whole op.
                        Err(e) => {
                            self.staged = staged0;
                            return Err(e);
                        }
                    },
                    read => memo.resolve(&*ctx, read).await,
                }
                continue;
            }

            return match call {
                Ok(Ok(())) => {
                    self.staged = data.staged;
                    // only a clean execute publishes its intents; a rejection leaks nothing.
                    for (target, payload) in data.out_msgs {
                        ctx.emit_msg(Msg { target, payload });
                    }
                    for (source, payload) in data.out_events {
                        ctx.emit_event(Event { source, payload });
                    }
                    Ok(())
                }
                // a rejected op stages nothing: the pre-dispatch overlay is
                // restored (the host aborts the whole block on any execute
                // error, so this only keeps the module-local invariant clean).
                Ok(Err(e)) => {
                    self.staged = staged0;
                    Err(wit_err(e))
                }
                Err(e) => {
                    self.staged = staged0;
                    Err(e)
                }
            };
        }
        self.staged = staged0;
        Err(memo.budget_error())
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, SdkError> {
        // ctx-less direct read: no SIBLING resolver, so module-root/query-module
        // answer the sealed stub surface (root `None`, query `unsupported`) —
        // host-routed reads go through `query_with` instead, which resolves
        // them for real. committed-STORE reads still replay (the injected store
        // is this module's own state; no ctx needed).
        let env = WitEnv {
            height: 0,
            consensus_time: 0,
            protocol_version: 0,
            me: self.id.clone(),
            origin: WitOrigin::System,
        };
        let mut memo = SiblingMemo::default();
        while memo.within_budgets() {
            let (outcome, returned, pending) = self.query_round(env.clone(), memo, true, req);
            memo = returned;
            match pending {
                None => return outcome,
                Some(PendingRead::State(key)) => {
                    let answer = self.resolve_state_read(&key).await?;
                    memo.states.insert(key, answer);
                }
                Some(_) => unreachable!("sealed runs never pause on sibling reads"),
            }
        }
        Err(memo.budget_error())
    }

    async fn query_with(&self, ctx: &dyn Ctx, req: &[u8]) -> Result<Vec<u8>, SdkError> {
        let mut memo = SiblingMemo::default();
        while memo.within_budgets() {
            let (outcome, returned, pending) =
                self.query_round(to_wit_env(ctx.env()), memo, false, req);
            memo = returned;
            match pending {
                None => return outcome,
                Some(PendingRead::State(key)) => {
                    let answer = self.resolve_state_read(&key).await?;
                    memo.states.insert(key, answer);
                }
                Some(read) => memo.resolve(ctx, read).await,
            }
        }
        Err(memo.budget_error())
    }

    async fn commit_block(&mut self) -> Result<(), SdkError> {
        match &mut self.backing {
            StateBacking::Map { committed } => {
                for (key, overlay) in std::mem::take(&mut self.staged) {
                    match overlay {
                        Some(value) => {
                            committed.insert(key, value);
                        }
                        None => {
                            committed.remove(&key);
                        }
                    }
                }
            }
            StateBacking::Store { store } => {
                // publish the whole block's staged writes in ONE store batch —
                // exactly the native store-backed contract: no-op (and no root
                // movement) when nothing staged, `None` ships as a delete. the
                // staged map orders by hashed key while a native module orders
                // by logical key, but the store's batch canonicalizes mutations
                // by key before merkleizing, so the committed op log — and the
                // root — is identical either way. keys were validated as
                // digests when staged content arrived from the guest; a
                // non-digest key here fails closed before any store touch.
                if self.staged.is_empty() {
                    return Ok(());
                }
                let mut writes: Vec<([u8; ROOT_LEN], Option<Vec<u8>>)> =
                    Vec::with_capacity(self.staged.len());
                for (key, value) in &self.staged {
                    let digest: [u8; ROOT_LEN] = key.as_slice().try_into().map_err(|_| {
                        SdkError::Module(format!(
                            "store-backed state keys must be {ROOT_LEN}-byte digests, got {}",
                            key.len()
                        ))
                    })?;
                    writes.push((digest, value.clone()));
                }
                store.commit_batch(writes).await?;
                self.staged.clear();
            }
        }
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), SdkError> {
        self.staged.clear();
        Ok(())
    }
}
