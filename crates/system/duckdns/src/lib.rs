//! Ducktape system-module adapter for DuckDNS.
//!
//! The deterministic registry and wire grammar live in `duckdns-core`; this
//! crate keeps the consensus module id and SDK glue. It resolves authenticated
//! submit nodes through `identity`, gates mutations/providers through `valset`,
//! and re-exports the core API as the module's public wire surface.

pub use duckdns_core::*;

mod module;
pub use module::DuckDns;
