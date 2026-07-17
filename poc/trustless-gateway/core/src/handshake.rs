//! Session-key handshake between the Computation Provider (client) and the
//! enclave. Both sides derive the same key from an X25519 ECDH where the
//! enclave end is its static `seal_pk` — the key the client read out of the
//! *attested* REPORTDATA. So possession of the session key proves the client
//! talked to the attested enclave, and a remote node operator relaying the
//! traffic cannot substitute its own key.
//!
//! The key then wraps the issued session token (AEAD), so only the client that
//! completed the handshake can open it. Body-level AEAD of proxied traffic is a
//! later transport slice; here the key protects the token handoff and binds the
//! session to the attestation.

use anyhow::{bail, Result};
use chacha20poly1305::aead::Aead;
use chacha20poly1305::{ChaCha20Poly1305, Key, KeyInit, Nonce};
use hkdf::Hkdf;
use rand_core::{OsRng, RngCore};
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::seal::SealKeypair;

const SESSION_LABEL: &[u8] = b"tcg-session-v1";

fn derive_session_key(shared: &[u8; 32]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(None, shared);
    let mut okm = [0u8; 32];
    hk.expand(SESSION_LABEL, &mut okm)
        .expect("32 is a valid HKDF-SHA256 output length");
    okm
}

/// Client side: given the enclave's attested `seal_pk`, produce this session's
/// ephemeral public key (sent to the enclave) and the shared session key.
pub fn client_handshake(seal_pk: &[u8; 32]) -> ([u8; 32], [u8; 32]) {
    let eph = StaticSecret::random_from_rng(OsRng);
    let eph_pk = PublicKey::from(&eph).to_bytes();
    let shared = eph.diffie_hellman(&PublicKey::from(*seal_pk));
    (eph_pk, derive_session_key(shared.as_bytes()))
}

/// Enclave side: derive the same session key from the client's ephemeral public
/// key using the enclave's static seal secret.
pub fn enclave_session_key(seal_kp: &SealKeypair, client_eph_pk: &[u8; 32]) -> [u8; 32] {
    derive_session_key(&seal_kp.ecdh(client_eph_pk))
}

/// AEAD-wrap a token under the session key. Layout: `nonce(12) || ct(+16 tag)`.
pub fn seal_token(session_key: &[u8; 32], token: &[u8]) -> Vec<u8> {
    let cipher = ChaCha20Poly1305::new(Key::from_slice(session_key));
    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let ct = cipher
        .encrypt(Nonce::from_slice(&nonce), token)
        .expect("ChaCha20-Poly1305 encryption does not fail on valid inputs");
    let mut out = Vec::with_capacity(12 + ct.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    out
}

pub fn open_token(session_key: &[u8; 32], blob: &[u8]) -> Result<Vec<u8>> {
    if blob.len() < 12 + 16 {
        bail!("sealed token too short: {} bytes", blob.len());
    }
    let nonce: [u8; 12] = blob[..12].try_into().unwrap();
    let cipher = ChaCha20Poly1305::new(Key::from_slice(session_key));
    cipher
        .decrypt(Nonce::from_slice(&nonce), &blob[12..])
        .map_err(|_| anyhow::anyhow!("open_token: AEAD failed (wrong session key or tampered)"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_sides_agree_and_token_round_trips() {
        let enclave = SealKeypair::generate();
        let seal_pk = enclave.public_bytes();

        let (client_eph_pk, client_key) = client_handshake(&seal_pk);
        let enclave_key = enclave_session_key(&enclave, &client_eph_pk);
        assert_eq!(client_key, enclave_key, "ECDH must agree");

        let blob = seal_token(&enclave_key, b"scoped.session.token");
        assert_eq!(open_token(&client_key, &blob).unwrap(), b"scoped.session.token");
    }

    #[test]
    fn a_different_client_key_cannot_open() {
        let enclave = SealKeypair::generate();
        let (client_eph_pk, _client_key) = client_handshake(&enclave.public_bytes());
        let enclave_key = enclave_session_key(&enclave, &client_eph_pk);
        let blob = seal_token(&enclave_key, b"secret-token");

        // A client that did its own handshake gets a different key.
        let (_other_eph, other_key) = client_handshake(&enclave.public_bytes());
        assert!(open_token(&other_key, &blob).is_err());
    }

    #[test]
    fn tampered_sealed_token_rejected() {
        let enclave = SealKeypair::generate();
        let (client_eph_pk, client_key) = client_handshake(&enclave.public_bytes());
        let enclave_key = enclave_session_key(&enclave, &client_eph_pk);
        let mut blob = seal_token(&enclave_key, b"secret-token");
        let last = blob.len() - 1;
        blob[last] ^= 0x01;
        assert!(open_token(&client_key, &blob).is_err());
    }
}
