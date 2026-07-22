//! IN-PROCESS multi-validator convergence — the first mesh-free multi-validator
//! coverage in the repo.
//!
//! Three `OrderedNode<SimplexOrderer>` over `host::Host`s carrying the native
//! `directory` module, each driving a LIVE simplex
//! [`Engine`](commonware_consensus::simplex::Engine), all wired to ONE commonware
//! `simulated::Network` in ONE process — NO OS processes, NO TCP, NO
//! `authenticated::discovery` mesh. The transport bundle each engine consumes is
//! the [`consensus::MeshCarrier`] seam's sim arm ([`consensus::SimMesh`]); the
//! real node wires the discovery-network arm to the IDENTICAL
//! [`SimplexOrderer::spawn_with_carrier`] entry point, so this exercises the exact
//! production spawn path (eager relay + resolver fetch, per-process stores) with
//! only the carrier swapped.
//!
//! Each validator submits ONE distinct directory op; simplex BFT-orders them and
//! every validator applies on finalization. `directory` is order-INDEPENDENT, so
//! the three writes converge on a BYTE-IDENTICAL app-hash under any interleaving —
//! isolating the property under test (multi-validator consensus over the swapped
//! carrier) from op ordering.
//!
//! WAIT DISCIPLINE: the loop terminates on the DELIVERED-FRAME event — it exits
//! exactly when every node has drained the full op-set, never after a fixed number
//! of iterations. The `context.sleep` between drains is the deterministic virtual
//! block-tick (a coarser but still valid cut of `bin/node`'s production drain
//! loop, which flushes pending ops every `DRAIN_TICK` and idle nops every
//! `BLOCK_TIME`), NOT a wall-clock wait — virtual time makes
//! it non-flaky regardless of CI speed. `Runner::timed` is the liveness backstop:
//! a wiring regression (unshared inbox, dropped engine handle, a carrier that
//! hands out a dead channel) surfaces as a deadline panic — you cannot make BFT
//! "not agree", so a stall is the only failure shape.

use std::collections::HashMap;
use std::time::Duration;

use commonware_consensus::simplex::{mocks, scheme::ed25519 as simplex_ed25519};
use commonware_consensus::types::Epoch;
use commonware_cryptography::{Sha256, Signer as _, ed25519};
use commonware_p2p::simulated::{self, Link};
use commonware_runtime::{Clock as _, Quota, Runner as _, Supervisor as _, deterministic};
use commonware_utils::{NZU32, NZUsize};

use consensus::{BLOCK_TIME, ContentStore, Digest, SimMesh, SimplexOrderer};
use directory::Directory;
use directory::{DirMsg, encode_msg};
use host::Host;
use node::OrderedNode;
use sdk::Msg;

const N: usize = 3;
const LABELS: [&str; N] = ["v0", "v1", "v2"];

