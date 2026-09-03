use super::*;

#[test]
fn create_page_is_idempotent_and_preserves_the_title() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        seed_page(&mut p, "p1").await;
        apply_commit(
            &mut p,
            &PageMsg::UpdateText {
                block_id: "p1".into(),
                text: "renamed".into(),
                marks: None,
            },
        )
        .await;
        // re-create must neither wipe blocks nor clobber the live title.
        apply_commit(
            &mut p,
            &PageMsg::CreatePage {
                page_id: "p1".into(),
                title: "stale title".into(),
            },
        )
        .await;
        let page = get_page(&p, "p1").await.unwrap();
        assert_eq!(ids(&page), ["p1", "b1", "b2", "b3"]);
        assert_eq!(page[0].text, "renamed");
    });
}

#[test]
fn page_query_replies_stop_before_the_rpc_client_limit() {
    deterministic::Runner::default().start(|context| async move {
        const RPC_CLIENT_LIMIT: usize = 8 * 1024 * 1024;
        let mut p = pages_on!(context, "pages");
        apply_commit(
            &mut p,
            &PageMsg::CreatePage {
                page_id: "root".into(),
                title: "root".into(),
            },
        )
        .await;
        // block TEXT is what can carry this much: a page title is capped at
        // MAX_PAGE_TITLE_LEN, so a title can no longer bloat a reply.
        let text = "x".repeat(700 * 1024);
        let mut after = None;
        for index in 0..10 {
            let id = format!("p{index:02}");
            apply_commit(
                &mut p,
                &PageMsg::InsertBlock {
                    parent: "root".into(),
                    after,
                    block: para(&id, &text),
                },
            )
            .await;
            after = Some(id);
        }

        let block_reply = p
            .query(&encode_query(&PageQuery::GetPage {
                page_id: "root".into(),
                after: None,
                limit: u16::MAX,
            }))
            .await
            .unwrap();
        assert!(block_reply.len() < RPC_CLIENT_LIMIT);
        let PageReply::Page(Some(block_page)) = decode_reply(&block_reply).unwrap() else {
            panic!("expected Page")
        };
        assert!(block_page.next_after.is_some());
        assert!(block_page.blocks.len() < 11);
        assert_eq!(get_page(&p, "root").await.unwrap().len(), 11);

    });
}

// the reserved sentinel is UNREACHABLE by any op, so a block write can
// never overwrite the enumeration index; and it reads as absence.
#[test]
fn reserved_index_id_is_rejected() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        seed_page(&mut p, "p1").await;
        let r_before = p.root();

        apply_expect_err(
            &mut p,
            &PageMsg::CreatePage {
                page_id: PAGE_INDEX_KEY.into(),
                title: "clobber".into(),
            },
            "reserved block id",
        )
        .await;
        apply_expect_err(
            &mut p,
            &PageMsg::InsertBlock {
                parent: "p1".into(),
                after: None,
                block: para(PAGE_INDEX_KEY, "clobber"),
            },
            "reserved block id",
        )
        .await;
        assert!(p.staged.is_empty(), "a rejected op must stage nothing");
        assert_eq!(p.root(), r_before, "a rejected op must not move the root");
        // the sentinel reads as absence on the query surface.
        assert!(get_block(&p, PAGE_INDEX_KEY).await.is_none());
        assert!(get_page(&p, PAGE_INDEX_KEY).await.is_none());
    });
}

// ── nested pages (Page blocks in the document tree) ──
// folder EDGES are enumeration-index shape, rendered by the index tier now
// (see `index::tests`); the write-path rules stay covered through kept
// surfaces below and in `block_tree`.

#[test]
fn page_block_accepts_any_real_block_parent() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        seed_page(&mut p, "p1").await; // p1 + blocks b1,b2,b3
        // parent does not exist
        apply_expect_err(
            &mut p,
            &PageMsg::InsertBlock {
                parent: "ghost".into(),
                after: None,
                block: page("x", "x"),
            },
            "parent block not found",
        )
        .await;
        // A subpage can sit under any block, not only another Page block.
        apply_commit(
            &mut p,
            &PageMsg::InsertBlock {
                parent: "b1".into(),
                after: None,
                block: page("y", "y"),
            },
        )
        .await;
        assert_eq!(
            get_block(&p, "y").await.unwrap().parent.as_deref(),
            Some("b1")
        );
        // the derived folder edge (y folders under p1) is index-tier shape,
        // rendered by the index guest's page list (see `index::tests`).
    });
}

