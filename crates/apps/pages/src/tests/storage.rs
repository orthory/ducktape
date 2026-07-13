use super::*;

#[test]
fn write_moves_root_and_composes_into_global_root() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        let r0 = p.root();
        seed_page(&mut p, "p1").await;
        let r1 = p.root();
        assert_ne!(r0, r1, "a write must move the root");
        assert_ne!(r1, StateRoot::ZERO, "root after write must be non-zero");

        struct Stub;
        #[async_trait::async_trait(?Send)]
        impl Module for Stub {
            fn id(&self) -> ModuleId {
                "stub".into()
            }
            fn root(&self) -> StateRoot {
                StateRoot([9u8; sdk::ROOT_LEN])
            }
            async fn execute(&mut self, _c: &mut dyn Ctx, _m: &Msg) -> Result<(), Error> {
                Ok(())
            }
        }
        let stub = Stub;
        let g = {
            let mods: [&dyn Module; 2] = [&p, &stub];
            global_root(&mods)
        };
        assert_ne!(g, host::global_root(&[&stub as &dyn Module]));
    });
}

// host-lent staging: a whole staged edit (including a staged DELETE) that
// then ABORTS must leave no trace.
#[test]
fn staged_writes_and_deletes_roll_back_on_abort() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        seed_page(&mut p, "p1").await;
        let r_before = p.root();

        // stage a removal (a delete) AND an insert, then abort.
        p.execute(
            &mut TestCtx::new(),
            &msg(&PageMsg::RemoveBlock {
                block_id: "b2".into(),
            }),
        )
        .await
        .unwrap();
        p.execute(
            &mut TestCtx::new(),
            &msg(&PageMsg::InsertBlock {
                parent: "p1".into(),
                after: None,
                block: para("ghost", "should vanish"),
            }),
        )
        .await
        .unwrap();
        p.abort_block().await.unwrap();

        assert_eq!(p.root(), r_before, "aborted block must not move the root");
        let page = get_page(&p, "p1").await.unwrap();
        assert_eq!(ids(&page), ["p1", "b1", "b2", "b3"]);
    });
}

// mid-block read-your-writes across the overlay, INCLUDING staged deletes:
// op2 parents on op1's staged insert; op4 sees op3's staged delete.
#[test]
fn staged_writes_are_visible_within_one_block() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        apply_commit(
            &mut p,
            &PageMsg::CreatePage {
                page_id: "p1".into(),
                title: "one".into(),
                parent: None,
            },
        )
        .await;
        // two inserts, NO commit between: the child hangs off a parent
        // that exists only in the overlay.
        p.execute(
            &mut TestCtx::new(),
            &msg(&PageMsg::InsertBlock {
                parent: "p1".into(),
                after: None,
                block: para("b1", "one"),
            }),
        )
        .await
        .unwrap();
        p.execute(
            &mut TestCtx::new(),
            &msg(&PageMsg::InsertBlock {
                parent: "b1".into(),
                after: None,
                block: para("c1", "two"),
            }),
        )
        .await
        .unwrap();
        // a staged delete is visible too: re-inserting the removed id in
        // the SAME block-height succeeds (absence through the overlay).
        p.execute(
            &mut TestCtx::new(),
            &msg(&PageMsg::RemoveBlock {
                block_id: "c1".into(),
            }),
        )
        .await
        .unwrap();
        p.execute(
            &mut TestCtx::new(),
            &msg(&PageMsg::InsertBlock {
                parent: "b1".into(),
                after: None,
                block: para("c1", "again"),
            }),
        )
        .await
        .unwrap();
        p.commit_block().await.unwrap();

        let page = get_page(&p, "p1").await.unwrap();
        assert_eq!(ids(&page), ["p1", "b1", "c1"]);
        assert_eq!(page[2].text, "again");
    });
}

// the poison-pill guard: an op that would grow one serialized block past
// MAX_BLOCK_LEN is rejected at WRITE time — never staged, never committed.
#[test]
fn oversized_block_is_rejected_before_staging() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        apply_commit(
            &mut p,
            &PageMsg::CreatePage {
                page_id: "p1".into(),
                title: "one".into(),
                parent: None,
            },
        )
        .await;
        let r_before = p.root();

        let huge = "x".repeat(MAX_BLOCK_LEN + 1);
        apply_expect_err(
            &mut p,
            &PageMsg::InsertBlock {
                parent: "p1".into(),
                after: None,
                block: para("big", &huge),
            },
            "block too large",
        )
        .await;
        assert!(p.pending.is_empty(), "a rejected write must not be staged");
        assert_eq!(
            p.root(),
            r_before,
            "a rejected write must not move the root"
        );
        assert_eq!(get_page(&p, "p1").await.unwrap().len(), 1);
    });
}

// corruption must surface as a DISTINCT error, never absence: mapping it
// to "not found" would let CreatePage re-seed a root over the corrupt
// bytes, silently destroying the data.
#[test]
fn corrupt_stored_block_errors_as_corruption_not_absence() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        // commit bytes that are NOT valid Block json under blk1's key
        // (simulating on-disk corruption; unreachable through PageMsg ops).
        p.pending
            .insert(b"blk1".to_vec(), Some(b"not json".to_vec()));
        p.commit_block().await.unwrap();

        apply_expect_err(
            &mut p,
            &PageMsg::UpdateText {
                block_id: "blk1".into(),
                text: "x".into(),
                marks: None,
            },
            "corrupt",
        )
        .await;
        apply_expect_err(
            &mut p,
            &PageMsg::CreatePage {
                page_id: "blk1".into(),
                title: "steal".into(),
                parent: None,
            },
            "corrupt",
        )
        .await;
        // the read path surfaces the decode failure too (error, not None).
        assert!(
            p.query(&encode_query(&PageQuery::GetBlock {
                block_id: "blk1".into()
            }))
            .await
            .is_err()
        );
    });
}
