//! duckfs — a consensus-replicated, copy-on-write, content-addressed
//! filesystem. every node holds every byte as consensus state; bytes travel
//! through blocks (putblob staging + atomic commits). immutable objects
//! (chunk/file/tree/snapshot) live in a content-addressed object store; the
//! module root is sha256 over the canonical encoding of the small mutable
//! [`state::Refs`] only. spec:
//! docs/superpowers/specs/2026-07-06-duckfs-real-filesystem-design.md
//!
//! one crate, feature-gated purity: the always-compiled core (wire, objects,
//! paths, store, state, tree, fs, queries, gc) is the future wasm unit — no
//! `std::fs`, no sdk, no async anywhere in it. the `native` feature (default)
//! adds the disk stores and the sdk module glue.

// the wire surface: this module's shared types, flattened at the crate root.
mod wire;
pub use wire::*;

pub mod fs;
pub mod gc;
pub mod objects;
pub mod paths;
pub mod queries;
pub mod state;
pub mod store;
pub mod tree;

pub use fs::{Fs, Notification, StagedObjects};
pub use objects::{Kind, ObjectId};
pub use state::{PinEntry, Refs, Staged, decode_refs, encode_refs, root_bytes};
pub use store::{MemRefs, MemStore, ObjectStore, RefsStore};

#[cfg(feature = "native")]
mod disk;
#[cfg(feature = "native")]
mod module;

#[cfg(feature = "native")]
pub use disk::{DiskRefs, DiskStore};
#[cfg(feature = "native")]
pub use module::{Files, owner_of};
