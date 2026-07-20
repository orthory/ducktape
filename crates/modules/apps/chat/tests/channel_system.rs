//! module-level behavior of the chat store: origin-derived authorship,
//! per-channel monotonic sequences, threads, edits/revisions, tombstones,
//! reactions, membership policy, hooks, pagination, write-time caps, the
//! reserved `:` channel-id namespace, and two-instance determinism.

use chat::Chat;
use chat::{
    AuthorRef, Block, ChatEvent, ChatMsg, ChatQuery, ChatReply, MAX_HOOKS_PER_CHANNEL,
    MAX_QUERY_LIMIT, Mark, PostPolicy, Span, decode_event, decode_reply, encode_msg, encode_query,
};
use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use sdk::{Ctx, Error, Module, Msg, Origin, StateRoot};
use statesync::qmdb::QmdbStore;

// build the module the way a host does: concrete store first, injected as
// `Box<dyn MerkleStore>`. a macro (not an fn) so the tests need no
// dev-dependency on commonware-storage just to spell the context bounds.
macro_rules! chat_on {
    ($context:expr, $id:expr) => {
        Chat::new($id, Box::new(QmdbStore::init($context, $id).await))
    };
}

struct TestCtx {
    env: sdk::Env,
    /// module ids `module_root` reports as registered (hook targets).
    known_modules: Vec<String>,
    /// follow-up msgs emitted during execute, in order.
    emitted: Vec<Msg>,
}

impl TestCtx {
    fn with_origin(consensus_time: u64, origin: Origin) -> Self {
        Self {
            env: sdk::Env { protocol_version: 0,
                height: 0,
                consensus_time,
                origin,
                me: "chat".into(),
            },
            known_modules: Vec::new(),
            emitted: Vec::new(),
        }
    }

    fn at(consensus_time: u64) -> Self {
        Self::with_origin(consensus_time, Origin::System)
    }

    fn knowing(mut self, module_id: &str) -> Self {
        self.known_modules.push(module_id.to_string());
        self
    }
}

#[async_trait::async_trait(?Send)]
impl Ctx for TestCtx {
    fn env(&self) -> &sdk::Env {
        &self.env
    }

    fn module_root(&self, target: &str) -> Option<StateRoot> {
        self.known_modules
            .iter()
            .any(|m| m == target)
            .then_some(StateRoot::ZERO)
    }

    async fn query(&self, _target: &str, _req: &[u8]) -> Result<Vec<u8>, Error> {
        Err(Error::QueryUnsupported)
    }

    fn emit_msg(&mut self, msg: Msg) {
        self.emitted.push(msg);
    }
    fn emit_event(&mut self, _ev: sdk::Event) {}
}

fn user(byte: u8) -> Origin {
    Origin::External(vec![byte; 32])
}

fn author_of(byte: u8) -> AuthorRef {
    AuthorRef::User(vec![byte; 32])
}

fn module_msg(payload: ChatMsg) -> Msg {
    Msg {
        target: "chat".into(),
        payload: encode_msg(&payload),
    }
}

fn create_channel(id: &str) -> ChatMsg {
    ChatMsg::CreateChannel {
        channel_id: id.into(),
        name: id.to_uppercase(),
        post_policy: PostPolicy::Open,
    }
}

fn rename(channel: &str, name: &str) -> ChatMsg {
    ChatMsg::RenameChannel {
        channel_id: channel.into(),
        name: name.into(),
    }
}

fn set_archived(channel: &str, archived: bool) -> ChatMsg {
    ChatMsg::SetChannelArchived {
        channel_id: channel.into(),
        archived,
    }
}

fn post(channel: &str, message_id: &str, text: &str, thread: Option<u64>) -> ChatMsg {
    ChatMsg::PostMessage {
        channel_id: channel.into(),
        message_id: message_id.into(),
        blocks: vec![Block::paragraph(text)],
        thread,
        as_agent: None,
    }
}

async fn query(module: &Chat, req: ChatQuery) -> ChatReply {
    let reply = module.query(&encode_query(&req)).await.unwrap();
    decode_reply(&reply).unwrap()
}

/// message sequences of a reply, for boundary assertions.
fn seqs(reply: &ChatReply) -> Vec<u64> {
    match reply {
        ChatReply::Messages(messages) => messages.iter().map(|m| m.seq).collect(),
        other => panic!("unexpected reply: {other:?}"),
    }
}

#[test]
fn assigns_monotonic_sequences_from_the_channel_counter_across_blocks() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        let root0 = module.root();

        module
            .execute(&mut TestCtx::at(10), &module_msg(create_channel("general")))
            .await
            .unwrap();
        assert_eq!(
            module.root(),
            root0,
            "root must reflect committed state only"
        );
        module.commit_block().await.unwrap();
        let root1 = module.root();
        assert_ne!(root1, root0, "committing the channel must move the root");

        // two posts in one block, a third in the next: sequences continue
        // from the persisted head_seq counter, gap-free.
        module
            .execute(
                &mut TestCtx::with_origin(20, user(1)),
                &module_msg(post("general", "m1", "hello", None)),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::with_origin(20, user(2)),
                &module_msg(post("general", "m2", "hi", None)),
            )
            .await
            .unwrap();
        assert_eq!(module.root(), root1, "posts stage until commit");
        module.commit_block().await.unwrap();
        module
            .execute(
                &mut TestCtx::with_origin(21, user(1)),
                &module_msg(post("general", "m3", "again", None)),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        let reply = query(
            &module,
            ChatQuery::MessagesLatest {
                channel_id: "general".into(),
                limit: 16,
            },
        )
        .await;
        assert_eq!(seqs(&reply), vec![1, 2, 3]);
        let ChatReply::Messages(messages) = reply else {
            unreachable!()
        };
        assert_eq!(messages[0].head.author, author_of(1));
        assert_eq!(messages[1].head.author, author_of(2));
        assert_eq!(messages[0].head.created_at, 20);
        assert_eq!(messages[2].head.created_at, 21);
        assert!(messages.iter().all(|m| m.channel_head_seq == 3));

        let ChatReply::Channel(Some(channel)) = query(
            &module,
            ChatQuery::Channel {
                channel_id: "general".into(),
            },
        )
        .await
        else {
            panic!("channel must exist");
        };
        assert_eq!(channel.head_seq, 3);
    });
}

