//! state-sync round trip over the REAL store: a joiner reconstructs a
//! byte-identical qmdb root by pulling the source store's operation range
//! through commonware's qmdb sync, then wraps a fresh [`DispatchModule`] around
//! the injected store — the sync lane that REPLACED this module's byte snapshot.
//!
//! the source drives ops through the real module so the op log is what a
//! validator produces, and it deliberately carries every shape a naive "export
//! live records and re-apply sorted" could not reproduce:
//!
//! * record OVERWRITES — a recipe update, and a dispatch walking
//!   AwaitingResult → AwaitingDelivery → Delivered,
//! * record DELETES — a removed recipe, and the mailbox entries the delivery
//!   sweep drains,
//! * the DELIVERED receipt whose outcome payload was dropped at delivery (the
//!   retention rule) sitting next to a still-PENDING delivery whose outcome is
//!   still in its record and whose mailbox entry still rides the cursor.
//!
//! a [`DispatchModule`] consumes its injected store, so the handoff-as-resolver
//! form is only reachable on the raw store: REOPEN the committed partitions
//! under the same id (exactly the recovery path a restarting node takes — the
//! deterministic runtime shares storage across child contexts).

use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use dispatch::{
    AdmissionPolicy, DispatchModule, DispatchMsg, DispatchQuery, DispatchReply, DispatchStatus,
    DispatchView, OutputContract, Recipe, Routing, decode_reply, encode_msg, encode_query,
};
use saga::{SagaCallback, SagaOutcome, encode_callback};
use sdk::{Env, MerkleStore as _, Module, Msg, Origin, StateRoot, StateSyncHandle};
use sdk_testkit::TestCtx;
use statesync::qmdb::QmdbStore;

const DISPATCH: &str = "dispatch";
const SAGA: &str = "saga";

fn ctx(height: u64, origin: Origin) -> TestCtx {
    TestCtx::with_env(Env {
        height,
        consensus_time: 1_000 + height,
        origin,
        me: DISPATCH.into(),
    })
}

/// drive one op through the REAL module path: execute + commit_block (one op
/// per block height), so the committed op log is what a validator produces.
async fn apply(module: &mut DispatchModule, height: u64, origin: Origin, payload: Vec<u8>) {
    let msg = Msg {
        target: DISPATCH.into(),
        payload,
    };
    module
        .execute(&mut ctx(height, origin), &msg)
        .await
        .expect("op applies");
    module.commit_block().await.expect("commit");
}

/// the saga's terminal callback for `key` — the intake that records the checked
/// outcome and enqueues the delivery.
async fn callback(module: &mut DispatchModule, height: u64, key: &str, outcome: SagaOutcome) {
    apply(
        module,
        height,
        Origin::Module(SAGA.into()),
        encode_callback(&SagaCallback {
            saga_id: format!("dispatch\x1f{key}"),
            payload: key.as_bytes().to_vec(),
            outcome,
        }),
    )
    .await;
}

fn register(recipe_id: &str, capability: &str) -> Vec<u8> {
    encode_msg(&DispatchMsg::RegisterRecipe {
        recipe_id: recipe_id.into(),
        description: "round trip".into(),
        capability: capability.into(),
        routing: Routing::Pinned(vec![7u8; 32]),
        output_contract: OutputContract::Text,
        max_attempts: 2,
        deadline_views: Some(100),
        lease_views: None,
    })
}

fn dispatch_op(dispatch_id: &str) -> Vec<u8> {
    encode_msg(&DispatchMsg::Dispatch {
        dispatch_id: dispatch_id.into(),
        recipe_id: "summarize".into(),
        payload: b"the entire input".to_vec(),
        demands: Default::default(),
        admission: AdmissionPolicy::Queue,
    })
}

async fn recipe(module: &DispatchModule, recipe_id: &str) -> Option<Recipe> {
    let reply = module
        .query(&encode_query(&DispatchQuery::Recipe {
            recipe_id: recipe_id.into(),
        }))
        .await
        .expect("recipe");
    match decode_reply(&reply).expect("decode") {
        DispatchReply::Recipe(r) => r,
        other => panic!("expected Recipe, got {other:?}"),
    }
}

async fn view(module: &DispatchModule, dispatch_id: &str) -> Option<DispatchView> {
    let reply = module
        .query(&encode_query(&DispatchQuery::Dispatch {
            receiver: "caller".into(),
            dispatch_id: dispatch_id.into(),
        }))
        .await
        .expect("dispatch");
    match decode_reply(&reply).expect("decode") {
        DispatchReply::Dispatch(v) => v,
        other => panic!("expected Dispatch, got {other:?}"),
    }
}

