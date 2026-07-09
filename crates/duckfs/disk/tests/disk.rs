//! the loose-object disk store (`DiskStore`) behaviors, adapted from the task-5
//! brief onto the `ObjectStore` trait. native + default features (tempfile).
//! the three brief invariants: round-trip/idempotence/absent-get; corrupt =
//! Err (never wrong bytes); open sweeps `*.tmp` debris + sorted `list`.

use duckfs_core::objects::{Kind, object_id};
use duckfs_core::{MemStore, ObjectStore, to_hex};
use duckfs_disk::DiskStore;

// the trait contract, run against BOTH stores so the mem and disk impls stay
// observably identical through the seam the pure core sees.
fn roundtrip_contract<S: ObjectStore>(store: &mut S) {
    let id = store.put(Kind::Chunk, b"bytes").unwrap();
    assert_eq!(id, object_id(Kind::Chunk, b"bytes"));
    assert!(store.has(&id));
    assert_eq!(
        store.get(&id).unwrap(),
        Some((Kind::Chunk, b"bytes".to_vec()))
    );
    // stat is metadata-only kind + BODY length (the disk file also carries a
    // kind tag byte, which must NOT count), absent = Ok(None).
    assert_eq!(store.stat(&id).unwrap(), Some((Kind::Chunk, 5)));
    let tree_id = store.put(Kind::Tree, b"bytes").unwrap();
    assert_eq!(store.stat(&tree_id).unwrap(), Some((Kind::Tree, 5)));
    store.remove(&tree_id).unwrap();
    assert_eq!(
        store.stat(&object_id(Kind::Chunk, b"absent")).unwrap(),
        None
    );
    // idempotent re-put: content-addressed, same id, no error, single entry.
    let id2 = store.put(Kind::Chunk, b"bytes").unwrap();
    assert_eq!(id, id2, "idempotent re-put");
    assert_eq!(store.list().unwrap(), vec![id]);
    // absent get is Ok(None), never an error.
    let absent = object_id(Kind::Chunk, b"absent");
    assert!(!store.has(&absent));
    assert_eq!(store.get(&absent).unwrap(), None);
    // remove drops it; removing an absent id is still Ok.
    store.remove(&absent).unwrap();
    store.remove(&id).unwrap();
    assert!(!store.has(&id));
    assert_eq!(store.get(&id).unwrap(), None);
    assert!(store.list().unwrap().is_empty());
}

#[test]
fn mem_and_disk_share_the_trait_contract() {
    roundtrip_contract(&mut MemStore::new());
    let d = tempfile::tempdir().unwrap();
    let mut disk = DiskStore::open(d.path().join("objects")).unwrap();
    roundtrip_contract(&mut disk);
}

#[test]
fn put_get_has_roundtrip_and_idempotence() {
    let d = tempfile::tempdir().unwrap();
    let mut odb = DiskStore::open(d.path().join("objects")).unwrap();
    let id = odb.put(Kind::Chunk, b"bytes").unwrap();
    assert_eq!(id, object_id(Kind::Chunk, b"bytes"));
    assert!(odb.has(&id));
    assert_eq!(
        odb.get(&id).unwrap(),
        Some((Kind::Chunk, b"bytes".to_vec()))
    );
    let id2 = odb.put(Kind::Chunk, b"bytes").unwrap();
    assert_eq!(id, id2, "idempotent re-put");
    assert_eq!(odb.get(&object_id(Kind::Chunk, b"absent")).unwrap(), None);
}

#[test]
fn corrupt_object_is_an_error_not_bad_bytes() {
    let d = tempfile::tempdir().unwrap();
    let dir = d.path().join("objects");
    let mut odb = DiskStore::open(dir.clone()).unwrap();
    let id = odb.put(Kind::Chunk, b"bytes").unwrap();
    // flip a byte on disk behind the store's back.
    let hex = to_hex(&id);
    let path = dir.join(&hex[..2]).join(&hex[2..]);
    let mut raw = std::fs::read(&path).unwrap();
    raw[3] ^= 0xff;
    std::fs::write(&path, raw).unwrap();
    // corrupt = Err (never silently wrong bytes)...
    assert!(
        odb.get(&id).is_err(),
        "hash mismatch must surface as an error"
    );
    // ...and absent on the SAME store is still a clean Ok(None), distinctly.
    assert_eq!(odb.get(&object_id(Kind::Chunk, b"nope")).unwrap(), None);
}

