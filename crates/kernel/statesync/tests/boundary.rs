use commonware_cryptography::sha256::Digest as Sha256Digest;
use futures::executor::block_on;
use host::Host;
use sdk::StateRoot;
use statesync::{
    BoundaryCoords, BoundaryId, MAX_CAPTURES, Manifest, ManifestEntry, PayloadKind, ResolverTarget,
    ServeStep, SyncError, SyncRequest, SyncResponse, SyncServer, TipCoords, decode_request,
    decode_response, encode_request, encode_response,
};

const DEGRADED_ID: &str = "degraded";
const HEALTHY_ID: &str = "healthy";

struct HealthyModule;

#[async_trait::async_trait(?Send)]
impl sdk::Module for HealthyModule {
    fn id(&self) -> sdk::ModuleId {
        HEALTHY_ID.into()
    }

    fn root(&self) -> StateRoot {
        StateRoot([1u8; 32])
    }

    fn state_sync_handle(&self) -> Result<sdk::StateSyncHandle, sdk::Error> {
        Ok(sdk::StateSyncHandle::SnapshotBytes(vec![42]))
    }

    async fn execute(
        &mut self,
        _ctx: &mut dyn sdk::Ctx,
        _msg: &sdk::Msg,
    ) -> Result<(), sdk::Error> {
        Ok(())
    }
}

struct DegradedModule;

#[async_trait::async_trait(?Send)]
impl sdk::Module for DegradedModule {
    fn id(&self) -> sdk::ModuleId {
        DEGRADED_ID.into()
    }

    fn root(&self) -> StateRoot {
        StateRoot([2u8; 32])
    }

    fn state_sync_handle(&self) -> Result<sdk::StateSyncHandle, sdk::Error> {
        Err(sdk::Error::Module("no pack for committed head".into()))
    }

    async fn execute(
        &mut self,
        _ctx: &mut dyn sdk::Ctx,
        _msg: &sdk::Msg,
    ) -> Result<(), sdk::Error> {
        Ok(())
    }
}

#[test]
fn a_module_that_cannot_serve_is_refused_per_module_not_by_the_whole_boundary() {
    // the joiner's answer to "one module cannot serve": it still gets the
    // boundary and every other module's payload, and that ONE module comes
    // back Unsupported. before, the module's error aborted the capture and
    // this node could not admit anyone at all.
    let host =
        Host::genesis(vec![Box::new(HealthyModule), Box::new(DegradedModule)]).expect("genesis");
    let coords = BoundaryCoords {
        epoch: 1,
        view_base: 0,
        participants: vec![],
        residents: vec![],
        floor_cert: None,
    };
    let finalized = host::FinalizedBlock {
        height: 12,
        root_hash: host.root_hash(),
    };

    let (id, data) = block_on(statesync::capture_boundary(&host, finalized, &coords))
        .expect("a degraded module must not take the boundary down");

    let mut srv = SyncServer::new();
    srv.install_capture(id, data);
    let SyncResponse::Manifest(manifest) = srv.manifest_for(id).expect("manifest") else {
        panic!("manifest_for must answer with a manifest");
    };

    let healthy = manifest.entry(HEALTHY_ID).expect("healthy entry");
    assert_eq!(healthy.kind, PayloadKind::Snapshot);
    let degraded = manifest.entry(DEGRADED_ID).expect("degraded entry");
    assert_eq!(degraded.kind, PayloadKind::Unsupported);
    assert_eq!(
        degraded.root,
        StateRoot([2u8; 32]),
        "the committed root is still known — only the transfer surface is gone",
    );
}

#[test]
fn captures_keyed_by_boundary_id_not_height() {
    let mut srv = SyncServer::new();
    let b1 = BoundaryId {
        height: 32,
        root_hash: StateRoot([1u8; 32]),
    };
    let b2 = BoundaryId {
        height: 32,
        root_hash: StateRoot([2u8; 32]),
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
        root_hash: StateRoot([9u8; 32]),
    };
    srv.insert_capture_for_test(held);
    srv.lease(held);

    for h in 100..100 + (MAX_CAPTURES as u64) + 3 {
        let b = BoundaryId {
            height: h,
            root_hash: StateRoot([(h % 251) as u8; 32]),
        };
        srv.insert_capture_for_test(b);
    }

    assert!(srv.has_capture(held), "leased boundary must not be evicted");
}

