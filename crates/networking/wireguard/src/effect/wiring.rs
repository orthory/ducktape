use std::collections::{BTreeMap, BTreeSet};
use std::net::SocketAddr;

use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};

use crate::{AllowedIp, TunnelInstallPlan, ValidatorIdentity, X25519PublicKey};

use crate::effect::WireGuardEffect;

/// Reduce validated plans to the plain [`PeerTunnelConfig`] parts —
/// the plan-independent form a caller can merge with peers from OTHER
/// sources (a restored mesh, a standby's signed record) before one
/// [`apply_peer_tunnels`]/[`update_peer_tunnels`] call over the union.
pub fn plan_peer_configs(
    plans: &[TunnelInstallPlan],
    endpoint_overrides: &BTreeMap<ValidatorIdentity, SocketAddr>,
) -> Vec<PeerTunnelConfig> {
    plans
        .iter()
        .map(|plan| PeerTunnelConfig {
            wireguard_public_key: plan.peer_wireguard_public_key(),
            // the override is matched by the plan's peer identity, never by
            // endpoint, so two peers advertising the same address can never
            // cross-match.
            endpoint: endpoint_overrides
                .get(&plan.peer_identity())
                .copied()
                .or_else(|| plan.peer_endpoint().map(|e| e.socket_addr())),
            allowed_ips: plan.allowed_ips().to_vec(),
            keepalive_seconds: plan.keepalive_seconds(),
        })
        .collect()
}

/// One peer relationship expressed as plain parts rather than a validated
/// `TunnelInstallPlan` — everything a WireGuard peer entry needs, already
/// resolved.
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct PeerTunnelConfig {
    pub wireguard_public_key: X25519PublicKey,
    /// `None` for an endpoint-less peer (it advertises no dialable address):
    /// the entry is installed without an endpoint and WireGuard waits for the
    /// peer's own authenticated initiation, then roams to its source.
    #[borsh(schema(with_funcs(
        declaration = "crate::wire_schema::option_socket_addr::declaration",
        definitions = "crate::wire_schema::option_socket_addr::definitions"
    )))]
    pub endpoint: Option<SocketAddr>,
    pub allowed_ips: Vec<AllowedIp>,
    pub keepalive_seconds: Option<u16>,
}

/// The full desired state of the ONE overlay interface, as the effect
/// layer receives it: own private key and listen port, own overlay
/// addresses, and every peer relationship. `WireGuardEffect::apply` is
/// create-or-replace at this level.
#[derive(Clone, PartialEq, Eq)]
pub struct InterfaceConfig {
    pub name: String,
    /// this node's static X25519 private key.
    pub private_key: [u8; 32],
    pub listen_port: u16,
    /// this node's own overlay addresses, deduplicated in first-seen order.
    pub addresses: Vec<AllowedIp>,
    pub peers: Vec<PeerTunnelConfig>,
}

/// `Debug` redacts the private key: an applied config is exactly the kind
/// of value that lands in a log line or a test failure message.
impl std::fmt::Debug for InterfaceConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InterfaceConfig")
            .field("name", &self.name)
            .field("private_key", &"<redacted>")
            .field("listen_port", &self.listen_port)
            .field("addresses", &self.addresses)
            .field("peers", &self.peers)
            .finish()
    }
}

