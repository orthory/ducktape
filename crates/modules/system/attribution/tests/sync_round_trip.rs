//! state-sync round-trip: a joiner reconstructs a byte-identical qmdb root by
//! pulling a source store's operation range through commonware's qmdb sync,
//! then wraps a fresh `AttributionModule` around the injected store — the
//! same discriminating property the rest of the store-backed family proves,
//! over the relation, change and index records.
//!
//! the source records an object's first relations, an edit that withdraws
//! one, a second object, and that object's deletion (an empty report), so
//! the joiner must reconstruct live relations, an emptied set that still
//! pins its revision, and the whole change history behind both — and, with
//! two subscribers wired in, the delivery queue behind every change: the
//! retired receipts (one applied, one failed with its reason), the committed
//! pending head the host reads, and the causal chain a change recorded under
//! a delivery carries.

use attribution::{
    Actor, AttributionModule, AttributionMsg, AttributionQuery, AttributionReply, ObjectRef,
    Reason, Relation, Source, decode_reply, encode_msg, encode_query,
};
use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use sdk::{
    Ack, Cause, DeliveryOutcome, Env, Hop, ItemRef, MerkleStore as _, Module, Msg, Origin, Root,
    StateRoot,
};
use sdk_testkit::TestCtx;
use statesync::qmdb::QmdbStore;

const ALICE: u64 = 7;
const BOB: u64 = 9;

/// the genesis subscribers the node's topology wires (the same constant the
/// guest shell carries).
const SUBSCRIBERS: [&str; 2] = ["inbox", "agent"];

fn with_cause(origin: Origin, height: u64, cause: Cause) -> TestCtx {
    TestCtx::with_env(Env {
        height,
        consensus_time: height,
        origin,
        me: "attribution".into(),
        cause,
    })
}

fn from_module(id: &str, height: u64) -> TestCtx {
    with_cause(Origin::Module(id.into()), height, Cause::Direct)
}

/// the causal context of a report the host ran as a delivery of another
/// source's queued item.
fn delivered_from(source: &str, item: u64) -> Cause {
    let hop = ItemRef {
        source: source.into(),
        item,
    };
    Cause::Chain {
        root: Root::Item(hop.clone()),
        hop: Hop::Delivery(hop),
    }
}

async fn retire(
    m: &mut AttributionModule,
    height: u64,
    item: u64,
    target: &str,
    outcome: DeliveryOutcome,
) {
    let mut host = with_cause(Origin::System, height, Cause::Direct);
    m.acknowledge(
        &mut host,
        &Ack {
            item,
            target: target.into(),
            outcome,
        },
    )
    .await
    .unwrap();
}

fn report(kind: &str, object: &str, revision: u64, recipients: &[(u64, Reason)]) -> Msg {
    let msg = AttributionMsg::Attribute {
        object: ObjectRef {
            kind: kind.into(),
            object: object.into(),
        },
        revision,
        actor: Actor::Account(ALICE),
        relations: recipients
            .iter()
            .map(|(recipient, reason)| Relation {
                recipient: *recipient,
                reason: reason.clone(),
                detail: Vec::new(),
            })
            .collect(),
        transfers: Vec::new(),
    };
    Msg {
        target: "attribution".into(),
        payload: encode_msg(&msg),
    }
}

async fn apply_commit(m: &mut AttributionModule, source: &str, height: u64, msg: Msg) {
    let mut c = from_module(source, height);
    m.execute(&mut c, &msg).await.unwrap();
    m.commit_block().await.unwrap();
}

async fn reply(m: &AttributionModule, q: &AttributionQuery) -> AttributionReply {
    decode_reply(&m.query(&encode_query(q)).await.unwrap()).unwrap()
}

fn source(module: &str, kind: &str, object: &str) -> Source {
    Source {
        module: module.into(),
        kind: kind.into(),
        object: object.into(),
    }
}

