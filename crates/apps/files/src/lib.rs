//! duckfs — a consensus-replicated, copy-on-write, content-addressed
//! filesystem module. every node holds every byte as consensus state, and every
//! honest node computes the byte-identical filesystem: this crate is the whole
//! deterministic story, from the wire op to the disk commit point.
//!
//! # objects, refs, and the root
//!
//! state is split into an immutable **object store** (the odb) and a small
//! mutable **refs** cell. objects — chunks, files, trees, and snapshots — are
//! content-addressed: an [`ObjectId`] is `sha256(kind_tag ‖ body)`, so identical
//! bytes are one object network-wide and every reference is self-verifying. the
//! odb is an append-only, dedup-by-hash store; it is NOT hashed into the module
//! root. the [`Refs`] cell holds the mutable pointers — the head snapshot, the
//! putblob staging table, pins, module watches, and the bounded history window —
//! and the module [`root()`](sdk::Module::root) is sha256 over the canonical
//! encoding of `Refs` ALONE ([`root_bytes`]). the odb is reachable from the head,
//! so the root commits to the whole tree transitively without hashing every byte.
//!
//! # the byte path: staging → commit
//!
//! large content rides two hops. a `putblob` op stages a raw chunk (capped at
//! [`CHUNK_SIZE`], per-owner quota, a deterministic op-stream-driven TTL sweep):
//! the bytes land in the odb and a staging entry lands in refs (staging IS state,
//! so the root moves). a later `Commit` references those chunks by digest (or
//! carries small files inline), threading a per-path CAS check against a base
//! snapshot: every changed path must be identical between the base and the live
//! head or the whole atomic commit rejects. small edits ride entirely inside the
//! commit op; only chunk-sized content needs the putblob hop.
//!
//! # copy-on-write trees and queries
//!
//! a commit rewrites only the tree nodes along each touched path — untouched
//! subtrees keep their existing object ids, so a snapshot shares almost all of
//! its structure with its parent (copy-on-write). reads are pure functions of a
//! snapshot: [`FilesQuery`] serves `Stat`/`Ls`/`Read`/`Find`/`Grep`/`History`/
//! `Diff`/`Refs`, each snapshot-addressed (defaulting to head) and paged with a
//! deterministic cursor, so a query is reproducible at any committed boundary.
//!
//! # gc neutrality
//!
//! garbage collection is mark-and-sweep over the refs-reachable object set. it is
//! consensus-NEUTRAL: it only ever removes objects unreachable from committed
//! refs, and unreachable-on-one-node is unreachable on every node, so a sweep
//! never moves the root. its cadence is per-node bookkeeping (a watermark trigger
//! in the glue), never a consensus input.
//!
//! # the sync lane
//!
//! a joining or self-healing node adopts the tiny verified refs image
//! (`snapshot_refs` → `install_refs`, checked against the expected root) and then
//! back-fills objects off-block through a batched [`FilesSyncReq`] fetch, driven
//! by `missing_objects` until possession is complete. the object fetch runs
//! outside the block lane, so it never blocks consensus.
//!
//! # durability ordering
//!
//! the disk glue persists in a strict order that survives a crash without a torn
//! root: drain the block purely, flush objects into the odb, fsync the odb dirs,
//! save the refs file (the atomic commit point), and only THEN adopt the new refs
//! in core — so the committed root can never advance ahead of the durable refs
//! file. see [`Files::commit_block`](struct@Files).
//!
//! # purity boundary and the wasm gate
//!
//! this is ONE crate with a feature-gated purity boundary. the always-compiled
//! core — `wire`, [`objects`], [`paths`], [`store`], [`state`], `tree`, [`fs`],
//! [`queries`], [`gc`] — is the future wasm unit: no `std::fs`, no sdk, no async
//! anywhere in it, enforced by the `cargo check -p files --no-default-features`
//! gate. the `native` feature (default) adds the disk stores (`disk`) and the sdk
//! [`Module`](sdk::Module) glue (`module`), which is the only place origins,
//! async, and the filesystem live.
//!
//! spec: `docs/superpowers/specs/2026-07-06-duckfs-real-filesystem-design.md`.

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
// `tree` stays private: its read/edit surface is `pub` (so the crate glue and
// the hidden `testkit` seam can reach it) but the module path is not part of the
// public api. tests drive it through `files::testkit`.
mod tree;

// `#[doc(hidden)]` test-only facade re-exporting the tree surface for the
// out-of-crate integration tests. always compiled — it is pure.
#[doc(hidden)]
pub mod testkit;

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
