//! the immutable object model — chunk/file/tree/snapshot records in the
//! content-addressed store. task 3 fills the canonical encodings and the id
//! derivation rules; this skeleton pins only the shared names.

/// a raw 32-byte object id: sha256 over `tag ‖ body` (64-char lowercase hex
/// on the wire).
pub type ObjectId = [u8; 32];

/// object kind — the domain-separating tag byte in every id preimage and the
/// `kind` field of a sync-fetched object. task 3 owns the final values; the
/// flag-day rule makes renumbering free until then.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Kind {
    Chunk = 0,
    File = 1,
    Tree = 2,
    Snapshot = 3,
}

impl Kind {
    /// the id-preimage / wire tag byte.
    pub fn tag(self) -> u8 {
        self as u8
    }
}