#[test]
fn fresh_install_serves_its_manifest_under_full_lease_pressure() {
    // the self-eviction regression: with every cache slot's capture leased
    // (leases only age out by overflow, never by client release), a newly
    // installed boundary was the sole unleased entry — install-time eviction
    // removed it before its own manifest_for could lease it, and every
    // manifest fetch at a fresh boundary failed with "no capture at boundary
    // N (refetch manifest)".
    let mut srv = SyncServer::new();
    for h in 1..=(MAX_CAPTURES as u64) {
        let b = BoundaryId {
            height: h,
            root_hash: StateRoot([(h % 251) as u8; 32]),
        };
        srv.install_capture_for_test(b);
        srv.lease(b);
    }

    let tip = BoundaryId {
        height: 1000,
        root_hash: StateRoot([42u8; 32]),
    };
    srv.install_capture_for_test(tip);
    let manifest = srv.manifest_for(tip);
    assert!(
        matches!(manifest, Ok(SyncResponse::Manifest(_))),
        "a fresh install must serve its own manifest even when every older \
         capture holds a lease: {manifest:?}",
    );
    assert!(
        srv.known_boundaries().len() <= MAX_CAPTURES,
        "manifest_for's lease must rebalance the cache back under its cap",
    );
}

#[test]
fn leased_boundaries_are_bounded_and_oldest_is_released() {
    let mut srv = SyncServer::new();
    let mut leased = Vec::new();

    for h in 1..=(MAX_CAPTURES as u64) + 2 {
        let boundary = BoundaryId {
            height: h,
            root_hash: StateRoot([(h % 251) as u8; 32]),
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
fn tip_coords_roundtrip_over_the_wire() {
    let req_bytes = encode_request(&SyncRequest::TipCoords);
    assert!(matches!(
        decode_request(&req_bytes),
        Ok(SyncRequest::TipCoords)
    ));

    let coords = TipCoords {
        height: 1880,
        root_hash: StateRoot([7u8; 32]),
        epoch: 3,
        view_base: 1800,
        participants: vec![vec![1u8; 32], vec![2u8; 32]],
        residents: vec![vec![3u8; 32]],
        has_floor: true,
    };
    let bytes = encode_response(&SyncResponse::TipCoords(coords.clone()));
    let SyncResponse::TipCoords(back) = decode_response(&bytes).unwrap() else {
        panic!("tip coords response expected");
    };
    assert_eq!(back, coords);

    // empty sets ride the same wire — a fresh net has no residents yet.
    let bare = TipCoords {
        residents: Vec::new(),
        has_floor: false,
        ..coords
    };
    let bytes = encode_response(&SyncResponse::TipCoords(bare.clone()));
    let SyncResponse::TipCoords(back) = decode_response(&bytes).unwrap() else {
        panic!("tip coords response expected");
    };
    assert_eq!(back, bare);
}

#[test]
fn tip_coords_request_never_touches_the_capture_cache() {
    // the detection lane's whole point: a TipCoords request routes to the
    // state owner (NeedCoords) without leasing or installing anything —
    // routine resident polling must not churn the join-shaped capture cache.
    let mut srv = SyncServer::new();
    assert!(matches!(
        srv.serve(SyncRequest::TipCoords),
        ServeStep::NeedCoords
    ));
    assert!(srv.known_boundaries().is_empty());
    assert_eq!(srv.leased_count_for_test(), 0);
}

#[test]
fn manifest_roundtrip_carries_pinned_resolver_target() {
    let m = Manifest {
        height: 77,
        root_hash: StateRoot([4u8; 32]),
        epoch: 2,
        view_base: 70,
        participants: vec![vec![3u8; 32]],
        residents: vec![],
        floor_cert: Some(vec![0xCC; 96]),
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
        root_hash: StateRoot([3u8; 32]),
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
        root_hash: StateRoot([6u8; 32]),
    };
    let host = Host::genesis(vec![]).unwrap();
    let coords = BoundaryCoords::default();
    let ask = |srv: &mut SyncServer, req| block_on(srv.handle(&host, None, &coords, req));

    // unleased boundary: refused like every other per-boundary request.
    let resp = ask(&mut srv, SyncRequest::IndexModules { boundary });
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
        root_hash: StateRoot([7u8; 32]),
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
