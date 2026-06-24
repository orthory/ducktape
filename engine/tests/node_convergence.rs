//! two async [`Node`]s on one [`LoopbackHub`]: an op driven into node A
//! propagates over the transport and node B's workspace converges.
//!
//! this is the integration capstone's proof. it wires the WHOLE node — engine
//! dispatcher + transport + inbound task — not the bare seams the earlier
//! `consensus_lane.rs` / `transport::convergence` tests poke. the difference
//! that matters: node B's inbound receiver is owned by the node's BACKGROUND
//! task, not drained by hand. so the test cannot compare "live A vs live B" mid-
//! flight (B may already have applied by the time A's apply returns). instead:
//!
//!   1. snapshot the shared start fingerprint (A == B).
//!   2. drive the op into A, snapshot A: assert it CHANGED from start. that's the
//!      non-tautology guard — a no-op `apply_local_direct` fails right here.
//!   3. await B's inbound tick (deterministic — parks on the node's notify, NOT a
//!      wall-clock sleep), snapshot B: assert B == A.
//!
//! the op is an `EntryMut` carrying a document `OnUserWrite` (broadcast lane); a
//! second case drives an `AddEntry` (consensus lane) to prove both lanes cross.

use document::Document;
use document::op::{Op as DocOp, OpId};
use engine::{Config, Engine, Node};
use transport::LoopbackHub;
use uid::Identify;
use workspace::{Entry, Workspace};

// frontmatter-only doc; `from_reader` mints the uids. see the NOTE in
// transport/tests/convergence.rs on why an empty `misc` keeps the fingerprint
// deterministic.
const SAMPLE: &str = "---\ntitle: t\nauthor: @a\ncreated_at: 1\nupdated_at: 1\n---\n";

/// canonical, hashmap-order-independent fingerprint of a workspace tree. same
/// shape the sibling convergence tests use.
fn fingerprint(ws: &Workspace) -> String {
    use serde::Serialize;

    #[derive(Serialize)]
    enum Canon {
        File { uid: String, nodes: Vec<nodes::Nodes> },
        Dir(Vec<(String, Canon)>),
    }

    fn walk(entry: &Entry) -> Canon {
        match entry {
            Entry::File(doc) => Canon::File {
                uid: doc.uid().to_string(),
                nodes: doc.nodes_iter().collect(),
            },
            Entry::Directory(items) => {
                Canon::Dir(items.iter().map(|(n, c)| (n.clone(), walk(c))).collect())
            }
        }
    }

    serde_json::to_string(&walk(ws.root())).expect("canon serializes")
}

/// a test config with a tiny id and the defaults filled in.
fn cfg(id: usize) -> Config {
    Config::from_toml_str(&format!("id = {id}\n")).expect("default config parses")
}

#[tokio::test]
async fn entry_mut_propagates_through_node_to_b() {
    // --- two nodes from the SAME initial state. parse the doc ONCE, then clone
    // the workspace for B (re-parsing would mint a fresh uid and break both
    // "start equal" and the EntryMut target). ----------------------------------
    let doc = Document::from_reader(SAMPLE.as_bytes()).expect("parse sample");
    let doc_id = doc.uid();

    let ws_a = Workspace::new_from_entry(Entry::Directory(vec![("a.md".into(), Entry::File(doc))]));
    let ws_b = ws_a.clone();

    let hub = LoopbackHub::new();
    let (transport_a, rx_a) = hub.node();
    let (transport_b, rx_b) = hub.node();

    let node_a = Node::new(cfg(0), Engine::new(ws_a), transport_a, rx_a);
    let node_b = Node::new(cfg(1), Engine::new(ws_b), transport_b, rx_b);

    // they start EQUAL.
    let start = fingerprint(&node_a.workspace_snapshot().await);
    assert_eq!(
        start,
        fingerprint(&node_b.workspace_snapshot().await),
        "nodes must start from identical state"
    );

    // --- drive a broadcast-lane edit into A: prepend "hello " to the title. ----
    let op = op::Op::Workspace(workspace::op::Op::EntryMut {
        entry_id: doc_id,
        op: DocOp::OnUserWrite {
            op_id: OpId::new(1, 1),
            node_id: doc_id,
            pos: 0,
            text: "hello ".into(),
        },
    });
    assert_eq!(op.lane(), op::Lane::Broadcast, "EntryMut rides broadcast");

    node_a.apply_local_direct(op).await.expect("A applies + sends");

    // A has diverged from the shared start. NON-TAUTOLOGY GUARD: a no-op local
    // apply (or a no-op transport) leaves A == start and fails here.
    let a_fp = fingerprint(&node_a.workspace_snapshot().await);
    assert_ne!(a_fp, start, "A's local edit must change A's state");

    // --- await B applying the propagated op (deterministic; not a sleep) -------
    node_b.wait_inbound().await;
    let b_fp = fingerprint(&node_b.workspace_snapshot().await);

    assert_eq!(a_fp, b_fp, "B converges to A after the op propagates");
}

