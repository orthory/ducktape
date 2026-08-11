//! Shared symmetric primitives: HKDF-SHA256 key derivation and a
//! ChaCha20-Poly1305 `nonce ‖ ciphertext` envelope. `seal` (anonymous sealed
//! box) and `handshake` (session key) share these and differ ONLY in how they
//! agree the key.

use anyhow::{bail, Result};
use chacha20poly1305::aead::Aead;
use chacha20poly1305::{ChaCha20Poly1305, Key, KeyInit, Nonce};
use rand_core::{OsRng, RngCore};
use hkdf::Hkdf;
use sha2::Sha256;

/// HKDF-SHA256 with an explicit salt (per-stream keys in `bodyseal`).
pub fn hkdf32_salted(shared: &[u8; 32], salt: &[u8], label: &[u8]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(Some(salt), shared);
    let mut okm = [0u8; 32];
    hk.expand(label, &mut okm)
        .expect("32 is a valid HKDF-SHA256 output length");
    okm
}

/// HKDF-SHA256 → 32-byte key from a shared secret, domain-separated by `label`.
pub fn hkdf32(shared: &[u8; 32], label: &[u8]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(None, shared);
    let mut okm = [0u8; 32];
    hk.expand(label, &mut okm)
        .expect("32 is a valid HKDF-SHA256 output length");
    okm
}

/// AEAD-seal `msg` under `key`. Layout: `nonce(12) ‖ ct(+16 tag)`.
pub fn seal(key: &[u8; 32], msg: &[u8]) -> Vec<u8> {
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let ct = cipher
        .encrypt(Nonce::from_slice(&nonce), msg)
        .expect("ChaCha20-Poly1305 encryption does not fail on valid inputs");
    let mut out = Vec::with_capacity(12 + ct.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    out
}

/// Open a `nonce ‖ ct` envelope. Errors on a short, wrong-key, or tampered blob.
pub fn open(key: &[u8; 32], blob: &[u8]) -> Result<Vec<u8>> {
    if blob.len() < 12 + 16 {
        bail!("AEAD blob too short: {} bytes", blob.len());
    }
    let nonce: [u8; 12] = blob[..12].try_into().unwrap();
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    cipher
        .decrypt(Nonce::from_slice(&nonce), &blob[12..])
        .map_err(|_| anyhow::anyhow!("AEAD decryption failed (wrong key or tampered blob)"))
}
