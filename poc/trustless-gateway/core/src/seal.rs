//! Anonymous public-key sealing: X25519 (ephemeral) -> HKDF-SHA256 ->
//! ChaCha20-Poly1305. Same construction as a NaCl sealed box, composed from
//! vetted RustCrypto primitives so it builds with no C dependency and both
//! ends are our own code. Recipient exposes only its public key; the sender is
//! anonymous.
//!
//! Blob layout: `eph_pk(32) || nonce(12) || ciphertext(+16 tag)`.

use anyhow::{bail, Result};
use chacha20poly1305::aead::Aead;
use chacha20poly1305::{ChaCha20Poly1305, Key, KeyInit, Nonce};
use hkdf::Hkdf;
use rand_core::{OsRng, RngCore};
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};

/// The recipient (enclave) keypair. The secret never leaves the enclave.
pub struct SealKeypair {
    secret: StaticSecret,
    public: PublicKey,
}

impl SealKeypair {
    pub fn generate() -> Self {
        let secret = StaticSecret::random_from_rng(OsRng);
        let public = PublicKey::from(&secret);
        Self { secret, public }
    }

    pub fn public_bytes(&self) -> [u8; 32] {
        self.public.to_bytes()
    }
}

fn derive_key(shared: &[u8; 32]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(None, shared);
    let mut okm = [0u8; 32];
    hk.expand(b"tcg-seal-v1", &mut okm)
        .expect("32 is a valid HKDF-SHA256 output length");
    okm
}

pub fn seal(recipient_pk: &[u8; 32], msg: &[u8]) -> Vec<u8> {
    let eph = StaticSecret::random_from_rng(OsRng);
    let eph_pk = PublicKey::from(&eph).to_bytes();
    let shared = eph.diffie_hellman(&PublicKey::from(*recipient_pk));
    let key = derive_key(shared.as_bytes());
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let ct = cipher
        .encrypt(Nonce::from_slice(&nonce), msg)
        .expect("ChaCha20-Poly1305 encryption does not fail on valid inputs");
    let mut out = Vec::with_capacity(32 + 12 + ct.len());
    out.extend_from_slice(&eph_pk);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    out
}

pub fn unseal(kp: &SealKeypair, blob: &[u8]) -> Result<Vec<u8>> {
    if blob.len() < 32 + 12 + 16 {
        bail!("sealed blob too short: {} bytes", blob.len());
    }
    let eph_pk: [u8; 32] = blob[..32].try_into().unwrap();
    let nonce: [u8; 12] = blob[32..44].try_into().unwrap();
    let ct = &blob[44..];
    let shared = kp.secret.diffie_hellman(&PublicKey::from(eph_pk));
    let key = derive_key(shared.as_bytes());
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
    cipher
        .decrypt(Nonce::from_slice(&nonce), ct)
        .map_err(|_| anyhow::anyhow!("unseal: AEAD decryption failed (wrong key or tampered blob)"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_round_trips() {
        let kp = SealKeypair::generate();
        let msg = b"refresh-token-abc123";
        let blob = seal(&kp.public_bytes(), msg);
        assert_eq!(unseal(&kp, &blob).unwrap(), msg);
    }

    #[test]
    fn wrong_recipient_cannot_open() {
        let kp = SealKeypair::generate();
        let other = SealKeypair::generate();
        let blob = seal(&kp.public_bytes(), b"secret");
        assert!(unseal(&other, &blob).is_err());
    }

    #[test]
    fn tampered_ciphertext_rejected() {
        let kp = SealKeypair::generate();
        let mut blob = seal(&kp.public_bytes(), b"secret");
        let last = blob.len() - 1;
        blob[last] ^= 0x01;
        assert!(unseal(&kp, &blob).is_err());
    }
}
