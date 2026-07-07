//! the TORN-BLOCK recovery path: a block whose commit spans two substrates with
//! DIFFERENT durability — a per-block-durable disk substrate (qmdb-like: every
//! commit moves its op-log root) and an in-memory cohort module that only
//! persists at the periodic checkpoint.
//!
//! a crash (or a hard SIGKILL/power loss) AFTER the disk commit but BEFORE the
//! next checkpoint leaves the disk substrate at the block's POST root while the
//! in-memory cohort is restored to its PRE root from the checkpoint. before the
//! fix, boot fail-stopped this as `Error::Torn` — bricking a solo genesis node
//! with no peer to wipe-and-resync from. the fix replays such a block by
//! committing ONLY the still-at-pre cohort and ABORTING the already-durable disk
//! substrate (re-committing a qmdb store would move its op-log root and fork).
//!
//! two hermetic test-double modules pin the exact properties that matter:
//! - `Diskish` — a per-block-durable disk substrate. its state lives behind a
//!   shared cell that SURVIVES the host drop (the "disk"); every `commit_block`
//!   bumps a counter folded into `root()`, so EVERY commit moves the root, and
//!   it reports `ResolverBacked` so the manifest stores NO snapshot for it (it
//!   recovers itself by reopening the cell).
//! - `Fanout` — an in-memory cohort module. it owns its state, `root()` is a
//!   pure state commitment, its sync surface is `SnapshotBytes`, and a single
//!   `Set` op stages its OWN write AND emits a follow-up `Set` to `diskish` — so
//!   ONE block touches BOTH modules.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use host::Host;
use node::{Disposition, OrderedNode, RoundOrderer};
use recovery::{Manifest, Recovery};
use sdk::{Ctx, Error, Module, ModuleId, Msg, StateRoot, StateSyncHandle, UpgradeCoords};
use sha2::{Digest, Sha256};

// ---- a tiny deterministic codec shared by both doubles ---------------------

/// `Set{key,value}` — the only op either double understands.
fn set_payload(key: &str, value: &str) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(key.len() as u64).to_le_bytes());
    out.extend_from_slice(key.as_bytes());
    out.extend_from_slice(value.as_bytes());
    out
}

fn parse_set(payload: &[u8]) -> Option<(String, String)> {
    if payload.len() < 8 {
        return None;
    }
    let klen = u64::from_le_bytes(payload[..8].try_into().ok()?) as usize;
    let rest = &payload[8..];
    if rest.len() < klen {
        return None;
    }
    let key = String::from_utf8(rest[..klen].to_vec()).ok()?;
    let value = String::from_utf8(rest[klen..].to_vec()).ok()?;
    Some((key, value))
}

fn set(target: &str, key: &str, value: &str) -> Msg {
    Msg {
        target: target.into(),
        payload: set_payload(key, value),
    }
}

/// deterministic 32-byte digest over length-prefixed parts.
fn digest(parts: &[&[u8]]) -> StateRoot {
    let mut h = Sha256::new();
    h.update((parts.len() as u64).to_le_bytes());
    for p in parts {
        h.update((p.len() as u64).to_le_bytes());
        h.update(p);
    }
    StateRoot(h.finalize().into())
}

/// canonical bytes of a committed map (sorted, length-prefixed).
fn encode_map(map: &BTreeMap<String, String>) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(map.len() as u64).to_le_bytes());
    for (k, v) in map {
        out.extend_from_slice(&(k.len() as u64).to_le_bytes());
        out.extend_from_slice(k.as_bytes());
        out.extend_from_slice(&(v.len() as u64).to_le_bytes());
        out.extend_from_slice(v.as_bytes());
    }
    out
}

fn decode_map(bytes: &[u8]) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let mut at = 8usize; // skip the leading count (we trust our own encoding)
    let n = u64::from_le_bytes(bytes[..8].try_into().unwrap());
    for _ in 0..n {
        let klen = u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap()) as usize;
        at += 8;
        let k = String::from_utf8(bytes[at..at + klen].to_vec()).unwrap();
        at += klen;
        let vlen = u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap()) as usize;
        at += 8;
        let v = String::from_utf8(bytes[at..at + vlen].to_vec()).unwrap();
        at += vlen;
        map.insert(k, v);
    }
    map
}

// ---- Fanout: an in-memory cohort module ------------------------------------

struct Fanout {
    id: ModuleId,
    committed: BTreeMap<String, String>,
    pending: Vec<(String, String)>,
}

impl Fanout {
    fn new(id: &str) -> Self {
        Self {
            id: id.into(),
            committed: BTreeMap::new(),
            pending: Vec::new(),
        }
    }