#[test]
fn put_replaces_a_length_corrupt_existing_object() {
    // finding #2: put must not treat mere path existence as success — a corrupt
    // existing object (here a torn/truncated external write) must be REPLACED, or
    // a "possessed" object stays permanently unreadable and put can never repair
    // it. (a length change is the cheap-to-detect corruption put itself catches;
    // a same-length bit-flip is caught by `verify` on the possession path.)
    let d = tempfile::tempdir().unwrap();
    let dir = d.path().join("objects");
    let mut odb = DiskStore::open(dir.clone()).unwrap();
    let id = odb.put(Kind::Chunk, b"bytes").unwrap();

    let hex = to_hex(&id);
    let path = dir.join(&hex[..2]).join(&hex[2..]);
    std::fs::write(&path, b"xx").unwrap(); // truncate behind the store's back
    assert!(odb.get(&id).is_err(), "the truncated object reads as corrupt");

    // re-put the correct bytes: put REPLACES the corrupt file rather than no-op.
    let id2 = odb.put(Kind::Chunk, b"bytes").unwrap();
    assert_eq!(id, id2);
    assert_eq!(
        odb.get(&id).unwrap(),
        Some((Kind::Chunk, b"bytes".to_vec())),
        "put replaced the corrupt object with the correct bytes"
    );
}

#[test]
fn verify_removes_a_bitflipped_object_so_it_self_heals() {
    // finding #2: possession must be integrity-verified — a present-but-corrupt
    // chunk must NOT count as possessed. `verify` re-hashes and, on a same-length
    // bit-flip, DELETES the corrupt file so it reads as absent and the self-heal
    // fetch loop re-fetches a good copy (which `put` then lands).
    let d = tempfile::tempdir().unwrap();
    let dir = d.path().join("objects");
    let mut odb = DiskStore::open(dir.clone()).unwrap();
    let id = odb.put(Kind::Chunk, b"bytes").unwrap();
    assert!(odb.verify(&id).unwrap(), "an intact object verifies");

    let hex = to_hex(&id);
    let path = dir.join(&hex[..2]).join(&hex[2..]);
    let mut raw = std::fs::read(&path).unwrap();
    raw[3] ^= 0xff; // same-length bit-flip
    std::fs::write(&path, raw).unwrap();

    assert!(!odb.verify(&id).unwrap(), "a corrupt object fails verify");
    assert!(
        !odb.has(&id),
        "verify removed the corrupt file so it re-fetches as absent"
    );
    // an absent object verifies false, never an error.
    assert!(!odb.verify(&object_id(Kind::Chunk, b"never")).unwrap());
}

#[test]
fn open_sweeps_tmp_debris_and_list_enumerates() {
    let d = tempfile::tempdir().unwrap();
    let dir = d.path().join("objects");
    let mut odb = DiskStore::open(dir.clone()).unwrap();
    let a = odb.put(Kind::Chunk, b"a").unwrap();
    let b = odb.put(Kind::Tree, b"b").unwrap();
    // crash debris at the odb root...
    std::fs::write(dir.join("junk.tmp"), b"crash leftovers").unwrap();
    // ...and buried inside a fanout subdir, to prove the sweep recurses.
    let hex_a = to_hex(&a);
    std::fs::write(dir.join(&hex_a[..2]).join("deadbeef.tmp"), b"debris").unwrap();
    let odb2 = DiskStore::open(dir.clone()).unwrap();
    assert!(!dir.join("junk.tmp").exists(), "tmp debris swept at open");
    assert!(
        !dir.join(&hex_a[..2]).join("deadbeef.tmp").exists(),
        "nested tmp debris swept at open"
    );
    let mut want = vec![a, b];
    want.sort();
    assert_eq!(odb2.list().unwrap(), want);
}
