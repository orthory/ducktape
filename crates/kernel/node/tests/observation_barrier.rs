//! the observation barrier on the ordered lane: with a watched module armed,
//! a drain batch ends right after any block that CHANGES that module's root,
//! deferring the remainder to the next drain. a caller observing the watched
//! module once per drain therefore observes the change at exactly the
//! changing block's view — the same view on every validator, no matter how
//! deliveries batched locally. (without the split, two nodes draining the
//! same finalized views in different batch shapes would observe a membership
//! change at different views and schedule DIFFERENT epoch cutovers.)

use directory::Directory;
use directory::{DirMsg, DirQuery, DirReply, decode_reply, encode_msg, encode_query};
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
        payload: encode_msg(&DirMsg::Set {
            key: key.into(),
            value: value.into(),
        }),
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

/// two order-independent modules; "dir_b" stands in for valset (the watched
/// module). ops that change the watched root end their batch; ops that leave
/// every root of the watched module unchanged (including a same-value
/// overwrite) do not.
///
/// each op is submitted then FLUSHED into its OWN single-member batch, so the
/// observation barrier (now once per BATCH) still splits per op. the module
/// names ("dir_a"/"dir_b", both 5 bytes), 2-byte keys and 1-byte values keep
/// EVERY member frame the SAME length, so the orderer's byte-sort over the
/// single-member batch super-frames is exactly `seq` order — the per-view
/// structure the pre-batch per-op frames had.
#[test]
fn batch_ends_at_the_block_that_moves_the_watched_root() {
    block_on(async {
        let host = Host::genesis(vec![
            Box::new(Directory::new("dir_a")),
            Box::new(Directory::new("dir_b")),
        ])
        .expect("genesis");
        let mut node = OrderedNode::new(host, RoundOrderer::new());
        node.watch_module("dir_b");

        // four one-op batches finalize in ONE round: bystander, watched-change,
        // bystander, bystander. the drain must stop AT the watched change.
        for (seq, op) in [
            set("dir_a", "d0", "x"),
            set("dir_b", "w0", "j"),
            set("dir_a", "d1", "x"),
            set("dir_a", "d2", "x"),
        ]
        .into_iter()
        .enumerate()
        {
            node.submit(&sk(1), seq as u64, op).await.expect("submit");
            node.flush_batch().await.expect("flush");
        }

        assert_eq!(
            node.drain_delivered().await.expect("drain"),
            2,
            "batch ends at the change"
        );
        assert_eq!(
            node.last_engine_view(),
            Some(1),
            "the observer sees exactly the changing block's view"
        );
        assert_eq!(get(&node, "dir_b", "w0").await.as_deref(), Some("j"));
        assert_eq!(
            get(&node, "dir_a", "d1").await,
            None,
            "the remainder is deferred"
        );

        // the deferred remainder drains next call, ahead of fresh deliveries.
        node.submit(&sk(1), 4, set("dir_a", "d3", "x"))
            .await
            .expect("submit");
        node.flush_batch().await.expect("flush");
        assert_eq!(node.drain_delivered().await.expect("drain"), 3);
        assert_eq!(node.last_engine_view(), Some(4));
        assert_eq!(get(&node, "dir_a", "d1").await.as_deref(), Some("x"));
        assert_eq!(get(&node, "dir_a", "d3").await.as_deref(), Some("x"));

        // a root-idempotent write to the watched module (same key, same value)
        // is NOT a change — no barrier, both batches drain.
        node.submit(&sk(1), 5, set("dir_b", "w0", "j"))
            .await
            .expect("submit");
        node.flush_batch().await.expect("flush");
        node.submit(&sk(1), 6, set("dir_a", "d4", "x"))
            .await
            .expect("submit");
        node.flush_batch().await.expect("flush");
        assert_eq!(
            node.drain_delivered().await.expect("drain"),
            2,
            "idempotent write does not split"
        );
        assert_eq!(get(&node, "dir_a", "d4").await.as_deref(), Some("x"));

        // two watched changes in one round: each ends its own batch, so each
        // is observed at its own view.
        node.submit(&sk(1), 7, set("dir_b", "w1", "a"))
            .await
            .expect("submit");
        node.flush_batch().await.expect("flush");
        node.submit(&sk(1), 8, set("dir_b", "w2", "b"))
            .await
            .expect("submit");
        node.flush_batch().await.expect("flush");
        assert_eq!(node.drain_delivered().await.expect("drain"), 1);
        assert_eq!(node.last_engine_view(), Some(7));
        assert_eq!(node.drain_delivered().await.expect("drain"), 1);
        assert_eq!(node.last_engine_view(), Some(8));
    });
}

/// the replica-shaped repro from ducktape#1819: WITHOUT the barrier armed, a
/// pass that folds several finalized frames in one `drain_delivered` call —
/// exactly how a replica's park loop folds a backfill burst — reports the
/// pass's LAST view, not the view that moved the watched (valset) module.
/// This is the bug: `bin/node/src/replica/park.rs` derived its `folded_view`
/// from the pass's last served height instead of `last_engine_view()`, so a
/// membership change folded anywhere but the last frame of a multi-block pass
/// armed a cutover ceiling (and, on promotion, a `view_base`) at a later view
/// than every validator armed (which — with `watch_module` — always stops
/// AT the changing block, per `batch_ends_at_the_block_that_moves_the_watched_root`
/// above).
#[test]
fn without_the_barrier_a_multi_frame_pass_reports_the_last_view_not_the_changing_one() {
    block_on(async {
        let host = Host::genesis(vec![
            Box::new(Directory::new("dir_a")),
            Box::new(Directory::new("dir_b")),
        ])
        .expect("genesis");
        let mut node = OrderedNode::new(host, RoundOrderer::new());
        // deliberately no `node.watch_module("dir_b")` — the exact wiring gap
        // #1819 found at both replica `OrderedNode::resume` sites.

        // views 0..=3 (v-1..v+2 with the watched change at v=1), all
        // finalized before a single drain call — the multi-frame pass shape a
        // replica folding a backfill burst produces.
        for (seq, op) in [
            set("dir_a", "d0", "x"),
            set("dir_b", "w0", "j"),
            set("dir_a", "d1", "x"),
            set("dir_a", "d2", "x"),
        ]
        .into_iter()
        .enumerate()
        {
            node.submit(&sk(1), seq as u64, op).await.expect("submit");
            node.flush_batch().await.expect("flush");
        }

        assert_eq!(
            node.drain_delivered().await.expect("drain"),
            4,
            "unwatched, the whole pass folds in one call — nothing defers"
        );
        assert_eq!(
            node.last_engine_view(),
            Some(3),
            "without the barrier the pass reports its LAST view, past the \
             view (1) that actually moved the watched module — the exact \
             defect: a replica computing its cutover coordinates off this \
             value arms them at a later view than every validator did"
        );
    });
}
