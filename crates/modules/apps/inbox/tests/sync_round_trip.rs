//! state-sync round-trip: a joiner reconstructs a byte-identical qmdb root by
//! pulling a source store's operation range through commonware's qmdb sync,
//! then wraps a fresh `Inbox` around the injected store — the same
//! discriminating property the rest of the store-backed family proves, over
//! the meta + item layout.
//!
//! the source takes attribution deliveries for two accounts, marks one item
//! read (a meta read-watermark bump), and fully clears the other (item
//! DELETES; the meta keeps its never-rewinding counters), so the joiner must
//! reconstruct every record family — and a post-sync delivery to the cleared
//! account must continue its numbering and still refuse the change it
//! already held.

use attribution::{Actor, AttributionEvent, Change, ChangeKind, Reason, Source, encode_event};
use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use identity::{
    AccountView, Control, IdentityQuery, IdentityReply, KeyScheme, KeyView,
    decode_query as identity_decode_query, encode_reply as identity_encode_reply,
};
use inbox::{AccountNumber, Inbox, InboxAssigned, InboxMsg, decode_assigned, encode_msg};
use sdk::{
    Cause, Env, Error, Hop, ItemRef, MerkleStore as _, Module, Msg, Origin, Root, StateRoot,
};
use sdk_testkit::TestCtx;
use statesync::qmdb::QmdbStore;

const ALICE: AccountNumber = 7;
const BOB: AccountNumber = 9;
const ALICE_KEY: [u8; 3] = [0xa1, 0xa1, 0xa1];
const BOB_KEY: [u8; 3] = [0xb0, 0xb0, 0xb0];

fn identity_stub(req: &[u8]) -> Result<Vec<u8>, Error> {
    let account = |number: AccountNumber, key: [u8; 3]| AccountView {
        number,
        name: format!("account-{number}"),
        control: Control::Keys,
        keys: vec![KeyView {
            scheme: KeyScheme::Ed25519,
            pubkey: key.to_vec(),
            label: None,
            added_at: 0,
        }],
        avatar: None,
        bio: None,
        updated_at: 0,
    };
    let accounts = [account(ALICE, ALICE_KEY), account(BOB, BOB_KEY)];
    let found = match identity_decode_query(req).map_err(Error::Module)? {
        IdentityQuery::Get { number } => accounts.into_iter().find(|a| a.number == number),
        IdentityQuery::OfKey { key } => accounts
            .into_iter()
            .find(|a| a.keys.iter().any(|k| k.pubkey == key)),
        other => {
            return Err(Error::Module(format!(
                "unexpected identity query {other:?}"
            )));
        }
    };
    Ok(identity_encode_reply(&IdentityReply::Account(found)))
}

fn as_origin(origin: Origin, height: u64, cause: Cause) -> TestCtx {
    TestCtx::with_env(Env {
        height,
        consensus_time: height,
        origin,
        me: "inbox".into(),
        cause,
    })
    .on_query("identity", identity_stub)
}

fn change(seq: u64, recipient: AccountNumber) -> Change {
    Change {
        seq,
        source: Source {
            module: "chat".into(),
            kind: "message".into(),
            object: format!("m{seq}"),
        },
        revision: 1,
        recipient,
        reason: Reason::Mention,
        kind: ChangeKind::Added,
        detail: Vec::new(),
        actor: Actor::Account(BOB),
        cause: Cause::Direct,
        height: seq,
    }
}

/// the host running attribution's delivery of `change` here, and the stamp
/// the inbox assigned.
async fn deliver_commit(m: &mut Inbox, height: u64, change: &Change) -> InboxAssigned {
    let mut c = as_origin(
        Origin::Module("attribution".into()),
        height,
        Cause::Chain {
            root: Root::Change {
                source: "attribution".into(),
                seq: change.seq,
            },
            hop: Hop::Delivery(ItemRef {
                source: "attribution".into(),
                item: change.seq,
            }),
        },
    );
    let op = Msg {
        target: "inbox".into(),
        payload: encode_event(&AttributionEvent::Changed(change.clone())),
    };
    m.execute(&mut c, &op).await.unwrap();
    m.commit_block().await.unwrap();
    decode_assigned(c.assigned().expect("a delivery stamps")).unwrap()
}

/// an admin op under the account's own key.
async fn admin_commit(m: &mut Inbox, key: [u8; 3], height: u64, msg: InboxMsg) {
    let mut c = as_origin(Origin::External(key.to_vec()), height, Cause::Direct);
    let op = Msg {
        target: "inbox".into(),
        payload: encode_msg(&msg),
    };
    m.execute(&mut c, &op).await.unwrap();
    m.commit_block().await.unwrap();
}

fn inbox_over(store: Box<dyn sdk::MerkleStore>) -> Inbox {
    Inbox::new("inbox", store, "attribution", "identity")
}

#[test]
fn synced_store_reconstructs_source_root_queues_and_counters() {
    deterministic::Runner::default().start(|context| async move {
        // SOURCE: an empty store — inbox carries no genesis config.
        let src_store = QmdbStore::init(context.child("src"), "src").await;
        let genesis_root = src_store.root();
        let mut src = inbox_over(Box::new(src_store));

        deliver_commit(&mut src, 1, &change(1, ALICE)).await;
        deliver_commit(&mut src, 2, &change(2, ALICE)).await;
        deliver_commit(&mut src, 3, &change(3, BOB)).await;
        // a read flip (meta overwrite) and a full clear (item delete; the
        // meta keeps its counters).
        admin_commit(
            &mut src,
            ALICE_KEY,
            4,
            InboxMsg::MarkRead {
                account: ALICE,
                up_to_seq: 1,
            },
        )
        .await;
        admin_commit(
            &mut src,
            BOB_KEY,
            5,
            InboxMsg::Clear {
                account: BOB,
                up_to_seq: 1,
            },
        )
        .await;

        let src_root: StateRoot = src.root();
        assert_ne!(src_root, genesis_root, "the ops moved the root");
        let src_alice = src.queue_view(ALICE).await.unwrap();
        let src_bob = src.queue_view(BOB).await.unwrap();
        assert_eq!(
            src_bob.as_ref().map(|(next, items)| (*next, items.len())),
            Some((2, 0)),
            "bob's emptied inbox keeps its meta record"
        );
        let src_alice_watermark = src.read_watermark_view(ALICE).await.unwrap();

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

        // queues, watermarks, and the cleared prefix synced together.
        assert_eq!(synced.queue_view(ALICE).await.unwrap(), src_alice);
        assert_eq!(synced.queue_view(BOB).await.unwrap(), src_bob);
        assert_eq!(
            synced.read_watermark_view(ALICE).await.unwrap(),
            src_alice_watermark
        );

        // and the joiner DECIDES like the source: bob's cleared inbox still
        // refuses the change it held (a duplicate) and continues its
        // numbering for the next one.
        assert_eq!(
            deliver_commit(&mut synced, 6, &change(3, BOB)).await,
            InboxAssigned::Duplicate
        );
        assert_eq!(synced.root(), src_root, "a duplicate stages nothing");
        assert_eq!(
            deliver_commit(&mut synced, 7, &change(4, BOB)).await,
            InboxAssigned::Delivered { seq: 2 }
        );
        let (next, items) = synced.queue_view(BOB).await.unwrap().expect("bob's inbox");
        assert_eq!(next, 3);
        assert_eq!(
            items.iter().map(|n| n.seq).collect::<Vec<_>>(),
            vec![2],
            "the post-sync delivery continued the numbering"
        );
        assert_ne!(synced.root(), src_root);
    });
}
