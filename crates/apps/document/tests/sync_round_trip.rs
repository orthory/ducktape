//! state-sync round-trip: a fresh `Document` reconstructs a byte-identical qmdb
//! root by pulling a source store's operation range through commonware's qmdb
//! sync — the smallest proof that module snapshot/install rebuilds the
//! authenticated root WITHOUT replaying ops in application order.
//!
//! the source UPDATES a block (`draft` then `final`), so the doc's qmdb key is
//! OVERWRITTEN and the committed op log carries a history that a naive "export
//! current docs and re-apply sorted" could never reproduce — the qmdb root is
//! operation-log ordered, not a canonical merkle over the live doc set. only a
//! real sync that ships the ACTUAL proven op range lands on the same root, which
//! is precisely what makes this test discriminating rather than a tautology.

use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use document::Document;
use document_interface::{
    Block, BlockKind, DocMsg, DocQuery, DocReply, decode_reply, encode_msg, encode_query,
};
use sdk::{Ctx, Error, Module, Msg, StateRoot};

// a minimal Ctx so execute can be driven without a full host.
struct TestCtx {
    env: sdk::Env,
}
impl TestCtx {
    fn new() -> Self {
        Self {
            env: sdk::Env {
                height: 0,
                consensus_time: 0,
                origin: sdk::Origin::System,
                me: "document".into(),
            },
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
    fn emit_msg(&mut self, _m: Msg) {}
    fn emit_event(&mut self, _e: sdk::Event) {}
    fn request_effect(&mut self, _e: sdk::Effect) {}
}

fn blk(id: &str, text: &str) -> Block {
    Block {
        id: id.into(),
        kind: BlockKind::Paragraph,
        text: text.into(),
    }
}

// drive one op through the REAL module path: execute + commit_block (one op per
// block), so the committed op log is exactly what a validator would produce.
async fn apply_commit<E>(d: &mut Document<E>, m: &DocMsg)
where
    E: commonware_storage::Context + commonware_runtime::BufferPooler,
{
    let msg = Msg {
        target: "document".into(),
        payload: encode_msg(m),
    };
    d.execute(&mut TestCtx::new(), &msg).await.unwrap();
    d.commit_block().await.unwrap();
}

async fn get_doc<E>(d: &Document<E>, doc_id: &str) -> Option<Vec<Block>>
where
    E: commonware_storage::Context + commonware_runtime::BufferPooler,
{
    let reply = d
        .query(&encode_query(&DocQuery::GetDoc {
            doc_id: doc_id.into(),
        }))
        .await
        .unwrap();
    match decode_reply(&reply).unwrap() {
        DocReply::Doc(v) => v,
        _ => panic!("expected Doc"),
    }
}

#[test]
fn synced_store_reconstructs_source_root() {
    deterministic::Runner::default().start(|context| async move {
        // SOURCE: build a doc through the real op path, including an UPDATE that
        // overwrites the doc's key in the op log.
        let mut src = Document::init(context.child("src"), "src").await;
        apply_commit(
            &mut src,
            &DocMsg::CreateDoc {
                doc_id: "doc1".into(),
            },
        )
        .await;
        apply_commit(
            &mut src,
            &DocMsg::InsertBlock {
                doc_id: "doc1".into(),
                after: None,
                block: blk("b1", "draft"),
            },
        )
        .await;
        apply_commit(
            &mut src,
            &DocMsg::InsertBlock {
                doc_id: "doc1".into(),
                after: Some("b1".into()),
                block: blk("b2", "second"),
            },
        )
        .await;
        apply_commit(
            &mut src,
            &DocMsg::UpdateBlock {
                doc_id: "doc1".into(),
                block_id: "b1".into(),
                text: "final".into(),
            },
        )
        .await; // overwrite: op-log order matters
        let src_root: StateRoot = src.root();
        assert_ne!(src_root, StateRoot::ZERO, "source must have a real root");

        // describe the target (root + op range), THEN hand the source off as the
        // sync resolver (consumes it — order matters).
        let target = src.sync_target().await;
        let resolver = src.into_resolver();

        // JOINER: reconstruct on a FRESH context + namespace by pulling from the
        // resolver. no ops are applied in application order on this side.
        let synced = Document::sync_from(context.child("dst"), "dst", target, resolver).await;

        // THE PROPERTY: identical qmdb root — the app-hash linkage a joiner needs
        // to be accepted as a consensus participant at the boundary height.
        assert_eq!(
            synced.root(),
            src_root,
            "synced store root must equal the source root"
        );

        // and the live doc view is correct: b1 updated to "final", order intact.
        let doc = get_doc(&synced, "doc1").await.unwrap();
        let ids: Vec<&str> = doc.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(ids, ["b1", "b2"]);
        assert_eq!(doc[0].text, "final");
    });
}