/// a fresh validator host carrying the native `directory` example module.
fn genesis_host() -> Host {
    Host::genesis(vec![Box::new(Directory::new("directory"))]).expect("genesis")
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

/// one distinct directory op per validator: distinct origins (so every frame is a
/// distinct digest a peer must resolve, not echo) and distinct keys (so the final
/// directory content is identical under any permutation — order-independent).
fn op_set() -> Vec<(ed25519::PrivateKey, u64, Msg)> {
    (0..N)
        .map(|i| {
            (
                ed25519::PrivateKey::from_seed(101 + i as u64),
                0u64,
                dir_set(&format!("k{i}"), &format!("node-{i}")),
            )
        })
        .collect()
}

async fn converge(mut context: deterministic::Context) {
    let namespace = b"consensus".to_vec();
    let epoch = Epoch::new(333);

    // N sorted participants + index-aligned per-validator ed25519 schemes.
    let fixture = simplex_ed25519::fixture(&mut context, &namespace, N as u32);
    let participants = fixture.participants.clone();
    let schemes = fixture.schemes.clone();

    // ONE simulated network seeded with the participant set — the drop-in for the
    // real encrypted-TCP discovery mesh (which live-locks under the deterministic
    // clock). No OS processes, no sockets.
    let (network, oracle) = simulated::Network::new_with_peers(
        context.child("network"),
        simulated::Config {
            max_size: 1024 * 1024,
            disconnect_on_block: true,
            tracked_peer_sets: NZUsize!(1),
        },
        participants.clone(),
    )
    .await;
    network.start();

    // register each validator's mesh carrier: the sim arm bundles the five channel
    // pairs (vote/cert/resolver/payload/fetch) + the oracle's provider/blocker,
    // registered up front (before any engine starts) exactly as the production boot
    // path pre-registers its channel bank.
    let quota = Quota::per_second(NZU32!(128));
    let mut carriers: HashMap<ed25519::PublicKey, SimMesh<deterministic::Context>> = HashMap::new();
    for v in participants.iter() {
        carriers.insert(v.clone(), SimMesh::register(&oracle, v.clone(), quota).await);
    }

    // perfect all-pairs links so votes/certs/relayed payloads propagate.
    let link = Link {
        latency: Duration::from_millis(10),
        jitter: Duration::from_millis(1),
        success_rate: 1.0,
    };
    for v1 in participants.iter() {
        for v2 in participants.iter() {
            if v1 == v2 {
                continue;
            }
            oracle
                .add_link(v1.clone(), v2.clone(), link.clone())
                .await
                .expect("link validators");
        }
    }

    // byte-identical genesis Floor on every validator (else engines never agree on
    // genesis and the deadline panics).
    let genesis_floor: Digest = mocks::application::genesis::<Sha256>(epoch);

    // stand up N validators, each an OrderedNode over a SimplexOrderer built from
    // its carrier through the SAME spawn_with_carrier the real node calls.
    let mut nodes: Vec<OrderedNode<SimplexOrderer>> = Vec::new();
    for (idx, v) in participants.iter().enumerate() {
        let carrier = carriers.remove(v).expect("validator carrier registered");
        // PER-PROCESS store (the production path): the leader gossips its proposed
        // frame's bytes on the payload channel, peers cache them, and the resolver
        // fetch backstops any miss — the carrier owns the wiring.
        let orderer = SimplexOrderer::spawn_with_carrier(
            context.child(LABELS[idx]),
            schemes[idx].clone(),
            carrier,
            v.clone(),
            v.to_string(),
            epoch,
            genesis_floor,
            None,
            ContentStore::new(),
            false,
        );
        nodes.push(OrderedNode::new(genesis_host(), orderer));
    }

    // identical genesis module set -> identical genesis app-hash.
    let genesis = nodes[0].app_hash();
    for n in &nodes {
        assert_eq!(n.app_hash(), genesis, "identical genesis -> identical app-hash");
    }

    // distribute the op-set ONE PER VALIDATOR: each digest lands in exactly one
    // node's FIFO, proposed only when that node leads.
    let ops = op_set();
    for (i, (origin, seq, msg)) in ops.iter().enumerate() {
        nodes[i]
            .submit(origin, *seq, msg.clone())
            .await
            .expect("submit");
    }

    // no optimistic echo: after every submit, before any finalization drains, every
    // node is still at genesis.
    for n in &nodes {
        assert_eq!(
            n.app_hash(),
            genesis,
            "no optimistic echo: submit does not advance app-hash"
        );
    }

    // PUMP to convergence: exit is EVENT-DRIVEN — the moment every node has drained
    // the full op-set (delivered frames == target). The block-tick advances the
    // deterministic sim so the engines exchange votes and finalize.
    let target = ops.len();
    let mut applied = [0usize; N];
    while applied.iter().any(|&c| c < target) {
        context.sleep(BLOCK_TIME).await;
        for (i, n) in nodes.iter_mut().enumerate() {
            n.flush_batch().await.expect("flush");
            applied[i] += n.drain_delivered().await.expect("drain");
        }
    }

    // THE MILESTONE: byte-identical app-hash on every validator, moved off genesis,
    // reached with no OS-process mesh — only the swapped in-process carrier.
    let converged = nodes[0].app_hash();
    assert_ne!(converged, genesis, "the finalized ops moved the app-hash off genesis");
    for (i, n) in nodes.iter().enumerate() {
        assert_eq!(applied[i], target, "validator {i} applied EXACTLY the op-set");
        assert_eq!(
            n.app_hash(),
            converged,
            "validator {i} converges on the identical app-hash"
        );
    }
}

#[test]
fn three_validators_converge_in_process() {
    deterministic::Runner::timed(Duration::from_secs(120)).start(converge);
}

#[test]
fn in_process_convergence_is_robust_across_schedules() {
    // convergence must not depend on one lucky task interleaving: round-robin
    // leaders + perfect links make the result schedule-independent — pin it down
    // under several distinct seeds (each its own 120s liveness bound).
    for seed in [1u64, 7, 99] {
        let cfg = deterministic::Config::default()
            .with_seed(seed)
            .with_timeout(Some(Duration::from_secs(120)));
        deterministic::Runner::new(cfg).start(converge);
    }
}
