use std::net::SocketAddr;

use wireguard_upgrade::{DefguardInterfaceConfig, Endpoint, TunnelInstallPlan};

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
    let mut iface = DefguardInterfaceConfig::from_plan(
        ifname,
        private_key_base64,
        listen_endpoint,
        vec![plan.clone()],
    );
    if let Some(addr) = peer_endpoint_override {
        for peer in &mut iface.config.peers {
            peer.endpoint = Some(addr);
        }
    }
    effect.create_interface()?;
    effect.apply(&iface.config)
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

    /// a minimal two-validator handshake, direct (no relay), yielding the
    /// INITIATOR's (a's) validated install plan and a's own listen endpoint —
    /// everything `apply_tunnel_plan` needs. `TunnelInstallPlan` has no
    /// public constructor by design (only `validate_upgrade`/
    /// `validate_upgrade_as` produce one), so this fixture runs the real
    /// signed handshake exactly like `wireguard-upgrade`'s own e2e tests do.
    fn two_party_plan() -> (TunnelInstallPlan, Endpoint) {
        let a = PrivateKey::from_seed(1);
        let b = PrivateKey::from_seed(2);
        let policy = PortPolicy::production();
        let overlay = OverlayPolicy::default_v4();
        let set = ActiveValidatorSet::new(
            "ducktape-wiring",
            1,
            Root([1u8; 32]),
            AdmissionRoot([2u8; 32]),
            vec![id(&a), id(&b)],
        )
        .unwrap();

        let record_a = EndpointRecord {
            namespace: set.namespace.clone(),
            epoch: set.epoch,
            valset_root: set.valset_root,
            admission_root: set.admission_root,
            validator_identity: id(&a),
            control_endpoint: endpoint(&policy, [1, 1, 1, 10], 443, Transport::Tcp),
            wireguard_endpoint: endpoint(&policy, [8, 8, 8, 10], 51820, Transport::Udp),
            capabilities: vec![],
            expires_at_view: 50,
            nonce: 1,
        };
        let record_b = EndpointRecord {
            namespace: set.namespace.clone(),
            epoch: set.epoch,
            valset_root: set.valset_root,
            admission_root: set.admission_root,
            validator_identity: id(&b),
            control_endpoint: endpoint(&policy, [1, 1, 1, 20], 443, Transport::Tcp),
            wireguard_endpoint: endpoint(&policy, [8, 8, 8, 20], 51820, Transport::Udp),
            capabilities: vec![],
            expires_at_view: 50,
            nonce: 1,
        };
        let records = vec![record_a.clone(), record_b.clone()];
        let mesh_version = compute_mesh_version(&records).unwrap();
        let ads = vec![
            EndpointAdvertisement::sign(record_a.clone(), mesh_version, &a),
            EndpointAdvertisement::sign(record_b.clone(), mesh_version, &b),
        ];
        let view = MeshView::verify(set.clone(), ads, &policy, 10).unwrap();

        let request = TunnelUpgradeRequest::sign(
            TunnelUpgradeRequestFields {
                namespace: set.namespace.clone(),
                epoch: set.epoch,
                valset_root: set.valset_root,
                admission_root: set.admission_root,
                mesh_version: view.mesh_version,
                initiator_identity: id(&a),
                responder_identity: id(&b),
                initiator_wireguard_public_key: xkey(0x0a),
                initiator_wireguard_endpoint: record_a.wireguard_endpoint,
                requested_allowed_ips: overlay.allowed_ips_for(&view, id(&b)).unwrap(),
                port_policy_hash: policy.hash(),
                expires_at_view: 40,
                nonce: 1,
            },
            &a,
        );
        let response = TunnelUpgradeResponse::sign(
            TunnelUpgradeResponseFields {
                request_hash: request.hash(),
                namespace: set.namespace.clone(),
                epoch: set.epoch,
                valset_root: set.valset_root,
                admission_root: set.admission_root,
                mesh_version: view.mesh_version,
                responder_identity: id(&b),
                initiator_identity: id(&a),
                responder_wireguard_public_key: xkey(0x0b),
                responder_wireguard_endpoint: record_b.wireguard_endpoint,
                accepted_allowed_ips: overlay.allowed_ips_for(&view, id(&a)).unwrap(),
                relay_candidates: vec![],
                direct_dial_failure: None,
                keepalive_seconds: Some(25),
                expires_at_view: 40,
                nonce: 1,
            },
            &b,
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
                initiator_identity: id(&a),
                responder_identity: id(&b),
                installed_at_view: 11,
                expires_at_view: 40,
                nonce: 2,
            },
            &a,
        );

        let mut replay = ReplayCache::default();
        let plan = validate_upgrade(
            &view, &policy, &overlay, 12, &request, &response, &ack, &mut replay,
        )
        .unwrap();
        (plan, record_a.wireguard_endpoint)
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
}
