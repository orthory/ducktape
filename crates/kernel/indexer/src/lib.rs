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
//! `fold/…` belongs to the shared guest SHELL ([`IndexStore::fold_tip`] reads
//! the one key it writes); everything else belongs to the guest's fold. the
//! two watermarks answer different questions and are not interchangeable:
//! `meta/height` vouches for the FEED and bumps on every block, the fold tip
//! vouches for the DERIVED ROWS and only moves when ops arrive. guest code
//! itself lives in the engine's own reserved 0x00 keyspace — invisible to
//! scans and wiped by nothing this crate does; every node installs its own
//! from the bundled artifacts at open.
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
//! frame catch-up drives [`IndexStore::apply_block`] again) or by BACKFILLING
//! the source's own op rows below it ([`IndexStore::write_backfill_rows`] +
//! [`IndexStore::set_backfill_floor`], the joiner's inline join-seam walk).
//!
//! a MAPPER change is the other way derived rows go stale, and it needs no
//! boundary: the op feed is still there. [`converge_guest`] clears the derived
//! keyspace and re-drives the fold over the rows the database already holds,
//! leaving `op/` and `meta/` alone — a new mapper changes what the rows MEAN,
//! never what the feed saw. that re-drive completes before [`IndexStore::open`]
//! returns, so no reader ever sees the cleared keyspace.
//!
//! the full contract a per-module index guest must satisfy (fold rules, view
//! rules, when NOT to index) is `docs/records/specs/indexable-spec.md`; the
//! authoring surface is the `index-guest` crate.

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
pub use index_guest::{
    FOLD_PREFIX, FOLD_TIP, META_PREFIX, OP_PREFIX, OpRow, OriginKind, OriginTag, op_key,
    parse_op_key, user_handle,
};

/// key prefix of the per-block explorer rows in the internal blocks database.
pub const BLOCK_PREFIX: &str = "blk/";
/// directory name of the store-internal blocks database — reserved, never a
/// module id (the leading underscore keeps it out of the module namespace).
pub const BLOCKS_DB_ID: &str = "_blocks";
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
/// how many bytes of op rows one refold batch re-writes before flushing.
/// bounds memory the way [`CLEAR_FLUSH_EVERY`] does, by SIZE because a replay
/// stages whole op payloads rather than bare keys.
const REPLAY_FLUSH_BYTES: usize = 4 * 1024 * 1024;
/// how long a fold drain may sit at the SAME pending count before it is called
/// stuck. not a total budget — a long backlog drains as long as it needs, as
/// long as it keeps shrinking (see [`drain_fold`]).
const FOLD_DRAIN_STALL: Duration = Duration::from_secs(60);
/// ceiling on [`drain_fold`]'s poll backoff. every poll costs a queue count,
/// so a long drain must not ask a thousand times a second.
const FOLD_DRAIN_POLL_MAX: Duration = Duration::from_millis(50);
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
    /// a fold trigger reported a drain error with a backlog still pending —
    /// the views cannot catch up to the feed without a rebuild.
    #[error("indexer: fold stuck: {0}")]
    FoldStuck(String),
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
    /// the module-assigned stamp of the dispatch, verbatim (empty = none).
    pub assigned: Vec<u8>,
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
        assigned: op.assigned.clone(),
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
}

