//! consensus-lane convergence through the [`Engine`] dispatcher.
//!
//! phase-0's spine only crossed the BROADCAST lane (a document `EntryMut`). this
//! proves the CONSENSUS lane end-to-end + the structural apply path:
//! - the op is `Op::Workspace(workspace::op::Op::AddEntry { path, entry })`,
//!   whose `lane()` is `Lane::Consensus` (only `EntryMut` rides broadcast; every
//!   other workspace op rides consensus).
//! - it routes through `Engine::apply`, which folds it into the workspace via
//!   `Workspace::hydrate` — the same structural `AddEntry` path the real node
//!   will use.
//!
//! convergence holds because `AddEntry` carries an explicit `path`, and that
//! path survives the serde round-trip through `encode_batch`/`decode_batch`. so
//! A (applying the original op) and B (applying the decoded op) insert the same
//! entry at the same path in the same root directory.
//!
//! the fingerprint mirrors `transport/tests/convergence.rs`: a canonical,
//! hashmap-order-independent string over the tree.

use document::Document;
use engine::Engine;
use op::{Lane, Op};
use serde::Serialize;
use transport::{LoopbackHub, Transport, decode_batch, encode_batch};
use uid::Identify;
use workspace::{Entry, Workspace};

// a minimal valid document: frontmatter only. `from_reader` mints the uids.
const SAMPLE: &str = "---\ntitle: t\nauthor: @a\ncreated_at: 1\nupdated_at: 1\n---\n";

/// canonical, hashmap-order-independent fingerprint of a workspace's tree.
/// (same shape as transport/tests/convergence.rs — see its NOTE on why this is
/// deterministic for the frontmatter-only fixture.)
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
async fn consensus_add_entry_propagates_identically_to_b() {
    // --- two engines from the SAME initial state ------------------------
    // parse the base document ONCE then CLONE the workspace for B: re-parsing
    // would mint fresh uids and break "start equal". (mirrors the spine.)
    let base = Document::from_reader(SAMPLE.as_bytes()).expect("parse base");
    let ws_a =
        Workspace::new_from_entry(Entry::Directory(vec![("a.md".into(), Entry::File(base))]));
    let ws_b = ws_a.clone();

    let mut engine_a = Engine::new(ws_a);
    let mut engine_b = Engine::new(ws_b);

    let hub = LoopbackHub::new();
    let (transport_a, mut rx_a) = hub.node();
    let (_transport_b, mut rx_b) = hub.node();

    // they start EQUAL.
    assert_eq!(
        fingerprint(engine_a.workspace()),
        fingerprint(engine_b.workspace()),
        "engines must start from identical state"
    );

    // --- the consensus-lane op: add a brand-new document entry ----------
    // the new doc is parsed once into a concrete `Document` carried by the op;
    // the op also carries an explicit `path`, which survives the wire, so both
    // nodes insert the entry at the same place in the tree.
    let new_doc = Document::from_reader(SAMPLE.as_bytes()).expect("parse new doc");
    let wire = vec![Op::Workspace(workspace::op::Op::AddEntry {
        path: "new.md".into(),
        entry: Entry::File(new_doc),
    })];

    let lane = wire[0].lane();
    assert_eq!(
        lane,
        Lane::Consensus,
        "structural AddEntry routes on the consensus lane"
    );

    // encode borrows; then we consume each op for A's local apply (Op: !Clone).
    let bytes = encode_batch(&wire);

    for o in wire {
        let follow_ups = engine_a.apply(o).expect("engine A applies the op");
        assert!(follow_ups.is_empty(), "AddEntry emits no follow-up ops");
    }

    // after A applies but BEFORE propagation, A and B DIFFER. this is the
    // non-tautology guard: a no-op transport fails right here.
    assert_ne!(
        fingerprint(engine_a.workspace()),
        fingerprint(engine_b.workspace()),
        "A diverges from B once the new entry lands"
    );

    transport_a.send(lane, bytes).await.expect("send ok");

    // --- node B: drain inbound, decode, apply ---------------------------
    let (recv_lane, recv_bytes) = rx_b.recv().await.expect("node B receives the op");
    // prove the consensus lane was crossed over the wire, not just asserted on A.
    assert_eq!(
        recv_lane,
        Lane::Consensus,
        "the op arrives at B on the consensus lane"
    );

    let ops = decode_batch(&recv_bytes).expect("decode batch");
    for o in ops {
        engine_b.apply(o).expect("engine B applies the op");
    }

    // node A is the sender: it must NOT receive its own op.
    assert!(matches!(
        rx_a.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    ));

    // --- convergence: A and B are EQUAL again ---------------------------
    assert_eq!(
        fingerprint(engine_a.workspace()),
        fingerprint(engine_b.workspace()),
        "engines converge after the AddEntry propagates"
    );
}
