//! the op-frame codec at the ordered lane — the continuation-capable
//! envelope. properties:
//!
//! 1. round-trip: with and without a continuation, decode returns exactly
//!    what encode framed, under the frame signing domain;
//! 2. the signature covers the CONTINUATION: mutating either continuation
//!    field (or the flag) after signing fails verification — no grafting a
//!    continuation onto (or stripping one off) someone else's op;
//! 3. `cont_flag` outside {0,1} is not a canonical frame;
//! 4. the continuation payload cap rejects at decode;
//! 5. `decode_member` carries an envelope's continuation (and the member
//!    frame id) onto the op;
//! 6. end-to-end: an envelope admits at the ordered lane, applies its
//!    parent, and releases the continuation in the same block — surfaced on
//!    [`node::DrainedOp::continuation`];
//! 7. determinism: two nodes fed the identical envelope-carrying batch drain
//!    identical root-hashes, dispositions, and continuation traces.

use commonware_cryptography::Signer as _;
use commonware_cryptography::ed25519::PrivateKey;
use directory::{
    DirMsg, DirQuery, DirReply, Directory, decode_reply as dir_decode_reply,
    encode_msg as dir_encode, encode_query as dir_encode_query,
};
use futures::executor::block_on;
use host::Host;
use node::{Disposition, OrderedNode, Orderer as _, RoundOrderer};
use sdk::{Continuation, Msg, Origin};

fn sk(seed: u64) -> PrivateKey {
    PrivateKey::from_seed(seed)
}

fn msg() -> Msg {
    Msg {
        target: "chat".into(),
        payload: b"add-reaction".to_vec(),
    }
}

fn cont() -> Continuation {
    Continuation {
        target: "runs".into(),
        payload: b"resume-run-7".to_vec(),
    }
}

// (1) round-trip, both arms.
#[test]
fn frame_roundtrips_with_and_without_continuation() {
    let signer = sk(1);
    for c in [None, Some(cont())] {
        let frame = node::encode_frame(&signer, 3, &msg(), c.as_ref());
        let (origin, m, dc) = node::decode_frame(&frame).expect("frame decodes");
        assert_eq!(m, msg(), "root msg survives the round-trip");
        assert_eq!(dc, c, "continuation survives the round-trip");
        let Origin::External(key) = origin else {
            panic!("a wire frame only ever yields External authorship");
        };
        assert_eq!(key, signer.public_key().as_ref().to_vec());
    }
}

// (2) the signature binds the continuation: flip one byte anywhere in the
// continuation section and verification must fail.
#[test]
fn signature_covers_continuation() {
    let frame = node::encode_frame(&sk(2), 0, &msg(), Some(&cont()));
    // the continuation section sits between the root payload and the 64-byte
    // signature; flip a byte there (the payload tail is well inside it).
    let idx = frame.len() - 64 - 2;
    for tamper in [idx, frame.len() - 64 - cont().payload.len() - 9] {
        let mut forged = frame.clone();
        forged[tamper] ^= 0x01;
        assert!(
            node::decode_frame(&forged).is_err(),
            "a tampered continuation byte (index {tamper}) must fail verification"
        );
    }
}

// (3) a flag outside {0,1} is rejected — exactly one valid encoding.
#[test]
fn rejects_bad_cont_flag() {
    let signer = sk(3);
    let frame = node::encode_frame(&signer, 0, &msg(), None);
    // the flag is the byte right before the 64-byte signature on the None arm.
    let mut forged = frame.clone();
    let flag_idx = forged.len() - 64 - 1;
    assert_eq!(forged[flag_idx], 0, "premise: locating the flag byte");
    forged[flag_idx] = 2;
    assert!(
        node::decode_frame(&forged).is_err(),
        "cont_flag 2 is not canonical (and breaks the signature anyway)"
    );
}

// (4) an over-cap continuation payload rejects at decode even when honestly
// signed — the cap is consensus admission, not composer politeness.
#[test]
fn rejects_over_cap_continuation_payload() {
    let big = Continuation {
        target: "runs".into(),
        payload: vec![0u8; sdk::MAX_CONTINUATION_BYTES + 1],
    };
    let frame = node::encode_frame(&sk(5), 0, &msg(), Some(&big));
    let err = node::decode_frame(&frame).expect_err("over-cap continuation must reject");
    assert!(
        err.to_string().contains("exceeds cap"),
        "rejection names the cap: {err}"
    );
}

// (5) the member decode: the envelope carrying its continuation onto the op.
#[test]
fn decode_member_carries_the_continuation() {
    let signer = sk(6);
    let plain = node::encode_frame(&signer, 0, &msg(), None);
    let op = node::decode_member(&plain).expect("plain frame decodes");
    assert!(op.continuation.is_none());
    assert_eq!(op.frame, node::frame_id(&plain), "member frame id stamped");

    let envelope = node::encode_frame(&signer, 0, &msg(), Some(&cont()));
    let op = node::decode_member(&envelope).expect("envelope decodes");
    assert_eq!(
        op.continuation,
        Some(cont()),
        "the envelope continuation rides the BlockOp"
    );
    assert_eq!(op.frame, node::frame_id(&envelope), "member frame id stamped");
    assert_eq!(op.msg, msg());

    assert!(node::decode_member(b"junk").is_err(), "junk is junk");
}

