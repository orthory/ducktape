//! The reachability orchestrator: the driver that takes epoch cutover events
//! from the node and turns them into a live WireGuard mesh — record gossip ->
//! signed advertisements -> `MeshView::verify` -> pairwise tunnel handshakes
//! -> ONE `apply_tunnel_plans` call per epoch through a `WireGuardEffect`.
//!
//! Runtime contract: the node runs [`run`] as the ROOT future of a dedicated
//! plain-tokio runtime on its own OS thread (the same split as the node's
//! app-surface thread), talking to the commonware runner over the two mpsc
//! channels. The future is not required to be `Send` — nothing here may
//! assume `tokio::spawn` onto a shared runtime.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;

use commonware_cryptography::ed25519;
use nat_traversal::NodeKey;
use tokio::sync::mpsc;
use wireguard_upgrade::{Endpoint, MeshVersion, PortPolicy, UpgradeError, ValidatorIdentity};

use crate::keys::KeyError;
use crate::msg::MsgError;

/// Views an advertisement stays valid for past the cutover view that minted
/// it. Generous: a re-advertisement (NAT rebind, key rotation) supersedes by
/// nonce long before expiry matters.
pub const ADVERT_TTL_VIEWS: u64 = 10_000;

/// Views a handshake message stays valid for. Tight relative to
/// [`ADVERT_TTL_VIEWS`]: a handshake is a live conversation, not a standing
/// record.
pub const HANDSHAKE_TTL_VIEWS: u64 = 500;

/// WireGuard persistent keepalive for every mesh peer: NAT mappings on the
/// punched path die in tens of seconds of silence, and a consensus mesh can
/// legitimately idle a data tunnel that long.
pub const KEEPALIVE_SECONDS: u16 = 25;

/// For each unordered member pair exactly ONE side runs the handshake, and
/// both sides agree which from public data alone: the lexicographically
/// lower identity initiates.
pub fn initiates(local: ValidatorIdentity, peer: ValidatorIdentity) -> bool {
    local.0 < peer.0
}

/// Everything the node resolves ONCE at boot and hands the orchestrator.
pub struct ReachabilityConfig {
    /// The chain id — doubles as the advertisement namespace and the ULA
    /// derivation input, exactly as it does for the commonware mesh.
    pub chain_id: String,
    /// The node's ed25519 identity: signs records, advertisements, and
    /// handshake messages. Its public key IS the member identity.
    pub signer: ed25519::PrivateKey,
    /// Where the X25519 keypair lives (beside `identity.key`);
    /// `keys::WireGuardKeypair::load_or_generate` runs against this path.
    pub wireguard_key_file: PathBuf,
    /// The node's own advertised WireGuard UDP endpoint.
    pub wireguard_listen: Endpoint,
    /// The node's own advertised control-mesh endpoint.
    pub control_endpoint: Endpoint,
    /// Rendezvous coordinators (from `Resolved.coordinated`), possibly none:
    /// with an empty list every peer resolves to its advertised endpoint.
    pub coordinators: Vec<SocketAddr>,
    /// The endpoint policy advertisements and handshakes validate against.
    pub port_policy: PortPolicy,
}

/// A valset cutover (or boot) the orchestrator must retarget to.
#[derive(Clone, Debug)]
pub struct MeshEpochEvent {
    pub epoch: u64,
    /// The epoch's consensus members' ed25519 public keys, this node
    /// included. Order is irrelevant — every derived commitment sorts.
    pub members: Vec<ed25519::PublicKey>,
    /// The consensus view at the cutover; the freshness clock for expiries.
    pub current_view: u64,
}

/// Node -> orchestrator.
#[derive(Debug)]
pub enum ReachabilityCommand {
    /// Boot or epoch cutover: (re)build the mesh for this member set. A
    /// retarget SUPERSEDES any epoch still assembling — tear down in-flight
    /// state, remove the previous interface, start over.
    Retarget(MeshEpochEvent),
    /// A reachability-channel message arrived from a mesh peer.
    Deliver {
        from: ed25519::PublicKey,
        bytes: Vec<u8>,
    },
    /// The consensus view advanced (drives expiry checks between cutovers).
    ViewTick(u64),
    /// Drain and exit; the interface is torn down on the way out.
    Shutdown,
}