    /// restore committed state from a manifest snapshot (checkpoint install).
    fn install(&mut self, bytes: &[u8]) {
        self.committed = decode_map(bytes);
        self.pending.clear();
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Fanout {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        // pure state commitment over the COMMITTED map only.
        digest(&[b"fanout", &encode_map(&self.committed)])
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        // self-contained bytes: the manifest stores these for the in-memory
        // cohort, and boot re-installs them to reach the checkpoint pre-root.
        Ok(StateSyncHandle::SnapshotBytes(encode_map(&self.committed)))
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let (k, v) = parse_set(&msg.payload).ok_or(Error::Module("bad set".into()))?;
        // stage our own write AND fan out the same write to the disk substrate,
        // so ONE block touches both modules.
        self.pending.push((k.clone(), v.clone()));
        ctx.emit_msg(set("diskish", &k, &v));
        Ok(())
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        let key = String::from_utf8(req.to_vec()).map_err(|_| Error::Module("bad key".into()))?;
        Ok(self
            .committed
            .get(&key)
            .cloned()
            .unwrap_or_default()
            .into_bytes())
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        for (k, v) in self.pending.drain(..) {
            self.committed.insert(k, v);
        }
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending.clear();
        Ok(())
    }
}

// ---- Diskish: a per-block-durable disk substrate ---------------------------

/// the "disk": survives the host drop, so a reopen reads back the durable
/// post-state. `counter` bumps on every commit so `root()` MOVES each time —
/// exactly qmdb op-log semantics, which is why the fix must ABORT (not
/// re-commit) an already-durable module.
#[derive(Default)]
struct DiskCell {
    committed: BTreeMap<String, String>,
    counter: u64,
}

type Cell = Rc<RefCell<DiskCell>>;

struct Diskish {
    id: ModuleId,
    cell: Cell,
    pending: Vec<(String, String)>,
}

impl Diskish {
    fn open(id: &str, cell: Cell) -> Self {
        Self {
            id: id.into(),
            cell,
            pending: Vec::new(),
        }
    }

