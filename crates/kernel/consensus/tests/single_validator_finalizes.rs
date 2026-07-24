//! a ONE-validator network is live: simplex with participants = {self}
//! finalizes real blocks.
//!
//! this is the property "local mode" stands on — a solo node is not a special
//! non-consensus host, it is a network of one (quorum 1, every view self-led).
//! two sequential ops prove ONGOING liveness (block after block), not just a
//! first-block fluke. `Runner::timed` is the verdict mechanism: if a single
//! participant cannot finalize, nothing drains and the deadline panics.

use std::time::Duration;

use commonware_consensus::simplex::{mocks, scheme::ed25519 as simplex_ed25519};
use commonware_consensus::types::Epoch;
use commonware_cryptography::{Sha256, Signer as _, ed25519};
use commonware_p2p::simulated;
use commonware_runtime::{Clock as _, Quota, Runner as _, Supervisor as _, deterministic};
use commonware_utils::{NZU32, NZUsize};

use consensus::{ContentStore, Digest, SimplexOrderer};
use directory::Directory;
use directory::{DirMsg, encode_msg};
use host::Host;
use node::OrderedNode;
use sdk::Msg;

fn dir_set(k: &str, v: &str) -> Msg {
    Msg {
        target: "directory".into(),
        payload: encode_msg(&DirMsg::Set {
            key: k.into(),
            value: v.into(),
        }),
    }
}

#[test]
fn a_single_validator_finalizes_sequential_blocks() {
    deterministic::Runner::timed(Duration::from_secs(300)).start(|mut context| async move {
        let namespace = b"consensus".to_vec();
        let epoch = Epoch::new(7);

        // ONE participant, production scheme construction via the fixture.
        let fixture = simplex_ed25519::fixture(&mut context, &namespace, 1);
        let me: ed25519::PublicKey = fixture.participants[0].clone();
        let scheme = fixture.schemes[0].clone();

        // a simulated network with a single peer: no links to add — every
        // message the engine sends is to itself or to nobody.
        let (network, oracle) = simulated::Network::new_with_peers(
            context.child("network"),
            simulated::Config {
                max_size: 1024 * 1024,
                disconnect_on_block: true,
                tracked_peer_sets: NZUsize!(1),
            },
            vec![me.clone()],
        )
        .await;
        network.start();

        let quota = Quota::per_second(NZU32!(128));
        let control = oracle.control(me.clone());
        let vote = control.register(0, quota).await.expect("register vote");
        let certificate = control
            .register(1, quota)
            .await
            .expect("register certificate");
        let resolver = control.register(2, quota).await.expect("register resolver");

        let genesis_floor: Digest = mocks::application::genesis::<Sha256>(epoch);
        let orderer = SimplexOrderer::spawn(
            context.child("validator"),
            scheme,
            oracle.control(me.clone()),
            me.to_string(),
            epoch,
            genesis_floor,
            ContentStore::new(),
            vote,
            certificate,
            resolver,
        );
        let host = Host::genesis(vec![Box::new(Directory::new("directory"))]).expect("genesis");
        let mut node = OrderedNode::new(host, orderer);

        let genesis = node.root_hash();

        // two ops from distinct origins, flushed as two SEPARATE batches so
        // the engine finalizes two SEQUENTIAL blocks (the property under
        // test). one flush would pack both members into a single block —
        // `drain_delivered` counts finalized FRAMES, and the sequential-
        // blocks claim would silently degrade to a one-block claim.
        let a = ed25519::PrivateKey::from_seed(201);
        let b = ed25519::PrivateKey::from_seed(202);
        node.submit(&a, 0, dir_set("solo", "first"))
            .await
            .expect("submit first");
        node.flush_batch().await.expect("flush first block");
        node.submit(&b, 0, dir_set("solo2", "second"))
            .await
            .expect("submit second");
        node.flush_batch().await.expect("flush second block");

        // pump simulated time until BOTH blocks applied in finalized order.
        let mut applied = 0usize;
        while applied < 2 {
            context.sleep(Duration::from_millis(50)).await;
            // the production run loop flushes pending ops event-driven (bin/node
            // `pump_eager_flush`); enqueue-only submits never propose without a flush —
            // the sim drives that flush on its own tick (a no-op when nothing is pending).
            node.flush_batch().await.expect("flush");
            applied += node.drain_delivered().await.expect("drain");
        }

        assert_ne!(
            node.root_hash(),
            genesis,
            "finalized solo blocks moved the root-hash"
        );
    });
}
