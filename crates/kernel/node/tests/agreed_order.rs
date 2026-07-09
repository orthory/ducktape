//! THE agreed-total-order convergence milestone.
//!
//! N validators, each a [`host::Host`] carrying a qmdb-backed `kv` module (whose
//! root is op-log/MMR-order-DEPENDENT) and an in-memory `directory` module (whose
//! root is state-based/order-INdependent). the SAME op-set arrives at every node
//! in a DIFFERENT order. under an AGREED TOTAL ORDER ([`RoundOrderer`]) all nodes
//! apply the identical sequence and converge on a BYTE-IDENTICAL app-hash —
//! INCLUDING the order-dependent qmdb root.
//!
//! the negative control swaps ONLY the orderer for [`ArrivalOrderer`] (raw
//! arrival order, no agreement): two nodes with opposite arrival orders then FORK
//! on the qmdb root while the state-based directory root stays equal. that
//! swap-only divergence proves the agreed order is load-bearing, not decoration.
//!
//! the semantic shift is asserted directly: after every node `submit`s but before
//! any node drains, every app-hash is still genesis — a locally-originated msg is
//! NOT applied optimistically on the ordered lane.

use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use directory::Directory;
use directory::{DirMsg, encode_msg};
use host::Host;
use kv::Kv;
use kv::{KvMsg, encode};
use node::{ArrivalOrderer, OrderedNode, Orderer, RoundOrderer};
use sdk::Msg;

const LABELS: [&str; 4] = ["v0", "v1", "v2", "v3"];

/// a fresh validator host: one qmdb `kv` module (order-dependent root) on an
/// ISOLATED child context + one in-memory `directory` module.
async fn genesis_host(ctx: deterministic::Context) -> Host {
    let kv = Kv::init(ctx, "kv").await;
    Host::genesis(vec![Box::new(kv), Box::new(Directory::new("directory"))]).expect("genesis")
}

fn kv_set(k: &[u8], v: &[u8]) -> Msg {
    Msg {
        target: "kv".into(),
        payload: encode(&KvMsg::Set {
            key: k.to_vec(),
            value: v.to_vec(),
        }),
    }
}

fn dir_set(k: &str, v: &str) -> Msg {
    Msg {
        target: "directory".into(),
        payload: encode_msg(&DirMsg::Set {
            key: k.into(),
            value: v.into(),
        }),
    }
}

/// the canonical op-set: distinct origins (so every frame is distinct and the
/// order key is tie-free), distinct kv keys (so the FINAL kv content is identical
/// under any permutation — divergence can only come from LOG ORDER, not content),
/// plus two directory writes for the order-independence contrast.
fn op_set() -> Vec<(commonware_cryptography::ed25519::PrivateKey, u64, Msg)> {
    vec![
        (sk(1), 0, kv_set(b"aaa", b"1")),
        (sk(2), 0, kv_set(b"bbb", b"2")),
        (sk(3), 0, kv_set(b"ccc", b"3")),
        (sk(4), 0, dir_set("x", "10")),
        (sk(5), 0, dir_set("y", "20")),
    ]
}
/// a deterministic dev signer for test frames (any u64 seed).
fn sk(seed: u64) -> commonware_cryptography::ed25519::PrivateKey {
    use commonware_cryptography::Signer as _;
    commonware_cryptography::ed25519::PrivateKey::from_seed(seed)
}

/// permute a vec by a rotation offset — a cheap way to give each validator a
/// genuinely different arrival order of the identical set.
fn rotated<T: Clone>(v: &[T], by: usize) -> Vec<T> {
    let n = v.len();
    (0..n).map(|i| v[(i + by) % n].clone()).collect()
}

/// feed a node its whole arrival order, then drain to a fixpoint. returns the
/// total number of ops applied.
async fn feed_and_drain<O: Orderer>(
    node: &mut OrderedNode<O>,
    arrival: &[(commonware_cryptography::ed25519::PrivateKey, u64, Msg)],
) -> usize {
    for (origin, seq, msg) in arrival {
        node.submit(origin, *seq, msg.clone())
            .await
            .expect("submit");
        // flush each op into its OWN single-member batch super-frame. cross-node
        // agreement then rides the orderer's sort over the identical SET of
        // super-frames (RoundOrderer) or their arrival order (ArrivalOrderer) —
        // exactly as the pre-batch per-op frames did. packing multiple ops into
        // one batch would fix their order to this node's LOCAL submit order (no
        // cross-node agreement) and fork the order-dependent qmdb root.
        node.flush_batch().await.expect("flush");
    }
    let mut total = 0;
    loop {
        let n = node.drain_delivered().await.expect("drain");
        total += n;
        if n == 0 {
            break;
        }
    }
    total
}

