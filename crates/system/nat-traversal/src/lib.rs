//! nat-traversal: reflexive-address discovery + UDP hole-punch mediated by an
//! untrusted coordinator. No WireGuard, no consensus — the reachability
//! primitive under the private-cutover epic.

pub mod coordinator;
pub mod wire;

pub use coordinator::Coordinator;
pub use wire::{Msg, NodeKey, WireError};
