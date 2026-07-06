//! the daemon's op-receipt blob store: op payloads staged at submit time and
//! served back over the http blob route. node-local, never consensus state,
//! never in any root. lives in the `blobstore` workspace crate so the files
//! module and forge (which cannot depend on this binary) share the exact type;
//! re-exported here as the daemon's own name for it.

pub use blobstore::{BlobHandle, BlobStore};
