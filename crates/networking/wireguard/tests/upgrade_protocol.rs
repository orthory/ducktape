use std::net::{IpAddr, Ipv4Addr};

use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
use wireguard::*;

fn id(sk: &PrivateKey) -> ValidatorIdentity {
    let pk = sk.public_key();
    ValidatorIdentity::try_from(pk.as_ref()).unwrap()
}

fn root(byte: u8) -> Root {
    Root([byte; 32])
}

fn admission(byte: u8) -> AdmissionRoot {
    AdmissionRoot([byte; 32])
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
    ActiveValidatorSet::new("demo", 7, root(9), admission(8), vec![a, b]).unwrap()
}

fn record_for(
    signer: &PrivateKey,
    set: &ActiveValidatorSet,
    wg_addr: [u8; 4],
    wg: X25519PublicKey,
    nonce: u64,
) -> EndpointRecord {
    let policy = prod_policy();
    EndpointRecord {
        namespace: set.namespace.clone(),
        epoch: set.epoch,
        valset_root: set.valset_root,
        admission_root: set.admission_root,
        validator_identity: id(signer),
        wireguard_public_key: wg,
        control_endpoint: endpoint([1, 1, 1, wg_addr[3]], 443, Transport::Tcp, &policy),
        wireguard_endpoint: Some(endpoint(wg_addr, 51820, Transport::Udp, &policy)),
        nonce,
    }
}

// fixture convention: a (seed 1) advertises xkey(1), b (seed 2) xkey(2) — the
// handshake tests below sign with the same keys, satisfying the record pin.
fn signed_ads(
    a: &PrivateKey,
    b: &PrivateKey,
    set: &ActiveValidatorSet,
    a_addr: [u8; 4],
    b_addr: [u8; 4],
) -> (EndpointAdvertisement, EndpointAdvertisement) {
    let record_a = record_for(a, set, a_addr, xkey(1), 1);
    let record_b = record_for(b, set, b_addr, xkey(2), 1);
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
    let view = MeshView::verify(set.clone(), vec![ad_b, ad_a], &policy).unwrap();
    (a, b, set, view, policy)
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
    assert_eq!(
        ActiveValidatorSet::new(
            "demo",
            7,
            root(9),
            AdmissionRoot([0; 32]),
            vec![id(&a), id(&b)]
        )
        .unwrap_err(),
        UpgradeError::MissingAdmissionRoot
    );
    let (ad_a, ad_b) = signed_ads(&a, &b, &set, [8, 8, 8, 11], [8, 8, 8, 12]);
    let outsider_record = record_for(&outsider, &set, [8, 8, 8, 13], xkey(3), 1);
    let outsider_version =
        compute_mesh_version(&[ad_a.record.clone(), outsider_record.clone()]).unwrap();
    let outsider_ad = EndpointAdvertisement::sign(outsider_record, outsider_version, &outsider);

    assert!(MeshView::verify(set.clone(), vec![ad_a.clone(), outsider_ad], &policy).is_err());

    let expected_a = set.stable_index(id(&a)).unwrap();
    let expected_b = set.stable_index(id(&b)).unwrap();
    let view_ab = MeshView::verify(set.clone(), vec![ad_a.clone(), ad_b.clone()], &policy).unwrap();
    let view_ba = MeshView::verify(set, vec![ad_b, ad_a], &policy).unwrap();

    assert_eq!(view_ab.mesh_version, view_ba.mesh_version);
    assert_eq!(view_ab.records.len(), 2);
    assert_eq!(view_ab.stable_index(id(&a)).unwrap(), expected_a);
    assert_eq!(view_ab.stable_index(id(&b)).unwrap(), expected_b);
    assert_ne!(expected_a, expected_b);
}