/// Orchestrator -> node.
#[derive(Debug)]
pub enum ReachabilityEvent {
    /// Send `bytes` to `to` on the reachability channel.
    Send {
        to: ed25519::PublicKey,
        bytes: Vec<u8>,
    },
    /// Every member's signed advertisement verified into one `MeshView`.
    MeshReady { epoch: u64, version: MeshVersion },
    /// The epoch's tunnel config went through the effect: one interface,
    /// `peers` peer relationships.
    TunnelsApplied {
        epoch: u64,
        interface: String,
        peers: usize,
    },
    /// A peer could not be brought up (handshake refused, resolution
    /// failed, effect rejected). The mesh keeps running without it; the
    /// node surfaces the warning.
    PeerFailed {
        peer: ed25519::PublicKey,
        reason: String,
    },
}

/// How a peer's WireGuard endpoint was resolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolution {
    /// Dial the advertised endpoint as-is (public or already-reachable
    /// address; also the no-coordinator dev path).
    Advertised,
    /// Hole-punch succeeded: dial the peer's punched reflexive.
    Punched(SocketAddr),
    /// Punch failed; a coordinator granted a relay session at this address.
    Relayed(SocketAddr),
}

/// Per-peer endpoint resolution, pluggable so the orchestrator's protocol
/// logic tests deterministically without UDP. The real implementation is
/// [`NatResolver`]; tests use [`StaticResolver`].
#[allow(async_fn_in_trait)] // consumed on a single-thread block_on root; no Send bound wanted
pub trait EndpointResolver {
    /// Resolve `peer`'s dialable UDP address given its advertised WireGuard
    /// endpoint. Errors mean the peer stays on its advertised endpoint and a
    /// `PeerFailed` is emitted for observability.
    async fn resolve(
        &mut self,
        peer: NodeKey,
        advertised: SocketAddr,
    ) -> Result<Resolution, String>;
}

/// Test resolver: a fixed map, `Advertised` for anything unlisted.
#[derive(Default)]
pub struct StaticResolver(pub HashMap<NodeKey, Resolution>);

impl EndpointResolver for StaticResolver {
    async fn resolve(
        &mut self,
        peer: NodeKey,
        _advertised: SocketAddr,
    ) -> Result<Resolution, String> {
        Ok(self.0.get(&peer).copied().unwrap_or(Resolution::Advertised))
    }
}

/// The production resolver: drives `nat_traversal::NatClient` against the
/// configured coordinators — STUN reflexive discovery + `register` at boot,
/// then per peer `lookup`/punch-sync/`send_punch_to`/`recv_punch_from`, and
/// `request_relay` after bounded punch failure. With NO coordinators
/// configured every resolution is `Advertised`.
pub struct NatResolver {
    // implementer note: holds the bound NatClient (bind_multi over
    // ReachabilityConfig.coordinators), the local NodeKey, and punch retry
    // bounds. Constructed by `NatResolver::bind`.
    _private: (),
}

impl NatResolver {
    /// Bind the nat client's UDP socket and register with the coordinators.
    /// `key` is this node's identity bytes (`binding::node_key`).
    pub async fn bind(_key: NodeKey, _coordinators: Vec<SocketAddr>) -> std::io::Result<Self> {
        todo!("phase A implementation: wrap NatClient::bind_multi + register")
    }
}

