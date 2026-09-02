//! task 13: consensus-neutral mark-and-sweep gc over the real module surface.
//! every test drives `Files::execute` + the async `commit_block`, forces gc via
//! the `#[doc(hidden)]` `force_gc` seam, and reads back committed state through
//! the query path or the object-inspection seams. the load-bearing invariants:
//! the mark set (head + window + pins + staging) always survives, the committed
//! root NEVER moves across a gc (consensus-neutral), and the watermark trigger
//! fires once per period and persists across a reopen.
//!
//! the pure mark/sweep unit tests live in-crate in `src/gc.rs` (they also build
//! under `--no-default-features`); this file is the native integration layer.

mod harness;
use harness::*;

use std::collections::BTreeMap;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use files::objects::object_id;
use files::{
    Change, Content, FilesMsg, FilesQuery, FilesReply, Kind, decode_refs, decode_reply, encode_msg,
    encode_putblob, encode_query, from_hex_32,
};
use sdk::{Module as _, Origin};

// ---- op / query drivers -----------------------------------------------------

fn msg(m: FilesMsg) -> sdk::Msg {
    sdk::Msg {
        target: "files".into(),
        payload: encode_msg(&m),
    }
}

fn exec(f: &mut files::Files, origin: Origin, h: u64, op: sdk::Msg) -> Result<TestCtx, sdk::Error> {
    let mut ctx = test_ctx(origin, h);
    futures::executor::block_on(f.execute(&mut ctx, &op))?;
    Ok(ctx)
}

fn commit(
    f: &mut files::Files,
    origin: Origin,
    h: u64,
    base: Option<&str>,
    changes: Vec<Change>,
) -> Result<TestCtx, sdk::Error> {
    exec(
        f,
        origin,
        h,
        msg(FilesMsg::Commit {
            base_snapshot: base.map(Into::into),
            message: "c".into(),
            changes,
        }),
    )
}

fn commit_block(f: &mut files::Files) {
    futures::executor::block_on(f.commit_block()).unwrap();
}

fn put_inline(path: &str, bytes: &[u8]) -> Change {
    Change::Put {
        path: path.into(),
        exec: false,
        meta: BTreeMap::new(),
        content: Content::Inline {
            b64: STANDARD.encode(bytes),
        },
    }
}

fn pin_op(snapshot: &str, name: &str) -> sdk::Msg {
    msg(FilesMsg::Pin {
        snapshot: snapshot.into(),
        name: name.into(),
    })
}

fn unpin_op(name: &str) -> sdk::Msg {
    msg(FilesMsg::Unpin { name: name.into() })
}

fn putblob_op(bytes: &[u8]) -> sdk::Msg {
    sdk::Msg {
        target: "files".into(),
        payload: encode_putblob(bytes),
    }
}

/// read a whole (small, single-chunk) file at head via the Read query.
fn read_all(f: &files::Files, path: &str) -> Vec<u8> {
    let raw = futures::executor::block_on(f.query(&encode_query(&FilesQuery::Read {
        path: path.into(),
        snapshot: None,
        offset: 0,
        len: files::MAX_READ_BYTES,
    })))
    .expect("read query ok");
    match decode_reply(&raw).expect("decode read reply") {
        FilesReply::Read { b64, .. } => STANDARD.decode(b64).expect("read b64 decodes"),
        other => panic!("expected a Read reply, got {other:?}"),
    }
}

/// how many snapshots the History query serves (the bounded window, clamped).
fn history_len(f: &files::Files) -> usize {
    let raw = futures::executor::block_on(f.query(&encode_query(&FilesQuery::History {
        limit: files::MAX_PAGE,
    })))
    .expect("history query ok");
    match decode_reply(&raw).expect("decode history reply") {
        FilesReply::History(v) => v.len(),
        other => panic!("expected a History reply, got {other:?}"),
    }
}

fn chunk_id(bytes: &[u8]) -> [u8; 32] {
    object_id(Kind::Chunk, bytes)
}

fn snap_id(hex: &str) -> [u8; 32] {
    from_hex_32(hex).expect("snapshot hex is valid")
}

