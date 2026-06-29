//! the engine→net edge proof — `engine::Node` over a production
//! `net::CommonwareTransport`.
//!
//! every other net test drives the transport in isolation; this integration test
//! is the FIRST to compose the two crates that the migration keeps apart: it
//! stands up N real production nodes via [`CommonwareTransport::new`] (real
//! `authenticated::discovery`, real ed25519 handshakes, real localhost TCP, real
//! per-node FS storage, a live simplex BFT engine) and wires each one into an
//! [`engine::Node`]. driving a single broadcast op into node 0 then converges at
//! node 1 over the wire — proving the `net` crate is no longer an island.
//!
//! a separate integration-test crate only sees `net` + dev-deps, so this names
//! every crate it touches (engine/workspace/document/op/transport + the
//! commonware runtime/crypto/macros) and Cargo.toml re-declares them as dev-deps.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use commonware_cryptography::{ed25519, Signer};
use commonware_macros::select;
use commonware_runtime::{Clock, Runner, Spawner, Supervisor};
use document::Document;
use engine::{Engine, Node};
use net::CommonwareTransport;
use op::Lane;
use transport::{encode_batch, Transport};
use workspace::{Entry, Workspace};

/// a frontmatter-only seed doc so every node starts from a byte-identical 1-entry
/// tree. copied from engine's `run_loopback_demo` — `Document` isn't `Clone`, so
/// each node parses its own from this source.
const SEED: &str = "---\ntitle: demo\nauthor: @a\ncreated_at: 1\nupdated_at: 1\n---\n";

/// runtime nesting spike — proves `tokio::spawn` works INSIDE commonware's tokio
/// `Runner`. [`Node::new`] spawns its inbound + outbound-forwarder tasks via
/// `tokio::spawn`, so if a nested spawn hung or failed under the Runner, the whole
/// edge would be dead before it started. plain `#[test]` (NOT ignored): binds no
/// sockets, touches no fs, must always pass — it's the cheap canary for the
/// expensive real-socket proof below.
#[test]
fn runtime_nesting_spike() {
    let executor = commonware_runtime::tokio::Runner::default();
    executor.start(|_ctx| async move {
        let v = tokio::spawn(async { 42u32 }).await.unwrap();
        assert_eq!(v, 42);
    });
}

