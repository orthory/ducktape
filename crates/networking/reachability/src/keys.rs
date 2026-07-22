//! The node's WireGuard identity: an X25519 keypair persisted beside
//! `identity.key`. Deliberately SEPARATE from the ed25519 signer — WireGuard
//! speaks X25519, and the two rotate independently: a WireGuard re-key is a
//! re-advertisement (a new mesh version), never an identity change.

use std::path::Path;

use wireguard::X25519PublicKey;

#[derive(Debug, thiserror::Error)]
pub enum KeyError {
    #[error("wireguard key file {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("wireguard key file {path}: expected 64 hex chars")]
    Malformed { path: String },
}

/// The persistent X25519 keypair driving this node's WireGuard interface.
pub struct WireGuardKeypair {
    secret: x25519_dalek::StaticSecret,
}

impl WireGuardKeypair {
    /// Load the keypair at `path`, or generate one and persist it. Returns
    /// `(keypair, generated)`. Mirrors `identity.key` discipline exactly:
    /// 64 hex chars on one line, and the file is BORN 0600 via `create_new`
    /// (no write-then-chmod window a co-tenant could read the secret
    /// through; a concurrent keygen loses cleanly instead of clobbering).
    pub fn load_or_generate(path: &Path) -> Result<(Self, bool), KeyError> {
        if path.exists() {
            return Self::load(path).map(|k| (k, false));
        }
        let mut raw = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut raw);
        // every 32-byte string is a valid X25519 secret (the scheme clamps),
        // so construction cannot fail on fresh OS randomness.
        let secret = x25519_dalek::StaticSecret::from(raw);
        let encoded = hex_encode(&secret.to_bytes());
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| KeyError::Io {
                path: path.display().to_string(),
                source: e,
            })?;
        }
        {
            use std::io::Write as _;
            let mut opts = std::fs::OpenOptions::new();
            opts.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt as _;
                opts.mode(0o600);
            }
            let mut file = opts.open(path).map_err(|e| KeyError::Io {
                path: path.display().to_string(),
                source: e,
            })?;
            file.write_all(encoded.as_bytes())
                .map_err(|e| KeyError::Io {
                    path: path.display().to_string(),
                    source: e,
                })?;
        }
        Ok((Self { secret }, true))
    }

    fn load(path: &Path) -> Result<Self, KeyError> {
        let text = std::fs::read_to_string(path).map_err(|e| KeyError::Io {
            path: path.display().to_string(),
            source: e,
        })?;
        let raw = hex_decode32(text.trim()).ok_or_else(|| KeyError::Malformed {
            path: path.display().to_string(),
        })?;
        Ok(Self {
            secret: x25519_dalek::StaticSecret::from(raw),
        })
    }

    /// The public key peers advertise for us and pin in tunnel handshakes.
    pub fn public_key(&self) -> X25519PublicKey {
        X25519PublicKey(x25519_dalek::PublicKey::from(&self.secret).to_bytes())
    }

    /// The private key in the base64 form
    /// `defguard_wireguard_rs::InterfaceConfiguration.prvkey` expects.
    pub fn private_key_base64(&self) -> String {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD.encode(self.secret.to_bytes())
    }

    /// Open a [`crate::seal`] envelope sealed to this node's WireGuard X25519
    /// key. The secret never leaves the keypair — a member opens a joiner's
    /// sealed first-contact intro through here (join ADR, item 5).
    pub fn open_sealed(&self, sealed: &[u8]) -> Result<Vec<u8>, String> {
        crate::seal::open(&self.secret, sealed)
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_decode32(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, chunk) in text.as_bytes().chunks(2).enumerate() {
        let hi = (chunk[0] as char).to_digit(16)?;
        let lo = (chunk[1] as char).to_digit(16)?;
        out[i] = ((hi << 4) | lo) as u8;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_persists_and_reloads_the_same_keypair() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wireguard.key");

        let (fresh, generated) = WireGuardKeypair::load_or_generate(&path).unwrap();
        assert!(generated);
        assert_ne!(fresh.public_key().0, [0u8; 32]);

        let (reloaded, generated_again) = WireGuardKeypair::load_or_generate(&path).unwrap();
        assert!(!generated_again);
        assert_eq!(reloaded.public_key(), fresh.public_key());
        assert_eq!(reloaded.private_key_base64(), fresh.private_key_base64());

        // the base64 form decodes back to 32 bytes — what defguard's prvkey
        // parser requires.
        use base64::Engine as _;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(fresh.private_key_base64())
            .unwrap();
        assert_eq!(decoded.len(), 32);
    }

    #[cfg(unix)]
    #[test]
    fn key_file_is_born_private() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wireguard.key");
        WireGuardKeypair::load_or_generate(&path).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn rejects_a_corrupt_key_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wireguard.key");
        std::fs::write(&path, "not hex at all").unwrap();
        assert!(matches!(
            WireGuardKeypair::load_or_generate(&path),
            Err(KeyError::Malformed { .. })
        ));
    }
}
