//! The node-driven WireGuard reachability plane: the orchestrator that turns
//! valset cutover events into a live validator↔validator WireGuard mesh on a
//! DEDICATED interface, composing proven crates the live node did not call
//! until now — `wireguard` (the signed advertisement + tunnel handshake
//! protocol plus the interface effect boundary) and `nat-traversal`
//! (STUN/rendezvous/hole-punch).
//!
//! Design anchors (docs/deploy/private-cutover-integration-gap.md §2/§3):
//! - The control mesh (commonware TCP) is UNTOUCHED — this plane composes
//!   orthogonally, exchanging its messages over one dedicated mesh channel
//!   and driving only the WireGuard DATA tunnel.
//! - Coexistence with a personal Tailscale is load-bearing: a chain-scoped
//!   `dt-*` interface, an fd::/48 ULA overlay derived from the chain id, and
//!   per-peer AllowedIPs of exactly one /128 — never a default route, never
//!   100.64.0.0/10.
//! - Everything derives from public inputs: overlay addresses from
//!   `(chain_id, identity)`, epoch bindings from `(chain_id, epoch,
//!   members)` — no allocator, no coordination, no consensus-state change.

pub mod binding;
pub mod keys;
pub mod msg;
pub mod orchestrator;
pub mod rendezvous;
pub mod seal;
pub mod store;

// the crate-root surface is exactly what consumers reach for; everything
// else stays addressable through its module (`binding::`, `msg::`, …).
pub use binding::{active_set, identity_of, node_key, open_port_policy};
pub use keys::WireGuardKeypair;
pub use msg::ReachabilityMsg;
pub use orchestrator::{
    CoordinatedInviteReply, InstallReply, MeshEpochEvent, ReachabilityCommand, ReachabilityConfig,
    ReachabilityError, ReachabilityEvent, run,
};
pub use rendezvous::{
    EndpointResolver, NatResolver, RENDEZVOUS_KEEPALIVE, RendezvousStatus, Resolution,
    StaticResolver,
};
pub use seal::seal;
pub use store::PersistedMesh;