/// The parts-level core both appliers share: ONE interface (own overlay
/// addresses, listen port, private key) carrying every peer relationship.
/// [`plan_peer_configs`] reduces validated plans to these parts; a mesh
/// restored from persisted state (whose plans were validated in a PREVIOUS
/// process life and are re-derived, not re-validated) calls this directly.
pub fn apply_peer_tunnels<E: WireGuardEffect>(
    effect: &mut E,
    ifname: impl Into<String>,
    private_key: [u8; 32],
    listen_port: u16,
    local_interface_ips: &[AllowedIp],
    peers: &[PeerTunnelConfig],
) -> Result<(), E::Error> {
    let config = interface_config(ifname, private_key, listen_port, local_interface_ips, peers);
    if let Err(err) = effect.create_interface() {
        tracing::warn!(
            target: "ducktape::dataplane",
            event = "overlay_interface_refused",
            reason = "create_failed",
            interface = %config.name,
            listen_port,
            error = ?err,
            "the tunnel backend refused to create the overlay interface"
        );
        return Err(err);
    }
    if let Err(err) = effect.apply(&config) {
        // `create_interface` already stood up the interface; don't leave it
        // behind just because this config was rejected (a listen port the
        // backend cannot bind). The `remove_interface` outcome is
        // secondary to the `apply` failure that's actually being reported,
        // so it's intentionally dropped rather than allowed to shadow it.
        let _ = effect.remove_interface();
        tracing::warn!(
            target: "ducktape::dataplane",
            event = "overlay_interface_refused",
            reason = "apply_rejected",
            interface = %config.name,
            listen_port,
            peers = config.peers.len(),
            error = ?err,
            "the tunnel backend refused the interface config — interface torn back down"
        );
        return Err(err);
    }
    // bring-up happens once per interface life; the peer census is the fact an
    // operator needs when a member is unreachable — an endpoint-less peer
    // cannot be dialed, it can only dial in.
    tracing::info!(
        target: "ducktape::dataplane",
        event = "overlay_interface_up",
        interface = %config.name,
        listen_port,
        addresses = config.addresses.len(),
        peers = config.peers.len(),
        endpointless = endpointless(&config.peers),
        "overlay interface configured"
    );
    Ok(())
}

/// Peers installed with no endpoint: WireGuard cannot dial them, it can only
/// wait for their own initiation and roam to its source. The count is the
/// difference between "the mesh is up" and "the mesh is up but half of it can
/// never be reached from here".
fn endpointless(peers: &[PeerTunnelConfig]) -> usize {
    peers.iter().filter(|peer| peer.endpoint.is_none()).count()
}

/// Re-apply the full peer set to an interface that is ALREADY live — the
/// mid-epoch form of [`apply_peer_tunnels`]: no `create_interface`, one
/// `apply` of the complete desired configuration. `WireGuardEffect::apply`
/// is create-or-replace at the config level (the userspace backend replaces
/// the peer table wholesale while keeping live sessions for unchanged
/// peers), so unchanged peers keep their sessions and the delta — an added
/// standby tunnel, a re-advertised endpoint — lands without tearing the
/// interface down.
pub fn update_peer_tunnels<E: WireGuardEffect>(
    effect: &mut E,
    ifname: impl Into<String>,
    private_key: [u8; 32],
    listen_port: u16,
    local_interface_ips: &[AllowedIp],
    peers: &[PeerTunnelConfig],
) -> Result<(), E::Error> {
    let config = interface_config(ifname, private_key, listen_port, local_interface_ips, peers);
    if let Err(err) = effect.apply(&config) {
        // Unlike bring-up there is nothing to unwind: the interface stays live
        // on its PREVIOUS peer set, so the delta this call carried (a new
        // standby tunnel, a re-advertised endpoint) is silently not installed.
        tracing::warn!(
            target: "ducktape::dataplane",
            event = "overlay_peers_refused",
            reason = "apply_rejected",
            interface = %config.name,
            peers = config.peers.len(),
            error = ?err,
            "the tunnel backend refused a peer-set re-apply — the live interface keeps its previous peers"
        );
        return Err(err);
    }
    tracing::debug!(
        target: "ducktape::dataplane",
        event = "overlay_peers_applied",
        interface = %config.name,
        peers = config.peers.len(),
        endpointless = endpointless(&config.peers),
        "re-applied the peer set to the live interface"
    );
    Ok(())
}

