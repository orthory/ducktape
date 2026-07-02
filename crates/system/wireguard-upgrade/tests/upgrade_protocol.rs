use std::net::{IpAddr, Ipv4Addr};

use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
use wireguard_upgrade::*;

fn id(sk: &PrivateKey) -> ValidatorIdentity {
    let pk = sk.public_key();
    ValidatorIdentity::try_from(pk.as_ref()).unwrap()
}

fn root(byte: u8) -> Root {
    Root([byte; 32])
}

fn xkey(byte: u8) -> X25519PublicKey {
    X25519PublicKey([byte; 32])
}

fn prod_policy() -> PortPolicy {
    PortPolicy::production()
}

fn endpoint(addr: [u8; 4], port: u16, transport: Transport, policy: &PortPolicy) -> Endpoint {
    Endpoint::new(IpAddr::V4(Ipv4Addr::from(addr)), port, transport, policy).unwrap()
}

fn active_set(a: ValidatorIdentity, b: ValidatorIdentity) -> ActiveValidatorSet {
    ActiveValidatorSet::new("demo", 7, root(9), vec![a, b]).unwrap()
}

fn record_for(
    signer: &PrivateKey,
    set: &ActiveValidatorSet,
    wg_addr: [u8; 4],
    nonce: u64,
) -> EndpointRecord {
    let policy = prod_policy();
    EndpointRecord {
        namespace: set.namespace.clone(),
        epoch: set.epoch,
        valset_root: set.valset_root,
        validator_identity: id(signer),
        control_endpoint: endpoint([1, 1, 1, wg_addr[3]], 443, Transport::Tcp, &policy),
        wireguard_endpoint: endpoint(wg_addr, 51820, Transport::Udp, &policy),
        capabilities: vec![MeshCapability::Bootnode, MeshCapability::Relay],
        expires_at_view: 50,
        nonce,
    }
}

fn signed_ads(
    a: &PrivateKey,
    b: &PrivateKey,
    set: &ActiveValidatorSet,
    a_addr: [u8; 4],
    b_addr: [u8; 4],
) -> (EndpointAdvertisement, EndpointAdvertisement) {
    let record_a = record_for(a, set, a_addr, 1);
    let record_b = record_for(b, set, b_addr, 1);
    let mesh_version = compute_mesh_version(&[record_a.clone(), record_b.clone()]).unwrap();
    (
        EndpointAdvertisement::sign(record_a, mesh_version, a),
        EndpointAdvertisement::sign(record_b, mesh_version, b),
    )
}

fn mesh() -> (
    PrivateKey,
    PrivateKey,
    ActiveValidatorSet,
    MeshView,
    PortPolicy,
) {
    let a = PrivateKey::from_seed(1);
    let b = PrivateKey::from_seed(2);
    let policy = prod_policy();
    let set = active_set(id(&a), id(&b));
    let (ad_a, ad_b) = signed_ads(&a, &b, &set, [8, 8, 8, 10], [8, 8, 8, 20]);
    let view = MeshView::verify(set.clone(), vec![ad_b, ad_a], &policy, 10).unwrap();
    (a, b, set, view, policy)
}

fn direct_dial_failure(
    observer: &PrivateKey,
    set: &ActiveValidatorSet,
    view: &MeshView,
    target_identity: ValidatorIdentity,
    target_endpoint: Endpoint,
    nonce: u64,
) -> DirectDialFailureEvidence {
    DirectDialFailureEvidence::sign(
        DirectDialFailureFields {
            namespace: set.namespace.clone(),
            epoch: set.epoch,
            valset_root: set.valset_root,
            mesh_version: view.mesh_version,
            observer_identity: id(observer),
            target_identity,
            target_wireguard_endpoint: target_endpoint,
            failed_at_view: 11,
            expires_at_view: 40,
            error_hash: [7; 32],
            nonce,
        },
        observer,
    )
}

#[test]
fn endpoint_policy_rejects_dns_wildcards_bad_ports_and_wrong_transport() {
    let policy = prod_policy();

    assert!(Endpoint::parse("example.com:51820", Transport::Udp, &policy).is_err());
    assert!(Endpoint::parse("0.0.0.0:51820", Transport::Udp, &policy).is_err());
    assert!(Endpoint::parse("0.1.2.3:51820", Transport::Udp, &policy).is_err());
    assert!(Endpoint::parse("127.0.0.1:51820", Transport::Udp, &policy).is_err());
    assert!(Endpoint::parse("10.0.0.1:51820", Transport::Udp, &policy).is_err());
    assert!(Endpoint::parse("100.64.0.1:51820", Transport::Udp, &policy).is_err());
    assert!(Endpoint::parse("192.0.0.1:51820", Transport::Udp, &policy).is_err());
    assert!(Endpoint::parse("198.18.0.1:51820", Transport::Udp, &policy).is_err());
    assert!(Endpoint::parse("240.0.0.1:51820", Transport::Udp, &policy).is_err());
    assert!(Endpoint::parse("8.8.8.10:53", Transport::Udp, &policy).is_err());
    assert!(Endpoint::parse("8.8.8.10:51820", Transport::Tcp, &policy).is_err());
}

