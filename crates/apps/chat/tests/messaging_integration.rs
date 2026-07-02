use chat::Chat;
use chat_interface::{
    ChatChannel, ChatMessage, ChatMsg, ChatQuery, ChatReply, DEFAULT_CHAT_TARGET, decode_reply,
    encode_msg, encode_query,
};
use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use host::Host;
use messaging::Messaging;
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
                me: DEFAULT_CHAT_TARGET.into(),
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

fn chat_msg(payload: ChatMsg) -> Msg {
    Msg {
        target: DEFAULT_CHAT_TARGET.into(),
        payload: encode_msg(&payload),
    }
}

async fn apply_commit<E>(module: &mut Chat<E>, at: u64, payload: ChatMsg)
where
    E: commonware_storage::Context + commonware_runtime::BufferPooler,
{
    module
        .execute(&mut TestCtx::at(at), &chat_msg(payload))
        .await
        .unwrap();
    module.commit_block().await.unwrap();
}

async fn messages<E>(module: &Chat<E>, channel_id: &str) -> Vec<ChatMessage>
where
    E: commonware_storage::Context + commonware_runtime::BufferPooler,
{
    let reply = module
        .query(&encode_query(&ChatQuery::Messages {
            channel_id: channel_id.into(),
        }))
        .await
        .unwrap();
    match decode_reply(&reply).unwrap() {
        ChatReply::Messages(messages) => messages,
        other => panic!("unexpected reply: {other:?}"),
    }
}

#[test]
fn chat_is_a_filtered_view_over_messaging_storage() {
    deterministic::Runner::default().start(|context| async move {
        let chat = Chat::init_with_messaging_id(context, DEFAULT_CHAT_TARGET, "messaging").await;
        let mut host = Host::genesis(vec![Box::new(chat)]).unwrap();
        let root0 = host.module_root(DEFAULT_CHAT_TARGET).unwrap();
        let app0 = host.app_hash();

        let out1 = host
            .submit(chat_msg(ChatMsg::CreateChannel {
                channel_id: "general".into(),
                name: "General".into(),
            }))
            .await
            .unwrap();
        assert_ne!(host.module_root(DEFAULT_CHAT_TARGET).unwrap(), root0);
        assert_ne!(out1.app_hash, app0);

        host.submit(chat_msg(ChatMsg::SendMessage {
            channel_id: "general".into(),
            message_id: "m1".into(),
            author: "alice".into(),
            body: "hello".into(),
        }))
        .await
        .unwrap();

        let channels = host
            .query(DEFAULT_CHAT_TARGET, &encode_query(&ChatQuery::Channels))
            .await
            .unwrap();
        assert_eq!(
            decode_reply(&channels).unwrap(),
            ChatReply::Channels(vec![ChatChannel {
                id: "general".into(),
                name: "General".into(),
                created_at: 0,
            }])
        );

        let history = host
            .query(
                DEFAULT_CHAT_TARGET,
                &encode_query(&ChatQuery::Messages {
                    channel_id: "general".into(),
                }),
            )
            .await
            .unwrap();
        assert_eq!(
            decode_reply(&history).unwrap(),
            ChatReply::Messages(vec![ChatMessage {
                id: "m1".into(),
                channel_id: "general".into(),
                author: "alice".into(),
                body: "hello".into(),
                sequence: 1,
                sent_at: 0,
            }])
        );
    });
}

#[test]
fn missing_channel_rolls_back_the_shared_storage() {
    deterministic::Runner::default().start(|context| async move {
        let chat = Chat::init_with_messaging_id(context, DEFAULT_CHAT_TARGET, "messaging").await;
        let mut host = Host::genesis(vec![Box::new(chat)]).unwrap();
        let root0 = host.module_root(DEFAULT_CHAT_TARGET).unwrap();
        let app0 = host.app_hash();

        let err = host
            .submit(chat_msg(ChatMsg::SendMessage {
                channel_id: "missing".into(),
                message_id: "m1".into(),
                author: "alice".into(),
                body: "hello".into(),
            }))
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        assert_eq!(host.module_root(DEFAULT_CHAT_TARGET).unwrap(), root0);
        assert_eq!(host.app_hash(), app0);
    });
}

#[test]
fn synced_chat_reconstructs_the_same_messaging_storage_view() {
    deterministic::Runner::default().start(|context| async move {
        let messaging = Messaging::init(context.child("src"), "messaging").await;
        let mut src = Chat::from_messaging(DEFAULT_CHAT_TARGET, messaging);
        apply_commit(
            &mut src,
            10,
            ChatMsg::CreateChannel {
                channel_id: "general".into(),
                name: "General".into(),
            },
        )
        .await;
        apply_commit(
            &mut src,
            20,
            ChatMsg::SendMessage {
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
            ChatMsg::SendMessage {
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

        let synced = Chat::sync_from_messaging_id(
            context.child("dst"),
            DEFAULT_CHAT_TARGET,
            "messaging",
            target,
            resolver,
        )
        .await;

        assert_eq!(
            synced.root(),
            src_root,
            "synced chat root must equal source messaging root"
        );
        assert_eq!(messages(&synced, "general").await, expected_messages);
    });
}
