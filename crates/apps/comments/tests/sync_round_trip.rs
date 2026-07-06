//! state-sync round-trip: a fresh `Comments` reconstructs a byte-identical qmdb
//! root by pulling a source store's op range. the source ADDS, EDITS, and
//! DELETES comments so the op log carries overwrites AND deletes — only a real
//! sync of the proven op range lands on the same root.

use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use comments::{
    Anchor, CommentMsg, CommentQuery, CommentReply, Comments, ThreadView, decode_reply, encode_msg,
    encode_query,
};
use sdk::{Ctx, Env, Error, Module, Msg, Origin, StateRoot};

struct TestCtx {
    env: Env,
}
impl TestCtx {
    fn new() -> Self {
        Self {
            env: Env {
                protocol_version: 0,
                height: 0,
                consensus_time: 3,
                origin: Origin::External(b"u".to_vec()),
                me: "comments".into(),
            },
        }
    }
}
#[async_trait::async_trait(?Send)]
impl Ctx for TestCtx {
    fn env(&self) -> &Env {
        &self.env
    }
    fn module_root(&self, _t: &str) -> Option<StateRoot> {
        None
    }
    async fn query(&self, _t: &str, _r: &[u8]) -> Result<Vec<u8>, Error> {
        Err(Error::QueryUnsupported)
    }
    fn emit_msg(&mut self, _m: Msg) {}
    fn emit_event(&mut self, _e: sdk::Event) {}
    fn request_effect(&mut self, _e: sdk::Effect) {}
}
async fn apply_commit<E>(c: &mut Comments<E>, m: &CommentMsg)
where
    E: commonware_storage::Context + commonware_runtime::BufferPooler,
{
    let msg = Msg {
        target: "comments".into(),
        payload: encode_msg(m),
    };
    c.execute(&mut TestCtx::new(), &msg).await.unwrap();
    c.commit_block().await.unwrap();
}
async fn thread_of<E>(c: &Comments<E>, id: &str) -> Option<ThreadView>
where
    E: commonware_storage::Context + commonware_runtime::BufferPooler,
{
    match decode_reply(
        &c.query(&encode_query(&CommentQuery::Thread { thread_id: id.into() }))
            .await
            .unwrap(),
    )
    .unwrap()
    {
        CommentReply::Thread(v) => v,
        _ => panic!("expected Thread"),
    }
}
fn anchor(t: &str) -> Anchor {
    Anchor { module: "pages".into(), target: t.into() }
}

#[test]
fn synced_store_reconstructs_source_root() {
    deterministic::Runner::default().start(|context| async move {
        let mut src = Comments::init(context.child("src"), "src").await;
        apply_commit(&mut src, &CommentMsg::AddComment { thread_id: "t1".into(), comment_id: "m1".into(), anchor: anchor("b1"), text: "draft".into() }).await;
        apply_commit(&mut src, &CommentMsg::AddComment { thread_id: "t1".into(), comment_id: "m2".into(), anchor: anchor("b1"), text: "doomed".into() }).await;
        apply_commit(&mut src, &CommentMsg::EditComment { comment_id: "m1".into(), text: "final".into() }).await; // overwrite
        apply_commit(&mut src, &CommentMsg::DeleteComment { comment_id: "m2".into() }).await; // delete rides the log
        let src_root = src.root();
        assert_ne!(src_root, StateRoot::ZERO);

        let target = src.sync_target().await;
        let resolver = src.into_resolver();
        let synced = Comments::sync_from(context.child("dst"), "dst", target, resolver).await.expect("sync_from");

        assert_eq!(synced.root(), src_root, "synced root must equal source root");
        let v = thread_of(&synced, "t1").await.unwrap();
        assert_eq!(v.comments.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(), ["final"]);
    });
}
