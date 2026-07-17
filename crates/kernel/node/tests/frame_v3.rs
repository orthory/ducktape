//! op-frame v3 — the continuation-capable envelope codec. properties:
//!
//! 1. round-trip: with and without a continuation, decode returns exactly
//!    what encode framed, under the v3 signing domain;
//! 2. the signature covers the CONTINUATION: mutating either continuation
//!    field (or the flag) after signing fails verification — no grafting a
//!    continuation onto (or stripping one off) someone else's op;
//! 3. `cont_flag` outside {0,1} is not a canonical frame;
//! 4. cross-codec: the v2 decoder deterministically rejects a v3 frame and
//!    the v3 decoder rejects a v2 frame — the pre-activation fence is
//!    structural, not a config check;
//! 5. the continuation payload cap rejects at decode.

use commonware_cryptography::Signer as _;
use commonware_cryptography::ed25519::PrivateKey;
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
