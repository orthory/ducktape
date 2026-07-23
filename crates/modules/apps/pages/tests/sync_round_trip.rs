//! state-sync round-trip: a joiner reconstructs a byte-identical qmdb root by
//! pulling a source store's operation range through commonware's qmdb sync,
//! then wraps a fresh `Pages` around the injected store — the same
//! discriminating property the kv module proves, over the per-block-per-key
//! layout.
//!
//! the source UPDATES a block's text and REMOVES a subtree, so the op log
//! carries overwrites AND deletes that a naive "export live blocks and
//! re-apply sorted" could never reproduce — the qmdb root is operation-log
//! ordered. only a real sync that ships the ACTUAL proven op range lands on
//! the same root.
//!
//! pages' tree surgery is too rich to mirror with raw store batches (the kv
//! test's source shape), so the source drives ops through a REAL `Pages` —
//! then REOPENS the committed partitions as a bare `QmdbStore` for the
//! resolver handoff: a `Pages` consumes its injected store, so the
//! handoff-as-resolver form is only reachable on the raw store, and reopening
//! under the same id is exactly the recovery path a restarting node takes
//! (the deterministic runtime shares storage across child contexts).

use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use pages::Pages;
use pages::{
    Block, BlockKind, NewBlock, PageMeta, PageMsg, PageQuery, PageReply, decode_reply, encode_msg,
    encode_query,
};
use sdk::{MerkleStore as _, Module, Msg, StateRoot};
use sdk_testkit::TestCtx;
use statesync::qmdb::QmdbStore;

fn para(id: &str, text: &str) -> NewBlock {
    NewBlock {
        id: id.into(),
        kind: BlockKind::Paragraph,
        text: text.into(),
        marks: Vec::new(),
    }
}

// drive one op through the REAL module path: execute + commit_block (one op
// per block-height), so the committed op log is what a validator produces.
async fn apply_commit(p: &mut Pages, m: &PageMsg) {
    let msg = Msg {
        target: "pages".into(),
        payload: encode_msg(m),
    };
    p.execute(&mut TestCtx::at_height(0), &msg).await.unwrap();
    p.commit_block().await.unwrap();
}

async fn get_page(p: &Pages, page_id: &str) -> Option<Vec<Block>> {
    let reply = p
        .query(&encode_query(&PageQuery::GetPage {
            page_id: page_id.into(),
        }))
        .await
        .unwrap();
    match decode_reply(&reply).unwrap() {
        PageReply::Page(v) => v,
        _ => panic!("expected Page"),
    }
}

async fn list_pages(p: &Pages) -> Vec<PageMeta> {
    let reply = p.query(&encode_query(&PageQuery::ListPages)).await.unwrap();
    match decode_reply(&reply).unwrap() {
        PageReply::PageList(l) => l,
        _ => panic!("expected PageList"),
    }
}

