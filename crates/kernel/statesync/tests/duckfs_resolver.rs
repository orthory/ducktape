//! the duckfs-odb resolver path, end-to-end and in-process.
//!
//! a rich source `Files` (multi-chunk file via putblob+commit, inline files, a
//! pin) and a FRESH target are wired to the generic
//! [`statesync::sync_object_possession`] driver over a source-backed
//! [`statesync::ModuleLane`]. the driver installs the boundary refs (root-
//! verified) then loops GetObjects to FULL object possession. the load-bearing
//! property: BYTES move, not just refs — the target's Stat/Ls/Read/History
//! replies come back byte-identical to the source, which is only possible once
//! every object its refs reach is present.
//!
//! this exercises the driver + the `ObjectFetch` adapter + the real duckfs
//! `serve_sync`/install/ingest wire. the ModuleLane is backed by the source
//! module directly (its `serve_sync` future is `?Send`); the kernel
//! SyncClient/mesh transport wiring is covered by the P2T5 cluster e2e.

use std::collections::BTreeMap;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use duckfs_core::objects::object_id;
use duckfs_core::{
    CHUNK_SIZE, Change, Content, FilesMsg, FilesQuery, Kind, MAX_SYNC_IDS, encode_msg,
    encode_putblob, encode_query, to_hex,
};
use files::Files;
use sdk::{Env, Error, Module as _, Msg, Origin, StateRoot};
use statesync::{ModuleLane, ObjectFetch, SyncError, sync_object_possession};

// ---- drivers ----------------------------------------------------------------

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    futures::executor::block_on(f)
}

fn open_files(dir: &tempfile::TempDir) -> Files {
    Files::open("files", dir.path().to_path_buf()).expect("open")
}

use sdk_testkit::TestCtx;

/// a minimal deterministic `Ctx` — enough to drive source commits/pins.
fn ctx(origin: Origin, height: u64) -> TestCtx {
    TestCtx::with_env(Env {
        height,
        consensus_time: height,
        origin,
        me: "files".into(),
    })
}

fn exec(f: &mut Files, origin: Origin, h: u64, op: FilesMsg) -> Result<(), Error> {
    block_on(f.execute(
        &mut ctx(origin, h),
        &Msg {
            target: "files".into(),
            payload: encode_msg(&op),
        },
    ))
}

fn putblob(f: &mut Files, h: u64, bytes: &[u8]) {
    block_on(f.execute(
        &mut ctx(Origin::System, h),
        &Msg {
            target: "files".into(),
            payload: encode_putblob(bytes),
        },
    ))
    .expect("putblob");
}

fn commit_block(f: &mut Files) {
    block_on(f.commit_block()).expect("commit_block");
}

fn put_inline(path: &str, bytes: &[u8]) -> Change {
    Change::Put {
        path: path.into(),
        exec: false,
        meta: BTreeMap::new(),
        content: Content::Inline {
            b64: STANDARD.encode(bytes),
        },
    }
}

fn put_chunks(path: &str, size: u64, chunk_hexes: Vec<String>) -> Change {
    Change::Put {
        path: path.into(),
        exec: false,
        meta: BTreeMap::new(),
        content: Content::Chunks {
            size,
            chunks: chunk_hexes,
        },
    }
}

fn chunk_hex(bytes: &[u8]) -> String {
    to_hex(&object_id(Kind::Chunk, bytes))
}

fn commit(f: &mut Files, h: u64, base: Option<&str>, changes: Vec<Change>) {
    exec(
        f,
        Origin::System,
        h,
        FilesMsg::Commit {
            base_snapshot: base.map(Into::into),
            message: "commit".into(),
            changes,
        },
    )
    .expect("commit");
}

/// seed a source spanning every object kind + a pin gc root: two inline files,
/// then a 2.5-chunk file (putblob + Chunks), then a pin of that head.
fn seed_source(f: &mut Files) {
    commit(
        f,
        1,
        None,
        vec![
            put_inline("/shared/a", b"alpha"),
            put_inline("/shared/dir/b", b"beta"),
        ],
    );
    commit_block(f);
    let s1 = f.committed_head_for_test().expect("head 1");

    let c0 = vec![0xAAu8; CHUNK_SIZE as usize];
    let c1 = vec![0xBBu8; CHUNK_SIZE as usize];
    let ct = vec![0xCCu8; (CHUNK_SIZE / 2) as usize];
    let big_size = CHUNK_SIZE * 2 + ct.len() as u64;
    putblob(f, 2, &c0);
    putblob(f, 2, &c1);
    putblob(f, 2, &ct);
    commit(
        f,
        2,
        Some(&s1),
        vec![put_chunks(
            "/shared/big",
            big_size,
            vec![chunk_hex(&c0), chunk_hex(&c1), chunk_hex(&ct)],
        )],
    );
    commit_block(f);
    let s2 = f.committed_head_for_test().expect("head 2");

    exec(
        f,
        Origin::System,
        3,
        FilesMsg::Pin {
            snapshot: s2,
            name: "release".into(),
        },
    )
    .expect("pin");
    commit_block(f);
}

fn query_bytes(f: &Files, q: FilesQuery) -> Vec<u8> {
    block_on(f.query(&encode_query(&q))).expect("query")
}

// ---- the resolver seams: a source-backed lane + a target-backed adapter -----

