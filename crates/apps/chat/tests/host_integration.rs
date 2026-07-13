//! chat under the real host lifecycle: origin-threaded blocks, rollback on
//! failure, and hook notifications committing (or aborting) atomically with
//! the post that caused them.

use std::cell::RefCell;
use std::rc::Rc;

use chat::Chat;
use chat::{
    AuthorRef, Block, ChatEvent, ChatMsg, ChatQuery, ChatReply, PostPolicy, decode_event,
    decode_reply, encode_msg, encode_query,
};
use commonware_runtime::{Runner as _, deterministic};
use host::{BlockContext, Host};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot};
use statesync::qmdb::QmdbStore;

// build the module the way a host does: concrete store first, injected as
// `Box<dyn MerkleStore>`.
macro_rules! chat_on {
    ($context:expr, $id:expr) => {
        Chat::new($id, Box::new(QmdbStore::init($context, $id).await))
    };
}

fn chat_msg(payload: ChatMsg) -> Msg {
    Msg {
        target: "chat".into(),
        payload: encode_msg(&payload),
    }
}

fn as_user(byte: u8) -> BlockContext {
    BlockContext { protocol_version: 0,
        height: 0,
        consensus_time: 0,
        origin: Origin::External(vec![byte; 32]),
    }
}

async fn chat_query(host: &Host, query: ChatQuery) -> ChatReply {
    let reply = host.query("chat", &encode_query(&query)).await.unwrap();
    decode_reply(&reply).unwrap()
}

/// a hook subscriber that stages received notifications during execute and
/// publishes them at commit — the same staging discipline as a real module —
/// so same-block atomicity with chat is observable through its root. the test
/// keeps a shared handle on the committed log (single-threaded runner).
struct Recorder {
    id: ModuleId,
    staged: Vec<Vec<u8>>,
    committed: Rc<RefCell<Vec<Vec<u8>>>>,
}

impl Recorder {
    fn new(id: &str) -> (Self, Rc<RefCell<Vec<Vec<u8>>>>) {
        let committed = Rc::new(RefCell::new(Vec::new()));
        (
            Self {
                id: id.into(),
                staged: Vec::new(),
                committed: committed.clone(),
            },
            committed,
        )
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Recorder {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    // a content commitment good enough for the test: notification count plus
    // a byte sum, so any committed difference moves the root (and app-hash).
    fn root(&self) -> StateRoot {
        let committed = self.committed.borrow();
        let mut root = [0u8; 32];
        root[..8].copy_from_slice(&(committed.len() as u64).to_le_bytes());
        let sum: u64 = committed
            .iter()
            .flat_map(|payload| payload.iter())
            .map(|byte| *byte as u64)
            .sum();
        root[8..16].copy_from_slice(&sum.to_le_bytes());
        StateRoot(root)
    }

    async fn execute(&mut self, _ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        // fail loud on garbage: a hook payload must be a chat event.
        decode_event(&msg.payload).map_err(Error::Module)?;
        self.staged.push(msg.payload.clone());
        Ok(())
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        self.committed.borrow_mut().append(&mut self.staged);
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.staged.clear();
        Ok(())
    }
}

/// a hook subscriber that always fails, poisoning any block that notifies it.
struct Boom {
    id: ModuleId,
}

#[async_trait::async_trait(?Send)]
impl Module for Boom {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }
    fn root(&self) -> StateRoot {
        StateRoot::ZERO
    }
    async fn execute(&mut self, _ctx: &mut dyn Ctx, _msg: &Msg) -> Result<(), Error> {
        Err(Error::Module("boom".into()))
    }
}

#[test]
fn host_commits_chat_blocks_and_serves_history_queries() {
    deterministic::Runner::default().start(|context| async move {
        let chat = chat_on!(context, "chat");
        let mut host = Host::genesis(vec![Box::new(chat)]).unwrap();
        let root0 = host.module_root("chat").unwrap();
        let app0 = host.app_hash();

        let out1 = host
            .submit_at(
                as_user(1),
                chat_msg(ChatMsg::CreateChannel {
                    channel_id: "general".into(),
                    name: "General".into(),
                    post_policy: PostPolicy::Open,
                }),
            )
            .await
            .unwrap();
        assert_ne!(host.module_root("chat").unwrap(), root0);
        assert_ne!(out1.app_hash, app0);

        host.submit_at(
            as_user(1),
            chat_msg(ChatMsg::PostMessage {
                channel_id: "general".into(),
                message_id: "m1".into(),
                blocks: vec![Block::paragraph("hello")],
                thread: None,
                as_agent: None,
            }),
        )
        .await
        .unwrap();

        let ChatReply::Messages(messages) = chat_query(
            &host,
            ChatQuery::MessagesLatest {
                channel_id: "general".into(),
                limit: 16,
            },
        )
        .await
        else {
            panic!("messages reply expected");
        };
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].seq, 1);
        assert_eq!(messages[0].head.author, AuthorRef::User(vec![1; 32]));
        assert_eq!(messages[0].head.blocks, vec![Block::paragraph("hello")]);
    });
}

