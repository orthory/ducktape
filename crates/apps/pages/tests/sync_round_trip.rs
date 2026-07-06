//! state-sync round-trip: a fresh `Pages` reconstructs a byte-identical qmdb
//! root by pulling a source store's operation range through commonware's qmdb
//! sync — the same discriminating property the document module proves, over
//! the per-block-per-key layout.
//!
//! the source UPDATES a block's text and REMOVES a subtree, so the op log
//! carries overwrites AND deletes that a naive "export live blocks and
//! re-apply sorted" could never reproduce — the qmdb root is operation-log
//! ordered. only a real sync that ships the ACTUAL proven op range lands on
//! the same root.

use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use pages::Pages;
use pages::{
    Block, BlockKind, NewBlock, PageEvent, PageMeta, PageMsg, PageQuery, PageReply, ThreadView,
    decode_page_event, decode_reply, encode_msg, encode_query,
};
use sdk::{Ctx, Error, Module, Msg, Origin, StateRoot};

// a minimal Ctx so execute can be driven without a full host.
struct TestCtx {
    env: sdk::Env,
    /// follow-up msgs emitted during execute, in order (hook fan-out).
    emitted: Vec<Msg>,
}
impl TestCtx {
    fn with_origin(origin: Origin) -> Self {
        Self {
            env: sdk::Env {
                protocol_version: 0,
                height: 0,
                consensus_time: 0,
                origin,
                me: "pages".into(),
            },
            emitted: Vec::new(),
        }
    }
}
#[async_trait::async_trait(?Send)]
impl Ctx for TestCtx {
    fn env(&self) -> &sdk::Env {
        &self.env
    }
    fn module_root(&self, _t: &str) -> Option<StateRoot> {
        None
    }
    async fn query(&self, _t: &str, _r: &[u8]) -> Result<Vec<u8>, Error> {
        Err(Error::QueryUnsupported)
    }
    fn emit_msg(&mut self, m: Msg) {
        self.emitted.push(m);
    }
    fn emit_event(&mut self, _e: sdk::Event) {}
    fn request_effect(&mut self, _e: sdk::Effect) {}
}

fn para(id: &str, text: &str) -> NewBlock {
    NewBlock {
        id: id.into(),
        kind: BlockKind::Paragraph,
        text: text.into(),
    }
}

// drive one op through the REAL module path: execute + commit_block (one op
// per block-height), so the committed op log is what a validator produces.
async fn apply_commit<E>(p: &mut Pages<E>, m: &PageMsg)
where
    E: commonware_storage::Context + commonware_runtime::BufferPooler,
{
    apply_commit_as(p, m, Origin::System).await;
}

// same, from an explicit origin (hook registration is module-origin only).
async fn apply_commit_as<E>(p: &mut Pages<E>, m: &PageMsg, origin: Origin)
where
    E: commonware_storage::Context + commonware_runtime::BufferPooler,
{
    let msg = Msg {
        target: "pages".into(),
        payload: encode_msg(m),
    };
    p.execute(&mut TestCtx::with_origin(origin), &msg)
        .await
        .unwrap();
    p.commit_block().await.unwrap();
}

async fn get_page<E>(p: &Pages<E>, page_id: &str) -> Option<Vec<Block>>
where
    E: commonware_storage::Context + commonware_runtime::BufferPooler,
{
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

async fn list_pages<E>(p: &Pages<E>) -> Vec<PageMeta>
where
    E: commonware_storage::Context + commonware_runtime::BufferPooler,
{
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
        let mut src = Pages::init(context.child("src"), "src").await;
        apply_commit(
            &mut src,
            &PageMsg::CreatePage {
                page_id: "p1".into(),
                title: "one".into(),
                parent: None,
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
        // a comment rides the SAME qmdb (reserved keys) — it must sync too.
        apply_commit(
            &mut src,
            &PageMsg::AddComment {
                thread_id: "th1".into(),
                comment_id: "cm1".into(),
                target: "b1".into(),
                text: "review this".into(),
            },
        )
        .await;
        // a registered hook rides the reserved `\0hooks` key: it must FOLD
        // INTO the root (assert movement) and survive the sync below.
        let unhooked = src.root();
        apply_commit_as(
            &mut src,
            &PageMsg::RegisterHook {},
            Origin::Module("docs-harness".into()),
        )
        .await;
        assert_ne!(src.root(), unhooked, "the hook set must fold into the root");
        let src_root: StateRoot = src.root();
        assert_ne!(src_root, StateRoot::ZERO, "source must have a real root");

        // describe the target (root + op range), THEN hand the source off as
        // the sync resolver (consumes it — order matters).
        let target = src.sync_target().await;
        let resolver = src.into_resolver();

        // JOINER: reconstruct on a FRESH context + namespace by pulling from
        // the resolver. no ops are applied in application order on this side.
        let synced = Pages::sync_from(context.child("dst"), "dst", target, resolver)
            .await
            .expect("sync_from");

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
        let view = match decode_reply(
            &synced
                .query(&encode_query(&PageQuery::CommentThread {
                    thread_id: "th1".into(),
                }))
                .await
                .unwrap(),
        )
        .unwrap()
        {
            PageReply::CommentThread(v) => v,
            other => panic!("expected CommentThread, got {other:?}"),
        };
        let view: ThreadView = view.expect("thread present after sync");
        assert_eq!(view.comments.len(), 1);
        assert_eq!(view.comments[0].text, "review this");

        // the hook set survived the sync: a comment on the SYNCED store fans
        // out to the subscriber registered on the source.
        let mut synced = synced;
        let mut ctx = TestCtx::with_origin(Origin::External(b"alice".to_vec()));
        synced
            .execute(
                &mut ctx,
                &Msg {
                    target: "pages".into(),
                    payload: encode_msg(&PageMsg::AddComment {
                        thread_id: "th2".into(),
                        comment_id: "cm2".into(),
                        target: "b1".into(),
                        text: "post-sync".into(),
                    }),
                },
            )
            .await
            .unwrap();
        synced.commit_block().await.unwrap();
        assert_eq!(ctx.emitted.len(), 1, "synced hook set must fan out");
        assert_eq!(ctx.emitted[0].target, "docs-harness");
        assert_eq!(
            decode_page_event(&ctx.emitted[0].payload).unwrap(),
            PageEvent::CommentAdded {
                page_id: "p1".into(),
                target: "b1".into(),
                thread_id: "th2".into(),
                comment_id: "cm2".into(),
                author: pages::AuthorRef::User(b"alice".to_vec()),
                text: "post-sync".into(),
            }
        );
    });
}

// the enumeration INDEX is ordinary qmdb state (a reserved sentinel key), so
// it state-syncs like any block: a joiner that rebuilds a byte-identical root
// reproduces the exact page set, titles included.
#[test]
fn synced_store_reproduces_the_page_index() {
    deterministic::Runner::default().start(|context| async move {
        let mut src = Pages::init(context.child("src"), "src").await;
        for (id, title) in [("zebra", "Z"), ("alpha", "A")] {
            apply_commit(
                &mut src,
                &PageMsg::CreatePage {
                    page_id: id.into(),
                    title: title.into(),
                    parent: None,
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

        let target = src.sync_target().await;
        let resolver = src.into_resolver();
        let synced = Pages::sync_from(context.child("dst"), "dst", target, resolver)
            .await
            .expect("sync_from");

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
