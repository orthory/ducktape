//! the derived read-model tier: one fluent31 database per module, materialized
//! from the finalized op stream.
//!
//! the canonical tier is deliberately not a database — any::unordered qmdb is
//! hashed keys and point lookups; no scans, no secondary indexes, no search.
//! this crate is the second tier: for every module it keeps an ordered,
//! scannable index fed block-by-block from the ops consensus applied. it is
//! DERIVED BY CONSTRUCTION:
//!
//! - never part of any `root()` or the app-hash — a wiped index changes no
//!   consensus-visible byte;
//! - node-local — no cross-node determinism claim is made for its contents;
//! - rebuildable — the crash story is "delete the module's index directory
//!   and replay", never repair.
//!
//! layout: one fluent31 `Db` under `<base>/<module-id>/` per module. inside a
//! module's database the key space is
//!
//! - `meta/height` — the watermark: every finalized block at or below this
//!   height is fully reflected (contiguity holds because a block's rows and
//!   the watermark move in ONE atomic [`WriteBatch`]). EVERY module's
//!   watermark advances on EVERY applied block — not only the dispatched
//!   modules' — so `watermark < H` always means "blocks are missing", never
//!   "the module was quiet";
//! - `meta/backfill` — present when the read model was re-derived from
//!   canonical state ([`IndexStore::rebuild_module`]): rows derived that way
//!   carry boundary-stamped coordinates and the op log starts above it;
//! - `op/{height:016x}/{seq:04x}` — one [`OpRow`] json envelope per dispatch
//!   the block applied to this module, in drain order (`seq` is the block-wide
//!   dispatch index, so cross-module ordering survives the per-module split);
//! - everything else — read-model keys owned by that module's registered
//!   [`ModuleIndexer`]; the two prefixes above are reserved and refused.
//!
//! alongside the per-module databases the store keeps ONE internal blocks
//! database (`<base>/_blocks/`, never a module): `blk/{height:016x}` holds the
//! block's explorer row ([`BlockOps::record`], opaque node-layer json) with
//! the same watermark discipline, so `GET /v1/blocks` survives a restart.
//!
//! writers: exactly one, the node's block loop, via [`IndexStore::apply_block`].
//! readers: any thread, via MVCC snapshots ([`IndexStore::scan`] /
//! [`IndexStore::get`]) — fluent31's `Db` is `Send + Sync`, so the http layer
//! reads concurrently with the writer without a lock between them.
//!
//! failure policy: an apply error POISONS the store (writes refuse, reads keep
//! serving) rather than skipping a block — a silent gap would break the
//! watermark's contiguity promise, and a derived tier's honest recovery is a
//! rebuild, not a patch.
//!
//! when canonical state advances WITHOUT the op stream — state-sync installs
//! a boundary, an index directory is wiped, a crash tears the index tail off
//! a suffix recovery re-execution skipped — a module's read model is
//! re-derived from VERIFIED canonical state instead:
//! [`IndexStore::rebuild_module`] clears the module's database, streams the
//! mapper's [`ModuleIndexer::rebuild_from_state`] rows back in, and stamps
//! the watermark at the boundary height, LAST. a crash mid-rebuild leaves no
//! watermark, so the caller's staleness check (`watermark < boundary`)
//! re-fires on the next boot — the rebuild is idempotent by re-trigger.
//!
//! the full "indexable" contract a per-module mapper must satisfy (fold
//! rules, view rules, when NOT to index) is `docs/indexable-spec.md`.

pub mod search;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use fluent31::{Db, IoBackend, Options, SyncMode, WriteBatch};
use serde::{Deserialize, Serialize};

/// reserved prefix for the per-op rows this crate writes itself.
pub const OP_PREFIX: &str = "op/";
/// reserved prefix for store bookkeeping.
pub const META_PREFIX: &str = "meta/";
/// key prefix of the per-block explorer rows in the internal blocks database.
pub const BLOCK_PREFIX: &str = "blk/";
/// directory name of the store-internal blocks database — reserved, never a
/// module id (the leading underscore keeps it out of the module namespace).
const BLOCKS_DB: &str = "_blocks";
/// the per-module watermark key: 8-byte big-endian height.
const META_HEIGHT: &str = "meta/height";
/// the backfill floor: 8-byte big-endian height, present only after a
/// from-state rebuild — everything derived at or below it is boundary-stamped.
const META_BACKFILL: &str = "meta/backfill";
/// how many staged rows a from-state rebuild accumulates before flushing a
/// batch: bounds memory while a mapper enumerates a large module's state.
const BACKFILL_FLUSH_EVERY: usize = 1024;
/// hard cap on one scan page; larger asks are clamped, mirroring the module
/// query convention (chat's MAX_QUERY_LIMIT) rather than erroring.
pub const MAX_SCAN_LIMIT: usize = 1024;
/// background fsync cadence for the index databases. the tier is rebuildable,
/// so a bounded loss window buys memory-speed block application; fluent31
/// recovery truncates a torn tail, never corrupts.
const SYNC_EVERY: Duration = Duration::from_millis(200);

