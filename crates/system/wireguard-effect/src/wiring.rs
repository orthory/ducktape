use std::collections::{BTreeMap, BTreeSet};
use std::net::SocketAddr;

use defguard_wireguard_rs::{key::Key, net::IpAddrMask, peer::Peer, InterfaceConfiguration};
use wireguard_upgrade::{AllowedIp, Endpoint, TunnelInstallPlan, ValidatorIdentity, X25519PublicKey};

use crate::WireGuardEffect;

/// Apply a validated `TunnelInstallPlan` through a `WireGuardEffect`,
/// bringing up (or replacing) the local WireGuard interface for this one
/// peer relationship.
///
/// `peer_endpoint_override`, when set, replaces the plan's statically
/// advertised `peer_endpoint` with a different address before applying —
/// this is how a punched path gets wired in without touching
/// `wireguard-upgrade`'s validated plan: the caller passes the hole-punch's
/// resolved reflexive address
/// (`nat_traversal::punch::PunchPlan::peer_reflexive` in the simulated rig;
/// a real `NatClient` observation in production). `None` uses the plan's
/// own advertised endpoint unchanged (the direct, no-NAT-surprises case).
pub fn apply_tunnel_plan<E: WireGuardEffect>(
    effect: &mut E,
    ifname: impl Into<String>,
    private_key_base64: impl Into<String>,
    listen_endpoint: Endpoint,
    plan: &TunnelInstallPlan,
    peer_endpoint_override: Option<SocketAddr>,
) -> Result<(), E::Error> {
    let overrides = peer_endpoint_override
        .map(|addr| BTreeMap::from([(plan.peer_identity(), addr)]))
        .unwrap_or_default();
    apply_tunnel_plans(
        effect,
        ifname,
        private_key_base64,
        listen_endpoint,
        std::slice::from_ref(plan),
        &overrides,
    )
}

/// The full-mesh form of [`apply_tunnel_plan`]: ONE interface carrying every
/// validated peer relationship, with per-peer endpoint overrides — each peer
/// resolves independently (this one punched, that one direct), so the
/// override is keyed by the peer's identity rather than applied
/// interface-wide. A peer absent from `endpoint_overrides` keeps its plan's
/// advertised endpoint.
pub fn apply_tunnel_plans<E: WireGuardEffect>(
    effect: &mut E,
    ifname: impl Into<String>,
    private_key_base64: impl Into<String>,
    listen_endpoint: Endpoint,
    plans: &[TunnelInstallPlan],
    endpoint_overrides: &BTreeMap<ValidatorIdentity, SocketAddr>,
) -> Result<(), E::Error> {
    let local_interface_ips: Vec<AllowedIp> = plans
        .iter()
        .flat_map(|plan| plan.local_interface_ips().iter().copied())
        .collect();
    let peers = plan_peer_configs(plans, endpoint_overrides);
    apply_peer_tunnels(
        effect,
        ifname,
        private_key_base64,
        listen_endpoint,
        &local_interface_ips,
        &peers,
    )
}

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
                .unwrap_or_else(|| plan.peer_endpoint().socket_addr()),
            allowed_ips: plan.allowed_ips().to_vec(),
            keepalive_seconds: plan.keepalive_seconds(),
        })
        .collect()
}

/// One peer relationship expressed as plain parts rather than a validated
/// `TunnelInstallPlan` — everything a WireGuard peer entry needs, already
/// resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeerTunnelConfig {
    pub wireguard_public_key: X25519PublicKey,
    pub endpoint: SocketAddr,
    pub allowed_ips: Vec<AllowedIp>,
    pub keepalive_seconds: Option<u16>,
}

/// The parts-level core both appliers share: ONE interface (own overlay
/// addresses, listen port, private key) carrying every peer relationship.
/// [`apply_tunnel_plans`] reduces validated plans to these parts; a mesh
/// restored from persisted state (whose plans were validated in a PREVIOUS
/// process life and are re-derived, not re-validated) calls this directly.
pub fn apply_peer_tunnels<E: WireGuardEffect>(
    effect: &mut E,
    ifname: impl Into<String>,
    private_key_base64: impl Into<String>,
    listen_endpoint: Endpoint,
    local_interface_ips: &[AllowedIp],
    peers: &[PeerTunnelConfig],
) -> Result<(), E::Error> {
    let config = interface_config(
        ifname,
        private_key_base64,
        listen_endpoint,
        local_interface_ips,
        peers,
    );
    effect.create_interface()?;
    if let Err(err) = effect.apply(&config) {
        // `create_interface` already stood up the interface (a real socket
        // at `/var/run/wireguard/<ifname>.sock` on the Defguard path); don't
        // leave it behind just because this config was rejected (e.g. a
        // malformed private key). The `remove_interface` outcome is
        // secondary to the `apply` failure that's actually being reported,
        // so it's intentionally dropped rather than allowed to shadow it.
        let _ = effect.remove_interface();
        return Err(err);
    }
    Ok(())
}

