//! the duckfs checkout/commit engine — the OS-side half of duckfs.
//!
//! consensus never sees this crate. everything here lives strictly on the client
//! side of the wire (spec: the determinism boundary): the tree, ids, and paths
//! are recomputed with the `files` pure core so a client-derived object id is
//! byte-identical to what every validator derives, but OS-specific concerns
//! (mtime granularity, case-insensitive filesystems, NFD normalization,
//! symlinks, exec bits) only ever shape a node's local working copy — never the
//! replicated state. all node access flows through one small SYNCHRONOUS
//! [`NodeApi`] trait so a colocated-odb fast path can slot in for phase-4 FUSE.
pub mod api;
pub mod checkout;
pub mod chunk;
pub mod index;
pub mod scan;
pub mod status;