// ============================================================================
// errors
// ============================================================================

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// the op stream named a module this store was not opened with.
    #[error("indexer: unknown module {0:?}")]
    UnknownModule(String),
    /// the module registered no materialized view (or no mapper at all) — the
    /// derived twin of the sdk's `QueryUnsupported`. some modules legitimately
    /// never will: forge's substrate is already a queryable git repo.
    #[error("indexer: module has no materialized view")]
    ViewUnsupported,
    /// a view request the module's mapper could not parse or serve.
    #[error("indexer: view: {0}")]
    View(String),
    /// a mapper failed folding an APPLIED op — interface drift or a damaged
    /// row; poisons the store, because guessing would silently skew the view.
    #[error("indexer: mapper: {0}")]
    Mapper(String),
    /// a [`ModuleIndexer`] tried to write into a reserved key space.
    #[error("indexer: derived write into reserved key {key:?} for module {module:?}")]
    ReservedKey { module: String, key: String },
    /// the module's mapper declares no from-state rebuild (or no mapper at
    /// all) — the module's views stay empty until new ops fold, which the
    /// spec treats as a first-class, documented degradation.
    #[error("indexer: module has no from-state rebuild")]
    RebuildUnsupported,
    /// a canonical-state read failed during a from-state rebuild — the node
    /// layer's [`StateReader`] adapter surfaces module/query errors here.
    #[error("indexer: state read: {0}")]
    State(String),
    /// a previous apply failed; the store refuses further writes until rebuilt.
    #[error("indexer: store is poisoned by an earlier apply failure — rebuild the index")]
    Poisoned,
    /// the storage engine failed.
    #[error("indexer: engine: {0}")]
    Engine(#[from] fluent31::Error),
    /// an op row failed to serialize (unreachable for well-formed input).
    #[error("indexer: row encoding: {0}")]
    Encoding(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

// ============================================================================
// the block-ops input — plain data, mapped by the node layer from its block
// outcome. deliberately NOT sdk/host types: the derived tier consumes an op
// stream, it is not part of the consensus contract.
// ============================================================================

/// who triggered a dispatch, flattened for the read model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OriginTag {
    pub kind: OriginKind,
    /// external: the submitter identity rendered lossily as utf-8;
    /// module: the emitting module id; system: absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OriginKind {
    External,
    Module,
    System,
}

impl OriginTag {
    pub fn external(id: impl Into<String>) -> Self {
        Self {
            kind: OriginKind::External,
            id: Some(id.into()),
        }
    }

    pub fn module(id: impl Into<String>) -> Self {
        Self {
            kind: OriginKind::Module,
            id: Some(id.into()),
        }
    }

    pub fn system() -> Self {
        Self {
            kind: OriginKind::System,
            id: None,
        }
    }
}

/// one dispatch a finalized block applied: the target module, the trigger, and
/// the op bytes. order within [`BlockOps::ops`] is drain order.
#[derive(Clone, Debug)]
pub struct AppliedOp {
    pub module: String,
    pub origin: OriginTag,
    pub payload: Vec<u8>,
}

/// one finalized block's op stream.
#[derive(Clone, Debug)]
pub struct BlockOps {
    pub height: u64,
    /// the block's agreed timestamp (consensus time, not wall clock).
    pub time: u64,
    pub ops: Vec<AppliedOp>,
    /// the block's explorer row — opaque, node-layer-defined json (the wire
    /// shape `GET /v1/blocks` serves), stored verbatim under
    /// `blk/{height:016x}` in the internal blocks database. `None` for blocks
    /// the explorer never shows (heartbeat nops, undecodable frames).
    pub record: Option<Vec<u8>>,
}

// ============================================================================
// the op row — the json envelope stored under `op/…`. module op payloads are
// serde_json across the workspace, so the common case embeds the payload
// verbatim (`payload`); bytes that are not valid json fall back to hex
// (`payloadHex`), mirroring the codebase's hex-not-base64 convention.
// ============================================================================

/// the stored shape of one applied op. `height`/`seq` repeat the key so a row
/// is self-describing when it travels without its key.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpRow<'a> {
    pub height: u64,
    pub seq: u32,
    pub time: u64,
    pub origin: OriginTag,
    #[serde(skip_serializing_if = "Option::is_none", borrow)]
    pub payload: Option<&'a serde_json::value::RawValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_hex: Option<String>,
}

fn encode_row(height: u64, seq: u32, time: u64, op: &AppliedOp) -> Result<Vec<u8>> {
    // valid json embeds verbatim; anything else ships as hex. `from_slice`
    // to &RawValue validates without building a tree.
    let raw: Option<&serde_json::value::RawValue> = serde_json::from_slice(&op.payload).ok();
    let row = OpRow {
        height,
        seq,
        time,
        origin: op.origin.clone(),
        payload: raw,
        payload_hex: raw.is_none().then(|| hex(&op.payload)),
    };
    Ok(serde_json::to_vec(&row)?)
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// the key of one op row: fixed-width hex so lexicographic order IS numeric
/// order. `seq` is the block-wide dispatch index (fits: the host drain budget
/// is 1024 dispatches per block).
fn op_key(height: u64, seq: u32) -> String {
    format!("{OP_PREFIX}{height:016x}/{seq:04x}")
}

/// the key of one block's explorer row, same fixed-width-hex ordering rule.
fn block_key(height: u64) -> String {
    format!("{BLOCK_PREFIX}{height:016x}")
}

// ============================================================================
// the module-indexer seam — a domain mapper registered per module. impls live
// with the node layer (or future per-module index crates that depend on the
// module's types-only interface crate); NEVER in this crate.
// ============================================================================

/// derived writes collected from a [`ModuleIndexer`] for one op. they land in
/// the SAME atomic batch as the op row and the watermark, so a read model can
/// never be half a block ahead of or behind the op log.
#[derive(Default)]
pub struct Derived {
    /// staged actions in mapper CALL ORDER — `Some` puts, `None` deletes. the
    /// order is load-bearing: when one op deletes and re-puts the same key (a
    /// retokenize whose old and new text share a token), the last action must
    /// win exactly as it would against the database; segregating puts from
    /// deletes would let a stale delete erase a fresh put.
    ops: Vec<(String, Option<Vec<u8>>)>,
}

impl Derived {
    pub fn put(&mut self, key: impl Into<String>, value: impl Into<Vec<u8>>) {
        self.ops.push((key.into(), Some(value.into())));
    }

    pub fn delete(&mut self, key: impl Into<String>) {
        self.ops.push((key.into(), None));
    }

    /// drain into the block batch AND the block's read overlay, refusing
    /// reserved key spaces. the overlay is what lets a later op in the same
    /// block read this op's staged writes.
    fn drain_into(
        self,
        module: &str,
        batch: &mut WriteBatch,
        overlay: &mut BTreeMap<Vec<u8>, Option<Vec<u8>>>,
    ) -> Result<()> {
        for (key, action) in self.ops {
            if key.starts_with(OP_PREFIX) || key.starts_with(META_PREFIX) {
                return Err(Error::ReservedKey {
                    module: module.to_string(),
                    key,
                });
            }
            match action {
                Some(value) => {
                    overlay.insert(key.clone().into_bytes(), Some(value.clone()));
                    batch.put(key, value);
                }
                None => {
                    overlay.insert(key.clone().into_bytes(), None);
                    batch.delete(key);
                }
            }
        }
        Ok(())
    }
}

/// the block-constant coordinates of one applied op, as handed to a mapper.
/// `seq` is the block-wide dispatch index, matching the op's `op/…` row.
#[derive(Clone, Debug)]
pub struct OpMeta<'a> {
    pub height: u64,
    pub time: u64,
    pub seq: u32,
    pub origin: &'a OriginTag,
}

/// read access during the fold: the module's COMMITTED index overlaid with
/// what this block staged so far, so an op can see the writes of ops earlier
/// in the same block (a post then an edit of it, one block apart by seq).
pub struct ApplyCtx<'a> {
    db: &'a Db,
    overlay: &'a BTreeMap<Vec<u8>, Option<Vec<u8>>>,
}

impl ApplyCtx<'_> {
    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        if let Some(staged) = self.overlay.get(key) {
            return Ok(staged.clone());
        }
        Ok(self.db.get(key)?)
    }
}

/// snapshot-consistent read access for a materialized view: every `get`/`scan`
/// of one [`ModuleIndexer::serve_view`] call sees the same MVCC snapshot.
pub struct ViewReader<'a> {
    db: &'a Db,
    snap: fluent31::Snapshot,
}

