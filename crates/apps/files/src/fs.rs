//! the pure duckfs state machine — every consensus semantic lands here, over
//! the [`ObjectStore`] seam, with no sdk, no async, and no disk io anywhere.
//! the native glue (`module.rs`) maps origin/env in and notifications out.
//! tasks 7-14 fill the op/query/sync semantics; this skeleton pins the shapes.

use crate::objects::{Kind, ObjectId};
use crate::state::Refs;
use crate::store::ObjectStore;
use crate::wire::{Change, FilesQuery, FilesReply, FilesSyncReq, FilesSyncResp};

pub struct Fs<S: ObjectStore> {
    pub(crate) store: S,
    pub(crate) refs: Refs,
    pub(crate) pending: Option<Pending>,
}

/// per-block overlay: refs-next plus objects awaiting the store flush.
pub(crate) struct Pending {
    pub refs: Refs,
    pub objects: Vec<(Kind, Vec<u8>)>,
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
        self.refs.root_bytes()
    }

    pub fn refs(&self) -> &Refs {
        &self.refs
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

    /// flush pending objects into the store and promote the staged refs.
    /// returns the promoted (refs, height) for the CALLER to persist via its
    /// [`crate::store::RefsStore`], or `None` when the block staged nothing.
    pub fn commit_block(&mut self) -> Result<Option<(Refs, u64)>, String> {
        let Some(pending) = self.pending.take() else {
            return Ok(None);
        };
        for (kind, body) in &pending.objects {
            self.store.put(*kind, body)?;
        }
        self.refs = pending.refs;
        Ok(Some((self.refs.clone(), pending.height)))
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
        self.refs.encode()
    }

    /// verify-then-adopt: strict-decode a peer's refs image, check it against
    /// the expected root, then swap committed refs in and drop any staged block.
    pub fn install_refs(&mut self, bytes: &[u8], expected_root: [u8; 32]) -> Result<(), String> {
        let refs = Refs::decode(bytes)?;
        if refs.root_bytes() != expected_root {
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
