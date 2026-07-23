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
//! COMMITTED state has three backings ([`StateBacking`]):
//!   * `Map` — the original host-KV `BTreeMap`, whose root is sha256 over the
//!     canonical encoding and whose sync surface is installable snapshot bytes.
//!   * `Odb` — a host-side duckfs substrate ([`OdbBacking`], the files port):
//!     the committed state is a single refs image the guest sees through the
//!     [`REFS_KEY`] state lane, `root()` is `sha256(refs_bytes())` (the refs
//!     image, byte-identical to native files — NOT the KV encoding), and the
//!     object plane, queries, and state-sync all delegate to the backing. at the
//!     block boundary the kernel publishes the staged objects into the backing
//!     BEFORE adopting the refs image (the duckfs crash-safety ordering).
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
            // object reads pause on a memo miss exactly like the sibling reads:
            // the driver resolves them against the odb backing and replays.
            // `object-put` is NOT trappable — the host computes the id purely
            // and returns it, so it can never fail.
            "ducktape:module/host.object-stat": trappable,
            "ducktape:module/host.object-get": trappable,
        },
    });
}

use bindings::Module as ModuleWorld;
use bindings::ducktape::module::host::{
    self, Env as WitEnv, Error as WitError, Origin as WitOrigin,
};

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

/// per-dispatch bound on DISTINCT object-plane reads (`object-stat` +
/// `object-get` misses of the same-dispatch put overlay). mirrors
/// [`MAX_STORE_READS`]: a content-addressed op walks many objects (a file's
/// chunks, a tree's entries), so object reads carry the same larger budget as
/// own-state reads. a protocol constant — an op that needs more is rejected
/// identically on every validator. only tenants that call the object imports
/// (the files guest) ever accrue against it; every other tenant leaves it at 0.
pub const MAX_OBJECT_READS: usize = 4096;

/// the host-side content-addressed object store a wasm files tenant reads from
/// and stages puts against. Task 1 ships only the trait + the plumbing that
/// routes the object imports here; NO backing is wired (`WasmModule::odb` is
/// `None`), so every read that misses the same-dispatch put overlay answers
/// `None`. Task 2 implements this over `DiskStore`/`DiskRefs` and injects it via
/// a builder. Existing Map/Store tenants never call the object imports, so they
/// never reach a resolver that would consult it — the plane is inert for them.
pub trait HostOdb {
    /// metadata-only: `(kind-tag, body-byte-length)` of a refs-reachable
    /// object, or `None` if absent. answered from metadata alone (map lookup /
    /// file stat), never a full body read on the consensus path.
    fn stat(&self, id: &[u8]) -> Option<(u8, u64)>;
    /// the TAGGED body (`kind-tag ‖ body`) of a refs-reachable object, or
    /// `None` if absent — the exact bytes the guest's `object-get` returns.
    fn get(&self, id: &[u8]) -> Option<Vec<u8>>;
    /// stage a put; the host computes `id = sha256(kind-tag ‖ body)`. the
    /// staged object is visible to same-block stats/gets and published or
    /// discarded at the block boundary by the backing.
    fn stage_put(&mut self, kind: u8, body: &[u8]) -> [u8; 32];
}

