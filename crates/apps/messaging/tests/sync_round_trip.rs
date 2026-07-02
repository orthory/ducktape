//! state-sync round-trip: a fresh `Messaging` reconstructs a byte-identical
//! storage root by pulling a source store's operation range through commonware's
//! qmdb sync. this is the storage-backed joiner surface; replaying exported
//! channel/message records in sorted order is not enough because the qmdb root
//! commits to the live operation log.

use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use messaging::Messaging;
use messaging_interface::{
    ChatMessage, MessagingMsg, MessagingQuery, MessagingReply, decode_reply, encode_msg,
    encode_query,
};
use sdk::{Ctx, Error, Module, Msg, Origin, StateRoot};

struct TestCtx {
    env: sdk::Env,
}

impl TestCtx {
    fn at(consensus_time: u64) -> Self {
        Self {
            env: sdk::Env {
                height: 0,
                consensus_time,
                origin: Origin::System,
                me: "messaging".into(),
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

fn module_msg(payload: MessagingMsg) -> Msg {
    Msg {
        target: "messaging".into(),
        payload: encode_msg(&payload),
    }
}

async fn apply_commit<E>(module: &mut Messaging<E>, at: u64, payload: MessagingMsg)
where
    E: commonware_storage::Context + commonware_runtime::BufferPooler,
{
    module
        .execute(&mut TestCtx::at(at), &module_msg(payload))
        .await
        .unwrap();
    module.commit_block().await.unwrap();
}

async fn messages<E>(module: &Messaging<E>, channel_id: &str) -> Vec<ChatMessage>
where
    E: commonware_storage::Context + commonware_runtime::BufferPooler,
{
    let reply = module
        .query(&encode_query(&MessagingQuery::Messages {
            channel_id: channel_id.into(),
        }))
        .await
        .unwrap();
    match decode_reply(&reply).unwrap() {
        MessagingReply::Messages(messages) => messages,
        other => panic!("unexpected reply: {other:?}"),
    }
}

#[test]
fn synced_store_reconstructs_source_root_and_history() {
    deterministic::Runner::default().start(|context| async move {
        let mut src = Messaging::init(context.child("src"), "src").await;
        apply_commit(
            &mut src,
            10,
            MessagingMsg::CreateChannel {
                channel_id: "general".into(),
                name: "General".into(),
            },
        )
        .await;
        apply_commit(
            &mut src,
            20,
            MessagingMsg::PostMessage {
                channel_id: "general".into(),
                message_id: "m1".into(),
                author: "alice".into(),
                body: "draft".into(),
            },
        )
        .await;
        apply_commit(
            &mut src,
            21,
            MessagingMsg::PostMessage {
                channel_id: "general".into(),
                message_id: "m2".into(),
                author: "bob".into(),
                body: "final".into(),
            },
        )
        .await;
        let src_root = src.root();
        assert_ne!(src_root, StateRoot::ZERO, "source must have a real root");

        let expected_messages = messages(&src, "general").await;
        let target = src.sync_target().await;
        let resolver = src.into_resolver();

        let synced = Messaging::sync_from(context.child("dst"), "dst", target, resolver).await;

        assert_eq!(
            synced.root(),
            src_root,
            "synced store root must equal source root"
        );
        assert_eq!(messages(&synced, "general").await, expected_messages);
    });
}
