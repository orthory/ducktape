//! op-frame v3 — the continuation-capable envelope codec at the ordered
//! lane. properties:
//!
//! 1. round-trip: with and without a continuation, decode returns exactly
//!    what encode framed, under the v3 signing domain;
//! 2. the signature covers the CONTINUATION: mutating either continuation
//!    field (or the flag) after signing fails verification — no grafting a
//!    continuation onto (or stripping one off) someone else's op;
//! 3. `cont_flag` outside {0,1} is not a canonical frame;
//! 4. cross-codec: the v2 decoder deterministically rejects a v3 frame and
//!    the v3 decoder rejects a v2 frame — the fence between the codecs is
//!    structural, not a config check;
//! 5. the continuation payload cap rejects at decode;
//! 6. `decode_member` / `decode_frame_any` read both codecs, a v3 envelope
//!    carrying its continuation (and the member frame id) onto the op;
//! 7. end-to-end: a v3 envelope admits at the ordered lane, applies its
//!    parent, and releases the continuation in the same block — surfaced on
//!    [`node::DrainedOp::continuation`];
//! 8. determinism: two nodes fed the identical v3-carrying batch drain
//!    identical app-hashes, dispositions, and continuation traces.

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
fn v3_roundtrips_with_and_without_continuation() {
    let signer = sk(1);
    for c in [None, Some(cont())] {
        let frame = node::encode_frame_v3(&signer, 3, &msg(), c.as_ref());
        let (origin, m, dc) = node::decode_frame_v3(&frame).expect("v3 frame decodes");
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
fn v3_signature_covers_continuation() {
    let frame = node::encode_frame_v3(&sk(2), 0, &msg(), Some(&cont()));
    // the continuation section sits between the root payload and the 64-byte
    // signature; flip a byte there (the payload tail is well inside it).
    let idx = frame.len() - 64 - 2;
    for tamper in [idx, frame.len() - 64 - cont().payload.len() - 9] {
        let mut forged = frame.clone();
        forged[tamper] ^= 0x01;
        assert!(
            node::decode_frame_v3(&forged).is_err(),
            "a tampered continuation byte (index {tamper}) must fail verification"
        );
    }
}

// (3) a flag outside {0,1} is rejected — exactly one valid encoding.
#[test]
fn v3_rejects_bad_cont_flag() {
    let signer = sk(3);
    let frame = node::encode_frame_v3(&signer, 0, &msg(), None);
    // the flag is the byte right before the 64-byte signature on the None arm.
    let mut forged = frame.clone();
    let flag_idx = forged.len() - 64 - 1;
    assert_eq!(forged[flag_idx], 0, "premise: locating the flag byte");
    forged[flag_idx] = 2;
    assert!(
        node::decode_frame_v3(&forged).is_err(),
        "cont_flag 2 is not canonical (and breaks the signature anyway)"
    );
}

// (4) cross-codec rejection, both directions — the structural fence.
#[test]
fn v2_and_v3_decoders_reject_each_other() {
    let signer = sk(4);
    let v3 = node::encode_frame_v3(&signer, 0, &msg(), Some(&cont()));
    assert!(
        node::decode_frame(&v3).is_err(),
        "the live v2 decoder deterministically rejects a v3 frame"
    );
    let v2 = node::encode_frame(&signer, 0, &msg());
    assert!(
        node::decode_frame_v3(&v2).is_err(),
        "the v3 decoder rejects a v2 frame (no flag byte, wrong domain)"
    );
}

// (5) an over-cap continuation payload rejects at decode even when honestly
// signed — the cap is consensus admission, not composer politeness.
#[test]
fn v3_rejects_over_cap_continuation_payload() {
    let big = Continuation {
        target: "runs".into(),
        payload: vec![0u8; sdk::MAX_CONTINUATION_BYTES + 1],
    };
    let frame = node::encode_frame_v3(&sk(5), 0, &msg(), Some(&big));
    let err = node::decode_frame_v3(&frame).expect_err("over-cap continuation must reject");
    assert!(
        err.to_string().contains("exceeds cap"),
        "rejection names the cap: {err}"
    );
}

// (6) the member decode: both codecs, a v3 envelope carrying its continuation.
#[test]
fn decode_member_reads_both_codecs() {
    let signer = sk(6);
    let v2 = node::encode_frame(&signer, 0, &msg());
    let op = node::decode_member(&v2).expect("v2 decodes");
    assert!(op.continuation.is_none());
    assert_eq!(op.frame, node::frame_id(&v2), "member frame id stamped");

    let v3 = node::encode_frame_v3(&signer, 0, &msg(), Some(&cont()));
    let op = node::decode_member(&v3).expect("v3 decodes");
    assert_eq!(op.continuation, Some(cont()), "the envelope continuation rides the BlockOp");
    assert_eq!(op.frame, node::frame_id(&v3), "member frame id stamped");
    assert_eq!(op.msg, msg());

    assert!(node::decode_member(b"junk").is_err(), "junk is junk");
}

// (6b) the policy-door decode: both codecs, no gate.
#[test]
fn decode_frame_any_reads_both_codecs() {
    let signer = sk(7);
    let v2 = node::encode_frame(&signer, 0, &msg());
    let (_, m, c) = node::decode_frame_any(&v2).expect("v2 arm");
    assert_eq!((m, c), (msg(), None));

    let v3 = node::encode_frame_v3(&signer, 0, &msg(), Some(&cont()));
    let (_, m, c) = node::decode_frame_any(&v3).expect("v3 arm");
    assert_eq!((m, c), (msg(), Some(cont())));

    assert!(node::decode_frame_any(b"junk").is_err());
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

/// a v3 envelope: parent sets `a=1`, continuation sets `b=2`.
fn v3_envelope(signer: &PrivateKey, seq: u64) -> Vec<u8> {
    node::encode_frame_v3(
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

// (7) end to end: a v3 envelope admits at the ordered lane, applies its
// parent, and releases the continuation in the same block.
#[test]
fn drain_applies_v3_and_releases_the_continuation() {
    block_on(async {
        let mut node = OrderedNode::new(dir_host(), RoundOrderer::new());
        let signer = sk(10);

        node.submit_frame(v3_envelope(&signer, 0))
            .await
            .expect("v3 admits");
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

// (8) determinism: two nodes fed the IDENTICAL v3-carrying batch drain the
// identical app-hash, dispositions, and continuation traces.
#[test]
fn identical_v3_batches_drain_identically_on_two_nodes() {
    block_on(async {
        let mut n1 = OrderedNode::new(dir_host(), RoundOrderer::new());
        let mut n2 = OrderedNode::new(dir_host(), RoundOrderer::new());

        let batch = node::encode_batch(&[
            node::encode_frame(&sk(11), 0, &dir_set("x", "9")),
            v3_envelope(&sk(12), 0),
        ]);
        for n in [&mut n1, &mut n2] {
            n.orderer_mut().submit(batch.clone()).await.expect("propose");
            while n.drain_delivered().await.expect("drain") != 0 {}
        }

        assert_eq!(n1.app_hash(), n2.app_hash(), "identical app-hashes");
        let (d1, d2) = (n1.take_drained(), n2.take_drained());
        assert_eq!(d1.len(), d2.len());
        for (a, b) in d1.iter().zip(&d2) {
            assert_eq!(a.id, b.id);
            assert_eq!(a.disposition, b.disposition);
            assert_eq!(a.app_hash, b.app_hash);
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
            "the v3 member's continuation surfaced on both nodes"
        );
    });
}
