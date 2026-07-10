//! deterministic DuckDNS core.
//!
//! This crate owns the reusable, SDK-free state machine and wire surface:
//! canonical `.duck` account names, handle ownership, query shapes, canonical
//! state bytes, and root calculation. It contains no nodes, services, sockets,
//! filesystem access, or host-module queries. The `duckdns` system crate maps
//! authenticated origins plus Identity authority into this core.

mod codec;
mod names;
mod registry;
mod wire;

pub use names::{parse_hostname, validate_handle};
pub use registry::Registry;
pub use wire::*;
