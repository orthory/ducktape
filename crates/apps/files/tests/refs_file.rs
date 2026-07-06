//! task 6, native side: the durable refs file (`DiskRefs`) and the block-commit
//! durability ordering. the refs file is the commit point; the whole test file
//! exists to prove that the committed root never runs ahead of it.
//!
//! - the envelope round-trips (incl. height/gc_watermark) and bricks LOUDLY on
//!   any corruption — a corrupt refs file must never silently default.
//! - the durability ordering: commit-with-save reloads to the SAME root; a
//!   crash before save reloads to the OLD root (the block was not adopted).
//! - a `Files` module restart preserves the root, and install verifies the root.

mod harness;
use harness::open_files;

use files::state::{PinEntry, Refs, Staged, root_bytes};
use files::{DiskRefs, DiskStore, Fs, Kind, ObjectStore as _, RefsStore as _};
use sdk::{Module as _, StateRoot};

fn a_refs() -> Refs {
    let mut r = Refs {
        head: Some([1; 32]),
        ..Default::default()
    };
    r.staging.insert(
        [2; 32],
        Staged {
            owner: "ext:aa".into(),
            len: 5,
            expires_at: 100,
        },
    );
    r.pins.insert(
        "release".into(),
        PinEntry {
            snapshot: [3; 32],
            owner: "kv".into(),
        },
    );
    r
}

// ---- the refs-file envelope -------------------------------------------------

#[test]
fn refs_file_round_trips_with_height_and_watermark() {
    let d = tempfile::tempdir().unwrap();
    let mut store = DiskRefs::open(d.path().to_path_buf()).unwrap();
    let r = a_refs();
    store.save(&r, 42, 7).unwrap();
    let (r2, h, gw) = DiskRefs::open(d.path().to_path_buf())
        .unwrap()
        .load()
        .unwrap()
        .expect("a saved refs file loads");
    assert_eq!((r2, h, gw), (r, 42, 7));
}

#[test]
fn absent_refs_file_is_ok_none() {
    let d = tempfile::tempdir().unwrap();
    // a fresh dir with no refs file is Ok(None) — a fresh node, NOT an error.
    assert!(
        DiskRefs::open(d.path().to_path_buf())
            .unwrap()
            .load()
            .unwrap()
            .is_none()
    );
}

#[test]
fn corrupt_payload_byte_bricks_loudly() {
    let d = tempfile::tempdir().unwrap();
    let mut store = DiskRefs::open(d.path().to_path_buf()).unwrap();
    store.save(&a_refs(), 1, 0).unwrap();
    // flip a byte inside the payload (before the trailing 32-byte checksum) —
    // load must Err, never return wrong-but-plausible refs.
    let path = d.path().join("refs");
    let mut raw = std::fs::read(&path).unwrap();
    let n = raw.len();
    raw[n - 40] ^= 0xff;
    std::fs::write(&path, raw).unwrap();
    assert!(
        DiskRefs::open(d.path().to_path_buf())
            .unwrap()
            .load()
            .is_err()
    );
}

#[test]
fn corrupt_magic_bricks_loudly() {
    let d = tempfile::tempdir().unwrap();
    let mut store = DiskRefs::open(d.path().to_path_buf()).unwrap();
    store.save(&a_refs(), 1, 0).unwrap();
    let path = d.path().join("refs");
    let mut raw = std::fs::read(&path).unwrap();
    raw[0] ^= 0xff; // clobber the "DUCKFS1\n" magic
    std::fs::write(&path, raw).unwrap();
    assert!(
        DiskRefs::open(d.path().to_path_buf())
            .unwrap()
            .load()
            .is_err()
    );
}

#[test]
fn truncated_refs_file_bricks_loudly() {
    let d = tempfile::tempdir().unwrap();
    let mut store = DiskRefs::open(d.path().to_path_buf()).unwrap();
    store.save(&a_refs(), 1, 0).unwrap();
    let path = d.path().join("refs");
    let raw = std::fs::read(&path).unwrap();
    // drop the trailing byte — the declared payload_len no longer fits.
    std::fs::write(&path, &raw[..raw.len() - 1]).unwrap();
    assert!(
        DiskRefs::open(d.path().to_path_buf())
            .unwrap()
            .load()
            .is_err()
    );
}

// ---- the durability ordering (closes task 2's review finding) ---------------
//
// both scenarios drive the exact glue sequence by hand over a real Fs +
// DiskStore + DiskRefs, so the ordering — objects+dirs durable, THEN refs file,
// THEN adopt — is exercised without waiting for the op semantics (tasks 7/9/10).

/// run the glue's block commit against a hand-built stack. if `save` is true,
/// it persists the refs file and adopts; if false, it stops right before save
/// (a crash exactly in the torn window the old code was vulnerable to).
fn drive_commit(fs: &mut Fs<DiskStore>, refs_store: &mut DiskRefs, save: bool) {
    let (refs, height, objects) = fs.commit_block().expect("a block was staged");
    {
        // 1. flush objects (idempotent) then 2. fsync their odb dirs.
        let store = fs.store_mut();
        for (kind, body) in &objects {
            store.put(*kind, body).unwrap();
        }
        store.sync_dirs().unwrap();
    }
    if save {
        // 3. persist the refs file (the commit point) then 4. adopt.
        refs_store.save(&refs, height, 0).unwrap();
        fs.adopt_refs(refs);
    }
    // when `save` is false we return with objects durable but refs NOT saved and
    // NOT adopted — the simulated crash.
}

