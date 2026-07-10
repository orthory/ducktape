//! task 6, module side: the `Files` glue over the durable refs file. this file
//! proves only what needs the sdk module — a module restart preserves the root,
//! `install` verifies the root and persists the sync-target height, and the
//! kernel-facing per-commit height cursor tracks the refs envelope.
//!
//! the pure `DiskRefs`/`DiskStore` surface — envelope round-trip, loud bricking
//! on corruption, and the commit/crash durability ordering — is owned by the
//! disk library now: `crates/duckfs/disk/tests/refs_file.rs`.

mod harness;
use harness::open_files;

use files::state::{PinEntry, Refs, Staged, root_bytes};
use files::{DiskStore, Kind, ObjectStore as _};
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
    // by-hand copy in the disk crate: stage a block, commit it through the
    // module, drop, reopen, assert the root and the published object are durable.
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
fn durable_commit_height_cursor_tracks_the_refs_envelope() {
    // the kernel-facing per-commit height cursor (Module::durable_commit_height):
    // None until a refs envelope exists — a fresh dir has no durable commit to
    // claim, and recovery's trailing bound-and-verify must never read "never
    // committed" as "committed at height 0" — then exactly the committed height,
    // surviving a restart (it rides the same atomic refs-envelope write as the
    // state itself).
    let d = tempfile::tempdir().unwrap();
    {
        let mut f = open_files(&d);
        assert_eq!(
            sdk::Module::durable_commit_height(&f),
            None,
            "a fresh dir claims no durable commit"
        );
        f.stage_pending_for_test(a_refs(), 12, vec![(Kind::Chunk, b"c".to_vec())]);
        futures::executor::block_on(f.commit_block()).unwrap();
        assert_eq!(
            sdk::Module::durable_commit_height(&f),
            Some(12),
            "the cursor claims exactly the committed height"
        );
    }
    // a reopen reads the cursor back from the envelope — the same durability
    // unit as the refs image, so the (root, height) binding cannot tear.
    let f2 = open_files(&d);
    assert_eq!(sdk::Module::durable_commit_height(&f2), Some(12));
    assert_eq!(f2.durable_height(), 12, "the inherent glue accessor agrees");
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
