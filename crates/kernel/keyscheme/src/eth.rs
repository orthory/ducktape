//! the `Secp256k1` arm: an Ethereum wallet's `personal_sign` as a proof.
//!
//! a wallet never signs our bytes directly — `personal_sign` wraps them in
//! the EIP-191 envelope `"\x19Ethereum Signed Message:\n" ‖ len ‖ msg` and
//! keccak-256 hashes that. the signature is `r‖s‖v` and carries no public
//! key, so verification RECOVERS the key from the signature and compares it
//! to the registered one. `msg` is [`personal_message`] — commonware's
//! `union_unique(ns, preimage)`, the same domain separation the ed25519 arm
//! gets from its namespaced verify — so a wallet proof minted for one
//! namespace can never pass under another.
//!
//! deterministic: pure-Rust k256 on every arch. low-S is NOT required (a
//! malleated signature authorizes the same bytes, which is harmless here);
//! a high-S signature is normalized before recovery and its parity bit
//! flipped accordingly.

use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
use sha3::{Digest, Keccak256};

const EIP191_PREFIX: &[u8] = b"\x19Ethereum Signed Message:\n";
/// r(32) ‖ s(32) ‖ v(1)
const PROOF_LEN: usize = 65;

/// the exact bytes a wallet is asked to `personal_sign` for `(ns, preimage)`
/// — commonware's namespaced preimage, so the enrollment side and this
/// verifier share one source of truth.
pub fn personal_message(ns: &[u8], preimage: &[u8]) -> Vec<u8> {
    commonware_utils::union_unique(ns, preimage)
}

/// `keccak256("\x19Ethereum Signed Message:\n" ‖ decimal(len(message)) ‖ message)`.
pub fn eip191_digest(message: &[u8]) -> [u8; 32] {
    let mut h = Keccak256::new();
    h.update(EIP191_PREFIX);
    h.update(message.len().to_string().as_bytes());
    h.update(message);
    h.finalize().into()
}

/// `v` as wallets emit it (27/28) or as raw parity (0/1); anything else is
/// not a recovery id.
fn recovery_id(v: u8) -> Option<RecoveryId> {
    let parity = match v {
        0 | 1 => v,
        27 | 28 => v - 27,
        _ => return None,
    };
    RecoveryId::from_byte(parity)
}

pub(crate) fn verify_personal_sign(
    pubkey: &[u8],
    ns: &[u8],
    preimage: &[u8],
    proof: &[u8],
) -> bool {
    if proof.len() != PROOF_LEN {
        return false;
    }
    let Ok(expected) = VerifyingKey::from_sec1_bytes(pubkey) else {
        return false;
    };
    let Ok(sig) = Signature::from_slice(&proof[..64]) else {
        return false;
    };
    let Some(recid) = recovery_id(proof[64]) else {
        return false;
    };
    // a high-S signature recovers to the wrong point unless S is normalized
    // and the parity bit flipped with it.
    let (sig, recid) = match sig.normalize_s() {
        Some(low) => (
            low,
            RecoveryId::new(!recid.is_y_odd(), recid.is_x_reduced()),
        ),
        None => (sig, recid),
    };
    let digest = eip191_digest(&personal_message(ns, preimage));
    match VerifyingKey::recover_from_prehash(&digest, &sig, recid) {
        Ok(recovered) => recovered == expected,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use crate::testkit::{eth_key, eth_proof, eth_pubkey};
    use crate::{KeyScheme, eip191_digest, personal_message};

    const NS: &[u8] = b"ducktape-test-ns-v1";

    #[test]
    fn eip191_digest_matches_the_known_vector() {
        // keccak256("\x19Ethereum Signed Message:\n11hello world") — the
        // canonical `personal_sign("hello world")` digest every wallet produces.
        let d = eip191_digest(b"hello world");
        let hex: String = d.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(
            hex,
            "d9eba16ed0ecae432b71fe008c98cc872bb4cc214d3220a36f365326cf807d68"
        );
    }

    #[test]
    fn personal_sign_proof_verifies_and_binds_namespace_and_preimage() {
        let sk = eth_key(3);
        let pk = eth_pubkey(&sk);
        let preimage = b"chain|scheme|newkey|gen";
        let proof = eth_proof(&sk, NS, preimage);
        assert_eq!(proof.len(), 65);
        assert!(KeyScheme::Secp256k1.verify(&pk, NS, preimage, &proof));
        assert!(!KeyScheme::Secp256k1.verify(&pk, b"other-ns", preimage, &proof));
        assert!(!KeyScheme::Secp256k1.verify(&pk, NS, b"different", &proof));
        // another wallet's key does not verify this proof.
        assert!(!KeyScheme::Secp256k1.verify(&eth_pubkey(&eth_key(4)), NS, preimage, &proof));
    }

    #[test]
    fn both_v_conventions_are_accepted() {
        let sk = eth_key(5);
        let pk = eth_pubkey(&sk);
        let preimage = b"v-test";
        let mut proof = eth_proof(&sk, NS, preimage);
        assert!(proof[64] == 27 || proof[64] == 28);
        assert!(KeyScheme::Secp256k1.verify(&pk, NS, preimage, &proof));
        proof[64] -= 27; // the 0/1 convention some signers emit
        assert!(KeyScheme::Secp256k1.verify(&pk, NS, preimage, &proof));
        proof[64] = 9; // neither convention
        assert!(!KeyScheme::Secp256k1.verify(&pk, NS, preimage, &proof));
    }

    #[test]
    fn uncompressed_pubkey_is_accepted_too() {
        let sk = eth_key(6);
        let uncompressed = sk
            .verifying_key()
            .to_encoded_point(false)
            .as_bytes()
            .to_vec();
        assert_eq!(uncompressed.len(), 65);
        let preimage = b"sec1-test";
        let proof = eth_proof(&sk, NS, preimage);
        assert!(KeyScheme::Secp256k1.verify(&uncompressed, NS, preimage, &proof));
    }

    #[test]
    fn wrong_length_and_tampered_proofs_fail() {
        let sk = eth_key(7);
        let pk = eth_pubkey(&sk);
        let preimage = b"tamper";
        let proof = eth_proof(&sk, NS, preimage);
        assert!(!KeyScheme::Secp256k1.verify(&pk, NS, preimage, &proof[..64]));
        let mut tampered = proof.clone();
        tampered[10] ^= 0xff;
        assert!(!KeyScheme::Secp256k1.verify(&pk, NS, preimage, &tampered));
        // the message a wallet is shown is the commonware-namespaced preimage.
        assert_eq!(
            personal_message(NS, preimage),
            commonware_utils::union_unique(NS, preimage)
        );
    }
}
