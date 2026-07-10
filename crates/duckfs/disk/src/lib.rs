//! native disk persistence for duckfs.

mod disk;
mod scratch;

pub use disk::{DiskRefs, DiskStore};
pub use scratch::SyncScratch;
