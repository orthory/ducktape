use super::*;

async fn page_slice(p: &Pages, page_id: &str, after: Option<&str>, limit: u16) -> PageBlockPage {
    let reply = p
        .query(&encode_query(&PageQuery::GetPage {
            page_id: page_id.into(),
            after: after.map(str::to_string),
            limit,
        }))
        .await
        .unwrap();
    match decode_reply(&reply).unwrap() {
        PageReply::Page(Some(page)) => page,
        other => panic!("expected Page, got {other:?}"),
    }
}

async fn insert_chain(p: &mut Pages, root: &str, prefix: &str, depth: usize) -> String {
    let mut parent = root.to_string();
    for level in 1..=depth {
        let id = format!("{prefix}{level}");
        apply_commit(
            p,
            &PageMsg::InsertBlock {
                parent,
                after: None,
                block: para(&id, "nested"),
            },
        )
        .await;
        parent = id;
    }
    parent
}

fn stage_page_ancestry(p: &mut Pages, count: usize) -> String {
    let ids: Vec<_> = (0..count).map(|index| format!("page-{index:04}")).collect();
    let mut index = BTreeMap::new();
    for (position, id) in ids.iter().enumerate() {
        let parent = position.checked_sub(1).map(|index| ids[index].clone());
        let children = ids.get(position + 1).cloned().into_iter().collect();
        p.store_block(&Block {
            id: id.clone(),
            parent: parent.clone(),
            page: id.clone(),
            kind: BlockKind::Page,
            text: String::new(),
            marks: Vec::new(),
            checked: false,
            children,
        })
        .unwrap();
        index.insert(id.clone(), parent);
    }
    p.store_block(&Block {
        id: "moving".into(),
        parent: None,
        page: "moving".into(),
        kind: BlockKind::Page,
        text: String::new(),
        marks: Vec::new(),
        checked: false,
        children: Vec::new(),
    })
    .unwrap();
    index.insert("moving".into(), None);
    p.stage_index(&index).unwrap();
    ids.last().unwrap().clone()
}

async fn seed_wide_branch(p: &mut Pages, child_count: usize) {
    let children: Vec<_> = (0..child_count)
        .map(|index| format!("leaf-{index:04}"))
        .collect();
    p.store_block(&Block {
        id: "outer".into(),
        parent: None,
        page: "outer".into(),
        kind: BlockKind::Page,
        text: String::new(),
        marks: Vec::new(),
        checked: false,
        children: vec!["branch".into()],
    })
    .unwrap();
    p.store_block(&Block {
        id: "branch".into(),
        parent: Some("outer".into()),
        page: "outer".into(),
        kind: BlockKind::Paragraph,
        text: String::new(),
        marks: Vec::new(),
        checked: false,
        children: children.clone(),
    })
    .unwrap();
    for id in children {
        p.store_block(&Block {
            id,
            parent: Some("branch".into()),
            page: "outer".into(),
            kind: BlockKind::Paragraph,
            text: String::new(),
            marks: Vec::new(),
            checked: false,
            children: Vec::new(),
        })
        .unwrap();
    }
    p.stage_index(&BTreeMap::from([("outer".into(), None)]))
        .unwrap();
    p.commit_block().await.unwrap();
}

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
            vec![SpanMark {
                start: 3,
                end: 5,
                kind: InlineMark::Bold
            }]
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
            vec![SpanMark {
                start: 0,
                end: 6,
                kind: InlineMark::Italic
            }]
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

