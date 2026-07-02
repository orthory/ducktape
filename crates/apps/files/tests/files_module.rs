use futures::executor::block_on;
use host::{BlockContext, Host};
use sdk::{Ctx, Effect, Env, Error, Event, Module, Msg, Origin, StateRoot};
use sha2::{Digest as _, Sha256};

use files::Files;
use files_interface::{
    FilesMsg, FilesQuery, FilesReply, FilesSyncReq, FilesSyncResp, Manifest, decode_reply,
    decode_sync_resp, encode_msg, encode_query, encode_sync_req,
};

const FILES: &str = "files";

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().into()
}

fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// deterministic chunk bytes + their lowercase-hex sha256 digest.
fn chunk(seed: &[u8]) -> (Vec<u8>, String) {
    (seed.to_vec(), to_hex(&sha256(seed)))
}

struct TestCtx {
    env: Env,
}

impl TestCtx {
    fn new(origin: Origin, height: u64) -> Self {
        Self {
            env: Env {
                height,
                consensus_time: height,
                origin,
                me: FILES.into(),
            },
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Ctx for TestCtx {
    fn env(&self) -> &Env {
        &self.env
    }

    fn module_root(&self, _target: &str) -> Option<StateRoot> {
        None
    }

    async fn query(&self, _target: &str, _req: &[u8]) -> Result<Vec<u8>, Error> {
        Err(Error::QueryUnsupported)
    }

    fn emit_msg(&mut self, _msg: Msg) {}
    fn emit_event(&mut self, _event: Event) {}
    fn request_effect(&mut self, _effect: Effect) {}
}

fn add_msg(
    file_id: &str,
    name: &str,
    mime: &str,
    size: u64,
    chunk_size: u64,
    chunks: Vec<String>,
) -> Msg {
    Msg {
        target: FILES.into(),
        payload: encode_msg(&FilesMsg::AddManifest {
            file_id: file_id.into(),
            name: name.into(),
            mime: mime.into(),
            size,
            chunk_size,
            chunks,
        }),
    }
}

fn remove_msg(file_id: &str) -> Msg {
    Msg {
        target: FILES.into(),
        payload: encode_msg(&FilesMsg::RemoveManifest {
            file_id: file_id.into(),
        }),
    }
}

/// a valid two-chunk baseline: chunk_size 4096, size 5000 -> ceil = 2 chunks.
fn valid_add() -> (Msg, Vec<u8>, Vec<u8>, String, String) {
    let (b0, d0) = chunk(b"chunk-zero");
    let (b1, d1) = chunk(b"chunk-one");
    let msg = add_msg(
        "doc/a",
        "a.txt",
        "text/plain",
        5000,
        4096,
        vec![d0.clone(), d1.clone()],
    );
    (msg, b0, b1, d0, d1)
}

async fn stat(files: &Files, file_id: &str) -> Option<Manifest> {
    match decode_reply(
        &files
            .query(&encode_query(&FilesQuery::Stat {
                file_id: file_id.into(),
            }))
            .await
            .expect("query stat"),
    )
    .expect("decode reply")
    {
        FilesReply::Stat(m) => m,
        other => panic!("expected Stat, got {other:?}"),
    }
}

async fn list(files: &Files, prefix: &str, limit: u64) -> Vec<Manifest> {
    match decode_reply(
        &files
            .query(&encode_query(&FilesQuery::List {
                prefix: prefix.into(),
                limit,
            }))
            .await
            .expect("query list"),
    )
    .expect("decode reply")
    {
        FilesReply::List(m) => m,
        other => panic!("expected List, got {other:?}"),
    }
}

fn valid_chunks() -> Vec<String> {
    let (_, d0) = chunk(b"chunk-zero");
    let (_, d1) = chunk(b"chunk-one");
    vec![d0, d1]
}

#[test]
fn validation_table_rejects_every_bad_manifest() {
    block_on(async {
        let cs = valid_chunks();
        let long_hex = "a".repeat(64);
        let cases: Vec<(&str, Msg)> = vec![
            (
                "empty file_id",
                add_msg("", "n", "m", 5000, 4096, cs.clone()),
            ),
            (
                "file_id too long",
                add_msg(&"x".repeat(257), "n", "m", 5000, 4096, cs.clone()),
            ),
            ("empty name", add_msg("f", "", "m", 5000, 4096, cs.clone())),
            (
                "name too long",
                add_msg("f", &"x".repeat(513), "m", 5000, 4096, cs.clone()),
            ),
            (
                "mime too long",
                add_msg("f", "n", &"x".repeat(129), 5000, 4096, cs.clone()),
            ),
            (
                "chunk_size below min",
                add_msg("f", "n", "m", 100, 4095, vec![long_hex.clone()]),
            ),
            (
                "chunk_size above max",
                add_msg(
                    "f",
                    "n",
                    "m",
                    100,
                    4 * 1024 * 1024 + 1,
                    vec![long_hex.clone()],
                ),
            ),
            ("empty chunks", add_msg("f", "n", "m", 5000, 4096, vec![])),
            (
                "too many chunks",
                add_msg("f", "n", "m", 1, 4096, vec![long_hex.clone(); 4097]),
            ),
            (
                "chunk count mismatch",
                add_msg("f", "n", "m", 100, 4096, cs.clone()),
            ),
            (
                "digest not 64 chars",
                add_msg("f", "n", "m", 10, 4096, vec!["abc".into()]),
            ),
            (
                "digest uppercase (not lowercase hex)",
                add_msg("f", "n", "m", 10, 4096, vec!["A".repeat(64)]),
            ),
            (
                "digest non-hex char",
                add_msg("f", "n", "m", 10, 4096, vec!["g".repeat(64)]),
            ),
        ];

        for (label, msg) in cases {
            let mut files = Files::new(FILES);
            let err = files
                .execute(&mut TestCtx::new(Origin::System, 1), &msg)
                .await
                .expect_err(label);
            assert!(matches!(err, Error::Module(_)), "{label}: {err:?}");
            files.commit_block().await.expect("commit");
            assert_eq!(
                files.root(),
                Files::new(FILES).root(),
                "{label}: a rejected op must not enter the root preimage"
            );
        }
    });
}

#[test]
fn accepts_single_chunk_at_span_boundary() {
    block_on(async {
        let (_, d0) = chunk(b"only");
        let mut files = Files::new(FILES);
        // n=1, chunk_size=4096: size in (0, 4096] is valid.
        files
            .execute(
                &mut TestCtx::new(Origin::System, 1),
                &add_msg("f", "n", "m", 4096, 4096, vec![d0]),
            )
            .await
            .expect("full single chunk is valid");
        files.commit_block().await.expect("commit");
        assert!(stat(&files, "f").await.is_some());
    });
}

#[test]
fn digest_is_sha256_of_concatenated_chunk_digests() {
    block_on(async {
        let (msg, _b0, _b1, d0, d1) = valid_add();
        let mut files = Files::new(FILES);
        files
            .execute(&mut TestCtx::new(Origin::System, 7), &msg)
            .await
            .expect("add");
        files.commit_block().await.expect("commit");

        let manifest = stat(&files, "doc/a").await.expect("manifest");

        let mut raw = Vec::new();
        raw.extend_from_slice(&hex32(&d0));
        raw.extend_from_slice(&hex32(&d1));
        let expected = to_hex(&sha256(&raw));

        assert_eq!(manifest.digest, expected, "digest-of-digests mismatch");
        assert_eq!(manifest.created_at_height, 7);
        assert_eq!(manifest.owner, "system");
    });
}

fn hex32(s: &str) -> [u8; 32] {
    let bytes = s.as_bytes();
    let mut out = [0u8; 32];
    for (i, slot) in out.iter_mut().enumerate() {
        let hi = (bytes[2 * i] as char).to_digit(16).unwrap() as u8;
        let lo = (bytes[2 * i + 1] as char).to_digit(16).unwrap() as u8;
        *slot = (hi << 4) | lo;
    }
    out
}

#[test]
fn owner_derives_from_origin_and_gates_remove() {
    block_on(async {
        let (msg, ..) = valid_add();
        let mut files = Files::new(FILES);
        files
            .execute(
                &mut TestCtx::new(Origin::External(b"alice".to_vec()), 1),
                &msg,
            )
            .await
            .expect("alice adds");
        files.commit_block().await.expect("commit add");

        let manifest = stat(&files, "doc/a").await.expect("manifest");
        assert_eq!(manifest.owner, to_hex(b"alice"));

        // wrong external origin may not remove.
        let err = files
            .execute(
                &mut TestCtx::new(Origin::External(b"bob".to_vec()), 2),
                &remove_msg("doc/a"),
            )
            .await
            .expect_err("bob must not remove alice's manifest");
        assert!(matches!(err, Error::Module(_)), "{err:?}");

        // a module/system origin may not remove an external-owned manifest.
        let err = files
            .execute(&mut TestCtx::new(Origin::System, 2), &remove_msg("doc/a"))
            .await
            .expect_err("system must not remove alice's manifest");
        assert!(matches!(err, Error::Module(_)), "{err:?}");

        // the stored owner may.
        files
            .execute(
                &mut TestCtx::new(Origin::External(b"alice".to_vec()), 3),
                &remove_msg("doc/a"),
            )
            .await
            .expect("alice removes");
        files.commit_block().await.expect("commit remove");
        assert!(stat(&files, "doc/a").await.is_none());
    });
}

#[test]
fn duplicate_file_id_rejected() {
    block_on(async {
        let (msg, ..) = valid_add();
        let mut files = Files::new(FILES);
        files
            .execute(&mut TestCtx::new(Origin::System, 1), &msg)
            .await
            .expect("first add");
        // second add in the SAME block is rejected via the staged overlay.
        let err = files
            .execute(&mut TestCtx::new(Origin::System, 1), &valid_add().0)
            .await
            .expect_err("duplicate in-block");
        assert!(matches!(err, Error::Module(_)), "{err:?}");
        files.commit_block().await.expect("commit");
        // and across blocks against committed state.
        let err = files
            .execute(&mut TestCtx::new(Origin::System, 2), &valid_add().0)
            .await
            .expect_err("duplicate across blocks");
        assert!(matches!(err, Error::Module(_)), "{err:?}");
    });
}

#[test]
fn blob_store_round_trips_and_digest_matches() {
    let mut files = Files::new(FILES);
    let bytes = b"hello blob world".to_vec();
    let digest = files.put_chunk(bytes.clone());
    assert_eq!(digest, sha256(&bytes), "put_chunk keys by sha256");
    assert!(files.has_chunk(&digest));
    assert_eq!(files.get_chunk(&digest), Some(bytes.as_slice()));
    assert!(!files.has_chunk(&sha256(b"absent")));
    assert_eq!(files.get_chunk(&sha256(b"absent")), None);
}

#[test]
fn serve_sync_serves_chunks_and_receiver_detects_tampering() {
    block_on(async {
        let (b0, d0) = chunk(b"real chunk bytes");
        let mut files = Files::new(FILES);
        // commit a manifest whose first chunk digest is d0.
        let (_, d1) = chunk(b"second");
        files
            .execute(
                &mut TestCtx::new(Origin::System, 1),
                &add_msg("f", "n", "m", 5000, 4096, vec![d0.clone(), d1]),
            )
            .await
            .expect("add");
        files.commit_block().await.expect("commit");
        // the node holds the body bytes off-consensus.
        files.put_chunk(b0.clone());

        let committed = stat(&files, "f").await.expect("manifest").chunks[0].clone();

        // honest serve: present, and the bytes verify against the committed digest.
        let resp = files
            .serve_sync(&encode_sync_req(&FilesSyncReq::GetChunk {
                digest: committed.clone(),
            }))
            .await
            .expect("serve");
        let FilesSyncResp::Chunk { present, bytes } = decode_sync_resp(&resp).expect("decode resp");
        assert!(present);
        assert_eq!(
            to_hex(&sha256(&bytes)),
            committed,
            "receiver verifies fetched bytes against committed digest"
        );

        // dishonest serve: flip a byte; the receiver's re-hash no longer matches.
        let mut tampered = bytes.clone();
        tampered[0] ^= 0xff;
        assert_ne!(
            to_hex(&sha256(&tampered)),
            committed,
            "a flipped byte fails receiver verification"
        );

        // absent chunk: present=false, empty bytes.
        let (_, missing) = chunk(b"never uploaded");
        let resp = files
            .serve_sync(&encode_sync_req(&FilesSyncReq::GetChunk {
                digest: missing,
            }))
            .await
            .expect("serve absent");
        let FilesSyncResp::Chunk { present, bytes } = decode_sync_resp(&resp).expect("decode resp");
        assert!(!present);
        assert!(bytes.is_empty());
    });
}

#[test]
fn list_filters_by_prefix_and_clamps_limit() {
    block_on(async {
        let (_, d0) = chunk(b"c");
        let mut files = Files::new(FILES);
        for i in 0..300u32 {
            files
                .execute(
                    &mut TestCtx::new(Origin::System, 1),
                    &add_msg(&format!("bulk/{i:04}"), "n", "m", 1, 4096, vec![d0.clone()]),
                )
                .await
                .expect("add bulk");
        }
        files
            .execute(
                &mut TestCtx::new(Origin::System, 1),
                &add_msg("other/x", "n", "m", 1, 4096, vec![d0.clone()]),
            )
            .await
            .expect("add other");
        files.commit_block().await.expect("commit");

        // prefix filter excludes "other/x".
        let all_bulk = list(&files, "bulk/", 1000).await;
        assert_eq!(all_bulk.len(), 256, "limit clamps to 256");
        assert!(all_bulk.iter().all(|m| m.file_id.starts_with("bulk/")));
        // sorted ascending by file_id.
        assert_eq!(all_bulk[0].file_id, "bulk/0000");

        let three = list(&files, "bulk/", 3).await;
        assert_eq!(three.len(), 3);

        let other = list(&files, "other/", 10).await;
        assert_eq!(other.len(), 1);
        assert_eq!(other[0].file_id, "other/x");
    });
}

#[test]
fn commit_and_abort_staging_semantics() {
    block_on(async {
        let (msg, ..) = valid_add();
        let mut files = Files::new(FILES);
        let root0 = files.root();

        files
            .execute(&mut TestCtx::new(Origin::System, 1), &msg)
            .await
            .expect("stage add");
        assert_eq!(files.root(), root0, "staged add must not move the root");
        assert!(
            stat(&files, "doc/a").await.is_none(),
            "queries read committed state only"
        );

        files.commit_block().await.expect("commit add");
        let root1 = files.root();
        assert_ne!(root1, root0, "commit moves the root");
        assert!(stat(&files, "doc/a").await.is_some());

        // stage a remove, then abort: the manifest survives.
        files
            .execute(&mut TestCtx::new(Origin::System, 2), &remove_msg("doc/a"))
            .await
            .expect("stage remove");
        assert_eq!(files.root(), root1, "staged remove must not move the root");
        files.abort_block().await.expect("abort remove");
        assert_eq!(files.root(), root1, "abort keeps the root byte-identical");
        assert!(stat(&files, "doc/a").await.is_some());

        // remove + commit: the only manifest is gone, root returns to empty.
        files
            .execute(&mut TestCtx::new(Origin::System, 3), &remove_msg("doc/a"))
            .await
            .expect("stage remove");
        files.commit_block().await.expect("commit remove");
        assert!(stat(&files, "doc/a").await.is_none());
        assert_eq!(
            files.root(),
            root0,
            "removing the last manifest restores the empty root"
        );
    });
}

#[test]
fn root_is_unaffected_by_blob_store_contents() {
    block_on(async {
        let (msg, ..) = valid_add();
        let mut files = Files::new(FILES);
        files
            .execute(&mut TestCtx::new(Origin::System, 1), &msg)
            .await
            .expect("add");
        files.commit_block().await.expect("commit");

        let before = files.root();
        files.put_chunk(b"some body bytes".to_vec());
        files.put_chunk(b"more body bytes".to_vec());
        assert_eq!(
            files.root(),
            before,
            "blob-store contents are off-consensus and must not move root()"
        );
    });
}

#[test]
fn snapshot_install_round_trips_and_root_is_stable() {
    block_on(async {
        let mut source = Files::new(FILES);
        let (_, d0) = chunk(b"a");
        let (_, d1) = chunk(b"b");
        source
            .execute(
                &mut TestCtx::new(Origin::External(b"alice".to_vec()), 4),
                &add_msg(
                    "doc/a",
                    "a.txt",
                    "text/plain",
                    5000,
                    4096,
                    vec![d0.clone(), d1.clone()],
                ),
            )
            .await
            .expect("add a");
        source
            .execute(
                &mut TestCtx::new(Origin::System, 4),
                &add_msg(
                    "doc/b",
                    "b.bin",
                    "application/octet-stream",
                    10,
                    4096,
                    vec![d0.clone()],
                ),
            )
            .await
            .expect("add b");
        source.commit_block().await.expect("commit");

        let expected = source.root();
        assert_eq!(expected, source.root(), "root() is deterministic");

        let bytes = source.snapshot();
        let mut target = Files::new(FILES);
        target
            .install(&bytes, expected)
            .expect("install verified snapshot");

        assert_eq!(target.root(), expected);
        assert_eq!(stat(&target, "doc/a").await, stat(&source, "doc/a").await);
        assert_eq!(stat(&target, "doc/b").await, stat(&source, "doc/b").await);

        // a wrong expected root is rejected.
        let mut bad = Files::new(FILES);
        assert!(bad.install(&bytes, StateRoot::ZERO).is_err());
    });
}

#[test]
fn host_dispatch_moves_app_hash_and_serves_query() {
    block_on(async {
        let mut host = Host::genesis(vec![Box::new(Files::new(FILES))]).expect("genesis");
        let app0 = host.app_hash();
        let (msg, ..) = valid_add();

        let out = host
            .submit_at(
                BlockContext {
                    height: 9,
                    consensus_time: 9,
                    origin: Origin::External(b"tester".to_vec()),
                },
                msg,
            )
            .await
            .expect("submit add");
        assert_ne!(out.app_hash, app0, "add must move the app-hash");
        assert_eq!(out.app_hash, host.app_hash());

        let reply = host
            .query(
                FILES,
                &encode_query(&FilesQuery::Stat {
                    file_id: "doc/a".into(),
                }),
            )
            .await
            .expect("host query");
        let manifest = match decode_reply(&reply).expect("decode") {
            FilesReply::Stat(m) => m.expect("manifest present"),
            other => panic!("expected Stat, got {other:?}"),
        };
        assert_eq!(manifest.owner, to_hex(b"tester"));
        assert_eq!(manifest.created_at_height, 9);
    });
}
