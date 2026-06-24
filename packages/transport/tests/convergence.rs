//! the verification spine: an op applied on node A propagates over the loopback
//! transport and lands identically on node B.
//!
//! this is deliberately end-to-end across the whole p0 stack:
//! - `op::Op` taxonomy + `lane()` routing  (p0.1)
//! - `Workspace::hydrate` local apply       (p0.2)
//! - `LoopbackHub` + `encode_batch`/`decode_batch` transport seam (p0.3)
//!
//! the op we propagate is an
//! `op::Op::Workspace(workspace::op::Op::EntryMut { .. })` carrying a document
//! `OnUserWrite` that prepends text into the frontmatter title. its `lane()` is
//! `Lane::Broadcast`.
//!
//! workspaces are compared by a canonical, hashmap-order-independent string:
//! `Document` stores its nodes in a `HashMap` (iteration order is per-instance
//! nondeterministic, so `serde_json` over the raw doc would flake), but
//! `Document::nodes_iter` walks the forward linked list in deterministic order.
//! so we serialize each file as `(uid, nodes_iter().collect::<Vec<_>>())` and
//! walk directories in tree order — a stable fingerprint of tree state.

use document::Document;
use document::op::{Op as DocOp, OpId};
use hydration::Hydratable;
use op::{Lane, Op};
use serde::Serialize;
use transport::{LoopbackHub, Transport, decode_batch, encode_batch};
use uid::Identify;
use workspace::{Entry, Workspace};

// a minimal valid document: frontmatter only. `from_reader` mints the uids, so
// we read the real document uid back out via `Document::uid()`. the
// frontmatter's `title` ("t") is the editable text buffer the op writes into.
const SAMPLE: &str = "---\ntitle: t\nauthor: @a\ncreated_at: 1\nupdated_at: 1\n---\n";

/// canonical, hashmap-order-independent fingerprint of a workspace's tree.
///
/// NOTE: this is deterministic only because the sample uses ONLY promoted
/// frontmatter keys, leaving `FrontmatterV1::misc` (a `HashMap`) empty — an
/// empty map serializes identically regardless of `RandomState` seed. if you
/// extend the fixture with unknown frontmatter keys (or add any future
/// map-bearing node), `misc` becomes populated and per-instance seed order
/// reintroduces nondeterminism here; sort the map or compare structurally then.
fn fingerprint(ws: &Workspace) -> String {
    #[derive(Serialize)]
    enum Canon {
        File {
            uid: String,
            nodes: Vec<nodes::Nodes>,
        },
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

#[tokio::test]
async fn op_on_a_propagates_identically_to_b() {
    // --- build two nodes from the SAME initial state ---------------------
    // parse the document ONCE: the parser mints fresh uids on every parse, so
    // re-parsing for B would give a different doc uid and break both "start
    // equal" and the EntryMut target (the op carries one uid). instead we clone
    // the workspace — `Workspace: Clone` + `hydrate`'s `Arc::make_mut` give A
    // copy-on-write isolation, so mutating A leaves B untouched.
    let doc = Document::from_reader(SAMPLE.as_bytes()).expect("parse sample");
    let doc_id = doc.uid();

    let mut ws_a =
        Workspace::new_from_entry(Entry::Directory(vec![("a.md".into(), Entry::File(doc))]));
    let mut ws_b = ws_a.clone();

    let hub = LoopbackHub::new();
    let (transport_a, mut rx_a) = hub.node();
    let (_transport_b, mut rx_b) = hub.node();

    // they start EQUAL.
    assert_eq!(
        fingerprint(&ws_a),
        fingerprint(&ws_b),
        "nodes must start from identical state"
    );

    // --- node A: apply locally, then route by lane -----------------------
    // the frontmatter node IS the document root, so its node uid == doc uid.
    // writing at pos 0 prepends "hello " to the title "t" -> "hello t".
    let wire = vec![Op::Workspace(workspace::op::Op::EntryMut {
        entry_id: doc_id,
        op: DocOp::OnUserWrite {
            op_id: OpId::new(1, 1),
            node_id: doc_id,
            pos: 0,
            text: "hello ".into(),
        },
    })];

    let lane = wire[0].lane();
    assert_eq!(lane, Lane::Broadcast, "EntryMut routes on the broadcast lane");

    // encode_batch borrows (`&[Op]`), so we can serialize for the wire and then
    // consume the same op for A's local apply — no Clone on the op needed.
    let bytes = encode_batch(&wire);

    for o in wire {
        if let Op::Workspace(w) = o {
            ws_a.hydrate(std::iter::once(w));
        }
    }

    // after applying to A but BEFORE propagation, A and B DIFFER. this is what
    // makes the test non-tautological: a no-op transport would fail right here.
    assert_ne!(
        fingerprint(&ws_a),
        fingerprint(&ws_b),
        "A diverges from B once the local edit lands"
    );

    transport_a.send(lane, bytes).await.expect("send ok");

    // --- node B: drain inbound, decode, hydrate --------------------------
    let (recv_lane, recv_bytes) = rx_b.recv().await.expect("node B receives the gossip");
    assert_eq!(recv_lane, Lane::Broadcast);

    let ops = decode_batch(&recv_bytes).expect("decode batch");
    for o in ops {
        if let Op::Workspace(w) = o {
            ws_b.hydrate(std::iter::once(w));
        }
    }

    // node A is the sender: it must NOT receive its own gossip.
    assert!(matches!(
        rx_a.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    ));

    // --- convergence: A and B are EQUAL again ----------------------------
    assert_eq!(
        fingerprint(&ws_a),
        fingerprint(&ws_b),
        "nodes converge after the op propagates"
    );
}