#[test]
fn insert_boundary_is_fully_queryable_and_one_deeper_is_rejected() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        apply_commit(
            &mut p,
            &PageMsg::CreatePage {
                page_id: "root".into(),
                title: "root".into(),
            },
        )
        .await;
        let deepest = insert_chain(&mut p, "root", "depth-", MAX_PAGE_DEPTH).await;

        let blocks = get_page(&p, "root").await.unwrap();
        assert_eq!(blocks.len(), MAX_PAGE_DEPTH + 1);
        assert_eq!(blocks.last().unwrap().id, deepest);

        let boundary_parent = format!("depth-{}", MAX_PAGE_DEPTH - 1);
        apply_commit(
            &mut p,
            &PageMsg::InsertBlock {
                parent: boundary_parent,
                after: None,
                block: page("subpage", "subpage"),
            },
        )
        .await;
        apply_commit(
            &mut p,
            &PageMsg::InsertBlock {
                parent: "subpage".into(),
                after: None,
                block: para("inside-subpage", "separate depth budget"),
            },
        )
        .await;
        assert_eq!(
            ids(&get_page(&p, "subpage").await.unwrap()),
            ["subpage", "inside-subpage"]
        );
        let outer_blocks = get_page(&p, "root").await.unwrap();
        let outer_ids = ids(&outer_blocks);
        assert!(outer_ids.contains(&"subpage"));
        assert!(!outer_ids.contains(&"inside-subpage"));

        apply_expect_err(
            &mut p,
            &PageMsg::InsertBlock {
                parent: deepest,
                after: None,
                block: para("too-deep", "rejected"),
            },
            "page nesting is too deep",
        )
        .await;
        assert!(get_block(&p, "too-deep").await.is_none());
    });
}

#[test]
fn page_cursor_preserves_preorder_and_stops_at_nested_pages() {
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
                block: page("sub", "nested"),
            },
        )
        .await;
        apply_commit(
            &mut p,
            &PageMsg::InsertBlock {
                parent: "sub".into(),
                after: None,
                block: para("inside", "belongs to sub"),
            },
        )
        .await;

        let first = page_slice(&p, "p1", None, 2).await;
        assert_eq!(ids(&first.blocks), ["p1", "b1"]);
        assert_eq!(first.next_after.as_deref(), Some("b1"));
        let second = page_slice(&p, "p1", first.next_after.as_deref(), 2).await;
        assert_eq!(ids(&second.blocks), ["c1", "sub"]);
        assert_eq!(second.next_after.as_deref(), Some("sub"));
        let last = page_slice(&p, "p1", second.next_after.as_deref(), 2).await;
        assert_eq!(ids(&last.blocks), ["b2", "b3"]);
        assert_eq!(last.next_after, None);

        let err = p
            .query(&encode_query(&PageQuery::GetPage {
                page_id: "p1".into(),
                after: Some("inside".into()),
                limit: 1,
            }))
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(message) if message == "invalid page cursor"));
        let reserved = p
            .query(&encode_query(&PageQuery::GetPage {
                page_id: "p1".into(),
                after: Some(PAGE_INDEX_KEY.into()),
                limit: 1,
            }))
            .await
            .unwrap_err();
        assert!(matches!(reserved, Error::Module(message) if message == "invalid page cursor"));
    });
}

