//! a validator JOIN that actually changes the CONSENSUS participant set.
//!
//! commonware simplex has NO native per-epoch validator-set reconfiguration: the
//! participant set is owned by the `scheme` (certificate::Scheme::participants ->
//! &Set, no epoch arg) and the elector, both FIXED at `Engine::new`; `cfg.epoch`
//! is a single scalar cloned into every actor. there is no Supervisor returning
//! participants(epoch). so a join is a TEAR-DOWN + RE-SPAWN at an epoch boundary:
//! the old engine finalizes up to a cutover, then every node drops its engine and
//! spins up a NEW one over the new (scheme, participants, epoch) at a fresh
//! `Floor::Genesis(genesis(new_epoch))` (an old-epoch finalized floor would fail
//! `Floor::assert`, which pins the floor cert to the new epoch).
//!
//! this proves the mechanism end to end at the raw `SimplexOrderer` seam — no
//! host/qmdb, so there is no app-state replay order to fork on; the ONLY property
//! under test is consensus membership:
//!
//!   epoch 0: 4 validators (v0..v3). an op submitted ONLY to v0 finalizes on all
//!            four -> the old set is live.
//!   RECONFIG: v4 joins. drop the four engines; respawn FIVE engines (v0..v4) at
//!            epoch 1 over the 5-key scheme + a fresh epoch-1 genesis floor.
//!   epoch 1: an op submitted ONLY to v4 (the NEW validator) finalizes on all
//!            FIVE. B could only finalize if v4 led a view AND the four incumbents
//!            counted its proposal toward quorum -> the join changed the consensus
//!            set. that is incorporation; root-hash-free, unforgeable.
//!
//! schemes are built the PRODUCTION way (`Scheme::signer`, as bin/node does), not
//! the mocks fixture, so the 4-subset and 5-superset share identical keys. the
//! scenario is SCHEME-PARAMETRIC: `scenario` takes a `(namespace, subset, member)
//! -> scheme` factory — the respawn contract itself is scheme-independent, which
//! is exactly the point; today only V1 ed25519 is wired.

use std::collections::HashMap;
use std::time::Duration;

use commonware_consensus::simplex::{mocks, scheme::ed25519 as simplex_ed25519};
use commonware_consensus::types::Epoch;
use commonware_cryptography::{Sha256, Signer, ed25519};
use commonware_p2p::simulated::{self, Link};
use commonware_runtime::{Clock as _, Quota, Runner as _, Supervisor as _, deterministic};
use commonware_utils::{NZU32, NZUsize, ordered::Set};

use consensus::{Cadence, ContentStore, SimplexOrderer};

/// the beat the sim runs at — simulated time, so its size is not wall-clock.
const CADENCE: Cadence = Cadence::from_millis(1_000);
use node::Orderer as _;

/// recover the dev seed behind a deterministic identity (all five live in 0..5).
fn seed_of(v: &ed25519::PublicKey) -> u64 {
    (0..5u64)
        .find(|s| ed25519::PrivateKey::from_seed(*s).public_key() == *v)
        .expect("known dev key")
}

/// V1 factory: the production ed25519 signer over the epoch's subset — identical
/// keys across the 4-set and the 5-superset.
fn ed25519_scheme_for(
    namespace: &[u8],
    participants: &Set<ed25519::PublicKey>,
    v: &ed25519::PublicKey,
) -> simplex_ed25519::Scheme {
    let sk = ed25519::PrivateKey::from_seed(seed_of(v));
    simplex_ed25519::Scheme::signer(namespace, participants.clone(), sk)
        .expect("member is in the set")
}

