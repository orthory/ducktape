//! tunnel-upgrade e2e at the protocol boundary: the full multi-validator
//! conversation — endpoint advertisements -> independently verified mesh views
//! -> signed request/response/ack -> both parties validating and deriving
//! their own install plans — plus the doc-mandated mesh-version fixed vector
//! and epoch cutover revocation. the effectful boundary (turning a plan into
//! an applied interface) is covered by the `effect` module's own tests.

use std::net::{IpAddr, Ipv4Addr};

use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
use wireguard::*;

// ── fixtures ────────────────────────────────────────────────────────────────

fn id(sk: &PrivateKey) -> ValidatorIdentity {
    ValidatorIdentity::try_from(sk.public_key().as_ref()).unwrap()
}

fn xkey(byte: u8) -> X25519PublicKey {
    X25519PublicKey([byte; 32])
}

fn endpoint(policy: &PortPolicy, addr: [u8; 4], port: u16, transport: Transport) -> Endpoint {
    Endpoint::new(IpAddr::V4(Ipv4Addr::from(addr)), port, transport, policy).unwrap()
}

fn overlay() -> OverlayPolicy {
    OverlayPolicy::ula_v6("ducktape-e2e")
}

/// one validator's deterministic record for `set`: control at 1.1.1.x:443,
/// wireguard at 8.8.8.x:51820, expiry view 50.
fn record(
    signer: &PrivateKey,
    set: &ActiveValidatorSet,
    last_octet: u8,
    wg: X25519PublicKey,
    nonce: u64,
) -> EndpointRecord {
    let policy = PortPolicy::production();
    EndpointRecord {
        namespace: set.namespace.clone(),
        epoch: set.epoch,
        valset_root: set.valset_root,
        admission_root: set.admission_root,
        validator_identity: id(signer),
        wireguard_public_key: wg,
        control_endpoint: endpoint(&policy, [1, 1, 1, last_octet], 443, Transport::Tcp),
        wireguard_endpoint: Some(endpoint(
            &policy,
            [8, 8, 8, last_octet],
            51820,
            Transport::Udp,
        )),
        nonce,
    }
}

/// a three-validator epoch: a and b handshake, c is a bystander member.
fn three_party_epoch(
    epoch: u64,
    valset_byte: u8,
    admission_byte: u8,
) -> (PrivateKey, PrivateKey, PrivateKey, ActiveValidatorSet) {
    let a = PrivateKey::from_seed(1);
    let b = PrivateKey::from_seed(2);
    let c = PrivateKey::from_seed(3);
    let set = ActiveValidatorSet::new(
        "ducktape-e2e",
        epoch,
        Root([valset_byte; 32]),
        AdmissionRoot([admission_byte; 32]),
        vec![id(&a), id(&b), id(&c)],
    )
    .unwrap();
    (a, b, c, set)
}

/// every validator's signed advertisement over the same records — the gossip
/// each node would receive off the mesh.
fn advertisements(
    parties: &[(&PrivateKey, u8, X25519PublicKey)],
    set: &ActiveValidatorSet,
) -> Vec<EndpointAdvertisement> {
    let records: Vec<EndpointRecord> = parties
        .iter()
        .map(|(sk, octet, wg)| record(sk, set, *octet, *wg, 1))
        .collect();
    let mesh_version = compute_mesh_version(&records).unwrap();
    parties
        .iter()
        .zip(records)
        .map(|((sk, _, _), rec)| EndpointAdvertisement::sign(rec, mesh_version, sk))
        .collect()
}

struct Handshake {
    request: TunnelUpgradeRequest,
    response: TunnelUpgradeResponse,
    ack: TunnelUpgradeAck,
}

/// the full signed a->b conversation: request (initiator), response
/// (responder), ack (initiator).
fn handshake(
    initiator: &PrivateKey,
    responder: &PrivateKey,
    initiator_xkey: X25519PublicKey,
    responder_xkey: X25519PublicKey,
    view: &MeshView,
    policy: &PortPolicy,
    overlay: &OverlayPolicy,
) -> Handshake {
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
            initiator_wireguard_public_key: initiator_xkey,
            initiator_wireguard_endpoint: view.record(id(initiator)).unwrap().wireguard_endpoint,
            requested_allowed_ips: overlay.allowed_ips_for(view, id(responder)).unwrap(),
            port_policy_hash: policy.hash(),
            expires_at_view: 40,
            nonce: 1,
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
            responder_wireguard_public_key: responder_xkey,
            responder_wireguard_endpoint: view.record(id(responder)).unwrap().wireguard_endpoint,
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
            nonce: 2,
        },
        initiator,
    );
    Handshake {
        request,
        response,
        ack,
    }
}

