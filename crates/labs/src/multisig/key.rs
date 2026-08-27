//! The multisig vault's chain-facing signing key, derived from the user's
//! EXISTING mnemonic seed.
//!
//! ## why derived and not stored
//!
//! `userkey.rs` establishes that the 24-word mnemonic *is* the identity: its 32
//! bytes of BIP39 entropy are the ed25519 seed verbatim (`from_entropy` /
//! `to_entropy`, never BIP39's `to_seed` PBKDF2 stretch). Deriving the vault's
//! secp256k1 key from those same bytes means a vault owner has **no new custody
//! surface, no second backup, and no separate recovery story** — restoring the
//! mnemonic restores the ability to approve.
//!
//! ## why this key is NOT a Ducktape member key
//!
//! It never enters consensus. `keyscheme::KeyScheme::Secp256k1` does exist for
//! account member keys (an EIP-191 `personal_sign` over a namespaced
//! preimage), but this key is never registered as one. Its only verifier is
//! Ethereum's `ecrecover`, reached through the Safe contract; consensus merely
//! orders the signatures it produces.
//!
//! Signing is intended to happen NODE-SIDE, so the key never reaches a webview —
//! least of all the browser webviews that render untrusted web content. The
//! derivation is a pure function of the seed, so it lives here beside the module
//! it signs for rather than in the node binary, which does not register this
//! module at all (see the crate docs).

// The wallet bridge (campaign PR-4) is this module's caller; until it lands, the
// derivation is exercised only by its own tests.
#![allow(dead_code)]

use alloy_primitives::keccak256;
use k256::ecdsa::SigningKey;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

/// Domain separation. A different label yields a different key, so this seed's
/// ed25519 identity and its secp256k1 owner key can never be confused for one
/// another, and a future second chain-key purpose gets its own label.
const OWNER_KEY_LABEL: &[u8] = b"ducktape-multisig-owner-key-v1";

/// Derive the vault-owner signing key from the 32-byte user seed.
///
/// `SHA-256(label ‖ seed ‖ counter)` reduced onto the curve. The counter exists
/// because a 256-bit hash can (with negligible probability, ~2^-128) land at or
/// above the secp256k1 group order, which is not a valid scalar — rejecting and
/// re-hashing is the standard construction, and silently clamping would bias
/// the key.
pub fn owner_signing_key(seed: &[u8; 32]) -> SigningKey {
    for counter in 0u8..=255 {
        let mut h = Sha256::new();
        h.update(OWNER_KEY_LABEL);
        h.update(seed);
        h.update([counter]);
        let candidate = Zeroizing::new(h.finalize());
        if let Ok(key) = SigningKey::from_slice(candidate.as_slice()) {
            return key;
        }
    }
    // Unreachable in any universe we ship to: 256 consecutive rejections each
    // of probability ~2^-128.
    unreachable!("no valid secp256k1 scalar in 256 candidates")
}

/// The Ethereum address of the derived owner key: the low 20 bytes of
/// `keccak256(uncompressed public key without its 0x04 tag)`.
pub fn owner_address(seed: &[u8; 32]) -> [u8; 20] {
    let key = owner_signing_key(seed);
    let point = key.verifying_key().to_encoded_point(false);
    let mut out = [0u8; 20];
    out.copy_from_slice(&keccak256(&point.as_bytes()[1..])[12..]);
    out
}

/// Sign a 32-byte digest (a SafeTx hash, or a module preimage hash) as the
/// vault owner, producing the 65-byte `r ‖ s ‖ v` Ethereum expects.
///
/// RustCrypto emits low-S signatures, which is what the module's `recover_owner`
/// demands: a high-S twin recovers to the same owner and would let one owner's
/// single approval be counted twice toward a threshold.
pub fn sign_digest(seed: &[u8; 32], digest: &[u8; 32]) -> [u8; 65] {
    let key = owner_signing_key(seed);
    let (sig, recid) = key
        .sign_prehash_recoverable(digest)
        .expect("a 32-byte digest is always signable");
    let mut out = [0u8; 65];
    out[..64].copy_from_slice(&sig.to_bytes());
    out[64] = recid.to_byte();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derivation_is_deterministic_and_seed_bound() {
        let a = [7u8; 32];
        let mut b = [7u8; 32];
        b[31] ^= 1;

        assert_eq!(
            owner_address(&a),
            owner_address(&a),
            "same seed, same address"
        );
        assert_ne!(
            owner_address(&a),
            owner_address(&b),
            "a one-bit seed change must move the address"
        );
    }

    /// The derived key must not be the ed25519 seed reinterpreted as a scalar —
    /// that would make the chain key recoverable from the identity key's raw
    /// bytes and collapse the domain separation.
    #[test]
    fn derived_key_is_not_the_raw_seed() {
        let seed = [0x11u8; 32];
        let derived = owner_signing_key(&seed);
        assert_ne!(derived.to_bytes().as_slice(), &seed[..]);
    }

    /// Signatures round-trip through exactly the verifier consensus uses.
    #[test]
    fn signature_recovers_to_the_derived_address() {
        use alloy_primitives::{Address, B256};

        let seed = [0x42u8; 32];
        let digest = [0x99u8; 32];
        let sig = sign_digest(&seed, &digest);

        let recovered = crate::multisig::safe::recover_owner(B256::from(digest), &sig)
            .expect("the module's verifier accepts our signature");
        assert_eq!(recovered, Address::from(owner_address(&seed)));
    }
}
