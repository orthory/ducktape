//! store tests. the fold path runs the REAL pipeline — the committed testmap
//! fixture (crates/kernel/index-guest/testmap, refreshed by `make
//! wasm-modules`) installed in a real engine, folded by the engine's trigger
//! runner. every wait is on the database's own subscription stream, never a
//! poll: a missing event fails the test at the deadline instead of flaking
//! early. guest-failure queue-holding semantics (a failing fold retains its
//! events and backs off) are the engine's own tested contract
//! (fluent31/tests/trigger_robustness.rs) and are not re-proven here; the
//! synchronous view path covers `Fail` surfacing.

use super::*;

use std::collections::BTreeSet;
use std::time::Duration;

use fluent31::StreamEvent;

/// the reference mapper, built by `guest-builder --index` (see its crate docs
/// for the derived key space: `seen/{height}/{seq}`, `count`).
const TESTMAP: &[u8] = include_bytes!("../../index-guest/testmap/index.wasm");
/// how long a fold may take before the test calls it stuck. generous: the
/// wait is event-driven, so healthy runs never sit it out.
const RECV_DEADLINE: Duration = Duration::from_secs(60);

fn bare_store(dir: &Path) -> IndexStore {
    let modules = [IndexModule::bare("chat"), IndexModule::bare("tasks")];
    IndexStore::open(dir, &modules).expect("open store")
}

fn mapped_store(dir: &Path) -> IndexStore {
    let modules = [
        IndexModule {
            id: "chat",
            guest: Some(TESTMAP),
        },
        IndexModule::bare("tasks"),
    ];
    IndexStore::open(dir, &modules).expect("open store")
}

fn chat_op(payload: &[u8]) -> AppliedOp {
    AppliedOp {
        module: "chat".into(),
        origin: OriginTag::external("jess"),
        payload: payload.to_vec(),
        assigned: Vec::new(),
    }
}

fn tasks_op() -> AppliedOp {
    AppliedOp {
        module: "tasks".into(),
        origin: OriginTag::module("chat"),
        payload: br#"{"create":"t"}"#.to_vec(),
        assigned: Vec::new(),
    }
}

fn block(height: u64, ops: Vec<AppliedOp>) -> BlockOps {
    BlockOps {
        height,
        time: 1_000 + height,
        ops,
        record: None,
    }
}

