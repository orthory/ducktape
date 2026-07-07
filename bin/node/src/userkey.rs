//! `user.key` v2 codec: argon2id-derived KEK + XChaCha20-Poly1305 at rest,
//! and the BIP39 mnemonic <-> 32-byte-seed encoding used to reveal/restore
//! the user's ed25519 identity. Mirrors `config::load_or_generate_identity`'s
//! file discipline (0600, `create_new`, temp+rename for in-place rewrites)
//! for the persisted file; see
//! docs/superpowers/specs/2026-07-07-identity-onboarding-design.md
//! ("The custody model" + "File format" sections) for the binding design.
//!
//! ## the custody model
//! - **the mnemonic IS the identity.** [`mnemonic_of_seed`] /
//!   [`seed_of_mnemonic`] use `Mnemonic::from_entropy` / `to_entropy` only —
//!   NEVER `to_seed`'s BIP39 PBKDF2 stretch — so the 24 words are a
//!   checksum-carrying, identity-preserving encoding of the raw 32-byte
//!   ed25519 seed. Every existing plaintext v1 `user.key` can be revealed as
//!   a mnemonic retroactively, and restore-by-mnemonic reproduces
//!   byte-identical keys.
//! - **the password is local-encryption-only.** It never enters key
//!   derivation for the identity itself; it only wraps the seed at rest
//!   (argon2id -> XChaCha20-Poly1305 KEK), so a forgotten password is
//!   recoverable via the mnemonic (restore, then set a new password).
//!
//! ## v2 line format
//! `ducktape-user-key-v2:<base64(salt(16) || argon2-params(12) ||
//! nonce(24) || pubkey(32) || ciphertext(32+16))>`
//! - `argon2-params` = `m_kib, t, p` as little-endian `u32`s (default
//!   65536 KiB / 3 / 1), encoded explicitly so they can be raised later
//!   without breaking old files.
//! - `pubkey` rides in the clear — it's public data, and it lets `status`
//!   report identity while locked, without a password.
//! - the version prefix ([`USER_KEY_V2_PREFIX`]) is bound as the AEAD
//!   associated data, so ciphertext minted under one version can never be
//!   replayed as another.
//! - legacy v1 stays a bare 64-lowercase-hex seed (unchanged, see
//!   `config::load_or_generate_identity`).
//!
//! this module's CLI wiring (`user-key init/restore/unlock/reveal/encrypt/
//! status`) lands in a follow-up task in this feature; until then nothing in
//! `main` calls it, so the whole surface reads as dead code to clippy. it IS
//! exercised — see the `#[cfg(test)]` matrix at the bottom of this file.
#![allow(dead_code)]

use std::path::Path;

use argon2::{Algorithm, Argon2, Params, Version};
use base64::Engine as _;
use chacha20poly1305::aead::{Aead as _, Payload};
use chacha20poly1305::{KeyInit as _, XChaCha20Poly1305, XNonce};
use commonware_codec::DecodeExt as _;
use commonware_cryptography::{Signer as _, ed25519};
use rand::RngCore as _;
use zeroize::Zeroizing;

use crate::config::unhex;

/// the v2 line prefix — also the literal AEAD associated data bound to the
/// ciphertext (see module docs).
pub const USER_KEY_V2_PREFIX: &str = "ducktape-user-key-v2:";

/// the error `open_user_key` returns for any AEAD failure — a bad password
/// and a tampered/corrupted file are cryptographically indistinguishable, so
/// this message deliberately doesn't claim to know which.
const WRONG_PASSWORD_ERR: &str = "corrupt or wrong password";

const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 24;
const PARAMS_LEN: usize = 12; // m_kib, t, p as LE u32
const SEED_LEN: usize = 32;
const TAG_LEN: usize = 16;
const CIPHERTEXT_LEN: usize = SEED_LEN + TAG_LEN;
const BLOB_LEN: usize = SALT_LEN + PARAMS_LEN + NONCE_LEN + SEED_LEN + CIPHERTEXT_LEN;

/// argon2id defaults for freshly-sealed files; encoded explicitly per-file
/// (not hardcoded at open time) so a future default bump doesn't strand
/// files sealed under the old ones.
const DEFAULT_M_KIB: u32 = 65536;
const DEFAULT_T_COST: u32 = 3;
const DEFAULT_P_COST: u32 = 1;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::URL_SAFE_NO_PAD;

