//! Node-side DuckDNS discovery integration.
//!
//! The replicated module owns names and provider eligibility. This crate owns
//! only the state-driven declaration pump; endpoint discovery and byte
//! transport remain in reachability and data-plane respectively.

mod announcer;

pub use announcer::{Announcer, ResidentAnnouncer};
