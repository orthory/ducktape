//! `submit_frame` — custody of an ALREADY-SIGNED frame: the relay entry
//! point. verification precedes pinning (junk never enters custody), and a
//! relayed frame's authorship is the SIGNER's key, not the submitting node's.

use directory::Directory;
use directory::{DirMsg, DirQuery, DirReply, decode_reply, encode_msg, encode_query};
use futures::executor::block_on;
use host::Host;
use node::{OrderedNode, RoundOrderer};
use sdk::Msg;

fn sk(seed: u64) -> commonware_cryptography::ed25519::PrivateKey {
    use commonware_cryptography::Signer as _;
    commonware_cryptography::ed25519::PrivateKey::from_seed(seed)
}

fn set(key: &str, value: &str) -> Msg {
    Msg {
        target: "directory".into(),
        payload: encode_msg(&DirMsg::Set {
            key: key.into(),
            value: value.into(),
        }),
    }
}

async fn get(node: &OrderedNode<RoundOrderer>, key: &str) -> Option<String> {
    let reply = node
        .host()
        .query(
            "directory",
            &encode_query(&DirQuery::Get { key: key.into() }),
        )
        .await
        .expect("query");
    match decode_reply(&reply).expect("decode") {
        DirReply::Value(v) => v,
    }
}

#[test]
fn submit_frame_takes_custody_and_keeps_signer_authorship() {
    block_on(async {
        use commonware_cryptography::Signer as _;
        let host = Host::genesis(vec![Box::new(Directory::new("directory"))]).expect("genesis");
        let mut node = OrderedNode::new(host, RoundOrderer::new());

        // a frame signed by a key that is NOT this node's submitter.
        let author = sk(7);
        let frame = node::encode_frame(&author, 0, &set("k", "v"));
        let expected_id = node::frame_id(&frame);

        let id = node.submit_frame(frame).await.expect("submit_frame");
        assert_eq!(id, expected_id, "the returned id is the frame's content address");

        assert_eq!(node.drain_delivered().await.expect("drain"), 1);
        assert_eq!(get(&node, "k").await.as_deref(), Some("v"));

        // authorship is the SIGNER's key — what modules read as Env.origin.
        let drained = node.take_drained();
        let d = drained.iter().find(|d| d.id == id).expect("drained frame");
        match &d.op {
            Some(op) => assert_eq!(
                op.origin,
                sdk::Origin::External(author.public_key().as_ref().to_vec()),
                "authorship rides the signature, not the custodian",
            ),
            None => panic!("applied frame carries its decoded op"),
        }
    });
}

#[test]
fn submit_frame_rejects_tampered_bytes_before_custody() {
    block_on(async {
        let host = Host::genesis(vec![Box::new(Directory::new("directory"))]).expect("genesis");
        let mut node = OrderedNode::new(host, RoundOrderer::new());

        let mut frame = node::encode_frame(&sk(7), 0, &set("k", "v"));
        // flip one payload byte: the signature no longer binds.
        let last = frame.len() - 1;
        frame[last] ^= 0x01;

        assert!(
            node.submit_frame(frame).await.is_err(),
            "a frame whose signature does not verify is refused at the door"
        );
        // nothing was proposed: the next drain delivers no frames.
        assert_eq!(node.drain_delivered().await.expect("drain"), 0);
        assert_eq!(get(&node, "k").await, None);
    });
}

#[test]
fn submit_still_equals_sign_plus_submit_frame() {
    block_on(async {
        let host = Host::genesis(vec![Box::new(Directory::new("directory"))]).expect("genesis");
        let mut node = OrderedNode::new(host, RoundOrderer::new());

        let signer = sk(1);
        let via_submit = node.submit(&signer, 0, set("a", "1")).await.expect("submit");
        let by_hand = node::frame_id(&node::encode_frame(&signer, 0, &set("a", "1")));
        assert_eq!(via_submit, by_hand, "submit is sign + submit_frame, byte-identical");
    });
}
