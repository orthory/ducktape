//! the pure core: chunk hashing byte-identical to the module, and the
//! `.duckfs` index (atomic save, round-trip, unknown-schema rejection).

use std::collections::BTreeMap;

use duckfs_client::chunk::{chunk_ids, file_object_id};
use duckfs_client::index::{EntryKind, Index, IndexEntry};
use duckfs_core::objects::{FileObj, object_id};
use duckfs_core::{CHUNK_SIZE, Kind};

/// the distinctive non-uniform pattern the large-file e2e uses (251 is prime, so
/// it aligns with no power-of-two chunk boundary).
fn pattern(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

#[test]
fn chunk_ids_and_file_id_are_byte_identical_to_the_module() {
    // 2 MiB + 1 byte: two full 1 MiB chunks and a one-byte tail — three ids.
    let size = 2 * CHUNK_SIZE as usize + 1;
    let bytes = pattern(size);

    let ids = chunk_ids(&bytes);
    assert_eq!(ids.len(), 3, "2 MiB + 1 = three fixed-size chunks");

    // each chunk id equals the module's hash over that exact slice.
    let expected: Vec<_> = bytes
        .chunks(CHUNK_SIZE as usize)
        .map(|c| object_id(Kind::Chunk, c))
        .collect();
    assert_eq!(ids, expected, "chunk ids match object_id(Chunk, slice)");

    // the file id equals object_id(File, FileObj{size, chunks, meta}.encode()) —
    // the same preimage the module derives, meta included.
    let mut meta = BTreeMap::new();
    meta.insert("kind".to_string(), "skill".to_string());
    let fid = file_object_id(bytes.len() as u64, &ids, &meta);
    let file = FileObj {
        size: bytes.len() as u64,
        chunks: ids.clone(),
        meta: meta.clone(),
    };
    assert_eq!(
        fid,
        object_id(Kind::File, &file.encode()),
        "file id matches the module preimage (meta carried)"
    );
}

#[test]
fn empty_bytes_have_no_chunks() {
    assert!(
        chunk_ids(&[]).is_empty(),
        "an empty file references no chunks"
    );
}

#[test]
fn index_round_trips_and_rejects_future_versions() {
    let dir = tempfile::tempdir().unwrap();

    let mut idx = Index::new("/shared/ws", "http://node:8080", Some("ab".repeat(32)));
    idx.entries.insert(
        "/shared/ws/a.txt".to_string(),
        IndexEntry {
            object: "cd".repeat(32),
            size: 5,
            mtime_secs: 1_700_000_000,
            mtime_nanos: 424_242,
            exec: false,
            kind: EntryKind::File,
            meta: BTreeMap::new(),
        },
    );
    idx.entries.insert(
        "/shared/ws/link".to_string(),
        IndexEntry {
            object: "ef".repeat(32),
            size: 7,
            mtime_secs: 1_700_000_001,
            mtime_nanos: 0,
            exec: false,
            kind: EntryKind::Symlink,
            meta: BTreeMap::new(),
        },
    );

    idx.save(dir.path()).unwrap();
    let loaded = Index::load(dir.path()).unwrap();
    assert_eq!(loaded, idx, "save -> load is a faithful round-trip");

    // A document must carry every current field. Missing fields are not an
    // older shape to upgrade in place; the checkout must be rebuilt, and the
    // parse error names the re-checkout remedy.
    let index_file = dir.path().join(".duckfs").join("index.json");
    std::fs::write(
        &index_file,
        br#"{"prefix":"/shared/ws","node":"http://node:8080"}"#,
    )
    .unwrap();
    let err = Index::load(dir.path()).unwrap_err();
    assert!(
        matches!(err, duckfs_client::index::IndexError::Parse(_)),
        "an incomplete index must not receive removed defaults: {err}"
    );
    assert!(
        err.to_string().contains("re-checkout"),
        "the parse error advises a re-checkout: {err}"
    );
}
