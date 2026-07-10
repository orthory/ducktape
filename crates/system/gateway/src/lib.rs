//! Global `.duck` gateway routes.
//!
//! The consensus surface is deliberately small: an Identity account signs one
//! monotonic route from its apex or a single service label to a typed upstream
//! target plus an invocation policy. DuckDNS remains the optional
//! human-name-to-AccountId layer. This module stores no node address, local
//! port, browser session, content bytes, or transport state.

mod interface;
mod module;
mod proxy;
mod registry;

pub use interface::*;
pub use module::Gateway;
pub use proxy::*;
pub use registry::Registry;