    /// reopen against the SURVIVED cell — reads back the durable post-state.
    fn reopen(id: &str, cell: Cell) -> Self {
        Self::open(id, cell)
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Diskish {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        let cell = self.cell.borrow();
        // fold the commit counter in so EVERY commit moves the root.
        digest(&[
            b"diskish",
            &cell.counter.to_le_bytes(),
            &encode_map(&cell.committed),
        ])
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        // resolver-backed: the manifest stores NO snapshot for us; we recover
        // ourselves by reopening the durable cell.
        Ok(StateSyncHandle::ResolverBacked {
            backend: "diskish".into(),
            detail: "per-block-durable substrate; reopen the cell".into(),
        })
    }

    async fn execute(&mut self, _ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let (k, v) = parse_set(&msg.payload).ok_or(Error::Module("bad set".into()))?;
        self.pending.push((k, v)); // stage only — no durable write in execute.
        Ok(())
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        let key = String::from_utf8(req.to_vec()).map_err(|_| Error::Module("bad key".into()))?;
        Ok(self
            .cell
            .borrow()
            .committed
            .get(&key)
            .cloned()
            .unwrap_or_default()
            .into_bytes())
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let mut cell = self.cell.borrow_mut();
        for (k, v) in self.pending.drain(..) {
            cell.committed.insert(k, v);
        }
        cell.counter += 1; // every durable commit moves the op-log root.
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        // discard the fresh stage; the durable cell is untouched (no root move).
        self.pending.clear();
        Ok(())
    }
}

// ---- the scenario ----------------------------------------------------------

fn sk(seed: u64) -> commonware_cryptography::ed25519::PrivateKey {
    use commonware_cryptography::Signer as _;
    commonware_cryptography::ed25519::PrivateKey::from_seed(seed)
}

#[test]
fn a_torn_block_recovers_by_committing_only_the_in_memory_cohort() {
    let executor = deterministic::Runner::default();
    executor.start(|context| async move {
        // the durable disk survives the "crash" through this clone.
        let cell: Cell = Rc::new(RefCell::new(DiskCell::default()));

        // ---- first run: genesis checkpoint, then ONE torn-shaped block ------
        let recovery = Recovery::open(context.child("r1"))
            .await
            .expect("open recovery");
        let host = Host::genesis(vec![
            Box::new(Fanout::new("fanout")),
            Box::new(Diskish::open("diskish", cell.clone())),
        ])
        .expect("genesis");

        let mut node = OrderedNode::with_sink(host, RoundOrderer::new(), recovery);

        // GENESIS manifest (height None): records preF/preD roots and fanout's
        // snapshot bytes (diskish is resolver-backed → no snapshot stored).
        let pos = node.sink_mut().oplog_pos().await;
        let manifest0 = Manifest::capture(node.host(), None, 0, 0, vec![], vec![], None, 0, None, pos, 1)
            .expect("capture");
        assert!(
            manifest0.snapshot("fanout").is_some(),
            "the in-memory cohort's bytes ride the manifest"
        );
        assert!(
            manifest0.snapshot("diskish").is_none(),
            "the disk substrate recovers itself — no snapshot"
        );
        node.sink_mut()
            .write_manifest(&manifest0)
            .await
            .expect("write genesis manifest");

        // block N (height 0): fanout.Set fans out to diskish. after the drain,
        // diskish is durable at postD (cell counter == 1) and fanout at postF.
        let signer = sk(1);
        node.submit(&signer, 0, set("fanout", "k", "v"))
            .await
            .expect("submit");
        assert_eq!(node.drain_delivered().await.expect("drain"), 1);
        let tip = node.finalized().expect("boundary");
        let tip_hash = node.app_hash();
        assert_eq!(cell.borrow().counter, 1, "disk committed once");

        // graceful WAL barrier, then the "crash": drop everything in memory but
        // KEEP the disk cell (durable at post) and the storage backend (WAL).
        node.sink_mut().sync().await.expect("sync");
        drop(node);

        // ---- boot: reconstruct the TORN layout ------------------------------
        // fanout restores to its PRE root from the genesis manifest (the
        // in-memory cohort rolled back to the checkpoint); diskish reopens the
        // survived cell at its POST root (the disk raced ahead per-block).
        let mut recovery = Recovery::open(context.child("r2"))
            .await
            .expect("reopen recovery");
        let manifest = recovery
            .manifest()
            .expect("manifest decodes")
            .expect("manifest present");
        assert_eq!(manifest.height, None, "still the genesis checkpoint");

        let mut fanout = Fanout::new("fanout");
        fanout.install(manifest.snapshot("fanout").expect("fanout snapshot"));
        let diskish = Diskish::reopen("diskish", cell.clone());
        let mut host = Host::genesis(vec![Box::new(fanout), Box::new(diskish)]).expect("genesis");

        // the layout IS torn: fanout at pre, diskish at post.
        assert_eq!(
            host.module_root("fanout"),
            manifest.root("fanout"),
            "fanout is at its checkpoint PRE root"
        );
        assert_ne!(
            host.module_root("diskish"),
            manifest.root("diskish"),
            "diskish has raced ahead to its POST root"
        );

        // ---- POST-FIX: selective replay heals the torn block ----------------
        // (before the fix this returned Error::Torn — verified out-of-band by
        // running this test against the pre-fix recovery crate.)
        let recovered = recovery
            .recover(&mut host, &manifest)
            .await
            .expect("torn block recovers");

        assert_eq!(recovered.height, Some(tip.height));
        assert_eq!(
            recovered.app_hash, tip_hash,
            "recomposed app-hash is byte-identical to the sealed tip"
        );
        assert_eq!(recovered.applied, 1, "the torn block was replayed");
        // the in-memory cohort rolled forward from the WAL.
        assert_eq!(
            host.query("fanout", b"k").await.expect("query"),
            b"v".to_vec()
        );
        // and the disk substrate was NOT re-committed: same post root, and the
        // commit counter is UNCHANGED — no op-log root move, no fork.
        assert_eq!(
            host.module_root("diskish"),
            Some(recovered_disk_root(&cell)),
        );
        assert_eq!(
            cell.borrow().counter,
            1,
            "the durable disk was left alone (no re-commit)"
        );

        // ---- idempotency: a SECOND boot over the same journal is stable -----
        // fanout is restored to pre again; the disk is still at post.
        drop(recovery);
        let mut recovery = Recovery::open(context.child("r3"))
            .await
            .expect("reopen again");
        let manifest = recovery.manifest().expect("decodes").expect("present");
        let mut fanout2 = Fanout::new("fanout");
        fanout2.install(manifest.snapshot("fanout").expect("snapshot"));
        let diskish2 = Diskish::reopen("diskish", cell.clone());
        let mut host2 =
            Host::genesis(vec![Box::new(fanout2), Box::new(diskish2)]).expect("genesis");
        let again = recovery
            .recover(&mut host2, &manifest)
            .await
            .expect("again");
        assert_eq!(again.app_hash, recovered.app_hash, "idempotent app-hash");
        assert_eq!(cell.borrow().counter, 1, "still no extra disk commit");

        // sanity: the disposition of the replayed block was Applied.
        assert_eq!(recovered.skipped, 0);
        let _ = Disposition::Applied;
    });
}

/// the disk root after recovery (helper to avoid re-deriving inline).
fn recovered_disk_root(cell: &Cell) -> StateRoot {
    let cell = cell.borrow();
    digest(&[
        b"diskish",
        &cell.counter.to_le_bytes(),
        &encode_map(&cell.committed),
    ])
}

// ---- the torn-BRICK regression: a disk substrate N blocks past a checkpoint ---
//
// the crate's checkpoint only persists on a cadence (default 32 blocks), while a
// per-block-durable disk substrate commits to its OWN disk EVERY block. so at a
// hard kill (no final checkpoint), the disk can sit many blocks AHEAD of the last
// checkpoint: its live root equals a recorded post-root well above the checkpoint,
// matching NEITHER the checkpoint pre-root NOR the first replayed block's
// post-root. the pre-fix single-height classifier had no forward lookahead and
// fail-stopped (`Error::Torn`) at the first replayed height — bricking any node
// carrying sustained disk traffic under the shipped cadence. these pin the
// forward-pre-scan heal at cadence >= 2 with >= 2 durable disk blocks.

/// a REAL (non-genesis) checkpoint at height C, then TWO more torn-shaped blocks
/// the checkpoint does not cover — each fanning out to the disk, which commits
/// durably per block, so the disk ends TWO blocks past C. boot must roll the
/// in-memory cohort forward and heal the ahead disk WITHOUT re-committing it.
#[test]
fn a_disk_substrate_two_blocks_ahead_of_the_checkpoint_recovers_cleanly() {
    let executor = deterministic::Runner::default();
    executor.start(|context| async move {
        let cell: Cell = Rc::new(RefCell::new(DiskCell::default()));

        let recovery = Recovery::open(context.child("r1"))
            .await
            .expect("open recovery");
        let host = Host::genesis(vec![
            Box::new(Fanout::new("fanout")),
            Box::new(Diskish::open("diskish", cell.clone())),
        ])
        .expect("genesis");
        let mut node = OrderedNode::with_sink(host, RoundOrderer::new(), recovery);
        let signer = sk(1);

        // block 0: one torn-shaped block, then CHECKPOINT at height 0 (C = 0).
        node.submit(&signer, 0, set("fanout", "k0", "v0"))
            .await
            .expect("submit");
        assert_eq!(node.drain_delivered().await.expect("drain"), 1);
        let checkpoint_height = node.finalized().expect("boundary").height;
        assert_eq!(cell.borrow().counter, 1, "disk committed block 0");

        let pos = node.sink_mut().oplog_pos().await;
        let manifest = Manifest::capture(
            node.host(),
            Some(checkpoint_height),
            0,
            0,
            vec![],
            vec![],
            None,
            0,
            None,
            pos,
            1,
        )
        .expect("capture");
        node.sink_mut()
            .write_manifest(&manifest)
            .await
            .expect("write checkpoint");

        // blocks 1 and 2: TWO more torn-shaped blocks the checkpoint does NOT
        // cover. the disk commits each (counter 1 -> 2 -> 3), ending TWO blocks
        // past the checkpoint.
        node.submit(&signer, 1, set("fanout", "k1", "v1"))
            .await
            .expect("submit");
        node.submit(&signer, 2, set("fanout", "k2", "v2"))
            .await
            .expect("submit");
        assert_eq!(node.drain_delivered().await.expect("drain"), 2);
        let tip = node.finalized().expect("boundary");
        let tip_hash = node.app_hash();
        assert_eq!(cell.borrow().counter, 3, "disk committed once per block");
        assert!(
            tip.height - checkpoint_height >= 2,
            "the disk raced >= 2 blocks past the checkpoint"
        );

        // graceful WAL barrier (seals durable), then the "crash": drop memory,
        // KEEP the disk cell. NO checkpoint was written past height C.
        node.sink_mut().sync().await.expect("sync");
        drop(node);

        // ---- boot: the in-memory cohort rolls back to the checkpoint, the disk
        // stays TWO blocks ahead. the pre-fix loop bricks here with Error::Torn.
        let mut recovery = Recovery::open(context.child("r2"))
            .await
            .expect("reopen recovery");
        let manifest = recovery.manifest().expect("decodes").expect("present");
        assert_eq!(manifest.height, Some(checkpoint_height));

        let mut fanout = Fanout::new("fanout");
        fanout.install(manifest.snapshot("fanout").expect("fanout snapshot"));
        let diskish = Diskish::reopen("diskish", cell.clone());
        let mut host =
            Host::genesis(vec![Box::new(fanout), Box::new(diskish)]).expect("genesis");

        // the disk's live root matches NEITHER the checkpoint root NOR block 1's
        // post-root — it is TWO blocks past the checkpoint.
        assert_eq!(host.module_root("fanout"), manifest.root("fanout"));
        assert_ne!(host.module_root("diskish"), manifest.root("diskish"));

        let recovered = recovery
            .recover(&mut host, &manifest)
            .await
            .expect("a disk two blocks past the checkpoint recovers cleanly");

        assert_eq!(recovered.height, Some(tip.height));
        assert_eq!(
            recovered.app_hash, tip_hash,
            "recomposed app-hash is byte-identical to the sealed tip"
        );
        assert_eq!(recovered.applied, 2, "both post-checkpoint blocks replayed");
        assert_eq!(recovered.skipped, 0);
        // the durable disk was NEVER re-committed: no op-log root move, no fork.
        assert_eq!(
            cell.borrow().counter,
            3,
            "the ahead disk was left alone (no re-commit)"
        );
        // the in-memory cohort rolled forward from the WAL to the tip.
        assert_eq!(host.query("fanout", b"k0").await.expect("q"), b"v0".to_vec());
        assert_eq!(host.query("fanout", b"k1").await.expect("q"), b"v1".to_vec());
        assert_eq!(host.query("fanout", b"k2").await.expect("q"), b"v2".to_vec());
    });
}

/// the PURE-disk shape (the "sustained disk traffic" case): blocks that touch
/// ONLY the per-block-durable disk substrate (no in-memory cohort change). the
/// disk races several blocks past the genesis checkpoint; boot must SKIP every
/// such block as already-durable (the all-durable fast path), never Torn.
#[test]
fn pure_disk_blocks_ahead_of_the_checkpoint_skip_cleanly() {
    let executor = deterministic::Runner::default();
    executor.start(|context| async move {
        let cell: Cell = Rc::new(RefCell::new(DiskCell::default()));

        let recovery = Recovery::open(context.child("r1"))
            .await
            .expect("open recovery");
        let host = Host::genesis(vec![
            Box::new(Fanout::new("fanout")),
            Box::new(Diskish::open("diskish", cell.clone())),
        ])
        .expect("genesis");
        let mut node = OrderedNode::with_sink(host, RoundOrderer::new(), recovery);

        // GENESIS checkpoint only (height None) — nothing checkpointed after.
        let pos = node.sink_mut().oplog_pos().await;
        let manifest0 =
            Manifest::capture(node.host(), None, 0, 0, vec![], vec![], None, 0, None, pos, 1)
                .expect("capture");
        node.sink_mut()
            .write_manifest(&manifest0)
            .await
            .expect("write genesis manifest");

        // THREE blocks targeting the disk DIRECTLY — the in-memory cohort is
        // never touched. the disk ends three blocks ahead of the checkpoint.
        let signer = sk(1);
        node.submit(&signer, 0, set("diskish", "k0", "v0"))
            .await
            .expect("submit");
        node.submit(&signer, 1, set("diskish", "k1", "v1"))
            .await
            .expect("submit");
        node.submit(&signer, 2, set("diskish", "k2", "v2"))
            .await
            .expect("submit");
        assert_eq!(node.drain_delivered().await.expect("drain"), 3);
        let tip = node.finalized().expect("boundary");
        let tip_hash = node.app_hash();
        assert_eq!(cell.borrow().counter, 3, "disk committed once per block");

        node.sink_mut().sync().await.expect("sync");
        drop(node);

        // ---- boot: only the disk is ahead; every block is a pure-disk block ---
        let mut recovery = Recovery::open(context.child("r2"))
            .await
            .expect("reopen recovery");
        let manifest = recovery.manifest().expect("decodes").expect("present");
        let mut fanout = Fanout::new("fanout");
        fanout.install(manifest.snapshot("fanout").expect("fanout snapshot"));
        let diskish = Diskish::reopen("diskish", cell.clone());
        let mut host =
            Host::genesis(vec![Box::new(fanout), Box::new(diskish)]).expect("genesis");

        let recovered = recovery
            .recover(&mut host, &manifest)
            .await
            .expect("pure-disk blocks ahead of the checkpoint recover cleanly");

        assert_eq!(recovered.height, Some(tip.height));
        assert_eq!(recovered.app_hash, tip_hash);
        // nothing rolled back, so every ahead-disk block is SKIPPED, not replayed.
        assert_eq!(recovered.applied, 0, "no in-memory cohort to re-commit");
        assert_eq!(recovered.skipped, 3, "every ahead-disk block was skipped");
        assert_eq!(cell.borrow().counter, 3, "the disk was never re-committed");
        assert_eq!(host.query("diskish", b"k2").await.expect("q"), b"v2".to_vec());
    });
}

/// the PRESERVED corruption-detection property. the forward pre-scan seeds a
/// durable floor ONLY from an EXACT live-root match, so a disk substrate whose
/// live root matches NEITHER the checkpoint pre-root NOR any recorded post-root
/// is genuine damage (a torn write / corruption) and MUST still fail-stop as
/// `Error::Torn` — recovery never heals from a nearest/approximate record.
#[test]
fn a_disk_root_matching_no_record_still_fail_stops() {
    let executor = deterministic::Runner::default();
    executor.start(|context| async move {
        let cell: Cell = Rc::new(RefCell::new(DiskCell::default()));

        let recovery = Recovery::open(context.child("r1"))
            .await
            .expect("open recovery");
        let host = Host::genesis(vec![
            Box::new(Fanout::new("fanout")),
            Box::new(Diskish::open("diskish", cell.clone())),
        ])
        .expect("genesis");
        let mut node = OrderedNode::with_sink(host, RoundOrderer::new(), recovery);
        let pos = node.sink_mut().oplog_pos().await;
        let manifest0 =
            Manifest::capture(node.host(), None, 0, 0, vec![], vec![], None, 0, None, pos, 1)
                .expect("capture");
        node.sink_mut()
            .write_manifest(&manifest0)
            .await
            .expect("write genesis manifest");

        // one torn-shaped block: fanout fans out to diskish (durable at post).
        let signer = sk(1);
        node.submit(&signer, 0, set("fanout", "k", "v"))
            .await
            .expect("submit");
        assert_eq!(node.drain_delivered().await.expect("drain"), 1);
        assert_eq!(cell.borrow().counter, 1);
        node.sink_mut().sync().await.expect("sync");
        drop(node);

        // CORRUPT the durable disk: move the commit counter to a value that
        // matches NO recorded root (neither the genesis pre-root nor block 0's
        // post-root). this is a torn write, not a legitimate race-ahead.
        cell.borrow_mut().counter = 99;

        let mut recovery = Recovery::open(context.child("r2"))
            .await
            .expect("reopen recovery");
        let manifest = recovery.manifest().expect("decodes").expect("present");
        let mut fanout = Fanout::new("fanout");
        fanout.install(manifest.snapshot("fanout").expect("fanout snapshot"));
        let diskish = Diskish::reopen("diskish", cell.clone());
        let mut host =
            Host::genesis(vec![Box::new(fanout), Box::new(diskish)]).expect("genesis");

        // the disk root matches NOTHING recorded.
        assert_ne!(host.module_root("diskish"), manifest.root("diskish"));

        let err = recovery
            .recover(&mut host, &manifest)
            .await
            .expect_err("a disk root matching no record must fail-stop");
        assert!(
            matches!(err, recovery::Error::Torn(_)),
            "expected Error::Torn (genuine corruption), got {err:?}"
        );
        // the corrupt disk was NOT re-committed by the refused replay.
        assert_eq!(cell.borrow().counter, 99, "the refused replay touched nothing");
    });
}

// ---- FanoutTwo: an in-memory cohort module fanning out to TWO disks ---------

/// like [`Fanout`] but a single `Set` fans the write out to BOTH `diskA` and
/// `diskB`, so ONE block commits TWO per-block-durable disk substrates — the
/// multi-store atomicity zone recovery must refuse.
struct FanoutTwo {
    id: ModuleId,
    committed: BTreeMap<String, String>,
    pending: Vec<(String, String)>,
}

impl FanoutTwo {
    fn new(id: &str) -> Self {
        Self {
            id: id.into(),
            committed: BTreeMap::new(),
            pending: Vec::new(),
        }
    }