#[test]
fn thread_replies_take_channel_sequences_and_update_the_root_summary() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        module
            .execute(&mut TestCtx::at(10), &module_msg(create_channel("general")))
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::with_origin(20, user(1)),
                &module_msg(post("general", "m1", "root", None)),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::with_origin(30, user(2)),
                &module_msg(post("general", "r1", "first reply", Some(1))),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::with_origin(31, user(3)),
                &module_msg(post("general", "r2", "second reply", Some(1))),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        let ChatReply::Thread(Some(thread)) = query(
            &module,
            ChatQuery::Thread {
                channel_id: "general".into(),
                root_seq: 1,
                from: 0,
                limit: 16,
            },
        )
        .await
        else {
            panic!("thread must exist");
        };
        assert_eq!(thread.root.head.reply_count, 2);
        assert_eq!(thread.root.head.last_reply_seq, Some(3));
        assert_eq!(
            thread.replies.iter().map(|r| r.seq).collect::<Vec<_>>(),
            vec![2, 3],
            "replies consume ordinary channel sequences"
        );
        assert!(thread.replies.iter().all(|r| r.head.thread == Some(1)));

        // a reply is not a thread root: no sub-threads, and Thread on it is None.
        let err = module
            .execute(
                &mut TestCtx::with_origin(32, user(1)),
                &module_msg(post("general", "r3", "subthread", Some(2))),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        module.abort_block().await.unwrap();
        assert_eq!(
            query(
                &module,
                ChatQuery::Thread {
                    channel_id: "general".into(),
                    root_seq: 2,
                    from: 0,
                    limit: 16,
                },
            )
            .await,
            ChatReply::Thread(None)
        );
    });
}

