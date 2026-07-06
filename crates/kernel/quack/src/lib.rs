//! Quack — the Ducktape package toolkit.
//!
//! A Quack package *source* is a directory (`quack.toml` at the root, resource
//! files beside it); a `.quack` file is a deterministic ustar tar of that
//! directory. This crate is the pure, dependency-light core the `ducktape-node
//! package` verbs (and, later, the on-chain `package` module tooling) build on:
//!
//! - [`manifest`] — parse + validate the `quack.toml` schema, and derive the
//!   domain-separated manifest hash that the signature commits to.
//! - [`capsule`] — read a package from a directory or a `.quack` tar, write the
//!   deterministic tar back, and check every declared content digest.
//! - [`sign`] — sign / verify the manifest hash with the platform's
//!   `commonware_cryptography` ed25519 shape.
//!
//! It carries no `sdk` dependency and no consensus code: it only ever touches
//! bytes on disk and the manifest that describes them.

pub mod capsule;
pub mod manifest;
pub mod sign;

pub use capsule::{
    Capsule, CapsuleError, build_tar, file_digest, open_dir, open_tar, verify_digests,
};
pub use manifest::{
    ActionEntry, AgentEntry, EngagementEntry, InstallPolicy, ManifestError, ModuleEntry,
    ModuleKind, PackageManifest, PromptEntry, Requires, UninstallPolicy, manifest_hash,
    parse_manifest, validate, validate_tag,
};
pub use sign::{PackageSig, SIG_NAMESPACE, sign_manifest, verify_manifest_sig};

/// Lowercase-hex encode raw bytes — the dependency-free codec every hash/key
/// field in this crate renders through (mirrors `bin/node`'s `hex_bytes`).
pub(crate) fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Decode strict lowercase-or-uppercase hex; `None` on any non-hex byte or an
/// odd length (so a malformed `sha256:` field is a clean rejection, never a
/// panic mid-parse).
pub(crate) fn from_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

/// serde adapter: encode a `Vec<u8>` field as a hex string (used by the
/// `signatures/package.sig` JSON so signer/sig read as hex, not byte arrays).
pub(crate) mod hexser {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&super::to_hex(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        super::from_hex(&s).ok_or_else(|| serde::de::Error::custom("field is not valid hex"))
    }
}

#[cfg(test)]
mod hex_tests {
    use super::{from_hex, to_hex};

    #[test]
    fn hex_round_trips() {
        let raw = [0x00u8, 0x0f, 0xa1, 0xff];
        assert_eq!(to_hex(&raw), "000fa1ff");
        assert_eq!(from_hex("000fa1ff").as_deref(), Some(&raw[..]));
    }

    #[test]
    fn hex_rejects_malformed() {
        assert_eq!(from_hex("abc"), None); // odd length
        assert_eq!(from_hex("zz"), None); // non-hex
    }
}