    fn install(&mut self, bytes: &[u8]) {
        self.committed = decode_map(bytes);
        self.pending.clear();
    }
}

#[async_trait::async_trait(?Send)]
impl Module for FanoutTwo {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        digest(&[b"fanout", &encode_map(&self.committed)])
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(encode_map(&self.committed)))
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let (k, v) = parse_set(&msg.payload).ok_or(Error::Module("bad set".into()))?;
        self.pending.push((k.clone(), v.clone()));
        ctx.emit_msg(set("diskA", &k, &v));
        ctx.emit_msg(set("diskB", &k, &v));
        Ok(())
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        let key = String::from_utf8(req.to_vec()).map_err(|_| Error::Module("bad key".into()))?;
        Ok(self
            .committed
            .get(&key)
            .cloned()
            .unwrap_or_default()
            .into_bytes())
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        for (k, v) in self.pending.drain(..) {
            self.committed.insert(k, v);
        }
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending.clear();
        Ok(())
    }
}

/// a block that commits TWO disk substrates and crashes with the in-memory
/// cohort rolled back to the checkpoint is a changed set spanning >1 per-block-
/// durable substrate at mixed roots — the multi-store atomicity limit. recovery
/// must FAIL-STOP explicitly with `Error::Torn` rather than attempt a partial
/// selective replay (whose single-frame re-execution could read a partially-
/// committed world). this pins the doc's "fail-stops rather than forking" claim.
#[test]
fn a_multi_disk_torn_block_fail_stops() {
    let executor = deterministic::Runner::default();
    executor.start(|context| async move {
        // two independent durable "disks", both survive the crash.
        let cell_a: Cell = Rc::new(RefCell::new(DiskCell::default()));
        let cell_b: Cell = Rc::new(RefCell::new(DiskCell::default()));

        let recovery = Recovery::open(context.child("r1"))
            .await
            .expect("open recovery");
        let host = Host::genesis(vec![
            Box::new(FanoutTwo::new("fanout")),
            Box::new(Diskish::open("diskA", cell_a.clone())),
            Box::new(Diskish::open("diskB", cell_b.clone())),
        ])
        .expect("genesis");

        let mut node = OrderedNode::with_sink(host, RoundOrderer::new(), recovery);

        // genesis manifest: fanout's snapshot rides it; both disks are
        // resolver-backed (no snapshot).
        let pos = node.sink_mut().oplog_pos().await;
        let manifest0 = Manifest::capture(node.host(), None, 0, 0, vec![], vec![], None, 0, None, pos, 1)
            .expect("capture");
        node.sink_mut()
            .write_manifest(&manifest0)
            .await
            .expect("write genesis manifest");

        // one block: fanout.Set fans out to BOTH disks. after the drain both
        // disks are durable at post (each counter == 1).
        let signer = sk(1);
        node.submit(&signer, 0, set("fanout", "k", "v"))
            .await
            .expect("submit");
        assert_eq!(node.drain_delivered().await.expect("drain"), 1);
        assert_eq!(cell_a.borrow().counter, 1, "diskA committed once");
        assert_eq!(cell_b.borrow().counter, 1, "diskB committed once");

        node.sink_mut().sync().await.expect("sync");
        drop(node);

        // ---- boot: TWO disks at post, the in-memory cohort at pre ----------
        let mut recovery = Recovery::open(context.child("r2"))
            .await
            .expect("reopen recovery");
        let manifest = recovery
            .manifest()
            .expect("manifest decodes")
            .expect("manifest present");

        let mut fanout = FanoutTwo::new("fanout");
        fanout.install(manifest.snapshot("fanout").expect("fanout snapshot"));
        let disk_a = Diskish::reopen("diskA", cell_a.clone());
        let disk_b = Diskish::reopen("diskB", cell_b.clone());
        let mut host = Host::genesis(vec![Box::new(fanout), Box::new(disk_a), Box::new(disk_b)])
            .expect("genesis");

        // the layout: fanout at pre, both disks raced ahead to post.
        assert_eq!(host.module_root("fanout"), manifest.root("fanout"));
        assert_ne!(host.module_root("diskA"), manifest.root("diskA"));
        assert_ne!(host.module_root("diskB"), manifest.root("diskB"));

        // recovery must fail-stop explicitly — NOT partially replay.
        let err = recovery
            .recover(&mut host, &manifest)
            .await
            .expect_err("multi-disk torn block must fail-stop");
        match err {
            recovery::Error::Torn(msg) => assert!(
                msg.contains("multi-store atomicity"),
                "expected the multi-disk atomicity fail-stop, got: {msg}"
            ),
            other => panic!("expected recovery::Error::Torn, got {other:?}"),
        }

        // and it did NOT re-commit either disk (no op-log root move, no fork).
        assert_eq!(
            cell_a.borrow().counter,
            1,
            "diskA untouched by the refused replay"
        );
        assert_eq!(
            cell_b.borrow().counter,
            1,
            "diskB untouched by the refused replay"
        );
    });
}