#[test]
fn synced_store_reconstructs_source_root_relations_and_history() {
    deterministic::Runner::default().start(|context| async move {
        // SOURCE: an empty store — attribution carries no genesis config.
        let src_store = QmdbStore::init(context.child("src"), "src").await;
        let genesis_root = src_store.root();
        let mut src = AttributionModule::new("attribution", Box::new(src_store))
            .with_subscribers(SUBSCRIBERS);

        // a message with an author and a mention, then an edit that drops
        // the mention; a comment that is then deleted (an empty report).
        apply_commit(
            &mut src,
            "chat",
            1,
            report(
                "message",
                "m1",
                1,
                &[(ALICE, Reason::Authorship), (BOB, Reason::Mention)],
            ),
        )
        .await;
        apply_commit(
            &mut src,
            "chat",
            2,
            report("message", "m1", 2, &[(ALICE, Reason::Authorship)]),
        )
        .await;
        apply_commit(
            &mut src,
            "pages",
            3,
            report("comment", "c1", 1, &[(BOB, Reason::Authorship)]),
        )
        .await;
        apply_commit(&mut src, "pages", 4, report("comment", "c1", 2, &[])).await;

        // a report recorded while the host delivers another source's item:
        // its change carries that chain, and so does every delivery of it.
        let mut delivered = with_cause(Origin::Module("runs".into()), 5, delivered_from("saga", 3));
        src.execute(
            &mut delivered,
            &report("run", "r1", 1, &[(ALICE, Reason::Result)]),
        )
        .await
        .unwrap();
        src.commit_block().await.unwrap();

        // the host retires the first two deliveries: one applied, one failed
        // with the subscriber's reason. the receipts are state like any other.
        retire(&mut src, 6, 1, "agent", DeliveryOutcome::Applied).await;
        retire(
            &mut src,
            6,
            2,
            "inbox",
            DeliveryOutcome::Failed {
                reason: "recipient account does not exist".into(),
            },
        )
        .await;
        src.commit_block().await.unwrap();

        let src_root: StateRoot = src.root();
        assert_ne!(src_root, genesis_root, "the reports moved the root");
        let src_pending = src.pending_items().await.unwrap();
        assert_eq!(
            src_pending.first().map(|item| item.item),
            Some(3),
            "the committed head follows the two retired items"
        );
        let inbox_deliveries = reply(
            &src,
            &AttributionQuery::DeliveriesOf {
                subscriber: "inbox".into(),
                after: 0,
                limit: u64::MAX,
            },
        )
        .await;
        let failed_receipt = reply(
            &src,
            &AttributionQuery::DeliveryOf {
                subscriber: "inbox".into(),
                seq: 1,
            },
        )
        .await;
        let subscribers = reply(&src, &AttributionQuery::Subscribers).await;
        let bobs_history = reply(
            &src,
            &AttributionQuery::ChangesFor {
                recipient: BOB,
                after: 0,
                limit: u64::MAX,
            },
        )
        .await;
        let comment_view = reply(
            &src,
            &AttributionQuery::Relations {
                source: source("pages", "comment", "c1"),
            },
        )
        .await;

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
        let mut synced =
            AttributionModule::new("attribution", Box::new(store)).with_subscribers(SUBSCRIBERS);

        // THE PROPERTY: identical qmdb root — the root-hash linkage a joiner
        // needs at the boundary height.
        assert_eq!(
            synced.root(),
            src_root,
            "synced store root must equal the source root"
        );

        // the synced records READ like the source's: the whole per-recipient
        // history (added, withdrawn across two objects) and the emptied
        // comment that still pins its final revision.
        assert_eq!(
            reply(
                &synced,
                &AttributionQuery::ChangesFor {
                    recipient: BOB,
                    after: 0,
                    limit: u64::MAX,
                },
            )
            .await,
            bobs_history
        );
        assert_eq!(
            reply(
                &synced,
                &AttributionQuery::Relations {
                    source: source("pages", "comment", "c1"),
                },
            )
            .await,
            comment_view
        );

        // the queue reads like the source's too: the same committed pending
        // head (items, targets, payloads and causes), the same receipts with
        // the failed reason, the same wired subscribers, and the delivered
        // report's change still carries its chain.
        assert_eq!(synced.pending_items().await.unwrap(), src_pending);
        assert_eq!(
            reply(
                &synced,
                &AttributionQuery::DeliveriesOf {
                    subscriber: "inbox".into(),
                    after: 0,
                    limit: u64::MAX,
                },
            )
            .await,
            inbox_deliveries
        );
        assert_eq!(
            reply(
                &synced,
                &AttributionQuery::DeliveryOf {
                    subscriber: "inbox".into(),
                    seq: 1,
                },
            )
            .await,
            failed_receipt
        );
        assert_eq!(
            subscribers,
            reply(&synced, &AttributionQuery::Subscribers).await
        );
        let AttributionReply::Changes(runs) = reply(
            &synced,
            &AttributionQuery::ChangesOf {
                source: source("runs", "run", "r1"),
                after: 0,
                limit: u64::MAX,
            },
        )
        .await
        else {
            panic!("a changes reply");
        };
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].change.cause, delivered_from("saga", 3));

        // and the queue DECIDES like the source's: the head retires (an exact
        // repeat of a retired item is a no-op), the next head is what the
        // host reads, and nothing but the head is acknowledgable.
        retire(&mut synced, 7, 1, "agent", DeliveryOutcome::Applied).await;
        synced.commit_block().await.unwrap();
        assert_eq!(
            synced.root(),
            src_root,
            "a repeated acknowledgment stages nothing"
        );
        retire(&mut synced, 7, 3, "agent", DeliveryOutcome::Applied).await;
        synced.commit_block().await.unwrap();
        let after_retire = synced.root();
        assert_ne!(after_retire, src_root);
        assert_eq!(
            synced
                .pending_items()
                .await
                .unwrap()
                .first()
                .map(|item| item.item),
            Some(4)
        );

        // and they DECIDE like the source's: a stale revision of the synced
        // comment is a conflict, while its next revision is accepted and
        // moves the root off the boundary.
        let mut stale = from_module("pages", 8);
        assert!(
            synced
                .execute(
                    &mut stale,
                    &report("comment", "c1", 2, &[(BOB, Reason::Authorship)])
                )
                .await
                .is_err(),
            "a replayed revision conflicts against the synced record"
        );
        synced.abort_block().await.unwrap();
        assert_eq!(synced.root(), after_retire);
        apply_commit(
            &mut synced,
            "pages",
            8,
            report("comment", "c1", 3, &[(BOB, Reason::Authorship)]),
        )
        .await;
        assert_ne!(
            synced.root(),
            after_retire,
            "the re-added relation is a fresh change on the synced store"
        );
    });
}