impl ViewReader<'_> {
    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        Ok(self.db.get_at(key, &self.snap)?)
    }

    /// one page of keys under `prefix`, strictly after cursor `after` when
    /// given, in key order. `limit` is clamped to [`MAX_SCAN_LIMIT`].
    pub fn scan(&self, prefix: &[u8], after: Option<&[u8]>, limit: usize) -> Result<Page> {
        let (lo, hi) = scan_bounds(prefix, after);
        let iter = self.db.iter_at(Some(&lo), hi.as_deref(), false, &self.snap)?;
        collect_page(iter, limit)
    }
}

/// read access to a module's VERIFIED canonical state during a from-state
/// rebuild: the module's own query wire, bytes in / bytes out. the node layer
/// adapts the module's sdk query surface onto this (mapping its errors into
/// [`Error::State`]); the mapper speaks its module's json request shapes
/// through it via the types-only interface crate it already depends on. this
/// keeps the crate domain-agnostic — no sdk, host, or module dep — and keeps
/// the derivation rooted in state that verified against the app-hash.
#[async_trait::async_trait(?Send)]
pub trait StateReader {
    async fn query(&self, req: &[u8]) -> Result<Vec<u8>>;
}

/// the boundary a from-state rebuild derives at. rows are stamped with these
/// coordinates because per-op coordinates do not survive a state transfer —
/// state carries values, not history. the spec calls this the documented
/// degradation: heights (and, where the module's state keeps no timestamps,
/// times) collapse to the boundary.
#[derive(Clone, Copy, Debug)]
pub struct RebuildMeta {
    pub height: u64,
    /// the boundary's consensus time when the caller knows it; 0 otherwise.
    pub time: u64,
}

/// streaming writer for a from-state rebuild. rows land in bounded batches as
/// the mapper enumerates state, so a large module never buffers its whole
/// read model in memory. puts only — a rebuild starts from a cleared
/// database. the watermark is stamped by the store AFTER the mapper returns,
/// riding the final batch, so an interrupted rebuild leaves no watermark and
/// the staleness trigger re-fires on the next boot.
pub struct Backfill<'a> {
    module: &'a str,
    db: &'a Db,
    batch: WriteBatch,
    staged: usize,
    written: u64,
}

impl Backfill<'_> {
    pub fn put(&mut self, key: impl Into<String>, value: impl Into<Vec<u8>>) -> Result<()> {
        let key = key.into();
        if key.starts_with(OP_PREFIX) || key.starts_with(META_PREFIX) {
            return Err(Error::ReservedKey {
                module: self.module.to_string(),
                key,
            });
        }
        self.batch.put(key, value.into());
        self.staged += 1;
        self.written += 1;
        if self.staged >= BACKFILL_FLUSH_EVERY {
            self.flush()?;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        if self.staged == 0 {
            return Ok(());
        }
        let batch = std::mem::replace(&mut self.batch, WriteBatch::new());
        self.db.write(batch)?;
        self.staged = 0;
        Ok(())
    }
}

/// a per-module read-model mapper: it folds the module's applied ops into
/// derived index keys (the WRITE side) and serves the module's materialized
/// view over them (the READ side — the module's own endpoint on the derived
/// tier). pure data-in/data-out — no IO of its own, no clock; writes go
/// through [`Derived`], reads through the handed-in ctx/reader.
#[async_trait::async_trait(?Send)]
pub trait ModuleIndexer: Send + Sync {
    /// the module whose ops this mapper consumes.
    fn module(&self) -> &str;

    /// map one applied op to derived writes. `ctx` reads the module's index
    /// as of this op (committed state plus this block's earlier staged
    /// writes); everything staged lands atomically with the op rows. an error
    /// poisons the store — the op WAS applied by the module, so a fold that
    /// cannot mirror it has no honest fallback.
    fn index_op(&self, ctx: &ApplyCtx, meta: &OpMeta, payload: &[u8], out: &mut Derived)
    -> Result<()>;

    /// the module's materialized-view projection: a module-defined request
    /// (json by convention, like the sdk query surface) in, module-defined
    /// response bytes out, at one MVCC snapshot. the default declares no
    /// view — right for modules whose fold is write-only and for modules
    /// that never register a mapper at all.
    fn serve_view(&self, _reader: &ViewReader, _req: &[u8]) -> Result<Vec<u8>> {
        Err(Error::ViewUnsupported)
    }

    /// whether this mapper can re-derive its read model from canonical state
    /// alone. checked BEFORE the store clears anything, so a mapper that
    /// cannot rebuild (default) leaves its database untouched.
    fn supports_rebuild(&self) -> bool {
        false
    }

    /// re-derive the module's read model from VERIFIED canonical state at a
    /// boundary: enumerate the module through `state` (its own query wire)
    /// and stream every derived row into `out`, stamped from `meta`. same
    /// determinism rules as the fold — no IO of its own, no clock; the only
    /// inputs are `state` and `meta`. a mapper that overrides this MUST also
    /// override [`ModuleIndexer::supports_rebuild`] to return true.
    async fn rebuild_from_state(
        &self,
        _state: &dyn StateReader,
        _meta: &RebuildMeta,
        _out: &mut Backfill<'_>,
    ) -> Result<()> {
        Err(Error::RebuildUnsupported)
    }
}

// ============================================================================
// the store
// ============================================================================

/// one scan page. `next_after` feeds the next call's `after` for cursoring;
/// it is only present when `has_more`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    /// key/value pairs in key order. values are the raw stored bytes.
    #[serde(skip)]
    pub entries: Vec<(Vec<u8>, Vec<u8>)>,
    pub has_more: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_after: Option<String>,
}

/// the per-module index store: one fluent31 database per registered module,
/// one writer (the block loop), snapshot readers everywhere else.
pub struct IndexStore {
    base: PathBuf,
    modules: BTreeMap<String, Arc<Db>>,
    /// the internal blocks database: `blk/…` explorer rows plus its own
    /// `meta/height` watermark. never listed in `modules` — it is not a
    /// module and must not surface on the per-module scan routes.
    blocks: Arc<Db>,
    mappers: BTreeMap<String, Box<dyn ModuleIndexer>>,
    /// set on the first apply failure; writes refuse from then on. reads stay
    /// available — stale-but-consistent beats unavailable for a derived tier.
    poisoned: AtomicBool,
}

impl IndexStore {
    /// open (creating if missing) one database per module id under `base`.
    pub fn open<S: AsRef<str>>(base: impl AsRef<Path>, module_ids: &[S]) -> Result<Self> {
        let base = base.as_ref().to_path_buf();
        let opts = Options {
            sync: SyncMode::Periodic { every: SYNC_EVERY },
            // portable positioned IO: the index shares its box with the node's
            // consensus lanes; io_uring buys nothing at this write rate.
            io_backend: IoBackend::Std,
            ..Options::default()
        };
        let mut modules = BTreeMap::new();
        for id in module_ids {
            let id = id.as_ref();
            let db = Db::open(base.join(id), opts.clone())?;
            modules.insert(id.to_string(), Arc::new(db));
        }
        let blocks = Arc::new(Db::open(base.join(BLOCKS_DB), opts)?);
        Ok(Self {
            base,
            modules,
            blocks,
            mappers: BTreeMap::new(),
            poisoned: AtomicBool::new(false),
        })
    }