impl IndexStore {
    /// open (creating if missing) one database per module under `base` and
    /// converge each onto its declared index guest: install (or replace) the
    /// mapper bytes, register the fold trigger when the guest folds, tear
    /// both down when a module no longer ships a guest.
    ///
    /// only the declared module ids (plus `_blocks`) are opened. a
    /// `<base>/_staging` directory left by an older build is INERT GARBAGE —
    /// the shipped-index lane that wrote it is gone, nothing adopts or
    /// enumerates it, and it is safe to delete by hand.
    pub fn open(base: impl AsRef<Path>, modules: &[IndexModule]) -> Result<Self> {
        let base = base.as_ref().to_path_buf();
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

    /// the FOLD's own watermark: the `(height, seq)` of the last op row the
    /// module's guest folded, written by the shared shell inside the fold
    /// transaction ([`index_guest::FOLD_TIP`]).
    ///
    /// this is the only honest answer to "is my op in the view yet": the
    /// caller knows the `(H, seq)` its op landed at, and a tip at or past it
    /// means the derived rows for that op are committed. it is NOT general
    /// freshness — the fold advances only on op traffic, so a quiet module
    /// keeps an old tip while being perfectly current, and `None` (fresh
    /// database, boundary stamp, a mapper refold still in flight) means
    /// UNKNOWN, never zero.
    pub fn fold_tip(&self, module: &str) -> Result<Option<(u64, u32)>> {
        let db = self.db(module)?;
        Ok(db
            .get(FOLD_TIP.as_bytes())
            .map(|v| v.as_deref().and_then(index_guest::decode_fold_tip))?)
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

    /// block until every folding module's trigger backlog is drained, so a
    /// view read after this answers everything `apply_block` already fed.
    /// the deterministic lanes' commit barrier: fluent31 drains folds on a
    /// background runner, and a sim must not let a read (or the ws `changed`
    /// event that prompts one) race it. mirrors fluent31's own
    /// `wait_flushed` progress-wait idiom. `Err` = a fold failed with a
    /// backlog still pending — the views cannot catch up.
    pub fn wait_folds_drained(&self) -> Result<()> {
        for (id, m) in &self.modules {
            if !m.has_fold {
                continue;
            }
            drain_fold(&m.db, id)?;
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

    /// write verbatim op rows into a module's feed WITHOUT touching the
    /// watermark — the joiner's backfill of history below a boundary stamp
    /// (indexable spec §7). rows are `(op key, borsh row bytes)` exactly as
    /// the source stored them; batches flush by size like the refold's.
    ///
    /// # the ascending-order invariant this rests on
    ///
    /// the fold trigger is a CHANGES-mode trigger, so it delivers committed
    /// writes in commit order and the guest folds them in that order. these
    /// rows are therefore only correct if COMMIT ORDER IS KEY ORDER — which
    /// the caller guarantees by writing strictly ascending `(height, seq)`,
    /// pre-serving, on a node with no live folds, no ws subscribers, and no
    /// view readers. under that discipline the guest sees exactly the
    /// block-and-drain sequence a live feed would have delivered, and the
    /// fold tip advances monotonically to the last backfilled row. writing
    /// these out of order (or concurrently with live block folds) would hand
    /// the guest history backwards and is a defect, not a slow path.
    ///
    /// [`META_HEIGHT`] is deliberately untouched: the heal already stamped it
    /// at the boundary, and it vouches for the FEED's contiguity from the
    /// floor up. the FLOOR is what says "incomplete below" — lower it with
    /// [`IndexStore::set_backfill_floor`] once the walk completes, never here.
    /// only puts, so the delete-side contract (a failing feed row never
    /// vanishes) is untouched.
    pub fn write_backfill_rows(&self, module: &str, rows: &[(String, Vec<u8>)]) -> Result<()> {
        if self.is_poisoned() {
            return Err(Error::Poisoned);
        }
        let db = self.db(module)?;
        let out = (|| -> Result<()> {
            let mut batch = WriteBatch::new();
            let mut staged = 0usize;
            for (key, value) in rows {
                staged += key.len() + value.len();
                batch.put(key.as_bytes(), value.clone());
                if staged >= REPLAY_FLUSH_BYTES {
                    db.write(std::mem::replace(&mut batch, WriteBatch::new()))?;
                    staged = 0;
                }
            }
            if staged > 0 {
                db.write(batch)?;
            }
            Ok(())
        })();
        if out.is_err() {
            self.poisoned.store(true, Ordering::Relaxed);
        }
        out
    }

    /// set (or clear) a module's backfill floor and NOTHING else — no wipe, no
    /// trigger teardown, unlike [`IndexStore::mark_backfilled`]. the closing
    /// move of a completed op-row backfill: `Some(floor)` composes the
    /// source's own truncation into this node's honesty (a late-joined source
    /// has no rows below its floor either), `None` clears it outright — the
    /// feed reaches genesis and nothing is missing.
    pub fn set_backfill_floor(&self, module: &str, floor: Option<u64>) -> Result<()> {
        if self.is_poisoned() {
            return Err(Error::Poisoned);
        }
        let db = self.db(module)?;
        let out = match floor {
            Some(height) => db.put(META_BACKFILL, height.to_be_bytes()),
            None => db.delete(META_BACKFILL),
        };
        if out.is_err() {
            self.poisoned.store(true, Ordering::Relaxed);
        }
        Ok(out?)
    }

    /// re-derive a module's read model from the op feed it already holds:
    /// every derived key cleared, then the whole `op/` range re-driven through
    /// the guest in KEY order. `op/` and `meta/` are untouched — a refold
    /// changes what the rows MEAN, never what the feed saw.
    ///
    /// the closing move of a backfill that extended the feed DOWNWARD (the
    /// floored-module seam, indexable spec §7): rows below what the fold has
    /// already consumed arrive out of order by construction, so the read model
    /// disagrees with the feed until this runs — and it must run whether the
    /// walk finished or died holding half a range. that is what buys the seam
    /// its safety: nothing is wiped ahead of a pull that might fail.
    ///
    /// the same sequence [`converge_guest`] runs for a new mapper (feed down,
    /// clear, feed up, replay, drain), and a no-op for a module with no
    /// folding guest. failures poison, like every other write here.
    pub fn refold(&self, module: &str) -> Result<()> {
        if self.is_poisoned() {
            return Err(Error::Poisoned);
        }
        let m = self.module(module)?;
        if !m.has_fold {
            return Ok(());
        }
        let out = (|| -> Result<()> {
            // THE MARKER COMES DOWN FIRST AND GOES BACK UP LAST, exactly as
            // `converge_guest` writes it last: while the derived keyspace is
            // cleared and re-driven, NOTHING may vouch for it. A crash in
            // between must leave no marker at all, so the next `open` finds
            // one absent, refolds whole, and writes it — instead of matching
            // the guest hash, returning early, and serving a half-built read
            // model with a fold tip below its own feed.
            let marker = m.db.get(META_GUEST.as_bytes())?;
            m.db.delete(META_GUEST)?;
            // the feed goes down next: its pending events describe rows the
            // clear below is about to delete, and `delete_trigger` discards
            // them with the registration.
            m.db.delete_trigger(FOLD_TRIGGER)?;
            clear_derived(&m.db)?;
            create_fold_trigger(&m.db)?;
            replay_op_feed(&m.db)?;
            // AND WAIT FOR IT, for `converge_guest`'s reason: returning over a
            // cleared keyspace serves "no such page" for every page, which is
            // indistinguishable from a workspace that lost its documents.
            drain_fold(&m.db, module)?;
            if let Some(marker) = marker {
                m.db.put(META_GUEST, marker)?;
            }
            Ok(())
        })();
        if out.is_err() {
            self.poisoned.store(true, Ordering::Relaxed);
        }
        out
    }

    /// advance a module's feed watermark to `height` and NOTHING else — the
    /// closing move of a RESUMED backfill, where the rows between the old
    /// watermark and the boundary just landed verbatim, so the feed honestly
    /// covers them. no wipe, no floor change, no trigger teardown (unlike
    /// [`IndexStore::mark_backfilled`]); a watermark already at or past
    /// `height` stands, so this is idempotent. failures poison, like every
    /// other feed write.
    pub fn advance_watermark(&self, module: &str, height: u64) -> Result<()> {
        if self.is_poisoned() {
            return Err(Error::Poisoned);
        }
        let db = self.db(module)?;
        let out = (|| -> Result<()> {
            if read_height(db)? >= height {
                return Ok(());
            }
            db.put(META_HEIGHT, height.to_be_bytes())?;
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
/// overwrite-put — replacing bytes is the upgrade path), REFOLD the read model
/// the previous mapper left behind, and register the fold trigger when the
/// guest folds; tear both down when the module ships no guest. roles come from
/// the CANDIDATE bytes (`wasm_entries`), so a broken artifact refuses at open,
/// not at first invocation. the marker written LAST makes the whole converge
/// idempotent-and-free on a warm boot: cranelift compiles are expensive enough
/// that paying them per open once blew e2e boot deadlines. returns
/// `(has_fold, has_view)`.
///
/// # the refold, and why it is unconditional
///
/// derived rows are the OUTPUT of a mapper, so a database whose mapper changed
/// holds rows no installed code would produce — while its fold tip happily
/// vouches for them (`indexable-spec.md` §3.2.4: a mapper upgrade leaves a
/// PRESENT tip standing over the previous mapper's work). the honest fixes are
/// a boundary stamp — which throws away the op feed and lies about coverage —
/// or a replay. the feed is right there: `op/` is never wiped by a converge,
/// so a replay is a clear of the DERIVED keyspace plus a re-drive of the fold
/// over rows the database already holds.
///
/// it fires on any hash change rather than on a declared shape break because a
/// declaration is a number an author has to remember to bump, and forgetting
/// it is exactly the silent-stale-rows failure this exists to prevent. an
/// author cannot forget a hash.
///
/// the replay is WAITED OUT before this returns, so `open` answers with a
/// complete read model or not at all — the cost lands on boot latency, never
/// on a view that would otherwise answer an empty keyspace as if it were an
/// empty workspace.
///
/// ponytail: a view-only mapper edit therefore pays a full replay of the feed,
/// synchronously, at the next open. the tier is rebuildable by construction
/// and the cost is bounded by the feed the database holds, so this stays until
/// a measured boot regression asks for a `shape` field in [`GuestMarker`] to
/// narrow which changes refold.
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
    // the feed goes down FIRST: its pending events describe the previous
    // mapper's work, and `delete_trigger` discards them with the registration
    // — the same clean slate a boundary stamp takes, minus the amnesia.
    if fold_registered {
        db.delete_trigger(FOLD_TRIGGER)?;
    }
    db.install_module(GUEST_NAME, bytes)?;
    if has_fold {
        clear_derived(db)?;
        create_fold_trigger(db)?;
        replay_op_feed(db)?;
        // AND WAIT FOR IT. `replay_op_feed` only STAGES the re-writes; the
        // trigger runner folds them behind it. Returning here would hand the
        // node an open store whose read model is the freshly CLEARED keyspace
        // — `/v1/index/*/view` up and answering "no such page" for every page,
        // for as long as the whole feed takes to re-derive, with nothing on
        // the boot path consulting `fold_status` to know better. An empty
        // answer is worse than a slow boot: it is indistinguishable from a
        // workspace that lost its documents.
        //
        // `Err` here refuses the OPEN, matching what a broken artifact already
        // does at `wasm_entries` above: a guest that cannot fold its own feed
        // has no read model to serve, and saying so beats serving nothing
        // quietly.
        drain_fold(db, spec.id)?;
    }
    // written LAST, so an interrupted refold re-runs whole at the next open
    // instead of leaving a marker that vouches for a half-derived read model.
    let marker = GuestMarker {
        hash,
        has_fold,
        has_view,
    };
    db.put(META_GUEST, borsh::to_vec(&marker)?)?;
    Ok((has_fold, has_view))
}

/// block until one module's fold trigger has nothing queued: fluent31 drains
/// folds on a background runner, so both the sim's commit barrier and the
/// refold above have to join it rather than assume it. a module with no
/// trigger has nothing to wait for. `Err` = the fold cannot finish, either
/// because it FAILED with a backlog still pending or because it stopped making
/// progress — neither of which more waiting fixes.
///
/// the wait is bounded on PROGRESS, never on total time: a backfill of a whole
/// chain's history is a legitimately long drain, so the only honest stall
/// signal is a backlog that stops shrinking. a wedged fold that never records
/// an error would otherwise spin here forever — and this now runs inside a
/// joining node's seam, not just a sim.
fn drain_fold(db: &Db, module: &str) -> Result<()> {
    // NOT the backfill floor — the fewest events ever seen queued, which is
    // what "still shrinking" is measured against.
    let mut fewest_pending = u64::MAX;
    let mut since_progress = std::time::Instant::now();
    // ASKING IS NOT FREE: `list_triggers` counts the queue by iterating it, so
    // a poll costs O(pending). At a flat 1ms that burns a core and contends
    // with the runner exactly when the backlog is biggest — a whole chain's
    // op rows landing at a join seam. Start tight so a sim's commit barrier
    // still returns in a millisecond, then back off.
    let mut poll = Duration::from_millis(1);
    loop {
        let trigger = db
            .list_triggers()?
            .into_iter()
            .find(|t| t.name == FOLD_TRIGGER);
        let Some(trigger) = trigger else {
            return Ok(());
        };
        if trigger.pending == 0 {
            return Ok(());
        }
        if let Some(err) = trigger.last_error {
            return Err(Error::FoldStuck(format!("{module}: {err}")));
        }
        if trigger.pending < fewest_pending {
            fewest_pending = trigger.pending;
            since_progress = std::time::Instant::now();
        } else if since_progress.elapsed() >= FOLD_DRAIN_STALL {
            return Err(Error::FoldStuck(format!(
                "{module}: {} events pending, no progress for {}s",
                trigger.pending,
                FOLD_DRAIN_STALL.as_secs()
            )));
        }
        std::thread::sleep(poll);
        poll = (poll * 2).min(FOLD_DRAIN_POLL_MAX);
    }
}

/// delete every DERIVED key: everything a mapper wrote (its own rows plus the
/// shell's `fold/` tip), leaving the host-reserved `op/` feed and `meta/`
/// bookkeeping — the watermark and the backfill floor — untouched. those two
/// answer for the FEED, which a mapper change does not touch.
fn clear_derived(db: &Db) -> Result<()> {
    let snap = db.snapshot();
    let iter = db.iter_at(None, None, false, &snap)?;
    let mut batch = WriteBatch::new();
    let mut staged = 0usize;
    for kv in iter {
        let (key, _) = kv?;
        let host_reserved =
            key.starts_with(OP_PREFIX.as_bytes()) || key.starts_with(META_PREFIX.as_bytes());
        if host_reserved {
            continue;
        }
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

/// re-drive the fold over every op row the database already holds.
///
/// a changes-mode trigger delivers writes COMMITTED AFTER its registration, so
/// re-registering one over a populated range replays nothing. re-writing each
/// row does: an identical put is still a committed change, and capture happens
/// inside the commit critical section, so the guest receives the feed in key
/// order — which for `op/{height:016x}/{seq:04x}` IS block-and-drain order.
fn replay_op_feed(db: &Db) -> Result<()> {
    let lo = OP_PREFIX.as_bytes();
    let hi = prefix_successor(lo);
    let snap = db.snapshot();
    let iter = db.iter_at(Some(lo), hi.as_deref(), false, &snap)?;
    let mut batch = WriteBatch::new();
    let mut staged = 0usize;
    for kv in iter {
        let (key, value) = kv?;
        staged += key.len() + value.len();
        batch.put(key, value);
        if staged >= REPLAY_FLUSH_BYTES {
            db.write(std::mem::replace(&mut batch, WriteBatch::new()))?;
            staged = 0;
        }
    }
    if staged > 0 {
        db.write(batch)?;
    }
    Ok(())
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
