//! Node-side DuckDNS integration.
//!
//! This crate is deliberately separate from both the replicated `duckdns`
//! module and the `duckdnsd` device helper. It owns the node-local publication
//! config, state-driven announcement pump, authenticated web overlay, and the
//! narrow HTTP ingress that queries the actor. `bin/node` only composes these
//! pieces with its lifecycle and transport handles.

mod announcer;
pub mod ingress;
pub mod plane;
mod site;

pub use announcer::{Announcer, ResidentAnnouncer};
