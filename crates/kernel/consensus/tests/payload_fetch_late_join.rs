//! LATE-JOIN / STARVED-NODE catch-up over the lazy commonware-resolver fetch path.
//!
//! `byzantine_payload_flood.rs` proves the EAGER path: the leader's relay gossips
//! a proposed frame's bytes, every peer caches them store-only ahead of
//! finalization, so `store.get(finalized_digest)` resolves on every node online
//! during the proposer's turn. this harness proves the MISS path: ONE validator
//! is STARVED — its eager payload drain is black-holed, so it caches NOTHING it
//! did not originate. it still finalizes every digest via consensus (verify is
//! trivially true, so it votes without holding payloads), so every finalization
//! for someone else's op is a `store.get -> None` MISS. instead of dropping, its
//! reporter FETCHES the bytes through a `commonware_resolver::p2p::Engine` on a
//! dedicated channel; the fetched bytes are verified (content-addressed), stored,
//! and released by the ordered gate ONLY in their finalization slot.
//!
//! the discriminating assertion: the starved node converges on the byte-identical
//! ORDER-DEPENDENT qmdb root. fetches complete in arbitrary order while later
//! views keep finalizing, so without the ordered-release gate the starved node
//! would apply out of order and diverge. content-addressing (garbage fetch
//! rejected) is pinned by the `payload_consumer_rejects_*` lib unit test.

use std::collections::HashMap;
use std::time::Duration;

use commonware_consensus::simplex::{mocks, scheme::ed25519 as simplex_ed25519};
use commonware_consensus::types::Epoch;
use commonware_cryptography::Sha256;
use commonware_p2p::simulated::{self, Link};
use commonware_runtime::{deterministic, Clock as _, Quota, Runner as _, Supervisor as _};
use commonware_utils::{NZUsize, NZU32};

use consensus::{ContentStore, Digest, SimplexOrderer};
use directory::Directory;
use directory_interface::{encode_msg, DirMsg};
use host::Host;
use kv::Kv;
use kv_interface::{encode, KvMsg};
use node::OrderedNode;
use sdk::Msg;

const N: usize = 5;
const LABELS: [&str; N] = ["v0", "v1", "v2", "v3", "v4"];
/// the single STARVED validator: a full consensus participant whose eager payload
/// drain is black-holed, so it fetches every non-originated finalized op.
const STARVED: usize = 3;

async fn genesis_host(ctx: deterministic::Context) -> Host {
    let kv = Kv::init(ctx, "kv").await;
    Host::genesis(vec![Box::new(kv), Box::new(Directory::new("directory"))]).expect("genesis")
}

fn kv_set(k: &[u8], v: &[u8]) -> Msg {
    Msg { target: "kv".into(), payload: encode(&KvMsg::Set { key: k.to_vec(), value: v.to_vec() }) }
}

fn dir_set(k: &str, v: &str) -> Msg {
    Msg { target: "directory".into(), payload: encode_msg(&DirMsg::Set { key: k.into(), value: v.into() }) }
}

/// distinct origins/keys so the final kv CONTENT is permutation-invariant
/// (divergence can only come from LOG ORDER), plus two order-INDEPENDENT dir
/// writes. one op per validator — the starved node originates op `D`.
fn op_set() -> Vec<(Vec<u8>, u64, Msg)> {
    vec![
        (b"A".to_vec(), 0, kv_set(b"aaa", b"1")),
        (b"B".to_vec(), 0, kv_set(b"bbb", b"2")),
        (b"C".to_vec(), 0, kv_set(b"ccc", b"3")),
        (b"D".to_vec(), 0, dir_set("x", "10")),
        (b"E".to_vec(), 0, dir_set("y", "20")),
    ]
}

