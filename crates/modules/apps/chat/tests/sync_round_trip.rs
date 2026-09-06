//! state-sync round-trip: a fresh `Chat` reconstructs a byte-identical storage
//! root by pulling a source store's operation range through commonware's qmdb
//! sync. this is the storage-backed joiner surface; replaying exported records
//! in sorted order is not enough because the qmdb root commits to the live
//! operation log. the source content covers every record family: a channel
//! record, message heads (a thread root and its reply), an edit revision,
//! reaction sets, and a membership point record.
//!
//! the source is driven through the real module so the op log carries genuine
//! chat history (edits, deletes-by-overwrite, index churn). the
//! handoff-as-resolver form is only reachable on the raw store and a `Chat`
//! consumes its injected store, so the source module is dropped and its
//! partitions REOPENED as a bare `QmdbStore` — recovery of a committed store
//! lands exactly on the committed root, which the test pins before handing
//! the reopened store to the joiner. query parity is asserted over the three
//! kept dispatch reads (`Channel` / `MessagesRange` / `Message`); every other
//! record family (revisions, reactions, membership) is pinned by the root
//! equality, which commits to the full op log.

use chat::Chat;
use chat::{Block, ChatMsg, ChatQuery, PostPolicy, encode_msg, encode_query};
use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use sdk::{MerkleStore as _, Module, Msg, Origin, StateRoot};
use statesync::qmdb::QmdbStore;

use sdk_testkit::TestCtx;

// chat's sync tests read only env (consensus_time); me/height are cosmetic.
fn ctx_at(consensus_time: u64) -> TestCtx {
    TestCtx::with_env(sdk::Env {
        height: 0,
        consensus_time,
        origin: Origin::System,
        me: "chat".into(),
        cause: sdk::Cause::Direct,
    })
}

fn module_msg(payload: ChatMsg) -> Msg {
    Msg {
        target: "chat".into(),
        payload: encode_msg(&payload),
    }
}

async fn apply_commit(module: &mut Chat, at: u64, payload: ChatMsg) {
    module
        .execute(&mut ctx_at(at), &module_msg(payload))
        .await
        .unwrap();
    module.commit_block().await.unwrap();
}

async fn reply_bytes(module: &Chat, query: ChatQuery) -> Vec<u8> {
    module.query(&encode_query(&query)).await.unwrap()
}

#[test]
fn synced_store_reconstructs_source_root_and_history() {
    deterministic::Runner::default().start(|context| async move {
        let mut src = Chat::new(
            "src",
            Box::new(QmdbStore::init(context.child("src"), "src").await),
        )
        .with_attribution("attribution");
        apply_commit(
            &mut src,
            10,
            ChatMsg::CreateChannel {
                channel_id: "general".into(),
                name: "General".into(),
                post_policy: PostPolicy::Open,
            },
        )
        .await;
        apply_commit(
            &mut src,
            20,
            ChatMsg::PostMessage {
                channel_id: "general".into(),
                message_id: "m1".into(),
                blocks: vec![Block::paragraph("draft")],
                thread: None,
            },
        )
        .await;
        apply_commit(
            &mut src,
            21,
            ChatMsg::PostMessage {
                channel_id: "general".into(),
                message_id: "m2".into(),
                blocks: vec![Block::paragraph("final")],
                thread: None,
            },
        )
        .await;
        apply_commit(
            &mut src,
            22,
            ChatMsg::PostMessage {
                channel_id: "general".into(),
                message_id: "r1".into(),
                blocks: vec![Block::paragraph("sync the thread too")],
                thread: Some(1),
            },
        )
        .await;
        // an overwrite of the head record (edit) plus its revision record —
        // op-log order matters, so only the real sync path reproduces it.
        apply_commit(
            &mut src,
            23,
            ChatMsg::EditMessage {
                channel_id: "general".into(),
                seq: 2,
                blocks: vec![Block::paragraph("final, edited")],
                base_rev: Some(0),
            },
        )
        .await;
        apply_commit(
            &mut src,
            24,
            ChatMsg::AddReaction {
                channel_id: "general".into(),
                seq: 2,
                emoji: "ship".into(),
            },
        )
        .await;
        apply_commit(
            &mut src,
            25,
            ChatMsg::SetMembership {
                channel_id: "general".into(),
                party: chat::Party::Key(vec![7; 32]),
                member: true,
            },
        )
        .await;
        let src_root = src.root();
        assert_ne!(src_root, StateRoot::ZERO, "source must have a real root");

        let queries = [
            ChatQuery::Channel {
                channel_id: "general".into(),
            },
            ChatQuery::MessagesRange {
                channel_id: "general".into(),
                from_seq: 1,
                limit: 16,
            },
            ChatQuery::Message {
                message_id: "m2".into(),
            },
            ChatQuery::Message {
                message_id: "r1".into(),
            },
        ];
        let mut expected = Vec::new();
        for query in &queries {
            expected.push(reply_bytes(&src, query.clone()).await);
        }
        // the module consumed its injected store, so the handoff-as-resolver
        // form needs the raw store back: drop the module and reopen the same
        // "src" partitions (deterministic storage is keyed by partition name,
        // not context label). recovery must land exactly on the committed root
        // — pinned here before the joiner trusts the reopened store as source.
        drop(src);
        let store = QmdbStore::init(context.child("src_serve"), "src").await;
        assert_eq!(
            store.root(),
            src_root,
            "reopened source store must recover the committed root"
        );
        let target = store.sync_boundary_target().await;
        let resolver = store.into_resolver();

        // JOINER: reconstruct on a fresh namespace by pulling from the
        // resolver, then wrap the module around the injected store — the exact
        // shape a joining host uses.
        let synced = Chat::new(
            "dst",
            Box::new(
                QmdbStore::sync_from(context.child("dst"), "dst", target, resolver)
                    .await
                    .expect("sync_from"),
            ),
        );

        assert_eq!(
            synced.root(),
            src_root,
            "synced store root must equal source root"
        );
        for (query, expected) in queries.iter().zip(&expected) {
            assert_eq!(
                &reply_bytes(&synced, query.clone()).await,
                expected,
                "synced reply must match source for {query:?}"
            );
        }
    });
}