#[test]
fn empty_nested_page_terminates_at_its_own_root() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        apply_commit(
            &mut p,
            &PageMsg::CreatePage {
                page_id: "outer".into(),
                title: "outer".into(),
            },
        )
        .await;
        apply_commit(
            &mut p,
            &PageMsg::InsertBlock {
                parent: "outer".into(),
                after: None,
                block: page("empty", "empty"),
            },
        )
        .await;

        let alone = page_slice(&p, "empty", None, 1).await;
        assert_eq!(ids(&alone.blocks), ["empty"]);
        assert_eq!(alone.next_after, None);

        apply_commit(
            &mut p,
            &PageMsg::InsertBlock {
                parent: "outer".into(),
                after: Some("empty".into()),
                block: para("outer-sibling", "outside"),
            },
        )
        .await;
        let with_sibling = page_slice(&p, "empty", None, 1).await;
        assert_eq!(ids(&with_sibling.blocks), ["empty"]);
        assert_eq!(with_sibling.next_after, None);
        let after_root = page_slice(&p, "empty", Some("empty"), 1).await;
        assert!(after_root.blocks.is_empty());
        assert_eq!(after_root.next_after, None);
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

        // UpdateText on the root IS the rename: the root block's text is the
        // live title (the page LIST rendering of it is `index::tests`').
        apply_commit(
            &mut p,
            &PageMsg::UpdateText {
                block_id: "p1".into(),
                text: "renamed".into(),
                marks: None,
            },
        )
        .await;
        let root = get_block(&p, "p1").await.unwrap();
        assert_eq!(root.kind, BlockKind::Page);
        assert_eq!(root.text, "renamed");
        assert_eq!(get_page(&p, "p1").await.unwrap()[0].text, "renamed");
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
        // Page membership is structural: SetKind cannot enter or leave it.
        apply_expect_err(
            &mut p,
            &PageMsg::SetKind {
                block_id: "b2".into(),
                kind: BlockKind::Page,
            },
            "page blocks",
        )
        .await;
        // Nor can it convert a Page block away from Page.
        apply_expect_err(
            &mut p,
            &PageMsg::SetKind {
                block_id: "p1".into(),
                kind: BlockKind::Paragraph,
            },
            "page blocks",
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
                parent: Some("p1".into()),
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
                parent: Some("p1".into()),
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
                parent: Some("p1".into()),
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
                parent: Some("b1".into()),
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
fn move_subtree_accepts_the_depth_boundary_and_rejects_overflow() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        apply_commit(
            &mut p,
            &PageMsg::CreatePage {
                page_id: "root".into(),
                title: "root".into(),
            },
        )
        .await;
        insert_chain(&mut p, "root", "spine-", MAX_PAGE_DEPTH - 1).await;
        apply_commit(
            &mut p,
            &PageMsg::InsertBlock {
                parent: "root".into(),
                after: None,
                block: para("branch", "branch"),
            },
        )
        .await;
        apply_commit(
            &mut p,
            &PageMsg::InsertBlock {
                parent: "branch".into(),
                after: None,
                block: para("leaf", "leaf"),
            },
        )
        .await;

        let boundary_parent = format!("spine-{}", MAX_PAGE_DEPTH - 2);
        apply_commit(
            &mut p,
            &PageMsg::MoveBlock {
                block_id: "branch".into(),
                parent: Some(boundary_parent.clone()),
                after: None,
            },
        )
        .await;
        assert_eq!(
            get_block(&p, "branch").await.unwrap().parent.as_deref(),
            Some(boundary_parent.as_str())
        );

        let overflow_parent = format!("spine-{}", MAX_PAGE_DEPTH - 1);
        apply_expect_err(
            &mut p,
            &PageMsg::MoveBlock {
                block_id: "branch".into(),
                parent: Some(overflow_parent),
                after: None,
            },
            "page nesting is too deep",
        )
        .await;
        assert_eq!(
            get_block(&p, "branch").await.unwrap().parent.as_deref(),
            Some(boundary_parent.as_str())
        );
        assert_eq!(
            get_page(&p, "root").await.unwrap().len(),
            MAX_PAGE_DEPTH + 2
        );
    });
}

#[test]
fn deepening_a_wide_subtree_rejects_before_the_wasm_read_ceiling() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        let children: Vec<_> = (0..=MAX_MOVE_SUBTREE_READS)
            .map(|index| format!("leaf-{index}"))
            .collect();
        p.store_block(&Block {
            id: "root".into(),
            parent: None,
            page: "root".into(),
            kind: BlockKind::Page,
            text: String::new(),
            marks: Vec::new(),
            checked: false,
            children: vec!["branch".into(), "target".into()],
        })
        .unwrap();
        p.store_block(&Block {
            id: "branch".into(),
            parent: Some("root".into()),
            page: "root".into(),
            kind: BlockKind::Paragraph,
            text: String::new(),
            marks: Vec::new(),
            checked: false,
            children: children.clone(),
        })
        .unwrap();
        p.store_block(&Block {
            id: "target".into(),
            parent: Some("root".into()),
            page: "root".into(),
            kind: BlockKind::Paragraph,
            text: String::new(),
            marks: Vec::new(),
            checked: false,
            children: Vec::new(),
        })
        .unwrap();
        for id in children {
            p.store_block(&Block {
                id,
                parent: Some("branch".into()),
                page: "root".into(),
                kind: BlockKind::Paragraph,
                text: String::new(),
                marks: Vec::new(),
                checked: false,
                children: Vec::new(),
            })
            .unwrap();
        }

        apply_expect_err(
            &mut p,
            &PageMsg::MoveBlock {
                block_id: "branch".into(),
                parent: Some("target".into()),
                after: None,
            },
            "subtree is too large to move deeper",
        )
        .await;
    });
}

