//! state-sync round trip over the REAL store: a joiner reconstructs a
//! byte-identical qmdb root by pulling the source store's operation range
//! through commonware's qmdb sync, then wraps a fresh `Collaboration` around
//! the injected store.
//!
//! The source drives ops through the real module, so the op log is what a
//! validator produces, and it deliberately carries every shape a naive
//! "export live records and re-apply" could not reproduce: record OVERWRITES
//! (a roster edit, a binding replacement, a receipt advancing), record DELETES
//! (a prune retiring an event, a message, a receipt and a dedup record) and
//! the counter keys — mailbox accounting and the replay floor — that ride the
//! same root.

use collaboration::{
    encode_msg, encode_query, Collaboration, CollaborationMsg, CollaborationQuery,
    CollaborationReply, DeliveryState, EventPage, MessageId, MessageKind, ProtectedRead, Role,
    SendRequest, SendState,
};
use commonware_runtime::{deterministic, Runner as _, Supervisor as _};
use sdk::{Cause, Env, MerkleStore as _, Module, Msg, Origin, StateRoot, StateSyncHandle};
use sdk_testkit::TestCtx;
use statesync::qmdb::QmdbStore;

const MODULE: &str = "collaboration";
const TTL: u64 = collaboration::HEIGHT_LANE_MAX_DELIVERY_TTL;

fn ctx(height: u64, origin: Origin) -> TestCtx {
    TestCtx::with_env(Env {
        height,
        consensus_time: height,
        origin,
        me: MODULE.into(),
        cause: Cause::Direct,
    })
    .on_query("identity", |_| {
        Ok(identity::encode_reply(&identity::IdentityReply::Account(
            None,
        )))
    })
    .on_query("tasks", |_| {
        Ok(tasks::encode_job_reply(&tasks::JobsReply::Job(None)))
    })
}

fn ext(byte: u8) -> Origin {
    Origin::External(vec![byte; 32])
}

async fn apply(module: &mut Collaboration, height: u64, origin: Origin, payload: CollaborationMsg) {
    let msg = Msg {
        target: MODULE.into(),
        payload: encode_msg(&payload),
    };
    module
        .execute(&mut ctx(height, origin), &msg)
        .await
        .expect("the op is admitted");
    module.commit_block().await.expect("commit");
}

/// read as the owner of `participant`, whose key is `byte`.
async fn read(
    module: &Collaboration,
    byte: u8,
    participant: &str,
    read: ProtectedRead,
) -> CollaborationReply {
    let request = encode_query(&CollaborationQuery::Read {
        participant_id: participant.into(),
        via: None,
        read,
    });
    let bytes = module
        .query_with(&ctx(99, ext(byte)), &request)
        .await
        .expect("a read answers");
    collaboration::decode_reply(&bytes).expect("decode")
}

fn note(sequence: u64, expires_at: u64) -> CollaborationMsg {
    CollaborationMsg::Send(SendRequest {
        conversation_id: "c1".into(),
        sender_participant_id: "alice".into(),
        message_id: MessageId {
            generation: 1,
            sequence,
        },
        recipient_participant_id: "bob".into(),
        kind: MessageKind::Notice,
        reply_to: None,
        task: None,
        body: format!("message {sequence}"),
        references: Vec::new(),
        expires_at,
    })
}