/// the host-side substrate a ROOT-CONTINUOUS files tenant ([`StateBacking::Odb`])
/// delegates its committed surface to — a native duckfs `Fs` over its disk odb +
/// durable refs file (Task 4), or the in-memory mock the kernel tests drive.
///
/// the crux is `root()` = `StateRoot(sha256(refs_bytes()))` — the canonical refs
/// image, NOT the host-KV encoding — so a wasm files tenant's app-hash is
/// byte-identical to native files' `sha256(encode_refs)` and the cutover moves
/// no root. the guest sees the refs image through the ordinary `state-*` lane
/// under [`REFS_KEY`] (state-get serves the committed image staged-over, state-set
/// stages a new one); the object plane rides [`HostOdb`] (the supertrait). at the
/// block boundary the kernel drives the two boundary hooks in the duckfs
/// durability order — staged objects published FIRST, then the refs image adopted
/// — the crash-safety contract Task 4 realizes on disk (native `module.rs:368-427`).
pub trait OdbBacking: HostOdb {
    /// the committed refs image — the `root()` preimage and the snapshot bytes.
    /// byte-identical to native `Fs::snapshot_refs` / `encode_refs`.
    fn refs_bytes(&self) -> Vec<u8>;
    /// adopt a refs image as the new committed refs (the root moves here). the
    /// image is consensus-validated (a committed block's staged refs) or
    /// root-verified (an installed snapshot); the backing swaps + durably saves
    /// it (native `Fs::adopt_refs` + `DiskRefs::save`). NOT trusted to verify —
    /// [`WasmModule::install`] checks the root before calling this.
    fn adopt_refs(&mut self, bytes: &[u8]) -> Result<(), SdkError>;
    /// the objects-durable barrier of the block boundary: make the objects
    /// received via [`HostOdb::stage_put`] this block durable (native
    /// `store.sync_dirs`). the kernel calls this BEFORE `adopt_refs`, so the
    /// refs commit point can never precede the objects it references.
    ///
    /// `height` is the committing block's height, captured by the kernel during
    /// `execute` (a committed block always ran at least one dispatch). the
    /// backing records it here and stamps it into the refs envelope at
    /// [`OdbBacking::adopt_refs`] — native saves refs+height in one
    /// `DiskRefs::save`, the kernel splits publish from adopt so the backing
    /// recombines them. a disk backing needs it for the durable-height recovery
    /// bookkeeping; the in-memory mock ignores it.
    fn publish_block(&mut self, height: u64) -> Result<(), SdkError>;
    /// drop this block's staged objects without publishing (native
    /// `Fs::abort_block` drops the in-memory pending; a disk backing may also
    /// sweep orphan object files). the committed refs + odb stay untouched.
    fn discard_block(&mut self);
    /// serve a committed-only query — the files read lane is HOST-side (never the
    /// guest) so an in-block sibling `FilesQuery::Refs` reads committed refs+odb,
    /// byte-identical to native `Fs::query` (`fs.rs:601-605`). off the execute
    /// path, so disk body reads are fine here.
    fn query(&self, req: &[u8]) -> Result<Vec<u8>, SdkError>;
    /// serve one committed-only state-sync request — the duckfs object-possession
    /// protocol (native `Fs::serve_sync`). the delegation twin of a store-backed
    /// tenant's `MerkleStore::serve_sync`.
    fn serve_sync(&self, req: &[u8]) -> Result<Vec<u8>, SdkError>;
    /// the last block height whose committed refs are durable, or `None` on a
    /// fresh substrate with no envelope yet — the per-commit recovery cursor
    /// [`Module::durable_commit_height`] surfaces so trailing-block recovery can
    /// verify a disk substrate that committed a block whose journal seal was lost
    /// to a crash. This keeps the files recovery path verifiable.
    fn durable_commit_height(&self) -> Option<u64>;
}

/// the single reserved state-lane key an ODB-backed (files) tenant reads and
/// writes its whole refs image under. the guest loads the committed refs via
/// `state-get(REFS_KEY)` and stages a new image via `state-set(REFS_KEY, ..)`;
/// the host seeds each execute round's committed view with exactly this one
/// entry (= `backing.refs_bytes()`) and, at commit, adopts the staged value.
/// reuses the `__state` single-value whole-state convention — but `root()` is
/// `sha256(refs_bytes)`, NOT the KV encoding, so there is no `__root` twin. the
/// files guest (Task 3) MUST read/write refs under this exact key.
pub const REFS_KEY: &[u8] = b"__state";

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
    /// an `object-stat` miss of the same-dispatch put overlay: the driver
    /// resolves it against the odb backing (`None` until Task 2 wires one) and
    /// replays. own-state-shaped, so it resolves without a ctx like `State`.
    ObjectStat(Vec<u8>),
    /// an `object-get` miss of the same-dispatch put overlay: resolved against
    /// the odb backing, exactly like [`PendingRead::ObjectStat`].
    ObjectGet(Vec<u8>),
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
    /// odb-backing answers for object reads. the same-dispatch put overlay
    /// shadows these at `object-stat`/`object-get` (overlay first), so a stale
    /// entry is unreachable. counted together against [`MAX_OBJECT_READS`].
    object_stats: BTreeMap<Vec<u8>, Option<(u8, u64)>>,
    object_gets: BTreeMap<Vec<u8>, Option<Vec<u8>>>,
}

