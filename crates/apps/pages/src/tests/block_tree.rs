use super::*;

#[test]
fn inline_marks_persist_and_rebase_in_utf16() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        seed_page(&mut p, "p1").await;
        apply_commit(
            &mut p,
            &PageMsg::UpdateText {
                block_id: "b1".into(),
                text: "a🦆bc".into(),
                marks: None,
            },
        )
        .await;
        apply_expect_err(
            &mut p,
            &PageMsg::SetSpanMark {
                block_id: "b1".into(),
                start: 1,
                end: 2,
                kind: InlineMark::Bold,
                active: true,
            },
            "invalid text range",
        )
        .await;
        apply_commit(
            &mut p,
            &PageMsg::SetSpanMark {
                block_id: "b1".into(),
                start: 1,
                end: 3,
                kind: InlineMark::Bold,
                active: true,
            },
        )
        .await;
        apply_commit(
            &mut p,
            &PageMsg::UpdateText {
                block_id: "b1".into(),
                text: "++a🦆bc".into(),
                marks: None,
            },
        )
        .await;
        assert_eq!(
            get_block(&p, "b1").await.unwrap().marks,
            vec![SpanMark { start: 3, end: 5, kind: InlineMark::Bold }]
        );
        apply_commit(
            &mut p,
            &PageMsg::UpdateText {
                block_id: "b1".into(),
                text: "merged".into(),
                marks: Some(vec![SpanMark {
                    start: 0,
                    end: 6,
                    kind: InlineMark::Italic,
                }]),
            },
        )
        .await;
        assert_eq!(
            get_block(&p, "b1").await.unwrap().marks,
            vec![SpanMark { start: 0, end: 6, kind: InlineMark::Italic }]
        );
    });
}

#[test]
fn create_page_and_insert_blocks_in_order() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        seed_page(&mut p, "p1").await;
        // after None -> first child, so b0 lands before b1.
        apply_commit(
            &mut p,
            &PageMsg::InsertBlock {
                parent: "p1".into(),
                after: None,
                block: para("b0", "zero"),
            },
        )
        .await;

        let page = get_page(&p, "p1").await.unwrap();
        assert_eq!(ids(&page), ["p1", "b0", "b1", "b2", "b3"]);
        assert_eq!(page[0].kind, BlockKind::Page);
        assert_eq!(page[0].text, "p1 title");
        assert_eq!(page[0].children, ["b0", "b1", "b2", "b3"]);
    });
}

#[test]
fn nested_children_come_back_in_preorder() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        seed_page(&mut p, "p1").await;
        // c1 under b1, d1 under c1: preorder puts b1's whole subtree
        // before its next sibling b2.
        apply_commit(
            &mut p,
            &PageMsg::InsertBlock {
                parent: "b1".into(),
                after: None,
                block: para("c1", "child"),
            },
        )
        .await;
        apply_commit(
            &mut p,
            &PageMsg::InsertBlock {
                parent: "c1".into(),
                after: None,
                block: para("d1", "grandchild"),
            },
        )
        .await;

        let page = get_page(&p, "p1").await.unwrap();
        assert_eq!(ids(&page), ["p1", "b1", "c1", "d1", "b2", "b3"]);
    });
}

// the addressability contract: a bare block id resolves with NO page
// context, and the answer carries where the block lives — exactly what a
// future cross-module reference needs.
#[test]
fn get_block_by_id_alone_carries_page_context() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        seed_page(&mut p, "p1").await;
        apply_commit(
            &mut p,
            &PageMsg::InsertBlock {
                parent: "b1".into(),
                after: None,
                block: para("c1", "deep"),
            },
        )
        .await;

        let blk = get_block(&p, "c1").await.unwrap();
        assert_eq!(blk.parent.as_deref(), Some("b1"));
        assert_eq!(blk.page, "p1");
        assert_eq!(blk.text, "deep");
        // a non-page block id is NOT a page.
        assert!(get_page(&p, "c1").await.is_none());
    });
}