/// drive the whole join-by-respawn scenario under one deterministic schedule,
/// parametric over the consensus scheme (`scheme_for` builds each member's signer
/// over an epoch's participant subset).
async fn scenario<S, F>(context: deterministic::Context, scheme_for: F)
where
    S: commonware_consensus::simplex::scheme::Scheme<
            consensus::Digest,
            PublicKey = ed25519::PublicKey,
        >,
    F: Fn(&[u8], &Set<ed25519::PublicKey>, &ed25519::PublicKey) -> S,
{
    let namespace = b"valset-reconfig".to_vec();
    let epoch0 = Epoch::new(0);
    let epoch1 = Epoch::new(1);

    // five deterministic identities. the joiner is seed 4; the epoch-0 set is the
    // first four, the epoch-1 set is all five. building schemes from these keys
    // directly (not a fixture) keeps the 4-subset and 5-superset key-identical.
    let keys: Vec<ed25519::PrivateKey> = (0..5u64).map(ed25519::PrivateKey::from_seed).collect();
    let pubs: Vec<ed25519::PublicKey> = keys.iter().map(|k| k.public_key()).collect();

    let participants4: Set<ed25519::PublicKey> =
        Set::try_from(pubs[0..4].to_vec()).expect("no dup 4-set");
    let participants5: Set<ed25519::PublicKey> = Set::try_from(pubs.clone()).expect("no dup 5-set");
    let joiner = pubs[4].clone(); // in the 5-set only.
    let epoch0_submitter = pubs[0].clone(); // any incumbent.

    // ONE simulated network seeded with ALL FIVE peers up front (transport is a
    // separate concern from consensus membership: the 5th peer is reachable from
    // the start; only the engine's SCHEME decides who counts toward quorum).
    let (network, oracle) = simulated::Network::new_with_peers(
        context.child("network"),
        simulated::Config {
            max_size: 1024 * 1024,
            disconnect_on_block: true,
            tracked_peer_sets: NZUsize!(1),
        },
        participants5.clone(),
    )
    .await;
    network.start();

    // register BOTH engine channel-triples up front: epoch 0 on 0/1/2 (the four
    // incumbents) and epoch 1 on 10/11/12 (all five). distinct channels mean an
    // aborted epoch-0 engine can never collide with its epoch-1 successor.
    let quota = Quota::per_second(NZU32!(128));
    let mut reg0 = HashMap::new();
    for v in participants4.iter() {
        let c = oracle.control(v.clone());
        let vote = c.register(0, quota).await.expect("reg vote e0");
        let cert = c.register(1, quota).await.expect("reg cert e0");
        let res = c.register(2, quota).await.expect("reg res e0");
        reg0.insert(v.clone(), (vote, cert, res));
    }
    let mut reg1 = HashMap::new();
    for v in participants5.iter() {
        let c = oracle.control(v.clone());
        let vote = c.register(10, quota).await.expect("reg vote e1");
        let cert = c.register(11, quota).await.expect("reg cert e1");
        let res = c.register(12, quota).await.expect("reg res e1");
        reg1.insert(v.clone(), (vote, cert, res));
    }

    // perfect all-pairs links among all five, so votes/certs propagate for BOTH
    // epochs deterministically.
    let link = Link {
        latency: Duration::from_millis(10),
        jitter: Duration::from_millis(1),
        success_rate: 1.0,
    };
    for a in participants5.iter() {
        for b in participants5.iter() {
            if a != b {
                oracle
                    .add_link(a.clone(), b.clone(), link.clone())
                    .await
                    .expect("link");
            }
        }
    }

    // ONE shared content store (the in-sim payload shortcut): the ORDER still
    // comes purely from finalization; the store only resolves finalized digests.
    let store = ContentStore::new();

    // ---- epoch 0: the four-validator set is live ----
    let genesis0 = mocks::application::genesis::<Sha256>(epoch0);
    let mut e0: HashMap<ed25519::PublicKey, SimplexOrderer> = HashMap::new();
    for v in participants4.iter() {
        let scheme = scheme_for(&namespace, &participants4, v);
        let (vote, cert, res) = reg0.remove(v).expect("registered e0");
        let orderer = SimplexOrderer::spawn(
            context.child("e0"),
            scheme,
            oracle.control(v.clone()),
            format!("{v}-e0"),
            epoch0,
            genesis0,
            store.clone(),
            CADENCE,
            vote,
            cert,
            res,
        );
        e0.insert(v.clone(), orderer);
    }

    // op A: submitted to ONE incumbent, must finalize on all four.
    let op_a = b"epoch0-op-from-an-incumbent".to_vec();
    e0.get_mut(&epoch0_submitter)
        .unwrap()
        .submit(op_a.clone())
        .await
        .expect("submit A");

    let mut a_seen: HashMap<ed25519::PublicKey, bool> =
        participants4.iter().map(|v| (v.clone(), false)).collect();
    loop {
        context.sleep(Duration::from_millis(50)).await;
        for (v, o) in e0.iter_mut() {
            for (_view, bytes) in o.poll_delivered() {
                if bytes == op_a {
                    a_seen.insert(v.clone(), true);
                }
            }
        }
        if a_seen.values().all(|&s| s) {
            break;
        }
    }

    // ---- RECONFIG: tear the epoch-0 engines down ----
    drop(e0); // each SimplexOrderer holds the engine keepalive; drop aborts them.

    // ---- epoch 1: v4 has joined; respawn the FIVE-validator set ----
    let genesis1 = mocks::application::genesis::<Sha256>(epoch1);
    let mut e1: HashMap<ed25519::PublicKey, SimplexOrderer> = HashMap::new();
    for v in participants5.iter() {
        let scheme = scheme_for(&namespace, &participants5, v);
        let (vote, cert, res) = reg1.remove(v).expect("registered e1");
        let orderer = SimplexOrderer::spawn(
            context.child("e1"),
            scheme,
            oracle.control(v.clone()),
            format!("{v}-e1"),
            epoch1,
            genesis1,
            store.clone(),
            CADENCE,
            vote,
            cert,
            res,
        );
        e1.insert(v.clone(), orderer);
    }

    // op B: submitted to the JOINER ONLY. B finalizing on all five is the proof —
    // v4 had to lead a view and the four incumbents had to count its proposal.
    let op_b = b"epoch1-op-from-the-joiner-v4".to_vec();
    e1.get_mut(&joiner)
        .unwrap()
        .submit(op_b.clone())
        .await
        .expect("submit B");

    let mut b_seen: HashMap<ed25519::PublicKey, bool> =
        participants5.iter().map(|v| (v.clone(), false)).collect();
    loop {
        context.sleep(Duration::from_millis(50)).await;
        for (v, o) in e1.iter_mut() {
            for (_view, bytes) in o.poll_delivered() {
                if bytes == op_b {
                    b_seen.insert(v.clone(), true);
                }
            }
        }
        if b_seen.values().all(|&s| s) {
            break;
        }
    }

    // if we got here the loops converged inside the deadline: the joiner's op was
    // finalized by the reconfigured five-validator set.
    assert!(
        b_seen.contains_key(&joiner),
        "the joiner is in the epoch-1 set"
    );
    assert_eq!(b_seen.len(), 5, "all five finalized the joiner's op");
}

#[test]
fn a_joined_validator_is_incorporated_after_epoch_respawn() {
    // timed runner is the liveness backstop: if the respawned five-set never
    // finalizes the joiner's op, the drain loop never completes and the deadline
    // panics (rather than hanging) — the simplex analog of a non-incorporation.
    deterministic::Runner::timed(Duration::from_secs(300))
        .start(|context| scenario(context, ed25519_scheme_for));
}