    /// register a domain mapper. replaces any earlier mapper for the module.
    pub fn with_indexer(mut self, mapper: Box<dyn ModuleIndexer>) -> Self {
        self.mappers.insert(mapper.module().to_string(), mapper);
        self
    }

    pub fn base(&self) -> &Path {
        &self.base
    }

    pub fn module_ids(&self) -> impl Iterator<Item = &str> {
        self.modules.keys().map(String::as_str)
    }

    pub fn is_poisoned(&self) -> bool {
        self.poisoned.load(Ordering::Relaxed)
    }

    fn db(&self, module: &str) -> Result<&Arc<Db>> {
        self.modules
            .get(module)
            .ok_or_else(|| Error::UnknownModule(module.to_string()))
    }

    /// the watermark: every block at or below this height is fully reflected
    /// in the module's index. 0 for a fresh index.
    pub fn applied_height(&self, module: &str) -> Result<u64> {
        let db = self.db(module)?;
        Ok(read_height(db)?)
    }

    /// the height the node's block counter must resume ABOVE: the max
    /// watermark across all modules and the blocks database. every module
    /// advances on every applied block, so the max only differs per module
    /// when a database was wiped or added — exactly the modules
    /// [`IndexStore::rebuild_module`] repairs. the blocks watermark can lag
    /// them all: it only advances when a block carries an explorer row.
    pub fn resume_height(&self) -> Result<u64> {
        let mut max = read_height(&self.blocks)?;
        for db in self.modules.values() {
            max = max.max(read_height(db)?);
        }
        Ok(max)
    }

    /// the blocks-database watermark: every explorer row at or below this
    /// height is durably stored.
    pub fn blocks_height(&self) -> Result<u64> {
        Ok(read_height(&self.blocks)?)
    }

    /// the backfill floor: when present, the module's read model was
    /// re-derived from canonical state at this height — rows derived that way
    /// carry boundary-stamped coordinates and the op log starts above it.
    pub fn backfill_height(&self, module: &str) -> Result<Option<u64>> {
        let db = self.db(module)?;
        Ok(db
            .get(META_BACKFILL.as_bytes())?
            .and_then(|v| <[u8; 8]>::try_from(v.as_slice()).ok())
            .map(u64::from_be_bytes))
    }

    /// fold one finalized block into the per-module databases. idempotent per
    /// module (a module skips heights at or below its watermark), atomic per
    /// module (op rows, derived writes, and the watermark share one batch).
    /// any failure poisons the store: no gaps, ever.
    pub fn apply_block(&self, block: &BlockOps) -> Result<()> {
        if self.is_poisoned() {
            return Err(Error::Poisoned);
        }
        let out = self.apply_inner(block);
        if out.is_err() {
            self.poisoned.store(true, Ordering::Relaxed);
        }
        out
    }

    fn apply_inner(&self, block: &BlockOps) -> Result<()> {
        // group by module, keeping the block-wide dispatch index as seq. an
        // unknown module refuses BEFORE any batch commits — a block folded
        // into some databases and refused for the rest would be torn.
        let mut per: BTreeMap<&str, Vec<(u32, &AppliedOp)>> = BTreeMap::new();
        for (seq, op) in block.ops.iter().enumerate() {
            if !self.modules.contains_key(op.module.as_str()) {
                return Err(Error::UnknownModule(op.module.clone()));
            }
            per.entry(op.module.as_str())
                .or_default()
                .push((seq as u32, op));
        }
        // EVERY module's watermark advances, ops or not: `watermark < H` must
        // mean "blocks are missing", never "the module was quiet" — that is
        // what lets the rebuild trigger tell a wiped database from a lagging
        // one. a quiet module's batch is the watermark key alone.
        for (module, db) in &self.modules {
            if read_height(db)? >= block.height {
                continue; // replay of an already-folded block — idempotent skip
            }
            let mut batch = WriteBatch::new();
            if let Some(ops) = per.get(module.as_str()) {
                let mut overlay: BTreeMap<Vec<u8>, Option<Vec<u8>>> = BTreeMap::new();
                for &(seq, op) in ops {
                    batch.put(
                        op_key(block.height, seq),
                        encode_row(block.height, seq, block.time, op)?,
                    );
                    if let Some(mapper) = self.mappers.get(module.as_str()) {
                        let mut derived = Derived::default();
                        let meta = OpMeta {
                            height: block.height,
                            time: block.time,
                            seq,
                            origin: &op.origin,
                        };
                        let ctx = ApplyCtx { db, overlay: &overlay };
                        mapper.index_op(&ctx, &meta, &op.payload, &mut derived)?;
                        derived.drain_into(module, &mut batch, &mut overlay)?;
                    }
                }
            }
            batch.put(META_HEIGHT, block.height.to_be_bytes());
            db.write(batch)?;
        }
        // the explorer row lands AFTER the module folds: a visible block row
        // never precedes its op rows. same idempotent-skip and one-batch-with-
        // watermark discipline, on the blocks database's own watermark.
        if let Some(record) = &block.record {
            if read_height(&self.blocks)? < block.height {
                let mut batch = WriteBatch::new();
                batch.put(block_key(block.height), record.clone());
                batch.put(META_HEIGHT, block.height.to_be_bytes());
                self.blocks.write(batch)?;
            }
        }
        Ok(())
    }

    /// the newest `limit` explorer rows, oldest-first — the durable equivalent
    /// of an in-memory ring's "recent" read. rows return verbatim (their json
    /// shape is the node layer's), at one MVCC snapshot. `limit` is clamped to
    /// [`MAX_SCAN_LIMIT`].
    pub fn recent_block_rows(&self, limit: usize) -> Result<Vec<Vec<u8>>> {
        let limit = limit.clamp(1, MAX_SCAN_LIMIT);
        let prefix = BLOCK_PREFIX.as_bytes();
        let hi = prefix_successor(prefix);
        let snap = self.blocks.snapshot();
        let iter = self.blocks.iter_at(Some(prefix), hi.as_deref(), true, &snap)?;
        let mut rows = Vec::new();
        for kv in iter {
            if rows.len() == limit {
                break;
            }
            rows.push(kv?.1);
        }
        rows.reverse();
        Ok(rows)
    }

