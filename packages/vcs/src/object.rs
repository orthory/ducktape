//! content addressing — a git object id.
//!
//! every piece of user content (a document blob, a tree, a commit) is a git
//! object keyed by its git oid. [`ObjectId`] is that key, and it is the SINGLE
//! content-address space for the versioned filesystem: a [`RefUpdate`] names its
//! target by `ObjectId`, an [`Announce`] lists the `ObjectId`s a peer holds.
//!
//! ## the `[u8; 32]` commitment (a named one-way door)
//!
//! 32 bytes is a **sha256** digest, so this type commits the substrate to
//! sha256-mode git — non-default, with real caveats (host/tooling git support
//! varies, no sha1↔sha256 interop). the alternative is sha1 (`[u8; 20]`, the
//! default, but cryptographically weak). this is deliberate and hard to reverse:
//! flipping it later means recomputing every stored hash. `net::ContentStore`'s
//! `sha256(op-batch-bytes)` is a SEPARATE id space (consensus payloads, not git
//! objects) — same width, no shared values.
//!
//! [`RefUpdate`]: crate::op::Op::RefUpdate
//! [`Announce`]: crate::op::Op::Announce

/// a git object id (sha256-mode): the content address of one git object.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct ObjectId([u8; 32]);

impl ObjectId {
    /// wrap raw digest bytes. the caller asserts these are a real git oid —
    /// nothing here recomputes the hash (that is the object store's job, A2).
    /// `const` so fixtures and tests can build ids in a `const` context.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// the raw 32-byte digest.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// the 64-char lowercase hex form — how git names this oid on its CLI.
    pub fn to_hex(&self) -> String {
        use std::fmt::Write;
        let mut s = String::with_capacity(64);
        for b in &self.0 {
            // infallible: writing to a String never errors.
            let _ = write!(s, "{b:02x}");
        }
        s
    }

    /// parse git's 64-char hex oid back into an `ObjectId`.
    ///
    /// rejects anything that isn't exactly 64 hex chars — this is the guard
    /// against a sha1 repo silently leaking 40-char oids into a `[u8; 32]`
    /// space (pair with creating stores via `--object-format=sha256`).
    pub fn from_hex(s: &str) -> Result<Self, FromHexError> {
        if s.len() != 64 {
            return Err(FromHexError::WrongLength(s.len()));
        }
        let mut out = [0u8; 32];
        for (i, byte) in out.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16)
                .map_err(|_| FromHexError::BadDigit)?;
        }
        Ok(Self(out))
    }
}

/// `ObjectId` renders as its git hex oid, so it can be passed straight to a
/// `git` CLI argument and logged readably.
impl std::fmt::Display for ObjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_hex())
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FromHexError {
    #[error("oid must be 64 hex chars (sha256), got {0}")]
    WrongLength(usize),
    #[error("oid contains a non-hex digit")]
    BadDigit,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trips() {
        let id = ObjectId::from_bytes([
            0x2c, 0xf8, 0xd8, 0x3d, 0x9e, 0xe2, 0x95, 0x43, 0xb3, 0x4a, 0x87, 0x72, 0x74, 0x21,
            0xfd, 0xec, 0xb7, 0xe3, 0xf3, 0xa1, 0x83, 0xd3, 0x37, 0x63, 0x90, 0x25, 0xde, 0x57,
            0x6d, 0xb9, 0xeb, 0xb4,
        ]);
        let hex = id.to_hex();
        assert_eq!(hex.len(), 64);
        assert_eq!(
            hex,
            "2cf8d83d9ee29543b34a87727421fdecb7e3f3a183d337639025de576db9ebb4"
        );
        assert_eq!(ObjectId::from_hex(&hex).unwrap(), id);
        assert_eq!(id.to_string(), hex); // Display == to_hex
    }

    #[test]
    fn from_hex_rejects_sha1_length() {
        // a 40-char sha1 oid must NOT parse into the 32-byte space.
        let sha1 = "da39a3ee5e6b4b0d3255bfef95601890afd80709";
        assert_eq!(
            ObjectId::from_hex(sha1),
            Err(FromHexError::WrongLength(40))
        );
    }

    #[test]
    fn from_hex_rejects_non_hex() {
        let bad = "z".repeat(64);
        assert_eq!(ObjectId::from_hex(&bad), Err(FromHexError::BadDigit));
    }
}