async fn run_late_join(mut context: deterministic::Context) {
    let namespace = b"consensus".to_vec();
    let epoch = Epoch::new(333);

    let fixture = simplex_ed25519::fixture(&mut context, &namespace, N as u32);
    let participants = fixture.participants.clone();
    let schemes = fixture.schemes.clone();

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

    // vote(0)/cert(1)/resolver(2) — consumed positionally by engine.start — plus
    // the eager payload channel(3) the relay gossips on AND the catch-up fetch
    // channel(4) the resolver engine runs on. all links registered per pair below.
    let quota = Quota::per_second(NZU32!(128));
    let mut registrations = HashMap::new();
    let mut payload_chans = HashMap::new();
    let mut fetch_chans = HashMap::new();
    for v in participants.iter() {
        let control = oracle.control(v.clone());
        let vote = control.register(0, quota).await.expect("register vote");
        let certificate = control.register(1, quota).await.expect("register certificate");
        let resolver = control.register(2, quota).await.expect("register resolver");
        let payload = control.register(3, quota).await.expect("register payload");
        let fetch = control.register(4, quota).await.expect("register fetch");
        registrations.insert(v.clone(), (vote, certificate, resolver));
        payload_chans.insert(v.clone(), payload);
        fetch_chans.insert(v.clone(), fetch);
    }

    let link = Link { latency: Duration::from_millis(10), jitter: Duration::from_millis(1), success_rate: 1.0 };
    for v1 in participants.iter() {
        for v2 in participants.iter() {
            if v1 == v2 {
                continue;
            }
            oracle.add_link(v1.clone(), v2.clone(), link.clone()).await.expect("link validators");
        }
    }

    let genesis_floor: Digest = mocks::application::genesis::<Sha256>(epoch);

    let mut nodes: Vec<OrderedNode<SimplexOrderer>> = Vec::new();
    for (idx, v) in participants.iter().enumerate() {
        let host = genesis_host(context.child(LABELS[idx])).await;
        let (vote, certificate, resolver) = registrations.remove(v).expect("validator registered");
        let payload = payload_chans.remove(v).expect("validator payload channel");
        let fetch = fetch_chans.remove(v).expect("validator fetch channel");

        // PER-PROCESS store. all nodes run the resolver engine (both serve + fetch);
        // only STARVED black-holes its eager drain, so it fetches every op it did
        // not originate. the others cache eagerly and only SERVE via the resolver.
        let store = ContentStore::new();
        let orderer = SimplexOrderer::spawn_with_resolver(
            context.child("validator"),
            schemes[idx].clone(),
            oracle.control(v.clone()),
            oracle.manager(),
            v.clone(),
            v.to_string(),
            epoch,
            genesis_floor,
            store,
            vote,
            certificate,
            resolver,
            payload,
            fetch,
            idx == STARVED,
        );
        nodes.push(OrderedNode::new(host, orderer));
    }

    let genesis = nodes[0].app_hash();
    for n in &nodes {
        assert_eq!(n.app_hash(), genesis, "identical genesis -> identical app-hash");
    }

    // the order-INDEPENDENT directory root the two legit dir writes MUST produce,
    // computed locally with NO consensus — the "only legit ops landed" anchor.
    let expected_dir = {
        let mut h = genesis_host(context.child("baseline")).await;
        h.submit(dir_set("x", "10")).await.expect("baseline dir x");
        h.submit(dir_set("y", "20")).await.expect("baseline dir y");
        h.module_root("directory").unwrap()
    };

    let ops = op_set();
    for (i, (origin, seq, msg)) in ops.iter().enumerate() {
        nodes[i].submit(origin, *seq, msg.clone()).await.expect("submit");
    }

    // PUMP to convergence: stop only when EVERY node (incl. the starved one, which
    // must FETCH four of five ops) has applied the whole op-set.
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

    // ---- convergence: identical CORRECT state on every validator, incl. the
    // starved one that reached its ops purely through the resolver fetch path ----
    let converged = nodes[0].app_hash();
    let converged_kv = nodes[0].host().module_root("kv").unwrap();
    assert_ne!(converged, genesis, "the finalized ops moved the app-hash off genesis");
    for (i, n) in nodes.iter().enumerate() {
        assert_eq!(applied[i], target, "validator {i} applied EXACTLY the finalized ops");
        assert_eq!(n.app_hash(), converged, "validator {i} converges on the identical app-hash");
        assert_eq!(
            n.host().module_root("kv").unwrap(),
            converged_kv,
            "validator {i} converges on the identical ORDER-DEPENDENT qmdb root"
        );
        assert_eq!(
            n.host().module_root("directory").unwrap(),
            expected_dir,
            "validator {i} directory root reflects only the legit dir writes"
        );
    }
}

#[test]
fn starved_node_fetches_missing_payloads_and_converges() {
    deterministic::Runner::timed(Duration::from_secs(300)).start(run_late_join);
}

#[test]
fn late_join_fetch_convergence_is_robust_across_schedules() {
    // convergence via fetch must not depend on one lucky interleaving: the starved
    // node's fetches complete in schedule-dependent order while later views keep
    // finalizing, so the ordered gate is exercised differently under each seed.
    for seed in [1u64, 7, 99, 2718] {
        let cfg = deterministic::Config::default()
            .with_seed(seed)
            .with_timeout(Some(Duration::from_secs(300)));
        deterministic::Runner::new(cfg).start(run_late_join);
    }
}
