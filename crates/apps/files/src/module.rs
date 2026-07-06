//! the native module glue: [`Files`] implements [`sdk::Module`] over the pure
//! [`Fs`] core. origin/env map in here; core `String` errors map out as
//! [`Error::Module`]; watch-notification emission (task 9) and the gc
//! watermark trigger (task 13) land here too.

use std::path::PathBuf;

use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};

use crate::disk::{DiskRefs, DiskStore};
use crate::fs::{Fs, StagedObjects};
use crate::state::Refs;
use crate::store::{ObjectStore as _, RefsStore as _};
use crate::wire::{
    FilesMsg, GC_PERIOD_BLOCKS, PUTBLOB_FRAME_TAG, decode_msg, decode_query, decode_sync_req,
    encode_reply, encode_sync_resp, to_hex,
};

/// gc is due at `height` iff `height` has crossed into a new
/// [`GC_PERIOD_BLOCKS`]-wide window since the last swept height (`watermark`).
/// integer-divide both to the window index and fire when the block's window is
/// strictly ahead — so exactly one gc runs per period, on the first files-active
/// block past each boundary, identically on every node (the trigger is a pure
/// function of the op stream, never the wall clock). `pub(crate)` so the task-13
/// trigger test can table-drive the boundary (re-exported via `testkit`).
pub(crate) fn gc_due(height: u64, watermark: u64) -> bool {
    height / GC_PERIOD_BLOCKS > watermark / GC_PERIOD_BLOCKS
}

/// derive the acting identity from the dispatch origin: a module id verbatim,
/// `"ext:"` + lowercase hex for an external submitter (the prefix
/// domain-separates external identities from hex-looking module ids), or
/// `"system"`. never taken from the payload.
pub fn owner_of(origin: &Origin) -> String {
    match origin {
        Origin::Module(id) => id.clone(),
        Origin::External(bytes) => format!("ext:{}", to_hex(bytes)),
        Origin::System => "system".to_string(),
    }
}

pub struct Files {
    id: ModuleId,
    /// the pure state machine over the disk odb at `<dir>/objects`.
    fs: Fs<DiskStore>,
    /// the durable refs file at `<dir>/refs` — the block commit point.
    refs_store: DiskRefs,
    /// last block height whose refs are durable; per-node recovery bookkeeping,
    /// persisted in the refs-file envelope, never in the root preimage.
    durable_height: u64,
    /// gc watermark (per-node bookkeeping); the trigger policy lands in task 13,
    /// so it stays 0 here but is threaded through save/load already.
    gc_watermark: u64,
}

impl Files {
    /// open (or create) the module over its data dir. the disk odb lives at
    /// `<dir>/objects` and the durable refs file at `<dir>/refs`; a fresh dir
    /// yields empty refs, an existing one recovers the committed refs (durable
    /// restart), height, and gc watermark from the refs-file envelope.
    pub fn open(id: impl Into<ModuleId>, dir: PathBuf) -> Result<Self, Error> {
        let refs_store = DiskRefs::open(dir.clone())
            .map_err(|e| Error::Module(format!("files: refs open: {e}")))?;
        let (refs, durable_height, gc_watermark) = match refs_store
            .load()
            .map_err(|e| Error::Module(format!("files: refs load: {e}")))?
        {
            Some((refs, height, gc_watermark)) => (refs, height, gc_watermark),
            None => (Refs::default(), 0, 0),
        };
        let store = DiskStore::open(dir.join("objects"))
            .map_err(|e| Error::Module(format!("files: odb open: {e}")))?;
        Ok(Self {
            id: id.into(),
            fs: Fs::new(store, refs),
            refs_store,
            durable_height,
            gc_watermark,
        })
    }

    /// the exact `root()` preimage — the refs image the snapshot lane ships.
    pub fn snapshot(&self) -> Vec<u8> {
        self.fs.snapshot_refs()
    }

