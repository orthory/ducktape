//! THE agreed-total-order milestone, now over REAL commonware-simplex BFT.
//!
//! N=5 validators, each a [`host::Host`] carrying a qmdb-backed `kv` module
//! (whose root is op-log/MMR-order-DEPENDENT) + an in-memory `directory` module,
//! each wrapped in an [`OrderedNode`] over a [`SimplexOrderer`] driving a LIVE
//! simplex [`Engine`](commonware_consensus::simplex::Engine). the ops are
//! distributed ONE PER VALIDATOR (the faithful BFT reading — a validator that
//! never submitted an op still converges via finalization replication). simplex
//! BFT-orders them; every validator applies `host.submit` on finalization in the
//! agreed (ascending-view) order and converges on a BYTE-IDENTICAL app-hash —
//! INCLUDING the order-dependent qmdb root.
//!
//! this is the [`RoundOrderer`] convergence property (see node/tests/
//! agreed_order.rs) now proven over real BFT consensus. `Runner::timed` is the
//! liveness backstop: a wiring bug (unshared store/FIFO/inbox, dropped engine
//! handle, drive loop that never yields) surfaces as a deadline panic — the
//! simplex analog of the arrival-order fork, because you cannot make BFT "not
//! agree".
//!
//! the scenario is SCHEME-PARAMETRIC: `converge` takes a factory producing the
//! sorted identity participant set + index-aligned per-validator schemes, and the
//! full path (propose -> finalize -> ordered delivery -> identical app-hash,
//! qmdb root included) is proven under BOTH genesis-selectable schemes — V1
//! ed25519 and V2 bls multisig (dual-key: ed25519 identities carry transport,
//! bls keys sign votes). only scheme construction differs between the twins.

use std::collections::HashMap;
use std::time::Duration;

use commonware_consensus::simplex::{mocks, scheme::ed25519 as simplex_ed25519};
use commonware_consensus::types::Epoch;
use commonware_cryptography::{Sha256, Signer as _, ed25519};
use commonware_p2p::simulated::{self, Link};
use commonware_runtime::{Clock as _, Quota, Runner as _, Supervisor as _, deterministic};
use commonware_utils::{NZU32, NZUsize};

use consensus::{ContentStore, Digest, SimplexOrderer};
use directory::Directory;
use directory_interface::{DirMsg, encode_msg};
use host::Host;
use kv::Kv;
use kv_interface::{KvMsg, encode};
use node::OrderedNode;
use sdk::Msg;

const N: usize = 5;
const LABELS: [&str; N] = ["v0", "v1", "v2", "v3", "v4"];

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

/// the canonical op-set: distinct origins (so every frame is a distinct digest),
/// distinct kv keys (so the FINAL kv content is identical under any permutation —
/// divergence can only come from LOG ORDER, not content), plus two directory
/// writes for the order-independence contrast. one op per validator.
fn op_set() -> Vec<(commonware_cryptography::ed25519::PrivateKey, u64, Msg)> {
    vec![
        (op_signer(101), 0, kv_set(b"aaa", b"1")),
        (op_signer(102), 0, kv_set(b"bbb", b"2")),
        (op_signer(103), 0, kv_set(b"ccc", b"3")),
        (op_signer(104), 0, dir_set("x", "10")),
        (op_signer(105), 0, dir_set("y", "20")),
    ]
}

/// V1 factory: the mocks ed25519 fixture — N random sorted participants +
/// per-validator schemes (identity key == signing key).
fn ed25519_schemes(
    context: &mut deterministic::Context,
    namespace: &[u8],
) -> (Vec<ed25519::PublicKey>, Vec<simplex_ed25519::Scheme>) {
    let fixture = simplex_ed25519::fixture(context, namespace, N as u32);
    (fixture.participants, fixture.schemes)
}

/// V2 factory: dual-key bls multisig built the PRODUCTION way from dev seeds
/// (the exact path bin/node's "bls-multisig" selector takes) — ed25519 identity
/// keys carry transport + participant order, bls keys sign votes/certificates.
fn bls_schemes(
    _context: &mut deterministic::Context,
    namespace: &[u8],
) -> (Vec<ed25519::PublicKey>, Vec<consensus::BlsScheme>) {
    let seeds: Vec<u64> = (0..N as u64).collect();
    // sort (identity, seed) pairs by identity key so schemes stay index-aligned
    // with the sorted participant vec, matching the fixture's contract.
    let mut pairs: Vec<(ed25519::PublicKey, u64)> = seeds
        .iter()
        .map(|s| (ed25519::PrivateKey::from_seed(*s).public_key(), *s))
        .collect();
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    let participants = pairs.iter().map(|(pk, _)| pk.clone()).collect();
    let schemes = pairs
        .iter()
        .map(|(_, seed)| {
            consensus::bls_dev_scheme(namespace, &seeds, *seed).expect("dev key in the set")
        })
        .collect();
    (participants, schemes)
}

