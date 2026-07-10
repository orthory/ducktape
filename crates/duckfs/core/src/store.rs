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
    /// integrity-verified presence: `Ok(true)` iff the object is present AND its
    /// stored bytes still hash to `id`. unlike [`ObjectStore::has`] (a path/key
    /// existence check), this proves the bytes are the ones the id names, so a
    /// self-heal terminator can distinguish "possessed" from "possessed-but-
    /// corrupt". the default TRUSTS presence — an in-memory store's bytes are the
    /// ones put and cannot bit-rot; a disk-backed store overrides to re-derive the
    /// hash and, on a mismatch, DELETE the corrupt file so it reads as absent and
    /// the fetch loop re-fetches a good copy. an absent object is `Ok(false)`, a
    /// genuine read error (not a proven mismatch) is an `Err`.
    fn verify(&self, id: &ObjectId) -> Result<bool, String> {
        Ok(self.has(id))
    }
    /// metadata-only: the stored object's kind and BODY byte length without
    /// reading the body. commit's execute-path chunk-length verification rides
    /// this, so an implementation must answer from metadata (a map lookup, a
    /// file stat + tag byte) — never a full body read on the consensus path.
    /// `Ok(None)` = absent, sharply distinct from a corrupt `Err`.
    fn stat(&self, id: &ObjectId) -> Result<Option<(Kind, u64)>, String>;
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

    fn stat(&self, id: &ObjectId) -> Result<Option<(Kind, u64)>, String> {
        Ok(self
            .objects
            .get(id)
            .map(|(kind, body)| (*kind, body.len() as u64)))
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

// these run under `--no-default-features` too: they touch only the pure core
// (no disk, no sdk), pinning the trait contract MemStore and DiskStore share.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_is_content_addressed_and_idempotent() {
        let mut s = MemStore::new();
        let id = s.put(Kind::Chunk, b"hello").unwrap();
        assert_eq!(id, object_id(Kind::Chunk, b"hello"));
        assert!(s.has(&id));
        // re-putting the same bytes yields the same id and never errors.
        assert_eq!(s.put(Kind::Chunk, b"hello").unwrap(), id);
        assert_eq!(s.list().unwrap(), vec![id]);
    }

    #[test]
    fn get_returns_stored_kind_and_body_absent_is_none() {
        let mut s = MemStore::new();
        let id = s.put(Kind::File, b"body").unwrap();
        assert_eq!(s.get(&id).unwrap(), Some((Kind::File, b"body".to_vec())));
        // absent is Ok(None) — distinct from any error path.
        let absent = object_id(Kind::File, b"missing");
        assert!(!s.has(&absent));
        assert_eq!(s.get(&absent).unwrap(), None);
    }

    #[test]
    fn stat_answers_kind_and_body_len_absent_is_none() {
        let mut s = MemStore::new();
        let id = s.put(Kind::Chunk, b"hello").unwrap();
        assert_eq!(s.stat(&id).unwrap(), Some((Kind::Chunk, 5)));
        let absent = object_id(Kind::Chunk, b"missing");
        assert_eq!(s.stat(&absent).unwrap(), None);
    }

    #[test]
    fn same_body_different_kind_are_distinct_objects() {
        let mut s = MemStore::new();
        let a = s.put(Kind::Chunk, b"x").unwrap();
        let b = s.put(Kind::Tree, b"x").unwrap();
        // the kind tag byte domain-separates the id, so one body maps to two ids.
        assert_ne!(a, b);
        assert_eq!(s.get(&a).unwrap(), Some((Kind::Chunk, b"x".to_vec())));
        assert_eq!(s.get(&b).unwrap(), Some((Kind::Tree, b"x".to_vec())));
    }

    #[test]
    fn list_is_sorted_and_remove_drops_the_object() {
        let mut s = MemStore::new();
        let mut ids = vec![
            s.put(Kind::Chunk, b"a").unwrap(),
            s.put(Kind::Chunk, b"b").unwrap(),
            s.put(Kind::Chunk, b"c").unwrap(),
        ];
        ids.sort();
        assert_eq!(s.list().unwrap(), ids);
        s.remove(&ids[1]).unwrap();
        let want = vec![ids[0].min(ids[2]), ids[0].max(ids[2])];
        assert_eq!(s.list().unwrap(), want);
        // removing an absent id is a no-op, not an error.
        s.remove(&object_id(Kind::Chunk, b"absent")).unwrap();
    }
}
