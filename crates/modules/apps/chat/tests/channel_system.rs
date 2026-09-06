//! module-level behavior of the chat store: origin-derived authorship,
//! per-channel monotonic sequences, threads, edits/revisions, tombstones,
//! reactions, membership policy, hooks, range pagination, write-time caps,
//! the reserved channel-id namespace, and two-instance determinism. reads go
//! through the three kept dispatch queries (`Channel` / `MessagesRange` /
//! `Message`); every UI-shaped listing (latest/around pages, threads,
//! revisions, reactions, members, channel lists) is an index-tier read,
//! covered by the native tests in `src/index.rs`.

use chat::Chat;
use chat::client::dm_channel_id;
use chat::{
    AuthorRef, Block, ChatEvent, ChatMsg, ChatQuery, ChatReply, MAX_CHANNELS_PER_CREATOR,
    MAX_HOOKS_PER_CHANNEL, MAX_QUERY_LIMIT, Mark, PostPolicy, Span, decode_event, decode_reply,
    encode_msg, encode_query,
};
use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use identity::{
    AccountView, IdentityQuery, IdentityReply, decode_query as identity_decode_query,
    encode_reply as identity_encode_reply,
};
use sdk::{Error, Module, Msg, Origin, StateRoot};
use sdk_testkit::TestCtx;
use statesync::qmdb::QmdbStore;

/// a minimal identity double: `key` (a 32-byte user origin id, as `user()`
/// mints) resolves to `number` through `OfKey`, and nothing else. registered
/// on a [`TestCtx`] via `.on_query("identity", ...)`.
fn identity_stub(accounts: Vec<(Vec<u8>, u64)>) -> impl FnMut(&[u8]) -> Result<Vec<u8>, Error> {
    move |req| {
        let IdentityQuery::OfKey { key } = identity_decode_query(req).map_err(Error::Module)?
        else {
            return Err(Error::Module(
                "test identity stub only answers OfKey".into(),
            ));
        };
        let account = accounts
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, number)| AccountView {
                number: *number,
                name: String::new(),
                keys: Vec::new(),
                avatar: None,
                bio: None,
                updated_at: 0,
            });
        Ok(identity_encode_reply(&IdentityReply::Account(account)))
    }
}

// build the module the way a host does: concrete store first, injected as
// `Box<dyn MerkleStore>`. a macro (not an fn) so the tests need no
// dev-dependency on commonware-storage just to spell the context bounds.
macro_rules! chat_on {
    ($context:expr, $id:expr) => {
        Chat::new($id, Box::new(QmdbStore::init($context, $id).await))
    };
}

// build the ctx a host hands chat: env at block 0, a chosen consensus time and
// origin, `me = "chat"`. `.with_module_root(id, StateRoot::ZERO)` marks a hook
// target live — chat gates hook dispatch on `module_root(target).is_some()`.
fn ctx_with_origin(consensus_time: u64, origin: Origin) -> TestCtx {
    TestCtx::with_env(sdk::Env {
        height: 0,
        consensus_time,
        origin,
        me: "chat".into(),
    })
}