/// a distinct, "largish" single-chunk body keyed by `i` — distinct bodies hash
/// to distinct chunk ids, so each snapshot's file is an exclusive object.
fn body_of(i: u64) -> Vec<u8> {
    (0..2048u32)
        .map(|j| (i as u32).wrapping_mul(2_654_435_761).wrapping_add(j) as u8)
        .collect()
}

fn pick(present: &[(String, Vec<u8>)], r: u64) -> Option<usize> {
    if present.is_empty() {
        None
    } else {
        Some((r as usize) % present.len())
    }
}

// ---- 1. reachability property ------------------------------------------------

#[test]
fn reachability_property_survives_gc() {
    let d = tempfile::tempdir().unwrap();
    let mut f = open_files(&d);
    // a small window turns ~50 seeded commits into real churn: overwrites and
    // rms leave exclusive objects that fall out of the window and MUST be swept,
    // while the mark set (head + window + pins + staging) MUST survive intact.
    f.set_history_window_for_tests(6);

    let owners = ["alice", "bob"];
    // knuth mmix lcg — deterministic, no rand dependency.
    let mut lcg: u64 = 0x1234_5678_9abc_def0;
    let mut rng = || {
        lcg = lcg
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        lcg >> 33
    };

    let mut present: Vec<(String, Vec<u8>)> = Vec::new(); // paths live in head
    let mut staged: Vec<[u8; 32]> = Vec::new(); // unreferenced putblob digests
    let mut pin_seq = 0u64;

    for i in 0..50u64 {
        let who = owners[(rng() % 2) as usize];
        let origin = Origin::Module(who.into());
        let h = i + 1; // stays well below GC_PERIOD_BLOCKS — no accidental trigger
        // read the live head fresh as the commit base every time: base == the
        // effective head, so per-path CAS never spuriously conflicts.
        let base = f.committed_head_for_test();
        let mut did = false;
        match rng() % 6 {
            // put a brand-new file (public /shared/**, writable by any module).
            0 | 1 => {
                let path = format!("/shared/{who}/f{i}");
                let body = format!("body-{i}-{}", rng()).into_bytes();
                commit(
                    &mut f,
                    origin,
                    h,
                    base.as_deref(),
                    vec![put_inline(&path, &body)],
                )
                .expect("put commits");
                present.push((path, body));
                did = true;
            }
            // overwrite an existing file — churns an exclusive chunk into garbage
            // once every window snapshot that held it evicts.
            2 => {
                if let Some(idx) = pick(&present, rng()) {
                    let path = present[idx].0.clone();
                    let body = format!("rw-{i}-{}", rng()).into_bytes();
                    commit(
                        &mut f,
                        origin,
                        h,
                        base.as_deref(),
                        vec![put_inline(&path, &body)],
                    )
                    .expect("overwrite commits");
                    present[idx].1 = body;
                    did = true;
                }
            }
            // remove an existing file.
            3 => {
                if let Some(idx) = pick(&present, rng()) {
                    let path = present[idx].0.clone();
                    commit(
                        &mut f,
                        origin,
                        h,
                        base.as_deref(),
                        vec![Change::Rm { path: path.clone() }],
                    )
                    .expect("rm commits");
                    present.remove(idx);
                    did = true;
                }
            }
            // pin the current head — rescues it from any later eviction.
            4 => {
                if let Some(hd) = base {
                    let name = format!("pin{pin_seq}");
                    pin_seq += 1;
                    exec(&mut f, origin, h, pin_op(&hd, &name)).expect("pin stages");
                    did = true;
                }
            }
            // stage an unreferenced putblob — a staging root that gc must keep.
            _ => {
                let body = format!("blob-{i}-{}", rng()).into_bytes();
                exec(&mut f, origin, h, putblob_op(&body)).expect("putblob stages");
                staged.push(chunk_id(&body));
                did = true;
            }
        }
        if did {
            commit_block(&mut f);
        }
    }

    let root_before = f.root();
    let removed = f.force_gc();
    // consensus-neutral: gc never moves the committed root.
    assert_eq!(f.root(), root_before, "gc must not move the committed root");
    // the churn under a size-6 window produced real garbage.
    assert!(
        removed > 0,
        "expected the small window to have swept something"
    );

    // every object reachable from head + window + pins + staging still resolves
    // (gc_mark_for_test re-marks and would panic if any were swept).
    let live = f.gc_mark_for_test();
    for id in &live {
        assert!(f.odb_has_for_test(id), "a reachable object was swept");
    }

    // every staged-unreferenced chunk survives, and stays a staging entry.
    let refs = decode_refs(&f.snapshot()).expect("committed refs decode");
    for dg in &staged {
        assert!(f.odb_has_for_test(dg), "a staged chunk was swept");
        assert!(
            refs.staging.contains_key(dg),
            "a staged chunk left the staging table"
        );
    }

    // functional under-mark detector: every live head file still reads its bytes.
    for (path, body) in &present {
        assert_eq!(
            &read_all(&f, path),
            body,
            "a live file lost its bytes to gc"
        );
    }

    // History still serves the bounded window.
    assert_eq!(
        history_len(&f),
        refs.window.len().min(files::MAX_PAGE as usize)
    );
}