fn block_with_record(height: u64, ops: Vec<AppliedOp>) -> BlockOps {
    BlockOps {
        record: Some(format!(r#"{{"height":{height}}}"#).into_bytes()),
        ..block(height, ops)
    }
}

/// block on the subscription until every expected key has streamed by.
fn wait_for_keys(sub: &mut fluent31::Subscription, expect: impl IntoIterator<Item = String>) {
    let mut expect: BTreeSet<Vec<u8>> = expect.into_iter().map(String::into_bytes).collect();
    while !expect.is_empty() {
        let event = sub
            .recv_timeout(RECV_DEADLINE)
            .expect("subscription stream healthy")
            .expect("expected derived write never streamed — the fold is stuck");
        let StreamEvent::Batch(entries) = event else {
            panic!("subscription lagged mid-test");
        };
        for entry in entries {
            expect.remove(&entry.key);
        }
    }
}

fn seen_key(height: u64, seq: u32) -> String {
    format!("seen/{height:016x}/{seq:04x}")
}

/// block on a `fold/` subscription until the tip reaches `target`, returning
/// every tip position that streamed by. one entry per fold INVOCATION, so the
/// list is also the transcript of how the engine cut the batch.
fn wait_for_tip(sub: &mut fluent31::Subscription, target: (u64, u32)) -> Vec<(u64, u32)> {
    let mut seen = Vec::new();
    loop {
        let event = sub
            .recv_timeout(RECV_DEADLINE)
            .expect("subscription stream healthy")
            .expect("the fold tip never streamed — the fold is stuck");
        let StreamEvent::Batch(entries) = event else {
            panic!("subscription lagged mid-test");
        };
        for entry in entries {
            assert_eq!(entry.key, FOLD_TIP.as_bytes(), "only the tip lives here");
            let value = entry.value.expect("a tip write always carries its value");
            seen.push(index_guest::decode_fold_tip(&value).expect("a well-formed tip"));
        }
        if seen.last() == Some(&target) {
            return seen;
        }
    }
}

// ----------------------------------------------------------------------------
// the host feed
// ----------------------------------------------------------------------------

#[test]
fn op_rows_land_per_module_in_drain_order() {
    let dir = tempfile::tempdir().unwrap();
    let store = bare_store(dir.path());

    store
        .apply_block(&block(
            1,
            vec![
                chat_op(br#"{"post":"hi"}"#),
                tasks_op(),
                chat_op(br#"{"post":"again"}"#),
            ],
        ))
        .expect("apply");

    let page = store.scan("chat", OP_PREFIX.as_bytes(), None, 10).unwrap();
    assert_eq!(page.entries.len(), 2);
    assert!(!page.has_more);
    // block-wide seq survives the per-module split: chat got 0 and 2.
    assert_eq!(page.entries[0].0, op_key(1, 0).into_bytes());
    assert_eq!(page.entries[1].0, op_key(1, 2).into_bytes());

    let row: OpRow = borsh::from_slice(&page.entries[0].1).unwrap();
    assert_eq!(row.height, 1);
    assert_eq!(row.seq, 0);
    assert_eq!(row.time, 1_001);
    assert_eq!(row.origin, OriginTag::external("jess"));
    assert_eq!(row.payload, br#"{"post":"hi"}"#.to_vec());

    assert_eq!(store.applied_height("chat").unwrap(), 1);
    assert_eq!(store.applied_height("tasks").unwrap(), 1);
}

#[test]
fn replay_is_idempotent_per_module() {
    let dir = tempfile::tempdir().unwrap();
    let store = bare_store(dir.path());

    let b1 = block(1, vec![chat_op(b"{}")]);
    store.apply_block(&b1).expect("first apply");
    store
        .apply_block(&b1)
        .expect("replay is a skip, not an error");

    let page = store.scan("chat", OP_PREFIX.as_bytes(), None, 10).unwrap();
    assert_eq!(page.entries.len(), 1, "no duplicate rows on replay");
    assert_eq!(store.applied_height("chat").unwrap(), 1);
}

#[test]
fn watermarks_survive_reopen() {
    let dir = tempfile::tempdir().unwrap();
    {
        let store = bare_store(dir.path());
        store.apply_block(&block(1, vec![chat_op(b"{}")])).unwrap();
        store.apply_block(&block(2, vec![chat_op(b"{}")])).unwrap();
    }
    let store = bare_store(dir.path());
    assert_eq!(store.applied_height("chat").unwrap(), 2);
    assert_eq!(
        store.applied_height("tasks").unwrap(),
        2,
        "a quiet module's watermark advances with every block — watermark \
         lag must mean missing blocks, not missing ops"
    );
    assert_eq!(
        store.resume_height().unwrap(),
        2,
        "resume from the max watermark"
    );
    let page = store.scan("chat", OP_PREFIX.as_bytes(), None, 10).unwrap();
    assert_eq!(page.entries.len(), 2);
}

#[test]
fn scan_pages_with_cursor() {
    let dir = tempfile::tempdir().unwrap();
    let store = bare_store(dir.path());
    for h in 1..=5 {
        store.apply_block(&block(h, vec![chat_op(b"{}")])).unwrap();
    }

    let first = store.scan("chat", OP_PREFIX.as_bytes(), None, 2).unwrap();
    assert_eq!(first.entries.len(), 2);
    assert!(first.has_more);
    let cursor = first.next_after.clone().expect("cursor when has_more");

    let second = store
        .scan("chat", OP_PREFIX.as_bytes(), Some(cursor.as_bytes()), 10)
        .unwrap();
    assert_eq!(second.entries.len(), 3, "resumes strictly after the cursor");
    assert!(!second.has_more);
    assert!(second.next_after.is_none());
    assert_eq!(second.entries[0].0, op_key(3, 0).into_bytes());
}

#[test]
fn unknown_module_is_refused_and_poisons() {
    let dir = tempfile::tempdir().unwrap();
    let store = bare_store(dir.path());
    let bad = block(
        1,
        vec![AppliedOp {
            module: "ghost".into(),
            origin: OriginTag::system(),
            payload: b"{}".to_vec(),
            assigned: Vec::new(),
        }],
    );
    assert!(matches!(
        store.apply_block(&bad),
        Err(Error::UnknownModule(_))
    ));
    assert!(store.is_poisoned());
    // writes refuse from now on; reads keep serving.
    assert!(matches!(
        store.apply_block(&block(2, vec![chat_op(b"{}")])),
        Err(Error::Poisoned)
    ));
    assert!(store.scan("chat", b"", None, 10).is_ok());
}

// ----------------------------------------------------------------------------
// the guest fold + view
// ----------------------------------------------------------------------------

#[test]
fn guest_folds_ops_and_serves_the_view() {
    let dir = tempfile::tempdir().unwrap();
    let store = mapped_store(dir.path());

    // subscribe BEFORE the write so no derived event can slip past.
    let mut sub = store.subscribe("chat", b"seen/", Some(b"seen0")).unwrap();
    store
        .apply_block(&block(
            1,
            vec![
                chat_op(br#"{"post":"hi"}"#),
                tasks_op(),
                chat_op(br#"{"post":"again"}"#),
            ],
        ))
        .unwrap();
    wait_for_keys(&mut sub, [seen_key(1, 0), seen_key(1, 2)]);

    // the fold's transaction committed rows and counter atomically.
    assert_eq!(
        store.view("chat", b"count").unwrap(),
        2u64.to_be_bytes().to_vec()
    );
    assert_eq!(
        store.view("chat", seen_key(1, 0).as_bytes()).unwrap(),
        br#"{"post":"hi"}"#.to_vec()
    );

    // a guest Fail is the module refusing the REQUEST — message intact.
    let Err(Error::View(msg)) = store.view("chat", b"boom") else {
        panic!("poison view request must surface as Error::View");
    };
    assert!(
        msg.contains("poison view request"),
        "guest message travels: {msg}"
    );

    // no guest (or no query role) → no materialized view; unknown module is
    // its own error.
    assert!(matches!(
        store.view("tasks", b"count"),
        Err(Error::ViewUnsupported)
    ));
    assert!(matches!(
        store.view("ghost", b"count"),
        Err(Error::UnknownModule(_))
    ));

    // fold health is observable exactly where a fold exists.
    assert!(store.fold_status("chat").unwrap().is_some());
    assert!(store.fold_status("tasks").unwrap().is_none());
}

/// THE TIP IS A ROW POSITION, NOT A BLOCK NUMBER. The shared shell records
/// `(height, seq)` of the last op row it CONSUMED, which is what makes
/// "is my op in the view yet" answerable: a height alone cannot tell a block
/// that folded whole from one the engine is halfway through.
#[test]
fn the_fold_tip_records_the_last_op_row_consumed() {
    let dir = tempfile::tempdir().unwrap();
    let store = mapped_store(dir.path());

    // a database that never folded reports UNKNOWN, never a zero position.
    assert_eq!(store.fold_tip("chat").unwrap(), None);

    let mut sub = store
        .subscribe("chat", FOLD_PREFIX.as_bytes(), Some(b"fold0"))
        .unwrap();
    // the block's LAST dispatch is tasks', so chat's tip must be chat's own
    // last row (1, 2) — the block-wide seq, kept exactly as the feed spells it.
    store
        .apply_block(&block(
            1,
            vec![chat_op(b"one"), tasks_op(), chat_op(b"two")],
        ))
        .unwrap();
    assert_eq!(wait_for_tip(&mut sub, (1, 2)), vec![(1, 2)]);
    assert_eq!(store.fold_tip("chat").unwrap(), Some((1, 2)));

    // a module with no folding guest has no tip to report — absent, and the
    // op FEED watermark says nothing about it either way.
    assert_eq!(store.fold_tip("tasks").unwrap(), None);
    assert_eq!(store.applied_height("tasks").unwrap(), 1);

    // a quiet block advances the feed watermark and leaves the tip alone —
    // the honest shape: the tip vouches for folded ROWS, not for blocks.
    store.apply_block(&block(2, vec![tasks_op()])).unwrap();
    assert_eq!(store.applied_height("chat").unwrap(), 2);
    assert_eq!(store.fold_tip("chat").unwrap(), Some((1, 2)));
}

/// A BLOCK THE ENGINE CUTS MID-BATCH NEVER OVER-CLAIMS. fluent31's
/// `trigger_batch` hands a fold at most 512 events per invocation, so a block
/// past that folds in several transactions — and the tip after each one has to
/// name where the cut FELL, not where the block ends. This is the whole reason
/// the record is `(height, seq)`.
#[test]
fn a_batch_cut_mid_block_parks_the_tip_at_the_cut() {
    /// past fluent31's `trigger_batch` (512), so the engine must cut.
    const OPS: u32 = 600;

    let dir = tempfile::tempdir().unwrap();
    let store = mapped_store(dir.path());
    let mut sub = store
        .subscribe("chat", FOLD_PREFIX.as_bytes(), Some(b"fold0"))
        .unwrap();

    let ops = (0..OPS).map(|n| chat_op(format!("op{n}").as_bytes()));
    store.apply_block(&block(1, ops.collect())).unwrap();

    let tips = wait_for_tip(&mut sub, (1, OPS - 1));
    assert!(
        tips.len() > 1,
        "{OPS} ops cannot fit one invocation — the cut is what this pins: {tips:?}"
    );
    assert!(
        tips.windows(2).all(|pair| pair[0] < pair[1]),
        "the tip only ever moves forward: {tips:?}"
    );
    // the intermediate tip is a REAL row of this block, strictly inside it: a
    // shell that stamped the block's end at the first invocation would claim
    // rows it had not folded yet.
    let cut = tips[0];
    assert_eq!(cut.0, 1);
    assert!(
        cut.1 < OPS - 1,
        "the first invocation stopped short: {cut:?}"
    );
    assert!(
        store
            .get("chat", seen_key(cut.0, cut.1).as_bytes())
            .unwrap()
            .is_some(),
        "the tip names a row whose derived write is committed"
    );
    assert_eq!(store.fold_tip("chat").unwrap(), Some((1, OPS - 1)));
}

/// A MAPPER SWAP LEAVES THE TIP STANDING — the honest upgrade hazard, and the
/// opposite of the absent-tip one. `converge_guest` installs the new wasm and
/// returns: no refold, no `clear_db`. So after an upgrade the tip still reads
/// the position the PREVIOUS mapper folded to, and still vouches for the rows
/// that mapper wrote. Reopening with the guest REMOVED is the extreme case of
/// the same swap, and it is what this drives — the tip survives a change that
/// tore the fold down entirely.
///
/// This is why a mapper whose derived shape changes ships with a boundary
/// stamp or a chain replay (spec §3.2.4): the tip reports fold PROGRESS over
/// the op feed, never that the rows match the installed mapper.
#[test]
fn a_guest_swap_leaves_the_fold_tip_where_it_stood() {
    let dir = tempfile::tempdir().unwrap();
    let store = mapped_store(dir.path());

    let mut sub = store.subscribe("chat", b"seen/", Some(b"seen0")).unwrap();
    store.apply_block(&block(1, vec![chat_op(b"one")])).unwrap();
    wait_for_keys(&mut sub, [seen_key(1, 0)]);
    assert_eq!(store.fold_tip("chat").unwrap(), Some((1, 0)));
    drop(sub);
    drop(store);

    let swapped = bare_store(dir.path());
    assert_eq!(
        swapped.fold_tip("chat").unwrap(),
        Some((1, 0)),
        "converge_guest never wipes the tip — it is stale, not absent"
    );
    assert!(
        swapped
            .get("chat", seen_key(1, 0).as_bytes())
            .unwrap()
            .is_some(),
        "the rows it vouches for are still the old mapper's"
    );
}

#[test]
fn backfill_wipes_derived_state_and_recreates_the_fold() {
    let dir = tempfile::tempdir().unwrap();
    let store = mapped_store(dir.path());

    let mut sub = store.subscribe("chat", b"seen/", Some(b"seen0")).unwrap();
    store.apply_block(&block(1, vec![chat_op(b"one")])).unwrap();
    wait_for_keys(&mut sub, [seen_key(1, 0)]);
    assert_eq!(store.fold_tip("chat").unwrap(), Some((1, 0)));

    store.mark_backfilled("chat", 5).unwrap();
    assert_eq!(store.applied_height("chat").unwrap(), 5);
    assert_eq!(store.backfill_height("chat").unwrap(), Some(5));
    // THE TIP GOES WITH THE ROWS IT VOUCHED FOR. `clear_db` wipes it like any
    // other derived key, so a boundary stamp reports UNKNOWN rather than a
    // position whose derived state no longer exists — which is exactly why a
    // client waiting on the tip must escape by timeout instead of blocking.
    assert_eq!(store.fold_tip("chat").unwrap(), None);
    let seen = store.scan("chat", b"seen/", None, 10).unwrap();
    assert!(seen.entries.is_empty(), "derived rows wiped");
    let ops = store.scan("chat", OP_PREFIX.as_bytes(), None, 10).unwrap();
    assert!(
        ops.entries.is_empty(),
        "op feed wiped — it honestly starts at the boundary"
    );

    // the fold trigger was re-registered: new blocks fold from a clean slate,
    // and no pre-wipe event ever resurrects a wiped row.
    let mut sub = store.subscribe("chat", b"seen/", Some(b"seen0")).unwrap();
    store.apply_block(&block(6, vec![chat_op(b"two")])).unwrap();
    wait_for_keys(&mut sub, [seen_key(6, 0)]);
    assert_eq!(
        store.view("chat", b"count").unwrap(),
        1u64.to_be_bytes().to_vec(),
        "the counter re-derives from zero — nothing pre-wipe survived"
    );
    let seen = store.scan("chat", b"seen/", None, 10).unwrap();
    assert_eq!(seen.entries.len(), 1);
}

#[test]
fn reopen_without_a_guest_converges_the_database() {
    let dir = tempfile::tempdir().unwrap();
    {
        let store = mapped_store(dir.path());
        let mut sub = store.subscribe("chat", b"seen/", Some(b"seen0")).unwrap();
        store.apply_block(&block(1, vec![chat_op(b"one")])).unwrap();
        wait_for_keys(&mut sub, [seen_key(1, 0)]);
    }
    {
        // the module stopped shipping an index guest: install + trigger go.
        let store = bare_store(dir.path());
        assert!(matches!(
            store.view("chat", b"count"),
            Err(Error::ViewUnsupported)
        ));
        assert!(store.fold_status("chat").unwrap().is_none());
        // already-derived rows still serve — the tier is read-available even
        // without its mapper.
        let seen = store.scan("chat", b"seen/", None, 10).unwrap();
        assert_eq!(seen.entries.len(), 1);
    }
    // and shipping one again re-registers the fold from where the feed is.
    let store = mapped_store(dir.path());
    let mut sub = store.subscribe("chat", b"seen/", Some(b"seen0")).unwrap();
    store.apply_block(&block(2, vec![chat_op(b"two")])).unwrap();
    wait_for_keys(&mut sub, [seen_key(2, 0)]);
    assert_eq!(
        store.view("chat", b"count").unwrap(),
        2u64.to_be_bytes().to_vec()
    );
}

// ----------------------------------------------------------------------------
// the blocks database
// ----------------------------------------------------------------------------

#[test]
fn block_records_serve_newest_first_tail_oldest_first() {
    let dir = tempfile::tempdir().unwrap();
    let store = bare_store(dir.path());
    for h in 1..=5 {
        store
            .apply_block(&block_with_record(h, vec![chat_op(b"{}")]))
            .unwrap();
    }

    let rows = store.recent_block_rows(3).unwrap();
    assert_eq!(rows.len(), 3);
    // the newest 3 (heights 3..=5), oldest-first — the ring's contract.
    assert_eq!(rows[0], br#"{"height":3}"#.to_vec());
    assert_eq!(rows[2], br#"{"height":5}"#.to_vec());
    assert_eq!(store.recent_block_rows(100).unwrap().len(), 5);
    assert_eq!(store.blocks_height().unwrap(), 5);
}

#[test]
fn block_record_lands_without_ops_and_advances_resume() {
    let dir = tempfile::tempdir().unwrap();
    let store = bare_store(dir.path());
    // a block whose op stream is empty for the index (e.g. a finalized-
    // but-rejected frame) still shows in the explorer.
    store
        .apply_block(&block_with_record(9, Vec::new()))
        .unwrap();

    assert_eq!(store.recent_block_rows(10).unwrap().len(), 1);
    // every module's watermark advances — quiet is not stale — but no op
    // rows were written.
    assert_eq!(store.applied_height("chat").unwrap(), 9);
    let ops = store.scan("chat", OP_PREFIX.as_bytes(), None, 10).unwrap();
    assert!(ops.entries.is_empty(), "no op rows");
    assert_eq!(store.resume_height().unwrap(), 9, "blocks watermark counts");
}

#[test]
fn block_record_without_feed_leaves_module_watermarks_alone() {
    let dir = tempfile::tempdir().unwrap();
    let store = bare_store(dir.path());
    // a boundary follower's write: the explorer row lands, the blocks
    // watermark advances, and NO module watermark moves — read models
    // answer to the boundary stamp, not to this row.
    store
        .apply_block_record(7, br#"{"height":7}"#.to_vec())
        .unwrap();

    assert_eq!(
        store.recent_block_rows(10).unwrap(),
        vec![br#"{"height":7}"#.to_vec()]
    );
    assert_eq!(store.blocks_height().unwrap(), 7);
    assert_eq!(store.applied_height("chat").unwrap(), 0);
    assert_eq!(store.applied_height("tasks").unwrap(), 0);

    // idempotent at or below the blocks watermark, exactly like the feed.
    store
        .apply_block_record(7, br#"{"height":"dup"}"#.to_vec())
        .unwrap();
    store
        .apply_block_record(3, br#"{"height":3}"#.to_vec())
        .unwrap();
    assert_eq!(
        store.recent_block_rows(10).unwrap(),
        vec![br#"{"height":7}"#.to_vec()]
    );
}

#[test]
fn block_records_are_idempotent_and_survive_reopen() {
    let dir = tempfile::tempdir().unwrap();
    {
        let store = bare_store(dir.path());
        let b = block_with_record(1, vec![chat_op(b"{}")]);
        store.apply_block(&b).unwrap();
        store.apply_block(&b).expect("replay is a skip");
    }
    let store = bare_store(dir.path());
    let rows = store.recent_block_rows(10).unwrap();
    assert_eq!(rows.len(), 1, "no duplicate rows; rows survive reopen");
    assert_eq!(store.blocks_height().unwrap(), 1);
}

#[test]
fn prefix_successor_edges() {
    assert_eq!(prefix_successor(b"op/"), Some(b"op0".to_vec()));
    assert_eq!(prefix_successor(&[0x01, 0xff]), Some(vec![0x02]));
    assert_eq!(prefix_successor(&[0xff, 0xff]), None);
    assert_eq!(prefix_successor(b""), None);
}

// ----------------------------------------------------------------------------
// the shipping lane
// ----------------------------------------------------------------------------

#[test]
fn checkpoint_ships_and_staged_install_opens_identically() {
    let src_dir = tempfile::tempdir().unwrap();
    let dest_dir = tempfile::tempdir().unwrap();
    let source = mapped_store(src_dir.path());

    let mut sub = source.subscribe("chat", b"seen/", Some(b"seen0")).unwrap();
    source
        .apply_block(&block_with_record(1, vec![chat_op(b"payload")]))
        .unwrap();
    wait_for_keys(&mut sub, [seen_key(1, 0)]);

    // ship every database: modules + blocks.
    let dest_base = dest_dir.path().join("index");
    for db in ["chat", "tasks", BLOCKS_DB_ID] {
        let files = source.checkpoint_files(db).expect("cut archive");
        assert!(!files.is_empty(), "an archive is never empty");
        stage_shipped_db(&DiskFs, &dest_base, db, &files).expect("stage");
    }
    commit_staged(&DiskFs, &dest_base).expect("commit");

    let shipped = mapped_store(&dest_base);
    // feed, derived rows, watermark, explorer rows — all travelled.
    assert_eq!(shipped.applied_height("chat").unwrap(), 1);
    let ops = shipped
        .scan("chat", OP_PREFIX.as_bytes(), None, 10)
        .unwrap();
    assert_eq!(ops.entries.len(), 1);
    let seen = shipped.scan("chat", b"seen/", None, 10).unwrap();
    assert_eq!(seen.entries.len(), 1);
    assert_eq!(
        shipped.view("chat", b"count").unwrap(),
        1u64.to_be_bytes().to_vec()
    );
    assert_eq!(shipped.recent_block_rows(10).unwrap().len(), 1);

    // the shipped copy keeps folding: it is a live store, not a snapshot.
    let mut sub = shipped.subscribe("chat", b"seen/", Some(b"seen0")).unwrap();
    shipped
        .apply_block(&block(2, vec![chat_op(b"more")]))
        .unwrap();
    wait_for_keys(&mut sub, [seen_key(2, 0)]);
    assert_eq!(
        shipped.view("chat", b"count").unwrap(),
        2u64.to_be_bytes().to_vec()
    );
}

#[test]
fn unmarked_staging_is_discarded_marked_staging_replaces() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("index");

    // seed a live store with one applied block, then close it.
    {
        let store = bare_store(&base);
        store.apply_block(&block(1, vec![chat_op(b"{}")])).unwrap();
    }

    // a torn fetch (no marker) must be discarded, keeping the live data.
    stage_shipped_db(&DiskFs, &base, "chat", &[("garbage".into(), vec![1, 2, 3])]).unwrap();
    {
        let store = bare_store(&base);
        assert_eq!(
            store.applied_height("chat").unwrap(),
            1,
            "torn staging discarded"
        );
    }

    // a marked install replaces: ship the current chat db, wipe, restage.
    let files = {
        let store = bare_store(&base);
        store.checkpoint_files("chat").unwrap()
    };
    stage_shipped_db(&DiskFs, &base, "chat", &files).unwrap();
    commit_staged(&DiskFs, &base).unwrap();
    let store = bare_store(&base);
    assert_eq!(
        store.applied_height("chat").unwrap(),
        1,
        "staged copy adopted"
    );
}

#[test]
fn stage_refuses_hostile_names() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("index");
    let files = [("f".to_string(), vec![0u8])];

    for bad in ["", ".", "..", "a/b", "a\\b", ".hidden", "LOCK", STAGING_DIR] {
        assert!(
            matches!(
                stage_shipped_db(&DiskFs, &base, bad, &files),
                Err(Error::Shipping(_))
            ),
            "db name {bad:?} must refuse"
        );
    }
    for bad in ["", "..", "a/b", ".hidden", "LOCK"] {
        assert!(
            matches!(
                stage_shipped_db(&DiskFs, &base, "chat", &[(bad.to_string(), vec![0u8])]),
                Err(Error::Shipping(_))
            ),
            "file name {bad:?} must refuse"
        );
    }
}

#[test]
fn checkpoint_files_sweeps_its_stale_archive_and_refuses_poisoned() {
    let dir = tempfile::tempdir().unwrap();
    let store = bare_store(dir.path());
    store.apply_block(&block(1, vec![chat_op(b"{}")])).unwrap();

    // twice in a row: the second cut must sweep nothing stale (the first
    // deleted its fork) and still succeed.
    let first = store.checkpoint_files("chat").unwrap();
    let second = store.checkpoint_files("chat").unwrap();
    assert_eq!(
        first.iter().map(|(n, _)| n).collect::<Vec<_>>(),
        second.iter().map(|(n, _)| n).collect::<Vec<_>>()
    );

    // poison the store; cuts must refuse rather than ship a torn model.
    store
        .apply_block(&block(
            2,
            vec![AppliedOp {
                module: "ghost".into(),
                origin: OriginTag::system(),
                payload: vec![],
                assigned: Vec::new(),
            }],
        ))
        .unwrap_err();
    assert!(matches!(
        store.checkpoint_files("chat"),
        Err(Error::Poisoned)
    ));
}

// ----------------------------------------------------------------------------
// staging on the mem disk
// ----------------------------------------------------------------------------

#[test]
fn staging_round_trips_on_the_mem_disk() {
    let disk = MemDisk::default();
    let base = Path::new("/index");
    let files = [
        ("CURRENT".to_string(), b"manifest".to_vec()),
        ("table-1".to_string(), vec![1, 2, 3]),
    ];
    stage_shipped_db(&disk, base, "chat", &files).unwrap();
    commit_staged(&disk, base).unwrap();
    adopt_staged(&disk, base).unwrap();

    assert!(disk.exists(&base.join("chat").join("CURRENT")));
    assert!(disk.exists(&base.join("chat").join("table-1")));
    assert!(!disk.exists(&base.join(STAGING_DIR)), "staging root swept");
}

#[test]
fn torn_staging_is_discarded_on_the_mem_disk() {
    let disk = MemDisk::default();
    let base = Path::new("/index");
    stage_shipped_db(&disk, base, "chat", &[("f".to_string(), vec![1])]).unwrap();
    // no commit marker: adoption must discard, not adopt.
    adopt_staged(&disk, base).unwrap();
    assert!(!disk.exists(&base.join("chat")));
    assert!(!disk.exists(&base.join(STAGING_DIR)));
}

#[test]
fn committed_staging_replaces_an_existing_dir_on_the_mem_disk() {
    let disk = MemDisk::default();
    let base = Path::new("/index");
    disk.create_dir_all(&base.join("chat")).unwrap();
    disk.write(&base.join("chat").join("stale"), b"old")
        .unwrap();

    stage_shipped_db(&disk, base, "chat", &[("fresh".to_string(), vec![1])]).unwrap();
    commit_staged(&disk, base).unwrap();
    adopt_staged(&disk, base).unwrap();

    assert!(
        !disk.exists(&base.join("chat").join("stale")),
        "old dir replaced"
    );
    assert!(disk.exists(&base.join("chat").join("fresh")));
}

// ----------------------------------------------------------------------------
// rendering
// ----------------------------------------------------------------------------

#[test]
fn user_handle_renders_names_and_keys() {
    assert_eq!(user_handle(b"jess"), "jess");
    assert_eq!(user_handle(&[0xab, 0xcd]), "abcd");
    assert_eq!(user_handle(b""), "");
    assert_eq!(user_handle(b"line\nbreak"), "6c696e650a627265616b");
}
