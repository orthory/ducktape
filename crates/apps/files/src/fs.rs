//! the pure duckfs state machine — every consensus semantic lands here, over
//! the [`ObjectStore`] seam, with no sdk, no async, and no disk io anywhere.
//! the native glue (`module.rs`) maps origin/env in and notifications out.
//! tasks 7-14 fill the op/query/sync semantics; this skeleton pins the shapes.

use crate::objects::{Kind, ObjectId};
use crate::state::{Refs, decode_refs, encode_refs, root_bytes};
use crate::store::ObjectStore;
use crate::wire::{Change, FilesQuery, FilesReply, FilesSyncReq, FilesSyncResp};

pub struct Fs<S: ObjectStore> {
    pub(crate) store: S,
    pub(crate) refs: Refs,
    pub(crate) pending: Option<Pending>,
}

/// a block's staged objects — `(kind, body)` pairs the glue flushes into the
/// odb at commit. named so the block-boundary signatures stay legible.
pub type StagedObjects = Vec<(Kind, Vec<u8>)>;

/// per-block overlay: refs-next plus objects awaiting the store flush.
pub(crate) struct Pending {
    pub refs: Refs,
    pub objects: StagedObjects,
    pub height: u64,
}

/// one watch hit produced by a commit; the glue turns each into an emitted
/// follow-up msg (task 9).
pub struct Notification {
    pub module_id: String,
    pub prefix: String,
    pub path: String,
    pub snapshot: String,
}

fn unimplemented_err() -> String {
    "files: unimplemented".into()
}

impl<S: ObjectStore> Fs<S> {
    pub fn new(store: S, refs: Refs) -> Self {
        Self {
            store,
            refs,
            pending: None,
        }
    }

    /// committed refs only — the pending overlay never leaks into the root.
    pub fn root_bytes(&self) -> [u8; 32] {
        root_bytes(&self.refs)
    }

    pub fn refs(&self) -> &Refs {
        &self.refs
    }

    /// direct access to the object store — the native glue (`module.rs`) needs
    /// the concrete `S` after [`Fs::commit_block`] to flush the block's objects
    /// and fsync their odb dirs, and by-hand durability tests drive the same
    /// seam. hidden because it is glue/test plumbing, not part of the semantic
    /// surface (all consensus reads/writes go through the typed methods).
    #[doc(hidden)]
    pub fn store_mut(&mut self) -> &mut S {
        &mut self.store
    }

    /// stage a pending block directly — a `#[doc(hidden)]` seam so durability
    /// tests can drive the block boundary before the op semantics (tasks 7/9/10)
    /// land. production staging happens inside the op methods.
    #[doc(hidden)]
    pub fn stage_pending(&mut self, refs: Refs, height: u64, objects: StagedObjects) {
        self.pending = Some(Pending {
            refs,
            objects,
            height,
        });
    }

    // ---- op surface (semantics land in tasks 7/9/10) ------------------------

    pub fn putblob(&mut self, _actor: &str, _height: u64, _bytes: &[u8]) -> Result<(), String> {
        Err(unimplemented_err())
    }

    pub fn commit(
        &mut self,
        _actor: &str,
        _height: u64,
        _time: u64,
        _base: Option<String>,
        _message: String,
        _changes: Vec<Change>,
    ) -> Result<Vec<Notification>, String> {
        Err(unimplemented_err())
    }

    pub fn pin(&mut self, _actor: &str, _snapshot: String, _name: String) -> Result<(), String> {
        Err(unimplemented_err())
    }

    pub fn unpin(&mut self, _actor: &str, _name: String) -> Result<(), String> {
        Err(unimplemented_err())
    }

    pub fn watch(
        &mut self,
        _actor: &str,
        _is_module: bool,
        _prefix: String,
        _module_id: String,
    ) -> Result<(), String> {
        Err(unimplemented_err())
    }

    pub fn unwatch(
        &mut self,
        _actor: &str,
        _is_module: bool,
        _prefix: String,
        _module_id: String,
    ) -> Result<(), String> {
        Err(unimplemented_err())
    }

    // ---- block boundary ------------------------------------------------------

    /// hand the block's staged `(refs, height, objects)` to the caller WITHOUT
    /// touching committed state — no object flush, no `self.refs` swap, no
    /// root movement. `None` when the block staged nothing.
    ///
    /// this pure hand-off is the whole point of task 6's durability ordering.
    /// the committed root must never run ahead of the durable refs file, or a
    /// crash mid-commit reproduces this repo's historic torn-commit brick (a
    /// disk module already at its post-root while its refs are still pre). so
    /// the caller (the native glue) must, in exactly this order:
    ///
    /// 1. `store_mut().put` every returned object (idempotent, content-addressed)
    /// 2. fsync the touched odb dirs (object dir-entries durable)
    /// 3. persist the refs file via `RefsStore::save` (the commit point)
    /// 4. only THEN [`Fs::adopt_refs`] — root moves here and nowhere else
    ///
    /// a crash before step 3 leaves the old refs file, the old root, and at
    /// worst some orphan objects (harmless: content-addressed, idempotently
    /// re-put on replay, swept by a later gc). a crash after step 3 has the new
    /// refs and — because step 2 preceded it — every object it names, durable.
    /// there is no torn window.
    pub fn commit_block(&mut self) -> Option<(Refs, u64, StagedObjects)> {
        let pending = self.pending.take()?;
        Some((pending.refs, pending.height, pending.objects))
    }

    /// adopt the block's refs as committed — the caller invokes this ONLY after
    /// the refs file is durably saved (see [`Fs::commit_block`]). this is the
    /// single place the committed root moves.
    pub fn adopt_refs(&mut self, refs: Refs) {
        self.refs = refs;
    }

    pub fn abort_block(&mut self) {
        self.pending = None;
    }

    // ---- read + sync surface (tasks 11/12/14) --------------------------------

    /// committed state only — never the pending overlay.
    pub fn query(&self, _q: FilesQuery) -> Result<FilesReply, String> {
        Err(unimplemented_err())
    }

    pub fn serve_sync(&self, _req: FilesSyncReq) -> Result<FilesSyncResp, String> {
        Err(unimplemented_err())
    }

    /// the exact `root_bytes` preimage — what the snapshot lane ships.
    pub fn snapshot_refs(&self) -> Vec<u8> {
        encode_refs(&self.refs)
    }

    /// verify-then-adopt: strict-decode a peer's refs image, check it against
    /// the expected root, then swap committed refs in and drop any staged block.
    pub fn install_refs(&mut self, bytes: &[u8], expected_root: [u8; 32]) -> Result<(), String> {
        let refs = decode_refs(bytes)?;
        if root_bytes(&refs) != expected_root {
            return Err("files: refs image does not match the expected root".into());
        }
        self.refs = refs;
        self.pending = None;
        Ok(())
    }

    pub fn missing_objects(&self, _limit: usize) -> Result<Vec<ObjectId>, String> {
        Err(unimplemented_err())
    }

    pub fn ingest_object(&mut self, _id: &ObjectId, _kind: u8, _body: &[u8]) -> Result<(), String> {
        Err(unimplemented_err())
    }

    /// mark + sweep now; the CALLER decides when (task 13 wires the trigger).
    pub fn gc(&mut self) -> Result<u64, String> {
        Err(unimplemented_err())
    }
}