// ---- 2. window expiry sweeps unpinned history --------------------------------

#[test]
fn window_expiry_sweeps_unpinned_history() {
    let d = tempfile::tempdir().unwrap();
    let mut f = open_files(&d);
    f.set_history_window_for_tests(4);

    let mut snaps = Vec::new();
    let mut chunks = Vec::new();
    let mut head: Option<String> = None;
    for i in 0..6u64 {
        let body = body_of(i);
        commit(
            &mut f,
            Origin::System,
            i + 1,
            head.as_deref(),
            vec![put_inline("/shared/f", &body)],
        )
        .expect("commit");
        commit_block(&mut f);
        head = f.committed_head_for_test();
        snaps.push(head.clone().unwrap());
        chunks.push(chunk_id(&body));
    }

    let before = f.odb_len_for_test();
    let root_before = f.root();
    let removed = f.force_gc();

    assert!(removed > 0, "evicted history was swept");
    assert!(f.odb_len_for_test() < before, "list() shrank");
    // snapshots 1,2 (indices 0,1) left the size-4 window → exclusive objects gone.
    for j in 0..2 {
        assert!(!f.odb_has_for_test(&chunks[j]), "evicted chunk {j} swept");
        assert!(
            !f.odb_has_for_test(&snap_id(&snaps[j])),
            "evicted snapshot {j} swept"
        );
    }
    // snapshots 3..6 (the window) intact.
    for j in 2..6 {
        assert!(f.odb_has_for_test(&chunks[j]), "window chunk {j} kept");
        assert!(
            f.odb_has_for_test(&snap_id(&snaps[j])),
            "window snapshot {j} kept"
        );
    }
    assert_eq!(history_len(&f), 4, "History serves the 4-snapshot window");
    assert_eq!(f.root(), root_before, "gc did not move the root");
    assert_eq!(
        read_all(&f, "/shared/f"),
        body_of(5),
        "the live file reads the newest body"
    );
}

// ---- 3. pin rescues history, unpin frees it ----------------------------------

#[test]
fn pin_rescues_history_then_unpin_frees_it() {
    let d = tempfile::tempdir().unwrap();
    let mut f = open_files(&d);
    f.set_history_window_for_tests(4);

    // commit 1 → snap1.
    let body0 = body_of(0);
    commit(
        &mut f,
        Origin::System,
        1,
        None,
        vec![put_inline("/shared/f", &body0)],
    )
    .expect("commit 1");
    commit_block(&mut f);
    let snap1 = f.committed_head_for_test().unwrap();
    let chunk0 = chunk_id(&body0);

    // pin snap1 before it can evict.
    exec(&mut f, Origin::System, 2, pin_op(&snap1, "keep")).expect("pin stages");
    commit_block(&mut f);

    // commits 2..6 overwrite /shared/f, pushing snap1 out of the size-4 window.
    let mut snap2 = String::new();
    let mut chunk1 = [0u8; 32];
    let mut head = Some(snap1.clone());
    for i in 1..6u64 {
        let body = body_of(i);
        commit(
            &mut f,
            Origin::System,
            i + 2,
            head.as_deref(),
            vec![put_inline("/shared/f", &body)],
        )
        .expect("overwrite");
        commit_block(&mut f);
        head = f.committed_head_for_test();
        if i == 1 {
            snap2 = head.clone().unwrap();
            chunk1 = chunk_id(&body);
        }
    }

    let root_before = f.root();
    let removed = f.force_gc();
    assert!(removed > 0);
    // snap1 pinned → survives even though it left the window.
    assert!(f.odb_has_for_test(&snap_id(&snap1)), "pinned snapshot kept");
    assert!(f.odb_has_for_test(&chunk0), "pinned snapshot's chunk kept");
    // snap2 evicted and NOT pinned → swept.
    assert!(
        !f.odb_has_for_test(&snap_id(&snap2)),
        "unpinned evicted snapshot swept"
    );
    assert!(!f.odb_has_for_test(&chunk1), "unpinned evicted chunk swept");
    assert_eq!(f.root(), root_before, "gc did not move the root");

    // unpin, then gc frees snap1.
    exec(&mut f, Origin::System, 8, unpin_op("keep")).expect("unpin stages");
    commit_block(&mut f);
    let root_before2 = f.root();
    f.force_gc();
    assert!(
        !f.odb_has_for_test(&snap_id(&snap1)),
        "after unpin the snapshot is swept"
    );
    assert!(
        !f.odb_has_for_test(&chunk0),
        "after unpin the chunk is swept"
    );
    assert_eq!(f.root(), root_before2, "gc did not move the root");
}