#[tokio::test]
async fn add_entry_consensus_lane_propagates_through_node_to_b() {
    // start both nodes from an identical single-file tree.
    let base = Document::from_reader(SAMPLE.as_bytes()).expect("parse base");
    let ws_a = Workspace::new_from_entry(Entry::Directory(vec![("a.md".into(), Entry::File(base))]));
    let ws_b = ws_a.clone();

    let hub = LoopbackHub::new();
    let (transport_a, rx_a) = hub.node();
    let (transport_b, rx_b) = hub.node();

    let node_a = Node::new(cfg(0), Engine::new(ws_a), transport_a, rx_a);
    let node_b = Node::new(cfg(1), Engine::new(ws_b), transport_b, rx_b);

    let start = fingerprint(&node_a.workspace_snapshot().await);
    assert_eq!(start, fingerprint(&node_b.workspace_snapshot().await));

    // a brand-new entry with an explicit path (survives the wire identically).
    let new_doc = Document::from_reader(SAMPLE.as_bytes()).expect("parse new doc");
    let op = op::Op::Workspace(workspace::op::Op::AddEntry {
        path: "new.md".into(),
        entry: Entry::File(new_doc),
    });
    assert_eq!(op.lane(), op::Lane::Consensus, "AddEntry rides consensus");

    node_a.apply_local_direct(op).await.expect("A applies + sends");

    let a_fp = fingerprint(&node_a.workspace_snapshot().await);
    assert_ne!(a_fp, start, "the new entry must change A's state");

    node_b.wait_inbound().await;
    let b_fp = fingerprint(&node_b.workspace_snapshot().await);

    assert_eq!(a_fp, b_fp, "B converges after the AddEntry propagates");
}

/// liveness for the REAL hydrator (debounced) path: `apply_local` feeds the op
/// into the hydrator, whose interval drain fires the on_hydrate callback, which
/// forwards the encoded batch to the outbound forwarder, which sends it. node B
/// eventually converges. this waits on B's inbound notify (which fires when the
/// debounced batch actually lands), not a fixed sleep — but it does ride the
/// hydrator's drain interval, so we shrink that interval to keep it fast.
#[tokio::test]
async fn hydrator_debounced_apply_local_propagates_to_b() {
    let doc = Document::from_reader(SAMPLE.as_bytes()).expect("parse sample");
    let doc_id = doc.uid();

    let ws_a = Workspace::new_from_entry(Entry::Directory(vec![("a.md".into(), Entry::File(doc))]));
    let ws_b = ws_a.clone();

    let hub = LoopbackHub::new();
    let (transport_a, rx_a) = hub.node();
    let (transport_b, rx_b) = hub.node();

    // tiny drain interval so the debounce fires promptly.
    let cfg_a = Config::from_toml_str("id = 0\ndrain_interval_ms = 5\n").expect("cfg");
    let cfg_b = Config::from_toml_str("id = 1\ndrain_interval_ms = 5\n").expect("cfg");
    let node_a = Node::new(cfg_a, Engine::new(ws_a), transport_a, rx_a);
    let node_b = Node::new(cfg_b, Engine::new(ws_b), transport_b, rx_b);

    let start = fingerprint(&node_a.workspace_snapshot().await);
    assert_eq!(start, fingerprint(&node_b.workspace_snapshot().await));

    let op = op::Op::Workspace(workspace::op::Op::EntryMut {
        entry_id: doc_id,
        op: DocOp::OnUserWrite {
            op_id: OpId::new(2, 1),
            node_id: doc_id,
            pos: 0,
            text: "world ".into(),
        },
    });

    // the DEBOUNCED path: local echo now, broadcast on the next drain.
    node_a.apply_local(op).await.expect("A applies via hydrator");

    let a_fp = fingerprint(&node_a.workspace_snapshot().await);
    assert_ne!(a_fp, start, "A's local echo lands immediately");

    node_b.wait_inbound().await;
    let b_fp = fingerprint(&node_b.workspace_snapshot().await);
    assert_eq!(a_fp, b_fp, "B converges after the debounced batch drains + sends");
}
