//! the observation barrier on the ordered lane: with a watched module armed,
//! a drain batch ends right after any block that CHANGES that module's root,
//! deferring the remainder to the next drain. a caller observing the watched
//! module once per drain therefore observes the change at exactly the
//! changing block's view — the same view on every validator, no matter how
//! deliveries batched locally. (without the split, two nodes draining the
//! same finalized views in different batch shapes would observe a membership
//! change at different views and schedule DIFFERENT epoch cutovers.)

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

fn set(module: &str, key: &str, value: &str) -> Msg {
    Msg {
        target: module.into(),
        payload: encode_msg(&DirMsg::Set { key: key.into(), value: value.into() }),
    }
}

async fn get(node: &OrderedNode<RoundOrderer>, module: &str, key: &str) -> Option<String> {
    let reply = node
        .host()
        .query(module, &encode_query(&DirQuery::Get { key: key.into() }))
        .await
        .expect("query");
    match decode_reply(&reply).expect("decode") {
        DirReply::Value(v) => v,
    }
}

/// two order-independent modules; "watched" stands in for valset. ops that
/// change the watched root end their batch; ops that leave every root of the
/// watched module unchanged (including a same-value overwrite) do not.
#[test]
fn batch_ends_at_the_block_that_moves_the_watched_root() {
    block_on(async {
        let host = Host::genesis(vec![
            Box::new(Directory::new("directory")),
            Box::new(Directory::new("watched")),
        ])
        .expect("genesis");
        let mut node = OrderedNode::new(host, RoundOrderer::new());
        node.watch_module("watched");

        // four frames finalize in ONE round: bystander, watched-change,
        // bystander, bystander. the drain must stop AT the watched change.
        node.submit(&sk(1), 0, set("directory", "d0", "x")).await.expect("submit");
        node.submit(&sk(1), 1, set("watched", "w0", "joined")).await.expect("submit");
        node.submit(&sk(1), 2, set("directory", "d1", "x")).await.expect("submit");
        node.submit(&sk(1), 3, set("directory", "d2", "x")).await.expect("submit");

        assert_eq!(node.drain_delivered().await.expect("drain"), 2, "batch ends at the change");
        assert_eq!(
            node.last_engine_view(),
            Some(1),
            "the observer sees exactly the changing block's view"
        );
        assert_eq!(get(&node, "watched", "w0").await.as_deref(), Some("joined"));
        assert_eq!(get(&node, "directory", "d1").await, None, "the remainder is deferred");

        // the deferred remainder drains next call, ahead of fresh deliveries.
        node.submit(&sk(1), 4, set("directory", "d3", "x")).await.expect("submit");
        assert_eq!(node.drain_delivered().await.expect("drain"), 3);
        assert_eq!(node.last_engine_view(), Some(4));
        assert_eq!(get(&node, "directory", "d1").await.as_deref(), Some("x"));
        assert_eq!(get(&node, "directory", "d3").await.as_deref(), Some("x"));

        // a root-idempotent write to the watched module (same key, same value)
        // is NOT a change — no barrier, the whole batch drains.
        node.submit(&sk(1), 5, set("watched", "w0", "joined")).await.expect("submit");
        node.submit(&sk(1), 6, set("directory", "d4", "x")).await.expect("submit");
        assert_eq!(node.drain_delivered().await.expect("drain"), 2, "idempotent write does not split");
        assert_eq!(get(&node, "directory", "d4").await.as_deref(), Some("x"));

        // two watched changes in one round: each ends its own batch, so each
        // is observed at its own view.
        node.submit(&sk(1), 7, set("watched", "w1", "a")).await.expect("submit");
        node.submit(&sk(1), 8, set("watched", "w2", "b")).await.expect("submit");
        assert_eq!(node.drain_delivered().await.expect("drain"), 1);
        assert_eq!(node.last_engine_view(), Some(7));
        assert_eq!(node.drain_delivered().await.expect("drain"), 1);
        assert_eq!(node.last_engine_view(), Some(8));
    });
}