// ---- 4. dedup: a shared chunk survives via the live tree ---------------------

#[test]
fn shared_chunk_survives_when_a_live_tree_references_it() {
    let d = tempfile::tempdir().unwrap();
    let mut f = open_files(&d);
    f.set_history_window_for_tests(4);

    let x = body_of(100); // the shared body
    let chunk_x = chunk_id(&x);
    let f0_chunk = chunk_id(&body_of(0));

    // commit 1: /shared/keep = X (never touched again) and /shared/f = body0.
    commit(
        &mut f,
        Origin::System,
        1,
        None,
        vec![
            put_inline("/shared/keep", &x),
            put_inline("/shared/f", &body_of(0)),
        ],
    )
    .expect("commit 1");
    commit_block(&mut f);
    let snap1 = f.committed_head_for_test().unwrap();

    // commits 2..5 overwrite ONLY /shared/f, leaving /shared/keep = X shared.
    let mut head = Some(snap1.clone());
    for i in 1..5u64 {
        commit(
            &mut f,
            Origin::System,
            i + 1,
            head.as_deref(),
            vec![put_inline("/shared/f", &body_of(i))],
        )
        .expect("overwrite");
        commit_block(&mut f);
        head = f.committed_head_for_test();
    }
    // window_cap 4, 5 commits → snap1 evicted.

    let root_before = f.root();
    f.force_gc();
    // snap1 evicted: its snapshot object and its exclusive f-chunk are swept...
    assert!(
        !f.odb_has_for_test(&snap_id(&snap1)),
        "evicted snapshot swept"
    );
    assert!(
        !f.odb_has_for_test(&f0_chunk),
        "snap1's exclusive f-chunk swept"
    );
    // ...but the body shared with the live head survives (dedup by content id).
    assert!(
        f.odb_has_for_test(&chunk_x),
        "the shared chunk survives via the live tree"
    );
    assert_eq!(f.root(), root_before, "gc did not move the root");
    assert_eq!(
        read_all(&f, "/shared/keep"),
        x,
        "Read still returns the shared body after the sweep"
    );
}

// ---- 5. trigger: gc_due table + real commit_block trigger --------------------

#[test]
fn gc_due_boundary_table() {
    use files::testkit::gc_due;
    // (height, watermark) -> due? one gc per GC_PERIOD_BLOCKS (=1024) window.
    assert!(!gc_due(1023, 0));
    assert!(gc_due(1024, 0));
    assert!(!gc_due(1025, 1024));
    assert!(gc_due(2048, 1024));
    assert!(!gc_due(2049, 2048));
}

