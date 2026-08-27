//! test-only SIGNING helpers — one place every suite (keyscheme, node,
//! identity, the wasm parity proofs) mints proofs from, so "what a signer
//! produces" is written once and matches the verifier by construction.

use commonware_cryptography::{Signer as _, ed25519};

/// an ed25519 proof: commonware's namespaced signature, 64 bytes.
pub fn ed25519_proof(signer: &ed25519::PrivateKey, ns: &[u8], preimage: &[u8]) -> Vec<u8> {
    signer.sign(ns, preimage).as_ref().to_vec()
}

/// a deterministic secp256k1 signing key from a non-zero seed byte.
pub fn eth_key(seed: u8) -> k256::ecdsa::SigningKey {
    assert_ne!(seed, 0, "seed 0 is not a valid scalar");
    k256::ecdsa::SigningKey::from_slice(&[seed; 32]).expect("valid scalar")
}

/// the 33-byte compressed SEC1 point — the form a wallet registers.
pub fn eth_pubkey(sk: &k256::ecdsa::SigningKey) -> Vec<u8> {
    sk.verifying_key().to_encoded_point(true).as_bytes().to_vec()
}

/// exactly what a wallet's `personal_sign` returns for
/// [`crate::personal_message`]`(ns, preimage)`: `r‖s‖v` with `v ∈ {27, 28}`.
pub fn eth_proof(sk: &k256::ecdsa::SigningKey, ns: &[u8], preimage: &[u8]) -> Vec<u8> {
    let digest = crate::eip191_digest(&crate::personal_message(ns, preimage));
    let (sig, recid) = sk
        .sign_prehash_recoverable(&digest)
        .expect("signing a 32-byte digest");
    let mut proof = sig.to_bytes().to_vec();
    proof.push(recid.to_byte() + 27);
    proof
}