/// PROOF — an `engine::Node` over a production `CommonwareTransport` converges a
/// broadcast op across real localhost TCP.
///
/// N production nodes via [`CommonwareTransport::new`], each wrapped in a
/// [`Node`]. node 0 is driven an `AddEntry`; node 1's tree grows from 1 entry
/// (seed) to 2 (seed + added) once the op crosses the wire.
///
/// LANE NOTE (the honest mechanism): `AddEntry` routes to [`Lane::Consensus`]
/// (only `EntryMut` is `Broadcast` — see `op::Op::lane`). so
/// [`Node::apply_local_direct`] applies node 0's local echo and sends ONCE on the
/// consensus lane — which exercises Node's real outbound seam over the live
/// simplex engine, but does NOT deliver the payload to node 1 (the engine's
/// `ConsensusRelay::broadcast` is a no-op for non-proposers). convergence at node
/// 1 is carried entirely by the explicit Broadcast resend loop: Node's inbound
/// task is lane-agnostic (it decodes any batch regardless of the lane tag), so
/// the same `AddEntry` bytes gossiped on `Broadcast` apply identically at node 1.
///
/// `#[ignore]` keeps the default `cargo test -p net` hermetic; run explicitly with
/// `cargo test -p net --test node_edge -- --ignored`.
#[test]
#[ignore = "real-socket: binds localhost TCP + writes FS; run with --ignored"]
fn node_converges_over_real_commonware() {
    // n=5: the production `new()` builds a live simplex engine, which wants the
    // canonical BFT participant count (3f+1). the engines idle here (this proof
    // rides the broadcast lane) but must stand up cleanly.
    const N: usize = 5;
    // distinct from the existing probes' 52111/52120 so a concurrent run can't
    // clash on a port.
    const BASE_PORT: u16 = 52140;

    let executor = commonware_runtime::tokio::Runner::default();
    executor.start(|context| async move {
        // authorized peer set: N ed25519 identities from seeds 0..N. every node
        // agrees on this set; all but node 0 bootstrap off node 0's address.
        let keys: Vec<ed25519::PublicKey> = (0..N as u64)
            .map(|i| ed25519::PrivateKey::from_seed(i).public_key())
            .collect();
        let node0_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), BASE_PORT);

        // stand up N real production nodes. each Node OWNS its transport + inbox
        // (both moved in); we keep every engine `Handle` alive in `handles` — a
        // dropped handle aborts that node's simplex engine, which would make it
        // not a faithful production node. per-node `with_attribute` keeps each
        // node's metric paths distinct (one shared child label would collide in
        // the registry).
        let mut nodes = Vec::new();
        let mut handles = Vec::new();
        let mut node0_tp = None;
        for i in 0..N {
            let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), BASE_PORT + i as u16);
            let bootstrappers = if i == 0 {
                Vec::new()
            } else {
                vec![(keys[0].clone(), node0_addr)]
            };
            let cfg = net::Config {
                seed: i as u64,
                namespace: b"ducktape-edge-proof".to_vec(),
                listen: addr,
                advertised: addr,
                peers: keys.clone(),
                bootstrappers,
            };
            let node_ctx = context.child("node").with_attribute("index", i);
            let (transport, inbox, handle) = CommonwareTransport::new(node_ctx, cfg);

            // node 0 drives the op; grab a transport clone for the resend loop
            // BEFORE the transport moves into the Node.
            if i == 0 {
                node0_tp = Some(transport.clone());
            }

            // each node parses its OWN seed doc (Document isn't Clone) so all
            // start from a byte-identical 1-entry tree.
            let seed_doc = Document::from_reader(SEED.as_bytes()).expect("seed doc parses");
            let ws = Workspace::new_from_entry(Entry::Directory(vec![(
                "seed.md".into(),
                Entry::File(seed_doc),
            )]));

            let node = Node::new(
                engine::Config::from_toml_str(&format!("id = {i}\n")).unwrap(),
                Engine::new(ws),
                transport,
                inbox,
            );
            nodes.push(node);
            handles.push(handle);
        }
        let node0_tp = node0_tp.expect("node 0 transport clone");

        // drive an AddEntry into node 0 (see the LANE NOTE on the fn). op::Op is
        // !Clone, so encode the wire bytes FIRST — apply consumes the op.
        let new_doc = Document::from_reader(SEED.as_bytes()).expect("driven doc parses");
        let op = op::Op::Workspace(workspace::op::Op::AddEntry {
            path: "added-by-0.md".into(),
            entry: Entry::File(new_doc),
        });
        let bytes = encode_batch(std::slice::from_ref(&op));
        nodes[0]
            .apply_local_direct(op)
            .await
            .expect("node 0 applies + sends on its lane");

        // resend on a loop: a single Recipients::All gossip before the discovery
        // mesh forms reaches nobody and is silently dropped, so keep re-gossiping
        // the AddEntry on the Broadcast lane until a peer's inbound drain delivers
        // it — exactly how commonware's own connectivity test drives sends. bind
        // to a NAMED handle: a bare `_` would drop + abort the task instantly.
        let resend_tp = node0_tp.clone();
        let resend_bytes = bytes.clone();
        let _resend = context.child("resend").spawn(move |ctx| async move {
            loop {
                let _ = resend_tp.send(Lane::Broadcast, resend_bytes.clone()).await;
                ctx.sleep(Duration::from_millis(250)).await;
            }
        });

        // node 1 must converge: its tree grows from 1 (seed) to 2 (seed + added).
        // capture the count at the FIRST crossing and break with it — AddEntry is
        // non-idempotent (insert_at_path does items.push), so a later 250ms resend
        // would push it to 3; but we're parked in wait_inbound before the first
        // batch lands and snapshot in microseconds, so the captured value is
        // exactly 2. a converged mesh delivers in well under a second; the 60s
        // ceiling only fails-fast a non-converging discovery instead of hanging.
        let node1 = &nodes[1];
        select! {
            converged = async {
                loop {
                    node1.wait_inbound().await;
                    let ws = node1.workspace_snapshot().await;
                    let count = match ws.root().as_ref() {
                        Entry::Directory(items) => items.len(),
                        Entry::File(_) => 1,
                    };
                    if count >= 2 {
                        break count;
                    }
                }
            } => {
                assert_eq!(
                    converged, 2,
                    "node 1 converged to seed + added = 2 top-level entries"
                );
            },
            _timeout = context.sleep(Duration::from_secs(60)) => {
                panic!(
                    "AddEntry did not converge at node 1 over real sockets within 60s — \
                     discovery never formed the mesh through production new()"
                );
            },
        }

        // hold everything until the assert resolves, then drop together (dropping
        // a handle aborts that node's engine; dropping a node drops its transport
        // + inbox).
        drop(nodes);
        drop(handles);
    });
}