/// a `user.key` file's contents, sniffed by shape: 64 lowercase hex chars
/// (v1, plaintext) vs the [`USER_KEY_V2_PREFIX`] (v2, encrypted).
#[derive(Debug)]
pub enum UserKeyFile {
    Plaintext(ed25519::PrivateKey),
    Encrypted(EncryptedUserKey),
}

/// a parsed v2 blob. `pubkey` rides in the clear (public data — lets
/// `status` report identity without a password); everything needed to
/// decrypt is held privately, exercised only through [`open_user_key`] (one
/// parse path, not two).
#[derive(Debug)]
pub struct EncryptedUserKey {
    pub pubkey: Vec<u8>,
    salt: [u8; SALT_LEN],
    m_kib: u32,
    t_cost: u32,
    p_cost: u32,
    nonce: [u8; NONCE_LEN],
    ciphertext: Vec<u8>,
}

// ============================================================================
// crypto primitives
// ============================================================================

/// argon2id(password, salt) -> a 32-byte KEK, zeroized on drop.
fn derive_kek(
    password: &str,
    salt: &[u8],
    m_kib: u32,
    t_cost: u32,
    p_cost: u32,
) -> Result<Zeroizing<[u8; 32]>, String> {
    let params = Params::new(m_kib, t_cost, p_cost, Some(32))
        .map_err(|e| format!("invalid argon2 params: {e}"))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut kek = Zeroizing::new([0u8; 32]);
    argon2
        .hash_password_into(password.as_bytes(), salt, kek.as_mut_slice())
        .map_err(|e| format!("argon2 derivation failed: {e}"))?;
    Ok(kek)
}

/// parse a v2 line into its fields, without touching the password/crypto.
fn parse_v2(line: &str) -> Result<EncryptedUserKey, String> {
    let body = line
        .strip_prefix(USER_KEY_V2_PREFIX)
        .ok_or_else(|| "not a v2 user-key line".to_string())?;
    let blob = B64
        .decode(body.trim_end())
        .map_err(|e| format!("v2 line is not valid base64: {e}"))?;
    if blob.len() != BLOB_LEN {
        return Err(format!(
            "v2 blob has wrong length: {} (want {BLOB_LEN})",
            blob.len()
        ));
    }

    let mut off = 0;
    let mut salt = [0u8; SALT_LEN];
    salt.copy_from_slice(&blob[off..off + SALT_LEN]);
    off += SALT_LEN;
    let m_kib = u32::from_le_bytes(blob[off..off + 4].try_into().expect("4 bytes"));
    off += 4;
    let t_cost = u32::from_le_bytes(blob[off..off + 4].try_into().expect("4 bytes"));
    off += 4;
    let p_cost = u32::from_le_bytes(blob[off..off + 4].try_into().expect("4 bytes"));
    off += 4;
    let mut nonce = [0u8; NONCE_LEN];
    nonce.copy_from_slice(&blob[off..off + NONCE_LEN]);
    off += NONCE_LEN;
    let pubkey = blob[off..off + SEED_LEN].to_vec();
    off += SEED_LEN;
    let ciphertext = blob[off..off + CIPHERTEXT_LEN].to_vec();

    Ok(EncryptedUserKey {
        pubkey,
        salt,
        m_kib,
        t_cost,
        p_cost,
        nonce,
        ciphertext,
    })
}

/// decrypt a parsed v2 blob's seed under `password`, binding `aad` (the
/// declared version prefix) as associated data. split out from
/// [`open_user_key`] so the AAD-binding property (a blob sealed under one
/// prefix must not open under another) is directly testable.
fn decrypt_seed(enc: &EncryptedUserKey, password: &str, aad: &[u8]) -> Result<[u8; 32], String> {
    let kek = derive_kek(password, &enc.salt, enc.m_kib, enc.t_cost, enc.p_cost)?;
    let cipher = XChaCha20Poly1305::new_from_slice(kek.as_slice())
        .map_err(|e| format!("bad KEK length: {e}"))?;
    let nonce = XNonce::from_slice(&enc.nonce);
    let plaintext = cipher
        .decrypt(
            nonce,
            Payload {
                msg: &enc.ciphertext,
                aad,
            },
        )
        .map_err(|_| WRONG_PASSWORD_ERR.to_string())?;
    plaintext
        .try_into()
        .map_err(|_| WRONG_PASSWORD_ERR.to_string())
}

