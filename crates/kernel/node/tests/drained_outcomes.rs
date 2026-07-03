//! [`OrderedNode::take_drained`] correlates a caller's own submits with their
//! finalized outcomes: the [`FrameId`] returned by submit reappears with a
//! disposition once the order delivers the frame. this is the seam an app
//! surface holds a reply open on ("your op landed / was rejected / resubmit"),
//! so the properties pinned here are id round-trip, per-frame disposition, and
//! take-clears semantics.

use commonware_cryptography::{Signer as _, ed25519};
use directory::Directory;
use directory_interface::{DirMsg, encode_msg};
use host::{BlockContext, Host};
use node::{Disposition, OrderedNode, RoundOrderer};
use sdk::{Msg, Origin};

fn dir_set(k: &str, v: &str) -> Msg {
    Msg {
        target: "directory".into(),
        payload: encode_msg(&DirMsg::Set {
            key: k.into(),
            value: v.into(),
        }),
    }
}

fn genesis() -> Host {
    Host::genesis(vec![Box::new(Directory::new("directory"))]).expect("genesis")
}

async fn root_after(signer: &ed25519::PrivateKey, msgs: &[Msg]) -> Vec<sdk::StateRoot> {
    let mut host = genesis();
    let origin = Origin::External(signer.public_key().as_ref().to_vec());
    let mut roots = Vec::new();
    for (height, msg) in msgs.iter().cloned().enumerate() {
        host.submit_at(
            BlockContext {
                height: height as u64,
                consensus_time: height as u64,
                origin: origin.clone(),
            },
            msg,
        )
        .await
        .expect("reference apply");
        roots.push(host.app_hash());
    }
    roots
}

#[test]
fn drained_outcomes_correlate_submits_with_dispositions() {
    futures::executor::block_on(async {
        let mut node = OrderedNode::new(genesis(), RoundOrderer::new());
        let signer = ed25519::PrivateKey::from_seed(1);

        let ok_id = node
            .submit(&signer, 0, dir_set("k", "v"))
            .await
            .expect("submit ok op");
        // a valid target whose payload the module cannot decode: finalizes,
        // then rejects deterministically — the no-op disposition.
        let bad_id = node
            .submit(
                &signer,
                1,
                Msg {
                    target: "directory".into(),
                    payload: b"not-json".to_vec(),
                },
            )
            .await
            .expect("submit rejectable op");
        assert_ne!(ok_id, bad_id, "distinct frames get distinct ids");

        node.drain_delivered().await.expect("drain");
        let drained = node.take_drained();
        assert_eq!(drained.len(), 2, "every finalized frame gets an outcome");

        let ok_frame = drained
            .iter()
            .find(|d| d.id == ok_id)
            .expect("ok id drained");
        let bad_frame = drained
            .iter()
            .find(|d| d.id == bad_id)
            .expect("bad id drained");
        assert_eq!(ok_frame.disposition, Disposition::Applied);
        assert_eq!(bad_frame.disposition, Disposition::Rejected);

        assert!(node.take_drained().is_empty(), "take clears the queue");
    });
}

#[test]
fn frames_past_the_ceiling_drain_as_discarded() {
    futures::executor::block_on(async {
        let mut node = OrderedNode::new(genesis(), RoundOrderer::new());
        let signer = ed25519::PrivateKey::from_seed(2);

        let id = node
            .submit(&signer, 0, dir_set("k", "v"))
            .await
            .expect("submit");
        node.set_view_ceiling(0); // every view >= 0 is past the cutover.

        node.drain_delivered().await.expect("drain");
        let drained = node.take_drained();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, id);
        assert_eq!(drained[0].disposition, Disposition::Discarded);
        assert!(
            node.finalized().is_none(),
            "discarded-only drain does not advance finalized"
        );
        assert_eq!(
            node.last_engine_view(),
            Some(0),
            "discarded view still advances the engine clock"
        );
    });
}

#[test]
fn discarded_tail_does_not_become_finalized_boundary() {
    futures::executor::block_on(async {
        let mut node = OrderedNode::new(genesis(), RoundOrderer::new());
        let signer = ed25519::PrivateKey::from_seed(3);
        let applied_msg = dir_set("k", "v");
        let expected = root_after(&signer, std::slice::from_ref(&applied_msg)).await;

        let applied_id = node
            .submit(&signer, 0, applied_msg)
            .await
            .expect("submit applied");
        let discarded_id = node
            .submit(&signer, 1, dir_set("past", "ceiling"))
            .await
            .expect("submit discarded");
        node.set_view_ceiling(1);

        node.drain_delivered().await.expect("drain");
        let drained = node.take_drained();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].id, applied_id);
        assert_eq!(drained[0].disposition, Disposition::Applied);
        assert_eq!(drained[0].app_hash, expected[0]);
        assert_eq!(drained[1].id, discarded_id);
        assert_eq!(drained[1].disposition, Disposition::Discarded);
        assert_eq!(drained[1].app_hash, expected[0]);

        let finalized = node.finalized().expect("applied frame finalizes");
        assert_eq!(finalized.height, drained[0].height);
        assert_eq!(finalized.app_hash, expected[0]);
        assert_eq!(node.last_engine_view(), Some(drained[1].height));
    });
}

#[test]
fn multi_applied_batch_records_each_frame_app_hash() {
    futures::executor::block_on(async {
        let mut node = OrderedNode::new(genesis(), RoundOrderer::new());
        let signer = ed25519::PrivateKey::from_seed(4);
        let first = dir_set("a", "1");
        let second = dir_set("b", "2");
        let expected = root_after(&signer, &[first.clone(), second.clone()]).await;

        let first_id = node.submit(&signer, 0, first).await.expect("submit first");
        let second_id = node
            .submit(&signer, 1, second)
            .await
            .expect("submit second");

        node.drain_delivered().await.expect("drain");
        let drained = node.take_drained();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].id, first_id);
        assert_eq!(drained[0].disposition, Disposition::Applied);
        assert_eq!(drained[0].app_hash, expected[0]);
        assert_eq!(drained[1].id, second_id);
        assert_eq!(drained[1].disposition, Disposition::Applied);
        assert_eq!(drained[1].app_hash, expected[1]);
        assert_ne!(drained[0].app_hash, drained[1].app_hash);

        let finalized = node.finalized().expect("finalized");
        assert_eq!(finalized.height, drained[1].height);
        assert_eq!(finalized.app_hash, drained[1].app_hash);
    });
}