async fn pending(module: &DispatchModule) -> u64 {
    let reply = module
        .query(&encode_query(&DispatchQuery::PendingDeliveries))
        .await
        .expect("pending");
    match decode_reply(&reply).expect("decode") {
        DispatchReply::PendingDeliveries(n) => n,
        other => panic!("expected PendingDeliveries, got {other:?}"),
    }
}

#[test]
fn synced_store_reconstructs_source_root_and_every_read() {
    deterministic::Runner::default().start(|context| async move {
        let mut src = DispatchModule::new(
            DISPATCH,
            SAGA,
            Box::new(QmdbStore::init(context.child("src"), "src").await),
        );
        let owner = Origin::External(b"owner".to_vec());
        let caller = Origin::Module("caller".into());

        // recipes: two registrations, one update that OVERWRITES a record, one
        // removal that DELETES one.
        apply(&mut src, 1, owner.clone(), register("summarize", "alpha")).await;
        apply(&mut src, 2, owner.clone(), register("classify", "beta")).await;
        apply(
            &mut src,
            3,
            owner.clone(),
            encode_msg(&DispatchMsg::UpdateRecipe {
                recipe_id: "summarize".into(),
                description: Some("updated".into()),
                capability: None,
                routing: None,
                output_contract: None,
                max_attempts: Some(5),
            }),
        )
        .await;
        apply(
            &mut src,
            4,
            owner.clone(),
            encode_msg(&DispatchMsg::RemoveRecipe {
                recipe_id: "classify".into(),
            }),
        )
        .await;

        // two dispatches. `done` runs the FULL lifecycle and ends Delivered —
        // its outcome payload dropped, its mailbox entry deleted. `live` stops
        // at AwaitingDelivery, so its outcome IS in the record and its mailbox
        // entry rides the committed cursor.
        apply(&mut src, 5, caller.clone(), dispatch_op("done")).await;
        apply(&mut src, 6, caller.clone(), dispatch_op("live")).await;
        callback(
            &mut src,
            7,
            "caller\x1fdone",
            SagaOutcome::Done(b"the whole result".to_vec()),
        )
        .await;
        apply(
            &mut src,
            8,
            Origin::System,
            encode_msg(&DispatchMsg::DeliverPending {}),
        )
        .await;
        callback(
            &mut src,
            9,
            "caller\x1flive",
            SagaOutcome::Failed("provider died".into()),
        )
        .await;

        // the module is resolver-backed: there is NO byte snapshot to ship.
        match src.state_sync_handle().expect("handle") {
            StateSyncHandle::ResolverBacked { backend, .. } => assert_eq!(backend, "qmdb"),
            other => panic!("expected ResolverBacked, got {other:?}"),
        }
        let src_root = src.root();
        assert_ne!(src_root, StateRoot::ZERO, "source must have a real root");
        let src_summarize = recipe(&src, "summarize").await.expect("the kept recipe");
        assert_eq!(src_summarize.description, "updated");
        assert_eq!(src_summarize.max_attempts, 5);
        assert_eq!(recipe(&src, "classify").await, None, "removed at height 4");
        let src_done = view(&src, "done").await.expect("the receipt survives");
        let src_live = view(&src, "live").await.expect("the pending delivery");
        assert_eq!(pending(&src).await, 1);

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

        // JOINER: rebuild on a FRESH namespace by pulling the proven op range,
        // then wrap the module around the injected store.
        let store = QmdbStore::sync_from(context.child("dst"), "dst", target, resolver)
            .await
            .expect("sync_from");
        let synced = DispatchModule::new(DISPATCH, SAGA, Box::new(store));

        // THE PROPERTY: identical qmdb root — the root-hash linkage a joiner
        // needs at the boundary height.
        assert_eq!(
            synced.root(),
            src_root,
            "synced store root must equal the source root"
        );

        // and every read answers exactly like the source: the updated record
        // rode the sync verbatim, the removed one is still gone.
        assert_eq!(recipe(&synced, "summarize").await, Some(src_summarize));
        assert_eq!(
            recipe(&synced, "classify").await,
            None,
            "the removed recipe is gone"
        );

        // the DELIVERED receipt rode the sync with its payload dropped...
        assert_eq!(view(&synced, "done").await, Some(src_done.clone()));
        assert_eq!(src_done.status, DispatchStatus::Delivered);
        assert_eq!(
            src_done.outcome, None,
            "delivery dropped this module's copy"
        );

        // ...and the still-PENDING delivery kept its outcome AND its mailbox
        // entry, so the joiner's host will inject exactly one more delivery.
        assert_eq!(view(&synced, "live").await, Some(src_live.clone()));
        assert_eq!(src_live.status, DispatchStatus::AwaitingDelivery);
        assert_eq!(src_live.outcome, Some(Err("provider died".into())));
        assert_eq!(pending(&synced).await, 1);

        assert_eq!(view(&synced, "never-dispatched").await, None);
    });
}