// ============================================================================
// public API
// ============================================================================

/// encrypt `seed` under `password` (argon2id defaults + fresh random
/// salt/nonce) and return the full v2 line, ready to write to disk.
pub fn seal_user_key(seed: &[u8; 32], password: &str) -> Result<String, String> {
    let signer = ed25519::PrivateKey::decode(seed.as_slice())
        .map_err(|e| format!("seed is not a valid ed25519 secret: {e}"))?;
    let pubkey = signer.public_key().as_ref().to_vec();

    let mut salt = [0u8; SALT_LEN];
    rand::rngs::OsRng.fill_bytes(&mut salt);
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);

    let kek = derive_kek(
        password,
        &salt,
        DEFAULT_M_KIB,
        DEFAULT_T_COST,
        DEFAULT_P_COST,
    )?;
    let cipher = XChaCha20Poly1305::new_from_slice(kek.as_slice())
        .map_err(|e| format!("bad KEK length: {e}"))?;
    let nonce = XNonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(
            nonce,
            Payload {
                msg: seed.as_slice(),
                aad: USER_KEY_V2_PREFIX.as_bytes(),
            },
        )
        .map_err(|e| format!("encryption failed: {e}"))?;

    let mut blob = Vec::with_capacity(BLOB_LEN);
    blob.extend_from_slice(&salt);
    blob.extend_from_slice(&DEFAULT_M_KIB.to_le_bytes());
    blob.extend_from_slice(&DEFAULT_T_COST.to_le_bytes());
    blob.extend_from_slice(&DEFAULT_P_COST.to_le_bytes());
    blob.extend_from_slice(&nonce_bytes);
    blob.extend_from_slice(&pubkey);
    blob.extend_from_slice(&ciphertext);
    debug_assert_eq!(blob.len(), BLOB_LEN);

    Ok(format!("{USER_KEY_V2_PREFIX}{}", B64.encode(blob)))
}

/// decrypt a v2 line under `password`, returning the seed as a ready-to-use
/// signer. any AEAD failure (wrong password OR a tampered/corrupted file —
/// the two are indistinguishable) returns the exact string
/// `"corrupt or wrong password"`.
pub fn open_user_key(line: &str, password: &str) -> Result<ed25519::PrivateKey, String> {
    let enc = parse_v2(line)?;
    let seed = decrypt_seed(&enc, password, USER_KEY_V2_PREFIX.as_bytes())?;
    ed25519::PrivateKey::decode(seed.as_slice())
        .map_err(|e| format!("decrypted seed is not a valid ed25519 secret: {e}"))
}

/// sniff `path`'s contents: a v2 line (encrypted), a bare 64-hex seed (v1,
/// legacy plaintext — see `config::load_or_generate_identity`), or an error
/// (absent file, or content that's neither shape).
pub fn read_user_key_file(path: &Path) -> Result<UserKeyFile, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("read {path:?}: {e}"))?;
    let line = text.trim();
    if line.is_empty() {
        return Err(format!("{path:?} is empty"));
    }
    if line.starts_with(USER_KEY_V2_PREFIX) {
        return Ok(UserKeyFile::Encrypted(parse_v2(line)?));
    }
    let raw = unhex(line).map_err(|e| format!("{path:?}: {e}"))?;
    let key = ed25519::PrivateKey::decode(raw.as_slice())
        .map_err(|e| format!("{path:?} is not an ed25519 secret: {e}"))?;
    Ok(UserKeyFile::Plaintext(key))
}

/// the 24-word BIP39 mnemonic encoding `seed`'s raw bytes as entropy (with
/// its checksum) — NEVER the BIP39 `to_seed` PBKDF2 stretch, so this is a
/// lossless, identity-preserving encoding: [`seed_of_mnemonic`] recovers the
/// exact same 32 bytes.
pub fn mnemonic_of_seed(seed: &[u8; 32]) -> String {
    bip39::Mnemonic::from_entropy(seed.as_slice())
        .expect("32 bytes is always valid bip39 entropy (256 bits)")
        .to_string()
}

/// the inverse of [`mnemonic_of_seed`]: validates the BIP39 checksum and
/// recovers the original 32-byte seed. a mangled word count, an unknown
/// word, or a flipped word (failing the checksum) all error.
pub fn seed_of_mnemonic(words: &str) -> Result<[u8; 32], String> {
    let mnemonic =
        bip39::Mnemonic::parse(words.trim()).map_err(|e| format!("invalid mnemonic: {e}"))?;
    let entropy = mnemonic.to_entropy();
    let len = entropy.len();
    entropy
        .try_into()
        .map_err(|_| format!("mnemonic encodes {len} bytes of entropy, want 32"))
}

