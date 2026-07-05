use std::collections::BTreeMap;
use std::net::SocketAddr;

use wireguard_upgrade::{
    DefguardInterfaceConfig, Endpoint, TunnelInstallPlan, ValidatorIdentity,
};

use crate::WireGuardEffect;

/// Apply a validated `TunnelInstallPlan` through a `WireGuardEffect`,
/// bringing up (or replacing) the local WireGuard interface for this one
/// peer relationship.
///
/// `peer_endpoint_override`, when set, replaces the plan's statically
/// advertised `peer_endpoint` with a different address before applying —
/// this is how a punched or relayed path gets wired in without touching
/// `wireguard-upgrade`'s validated plan: the caller passes the hole-punch's
/// resolved reflexive address
/// (`nat_traversal::punch::PunchPlan::peer_reflexive` in the simulated rig;
/// a real `NatClient` observation in production) or, on hole-punch failure,
/// a coordinator relay socket. `None` uses the plan's own advertised
/// endpoint unchanged (the direct, no-NAT-surprises case).
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
/// resolves independently (this one punched, that one relayed, another
/// direct), so the override is keyed by the peer's identity rather than
/// applied interface-wide. A peer absent from `endpoint_overrides` keeps its
/// plan's advertised endpoint.
pub fn apply_tunnel_plans<E: WireGuardEffect>(
    effect: &mut E,
    ifname: impl Into<String>,
    private_key_base64: impl Into<String>,
    listen_endpoint: Endpoint,
    plans: &[TunnelInstallPlan],
    endpoint_overrides: &BTreeMap<ValidatorIdentity, SocketAddr>,
) -> Result<(), E::Error> {
    let mut iface = DefguardInterfaceConfig::from_plan(
        ifname,
        private_key_base64,
        listen_endpoint,
        plans.to_vec(),
    );
    // `from_plan` emits peers in plan order — pair them back up by position,
    // not by endpoint, so two peers advertising the same address can never
    // cross-match.
    for (plan, peer) in plans.iter().zip(iface.config.peers.iter_mut()) {
        if let Some(addr) = endpoint_overrides.get(&plan.peer_identity()) {
            peer.endpoint = Some(*addr);
        }
    }
    effect.create_interface()?;
    if let Err(err) = effect.apply(&iface.config) {
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