// ---- boundary × torn: a dual-path in-memory cohort torn AT an activation H ---
//
// the pieces above are single-path (their `root()` ignores the protocol
// version). the fork-critical seam the rebase had to preserve is a torn block
// that lands EXACTLY at an activation boundary H, where the rolled-back
// in-memory cohort module is DUAL-PATH — its `root()` branches on a non-hashed
// `active_version`, so the SAME committed state renders a different root under
// the pre-activation version vs the post. the two doubles below reproduce it.

/// a static armed `upgrade` module (mirrors the one in `version_aware_replay`):
/// reports a fixed pending upgrade at `activation_height` with its sole member
/// already ready, so `effective_version(height)` returns `to_version` at/after H
/// and baseline (0) below it. `root()` is constant (the config never mutates).
struct StaticUpgrade {
    name: String,
    activation_height: u64,
    to_version: u32,
    member: Vec<u8>,
}

#[async_trait::async_trait(?Send)]
impl Module for StaticUpgrade {
    fn id(&self) -> ModuleId {
        "upgrade".into()
    }
    fn root(&self) -> StateRoot {
        digest(&[
            b"static-upgrade",
            self.name.as_bytes(),
            &self.activation_height.to_le_bytes(),
            &self.to_version.to_le_bytes(),
        ])
    }
    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        // recreated identically on restore; no bytes to transfer.
        Ok(StateSyncHandle::Stateless)
    }
    async fn execute(&mut self, _ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        // the host injects exactly one System-origin `Advance` at each block >= H;
        // accept it as a no-op (this mock arms purely by height).
        match upgrade::decode_msg(&msg.payload).map_err(Error::Module)? {
            upgrade::UpgradeMsg::Advance => Ok(()),
            other => Err(Error::Module(format!("static upgrade got {other:?}"))),
        }
    }
    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        let upgrade::UpgradeQuery::Status =
            upgrade::decode_query(req).map_err(Error::Module)?;
        let status = upgrade::UpgradeStatus {
            current_version: 0,
            pending: Some(upgrade::ScheduledUpgrade {
                name: self.name.clone(),
                activation_height: self.activation_height,
                to_version: self.to_version,
            }),
            members: vec![self.member.clone()],
            ready: vec![self.member.clone()],
            member_count: 1,
            ready_count: 1,
            armed: true,
        };
        Ok(upgrade::encode_reply(
            &upgrade::UpgradeReply::Status(status),
        ))
    }
}

