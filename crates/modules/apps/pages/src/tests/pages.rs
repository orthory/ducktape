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
                parent: None,
            },
        )
        .await;
        let page = get_page(&p, "p1").await.unwrap();
        assert_eq!(ids(&page), ["p1", "b1", "b2", "b3"]);
        assert_eq!(page[0].text, "renamed");
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
                parent: None,
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

// ── nested pages (folder relation in the index) ──
// the folder EDGES themselves are enumeration-index shape, rendered by the
// index tier now (see `index::tests`); here the write-path rules stay covered
// through kept surfaces — a folder edge is observable exactly where it binds:
// the cycle guard.

#[test]
fn create_under_missing_or_nonpage_parent_is_rejected() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        seed_page(&mut p, "p1").await; // p1 + blocks b1,b2,b3
        // parent does not exist
        apply_expect_err(
            &mut p,
            &PageMsg::CreatePage {
                page_id: "x".into(),
                title: "x".into(),
                parent: Some("ghost".into()),
            },
            "parent page not found",
        )
        .await;
        // parent exists but is a non-page block
        apply_expect_err(
            &mut p,
            &PageMsg::CreatePage {
                page_id: "y".into(),
                title: "y".into(),
                parent: Some("b1".into()),
            },
            "parent page not found",
        )
        .await;
    });
}

#[test]
fn set_page_parent_renests_and_rejects_cycles() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        for id in ["a", "b", "c"] {
            apply_commit(
                &mut p,
                &PageMsg::CreatePage {
                    page_id: id.into(),
                    title: id.into(),
                    parent: None,
                },
            )
            .await;
        }
        // b under a, c under b.
        apply_commit(
            &mut p,
            &PageMsg::SetPageParent {
                page_id: "b".into(),
                parent: Some("a".into()),
            },
        )
        .await;
        apply_commit(
            &mut p,
            &PageMsg::SetPageParent {
                page_id: "c".into(),
                parent: Some("b".into()),
            },
        )
        .await;
        // a under c would cycle (a -> c -> b -> a) — the rejection is ALSO the
        // proof both renests landed: the guard walks the live folder edges.
        apply_expect_err(
            &mut p,
            &PageMsg::SetPageParent {
                page_id: "a".into(),
                parent: Some("c".into()),
            },
            "page cycle",
        )
        .await;
        // self-parent cycles too.
        apply_expect_err(
            &mut p,
            &PageMsg::SetPageParent {
                page_id: "a".into(),
                parent: Some("a".into()),
            },
            "page cycle",
        )
        .await;
        // detach b to top level — the just-rejected renest becomes legal
        // (c's ancestry is now c -> b -> top), proving the detach landed.
        apply_commit(
            &mut p,
            &PageMsg::SetPageParent {
                page_id: "b".into(),
                parent: None,
            },
        )
        .await;
        apply_commit(
            &mut p,
            &PageMsg::SetPageParent {
                page_id: "a".into(),
                parent: Some("c".into()),
            },
        )
        .await;
        // target must be a page root.
        seed_page(&mut p, "pg").await;
        apply_expect_err(
            &mut p,
            &PageMsg::SetPageParent {
                page_id: "b1".into(),
                parent: None,
            },
            "not a page",
        )
        .await;
        // parent must be a page.
        apply_expect_err(
            &mut p,
            &PageMsg::SetPageParent {
                page_id: "a".into(),
                parent: Some("b1".into()),
            },
            "parent page not found",
        )
        .await;
    });
}

#[test]
fn delete_page_removes_subtree_and_promotes_children() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        // grand -> parent -> child ; parent also has a content block pb1.
        apply_commit(
            &mut p,
            &PageMsg::CreatePage {
                page_id: "grand".into(),
                title: "G".into(),
                parent: None,
            },
        )
        .await;
        apply_commit(
            &mut p,
            &PageMsg::CreatePage {
                page_id: "parent".into(),
                title: "P".into(),
                parent: Some("grand".into()),
            },
        )
        .await;
        apply_commit(
            &mut p,
            &PageMsg::CreatePage {
                page_id: "child".into(),
                title: "C".into(),
                parent: Some("parent".into()),
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
            &PageMsg::DeletePage {
                page_id: "parent".into(),
            },
        )
        .await;

        // parent's root + content block are gone …
        assert!(get_block(&p, "parent").await.is_none());
        assert!(get_block(&p, "pb1").await.is_none());
        assert!(get_page(&p, "parent").await.is_none());
        // … child was NOT deleted (its own root survives) …
        assert!(get_page(&p, "child").await.is_some());
        // … and child was PROMOTED to grand: nesting grand under child must
        // now cycle (grand -> child -> grand). had promotion detached child
        // to top level — or left it dangling on the deleted parent — this op
        // would succeed. (list rendering of the edge is `index::tests`'.)
        apply_expect_err(
            &mut p,
            &PageMsg::SetPageParent {
                page_id: "grand".into(),
                parent: Some("child".into()),
            },
            "page cycle",
        )
        .await;

        // deleting a non-page id is rejected.
        seed_page(&mut p, "pg").await;
        apply_expect_err(
            &mut p,
            &PageMsg::DeletePage {
                page_id: "b1".into(),
            },
            "not a page",
        )
        .await;
    });
}