    /// re-derive one module's read model from VERIFIED canonical state at a
    /// boundary. the sequence is crash-safe by re-trigger:
    ///
    /// 1. drop the watermark (its own batch, FIRST) — any interruption from
    ///    here on leaves `applied_height` at 0, so the caller's staleness
    ///    check re-fires on the next boot;
    /// 2. clear the database in bounded batches, op log included — per-op
    ///    history cannot be re-derived from state, so it honestly starts at
    ///    the boundary;
    /// 3. stream the mapper's rows in via [`Backfill`];
    /// 4. stamp the watermark and the backfill floor at `meta.height`, LAST,
    ///    riding the final row batch.
    ///
    /// a mapper that declares no rebuild refuses up front, before anything is
    /// touched. any later failure poisons the store — the database is part
    /// way between two states and only a rebuild is honest. returns the
    /// number of derived rows written.
    pub async fn rebuild_module(
        &self,
        module: &str,
        state: &dyn StateReader,
        meta: RebuildMeta,
    ) -> Result<u64> {
        if self.is_poisoned() {
            return Err(Error::Poisoned);
        }
        let db = self.db(module)?;
        let mapper = self
            .mappers
            .get(module)
            .ok_or(Error::RebuildUnsupported)?;
        if !mapper.supports_rebuild() {
            return Err(Error::RebuildUnsupported);
        }
        let out = Self::rebuild_inner(module, db, mapper.as_ref(), state, meta).await;
        if out.is_err() {
            self.poisoned.store(true, Ordering::Relaxed);
        }
        out
    }

    /// stamp a module as backfilled at a boundary WITHOUT re-deriving rows:
    /// clear the database and set the watermark + backfill floor. this is the
    /// honest answer for a module whose mapper declares no from-state rebuild
    /// (or that has no mapper at all) when canonical state advanced without
    /// the op stream — its op log and views simply BEGIN at the boundary,
    /// visibly via the floor, instead of a watermark that silently claims
    /// pre-boundary coverage the fold never saw. same crash story as
    /// [`IndexStore::rebuild_module`]: watermark falls first, failures poison.
    pub fn mark_backfilled(&self, module: &str, meta: RebuildMeta) -> Result<()> {
        if self.is_poisoned() {
            return Err(Error::Poisoned);
        }
        let db = self.db(module)?;
        let out = (|| -> Result<()> {
            let mut drop_mark = WriteBatch::new();
            drop_mark.delete(META_HEIGHT);
            db.write(drop_mark)?;
            clear_db(db)?;
            let mut stamp = WriteBatch::new();
            stamp.put(META_HEIGHT, meta.height.to_be_bytes());
            stamp.put(META_BACKFILL, meta.height.to_be_bytes());
            db.write(stamp)?;
            Ok(())
        })();
        if out.is_err() {
            self.poisoned.store(true, Ordering::Relaxed);
        }
        out
    }

    async fn rebuild_inner(
        module: &str,
        db: &Db,
        mapper: &dyn ModuleIndexer,
        state: &dyn StateReader,
        meta: RebuildMeta,
    ) -> Result<u64> {
        // the watermark falls first so an interrupted rebuild re-triggers;
        // clearing sweeps whatever key order the database holds, and the
        // watermark must not be the key a crash happens to leave behind.
        let mut drop_mark = WriteBatch::new();
        drop_mark.delete(META_HEIGHT);
        db.write(drop_mark)?;
        clear_db(db)?;

        let mut out = Backfill {
            module,
            db,
            batch: WriteBatch::new(),
            staged: 0,
            written: 0,
        };
        mapper.rebuild_from_state(state, &meta, &mut out).await?;
        let Backfill {
            mut batch, written, ..
        } = out;
        batch.put(META_HEIGHT, meta.height.to_be_bytes());
        batch.put(META_BACKFILL, meta.height.to_be_bytes());
        db.write(batch)?;
        Ok(written)
    }

    /// point read of one stored key at the current snapshot.
    pub fn get(&self, module: &str, key: &[u8]) -> Result<Option<Vec<u8>>> {
        Ok(self.db(module)?.get(key)?)
    }

    /// one page of keys under `prefix`, strictly after cursor `after` when
    /// given, in key order, at one MVCC snapshot. `limit` is clamped to
    /// [`MAX_SCAN_LIMIT`].
    pub fn scan(
        &self,
        module: &str,
        prefix: &[u8],
        after: Option<&[u8]>,
        limit: usize,
    ) -> Result<Page> {
        let db = self.db(module)?;
        let (lo, hi) = scan_bounds(prefix, after);
        let snap = db.snapshot();
        let iter = db.iter_at(Some(&lo), hi.as_deref(), false, &snap)?;
        collect_page(iter, limit)
    }

    /// serve the module's materialized view: the module-defined request goes
    /// to the registered mapper's [`ModuleIndexer::serve_view`] with a
    /// snapshot reader over that module's index. modules without a mapper —
    /// or whose mapper declares no view — answer [`Error::ViewUnsupported`].
    /// a poisoned store still serves views: stale but consistent.
    pub fn view(&self, module: &str, req: &[u8]) -> Result<Vec<u8>> {
        let db = self.db(module)?;
        let mapper = self.mappers.get(module).ok_or(Error::ViewUnsupported)?;
        let reader = ViewReader {
            db,
            snap: db.snapshot(),
        };
        mapper.serve_view(&reader, req)
    }
}

/// lo/hi iteration bounds for a prefix scan resuming strictly after `after`:
/// lo is the cursor plus one 0x00 byte (the smallest strictly-greater key),
/// else the prefix itself; hi is the prefix successor (`None` = open-ended).
fn scan_bounds(prefix: &[u8], after: Option<&[u8]>) -> (Vec<u8>, Option<Vec<u8>>) {
    let lo = match after {
        Some(a) if a >= prefix => {
            let mut lo = a.to_vec();
            lo.push(0);
            lo
        }
        _ => prefix.to_vec(),
    };
    (lo, prefix_successor(prefix))
}

/// drain up to `limit` (clamped) pairs out of an iterator into a [`Page`].
fn collect_page(iter: fluent31::DbIterator, limit: usize) -> Result<Page> {
    let limit = limit.clamp(1, MAX_SCAN_LIMIT);
    let mut entries = Vec::new();
    let mut has_more = false;
    for kv in iter {
        let (key, value) = kv?;
        if entries.len() == limit {
            has_more = true;
            break;
        }
        entries.push((key, value));
    }
    let next_after = (has_more && !entries.is_empty())
        .then(|| String::from_utf8_lossy(&entries[entries.len() - 1].0).into_owned());
    Ok(Page {
        entries,
        has_more,
        next_after,
    })
}

/// delete every key in the database, in bounded batches, off one MVCC
/// snapshot — readers holding older snapshots keep serving while the sweep
/// runs. the caller has already dropped the watermark, so a crash mid-sweep
/// re-triggers the rebuild rather than leaving a half-empty index live.
fn clear_db(db: &Db) -> Result<()> {
    let snap = db.snapshot();
    let iter = db.iter_at(None, None, false, &snap)?;
    let mut batch = WriteBatch::new();
    let mut staged = 0usize;
    for kv in iter {
        let (key, _) = kv?;
        batch.delete(key);
        staged += 1;
        if staged >= BACKFILL_FLUSH_EVERY {
            db.write(std::mem::replace(&mut batch, WriteBatch::new()))?;
            staged = 0;
        }
    }
    if staged > 0 {
        db.write(batch)?;
    }
    Ok(())
}

