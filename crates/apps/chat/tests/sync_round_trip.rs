//! state-sync round-trip: a fresh `Chat` reconstructs a byte-identical storage
//! root by pulling a source store's operation range through commonware's qmdb
//! sync. this is the storage-backed joiner surface; replaying exported records
//! in sorted order is not enough because the qmdb root commits to the live
//! operation log. the source content covers every record family: channel +
//! index, message heads, a thread index, an edit revision, reaction sets, and
//! membership records.

use chat::Chat;
use chat_interface::{Block, ChatMsg, ChatQuery, PostPolicy, encode_msg, encode_query};
use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use sdk::{Ctx, Error, Module, Msg, Origin, StateRoot};

struct TestCtx {
    env: sdk::Env,
}

impl TestCtx {
    fn at(consensus_time: u64) -> Self {
        Self {
            env: sdk::Env { protocol_version: 0,
                height: 0,
                consensus_time,
                origin: Origin::System,
                me: "chat".into(),
            },
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Ctx for TestCtx {
    fn env(&self) -> &sdk::Env {
        &self.env
    }

    fn module_root(&self, _target: &str) -> Option<StateRoot> {
        None
    }

    async fn query(&self, _target: &str, _req: &[u8]) -> Result<Vec<u8>, Error> {
        Err(Error::QueryUnsupported)
    }

    fn emit_msg(&mut self, _msg: Msg) {}
    fn emit_event(&mut self, _ev: sdk::Event) {}
    fn request_effect(&mut self, _eff: sdk::Effect) {}
}

fn module_msg(payload: ChatMsg) -> Msg {
    Msg {
        target: "chat".into(),
        payload: encode_msg(&payload),
    }
}

async fn apply_commit<E>(module: &mut Chat<E>, at: u64, payload: ChatMsg)
where
    E: commonware_storage::Context + commonware_runtime::BufferPooler,
{
    module
        .execute(&mut TestCtx::at(at), &module_msg(payload))
        .await
        .unwrap();
    module.commit_block().await.unwrap();
}

async fn reply_bytes<E>(module: &Chat<E>, query: ChatQuery) -> Vec<u8>
where
    E: commonware_storage::Context + commonware_runtime::BufferPooler,
{
    module.query(&encode_query(&query)).await.unwrap()
}

#[test]
fn synced_store_reconstructs_source_root_and_history() {
    deterministic::Runner::default().start(|context| async move {
        let mut src = Chat::init(context.child("src"), "src").await;
        apply_commit(
            &mut src,
            10,
            ChatMsg::CreateChannel {
                channel_id: "general".into(),
                name: "General".into(),
                post_policy: PostPolicy::Open,
            },
        )
        .await;
        apply_commit(
            &mut src,
            20,
            ChatMsg::PostMessage {
                channel_id: "general".into(),
                message_id: "m1".into(),
                blocks: vec![Block::paragraph("draft")],
                thread: None,
                as_agent: None,
            },
        )
        .await;
        apply_commit(
            &mut src,
            21,
            ChatMsg::PostMessage {
                channel_id: "general".into(),
                message_id: "m2".into(),
                blocks: vec![Block::paragraph("final")],
                thread: None,
                as_agent: None,
            },
        )
        .await;
        apply_commit(
            &mut src,
            22,
            ChatMsg::PostMessage {
                channel_id: "general".into(),
                message_id: "r1".into(),
                blocks: vec![Block::paragraph("sync the thread too")],
                thread: Some(1),
                as_agent: None,
            },
        )
        .await;
        // an overwrite of the head record (edit) plus its revision record —
        // op-log order matters, so only the real sync path reproduces it.
        apply_commit(
            &mut src,
            23,
            ChatMsg::EditMessage {
                channel_id: "general".into(),
                seq: 2,
                blocks: vec![Block::paragraph("final, edited")],
                base_rev: Some(0),
            },
        )
        .await;
        apply_commit(
            &mut src,
            24,
            ChatMsg::AddReaction {
                channel_id: "general".into(),
                seq: 2,
                emoji: "ship".into(),
            },
        )
        .await;
        apply_commit(
            &mut src,
            25,
            ChatMsg::SetMembership {
                channel_id: "general".into(),
                user: vec![7; 32],
                member: true,
            },
        )
        .await;
        let src_root = src.root();
        assert_ne!(src_root, StateRoot::ZERO, "source must have a real root");

        let queries = [
            ChatQuery::Channels,
            ChatQuery::MessagesLatest {
                channel_id: "general".into(),
                limit: 16,
            },
            ChatQuery::Thread {
                channel_id: "general".into(),
                root_seq: 1,
                from: 0,
                limit: 16,
            },
            ChatQuery::Revisions {
                channel_id: "general".into(),
                seq: 2,
            },
            ChatQuery::Reactions {
                channel_id: "general".into(),
                seq: 2,
            },
            ChatQuery::Members {
                channel_id: "general".into(),
            },
        ];
        let mut expected = Vec::new();
        for query in &queries {
            expected.push(reply_bytes(&src, query.clone()).await);
        }
        let target = src.sync_target().await;
        let resolver = src.into_resolver();

        let synced = Chat::sync_from(context.child("dst"), "dst", target, resolver).await;

        assert_eq!(
            synced.root(),
            src_root,
            "synced store root must equal source root"
        );
        for (query, expected) in queries.iter().zip(&expected) {
            assert_eq!(
                &reply_bytes(&synced, query.clone()).await,
                expected,
                "synced reply must match source for {query:?}"
            );
        }
    });
}
