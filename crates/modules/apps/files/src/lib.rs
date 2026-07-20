//! Ducktape module adapter for duckfs.
//!
//! The deterministic filesystem core lives in `duckfs-core`; native persistence
//! lives in `duckfs-disk`; this crate keeps the consensus module id and the SDK
//! module implementation, and re-exports the core API.

pub use duckfs_core::*;

#[cfg(feature = "native")]
mod module;

#[cfg(feature = "native")]
pub use module::Files;

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
