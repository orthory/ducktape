//! state-sync round-trip: a joiner reconstructs a byte-identical qmdb root by
//! pulling a source store's operation range through commonware's qmdb sync,
//! then wraps a fresh `Inbox` around the injected store — the same
//! discriminating property the rest of the store-backed family proves, over
//! the meta + item + member-count layout.
//!
//! the source delivers to two members, marks one item read (record
//! overwrite), and fully clears the other (the op log carries item DELETES
//! plus a META delete and a member-count decrement), so the joiner must
//! reconstruct every record family — and a post-sync delivery to the cleared
//! member must re-mint from seq 1, since its meta record no longer exists.

use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use inbox::{Inbox, InboxMsg, encode_msg};
use sdk::{Env, MerkleStore as _, Module, Msg, Origin, StateRoot};
use sdk_testkit::TestCtx;
use statesync::qmdb::QmdbStore;

/// alice's and bob's submitter keys. a member IS an origin's actor string, so
/// only these origins may ack the queues named after them.
const ALICE_KEY: [u8; 3] = [0xa1, 0xa1, 0xa1];
const BOB_KEY: [u8; 3] = [0xb0, 0xb0, 0xb0];

fn queue_of(key: [u8; 3]) -> String {
    Origin::External(key.to_vec()).actor_string()
}

fn as_origin(origin: Origin, height: u64) -> TestCtx {
    TestCtx::with_env(Env {
        height,
        consensus_time: height,
        origin,
        me: "inbox".into(),
    })
}

fn deliver(member: &str, kind: &str, body: &str) -> Msg {
    Msg {
        target: "inbox".into(),
        payload: encode_msg(&InboxMsg::Deliver {
            member: member.into(),
            kind: kind.into(),
            body: body.into(),
        }),
    }
}

fn mark_read(member: &str, up_to_seq: u64) -> Msg {
    Msg {
        target: "inbox".into(),
        payload: encode_msg(&InboxMsg::MarkRead {
            member: member.into(),
            up_to_seq,
        }),
    }
}

fn clear(member: &str, up_to_seq: u64) -> Msg {
    Msg {
        target: "inbox".into(),
        payload: encode_msg(&InboxMsg::Clear {
            member: member.into(),
            up_to_seq,
        }),
    }
}

/// deliveries ride a module follow-up (the primary writer); acks ride the
/// member's own submitter origin, which is the only one the gate admits.
async fn apply_commit(m: &mut Inbox, origin: Origin, height: u64, op: Msg) {
    let mut c = as_origin(origin, height);
    m.execute(&mut c, &op).await.unwrap();
    m.commit_block().await.unwrap();
}

fn chat() -> Origin {
    Origin::Module("chat".into())
}

fn member(key: [u8; 3]) -> Origin {
    Origin::External(key.to_vec())
}

fn inbox_over(store: Box<dyn sdk::MerkleStore>) -> Inbox {
    Inbox::new("inbox", store)
}

#[test]
fn synced_store_reconstructs_source_root_queues_and_counters() {
    deterministic::Runner::default().start(|context| async move {
        // SOURCE: an empty store — inbox carries no genesis config.
        let src_store = QmdbStore::init(context.child("src"), "src").await;
        let genesis_root = src_store.root();
        let mut src = inbox_over(Box::new(src_store));

        let (alice, bob) = (queue_of(ALICE_KEY), queue_of(BOB_KEY));
        apply_commit(&mut src, chat(), 1, deliver(&alice, "mention", "hi")).await;
        apply_commit(&mut src, chat(), 2, deliver(&alice, "reply", "yo")).await;
        apply_commit(&mut src, chat(), 3, deliver(&bob, "k", "solo")).await;
        // a read flip (record overwrite) and a prefix clear (item deletes;
        // the meta keeps its never-rewinding counter).
        apply_commit(&mut src, member(ALICE_KEY), 4, mark_read(&alice, 1)).await;
        apply_commit(&mut src, member(BOB_KEY), 5, clear(&bob, 1)).await;

        let src_root: StateRoot = src.root();
        assert_ne!(src_root, genesis_root, "the ops moved the root");
        let src_alice = src.queue_view(&alice).await.unwrap();
        // bob's ONLY item (seq 1) was cleared, so his queue is now fully
        // empty and its meta record is deleted — see `stage_clear`.
        let src_bob = src.queue_view(&bob).await.unwrap();
        assert!(src_bob.is_none(), "bob's emptied queue has no meta record");

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
        let mut synced = inbox_over(Box::new(store));

        // THE PROPERTY: identical qmdb root — the root-hash linkage a joiner
        // needs at the boundary height.
        assert_eq!(
            synced.root(),
            src_root,
            "synced store root must equal the source root"
        );

        // queues, read flags, and the cleared prefix synced together.
        assert_eq!(synced.queue_view(&alice).await.unwrap(), src_alice);
        assert_eq!(synced.queue_view(&bob).await.unwrap(), src_bob);

        // the cleared member's meta record did not survive the trip (it was
        // deleted along with the last item): the next delivery re-mints a
        // fresh MemberMeta, starting back at seq 1.
        apply_commit(&mut synced, chat(), 6, deliver(&bob, "k", "after clear")).await;
        let (next, items) = synced
            .queue_view(&bob)
            .await
            .unwrap()
            .expect("bob exists again");
        assert_eq!(next, 2);
        assert_eq!(
            items.iter().map(|n| n.seq).collect::<Vec<_>>(),
            vec![1],
            "the post-sync delivery re-minted at seq 1"
        );
    });
}