#[test]
fn block_ids_are_globally_unique_across_pages() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        seed_page(&mut p, "p1").await;
        apply_commit(
            &mut p,
            &PageMsg::CreatePage {
                page_id: "p2".into(),
                title: "two".into(),
                parent: None,
            },
        )
        .await;
        // b1 lives in p1 — inserting it into p2 must fail globally.
        apply_expect_err(
            &mut p,
            &PageMsg::InsertBlock {
                parent: "p2".into(),
                after: None,
                block: para("b1", "dup"),
            },
            "duplicate block id",
        )
        .await;
        // a page id is a block id too: reusing one as a block id fails …
        apply_expect_err(
            &mut p,
            &PageMsg::InsertBlock {
                parent: "p2".into(),
                after: None,
                block: para("p1", "dup"),
            },
            "duplicate block id",
        )
        .await;
        // … and creating a page over an existing NON-page block fails.
        apply_expect_err(
            &mut p,
            &PageMsg::CreatePage {
                page_id: "b1".into(),
                title: "steal".into(),
                parent: None,
            },
            "duplicate block id",
        )
        .await;
    });
}

#[test]
fn update_text_edits_blocks_and_renames_pages() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        seed_page(&mut p, "p1").await;
        apply_commit(
            &mut p,
            &PageMsg::UpdateText {
                block_id: "b1".into(),
                text: "edited".into(),
                marks: None,
            },
        )
        .await;
        assert_eq!(get_block(&p, "b1").await.unwrap().text, "edited");

        // UpdateText on the root IS the rename; ListPages reads live roots.
        apply_commit(
            &mut p,
            &PageMsg::UpdateText {
                block_id: "p1".into(),
                text: "renamed".into(),
                marks: None,
            },
        )
        .await;
        let pages = list_pages(&p).await;
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].id, "p1");
        assert_eq!(pages[0].title, "renamed");
    });
}

#[test]
fn set_kind_and_checked_enforce_their_domains() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        seed_page(&mut p, "p1").await;
        // paragraph -> todo, then check it off.
        apply_commit(
            &mut p,
            &PageMsg::SetKind {
                block_id: "b1".into(),
                kind: BlockKind::Todo,
            },
        )
        .await;
        apply_commit(
            &mut p,
            &PageMsg::SetChecked {
                block_id: "b1".into(),
                checked: true,
            },
        )
        .await;
        let b1 = get_block(&p, "b1").await.unwrap();
        assert_eq!(b1.kind, BlockKind::Todo);
        assert!(b1.checked);

        // checked is a todo-only surface.
        apply_expect_err(
            &mut p,
            &PageMsg::SetChecked {
                block_id: "b2".into(),
                checked: true,
            },
            "todo",
        )
        .await;
        // pages come only from CreatePage — no converting to Page …
        apply_expect_err(
            &mut p,
            &PageMsg::SetKind {
                block_id: "b2".into(),
                kind: BlockKind::Page,
            },
            "CreatePage",
        )
        .await;
        // … and no converting a root away from Page.
        apply_expect_err(
            &mut p,
            &PageMsg::SetKind {
                block_id: "p1".into(),
                kind: BlockKind::Paragraph,
            },
            "page roots",
        )
        .await;
    });
}

#[test]
fn move_reorders_within_a_parent() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        seed_page(&mut p, "p1").await;
        // b1,b2,b3 -> move b1 after b3 -> b2,b3,b1.
        apply_commit(
            &mut p,
            &PageMsg::MoveBlock {
                block_id: "b1".into(),
                parent: "p1".into(),
                after: Some("b3".into()),
            },
        )
        .await;
        assert_eq!(
            get_page(&p, "p1").await.unwrap()[0].children,
            ["b2", "b3", "b1"]
        );
        // back to the front (after None).
        apply_commit(
            &mut p,
            &PageMsg::MoveBlock {
                block_id: "b1".into(),
                parent: "p1".into(),
                after: None,
            },
        )
        .await;
        assert_eq!(
            get_page(&p, "p1").await.unwrap()[0].children,
            ["b1", "b2", "b3"]
        );
        // self-anchor is a benign no-op.
        apply_commit(
            &mut p,
            &PageMsg::MoveBlock {
                block_id: "b1".into(),
                parent: "p1".into(),
                after: Some("b1".into()),
            },
        )
        .await;
        assert_eq!(
            get_page(&p, "p1").await.unwrap()[0].children,
            ["b1", "b2", "b3"]
        );
    });
}

