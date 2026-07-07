//! `#[doc(hidden)]` test facade — always compiled (pure). integration tests
//! live outside the crate and cannot name `pub(crate)`/private-module internals,
//! so this module re-exports exactly the tree read/edit surface `tests/
//! tree_edit.rs` drives. keeping `mod tree` private and its items `pub` means
//! the real public api never grows a `files::tree::*` path — the engine is
//! reachable only through this hidden seam.

pub use crate::tree::{Store, TreeEdit, entry_at, snapshot_root_tree};

// `gc_due` is the `pub(crate)` gc trigger predicate in the native glue; expose a
// thin pub wrapper here (native-gated) so the out-of-crate task-13 trigger test
// can table-drive the period boundary without widening the real public api.
#[cfg(feature = "native")]
pub fn gc_due(height: u64, watermark: u64) -> bool {
    crate::module::gc_due(height, watermark)
}