/// the whole N-validator convergence scenario, parameterized by the runtime
/// `context` (so it can be driven under multiple deterministic schedules) and by
/// the scheme factory (so V1 ed25519 and V2 bls run the IDENTICAL scenario).
async fn converge<S, F>(mut context: deterministic::Context, make_schemes: F)
where
    S: commonware_consensus::simplex::scheme::Scheme<Digest, PublicKey = ed25519::PublicKey>,
    F: FnOnce(&mut deterministic::Context, &[u8]) -> (Vec<ed25519::PublicKey>, Vec<S>),
{
    let namespace = b"consensus".to_vec();
    let epoch = Epoch::new(333);
    {
        // N sorted participants + index-aligned per-validator schemes.
        let (participants, schemes) = make_schemes(&mut context, &namespace);

        // ONE simulated network seeded with the participant set (instant,
        // deterministic links — NO authenticated::discovery, which live-locks
        // under the deterministic clock).
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

        // register each validator's engine channels: vote(0)/cert(1)/resolver(2),
        // consumed positionally by `engine.start(vote, cert, resolver)`.
        let quota = Quota::per_second(NZU32!(128));
        let mut registrations = HashMap::new();
        for v in participants.iter() {
            let control = oracle.control(v.clone());
            let vote = control.register(0, quota).await.expect("register vote");
            let certificate = control
                .register(1, quota)
                .await
                .expect("register certificate");
            let resolver = control.register(2, quota).await.expect("register resolver");
            registrations.insert(v.clone(), (vote, certificate, resolver));
        }

        // perfect all-pairs links so votes/certs propagate deterministically.
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

        // ONE shared ContentStore cloned into every SimplexOrderer: every reporter
        // resolves any finalized digest back to its frame bytes. the ORDER still
        // comes purely from simplex finalization; the store only resolves digests.
        let store = ContentStore::new();

        // byte-identical genesis Floor on every validator (else engines never agree
        // on genesis -> deadline panic that looks like a hang).
        let genesis_floor: Digest = mocks::application::genesis::<Sha256>(epoch);

        // stand up N validators, each an OrderedNode over a SimplexOrderer driving
        // its own live engine.
        let mut nodes: Vec<OrderedNode<SimplexOrderer>> = Vec::new();
        for (idx, v) in participants.iter().enumerate() {
            let host = genesis_host(context.child(LABELS[idx])).await;
            let (vote, certificate, resolver) =
                registrations.remove(v).expect("validator registered");
            let orderer = SimplexOrderer::spawn(
                context.child("validator"),
                schemes[idx].clone(),
                oracle.control(v.clone()), // the Control IS the Blocker.
                v.to_string(),             // distinct FS partition per validator.
                epoch,
                genesis_floor,
                store.clone(),
                vote,
                certificate,
                resolver,
            );
            nodes.push(OrderedNode::new(host, orderer));
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

        // distribute the op-set ONE PER VALIDATOR: each digest lands in exactly one
        // node's FIFO, proposed only when that node leads (peek-not-pop keeps it
        // queued across nullified views until then).
        let ops = op_set();
        for (i, (origin, seq, msg)) in ops.iter().enumerate() {
            nodes[i]
                .submit(origin, *seq, msg.clone())
                .await
                .expect("submit");
        }

        // SEMANTIC SHIFT: after every submit, before any finalization drains,
        // EVERY node is still at genesis — nothing was applied optimistically.
        for n in &nodes {
            assert_eq!(
                n.app_hash(),
                genesis,
                "no optimistic echo: submit does not advance app-hash"
            );
        }

        // PUMP: advance simulated time so the spawned engines exchange votes and
        // finalize, then drain every node. stop when ALL nodes have applied the
        // whole op-set — NOT on drain==0 (drain is 0 before the first finalization,
        // so a per-node "drain to 0" fixpoint would complete at genesis and fork).
        let target = ops.len();
        let mut applied = vec![0usize; N];
        loop {
            context.sleep(Duration::from_millis(50)).await;
            for (i, n) in nodes.iter_mut().enumerate() {
                applied[i] += n.drain_delivered().await.expect("drain");
            }
            if applied.iter().all(|&c| c == target) {
                break;
            }
        }

        // THE MILESTONE: byte-identical app-hash on every validator, moved off
        // genesis, INCLUDING the order-dependent qmdb root — under REAL BFT order.
        let converged = nodes[0].app_hash();
        let converged_kv = nodes[0].host().module_root("kv").unwrap();
        assert_ne!(
            converged, genesis,
            "the finalized ops moved the app-hash off genesis"
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
    }
}

#[test]
fn n_validators_converge_under_real_simplex_including_qmdb_root() {
    // timed runner: the liveness backstop. a stall (nothing finalizes) hits the
    // deadline and panics rather than hanging or silently passing at genesis.
    deterministic::Runner::timed(Duration::from_secs(300))
        .start(|context| converge(context, ed25519_schemes));
}

#[test]
fn n_validators_converge_under_bls_multisig_including_qmdb_root() {
    // the V2 twin: the IDENTICAL scenario over the dual-key bls multisig scheme —
    // propose -> finalize -> ordered delivery -> byte-identical app-hash including
    // the order-dependent qmdb root, with ONE aggregated signature per certificate.
    deterministic::Runner::timed(Duration::from_secs(300))
        .start(|context| converge(context, bls_schemes));
}

#[test]
fn convergence_is_robust_across_schedules() {
    // the deterministic runner defaults to a FIXED seed (42), so the headline
    // test proves one schedule. round-robin leaders + perfect links make the
    // result schedule-independent — this pins that down by finalizing under
    // several distinct task-interleaving seeds (each its own 300s liveness bound).
    // V1-only: schedule-robustness is a property of the orderer machinery, not the
    // signature scheme, and bls signing makes each extra schedule much slower.
    for seed in [1u64, 7, 99, 2718] {
        let cfg = deterministic::Config::default()
            .with_seed(seed)
            .with_timeout(Some(Duration::from_secs(300)));
        deterministic::Runner::new(cfg).start(|context| converge(context, ed25519_schemes));
    }
}

/// a deterministic dev signer for test frames (any u64 seed).
fn op_signer(seed: u64) -> commonware_cryptography::ed25519::PrivateKey {
    use commonware_cryptography::Signer as _;
    commonware_cryptography::ed25519::PrivateKey::from_seed(seed)
}
