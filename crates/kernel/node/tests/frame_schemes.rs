//! a frame's origin may be ANY scheme in `keyscheme`: the frame declares its
//! scheme in the first byte, the proof bytes follow the preimage, and
//! `decode_frame` verifies under that scheme. `Origin::External` stays the
//! raw pubkey bytes — no consumer learns or needs the scheme.

use keyscheme::KeyScheme;
use keyscheme::testkit::{eth_key, eth_proof, eth_pubkey, passkey, passkey_proof, passkey_pubkey};
use sdk::{Msg, Origin};

fn msg() -> Msg {
    Msg {
        target: "kv".into(),
        payload: b"{\"set\":{\"k\":\"v\"}}".to_vec(),
    }
}

#[test]
fn an_ed25519_frame_declares_tag_zero() {
    use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
    let signer = PrivateKey::from_seed(1);
    let frame = node::encode_frame(&signer, 5, &msg());
    assert_eq!(frame[0], KeyScheme::Ed25519.tag());
    let (origin, m) = node::decode_frame(&frame).expect("decodes");
    assert_eq!(
        origin,
        Origin::External(signer.public_key().as_ref().to_vec())
    );
    assert_eq!(m, msg());
    assert_eq!(
        node::frame_origin_seq(&frame),
        Some((signer.public_key().as_ref().to_vec(), 5))
    );
}

#[test]
fn a_wallet_signed_frame_decodes_to_the_wallet() {
    let sk = eth_key(9);
    let pk = eth_pubkey(&sk);
    let mut frame = node::frame_preimage(KeyScheme::Secp256k1, &pk, 7, &msg());
    let proof = eth_proof(&sk, node::FRAME_NS, &frame);
    frame.extend_from_slice(&proof);
    let (origin, m) = node::decode_frame(&frame).expect("a wallet frame decodes");
    assert_eq!(origin, Origin::External(pk.clone()));
    assert_eq!(m, msg());
    assert_eq!(node::frame_origin_seq(&frame), Some((pk, 7)));
}

#[test]
fn a_passkey_signed_frame_decodes_to_the_passkey() {
    let sk = passkey(0x31);
    let pk = passkey_pubkey(&sk);
    let mut frame = node::frame_preimage(KeyScheme::Secp256r1, &pk, 1, &msg());
    let proof = passkey_proof(
        &sk,
        "auth.ducktape.byeongsu.dev",
        node::FRAME_NS,
        &frame,
        true,
    );
    frame.extend_from_slice(&proof);
    let (origin, _) = node::decode_frame(&frame).expect("a passkey frame decodes");
    assert_eq!(origin, Origin::External(pk));
}

#[test]
fn an_unknown_scheme_tag_is_rejected() {
    let sk = eth_key(9);
    let pk = eth_pubkey(&sk);
    let mut frame = node::frame_preimage(KeyScheme::Secp256k1, &pk, 7, &msg());
    let proof = eth_proof(&sk, node::FRAME_NS, &frame);
    frame.extend_from_slice(&proof);
    frame[0] = 9;
    assert!(node::decode_frame(&frame).is_err());
    assert_eq!(node::frame_origin_seq(&frame), None);
}

#[test]
fn a_key_under_the_wrong_scheme_is_rejected() {
    // an ed25519 key claiming to be a passkey: well-formedness (32 bytes is
    // not a SEC1 point) and the verify both refuse.
    use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
    let signer = PrivateKey::from_seed(2);
    let pk = signer.public_key().as_ref().to_vec();
    let mut frame = node::frame_preimage(KeyScheme::Secp256r1, &pk, 0, &msg());
    let proof = signer.sign(node::FRAME_NS, &frame).as_ref().to_vec();
    frame.extend_from_slice(&proof);
    assert!(node::decode_frame(&frame).is_err());
    // and a wallet key with an ed25519-length proof under tag 0.
    let sk = eth_key(3);
    let mut frame = node::frame_preimage(KeyScheme::Ed25519, &eth_pubkey(&sk), 0, &msg());
    frame.extend_from_slice(&[0u8; 64]);
    assert!(node::decode_frame(&frame).is_err());
}

#[test]
fn a_tampered_wallet_frame_is_rejected() {
    let sk = eth_key(4);
    let pk = eth_pubkey(&sk);
    let mut frame = node::frame_preimage(KeyScheme::Secp256k1, &pk, 2, &msg());
    let proof = eth_proof(&sk, node::FRAME_NS, &frame);
    frame.extend_from_slice(&proof);
    let last_payload_byte = frame.len() - 65 - 1;
    frame[last_payload_byte] ^= 0x01;
    assert!(node::decode_frame(&frame).is_err());
}
