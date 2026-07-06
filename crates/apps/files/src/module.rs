//! the native module glue: [`Files`] implements [`sdk::Module`] over the pure
//! [`Fs`] core. origin/env map in here; core `String` errors map out as
//! [`Error::Module`]; watch-notification emission (task 9) and the gc
//! watermark trigger (task 13) land here too.

use std::path::PathBuf;

use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};

use crate::disk::DiskStore;
use crate::fs::Fs;
use crate::state::Refs;
use crate::store::{MemRefs, RefsStore as _};
use crate::wire::{
    FilesMsg, PUTBLOB_FRAME_TAG, decode_msg, decode_query, decode_sync_req, encode_reply,
    encode_sync_resp, to_hex,
};

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
    /// the module data dir (`<dir>/objects` + `<dir>/refs`). objects now live in
    /// the disk odb; refs persistence stays MemRefs until task 6.
    #[allow(dead_code)]
    dir: PathBuf,
    fs: Fs<DiskStore>,
    refs_store: MemRefs,
}

impl Files {
    /// open (or create) the module over its data dir. the disk odb lives at
    /// `<dir>/objects`; a fresh refs store yields empty refs (refs persistence
    /// swaps to the disk pair in task 6).
    pub fn open(id: impl Into<ModuleId>, dir: PathBuf) -> Result<Self, Error> {
        let refs_store = MemRefs::new();
        let refs = match refs_store
            .load()
            .map_err(|e| Error::Module(format!("files: refs load: {e}")))?
        {
            Some((refs, _height, _gc_watermark)) => refs,
            None => Refs::default(),
        };
        let store = DiskStore::open(dir.join("objects"))
            .map_err(|e| Error::Module(format!("files: odb open: {e}")))?;
        Ok(Self {
            id: id.into(),
            dir,
            fs: Fs::new(store, refs),
            refs_store,
        })
    }

    /// the exact `root()` preimage — the refs image the snapshot lane ships.
    pub fn snapshot(&self) -> Vec<u8> {
        self.fs.snapshot_refs()
    }

    /// verify-then-adopt a peer's refs image against the expected root.
    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        self.fs
            .install_refs(bytes, expected.0)
            .map_err(Error::Module)
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
        let is_module = matches!(env.origin, Origin::Module(_));
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
                    // watch fan-out (task 9) turns each returned notification
                    // into an emitted follow-up msg here.
                    let _notifications = self
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
                    Ok(())
                }
                FilesMsg::Pin { snapshot, name } => {
                    self.fs.pin(&actor, snapshot, name).map_err(Error::Module)
                }
                FilesMsg::Unpin { name } => self.fs.unpin(&actor, name).map_err(Error::Module),
                FilesMsg::Watch { prefix, module_id } => self
                    .fs
                    .watch(&actor, is_module, prefix, module_id)
                    .map_err(Error::Module),
                FilesMsg::Unwatch { prefix, module_id } => self
                    .fs
                    .unwatch(&actor, is_module, prefix, module_id)
                    .map_err(Error::Module),
            },
        }
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        let q = decode_query(req).map_err(Error::Module)?;
        let reply = self.fs.query(q).map_err(Error::Module)?;
        Ok(encode_reply(&reply))
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        let Some((refs, height)) = self.fs.commit_block().map_err(Error::Module)? else {
            return Ok(());
        };
        // gc watermark bookkeeping is per-node glue and lands with task 13.
        self.refs_store
            .save(&refs, height, 0)
            .map_err(|e| Error::Module(format!("files: refs save: {e}")))?;
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.fs.abort_block();
        Ok(())
    }
}