#[test]
fn page_move_ancestry_stops_before_the_wasm_read_ceiling() {
    deterministic::Runner::default().start(|_context| async move {
        let mut boundary = Pages::new("boundary", Box::new(sdk_testkit::MemStore::new()));
        let boundary_parent = stage_page_ancestry(&mut boundary, MAX_TRAVERSAL_WORK);
        boundary
            .apply(
                PageMsg::MoveBlock {
                    block_id: "moving".into(),
                    parent: Some(boundary_parent.clone()),
                    after: None,
                },
                &Origin::System,
                0,
            )
            .await
            .unwrap();
        assert_eq!(
            boundary.load_block("moving").await.unwrap().unwrap().parent,
            Some(boundary_parent)
        );

        let mut over = Pages::new("over", Box::new(sdk_testkit::MemStore::new()));
        let over_parent = stage_page_ancestry(&mut over, MAX_TRAVERSAL_WORK + 1);
        let error = over
            .apply(
                PageMsg::MoveBlock {
                    block_id: "moving".into(),
                    parent: Some(over_parent.clone()),
                    after: None,
                },
                &Origin::System,
                0,
            )
            .await
            .unwrap_err();
        assert_eq!(error, PageError::MoveAncestryTooDeep);
        assert_eq!(
            over.load_block("moving").await.unwrap().unwrap().parent,
            None
        );
        assert!(
            over.load_block(&over_parent)
                .await
                .unwrap()
                .unwrap()
                .children
                .is_empty()
        );
    });
}

#[test]
fn subtree_removal_preflights_every_read_before_staging() {
    deterministic::Runner::default().start(|_context| async move {
        let boundary_children = 1_748;
        let mut boundary = Pages::new("boundary", Box::new(sdk_testkit::MemStore::new()));
        seed_wide_branch(&mut boundary, boundary_children).await;
        for index in 0..2 {
            apply_commit_as(
                &mut boundary,
                &PageMsg::AddComment {
                    thread_id: "boundary-thread".into(),
                    comment_id: format!("boundary-comment-{index}"),
                    target: "branch".into(),
                    text: "counts staged comment deletes".into(),
                    mentions: Vec::new(),
                    as_agent: None,
                    anchor: None,
                },
                user("alice"),
            )
            .await;
        }
        boundary
            .apply(
                PageMsg::RemoveBlock {
                    block_id: "branch".into(),
                },
                &Origin::System,
                0,
            )
            .await
            .unwrap();
        assert!(boundary.load_block("branch").await.unwrap().is_none());
        assert!(
            boundary
                .load_block("outer")
                .await
                .unwrap()
                .unwrap()
                .children
                .is_empty()
        );

        let mut over = Pages::new("over", Box::new(sdk_testkit::MemStore::new()));
        seed_wide_branch(&mut over, boundary_children).await;
        for index in 0..3 {
            apply_commit_as(
                &mut over,
                &PageMsg::AddComment {
                    thread_id: "over-thread".into(),
                    comment_id: format!("over-comment-{index}"),
                    target: "branch".into(),
                    text: "one staged delete beyond the work budget".into(),
                    mentions: Vec::new(),
                    as_agent: None,
                    anchor: None,
                },
                user("alice"),
            )
            .await;
        }
        let error = over
            .apply(
                PageMsg::RemoveBlock {
                    block_id: "branch".into(),
                },
                &Origin::System,
                0,
            )
            .await
            .unwrap_err();
        assert_eq!(error, PageError::RemoveSubtreeTooLarge);
        assert_eq!(
            over.load_block("outer").await.unwrap().unwrap().children,
            ["branch"]
        );
        assert!(over.load_block("branch").await.unwrap().is_some());
        assert!(
            over.load_block(&format!("leaf-{:04}", boundary_children - 1))
                .await
                .unwrap()
                .is_some()
        );
        assert!(query_comment(&over, "over-comment-2").await.is_some());
    });
}