/// a [`ModuleLane`] that answers straight from the source module's `serve_sync`
/// (no mesh): exactly the bytes the kernel Module lane would carry.
struct SourceLane<'a> {
    source: &'a Files,
}

impl ModuleLane for SourceLane<'_> {
    async fn fetch(&self, _module_id: &str, body: Vec<u8>) -> Result<Vec<u8>, SyncError> {
        self.source
            .serve_sync(&body)
            .await
            .map_err(|e| SyncError::Server(e.to_string()))
    }
}

/// the same `ObjectFetch` adapter shape the node uses (`FilesOdb`), over the
/// target module — the driver owns the loop, this owns the duckfs wire.
struct TargetOdb<'a>(&'a mut Files);

impl ObjectFetch for TargetOdb<'_> {
    fn refs_request(&self) -> Vec<u8> {
        duckfs_core::encode_get_refs()
    }
    fn install_refs(&mut self, reply: &[u8], root: StateRoot, height: u64) -> Result<(), String> {
        let bytes = duckfs_core::decode_refs_reply(reply)?;
        self.0
            .install(&bytes, root, height)
            .map_err(|e| e.to_string())
    }
    fn missing_request(&self, limit: usize) -> Result<Option<Vec<u8>>, String> {
        let ids = self.0.missing_objects(limit).map_err(|e| e.to_string())?;
        if ids.is_empty() {
            return Ok(None);
        }
        Ok(Some(duckfs_core::encode_get_objects(&ids)))
    }
    fn ingest(&mut self, reply: &[u8]) -> Result<usize, String> {
        let batch = duckfs_core::decode_objects_reply(reply)?;
        let landed = batch.len();
        self.0.ingest_objects(&batch).map_err(|e| e.to_string())?;
        Ok(landed)
    }
    fn possession_complete(&self) -> Result<bool, String> {
        self.0.possession_complete().map_err(|e| e.to_string())
    }
}

// ---- the test ---------------------------------------------------------------

#[test]
fn duckfs_odb_resolver_reaches_full_possession_with_identical_reads() {
    const SYNC_HEIGHT: u64 = 42;

    let src_dir = tempfile::tempdir().unwrap();
    let mut source = open_files(&src_dir);
    seed_source(&mut source);
    let src_root = source.root();
    assert_ne!(src_root, StateRoot::ZERO, "source has non-trivial state");

    let tgt_dir = tempfile::tempdir().unwrap();
    let mut target = open_files(&tgt_dir);

    // drive the generic possession loop: install refs -> GetObjects -> ingest,
    // over the source-backed lane. a small batch forces MULTIPLE rounds so the
    // BFS layering + the driver's no-livelock control flow are exercised.
    let lane = SourceLane { source: &source };
    block_on(sync_object_possession(
        &lane,
        "files",
        src_root,
        SYNC_HEIGHT,
        &mut TargetOdb(&mut target),
        4,
    ))
    .expect("sync to full possession");

    // full possession, identical root, and the sync-target height persisted
    // (a fresh joiner must not persist height 0 — the Task-14 replay contract).
    assert!(
        target.possession_complete().unwrap(),
        "target holds every reachable object"
    );
    assert_eq!(target.root(), src_root, "target root equals source root");
    assert_eq!(
        target.durable_height(),
        SYNC_HEIGHT,
        "installed at the sync-target height"
    );

    // the load-bearing proof that BYTES moved (not just refs): reads that touch
    // chunk bytes come back byte-identical. an empty odb would error on Read.
    let queries = vec![
        FilesQuery::Stat {
            path: "/shared/a".into(),
            snapshot: None,
        },
        FilesQuery::Ls {
            path: "/shared".into(),
            snapshot: None,
            after: None,
            limit: 256,
        },
        FilesQuery::Read {
            path: "/shared/big".into(),
            snapshot: None,
            offset: CHUNK_SIZE - 8,
            len: 16,
        },
        FilesQuery::History { limit: 16 },
    ];
    for q in queries {
        assert_eq!(
            query_bytes(&source, q.clone()),
            query_bytes(&target, q.clone()),
            "reply byte-identical for {q:?}"
        );
    }

    // a restart right after sync recovers BOTH the root and the height from disk.
    drop(target);
    let target2 = open_files(&tgt_dir);
    assert_eq!(target2.root(), src_root, "root survives restart");
    assert_eq!(
        target2.durable_height(),
        SYNC_HEIGHT,
        "height survives restart"
    );
    assert!(target2.possession_complete().unwrap(), "objects on disk");
}

// ---- the batch cap the node threads must fit the module's serve cap ---------

#[test]
fn node_batch_cap_fits_the_serve_sync_id_ceiling() {
    // `sync_all_modules` passes `duckfs_core::MAX_SYNC_IDS` as the driver batch. a
    // MAX_SYNC_IDS-sized GetObjects request must serve WITHOUT the "too many ids"
    // rejection — pinning that the node's batch and the module's serve cap agree.
    let req = duckfs_core::encode_get_objects(&vec![[0u8; 32]; MAX_SYNC_IDS]);
    let src_dir = tempfile::tempdir().unwrap();
    let source = open_files(&src_dir);
    let resp = block_on(source.serve_sync(&req)).expect("serve at the id ceiling");
    assert!(
        duckfs_core::decode_objects_reply(&resp)
            .expect("decode")
            .is_empty(),
        "all ids absent on a fresh source"
    );
}