#[test]
fn synced_store_reconstructs_source_root() {
    deterministic::Runner::default().start(|context| async move {
        // SOURCE: build a page through the real op path, including an UPDATE
        // (key overwrite) and a subtree REMOVE (key delete) in the op log.
        // built the way a host does: concrete store first, injected as a box.
        let mut src = Pages::new(
            "src",
            Box::new(QmdbStore::init(context.child("src"), "src").await),
        );
        apply_commit(
            &mut src,
            &PageMsg::CreatePage {
                page_id: "p1".into(),
                title: "one".into(),
            },
        )
        .await;
        apply_commit(
            &mut src,
            &PageMsg::InsertBlock {
                parent: "p1".into(),
                after: None,
                block: para("b1", "draft"),
            },
        )
        .await;
        apply_commit(
            &mut src,
            &PageMsg::InsertBlock {
                parent: "b1".into(),
                after: None,
                block: para("c1", "doomed"),
            },
        )
        .await;
        apply_commit(
            &mut src,
            &PageMsg::UpdateText {
                block_id: "b1".into(),
                text: "final".into(),
                marks: None,
            },
        )
        .await; // overwrite: op-log order matters
        apply_commit(
            &mut src,
            &PageMsg::RemoveBlock {
                block_id: "c1".into(),
            },
        )
        .await; // delete rides the log too
        // a comment rides the SAME store (reserved keys) — it must sync too.
        apply_commit(
            &mut src,
            &PageMsg::AddComment {
                thread_id: "th1".into(),
                comment_id: "cm1".into(),
                target: "b1".into(),
                text: "review this".into(),
                anchor: None,
                mentions: Vec::new(),
                as_agent: None,
            },
        )
        .await;
        let src_root: StateRoot = src.root();
        assert_ne!(src_root, StateRoot::ZERO, "source must have a real root");

        // the module consumed its store, so REOPEN the committed partitions
        // as a bare store for the handoff (drop first — one owner at a time).
        drop(src);
        let src_store = QmdbStore::init(context.child("src_serve"), "src").await;
        assert_eq!(
            src_store.root(),
            src_root,
            "reopened store must recover the committed root"
        );

        // describe the target (root + op range), THEN hand the source off as
        // the sync resolver (consumes it — order matters).
        let target = src_store.sync_boundary_target().await;
        let resolver = src_store.into_resolver();

        // JOINER: reconstruct on a FRESH context + namespace by pulling from
        // the resolver, then wrap the module around the injected store — the
        // exact shape a joining host uses. no ops are applied in application
        // order on this side.
        let store = QmdbStore::sync_from(context.child("dst"), "dst", target, resolver)
            .await
            .expect("sync_from");
        let synced = Pages::new("dst", Box::new(store));

        // THE PROPERTY: identical qmdb root — the app-hash linkage a joiner
        // needs at the boundary height.
        assert_eq!(
            synced.root(),
            src_root,
            "synced store root must equal the source root"
        );

        // and the live page view is correct: text updated, subtree gone.
        let page = get_page(&synced, "p1").await.unwrap();
        let ids: Vec<&str> = page.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(ids, ["p1", "b1"]);
        assert_eq!(page[1].text, "final");

        // the comment survived the sync too.
        let page = match decode_reply(
            &synced
                .query(&encode_query(&PageQuery::CommentsForThread {
                    thread_id: "th1".into(),
                    from: 0,
                    limit: 1,
                }))
                .await
                .unwrap(),
        )
        .unwrap()
        {
            PageReply::CommentPage(v) => v,
            other => panic!("expected CommentPage, got {other:?}"),
        };
        let page = page.expect("thread present after sync");
        assert_eq!(page.comments.len(), 1);
        assert_eq!(page.comments[0].comment.text, "review this");
    });
}

// the enumeration INDEX is ordinary store state (a reserved sentinel key), so
// it state-syncs like any block: a joiner that rebuilds a byte-identical root
// reproduces the exact page set, titles included.
#[test]
fn synced_store_reproduces_the_page_index() {
    deterministic::Runner::default().start(|context| async move {
        let mut src = Pages::new(
            "src",
            Box::new(QmdbStore::init(context.child("src"), "src").await),
        );
        for (id, title) in [("zebra", "Z"), ("alpha", "A")] {
            apply_commit(
                &mut src,
                &PageMsg::CreatePage {
                    page_id: id.into(),
                    title: title.into(),
                },
            )
            .await;
        }
        // a block edit on one page: proves edits don't disturb the index.
        apply_commit(
            &mut src,
            &PageMsg::InsertBlock {
                parent: "alpha".into(),
                after: None,
                block: para("b1", "hello"),
            },
        )
        .await;
        let src_pages = list_pages(&src).await;
        assert_eq!(src_pages.len(), 2);
        assert_eq!(src_pages[0].id, "alpha");
        let src_root = src.root();

        // reopen-as-raw handoff (see the header doc): target, then resolver.
        drop(src);
        let src_store = QmdbStore::init(context.child("src_serve"), "src").await;
        assert_eq!(src_store.root(), src_root, "reopened root must match");
        let target = src_store.sync_boundary_target().await;
        let resolver = src_store.into_resolver();
        let store = QmdbStore::sync_from(context.child("dst"), "dst", target, resolver)
            .await
            .expect("sync_from");
        let synced = Pages::new("dst", Box::new(store));

        assert_eq!(
            list_pages(&synced).await,
            src_pages,
            "a synced store must reproduce the source's page index"
        );
        let page = get_page(&synced, "alpha").await.unwrap();
        assert_eq!(page.len(), 2);
        assert_eq!(page[1].id, "b1");
    });
}