#[test]
fn commit_with_save_reloads_to_the_same_root() {
    let d = tempfile::tempdir().unwrap();
    let mut store = DiskStore::open(d.path().join("objects")).unwrap();
    // pre-seed the object the staged refs will reference, mirroring a prior
    // putblob block; the commit re-puts it idempotently.
    let obj = (Kind::Chunk, b"staged bytes".to_vec());
    let mut fs = {
        store.put(obj.0, &obj.1).unwrap();
        Fs::new(store, Refs::default())
    };
    let mut refs_store = DiskRefs::open(d.path().to_path_buf()).unwrap();

    fs.stage_pending(a_refs(), 9, vec![obj.clone()]);
    drive_commit(&mut fs, &mut refs_store, true);
    let root_after = fs.root_bytes();
    assert_eq!(root_after, root_bytes(&a_refs()), "adopted the staged refs");
    drop(fs);
    drop(refs_store);

    // a FRESH stack reconstructed from disk agrees on the root — durable commit.
    let (loaded, h, gw) = DiskRefs::open(d.path().to_path_buf())
        .unwrap()
        .load()
        .unwrap()
        .expect("refs durable");
    assert_eq!((h, gw), (9, 0));
    let store2 = DiskStore::open(d.path().join("objects")).unwrap();
    let fs2 = Fs::new(store2, loaded);
    assert_eq!(fs2.root_bytes(), root_after, "reload preserves the root");
}

#[test]
fn crash_before_save_reloads_to_the_old_root() {
    let d = tempfile::tempdir().unwrap();
    let store = DiskStore::open(d.path().join("objects")).unwrap();
    let mut refs_store = DiskRefs::open(d.path().to_path_buf()).unwrap();
    // establish a durable baseline (empty refs at height 0).
    refs_store.save(&Refs::default(), 0, 0).unwrap();
    let mut fs = Fs::new(store, Refs::default());

    // stage a NON-trivial block, then crash right before the refs save.
    fs.stage_pending(a_refs(), 9, vec![(Kind::Chunk, b"orphan".to_vec())]);
    drive_commit(&mut fs, &mut refs_store, false);
    drop(fs);
    drop(refs_store);

    // reload: the refs file is still the baseline — the block was NOT adopted.
    let (loaded, h, gw) = DiskRefs::open(d.path().to_path_buf())
        .unwrap()
        .load()
        .unwrap()
        .expect("baseline refs durable");
    assert_eq!((loaded.clone(), h, gw), (Refs::default(), 0, 0));
    let store2 = DiskStore::open(d.path().join("objects")).unwrap();
    // the flushed object is a durable orphan — harmless (content-addressed,
    // idempotently re-put on replay, swept by a later gc).
    let orphan = files::objects::object_id(Kind::Chunk, b"orphan");
    assert!(
        store2.has(&orphan),
        "objects flush before the refs commit point"
    );
    let fs2 = Fs::new(store2, loaded);
    assert_eq!(
        fs2.root_bytes(),
        root_bytes(&Refs::default()),
        "old root survives the crash"
    );
    assert_ne!(
        fs2.root_bytes(),
        root_bytes(&a_refs()),
        "the un-saved block is not the root"
    );
}

// ---- module restart + install (task 2 round-trip, over the new Refs) --------

#[test]
fn module_restart_preserves_root_via_disk_refs() {
    let d = tempfile::tempdir().unwrap();
    let expected;
    {
        // open a module, install a non-trivial refs snapshot (which persists it
        // durably), then drop (a clean stop).
        let mut f = open_files(&d);
        let snapshot = files::encode_refs(&a_refs());
        // install at a non-zero sync-target height; the reopen below proves both
        // the root AND that height persist (the task-14 replay contract).
        f.install(&snapshot, StateRoot(root_bytes(&a_refs())), 9)
            .unwrap();
        expected = f.root();
        assert_ne!(expected, StateRoot::ZERO);
    }
    // re-open over the same dir: Files::open loads the durable refs → same root,
    // and the sync-target height survives too (the fix-1 replay contract).
    let f2 = open_files(&d);
    assert_eq!(f2.root(), expected, "durable restart preserves the root");
    assert_eq!(
        f2.durable_height(),
        9,
        "install persists the sync-target height across restart"
    );
}

#[test]
fn module_commit_block_persists_and_survives_restart() {
    // exercise the REAL glue ordering in module.rs `commit_block`, not the
    // by-hand copy above: stage a block, commit it through the module, drop,
    // reopen, assert the root and the published object are both durable.
    let d = tempfile::tempdir().unwrap();
    let expected;
    {
        let mut f = open_files(&d);
        f.stage_pending_for_test(a_refs(), 12, vec![(Kind::Chunk, b"c".to_vec())]);
        futures::executor::block_on(f.commit_block()).unwrap();
        expected = f.root();
        assert_eq!(expected, StateRoot(root_bytes(&a_refs())));
    }
    let f2 = open_files(&d);
    assert_eq!(
        f2.root(),
        expected,
        "committed root is durable across restart"
    );
    let odb = DiskStore::open(d.path().join("objects")).unwrap();
    assert!(
        odb.has(&files::objects::object_id(Kind::Chunk, b"c")),
        "the block's object is durable"
    );
}

#[test]
fn install_round_trips_and_rejects_wrong_root() {
    let d = tempfile::tempdir().unwrap();
    let mut f = open_files(&d);
    let snapshot = files::encode_refs(&a_refs());
    // install against the correct root succeeds...
    f.install(&snapshot, StateRoot(root_bytes(&a_refs())), 1)
        .unwrap();
    assert_eq!(f.root(), StateRoot(root_bytes(&a_refs())));
    // ...and against a mismatched root (ZERO) rejects — a colluding image can
    // never adopt under a root it does not hash to.
    let mut g = open_files(&d);
    assert!(g.install(&snapshot, StateRoot::ZERO, 1).is_err());
}