#[test]
fn synced_store_reconstructs_source_root_and_every_read() {
    deterministic::Runner::default().start(|context| async move {
        let mut src = Collaboration::new(
            MODULE,
            "identity",
            "tasks",
            Box::new(QmdbStore::init(context.child("src"), "src").await),
            TTL,
        );

        // registry: two participants under two keys, one conversation, both
        // seated (roster OVERWRITES the conversation record twice).
        apply(
            &mut src,
            1,
            ext(1),
            CollaborationMsg::RegisterParticipant {
                participant_id: "alice".into(),
                display_name: "alice".into(),
                agent_account: None,
            },
        )
        .await;
        apply(
            &mut src,
            1,
            ext(2),
            CollaborationMsg::RegisterParticipant {
                participant_id: "bob".into(),
                display_name: "bob".into(),
                agent_account: None,
            },
        )
        .await;
        apply(
            &mut src,
            2,
            ext(1),
            CollaborationMsg::CreateConversation {
                conversation_id: "c1".into(),
                topic: "review".into(),
            },
        )
        .await;
        for who in ["alice", "bob"] {
            apply(
                &mut src,
                3,
                ext(1),
                CollaborationMsg::SetRoster {
                    conversation_id: "c1".into(),
                    participant_id: who.into(),
                    role: Some(Role::Member),
                },
            )
            .await;
        }

        // a binding and its REPLACEMENT: the binding record is overwritten and
        // the participant's credential allocator moves with it.
        apply(
            &mut src,
            4,
            ext(2),
            CollaborationMsg::Bind {
                conversation_id: "c1".into(),
                participant_id: "bob".into(),
                device: "laptop".into(),
                service_key: vec![20; 32],
                expected_credential: 0,
            },
        )
        .await;
        apply(
            &mut src,
            5,
            ext(2),
            CollaborationMsg::Bind {
                conversation_id: "c1".into(),
                participant_id: "bob".into(),
                device: "desktop".into(),
                service_key: vec![21; 32],
                expected_credential: 2,
            },
        )
        .await;

        // two admissions: one that will be retired, one that survives.
        apply(&mut src, 6, ext(1), note(1, 100)).await;
        apply(&mut src, 7, ext(1), note(2, 1_000)).await;

        // the first settles (receipt OVERWRITE + mailbox accounting release)
        // and is then pruned (event, message, receipt and dedup DELETES, and
        // the replay floor counter rises).
        // derived, never counted by hand: every committed change above took a
        // sequence, so the admission's own sequence is what the module says.
        let CollaborationReply::SendState(SendState::Admitted { seq: first_seq, .. }) = read(
            &src,
            1,
            "alice",
            ProtectedRead::SendState {
                generation: 1,
                sequence: 1,
            },
        )
        .await
        else {
            panic!("the first send was admitted");
        };
        apply(
            &mut src,
            101,
            ext(9),
            CollaborationMsg::ExpireMessage {
                conversation_id: "c1".into(),
                seq: first_seq,
            },
        )
        .await;
        apply(
            &mut src,
            102,
            ext(1),
            CollaborationMsg::Prune {
                conversation_id: "c1".into(),
                through_seq: first_seq + 1,
            },
        )
        .await;

        // the module is resolver-backed: there is NO byte snapshot to ship.
        match src.state_sync_handle().expect("handle") {
            StateSyncHandle::ResolverBacked { backend, .. } => assert_eq!(backend, "qmdb"),
            other => panic!("expected ResolverBacked, got {other:?}"),
        }
        let src_root = src.root();
        assert_ne!(src_root, StateRoot::ZERO, "source must have a real root");

        let src_events = read(
            &src,
            2,
            "bob",
            ProtectedRead::Events {
                conversation_id: "c1".into(),
                from_seq: first_seq + 1,
                limit: 64,
            },
        )
        .await;
        let src_mailbox = read(&src, 2, "bob", ProtectedRead::Mailbox).await;
        let src_binding = read(
            &src,
            2,
            "bob",
            ProtectedRead::Binding {
                conversation_id: "c1".into(),
            },
        )
        .await;

        // the module consumed its store, so REOPEN the committed partitions as
        // a bare store for the handoff (drop first — one owner at a time).
        drop(src);
        let src_store = QmdbStore::init(context.child("src_serve"), "src").await;
        assert_eq!(
            src_store.root(),
            src_root,
            "reopened store must recover the committed root"
        );
        let target = src_store.sync_boundary_target().await;
        let resolver = src_store.into_resolver();

        // JOINER: rebuild on a FRESH namespace by pulling the proven op range.
        let store = QmdbStore::sync_from(context.child("dst"), "dst", target, resolver)
            .await
            .expect("sync_from");
        let mut synced = Collaboration::new(MODULE, "identity", "tasks", Box::new(store), TTL);

        assert_eq!(
            synced.root(),
            src_root,
            "synced store root must equal the source root"
        );

        // every read answers exactly like the source.
        assert_eq!(
            read(
                &synced,
                2,
                "bob",
                ProtectedRead::Events {
                    conversation_id: "c1".into(),
                    from_seq: first_seq + 1,
                    limit: 64,
                }
            )
            .await,
            src_events
        );
        assert_eq!(read(&synced, 2, "bob", ProtectedRead::Mailbox).await, src_mailbox);
        assert_eq!(
            read(
                &synced,
                2,
                "bob",
                ProtectedRead::Binding {
                    conversation_id: "c1".into()
                }
            )
            .await,
            src_binding
        );

        // the retired sequence's dedup record is gone and the replay FLOOR
        // counter survived the sync: a retry is still refused on the joiner.
        let CollaborationReply::SendState(state) = read(
            &synced,
            1,
            "alice",
            ProtectedRead::SendState {
                generation: 1,
                sequence: 1,
            },
        )
        .await
        else {
            panic!("send state")
        };
        assert_eq!(
            state,
            SendState::ReceiptPruned,
            "the replay floor rode the sync, so a pruned sequence stays refused"
        );

        // the surviving message is readable and still charged to the mailbox.
        let CollaborationReply::Events(EventPage::Page { messages, .. }) = read(
            &synced,
            2,
            "bob",
            ProtectedRead::Events {
                conversation_id: "c1".into(),
                from_seq: first_seq + 1,
                limit: 64,
            },
        )
        .await
        else {
            panic!("an event page")
        };
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].message_id.sequence, 2);

        // and the joiner keeps writing: the replaced binding's credential is
        // the one that acknowledges.
        let mut write_ctx = ctx(103, Origin::External(vec![21; 32]));
        synced
            .execute(
                &mut write_ctx,
                &Msg {
                    target: MODULE.into(),
                    payload: encode_msg(&CollaborationMsg::Acknowledge {
                        conversation_id: "c1".into(),
                        seq: messages[0].seq,
                        binding_credential: 3,
                        state: DeliveryState::Queued,
                        reason: None,
                    }),
                },
            )
            .await
            .expect("the current binding acknowledges after a sync");
        assert!(
            write_ctx.output().is_some(),
            "the op reports the sequence it advanced the conversation to"
        );
    });
}