#[test]
fn upgrade_validation_binds_ads_routes_ack_freshness_and_replay() {
    let (a, b, set, view, policy) = mesh();
    let overlay = OverlayPolicy::ula_v6("demo");
    let mut cache = ReplayCache::default();
    let request = TunnelUpgradeRequest::sign(
        TunnelUpgradeRequestFields {
            namespace: set.namespace.clone(),
            epoch: set.epoch,
            valset_root: set.valset_root,
            admission_root: set.admission_root,
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
            admission_root: set.admission_root,
            mesh_version: view.mesh_version,
            responder_identity: id(&b),
            initiator_identity: id(&a),
            responder_wireguard_public_key: xkey(2),
            responder_wireguard_endpoint: view.record(id(&b)).unwrap().wireguard_endpoint,
            accepted_allowed_ips: overlay.allowed_ips_for(&view, id(&a)).unwrap(),
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
            admission_root: set.admission_root,
            mesh_version: view.mesh_version,
            initiator_identity: id(&a),
            responder_identity: id(&b),
            installed_at_view: 11,
            expires_at_view: 40,
            nonce: 3,
        },
        &a,
    );

    let plan = validate_upgrade_as(
        Perspective::Initiator,
        &view,
        &policy,
        &overlay,
        12,
        &request,
        &response,
        &ack,
        &mut cache,
    )
    .unwrap();
    assert_eq!(plan.context().admission_root, set.admission_root);
    assert_eq!(
        plan.peer_endpoint(),
        view.record(id(&b)).unwrap().wireguard_endpoint
    );
    assert_eq!(
        plan.allowed_ips(),
        overlay.allowed_ips_for(&view, id(&b)).unwrap().as_slice()
    );

    let duplicate_nonce_ack = TunnelUpgradeAck::sign(
        TunnelUpgradeAckFields {
            nonce: request.fields.nonce,
            ..ack.fields.clone()
        },
        &a,
    );
    let mut fresh_cache = ReplayCache::default();
    let err = validate_upgrade_as(
        Perspective::Initiator,
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
        validate_upgrade_as(
            Perspective::Initiator,
            &view,
            &policy,
            &overlay,
            12,
            &request,
            &response,
            &ack,
            &mut cache
        )
        .is_err()
    );

    let mut bad_request = request.clone();
    bad_request.fields.initiator_wireguard_endpoint =
        Some(endpoint([8, 8, 8, 99], 51820, Transport::Udp, &policy));
    let mut fresh_cache = ReplayCache::default();
    assert!(
        validate_upgrade_as(
            Perspective::Initiator,
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
        validate_upgrade_as(
            Perspective::Initiator,
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
        validate_upgrade_as(
            Perspective::Initiator,
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

// ── the ULA-v6 overlay + advertised wireguard keys ──────────────────────────

/// the identity-hash ULA overlay: a chain-scoped fd::/48 whose member /128s
/// derive from (chain_id, identity) alone — deterministic, allocator-free,
/// and stable across membership churn (an identity hash never moves when the
/// sorted set changes).
#[test]
fn ula_overlay_is_deterministic_chain_scoped_and_identity_pinned() {
    let prefix = ula_v6_prefix("demo");
    assert_eq!(prefix, ula_v6_prefix("demo"));
    assert_eq!(prefix.octets()[0], 0xfd);
    assert_ne!(ula_v6_prefix("other-chain"), prefix);

    let (a, b, _set, view, _policy) = mesh();
    let addr_a = ula_v6_member_addr("demo", id(&a));
    let addr_b = ula_v6_member_addr("demo", id(&b));
    assert_eq!(addr_a.octets()[..6], prefix.octets()[..6]);
    assert_eq!(addr_b.octets()[..6], prefix.octets()[..6]);
    assert_ne!(addr_a, addr_b);

    let overlay = OverlayPolicy::ula_v6("demo");
    assert_eq!(
        overlay.allowed_ips_for(&view, id(&a)).unwrap(),
        vec![AllowedIp {
            addr: IpAddr::V6(addr_a),
            cidr: 128,
        }]
    );
    assert_eq!(
        overlay.allowed_ips_for(&view, id(&b)).unwrap(),
        vec![AllowedIp {
            addr: IpAddr::V6(addr_b),
            cidr: 128,
        }]
    );

    // no overlay address exists for an identity outside the view, however
    // well-formed — the membership gate holds even though the ULA derivation
    // would happily hash any identity.
    let outsider = PrivateKey::from_seed(9);
    assert_eq!(
        overlay.allowed_ips_for(&view, id(&outsider)).unwrap_err(),
        UpgradeError::UnknownValidator
    );
}

/// `MeshView::verify` refuses a record advertising the all-zero X25519 key —
/// the one value that can never be a real WireGuard public key.
#[test]
fn mesh_view_rejects_a_zero_wireguard_key() {
    let a = PrivateKey::from_seed(1);
    let b = PrivateKey::from_seed(2);
    let policy = prod_policy();
    let set = active_set(id(&a), id(&b));
    let mut record_a = record_for(&a, &set, [8, 8, 8, 10], xkey(1), 1);
    record_a.wireguard_public_key = X25519PublicKey([0u8; 32]);
    let record_b = record_for(&b, &set, [8, 8, 8, 20], xkey(2), 1);
    let mesh_version = compute_mesh_version(&[record_a.clone(), record_b.clone()]).unwrap();
    let ads = vec![
        EndpointAdvertisement::sign(record_a, mesh_version, &a),
        EndpointAdvertisement::sign(record_b, mesh_version, &b),
    ];
    assert_eq!(
        MeshView::verify(set, ads, &policy).unwrap_err(),
        UpgradeError::InvalidWireGuardKey
    );
}

/// A gossiped record relayed by a third member carries its OWNER's signature
/// — verification binds to `record.validator_identity`, so neither a tampered
/// field, a wrong signer, nor a grafted advertisement signature (different
/// domain) can pass.
#[test]
fn signed_record_verifies_owner_and_rejects_tamper_and_cross_domain() {
    let a = PrivateKey::from_seed(1);
    let b = PrivateKey::from_seed(2);
    let set = active_set(id(&a), id(&b));
    let record = record_for(&a, &set, [8, 8, 8, 10], xkey(1), 1);

    let signed = SignedEndpointRecord::sign(record.clone(), &a);
    signed.verify().expect("own signature verifies");

    // any signed field mutated after signing breaks verification — the
    // forwarder-forgery the signature exists to prevent.
    let mut forged = signed.clone();
    forged.record.wireguard_public_key = xkey(9);
    assert_eq!(forged.verify().unwrap_err(), UpgradeError::BadSignature);

    // signed by someone other than its claimed owner: never verifies.
    let cross = SignedEndpointRecord::sign(record.clone(), &b);
    assert_eq!(cross.verify().unwrap_err(), UpgradeError::BadSignature);

    // an advertisement signature over the same record must not verify under
    // the record domain.
    let version = compute_mesh_version(std::slice::from_ref(&record)).unwrap();
    let ad = EndpointAdvertisement::sign(record.clone(), version, &a);
    let grafted = SignedEndpointRecord {
        record,
        signature: ad.signature,
    };
    assert_eq!(grafted.verify().unwrap_err(), UpgradeError::BadSignature);
}

/// The two ends of a handshake run INDEPENDENT view clocks (each node's
/// plane learns views from its own finalization drain), so the ack's
/// `installed_at_view` routinely lands a tick or two AHEAD of the
/// validating responder's clock. Forward skew within the same freshness
/// lag must validate — a zero-tolerance future check permanently failed
/// real cross-node pairs (observed live: initiator applied, responder
/// refused the same triple with BadAckView). Skew beyond the lag still
/// refuses in both directions.
#[test]
fn ack_view_tolerates_cross_node_skew_within_the_lag() {
    let (a, b, set, view, policy) = mesh();
    let overlay = OverlayPolicy::ula_v6(set.namespace.clone());
    let request = TunnelUpgradeRequest::sign(
        TunnelUpgradeRequestFields {
            namespace: set.namespace.clone(),
            epoch: set.epoch,
            valset_root: set.valset_root,
            admission_root: set.admission_root,
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
            admission_root: set.admission_root,
            mesh_version: view.mesh_version,
            responder_identity: id(&b),
            initiator_identity: id(&a),
            responder_wireguard_public_key: xkey(2),
            responder_wireguard_endpoint: view.record(id(&b)).unwrap().wireguard_endpoint,
            accepted_allowed_ips: overlay.allowed_ips_for(&view, id(&a)).unwrap(),
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
            admission_root: set.admission_root,
            mesh_version: view.mesh_version,
            initiator_identity: id(&a),
            responder_identity: id(&b),
            installed_at_view: 11,
            expires_at_view: 40,
            nonce: 3,
        },
        &a,
    );

    // the responder's clock is ONE view behind the initiator's mint — the
    // routine cross-node case that must validate.
    let mut cache = ReplayCache::default();
    validate_upgrade_as(
        Perspective::Initiator,
        &view,
        &policy,
        &overlay,
        10,
        &request,
        &response,
        &ack,
        &mut cache,
    )
    .expect("one view of forward skew validates");

    // nine views behind (past MAX_ACK_INSTALL_LAG = 8): still refused.
    let mut cache = ReplayCache::default();
    assert_eq!(
        validate_upgrade_as(
            Perspective::Initiator,
            &view,
            &policy,
            &overlay,
            2,
            &request,
            &response,
            &ack,
            &mut cache,
        )
        .unwrap_err(),
        UpgradeError::BadAckView
    );
}

#[test]
fn record_check_mirrors_the_per_record_view_rules() {
    // `EndpointRecord::check` is the standalone form of the per-record
    // checks `MeshView::verify` runs — for records consumed outside a
    // verified view (a standby's pre-warm record). Each rule must hold
    // independently: endpoint policy on both endpoints, and a non-zero
    // X25519 key. Records carry NO freshness rule: signed once per epoch
    // and re-offered verbatim, they stay valid for the epoch's whole life.
    let a = PrivateKey::from_seed(1);
    let policy = prod_policy();
    let set = active_set(id(&a), id(&PrivateKey::from_seed(2)));
    let good = record_for(&a, &set, [8, 8, 8, 10], xkey(1), 1);

    good.check(&policy).expect("a policy-clean record checks");

    // an all-zero X25519 key can never be a real WireGuard peer key.
    let zero_key = EndpointRecord {
        wireguard_public_key: X25519PublicKey([0u8; 32]),
        ..good.clone()
    };
    assert_eq!(
        zero_key.check(&policy).unwrap_err(),
        UpgradeError::InvalidWireGuardKey
    );

    // an endpoint the policy forbids (private ip under production policy).
    // built under a permissive policy so construction succeeds, refused by
    // the strict one at check time — exactly the cross-policy gossip case.
    let open = PortPolicy {
        name: "open".into(),
        allowed_control_tcp_ports: vec![443],
        allowed_wireguard_udp_ports: vec![51820],
        allow_loopback: true,
        allow_private_ip: true,
    };
    let private_wg = EndpointRecord {
        wireguard_endpoint: Some(endpoint([10, 0, 0, 9], 51820, Transport::Udp, &open)),
        ..good.clone()
    };
    assert!(matches!(
        private_wg.check(&policy).unwrap_err(),
        UpgradeError::InvalidEndpoint(_)
    ));
}

#[test]
fn an_endpoint_less_record_signs_verifies_and_round_trips() {
    // the NAT'd-joiner shape: a record advertising NO WireGuard endpoint.
    // it must (a) pass the per-record checks (there is no endpoint to
    // policy-check), (b) sign and verify, (c) omit the field on the JSON
    // wire, and (d) round-trip endpoint-FUL records.
    let a = PrivateKey::from_seed(1);
    let policy = prod_policy();
    let set = active_set(id(&a), id(&PrivateKey::from_seed(2)));
    let endpoint_less = EndpointRecord {
        wireguard_endpoint: None,
        ..record_for(&a, &set, [8, 8, 8, 10], xkey(1), 1)
    };

    endpoint_less
        .check(&policy)
        .expect("no endpoint means nothing to policy-check");

    let signed = SignedEndpointRecord::sign(endpoint_less.clone(), &a);
    signed.verify().expect("owner signature verifies");

    // None omits the field entirely.
    let json = serde_json::to_string(&endpoint_less).unwrap();
    assert!(
        !json.contains("wireguard_endpoint"),
        "None must be absent on the wire: {json}"
    );
    let round: EndpointRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(round, endpoint_less);

    // The current endpoint-ful shape decodes as Some.
    let with_endpoint = record_for(&a, &set, [8, 8, 8, 10], xkey(1), 1);
    let endpoint_json = serde_json::to_string(&with_endpoint).unwrap();
    assert!(
        endpoint_json.contains("wireguard_endpoint"),
        "{endpoint_json}"
    );
    let decoded: EndpointRecord = serde_json::from_str(&endpoint_json).unwrap();
    assert_eq!(decoded.wireguard_endpoint, with_endpoint.wireguard_endpoint);

    // and the two SIGNING encodings can never collide: flipping the same
    // record between None and Some changes its signature domain bytes.
    let signed_some = SignedEndpointRecord::sign(with_endpoint, &a);
    let mut forged = signed_some.clone();
    forged.record.wireguard_endpoint = None;
    assert!(
        forged.verify().is_err(),
        "a stripped endpoint must break the owner signature"
    );
}

#[test]
fn duplicate_requested_allowed_ips_is_rejected_even_though_the_signature_verifies() {
    // The signed preimage sorts+dedups allowed-ips, so a request that REPEATS a
    // canonical route keeps the SAME hash and a valid signature — yet the effect
    // layer would materialize every entry into the WireGuard peer config. The
    // validator must reject the duplicate-bearing vector. A legitimate request
    // carries the canonical singleton (OverlayPolicy::allowed_ips_for returns one
    // route), so this guard can never reject an honest upgrade.
    let (a, b, set, view, policy) = mesh();
    let overlay = OverlayPolicy::ula_v6("demo");

    let canonical = overlay.allowed_ips_for(&view, id(&b)).unwrap();
    assert_eq!(
        canonical.len(),
        1,
        "canonical overlay routes are a singleton"
    );

    let request = TunnelUpgradeRequest::sign(
        TunnelUpgradeRequestFields {
            namespace: set.namespace.clone(),
            epoch: set.epoch,
            valset_root: set.valset_root,
            admission_root: set.admission_root,
            mesh_version: view.mesh_version,
            initiator_identity: id(&a),
            responder_identity: id(&b),
            initiator_wireguard_public_key: xkey(1),
            initiator_wireguard_endpoint: view.record(id(&a)).unwrap().wireguard_endpoint,
            requested_allowed_ips: canonical.clone(),
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
            responder_wireguard_public_key: xkey(2),
            responder_wireguard_endpoint: view.record(id(&b)).unwrap().wireguard_endpoint,
            accepted_allowed_ips: overlay.allowed_ips_for(&view, id(&a)).unwrap(),
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
            admission_root: set.admission_root,
            mesh_version: view.mesh_version,
            initiator_identity: id(&a),
            responder_identity: id(&b),
            installed_at_view: 11,
            expires_at_view: 40,
            nonce: 3,
        },
        &a,
    );

    // Sanity: the canonical (singleton) request validates.
    validate_upgrade_as(
        Perspective::Initiator,
        &view,
        &policy,
        &overlay,
        12,
        &request,
        &response,
        &ack,
        &mut ReplayCache::default(),
    )
    .expect("the canonical request validates");

    // Repeat the single canonical route. The dedup'd signing preimage makes the
    // hash and signature UNCHANGED, so the response/ack still match — only the
    // stored vector now carries a duplicate.
    let mut dup = canonical.clone();
    dup.push(canonical[0]);
    let request_dup = TunnelUpgradeRequest::sign(
        TunnelUpgradeRequestFields {
            requested_allowed_ips: dup,
            ..request.fields.clone()
        },
        &a,
    );
    assert_eq!(
        request_dup.hash(),
        request.hash(),
        "the dedup'd signing preimage makes a duplicate hash-invariant"
    );

    let err = validate_upgrade_as(
        Perspective::Initiator,
        &view,
        &policy,
        &overlay,
        12,
        &request_dup,
        &response,
        &ack,
        &mut ReplayCache::default(),
    )
    .expect_err("a duplicate-bearing allowed-ips vector must be rejected");
    assert!(
        matches!(err, UpgradeError::InvalidAllowedIp),
        "expected InvalidAllowedIp, got {err:?}"
    );
}
