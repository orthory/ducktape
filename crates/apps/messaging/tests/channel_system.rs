use commonware_runtime::{Runner as _, deterministic};
use messaging::Messaging;
use messaging_interface::{
    Channel, ChatMessage, MessagingMsg, MessagingQuery, MessagingReply, Thread, decode_reply,
    encode_msg, encode_query,
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
                    thread_id: None,
                    reply_count: 0,
                    last_reply_at: None,
                },
                ChatMessage {
                    id: "m2".into(),
                    channel_id: "general".into(),
                    author: "bob".into(),
                    body: "hi".into(),
                    sequence: 2,
                    sent_at: 21,
                    thread_id: None,
                    reply_count: 0,
                    last_reply_at: None,
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
fn posts_thread_replies_and_updates_parent_summary() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = Messaging::init(context, "messaging").await;

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
        module
            .execute(
                &mut TestCtx::at(20),
                &module_msg(MessagingMsg::PostMessage {
                    channel_id: "general".into(),
                    message_id: "m1".into(),
                    author: "alice".into(),
                    body: "ship thread model".into(),
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let root_after_parent = module.root();

        module
            .execute(
                &mut TestCtx::at(30),
                &module_msg(MessagingMsg::PostThreadReply {
                    channel_id: "general".into(),
                    thread_id: "m1".into(),
                    message_id: "r1".into(),
                    author: "bob".into(),
                    body: "reply in context".into(),
                }),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::at(31),
                &module_msg(MessagingMsg::PostThreadReply {
                    channel_id: "general".into(),
                    thread_id: "m1".into(),
                    message_id: "r2".into(),
                    author: "carol".into(),
                    body: "keep sidebar quiet".into(),
                }),
            )
            .await
            .unwrap();

        let parent = ChatMessage {
            id: "m1".into(),
            channel_id: "general".into(),
            author: "alice".into(),
            body: "ship thread model".into(),
            sequence: 1,
            sent_at: 20,
            thread_id: None,
            reply_count: 2,
            last_reply_at: Some(31),
        };
        let replies = vec![
            ChatMessage {
                id: "r1".into(),
                channel_id: "general".into(),
                author: "bob".into(),
                body: "reply in context".into(),
                sequence: 1,
                sent_at: 30,
                thread_id: Some("m1".into()),
                reply_count: 0,
                last_reply_at: None,
            },
            ChatMessage {
                id: "r2".into(),
                channel_id: "general".into(),
                author: "carol".into(),
                body: "keep sidebar quiet".into(),
                sequence: 2,
                sent_at: 31,
                thread_id: Some("m1".into()),
                reply_count: 0,
                last_reply_at: None,
            },
        ];

        assert_eq!(
            query(
                &module,
                MessagingQuery::Messages {
                    channel_id: "general".into()
                }
            )
            .await,
            MessagingReply::Messages(vec![parent.clone()]),
            "channel history should show top-level parent with thread summary only"
        );
        assert_eq!(
            query(
                &module,
                MessagingQuery::Thread {
                    channel_id: "general".into(),
                    thread_id: "m1".into(),
                }
            )
            .await,
            MessagingReply::Thread(Some(Thread {
                root: parent,
                replies
            }))
        );

        assert_eq!(
            module.root(),
            root_after_parent,
            "thread replies are staged until commit"
        );
        module.commit_block().await.unwrap();
        assert_ne!(module.root(), root_after_parent);
    });
}

#[test]
fn rejected_thread_reply_rolls_back_parent_summary_and_replies() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = Messaging::init(context, "messaging").await;
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
        module.commit_block().await.unwrap();
        let root = module.root();

        let err = module
            .execute(
                &mut TestCtx::at(30),
                &module_msg(MessagingMsg::PostThreadReply {
                    channel_id: "general".into(),
                    thread_id: "missing".into(),
                    message_id: "r1".into(),
                    author: "bob".into(),
                    body: "no parent".into(),
                }),
            )
            .await
            .unwrap_err();

        assert!(matches!(err, Error::Module(_)));
        module.abort_block().await.unwrap();
        assert_eq!(module.root(), root);
        assert_eq!(
            query(
                &module,
                MessagingQuery::Thread {
                    channel_id: "general".into(),
                    thread_id: "missing".into(),
                }
            )
            .await,
            MessagingReply::Thread(None)
        );
    });
}

#[test]
fn thread_keys_do_not_collide_when_ids_contain_separators() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = Messaging::init(context, "messaging").await;

        module
            .execute(
                &mut TestCtx::at(10),
                &module_msg(MessagingMsg::CreateChannel {
                    channel_id: "a\0b".into(),
                    name: "A/B".into(),
                }),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::at(11),
                &module_msg(MessagingMsg::CreateChannel {
                    channel_id: "a".into(),
                    name: "A".into(),
                }),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::at(20),
                &module_msg(MessagingMsg::PostMessage {
                    channel_id: "a\0b".into(),
                    message_id: "c".into(),
                    author: "alice".into(),
                    body: "first root".into(),
                }),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::at(21),
                &module_msg(MessagingMsg::PostMessage {
                    channel_id: "a".into(),
                    message_id: "b\0c".into(),
                    author: "bob".into(),
                    body: "second root".into(),
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        module
            .execute(
                &mut TestCtx::at(30),
                &module_msg(MessagingMsg::PostThreadReply {
                    channel_id: "a\0b".into(),
                    thread_id: "c".into(),
                    message_id: "r1".into(),
                    author: "alice".into(),
                    body: "first reply".into(),
                }),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::at(31),
                &module_msg(MessagingMsg::PostThreadReply {
                    channel_id: "a".into(),
                    thread_id: "b\0c".into(),
                    message_id: "r2".into(),
                    author: "bob".into(),
                    body: "second reply".into(),
                }),
            )
            .await
            .unwrap();

        let first = query(
            &module,
            MessagingQuery::Thread {
                channel_id: "a\0b".into(),
                thread_id: "c".into(),
            },
        )
        .await;
        let second = query(
            &module,
            MessagingQuery::Thread {
                channel_id: "a".into(),
                thread_id: "b\0c".into(),
            },
        )
        .await;

        let MessagingReply::Thread(Some(first)) = first else {
            panic!("missing first thread")
        };
        let MessagingReply::Thread(Some(second)) = second else {
            panic!("missing second thread")
        };
        assert_eq!(
            first
                .replies
                .iter()
                .map(|m| m.id.as_str())
                .collect::<Vec<_>>(),
            vec!["r1"]
        );
        assert_eq!(
            second
                .replies
                .iter()
                .map(|m| m.id.as_str())
                .collect::<Vec<_>>(),
            vec!["r2"]
        );
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
