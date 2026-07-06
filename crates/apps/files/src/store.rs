//! the persistence seams. the pure core only ever sees these two traits; the
//! mem pair beside them backs tests and the phase-1 module glue, and the disk
//! pair (`disk.rs`, tasks 5/6) lives behind the `native` feature.

use std::collections::BTreeMap;

use crate::objects::{Kind, ObjectId, object_id};
use crate::state::Refs;

pub trait ObjectStore {
    fn put(&mut self, kind: Kind, body: &[u8]) -> Result<ObjectId, String>;
    fn get(&self, id: &ObjectId) -> Result<Option<(Kind, Vec<u8>)>, String>;
    fn has(&self, id: &ObjectId) -> bool;
    fn remove(&mut self, id: &ObjectId) -> Result<(), String>;
    fn list(&self) -> Result<Vec<ObjectId>, String>;
}

pub trait RefsStore {
    /// None = fresh dir. Ok(Some((refs, height, gc_watermark))) otherwise.
    fn load(&self) -> Result<Option<(Refs, u64, u64)>, String>;
    fn save(&mut self, refs: &Refs, height: u64, gc_watermark: u64) -> Result<(), String>;
}

/// in-memory object store — tests and the phase-1 glue.
#[derive(Default)]
pub struct MemStore {
    objects: BTreeMap<ObjectId, (Kind, Vec<u8>)>,
}

impl MemStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl ObjectStore for MemStore {
    fn put(&mut self, kind: Kind, body: &[u8]) -> Result<ObjectId, String> {
        let id = object_id(kind, body);
        self.objects.insert(id, (kind, body.to_vec()));
        Ok(id)
    }

    fn get(&self, id: &ObjectId) -> Result<Option<(Kind, Vec<u8>)>, String> {
        Ok(self.objects.get(id).cloned())
    }

    fn has(&self, id: &ObjectId) -> bool {
        self.objects.contains_key(id)
    }

    fn remove(&mut self, id: &ObjectId) -> Result<(), String> {
        self.objects.remove(id);
        Ok(())
    }

    fn list(&self) -> Result<Vec<ObjectId>, String> {
        Ok(self.objects.keys().copied().collect())
    }
}

/// single-slot in-memory refs store: an empty slot is a fresh dir.
#[derive(Default)]
pub struct MemRefs {
    slot: Option<(Refs, u64, u64)>,
}

impl MemRefs {
    pub fn new() -> Self {
        Self::default()
    }
}

impl RefsStore for MemRefs {
    fn load(&self) -> Result<Option<(Refs, u64, u64)>, String> {
        Ok(self.slot.clone())
    }

    fn save(&mut self, refs: &Refs, height: u64, gc_watermark: u64) -> Result<(), String> {
        self.slot = Some((refs.clone(), height, gc_watermark));
        Ok(())
    }
}