#[test]
fn move_reparents_a_subtree() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        seed_page(&mut p, "p1").await;
        apply_commit(
            &mut p,
            &PageMsg::InsertBlock {
                parent: "b2".into(),
                after: None,
                block: para("c1", "rides along"),
            },
        )
        .await;
        // b2 (with c1 below) becomes b1's child — the subtree rides along.
        apply_commit(
            &mut p,
            &PageMsg::MoveBlock {
                block_id: "b2".into(),
                parent: "b1".into(),
                after: None,
            },
        )
        .await;
        let page = get_page(&p, "p1").await.unwrap();
        assert_eq!(ids(&page), ["p1", "b1", "b2", "c1", "b3"]);
        assert_eq!(
            get_block(&p, "b2").await.unwrap().parent.as_deref(),
            Some("b1")
        );
        // c1 still knows its page.
        assert_eq!(get_block(&p, "c1").await.unwrap().page, "p1");
    });
}

#[test]
fn illegal_moves_are_rejected() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        seed_page(&mut p, "p1").await;
        apply_commit(
            &mut p,
            &PageMsg::CreatePage {
                page_id: "p2".into(),
                title: "two".into(),
                parent: None,
            },
        )
        .await;
        apply_commit(
            &mut p,
            &PageMsg::InsertBlock {
                parent: "b1".into(),
                after: None,
                block: para("c1", "child"),
            },
        )
        .await;
        // into one's own subtree: b1 under its child c1.
        apply_expect_err(
            &mut p,
            &PageMsg::MoveBlock {
                block_id: "b1".into(),
                parent: "c1".into(),
                after: None,
            },
            "inside the moved subtree",
        )
        .await;
        // across pages.
        apply_expect_err(
            &mut p,
            &PageMsg::MoveBlock {
                block_id: "b1".into(),
                parent: "p2".into(),
                after: None,
            },
            "cross-page",
        )
        .await;
        // a page root.
        apply_expect_err(
            &mut p,
            &PageMsg::MoveBlock {
                block_id: "p2".into(),
                parent: "b1".into(),
                after: None,
            },
            "page roots",
        )
        .await;
        // a bad sibling anchor.
        apply_expect_err(
            &mut p,
            &PageMsg::MoveBlock {
                block_id: "b1".into(),
                parent: "p1".into(),
                after: Some("ghost".into()),
            },
            "after-anchor",
        )
        .await;
    });
}

#[test]
fn remove_deletes_the_whole_subtree() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        seed_page(&mut p, "p1").await;
        apply_commit(
            &mut p,
            &PageMsg::InsertBlock {
                parent: "b1".into(),
                after: None,
                block: para("c1", "child"),
            },
        )
        .await;
        apply_commit(
            &mut p,
            &PageMsg::InsertBlock {
                parent: "c1".into(),
                after: None,
                block: para("d1", "grandchild"),
            },
        )
        .await;
        apply_commit(
            &mut p,
            &PageMsg::RemoveBlock {
                block_id: "b1".into(),
            },
        )
        .await;

        // b1, c1, d1 all gone — by id and from the page.
        for gone in ["b1", "c1", "d1"] {
            assert!(get_block(&p, gone).await.is_none(), "{gone} must be gone");
        }
        let page = get_page(&p, "p1").await.unwrap();
        assert_eq!(ids(&page), ["p1", "b2", "b3"]);

        // roots are not removable.
        apply_expect_err(
            &mut p,
            &PageMsg::RemoveBlock {
                block_id: "p1".into(),
            },
            "page roots",
        )
        .await;
    });
}
