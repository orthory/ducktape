use super::*;
use crate::{NewBlock, decode_reply, encode_msg, encode_query};
use commonware_runtime::{Runner as _, deterministic};
use host::global_root;

fn nb(id: &str, kind: BlockKind, text: &str) -> NewBlock {
    NewBlock {
        id: id.into(),
        kind,
        text: text.into(),
    }
}

fn para(id: &str, text: &str) -> NewBlock {
    nb(id, BlockKind::Paragraph, text)
}

fn msg(m: &PageMsg) -> Msg {
    Msg {
        target: "pages".into(),
        payload: encode_msg(m),
    }
}

// a minimal Ctx so execute can be driven without a full host.
struct TestCtx {
    env: sdk::Env,
    msgs: Vec<Msg>,
}
impl TestCtx {
    fn new() -> Self {
        Self {
            env: sdk::Env {
                protocol_version: 0,
                height: 0,
                consensus_time: 0,
                origin: sdk::Origin::System,
                me: "pages".into(),
            },
            msgs: Vec::new(),
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
        self.msgs.push(m);
    }
    fn emit_event(&mut self, _e: sdk::Event) {}
    fn request_effect(&mut self, _e: sdk::Effect) {}
}

// drive one op through execute + commit_block (one op per block-height).
async fn apply_commit<E: Context + BufferPooler>(p: &mut Pages<E>, m: &PageMsg) {
    p.execute(&mut TestCtx::new(), &msg(m)).await.unwrap();
    p.commit_block().await.unwrap();
}

// an op that must FAIL, followed by the host's abort.
async fn apply_expect_err<E: Context + BufferPooler>(p: &mut Pages<E>, m: &PageMsg, needle: &str) {
    let err = p
        .execute(&mut TestCtx::new(), &msg(m))
        .await
        .expect_err("op must be rejected");
    assert!(
        matches!(err, Error::Module(ref s) if s.contains(needle)),
        "unexpected error: {err:?}"
    );
    p.abort_block().await.unwrap();
}

async fn get_page<E: Context + BufferPooler>(p: &Pages<E>, page_id: &str) -> Option<Vec<Block>> {
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

async fn get_block<E: Context + BufferPooler>(p: &Pages<E>, block_id: &str) -> Option<Block> {
    let reply = p
        .query(&encode_query(&PageQuery::GetBlock {
            block_id: block_id.into(),
        }))
        .await
        .unwrap();
    match decode_reply(&reply).unwrap() {
        PageReply::Block(b) => b,
        _ => panic!("expected Block"),
    }
}

async fn list_pages<E: Context + BufferPooler>(p: &Pages<E>) -> Vec<PageMeta> {
    let reply = p.query(&encode_query(&PageQuery::ListPages)).await.unwrap();
    match decode_reply(&reply).unwrap() {
        PageReply::PageList(l) => l,
        _ => panic!("expected PageList"),
    }
}

fn ids(blocks: &[Block]) -> Vec<&str> {
    blocks.iter().map(|b| b.id.as_str()).collect()
}

// ── comment test helpers (author-carrying origins) ──

fn user(name: &str) -> sdk::Origin {
    sdk::Origin::External(name.as_bytes().to_vec())
}
fn ctx_as(origin: sdk::Origin) -> TestCtx {
    TestCtx {
        env: sdk::Env {
            protocol_version: 0,
            height: 0,
            consensus_time: 7,
            origin,
            me: "pages".into(),
        },
        msgs: Vec::new(),
    }
}
async fn apply_commit_as<E: Context + BufferPooler>(
    p: &mut Pages<E>,
    m: &PageMsg,
    origin: sdk::Origin,
) {
    p.execute(&mut ctx_as(origin), &msg(m)).await.unwrap();
    p.commit_block().await.unwrap();
}
async fn apply_err_as<E: Context + BufferPooler>(
    p: &mut Pages<E>,
    m: &PageMsg,
    origin: sdk::Origin,
    needle: &str,
) {
    let err = p
        .execute(&mut ctx_as(origin), &msg(m))
        .await
        .expect_err("op must be rejected");
    assert!(
        matches!(err, Error::Module(ref s) if s.contains(needle)),
        "unexpected error: {err:?}"
    );
    p.abort_block().await.unwrap();
}
async fn query_threads<E: Context + BufferPooler>(
    p: &Pages<E>,
    targets: &[&str],
) -> Vec<TargetThreads> {
    let q = PageQuery::ThreadsForTargets {
        targets: targets.iter().map(|s| s.to_string()).collect(),
    };
    match decode_reply(&p.query(&encode_query(&q)).await.unwrap()).unwrap() {
        PageReply::CommentThreads(v) => v,
        _ => panic!("expected CommentThreads"),
    }
}
async fn query_thread<E: Context + BufferPooler>(
    p: &Pages<E>,
    thread_id: &str,
) -> Option<ThreadView> {
    let q = PageQuery::CommentThread {
        thread_id: thread_id.into(),
    };
    match decode_reply(&p.query(&encode_query(&q)).await.unwrap()).unwrap() {
        PageReply::CommentThread(v) => v,
        _ => panic!("expected CommentThread"),
    }
}
async fn query_comment<E: Context + BufferPooler>(
    p: &Pages<E>,
    comment_id: &str,
) -> Option<Comment> {
    let q = PageQuery::GetComment {
        comment_id: comment_id.into(),
    };
    match decode_reply(&p.query(&encode_query(&q)).await.unwrap()).unwrap() {
        PageReply::Comment(c) => c,
        _ => panic!("expected Comment"),
    }
}

// seed one page with three top-level paragraphs b1, b2, b3.
async fn seed_page<E: Context + BufferPooler>(p: &mut Pages<E>, page: &str) {
    apply_commit(
        p,
        &PageMsg::CreatePage {
            page_id: page.into(),
            title: format!("{page} title"),
            parent: None,
        },
    )
    .await;
    let mut after = None;
    for id in ["b1", "b2", "b3"] {
        apply_commit(
            p,
            &PageMsg::InsertBlock {
                parent: page.into(),
                after,
                block: para(id, id),
            },
        )
        .await;
        after = Some(id.to_string());
    }
}

mod block_tree;
mod comments;
mod pages;
mod storage;
