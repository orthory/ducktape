//! disk persistence: the content-addressed odb as [`DiskStore`] (task 5) and
//! the refs-file envelope as [`DiskRefs`] (task 6). the skeleton pins the
//! shapes only — the phase-1 glue still runs over the mem pair, so nothing
//! here executes yet.

use std::path::PathBuf;

use crate::objects::{Kind, ObjectId};
use crate::state::Refs;
use crate::store::{ObjectStore, RefsStore};

fn unimplemented_err() -> String {
    "files: unimplemented".into()
}

/// content-addressed object database over `dir/<aa>/<hex>` files (task 5).
pub struct DiskStore {
    /// odb root; task 5 lays out the fanout dirs and the tmp-sweep under it.
    #[allow(dead_code)]
    dir: PathBuf,
}

impl DiskStore {
    pub fn open(dir: PathBuf) -> Result<Self, String> {
        Ok(Self { dir })
    }
}

impl ObjectStore for DiskStore {
    fn put(&mut self, _kind: Kind, _body: &[u8]) -> Result<ObjectId, String> {
        Err(unimplemented_err())
    }

    fn get(&self, _id: &ObjectId) -> Result<Option<(Kind, Vec<u8>)>, String> {
        Err(unimplemented_err())
    }

    fn has(&self, _id: &ObjectId) -> bool {
        false
    }

    fn remove(&mut self, _id: &ObjectId) -> Result<(), String> {
        Err(unimplemented_err())
    }

    fn list(&self) -> Result<Vec<ObjectId>, String> {
        Err(unimplemented_err())
    }
}

/// the atomically-replaced refs file under `dir/refs` (task 6).
pub struct DiskRefs {
    /// module data dir holding the refs file; task 6 adds the envelope codec.
    #[allow(dead_code)]
    dir: PathBuf,
}

impl DiskRefs {
    pub fn open(dir: PathBuf) -> Result<Self, String> {
        Ok(Self { dir })
    }
}

impl RefsStore for DiskRefs {
    fn load(&self) -> Result<Option<(Refs, u64, u64)>, String> {
        Err(unimplemented_err())
    }

    fn save(&mut self, _refs: &Refs, _height: u64, _gc_watermark: u64) -> Result<(), String> {
        Err(unimplemented_err())
    }
}