// ---- the ordered lane, end to end -------------------------------------------

const DIR: &str = "directory";

fn dir_host() -> Host {
    Host::genesis(vec![Box::new(Directory::new(DIR))]).expect("genesis")
}

fn dir_set(key: &str, value: &str) -> Msg {
    Msg {
        target: DIR.into(),
        payload: dir_encode(&DirMsg::Set {
            key: key.into(),
            value: value.into(),
        }),
    }
}

async fn dir_get(host: &Host, key: &str) -> Option<String> {
    let reply = host
        .query(DIR, &dir_encode_query(&DirQuery::Get { key: key.into() }))
        .await
        .expect("dir query");
    match dir_decode_reply(&reply).expect("dir reply") {
        DirReply::Value(v) => v,
    }
}

/// an envelope: parent sets `a=1`, continuation sets `b=2`.
fn envelope(signer: &PrivateKey, seq: u64) -> Vec<u8> {
    node::encode_frame(
        signer,
        seq,
        &dir_set("a", "1"),
        Some(&Continuation {
            target: DIR.into(),
            payload: dir_encode(&DirMsg::Set {
                key: "b".into(),
                value: "2".into(),
            }),
        }),
    )
}

// (6) end to end: an envelope admits at the ordered lane, applies its
// parent, and releases the continuation in the same block.
#[test]
fn drain_applies_the_envelope_and_releases_the_continuation() {
    block_on(async {
        let mut node = OrderedNode::new(dir_host(), RoundOrderer::new());
        let signer = sk(10);

        node.submit_frame(envelope(&signer, 0))
            .await
            .expect("envelope admits");
        node.flush_batch().await.expect("flush");
        while node.drain_delivered().await.expect("drain") != 0 {}

        let drained = node.take_drained();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].disposition, Disposition::Applied);
        let op = drained[0].op.as_ref().expect("decoded op");
        assert_eq!(op.target, DIR);
        let released = op.continuation.as_ref().expect("continuation surfaced");
        assert_eq!(released.target, DIR);
        assert_eq!(released.disposition, Disposition::Applied);
        assert_eq!(released.reason, None);
        assert_eq!(released.dispatches.len(), 1, "the continuation's own trace");
        assert_eq!(
            released.dispatches[0].origin,
            Origin::Module(DIR.into()),
            "released on the parent target's module lane"
        );

        // both writes committed: parent then continuation, one block.
        assert_eq!(dir_get(node.host(), "a").await.as_deref(), Some("1"));
        assert_eq!(dir_get(node.host(), "b").await.as_deref(), Some("2"));
    });
}

// (7) determinism: two nodes fed the IDENTICAL envelope-carrying batch drain
// the identical root-hash, dispositions, and continuation traces.
#[test]
fn identical_envelope_batches_drain_identically_on_two_nodes() {
    block_on(async {
        let mut n1 = OrderedNode::new(dir_host(), RoundOrderer::new());
        let mut n2 = OrderedNode::new(dir_host(), RoundOrderer::new());

        let batch = node::encode_batch(&[
            node::encode_frame(&sk(11), 0, &dir_set("x", "9"), None),
            envelope(&sk(12), 0),
        ]);
        for n in [&mut n1, &mut n2] {
            n.orderer_mut().submit(batch.clone()).await.expect("propose");
            while n.drain_delivered().await.expect("drain") != 0 {}
        }

        assert_eq!(n1.root_hash(), n2.root_hash(), "identical root-hashes");
        let (d1, d2) = (n1.take_drained(), n2.take_drained());
        assert_eq!(d1.len(), d2.len());
        for (a, b) in d1.iter().zip(&d2) {
            assert_eq!(a.id, b.id);
            assert_eq!(a.disposition, b.disposition);
            assert_eq!(a.root_hash, b.root_hash);
            let (oa, ob) = (a.op.as_ref().expect("op"), b.op.as_ref().expect("op"));
            assert_eq!(oa.dispatches, ob.dispatches, "identical member traces");
            match (&oa.continuation, &ob.continuation) {
                (None, None) => {}
                (Some(ca), Some(cb)) => {
                    assert_eq!(ca.target, cb.target);
                    assert_eq!(ca.disposition, cb.disposition);
                    assert_eq!(ca.dispatches, cb.dispatches, "identical continuation traces");
                }
                other => panic!("continuation presence diverged: {other:?}"),
            }
        }
        assert!(
            d2.iter().any(|d| d
                .op
                .as_ref()
                .is_some_and(|op| op.continuation.is_some())),
            "the envelope member's continuation surfaced on both nodes"
        );
    });
}
