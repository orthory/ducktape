//! state-sync round trip over the REAL store: a joiner reconstructs a
//! byte-identical qmdb root by pulling the source store's operation range
//! through commonware's qmdb sync, then wraps a fresh `Collaboration` around
//! the injected store.
//!
//! The source drives ops through the real module, so the op log is what a
//! validator produces, and it deliberately carries every shape a naive
//! "export live records and re-apply" could not reproduce: record OVERWRITES
//! (a binding replacement, a delivery advancing) and the counter keys — the
//! event head, mailbox accounting and the per-sender quota — that ride the
//! same root.

mod common;

use collaboration::{
    Collaboration, CollaborationMsg, CollaborationReply, DeliveryState, ProtectedRead,
    encode_msg,
};
use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use common::*;
use sdk::{MerkleStore as _, Module, Msg, Origin, StateRoot, StateSyncHandle};
use statesync::qmdb::QmdbStore;

fn ext(byte: u8) -> Origin {
    Origin::External(key(byte))
}

async fn drive(module: &mut Collaboration, chat: &Chat, height: u64, origin: Origin, payload: CollaborationMsg) {
    let mut ctx = at(chat, height, origin);
    ok(module, &mut ctx, payload).await;
}

#[test]
fn synced_store_reconstructs_source_root_and_every_read() {
    deterministic::Runner::default().start(|context| async move {
        let scene = scene("c1");
        let (chat, alice, bob) = (scene.chat.clone(), scene.alice.clone(), scene.bob.clone());
        let mut src = Collaboration::new(
            MODULE,
            IDENTITY,
            TASKS,
            CHAT,
            Box::new(QmdbStore::init(context.child("src"), "src").await),
            MAX_TTL,
            NETWORK,
        );

        // a binding and its REPLACEMENT: the binding record is overwritten.
        drive(&mut src, &chat, 4, ext(2), bind("c1", &bob, key(20), 0)).await;
        drive(&mut src, &chat, 5, ext(2), bind("c1", &bob, key(21), 1)).await;

        // two deliveries: one that settles by expiry, one that advances.
        let first = scene.alice_posts("c1", "m1");
        let second = scene.alice_posts("c1", "m2");
        drive(
            &mut src,
            &chat,
            6,
            ext(1),
            CollaborationMsg::Deliver(deliver("c1", "m1", &bob, 100)),
        )
        .await;
        drive(
            &mut src,
            &chat,
            7,
            ext(1),
            CollaborationMsg::Deliver(deliver("c1", "m2", &bob, 1_000)),
        )
        .await;
        drive(
            &mut src,
            &chat,
            101,
            ext(9),
            CollaborationMsg::ExpireMessage {
                channel_id: "c1".into(),
                seq: first,
                recipient: bob.clone(),
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

        let reads = |channel: &str| {
            vec![
                ProtectedRead::Events {
                    channel_id: channel.into(),
                    from_seq: 0,
                    limit: 64,
                },
                ProtectedRead::Mailbox,
                ProtectedRead::Binding {
                    channel_id: channel.into(),
                },
                ProtectedRead::Delivery {
                    channel_id: channel.into(),
                    seq: second,
                },
            ]
        };
        let bob_ctx = at(&chat, 102, ext(2));
        let mut src_answers = Vec::new();
        for probe in reads("c1") {
            src_answers.push(read(&src, &bob_ctx, &bob, None, probe).await);
        }

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
        let mut synced = Collaboration::new(
            MODULE,
            IDENTITY,
            TASKS,
            CHAT,
            Box::new(store),
            MAX_TTL,
            NETWORK,
        );

        assert_eq!(
            synced.root(),
            src_root,
            "synced store root must equal the source root"
        );

        // every read answers exactly like the source.
        for (probe, expected) in reads("c1").into_iter().zip(src_answers) {
            assert_eq!(read(&synced, &bob_ctx, &bob, None, probe).await, expected);
        }
        let CollaborationReply::Delivery(Some(delivery)) = read(
            &synced,
            &bob_ctx,
            &bob,
            None,
            ProtectedRead::Delivery {
                channel_id: "c1".into(),
                seq: first,
            },
        )
        .await
        else {
            panic!("the expired record survives the sync");
        };
        assert_eq!(delivery.state, DeliveryState::Expired);

        // and the joiner keeps writing: the replaced binding's credential is
        // the one that acknowledges.
        let mut write_ctx = at(&chat, 103, ext(21));
        synced
            .execute(
                &mut write_ctx,
                &Msg {
                    target: MODULE.into(),
                    payload: encode_msg(&collaboration::Request::new(
                        NETWORK,
                        CollaborationMsg::Acknowledge {
                            channel_id: "c1".into(),
                            seq: second,
                            recipient: bob.clone(),
                            binding_credential: 2,
                            state: DeliveryState::Queued,
                            reason: None,
                        },
                    )),
                },
            )
            .await
            .expect("the current binding acknowledges after a sync");
        assert!(
            write_ctx.output().is_some(),
            "the op reports the sequence it advanced the channel to"
        );
        let _ = alice;
    });
}
