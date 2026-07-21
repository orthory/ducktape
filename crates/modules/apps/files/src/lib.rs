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

// the wasm-guest port. compiled for the `guest` feature (the wasm build) and
// under `test` (so the native suite can drive the pure `dispatch` seam against
// an in-memory odb); ABSENT under the bare `--no-default-features` wasm-
// readiness gate, keeping sdk/guest-adapter out of the pure core.
#[cfg(any(feature = "guest", test))]
mod guest;

#[cfg(feature = "guest")]
pub use guest::FilesGuest;

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
