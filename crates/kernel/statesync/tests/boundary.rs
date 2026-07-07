use commonware_cryptography::sha256::Digest as Sha256Digest;
use futures::executor::block_on;
use host::Host;
use sdk::StateRoot;
use statesync::{
    BoundaryCoords, BoundaryId, MAX_CAPTURES, Manifest, ManifestEntry, PayloadKind, ResolverTarget,
    SyncError, SyncRequest, SyncResponse, SyncServer, decode_response, encode_response,
};

#[test]
fn captures_keyed_by_boundary_id_not_height() {
    let mut srv = SyncServer::new();
    let b1 = BoundaryId {
        height: 32,
        app_hash: StateRoot([1u8; 32]),
    };
    let b2 = BoundaryId {
        height: 32,
        app_hash: StateRoot([2u8; 32]),
    };

    srv.insert_capture_for_test(b1);
    srv.insert_capture_for_test(b2);

    assert!(srv.has_capture(b1) && srv.has_capture(b2));
}

#[test]
fn leased_capture_survives_eviction() {
    let mut srv = SyncServer::new();
    let held = BoundaryId {
        height: 10,
        app_hash: StateRoot([9u8; 32]),
    };
    srv.insert_capture_for_test(held);
    srv.lease(held);

    for h in 100..100 + (MAX_CAPTURES as u64) + 3 {
        let b = BoundaryId {
            height: h,
            app_hash: StateRoot([(h % 251) as u8; 32]),
        };
        srv.insert_capture_for_test(b);
    }

    assert!(srv.has_capture(held), "leased boundary must not be evicted");
}

#[test]
fn leased_boundaries_are_bounded_and_oldest_is_released() {
    let mut srv = SyncServer::new();
    let mut leased = Vec::new();

    for h in 1..=(MAX_CAPTURES as u64) + 2 {
        let boundary = BoundaryId {
            height: h,
            app_hash: StateRoot([(h % 251) as u8; 32]),
        };
        srv.insert_capture_for_test(boundary);
        srv.lease(boundary);
        leased.push(boundary);
    }

    assert_eq!(
        srv.leased_count_for_test(),
        MAX_CAPTURES,
        "lease set must stay bounded by the capture retention cap",
    );
    assert!(
        !srv.is_leased_for_test(leased[0]),
        "oldest abandoned lease must be released first",
    );
    assert!(
        !srv.has_capture(leased[0]),
        "released overflow boundary should become evictable",
    );
    assert!(
        srv.is_leased_for_test(*leased.last().unwrap()),
        "most recently leased boundary remains active",
    );
}

#[test]
fn manifest_roundtrip_carries_pinned_resolver_target() {
    let m = Manifest {
        height: 77,
        app_hash: StateRoot([4u8; 32]),
        epoch: 2,
        view_base: 70,
        participants: vec![vec![3u8; 32]],
        residents: vec![],
        floor_cert: Some(vec![0xCC; 96]),
        current_version: host::BASELINE_VERSION,
        pending_upgrade: None,
        required_min_version: host::BASELINE_VERSION,
        entries: vec![ManifestEntry {
            module_id: "kv".into(),
            root: StateRoot([7u8; 32]),
            kind: PayloadKind::Resolver,
            resolver_target: Some(ResolverTarget {
                root: Sha256Digest([7u8; 32]),
                start: 5,
                op_count: 42,
            }),
        }],
    };

    let bytes = encode_response(&SyncResponse::Manifest(m.clone()));
    let SyncResponse::Manifest(back) = decode_response(&bytes).unwrap() else {
        panic!("manifest response expected");
    };

    assert_eq!(back.boundary_id(), m.boundary_id());
    let e = back.entry("kv").unwrap();
    assert_eq!(e.resolver_target.as_ref().unwrap().op_count, 42);
    assert_eq!(e.resolver_target.as_ref().unwrap().start, 5);
}

#[test]
fn server_rejects_chunk_for_unleased_boundary() {
    let mut srv = SyncServer::new();
    let stale = BoundaryId {
        height: 5,
        app_hash: StateRoot([3u8; 32]),
    };
    let host = Host::genesis(vec![]).unwrap();
    let coords = BoundaryCoords::default();

    let resp = block_on(srv.handle(
        &host,
        None,
        &coords,
        SyncRequest::Chunk {
            boundary: stale,
            module_id: "directory".into(),
            offset: 0,
        },
    ));

    assert!(
        matches!(resp, SyncResponse::Error(_)),
        "unleased boundary must be rejected"
    );
}

