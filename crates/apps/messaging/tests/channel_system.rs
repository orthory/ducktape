use commonware_runtime::{Runner as _, deterministic};
use messaging::Messaging;
use messaging_interface::{
    Channel, ChatMessage, MessagingMsg, MessagingQuery, MessagingReply, decode_reply, encode_msg,
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

async fn query<E>(module: &Messaging<E>, req: MessagingQuery) -> MessagingReply
where
    E: commonware_storage::Context + commonware_runtime::BufferPooler,
{
    let reply = module.query(&encode_query(&req)).await.unwrap();
    decode_reply(&reply).unwrap()
}

#[test]
fn creates_channels_and_posts_ordered_messages() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = Messaging::init(context, "messaging").await;
        let root0 = module.root();

        module
            .execute(
                &mut TestCtx::at(10),
                &module_msg(MessagingMsg::CreateChannel {
                    channel_id: "general".into(),
                    name: "General".into(),
                }),
            )
            .await
            .unwrap();

        assert_eq!(
            module.root(),
            root0,
            "root must reflect committed state only"
        );
        assert_eq!(
            query(&module, MessagingQuery::Channels).await,
            MessagingReply::Channels(vec![Channel {
                id: "general".into(),
                name: "General".into(),
                created_at: 10,
            }]),
            "queries must see this block's staged channel"
        );

        module.commit_block().await.unwrap();
        let root1 = module.root();
        assert_ne!(root1, root0, "committing the channel must move the root");

        module
            .execute(
                &mut TestCtx::at(20),
                &module_msg(MessagingMsg::PostMessage {
                    channel_id: "general".into(),
                    message_id: "m1".into(),
                    author: "alice".into(),
                    body: "hello".into(),
                }),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::at(21),
                &module_msg(MessagingMsg::PostMessage {
                    channel_id: "general".into(),
                    message_id: "m2".into(),
                    author: "bob".into(),
                    body: "hi".into(),
                }),
            )
            .await
            .unwrap();

        assert_eq!(
            query(
                &module,
                MessagingQuery::Messages {
                    channel_id: "general".into()
                }
            )
            .await,
            MessagingReply::Messages(vec![
                ChatMessage {
                    id: "m1".into(),
                    channel_id: "general".into(),
                    author: "alice".into(),
                    body: "hello".into(),
                    sequence: 1,
                    sent_at: 20,
                },
                ChatMessage {
                    id: "m2".into(),
                    channel_id: "general".into(),
                    author: "bob".into(),
                    body: "hi".into(),
                    sequence: 2,
                    sent_at: 21,
                },
            ]),
            "messages must be returned in per-channel sequence order"
        );

        assert_eq!(
            module.root(),
            root1,
            "staged messages must not move root before commit"
        );
        module.commit_block().await.unwrap();
        assert_ne!(module.root(), root1, "committed messages must move root");
    });
}

#[test]
fn rejects_posts_to_missing_channels_and_aborts_cleanly() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = Messaging::init(context, "messaging").await;
        let root0 = module.root();

        let err = module
            .execute(
                &mut TestCtx::at(20),
                &module_msg(MessagingMsg::PostMessage {
                    channel_id: "ghost".into(),
                    message_id: "m1".into(),
                    author: "alice".into(),
                    body: "hello".into(),
                }),
            )
            .await
            .unwrap_err();

        assert!(matches!(err, Error::Module(_)));
        module.abort_block().await.unwrap();
        assert_eq!(
            module.root(),
            root0,
            "rejected post must leave committed root unchanged"
        );
        assert_eq!(
            query(
                &module,
                MessagingQuery::Messages {
                    channel_id: "ghost".into()
                }
            )
            .await,
            MessagingReply::Messages(Vec::new())
        );
    });
}