/// Re-apply the full peer set to an interface that is ALREADY live — the
/// mid-epoch form of [`apply_peer_tunnels`]: no `create_interface`, one
/// `apply` of the complete desired configuration. `WireGuardEffect::apply`
/// is create-or-replace at the config level (the Defguard userspace path
/// writes `replace_peers=true`, re-assigns addresses with netlink replace
/// semantics, and tolerates already-present peer routes), so unchanged
/// peers keep their sessions and the delta — an added standby tunnel, a
/// re-advertised endpoint — lands without tearing the interface down.
pub fn update_peer_tunnels<E: WireGuardEffect>(
    effect: &mut E,
    ifname: impl Into<String>,
    private_key_base64: impl Into<String>,
    listen_endpoint: Endpoint,
    local_interface_ips: &[AllowedIp],
    peers: &[PeerTunnelConfig],
) -> Result<(), E::Error> {
    let config = interface_config(
        ifname,
        private_key_base64,
        listen_endpoint,
        local_interface_ips,
        peers,
    );
    effect.apply(&config)
}

/// The shared config assembly: ONE interface (own overlay addresses, listen
/// port, private key) carrying every peer relationship.
fn interface_config(
    ifname: impl Into<String>,
    private_key_base64: impl Into<String>,
    listen_endpoint: Endpoint,
    local_interface_ips: &[AllowedIp],
    peers: &[PeerTunnelConfig],
) -> InterfaceConfiguration {
    // every peer relationship carries the same local side — dedup while
    // preserving first-seen order.
    let mut seen = BTreeSet::new();
    let addresses: Vec<IpAddrMask> = local_interface_ips
        .iter()
        .filter(|route| seen.insert((route.addr, route.cidr)))
        .map(|route| IpAddrMask::new(route.addr, route.cidr))
        .collect();
    let peers = peers
        .iter()
        .map(|cfg| {
            let mut peer = Peer::new(Key::new(cfg.wireguard_public_key.0));
            peer.endpoint = Some(cfg.endpoint);
            peer.persistent_keepalive_interval = cfg.keepalive_seconds;
            peer.set_allowed_ips(
                cfg.allowed_ips
                    .iter()
                    .map(|route| IpAddrMask::new(route.addr, route.cidr))
                    .collect(),
            );
            peer
        })
        .collect();
    InterfaceConfiguration {
        name: ifname.into(),
        prvkey: private_key_base64.into(),
        addresses,
        port: listen_endpoint.port,
        peers,
        mtu: None,
        fwmark: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
    use defguard_wireguard_rs::net::IpAddrMask;
    use std::net::{IpAddr, Ipv4Addr};
    use wireguard_upgrade::*;

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
                wireguard_endpoint: endpoint(&policy, [8, 8, 8, *octet], 51820, Transport::Udp),
                capabilities: vec![],
                expires_at_view: 50,
                nonce: 1,
            })
            .collect();
        let mesh_version = compute_mesh_version(&records).unwrap();
        let ads = parties
            .iter()
            .zip(records)
            .map(|((sk, _, _), rec)| EndpointAdvertisement::sign(rec, mesh_version, sk))
            .collect();
        MeshView::verify(set, ads, &policy, 10).unwrap()
    }

    /// sign the full initiator->responder conversation against `view` and
    /// validate it from the initiator's perspective. `TunnelInstallPlan` has
    /// no public constructor by design (only `validate_upgrade`/
    /// `validate_upgrade_as` produce one), so this fixture runs the real
    /// signed handshake exactly like `wireguard-upgrade`'s own e2e tests do.
    /// `req_nonce`/`ack_nonce` keep an initiator's REPEATED handshakes fresh
    /// in a shared `ReplayCache` (replay state is keyed by identity+nonce).
    #[allow(clippy::too_many_arguments)]
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
                relay_candidates: vec![],
                direct_dial_failure: None,
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
        validate_upgrade(
            view, policy, overlay, 12, &request, &response, &ack, replay,
        )
        .unwrap()
    }

    /// a minimal two-validator handshake, direct (no relay), yielding the
    /// INITIATOR's (a's) validated install plan and a's own listen endpoint —
    /// everything `apply_tunnel_plan` needs.
    fn two_party_plan() -> (TunnelInstallPlan, Endpoint) {
        let a = PrivateKey::from_seed(1);
        let b = PrivateKey::from_seed(2);
        let policy = PortPolicy::production();
        let overlay = OverlayPolicy::default_v4();
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
        let listen = view.record(id(&a)).unwrap().wireguard_endpoint;
        (plan, listen)
    }

    /// a's validated plans toward BOTH b and c — the full-mesh shape: one
    /// interface, two peer relationships, one shared replay cache.
    fn three_party_plans() -> (TunnelInstallPlan, TunnelInstallPlan, Endpoint) {
        let a = PrivateKey::from_seed(1);
        let b = PrivateKey::from_seed(2);
        let c = PrivateKey::from_seed(3);
        let policy = PortPolicy::production();
        let overlay = OverlayPolicy::default_v4();
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
        let listen = view.record(id(&a)).unwrap().wireguard_endpoint;
        (plan_ab, plan_ac, listen)
    }

    #[test]
    fn applies_plan_with_punch_resolved_peer_endpoint() {
        let (plan, listen) = two_party_plan();
        let mut fake = crate::FakeWireGuardEffect::default();
        let override_addr: SocketAddr = "203.0.113.9:51820".parse().unwrap();

        apply_tunnel_plan(
            &mut fake,
            "ducktape-wg0",
            "cHJpdmF0ZS1rZXktYmFzZTY0LXBsYWNlaG9sZGVy",
            listen,
            &plan,
            Some(override_addr),
        )
        .unwrap();

        assert_eq!(fake.create_calls, 1);
        assert_eq!(fake.applied.len(), 1);
        let applied = &fake.applied[0];
        // The interface's own addresses must come from the plan's local
        // side (`local_interface_ips`), not the peer's allowed IPs — mixing
        // these up would install the peer's /32 as this host's interface
        // address and silently break the tunnel while `apply_tunnel_plan`
        // still reports success.
        assert_eq!(
            applied.addresses,
            plan.local_interface_ips()
                .iter()
                .map(|r| IpAddrMask::new(r.addr, r.cidr))
                .collect::<Vec<_>>()
        );
        assert_eq!(applied.peers.len(), 1);
        let peer = &applied.peers[0];
        assert_eq!(peer.public_key.as_array(), plan.peer_wireguard_public_key().0);
        assert_eq!(peer.endpoint, Some(override_addr));
        assert_eq!(
            peer.allowed_ips,
            plan.allowed_ips()
                .iter()
                .map(|r| IpAddrMask::new(r.addr, r.cidr))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn removes_the_interface_it_just_created_when_apply_is_rejected() {
        // Mirrors a real `DefguardWireGuardEffect` run where
        // `create_interface` succeeds (the userspace socket comes up) but
        // `configure_interface` then rejects the config — e.g. Defguard's
        // `InterfaceConfiguration.prvkey` failing to decode to a 32-byte
        // key. `apply_tunnel_plan` must not leave that interface behind.
        let (plan, listen) = two_party_plan();
        let mut fake = crate::FakeWireGuardEffect {
            reject_next_apply: true,
            ..Default::default()
        };

        let err = apply_tunnel_plan(
            &mut fake,
            "ducktape-wg0",
            "cHJpdmF0ZS1rZXktYmFzZTY0LXBsYWNlaG9sZGVy",
            listen,
            &plan,
            None,
        )
        .unwrap_err();

        assert_eq!(err, crate::FakeWireGuardEffectError::Rejected);
        assert_eq!(fake.create_calls, 1);
        assert_eq!(
            fake.remove_calls, 1,
            "apply_tunnel_plan must tear down the interface it created when apply fails"
        );
        assert!(fake.applied.is_empty());
    }

    #[test]
    fn falls_back_to_the_plans_own_endpoint_when_no_override_is_given() {
        let (plan, listen) = two_party_plan();
        let mut fake = crate::FakeWireGuardEffect::default();

        apply_tunnel_plan(
            &mut fake,
            "ducktape-wg0",
            "cHJpdmF0ZS1rZXktYmFzZTY0LXBsYWNlaG9sZGVy",
            listen,
            &plan,
            None,
        )
        .unwrap();

        assert_eq!(
            fake.applied[0].peers[0].endpoint,
            Some(plan.peer_endpoint().socket_addr())
        );
    }

    #[test]
    fn update_reconfigures_the_live_interface_without_recreating_it() {
        // The mid-epoch shape: an interface applied once (create + apply),
        // then a peer set change — a standby tunnel joining the mesh —
        // re-applied through `update_peer_tunnels`. One create, two applied
        // configs, no teardown in between.
        let (plan_ab, plan_ac, listen) = three_party_plans();
        let mut fake = crate::FakeWireGuardEffect::default();
        let base = plan_peer_configs(std::slice::from_ref(&plan_ab), &BTreeMap::new());
        let local = plan_ab.local_interface_ips().to_vec();

        apply_peer_tunnels(
            &mut fake,
            "ducktape-wg0",
            "cHJpdmF0ZS1rZXktYmFzZTY0LXBsYWNlaG9sZGVy",
            listen,
            &local,
            &base,
        )
        .unwrap();

        let grown: Vec<PeerTunnelConfig> = base
            .iter()
            .cloned()
            .chain(plan_peer_configs(
                std::slice::from_ref(&plan_ac),
                &BTreeMap::new(),
            ))
            .collect();
        update_peer_tunnels(
            &mut fake,
            "ducktape-wg0",
            "cHJpdmF0ZS1rZXktYmFzZTY0LXBsYWNlaG9sZGVy",
            listen,
            &local,
            &grown,
        )
        .unwrap();

        assert_eq!(fake.create_calls, 1, "the live interface is never re-created");
        assert_eq!(fake.remove_calls, 0, "and never torn down");
        assert_eq!(fake.applied.len(), 2);
        assert_eq!(fake.applied[0].peers.len(), 1);
        assert_eq!(fake.applied[1].peers.len(), 2, "the update carries the grown set");
    }

    #[test]
    fn update_before_any_apply_is_rejected() {
        // `update_peer_tunnels` is strictly the live-interface form — calling
        // it with no interface up must fail exactly like the real UAPI
        // socket being absent, never silently record a config.
        let (plan, listen) = two_party_plan();
        let mut fake = crate::FakeWireGuardEffect::default();
        let peers = plan_peer_configs(std::slice::from_ref(&plan), &BTreeMap::new());

        let err = update_peer_tunnels(
            &mut fake,
            "ducktape-wg0",
            "cHJpdmF0ZS1rZXktYmFzZTY0LXBsYWNlaG9sZGVy",
            listen,
            plan.local_interface_ips(),
            &peers,
        )
        .unwrap_err();

        assert_eq!(err, crate::FakeWireGuardEffectError::NotCreated);
        assert!(fake.applied.is_empty());
    }

    #[test]
    fn plan_peer_configs_matches_the_applied_reduction() {
        // `apply_tunnel_plans` and a caller composing over
        // `plan_peer_configs` must produce the same peer entries — the
        // orchestrator merges these parts with record-derived peers, and a
        // drift here would mean the merged apply diverges from the plan-only
        // apply.
        let (plan_ab, plan_ac, _listen) = three_party_plans();
        let punched: SocketAddr = "203.0.113.9:40001".parse().unwrap();
        let overrides = BTreeMap::from([(plan_ac.peer_identity(), punched)]);

        let parts = plan_peer_configs(&[plan_ab.clone(), plan_ac.clone()], &overrides);

        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].wireguard_public_key, plan_ab.peer_wireguard_public_key());
        assert_eq!(parts[0].endpoint, plan_ab.peer_endpoint().socket_addr());
        assert_eq!(parts[0].allowed_ips, plan_ab.allowed_ips().to_vec());
        assert_eq!(parts[1].wireguard_public_key, plan_ac.peer_wireguard_public_key());
        assert_eq!(parts[1].endpoint, punched, "override lands by identity");
    }

    #[test]
    fn applies_all_plans_on_one_interface_with_per_peer_overrides() {
        let (plan_ab, plan_ac, listen) = three_party_plans();
        let mut fake = crate::FakeWireGuardEffect::default();
        let punched: SocketAddr = "203.0.113.9:40001".parse().unwrap();
        // only c resolved through the nat client; b stays on its advert.
        let overrides = BTreeMap::from([(plan_ac.peer_identity(), punched)]);

        apply_tunnel_plans(
            &mut fake,
            "ducktape-wg0",
            "cHJpdmF0ZS1rZXktYmFzZTY0LXBsYWNlaG9sZGVy",
            listen,
            &[plan_ab.clone(), plan_ac.clone()],
            &overrides,
        )
        .unwrap();

        assert_eq!(fake.create_calls, 1, "one interface for the whole mesh");
        assert_eq!(fake.applied.len(), 1);
        let applied = &fake.applied[0];
        // a's own overlay address appears ONCE — every plan carries the same
        // local side and `from_plan` dedupes it.
        assert_eq!(
            applied.addresses,
            plan_ab
                .local_interface_ips()
                .iter()
                .map(|r| IpAddrMask::new(r.addr, r.cidr))
                .collect::<Vec<_>>()
        );
        assert_eq!(applied.peers.len(), 2);
        assert_eq!(
            applied.peers[0].public_key.as_array(),
            plan_ab.peer_wireguard_public_key().0
        );
        assert_eq!(
            applied.peers[0].endpoint,
            Some(plan_ab.peer_endpoint().socket_addr()),
            "peer absent from the override map keeps its advertised endpoint"
        );
        assert_eq!(
            applied.peers[1].public_key.as_array(),
            plan_ac.peer_wireguard_public_key().0
        );
        assert_eq!(applied.peers[1].endpoint, Some(punched));
    }
}