#[test]
fn shipped_index_serves_only_attached_leased_boundaries() {
    let mut srv = SyncServer::new();
    let boundary = BoundaryId {
        height: 12,
        app_hash: StateRoot([6u8; 32]),
    };
    let host = Host::genesis(vec![]).unwrap();
    let coords = BoundaryCoords::default();
    let ask = |srv: &mut SyncServer, req| block_on(srv.handle(&host, None, &coords, req));

    // unleased boundary: refused like every other per-boundary request.
    let resp = ask(
        &mut srv,
        SyncRequest::IndexModules { boundary },
    );
    assert!(matches!(resp, SyncResponse::Error(_)));

    srv.insert_capture_for_test(boundary);
    srv.lease(boundary);

    // leased but unattached: an EMPTY list — the joiner falls back to the
    // from-state rebuild, no error.
    let resp = ask(&mut srv, SyncRequest::IndexModules { boundary });
    assert!(matches!(
        resp,
        SyncResponse::IndexModules { ref entries } if entries.is_empty()
    ));
    assert!(!srv.index_attached(boundary));

    // attach two blobs; one larger than a chunk to exercise offset paging.
    let big = vec![0xEE; statesync::CHUNK_LEN + 7];
    let mut blobs = std::collections::BTreeMap::new();
    blobs.insert("chat".to_string(), big.clone());
    blobs.insert("_blocks".to_string(), vec![1, 2, 3]);
    srv.attach_index(boundary, blobs).expect("attach");
    assert!(srv.index_attached(boundary));

    let resp = ask(&mut srv, SyncRequest::IndexModules { boundary });
    match resp {
        SyncResponse::IndexModules { entries } => assert_eq!(
            entries,
            vec![
                ("_blocks".to_string(), 3),
                ("chat".to_string(), big.len() as u64)
            ]
        ),
        other => panic!("want IndexModules, got {}", other.kind_name()),
    }

    // chunked fetch reassembles the exact blob.
    let mut out = Vec::new();
    loop {
        let resp = ask(
            &mut srv,
            SyncRequest::IndexChunk {
                boundary,
                db: "chat".into(),
                offset: out.len() as u64,
            },
        );
        match resp {
            SyncResponse::Chunk { total, bytes } => {
                assert_eq!(total, big.len() as u64);
                out.extend_from_slice(&bytes);
                if out.len() as u64 >= total {
                    break;
                }
            }
            other => panic!("want Chunk, got {}", other.kind_name()),
        }
    }
    assert_eq!(out, big);

    // a database the source never attached is a loud error.
    let resp = ask(
        &mut srv,
        SyncRequest::IndexChunk {
            boundary,
            db: "ghost".into(),
            offset: 0,
        },
    );
    assert!(matches!(resp, SyncResponse::Error(_)));

    // attaching to an unleased boundary is refused.
    let stale = BoundaryId {
        height: 99,
        app_hash: StateRoot([7u8; 32]),
    };
    assert!(srv.attach_index(stale, Default::default()).is_err());
}

#[test]
fn pruned_pinned_range_errors_for_refetch() {
    let err = statesync::qmdb::module_lane_error(
        "kv",
        "historical proof failed: operation pruned: 5".to_string(),
    );

    assert!(
        matches!(err, SyncError::Pruned { ref module, .. } if module == "kv"),
        "pruned pinned range must become a typed refetch error, got {err:?}"
    );
}

#[test]
fn lease_retention_floor_uses_oldest_active_resolver_start() {
    let mut srv = SyncServer::new();
    let newer = BoundaryId {
        height: 20,
        app_hash: StateRoot([2u8; 32]),
    };
    let older = BoundaryId {
        height: 10,
        app_hash: StateRoot([1u8; 32]),
    };

    srv.insert_resolver_capture_for_test(newer, "kv", 50);
    srv.insert_resolver_capture_for_test(older, "kv", 25);
    srv.lease(newer);
    srv.lease(older);

    assert_eq!(
        srv.oldest_active_lease_start_for_module("kv"),
        Some(25),
        "retention floor must honor the oldest leased pinned range"
    );

    srv.release(older);
    assert_eq!(srv.oldest_active_lease_start_for_module("kv"), Some(50));
}
