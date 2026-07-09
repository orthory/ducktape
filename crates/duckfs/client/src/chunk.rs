//! content hashing, byte-identical to the module.
//!
//! the whole point: a client recomputes chunk ids and file ids with the exact
//! same preimage every validator uses, so it can name a file's id (for dedup and
//! for verifying a materialized checkout) with no network round-trip and no risk
//! of the cluster deriving a different id from the same bytes.

use std::collections::BTreeMap;

use duckfs_core::objects::{FileObj, object_id};
use duckfs_core::{CHUNK_SIZE, Kind, ObjectId};

/// split `bytes` into fixed [`CHUNK_SIZE`] (1 MiB) chunks and hash each one. a
/// chunk id is `sha256(chunk_tag ‖ bytes)` — the same object id the module
/// stages under, so a staged/committed chunk dedups exactly. an empty file has
/// no chunks (the module's empty-file rule).
pub fn chunk_ids(bytes: &[u8]) -> Vec<ObjectId> {
    if bytes.is_empty() {
        return Vec::new();
    }
    bytes
        .chunks(CHUNK_SIZE as usize)
        .map(|slice| object_id(Kind::Chunk, slice))
        .collect()
}

/// the file object id: `sha256(file_tag ‖ FileObj{size, chunks, meta}.encode())`.
/// the meta map is part of the preimage, so the id changes when meta changes —
/// which is why the `.duckfs` index carries meta, to recompute this exactly on a
/// status rehash. `symlink` content is a `FileObj` holding the target bytes, so
/// its id is derived the same way (the symlink-ness lives on the tree entry).
pub fn file_object_id(size: u64, chunks: &[ObjectId], meta: &BTreeMap<String, String>) -> ObjectId {
    let file = FileObj {
        size,
        chunks: chunks.to_vec(),
        meta: meta.clone(),
    };
    object_id(Kind::File, &file.encode())
}
