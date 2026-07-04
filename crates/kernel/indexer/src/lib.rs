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
//!   the watermark move in ONE atomic [`WriteBatch`]);
//! - `op/{height:016x}/{seq:04x}` — one [`OpRow`] json envelope per dispatch
//!   the block applied to this module, in drain order (`seq` is the block-wide
//!   dispatch index, so cross-module ordering survives the per-module split);
//! - everything else — read-model keys owned by that module's registered
//!   [`ModuleIndexer`]; the two prefixes above are reserved and refused.
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
/// the per-module watermark key: 8-byte big-endian height.
const META_HEIGHT: &str = "meta/height";
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
    /// a [`ModuleIndexer`] tried to write into a reserved key space.
    #[error("indexer: derived write into reserved key {key:?} for module {module:?}")]
    ReservedKey { module: String, key: String },
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
    puts: Vec<(String, Vec<u8>)>,
    deletes: Vec<String>,
}

impl Derived {
    pub fn put(&mut self, key: impl Into<String>, value: impl Into<Vec<u8>>) {
        self.puts.push((key.into(), value.into()));
    }

    pub fn delete(&mut self, key: impl Into<String>) {
        self.deletes.push(key.into());
    }

    /// drain into the block batch, refusing reserved key spaces.
    fn drain_into(self, module: &str, batch: &mut WriteBatch) -> Result<()> {
        let check = |key: &str| -> Result<()> {
            if key.starts_with(OP_PREFIX) || key.starts_with(META_PREFIX) {
                return Err(Error::ReservedKey {
                    module: module.to_string(),
                    key: key.to_string(),
                });
            }
            Ok(())
        };
        for (key, value) in self.puts {
            check(&key)?;
            batch.put(key, value);
        }
        for key in self.deletes {
            check(&key)?;
            batch.delete(key);
        }
        Ok(())
    }
}

/// a per-module read-model mapper: turns one applied op into derived index
/// writes. pure data-in/data-out — no IO, no clock; everything it may write
/// goes through [`Derived`].
pub trait ModuleIndexer: Send + Sync {
    /// the module whose ops this mapper consumes.
    fn module(&self) -> &str;

    /// map one applied op to derived writes. `seq` is the block-wide dispatch
    /// index of the op, matching its `op/…` row.
    fn index_op(
        &self,
        height: u64,
        time: u64,
        seq: u32,
        origin: &OriginTag,
        payload: &[u8],
        out: &mut Derived,
    );
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
        Ok(Self {
            base,
            modules,
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
    /// watermark across all modules (a block only touches the modules it
    /// dispatched, so quieter modules lag the max).
    pub fn resume_height(&self) -> Result<u64> {
        let mut max = 0;
        for db in self.modules.values() {
            max = max.max(read_height(db)?);
        }
        Ok(max)
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
        // group by module, keeping the block-wide dispatch index as seq.
        let mut per: BTreeMap<&str, Vec<(u32, &AppliedOp)>> = BTreeMap::new();
        for (seq, op) in block.ops.iter().enumerate() {
            per.entry(op.module.as_str())
                .or_default()
                .push((seq as u32, op));
        }
        for (module, ops) in per {
            let db = self.db(module)?;
            if read_height(db)? >= block.height {
                continue; // replay of an already-folded block — idempotent skip
            }
            let mut batch = WriteBatch::new();
            for (seq, op) in ops {
                batch.put(
                    op_key(block.height, seq),
                    encode_row(block.height, seq, block.time, op)?,
                );
                if let Some(mapper) = self.mappers.get(module) {
                    let mut derived = Derived::default();
                    mapper.index_op(block.height, block.time, seq, &op.origin, &op.payload, &mut derived);
                    derived.drain_into(module, &mut batch)?;
                }
            }
            batch.put(META_HEIGHT, block.height.to_be_bytes());
            db.write(batch)?;
        }
        Ok(())
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
        let limit = limit.clamp(1, MAX_SCAN_LIMIT);

        // lo: the smallest key strictly above the cursor (append 0x00), else
        // the prefix itself. hi: the prefix successor, or open-ended when the
        // prefix is empty/all-0xff.
        let lo: Vec<u8> = match after {
            Some(a) if a >= prefix => {
                let mut lo = a.to_vec();
                lo.push(0);
                lo
            }
            _ => prefix.to_vec(),
        };
        let hi = prefix_successor(prefix);

        let snap = db.snapshot();
        let iter = db.iter_at(Some(&lo), hi.as_deref(), false, &snap)?;

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
        assert_eq!(store.applied_height("tasks").unwrap(), 0, "tasks saw no block");
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

    /// a mapper that writes one derived key per op and, on demand, misbehaves
    /// into the reserved key space.
    struct TestMapper {
        reserved: bool,
    }

    impl ModuleIndexer for TestMapper {
        fn module(&self) -> &str {
            "chat"
        }
        fn index_op(
            &self,
            height: u64,
            _time: u64,
            _seq: u32,
            origin: &OriginTag,
            payload: &[u8],
            out: &mut Derived,
        ) {
            if self.reserved {
                out.put("meta/evil", b"nope".to_vec());
                return;
            }
            let who = origin.id.clone().unwrap_or_default();
            out.put(format!("by-origin/{who}/{height:016x}"), payload.to_vec());
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

    #[test]
    fn prefix_successor_edges() {
        assert_eq!(prefix_successor(b"op/"), Some(b"op0".to_vec()));
        assert_eq!(prefix_successor(&[0x01, 0xff]), Some(vec![0x02]));
        assert_eq!(prefix_successor(&[0xff, 0xff]), None);
        assert_eq!(prefix_successor(b""), None);
    }
}