    /// verify-then-adopt a peer's refs image against the expected root, and
    /// persist it immediately so a restart right after sync recovers it. it is
    /// saved at the current per-node (height, gc_watermark); the sync-height
    /// refinement lands with the node integration (task 14).
    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        self.fs
            .install_refs(bytes, expected.0)
            .map_err(Error::Module)?;
        self.refs_store
            .save(self.fs.refs(), self.durable_height, self.gc_watermark)
            .map_err(|e| Error::Module(format!("files: refs save: {e}")))?;
        Ok(())
    }

    /// last height whose refs are durable — glue surface for the node sync
    /// integration (task 14).
    pub fn durable_height(&self) -> u64 {
        self.durable_height
    }

    /// `#[doc(hidden)]` test seam: stage a pending block directly so the real
    /// `commit_block` glue can be driven before the op semantics (tasks 7/9/10).
    #[doc(hidden)]
    pub fn stage_pending_for_test(&mut self, refs: Refs, height: u64, objects: StagedObjects) {
        self.fs.stage_pending(refs, height, objects);
    }

    /// `#[doc(hidden)]` test seam: shrink the per-owner staging quota so the
    /// quota-boundary logic is exercised without staging a full gibibyte.
    #[doc(hidden)]
    pub fn set_staging_quota_for_tests(&mut self, quota: u64) {
        self.fs.set_staging_quota_for_tests(quota);
    }

    /// `#[doc(hidden)]` test seam: shrink the per-call grep scan budget so the
    /// budget-boundary + resume-cursor logic is exercised without a multi-MiB
    /// fixture per call.
    #[doc(hidden)]
    pub fn set_grep_budget_for_tests(&mut self, budget: u64) {
        self.fs.set_grep_budget_for_tests(budget);
    }

    /// `#[doc(hidden)]` test seam: register a watch directly in committed refs so
    /// commit-time watch fan-out can be exercised before the watch op (task 10).
    #[doc(hidden)]
    pub fn insert_watch_for_test(
        &mut self,
        prefix: impl Into<String>,
        module_id: impl Into<String>,
    ) {
        self.fs
            .insert_watch_for_test(prefix.into(), module_id.into());
    }

    /// `#[doc(hidden)]` test seam: the committed head snapshot as hex — the base a
    /// per-path-CAS test threads into a follow-up commit.
    #[doc(hidden)]
    pub fn committed_head_for_test(&self) -> Option<String> {
        self.fs.committed_head_for_test()
    }

    /// `#[doc(hidden)]` test seam: run mark+sweep NOW over committed state,
    /// returning the removed-object count. gc is consensus-neutral, so this never
    /// moves the root; a corrupt store panics (the negative signal a reachability
    /// test wants). production gc runs only via the [`commit_block`] watermark
    /// trigger; this forces it for tests.
    #[doc(hidden)]
    pub fn force_gc(&mut self) -> u64 {
        self.fs.gc().expect("gc over a consistent store")
    }

    /// `#[doc(hidden)]` test seam: shrink the bounded history window so gc's
    /// window-expiry sweep can be driven with a few commits.
    #[doc(hidden)]
    pub fn set_history_window_for_tests(&mut self, n: usize) {
        self.fs.set_history_window_for_tests(n);
    }

    /// `#[doc(hidden)]` test seam: force the per-node gc watermark so the NEXT
    /// `commit_block` past a period boundary triggers gc for real.
    #[doc(hidden)]
    pub fn set_gc_watermark_for_tests(&mut self, watermark: u64) {
        self.gc_watermark = watermark;
    }

    /// `#[doc(hidden)]` test seam: the current per-node gc watermark — asserted
    /// after a trigger and after a reopen to prove it persisted.
    #[doc(hidden)]
    pub fn gc_watermark_for_test(&self) -> u64 {
        self.gc_watermark
    }

    /// `#[doc(hidden)]` test seam: does the committed odb hold `id`?
    #[doc(hidden)]
    pub fn odb_has_for_test(&self, id: &crate::ObjectId) -> bool {
        self.fs.odb_has_for_test(id)
    }

    /// `#[doc(hidden)]` test seam: the committed odb object count.
    #[doc(hidden)]
    pub fn odb_len_for_test(&self) -> usize {
        self.fs.odb_len_for_test()
    }

    /// `#[doc(hidden)]` test seam: the gc mark set over committed refs.
    #[doc(hidden)]
    pub fn gc_mark_for_test(&self) -> std::collections::BTreeSet<crate::ObjectId> {
        self.fs.gc_mark_for_test()
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Files {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        StateRoot(self.fs.root_bytes())
    }

    /// phase-1 bridge: refs are small, so the snapshot lane ships them whole;
    /// the resolver-backed handle lands with the node integration (task 14).
    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(self.snapshot()))
    }

    async fn serve_sync(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        let req = decode_sync_req(req).map_err(Error::Module)?;
        let resp = self.fs.serve_sync(req).map_err(Error::Module)?;
        Ok(encode_sync_resp(&resp))
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let env = ctx.env().clone();
        let actor = owner_of(&env.origin);
        // the watch origin gate treats system as a module origin: it may register a
        // watch for ANY module_id (the `actor == "system"` branch of the gate lets
        // it through), so system must map to `is_module = true`. an external
        // submitter is not a module and cannot register watches at all.
        let is_module = matches!(env.origin, Origin::Module(_) | Origin::System);
        match msg.payload.first() {
            Some(&PUTBLOB_FRAME_TAG) => self
                .fs
                .putblob(&actor, env.height, &msg.payload[1..])
                .map_err(Error::Module),
            _ => match decode_msg(&msg.payload).map_err(Error::Module)? {
                FilesMsg::Commit {
                    base_snapshot,
                    message,
                    changes,
                } => {
                    let notifications = self
                        .fs
                        .commit(
                            &actor,
                            env.height,
                            env.consensus_time,
                            base_snapshot,
                            message,
                            changes,
                        )
                        .map_err(Error::Module)?;
                    // watch fan-out: each notification becomes a follow-up msg at
                    // the watching module, re-dispatched after this execute returns
                    // (never a reentrant call). the payload is the task-9 shape the
                    // FsCap `decode_notify` reads back.
                    for n in notifications {
                        let payload = serde_json::to_vec(&serde_json::json!({
                            "duckfs_notify": {
                                "prefix": n.prefix,
                                "path": n.path,
                                "snapshot": n.snapshot,
                            }
                        }))
                        .expect("serde_json::Value serializes");
                        ctx.emit_msg(Msg {
                            target: n.module_id,
                            payload,
                        });
                    }
                    Ok(())
                }
                FilesMsg::Pin { snapshot, name } => self
                    .fs
                    .pin(&actor, env.height, snapshot, name)
                    .map_err(Error::Module),
                FilesMsg::Unpin { name } => self
                    .fs
                    .unpin(&actor, env.height, name)
                    .map_err(Error::Module),
                FilesMsg::Watch { prefix, module_id } => self
                    .fs
                    .watch(&actor, env.height, is_module, prefix, module_id)
                    .map_err(Error::Module),
                FilesMsg::Unwatch { prefix, module_id } => self
                    .fs
                    .unwatch(&actor, env.height, is_module, prefix, module_id)
                    .map_err(Error::Module),
            },
        }
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        let q = decode_query(req).map_err(Error::Module)?;
        let reply = self.fs.query(q).map_err(Error::Module)?;
        Ok(encode_reply(&reply))
    }

    /// the task-6 durability ordering — the load-bearing recovery contract
    /// (see [`DiskRefs`]). the committed root must never advance ahead of the
    /// durable refs file, or a crash reproduces this repo's historic
    /// torn-commit brick, so we persist strictly in this order and adopt LAST:
    ///
    /// 1. drain the pending block WITHOUT touching committed state (pure core)
    /// 2. flush its objects into the odb (idempotent, content-addressed)
    /// 3. fsync the touched odb dirs — the objects are now fully durable
    /// 4. save the refs file (atomic + parent-dir fsync) — the commit point
    /// 5. only now adopt the refs in core — the root moves here and nowhere else
    ///
    /// any error before step 4 aborts the whole block WITHOUT adopting: the node
    /// halts loudly on a fresh genesis rather than diverging, and a restart
    /// recovers the old refs, old root, and (harmless) orphan objects.
    async fn commit_block(&mut self) -> Result<(), Error> {
        // 1. pure hand-off — no object flush, no refs swap, no root movement.
        let Some((refs, height, objects)) = self.fs.commit_block() else {
            return Ok(()); // the block staged nothing
        };
        {
            let store = self.fs.store_mut();
            // 2. flush objects; a failure aborts before adoption (no torn root).
            for (kind, body) in &objects {
                store
                    .put(*kind, body)
                    .map_err(|e| Error::Module(format!("files: odb put: {e}")))?;
            }
            // 3. object dir-entries durable BEFORE the refs commit point below.
            store
                .sync_dirs()
                .map_err(|e| Error::Module(format!("files: odb sync: {e}")))?;
        }
        // 4. the commit point: refs file durable (atomic rename + parent fsync).
        self.refs_store
            .save(&refs, height, self.gc_watermark)
            .map_err(|e| Error::Module(format!("files: refs save: {e}")))?;
        // 5. adopt — root advances only now that the refs file is durable.
        self.fs.adopt_refs(refs);
        self.durable_height = height;

        // 6. gc watermark trigger — per-node bookkeeping, NOT consensus (the root
        // covers refs only, and unreachable-on-one-node is unreachable on every
        // node). run AFTER adopt so a gc crash can never lose committed state: the
        // block is already durable above, so any error here bricks the node loudly
        // (corruption) without rolling the block back. the deterministic cadence
        // is op-stream-driven — the first files-active block past each period
        // boundary — so nodes may lag in wall time but never diverge in state. the
        // advanced watermark lives ONLY in the refs-file envelope (never the root),
        // so re-save it here.
        if gc_due(height, self.gc_watermark) {
            self.fs
                .gc()
                .map_err(|e| Error::Module(format!("files: gc: {e}")))?;
            self.gc_watermark = height;
            self.refs_store
                .save(self.fs.refs(), self.durable_height, self.gc_watermark)
                .map_err(|e| Error::Module(format!("files: refs save (gc watermark): {e}")))?;
        }
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.fs.abort_block();
        Ok(())
    }
}
