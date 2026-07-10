//! Ducktape module adapter for duckfs.
//!
//! The deterministic filesystem core lives in `duckfs-core`; native persistence
//! lives in `duckfs-disk`; this crate keeps the consensus module id, SDK module
//! implementation, and typed `FsCap` helper. It re-exports the core and disk
//! APIs for one compatibility pass while repo callers migrate to the new crates.

pub use duckfs_core::*;

#[cfg(feature = "native")]
mod cap;
#[cfg(feature = "native")]
mod module;

#[cfg(feature = "native")]
pub use cap::{FsCap, Notify, decode_notify};
#[cfg(feature = "native")]
pub use duckfs_disk::{DiskRefs, DiskStore, SyncScratch};
#[cfg(feature = "native")]
pub use module::{Files, owner_of};

#[cfg(not(feature = "native"))]
pub use duckfs_core::testkit;

#[cfg(feature = "native")]
#[doc(hidden)]
pub mod testkit {
    pub use duckfs_core::testkit::*;

    pub fn gc_due(height: u64, watermark: u64) -> bool {
        crate::module::gc_due(height, watermark)
    }
}