fn ctx_at(consensus_time: u64) -> TestCtx {
    ctx_with_origin(consensus_time, Origin::System)
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

/// one user's standing in one channel, as the dispatch probe reads it.
async fn access(module: &Chat, channel: &str, byte: u8) -> chat::ChannelAccess {
    let req = ChatQuery::Access {
        channel_id: channel.into(),
        user: vec![byte; 32],
    };
    match query(module, req).await {
        ChatReply::Access(access) => access,
        other => panic!("expected Access, got {other:?}"),
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
            .execute(&mut ctx_at(10), &module_msg(create_channel("general")))
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
                &mut ctx_with_origin(20, user(1)),
                &module_msg(post("general", "m1", "hello", None)),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut ctx_with_origin(20, user(2)),
                &module_msg(post("general", "m2", "hi", None)),
            )
            .await
            .unwrap();
        assert_eq!(module.root(), root1, "posts stage until commit");
        module.commit_block().await.unwrap();
        module
            .execute(
                &mut ctx_with_origin(21, user(1)),
                &module_msg(post("general", "m3", "again", None)),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        let reply = query(
            &module,
            ChatQuery::MessagesRange {
                channel_id: "general".into(),
                from_seq: 1,
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
            .execute(&mut ctx_at(10), &module_msg(create_channel("general")))
            .await
            .unwrap();
        module
            .execute(
                &mut ctx_with_origin(20, user(1)),
                &module_msg(post("general", "m1", "root", None)),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut ctx_with_origin(30, user(2)),
                &module_msg(post("general", "r1", "first reply", Some(1))),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut ctx_with_origin(31, user(3)),
                &module_msg(post("general", "r2", "second reply", Some(1))),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        // the root's head carries the reply summary; the replies themselves
        // are ordinary channel sequences readable through the kept range read.
        let ChatReply::Messages(messages) = query(
            &module,
            ChatQuery::MessagesRange {
                channel_id: "general".into(),
                from_seq: 1,
                limit: 16,
            },
        )
        .await
        else {
            panic!("messages reply expected");
        };
        let (root, replies) = messages.split_first().expect("root must exist");
        assert_eq!(root.head.reply_count, 2);
        assert_eq!(root.head.last_reply_seq, Some(3));
        assert_eq!(
            replies.iter().map(|r| r.seq).collect::<Vec<_>>(),
            vec![2, 3],
            "replies consume ordinary channel sequences"
        );
        assert!(replies.iter().all(|r| r.head.thread == Some(1)));

        // a reply is not a thread root: no sub-threads.
        let err = module
            .execute(
                &mut ctx_with_origin(32, user(1)),
                &module_msg(post("general", "r3", "subthread", Some(2))),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        module.abort_block().await.unwrap();
    });
}

#[test]
fn delete_tombstones_the_head_but_preserves_thread_integrity() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        module
            .execute(&mut ctx_at(10), &module_msg(create_channel("general")))
            .await
            .unwrap();
        module
            .execute(
                &mut ctx_with_origin(20, user(1)),
                &module_msg(post("general", "m1", "root", None)),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut ctx_with_origin(21, user(2)),
                &module_msg(post("general", "r1", "reply", Some(1))),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut ctx_with_origin(22, user(2)),
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
                &mut ctx_with_origin(30, user(1)),
                &module_msg(ChatMsg::DeleteMessage {
                    channel_id: "general".into(),
                    seq: 1,
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        // the tombstoned root pages as an ordinary row of the range read —
        // a row, not a hole — and still anchors its thread. (the reaction
        // records are cleared too; that is index-observed now: the index
        // test `reactions_mirror_set_semantics_and_clear_on_tombstone`.)
        let ChatReply::Messages(messages) = query(
            &module,
            ChatQuery::MessagesRange {
                channel_id: "general".into(),
                from_seq: 1,
                limit: 16,
            },
        )
        .await
        else {
            panic!("messages reply expected");
        };
        let (root, replies) = messages.split_first().expect("root must exist");
        assert!(root.head.deleted);
        assert!(root.head.blocks.is_empty(), "content cleared");
        assert_eq!(root.head.reply_count, 1, "summary preserved");
        assert_eq!(root.head.author, author_of(1), "skeleton keeps author");
        assert_eq!(replies.len(), 1, "replies remain readable");
        assert_eq!(replies[0].head.thread, Some(1), "thread linkage survives");

        // the sequence promise survives: the next post takes seq 3.
        module
            .execute(
                &mut ctx_with_origin(40, user(2)),
                &module_msg(post("general", "m2", "after delete", None)),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let reply = query(
            &module,
            ChatQuery::MessagesRange {
                channel_id: "general".into(),
                from_seq: 1,
                limit: 16,
            },
        )
        .await;
        assert_eq!(seqs(&reply), vec![1, 2, 3]);

        // double delete and edits of a tombstone are rejected.
        let err = module
            .execute(
                &mut ctx_with_origin(41, user(1)),
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
                &mut ctx_with_origin(42, user(1)),
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
            .execute(&mut ctx_at(10), &module_msg(create_channel("general")))
            .await
            .unwrap();
        module
            .execute(
                &mut ctx_with_origin(20, user(1)),
                &module_msg(post("general", "m1", "v0", None)),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        module
            .execute(
                &mut ctx_with_origin(30, user(1)),
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
                &mut ctx_with_origin(31, user(1)),
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
        // the revision LISTING (prior blocks, per-revision base_rev) is an
        // index-tier read now: the index test `edits_keep_revision_history`.
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
                &mut ctx_with_origin(10, Origin::External(Vec::new())),
                &module_msg(create_channel("general")),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        module.abort_block().await.unwrap();
        assert_eq!(module.root(), root0);

        module
            .execute(&mut ctx_at(10), &module_msg(create_channel("general")))
            .await
            .unwrap();
        module
            .execute(
                &mut ctx_with_origin(20, user(1)),
                &module_msg(post("general", "m1", "alice's message", None)),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut ctx_with_origin(21, Origin::Module("agent".into())),
                &module_msg(post("general", "m2", "module message", None)),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        let ChatReply::Messages(messages) = query(
            &module,
            ChatQuery::MessagesRange {
                channel_id: "general".into(),
                from_seq: 1,
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
                .execute(&mut ctx_with_origin(30, user(2)), &module_msg(op))
                .await
                .unwrap_err();
            assert!(matches!(err, Error::Module(_)));
            module.abort_block().await.unwrap();
        }
        // and a module origin cannot touch a user's message either.
        let err = module
            .execute(
                &mut ctx_with_origin(31, Origin::Module("agent".into())),
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
            .execute(&mut ctx_at(10), &module_msg(create_channel("general")))
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
                    &mut ctx_with_origin(20, origin),
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
                &mut ctx_with_origin(20, Origin::Module("agent".into())),
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
                &mut ctx_with_origin(21, Origin::Module("agent".into())),
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
                &mut ctx_with_origin(22, Origin::Module("agent".into())),
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
            .execute(&mut ctx_at(10), &module_msg(create_channel("general")))
            .await
            .unwrap();
        module
            .execute(
                &mut ctx_with_origin(20, user(1)),
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
            .execute(&mut ctx_with_origin(21, user(2)), &module_msg(add()))
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let root_after_add = module.root();

        // add twice = once: the duplicate stages nothing, so the committed
        // qmdb op log — and the root — is byte-identical.
        module
            .execute(&mut ctx_with_origin(22, user(2)), &module_msg(add()))
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
                &mut ctx_with_origin(23, user(3)),
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

        // ...and the reactor's own remove clears the record: it stages a
        // delete (the root moves), after which the SAME remove is an exact
        // no-op again — the record is gone. (the reactor-set VIEW is an
        // index-tier read now: the index test
        // `reactions_mirror_set_semantics_and_clear_on_tombstone`.)
        module
            .execute(
                &mut ctx_with_origin(24, user(2)),
                &module_msg(ChatMsg::RemoveReaction {
                    channel_id: "general".into(),
                    seq: 1,
                    emoji: "duck".into(),
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let root_after_remove = module.root();
        assert_ne!(
            root_after_remove, root_after_add,
            "the reactor's remove must stage the record's deletion"
        );
        module
            .execute(
                &mut ctx_with_origin(25, user(2)),
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
            module.root(),
            root_after_remove,
            "removing the already-removed reaction must stage nothing"
        );

        // emoji byte cap is enforced at write time.
        let err = module
            .execute(
                &mut ctx_with_origin(26, user(2)),
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
fn pagination_is_correct_at_the_boundaries() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        module
            .execute(&mut ctx_at(10), &module_msg(create_channel("general")))
            .await
            .unwrap();
        for i in 1..=5u64 {
            module
                .execute(
                    &mut ctx_with_origin(20 + i, user(1)),
                    &module_msg(post("general", &format!("m{i}"), "body", None)),
                )
                .await
                .unwrap();
        }
        module.commit_block().await.unwrap();

        // the kept dispatch window: `MessagesRange` over the gap-free sequence
        // space. latest/around/thread paging are index-tier reads now (the
        // index test `message_pages_range_latest_around` and
        // `threads_page_in_post_order_and_roots_carry_summaries`).
        let range = |from_seq, limit| ChatQuery::MessagesRange {
            channel_id: "general".into(),
            from_seq,
            limit,
        };
        assert_eq!(seqs(&query(&module, range(1, 2)).await), vec![1, 2]);
        assert_eq!(seqs(&query(&module, range(4, 10)).await), vec![4, 5]);
        assert_eq!(seqs(&query(&module, range(5, 1)).await), vec![5]);
        assert_eq!(seqs(&query(&module, range(6, 1)).await), Vec::<u64>::new());
        assert_eq!(seqs(&query(&module, range(0, 2)).await), vec![1, 2]);
        // limit bounds: 0 pages nothing, and an over-ask clamps to
        // MAX_QUERY_LIMIT (the whole channel here) rather than erroring.
        assert_eq!(seqs(&query(&module, range(1, 0)).await), Vec::<u64>::new());
        assert_eq!(
            seqs(&query(&module, range(1, MAX_QUERY_LIMIT + 1_000)).await),
            vec![1, 2, 3, 4, 5]
        );
    });
}

#[test]
fn oversized_writes_are_rejected_before_staging_anything() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        module
            .execute(&mut ctx_at(10), &module_msg(create_channel("general")))
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let root = module.root();

        // > 64 KiB serialized head — rejected at write time (the qmdb codec
        // cap is decode-only; committing it would poison every later read).
        let err = module
            .execute(
                &mut ctx_with_origin(20, user(1)),
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
                &mut ctx_with_origin(21, user(1)),
                &module_msg(post("general", "m2", "small", None)),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let reply = query(
            &module,
            ChatQuery::MessagesRange {
                channel_id: "general".into(),
                from_seq: 1,
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
        // user(1) creates it, so user(1) is the owner and may write the roster
        // (`SetMembership` is channel-admin authority). owning is not membership:
        // the owner still cannot POST until the roster admits them.
        module
            .execute(
                &mut ctx_with_origin(10, user(1)),
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
                &mut ctx_with_origin(20, user(1)),
                &module_msg(post("core", "m1", "let me in", None)),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        module.abort_block().await.unwrap();

        // ...a module author always may (genesis-fixed trusted code)...
        module
            .execute(
                &mut ctx_with_origin(21, Origin::Module("agent".into())),
                &module_msg(post("core", "m1", "agent reply", None)),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        // ...and membership, written by the channel's OWNER, admits the user
        // for posts and reactions alike.
        module
            .execute(
                &mut ctx_with_origin(22, user(1)),
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
                &mut ctx_with_origin(23, user(1)),
                &module_msg(post("core", "m2", "member now", None)),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut ctx_with_origin(24, user(1)),
                &module_msg(ChatMsg::AddReaction {
                    channel_id: "core".into(),
                    seq: 1,
                    emoji: "wave".into(),
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        // the member point record is the policy read AND the idempotence
        // anchor: re-granting an unchanged membership stages nothing, so the
        // root is byte-identical. (the roster VIEW is an index-tier read now:
        // the index test `members_and_huddles_track_rosters`.)
        let settled = module.root();
        module
            .execute(
                &mut ctx_with_origin(25, user(1)),
                &module_msg(ChatMsg::SetMembership {
                    channel_id: "core".into(),
                    user: vec![1; 32],
                    member: true,
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        assert_eq!(
            module.root(),
            settled,
            "unchanged membership stages nothing"
        );

        // removal closes the door again.
        module
            .execute(
                &mut ctx_with_origin(26, user(1)),
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
                &mut ctx_with_origin(27, user(1)),
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
        // user(1) owns the channel — hook (un)registration is channel-admin
        // authority, so every RegisterHook below is the owner's own.
        module
            .execute(
                &mut ctx_with_origin(10, user(1)),
                &module_msg(create_channel("general")),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        // unknown target and self-hooks are rejected at registration time.
        let err = module
            .execute(
                &mut ctx_with_origin(11, user(1)),
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
                &mut ctx_with_origin(12, user(1)).with_module_root("chat", StateRoot::ZERO),
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
                &mut ctx_with_origin(13, user(1)).with_module_root("agent", StateRoot::ZERO),
                &module_msg(ChatMsg::RegisterHook {
                    channel_id: "general".into(),
                    module_id: "agent".into(),
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        // a post emits exactly one follow-up per hook, carrying the event.
        let mut ctx = ctx_with_origin(20, user(1));
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
        assert_eq!(ctx.msgs().len(), 1);
        assert_eq!(ctx.msgs()[0].target, "agent");
        assert_eq!(
            decode_event(&ctx.msgs()[0].payload).unwrap(),
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
                    &mut ctx_with_origin(30, user(1)).with_module_root(&hook, StateRoot::ZERO),
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
                &mut ctx_with_origin(31, user(1)).with_module_root("overflow", StateRoot::ZERO),
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
                &mut ctx_with_origin(40, user(1)),
                &module_msg(ChatMsg::UnregisterHook {
                    channel_id: "general".into(),
                    module_id: "agent".into(),
                }),
            )
            .await
            .unwrap();
        let mut ctx = ctx_with_origin(41, user(1));
        module
            .execute(&mut ctx, &module_msg(post("general", "m2", "quiet", None)))
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        assert!(ctx.msgs().is_empty());
    });
}

#[test]
fn duplicate_message_ids_are_rejected_globally() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        module
            .execute(&mut ctx_at(10), &module_msg(create_channel("a")))
            .await
            .unwrap();
        module
            .execute(&mut ctx_at(11), &module_msg(create_channel("b")))
            .await
            .unwrap();
        module
            .execute(
                &mut ctx_with_origin(20, user(1)),
                &module_msg(post("a", "m1", "first", None)),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        // the same message id in ANOTHER channel is still a duplicate.
        let err = module
            .execute(
                &mut ctx_with_origin(21, user(1)),
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
            .execute(&mut ctx_at(10), &module_msg(create_channel("a\0b")))
            .await
            .unwrap();
        module
            .execute(&mut ctx_at(11), &module_msg(create_channel("a")))
            .await
            .unwrap();
        module
            .execute(
                &mut ctx_with_origin(20, user(1)),
                &module_msg(post("a\0b", "m1", "first channel", None)),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut ctx_with_origin(21, user(2)),
                &module_msg(post("a", "m2", "second channel", None)),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut ctx_with_origin(22, user(1)),
                &module_msg(post("a\0b", "r1", "reply one", Some(1))),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut ctx_with_origin(23, user(2)),
                &module_msg(post("a", "r2", "reply two", Some(1))),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        // each channel's range read must page ONLY its own records — a key
        // collision would bleed one channel's rows (or its root's reply
        // summary) into the other.
        let message_ids = |reply: &ChatReply| -> Vec<String> {
            let ChatReply::Messages(messages) = reply else {
                panic!("messages reply expected");
            };
            messages.iter().map(|m| m.head.message_id.clone()).collect()
        };
        let first = query(
            &module,
            ChatQuery::MessagesRange {
                channel_id: "a\0b".into(),
                from_seq: 1,
                limit: 16,
            },
        )
        .await;
        let second = query(
            &module,
            ChatQuery::MessagesRange {
                channel_id: "a".into(),
                from_seq: 1,
                limit: 16,
            },
        )
        .await;
        assert_eq!(message_ids(&first), vec!["m1", "r1"]);
        assert_eq!(message_ids(&second), vec!["m2", "r2"]);
        for reply in [&first, &second] {
            let ChatReply::Messages(messages) = reply else {
                unreachable!()
            };
            assert_eq!(
                messages[0].head.reply_count, 1,
                "each root counts its own reply"
            );
            assert_eq!(messages[1].head.thread, Some(1));
        }
    });
}

#[test]
fn rejects_posts_to_missing_channels_and_aborts_cleanly() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        let root0 = module.root();

        let err = module
            .execute(
                &mut ctx_with_origin(20, user(1)),
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
            query(
                &module,
                ChatQuery::Channel {
                    channel_id: "ghost".into(),
                },
            )
            .await,
            ChatReply::Channel(None),
            "the rejected post must not have minted the channel"
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
            // user(1) owns "general", so the SetMembership block below is the
            // owner's own write (channel-admin authority).
            vec![(10, user(1), create_channel("general"))],
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
                    &mut ctx_with_origin(at, origin.clone()),
                    &module_msg(op.clone()),
                )
                .await
                .unwrap();
                right
                    .execute(&mut ctx_with_origin(at, origin), &module_msg(op))
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
            .execute(&mut ctx_at(10), &module_msg(create_channel("general")))
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        let join = |node_byte: u8| ChatMsg::JoinHuddle {
            channel_id: "general".into(),
            node: vec![node_byte; 32],
        };
        module
            .execute(&mut ctx_with_origin(20, user(1)), &module_msg(join(0xa1)))
            .await
            .unwrap();
        module
            .execute(&mut ctx_with_origin(21, user(2)), &module_msg(join(0xa2)))
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
            .execute(&mut ctx_with_origin(30, user(1)), &module_msg(join(0xa1)))
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        assert_eq!(module.root(), settled, "duplicate join must stage nothing");

        // re-join with a NEW node key re-routes without duplicating the entry
        // or resetting join order.
        module
            .execute(&mut ctx_with_origin(31, user(1)), &module_msg(join(0xb1)))
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
                &mut ctx_with_origin(40, user(1)),
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
                &mut ctx_with_origin(41, user(3)),
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
            .execute(&mut ctx_at(10), &module_msg(create_channel("general")))
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        // module and system origins are not people — rejected.
        for origin in [Origin::Module("agent".into()), Origin::System] {
            let err = module
                .execute(
                    &mut ctx_with_origin(20, origin),
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
                &mut ctx_with_origin(20, user(1)),
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
                    &mut ctx_with_origin(20, user(i as u8)),
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
                &mut ctx_with_origin(20, user(200)),
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
                &mut ctx_at(10),
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
                &mut ctx_at(10),
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
            .execute(&mut ctx_with_origin(20, user(2)), &module_msg(join.clone()))
            .await
            .unwrap_err();
        assert!(format!("{err:?}").contains("members-only"));
        // the member joins fine.
        module
            .execute(&mut ctx_with_origin(20, user(1)), &module_msg(join))
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
fn sweep_huddle_self_is_a_leave_and_is_idempotent() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        module
            .execute(&mut ctx_at(10), &module_msg(create_channel("general")))
            .await
            .unwrap();
        module
            .execute(
                &mut ctx_with_origin(20, user(1)),
                &module_msg(ChatMsg::JoinHuddle {
                    channel_id: "general".into(),
                    node: vec![0xa1; 32],
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        // A names itself — a sweep of yourself is a leave, always allowed.
        module
            .execute(
                &mut ctx_with_origin(30, user(1)),
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
        assert_eq!(channel.huddle.len(), 0, "self-sweep evicts the caller");

        // sweeping an absent user is a deterministic no-op.
        let settled = module.root();
        module
            .execute(
                &mut ctx_with_origin(31, user(1)),
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
fn sweep_huddle_of_another_user_by_a_non_admin_is_refused() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        // system-minted, so unowned: no user administers it.
        module
            .execute(&mut ctx_at(10), &module_msg(create_channel("general")))
            .await
            .unwrap();
        module
            .execute(
                &mut ctx_with_origin(20, user(1)),
                &module_msg(ChatMsg::JoinHuddle {
                    channel_id: "general".into(),
                    node: vec![0xa1; 32],
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        // any poster on the channel naming a DIFFERENT, still-live user is
        // refused — this is the eviction #1625 closes.
        let err = module
            .execute(
                &mut ctx_with_origin(30, user(2)),
                &module_msg(ChatMsg::SweepHuddle {
                    channel_id: "general".into(),
                    user: vec![1u8; 32],
                }),
            )
            .await
            .unwrap_err();
        assert!(format!("{err:?}").contains("channel admin"), "{err:?}");
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
        assert_eq!(channel.huddle.len(), 1, "the refused sweep changes nothing");
    });
}

#[test]
fn sweep_huddle_of_another_user_by_the_channel_admin_succeeds() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        // user(9) creates it, so user(9) is the owner (channel admin).
        module
            .execute(
                &mut ctx_with_origin(10, user(9)),
                &module_msg(create_channel("general")),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut ctx_with_origin(20, user(1)),
                &module_msg(ChatMsg::JoinHuddle {
                    channel_id: "general".into(),
                    node: vec![0xa1; 32],
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        module
            .execute(
                &mut ctx_with_origin(30, user(9)),
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
        assert_eq!(
            channel.huddle.len(),
            0,
            "the admin's sweep evicts the member"
        );
    });
}

#[test]
fn sweep_huddle_rejects_module_origin() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        module
            .execute(&mut ctx_at(10), &module_msg(create_channel("general")))
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        let err = module
            .execute(
                &mut ctx_with_origin(20, Origin::Module("agent".into())),
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
                &mut ctx_with_origin(10, user(1)),
                &module_msg(create_channel("forge:demo:1")),
            )
            .await
            .unwrap_err();
        assert!(format!("{err:?}").contains("reserved for modules"));

        // '/' is the index tier's key-path separator — banned for every
        // origin, mirroring the ':' gate.
        for origin in [user(1), Origin::Module("forge".into()), Origin::System] {
            let err = module
                .execute(
                    &mut ctx_with_origin(10, origin),
                    &module_msg(create_channel("general/sub")),
                )
                .await
                .unwrap_err();
            assert!(format!("{err:?}").contains("may not contain '/'"));
        }

        // plain user channel ids keep working.
        module
            .execute(
                &mut ctx_with_origin(11, user(1)),
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
                &mut ctx_with_origin(10, Origin::Module("forge".into())),
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
                &mut ctx_with_origin(11, Origin::Module("agent".into())),
                &module_msg(create_channel("forge:demo:2")),
            )
            .await
            .unwrap_err();
        assert!(format!("{err:?}").contains("prefixed 'agent:'"));

        // ...a module cannot mint an unprefixed id in the user plane...
        let err = module
            .execute(
                &mut ctx_with_origin(12, Origin::Module("forge".into())),
                &module_msg(create_channel("announcements")),
            )
            .await
            .unwrap_err();
        assert!(format!("{err:?}").contains("prefixed 'forge:'"));

        // ...and a shared string prefix is not the namespace: the colon must
        // immediately follow the module's own id.
        let err = module
            .execute(
                &mut ctx_with_origin(13, Origin::Module("forge".into())),
                &module_msg(create_channel("forgery:demo:1")),
            )
            .await
            .unwrap_err();
        assert!(format!("{err:?}").contains("prefixed 'forge:'"));

        // system origin stays unrestricted (genesis/system-internal writes).
        module
            .execute(&mut ctx_at(14), &module_msg(create_channel("sys:ops")))
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
                &mut ctx_with_origin(10, user(1)),
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
                &mut ctx_with_origin(11, user(2)),
                &module_msg(rename("general", "Hijacked")),
            )
            .await
            .unwrap_err();
        assert!(format!("{err:?}").contains("owner"));
        module.abort_block().await.unwrap();

        // an empty name is rejected — the reused CreateChannel name validation.
        let err = module
            .execute(
                &mut ctx_with_origin(12, user(1)),
                &module_msg(rename("general", "")),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        module.abort_block().await.unwrap();

        // the owner renames happily.
        module
            .execute(
                &mut ctx_with_origin(13, user(1)),
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
                &mut ctx_with_origin(10, user(1)),
                &module_msg(create_channel("general")),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut ctx_with_origin(11, user(1)),
                &module_msg(post("general", "m1", "before archive", None)),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        // the owner archives the channel.
        module
            .execute(
                &mut ctx_with_origin(12, user(1)),
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
                .execute(&mut ctx_with_origin(13, user(2)), &module_msg(op))
                .await
                .unwrap_err();
            assert!(format!("{err:?}").contains("archived"));
            module.abort_block().await.unwrap();
        }

        // unarchiving restores posting; the sequence promise survives the pause.
        module
            .execute(
                &mut ctx_with_origin(14, user(1)),
                &module_msg(set_archived("general", false)),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut ctx_with_origin(15, user(2)),
                &module_msg(post("general", "m2", "after unarchive", None)),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let reply = query(
            &module,
            ChatQuery::MessagesRange {
                channel_id: "general".into(),
                from_seq: 1,
                limit: 16,
            },
        )
        .await;
        assert_eq!(seqs(&reply), vec![1, 2]);
    });
}

#[test]
fn ownerless_channels_refuse_every_user_admin_op() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        // a system-minted channel has no owner (owner == None). the live case
        // is a MODULE-minted one (`forge:<repo>:<n>`); this one is reachable
        // without a colon id so the ':' namespace gate cannot be what refuses.
        module
            .execute(&mut ctx_at(10), &module_msg(create_channel("general")))
            .await
            .unwrap();
        module
            .execute(
                &mut ctx_at(11).with_module_root("agent", StateRoot::ZERO),
                &module_msg(ChatMsg::RegisterHook {
                    channel_id: "general".into(),
                    module_id: "agent".into(),
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
        assert_eq!(channel.owner, None, "system-minted channels are unowned");

        // NO user administers an unowned channel: there is no owner to be, and
        // the minting principal is a module. every channel-admin op refuses.
        for op in [
            rename("general", "Hijacked"),
            set_archived("general", true),
            ChatMsg::SetMembership {
                channel_id: "general".into(),
                user: vec![7; 32],
                member: true,
            },
            ChatMsg::RegisterHook {
                channel_id: "general".into(),
                module_id: "tasks".into(),
            },
            ChatMsg::UnregisterHook {
                channel_id: "general".into(),
                module_id: "agent".into(),
            },
        ] {
            let err = module
                .execute(
                    &mut ctx_with_origin(20, user(7))
                        .with_module_root("tasks", StateRoot::ZERO)
                        .with_module_root("agent", StateRoot::ZERO),
                    &module_msg(op),
                )
                .await
                .unwrap_err();
            assert!(
                format!("{err:?}").contains("unowned"),
                "unowned channel must refuse the user: {err:?}"
            );
            module.abort_block().await.unwrap();
        }

        // nothing landed: name, archived flag, hook list and roster untouched.
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
        assert_eq!(channel.name, "GENERAL");
        assert!(!channel.archived);
        assert_eq!(channel.hooks, vec!["agent".to_string()]);

        // the trusted principals still administer it — that is who an unowned
        // channel belongs to.
        module
            .execute(
                &mut ctx_with_origin(30, Origin::Module("agent".into())),
                &module_msg(ChatMsg::SetMembership {
                    channel_id: "general".into(),
                    user: vec![7; 32],
                    member: true,
                }),
            )
            .await
            .unwrap();
        module
            .execute(&mut ctx_at(31), &module_msg(rename("general", "Renamed")))
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
    });
}

#[test]
fn membership_and_hooks_are_owner_gated_like_rename() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        // user(1) creates — and therefore owns — a members-only channel, and
        // registers a hook on it.
        module
            .execute(
                &mut ctx_with_origin(10, user(1)),
                &module_msg(ChatMsg::CreateChannel {
                    channel_id: "core".into(),
                    name: "Core".into(),
                    post_policy: PostPolicy::MembersOnly,
                }),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut ctx_with_origin(11, user(1)).with_module_root("agent", StateRoot::ZERO),
                &module_msg(ChatMsg::RegisterHook {
                    channel_id: "core".into(),
                    module_id: "agent".into(),
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        // a stranger may not add THEMSELVES to the roster — the whole point:
        // a self-service roster would make `MembersOnly` no admission rule.
        // nor may they attach a hook, nor detach the owner's.
        for op in [
            ChatMsg::SetMembership {
                channel_id: "core".into(),
                user: vec![2; 32],
                member: true,
            },
            ChatMsg::RegisterHook {
                channel_id: "core".into(),
                module_id: "tasks".into(),
            },
            ChatMsg::UnregisterHook {
                channel_id: "core".into(),
                module_id: "agent".into(),
            },
        ] {
            let err = module
                .execute(
                    &mut ctx_with_origin(20, user(2)).with_module_root("tasks", StateRoot::ZERO),
                    &module_msg(op),
                )
                .await
                .unwrap_err();
            assert!(
                format!("{err:?}").contains("only the owner"),
                "a non-owner must be refused: {err:?}"
            );
            module.abort_block().await.unwrap();
        }

        // the roster and the hook list are exactly as the owner left them, so
        // the stranger is still locked out of the members-only channel.
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
        assert_eq!(channel.hooks, vec!["agent".to_string()]);
        let err = module
            .execute(
                &mut ctx_with_origin(21, user(2)),
                &module_msg(post("core", "m1", "let me in", None)),
            )
            .await
            .unwrap_err();
        assert!(format!("{err:?}").contains("members-only"));
        module.abort_block().await.unwrap();

        // the owner performs all three, and the admitted user can then post.
        module
            .execute(
                &mut ctx_with_origin(30, user(1)),
                &module_msg(ChatMsg::SetMembership {
                    channel_id: "core".into(),
                    user: vec![2; 32],
                    member: true,
                }),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut ctx_with_origin(31, user(1)).with_module_root("tasks", StateRoot::ZERO),
                &module_msg(ChatMsg::RegisterHook {
                    channel_id: "core".into(),
                    module_id: "tasks".into(),
                }),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut ctx_with_origin(32, user(1)),
                &module_msg(ChatMsg::UnregisterHook {
                    channel_id: "core".into(),
                    module_id: "agent".into(),
                }),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut ctx_with_origin(33, user(2)),
                &module_msg(post("core", "m1", "admitted", None)),
            )
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
        assert_eq!(channel.hooks, vec!["tasks".to_string()]);
        assert_eq!(channel.head_seq, 1);
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
                &mut ctx_with_origin(10, Origin::Module("forge".into())),
                &module_msg(create_channel("forge:demo:1")),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        // a user may not archive forge's discussion channel: archiving turns away
        // every posting author, the owning module included — a cross-principal DoS.
        let err = module
            .execute(
                &mut ctx_with_origin(11, user(1)),
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
                &mut ctx_with_origin(12, Origin::Module("forge".into())),
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
                &mut ctx_with_origin(13, Origin::Module("forge".into())),
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
                &mut ctx_with_origin(14, Origin::Module("forge".into())),
                &module_msg(set_archived("forge:demo:1", false)),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut ctx_with_origin(15, Origin::Module("forge".into())),
                &module_msg(post("forge:demo:1", "m2", "after unarchive", None)),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let reply = query(
            &module,
            ChatQuery::MessagesRange {
                channel_id: "forge:demo:1".into(),
                from_seq: 1,
                limit: 16,
            },
        )
        .await;
        assert_eq!(seqs(&reply), vec![1, 2]);
    });
}

/// the standing probe a module acting on a user's behalf reads: chat answers
/// what that ONE user may do in ONE channel, so the caller never carries a
/// second copy of the admission rule. an absent channel is closed.
#[test]
fn access_answers_one_users_standing_from_chats_own_gates() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        // an absent channel is closed: the caller fails closed on it.
        let absent = access(&module, "ghost", 1).await;
        assert!(!absent.may_read && !absent.may_post);

        // an OPEN channel admits any authenticated user, member or not.
        module
            .execute(
                &mut ctx_with_origin(10, user(1)),
                &module_msg(create_channel("open")),
            )
            .await
            .unwrap();
        // a MEMBERS-ONLY channel admits only its roster — owning is not
        // membership, exactly as the post gate has it.
        module
            .execute(
                &mut ctx_with_origin(11, user(1)),
                &module_msg(ChatMsg::CreateChannel {
                    channel_id: "core".into(),
                    name: "Core".into(),
                    post_policy: PostPolicy::MembersOnly,
                }),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut ctx_with_origin(12, user(1)),
                &module_msg(ChatMsg::SetMembership {
                    channel_id: "core".into(),
                    user: vec![2; 32],
                    member: true,
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        let stranger_open = access(&module, "open", 9).await;
        assert!(stranger_open.may_read && stranger_open.may_post);
        let owner_core = access(&module, "core", 1).await;
        assert!(
            !owner_core.may_read && !owner_core.may_post,
            "owning is not membership"
        );
        let member_core = access(&module, "core", 2).await;
        assert!(member_core.may_read && member_core.may_post);

        // archiving closes POSTING (the post gate, verbatim) and leaves
        // reading open.
        module
            .execute(
                &mut ctx_with_origin(13, user(1)),
                &module_msg(set_archived("core", true)),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let archived = access(&module, "core", 2).await;
        assert!(archived.may_read, "archival does not close reading");
        assert!(!archived.may_post, "an archived channel takes no posts");
    });
}

#[test]
fn a_bare_dm_shaped_id_is_reserved_from_plain_create_channel() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        let squatted = dm_channel_id("1", "2");

        // any user origin, including one of the pair itself, is refused —
        // minting a dm- id always goes through CreateDmChannel.
        let err = module
            .execute(
                &mut ctx_with_origin(10, user(1)),
                &module_msg(create_channel(&squatted)),
            )
            .await
            .unwrap_err();
        assert!(format!("{err:?}").contains("reserved"));
    });
}

#[test]
fn a_third_account_can_never_mint_the_pairs_derived_dm_id() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat").with_identity("identity");
        let alice_number = 1u64;
        let bob_number = 2u64;
        let mallory_number = 3u64;
        let pairs = vec![
            (vec![1u8; 32], alice_number),
            (vec![3u8; 32], mallory_number),
        ];
        let their_dm = dm_channel_id(&alice_number.to_string(), &bob_number.to_string());

        // mallory can only ever derive HER OWN pair's id — never alice &
        // bob's — because the module resolves the creator from mallory's
        // OWN key, not from anything the payload claims.
        module
            .execute(
                &mut ctx_with_origin(10, user(3))
                    .on_query("identity", identity_stub(pairs.clone())),
                &module_msg(ChatMsg::CreateDmChannel {
                    counterpart: bob_number,
                    name: "not alice and bob".into(),
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        let minted = dm_channel_id(&mallory_number.to_string(), &bob_number.to_string());
        assert_ne!(
            minted, their_dm,
            "mallory must land in her own DM, never alice's"
        );
        let ChatReply::Channel(None) = query(
            &module,
            ChatQuery::Channel {
                channel_id: their_dm,
            },
        )
        .await
        else {
            panic!("alice & bob's DM must not exist — mallory never touched it");
        };
    });
}

#[test]
fn a_participant_opens_their_derived_dm_and_both_get_seated() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat").with_identity("identity");
        let alice_number = 1u64;
        let bob_number = 2u64;
        let pairs = vec![(vec![1u8; 32], alice_number)];
        let expected_id = dm_channel_id(&alice_number.to_string(), &bob_number.to_string());

        module
            .execute(
                &mut ctx_with_origin(10, user(1)).on_query("identity", identity_stub(pairs)),
                &module_msg(ChatMsg::CreateDmChannel {
                    counterpart: bob_number,
                    name: "Bob".into(),
                }),
            )
            .await
            .unwrap();
        // seat both ends, exactly like `app::open_dm`'s follow-up writes.
        for byte in [1u8, 2u8] {
            module
                .execute(
                    &mut ctx_with_origin(11, user(1)),
                    &module_msg(ChatMsg::SetMembership {
                        channel_id: expected_id.clone(),
                        user: vec![byte; 32],
                        member: true,
                    }),
                )
                .await
                .unwrap();
        }
        module.commit_block().await.unwrap();

        let ChatReply::Channel(Some(channel)) = query(
            &module,
            ChatQuery::Channel {
                channel_id: expected_id.clone(),
            },
        )
        .await
        else {
            panic!("the derived DM must exist under its canonical id");
        };
        assert_eq!(channel.id, expected_id);
        assert_eq!(channel.post_policy, PostPolicy::MembersOnly);

        let alice_access = access(&module, &expected_id, 1).await;
        let bob_access = access(&module, &expected_id, 2).await;
        assert!(
            alice_access.may_post && bob_access.may_post,
            "both ends must be seated"
        );
    });
}

#[test]
fn a_non_dm_channel_id_is_unaffected_by_the_dm_reservation() {
    deterministic::Runner::default().start(|context| async move {
        // no `.with_identity` at all — a host with no identity sibling wired
        // must still create ordinary channels exactly as before.
        let mut module = chat_on!(context, "chat");
        module
            .execute(
                &mut ctx_with_origin(10, user(1)),
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
            panic!("an ordinary channel must still be created with no identity sibling");
        };
        assert_eq!(channel.id, "general");
    });
}

#[test]
fn a_creator_is_capped_and_a_different_origin_is_not() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = chat_on!(context, "chat");
        for n in 0..MAX_CHANNELS_PER_CREATOR as u64 {
            module
                .execute(
                    &mut ctx_with_origin(n, user(1)),
                    &module_msg(create_channel(&format!("room-{n}"))),
                )
                .await
                .unwrap();
        }
        let err = module
            .execute(
                &mut ctx_with_origin(MAX_CHANNELS_PER_CREATOR as u64, user(1)),
                &module_msg(create_channel("one-too-many")),
            )
            .await
            .unwrap_err();
        assert!(format!("{err:?}").contains("already have"));

        // a different origin is untouched by user(1)'s cap.
        module
            .execute(
                &mut ctx_with_origin(MAX_CHANNELS_PER_CREATOR as u64 + 1, user(2)),
                &module_msg(create_channel("someone-elses-room")),
            )
            .await
            .unwrap();
    });
}