#[test]
fn mesh_view_uses_only_admitted_validators_and_has_deterministic_version() {
    let a = PrivateKey::from_seed(11);
    let b = PrivateKey::from_seed(12);
    let outsider = PrivateKey::from_seed(13);
    let policy = prod_policy();
    let set = active_set(id(&a), id(&b));
    let (ad_a, ad_b) = signed_ads(&a, &b, &set, [8, 8, 8, 11], [8, 8, 8, 12]);
    let outsider_record = record_for(&outsider, &set, [8, 8, 8, 13], 1);
    let outsider_version =
        compute_mesh_version(&[ad_a.record.clone(), outsider_record.clone()]).unwrap();
    let outsider_ad = EndpointAdvertisement::sign(outsider_record, outsider_version, &outsider);

    assert!(MeshView::verify(set.clone(), vec![ad_a.clone(), outsider_ad], &policy, 10).is_err());

    let expected_a = set.stable_index(id(&a)).unwrap();
    let expected_b = set.stable_index(id(&b)).unwrap();
    let view_ab =
        MeshView::verify(set.clone(), vec![ad_a.clone(), ad_b.clone()], &policy, 10).unwrap();
    let view_ba = MeshView::verify(set, vec![ad_b, ad_a], &policy, 10).unwrap();

    assert_eq!(view_ab.mesh_version, view_ba.mesh_version);
    assert_eq!(view_ab.records.len(), 2);
    assert_eq!(view_ab.stable_index(id(&a)).unwrap(), expected_a);
    assert_eq!(view_ab.stable_index(id(&b)).unwrap(), expected_b);
    assert_ne!(expected_a, expected_b);
}

#[test]
fn upgrade_validation_binds_ads_routes_ack_freshness_and_replay() {
    let (a, b, set, view, policy) = mesh();
    let overlay = OverlayPolicy::default_v4();
    let mut cache = ReplayCache::default();
    let request = TunnelUpgradeRequest::sign(
        TunnelUpgradeRequestFields {
            namespace: set.namespace.clone(),
            epoch: set.epoch,
            valset_root: set.valset_root,
            mesh_version: view.mesh_version,
            initiator_identity: id(&a),
            responder_identity: id(&b),
            initiator_wireguard_public_key: xkey(1),
            initiator_wireguard_endpoint: view.record(id(&a)).unwrap().wireguard_endpoint,
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
            mesh_version: view.mesh_version,
            responder_identity: id(&b),
            initiator_identity: id(&a),
            responder_wireguard_public_key: xkey(2),
            responder_wireguard_endpoint: view.record(id(&b)).unwrap().wireguard_endpoint,
            accepted_allowed_ips: overlay.allowed_ips_for(&view, id(&a)).unwrap(),
            relay_candidates: view.relay_candidates(),
            direct_dial_failure: Some(direct_dial_failure(
                &a,
                &set,
                &view,
                id(&b),
                view.record(id(&b)).unwrap().wireguard_endpoint,
                4,
            )),
            keepalive_seconds: Some(25),
            expires_at_view: 40,
            nonce: 2,
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
            mesh_version: view.mesh_version,
            initiator_identity: id(&a),
            responder_identity: id(&b),
            installed_at_view: 11,
            expires_at_view: 40,
            nonce: 3,
        },
        &a,
    );

    let plan = validate_upgrade(
        &view, &policy, &overlay, 12, &request, &response, &ack, &mut cache,
    )
    .unwrap();
    assert_eq!(
        plan.peer_endpoint,
        view.record(id(&b)).unwrap().wireguard_endpoint
    );
    assert_eq!(
        plan.allowed_ips,
        overlay.allowed_ips_for(&view, id(&b)).unwrap()
    );

    let response_without_failure = TunnelUpgradeResponse::sign(
        TunnelUpgradeResponseFields {
            direct_dial_failure: None,
            ..response.fields.clone()
        },
        &b,
    );
    let ack_without_failure = TunnelUpgradeAck::sign(
        TunnelUpgradeAckFields {
            response_hash: response_without_failure.hash(),
            ..ack.fields.clone()
        },
        &a,
    );
    let mut fresh_cache = ReplayCache::default();
    let err = validate_upgrade(
        &view,
        &policy,
        &overlay,
        12,
        &request,
        &response_without_failure,
        &ack_without_failure,
        &mut fresh_cache,
    )
    .unwrap_err();
    assert_eq!(err, UpgradeError::InvalidRelay);

    let duplicate_nonce_ack = TunnelUpgradeAck::sign(
        TunnelUpgradeAckFields {
            nonce: request.fields.nonce,
            ..ack.fields.clone()
        },
        &a,
    );
    let mut fresh_cache = ReplayCache::default();
    let err = validate_upgrade(
        &view,
        &policy,
        &overlay,
        12,
        &request,
        &response,
        &duplicate_nonce_ack,
        &mut fresh_cache,
    )
    .unwrap_err();
    assert_eq!(err, UpgradeError::Replay);

    assert!(
        validate_upgrade(
            &view, &policy, &overlay, 12, &request, &response, &ack, &mut cache
        )
        .is_err()
    );

    let mut bad_request = request.clone();
    bad_request.fields.initiator_wireguard_endpoint =
        endpoint([8, 8, 8, 99], 51820, Transport::Udp, &policy);
    let mut fresh_cache = ReplayCache::default();
    assert!(
        validate_upgrade(
            &view,
            &policy,
            &overlay,
            12,
            &bad_request,
            &response,
            &ack,
            &mut fresh_cache
        )
        .is_err()
    );

    let mut bad_response = response.clone();
    bad_response.fields.accepted_allowed_ips =
        vec![AllowedIp::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0).unwrap()];
    let mut fresh_cache = ReplayCache::default();
    assert!(
        validate_upgrade(
            &view,
            &policy,
            &overlay,
            12,
            &request,
            &bad_response,
            &ack,
            &mut fresh_cache
        )
        .is_err()
    );

    let mut stale_ack = ack.clone();
    stale_ack.fields.installed_at_view = 4;
    stale_ack.fields.expires_at_view = 10;
    let stale_ack = TunnelUpgradeAck::sign(stale_ack.fields, &a);
    let mut fresh_cache = ReplayCache::default();
    assert!(
        validate_upgrade(
            &view,
            &policy,
            &overlay,
            12,
            &request,
            &response,
            &stale_ack,
            &mut fresh_cache
        )
        .is_err()
    );
}

