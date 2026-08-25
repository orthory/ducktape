//! nat-traversal: reflexive-address discovery + UDP hole-punch mediated by an
//! untrusted coordinator. No WireGuard, no consensus — the reachability
//! primitive under the private-cutover epic. The coordinator is rendezvous
//! ONLY (STUN-style reflexive observation + punch brokering): it never carries
//! peer traffic, so no data path ever depends on it. The one deliberate
//! exception is the `relay` lane's SEALED join intro: on a UDP-dead network a
//! joiner rides TCP to the coordinator, which forwards the opaque sealed bytes
//! (it cannot read them) as a single datagram — a bootstrap fallback, never a
//! peer data path.

pub mod advert;
pub mod auth;
pub mod client;
pub mod coordinator;
pub mod relay;
// The simulation arms (`simnat`, `simnet`) are gated: available under test
// cfg or the `simnat` feature, never pulled into a plain non-test,
// non-feature build (e.g. `coordinator-bin`).
#[cfg(any(test, feature = "simnat"))]
pub mod simnat;
#[cfg(any(test, feature = "simnat"))]
pub mod simnet;
pub mod wire;

pub use advert::{
    AdvertBook, AdvertOutcome, REGISTRATION_TTL_SECS, ReflexiveAdvert, SharedAdverts,
};
pub use auth::{
    AuthError, AuthPolicy, Authenticator, COORD_CAP_NS, COORD_CAP_TTL_SECS, COORD_REQ_NS, CoordCap,
    DEFAULT_FRESHNESS_WINDOW_SECS, mint_coord_cap, now_secs, sign_authenticator, verify_request,
};
pub use client::{
    ClientEvent, CoordinatorMetrics, CoordinatorMetricsSnapshot, NatClient, NatSocket, SocketEvent,
    run_coordinator, run_coordinator_with, run_coordinator_workers_with_metrics,
    run_coordinator_workers_with_metrics_using,
};
pub use coordinator::{Coordinator, CoordinatorReplies, CoordinatorReply};
pub use relay::{
    FRAME_READ_TIMEOUT, FrameError, MAX_FRAME_LEN, MAX_RELAY_PAYLOAD, MAX_RELAY_SESSIONS,
    MAX_SESSION_FORWARDS, MAX_SESSIONS_PER_IP, MIN_FORWARD_GAP, REASON_MALFORMED,
    REASON_NOT_AUTHORIZED, REASON_SESSION_LIMIT, REASON_TARGET_UNREGISTERED, RelayConn, RelayFrame,
    RelayIntro, RelayMetrics, RelayMetricsSnapshot, SESSION_TTL, read_frame, run_relay_listener,
    sign_relay_intro, write_frame,
};
#[cfg(any(test, feature = "simnat"))]
pub use simnat::SimNat;
#[cfg(any(test, feature = "simnat"))]
pub use simnet::{SimHandle, SimNetwork, SimSocket};
pub use wire::{AuthRequest, Msg, NodeKey, WireError};