#[test]
fn n_validators_converge_under_agreed_order_including_qmdb_root() {
    deterministic::Runner::timed(std::time::Duration::from_secs(60)).start(|context| async move {
        const N: usize = 3;
        let ops = op_set();

        // stand up N validators on isolated child contexts.
        let mut nodes: Vec<OrderedNode<RoundOrderer>> = Vec::new();
        for label in LABELS.iter().take(N) {
            let host = genesis_host(context.child(label)).await;
            nodes.push(OrderedNode::new(host, RoundOrderer::new()));
        }

        // identical genesis module set -> identical genesis app-hash.
        let genesis = nodes[0].app_hash();
        for n in &nodes {
            assert_eq!(
                n.app_hash(),
                genesis,
                "identical genesis -> identical app-hash"
            );
        }
        let genesis_kv = nodes[0].host().module_root("kv").unwrap();

        // each validator receives the SAME op-set in a DIFFERENT arrival order,
        // and SUBMITS + FLUSHES every op (no local apply — the semantic shift).
        // each op becomes its own single-member batch super-frame, so the agreed
        // order is the orderer's sort over the identical SET (as before batching).
        for (i, node) in nodes.iter_mut().enumerate() {
            let arrival = rotated(&ops, i);
            for (origin, seq, msg) in &arrival {
                node.submit(origin, *seq, msg.clone())
                    .await
                    .expect("submit");
                node.flush_batch().await.expect("flush");
            }
        }

        // SEMANTIC SHIFT: after submit+flush, before any drain, EVERY node is
        // still at genesis. flush pins+proposes but does NOT apply — nothing is
        // applied optimistically; the originator is NOT ahead.
        for n in &nodes {
            assert_eq!(
                n.app_hash(),
                genesis,
                "no optimistic echo: submit does not advance app-hash"
            );
        }

        // drain every node to a fixpoint: all read the agreed order -> all apply
        // the identical sequence.
        for node in nodes.iter_mut() {
            loop {
                if node.drain_delivered().await.expect("drain") == 0 {
                    break;
                }
            }
        }

        // THE MILESTONE: byte-identical app-hash on every validator, moved off
        // genesis, INCLUDING the order-dependent qmdb root.
        let converged = nodes[0].app_hash();
        let converged_kv = nodes[0].host().module_root("kv").unwrap();
        assert_ne!(
            converged, genesis,
            "the applied ops moved the app-hash off genesis"
        );
        assert_ne!(converged_kv, genesis_kv, "the qmdb root moved off genesis");
        for n in &nodes {
            assert_eq!(
                n.app_hash(),
                converged,
                "all validators converge on identical app-hash"
            );
            assert_eq!(
                n.host().module_root("kv").unwrap(),
                converged_kv,
                "all validators converge on the identical qmdb (order-DEPENDENT) root"
            );
        }
    });
}

#[test]
fn arrival_order_forks_the_qmdb_root_but_not_the_directory_root() {
    // NEGATIVE CONTROL — identical to the positive path except the orderer: two
    // validators, OPPOSITE arrival orders, applied WITHOUT an agreed order.
    deterministic::Runner::timed(std::time::Duration::from_secs(60)).start(|context| async move {
        let ops = op_set();
        let reversed: Vec<_> = ops.iter().rev().cloned().collect();

        let mut a = OrderedNode::new(
            genesis_host(context.child("a")).await,
            ArrivalOrderer::new(),
        );
        let mut b = OrderedNode::new(
            genesis_host(context.child("b")).await,
            ArrivalOrderer::new(),
        );

        // sanity: they start converged.
        assert_eq!(a.app_hash(), b.app_hash(), "identical genesis");

        feed_and_drain(&mut a, &ops).await;
        feed_and_drain(&mut b, &reversed).await;

        // the order-DEPENDENT qmdb root FORKS under arrival order — this is the
        // fork the agreed total order prevents.
        assert_ne!(
            a.host().module_root("kv").unwrap(),
            b.host().module_root("kv").unwrap(),
            "arrival order (no agreement) MUST fork the qmdb root — else the order is decoration"
        );

        // the state-based directory root does NOT fork under the same swap: the
        // divergence is specific to qmdb order-dependence, not a harness artifact.
        assert_eq!(
            a.host().module_root("directory").unwrap(),
            b.host().module_root("directory").unwrap(),
            "the order-INdependent directory root stays equal under the same arrival swap"
        );

        // and the whole app-hash forks (it folds in the forked qmdb root).
        assert_ne!(
            a.app_hash(),
            b.app_hash(),
            "the composed app-hash forks with the qmdb root"
        );
    });
}

#[test]
fn agreed_order_converges_where_arrival_order_forks_same_two_nodes() {
    // the two controls side by side on the SAME two opposite arrival orders: only
    // the orderer differs, and that alone flips fork -> converge.
    deterministic::Runner::timed(std::time::Duration::from_secs(60)).start(|context| async move {
        let ops = op_set();
        let reversed: Vec<_> = ops.iter().rev().cloned().collect();

        let mut a = OrderedNode::new(genesis_host(context.child("a")).await, RoundOrderer::new());
        let mut b = OrderedNode::new(genesis_host(context.child("b")).await, RoundOrderer::new());

        feed_and_drain(&mut a, &ops).await;
        feed_and_drain(&mut b, &reversed).await;

        assert_eq!(
            a.host().module_root("kv").unwrap(),
            b.host().module_root("kv").unwrap(),
            "the SAME opposite arrival orders CONVERGE the qmdb root once agreed-ordered"
        );
        assert_eq!(
            a.app_hash(),
            b.app_hash(),
            "and the whole app-hash converges"
        );
    });
}