#[test]
fn comment_work_cap_keeps_removal_reachable_against_a_stranger_flooding_threads() {
    // #1686: without an aggregate per-target cap, an unprivileged account
    // (mallory) can open enough threads on someone else's block to push
    // `preflight_subtree_removal`'s shared work budget over the top, and the
    // block's real author has no author-gated way to shed those threads
    // (DeleteComment/MoveCommentThread are stored-author/opener-gated). The
    // fix caps a target's AGGREGATE thread+comment work directly, so the
    // flood is refused long before it could ever exhaust the removal budget.
    deterministic::Runner::default().start(|_context| async move {
        let mut p = Pages::new("agg", Box::new(sdk_testkit::MemStore::new()));
        seed_wide_branch(&mut p, 1).await;
        // one thread (well under MAX_THREADS_PER_TARGET), flooded with
        // replies up to the aggregate cap: opening it costs 2 units (the
        // thread itself plus its first comment), every reply after costs 1.
        apply_commit_as(
            &mut p,
            &PageMsg::AddComment {
                thread_id: "mallory-thread".into(),
                comment_id: "mallory-comment-0".into(),
                target: "branch".into(),
                text: "grief".into(),
                anchor: None,
                mentions: Vec::new(),
                as_agent: None,
            },
            user("mallory"),
        )
        .await;
        for i in 0..MAX_COMMENT_WORK_PER_TARGET - 2 {
            apply_commit_as(
                &mut p,
                &PageMsg::AddComment {
                    thread_id: "mallory-thread".into(),
                    comment_id: format!("mallory-comment-{}", i + 1),
                    target: "branch".into(),
                    text: "grief".into(),
                    anchor: None,
                    mentions: Vec::new(),
                    as_agent: None,
                },
                user("mallory"),
            )
            .await;
        }
        // one more reply would push the aggregate past the cap.
        let error = p
            .apply(
                PageMsg::AddComment {
                    thread_id: "mallory-thread".into(),
                    comment_id: "mallory-comment-over".into(),
                    target: "branch".into(),
                    text: "grief".into(),
                    anchor: None,
                    mentions: Vec::new(),
                    as_agent: None,
                },
                &user("mallory"),
                0,
            )
            .await
            .unwrap_err();
        assert_eq!(error, PageError::TooMuchCommentWork);
        // the flood never actually threatened the removal budget: the block
        // (and its one real author-owned leaf) still removes cleanly.
        p.apply(
            PageMsg::RemoveBlock {
                block_id: "branch".into(),
            },
            &Origin::System,
            0,
        )
        .await
        .unwrap();
        assert!(p.load_block("branch").await.unwrap().is_none());
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
                parent: Some("c1".into()),
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
                parent: Some("p2".into()),
                after: None,
            },
            "cross-page",
        )
        .await;
        // Page blocks may become subpages under any content block.
        apply_commit(
            &mut p,
            &PageMsg::MoveBlock {
                block_id: "p2".into(),
                parent: Some("b1".into()),
                after: None,
            },
        )
        .await;
        assert_eq!(
            get_block(&p, "p2").await.unwrap().parent.as_deref(),
            Some("b1")
        );
        // a bad sibling anchor.
        apply_expect_err(
            &mut p,
            &PageMsg::MoveBlock {
                block_id: "b1".into(),
                parent: Some("p1".into()),
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

        // A root Page uses the same removal wire and drops its remaining tree.
        apply_commit(
            &mut p,
            &PageMsg::RemoveBlock {
                block_id: "p1".into(),
            },
        )
        .await;
        assert!(get_page(&p, "p1").await.is_none());
    });
}