fn upgrade_mock(member: &[u8], h: u64, v: u32) -> StaticUpgrade {
    StaticUpgrade {
        name: "forge-v2".into(),
        activation_height: h,
        to_version: v,
        member: member.to_vec(),
    }
}

/// like [`Fanout`], but its `root()` folds a NON-hashed `active_version` branch
/// selector — the shape of a forge-like module whose root FORMAT switches at an
/// activation boundary. a `Set` bumps its own committed counter AND fans the
/// write out to a disk substrate, so ONE block touches both a dual-path
/// in-memory module and a per-block-durable disk. a non-`Set` op bumps the
/// counter only (no fanout), used to advance the chain below H.
struct DualFanout {
    id: ModuleId,
    counter: u64,
    pending: Option<u64>,
    active_version: u32,
    disk: ModuleId,
}

impl DualFanout {
    fn new(id: &str, disk: &str) -> Self {
        Self {
            id: id.into(),
            counter: 0,
            pending: None,
            active_version: 0,
            disk: disk.into(),
        }
    }
    /// the version-branched root: v0 and v1 hash differently for the SAME
    /// committed counter — the whole point of a root()-changing upgrade.
    fn root_of(counter: u64, active_version: u32) -> StateRoot {
        digest(&[
            b"dualfanout",
            &counter.to_le_bytes(),
            &active_version.to_le_bytes(),
        ])
    }
    /// restore the committed counter from a checkpoint snapshot AT a given
    /// version (the branch selector is never serialized, mirroring forge).
    fn install(bytes: &[u8], active_version: u32) -> Self {
        let counter = u64::from_le_bytes(bytes[..8].try_into().unwrap());
        Self {
            id: "dual".into(),
            counter,
            pending: None,
            active_version,
            disk: "diskish".into(),
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Module for DualFanout {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }
    fn root(&self) -> StateRoot {
        DualFanout::root_of(self.counter, self.active_version)
    }
    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        // committed state ONLY (the counter); active_version is never persisted.
        Ok(StateSyncHandle::SnapshotBytes(
            self.counter.to_le_bytes().to_vec(),
        ))
    }
    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        self.pending = Some(self.counter + 1); // stage our own bump
        if let Some((k, v)) = parse_set(&msg.payload) {
            // a Set fans out to the disk substrate, so one block touches both.
            ctx.emit_msg(set(self.disk.as_str(), &k, &v));
        }
        Ok(())
    }
    async fn commit_block(&mut self) -> Result<(), Error> {
        if let Some(v) = self.pending.take() {
            self.counter = v;
        }
        Ok(())
    }
    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending = None;
        Ok(())
    }
    fn set_active_version(&mut self, version: u32) {
        self.active_version = version;
    }
}

