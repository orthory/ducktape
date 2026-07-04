//! nat-traversal: reflexive-address discovery + UDP hole-punch mediated by an
//! untrusted coordinator. No WireGuard, no consensus — the reachability
//! primitive under the private-cutover epic.

pub mod client;
pub mod coordinator;
// `punch` depends on `simnat::SimNat` directly in its (non-test) API, so it is
// gated identically: available under test cfg or the `simnat` feature, never
// pulled into a plain non-test, non-feature build (e.g. `coordinator-bin`).
#[cfg(any(test, feature = "simnat"))]
pub mod punch;
pub mod relay;
#[cfg(any(test, feature = "simnat"))]
pub mod simnat;
pub mod wire;

pub use client::{NatClient, run_coordinator, run_relay_pair};
pub use coordinator::{Coordinator, Side};
pub use relay::{Forward, RelaySplice};
#[cfg(any(test, feature = "simnat"))]
pub use punch::{
    FallbackOutcome, PunchError, PunchPlan, RelayFallbackProof, drive_simulated,
    drive_with_relay_fallback,
};
#[cfg(any(test, feature = "simnat"))]
pub use simnat::SimNat;
pub use wire::{Msg, NodeKey, WireError};
