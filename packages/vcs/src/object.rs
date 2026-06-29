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
}
