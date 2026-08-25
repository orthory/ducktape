//! A sealed envelope to an X25519 recipient — a crypto_box-style sealed box.
//!
//! the join protocol makes every invite BEARER: the token IS the admission
//! credential, so it must never cross a link in the clear (an on-path observer
//! on café wifi would otherwise redeem it first). The joiner's first-contact
//! intro is sealed to the receiving member's WireGuard X25519 public key —
//! that key rides in the invite blob under the issuer's envelope signature, so
//! the joiner knows it offline, and only the member holding the matching secret
//! can open the envelope. The seal is one-shot (a fresh ephemeral key per
//! message), so there is no extra round trip: the doorbell datagram is
//! confidential by construction.
//!
//! Layout: `ephemeral_pub(32) ‖ nonce(12) ‖ ciphertext+tag`. The recipient
//! reads the ephemeral public key, repeats the ECDH against its own secret,
//! derives the same key, and decrypts.

use chacha20poly1305::aead::Aead as _;
use chacha20poly1305::{ChaCha20Poly1305, KeyInit as _, Nonce};
use sha2::{Digest as _, Sha256};

/// KDF namespace so the ECDH output derived here can never collide with another
/// protocol's use of the same shared secret.
const SEAL_KDF_NAMESPACE: &[u8] = b"ducktape-invite-seal-v1";

const EPHEMERAL_PUB_LEN: usize = 32;
const NONCE_LEN: usize = 12;
/// smallest possible sealed envelope: the header plus a 16-byte AEAD tag over
/// empty plaintext.
const SEAL_MIN_LEN: usize = EPHEMERAL_PUB_LEN + NONCE_LEN + 16;

/// derive the AEAD key from the ECDH shared secret, binding BOTH public keys
/// into the transcript (standard sealed-box KDF — stops a shared secret being
/// reused in another context).
fn derive_key(shared: &[u8; 32], ephemeral_pub: &[u8; 32], recipient_pub: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(SEAL_KDF_NAMESPACE);
    hasher.update(shared);
    hasher.update(ephemeral_pub);
    hasher.update(recipient_pub);
    hasher.finalize().into()
}

/// Seal `plaintext` to `recipient_pub` (a raw X25519 public key). A fresh
/// ephemeral keypair is minted per call, so the same plaintext seals
/// differently every time and the nonce never repeats under one key.
pub fn seal(recipient_pub: &[u8; 32], plaintext: &[u8]) -> Vec<u8> {
    // mint the ephemeral secret from OS randomness exactly like `keys.rs` mints
    // the persistent WireGuard secret — every 32-byte string is a valid X25519
    // secret (the scheme clamps), so this cannot fail.
    let mut ephemeral_raw = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut ephemeral_raw);
    let ephemeral_secret = x25519_dalek::StaticSecret::from(ephemeral_raw);
    let ephemeral_pub = x25519_dalek::PublicKey::from(&ephemeral_secret);

    let shared = ephemeral_secret.diffie_hellman(&x25519_dalek::PublicKey::from(*recipient_pub));
    let key = derive_key(shared.as_bytes(), ephemeral_pub.as_bytes(), recipient_pub);

    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut nonce_bytes);
    let cipher = ChaCha20Poly1305::new((&key).into());
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), plaintext)
        .expect("chacha20poly1305 encryption of an in-memory buffer cannot fail");

    let mut out = Vec::with_capacity(EPHEMERAL_PUB_LEN + NONCE_LEN + ciphertext.len());
    out.extend_from_slice(ephemeral_pub.as_bytes());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    out
}

/// Open a sealed envelope with the recipient's static X25519 secret. Fails
/// closed on truncation, a wrong key, or any tampering (the AEAD tag).
pub fn open(
    recipient_secret: &x25519_dalek::StaticSecret,
    sealed: &[u8],
) -> Result<Vec<u8>, String> {
    let Some(rest) = sealed.get(..).filter(|s| s.len() >= SEAL_MIN_LEN) else {
        return Err("sealed envelope truncated".into());
    };
    let ephemeral_pub: [u8; 32] = rest[..EPHEMERAL_PUB_LEN].try_into().expect("32 bytes");
    let nonce = &rest[EPHEMERAL_PUB_LEN..EPHEMERAL_PUB_LEN + NONCE_LEN];
    let ciphertext = &rest[EPHEMERAL_PUB_LEN + NONCE_LEN..];

    let recipient_pub = x25519_dalek::PublicKey::from(recipient_secret);
    let shared = recipient_secret.diffie_hellman(&x25519_dalek::PublicKey::from(ephemeral_pub));
    let key = derive_key(shared.as_bytes(), &ephemeral_pub, recipient_pub.as_bytes());

    let cipher = ChaCha20Poly1305::new((&key).into());
    cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| "sealed envelope failed to open (wrong recipient key or tampered)".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keypair(seed: u8) -> (x25519_dalek::StaticSecret, [u8; 32]) {
        let secret = x25519_dalek::StaticSecret::from([seed; 32]);
        let public = x25519_dalek::PublicKey::from(&secret);
        (secret, *public.as_bytes())
    }

    #[test]
    fn seal_opens_for_the_recipient_and_roundtrips() {
        let (secret, public) = keypair(1);
        let plaintext = b"an invite token bundle";
        let sealed = seal(&public, plaintext);
        // header is present and the ciphertext expanded by the AEAD tag.
        assert!(sealed.len() >= SEAL_MIN_LEN + plaintext.len());
        assert_eq!(open(&secret, &sealed).expect("opens"), plaintext);
    }

    #[test]
    fn a_fresh_ephemeral_makes_every_seal_distinct() {
        let (_secret, public) = keypair(1);
        let a = seal(&public, b"same plaintext");
        let b = seal(&public, b"same plaintext");
        assert_ne!(a, b, "each seal mints a fresh ephemeral key + nonce");
    }

    #[test]
    fn the_wrong_recipient_cannot_open() {
        let (_secret, public) = keypair(1);
        let (other_secret, _other_public) = keypair(2);
        let sealed = seal(&public, b"secret");
        assert!(
            open(&other_secret, &sealed).is_err(),
            "a different key must fail closed"
        );
    }

    #[test]
    fn tampering_any_byte_fails_the_tag() {
        let (secret, public) = keypair(1);
        let sealed = seal(&public, b"secret payload");
        for flip in [0usize, EPHEMERAL_PUB_LEN, sealed.len() - 1] {
            let mut bad = sealed.clone();
            bad[flip] ^= 0x01;
            assert!(
                open(&secret, &bad).is_err(),
                "flipping byte {flip} must fail closed"
            );
        }
    }

    #[test]
    fn a_truncated_envelope_is_rejected_not_panicked() {
        let (secret, _public) = keypair(1);
        assert!(open(&secret, b"too short").is_err());
        assert!(open(&secret, &[]).is_err());
    }
}
