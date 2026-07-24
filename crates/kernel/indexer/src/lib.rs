//! the derived read-model tier: one fluent31 database per module, fed the
//! finalized op stream by the node and FOLDED by the module's own wasm index
//! guest inside the engine.
//!
//! the canonical tier is deliberately not a database — any::unordered qmdb is
//! hashed keys and point lookups; no scans, no secondary indexes, no search.
//! this crate is the second tier: for every module it keeps an ordered,
//! scannable index fed block-by-block from the ops consensus applied. it is
//! DERIVED BY CONSTRUCTION:
//!
//! - never part of any `root()` or the root-hash — a wiped index changes no
//!   consensus-visible byte;
//! - node-local — no cross-node determinism claim is made for its contents;
//! - rebuildable — the crash story is "delete the module's index directory
//!   and replay", never repair.
//!
//! the tier is two loops, coupled only through the database:
//!
//! - **the host writer** (this crate, [`IndexStore::apply_block`], called by
//!   the node's block loop): writes one borsh [`OpRow`] per dispatch under
//!   `op/{height:016x}/{seq:04x}` plus the watermark, one atomic batch per
//!   module per block. NO domain logic lives host-side.
//! - **the fold** (the module's index guest, installed IN the module's
//!   database as fluentabi module `"index"`): a changes-mode trigger
//!   (`"fold"`) on the `op/` range delivers every committed op row to the
//!   guest's `on_apply`, exactly once, in commit order; the guest folds it
//!   into derived read-model keys inside its own transaction. the guest also
//!   serves the module's materialized view (`query` role,
//!   [`IndexStore::view`]).
//!
//! the fold is ASYNC and OPTIMISTIC by design: derived views trail the op
//! log by the trigger backlog ([`IndexStore::fold_status`] surfaces depth and
//! last error; nothing is ever lost — a failing guest holds its queue). the
//! watermark (`meta/height`) therefore vouches for the OP LOG alone: every
//! finalized block at or below it is fully in the feed. EVERY module's
//! watermark advances on EVERY applied block — not only the dispatched
//! modules' — so `watermark < H` always means "blocks are missing", never
//! "the module was quiet".
//!
//! per-module key space: `op/…` and `meta/…` are host-reserved (the trigger
//! range spans `op/` only, so the guest never sees bookkeeping writes);
//! everything else belongs to the guest's fold. guest code itself lives in
//! the engine's own reserved 0x00 keyspace — invisible to scans, wiped by
//! nothing this crate does, and shipped WITH the data by the shipping lane.
//!
//! alongside the per-module databases the store keeps ONE internal blocks
//! database (`<base>/_blocks/`, never a module): `blk/{height:016x}` holds the
//! block's explorer row ([`BlockOps::record`], opaque node-layer json) with
//! the same watermark discipline, so `GET /v1/blocks` survives a restart.
//!
//! readers: any thread, via MVCC snapshots ([`IndexStore::scan`] /
//! [`IndexStore::get`] / [`IndexStore::view`]) — fluent31's `Db` is
//! `Send + Sync`, so the http layer reads concurrently with both writers.
//!
//! failure policy: a host-write error POISONS the store (writes refuse, reads
//! keep serving) rather than skipping a block — a silent gap would break the
//! watermark's contiguity promise. a guest-fold error never poisons: the
//! engine retains the events and retries, and the backlog is observable.
//!
//! when canonical state advances WITHOUT the op stream — state-sync installs
//! a boundary, an index directory is wiped, a crash tears the index tail off
//! a suffix recovery re-execution skipped — the module is stamped BACKFILLED
//! at the boundary ([`IndexStore::mark_backfilled`]): its op log and views
//! honestly BEGIN there, visibly via `meta/backfill`, instead of a watermark
//! that silently claims pre-boundary coverage the feed never saw. history
//! below a boundary re-enters only by replaying blocks (the node's journal /
//! frame catch-up drives [`IndexStore::apply_block`] again) or by adopting a
//! shipped index (the staging lane below).
//!
//! the full contract a per-module index guest must satisfy (fold rules, view
//! rules, when NOT to index) is `docs/records/specs/indexable-spec.md`; the
//! authoring surface is the `index-guest` crate.

