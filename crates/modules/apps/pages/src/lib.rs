//! qmdb-backed pages module — a notion-like block tree, one block per key.
//!
//! a page is a TREE of [`Block`]s: the page itself is the root block (kind
//! `Page`, text == title), every block carries an ordered `children` list, and
//! every block id is GLOBALLY UNIQUE within the module. unlike the document
//! module's whole-doc-per-key layout, the store key here is `sha256(block_id)`
//! and the value is ONE serialized block — so the merkle root commits to every
//! block individually, and a single block is readable (and one day provable)
//! by id alone with no page context. that is the addressability contract: any
//! module can resolve a bare block id via `Ctx::query(pages, GetBlock {
//! block_id })`.
//!
//! pure logic over a host-injected [`sdk::MerkleStore`]: the HOST constructs
//! the concrete store (qmdb today — `statesync::qmdb::QmdbStore`) and hands it
//! to [`Pages::new`], so this crate never names a storage crate. the module's
//! authenticated [`StateRoot`] IS the store's merkle root, so it flows
//! directly into the global root-hash via `host::global_root`.
//!
//! ## keys are hashed to a fixed width
//!
//! the logical key is the `block_id` string at the interface seam, but the
//! store key is `sha256(block_id)` — a fixed 32-byte digest, mirroring the
//! kv/document modules. this is load-bearing: the store's state-sync
//! resolvers are bounded on fixed-width keys.
//!
//! ## enumeration via a reserved index entry
//!
//! one extra store entry is reserved: the sentinel logical key
//! [`PAGE_INDEX_KEY`] whose value is the serialized SORTED set of every page
//! (root block) id. its leading NUL makes it uncollidable with a client-minted
//! block id, and every op that names it is rejected before any storage touch.
//! only [`PageMsg::CreatePage`] grows the index; block edits never touch it —
//! and because block ops can neither insert nor convert to kind `Page`,
//! removal of a subtree can never orphan an index entry.
//!
//! ## host-lent staging (the kv/document pattern, plus deletes)
//!
//! writes made during a block are STAGED in an in-memory `pending` overlay and
//! flushed to the store in ONE batch by `commit_block`; `abort_block` drops
//! the overlay. the pages twist: `RemoveBlock` deletes a whole subtree, so the
//! overlay value is an `Option<Vec<u8>>` — `Some` stages a write, `None`
//! stages a DELETE (`commit_batch`'s `None`), and reads through the overlay
//! see a staged delete as absence.
//!
//! ## state-sync
//!
//! sync belongs to the injected store, not this module: a joiner (dynamic-
//! valset catch-up, a fresh full node, crash recovery) rebuilds the CONCRETE
//! store from a peer (`QmdbStore::sync_from`) and wraps a fresh `Pages` around
//! it. this module only forwards the trait's serve surface —
//! [`Module::serve_sync`] and [`Module::resolver_sync_target`] delegate
//! straight to the store.

// the wire surface: this module's shared types, flattened at the crate root.
mod interface;
pub use interface::*;
// the derived-tier materialized view; registered only by serving binaries —
// native-only (indexer drags fluent31's unix IO), never consensus state, so
// the wasm guest builds without it.
#[cfg(feature = "native")]
pub mod index;

use std::collections::BTreeMap;

use sdk::{
    Ctx, Error, MerkleStore, Module, ModuleId, Msg, Origin, ResolverSyncTarget, StagedStore,
    StateRoot, StateSyncHandle,
};
use tagging::{TagEvent, TaggingMsg};

mod block_ops;
mod comment_ops;
mod error;
mod module_impl;
mod ops;
mod page_ops;
mod store;
mod text_ranges;

use error::{PageError, to_page_err};

/// write-time cap on ONE serialized block record (and on the enumeration
/// index value — both stage through the same guard). the concrete store's
/// codec bounds a stored value at 1 MiB AT DECODE TIME only (see
/// `statesync::qmdb::store_config`), so an oversized value that staged fine
/// would panic every later read on every validator: a poison pill. 768 KiB
/// leaves the same 256 KiB framing margin the document module keeps. a block
/// record carries its text plus its ordered child-id list, so this also
/// bounds a single parent to tens of thousands of children.
pub const MAX_BLOCK_LEN: usize = 768 * 1024;

/// the reserved logical key under which the page-enumeration INDEX rides in
/// the same store. its value is a serialized sorted `Vec<String>` of every
/// page (root block) id. the leading NUL makes it UNCOLLIDABLE with a real
/// block id (clients mint uuids), and every op that names it is rejected
/// ([`PageError::ReservedId`]) before it can reach storage.
const PAGE_INDEX_KEY: &str = "\u{0}page-index";

/// how many parent hops a MoveBlock ancestry walk will follow before declaring
/// the stored tree corrupt. committed state is acyclic by construction (every
/// move re-checks), so a walk this deep can only mean a broken parent chain —
/// the cap turns a would-be infinite loop into a loud deterministic error.
const MAX_DEPTH: usize = 10_000;

/// a block-tree pages module over a host-injected authenticated store.
pub struct Pages {
    id: ModuleId,
    /// the host-injected authenticated store plus this block-height's staging
    /// overlay: blocks touched this block are staged (a write, or a `None`
    /// DELETE for subtree removal), read ahead of committed state
    /// (read-your-writes), and flushed to the store in one batch at
    /// `commit_block`; NOT in `root()` until then. store key is
    /// `sha256(block_id)`, owned by [`StagedStore`].
    staged: StagedStore,
    /// Optional engagement router. Tests/minimal registries may leave it
    /// unwired; production reports each newly-added comment after staging it.
    tagging: Option<ModuleId>,
}

#[cfg(test)]
mod tests;

// the wasm-guest port: the dispatch shell that adapts this module to the
// ducktape:module world. compiled only by the guest-builder's synthesized
// wasm32 cdylib workspace (feature `guest`), never by the native build.
#[cfg(feature = "guest")]
mod guest;
