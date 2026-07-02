use agent::Agent;
use agent_interface::AgentMsg;
use chat::Chat;
use chat_interface::{
    ChatChannel, ChatMessage, ChatMsg, ChatQuery, ChatReply, DEFAULT_CHAT_TARGET, decode_reply,
    encode_msg, encode_query,
};
use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use host::Host;
use messaging::Messaging;
use messaging_interface::{MessagingQuery, MessagingReply};
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

fn agent_msg(payload: AgentMsg) -> Msg {
    Msg {
        target: agent_interface::DEFAULT_AGENT_TARGET.into(),
        payload: agent_interface::encode_msg(&payload),
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
fn chat_channels_include_agent_opened_sessions_when_backed_by_registered_messaging() {
    deterministic::Runner::default().start(|context| async move {
        let messaging = Messaging::init(
            context.child("messaging"),
            agent_interface::DEFAULT_MESSAGING_TARGET,
        )
        .await;
        let chat = Chat::init_with_messaging_id(
            context.child("chat"),
            DEFAULT_CHAT_TARGET,
            agent_interface::DEFAULT_MESSAGING_TARGET,
        )
        .await;
        let agent = Agent::init_with_messaging_id(
            context.child("agent"),
            agent_interface::DEFAULT_AGENT_TARGET,
            agent_interface::DEFAULT_MESSAGING_TARGET,
        )
        .await;
        let mut host =
            Host::genesis(vec![Box::new(messaging), Box::new(chat), Box::new(agent)]).unwrap();
        let chat_root = host.module_root(DEFAULT_CHAT_TARGET).unwrap();
        let messaging_root = host
            .module_root(agent_interface::DEFAULT_MESSAGING_TARGET)
            .unwrap();

        host.submit(agent_msg(AgentMsg::OpenSession {
            session_id: "agent-inbox".into(),
            title: "Agent Inbox".into(),
        }))
        .await
        .unwrap();
        host.submit(agent_msg(AgentMsg::AppendMessage {
            session_id: "agent-inbox".into(),
            message_id: "m1".into(),
            author: "agent".into(),
            body: "ready".into(),
        }))
        .await
        .unwrap();

        assert_eq!(
            host.module_root(DEFAULT_CHAT_TARGET).unwrap(),
            chat_root,
            "registered chat is a stateless filtered view"
        );
        assert_ne!(
            host.module_root(agent_interface::DEFAULT_MESSAGING_TARGET)
                .unwrap(),
            messaging_root,
            "registered messaging owns the storage root"
        );

        let channels = host
            .query(DEFAULT_CHAT_TARGET, &encode_query(&ChatQuery::Channels))
            .await
            .unwrap();
        assert_eq!(
            decode_reply(&channels).unwrap(),
            ChatReply::Channels(vec![ChatChannel {
                id: "agent-inbox".into(),
                name: "Agent Inbox".into(),
                created_at: 0,
            }])
        );

        let history = host
            .query(
                DEFAULT_CHAT_TARGET,
                &encode_query(&ChatQuery::Messages {
                    channel_id: "agent-inbox".into(),
                }),
            )
            .await
            .unwrap();
        assert_eq!(
            decode_reply(&history).unwrap(),
            ChatReply::Messages(vec![ChatMessage {
                id: "m1".into(),
                channel_id: "agent-inbox".into(),
                author: "agent".into(),
                body: "ready".into(),
                sequence: 1,
                sent_at: 0,
                thread_id: None,
                reply_count: 0,
                last_reply_at: None,
            }])
        );
    });
}

#[test]
fn chat_writes_land_in_registered_messaging_backing() {
    deterministic::Runner::default().start(|context| async move {
        let messaging = Messaging::init(
            context.child("messaging"),
            agent_interface::DEFAULT_MESSAGING_TARGET,
        )
        .await;
        let chat = Chat::init_with_messaging_id(
            context.child("chat"),
            DEFAULT_CHAT_TARGET,
            agent_interface::DEFAULT_MESSAGING_TARGET,
        )
        .await;
        let mut host = Host::genesis(vec![Box::new(messaging), Box::new(chat)]).unwrap();

        host.submit(chat_msg(ChatMsg::CreateChannel {
            channel_id: "general".into(),
            name: "General".into(),
        }))
        .await
        .unwrap();
        host.submit(chat_msg(ChatMsg::SendMessage {
            channel_id: "general".into(),
            message_id: "m1".into(),
            author: "alice".into(),
            body: "hello".into(),
        }))
        .await
        .unwrap();

        let backing_history = host
            .query(
                agent_interface::DEFAULT_MESSAGING_TARGET,
                &messaging_interface::encode_query(&MessagingQuery::Messages {
                    channel_id: "general".into(),
                    before: None,
                    limit: None,
                }),
            )
            .await
            .unwrap();
        assert_eq!(
            messaging_interface::decode_reply(&backing_history).unwrap(),
            MessagingReply::Messages(vec![messaging_interface::ChatMessage {
                id: "m1".into(),
                channel_id: "general".into(),
                author: "alice".into(),
                body: "hello".into(),
                sequence: 1,
                sent_at: 0,
                thread_id: None,
                reply_count: 0,
                last_reply_at: None,
            }])
        );
    });
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
                thread_id: None,
                reply_count: 0,
                last_reply_at: None,
            }])
        );
    });
}