#[test]
fn valid_plan_builds_defguard_peer_config() {
    let (a, b, set, view, policy) = mesh();
    let overlay = OverlayPolicy::default_v4();
    let mut cache = ReplayCache::default();
    let request = TunnelUpgradeRequest::sign(
        TunnelUpgradeRequestFields {
            namespace: set.namespace.clone(),
            epoch: set.epoch,
            valset_root: set.valset_root,
            mesh_version: view.mesh_version,
            initiator_identity: id(&a),
            responder_identity: id(&b),
            initiator_wireguard_public_key: xkey(1),
            initiator_wireguard_endpoint: view.record(id(&a)).unwrap().wireguard_endpoint,
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
            mesh_version: view.mesh_version,
            responder_identity: id(&b),
            initiator_identity: id(&a),
            responder_wireguard_public_key: xkey(2),
            responder_wireguard_endpoint: view.record(id(&b)).unwrap().wireguard_endpoint,
            accepted_allowed_ips: overlay.allowed_ips_for(&view, id(&a)).unwrap(),
            relay_candidates: view.relay_candidates(),
            direct_dial_failure: Some(direct_dial_failure(
                &a,
                &set,
                &view,
                id(&b),
                view.record(id(&b)).unwrap().wireguard_endpoint,
                4,
            )),
            keepalive_seconds: Some(25),
            expires_at_view: 40,
            nonce: 2,
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
            mesh_version: view.mesh_version,
            initiator_identity: id(&a),
            responder_identity: id(&b),
            installed_at_view: 11,
            expires_at_view: 40,
            nonce: 3,
        },
        &a,
    );
    let plan = validate_upgrade(
        &view, &policy, &overlay, 12, &request, &response, &ack, &mut cache,
    )
    .unwrap();

    let peer = DefguardPeerConfig::from_plan(&plan);
    assert_eq!(peer.peer.endpoint, Some(plan.peer_endpoint.socket_addr()));
    assert_eq!(peer.allowed_ips, plan.allowed_ips);

    let interface = DefguardInterfaceConfig::from_plan(
        "wg-ducktape0",
        "AAECAwQFBgcICQoLDA0OD/Dh0sO0pZaHeGlaSzwtHg8=",
        endpoint([1, 1, 1, 200], 51820, Transport::Udp, &policy),
        vec![plan.clone()],
    );
    assert_eq!(interface.config.name, "wg-ducktape0");
    assert_eq!(interface.config.port, 51820);
    assert_eq!(interface.config.peers.len(), 1);
}
