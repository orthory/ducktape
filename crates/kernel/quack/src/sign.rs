//! Package signatures — `signatures/package.sig`.
//!
//! A package signature is an ed25519 signature over the *manifest hash*
//! (`manifest::manifest_hash`), under a dedicated domain namespace so it can
//! never cross-verify with any other signature the platform mints. The signer's
//! public key travels with the signature (a package is self-describing: the
//! recipient decides whether it trusts that key). Copies the
//! `commonware_cryptography` ed25519 sign/verify shape from `wireguard-upgrade`.

use commonware_codec::DecodeExt as _;
use commonware_cryptography::{Signer, Verifier, ed25519};
use serde::{Deserialize, Serialize};

/// The signing domain for a package manifest hash. A distinct namespace from
/// every other platform signature (endpoint records, invites, ...), so a
/// package signature is usable only as a package signature.
pub const SIG_NAMESPACE: &[u8] = b"ducktape:quack:sig:v1:";

/// A detached package signature. Serialized to `signatures/package.sig` as JSON
/// with hex-encoded fields: `{"signer":"<hex ed25519 pub>","sig":"<hex>"}`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageSig {
    #[serde(with = "crate::hexser")]
    pub signer: Vec<u8>,
    #[serde(with = "crate::hexser")]
    pub sig: Vec<u8>,
}

/// Sign a manifest hash, recording the signer's public key alongside the
/// signature.
pub fn sign_manifest(signer: &ed25519::PrivateKey, manifest_hash: &[u8; 32]) -> PackageSig {
    let signature = signer.sign(SIG_NAMESPACE, manifest_hash);
    PackageSig {
        signer: signer.public_key().as_ref().to_vec(),
        sig: signature.as_ref().to_vec(),
    }
}

/// Verify a package signature against a manifest hash. `false` on any
/// malformed key/signature or a mismatch — never panics on attacker bytes.
pub fn verify_manifest_sig(sig: &PackageSig, manifest_hash: &[u8; 32]) -> bool {
    let Ok(public) = ed25519::PublicKey::decode(&sig.signer[..]) else {
        return false;
    };
    if sig.sig.len() != 64 {
        return false;
    }
    let Ok(signature) = ed25519::Signature::decode(&sig.sig[..]) else {
        return false;
    };
    public.verify(SIG_NAMESPACE, manifest_hash, &signature)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(seed: u64) -> ed25519::PrivateKey {
        ed25519::PrivateKey::from_seed(seed)
    }

    #[test]
    fn sign_verify_round_trip() {
        let hash = [0x11u8; 32];
        let sig = sign_manifest(&key(1), &hash);
        assert!(verify_manifest_sig(&sig, &hash));
    }

    #[test]
    fn rejects_a_different_manifest_hash() {
        let sig = sign_manifest(&key(1), &[0x11u8; 32]);
        assert!(!verify_manifest_sig(&sig, &[0x22u8; 32]));
    }

    #[test]
    fn rejects_a_wrong_signer_key() {
        // sign with key 1, then claim key 2 signed it: the signature is
        // valid ed25519 but not over key 2's public key.
        let mut sig = sign_manifest(&key(1), &[0x11u8; 32]);
        sig.signer = key(2).public_key().as_ref().to_vec();
        assert!(!verify_manifest_sig(&sig, &[0x11u8; 32]));
    }

    #[test]
    fn rejects_a_malformed_signature() {
        let mut sig = sign_manifest(&key(1), &[0x11u8; 32]);
        sig.sig.truncate(10);
        assert!(!verify_manifest_sig(&sig, &[0x11u8; 32]));
    }

    #[test]
    fn json_is_hex_and_round_trips() {
        let sig = sign_manifest(&key(7), &[0xabu8; 32]);
        let json = serde_json::to_string(&sig).unwrap();
        // fields render as hex strings, not byte arrays.
        assert!(json.contains(&format!("\"{}\"", crate::to_hex(&sig.signer))));
        let back: PackageSig = serde_json::from_str(&json).unwrap();
        assert_eq!(back, sig);
        assert!(verify_manifest_sig(&back, &[0xabu8; 32]));
    }
}