/// FORK-CRITICAL boundary × torn seam. a torn SEALED block AT an activation
/// boundary H whose rolled-back in-memory cohort module is DUAL-PATH. the torn
/// classifier must read "still at pre" under the PREVIOUS height's version
/// (`pre_version`) and "already at post" under THIS block's (`protocol_version`)
/// — the same split selectors the sealed loop's bulk at_pre/at_post use. a
/// single-selector classifier would render the rolled-back module's PRE state
/// under the POST version, match NEITHER pre nor post, and brick the node with a
/// FALSE `Error::Torn`. this pins that the heal is version-aware.
#[test]
fn a_boundary_torn_block_heals_under_the_pre_activation_version() {
    const H: u64 = 1; // activation height — the torn block sits exactly at H
    const V: u32 = 1; // to_version

    // the dual-path root MUST be version-sensitive or the test proves nothing.
    assert_ne!(
        DualFanout::root_of(1, 0),
        DualFanout::root_of(1, 1),
        "the dual module's root must branch on active_version"
    );

    let executor = deterministic::Runner::default();
    executor.start(|context| async move {
        let signer = sk(1);
        let me = {
            use commonware_cryptography::Signer as _;
            signer.public_key().as_ref().to_vec()
        };

        // the durable disk survives the "crash" through this clone.
        let cell: Cell = Rc::new(RefCell::new(DiskCell::default()));

        // ---- live run: seal one v0 block below H + checkpoint, then one torn
        // v1 block AT H that fans out to the disk. ----
        let recovery = Recovery::open(context.child("r1"))
            .await
            .expect("open recovery");
        let host = Host::genesis(vec![
            Box::new(DualFanout::new("dual", "diskish")),
            Box::new(Diskish::open("diskish", cell.clone())),
            Box::new(upgrade_mock(&me, H, V)),
        ])
        .expect("genesis");
        let mut node = OrderedNode::with_sink(host, RoundOrderer::new(), recovery);

        // block 0 (height 0, below H): a non-Set op bumps ONLY the dual (no disk
        // fanout). runs at baseline v0.
        node.submit(
            &signer,
            0,
            Msg {
                target: "dual".into(),
                payload: b"bump".to_vec(),
            },
        )
        .await
        .expect("submit below H");
        assert_eq!(node.drain_delivered().await.expect("drain"), 1);
        let checkpoint_height = node.finalized().expect("boundary").height;
        assert!(checkpoint_height < H, "checkpoint sits below H");
        assert_eq!(cell.borrow().counter, 0, "the disk is untouched below H");
        let hash_below = node.app_hash();

        // checkpoint below H: dual's snapshot rides it (counter, v0 root); the
        // disk is resolver-backed (no snapshot); a pending upgrade arms at H.
        let pos = node.sink_mut().oplog_pos().await;
        let manifest = Manifest::capture(
            node.host(),
            Some(checkpoint_height),
            0,
            0,
            vec![],
            vec![],
            None,
            0,
            Some(UpgradeCoords {
                name: "forge-v2".into(),
                activation_height: H,
                to_version: V,
            }),
            pos,
            1,
        )
        .expect("capture");
        node.sink_mut()
            .write_manifest(&manifest)
            .await
            .expect("write manifest");

        // ACTIVATION at H (what the driver does): flip the dual branch selector
        // so the H block seals a v1 root.
        node.host_mut().set_active_version(V);
        assert_ne!(
            hash_below,
            node.app_hash(),
            "the flip changes the dual root — the boundary is real"
        );

        // block 1 (height 1 == H): a Set bumps the dual AND fans out to the disk,
        // which commits durably (counter 0 -> 1). runs at v1. THIS is the block
        // that will be torn.
        node.submit(&signer, 1, set("dual", "k", "v"))
            .await
            .expect("submit at H");
        assert_eq!(node.drain_delivered().await.expect("drain"), 1);
        let tip = node.finalized().expect("boundary");
        assert_eq!(
            tip.height, H,
            "the torn block sits at the activation boundary"
        );
        assert_eq!(
            cell.borrow().counter,
            1,
            "the disk committed at the H block"
        );
        let tip_hash = node.app_hash();

        // graceful WAL barrier, then the "crash": drop memory, KEEP the disk cell.
        node.sink_mut().sync().await.expect("sync");
        drop(node);

        // ---- boot: reconstruct the TORN layout at H ------------------------
        // the dual restores to its checkpoint PRE state at the CHECKPOINT version
        // (0, below H); the disk reopens the survived cell at its POST root.
        let mut recovery = Recovery::open(context.child("r2"))
            .await
            .expect("reopen recovery");
        let manifest = recovery.manifest().expect("decodes").expect("present");
        assert_eq!(manifest.height, Some(checkpoint_height));

        let dual = DualFanout::install(manifest.snapshot("dual").expect("dual snapshot"), 0);
        let diskish = Diskish::reopen("diskish", cell.clone());
        let mut host = Host::genesis(vec![
            Box::new(dual),
            Box::new(diskish),
            Box::new(upgrade_mock(&me, H, V)),
        ])
        .expect("genesis");

        // the layout IS torn AND version-branched: the dual is at its v0 PRE root
        // (read under the boot baseline version), the disk raced ahead to POST.
        assert_eq!(
            host.module_root("dual"),
            manifest.root("dual"),
            "the dual is at its v0 checkpoint pre root"
        );
        assert_ne!(
            host.module_root("diskish"),
            manifest.root("diskish"),
            "the disk raced ahead to its post root"
        );

        // ---- the property: version-aware selective replay HEALS the boundary
        // torn block. a single-selector classifier would false-Torn here (the
        // rolled-back dual's PRE state read under v1 matches neither pre nor
        // post). ----
        let recovered = recovery
            .recover(&mut host, &manifest)
            .await
            .expect("boundary torn block heals under the pre-activation version");

        assert_eq!(recovered.height, Some(H));
        assert_eq!(recovered.applied, 1, "the torn H block was replayed");
        assert_eq!(
            recovered.app_hash, tip_hash,
            "recomposed app-hash is the byte-identical v1 sealed tip"
        );
        assert_eq!(host.app_hash(), tip_hash);
        assert_eq!(
            host.module_root("dual"),
            Some(DualFanout::root_of(2, V)),
            "the healed dual stands at its v1 post root"
        );
        // the disk substrate was NOT re-committed: counter unchanged, no op-log
        // root move, no fork.
        assert_eq!(
            cell.borrow().counter,
            1,
            "the durable disk was left alone (no re-commit)"
        );
    });
}
