//! deterministic DuckDNS core.
//!
//! This crate owns the reusable, SDK-free state machine and wire surface:
//! canonical `.duck` names, handle ownership, declarative provider records,
//! query shapes, canonical state bytes, and root calculation. It contains no
//! sockets, local target addresses, filesystem access, or host-module queries.
//! The `duckdns` system crate maps authenticated origins plus live identity and
//! membership facts into this core, just as `files` adapts `duckfs-core`.

mod codec;
mod names;
mod registry;
mod wire;

pub use names::{derive_chain_label, node_label, parse_hostname, validate_handle, validate_label};
pub use registry::Registry;
pub use wire::*;