/// The shared config assembly: ONE interface (own overlay addresses, listen
/// port, private key) carrying every peer relationship.
fn interface_config(
    ifname: impl Into<String>,
    private_key: [u8; 32],
    listen_port: u16,
    local_interface_ips: &[AllowedIp],
    peers: &[PeerTunnelConfig],
) -> InterfaceConfig {
    // every peer relationship carries the same local side — dedup while
    // preserving first-seen order.
    let mut seen = BTreeSet::new();
    let addresses = local_interface_ips
        .iter()
        .filter(|route| seen.insert(**route))
        .copied()
        .collect();
    InterfaceConfig {
        name: ifname.into(),
        private_key,
        listen_port,
        addresses,
        peers: peers.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;
    use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
    use std::net::{IpAddr, Ipv4Addr};

    fn id(sk: &PrivateKey) -> ValidatorIdentity {
        ValidatorIdentity::try_from(sk.public_key().as_ref()).unwrap()
    }

    fn xkey(byte: u8) -> X25519PublicKey {
        X25519PublicKey([byte; 32])
    }

    fn endpoint(policy: &PortPolicy, addr: [u8; 4], port: u16, transport: Transport) -> Endpoint {
        Endpoint::new(IpAddr::V4(Ipv4Addr::from(addr)), port, transport, policy).unwrap()
    }

    /// records + verified view for `parties` (`(signer, wireguard key, host
    /// octet)`): control at 1.1.1.x:443, wireguard at 8.8.8.x:51820.
    fn mesh_view(parties: &[(&PrivateKey, X25519PublicKey, u8)]) -> MeshView {
        let policy = PortPolicy::production();
        let set = ActiveValidatorSet::new(
            "ducktape-wiring",
            1,
            Root([1u8; 32]),
            AdmissionRoot([2u8; 32]),
            parties.iter().map(|(sk, _, _)| id(sk)).collect(),
        )
        .unwrap();
        let records: Vec<EndpointRecord> = parties
            .iter()
            .map(|(sk, wg, octet)| EndpointRecord {
                namespace: set.namespace.clone(),
                epoch: set.epoch,
                valset_root: set.valset_root,
                admission_root: set.admission_root,
                validator_identity: id(sk),
                wireguard_public_key: *wg,
                control_endpoint: endpoint(&policy, [1, 1, 1, *octet], 443, Transport::Tcp),
                wireguard_endpoint: Some(endpoint(
                    &policy,
                    [8, 8, 8, *octet],
                    51820,
                    Transport::Udp,
                )),
                nonce: 1,
            })
            .collect();
        let mesh_version = compute_mesh_version(&records).unwrap();
        let ads = parties
            .iter()
            .zip(records)
            .map(|((sk, _, _), rec)| EndpointAdvertisement::sign(rec, mesh_version, sk))
            .collect();
        MeshView::verify(set, ads, &policy).unwrap()
    }

    /// sign the full initiator->responder conversation against `view` and
    /// validate it from the initiator's perspective. `TunnelInstallPlan` has
    /// no public constructor by design (only `validate_upgrade_as` produces
    /// one), so this fixture runs the real signed handshake exactly like the
    /// crate's own e2e tests do.
    /// `req_nonce`/`ack_nonce` keep an initiator's REPEATED handshakes fresh
    /// in a shared `ReplayCache` (replay state is keyed by identity+nonce).
    #[allow(
        clippy::too_many_arguments,
        reason = "the signed-handshake fixture keeps both peers and nonces explicit"
    )]
    fn plan_between(
        initiator: &PrivateKey,
        responder: &PrivateKey,
        initiator_key: X25519PublicKey,
        responder_key: X25519PublicKey,
        view: &MeshView,
        policy: &PortPolicy,
        overlay: &OverlayPolicy,
        replay: &mut ReplayCache,
        req_nonce: u64,
        ack_nonce: u64,
    ) -> TunnelInstallPlan {
        let set = &view.active_set;
        let request = TunnelUpgradeRequest::sign(
            TunnelUpgradeRequestFields {
                namespace: set.namespace.clone(),
                epoch: set.epoch,
                valset_root: set.valset_root,
                admission_root: set.admission_root,
                mesh_version: view.mesh_version,
                initiator_identity: id(initiator),
                responder_identity: id(responder),
                initiator_wireguard_public_key: initiator_key,
                initiator_wireguard_endpoint: view
                    .record(id(initiator))
                    .unwrap()
                    .wireguard_endpoint,
                requested_allowed_ips: overlay.allowed_ips_for(view, id(responder)).unwrap(),
                port_policy_hash: policy.hash(),
                expires_at_view: 40,
                nonce: req_nonce,
            },
            initiator,
        );
        let response = TunnelUpgradeResponse::sign(
            TunnelUpgradeResponseFields {
                request_hash: request.hash(),
                namespace: set.namespace.clone(),
                epoch: set.epoch,
                valset_root: set.valset_root,
                admission_root: set.admission_root,
                mesh_version: view.mesh_version,
                responder_identity: id(responder),
                initiator_identity: id(initiator),
                responder_wireguard_public_key: responder_key,
                responder_wireguard_endpoint: view
                    .record(id(responder))
                    .unwrap()
                    .wireguard_endpoint,
                accepted_allowed_ips: overlay.allowed_ips_for(view, id(initiator)).unwrap(),
                keepalive_seconds: Some(25),
                expires_at_view: 40,
                nonce: 1,
            },
            responder,
        );
        let ack = TunnelUpgradeAck::sign(
            TunnelUpgradeAckFields {
                request_hash: request.hash(),
                response_hash: response.hash(),
                namespace: set.namespace.clone(),
                epoch: set.epoch,
                valset_root: set.valset_root,
                admission_root: set.admission_root,
                mesh_version: view.mesh_version,
                initiator_identity: id(initiator),
                responder_identity: id(responder),
                installed_at_view: 11,
                expires_at_view: 40,
                nonce: ack_nonce,
            },
            initiator,
        );
        validate_upgrade_as(
            Perspective::Initiator,
            view,
            policy,
            overlay,
            12,
            &request,
            &response,
            &ack,
            replay,
        )
        .unwrap()
    }

    /// a minimal two-validator handshake, direct (no relay), yielding the
    /// INITIATOR's (a's) validated install plan and a's own listen port —
    /// everything `apply_peer_tunnels` needs.
    fn two_party_plan() -> (TunnelInstallPlan, u16) {
        let a = PrivateKey::from_seed(1);
        let b = PrivateKey::from_seed(2);
        let policy = PortPolicy::production();
        let overlay = OverlayPolicy::ula_v6("ducktape-wiring");
        let view = mesh_view(&[(&a, xkey(0x0a), 10), (&b, xkey(0x0b), 20)]);
        let mut replay = ReplayCache::default();
        let plan = plan_between(
            &a,
            &b,
            xkey(0x0a),
            xkey(0x0b),
            &view,
            &policy,
            &overlay,
            &mut replay,
            1,
            2,
        );
        let listen = view
            .record(id(&a))
            .unwrap()
            .wireguard_endpoint
            .unwrap()
            .port;
        (plan, listen)
    }

    /// the production two-step for one plan: reduce it to parts, then ONE
    /// `apply_peer_tunnels` — exactly the composition the orchestrator runs.
    fn apply_plan(
        fake: &mut crate::effect::FakeWireGuardEffect,
        listen: u16,
        plan: &TunnelInstallPlan,
        override_addr: Option<SocketAddr>,
    ) -> Result<(), crate::effect::FakeWireGuardEffectError> {
        let overrides = override_addr
            .map(|addr| BTreeMap::from([(plan.peer_identity(), addr)]))
            .unwrap_or_default();
        let peers = plan_peer_configs(std::slice::from_ref(plan), &overrides);
        apply_peer_tunnels(
            fake,
            "ducktape-wg0",
            [7u8; 32],
            listen,
            plan.local_interface_ips(),
            &peers,
        )
    }

    /// a's validated plans toward BOTH b and c — the full-mesh shape: one
    /// interface, two peer relationships, one shared replay cache.
    fn three_party_plans() -> (TunnelInstallPlan, TunnelInstallPlan, u16) {
        let a = PrivateKey::from_seed(1);
        let b = PrivateKey::from_seed(2);
        let c = PrivateKey::from_seed(3);
        let policy = PortPolicy::production();
        let overlay = OverlayPolicy::ula_v6("ducktape-wiring");
        let view = mesh_view(&[
            (&a, xkey(0x0a), 10),
            (&b, xkey(0x0b), 20),
            (&c, xkey(0x0c), 30),
        ]);
        let mut replay = ReplayCache::default();
        let plan_ab = plan_between(
            &a,
            &b,
            xkey(0x0a),
            xkey(0x0b),
            &view,
            &policy,
            &overlay,
            &mut replay,
            1,
            2,
        );
        let plan_ac = plan_between(
            &a,
            &c,
            xkey(0x0a),
            xkey(0x0c),
            &view,
            &policy,
            &overlay,
            &mut replay,
            3,
            4,
        );
        let listen = view
            .record(id(&a))
            .unwrap()
            .wireguard_endpoint
            .unwrap()
            .port;
        (plan_ab, plan_ac, listen)
    }

    #[test]
    fn applies_plan_with_punch_resolved_peer_endpoint() {
        let (plan, listen) = two_party_plan();
        let mut fake = crate::effect::FakeWireGuardEffect::default();
        let override_addr: SocketAddr = "203.0.113.9:51820".parse().unwrap();

        apply_plan(&mut fake, listen, &plan, Some(override_addr)).unwrap();

        assert_eq!(fake.create_calls, 1);
        assert_eq!(fake.applied.len(), 1);
        let applied = &fake.applied[0];
        // The interface's own addresses must come from the plan's local
        // side (`local_interface_ips`), not the peer's allowed IPs — mixing
        // these up would install the peer's /32 as this host's interface
        // address and silently break the tunnel while the apply still
        // reports success.
        assert_eq!(applied.addresses, plan.local_interface_ips().to_vec());
        assert_eq!(applied.peers.len(), 1);
        let peer = &applied.peers[0];
        assert_eq!(peer.wireguard_public_key, plan.peer_wireguard_public_key());
        assert_eq!(peer.endpoint, Some(override_addr));
        assert_eq!(peer.allowed_ips, plan.allowed_ips().to_vec());
    }

    #[test]
    fn removes_the_interface_it_just_created_when_apply_is_rejected() {
        // Mirrors a real run where `create_interface` succeeds but the
        // backend then rejects the config (a listen port it cannot bind).
        // `apply_peer_tunnels` must not leave that interface behind.
        let (plan, listen) = two_party_plan();
        let mut fake = crate::effect::FakeWireGuardEffect {
            reject_next_apply: true,
            ..Default::default()
        };

        let err = apply_plan(&mut fake, listen, &plan, None).unwrap_err();

        assert_eq!(err, crate::effect::FakeWireGuardEffectError::Rejected);
        assert_eq!(fake.create_calls, 1);
        assert_eq!(
            fake.remove_calls, 1,
            "apply_peer_tunnels must tear down the interface it created when apply fails"
        );
        assert!(fake.applied.is_empty());
    }

    #[test]
    fn falls_back_to_the_plans_own_endpoint_when_no_override_is_given() {
        let (plan, listen) = two_party_plan();
        let mut fake = crate::effect::FakeWireGuardEffect::default();

        apply_plan(&mut fake, listen, &plan, None).unwrap();

        assert_eq!(
            fake.applied[0].peers[0].endpoint,
            plan.peer_endpoint().map(|e| e.socket_addr())
        );
    }

    #[test]
    fn update_reconfigures_the_live_interface_without_recreating_it() {
        // The mid-epoch shape: an interface applied once (create + apply),
        // then a peer set change — a standby tunnel joining the mesh —
        // re-applied through `update_peer_tunnels`. One create, two applied
        // configs, no teardown in between.
        let (plan_ab, plan_ac, listen) = three_party_plans();
        let mut fake = crate::effect::FakeWireGuardEffect::default();
        let base = plan_peer_configs(std::slice::from_ref(&plan_ab), &BTreeMap::new());
        let local = plan_ab.local_interface_ips().to_vec();

        apply_peer_tunnels(&mut fake, "ducktape-wg0", [7u8; 32], listen, &local, &base).unwrap();

        let grown: Vec<PeerTunnelConfig> = base
            .iter()
            .cloned()
            .chain(plan_peer_configs(
                std::slice::from_ref(&plan_ac),
                &BTreeMap::new(),
            ))
            .collect();
        update_peer_tunnels(&mut fake, "ducktape-wg0", [7u8; 32], listen, &local, &grown).unwrap();

        assert_eq!(
            fake.create_calls, 1,
            "the live interface is never re-created"
        );
        assert_eq!(fake.remove_calls, 0, "and never torn down");
        assert_eq!(fake.applied.len(), 2);
        assert_eq!(fake.applied[0].peers.len(), 1);
        assert_eq!(
            fake.applied[1].peers.len(),
            2,
            "the update carries the grown set"
        );
    }

    #[test]
    fn update_before_any_apply_is_rejected() {
        // `update_peer_tunnels` is strictly the live-interface form — calling
        // it with no interface up must fail exactly like the real UAPI
        // socket being absent, never silently record a config.
        let (plan, listen) = two_party_plan();
        let mut fake = crate::effect::FakeWireGuardEffect::default();
        let peers = plan_peer_configs(std::slice::from_ref(&plan), &BTreeMap::new());

        let err = update_peer_tunnels(
            &mut fake,
            "ducktape-wg0",
            [7u8; 32],
            listen,
            plan.local_interface_ips(),
            &peers,
        )
        .unwrap_err();

        assert_eq!(err, crate::effect::FakeWireGuardEffectError::NotCreated);
        assert!(fake.applied.is_empty());
    }

    #[test]
    fn plan_peer_configs_matches_the_applied_reduction() {
        // the `plan_peer_configs` + `apply_peer_tunnels` two-step and a caller composing over
        // `plan_peer_configs` must produce the same peer entries — the
        // orchestrator merges these parts with record-derived peers, and a
        // drift here would mean the merged apply diverges from the plan-only
        // apply.
        let (plan_ab, plan_ac, _listen) = three_party_plans();
        let punched: SocketAddr = "203.0.113.9:40001".parse().unwrap();
        let overrides = BTreeMap::from([(plan_ac.peer_identity(), punched)]);

        let parts = plan_peer_configs(&[plan_ab.clone(), plan_ac.clone()], &overrides);

        assert_eq!(parts.len(), 2);
        assert_eq!(
            parts[0].wireguard_public_key,
            plan_ab.peer_wireguard_public_key()
        );
        assert_eq!(
            parts[0].endpoint,
            plan_ab.peer_endpoint().map(|e| e.socket_addr())
        );
        assert_eq!(parts[0].allowed_ips, plan_ab.allowed_ips().to_vec());
        assert_eq!(
            parts[1].wireguard_public_key,
            plan_ac.peer_wireguard_public_key()
        );
        assert_eq!(
            parts[1].endpoint,
            Some(punched),
            "override lands by identity"
        );
    }

    #[test]
    fn applies_all_plans_on_one_interface_with_per_peer_overrides() {
        let (plan_ab, plan_ac, listen) = three_party_plans();
        let mut fake = crate::effect::FakeWireGuardEffect::default();
        let punched: SocketAddr = "203.0.113.9:40001".parse().unwrap();
        // only c resolved through the nat client; b stays on its advert.
        let overrides = BTreeMap::from([(plan_ac.peer_identity(), punched)]);

        let peers = plan_peer_configs(&[plan_ab.clone(), plan_ac.clone()], &overrides);
        apply_peer_tunnels(
            &mut fake,
            "ducktape-wg0",
            [7u8; 32],
            listen,
            plan_ab.local_interface_ips(),
            &peers,
        )
        .unwrap();

        assert_eq!(fake.create_calls, 1, "one interface for the whole mesh");
        assert_eq!(fake.applied.len(), 1);
        let applied = &fake.applied[0];
        // a's own overlay address appears ONCE — every plan carries the same
        // local side and `from_plan` dedupes it.
        assert_eq!(applied.addresses, plan_ab.local_interface_ips().to_vec());
        assert_eq!(applied.peers.len(), 2);
        assert_eq!(
            applied.peers[0].wireguard_public_key,
            plan_ab.peer_wireguard_public_key()
        );
        assert_eq!(
            applied.peers[0].endpoint,
            plan_ab.peer_endpoint().map(|e| e.socket_addr()),
            "peer absent from the override map keeps its advertised endpoint"
        );
        assert_eq!(
            applied.peers[1].wireguard_public_key,
            plan_ac.peer_wireguard_public_key()
        );
        assert_eq!(applied.peers[1].endpoint, Some(punched));
    }
}