mod disk;
pub use disk::{DiskEntry, DiskFs, IndexDisk};

// the mem arm of the disk seam — behind `sim` (and always in test) so it never
// ships in a release build. the fluent31-backed read models cannot run on it
// (they own their IO); it drives the shipping lane's staging with no tempdir.
#[cfg(any(test, feature = "sim"))]
mod mem;
#[cfg(any(test, feature = "sim"))]
pub use mem::MemDisk;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use borsh::{BorshDeserialize, BorshSerialize};
use fluent31::{Db, IoBackend, Options, SyncMode, WriteBatch};
use serde::Serialize;
use sha2::Digest as _;

// the shared host↔guest vocabulary: key conventions + the borsh op-row
// envelope. re-exported so every host-side consumer names them through this
// crate, exactly as before the wasm cutover.
pub use index_guest::{META_PREFIX, OP_PREFIX, OpRow, OriginKind, OriginTag, op_key, user_handle};

/// key prefix of the per-block explorer rows in the internal blocks database.
pub const BLOCK_PREFIX: &str = "blk/";
/// directory name of the store-internal blocks database — reserved, never a
/// module id (the leading underscore keeps it out of the module namespace).
/// public because the shipping lane (spec §7 lane 2) addresses it by name:
/// a source ships it alongside the module databases so a joiner's explorer
/// history starts warm too.
pub const BLOCKS_DB_ID: &str = "_blocks";
/// directory name of a staged shipped-index install awaiting adoption at the
/// next [`IndexStore::open`] — same underscore convention as [`BLOCKS_DB_ID`].
const STAGING_DIR: &str = "_staging";
/// marker file inside [`STAGING_DIR`], written LAST (after every staged file
/// is durable): a staging directory without it is a torn fetch and is
/// discarded at open instead of adopted.
const STAGING_COMPLETE: &str = ".complete";
/// fixed fork name for a shipping cut. one cut is in flight per database at a
/// time (the block loop serializes them), so a constant name suffices — a
/// stale same-name leftover from a crash is deleted before the fresh cut.
const SHIP_FORK: &str = "ship";
/// the fluentabi module name every index guest installs under, inside its
/// module's own database.
const GUEST_NAME: &str = "index";
/// the changes-mode trigger binding [`GUEST_NAME`]'s `on_apply` to the `op/`
/// range — the fold feed.
const FOLD_TRIGGER: &str = "fold";
/// the per-module watermark key: 8-byte big-endian height.
const META_HEIGHT: &str = "meta/height";
/// the backfill floor: 8-byte big-endian height, present only after a
/// boundary stamp — everything below it is absent from the feed, visibly.
const META_BACKFILL: &str = "meta/backfill";
/// the guest-converge marker (borsh [`GuestMarker`]): which artifact this
/// database is converged on. a warm boot that finds a matching marker skips
/// every wasm compile.
const META_GUEST: &str = "meta/guest";
/// how many staged deletes a database wipe accumulates before flushing a
/// batch: bounds memory while sweeping a large read model.
const CLEAR_FLUSH_EVERY: usize = 1024;
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
    /// the module ships no index guest — or one without a `query` role — so
    /// it has no materialized view; the derived twin of the sdk's
    /// `QueryUnsupported`. some modules legitimately never will: forge's
    /// substrate is already a queryable git repo.
    #[error("indexer: module has no materialized view")]
    ViewUnsupported,
    /// a view request the module's index guest refused (its `Fail` message).
    #[error("indexer: view: {0}")]
    View(String),
    /// a previous apply failed; the store refuses further writes until rebuilt.
    #[error("indexer: store is poisoned by an earlier apply failure — rebuild the index")]
    Poisoned,
    /// filesystem io in the shipping lane (fork archive reads, staged
    /// installs) — io this crate performs itself, outside the engine's own
    /// error surface.
    #[error("indexer: index shipping: {0}")]
    Shipping(String),
    /// the storage engine failed.
    #[error("indexer: engine: {0}")]
    Engine(#[from] fluent31::Error),
    /// an op row failed to serialize (unreachable for well-formed input).
    #[error("indexer: row encoding: {0}")]
    Encoding(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

// ============================================================================
// the block-ops input — plain data, mapped by the node layer from its block
// outcome. deliberately NOT sdk/host types: the derived tier consumes an op
// stream, it is not part of the consensus contract.
// ============================================================================

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

fn encode_row(height: u64, seq: u32, time: u64, op: &AppliedOp) -> Result<Vec<u8>> {
    Ok(borsh::to_vec(&OpRow {
        height,
        seq,
        time,
        origin: op.origin.clone(),
        payload: op.payload.clone(),
    })?)
}

/// the key of one block's explorer row, same fixed-width-hex ordering rule as
/// [`op_key`].
fn block_key(height: u64) -> String {
    format!("{BLOCK_PREFIX}{height:016x}")
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

/// one module's index-guest wiring, as declared to [`IndexStore::open`].
/// `guest` is the fluentabi mapper to install into the module's database
/// (`None` for a module that ships no index guest — its database still holds
/// the op log and watermark, and scans still serve).
pub struct IndexModule<'a> {
    pub id: &'a str,
    pub guest: Option<&'a [u8]>,
}

impl<'a> IndexModule<'a> {
    /// a module with no index guest — op log + watermark only.
    pub fn bare(id: &'a str) -> Self {
        Self { id, guest: None }
    }
}

/// the fold trigger's health, for the status surface: how many committed op
/// rows the guest has not folded yet, and why the last drain failed.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FoldStatus {
    pub pending: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

/// one module's open database plus its guest's declared roles.
struct ModuleIndex {
    db: Arc<Db>,
    /// the guest exports `on_apply` — a fold trigger is registered.
    has_fold: bool,
    /// the guest exports `query` — [`IndexStore::view`] routes to it.
    has_view: bool,
}

/// the per-module index store: one fluent31 database per registered module,
/// one host writer (the block loop), the engine's trigger runner folding
/// behind it, snapshot readers everywhere else.
pub struct IndexStore {
    base: PathBuf,
    modules: BTreeMap<String, ModuleIndex>,
    /// the internal blocks database: `blk/…` explorer rows plus its own
    /// `meta/height` watermark. never listed in `modules` — it is not a
    /// module, must not surface on the per-module scan routes, and never
    /// hosts a guest.
    blocks: Arc<Db>,
    /// set on the first host-write failure; writes refuse from then on. reads
    /// stay available — stale-but-consistent beats unavailable for a derived
    /// tier. guest-fold failures never set this: the engine retains their
    /// events and the backlog is observable instead.
    poisoned: AtomicBool,
    /// the filesystem the shipping lane ([`IndexStore::checkpoint_files`] and
    /// the staged-install adoption at open) reads and writes through. defaults
    /// to [`DiskFs`]; a mem arm exists for driving the staging lane in tests.
    disk: Box<dyn IndexDisk>,
}

impl IndexStore {
    /// open (creating if missing) one database per module under `base` and
    /// converge each onto its declared index guest: install (or replace) the
    /// mapper bytes, register the fold trigger when the guest folds, tear
    /// both down when a module no longer ships a guest.
    ///
    /// a COMPLETE staged shipped-index install under `<base>/_staging` (see
    /// [`stage_shipped_db`]) is adopted first — database directories swap in
    /// before any engine open, so adoption always precedes open by
    /// construction. a torn staging directory (no completion marker) is
    /// discarded, falling back to whatever the databases already hold.
    pub fn open(base: impl AsRef<Path>, modules: &[IndexModule]) -> Result<Self> {
        let base = base.as_ref().to_path_buf();
        let disk: Box<dyn IndexDisk> = Box::new(DiskFs);
        adopt_staged(disk.as_ref(), &base)?;
        let opts = Options {
            sync: SyncMode::Periodic { every: SYNC_EVERY },
            // portable positioned IO: the index shares its box with the node's
            // consensus lanes; io_uring buys nothing at this write rate.
            io_backend: IoBackend::Std,
            ..Options::default()
        };
        let mut open = BTreeMap::new();
        for spec in modules {
            let db = Arc::new(Db::open(base.join(spec.id), opts.clone())?);
            let (has_fold, has_view) = converge_guest(&db, spec)?;
            open.insert(
                spec.id.to_string(),
                ModuleIndex {
                    db,
                    has_fold,
                    has_view,
                },
            );
        }
        let blocks = Arc::new(Db::open(base.join(BLOCKS_DB_ID), opts)?);
        Ok(Self {
            base,
            modules: open,
            blocks,
            poisoned: AtomicBool::new(false),
            disk,
        })
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

    fn module(&self, module: &str) -> Result<&ModuleIndex> {
        self.modules
            .get(module)
            .ok_or_else(|| Error::UnknownModule(module.to_string()))
    }

    fn db(&self, module: &str) -> Result<&Arc<Db>> {
        Ok(&self.module(module)?.db)
    }

    /// the watermark: every block at or below this height is fully in the
    /// module's op feed. 0 for a fresh index. says NOTHING about the derived
    /// view, which trails by [`IndexStore::fold_status`]'s backlog.
    pub fn applied_height(&self, module: &str) -> Result<u64> {
        let db = self.db(module)?;
        Ok(read_height(db)?)
    }

    /// the height the node's block counter must resume ABOVE: the max
    /// watermark across all modules and the blocks database. every module
    /// advances on every applied block, so the max only differs per module
    /// when a database was wiped or added — exactly the modules
    /// [`IndexStore::mark_backfilled`] stamps. the blocks watermark can lag
    /// them all: it only advances when a block carries an explorer row.
    pub fn resume_height(&self) -> Result<u64> {
        let mut max = read_height(&self.blocks)?;
        for module in self.modules.values() {
            max = max.max(read_height(&module.db)?);
        }
        Ok(max)
    }

    /// the blocks-database watermark: every explorer row at or below this
    /// height is durably stored.
    pub fn blocks_height(&self) -> Result<u64> {
        Ok(read_height(&self.blocks)?)
    }

    /// the backfill floor: when present, the module was stamped at a boundary
    /// — its op feed (and everything derived) visibly begins above it.
    pub fn backfill_height(&self, module: &str) -> Result<Option<u64>> {
        let db = self.db(module)?;
        Ok(db
            .get(META_BACKFILL.as_bytes())?
            .and_then(|v| <[u8; 8]>::try_from(v.as_slice()).ok())
            .map(u64::from_be_bytes))
    }

    /// the fold trigger's backlog + last drain error, `None` for a module
    /// with no folding guest. a deep or stuck backlog is the tier's honest
    /// "the view is stale" signal — surfaced, never guessed.
    pub fn fold_status(&self, module: &str) -> Result<Option<FoldStatus>> {
        let m = self.module(module)?;
        if !m.has_fold {
            return Ok(None);
        }
        let triggers = m.db.list_triggers()?;
        Ok(triggers
            .into_iter()
            .find(|t| t.name == FOLD_TRIGGER)
            .map(|t| FoldStatus {
                pending: t.pending,
                last_error: t.last_error,
            }))
    }

    /// fold one finalized block into the per-module feeds. idempotent per
    /// module (a module skips heights at or below its watermark), atomic per
    /// module (op rows and the watermark share one batch; the guest folds
    /// asynchronously behind the trigger). any failure poisons the store: no
    /// gaps, ever.
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
        // what lets the staleness check tell a wiped database from a lagging
        // one. a quiet module's batch is the watermark key alone.
        for (id, module) in &self.modules {
            if read_height(&module.db)? >= block.height {
                continue; // replay of an already-folded block — idempotent skip
            }
            let mut batch = WriteBatch::new();
            if let Some(ops) = per.get(id.as_str()) {
                for &(seq, op) in ops {
                    batch.put(
                        op_key(block.height, seq),
                        encode_row(block.height, seq, block.time, op)?,
                    );
                }
            }
            batch.put(META_HEIGHT, block.height.to_be_bytes());
            module.db.write(batch)?;
        }
        // the explorer row lands AFTER the module feeds: a visible block row
        // never precedes its op rows. same idempotent-skip and one-batch-with-
        // watermark discipline, on the blocks database's own watermark.
        if let Some(record) = &block.record
            && read_height(&self.blocks)? < block.height
        {
            let mut batch = WriteBatch::new();
            batch.put(block_key(block.height), record.clone());
            batch.put(META_HEIGHT, block.height.to_be_bytes());
            self.blocks.write(batch)?;
        }
        Ok(())
    }

    /// store one explorer row at `height` WITHOUT a dispatch feed — the write
    /// side for a follower that observes state boundaries, never sealed
    /// blocks. the module read models are the caller's problem (they are
    /// stamped at the boundary, [`IndexStore::mark_backfilled`]); this keeps
    /// the blocks database honest about the one thing such a caller DID
    /// observe: the boundary itself. same discipline as the fold's record
    /// write — idempotent skip at or below the blocks watermark, row and
    /// watermark in one atomic batch, failures poison.
    pub fn apply_block_record(&self, height: u64, record: Vec<u8>) -> Result<()> {
        if self.is_poisoned() {
            return Err(Error::Poisoned);
        }
        let out = (|| -> Result<()> {
            if read_height(&self.blocks)? < height {
                let mut batch = WriteBatch::new();
                batch.put(block_key(height), record);
                batch.put(META_HEIGHT, height.to_be_bytes());
                self.blocks.write(batch)?;
            }
            Ok(())
        })();
        if out.is_err() {
            self.poisoned.store(true, Ordering::Relaxed);
        }
        out
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
        let iter = self
            .blocks
            .iter_at(Some(prefix), hi.as_deref(), true, &snap)?;
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

    /// stamp a module as backfilled at a boundary: clear the database and set
    /// the watermark + backfill floor. this is the honest answer when
    /// canonical state advanced without the op stream — the module's feed and
    /// views simply BEGIN at the boundary, visibly via the floor, instead of
    /// a watermark that silently claims pre-boundary coverage the feed never
    /// saw. crash story: watermark falls first, failures poison.
    ///
    /// the fold trigger is torn down for the wipe and re-registered after:
    /// its pending events describe rows the wipe deletes, and the wipe's own
    /// deletes must never reach the guest as feed traffic. `delete_trigger`
    /// discards pending events with the registration — exactly the clean
    /// slate a boundary stamp means.
    pub fn mark_backfilled(&self, module: &str, height: u64) -> Result<()> {
        if self.is_poisoned() {
            return Err(Error::Poisoned);
        }
        let m = self.module(module)?;
        let out = (|| -> Result<()> {
            if m.has_fold {
                m.db.delete_trigger(FOLD_TRIGGER)?;
            }
            let mut drop_mark = WriteBatch::new();
            drop_mark.delete(META_HEIGHT);
            m.db.write(drop_mark)?;
            clear_db(&m.db)?;
            let mut stamp = WriteBatch::new();
            stamp.put(META_HEIGHT, height.to_be_bytes());
            stamp.put(META_BACKFILL, height.to_be_bytes());
            m.db.write(stamp)?;
            if m.has_fold {
                create_fold_trigger(&m.db)?;
            }
            Ok(())
        })();
        if out.is_err() {
            self.poisoned.store(true, Ordering::Relaxed);
        }
        out
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
    /// to the index guest's `query` role, read-only at one MVCC snapshot.
    /// modules without a guest — or whose guest declares no view — answer
    /// [`Error::ViewUnsupported`]. a poisoned store still serves views:
    /// stale but consistent.
    pub fn view(&self, module: &str, req: &[u8]) -> Result<Vec<u8>> {
        let m = self.module(module)?;
        if !m.has_view {
            return Err(Error::ViewUnsupported);
        }
        m.db.query(GUEST_NAME, req).map_err(view_error)
    }

    /// a live feed of committed writes in `[lo, hi)` on one module's
    /// database — every host op-row write AND every guest fold write lands on
    /// it in commit order. the wait seam for anything that needs "the fold
    /// caught up to X": subscribe, act, block on the stream — never poll.
    pub fn subscribe(
        &self,
        module: &str,
        lo: &[u8],
        hi: Option<&[u8]>,
    ) -> Result<fluent31::Subscription> {
        Ok(self.db(module)?.subscribe(lo, hi)?)
    }

    /// cut a point-in-time archive of one database (a module id or
    /// [`BLOCKS_DB_ID`]) and return its complete file set, for the shipping
    /// lane (spec §7 lane 2): a fork archive is a self-contained database
    /// directory, so these files written verbatim to a fresh directory open
    /// as an identical database — watermark, backfill floor, rows, AND the
    /// installed index guest + trigger state (engine keyspace) included. the
    /// on-disk archive is transient: cut, read into memory, deleted — nothing
    /// to sweep after a normal return. safe against the live writers
    /// (fluent31 cuts are crash-atomic and pin their view).
    ///
    /// a poisoned store refuses: shipping a torn read model would hand the
    /// joiner exactly the state a rebuild exists to replace.
    pub fn checkpoint_files(&self, db: &str) -> Result<Vec<(String, Vec<u8>)>> {
        if self.is_poisoned() {
            return Err(Error::Poisoned);
        }
        let handle = if db == BLOCKS_DB_ID {
            &self.blocks
        } else {
            self.db(db)?
        };
        // a same-name leftover means an earlier cut crashed between create
        // and delete; deleting a fork that does not exist is the normal case
        // and not an error worth surfacing.
        if handle.list_forks()?.iter().any(|f| f.name == SHIP_FORK) {
            handle.delete_fork(SHIP_FORK)?;
        }
        let info = handle.fork(SHIP_FORK)?;
        let read = (|| -> std::io::Result<Vec<(String, Vec<u8>)>> {
            let mut files = Vec::new();
            for entry in self.disk.read_dir(&info.path)? {
                if entry.name == "LOCK" {
                    continue; // never present in an archive; skip defensively
                }
                let bytes = self.disk.read(&info.path.join(&entry.name))?;
                files.push((entry.name, bytes));
            }
            files.sort_by(|(a, _), (b, _)| a.cmp(b));
            Ok(files)
        })();
        let files = read.map_err(|e| Error::Shipping(format!("read {db} archive: {e}")))?;
        handle.delete_fork(SHIP_FORK)?;
        Ok(files)
    }
}

/// map a view invocation's engine error onto the tier's surface: a guest
/// `Fail` is the module refusing the REQUEST (its message travels), anything
/// else is the engine itself failing.
fn view_error(err: fluent31::Error) -> Error {
    match err {
        fluent31::Error::GuestFailed { output, .. } => {
            Error::View(String::from_utf8_lossy(&output).into_owned())
        }
        other => Error::Engine(other),
    }
}

/// the converge marker stored under [`META_GUEST`]: which artifact this
/// database is converged on, and the roles it declared. lets a warm boot
/// skip every wasm compile — [`converge_guest`] trusts a matching marker
/// outright, because everything it vouches for (install, trigger state) is
/// durable engine state written before the marker.
#[derive(BorshSerialize, BorshDeserialize, PartialEq)]
struct GuestMarker {
    /// sha256 of the artifact bytes.
    hash: [u8; 32],
    has_fold: bool,
    has_view: bool,
}

/// converge one module's database onto its declared guest: install (an
/// overwrite-put — replacing bytes is the upgrade path) and register the fold
/// trigger when the guest folds; tear both down when the module ships no
/// guest. roles come from the CANDIDATE bytes (`wasm_entries`), so a broken
/// artifact refuses at open, not at first invocation. the marker written
/// LAST makes the whole converge idempotent-and-free on a warm boot: cranelift
/// compiles are expensive enough that paying them per open once blew e2e
/// boot deadlines. returns `(has_fold, has_view)`.
fn converge_guest(db: &Db, spec: &IndexModule) -> Result<(bool, bool)> {
    let marker = db
        .get(META_GUEST.as_bytes())?
        .and_then(|bytes| borsh::from_slice::<GuestMarker>(&bytes).ok());
    let Some(bytes) = spec.guest else {
        let fold_registered = db.list_triggers()?.iter().any(|t| t.name == FOLD_TRIGGER);
        if fold_registered {
            db.delete_trigger(FOLD_TRIGGER)?;
        }
        let installed = db.list_modules()?.iter().any(|m| m.name == GUEST_NAME);
        if installed {
            db.uninstall_module(GUEST_NAME)?;
        }
        if marker.is_some() {
            db.delete(META_GUEST)?;
        }
        return Ok((false, false));
    };
    let hash: [u8; 32] = sha2::Sha256::digest(bytes).into();
    if let Some(marker) = marker
        && marker.hash == hash
    {
        return Ok((marker.has_fold, marker.has_view));
    }
    let fold_registered = db.list_triggers()?.iter().any(|t| t.name == FOLD_TRIGGER);
    let roles = db.wasm_entries(bytes)?;
    let has_fold = roles.iter().any(|r| r == "on_apply");
    let has_view = roles.iter().any(|r| r == "query");
    db.install_module(GUEST_NAME, bytes)?;
    match (has_fold, fold_registered) {
        (true, false) => {
            create_fold_trigger(db)?;
        }
        (false, true) => db.delete_trigger(FOLD_TRIGGER)?,
        _ => {}
    }
    let marker = GuestMarker {
        hash,
        has_fold,
        has_view,
    };
    db.put(META_GUEST, borsh::to_vec(&marker)?)?;
    Ok((has_fold, has_view))
}

/// register the fold feed: [`GUEST_NAME`]'s `on_apply` over exactly the
/// host-written `op/` range, so bookkeeping writes never reach the guest.
fn create_fold_trigger(db: &Db) -> Result<()> {
    let hi = prefix_successor(OP_PREFIX.as_bytes());
    db.create_trigger(
        FOLD_TRIGGER,
        GUEST_NAME,
        Some(OP_PREFIX.as_bytes()),
        hi.as_deref(),
    )?;
    Ok(())
}

// ============================================================================
// shipped-index staging — the joiner side of the shipping lane. a fetched
// database lands here file by file, is committed with a marker once every
// byte is durable, and is adopted by the next [`IndexStore::open`]. the
// ordering mirrors the boundary stamp's crash story inverted: the stamp drops
// its watermark FIRST so interruption re-triggers; staging writes its marker
// LAST so interruption discards. free functions, not methods — the writer (a
// syncing joiner) stages against a base whose store is still open elsewhere
// in the process, and never needs a handle of its own. they take the
// [`IndexDisk`] to write through (production passes [`DiskFs`]); the mem arm
// drives this whole sequence with no tempdir.
// ============================================================================

/// one path component: non-empty, no separators or traversal, no hidden
/// files, and never the engine's lock file. shipped names cross a trust
/// boundary (an unverified server chose them), so anything else is refused.
fn valid_component(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.starts_with('.')
        && name != "LOCK"
        && !name.contains(['/', '\\'])
        && !name.contains('\0')
}

/// stage one shipped database's file set under `<base>/_staging/<db>` for
/// adoption at the next [`IndexStore::open`]. every file is fsynced before
/// return — the completion marker ([`commit_staged`]) must never become
/// durable ahead of the bytes it vouches for, or a crash could adopt garbage
/// and turn the next open into a boot failure.
pub fn stage_shipped_db(
    disk: &dyn IndexDisk,
    base: &Path,
    db: &str,
    files: &[(String, Vec<u8>)],
) -> Result<()> {
    if !valid_component(db) || db == STAGING_DIR {
        return Err(Error::Shipping(format!("invalid shipped db name {db:?}")));
    }
    if let Some((name, _)) = files.iter().find(|(name, _)| !valid_component(name)) {
        return Err(Error::Shipping(format!(
            "invalid shipped file name {name:?} for {db}"
        )));
    }
    let dir = base.join(STAGING_DIR).join(db);
    (|| -> std::io::Result<()> {
        disk.create_dir_all(&dir)?;
        for (name, bytes) in files {
            disk.write(&dir.join(name), bytes)?;
        }
        disk.sync_dir(&dir)?;
        Ok(())
    })()
    .map_err(|e| Error::Shipping(format!("stage {db}: {e}")))
}

/// mark a staged install complete. written LAST: only a marked staging
/// directory is adopted; everything else is discarded as a torn fetch.
pub fn commit_staged(disk: &dyn IndexDisk, base: &Path) -> Result<()> {
    let staging = base.join(STAGING_DIR);
    (|| -> std::io::Result<()> {
        disk.write(&staging.join(STAGING_COMPLETE), b"")?;
        disk.sync_dir(&staging)?;
        Ok(())
    })()
    .map_err(|e| Error::Shipping(format!("commit staged install: {e}")))
}

/// drop any staged install — the fetch failed partway and lane 1's heal is
/// the fallback. missing staging is a no-op, so callers can clean
/// unconditionally.
pub fn discard_staged(disk: &dyn IndexDisk, base: &Path) -> Result<()> {
    let staging = base.join(STAGING_DIR);
    if !disk.exists(&staging) {
        return Ok(());
    }
    disk.remove_dir_all(&staging)
        .map_err(|e| Error::Shipping(format!("discard staged install: {e}")))
}

/// adopt a complete staged install: swap each staged database directory into
/// place, then remove the staging root (marker included) LAST. re-entrant
/// across crashes — each directory rename is atomic, an interrupted sweep
/// leaves the marker and the not-yet-adopted remainder for the next open,
/// and a marker-less staging directory is discarded wholesale.
fn adopt_staged(disk: &dyn IndexDisk, base: &Path) -> Result<()> {
    let staging = base.join(STAGING_DIR);
    if !disk.exists(&staging) {
        return Ok(());
    }
    if !disk.exists(&staging.join(STAGING_COMPLETE)) {
        return discard_staged(disk, base);
    }
    (|| -> std::io::Result<()> {
        for entry in disk.read_dir(&staging)? {
            if !entry.is_dir {
                continue; // the marker file
            }
            let dest = base.join(&entry.name);
            if disk.exists(&dest) {
                disk.remove_dir_all(&dest)?;
            }
            disk.rename(&staging.join(&entry.name), &dest)?;
        }
        disk.remove_dir_all(&staging)?;
        Ok(())
    })()
    .map_err(|e| Error::Shipping(format!("adopt staged install: {e}")))
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

/// delete every user key in the database, in bounded batches, off one MVCC
/// snapshot — readers holding older snapshots keep serving while the sweep
/// runs. the engine keyspace (installed guest, trigger state) is invisible to
/// this iterator by construction and survives. the caller has already dropped
/// the watermark, so a crash mid-sweep re-triggers the stamp rather than
/// leaving a half-empty index live.
fn clear_db(db: &Db) -> Result<()> {
    let snap = db.snapshot();
    let iter = db.iter_at(None, None, false, &snap)?;
    let mut batch = WriteBatch::new();
    let mut staged = 0usize;
    for kv in iter {
        let (key, _) = kv?;
        batch.delete(key);
        staged += 1;
        if staged >= CLEAR_FLUSH_EVERY {
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

#[cfg(test)]
mod tests;
