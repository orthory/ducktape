//! the epoch-cutover gate on the ordered lane: frames finalized at or past the
//! agreed cutover view are DISCARDED by the same deterministic rule on every
//! node (no straggler fork during engine teardown), and `cutover` rebases app
//! heights so `Env` stays monotone across epochs.

use directory::Directory;
use directory_interface::{decode_reply, encode_msg, encode_query, DirMsg, DirQuery, DirReply};
use futures::executor::block_on;
use host::Host;
use node::{OrderedNode, RoundOrderer};
use sdk::Msg;

fn sk(seed: u64) -> commonware_cryptography::ed25519::PrivateKey {
    use commonware_cryptography::Signer as _;
    commonware_cryptography::ed25519::PrivateKey::from_seed(seed)
}

fn set(key: &str, value: &str) -> Msg {
    Msg {
        target: "directory".into(),
        payload: encode_msg(&DirMsg::Set { key: key.into(), value: value.into() }),
    }
}

async fn get(node: &OrderedNode<RoundOrderer>, key: &str) -> Option<String> {
    let reply = node
        .host()
        .query("directory", &encode_query(&DirQuery::Get { key: key.into() }))
        .await
        .expect("query");
    match decode_reply(&reply).expect("decode") {
        DirReply::Value(v) => v,
    }
}

#[test]
fn ceiling_discards_and_cutover_rebases_heights() {
    block_on(async {
        let host = Host::genesis(vec![Box::new(Directory::new("directory"))]).expect("genesis");
        let mut node = OrderedNode::new(host, RoundOrderer::new());

        // epoch 0, view 0: a normal op applies.
        node.submit(&sk(1), 0, set("k0", "v0")).await.expect("submit k0");
        assert_eq!(node.drain_delivered().await.expect("drain"), 1);
        assert_eq!(node.finalized().expect("boundary").height, 0);
        assert_eq!(node.last_engine_view(), Some(0));

        // views 1 and 2 arrive in one round; the agreed cutover is view 2 —
        // view 1 applies, view 2 is DISCARDED (but still counts as processed).
        node.submit(&sk(1), 1, set("k1", "v1")).await.expect("submit k1");
        node.submit(&sk(1), 2, set("k2", "v2")).await.expect("submit k2");
        node.set_view_ceiling(2);
        assert_eq!(node.drain_delivered().await.expect("drain"), 2);
        assert_eq!(get(&node, "k1").await.as_deref(), Some("v1"));
        assert_eq!(get(&node, "k2").await, None, "the past-ceiling frame is discarded");
        // the AGREED view still advances the engine clock (a node must be able
        // to observe the views that carry it past its own cutover), so
        // last_engine_view moves to 2 — but the served BOUNDARY tracks the last
        // NON-discarded frame (applied view 1), never the discarded view, so
        // its height and app-hash stay consistent and never regress at cutover.
        assert_eq!(node.finalized().expect("boundary").height, 1);
        assert_eq!(node.last_engine_view(), Some(2));

        // CUTOVER: fresh orderer (engine views restart at 0), app heights
        // rebased at the cutover height. the discarded op stays discarded; a
        // resubmission in the new epoch applies at a monotone height.
        node.cutover(RoundOrderer::new(), 2);
        assert_eq!(node.last_engine_view(), None, "engine view resets");
        node.submit(&sk(1), 3, set("k2", "v2-epoch1")).await.expect("resubmit");
        assert_eq!(node.drain_delivered().await.expect("drain"), 1);
        assert_eq!(get(&node, "k2").await.as_deref(), Some("v2-epoch1"));
        // heights are monotone (non-strict at the seam: the discarded view's
        // height is reused by the new epoch's first block — identically on
        // every node, with the discard recorded in neither's app state).
        let boundary = node.finalized().expect("boundary");
        assert_eq!(boundary.height, 2, "engine view 0 + base 2 = app height 2");
        assert_eq!(node.last_engine_view(), Some(0), "engine-relative view restarted");
    });
}
