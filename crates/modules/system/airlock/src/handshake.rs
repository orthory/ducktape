//! Session-key handshake between the Computation Provider (client) and the
//! enclave. Both sides derive the same key from an X25519 ECDH where the
//! enclave end is its static `seal_pk`. Opening the sealed token proves the
//! responder holds the matching `seal_sk` — and once the CLIENT has verified
//! the quote (in `handshake_token`, not here), that `seal_pk` is the *attested*
//! one, so the session binds to the attested enclave and a relaying node
//! operator cannot substitute its key or read the token.
//!
//! The key wraps the issued session token (AEAD), so only the client that
//! completed the handshake can open it. Body-level AEAD of proxied traffic is a
//! later transport slice; here the key protects the token handoff.

use anyhow::Result;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::aead;
use crate::seal::SealKeypair;

const SESSION_LABEL: &[u8] = b"airlock-session-v1";

/// Client side: given the enclave's attested `seal_pk`, produce this session's
/// ephemeral public key (sent to the enclave) and the shared session key.
pub fn client_handshake(seal_pk: &[u8; 32]) -> ([u8; 32], [u8; 32]) {
    let eph = StaticSecret::random_from_rng(rand_core::OsRng);
    let eph_pk = PublicKey::from(&eph).to_bytes();
    let shared = eph.diffie_hellman(&PublicKey::from(*seal_pk));
    (eph_pk, aead::hkdf32(shared.as_bytes(), SESSION_LABEL))
}

/// Enclave side: derive the same session key from the client's ephemeral public
/// key using the enclave's static seal secret.
pub fn enclave_session_key(seal_kp: &SealKeypair, client_eph_pk: &[u8; 32]) -> [u8; 32] {
    aead::hkdf32(&seal_kp.ecdh(client_eph_pk), SESSION_LABEL)
}

/// AEAD-wrap a token under the session key.
pub fn seal_token(session_key: &[u8; 32], token: &[u8]) -> Vec<u8> {
    aead::seal(session_key, token)
}

pub fn open_token(session_key: &[u8; 32], blob: &[u8]) -> Result<Vec<u8>> {
    aead::open(session_key, blob)
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
