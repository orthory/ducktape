//! The node-driven WireGuard reachability plane: the orchestrator that turns
//! valset cutover events into a live validator↔validator WireGuard mesh on a
//! DEDICATED interface, composing three proven crates the live node did not
//! call until now — `wireguard-upgrade` (the signed advertisement + tunnel
//! handshake protocol), `wireguard-effect` (the interface effect boundary),
//! and `nat-traversal` (STUN/rendezvous/hole-punch).
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
pub mod store;

pub use binding::{
    active_set, admission_root, identity_of, interface_name, node_key, open_port_policy,
    valset_root,
};
pub use keys::{KeyError, WireGuardKeypair};
pub use msg::{MsgError, ReachabilityMsg};
pub use orchestrator::{
    ADVERT_TTL_VIEWS, CoordinatedInviteReply, EndpointResolver, HANDSHAKE_TTL_VIEWS, InstallReply,
    KEEPALIVE_SECONDS, MeshEpochEvent, NatResolver, RENDEZVOUS_KEEPALIVE, ReachabilityCommand,
    ReachabilityConfig, ReachabilityError, ReachabilityEvent, Resolution, StaticResolver,
    initiates, run,
};
pub use store::{MESH_STORE_FORMAT, PersistedMesh, StoreError};