#[test]
fn chat_exposes_threaded_conversations_from_shared_messaging_storage() {
    deterministic::Runner::default().start(|context| async move {
        let chat = Chat::init_with_messaging_id(context, DEFAULT_CHAT_TARGET, "messaging").await;
        let mut host = Host::genesis(vec![Box::new(chat)]).unwrap();

        host.submit(chat_msg(ChatMsg::CreateChannel {
            channel_id: "general".into(),
            name: "General".into(),
        }))
        .await
        .unwrap();
        host.submit(chat_msg(ChatMsg::SendMessage {
            channel_id: "general".into(),
            message_id: "m1".into(),
            author: "alice".into(),
            body: "ship richer chat".into(),
        }))
        .await
        .unwrap();
        host.submit(chat_msg(ChatMsg::ReplyInThread {
            channel_id: "general".into(),
            thread_id: "m1".into(),
            message_id: "r1".into(),
            author: "bob".into(),
            body: "threaded reply".into(),
        }))
        .await
        .unwrap();

        let parent = ChatMessage {
            id: "m1".into(),
            channel_id: "general".into(),
            author: "alice".into(),
            body: "ship richer chat".into(),
            sequence: 1,
            sent_at: 0,
            thread_id: None,
            reply_count: 1,
            last_reply_at: Some(0),
        };
        let reply = ChatMessage {
            id: "r1".into(),
            channel_id: "general".into(),
            author: "bob".into(),
            body: "threaded reply".into(),
            sequence: 1,
            sent_at: 0,
            thread_id: Some("m1".into()),
            reply_count: 0,
            last_reply_at: None,
        };

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
            ChatReply::Messages(vec![parent.clone()])
        );

        let thread = host
            .query(
                DEFAULT_CHAT_TARGET,
                &encode_query(&ChatQuery::Thread {
                    channel_id: "general".into(),
                    thread_id: "m1".into(),
                }),
            )
            .await
            .unwrap();
        assert_eq!(
            decode_reply(&thread).unwrap(),
            ChatReply::Thread(Some(chat_interface::ChatThread {
                root: parent,
                replies: vec![reply],
            }))
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
        assert!(matches!(err, host::SubmitError::Rejected(Error::Module(_))));
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
        apply_commit(
            &mut src,
            22,
            ChatMsg::ReplyInThread {
                channel_id: "general".into(),
                thread_id: "m1".into(),
                message_id: "r1".into(),
                author: "carol".into(),
                body: "thread survives sync".into(),
            },
        )
        .await;
        let src_root = src.root();
        assert_ne!(src_root, StateRoot::ZERO, "source must have a real root");

        let expected_messages = messages(&src, "general").await;
        let expected_thread = src
            .query(&encode_query(&ChatQuery::Thread {
                channel_id: "general".into(),
                thread_id: "m1".into(),
            }))
            .await
            .unwrap();
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
        assert_eq!(
            synced
                .query(&encode_query(&ChatQuery::Thread {
                    channel_id: "general".into(),
                    thread_id: "m1".into(),
                }))
                .await
                .unwrap(),
            expected_thread
        );
    });
}
