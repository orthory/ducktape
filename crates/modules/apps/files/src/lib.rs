//! Ducktape module adapter for duckfs.
//!
//! The deterministic filesystem core lives in `duckfs-core`; native persistence
//! lives in `duckfs-disk`; this crate keeps the consensus module id and the SDK
//! module implementation, and re-exports the core API.

pub use duckfs_core::*;

#[cfg(any(feature = "native", feature = "guest"))]
mod adapter;

#[cfg(feature = "native")]
mod module;

#[cfg(feature = "native")]
pub use module::Files;

// the host-side ODB substrate a wasm files tenant delegates its committed
// surface to (`wasm_host::OdbBacking` over the SAME disk machinery as `Files`).
// native-only: it depends on duckfs-disk + the kernel host, never the pure core.
#[cfg(feature = "native")]
mod backing;

#[cfg(feature = "native")]
pub use backing::FilesOdbBacking;

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
