//! qmdb-backed pages module — a notion-like block tree, one block per key.
//!
//! a page is a TREE of [`Block`]s: the page itself is the root block (kind
//! `Page`, text == title), every block carries an ordered `children` list, and
//! every block id is GLOBALLY UNIQUE within the module. unlike the document
//! module's whole-doc-per-key layout, the qmdb key here is `sha256(block_id)`
//! and the value is ONE serialized block — so the merkle root commits to every
//! block individually, and a single block is readable (and one day provable)
//! by id alone with no page context. that is the addressability contract that
//! lets other modules hold a [`crate::BlockRef`] today and resolve
//! it via `Ctx::query(pages, GetBlock { block_id })`.
//!
//! ## keys are hashed to a fixed width
//!
//! the logical key is the `block_id` string at the interface seam, but the
//! qmdb key is `sha256(block_id)` — a fixed 32-byte [`commonware_utils`]
//! `Array`, mirroring the kv/document modules. this is load-bearing:
//! commonware's state-sync resolvers for the overwriteable variable db are
//! bounded on `K: Array`.
//!
//! ## enumeration via a reserved index entry
//!
//! one extra qmdb entry is reserved: the sentinel logical key
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
//! flushed to qmdb in ONE batch by `commit_block`; `abort_block` drops the
//! overlay. the pages twist: `RemoveBlock` deletes a whole subtree, so the
//! overlay value is an `Option<Vec<u8>>` — `Some` stages a write, `None`
//! stages a DELETE (qmdb's `batch.write(key, None)`), and reads through the
//! overlay see a staged delete as absence.
//!
//! ## state-sync
//!
//! identical to the document module: [`Pages::sync_target`] /
//! [`Pages::sync_from`] delegate to commonware's qmdb sync engine, so a joiner
//! rebuilds a byte-identical root from an untrusted peer, merkle-verified
//! against the target root.

// the wire surface: this module's shared types, flattened at the crate root.
mod interface;
pub use interface::*;
// the derived-tier materialized view; registered only by serving binaries.
pub mod index;

use std::collections::BTreeMap;
use std::num::{NonZeroU16, NonZeroU64, NonZeroUsize};
use std::sync::Arc;

use commonware_codec::RangeCfg;
use commonware_cryptography::{Hasher, Sha256};
use commonware_parallel::Sequential;
use commonware_runtime::{BufferPooler, buffer::paged::CacheRef};
use commonware_storage::{
    Context, journal, mmr,
    qmdb::{
        any::{VariableConfig, unordered::variable::Db},
        sync::{self, DbResolver, Target, engine::Config as SyncConfig},
    },
    translator::TwoCap,
};
use commonware_utils::range::NonEmptyRange;

use sdk::{
    Ctx, Error, Module, ModuleId, Msg, Origin, ResolverSyncTarget, StateRoot, StateSyncHandle,
};

mod block_ops;
mod comment_ops;
mod error;
mod module_impl;
mod ops;
mod page_ops;
mod store;

use error::{PageError, to_page_err};
use store::hash_key;

/// write-time cap on ONE serialized block record (and on the enumeration
/// index value — both stage through the same guard). the codec [`RangeCfg`]
/// bounds a stored value at 1 MiB AT DECODE TIME only, so an oversized value
/// that staged fine would panic every later read on every validator: a poison
/// pill. 768 KiB leaves the same 256 KiB framing margin the document module
/// keeps. a block record carries its text plus its ordered child-id list, so
/// this also bounds a single parent to tens of thousands of children.
pub const MAX_BLOCK_LEN: usize = 768 * 1024;

/// the reserved logical key under which the page-enumeration INDEX rides in
/// the same qmdb. its value is a serialized sorted `Vec<String>` of every page
/// (root block) id. the leading NUL makes it UNCOLLIDABLE with a real block id
/// (clients mint uuids), and every op that names it is rejected
/// ([`PageError::ReservedId`]) before it can reach storage.
const PAGE_INDEX_KEY: &str = "\u{0}page-index";

/// how many parent hops a MoveBlock ancestry walk will follow before declaring
/// the stored tree corrupt. committed state is acyclic by construction (every
/// move re-checks), so a walk this deep can only mean a broken parent chain —
/// the cap turns a would-be infinite loop into a loud deterministic error.
const MAX_DEPTH: usize = 10_000;

/// the qmdb key: a fixed 32-byte sha256 digest of the `block_id`. fixed width
/// is what lets a store be state-synced (resolvers require `K: Array`).
type PageKey = <Sha256 as Hasher>::Digest;

/// the concrete qmdb store — identical params to the kv/document modules, so
/// all qmdb plumbing is shared verbatim.
type PagesDb<E> = Db<mmr::Family, E, PageKey, Vec<u8>, Sha256, TwoCap, Sequential>;

/// the qmdb configuration — shared by [`Pages::init`] (fresh open) and
/// [`Pages::sync_from`] (state-sync target) so a synced store's storage layout
/// is byte-identical to a freshly-opened one.
type PagesConfig = VariableConfig<TwoCap, ((), (RangeCfg<usize>, ())), Sequential>;

/// a state-sync target: a qmdb merkle root plus the operation range a joiner
/// must pull to reconstruct a store with an identical root.
pub type PagesTarget = Target<mmr::Family, PageKey>;

/// a qmdb-backed, block-tree pages module.
pub struct Pages<E>
where
    E: Context + BufferPooler,
{
    id: ModuleId,
    db: PagesDb<E>,
    /// blocks touched this block-height, keyed by LOGICAL `block_id` bytes.
    /// `Some(bytes)` stages a write, `None` stages a DELETE (subtree removal).
    /// read ahead of committed state by `get` (read-your-writes) and flushed
    /// to qmdb in one batch by `commit_block`; NOT in `root()` until then.
    pending: BTreeMap<Vec<u8>, Option<Vec<u8>>>,
}

#[cfg(test)]
mod tests;