fn hex32(bytes: [u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ── the e2e conversation ────────────────────────────────────────────────────

/// the full three-party flow: every node independently verifies the SAME mesh
/// from differently-ordered gossip, and both handshake parties validate the
/// signed triple against their own replay state. `Perspective::Initiator`
/// yields the initiator's plan from EITHER party's view — the perspective,
/// not the caller, decides whose plan comes back.
#[test]
fn both_parties_validate_but_the_plan_is_initiator_local() {
    let (a, b, c, set) = three_party_epoch(7, 9, 8);
    let policy = PortPolicy::production();
    let overlay = overlay();
    let ads = advertisements(
        &[
            (&a, 10, xkey(0x0a)),
            (&b, 20, xkey(0x0b)),
            (&c, 30, xkey(0x0c)),
        ],
        &set,
    );

    // each party builds its OWN view from its OWN gossip arrival order; the
    // mesh version is content-derived, so all three converge byte-identically.
    let view_a = MeshView::verify(set.clone(), ads.clone(), &policy).unwrap();
    let mut reversed = ads.clone();
    reversed.reverse();
    let view_b = MeshView::verify(set.clone(), reversed, &policy).unwrap();
    let mut rotated = ads.clone();
    rotated.rotate_left(1);
    let view_c = MeshView::verify(set.clone(), rotated, &policy).unwrap();
    assert_eq!(view_a.mesh_version, view_b.mesh_version);
    assert_eq!(view_a.mesh_version, view_c.mesh_version);

    // a -> b, direct.
    let hs = handshake(&a, &b, xkey(0x0a), xkey(0x0b), &view_a, &policy, &overlay);

    // BOTH parties accept the triple, each against its own replay cache.
    let mut cache_a = ReplayCache::default();
    let plan_a = validate_upgrade_as(
        Perspective::Initiator,
        &view_a,
        &policy,
        &overlay,
        12,
        &hs.request,
        &hs.response,
        &hs.ack,
        &mut cache_a,
    )
    .unwrap();
    let mut cache_b = ReplayCache::default();
    let plan_b = validate_upgrade_as(
        Perspective::Initiator,
        &view_b,
        &policy,
        &overlay,
        12,
        &hs.request,
        &hs.response,
        &hs.ack,
        &mut cache_b,
    )
    .unwrap();

    // the initiator perspective from b's own view still returns a's plan —
    // `Perspective::Responder` is how b derives ITS OWN plan, see
    // `responder_derives_its_own_install_plan_from_validate_upgrade_as`.
    assert_eq!(plan_a, plan_b);
    assert_eq!(plan_b.local_identity(), id(&a));
    assert_eq!(plan_b.peer_identity(), id(&b));

    // complementary overlay routing: a's interface owns a's /128, the tunnel
    // routes b's /128 — and the two assignments never overlap.
    assert_eq!(
        plan_a.local_interface_ips(),
        overlay.allowed_ips_for(&view_a, id(&a)).unwrap().as_slice()
    );
    assert_eq!(
        plan_a.allowed_ips(),
        overlay.allowed_ips_for(&view_a, id(&b)).unwrap().as_slice()
    );
    assert_ne!(plan_a.local_interface_ips(), plan_a.allowed_ips());
    assert_eq!(plan_a.local_wireguard_public_key(), xkey(0x0a));
    assert_eq!(plan_a.peer_wireguard_public_key(), xkey(0x0b));
    assert_eq!(
        plan_a.peer_endpoint(),
        view_a.record(id(&b)).unwrap().wireguard_endpoint
    );
    assert_eq!(plan_a.keepalive_seconds(), Some(25));
}

/// `validate_upgrade_as` lets the RESPONDER derive its own install plan from
/// the identical signed triple: the same checks run once, from the
/// responder's own view + replay cache, with the responder's own perspective.
#[test]
fn responder_derives_its_own_install_plan_from_validate_upgrade_as() {
    let (a, b, c, set) = three_party_epoch(7, 9, 8);
    let policy = PortPolicy::production();
    let overlay = overlay();
    let ads = advertisements(
        &[
            (&a, 10, xkey(0x0a)),
            (&b, 20, xkey(0x0b)),
            (&c, 30, xkey(0x0c)),
        ],
        &set,
    );
    let view_a = MeshView::verify(set.clone(), ads.clone(), &policy).unwrap();
    let mut reversed = ads.clone();
    reversed.reverse();
    let view_b = MeshView::verify(set.clone(), reversed, &policy).unwrap();

    let hs = handshake(&a, &b, xkey(0x0a), xkey(0x0b), &view_a, &policy, &overlay);

    // the initiator's plan, exactly as the previous test covers.
    let mut cache_a = ReplayCache::default();
    let plan_a = validate_upgrade_as(
        Perspective::Initiator,
        &view_a,
        &policy,
        &overlay,
        12,
        &hs.request,
        &hs.response,
        &hs.ack,
        &mut cache_a,
    )
    .unwrap();

    // the responder validates the SAME triple against its OWN view and its
    // OWN replay cache, asking for ITS perspective in one call.
    let mut cache_b = ReplayCache::default();
    let plan_b = validate_upgrade_as(
        Perspective::Responder,
        &view_b,
        &policy,
        &overlay,
        12,
        &hs.request,
        &hs.response,
        &hs.ack,
        &mut cache_b,
    )
    .unwrap();

    // b's plan is local-to-b: the mirror image of a's plan, not a copy of it.
    assert_eq!(plan_b.local_identity(), id(&b));
    assert_eq!(plan_b.peer_identity(), id(&a));
    assert_eq!(plan_b.local_wireguard_public_key(), xkey(0x0b));
    assert_eq!(plan_b.peer_wireguard_public_key(), xkey(0x0a));
    assert_eq!(
        plan_b.peer_endpoint(),
        view_b.record(id(&a)).unwrap().wireguard_endpoint
    );
    assert_eq!(
        plan_b.local_interface_ips(),
        overlay.allowed_ips_for(&view_b, id(&b)).unwrap().as_slice()
    );
    assert_eq!(
        plan_b.allowed_ips(),
        overlay.allowed_ips_for(&view_b, id(&a)).unwrap().as_slice()
    );
    assert_ne!(plan_a, plan_b);

    // complementary: a's local address is what b routes to, and vice versa.
    assert_eq!(plan_a.local_interface_ips(), plan_b.allowed_ips());
    assert_eq!(plan_b.local_interface_ips(), plan_a.allowed_ips());
    assert_eq!(
        plan_a.peer_wireguard_public_key(),
        plan_b.local_wireguard_public_key()
    );
    assert_eq!(
        plan_b.peer_wireguard_public_key(),
        plan_a.local_wireguard_public_key()
    );
}

// ── the doc-mandated fixed vector ───────────────────────────────────────────

/// docs/records/protocols/wireguard-tunnel-upgrade.md: "Implementations must ship fixed test
/// vectors for this preimage so that independent nodes produce the same mesh
/// version from the same admitted set." this pins the current preimage —
/// an accidental encoding change fails HERE instead of splitting a live mesh.
/// (a deliberate protocol change updates the constant in the same commit.)
///
/// every input is a LITERAL — identity bytes are written out, not derived
/// through a signature crate — so an independent implementation can reproduce
/// the vector from this file alone, and no dependency bump can move it.
#[test]
fn mesh_version_fixed_vector() {
    const MESH_VERSION_VECTOR: &str =
        "760cdf81e882e55e960262b7aa5b5845fb37e616f83a568e473194a6c703b1e5";

    let policy = PortPolicy::production();
    let literal_record = |identity_byte: u8, host_octet: u8| EndpointRecord {
        namespace: "ducktape-vector".into(),
        epoch: 7,
        valset_root: Root([0x22; 32]),
        admission_root: AdmissionRoot([0x33; 32]),
        validator_identity: ValidatorIdentity([identity_byte; 32]),
        // literal like the identity: the wg key is the identity byte with the
        // top bit flipped, written out so the vector derives from this file
        // alone.
        wireguard_public_key: X25519PublicKey([identity_byte ^ 0x80; 32]),
        control_endpoint: endpoint(&policy, [1, 1, 1, host_octet], 443, Transport::Tcp),
        wireguard_endpoint: Some(endpoint(
            &policy,
            [8, 8, 8, host_octet],
            51820,
            Transport::Udp,
        )),
        nonce: 1,
    };
    let records = vec![
        literal_record(0xa1, 10),
        literal_record(0xb2, 20),
        literal_record(0xc3, 30),
    ];
    let version = compute_mesh_version(&records).unwrap();
    assert_eq!(
        hex32(version.0),
        MESH_VERSION_VECTOR,
        "the mesh-version preimage changed — if this is deliberate, update the vector"
    );

    // key sensitivity: rotating one validator's wireguard key moves
    // the version — the mesh commits to the key set, not just endpoints.
    let rekeyed = {
        let mut r = records.clone();
        r[0].wireguard_public_key = X25519PublicKey([0x5a; 32]);
        compute_mesh_version(&r).unwrap()
    };
    assert_ne!(rekeyed, version);

    // sensitivity: any endpoint or set change moves the version...
    let moved = {
        let mut r = records.clone();
        r[1] = literal_record(0xb2, 21);
        compute_mesh_version(&r).unwrap()
    };
    assert_ne!(moved, version);
    let other_epoch = {
        let mut r = records.clone();
        for rec in &mut r {
            rec.epoch = 8;
        }
        compute_mesh_version(&r).unwrap()
    };
    assert_ne!(other_epoch, version);

    // ...and record ORDER does not (the preimage sorts record hashes).
    let mut shuffled = records.clone();
    shuffled.reverse();
    assert_eq!(compute_mesh_version(&shuffled).unwrap(), version);
}

// ── epoch cutover ───────────────────────────────────────────────────────────

/// a valset cutover revokes departed validators and re-keys the survivors:
/// epoch-7 handshakes die against the epoch-8 view, c (departed) cannot join
/// the epoch-8 mesh at all, and a surviving pair re-upgrades with rotated
/// session keys — reusing nonce NUMBERS legally, because replay state is
/// keyed by (identity, epoch, nonce).
#[test]
fn epoch_cutover_revokes_departed_validators_and_rekeys_survivors() {
    let policy = PortPolicy::production();
    let overlay = overlay();

    // epoch 7: {a, b, c}; a->b tunnel established.
    let (a, b, c, set7) = three_party_epoch(7, 9, 8);
    let ads7 = advertisements(
        &[
            (&a, 10, xkey(0x0a)),
            (&b, 20, xkey(0x0b)),
            (&c, 30, xkey(0x0c)),
        ],
        &set7,
    );
    let view7 = MeshView::verify(set7.clone(), ads7, &policy).unwrap();
    let hs7 = handshake(&a, &b, xkey(0x0a), xkey(0x0b), &view7, &policy, &overlay);
    let mut cache = ReplayCache::default();
    validate_upgrade_as(
        Perspective::Initiator,
        &view7,
        &policy,
        &overlay,
        12,
        &hs7.request,
        &hs7.response,
        &hs7.ack,
        &mut cache,
    )
    .expect("epoch-7 upgrade is valid");

    // epoch 8: c departs; new roots, new admission commitment.
    let a8 = PrivateKey::from_seed(1);
    let b8 = PrivateKey::from_seed(2);
    let set8 = ActiveValidatorSet::new(
        "ducktape-e2e",
        8,
        Root([11; 32]),
        AdmissionRoot([12; 32]),
        vec![id(&a8), id(&b8)],
    )
    .unwrap();
    let ads8 = advertisements(&[(&a8, 10, xkey(0xa8)), (&b8, 20, xkey(0xb8))], &set8);
    let view8 = MeshView::verify(set8.clone(), ads8.clone(), &policy).unwrap();
    assert_ne!(view8.mesh_version, view7.mesh_version);

    // the departed validator's fresh epoch-8 advertisement is rejected: it is
    // simply not in the admitted set, however well-signed.
    let mut c_record = record(&c, &set8, 30, xkey(0x0c), 2);
    c_record.epoch = 8;
    let c_ad = EndpointAdvertisement::sign(c_record, view8.mesh_version, &c);
    let mut with_c = ads8.clone();
    with_c.push(c_ad);
    assert_eq!(
        MeshView::verify(set8.clone(), with_c, &policy).unwrap_err(),
        UpgradeError::UnknownValidator
    );

    // every epoch-7 signed message is dead against the epoch-8 view.
    assert_eq!(
        validate_upgrade_as(
            Perspective::Initiator,
            &view8,
            &policy,
            &overlay,
            12,
            &hs7.request,
            &hs7.response,
            &hs7.ack,
            &mut cache,
        )
        .unwrap_err(),
        UpgradeError::HandshakeMismatch
    );

    // survivors re-upgrade with ROTATED wireguard session keys; the same
    // nonce numbers are fresh again under the epoch-8 replay domain, in the
    // SAME cache that recorded epoch 7.
    let hs8 = handshake(&a8, &b8, xkey(0xa8), xkey(0xb8), &view8, &policy, &overlay);
    let plan8 = validate_upgrade_as(
        Perspective::Initiator,
        &view8,
        &policy,
        &overlay,
        12,
        &hs8.request,
        &hs8.response,
        &hs8.ack,
        &mut cache,
    )
    .expect("survivors re-upgrade at epoch 8");
    assert_eq!(plan8.local_wireguard_public_key(), xkey(0xa8));
    assert_eq!(plan8.peer_wireguard_public_key(), xkey(0xb8));
    assert_eq!(plan8.context().epoch, 8);

    // but WITHIN epoch 8, replaying the identical triple is refused.
    assert_eq!(
        validate_upgrade_as(
            Perspective::Initiator,
            &view8,
            &policy,
            &overlay,
            12,
            &hs8.request,
            &hs8.response,
            &hs8.ack,
            &mut cache,
        )
        .unwrap_err(),
        UpgradeError::Replay
    );
}

// ── advertised-key pinning + the ULA-v6 overlay ─────────────────────────────

/// the handshake's X25519 keys are pinned to the mesh-versioned records: a
/// fresh session key the mesh never advertised is refused even though every
/// signature in the triple checks out.
#[test]
fn handshake_wireguard_keys_must_match_the_advertised_records() {
    let (a, b, c, set) = three_party_epoch(7, 9, 8);
    let policy = PortPolicy::production();
    let overlay = overlay();
    let ads = advertisements(
        &[
            (&a, 10, xkey(0x0a)),
            (&b, 20, xkey(0x0b)),
            (&c, 30, xkey(0x0c)),
        ],
        &set,
    );
    let view = MeshView::verify(set.clone(), ads, &policy).unwrap();

    // a signs its request under an unadvertised key: refused at the record
    // pin, not at any signature check.
    let hs = handshake(&a, &b, xkey(0x77), xkey(0x0b), &view, &policy, &overlay);
    let mut cache = ReplayCache::default();
    assert_eq!(
        validate_upgrade_as(
            Perspective::Initiator,
            &view,
            &policy,
            &overlay,
            12,
            &hs.request,
            &hs.response,
            &hs.ack,
            &mut cache,
        )
        .unwrap_err(),
        UpgradeError::HandshakeMismatch
    );
}

/// the ULA-v6 overlay end-to-end: the validated plan's interface address and
/// tunnel route are the two parties' identity-hash /128s inside the chain's
/// fd::/48, complementary and disjoint.
#[test]
fn ula_v6_overlay_routes_the_tunnel_with_identity_pinned_128s() {
    let (a, b, c, set) = three_party_epoch(7, 9, 8);
    let policy = PortPolicy::production();
    let overlay = overlay();
    let ads = advertisements(
        &[
            (&a, 10, xkey(0x0a)),
            (&b, 20, xkey(0x0b)),
            (&c, 30, xkey(0x0c)),
        ],
        &set,
    );
    let view = MeshView::verify(set.clone(), ads, &policy).unwrap();
    let hs = handshake(&a, &b, xkey(0x0a), xkey(0x0b), &view, &policy, &overlay);
    let mut cache = ReplayCache::default();
    let plan = validate_upgrade_as(
        Perspective::Initiator,
        &view,
        &policy,
        &overlay,
        12,
        &hs.request,
        &hs.response,
        &hs.ack,
        &mut cache,
    )
    .unwrap();

    let expected_local = AllowedIp {
        addr: IpAddr::V6(ula_v6_member_addr("ducktape-e2e", id(&a))),
        cidr: 128,
    };
    let expected_peer = AllowedIp {
        addr: IpAddr::V6(ula_v6_member_addr("ducktape-e2e", id(&b))),
        cidr: 128,
    };
    assert_eq!(plan.local_interface_ips(), &[expected_local]);
    assert_eq!(plan.allowed_ips(), &[expected_peer]);
    assert_ne!(expected_local, expected_peer);
    let prefix = ula_v6_prefix("ducktape-e2e").octets();
    for route in plan.local_interface_ips().iter().chain(plan.allowed_ips()) {
        let IpAddr::V6(v6) = route.addr else {
            panic!("ULA overlay routes must be v6");
        };
        assert_eq!(v6.octets()[..6], prefix[..6], "route inside the chain /48");
    }
}