#[test]
fn delete_tombstones_the_head_but_preserves_thread_integrity() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        module
            .execute(&mut TestCtx::at(10), &module_msg(create_channel("general")))
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::with_origin(20, user(1)),
                &module_msg(post("general", "m1", "root", None)),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::with_origin(21, user(2)),
                &module_msg(post("general", "r1", "reply", Some(1))),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::with_origin(22, user(2)),
                &module_msg(ChatMsg::AddReaction {
                    channel_id: "general".into(),
                    seq: 1,
                    emoji: "wave".into(),
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        module
            .execute(
                &mut TestCtx::with_origin(30, user(1)),
                &module_msg(ChatMsg::DeleteMessage {
                    channel_id: "general".into(),
                    seq: 1,
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        let ChatReply::Thread(Some(thread)) = query(
            &module,
            ChatQuery::Thread {
                channel_id: "general".into(),
                root_seq: 1,
                from: 0,
                limit: 16,
            },
        )
        .await
        else {
            panic!("tombstoned root must still anchor its thread");
        };
        assert!(thread.root.head.deleted);
        assert!(thread.root.head.blocks.is_empty(), "content cleared");
        assert_eq!(thread.root.head.reply_count, 1, "summary preserved");
        assert_eq!(
            thread.root.head.author,
            author_of(1),
            "skeleton keeps author"
        );
        assert!(thread.root.reactions.is_empty(), "reactions cleared");
        assert_eq!(thread.replies.len(), 1, "replies remain readable");

        // the sequence promise survives: the next post takes seq 3.
        module
            .execute(
                &mut TestCtx::with_origin(40, user(2)),
                &module_msg(post("general", "m2", "after delete", None)),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let reply = query(
            &module,
            ChatQuery::MessagesLatest {
                channel_id: "general".into(),
                limit: 16,
            },
        )
        .await;
        assert_eq!(seqs(&reply), vec![1, 2, 3]);

        // double delete and edits of a tombstone are rejected.
        let err = module
            .execute(
                &mut TestCtx::with_origin(41, user(1)),
                &module_msg(ChatMsg::DeleteMessage {
                    channel_id: "general".into(),
                    seq: 1,
                }),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        module.abort_block().await.unwrap();
        let err = module
            .execute(
                &mut TestCtx::with_origin(42, user(1)),
                &module_msg(ChatMsg::EditMessage {
                    channel_id: "general".into(),
                    seq: 1,
                    blocks: vec![Block::paragraph("resurrect")],
                    base_rev: None,
                }),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        module.abort_block().await.unwrap();
    });
}

#[test]
fn edits_append_revisions_keep_lww_heads_and_record_base_rev() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        module
            .execute(&mut TestCtx::at(10), &module_msg(create_channel("general")))
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::with_origin(20, user(1)),
                &module_msg(post("general", "m1", "v0", None)),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        module
            .execute(
                &mut TestCtx::with_origin(30, user(1)),
                &module_msg(ChatMsg::EditMessage {
                    channel_id: "general".into(),
                    seq: 1,
                    blocks: vec![Block::paragraph("v1")],
                    base_rev: Some(0),
                }),
            )
            .await
            .unwrap();
        // a second edit claiming the SAME base: stale, recorded, not rejected —
        // the head is last-write-wins under the consensus total order.
        module
            .execute(
                &mut TestCtx::with_origin(31, user(1)),
                &module_msg(ChatMsg::EditMessage {
                    channel_id: "general".into(),
                    seq: 1,
                    blocks: vec![Block::paragraph("v2")],
                    base_rev: Some(0),
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        let ChatReply::Message(Some(view)) = query(
            &module,
            ChatQuery::Message {
                message_id: "m1".into(),
            },
        )
        .await
        else {
            panic!("message must exist");
        };
        assert_eq!(view.head.blocks, vec![Block::paragraph("v2")]);
        assert_eq!(view.head.rev, 2);
        assert_eq!(view.head.edited_at, Some(31));
        assert_eq!(
            view.head.base_rev,
            Some(0),
            "the stale claimed base is recorded (rev 2 edited from base 0)"
        );

        let ChatReply::Revisions(revisions) = query(
            &module,
            ChatQuery::Revisions {
                channel_id: "general".into(),
                seq: 1,
            },
        )
        .await
        else {
            panic!("revisions reply expected");
        };
        assert_eq!(revisions.len(), 2);
        assert_eq!(revisions[0].blocks, vec![Block::paragraph("v0")]);
        assert_eq!(revisions[0].rev, 0);
        assert_eq!(revisions[1].blocks, vec![Block::paragraph("v1")]);
        assert_eq!(revisions[1].rev, 1);
        assert_eq!(revisions[1].base_rev, Some(0));
    });
}

#[test]
fn authorship_derives_from_origin_and_cannot_be_spoofed() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        let root0 = module.root();

        // the demo-default empty external origin never passes.
        let err = module
            .execute(
                &mut TestCtx::with_origin(10, Origin::External(Vec::new())),
                &module_msg(create_channel("general")),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        module.abort_block().await.unwrap();
        assert_eq!(module.root(), root0);

        module
            .execute(&mut TestCtx::at(10), &module_msg(create_channel("general")))
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::with_origin(20, user(1)),
                &module_msg(post("general", "m1", "alice's message", None)),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::with_origin(21, Origin::Module("agent".into())),
                &module_msg(post("general", "m2", "module message", None)),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        let ChatReply::Messages(messages) = query(
            &module,
            ChatQuery::MessagesLatest {
                channel_id: "general".into(),
                limit: 16,
            },
        )
        .await
        else {
            panic!("messages reply expected");
        };
        assert_eq!(messages[0].head.author, author_of(1));
        assert_eq!(messages[1].head.author, AuthorRef::Module("agent".into()));

        // external origin B cannot edit or delete A's message.
        for op in [
            ChatMsg::EditMessage {
                channel_id: "general".into(),
                seq: 1,
                blocks: vec![Block::paragraph("stolen")],
                base_rev: None,
            },
            ChatMsg::DeleteMessage {
                channel_id: "general".into(),
                seq: 1,
            },
        ] {
            let err = module
                .execute(&mut TestCtx::with_origin(30, user(2)), &module_msg(op))
                .await
                .unwrap_err();
            assert!(matches!(err, Error::Module(_)));
            module.abort_block().await.unwrap();
        }
        // and a module origin cannot touch a user's message either.
        let err = module
            .execute(
                &mut TestCtx::with_origin(31, Origin::Module("agent".into())),
                &module_msg(ChatMsg::EditMessage {
                    channel_id: "general".into(),
                    seq: 1,
                    blocks: vec![Block::paragraph("stolen")],
                    base_rev: None,
                }),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        module.abort_block().await.unwrap();

        let ChatReply::Message(Some(view)) = query(
            &module,
            ChatQuery::Message {
                message_id: "m1".into(),
            },
        )
        .await
        else {
            panic!("message must exist");
        };
        assert_eq!(view.head.blocks, vec![Block::paragraph("alice's message")]);
        assert!(!view.head.deleted);
    });
}

#[test]
fn as_agent_is_honored_for_module_origins_and_rejected_for_everyone_else() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        module
            .execute(&mut TestCtx::at(10), &module_msg(create_channel("general")))
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let root_before = module.root();

        let as_agent_post = |message_id: &str| ChatMsg::PostMessage {
            channel_id: "general".into(),
            message_id: message_id.into(),
            blocks: vec![Block::paragraph("agent reply")],
            thread: None,
            as_agent: Some("quackbot".into()),
        };

        // an external user claiming an agent identity is rejected — users are
        // not genesis-trusted code — and so is the system origin.
        for origin in [user(1), Origin::System] {
            let err = module
                .execute(
                    &mut TestCtx::with_origin(20, origin),
                    &module_msg(as_agent_post("m1")),
                )
                .await
                .unwrap_err();
            assert!(matches!(err, Error::Module(_)));
            module.abort_block().await.unwrap();
            assert_eq!(module.root(), root_before, "the rejection leaves no trace");
        }

        // an empty agent id never passes, even from a module origin.
        let err = module
            .execute(
                &mut TestCtx::with_origin(20, Origin::Module("agent".into())),
                &module_msg(ChatMsg::PostMessage {
                    channel_id: "general".into(),
                    message_id: "m1".into(),
                    blocks: vec![Block::paragraph("agent reply")],
                    thread: None,
                    as_agent: Some(String::new()),
                }),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        module.abort_block().await.unwrap();

        // a module origin is honored: the stored author is the FULL agent ref,
        // module half from the origin, agent half from the payload.
        module
            .execute(
                &mut TestCtx::with_origin(21, Origin::Module("agent".into())),
                &module_msg(as_agent_post("m1")),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let ChatReply::Message(Some(view)) = query(
            &module,
            ChatQuery::Message {
                message_id: "m1".into(),
            },
        )
        .await
        else {
            panic!("message must exist");
        };
        assert_eq!(
            view.head.author,
            AuthorRef::Agent {
                module: "agent".into(),
                agent_id: "quackbot".into(),
            }
        );

        // author checks compare the FULL AuthorRef: the bare module origin is
        // a different author than its agent, so it cannot edit the agent post.
        let err = module
            .execute(
                &mut TestCtx::with_origin(22, Origin::Module("agent".into())),
                &module_msg(ChatMsg::EditMessage {
                    channel_id: "general".into(),
                    seq: 1,
                    blocks: vec![Block::paragraph("rewritten")],
                    base_rev: None,
                }),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        module.abort_block().await.unwrap();
    });
}

#[test]
fn reactions_are_idempotent_sets_per_emoji_and_author() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        module
            .execute(&mut TestCtx::at(10), &module_msg(create_channel("general")))
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::with_origin(20, user(1)),
                &module_msg(post("general", "m1", "react to me", None)),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        let add = || ChatMsg::AddReaction {
            channel_id: "general".into(),
            seq: 1,
            emoji: "duck".into(),
        };
        module
            .execute(&mut TestCtx::with_origin(21, user(2)), &module_msg(add()))
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let root_after_add = module.root();

        // add twice = once: the duplicate stages nothing, so the committed
        // qmdb op log — and the root — is byte-identical.
        module
            .execute(&mut TestCtx::with_origin(22, user(2)), &module_msg(add()))
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        assert_eq!(
            module.root(),
            root_after_add,
            "duplicate add must be a no-op"
        );

        // exact remove: an author who never reacted is a no-op...
        module
            .execute(
                &mut TestCtx::with_origin(23, user(3)),
                &module_msg(ChatMsg::RemoveReaction {
                    channel_id: "general".into(),
                    seq: 1,
                    emoji: "duck".into(),
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        assert_eq!(module.root(), root_after_add);
        assert_eq!(
            query(
                &module,
                ChatQuery::Reactions {
                    channel_id: "general".into(),
                    seq: 1,
                },
            )
            .await,
            ChatReply::Reactions(vec![chat::ReactionSummary {
                emoji: "duck".into(),
                reactors: [author_of(2)].into(),
            }])
        );

        // ...and the reactor's own remove clears the record.
        module
            .execute(
                &mut TestCtx::with_origin(24, user(2)),
                &module_msg(ChatMsg::RemoveReaction {
                    channel_id: "general".into(),
                    seq: 1,
                    emoji: "duck".into(),
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        assert_eq!(
            query(
                &module,
                ChatQuery::Reactions {
                    channel_id: "general".into(),
                    seq: 1,
                },
            )
            .await,
            ChatReply::Reactions(Vec::new())
        );

        // emoji byte cap is enforced at write time.
        let err = module
            .execute(
                &mut TestCtx::with_origin(25, user(2)),
                &module_msg(ChatMsg::AddReaction {
                    channel_id: "general".into(),
                    seq: 1,
                    emoji: "x".repeat(65),
                }),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        module.abort_block().await.unwrap();
    });
}

#[test]
fn messages_around_windows_the_history_at_one_sequence() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        module
            .execute(&mut TestCtx::at(10), &module_msg(create_channel("general")))
            .await
            .unwrap();
        for i in 1..=12u64 {
            module
                .execute(
                    &mut TestCtx::with_origin(20 + i, user(1)),
                    &module_msg(post("general", &format!("m{i}"), "body", None)),
                )
                .await
                .unwrap();
        }
        // a tombstone inside the window must page like any other row: the
        // jump-to projection has to match what `MessagesLatest` would return.
        module
            .execute(
                &mut TestCtx::with_origin(40, user(1)),
                &module_msg(ChatMsg::DeleteMessage {
                    channel_id: "general".into(),
                    seq: 6,
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        let around = |seq, limit| ChatQuery::MessagesAround {
            channel_id: "general".into(),
            seq,
            limit,
        };
        // mid-history: half the window before the target, the rest from it on.
        assert_eq!(seqs(&query(&module, around(7, 4)).await), vec![5, 6, 7, 8]);
        let ChatReply::Messages(window) = query(&module, around(7, 4)).await else {
            panic!("messages reply");
        };
        assert!(
            window.iter().any(|m| m.seq == 6 && m.head.deleted),
            "the tombstoned seq is a row of the window, not a hole"
        );
        // clamped at the start: the window slides forward, never below seq 1.
        assert_eq!(seqs(&query(&module, around(1, 4)).await), vec![1, 2, 3, 4]);
        assert_eq!(seqs(&query(&module, around(2, 4)).await), vec![1, 2, 3, 4]);
        // clamped at the head: nothing exists past it, so the window is short.
        assert_eq!(seqs(&query(&module, around(12, 4)).await), vec![10, 11, 12]);
        // a seq past the head windows the head rather than answering empty.
        assert_eq!(seqs(&query(&module, around(99, 4)).await), vec![10, 11, 12]);
        // limit bounds: 0 pages nothing, and an over-ask clamps to
        // MAX_QUERY_LIMIT (the whole channel here) like every other page.
        assert_eq!(seqs(&query(&module, around(7, 0)).await), Vec::<u64>::new());
        assert_eq!(
            seqs(&query(&module, around(7, MAX_QUERY_LIMIT + 1_000)).await),
            (1..=12).collect::<Vec<u64>>()
        );
    });
}

#[test]
fn pagination_is_correct_at_the_boundaries() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        module
            .execute(&mut TestCtx::at(10), &module_msg(create_channel("general")))
            .await
            .unwrap();
        for i in 1..=5u64 {
            module
                .execute(
                    &mut TestCtx::with_origin(20 + i, user(1)),
                    &module_msg(post("general", &format!("m{i}"), "body", None)),
                )
                .await
                .unwrap();
        }
        module.commit_block().await.unwrap();

        let latest = |limit| ChatQuery::MessagesLatest {
            channel_id: "general".into(),
            limit,
        };
        let range = |from_seq, limit| ChatQuery::MessagesRange {
            channel_id: "general".into(),
            from_seq,
            limit,
        };
        assert_eq!(seqs(&query(&module, latest(3)).await), vec![3, 4, 5]);
        assert_eq!(seqs(&query(&module, latest(5)).await), vec![1, 2, 3, 4, 5]);
        assert_eq!(
            seqs(&query(&module, latest(500)).await),
            vec![1, 2, 3, 4, 5]
        );
        assert_eq!(seqs(&query(&module, latest(0)).await), Vec::<u64>::new());
        assert_eq!(seqs(&query(&module, range(1, 2)).await), vec![1, 2]);
        assert_eq!(seqs(&query(&module, range(4, 10)).await), vec![4, 5]);
        assert_eq!(seqs(&query(&module, range(5, 1)).await), vec![5]);
        assert_eq!(seqs(&query(&module, range(6, 1)).await), Vec::<u64>::new());
        assert_eq!(seqs(&query(&module, range(0, 2)).await), vec![1, 2]);

        // thread paging: `from` is an offset into the reply list.
        for i in 1..=3u64 {
            module
                .execute(
                    &mut TestCtx::with_origin(30 + i, user(2)),
                    &module_msg(post("general", &format!("r{i}"), "reply", Some(1))),
                )
                .await
                .unwrap();
        }
        module.commit_block().await.unwrap();
        let ChatReply::Thread(Some(thread)) = query(
            &module,
            ChatQuery::Thread {
                channel_id: "general".into(),
                root_seq: 1,
                from: 1,
                limit: 1,
            },
        )
        .await
        else {
            panic!("thread must exist");
        };
        assert_eq!(
            thread.replies.iter().map(|r| r.seq).collect::<Vec<_>>(),
            vec![7],
            "offset 1, limit 1 of replies [6, 7, 8]"
        );
    });
}

#[test]
fn oversized_writes_are_rejected_before_staging_anything() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        module
            .execute(&mut TestCtx::at(10), &module_msg(create_channel("general")))
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let root = module.root();

        // > 64 KiB serialized head — rejected at write time (the qmdb codec
        // cap is decode-only; committing it would poison every later read).
        let err = module
            .execute(
                &mut TestCtx::with_origin(20, user(1)),
                &module_msg(post("general", "m1", &"x".repeat(65 * 1024), None)),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        module.abort_block().await.unwrap();
        assert_eq!(module.root(), root, "a rejected write leaves no trace");

        // the sequence counter did not advance: the next post takes seq 1.
        module
            .execute(
                &mut TestCtx::with_origin(21, user(1)),
                &module_msg(post("general", "m2", "small", None)),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let reply = query(
            &module,
            ChatQuery::MessagesLatest {
                channel_id: "general".into(),
                limit: 16,
            },
        )
        .await;
        assert_eq!(seqs(&reply), vec![1]);
    });
}

#[test]
fn members_only_channels_gate_external_posts_and_reactions() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        module
            .execute(
                &mut TestCtx::at(10),
                &module_msg(ChatMsg::CreateChannel {
                    channel_id: "core".into(),
                    name: "Core".into(),
                    post_policy: PostPolicy::MembersOnly,
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        // a non-member external user cannot post...
        let err = module
            .execute(
                &mut TestCtx::with_origin(20, user(1)),
                &module_msg(post("core", "m1", "let me in", None)),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        module.abort_block().await.unwrap();

        // ...a module author always may (genesis-fixed trusted code)...
        module
            .execute(
                &mut TestCtx::with_origin(21, Origin::Module("agent".into())),
                &module_msg(post("core", "m1", "agent reply", None)),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        // ...and membership (any non-empty origin may set it, for now) admits
        // the user for posts and reactions alike.
        module
            .execute(
                &mut TestCtx::with_origin(22, user(1)),
                &module_msg(ChatMsg::SetMembership {
                    channel_id: "core".into(),
                    user: vec![1; 32],
                    member: true,
                }),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::with_origin(23, user(1)),
                &module_msg(post("core", "m2", "member now", None)),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::with_origin(24, user(1)),
                &module_msg(ChatMsg::AddReaction {
                    channel_id: "core".into(),
                    seq: 1,
                    emoji: "wave".into(),
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        assert_eq!(
            query(
                &module,
                ChatQuery::Members {
                    channel_id: "core".into(),
                },
            )
            .await,
            ChatReply::Members(vec![vec![1; 32]])
        );

        // removal closes the door again.
        module
            .execute(
                &mut TestCtx::with_origin(25, user(1)),
                &module_msg(ChatMsg::SetMembership {
                    channel_id: "core".into(),
                    user: vec![1; 32],
                    member: false,
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let err = module
            .execute(
                &mut TestCtx::with_origin(26, user(1)),
                &module_msg(post("core", "m3", "locked out", None)),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        module.abort_block().await.unwrap();
    });
}

#[test]
fn hooks_are_validated_capped_and_emit_one_notification_per_post() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        module
            .execute(&mut TestCtx::at(10), &module_msg(create_channel("general")))
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        // unknown target and self-hooks are rejected at registration time.
        let err = module
            .execute(
                &mut TestCtx::with_origin(11, user(1)),
                &module_msg(ChatMsg::RegisterHook {
                    channel_id: "general".into(),
                    module_id: "ghost".into(),
                }),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        module.abort_block().await.unwrap();
        let err = module
            .execute(
                &mut TestCtx::with_origin(12, user(1)).knowing("chat"),
                &module_msg(ChatMsg::RegisterHook {
                    channel_id: "general".into(),
                    module_id: "chat".into(),
                }),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        module.abort_block().await.unwrap();

        module
            .execute(
                &mut TestCtx::with_origin(13, user(1)).knowing("agent"),
                &module_msg(ChatMsg::RegisterHook {
                    channel_id: "general".into(),
                    module_id: "agent".into(),
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        // a post emits exactly one follow-up per hook, carrying the event.
        let mut ctx = TestCtx::with_origin(20, user(1));
        module
            .execute(
                &mut ctx,
                &module_msg(ChatMsg::PostMessage {
                    channel_id: "general".into(),
                    message_id: "m1".into(),
                    blocks: vec![Block::Paragraph(vec![
                        Span::plain("ping "),
                        Span {
                            text: "@agent".into(),
                            marks: vec![Mark::Mention(AuthorRef::Module("agent".into()))],
                        },
                    ])],
                    thread: None,
                    as_agent: None,
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        assert_eq!(ctx.emitted.len(), 1);
        assert_eq!(ctx.emitted[0].target, "agent");
        assert_eq!(
            decode_event(&ctx.emitted[0].payload).unwrap(),
            ChatEvent::MessagePosted {
                channel_id: "general".into(),
                seq: 1,
                thread_root: None,
                author: author_of(1),
                mentions: vec![AuthorRef::Module("agent".into())],
            }
        );

        // the hook cap holds.
        for i in 0..MAX_HOOKS_PER_CHANNEL - 1 {
            let hook = format!("hook{i}");
            module
                .execute(
                    &mut TestCtx::with_origin(30, user(1)).knowing(&hook),
                    &module_msg(ChatMsg::RegisterHook {
                        channel_id: "general".into(),
                        module_id: hook.clone(),
                    }),
                )
                .await
                .unwrap();
        }
        let err = module
            .execute(
                &mut TestCtx::with_origin(31, user(1)).knowing("overflow"),
                &module_msg(ChatMsg::RegisterHook {
                    channel_id: "general".into(),
                    module_id: "overflow".into(),
                }),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        module.abort_block().await.unwrap();

        // unregistering stops the notifications.
        module
            .execute(
                &mut TestCtx::with_origin(40, user(1)),
                &module_msg(ChatMsg::UnregisterHook {
                    channel_id: "general".into(),
                    module_id: "agent".into(),
                }),
            )
            .await
            .unwrap();
        let mut ctx = TestCtx::with_origin(41, user(1));
        module
            .execute(&mut ctx, &module_msg(post("general", "m2", "quiet", None)))
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        assert!(ctx.emitted.is_empty());
    });
}

#[test]
fn duplicate_message_ids_are_rejected_globally() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        module
            .execute(&mut TestCtx::at(10), &module_msg(create_channel("a")))
            .await
            .unwrap();
        module
            .execute(&mut TestCtx::at(11), &module_msg(create_channel("b")))
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::with_origin(20, user(1)),
                &module_msg(post("a", "m1", "first", None)),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        // the same message id in ANOTHER channel is still a duplicate.
        let err = module
            .execute(
                &mut TestCtx::with_origin(21, user(1)),
                &module_msg(post("b", "m1", "second", None)),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        module.abort_block().await.unwrap();

        // and the global msgid index resolves to the original.
        let ChatReply::Message(Some(view)) = query(
            &module,
            ChatQuery::Message {
                message_id: "m1".into(),
            },
        )
        .await
        else {
            panic!("message must exist");
        };
        assert_eq!(view.channel_id, "a");
        assert_eq!(view.seq, 1);
    });
}

#[test]
fn channel_scoped_keys_do_not_collide_when_ids_contain_separators() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        // "a\0b" vs "a": a 0-byte-separator scheme would collide these once a
        // suffix follows; the length-prefixed components must not.
        module
            .execute(&mut TestCtx::at(10), &module_msg(create_channel("a\0b")))
            .await
            .unwrap();
        module
            .execute(&mut TestCtx::at(11), &module_msg(create_channel("a")))
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::with_origin(20, user(1)),
                &module_msg(post("a\0b", "m1", "first channel", None)),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::with_origin(21, user(2)),
                &module_msg(post("a", "m2", "second channel", None)),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::with_origin(22, user(1)),
                &module_msg(post("a\0b", "r1", "reply one", Some(1))),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::with_origin(23, user(2)),
                &module_msg(post("a", "r2", "reply two", Some(1))),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        let ChatReply::Thread(Some(first)) = query(
            &module,
            ChatQuery::Thread {
                channel_id: "a\0b".into(),
                root_seq: 1,
                from: 0,
                limit: 16,
            },
        )
        .await
        else {
            panic!("first thread must exist");
        };
        let ChatReply::Thread(Some(second)) = query(
            &module,
            ChatQuery::Thread {
                channel_id: "a".into(),
                root_seq: 1,
                from: 0,
                limit: 16,
            },
        )
        .await
        else {
            panic!("second thread must exist");
        };
        assert_eq!(
            first
                .replies
                .iter()
                .map(|r| r.head.message_id.as_str())
                .collect::<Vec<_>>(),
            vec!["r1"]
        );
        assert_eq!(
            second
                .replies
                .iter()
                .map(|r| r.head.message_id.as_str())
                .collect::<Vec<_>>(),
            vec!["r2"]
        );
    });
}

#[test]
fn rejects_posts_to_missing_channels_and_aborts_cleanly() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        let root0 = module.root();

        let err = module
            .execute(
                &mut TestCtx::with_origin(20, user(1)),
                &module_msg(post("ghost", "m1", "hello", None)),
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
            query(&module, ChatQuery::Channels).await,
            ChatReply::Channels(Vec::new())
        );
    });
}

#[test]
fn two_instances_replaying_the_same_ops_produce_identical_roots() {
    deterministic::Runner::default().start(|context| async move {
        let mut left = chat_on!(context.child("left"), "left");
        let mut right = chat_on!(context.child("right"), "right");

        // one op sequence, grouped into the same blocks, driven through both
        // stores: every block boundary must land on byte-identical roots.
        let blocks: Vec<Vec<(u64, Origin, ChatMsg)>> = vec![
            vec![(10, Origin::System, create_channel("general"))],
            vec![
                (20, user(1), post("general", "m1", "hello", None)),
                (20, user(2), post("general", "m2", "hi", None)),
            ],
            vec![(30, user(2), post("general", "r1", "reply", Some(1)))],
            vec![(
                40,
                user(1),
                ChatMsg::EditMessage {
                    channel_id: "general".into(),
                    seq: 1,
                    blocks: vec![Block::paragraph("hello, edited")],
                    base_rev: Some(0),
                },
            )],
            vec![
                (
                    50,
                    user(2),
                    ChatMsg::AddReaction {
                        channel_id: "general".into(),
                        seq: 2,
                        emoji: "duck".into(),
                    },
                ),
                (
                    50,
                    user(1),
                    ChatMsg::SetMembership {
                        channel_id: "general".into(),
                        user: vec![9; 32],
                        member: true,
                    },
                ),
            ],
            vec![(
                60,
                user(1),
                ChatMsg::DeleteMessage {
                    channel_id: "general".into(),
                    seq: 1,
                },
            )],
        ];

        for block in blocks {
            for (at, origin, op) in block {
                left.execute(
                    &mut TestCtx::with_origin(at, origin.clone()),
                    &module_msg(op.clone()),
                )
                .await
                .unwrap();
                right
                    .execute(&mut TestCtx::with_origin(at, origin), &module_msg(op))
                    .await
                    .unwrap();
            }
            left.commit_block().await.unwrap();
            right.commit_block().await.unwrap();
            assert_eq!(
                left.root(),
                right.root(),
                "same ops, same blocks -> byte-identical roots"
            );
        }
        assert_ne!(left.root(), StateRoot::ZERO);
    });
}

#[test]
fn huddle_join_and_leave_maintain_the_roster_in_join_order() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        module
            .execute(&mut TestCtx::at(10), &module_msg(create_channel("general")))
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        let join = |node_byte: u8| ChatMsg::JoinHuddle {
            channel_id: "general".into(),
            node: vec![node_byte; 32],
        };
        module
            .execute(&mut TestCtx::with_origin(20, user(1)), &module_msg(join(0xa1)))
            .await
            .unwrap();
        module
            .execute(&mut TestCtx::with_origin(21, user(2)), &module_msg(join(0xa2)))
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        let ChatReply::Channel(Some(channel)) = query(
            &module,
            ChatQuery::Channel {
                channel_id: "general".into(),
            },
        )
        .await
        else {
            panic!("channel must exist");
        };
        assert_eq!(channel.huddle.len(), 2);
        assert_eq!(channel.huddle[0].user, vec![1u8; 32]);
        assert_eq!(channel.huddle[0].node, vec![0xa1; 32]);
        assert_eq!(channel.huddle[0].joined_at, 20);
        assert_eq!(channel.huddle[1].user, vec![2u8; 32]);
        assert_eq!(channel.huddle[1].joined_at, 21);

        // re-join with the same node key is idempotent: root unchanged.
        let settled = module.root();
        module
            .execute(&mut TestCtx::with_origin(30, user(1)), &module_msg(join(0xa1)))
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        assert_eq!(module.root(), settled, "duplicate join must stage nothing");

        // re-join with a NEW node key re-routes without duplicating the entry
        // or resetting join order.
        module
            .execute(&mut TestCtx::with_origin(31, user(1)), &module_msg(join(0xb1)))
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let ChatReply::Channel(Some(channel)) = query(
            &module,
            ChatQuery::Channel {
                channel_id: "general".into(),
            },
        )
        .await
        else {
            panic!("channel must exist");
        };
        assert_eq!(channel.huddle.len(), 2);
        assert_eq!(channel.huddle[0].user, vec![1u8; 32]);
        assert_eq!(channel.huddle[0].node, vec![0xb1; 32]);
        assert_eq!(channel.huddle[0].joined_at, 20, "rejoin keeps join order");

        // leave removes exactly the leaver; the last leave empties the roster.
        module
            .execute(
                &mut TestCtx::with_origin(40, user(1)),
                &module_msg(ChatMsg::LeaveHuddle {
                    channel_id: "general".into(),
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let ChatReply::Channel(Some(channel)) = query(
            &module,
            ChatQuery::Channel {
                channel_id: "general".into(),
            },
        )
        .await
        else {
            panic!("channel must exist");
        };
        assert_eq!(channel.huddle.len(), 1);
        assert_eq!(channel.huddle[0].user, vec![2u8; 32]);

        // leaving while not in the huddle is a deterministic no-op.
        let settled = module.root();
        module
            .execute(
                &mut TestCtx::with_origin(41, user(3)),
                &module_msg(ChatMsg::LeaveHuddle {
                    channel_id: "general".into(),
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        assert_eq!(module.root(), settled, "absent leave must stage nothing");
    });
}

#[test]
fn huddle_rejects_non_users_bad_node_keys_and_over_capacity() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        module
            .execute(&mut TestCtx::at(10), &module_msg(create_channel("general")))
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        // module and system origins are not people — rejected.
        for origin in [Origin::Module("agent".into()), Origin::System] {
            let err = module
                .execute(
                    &mut TestCtx::with_origin(20, origin),
                    &module_msg(ChatMsg::JoinHuddle {
                        channel_id: "general".into(),
                        node: vec![0xaa; 32],
                    }),
                )
                .await
                .unwrap_err();
            assert!(format!("{err:?}").contains("external users"));
        }

        // a node key that is not raw ed25519 bytes is rejected.
        let err = module
            .execute(
                &mut TestCtx::with_origin(20, user(1)),
                &module_msg(ChatMsg::JoinHuddle {
                    channel_id: "general".into(),
                    node: vec![0xaa; 31],
                }),
            )
            .await
            .unwrap_err();
        assert!(format!("{err:?}").contains("32 bytes"));

        // the roster cap rejects the 33rd participant.
        for i in 0..chat::MAX_HUDDLE_MEMBERS {
            module
                .execute(
                    &mut TestCtx::with_origin(20, user(i as u8)),
                    &module_msg(ChatMsg::JoinHuddle {
                        channel_id: "general".into(),
                        node: vec![i as u8; 32],
                    }),
                )
                .await
                .unwrap();
        }
        let err = module
            .execute(
                &mut TestCtx::with_origin(20, user(200)),
                &module_msg(ChatMsg::JoinHuddle {
                    channel_id: "general".into(),
                    node: vec![0xcc; 32],
                }),
            )
            .await
            .unwrap_err();
        assert!(format!("{err:?}").contains("full"));
        module.abort_block().await.unwrap();
    });
}

#[test]
fn huddle_join_gates_on_members_only_policy_like_posting() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        module
            .execute(
                &mut TestCtx::at(10),
                &module_msg(ChatMsg::CreateChannel {
                    channel_id: "core".into(),
                    name: "CORE".into(),
                    post_policy: PostPolicy::MembersOnly,
                }),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::at(10),
                &module_msg(ChatMsg::SetMembership {
                    channel_id: "core".into(),
                    user: vec![1u8; 32],
                    member: true,
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        let join = ChatMsg::JoinHuddle {
            channel_id: "core".into(),
            node: vec![0xaa; 32],
        };
        // a non-member is turned away exactly like a non-member post.
        let err = module
            .execute(&mut TestCtx::with_origin(20, user(2)), &module_msg(join.clone()))
            .await
            .unwrap_err();
        assert!(format!("{err:?}").contains("members-only"));
        // the member joins fine.
        module
            .execute(&mut TestCtx::with_origin(20, user(1)), &module_msg(join))
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let ChatReply::Channel(Some(channel)) = query(
            &module,
            ChatQuery::Channel {
                channel_id: "core".into(),
            },
        )
        .await
        else {
            panic!("channel must exist");
        };
        assert_eq!(channel.huddle.len(), 1);
    });
}

#[test]
fn sweep_huddle_evicts_a_stale_member_and_is_idempotent() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        module
            .execute(&mut TestCtx::at(10), &module_msg(create_channel("general")))
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::with_origin(20, user(1)),
                &module_msg(ChatMsg::JoinHuddle {
                    channel_id: "general".into(),
                    node: vec![0xa1; 32],
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        // member B sweeps A's stale entry — A crashed and could not leave.
        module
            .execute(
                &mut TestCtx::with_origin(30, user(2)),
                &module_msg(ChatMsg::SweepHuddle {
                    channel_id: "general".into(),
                    user: vec![1u8; 32],
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let ChatReply::Channel(Some(channel)) = query(
            &module,
            ChatQuery::Channel {
                channel_id: "general".into(),
            },
        )
        .await
        else {
            panic!("channel must exist");
        };
        assert_eq!(channel.huddle.len(), 0, "sweep evicts the stale member");

        // sweeping an absent user is a deterministic no-op.
        let settled = module.root();
        module
            .execute(
                &mut TestCtx::with_origin(31, user(2)),
                &module_msg(ChatMsg::SweepHuddle {
                    channel_id: "general".into(),
                    user: vec![1u8; 32],
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        assert_eq!(module.root(), settled, "absent sweep must stage nothing");
    });
}

#[test]
fn sweep_huddle_gates_on_members_only_policy_like_posting() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        module
            .execute(
                &mut TestCtx::at(10),
                &module_msg(ChatMsg::CreateChannel {
                    channel_id: "core".into(),
                    name: "CORE".into(),
                    post_policy: PostPolicy::MembersOnly,
                }),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::at(10),
                &module_msg(ChatMsg::SetMembership {
                    channel_id: "core".into(),
                    user: vec![1u8; 32],
                    member: true,
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        // a non-member's sweep is turned away exactly like a non-member post.
        let err = module
            .execute(
                &mut TestCtx::with_origin(20, user(2)),
                &module_msg(ChatMsg::SweepHuddle {
                    channel_id: "core".into(),
                    user: vec![1u8; 32],
                }),
            )
            .await
            .unwrap_err();
        assert!(format!("{err:?}").contains("members-only"));
    });
}

#[test]
fn sweep_huddle_rejects_module_origin() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        module
            .execute(&mut TestCtx::at(10), &module_msg(create_channel("general")))
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        let err = module
            .execute(
                &mut TestCtx::with_origin(20, Origin::Module("agent".into())),
                &module_msg(ChatMsg::SweepHuddle {
                    channel_id: "general".into(),
                    user: vec![1u8; 32],
                }),
            )
            .await
            .unwrap_err();
        assert!(format!("{err:?}").contains("external users"));
    });
}

#[test]
fn external_users_cannot_create_reserved_colon_channel_ids() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");

        // ':' anywhere in the id is the module namespace — squatting rejected.
        let err = module
            .execute(
                &mut TestCtx::with_origin(10, user(1)),
                &module_msg(create_channel("forge:demo:1")),
            )
            .await
            .unwrap_err();
        assert!(format!("{err:?}").contains("reserved for modules"));

        // plain user channel ids keep working.
        module
            .execute(
                &mut TestCtx::with_origin(11, user(1)),
                &module_msg(create_channel("general")),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let ChatReply::Channel(Some(channel)) = query(
            &module,
            ChatQuery::Channel {
                channel_id: "general".into(),
            },
        )
        .await
        else {
            panic!("user channel must exist");
        };
        assert_eq!(channel.id, "general");
    });
}

#[test]
fn module_channels_must_use_the_modules_own_prefix() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");

        // forge minting under its own namespace is the supported shape.
        module
            .execute(
                &mut TestCtx::with_origin(10, Origin::Module("forge".into())),
                &module_msg(create_channel("forge:demo:1")),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let ChatReply::Channel(Some(channel)) = query(
            &module,
            ChatQuery::Channel {
                channel_id: "forge:demo:1".into(),
            },
        )
        .await
        else {
            panic!("module channel must exist");
        };
        assert_eq!(channel.id, "forge:demo:1");

        // another module cannot mint inside forge's namespace...
        let err = module
            .execute(
                &mut TestCtx::with_origin(11, Origin::Module("agent".into())),
                &module_msg(create_channel("forge:demo:2")),
            )
            .await
            .unwrap_err();
        assert!(format!("{err:?}").contains("prefixed 'agent:'"));

        // ...a module cannot mint an unprefixed id in the user plane...
        let err = module
            .execute(
                &mut TestCtx::with_origin(12, Origin::Module("forge".into())),
                &module_msg(create_channel("announcements")),
            )
            .await
            .unwrap_err();
        assert!(format!("{err:?}").contains("prefixed 'forge:'"));

        // ...and a shared string prefix is not the namespace: the colon must
        // immediately follow the module's own id.
        let err = module
            .execute(
                &mut TestCtx::with_origin(13, Origin::Module("forge".into())),
                &module_msg(create_channel("forgery:demo:1")),
            )
            .await
            .unwrap_err();
        assert!(format!("{err:?}").contains("prefixed 'forge:'"));

        // system origin stays unrestricted (genesis/system-internal writes).
        module
            .execute(&mut TestCtx::at(14), &module_msg(create_channel("sys:ops")))
            .await
            .unwrap();
        module.commit_block().await.unwrap();
    });
}

#[test]
fn rename_stamps_the_owner_at_create_and_gates_on_it() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        // a user-created channel is owned by its creator.
        module
            .execute(
                &mut TestCtx::with_origin(10, user(1)),
                &module_msg(create_channel("general")),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let ChatReply::Channel(Some(channel)) = query(
            &module,
            ChatQuery::Channel {
                channel_id: "general".into(),
            },
        )
        .await
        else {
            panic!("channel must exist");
        };
        assert_eq!(channel.owner, Some(vec![1u8; 32]), "creator is the owner");
        assert!(!channel.archived);

        // a non-owner user cannot rename an owned channel.
        let err = module
            .execute(
                &mut TestCtx::with_origin(11, user(2)),
                &module_msg(rename("general", "Hijacked")),
            )
            .await
            .unwrap_err();
        assert!(format!("{err:?}").contains("owner"));
        module.abort_block().await.unwrap();

        // an empty name is rejected — the reused CreateChannel name validation.
        let err = module
            .execute(
                &mut TestCtx::with_origin(12, user(1)),
                &module_msg(rename("general", "")),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        module.abort_block().await.unwrap();

        // the owner renames happily.
        module
            .execute(
                &mut TestCtx::with_origin(13, user(1)),
                &module_msg(rename("general", "General v2")),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let ChatReply::Channel(Some(channel)) = query(
            &module,
            ChatQuery::Channel {
                channel_id: "general".into(),
            },
        )
        .await
        else {
            panic!("channel must exist");
        };
        assert_eq!(channel.name, "General v2");
    });
}

#[test]
fn archived_channels_reject_writes_until_unarchived() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        module
            .execute(
                &mut TestCtx::with_origin(10, user(1)),
                &module_msg(create_channel("general")),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::with_origin(11, user(1)),
                &module_msg(post("general", "m1", "before archive", None)),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        // the owner archives the channel.
        module
            .execute(
                &mut TestCtx::with_origin(12, user(1)),
                &module_msg(set_archived("general", true)),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        // posts, reactions, and huddle joins are all turned away while archived.
        for op in [
            post("general", "m2", "blocked", None),
            ChatMsg::AddReaction {
                channel_id: "general".into(),
                seq: 1,
                emoji: "wave".into(),
            },
            ChatMsg::JoinHuddle {
                channel_id: "general".into(),
                node: vec![0xa1; 32],
            },
        ] {
            let err = module
                .execute(&mut TestCtx::with_origin(13, user(2)), &module_msg(op))
                .await
                .unwrap_err();
            assert!(format!("{err:?}").contains("archived"));
            module.abort_block().await.unwrap();
        }

        // unarchiving restores posting; the sequence promise survives the pause.
        module
            .execute(
                &mut TestCtx::with_origin(14, user(1)),
                &module_msg(set_archived("general", false)),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::with_origin(15, user(2)),
                &module_msg(post("general", "m2", "after unarchive", None)),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let reply = query(
            &module,
            ChatQuery::MessagesLatest {
                channel_id: "general".into(),
                limit: 16,
            },
        )
        .await;
        assert_eq!(seqs(&reply), vec![1, 2]);
    });
}

#[test]
fn ownerless_channels_admit_any_user_for_rename_and_archive() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        // a system-minted channel has no owner — also the shape a legacy record
        // (created before the field existed) decodes to via serde defaults.
        module
            .execute(&mut TestCtx::at(10), &module_msg(create_channel("general")))
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let ChatReply::Channel(Some(channel)) = query(
            &module,
            ChatQuery::Channel {
                channel_id: "general".into(),
            },
        )
        .await
        else {
            panic!("channel must exist");
        };
        assert_eq!(channel.owner, None, "system-minted channels are unowned");

        // any user may rename and archive an owner-less channel (mirrors the
        // existing SetMembership permissiveness).
        module
            .execute(
                &mut TestCtx::with_origin(11, user(7)),
                &module_msg(rename("general", "Renamed")),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::with_origin(12, user(9)),
                &module_msg(set_archived("general", true)),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let ChatReply::Channel(Some(channel)) = query(
            &module,
            ChatQuery::Channel {
                channel_id: "general".into(),
            },
        )
        .await
        else {
            panic!("channel must exist");
        };
        assert_eq!(channel.name, "Renamed");
        assert!(channel.archived);
    });
}

#[test]
fn users_cannot_archive_module_namespaced_channels() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        // a module-minted channel is unowned (owner = None), so `check_channel_admin`
        // alone would admit ANY user — the ':' namespace gate is what keeps them out.
        module
            .execute(
                &mut TestCtx::with_origin(10, Origin::Module("forge".into())),
                &module_msg(create_channel("forge:demo:1")),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        // a user may not archive forge's discussion channel: archiving turns away
        // every posting author, the owning module included — a cross-principal DoS.
        let err = module
            .execute(
                &mut TestCtx::with_origin(11, user(1)),
                &module_msg(set_archived("forge:demo:1", true)),
            )
            .await
            .unwrap_err();
        assert!(format!("{err:?}").contains("reserved for modules"));
        module.abort_block().await.unwrap();

        // the rejected attempt left no mark: the channel is still open and the
        // owning module can still post to it.
        module
            .execute(
                &mut TestCtx::with_origin(12, Origin::Module("forge".into())),
                &module_msg(post("forge:demo:1", "m1", "still open", None)),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let ChatReply::Channel(Some(channel)) = query(
            &module,
            ChatQuery::Channel {
                channel_id: "forge:demo:1".into(),
            },
        )
        .await
        else {
            panic!("module channel must exist");
        };
        assert!(!channel.archived, "the user's archive must not have landed");

        // the owning module still administers its own channel: archive...
        module
            .execute(
                &mut TestCtx::with_origin(13, Origin::Module("forge".into())),
                &module_msg(set_archived("forge:demo:1", true)),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let ChatReply::Channel(Some(channel)) = query(
            &module,
            ChatQuery::Channel {
                channel_id: "forge:demo:1".into(),
            },
        )
        .await
        else {
            panic!("module channel must exist");
        };
        assert!(channel.archived, "the module may archive its own channel");

        // ...and unarchive, which restores posting.
        module
            .execute(
                &mut TestCtx::with_origin(14, Origin::Module("forge".into())),
                &module_msg(set_archived("forge:demo:1", false)),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::with_origin(15, Origin::Module("forge".into())),
                &module_msg(post("forge:demo:1", "m2", "after unarchive", None)),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let reply = query(
            &module,
            ChatQuery::MessagesLatest {
                channel_id: "forge:demo:1".into(),
                limit: 16,
            },
        )
        .await;
        assert_eq!(seqs(&reply), vec![1, 2]);
    });
}
