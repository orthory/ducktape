//! a thin key→value directory module.
//!
//! the wire surface (`interface`) is dependency-light and always compiles —
//! `default-features = false` is the dependency shape the wasm guest
//! (the `directory` guest port) takes, so the wire types stay single-sourced. the native
//! `sdk::Module` implementation lives behind the `native` feature (the
//! `files`/`duckfs-core` split, applied here).

// the wire surface: this module's shared types, flattened at the crate root.
mod interface;
pub use interface::*;

#[cfg(feature = "native")]
mod module;

#[cfg(feature = "native")]
pub use module::Directory;

// the wasm-guest port: the dispatch shell that adapts this module to the
// ducktape:module world. compiled only by the guest-builder's synthesized
// wasm32 cdylib workspace (feature `guest`), never by the native build.
#[cfg(feature = "guest")]
mod guest;
