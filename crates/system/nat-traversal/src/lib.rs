//! nat-traversal: reflexive-address discovery + UDP hole-punch mediated by an
//! untrusted coordinator. No WireGuard, no consensus — the reachability
//! primitive under the private-cutover epic. The coordinator is rendezvous
//! ONLY (STUN-style reflexive observation + punch brokering): it never carries
//! peer traffic, so no data path ever depends on it.

pub mod advert;
pub mod auth;
pub mod client;
pub mod coordinator;
// `punch` depends on `simnat::SimNat` directly in its (non-test) API, so it is
// gated identically: available under test cfg or the `simnat` feature, never
// pulled into a plain non-test, non-feature build (e.g. `coordinator-bin`).
#[cfg(any(test, feature = "simnat"))]
pub mod punch;
#[cfg(any(test, feature = "simnat"))]
pub mod simnat;
pub mod wire;

pub use advert::{AdvertBook, AdvertOutcome, ReflexiveAdvert};
pub use auth::{
    mint_coord_cap, now_secs, sign_authenticator, verify_request, AuthError, AuthPolicy,
    Authenticator, CoordCap, COORD_CAP_NS, COORD_REQ_NS, DEFAULT_FRESHNESS_WINDOW_SECS,
};
pub use client::{NatClient, run_coordinator};
pub use coordinator::Coordinator;
#[cfg(any(test, feature = "simnat"))]
pub use punch::{PunchError, PunchPlan, RebindProof, drive_rebind_reconnect, drive_simulated};
#[cfg(any(test, feature = "simnat"))]
pub use simnat::SimNat;
pub use wire::{AuthRequest, Msg, NodeKey, WireError};