fn read_height(db: &Db) -> fluent31::Result<u64> {
    Ok(db
        .get(META_HEIGHT.as_bytes())?
        .and_then(|v| <[u8; 8]>::try_from(v.as_slice()).ok())
        .map(u64::from_be_bytes)
        .unwrap_or(0))
}

/// the smallest byte string greater than every key with `prefix`: increment
/// the last non-0xff byte and truncate. `None` = scan to the end of the space.
fn prefix_successor(prefix: &[u8]) -> Option<Vec<u8>> {
    let mut succ = prefix.to_vec();
    while let Some(last) = succ.last_mut() {
        if *last < 0xff {
            *last += 1;
            return Some(succ);
        }
        succ.pop();
    }
    None
}

// ============================================================================
// tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn store(dir: &Path) -> IndexStore {
        IndexStore::open(dir, &["chat", "tasks"]).expect("open store")
    }

    fn chat_op(payload: &[u8]) -> AppliedOp {
        AppliedOp {
            module: "chat".into(),
            origin: OriginTag::external("jess"),
            payload: payload.to_vec(),
        }
    }

    fn block(height: u64, ops: Vec<AppliedOp>) -> BlockOps {
        BlockOps {
            height,
            time: 1_000 + height,
            ops,
            record: None,
        }
    }

    fn block_with_record(height: u64, ops: Vec<AppliedOp>) -> BlockOps {
        BlockOps {
            record: Some(format!(r#"{{"height":{height}}}"#).into_bytes()),
            ..block(height, ops)
        }
    }

    #[test]
    fn op_rows_land_per_module_in_drain_order() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());

        store
            .apply_block(&block(
                1,
                vec![
                    chat_op(br#"{"post":"hi"}"#),
                    AppliedOp {
                        module: "tasks".into(),
                        origin: OriginTag::module("chat"),
                        payload: br#"{"create":"t"}"#.to_vec(),
                    },
                    chat_op(br#"{"post":"again"}"#),
                ],
            ))
            .expect("apply");

        let page = store.scan("chat", OP_PREFIX.as_bytes(), None, 10).unwrap();
        assert_eq!(page.entries.len(), 2);
        assert!(!page.has_more);
        // block-wide seq survives the per-module split: chat got 0 and 2.
        assert_eq!(page.entries[0].0, op_key(1, 0).into_bytes());
        assert_eq!(page.entries[1].0, op_key(1, 2).into_bytes());

        let row: OpRow = serde_json::from_slice(&page.entries[0].1).unwrap();
        assert_eq!(row.height, 1);
        assert_eq!(row.seq, 0);
        assert_eq!(row.time, 1_001);
        assert_eq!(row.origin, OriginTag::external("jess"));
        assert_eq!(row.payload.unwrap().get(), r#"{"post":"hi"}"#);
        assert!(row.payload_hex.is_none());

        assert_eq!(store.applied_height("chat").unwrap(), 1);
        assert_eq!(store.applied_height("tasks").unwrap(), 1);
    }

    #[test]
    fn replay_is_idempotent_per_module() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());

        let b1 = block(1, vec![chat_op(b"{}")]);
        store.apply_block(&b1).expect("first apply");
        store.apply_block(&b1).expect("replay is a skip, not an error");

        let page = store.scan("chat", OP_PREFIX.as_bytes(), None, 10).unwrap();
        assert_eq!(page.entries.len(), 1, "no duplicate rows on replay");
        assert_eq!(store.applied_height("chat").unwrap(), 1);
    }

    #[test]
    fn watermarks_survive_reopen() {
        let dir = tempfile::tempdir().unwrap();
        {
            let store = store(dir.path());
            store.apply_block(&block(1, vec![chat_op(b"{}")])).unwrap();
            store.apply_block(&block(2, vec![chat_op(b"{}")])).unwrap();
        }
        let store = store(dir.path());
        assert_eq!(store.applied_height("chat").unwrap(), 2);
        assert_eq!(
            store.applied_height("tasks").unwrap(),
            2,
            "a quiet module's watermark advances with every block — watermark \
             lag must mean missing blocks, not missing ops"
        );
        assert_eq!(store.resume_height().unwrap(), 2, "resume from the max watermark");
        let page = store.scan("chat", OP_PREFIX.as_bytes(), None, 10).unwrap();
        assert_eq!(page.entries.len(), 2);
    }

    #[test]
    fn non_json_payload_falls_back_to_hex() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        store
            .apply_block(&block(1, vec![chat_op(&[0xde, 0xad])]))
            .unwrap();
        let page = store.scan("chat", OP_PREFIX.as_bytes(), None, 10).unwrap();
        let row: OpRow = serde_json::from_slice(&page.entries[0].1).unwrap();
        assert!(row.payload.is_none());
        assert_eq!(row.payload_hex.as_deref(), Some("dead"));
    }

    #[test]
    fn scan_pages_with_cursor() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        for h in 1..=5 {
            store.apply_block(&block(h, vec![chat_op(b"{}")])).unwrap();
        }

        let first = store.scan("chat", OP_PREFIX.as_bytes(), None, 2).unwrap();
        assert_eq!(first.entries.len(), 2);
        assert!(first.has_more);
        let cursor = first.next_after.clone().expect("cursor when has_more");

        let second = store
            .scan("chat", OP_PREFIX.as_bytes(), Some(cursor.as_bytes()), 10)
            .unwrap();
        assert_eq!(second.entries.len(), 3, "resumes strictly after the cursor");
        assert!(!second.has_more);
        assert!(second.next_after.is_none());
        assert_eq!(second.entries[0].0, op_key(3, 0).into_bytes());
    }

    #[test]
    fn unknown_module_is_refused_and_poisons() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        let bad = block(
            1,
            vec![AppliedOp {
                module: "ghost".into(),
                origin: OriginTag::system(),
                payload: b"{}".to_vec(),
            }],
        );
        assert!(matches!(
            store.apply_block(&bad),
            Err(Error::UnknownModule(_))
        ));
        assert!(store.is_poisoned());
        // writes refuse from now on; reads keep serving.
        assert!(matches!(
            store.apply_block(&block(2, vec![chat_op(b"{}")])),
            Err(Error::Poisoned)
        ));
        assert!(store.scan("chat", b"", None, 10).is_ok());
    }

    /// a mapper that counts ops per origin (a read-modify-write fold, so it
    /// exercises the block overlay), serves a tiny view, and, on demand,
    /// misbehaves into the reserved key space.
    struct TestMapper {
        reserved: bool,
    }

    impl ModuleIndexer for TestMapper {
        fn module(&self) -> &str {
            "chat"
        }
        fn index_op(
            &self,
            ctx: &ApplyCtx,
            meta: &OpMeta,
            payload: &[u8],
            out: &mut Derived,
        ) -> Result<()> {
            if self.reserved {
                out.put("meta/evil", b"nope".to_vec());
                return Ok(());
            }
            let who = meta.origin.id.clone().unwrap_or_default();
            out.put(
                format!("by-origin/{who}/{:016x}/{:04x}", meta.height, meta.seq),
                payload.to_vec(),
            );
            // read-modify-write: sees writes staged earlier in this block.
            let count_key = format!("count/{who}");
            let count = ctx
                .get(count_key.as_bytes())?
                .and_then(|v| <[u8; 8]>::try_from(v.as_slice()).ok())
                .map(u64::from_be_bytes)
                .unwrap_or(0);
            out.put(count_key, (count + 1).to_be_bytes().to_vec());
            Ok(())
        }

        fn serve_view(&self, reader: &ViewReader, req: &[u8]) -> Result<Vec<u8>> {
            // request: an origin id; response: that origin's op count.
            let who = std::str::from_utf8(req).map_err(|e| Error::View(e.to_string()))?;
            let count = reader
                .get(format!("count/{who}").as_bytes())?
                .and_then(|v| <[u8; 8]>::try_from(v.as_slice()).ok())
                .map(u64::from_be_bytes)
                .unwrap_or(0);
            Ok(count.to_string().into_bytes())
        }
    }

    #[test]
    fn mapper_writes_ride_the_same_batch() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path()).with_indexer(Box::new(TestMapper { reserved: false }));
        store
            .apply_block(&block(7, vec![chat_op(br#"{"post":"hi"}"#)]))
            .unwrap();

        let page = store.scan("chat", b"by-origin/jess/", None, 10).unwrap();
        assert_eq!(page.entries.len(), 1);
        assert_eq!(page.entries[0].1, br#"{"post":"hi"}"#.to_vec());
    }

    #[test]
    fn same_block_ops_read_each_others_staged_writes() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path()).with_indexer(Box::new(TestMapper { reserved: false }));
        // two ops in ONE block: the second's read-modify-write must see the
        // first's staged count, or the fold loses writes.
        store
            .apply_block(&block(1, vec![chat_op(b"{}"), chat_op(b"{}")]))
            .unwrap();
        assert_eq!(store.view("chat", b"jess").unwrap(), b"2".to_vec());
    }

    #[test]
    fn view_routes_to_the_mapper_and_defaults_to_unsupported() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path()).with_indexer(Box::new(TestMapper { reserved: false }));
        store
            .apply_block(&block(1, vec![chat_op(b"{}")]))
            .unwrap();
        assert_eq!(store.view("chat", b"jess").unwrap(), b"1".to_vec());
        // no mapper registered for tasks → no materialized view.
        assert!(matches!(
            store.view("tasks", b"x"),
            Err(Error::ViewUnsupported)
        ));
        assert!(matches!(
            store.view("ghost", b"x"),
            Err(Error::UnknownModule(_))
        ));
    }

    #[test]
    fn mapper_reserved_key_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path()).with_indexer(Box::new(TestMapper { reserved: true }));
        assert!(matches!(
            store.apply_block(&block(1, vec![chat_op(b"{}")])),
            Err(Error::ReservedKey { .. })
        ));
        assert!(store.is_poisoned());
        // the refused block left nothing behind — the batch never committed.
        let page = store.scan("chat", b"", None, 10).unwrap();
        assert!(page.entries.is_empty());
    }

    /// a mapper that deletes then re-puts ONE key inside a single op — the
    /// retokenize shape. the last staged action must win; if the drain
    /// segregated puts from deletes, the stale delete would erase the fresh
    /// put and a still-present posting would vanish.
    struct DeleteThenPut;

    impl ModuleIndexer for DeleteThenPut {
        fn module(&self) -> &str {
            "chat"
        }
        fn index_op(
            &self,
            _ctx: &ApplyCtx,
            _meta: &OpMeta,
            payload: &[u8],
            out: &mut Derived,
        ) -> Result<()> {
            out.delete("kept");
            out.put("kept", payload.to_vec());
            out.put("dropped", b"old".to_vec());
            out.delete("dropped");
            Ok(())
        }
    }

    #[test]
    fn derived_actions_apply_in_call_order() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path()).with_indexer(Box::new(DeleteThenPut));
        store
            .apply_block(&block(1, vec![chat_op(b"fresh")]))
            .unwrap();
        assert_eq!(
            store.get("chat", b"kept").unwrap(),
            Some(b"fresh".to_vec()),
            "delete-then-put keeps the put"
        );
        assert_eq!(
            store.get("chat", b"dropped").unwrap(),
            None,
            "put-then-delete keeps the delete"
        );
    }

    #[test]
    fn block_records_serve_newest_first_tail_oldest_first() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        for h in 1..=5 {
            store
                .apply_block(&block_with_record(h, vec![chat_op(b"{}")]))
                .unwrap();
        }

        let rows = store.recent_block_rows(3).unwrap();
        assert_eq!(rows.len(), 3);
        // the newest 3 (heights 3..=5), oldest-first — the ring's contract.
        assert_eq!(rows[0], br#"{"height":3}"#.to_vec());
        assert_eq!(rows[2], br#"{"height":5}"#.to_vec());
        assert_eq!(store.recent_block_rows(100).unwrap().len(), 5);
        assert_eq!(store.blocks_height().unwrap(), 5);
    }

    #[test]
    fn block_record_lands_without_ops_and_advances_resume() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        // a block whose op stream is empty for the index (e.g. a finalized-
        // but-rejected frame) still shows in the explorer.
        store.apply_block(&block_with_record(9, Vec::new())).unwrap();

        assert_eq!(store.recent_block_rows(10).unwrap().len(), 1);
        // every module's watermark advances — quiet is not stale — but no op
        // rows were written.
        assert_eq!(store.applied_height("chat").unwrap(), 9);
        let ops = store.scan("chat", OP_PREFIX.as_bytes(), None, 10).unwrap();
        assert!(ops.entries.is_empty(), "no op rows");
        assert_eq!(store.resume_height().unwrap(), 9, "blocks watermark counts");
    }

    #[test]
    fn block_records_are_idempotent_and_survive_reopen() {
        let dir = tempfile::tempdir().unwrap();
        {
            let store = store(dir.path());
            let b = block_with_record(1, vec![chat_op(b"{}")]);
            store.apply_block(&b).unwrap();
            store.apply_block(&b).expect("replay is a skip");
        }
        let store = store(dir.path());
        let rows = store.recent_block_rows(10).unwrap();
        assert_eq!(rows.len(), 1, "no duplicate rows; rows survive reopen");
        assert_eq!(store.blocks_height().unwrap(), 1);
    }

    #[test]
    fn prefix_successor_edges() {
        assert_eq!(prefix_successor(b"op/"), Some(b"op0".to_vec()));
        assert_eq!(prefix_successor(&[0x01, 0xff]), Some(vec![0x02]));
        assert_eq!(prefix_successor(&[0xff, 0xff]), None);
        assert_eq!(prefix_successor(b""), None);
    }

    // ------------------------------------------------------------------------
    // from-state rebuild
    // ------------------------------------------------------------------------

    /// canonical state standing in for a module: a fixed item list the mapper
    /// re-derives from, or a read failure when `fail` is set.
    struct FakeState {
        items: Vec<String>,
        fail: bool,
    }

    #[async_trait::async_trait(?Send)]
    impl StateReader for FakeState {
        async fn query(&self, _req: &[u8]) -> Result<Vec<u8>> {
            if self.fail {
                return Err(Error::State("boom".into()));
            }
            Ok(serde_json::to_vec(&self.items)?)
        }
    }

    /// a mapper whose fold writes one `row/…` key per op and whose rebuild
    /// re-derives `row/…` keys from [`FakeState`], boundary-stamped.
    struct RebuildMapper {
        reserved_on_rebuild: bool,
    }

    #[async_trait::async_trait(?Send)]
    impl ModuleIndexer for RebuildMapper {
        fn module(&self) -> &str {
            "chat"
        }

        fn index_op(
            &self,
            _ctx: &ApplyCtx,
            meta: &OpMeta,
            payload: &[u8],
            out: &mut Derived,
        ) -> Result<()> {
            out.put(
                format!("row/{:016x}/{:04x}", meta.height, meta.seq),
                payload.to_vec(),
            );
            Ok(())
        }

        fn supports_rebuild(&self) -> bool {
            true
        }

        async fn rebuild_from_state(
            &self,
            state: &dyn StateReader,
            meta: &RebuildMeta,
            out: &mut Backfill<'_>,
        ) -> Result<()> {
            if self.reserved_on_rebuild {
                out.put("op/evil", b"nope".to_vec())?;
                return Ok(());
            }
            let items: Vec<String> = serde_json::from_slice(&state.query(b"list").await?)?;
            for item in items {
                out.put(format!("row/{item}"), meta.height.to_be_bytes().to_vec())?;
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn rebuild_replaces_rows_and_stamps_the_boundary() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path()).with_indexer(Box::new(RebuildMapper {
            reserved_on_rebuild: false,
        }));
        // a folded history: op rows + derived rows through height 2.
        store.apply_block(&block(1, vec![chat_op(b"{}")])).unwrap();
        store.apply_block(&block(2, vec![chat_op(b"{}")])).unwrap();

        let state = FakeState {
            items: vec!["a".into(), "b".into()],
            fail: false,
        };
        let written = store
            .rebuild_module("chat", &state, RebuildMeta { height: 10, time: 0 })
            .await
            .expect("rebuild");
        assert_eq!(written, 2);

        // the old fold's rows AND its op log are gone — op history starts at
        // the boundary — and the re-derived rows are in.
        assert!(store.scan("chat", OP_PREFIX.as_bytes(), None, 10).unwrap().entries.is_empty());
        let rows = store.scan("chat", b"row/", None, 10).unwrap();
        let keys: Vec<_> = rows.entries.iter().map(|(k, _)| k.as_slice()).collect();
        assert_eq!(keys, vec![b"row/a".as_slice(), b"row/b".as_slice()]);

        assert_eq!(store.applied_height("chat").unwrap(), 10);
        assert_eq!(store.backfill_height("chat").unwrap(), Some(10));
        assert_eq!(store.resume_height().unwrap(), 10);
        assert!(!store.is_poisoned());

        // the fold continues above the boundary as if it had always run.
        store.apply_block(&block(11, vec![chat_op(b"{}")])).unwrap();
        assert_eq!(store.applied_height("chat").unwrap(), 11);
        assert_eq!(store.backfill_height("chat").unwrap(), Some(10), "floor survives folding");
    }

    #[tokio::test]
    async fn rebuild_unsupported_leaves_the_database_untouched() {
        let dir = tempfile::tempdir().unwrap();
        // TestMapper declares no rebuild (the trait default).
        let store = store(dir.path()).with_indexer(Box::new(TestMapper { reserved: false }));
        store.apply_block(&block(1, vec![chat_op(b"{}")])).unwrap();

        let state = FakeState { items: vec![], fail: false };
        assert!(matches!(
            store
                .rebuild_module("chat", &state, RebuildMeta { height: 5, time: 0 })
                .await,
            Err(Error::RebuildUnsupported)
        ));
        // refused up front: nothing cleared, nothing poisoned.
        assert!(!store.is_poisoned());
        assert_eq!(store.applied_height("chat").unwrap(), 1);
        assert_eq!(store.backfill_height("chat").unwrap(), None);
        assert_eq!(store.scan("chat", OP_PREFIX.as_bytes(), None, 10).unwrap().entries.len(), 1);

        // no mapper at all refuses the same way.
        assert!(matches!(
            store
                .rebuild_module("tasks", &state, RebuildMeta { height: 5, time: 0 })
                .await,
            Err(Error::RebuildUnsupported)
        ));
    }

    #[test]
    fn mark_backfilled_clears_and_stamps_without_a_mapper() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        // tasks has NO mapper here — only its op log. a boundary passing
        // without the op stream stamps the floor where its content begins.
        store
            .apply_block(&block(
                1,
                vec![AppliedOp {
                    module: "tasks".into(),
                    origin: OriginTag::system(),
                    payload: b"{}".to_vec(),
                }],
            ))
            .unwrap();
        store
            .mark_backfilled("tasks", RebuildMeta { height: 7, time: 0 })
            .unwrap();
        assert!(
            store.scan("tasks", OP_PREFIX.as_bytes(), None, 10).unwrap().entries.is_empty(),
            "op history starts at the boundary"
        );
        assert_eq!(store.applied_height("tasks").unwrap(), 7);
        assert_eq!(store.backfill_height("tasks").unwrap(), Some(7));
        assert!(!store.is_poisoned());
    }

    #[tokio::test]
    async fn rebuild_failure_poisons_and_drops_the_watermark() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path()).with_indexer(Box::new(RebuildMapper {
            reserved_on_rebuild: false,
        }));
        store.apply_block(&block(3, vec![chat_op(b"{}")])).unwrap();

        let state = FakeState { items: vec![], fail: true };
        assert!(matches!(
            store
                .rebuild_module("chat", &state, RebuildMeta { height: 9, time: 0 })
                .await,
            Err(Error::State(_))
        ));
        assert!(store.is_poisoned());
        // the watermark fell before the failure, so a fresh process (poison
        // is in-memory only) re-detects staleness and re-triggers.
        assert_eq!(store.applied_height("chat").unwrap(), 0);
    }

    #[tokio::test]
    async fn rebuild_refuses_reserved_keys() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path()).with_indexer(Box::new(RebuildMapper {
            reserved_on_rebuild: true,
        }));
        let state = FakeState { items: vec![], fail: false };
        assert!(matches!(
            store
                .rebuild_module("chat", &state, RebuildMeta { height: 4, time: 0 })
                .await,
            Err(Error::ReservedKey { .. })
        ));
        assert!(store.is_poisoned());
    }
}