#[test]
fn moving_page_blocks_renests_and_rejects_cycles() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        for id in ["a", "b", "c"] {
            apply_commit(
                &mut p,
                &PageMsg::CreatePage {
                    page_id: id.into(),
                    title: id.into(),
                },
            )
            .await;
        }
        // b under a, c under b.
        apply_commit(
            &mut p,
            &PageMsg::MoveBlock {
                block_id: "b".into(),
                parent: Some("a".into()),
                after: None,
            },
        )
        .await;
        apply_commit(
            &mut p,
            &PageMsg::MoveBlock {
                block_id: "c".into(),
                parent: Some("b".into()),
                after: None,
            },
        )
        .await;
        // a under c would cycle (a -> c -> b -> a) — the rejection is ALSO the
        // proof both renests landed: the guard walks the live folder edges.
        apply_expect_err(
            &mut p,
            &PageMsg::MoveBlock {
                block_id: "a".into(),
                parent: Some("c".into()),
                after: None,
            },
            "inside the moved subtree",
        )
        .await;
        // self-parent cycles too.
        apply_expect_err(
            &mut p,
            &PageMsg::MoveBlock {
                block_id: "a".into(),
                parent: Some("a".into()),
                after: None,
            },
            "inside the moved subtree",
        )
        .await;
        // detach b to top level — the just-rejected renest becomes legal
        // (c's ancestry is now c -> b -> top), proving the detach landed.
        apply_commit(
            &mut p,
            &PageMsg::MoveBlock {
                block_id: "b".into(),
                parent: None,
                after: None,
            },
        )
        .await;
        let b = get_block(&p, "b").await.unwrap();
        assert!(b.parent.is_none(), "detach landed");
        // Only Page blocks can detach to the top level.
        seed_page(&mut p, "pg").await;
        apply_expect_err(
            &mut p,
            &PageMsg::MoveBlock {
                block_id: "b1".into(),
                parent: None,
                after: None,
            },
            "only page blocks",
        )
        .await;
        // A page may move under a regular content block.
        apply_commit(
            &mut p,
            &PageMsg::MoveBlock {
                block_id: "a".into(),
                parent: Some("b1".into()),
                after: None,
            },
        )
        .await;
        assert_eq!(
            get_block(&p, "a").await.unwrap().parent.as_deref(),
            Some("b1")
        );
    });
}

#[test]
fn removing_page_block_removes_its_entire_nested_subtree() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        // grand -> parent -> child ; parent also has a content block pb1.
        apply_commit(
            &mut p,
            &PageMsg::CreatePage {
                page_id: "grand".into(),
                title: "G".into(),
            },
        )
        .await;
        apply_commit(
            &mut p,
            &PageMsg::InsertBlock {
                parent: "grand".into(),
                after: None,
                block: page("parent", "P"),
            },
        )
        .await;
        apply_commit(
            &mut p,
            &PageMsg::InsertBlock {
                parent: "parent".into(),
                after: None,
                block: page("child", "C"),
            },
        )
        .await;
        apply_commit(
            &mut p,
            &PageMsg::InsertBlock {
                parent: "parent".into(),
                after: None,
                block: para("pb1", "body"),
            },
        )
        .await;

        apply_commit(
            &mut p,
            &PageMsg::RemoveBlock {
                block_id: "parent".into(),
            },
        )
        .await;

        // parent's root + content block are gone …
        assert!(get_block(&p, "parent").await.is_none());
        assert!(get_block(&p, "pb1").await.is_none());
        assert!(get_page(&p, "parent").await.is_none());
        // Nested Page blocks are part of the removed subtree too.
        assert!(get_page(&p, "child").await.is_none());
        assert!(get_block(&p, "child").await.is_none());
    });
}