impl EndpointResolver for NatResolver {
    async fn resolve(
        &mut self,
        _peer: NodeKey,
        _advertised: SocketAddr,
    ) -> Result<Resolution, String> {
        todo!("phase A implementation: lookup -> punch -> relay fallback")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ReachabilityError {
    #[error("wireguard keystore: {0}")]
    Key(#[from] KeyError),
    #[error("protocol: {0:?}")]
    Upgrade(UpgradeError),
    #[error("message codec: {0}")]
    Msg(#[from] MsgError),
    #[error("the node dropped the command channel")]
    ChannelClosed,
    #[error("wireguard effect: {0}")]
    Effect(String),
}

impl From<UpgradeError> for ReachabilityError {
    fn from(err: UpgradeError) -> Self {
        Self::Upgrade(err)
    }
}

/// Drive the reachability plane until `Shutdown` (clean exit) or the command
/// channel closes (error). One call outlives every epoch; `Retarget` events
/// move it between epochs.
///
/// # The per-epoch state machine (the implementation contract)
///
/// On `Retarget { epoch, members, current_view }`:
///
/// 1. **Bind.** Derive the epoch's `ActiveValidatorSet` via
///    `binding::active_set(chain_id, epoch, identities)`. Load the WireGuard
///    keypair (once, at startup — not per epoch).
/// 2. **Record gossip.** Build this node's `EndpointRecord` — identity, WG
///    public key, `control_endpoint`, `wireguard_listen`, `expires_at_view =
///    current_view + ADVERT_TTL_VIEWS`, nonce from the per-epoch counter —
///    and `Send` it (as `ReachabilityMsg::Record`) to every OTHER member.
///    Collect members' records; re-send ours on receiving a record from a
///    member we haven't answered this epoch (join-order independence).
/// 3. **Advertise.** Once ALL members' records (ours included) are held:
///    `compute_mesh_version`, sign `EndpointAdvertisement`, `Send` to every
///    other member. Collect signed advertisements the same way.
/// 4. **Verify.** With all advertisements held, `MeshView::verify(...)` at
///    the latest known view. Emit `MeshReady`. A verification failure is an
///    epoch-fatal error event, not a crash: emit `PeerFailed` per offender
///    where attributable and keep serving the previous epoch's tunnels.
/// 5. **Handshakes.** For every peer where `initiates(local, peer)`: resolve
///    the endpoint via the `EndpointResolver`, then run request ->
///    (peer's response) -> ack over `Send`/`Deliver`, nonces from the
///    per-epoch counter, expiries at `+ HANDSHAKE_TTL_VIEWS`, keepalive
///    `KEEPALIVE_SECONDS`. Validate the completed triple with
///    `validate_upgrade_as(Perspective::Initiator, ...)`. As RESPONDER
///    (`!initiates`): on `Request`, sign the matching response (accepting
///    the canonical overlay routes), send it, await the ack, validate with
///    `Perspective::Responder`. Overlay policy: `OverlayPolicy::ula_v6(chain_id)`.
///    One shared `ReplayCache` per epoch.
/// 6. **Apply.** When every peer's plan is validated (or each failure has
///    emitted `PeerFailed`), remove any previous epoch's interface and make
///    the epoch's ONE effect call:
///    `apply_tunnel_plans(effect, binding::interface_name(chain_id),
///    keypair.private_key_base64(), wireguard_listen, &plans, &overrides)`
///    where `overrides` maps each peer resolved `Punched`/`Relayed` to that
///    address. Emit `TunnelsApplied`. Partial meshes apply what validated.
///
/// Nonce discipline: ONE strictly-monotonic counter per epoch for everything
/// this identity signs (record, advertisement, request, response, ack) —
/// replay keys are `(identity, epoch, nonce)`, and `MeshView`'s duplicate
/// rule needs re-advertisements strictly increasing.
///
/// `ViewTick` advances the freshness clock used for expiry checks;
/// `Deliver` from a non-member of the current epoch is dropped with a
/// `PeerFailed` (stale or hostile traffic, never a crash).
pub async fn run<E, R>(
    config: ReachabilityConfig,
    effect: E,
    resolver: R,
    commands: mpsc::Receiver<ReachabilityCommand>,
    events: mpsc::Sender<ReachabilityEvent>,
) -> Result<(), ReachabilityError>
where
    E: wireguard_effect::WireGuardEffect,
    R: EndpointResolver,
{
    let _ = (config, effect, resolver, commands, events);
    todo!("phase A implementation: the per-epoch state machine documented above")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exactly_one_side_of_every_pair_initiates() {
        let low = ValidatorIdentity([1; 32]);
        let high = ValidatorIdentity([2; 32]);
        assert!(initiates(low, high));
        assert!(!initiates(high, low));
        // self-pairs never occur (a node has no tunnel to itself), and the
        // rule is strict either way.
        assert!(!initiates(low, low));
    }
}
