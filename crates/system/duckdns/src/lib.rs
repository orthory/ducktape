//! Ducktape system-module adapter for the `.duck` account naming service.
//!
//! The deterministic registry and wire grammar live in `duckdns-core`; this
//! crate derives AccountId from authenticated submit nodes through `identity`,
//! gates namespace mutations through `valset`, and re-exports the core API.

pub use duckdns_core::*;

mod module;
pub use module::DuckDns;