impl SiblingMemo {
    /// DISTINCT sibling reads so far (the [`MAX_SIBLING_READS`] budget); the
    /// store-read and object-read budgets are tracked separately.
    fn len(&self) -> usize {
        self.roots.len() + self.queries.len()
    }

    /// DISTINCT object-plane reads so far (the [`MAX_OBJECT_READS`] budget):
    /// stats and gets share one budget, like roots and queries share the
    /// sibling budget.
    fn object_len(&self) -> usize {
        self.object_stats.len() + self.object_gets.len()
    }

    /// every replay budget still holds — the loop guard every driver shares.
    fn within_budgets(&self) -> bool {
        self.len() <= MAX_SIBLING_READS
            && self.states.len() <= MAX_STORE_READS
            && self.object_len() <= MAX_OBJECT_READS
    }

    /// the deterministic rejection for a blown replay budget.
    fn budget_error(&self) -> SdkError {
        if self.len() > MAX_SIBLING_READS {
            SdkError::Module(format!(
                "sibling-read budget exceeded ({MAX_SIBLING_READS})"
            ))
        } else if self.states.len() > MAX_STORE_READS {
            SdkError::Module(format!("store-read budget exceeded ({MAX_STORE_READS})"))
        } else {
            SdkError::Module(format!("object-read budget exceeded ({MAX_OBJECT_READS})"))
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
            PendingRead::ObjectStat(_) | PendingRead::ObjectGet(_) => {
                unreachable!("object reads resolve against the odb backing, never the ctx")
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
    /// miss on module-root/query-module answers the ctx-less stub surface
    /// (root `None`, query `unsupported`) instead of pausing the run. state
    /// reads are NOT sealed — the injected store is the module's own state and
    /// is always resolvable, ctx or not.
    sealed: bool,
    /// committed state lives in an injected [`MerkleStore`], not `committed`:
    /// a `state-get` miss (staged, then memo) pauses the run for the driver to
    /// resolve `store.get` and replay. `committed` stays empty in this mode.
    store_backed: bool,
    /// this dispatch's staged object puts, keyed by id → the TAGGED body
    /// (`kind ‖ body`). seeded from the block-level accumulator each round and
    /// grown by `object-put`; read BEFORE the memo/backing so a just-put id
    /// answers `object-stat`/`object-get` without a pause (read-your-writes
    /// within the block). a clean dispatch promotes it back to the block
    /// accumulator; an aborted dispatch drops it.
    object_puts: BTreeMap<Vec<u8>, Vec<u8>>,
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
    /// overlay-over-backing metadata read: this dispatch's staged puts first
    /// (kind = tag byte, len = body length), then the memo, else pause for the
    /// driver to resolve against the odb backing. never sealed — the object
    /// store is this module's own state, resolvable ctx or not, like `state`.
    fn object_stat(&mut self, id: Vec<u8>) -> wasmtime::Result<Option<(u8, u64)>> {
        if let Some(tagged) = self.object_puts.get(&id) {
            // a staged put always carries at least its kind tag byte.
            return Ok(Some((tagged[0], (tagged.len() - 1) as u64)));
        }
        if let Some(answer) = self.memo.object_stats.get(&id) {
            return Ok(*answer);
        }
        self.pending = Some(PendingRead::ObjectStat(id));
        Err(wasmtime::Error::msg(PENDING_READ_TRAP))
    }
    /// overlay-over-backing full read: staged puts (the tagged body verbatim)
    /// first, then the memo, else pause for the driver.
    fn object_get(&mut self, id: Vec<u8>) -> wasmtime::Result<Option<Vec<u8>>> {
        if let Some(tagged) = self.object_puts.get(&id) {
            return Ok(Some(tagged.clone()));
        }
        if let Some(answer) = self.memo.object_gets.get(&id) {
            return Ok(answer.clone());
        }
        self.pending = Some(PendingRead::ObjectGet(id));
        Err(wasmtime::Error::msg(PENDING_READ_TRAP))
    }
    /// stage a put: the host computes `id = sha256(kind ‖ body)` and returns it
    /// ALONE (a hash mismatch is impossible here — the fail-closed publish check
    /// rides the disk backing's staged→published seam). the tagged body lands
    /// in this round's overlay so a later stat/get of `id` answers immediately.
    fn object_put(&mut self, kind: u8, body: Vec<u8>) -> Vec<u8> {
        let mut tagged = Vec::with_capacity(1 + body.len());
        tagged.push(kind);
        tagged.extend_from_slice(&body);
        let id = sha256(&tagged);
        self.object_puts.insert(id.clone(), tagged);
        id
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
    /// a host-side duckfs substrate ([`OdbBacking`]): root is
    /// `sha256(refs_bytes())` (the refs image, NOT a KV encoding), the committed
    /// state is a single refs image the guest sees through the [`REFS_KEY`]
    /// state lane, and the object plane + queries + sync all delegate to the
    /// backing. this is the ROOT-CONTINUOUS files port shape (native files'
    /// `sha256(encode_refs)` root, verbatim). the boxed backing IS this tenant's
    /// [`HostOdb`] too (`OdbBacking: HostOdb`), so object reads resolve against it.
    Odb { backing: Box<dyn OdbBacking> },
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
    /// this block's accumulated staged object puts (id → tagged body), across
    /// every dispatch since the last commit/abort — the object-plane twin of
    /// `staged`. seeds each dispatch's overlay; published (Task 2) or dropped at
    /// the block boundary. always empty for non-object tenants.
    staged_objects: BTreeMap<Vec<u8>, Vec<u8>>,
    fuel: u64,
    /// serve QUERY rounds from committed state ALONE — the staged overlay is
    /// dropped for the read (execute rounds are untouched). opt-in, for a ported
    /// native module whose query surface was committed-only regardless of caller
    /// (e.g. dispatch, whose between-block delivery injection must never observe a
    /// same-block staged write). off by default: every other tenant keeps the
    /// read-your-writes query surface.
    committed_queries: bool,
    /// the height of the block currently staging, captured each `execute` and
    /// consumed at the block boundary. only [`StateBacking::Odb`] reads it — it
    /// threads to [`OdbBacking::publish_block`] so a disk backing can stamp the
    /// durable-height into its refs envelope (the recovery bookkeeping the native
    /// module records atomically with the refs). `None` between blocks / when no
    /// dispatch has run this block.
    block_height: Option<u64>,
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
            staged_objects: BTreeMap::new(),
            fuel: DEFAULT_FUEL,
            committed_queries: false,
            block_height: None,
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

    /// Load a module from component bytes over a host-side duckfs substrate —
    /// committed state is the backing's refs image, `root()` is
    /// `sha256(refs_bytes())` (byte-identical to native files), queries and
    /// state-sync delegate to the backing, and the object plane resolves against
    /// it. this is the ROOT-CONTINUOUS files port: a native module written over
    /// duckfs's disk odb + refs file cuts over with no root movement. the boxed
    /// backing is both the committed refs owner and this tenant's [`HostOdb`].
    pub fn with_odb(
        id: impl Into<ModuleId>,
        component_bytes: &[u8],
        backing: Box<dyn OdbBacking>,
    ) -> Result<Self, SdkError> {
        Self::load(id.into(), component_bytes, StateBacking::Odb { backing })
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

    /// resolve one paused object-plane read against the odb backing and memoize
    /// the answer. only an [`StateBacking::Odb`] tenant has a backing; Map/Store
    /// tenants never call the object imports, so they never pause here and their
    /// `None` backing answers the (never-produced) read. the answers serve
    /// COMMITTED objects only — the same-block staged puts are shadowed earlier,
    /// by the [`HostData::object_puts`] overlay. synchronous (no ctx, no await),
    /// like a map-backed state read.
    fn resolve_object_read(&self, read: PendingRead, memo: &mut SiblingMemo) {
        let backing = match &self.backing {
            StateBacking::Odb { backing } => Some(backing),
            StateBacking::Map { .. } | StateBacking::Store { .. } => None,
        };
        match read {
            PendingRead::ObjectStat(id) => {
                let answer = backing.and_then(|b| b.stat(&id));
                memo.object_stats.insert(id, answer);
            }
            PendingRead::ObjectGet(id) => {
                let answer = backing.and_then(|b| b.get(&id));
                memo.object_gets.insert(id, answer);
            }
            PendingRead::Root(_) | PendingRead::Query(_, _) | PendingRead::State(_) => {
                unreachable!("resolve_object_read only handles object-plane reads")
            }
        }
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
    /// snapshot format (verify-then-adopt against the root, like modreg). The
    /// shared [`sdk::hash::encode_pairs`] IS this byte contract (the `sdk-testkit`
    /// `MemStore::root` preimage is checked against the same helper).
    fn encode_state(committed: &BTreeMap<Vec<u8>, Vec<u8>>) -> Vec<u8> {
        sdk::hash::encode_pairs(committed)
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
            // the refs image IS the snapshot (native `Fs::snapshot_refs`) — the
            // exact `root()` preimage, shipped over the duckfs-odb resolver lane.
            StateBacking::Odb { backing } => backing.refs_bytes(),
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
        match &mut self.backing {
            StateBacking::Map { committed } => {
                let decoded = decode_state(bytes)?;
                if Self::root_of(&decoded) != expected {
                    return Err(SdkError::Module("snapshot root mismatch".into()));
                }
                *committed = decoded;
                self.staged.clear();
                Ok(())
            }
            StateBacking::Store { .. } => Err(SdkError::Module(
                "a store-backed wasm module adopts state through its injected store, not install"
                    .into(),
            )),
            // verify-then-adopt the refs image (native `Fs::install_refs`): check
            // the root here, then hand the verified bytes to the backing. the
            // backing does not re-verify, so the root check is load-bearing.
            StateBacking::Odb { backing } => {
                if StateRoot(sha256_array(bytes)) != expected {
                    return Err(SdkError::Module("snapshot root mismatch".into()));
                }
                backing.adopt_refs(bytes)?;
                self.staged.clear();
                Ok(())
            }
        }
    }

    /// this round's copy of map-backed committed state. store-backed rounds
    /// carry an EMPTY map — their committed reads pause and resolve through
    /// the injected store instead.
    fn committed_for_round(&self) -> BTreeMap<Vec<u8>, Vec<u8>> {
        match &self.backing {
            StateBacking::Map { committed } => committed.clone(),
            StateBacking::Store { .. } => BTreeMap::new(),
            // the refs image is the whole committed state, served under the one
            // reserved key; the guest reads it staged-over via the state lane.
            // (queries delegate to the backing, so this only feeds execute
            // rounds — a query never instantiates the guest for this backing.)
            StateBacking::Odb { backing } => {
                BTreeMap::from([(REFS_KEY.to_vec(), backing.refs_bytes())])
            }
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
            // a query never stages puts; its object reads answer from the
            // committed backing alone (the files query lane is host-side per
            // Task 2, so the guest query never reaches the object plane).
            object_puts: BTreeMap::new(),
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
        return Err(SdkError::Module(
            "snapshot entry count exceeds buffer".into(),
        ));
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

/// sha256 as a fixed `[u8; 32]` — the [`StateBacking::Odb`] root preimage hash
/// (`sha256(refs_bytes)`), where a `StateRoot` needs the array, not a `Vec`.
fn sha256_array(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

#[async_trait::async_trait(?Send)]
impl Module for WasmModule {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// map mode: sha256 over the canonical host-KV encoding. store mode: the
    /// injected store's real Merkle root.
    fn root(&self) -> StateRoot {
        match &self.backing {
            StateBacking::Map { committed } => Self::root_of(committed),
            StateBacking::Store { store } => store.root(),
            // the ROOT-CONTINUITY crux: sha256 over the canonical refs image,
            // byte-identical to native files' `sha256(encode_refs)`. moves only
            // when the backing adopts a new image (commit/install).
            StateBacking::Odb { backing } => StateRoot(sha256_array(&backing.refs_bytes())),
        }
    }

    fn code_hash(&self) -> Option<Vec<u8>> {
        Some(self.code_hash.clone())
    }

    /// only an ODB substrate tracks a durable-commit cursor (the native files
    /// recovery bookkeeping it inherits); Map/Store tenants self-durably commit
    /// through their own stores and expose no cursor (the trait default `None`).
    /// Delegating this lets recovery verify a trailing unsealed files block.
    fn durable_commit_height(&self) -> Option<u64> {
        match &self.backing {
            StateBacking::Map { .. } | StateBacking::Store { .. } => None,
            StateBacking::Odb { backing } => backing.durable_commit_height(),
        }
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
            // byte-identical to native files' handle: the joiner fetches the refs
            // image then walks `missing_objects` -> `GetObjects` -> ingest over
            // `serve_sync` to full possession — no qmdb op-range target.
            StateBacking::Odb { .. } => Ok(StateSyncHandle::ResolverBacked {
                backend: "duckfs-odb".into(),
                detail: "refs image + GetObjects fetch to full object possession".into(),
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
            // the duckfs object-possession serve lane (native `Fs::serve_sync`),
            // committed-only, off the execute path.
            StateBacking::Odb { backing } => backing.serve_sync(req),
        }
    }

    async fn resolver_sync_target(&self) -> Result<ResolverSyncTarget, SdkError> {
        match &self.backing {
            StateBacking::Map { .. } => Err(SdkError::SyncUnsupported),
            StateBacking::Store { store } => store.sync_target().await,
            // duckfs sync is object possession, not a qmdb op-range — native
            // files declares no resolver target (the default `SyncUnsupported`).
            StateBacking::Odb { .. } => Err(SdkError::SyncUnsupported),
        }
    }

    /// Replace the component code IN PLACE, keeping the host-owned state store.
    /// This is the live-update primitive: same store, new logic, and the root is
    /// computed from the (untouched) store — so app-hash is continuous across the
    /// swap. Staged (yet uncommitted) writes are discarded: a swap is only ever
    /// driven at a clean block boundary, never mid-block.
    fn swap_code(&mut self, component_bytes: &[u8]) -> Result<(), SdkError> {
        let component =
            Component::from_binary(&self.engine, component_bytes).map_err(module_err)?;
        self.component = component;
        self.code_hash = sha256(component_bytes);
        self.staged.clear();
        self.staged_objects.clear();
        Ok(())
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), SdkError> {
        let env = to_wit_env(ctx.env());
        // capture the block height for the boundary: an Odb backing stamps it into
        // its durable-height envelope at commit. every dispatch this block carries
        // the same height, so re-setting it per dispatch is idempotent.
        self.block_height = Some(env.height);
        // every replay round re-runs the pure guest over the SAME pre-dispatch
        // stage: an aborted round's writes must not leak into the next, or a
        // replay could observe (e.g. double-apply) its own discarded effects.
        let staged0 = std::mem::take(&mut self.staged);
        // the object-plane twin of `staged0`: this block's staged puts so far.
        // each round re-seeds its overlay from this and the pure guest re-issues
        // its puts on top, so the overlay is identical across replay rounds.
        let staged_objects0 = std::mem::take(&mut self.staged_objects);
        let mut memo = SiblingMemo::default();
        while memo.within_budgets() {
            // move map-backed committed + memo into owned per-round data;
            // staged is a copy. store-backed rounds carry an empty map and
            // resolve committed reads through the injected store instead.
            let round_committed = match &mut self.backing {
                StateBacking::Map { committed } => std::mem::take(committed),
                StateBacking::Store { .. } => BTreeMap::new(),
                // seed the round with the one refs entry; the guest reads it
                // staged-over via the state lane. the backing keeps ownership of
                // the committed refs (unlike Map's move-in/reclaim), so this
                // round's copy is discarded after the call.
                StateBacking::Odb { backing } => {
                    BTreeMap::from([(REFS_KEY.to_vec(), backing.refs_bytes())])
                }
            };
            let data = HostData {
                env: Some(env.clone()),
                committed: round_committed,
                staged: staged0.clone(),
                memo: std::mem::take(&mut memo),
                pending: None,
                sealed: false,
                store_backed: self.is_store_backed(),
                object_puts: staged_objects0.clone(),
                out_msgs: Vec::new(),
                out_events: Vec::new(),
            };
            let mut store = Store::new(&self.engine, data);

            let call: Result<Result<(), WitError>, SdkError> = match store.set_fuel(self.fuel) {
                Err(e) => Err(module_err(e)),
                Ok(()) => match ModuleWorld::instantiate(&mut store, &self.component, &self.linker)
                {
                    Err(e) => Err(module_err(e)),
                    Ok(inst) => inst
                        .call_execute(&mut store, &msg.payload)
                        .map_err(module_err),
                },
            };

            // reclaim state regardless of outcome (a trap leaves the moved-in
            // state in the store; take it back so the module is never left empty).
            let data = store.into_data();
            if let StateBacking::Map { committed } = &mut self.backing {
                *committed = data.committed;
            }
            memo = data.memo;

            // a paused run: resolve the read (own store, odb backing, or host
            // ctx) and replay.
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
                            self.staged_objects = staged_objects0;
                            return Err(e);
                        }
                    },
                    read @ (PendingRead::ObjectStat(_) | PendingRead::ObjectGet(_)) => {
                        self.resolve_object_read(read, &mut memo);
                    }
                    read @ (PendingRead::Root(_) | PendingRead::Query(_, _)) => {
                        memo.resolve(&*ctx, read).await;
                    }
                }
                continue;
            }

            return match call {
                Ok(Ok(())) => {
                    self.staged = data.staged;
                    // a clean dispatch promotes its staged puts into the block
                    // accumulator (this dispatch's puts on top of the block's).
                    self.staged_objects = data.object_puts;
                    // only a clean execute publishes its intents; a rejection leaks nothing.
                    for (target, payload) in data.out_msgs {
                        ctx.emit_msg(Msg { target, payload });
                    }
                    for (source, payload) in data.out_events {
                        ctx.emit_event(Event { source, payload });
                    }
                    Ok(())
                }
                // a rejected op stages nothing: the pre-dispatch overlays are
                // restored (the host aborts the whole block on any execute
                // error, so this only keeps the module-local invariant clean).
                Ok(Err(e)) => {
                    self.staged = staged0;
                    self.staged_objects = staged_objects0;
                    Err(wit_err(e))
                }
                Err(e) => {
                    self.staged = staged0;
                    self.staged_objects = staged_objects0;
                    Err(e)
                }
            };
        }
        self.staged = staged0;
        self.staged_objects = staged_objects0;
        Err(memo.budget_error())
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, SdkError> {
        // an odb-backed (files) tenant answers queries HOST-side from committed
        // refs+odb — the read lane NEVER instantiates the guest — so an in-block
        // sibling `FilesQuery::Refs` reads committed-only, byte-identical to
        // native `Fs::query`. every other backing runs the guest's query export.
        match &self.backing {
            StateBacking::Odb { backing } => return backing.query(req),
            StateBacking::Map { .. } | StateBacking::Store { .. } => {}
        }
        // ctx-less direct read: no SIBLING resolver, so module-root/query-module
        // answer the sealed stub surface (root `None`, query `unsupported`) —
        // host-routed reads go through `query_with` instead, which resolves
        // them for real. committed-STORE reads still replay (the injected store
        // is this module's own state; no ctx needed).
        let env = WitEnv {
            height: 0,
            consensus_time: 0,
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
                // object reads are the module's own state (not sibling reads),
                // so they resolve against the backing even ctx-less, like State.
                Some(read @ (PendingRead::ObjectStat(_) | PendingRead::ObjectGet(_))) => {
                    self.resolve_object_read(read, &mut memo);
                }
                Some(PendingRead::Root(_) | PendingRead::Query(_, _)) => {
                    unreachable!("sealed runs never pause on sibling reads")
                }
            }
        }
        Err(memo.budget_error())
    }

    async fn query_with(&self, ctx: &dyn Ctx, req: &[u8]) -> Result<Vec<u8>, SdkError> {
        // odb-backed queries are host-side committed-only (see `query`); the
        // ctx (sibling reads) is unused, matching native files' standalone query.
        match &self.backing {
            StateBacking::Odb { backing } => return backing.query(req),
            StateBacking::Map { .. } | StateBacking::Store { .. } => {}
        }
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
                Some(read @ (PendingRead::ObjectStat(_) | PendingRead::ObjectGet(_))) => {
                    self.resolve_object_read(read, &mut memo);
                }
                Some(read @ (PendingRead::Root(_) | PendingRead::Query(_, _))) => {
                    memo.resolve(ctx, read).await;
                }
            }
        }
        Err(memo.budget_error())
    }

    async fn commit_block(&mut self) -> Result<(), SdkError> {
        // the committing block's height, captured during execute; consumed here.
        // `0` only if no dispatch ran this block — impossible for a touched (=
        // committing) module, and inert regardless (nothing staged → the Odb arm
        // never reaches `adopt_refs`, so the height is never persisted).
        let height = self.block_height.take().unwrap_or(0);
        match &mut self.backing {
            StateBacking::Map { committed } => {
                // Map/Store guests never stage objects; drop any (there are none)
                // alongside the state publish — a no-op that keeps the invariant.
                self.staged_objects.clear();
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
            StateBacking::Odb { backing } => {
                // the duckfs durability ordering (native `module.rs:368-427`), as
                // an ORDER OF BACKING CALLS — the crash-safety contract Task 4
                // realizes on disk (objects fsync'd BEFORE the refs commit point,
                // refs adopted LAST so the root never advances ahead of durable
                // objects). the staged objects live in-memory (`staged_objects`)
                // and the new refs image in the state stage (`staged[REFS_KEY]`),
                // exactly like a native pending block; publish both here or drop
                // both on abort.
                //
                // 1. flush the block's staged objects into the backing (native
                //    `store.put` per object). `staged_objects` is id → tagged
                //    body (`kind ‖ body`); split the tag back off for the put.
                for tagged in std::mem::take(&mut self.staged_objects).into_values() {
                    let (&kind, body) = tagged
                        .split_first()
                        .expect("a staged object always carries its kind tag");
                    backing.stage_put(kind, body);
                }
                // 2. objects-durable barrier (native `store.sync_dirs`) — BEFORE
                //    the refs commit point below. threads the block height so the
                //    backing can stamp its durable-height envelope at adopt.
                backing.publish_block(height)?;
                // 3. adopt the new refs image IFF the block staged one — the sole
                //    place the root moves (native `refs_store.save` + `adopt_refs`).
                //    an empty stage leaves refs, and the root, untouched.
                if let Some(overlay) = self.staged.remove(REFS_KEY) {
                    // the refs lane only ever stages a value (`state-set`); a
                    // staged delete is a guest bug — reject deterministically
                    // (identical on every validator) rather than panic.
                    let refs = overlay.ok_or_else(|| {
                        SdkError::Module("files: refs lane staged a delete, never valid".into())
                    })?;
                    backing.adopt_refs(&refs)?;
                }
                self.staged.clear();
            }
            StateBacking::Store { store } => {
                self.staged_objects.clear();
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
        // discard this block's staged object puts alongside the state stage.
        self.staged_objects.clear();
        // the aborted block's captured height is void — the next block recaptures.
        self.block_height = None;
        // tell an odb backing to drop any block-local pending too (native
        // `Fs::abort_block`; a disk backing may sweep orphan object files). in
        // the fatal-or-complete commit model the backing has no pending here
        // unless a commit failed partway.
        match &mut self.backing {
            StateBacking::Odb { backing } => backing.discard_block(),
            StateBacking::Map { .. } | StateBacking::Store { .. } => {}
        }
        Ok(())
    }
}