#[test]
fn host_rolls_back_failed_chat_blocks() {
    deterministic::Runner::default().start(|context| async move {
        let chat = chat_on!(context, "chat");
        let mut host = Host::genesis(vec![Box::new(chat)]).unwrap();
        let root0 = host.module_root("chat").unwrap();
        let app0 = host.app_hash();

        let err = host
            .submit_at(
                as_user(1),
                chat_msg(ChatMsg::PostMessage {
                    channel_id: "missing".into(),
                    message_id: "m1".into(),
                    blocks: vec![Block::paragraph("hello")],
                    thread: None,
                    as_agent: None,
                }),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, host::SubmitError::Rejected(Error::Module(_))));
        assert_eq!(host.module_root("chat").unwrap(), root0);
        assert_eq!(host.app_hash(), app0);
    });
}

#[test]
fn default_empty_external_origin_is_rejected() {
    deterministic::Runner::default().start(|context| async move {
        let chat = chat_on!(context, "chat");
        let mut host = Host::genesis(vec![Box::new(chat)]).unwrap();
        let app0 = host.app_hash();

        // Host::submit uses BlockContext::default() = Origin::External(vec![]),
        // which must never pass as an authenticated author.
        let err = host
            .submit(chat_msg(ChatMsg::CreateChannel {
                channel_id: "general".into(),
                name: "General".into(),
                post_policy: PostPolicy::Open,
            }))
            .await
            .unwrap_err();
        assert!(matches!(err, host::SubmitError::Rejected(Error::Module(_))));
        assert_eq!(host.app_hash(), app0);
    });
}

#[test]
fn hook_notifications_commit_atomically_with_the_post() {
    deterministic::Runner::default().start(|context| async move {
        let chat = chat_on!(context, "chat");
        let (recorder, recorded) = Recorder::new("recorder");
        let boom = Boom { id: "boom".into() };
        let mut host =
            Host::genesis(vec![Box::new(chat), Box::new(recorder), Box::new(boom)]).unwrap();

        host.submit_at(
            as_user(1),
            chat_msg(ChatMsg::CreateChannel {
                channel_id: "general".into(),
                name: "General".into(),
                post_policy: PostPolicy::Open,
            }),
        )
        .await
        .unwrap();
        host.submit_at(
            as_user(1),
            chat_msg(ChatMsg::RegisterHook {
                channel_id: "general".into(),
                module_id: "recorder".into(),
            }),
        )
        .await
        .unwrap();

        // one block: the post AND the recorder's notification commit together.
        host.submit_at(
            as_user(1),
            chat_msg(ChatMsg::PostMessage {
                channel_id: "general".into(),
                message_id: "m1".into(),
                blocks: vec![Block::paragraph("notify me")],
                thread: None,
                as_agent: None,
            }),
        )
        .await
        .unwrap();
        {
            let recorded = recorded.borrow();
            assert_eq!(recorded.len(), 1, "same-block notification delivery");
            assert_eq!(
                decode_event(&recorded[0]).unwrap(),
                ChatEvent::MessagePosted {
                    channel_id: "general".into(),
                    seq: 1,
                    thread_root: None,
                    author: AuthorRef::User(vec![1; 32]),
                    mentions: Vec::new(),
                }
            );
        }

        // now poison the channel with a failing second hook: the whole block —
        // message, recorder notification, everything — must abort together.
        host.submit_at(
            as_user(1),
            chat_msg(ChatMsg::RegisterHook {
                channel_id: "general".into(),
                module_id: "boom".into(),
            }),
        )
        .await
        .unwrap();
        let chat_root = host.module_root("chat").unwrap();
        let recorder_root = host.module_root("recorder").unwrap();
        let app_hash = host.app_hash();

        let err = host
            .submit_at(
                as_user(1),
                chat_msg(ChatMsg::PostMessage {
                    channel_id: "general".into(),
                    message_id: "m2".into(),
                    blocks: vec![Block::paragraph("this must vanish")],
                    thread: None,
                    as_agent: None,
                }),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, host::SubmitError::Rejected(Error::Module(_))));
        assert_eq!(host.module_root("chat").unwrap(), chat_root);
        assert_eq!(host.module_root("recorder").unwrap(), recorder_root);
        assert_eq!(host.app_hash(), app_hash);
        assert_eq!(
            recorded.borrow().len(),
            1,
            "the aborted block's notification leaves no trace"
        );
        assert_eq!(
            chat_query(
                &host,
                ChatQuery::Message {
                    message_id: "m2".into(),
                },
            )
            .await,
            ChatReply::Message(None),
            "the aborted post leaves no trace"
        );
    });
}
