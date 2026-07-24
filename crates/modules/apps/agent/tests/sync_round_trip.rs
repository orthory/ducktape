//! state-sync round-trip: a joiner reconstructs a byte-identical qmdb root by
//! pulling a source store's operation range through commonware's qmdb sync,
//! then wraps a fresh `AgentModule` around the injected store — the same
//! discriminating property chat and pages prove, over the registry layout.
//!
//! the source registers agents covering both owner origin shapes and both
//! statuses, UPDATES a record (key overwrite) and pauses another, so the op
//! log carries overwrites a naive "export live records and re-apply sorted"
//! could never reproduce — the qmdb root is operation-log ordered. only a
//! real sync that ships the ACTUAL proven op range lands on the same root.
//!
//! the roster (the registry's one enumeration record, consensus-consumed by
//! runs' engagement domain) is ordinary store state under a reserved key, so
//! it syncs like any record — the joiner answers the full listing exactly
//! like the source.

use agent::AgentModule;
use agent::{
    ACTION_CHAT_POST, ACTION_TASKS_CREATE, AgentMsg, AgentQuery, AgentReply, AgentStatus,
    decode_reply, encode_msg, encode_query,
};
use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use saga::SagaOrigin;
use sdk::{Env, MerkleStore as _, Module, Msg, Origin, StateRoot};
use sdk_testkit::TestCtx;
use statesync::qmdb::QmdbStore;

/// drives `execute` with a controllable env; the registry queries nothing.
fn ctx(height: u64, origin: Origin) -> TestCtx {
    TestCtx::with_env(Env {
        height,
        consensus_time: height,
        origin,
        me: "agent".into(),
    })
}

fn user(byte: u8) -> Origin {
    Origin::External(vec![byte; 32])
}

fn register(agent_id: &str, actions: &[&str]) -> AgentMsg {
    AgentMsg::RegisterAgent {
        agent_id: agent_id.into(),
        display_name: agent_id.to_uppercase(),
        capability: "model-1".into(),
        allowed_actions: actions.iter().map(|s| s.to_string()).collect(),
        recipe_hash: None,
        caps: None,
        skills: None,
    }
}

// drive one op through the REAL module path: execute + commit_block (one op
// per block-height), so the committed op log is what a validator produces.
async fn apply_commit(m: &mut AgentModule, height: u64, origin: Origin, op: &AgentMsg) {
    let msg = Msg {
        target: "agent".into(),
        payload: encode_msg(op),
    };
    m.execute(&mut ctx(height, origin), &msg).await.unwrap();
    m.commit_block().await.unwrap();
}

async fn query_reply(m: &AgentModule, q: &AgentQuery) -> AgentReply {
    decode_reply(&m.query(&encode_query(q)).await.unwrap()).unwrap()
}

#[test]
fn synced_store_reconstructs_source_root_and_registry() {
    deterministic::Runner::default().start(|context| async move {
        // SOURCE: build the registry through the real op path — an external
        // and a module owner, an update (overwrite) and a pause (overwrite),
        // so the op log carries more than inserts. hook None: this proof is
        // about the store lane, not the recipe plane.
        let mut src = AgentModule::new(
            "agent",
            Box::new(QmdbStore::init(context.child("src"), "src").await),
            "saga",
            None,
        );
        apply_commit(
            &mut src,
            1,
            user(9),
            &register("alpha", &[ACTION_CHAT_POST, ACTION_TASKS_CREATE]),
        )
        .await;
        apply_commit(
            &mut src,
            2,
            Origin::Module("automations".into()),
            &register("beta", &[]),
        )
        .await;
        apply_commit(
            &mut src,
            3,
            user(9),
            &AgentMsg::UpdateAgent {
                agent_id: "alpha".into(),
                display_name: Some("Alpha Prime".into()),
                capability: Some("model-2".into()),
                allowed_actions: None,
                recipe_hash: None,
                caps: None,
                skills: None,
            },
        )
        .await;
        apply_commit(
            &mut src,
            4,
            Origin::Module("automations".into()),
            &AgentMsg::PauseAgent {
                agent_id: "beta".into(),
            },
        )
        .await;
        let src_root: StateRoot = src.root();
        assert_ne!(src_root, StateRoot::ZERO, "source must have a real root");
        // capture the source's answers to compare against the joiner's.
        let AgentReply::Agents(src_listing) = query_reply(&src, &AgentQuery::Agents).await else {
            panic!("expected a listing");
        };

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
        // the resolver, then wrap the module around the injected store — the
        // exact shape a joining host uses. no ops are applied in application
        // order on this side.
        let store = QmdbStore::sync_from(context.child("dst"), "dst", target, resolver)
            .await
            .expect("sync_from");
        let synced = AgentModule::new("agent", Box::new(store), "saga", None);

        // THE PROPERTY: identical qmdb root — the root-hash linkage a joiner
        // needs at the boundary height.
        assert_eq!(
            synced.root(),
            src_root,
            "synced store root must equal the source root"
        );

        // the roster synced with the records: the joiner lists exactly like
        // the source, and every committed mutation survived.
        let AgentReply::Agents(listing) = query_reply(&synced, &AgentQuery::Agents).await else {
            panic!("expected a listing");
        };
        assert_eq!(listing, src_listing);
        assert_eq!(listing.len(), 2);
        assert_eq!(listing[0].agent_id, "alpha");
        assert_eq!(listing[0].display_name, "Alpha Prime");
        assert_eq!(listing[0].capability, "model-2");
        assert_eq!(listing[0].owner, SagaOrigin::External(vec![9; 32]));
        assert_eq!(listing[1].agent_id, "beta");
        assert_eq!(listing[1].status, AgentStatus::Paused);
        assert_eq!(listing[1].owner, SagaOrigin::Module("automations".into()));

        // the point read answers like the source too.
        let AgentReply::Agent(Some(alpha)) = query_reply(
            &synced,
            &AgentQuery::Agent {
                agent_id: "alpha".into(),
            },
        )
        .await
        else {
            panic!("expected the alpha record");
        };
        assert_eq!(alpha, listing[0]);
    });
}
