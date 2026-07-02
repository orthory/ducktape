use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use demo::state_sync::{
    LoopbackStateSyncResolver, MeshParticipant, MeshRole, StateSyncError, StateSyncKind,
    StateSyncPayload, StateSyncPeerId, StateSyncRequest, decode_qmdb_target, decode_response,
    encode_qmdb_target, encode_request,
};
use kv::Kv;
use sdk::{Module as _, StateRoot};

fn peer(byte: u8) -> StateSyncPeerId {
    StateSyncPeerId::ed25519_public_key(vec![byte; sdk::ROOT_LEN]).unwrap()
}

#[test]
fn validator_set_member_serves_snapshot_frame() {
    let source = peer(7);
    let joiner = peer(11);
    let root = StateRoot([3; sdk::ROOT_LEN]);
    let mut resolver = LoopbackStateSyncResolver::default();
    resolver.insert_participant(MeshParticipant::validator_set_participant(source.clone()));
    resolver
        .serve_module(
            &source,
            "directory",
            root,
            StateSyncPayload::Snapshot(b"canonical snapshot".to_vec()),
        )
        .unwrap();

    let request = StateSyncRequest::new(
        joiner,
        source.clone(),
        "directory",
        root,
        StateSyncKind::Snapshot,
    );
    let response = decode_response(
        &resolver
            .resolve_bytes(&encode_request(&request))
            .expect("resolve snapshot frame"),
    )
    .expect("decode snapshot response");

    assert_eq!(response.source.peer_id(), &source);
    assert!(response.source.has_role(MeshRole::Bootnode));
    assert!(response.source.has_role(MeshRole::Relayer));
    assert_eq!(
        response.payload.into_snapshot_bytes().unwrap(),
        b"canonical snapshot"
    );
}

#[test]
fn source_outside_membership_is_rejected() {
    let source = peer(7);
    let joiner = peer(11);
    let resolver = LoopbackStateSyncResolver::default();
    let request = StateSyncRequest::new(
        joiner,
        source.clone(),
        "directory",
        StateRoot([3; sdk::ROOT_LEN]),
        StateSyncKind::Snapshot,
    );

    assert_eq!(
        resolver.resolve(request).unwrap_err(),
        StateSyncError::UnknownParticipant(source)
    );
}

#[test]
fn qmdb_target_payload_round_trips_through_frame() {
    deterministic::Runner::default().start(|context| async move {
        let source = peer(7);
        let joiner = peer(11);
        let mut resolver = LoopbackStateSyncResolver::default();
        resolver.insert_participant(MeshParticipant::validator_set_participant(source.clone()));

        let mut kv = Kv::init(context.child("source_kv"), "kv").await;
        kv.set(b"greeting".to_vec(), b"hello".to_vec()).await;
        let target = kv.sync_target().await;
        let root = kv.root();

        resolver
            .serve_module(
                &source,
                "kv",
                root,
                StateSyncPayload::QmdbTarget(encode_qmdb_target(&target)),
            )
            .unwrap();

        let request = StateSyncRequest::new(joiner, source, "kv", root, StateSyncKind::QmdbTarget);
        let response = decode_response(
            &resolver
                .resolve_bytes(&encode_request(&request))
                .expect("resolve qmdb target frame"),
        )
        .expect("decode qmdb target response");
        let decoded =
            decode_qmdb_target::<kv::KvTarget>(&response.payload.into_qmdb_target_bytes().unwrap())
                .expect("decode qmdb target");

        assert_eq!(decoded, target);
    });
}
