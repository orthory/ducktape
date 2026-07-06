use nat_traversal::{Coordinator, FallbackOutcome, NodeKey, SimNat, drive_with_relay_fallback};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use wireguard_effect::{FakeWireGuardEffect, apply_tunnel_plan};

use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
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

/// A minimal two-validator handshake, direct (no validator relay), yielding the
/// initiator's validated install plan and its listen endpoint. Copied from
/// `wireguard-effect`'s `src/wiring.rs` test fixture; `relay_candidates` is
/// empty, which the invariant test below relies on.
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
        wireguard_public_key: xkey(0x0a),
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
        wireguard_public_key: xkey(0x0b),
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
fn hole_punch_failure_relays_via_peer_endpoint_override() {
    // 1. A symmetric-NAT pair cannot hole-punch; the reachability plane falls
    //    back to the coordinator relay and returns the relay endpoint each side
    //    points WireGuard at.
    let mut a_nat = SimNat::symmetric(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1)));
    let mut b_nat = SimNat::symmetric(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 2)));
    let mut coord = Coordinator::new();
    let outcome = drive_with_relay_fallback(
        NodeKey([0xaa; 32]),
        NodeKey([0xbb; 32]),
        &mut a_nat,
        &mut b_nat,
        &mut coord,
        b"a",
        b"b",
    )
    .expect("relay fallback");
    let relay_endpoint = match outcome {
        FallbackOutcome::Relayed(p) => p.a_relay_endpoint,
        FallbackOutcome::Punched { .. } => panic!("symmetric pair must not punch"),
    };

    // 2. The relay endpoint is wired into WireGuard EXACTLY through the Slice 0b
    //    seam: apply_tunnel_plan's peer_endpoint_override. No wireguard-upgrade
    //    plan surgery.
    let (plan, listen) = two_party_plan();
    let mut fake = FakeWireGuardEffect::default();
    apply_tunnel_plan(
        &mut fake,
        "ducktape-wg0",
        "cHJpdmF0ZS1rZXktYmFzZTY0LXBsYWNlaG9sZGVy",
        listen,
        &plan,
        Some(relay_endpoint),
    )
    .unwrap();

    assert_eq!(fake.applied[0].peers[0].endpoint, Some(relay_endpoint));
}

#[test]
fn coordinator_relay_never_touches_the_validator_relay_mechanism() {
    // INVARIANT: the reachability-plane coordinator relay is a DIFFERENT layer
    // from wireguard-upgrade's validator-only relay_candidates /
    // DirectDialFailureEvidence. Relaying through peer_endpoint_override must
    // leave the validated plan's relay_candidates untouched (empty here) — the
    // data plane's "relay must be a validator" rule is preserved, and the two
    // relay concepts never couple.
    let (plan, listen) = two_party_plan();
    assert!(
        plan.relay_candidates().is_empty(),
        "the fixture has no validator relay to begin with"
    );

    let relay_endpoint: SocketAddr = "192.0.2.1:4000".parse().unwrap();
    let mut fake = FakeWireGuardEffect::default();
    apply_tunnel_plan(
        &mut fake,
        "ducktape-wg0",
        "cHJpdmF0ZS1rZXktYmFzZTY0LXBsYWNlaG9sZGVy",
        listen,
        &plan,
        Some(relay_endpoint),
    )
    .unwrap();

    // The applied config carries the coordinator relay endpoint but the plan's
    // validator relay set is STILL empty: reachability-plane relay and
    // data-plane validator relay stayed separate.
    assert_eq!(fake.applied[0].peers[0].endpoint, Some(relay_endpoint));
    assert!(plan.relay_candidates().is_empty());
}
