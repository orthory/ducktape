//! state-sync round-trip: a joiner reconstructs a byte-identical qmdb root by
//! pulling a source store's operation range through commonware's qmdb sync,
//! then wraps a fresh `TaggingModule` around the injected store — the same
//! discriminating property the rest of the store-backed family proves, over
//! the per-scope subscription records.
//!
//! the source subscribes two modules to one scope (record insert + record
//! overwrite), subscribes and fully unsubscribes a second scope (the op log
//! carries a record DELETE), so the joiner must reconstruct both the live
//! record and the absence.

use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use sdk::{Env, MerkleStore as _, Module, Msg, Origin, StateRoot};
use sdk_testkit::TestCtx;
use statesync::qmdb::QmdbStore;
use tagging::{TaggingModule, TaggingMsg, encode_msg};

fn from_module(id: &str, height: u64) -> TestCtx {
    TestCtx::with_env(Env {
        height,
        consensus_time: height,
        origin: Origin::Module(id.into()),
        me: "tagging".into(),
    })
    .with_module_root("chat", StateRoot([7u8; 32]))
    .with_module_root("pages", StateRoot([8u8; 32]))
}

fn op(m: &TaggingMsg) -> Msg {
    Msg {
        target: "tagging".into(),
        payload: encode_msg(m),
    }
}

fn subscribe(source: &str, container: &str) -> TaggingMsg {
    TaggingMsg::Subscribe {
        source: source.into(),
        container: container.into(),
    }
}

fn unsubscribe(source: &str, container: &str) -> TaggingMsg {
    TaggingMsg::Unsubscribe {
        source: source.into(),
        container: container.into(),
    }
}

async fn apply_commit(m: &mut TaggingModule, subscriber: &str, height: u64, msg: TaggingMsg) {
    let mut c = from_module(subscriber, height);
    m.execute(&mut c, &op(&msg)).await.unwrap();
    m.commit_block().await.unwrap();
}

/// tagging has no query surface — the observable state is the ROOT plus the
/// routing decisions the parity proof pins. the round trip therefore asserts
/// the root linkage and the store-level record set (via a probe write whose
/// acceptance depends on the synced records).
fn tagging_over(store: Box<dyn sdk::MerkleStore>) -> TaggingModule {
    TaggingModule::new("tagging", store).with_direct_owner("runs")
}

#[test]
fn synced_store_reconstructs_source_root_and_subscriptions() {
    deterministic::Runner::default().start(|context| async move {
        // SOURCE: an empty store — tagging carries no genesis config.
        let src_store = QmdbStore::init(context.child("src"), "src").await;
        let genesis_root = src_store.root();
        let mut src = tagging_over(Box::new(src_store));

        // two subscribers on one scope (insert + overwrite), and a second
        // scope subscribed then fully unsubscribed (record delete).
        apply_commit(&mut src, "agent", 1, subscribe("chat", "general")).await;
        apply_commit(&mut src, "runs", 2, subscribe("chat", "general")).await;
        apply_commit(&mut src, "agent", 3, subscribe("pages", "space-1")).await;
        apply_commit(&mut src, "agent", 4, unsubscribe("pages", "space-1")).await;

        let src_root: StateRoot = src.root();
        assert_ne!(src_root, genesis_root, "the ops moved the root");

        // the module consumed its store, so REOPEN the committed partitions
        // as a bare store for the handoff (drop first — one owner at a time).
        drop(src);
        let src_store = QmdbStore::init(context.child("src_serve"), "src").await;
        assert_eq!(
            src_store.root(),
            src_root,
            "reopened store must recover the committed root"
        );

        // describe the target (root + op range), THEN hand the source off as
        // the sync resolver (consumes it — order matters).
        let target = src_store.sync_boundary_target().await;
        let resolver = src_store.into_resolver();

        // JOINER: reconstruct on a FRESH context + namespace by pulling from
        // the resolver, then wrap the module around the injected store.
        let store = QmdbStore::sync_from(context.child("dst"), "dst", target, resolver)
            .await
            .expect("sync_from");
        let mut synced = tagging_over(Box::new(store));

        // THE PROPERTY: identical qmdb root — the root-hash linkage a joiner
        // needs at the boundary height.
        assert_eq!(
            synced.root(),
            src_root,
            "synced store root must equal the source root"
        );

        // the synced records DECIDE like the source's: a re-subscribe of an
        // existing subscriber stages nothing (the record arrived), while the
        // fully unsubscribed scope accepts a fresh subscriber (the delete
        // arrived) and moves the root off the boundary.
        apply_commit(&mut synced, "agent", 5, subscribe("chat", "general")).await;
        assert_eq!(
            synced.root(),
            src_root,
            "an idempotent re-subscribe against the synced record stages nothing"
        );
        apply_commit(&mut synced, "runs", 6, subscribe("pages", "space-1")).await;
        assert_ne!(
            synced.root(),
            src_root,
            "the unsubscribed scope re-subscribes as a fresh record"
        );
    });
}