#[test]
fn commit_block_triggers_gc_and_persists_watermark_across_reopen() {
    let d = tempfile::tempdir().unwrap();
    {
        let mut f = open_files(&d);
        // seed at a low height — the watermark stays 0.
        commit(
            &mut f,
            Origin::System,
            1,
            None,
            vec![put_inline("/shared/f", b"a")],
        )
        .expect("seed commits");
        commit_block(&mut f);
        assert_eq!(f.gc_watermark_for_test(), 0);
        let head = f.committed_head_for_test();

        // force the watermark so the next commit crosses the SECOND period
        // boundary: gc_due(2048, 1024) is true, so commit_block runs gc for real
        // and advances + persists the watermark.
        f.set_gc_watermark_for_tests(files::GC_PERIOD_BLOCKS);
        commit(
            &mut f,
            Origin::System,
            2 * files::GC_PERIOD_BLOCKS,
            head.as_deref(),
            vec![put_inline("/shared/g", b"b")],
        )
        .expect("commit at the boundary");
        commit_block(&mut f);
        assert_eq!(
            f.gc_watermark_for_test(),
            2 * files::GC_PERIOD_BLOCKS,
            "the trigger advanced the watermark to the block height"
        );
    }
    // reopen: the persisted refs envelope carries the advanced watermark.
    let f2 = open_files(&d);
    assert_eq!(
        f2.gc_watermark_for_test(),
        2 * files::GC_PERIOD_BLOCKS,
        "the watermark persisted across a reopen"
    );
}

// ---- 6. a lost object skips the sweep, it does not brick the node ------------

/// delete an object straight out of the odb — the bad-sector case, reproduced
/// the way the issue's validation step does (no test seam: the store must see
/// exactly what a lost disk block leaves behind).
fn lose_object(d: &tempfile::TempDir, id: &[u8; 32]) {
    let hex = to_hex(id);
    let path = d.path().join("objects").join(&hex[..2]).join(&hex[2..]);
    std::fs::remove_file(&path).expect("the odb holds the object");
}

#[test]
fn a_lost_object_skips_the_sweep_and_the_block_still_commits() {
    let d = tempfile::tempdir().unwrap();
    let mut f = open_files(&d);
    // a size-1 window, so the FIRST version's chunk is unreachable by the
    // second commit — a healthy sweep at the boundary would remove it, which is
    // what makes "nothing was swept" a real assertion below.
    f.set_history_window_for_tests(1);
    commit(
        &mut f,
        Origin::System,
        1,
        None,
        vec![put_inline("/shared/f", b"one")],
    )
    .expect("first commit");
    commit_block(&mut f);
    let head = f.committed_head_for_test();
    commit(
        &mut f,
        Origin::System,
        2,
        head.as_deref(),
        vec![put_inline("/shared/f", b"two")],
    )
    .expect("second commit");
    commit_block(&mut f);

    let evicted = chunk_id(b"one");
    let reachable = chunk_id(b"two");
    assert!(f.odb_has_for_test(&evicted), "the evicted chunk is present");
    assert!(f.odb_has_for_test(&reachable), "head's chunk is present");

    // lose a REACHABLE object: the mark can no longer complete.
    lose_object(&d, &reachable);

    // drive a block across a gc period boundary. this used to be a FATAL
    // block-boundary fault — one lost blob stopped every node holding it — and
    // must now be a skipped sweep: the block commits (the helper unwraps), the
    // boundary is consumed, and NOT ONE object was removed.
    f.set_gc_watermark_for_tests(files::GC_PERIOD_BLOCKS);
    let head = f.committed_head_for_test();
    let root_before = f.root();
    let before = f.odb_len_for_test();
    commit(
        &mut f,
        Origin::System,
        2 * files::GC_PERIOD_BLOCKS,
        head.as_deref(),
        vec![put_inline("/shared/g", b"three")],
    )
    .expect("commit at the boundary");
    commit_block(&mut f);

    assert_eq!(
        f.gc_watermark_for_test(),
        2 * files::GC_PERIOD_BLOCKS,
        "a skipped sweep still consumes its boundary — the retry is the next period"
    );
    assert!(
        f.odb_has_for_test(&evicted),
        "a skipped sweep removes NOTHING, not even the unreachable chunk"
    );
    assert!(
        f.odb_len_for_test() > before,
        "the block's own objects landed"
    );
    assert_ne!(f.root(), root_before, "the block committed");
    // and the store stays usable: every file whose bytes are intact still reads.
    assert_eq!(read_all(&f, "/shared/g"), b"three");
}