/// write a fresh v2 (or legacy v1) line to `path`. born 0600 (no
/// write-then-chmod window), and `create_new` so a concurrent init can't
/// clobber an existing identity.
pub fn write_user_key_new(path: &Path, line: &str) -> Result<(), String> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("create {dir:?}: {e}"))?;
    }
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        opts.mode(0o600);
    }
    let mut f = opts
        .open(path)
        .map_err(|e| format!("create {path:?}: {e}"))?;
    if let Err(e) = std::io::Write::write_all(&mut f, format!("{line}\n").as_bytes()) {
        // a partial file would shadow every future read with a parse error;
        // remove it so the next attempt starts clean.
        let _ = std::fs::remove_file(path);
        return Err(format!("write {path:?}: {e}"));
    }
    Ok(())
}

/// replace `path`'s content with `line` atomically: write a temp file in
/// the SAME directory (so the rename is same-filesystem, hence atomic),
/// then rename over the target. used for in-place migrations (v1 -> v2,
/// password rotation) where the identity file must never observably not
/// exist.
pub fn rewrite_user_key(path: &Path, line: &str) -> Result<(), String> {
    let dir = path
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .ok_or_else(|| format!("{path:?} has no file name"))?
        .to_string_lossy();
    let tmp_path = dir.join(format!(".{file_name}.tmp-{}", std::process::id()));

    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        opts.mode(0o600);
    }
    {
        let mut f = opts
            .open(&tmp_path)
            .map_err(|e| format!("create {tmp_path:?}: {e}"))?;
        if let Err(e) = std::io::Write::write_all(&mut f, format!("{line}\n").as_bytes()) {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(format!("write {tmp_path:?}: {e}"));
        }
        if let Err(e) = f.sync_all() {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(format!("sync {tmp_path:?}: {e}"));
        }
    }
    std::fs::rename(&tmp_path, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        format!("rename {tmp_path:?} -> {path:?}: {e}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use commonware_cryptography::ed25519;
    use std::io::Read as _;

    fn read_file(path: &std::path::Path) -> String {
        let mut s = String::new();
        std::fs::File::open(path)
            .unwrap()
            .read_to_string(&mut s)
            .unwrap();
        s
    }

    #[test]
    fn seal_open_round_trip() {
        let seed = [1u8; 32];
        let line = seal_user_key(&seed, "correct horse battery staple").unwrap();
        assert!(line.starts_with(USER_KEY_V2_PREFIX));
        let key = open_user_key(&line, "correct horse battery staple").unwrap();
        let expected = ed25519::PrivateKey::decode(seed.as_slice()).unwrap();
        assert_eq!(key.public_key(), expected.public_key());
    }

    #[test]
    fn open_wrong_password_fails_with_exact_message() {
        let seed = [2u8; 32];
        let line = seal_user_key(&seed, "right-password").unwrap();
        let err = open_user_key(&line, "wrong-password").unwrap_err();
        assert_eq!(err, "corrupt or wrong password");
    }

    #[test]
    fn open_tampered_ciphertext_fails_with_exact_message() {
        let seed = [3u8; 32];
        let line = seal_user_key(&seed, "pw").unwrap();
        let body = line.strip_prefix(USER_KEY_V2_PREFIX).unwrap();
        let mut blob = B64.decode(body).unwrap();
        let last = blob.len() - 1;
        blob[last] ^= 0xff; // flip a byte inside the AEAD tag/ciphertext
        let tampered = format!("{USER_KEY_V2_PREFIX}{}", B64.encode(blob));
        let err = open_user_key(&tampered, "pw").unwrap_err();
        assert_eq!(err, "corrupt or wrong password");
    }

    #[test]
    fn decrypt_seed_with_wrong_aad_fails_with_exact_message() {
        // white-box: the version prefix is bound as AEAD associated data,
        // so a blob sealed under one declared prefix must fail to open
        // under a different one, even with the right password.
        let seed = [4u8; 32];
        let line = seal_user_key(&seed, "pw").unwrap();
        let enc = parse_v2(&line).unwrap();
        let err = decrypt_seed(&enc, "pw", b"ducktape-user-key-v3:").unwrap_err();
        assert_eq!(err, "corrupt or wrong password");
    }

    #[test]
    fn v2_line_parses_to_encrypted_with_clear_pubkey() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("user.key");
        let seed = [5u8; 32];
        let line = seal_user_key(&seed, "pw").unwrap();
        write_user_key_new(&path, &line).unwrap();

        let expected = ed25519::PrivateKey::decode(seed.as_slice())
            .unwrap()
            .public_key();
        match read_user_key_file(&path).unwrap() {
            UserKeyFile::Encrypted(enc) => assert_eq!(enc.pubkey, expected.as_ref().to_vec()),
            UserKeyFile::Plaintext(_) => panic!("expected Encrypted"),
        }
    }

    #[test]
    fn legacy_hex_parses_to_plaintext_with_matching_pubkey() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("user.key");
        let seed = [6u8; 32];
        let key = ed25519::PrivateKey::decode(seed.as_slice()).unwrap();
        let hex_line = seed.iter().map(|b| format!("{b:02x}")).collect::<String>();
        write_user_key_new(&path, &hex_line).unwrap();

        match read_user_key_file(&path).unwrap() {
            UserKeyFile::Plaintext(k) => assert_eq!(k.public_key(), key.public_key()),
            UserKeyFile::Encrypted(_) => panic!("expected Plaintext"),
        }
    }

    #[test]
    fn absent_file_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope.key");
        assert!(read_user_key_file(&path).is_err());
    }

    #[test]
    fn junk_content_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("user.key");
        write_user_key_new(&path, "not a hex seed and not a v2 line").unwrap();
        assert!(read_user_key_file(&path).is_err());
    }

    #[test]
    fn mnemonic_seed_round_trip_fixed_vector() {
        let seed = [7u8; 32];
        let words = mnemonic_of_seed(&seed);
        assert_eq!(words.split_whitespace().count(), 24);

        let restored = seed_of_mnemonic(&words).unwrap();
        assert_eq!(restored, seed);

        // identity-preserving: the pubkey from the restored bytes, decoded
        // the same way `config::load_or_generate_identity` decodes a raw
        // seed file, matches the original.
        let original_key = ed25519::PrivateKey::decode(seed.as_slice()).unwrap();
        let restored_key = ed25519::PrivateKey::decode(restored.as_slice()).unwrap();
        assert_eq!(original_key.public_key(), restored_key.public_key());
    }

    #[test]
    fn one_word_flipped_mnemonic_fails_checksum() {
        let seed = [8u8; 32];
        let words = mnemonic_of_seed(&seed);
        let mut parts: Vec<&str> = words.split_whitespace().collect();
        let wordlist = bip39::Language::English.word_list();
        let idx0 = bip39::Language::English.find_word(parts[0]).unwrap() as usize;
        let replacement = wordlist[(idx0 + 1) % wordlist.len()];
        parts[0] = replacement;
        let flipped = parts.join(" ");
        assert!(seed_of_mnemonic(&flipped).is_err());
    }

    #[test]
    fn argon2_params_round_trip_through_encoded_line() {
        let seed = [9u8; 32];
        let line = seal_user_key(&seed, "pw").unwrap();
        let enc = parse_v2(&line).unwrap();
        assert_eq!(enc.m_kib, 65536);
        assert_eq!(enc.t_cost, 3);
        assert_eq!(enc.p_cost, 1);
    }

    #[test]
    fn write_user_key_new_refuses_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("user.key");
        write_user_key_new(&path, "first").unwrap();
        let err = write_user_key_new(&path, "second").unwrap_err();
        assert!(!err.is_empty());
        assert_eq!(read_file(&path).trim(), "first");
    }

    #[test]
    fn rewrite_user_key_replaces_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("user.key");
        write_user_key_new(&path, "old-content").unwrap();
        rewrite_user_key(&path, "new-content").unwrap();
        let contents = read_file(&path);
        assert_eq!(contents.trim(), "new-content");
        assert!(!contents.contains("old-content"));

        // no stray temp file left behind in the directory.
        let entries: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(entries, vec![std::ffi::OsString::from("user.key")]);
    }

    #[cfg(unix)]
    #[test]
    fn files_are_written_0600() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("user.key");
        write_user_key_new(&path, "content").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);

        rewrite_user_key(&path, "content2").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
