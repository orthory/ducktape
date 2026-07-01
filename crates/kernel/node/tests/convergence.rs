//! the replication milestone: two nodes, each a host with the SAME genesis
//! module set, converge on the BYTE-IDENTICAL app-hash after an op submitted at
//! node A propagates to node B — and the target module's STATE (not just the
//! hash) is present on both nodes.
//!
//! the module set is a single in-memory `directory`: its root is state-based
//! (idempotent, order-independent), so two nodes that end at the same final
//! content converge without needing a total order — exactly the property this
//! propagation-only slice can prove. (a qmdb-backed module's root is
//! order-dependent and would NOT converge under mere fan-out; that's what the
//! commonware/BFT ordering slice is for.)

use directory::Directory;
use directory_interface::{
    decode_reply, encode_msg, encode_query, DirMsg, DirQuery, DirReply,
};
use futures::executor::block_on;
use host::Host;
use node::{LoopbackHub, Node};
use sdk::Msg;

/// a fresh host whose genesis is one directory module — identical on every node.
fn genesis_host() -> Host {
    Host::genesis(vec![Box::new(Directory::new("directory"))]).expect("genesis")
}

fn set_name_world() -> Msg {
    Msg {
        target: "directory".into(),
        payload: encode_msg(&DirMsg::Set { key: "name".into(), value: "world".into() }),
    }
}

/// read `directory["name"]` off a node's own host — proves the STATE replicated,
/// not merely that two opaque hashes matched.
async fn read_name<T: node::Transport>(n: &Node<T>) -> Option<String> {
    let reply = n
        .host()
        .query("directory", &encode_query(&DirQuery::Get { key: "name".into() }))
        .await
        .expect("directory query ok");
    match decode_reply(&reply).expect("decode reply") {
        DirReply::Value(v) => v,
    }
}

#[test]
fn two_nodes_converge_on_identical_app_hash() {
    block_on(async {
        let hub = LoopbackHub::new();
        let (ta, ra) = hub.node();
        let (tb, rb) = hub.node();
        let mut a = Node::new(genesis_host(), ta, ra);
        let mut b = Node::new(genesis_host(), tb, rb);

        // identical genesis module set -> identical app-hash at the start.
        let genesis = a.app_hash();
        assert_eq!(genesis, b.app_hash(), "identical genesis -> identical app-hash");

        // originate an op at A: apply locally + propagate to peers.
        a.apply_local(set_name_world()).await.expect("A applies + propagates");

        // A advanced; B has not polled yet -> they diverge (the intermediate,
        // deterministic "A is ahead" state).
        assert_ne!(a.app_hash(), genesis, "A's op moved its app-hash off genesis");
        assert_ne!(a.app_hash(), b.app_hash(), "before B polls, A and B diverge");
        assert_eq!(read_name(&a).await, Some("world".into()), "A holds the write it originated");
        assert_eq!(read_name(&b).await, None, "B has not seen the op yet");

        // B drains the propagated op and applies it.
        let applied = b.poll_inbound().await.expect("B drains inbound");
        assert_eq!(applied, 1, "B applied exactly the one propagated op");

        // THE MILESTONE: both nodes now hold the byte-identical app-hash AND the
        // replicated module STATE (the value crossed the wire, not just the hash).
        assert_eq!(a.app_hash(), b.app_hash(), "A and B converge on identical app-hash");
        assert_eq!(read_name(&a).await, Some("world".into()), "A still holds name=world");
        assert_eq!(read_name(&b).await, Some("world".into()), "the write REPLICATED to B");

        // the local-only rule: B applying an INBOUND op must not have propagated
        // anything back. so A, polling now, sees zero traffic — no echo, no
        // ping-pong. this is the property that keeps a 2-node loop finite (the
        // run terminates: no infinite re-propagation).
        let echoed_back = a.poll_inbound().await.expect("A polls");
        assert_eq!(echoed_back, 0, "an inbound op is never re-broadcast (local-only rule)");
    });
}

#[test]
fn originating_at_either_node_converges() {
    // symmetry: the same convergence holds when B is the originator.
    block_on(async {
        let hub = LoopbackHub::new();
        let (ta, ra) = hub.node();
        let (tb, rb) = hub.node();
        let mut a = Node::new(genesis_host(), ta, ra);
        let mut b = Node::new(genesis_host(), tb, rb);

        b.apply_local(set_name_world()).await.expect("B applies + propagates");
        let applied = a.poll_inbound().await.expect("A drains inbound");
        assert_eq!(applied, 1);
        assert_eq!(a.app_hash(), b.app_hash(), "converge when B originates");
        assert_eq!(read_name(&a).await, Some("world".into()), "the write REPLICATED to A");
        assert_eq!(read_name(&b).await, Some("world".into()), "B still holds name=world");
    });
}
