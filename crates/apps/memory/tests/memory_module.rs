//! module-level behavior of the memory workspace: path normalization,
//! write-once publish generations, origin-derived authorship, execute-time
//! caps, the progressive-disclosure read verbs (ls/stat/read/find/grep),
//! snapshot retention across deletes, watch fan-out, commit/abort staging, and
//! the snapshot/install sync boundary.

use std::collections::BTreeMap;

use files::Files;
use files_interface::{
    FilesMsg, FilesQuery, FilesReply, Manifest, decode_reply as files_decode_reply,
    digest_hex as file_digest_hex, encode_msg as files_encode_msg,
    encode_query as files_encode_query, encode_reply as files_encode_reply,
};
use futures::executor::block_on;
use host::{BlockContext, Host};
use memory::Memory;
use memory_interface::{
    Body, FileStat, Generation, GrepHit, LsEntry, MAX_BODY_BYTES, MAX_FILES,
    MAX_GENERATIONS_PER_PATH, MAX_GREP_LINE_BYTES, MAX_META_ENTRIES, MAX_SNAPSHOTS, MAX_WATCHES,
    MemoryEvent, MemoryMsg, MemoryQuery, MemoryReply, Meta, PublishBody, decode_event,
    decode_reply, encode_msg, encode_query,
};
use sdk::{Ctx, Error, Event, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};

const FILES: &str = "files";
const MEMORY: &str = "memory";

struct TestCtx {
    env: sdk::Env,
    /// module ids `module_root` reports as registered (watch targets).
    known_modules: Vec<String>,
    file_manifests: BTreeMap<String, Manifest>,
    /// follow-up msgs emitted during execute, in order.
    emitted: Vec<Msg>,
}

impl TestCtx {
    fn with_origin(height: u64, origin: Origin) -> Self {
        Self {
            env: sdk::Env { protocol_version: 0,
                height,
                consensus_time: 0,
                origin,
                me: MEMORY.into(),
            },
            known_modules: Vec::new(),
            file_manifests: BTreeMap::new(),
            emitted: Vec::new(),
        }
    }

    fn at(height: u64) -> Self {
        Self::with_origin(height, Origin::System)
    }

    fn knowing(mut self, module_id: &str) -> Self {
        self.known_modules.push(module_id.to_string());
        self
    }

    fn with_file_manifest(mut self, manifest: Manifest) -> Self {
        self.file_manifests
            .insert(manifest.file_id.clone(), manifest);
        self
    }
}

#[async_trait::async_trait(?Send)]
impl Ctx for TestCtx {
    fn env(&self) -> &sdk::Env {
        &self.env
    }

    fn module_root(&self, target: &str) -> Option<StateRoot> {
        self.known_modules
            .iter()
            .any(|m| m == target)
            .then_some(StateRoot::ZERO)
    }

    async fn query(&self, target: &str, req: &[u8]) -> Result<Vec<u8>, Error> {
        if target != FILES {
            return Err(Error::QueryUnsupported);
        }
        let reply = match files_interface::decode_query(req).map_err(Error::Module)? {
            FilesQuery::Stat { file_id } => {
                FilesReply::Stat(self.file_manifests.get(&file_id).cloned())
            }
            FilesQuery::List { .. } => FilesReply::List(Vec::new()),
        };
        Ok(files_encode_reply(&reply))
    }

    fn emit_msg(&mut self, msg: Msg) {
        self.emitted.push(msg);
    }
    fn emit_event(&mut self, _ev: Event) {}
    fn request_effect(&mut self, _eff: sdk::Effect) {}
}

fn module_msg(payload: MemoryMsg) -> Msg {
    Msg {
        target: MEMORY.into(),
        payload: encode_msg(&payload),
    }
}

fn publish(path: &str, body: &str) -> MemoryMsg {
    MemoryMsg::Publish {
        path: path.into(),
        body: PublishBody::Inline(body.into()),
        meta: Meta::new(),
    }
}

fn publish_meta(path: &str, body: &str, meta: &[(&str, &str)]) -> MemoryMsg {
    MemoryMsg::Publish {
        path: path.into(),
        body: PublishBody::Inline(body.into()),
        meta: meta
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    }
}

fn publish_file(path: &str, file_id: &str) -> MemoryMsg {
    MemoryMsg::Publish {
        path: path.into(),
        body: PublishBody::File {
            file_id: file_id.into(),
        },
        meta: Meta::new(),
    }
}

fn inline_body(record: &Generation) -> &str {
    match &record.body {
        Body::Inline(body) => body,
        other => panic!("expected inline body, got {other:?}"),
    }
}

fn file_body(record: &Generation) -> (&str, &str, u64) {
    match &record.body {
        Body::File {
            file_id,
            digest,
            size,
        } => (file_id, digest, *size),
        other => panic!("expected file body, got {other:?}"),
    }
}

fn file_manifest(file_id: &str, size: u64) -> Manifest {
    let chunk_digest = file_digest_hex(&vec![b'x'; size as usize]);
    let mut raw = Vec::new();
    raw.extend_from_slice(&hex32(&chunk_digest));
    Manifest {
        file_id: file_id.into(),
        name: format!("{file_id}.bin"),
        mime: "application/octet-stream".into(),
        size,
        chunk_size: size,
        chunks: vec![chunk_digest],
        digest: file_digest_hex(&raw),
        owner: "system".into(),
        created_at_height: 1,
    }
}

fn hex32(s: &str) -> [u8; 32] {
    let bytes = s.as_bytes();
    assert_eq!(bytes.len(), 64, "digest fixture length");
    let mut out = [0u8; 32];
    for (i, slot) in out.iter_mut().enumerate() {
        let hi = (bytes[2 * i] as char).to_digit(16).unwrap() as u8;
        let lo = (bytes[2 * i + 1] as char).to_digit(16).unwrap() as u8;
        *slot = (hi << 4) | lo;
    }
    out
}

async fn query(module: &Memory, req: MemoryQuery) -> MemoryReply {
    let reply = module.query(&encode_query(&req)).await.unwrap();
    decode_reply(&reply).unwrap()
}

async fn stat_of(module: &Memory, path: &str) -> Option<FileStat> {
    match query(module, MemoryQuery::Stat { path: path.into() }).await {
        MemoryReply::Stat(stat) => stat,
        other => panic!("unexpected reply: {other:?}"),
    }
}

async fn read(
    module: &Memory,
    path: &str,
    generation: Option<u64>,
    snapshot: Option<&str>,
) -> Option<memory_interface::Generation> {
    match query(
        module,
        MemoryQuery::Read {
            path: path.into(),
            generation,
            snapshot: snapshot.map(str::to_string),
        },
    )
    .await
    {
        MemoryReply::Read(record) => record,
        other => panic!("unexpected reply: {other:?}"),
    }
}

async fn grep(module: &Memory, prefix: &str, pattern: &str, limit: u64) -> Vec<GrepHit> {
    match query(
        module,
        MemoryQuery::Grep {
            prefix: prefix.into(),
            pattern: pattern.into(),
            limit,
        },
    )
    .await
    {
        MemoryReply::Grep(hits) => hits,
        other => panic!("unexpected reply: {other:?}"),
    }
}

async fn find(
    module: &Memory,
    prefix: &str,
    meta_filter: &[(&str, &str)],
    limit: u64,
) -> Vec<FileStat> {
    match query(
        module,
        MemoryQuery::Find {
            prefix: prefix.into(),
            meta_filter: meta_filter
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            limit,
        },
    )
    .await
    {
        MemoryReply::Find(stats) => stats,
        other => panic!("unexpected reply: {other:?}"),
    }
}

fn files_module_msg(payload: FilesMsg) -> Msg {
    Msg {
        target: FILES.into(),
        payload: files_encode_msg(&payload),
    }
}

fn add_manifest(file_id: &str, bytes: &[u8]) -> FilesMsg {
    assert!(bytes.len() >= 4096, "files module chunk_size minimum");
    FilesMsg::AddManifest {
        file_id: file_id.into(),
        name: format!("{file_id}.bin"),
        mime: "application/octet-stream".into(),
        size: bytes.len() as u64,
        chunk_size: bytes.len() as u64,
        chunks: vec![file_digest_hex(bytes)],
    }
}

fn block_ctx(height: u64) -> BlockContext {
    BlockContext { protocol_version: 0,
        height,
        consensus_time: height,
        origin: Origin::System,
    }
}

async fn host_files_stat(host: &Host, file_id: &str) -> Option<Manifest> {
    let bytes = host
        .query(
            FILES,
            &files_encode_query(&FilesQuery::Stat {
                file_id: file_id.into(),
            }),
        )
        .await
        .expect("files stat");
    match files_decode_reply(&bytes).expect("files reply") {
        FilesReply::Stat(manifest) => manifest,
        other => panic!("unexpected files reply: {other:?}"),
    }
}

async fn host_memory_query(host: &Host, req: MemoryQuery) -> MemoryReply {
    let bytes = host
        .query(MEMORY, &encode_query(&req))
        .await
        .expect("memory query");
    decode_reply(&bytes).expect("memory reply")
}

async fn host_read(
    host: &Host,
    path: &str,
    generation: Option<u64>,
    snapshot: Option<&str>,
) -> Option<Generation> {
    match host_memory_query(
        host,
        MemoryQuery::Read {
            path: path.into(),
            generation,
            snapshot: snapshot.map(str::to_string),
        },
    )
    .await
    {
        MemoryReply::Read(record) => record,
        other => panic!("unexpected memory reply: {other:?}"),
    }
}

async fn host_stat(host: &Host, path: &str) -> Option<FileStat> {
    match host_memory_query(host, MemoryQuery::Stat { path: path.into() }).await {
        MemoryReply::Stat(stat) => stat,
        other => panic!("unexpected memory reply: {other:?}"),
    }
}

async fn host_grep(host: &Host, prefix: &str, pattern: &str, limit: u64) -> Vec<GrepHit> {
    match host_memory_query(
        host,
        MemoryQuery::Grep {
            prefix: prefix.into(),
            pattern: pattern.into(),
            limit,
        },
    )
    .await
    {
        MemoryReply::Grep(hits) => hits,
        other => panic!("unexpected memory reply: {other:?}"),
    }
}

#[test]
fn path_normalization_accepts_canonical_and_rejects_malformed() {
    block_on(async {
        let mut module = Memory::new(MEMORY, FILES);
        let root0 = module.root();

        let long_segment_ok = format!("/{}", "s".repeat(128));
        // four segments (126+126+128+128 bytes) + four slashes: the full path
        // lands exactly on the 512-byte cap.
        let max_path_ok = format!(
            "/{}/{}/{}/{}",
            "a".repeat(126),
            "b".repeat(126),
            "c".repeat(128),
            "d".repeat(128),
        );
        assert_eq!(max_path_ok.len(), 512);
        let valid = [
            "/a",
            "/a/b/c",
            "/skills/review",
            "/UPPER/MiXeD.case-file_v2",
            long_segment_ok.as_str(),
            max_path_ok.as_str(),
        ];
        for path in valid {
            module
                .execute(&mut TestCtx::at(1), &module_msg(publish(path, "body")))
                .await
                .unwrap_or_else(|e| panic!("{path} must be accepted: {e}"));
        }
        module.commit_block().await.unwrap();
        for path in valid {
            assert!(stat_of(&module, path).await.is_some(), "{path} must exist");
        }

        let over_segment = format!("/{}", "s".repeat(129));
        let over_path = format!("/{}/{}", "a".repeat(126), "b".repeat(400));
        assert!(over_path.len() > 512);
        let invalid = [
            "",        // empty
            "/",       // the root is not a file
            "a/b",     // relative
            "/a/",     // trailing slash
            "//a",     // empty segment (leading)
            "/a//b",   // empty segment (inner)
            "/a/./b",  // `.` segment
            "/a/../b", // `..` segment
            "/.",      // bare `.`
            over_segment.as_str(),
            over_path.as_str(),
        ];
        let root1 = module.root();
        for path in invalid {
            let err = module
                .execute(&mut TestCtx::at(2), &module_msg(publish(path, "body")))
                .await
                .expect_err(&format!("{path:?} must be rejected"));
            assert!(matches!(err, Error::Module(_)));
            module.abort_block().await.unwrap();
            assert_eq!(module.root(), root1, "rejected path {path:?} left a trace");
        }

        // the same normalization gates Delete and the file-path read verbs.
        for msg in [
            MemoryMsg::Delete { path: "/a/".into() },
            MemoryMsg::Delete { path: "/".into() },
        ] {
            let err = module
                .execute(&mut TestCtx::at(3), &module_msg(msg))
                .await
                .unwrap_err();
            assert!(matches!(err, Error::Module(_)));
            module.abort_block().await.unwrap();
        }
        let err = module
            .query(&encode_query(&MemoryQuery::Stat { path: "/".into() }))
            .await
            .unwrap_err();
        assert!(
            matches!(err, Error::Module(_)),
            "Stat on the root is not a file"
        );

        assert_ne!(module.root(), root0, "the valid publishes committed");
    });
}

#[test]
fn publish_assigns_monotonic_generations_and_keeps_them_immutable() {
    block_on(async {
        let mut module = Memory::new(MEMORY, FILES);

        for (height, body) in [(1u64, "v1"), (2, "v2"), (5, "v3")] {
            module
                .execute(
                    &mut TestCtx::at(height),
                    &module_msg(publish_meta("/doc", body, &[("rev", body)])),
                )
                .await
                .unwrap();
            module.commit_block().await.unwrap();
        }

        let stat = stat_of(&module, "/doc").await.expect("file exists");
        assert_eq!(stat.latest_generation, 3);
        assert_eq!(stat.generations, 3);
        assert_eq!(stat.latest_published_at_height, 5);
        assert_eq!(stat.body_len, 2);

        // every generation stays readable and byte-identical after later writes.
        for (g, body, height) in [(1u64, "v1", 1u64), (2, "v2", 2), (3, "v3", 5)] {
            let record = read(&module, "/doc", Some(g), None).await.expect("gen");
            assert_eq!(record.generation, g);
            assert_eq!(inline_body(&record), body);
            assert_eq!(record.published_at_height, height);
            assert_eq!(record.meta.get("rev").map(String::as_str), Some(body));
        }
        // latest == generation 3; out-of-range generations read as None.
        assert_eq!(
            inline_body(&read(&module, "/doc", None, None).await.unwrap()),
            "v3"
        );
        assert!(read(&module, "/doc", Some(0), None).await.is_none());
        assert!(read(&module, "/doc", Some(4), None).await.is_none());
        assert!(read(&module, "/missing", None, None).await.is_none());
    });
}

#[test]
fn author_derives_from_origin_and_cannot_be_spoofed() {
    block_on(async {
        let mut module = Memory::new(MEMORY, FILES);
        let root0 = module.root();

        // the demo-default empty external origin never passes.
        let err = module
            .execute(
                &mut TestCtx::with_origin(1, Origin::External(Vec::new())),
                &module_msg(publish("/a", "x")),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        module.abort_block().await.unwrap();
        assert_eq!(module.root(), root0);

        // authorship is never in the payload: external = domain-separated
        // "ext:" + hex, module = id verbatim, system = "system".
        module
            .execute(
                &mut TestCtx::with_origin(1, Origin::External(vec![0xab, 0x01, 0xff])),
                &module_msg(publish("/user", "x")),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::with_origin(1, Origin::Module("agent".into())),
                &module_msg(publish("/module", "x")),
            )
            .await
            .unwrap();
        module
            .execute(&mut TestCtx::at(1), &module_msg(publish("/system", "x")))
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        for (path, author) in [
            ("/user", "ext:ab01ff"),
            ("/module", "agent"),
            ("/system", "system"),
        ] {
            let record = read(&module, path, None, None).await.expect("record");
            assert_eq!(record.author, author);
            let stat = stat_of(&module, path).await.expect("stat");
            assert_eq!(stat.latest_author, author);
        }
    });
}

#[test]
fn caps_reject_oversized_bodies_and_meta_before_staging() {
    block_on(async {
        let mut module = Memory::new(MEMORY, FILES);
        module
            .execute(&mut TestCtx::at(1), &module_msg(publish("/ok", "small")))
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let root = module.root();

        let oversized_meta: Meta = (0..MAX_META_ENTRIES + 1)
            .map(|i| (format!("k{i}"), "v".into()))
            .collect();
        let rejects = [
            publish("/big", &"x".repeat(MAX_BODY_BYTES + 1)),
            MemoryMsg::Publish {
                path: "/meta".into(),
                body: PublishBody::Inline("x".into()),
                meta: oversized_meta,
            },
            publish_meta("/meta", "x", &[(&"k".repeat(65), "v")]),
            publish_meta("/meta", "x", &[("k", &"v".repeat(257))]),
        ];
        for msg in rejects {
            let err = module
                .execute(&mut TestCtx::at(2), &module_msg(msg))
                .await
                .unwrap_err();
            assert!(matches!(err, Error::Module(_)));
            module.abort_block().await.unwrap();
            assert_eq!(module.root(), root, "a rejected write leaves no trace");
        }

        // at-cap values pass: the caps are inclusive bounds.
        module
            .execute(
                &mut TestCtx::at(3),
                &module_msg(publish("/exact", &"x".repeat(MAX_BODY_BYTES))),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::at(3),
                &module_msg(publish_meta(
                    "/exact-meta",
                    "x",
                    &[(&"k".repeat(64), &"v".repeat(256))],
                )),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        assert_eq!(
            stat_of(&module, "/exact").await.unwrap().body_len,
            MAX_BODY_BYTES as u64
        );
    });
}

#[test]
fn caps_bound_generations_and_file_count() {
    block_on(async {
        let mut module = Memory::new(MEMORY, FILES);

        // generations: 1024 publishes to one path pass, the 1025th is rejected.
        for _ in 0..MAX_GENERATIONS_PER_PATH {
            module
                .execute(&mut TestCtx::at(1), &module_msg(publish("/gen", "b")))
                .await
                .unwrap();
        }
        module.commit_block().await.unwrap();
        let err = module
            .execute(&mut TestCtx::at(2), &module_msg(publish("/gen", "b")))
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        module.abort_block().await.unwrap();
        assert_eq!(
            stat_of(&module, "/gen").await.unwrap().latest_generation,
            MAX_GENERATIONS_PER_PATH
        );

        // files: fill to the cap (one already exists), then reject the overflow.
        for i in 0..MAX_FILES - 1 {
            module
                .execute(
                    &mut TestCtx::at(3),
                    &module_msg(publish(&format!("/f/{i}"), "")),
                )
                .await
                .unwrap();
        }
        module.commit_block().await.unwrap();
        let err = module
            .execute(&mut TestCtx::at(4), &module_msg(publish("/overflow", "")))
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        module.abort_block().await.unwrap();

        // an EXISTING file still accepts new generations at the file cap, and
        // deleting one frees a slot.
        module
            .execute(&mut TestCtx::at(5), &module_msg(publish("/f/0", "update")))
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::at(5),
                &module_msg(MemoryMsg::Delete {
                    path: "/f/1".into(),
                }),
            )
            .await
            .unwrap();
        module
            .execute(&mut TestCtx::at(5), &module_msg(publish("/overflow", "")))
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        assert!(stat_of(&module, "/overflow").await.is_some());
    });
}

#[test]
fn caps_bound_snapshots_and_watches() {
    block_on(async {
        let mut module = Memory::new(MEMORY, FILES);
        module
            .execute(&mut TestCtx::at(1), &module_msg(publish("/a", "x")))
            .await
            .unwrap();

        for i in 0..MAX_SNAPSHOTS {
            module
                .execute(
                    &mut TestCtx::at(1),
                    &module_msg(MemoryMsg::Snapshot {
                        name: format!("s{i}"),
                    }),
                )
                .await
                .unwrap();
        }
        module.commit_block().await.unwrap();
        for (msg, why) in [
            (
                MemoryMsg::Snapshot {
                    name: "over".into(),
                },
                "cap",
            ),
            (MemoryMsg::Snapshot { name: "s0".into() }, "duplicate name"),
            (
                MemoryMsg::Snapshot {
                    name: String::new(),
                },
                "empty name",
            ),
            (
                MemoryMsg::Snapshot {
                    name: "n".repeat(129),
                },
                "name byte cap",
            ),
        ] {
            let err = module
                .execute(&mut TestCtx::at(2), &module_msg(msg))
                .await
                .expect_err(why);
            assert!(matches!(err, Error::Module(_)));
            module.abort_block().await.unwrap();
        }

        for i in 0..MAX_WATCHES {
            module
                .execute(
                    &mut TestCtx::at(3).knowing("agent"),
                    &module_msg(MemoryMsg::RegisterWatch {
                        prefix: format!("/w{i}"),
                        module_id: "agent".into(),
                    }),
                )
                .await
                .unwrap();
        }
        module.commit_block().await.unwrap();
        let err = module
            .execute(
                &mut TestCtx::at(4).knowing("agent"),
                &module_msg(MemoryMsg::RegisterWatch {
                    prefix: "/overflow".into(),
                    module_id: "agent".into(),
                }),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        module.abort_block().await.unwrap();

        // re-registering an existing watch is an idempotent no-op, not a cap hit.
        module
            .execute(
                &mut TestCtx::at(5).knowing("agent"),
                &module_msg(MemoryMsg::RegisterWatch {
                    prefix: "/w0".into(),
                    module_id: "agent".into(),
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
    });
}

async fn ls(module: &Memory, path: &str, limit: u64) -> Vec<LsEntry> {
    match query(
        module,
        MemoryQuery::Ls {
            path: path.into(),
            limit,
        },
    )
    .await
    {
        MemoryReply::Ls(entries) => entries,
        other => panic!("unexpected reply: {other:?}"),
    }
}

#[test]
fn ls_lists_implicit_dirs_and_files_sorted() {
    block_on(async {
        let mut module = Memory::new(MEMORY, FILES);
        for path in ["/a/b/c", "/a/b/d", "/a/x", "/top", "/a"] {
            module
                .execute(&mut TestCtx::at(1), &module_msg(publish(path, "body")))
                .await
                .unwrap();
        }
        module.commit_block().await.unwrap();

        // "/a" is BOTH a file and an implied dir: the file entry wins in the
        // listing, and the dir remains listable directly.
        let rendered: Vec<(String, bool)> = ls(&module, "/", 256)
            .await
            .iter()
            .map(|e| match e {
                LsEntry::Dir { path } => (path.clone(), true),
                LsEntry::File(stat) => (stat.path.clone(), false),
            })
            .collect();
        assert_eq!(
            rendered,
            vec![("/a".into(), false), ("/top".into(), false)],
            "root lists its direct children sorted, file shadowing the dir"
        );

        assert_eq!(
            ls(&module, "/a", 256).await,
            vec![
                LsEntry::Dir {
                    path: "/a/b".into()
                },
                LsEntry::File(stat_of(&module, "/a/x").await.unwrap()),
            ]
        );

        let entries = ls(&module, "/a/b", 256).await;
        let files: Vec<&str> = entries
            .iter()
            .map(|e| match e {
                LsEntry::File(stat) => stat.path.as_str(),
                LsEntry::Dir { path } => panic!("unexpected dir {path}"),
            })
            .collect();
        assert_eq!(files, ["/a/b/c", "/a/b/d"]);

        // a leaf file and a missing dir both list as empty.
        for path in ["/a/b/c", "/ghost"] {
            assert!(
                ls(&module, path, 256).await.is_empty(),
                "{path} must list empty"
            );
        }
    });
}

#[test]
fn ls_clamps_the_limit_and_pages_in_sorted_order() {
    block_on(async {
        let mut module = Memory::new(MEMORY, FILES);
        // 257 children under one dir: one more than the clamp.
        for i in 0..257 {
            module
                .execute(
                    &mut TestCtx::at(1),
                    &module_msg(publish(&format!("/d/f{i:03}"), "")),
                )
                .await
                .unwrap();
        }
        module.commit_block().await.unwrap();

        let all = ls(&module, "/d", 1_000).await;
        assert_eq!(all.len(), 256, "an oversized limit clamps to 256");

        let page = ls(&module, "/d", 5).await;
        let paths: Vec<&str> = page
            .iter()
            .map(|e| match e {
                LsEntry::File(stat) => stat.path.as_str(),
                LsEntry::Dir { path } => panic!("unexpected dir {path}"),
            })
            .collect();
        assert_eq!(
            paths,
            ["/d/f000", "/d/f001", "/d/f002", "/d/f003", "/d/f004"],
            "a small limit takes the sorted-first entries"
        );
        assert!(ls(&module, "/d", 0).await.is_empty());
    });
}

#[test]
fn read_resolves_latest_generation_and_snapshot_exclusively() {
    block_on(async {
        let mut module = Memory::new(MEMORY, FILES);
        module
            .execute(&mut TestCtx::at(1), &module_msg(publish("/doc", "v1")))
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        module
            .execute(
                &mut TestCtx::at(2),
                &module_msg(MemoryMsg::Snapshot { name: "pin".into() }),
            )
            .await
            .unwrap();
        module
            .execute(&mut TestCtx::at(2), &module_msg(publish("/doc", "v2")))
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        assert_eq!(
            inline_body(&read(&module, "/doc", None, None).await.unwrap()),
            "v2"
        );
        assert_eq!(
            inline_body(&read(&module, "/doc", Some(1), None).await.unwrap()),
            "v1"
        );
        // the snapshot was taken BEFORE v2 (same block, but the pin captured
        // the committed latest = generation 1).
        assert_eq!(
            inline_body(&read(&module, "/doc", None, Some("pin")).await.unwrap()),
            "v1"
        );
        assert!(read(&module, "/doc", None, Some("ghost")).await.is_none());

        // generation and snapshot are mutually exclusive.
        let err = module
            .query(&encode_query(&MemoryQuery::Read {
                path: "/doc".into(),
                generation: Some(1),
                snapshot: Some("pin".into()),
            }))
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
    });
}

#[test]
fn snapshot_pins_survive_delete_and_drop_releases_retention() {
    block_on(async {
        let mut module = Memory::new(MEMORY, FILES);
        let empty_root = module.root();

        module
            .execute(&mut TestCtx::at(1), &module_msg(publish("/doc", "v1")))
            .await
            .unwrap();
        module
            .execute(&mut TestCtx::at(1), &module_msg(publish("/doc", "v2")))
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        module
            .execute(
                &mut TestCtx::at(2),
                &module_msg(MemoryMsg::Snapshot { name: "s1".into() }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        // delete: the live file vanishes, but the pinned generation (2) stays
        // readable through the snapshot. generation 1 (pinned by nothing) drops.
        module
            .execute(
                &mut TestCtx::at(3),
                &module_msg(MemoryMsg::Delete {
                    path: "/doc".into(),
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        assert!(stat_of(&module, "/doc").await.is_none());
        assert!(read(&module, "/doc", None, None).await.is_none());
        let pinned = read(&module, "/doc", None, Some("s1")).await.expect("pin");
        assert_eq!(inline_body(&pinned), "v2");
        assert_eq!(pinned.generation, 2);

        // deleting a missing file is a rejection.
        let err = module
            .execute(
                &mut TestCtx::at(4),
                &module_msg(MemoryMsg::Delete {
                    path: "/doc".into(),
                }),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        module.abort_block().await.unwrap();

        // a re-created path continues ABOVE the pinned generation: (path, 2)
        // stays a stable hash-pinned reference to the snapshot's record.
        module
            .execute(&mut TestCtx::at(5), &module_msg(publish("/doc", "v3")))
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let stat = stat_of(&module, "/doc").await.expect("recreated");
        assert_eq!(stat.latest_generation, 3);
        assert_eq!(stat.generations, 1, "the new incarnation owns only gen 3");
        assert!(
            read(&module, "/doc", Some(2), None).await.is_none(),
            "the pinned predecessor generation is only reachable via the snapshot"
        );
        assert_eq!(
            inline_body(&read(&module, "/doc", None, Some("s1")).await.unwrap()),
            "v2"
        );

        // dropping the snapshot releases the retained record; deleting the live
        // file then leaves the namespace byte-identical to genesis.
        module
            .execute(
                &mut TestCtx::at(6),
                &module_msg(MemoryMsg::DropSnapshot { name: "s1".into() }),
            )
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::at(6),
                &module_msg(MemoryMsg::Delete {
                    path: "/doc".into(),
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        assert!(read(&module, "/doc", None, Some("s1")).await.is_none());
        assert_eq!(
            module.root(),
            empty_root,
            "released pins must leave no retained bytes in the root preimage"
        );

        // dropping a missing snapshot is a rejection.
        let err = module
            .execute(
                &mut TestCtx::at(7),
                &module_msg(MemoryMsg::DropSnapshot { name: "s1".into() }),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        module.abort_block().await.unwrap();
    });
}

#[test]
fn file_publish_pins_manifest_digest_and_survives_manifest_removal() {
    block_on(async {
        let mut host = Host::genesis(vec![
            Box::new(Files::new(FILES)),
            Box::new(Memory::new(MEMORY, FILES)),
        ])
        .expect("genesis");
        let file_id = "large/body";
        let bytes = vec![0x42; 4096];

        host.submit_at(
            block_ctx(1),
            files_module_msg(add_manifest(file_id, &bytes)),
        )
        .await
        .expect("add manifest");
        let manifest = host_files_stat(&host, file_id).await.expect("manifest");

        host.submit_at(block_ctx(2), module_msg(publish_file("/large", file_id)))
            .await
            .expect("publish file body");

        let record = host_read(&host, "/large", None, None)
            .await
            .expect("record");
        assert_eq!(
            file_body(&record),
            (
                manifest.file_id.as_str(),
                manifest.digest.as_str(),
                manifest.size
            ),
            "memory stores the manifest digest/size at publish time"
        );
        assert_eq!(
            host_stat(&host, "/large").await.unwrap().body_len,
            manifest.size
        );

        host.submit_at(
            block_ctx(3),
            files_module_msg(FilesMsg::RemoveManifest {
                file_id: file_id.into(),
            }),
        )
        .await
        .expect("remove manifest");
        assert!(
            host_files_stat(&host, file_id).await.is_none(),
            "the files manifest is gone"
        );

        let pinned = host_read(&host, "/large", None, None)
            .await
            .expect("pinned generation");
        assert_eq!(
            file_body(&pinned),
            (
                manifest.file_id.as_str(),
                manifest.digest.as_str(),
                manifest.size
            ),
            "manifest removal must not rewrite or invalidate the generation"
        );
    });
}

#[test]
fn file_publish_rejects_missing_manifest_before_staging() {
    block_on(async {
        let mut host = Host::genesis(vec![
            Box::new(Files::new(FILES)),
            Box::new(Memory::new(MEMORY, FILES)),
        ])
        .expect("genesis");
        let root = host.module_root(MEMORY).expect("memory root");

        host.submit_at(block_ctx(1), module_msg(publish_file("/missing", "ghost")))
            .await
            .expect_err("missing manifest rejects the publish");

        assert_eq!(
            host.module_root(MEMORY).expect("memory root"),
            root,
            "the rejected file publish leaves no memory trace"
        );
        assert!(host_read(&host, "/missing", None, None).await.is_none());
    });
}

#[test]
fn grep_skips_file_bodies_and_stats_use_their_sizes() {
    block_on(async {
        let mut host = Host::genesis(vec![
            Box::new(Files::new(FILES)),
            Box::new(Memory::new(MEMORY, FILES)),
        ])
        .expect("genesis");
        let file_id = "notes/blob";
        let bytes = vec![0x77; 8192];

        host.submit_at(
            block_ctx(1),
            files_module_msg(add_manifest(file_id, &bytes)),
        )
        .await
        .expect("add manifest");
        let manifest = host_files_stat(&host, file_id).await.expect("manifest");
        host.submit_at(
            block_ctx(2),
            module_msg(publish_file("/notes/blob", file_id)),
        )
        .await
        .expect("publish file body");
        host.submit_at(
            block_ctx(3),
            module_msg(publish("/notes/inline", "needle is inline")),
        )
        .await
        .expect("publish inline sibling");

        assert_eq!(
            host_grep(&host, "/notes", "needle", 100)
                .await
                .iter()
                .map(|hit| hit.path.as_str())
                .collect::<Vec<_>>(),
            ["/notes/inline"],
            "grep scans inline siblings and silently skips file bodies"
        );
        assert_eq!(
            host_stat(&host, "/notes/blob").await.unwrap().body_len,
            manifest.size
        );
        assert_eq!(
            host_stat(&host, "/notes/inline").await.unwrap().body_len,
            "needle is inline".len() as u64
        );

        let MemoryReply::Ls(entries) = host_memory_query(
            &host,
            MemoryQuery::Ls {
                path: "/notes".into(),
                limit: 10,
            },
        )
        .await
        else {
            panic!("expected ls reply");
        };
        let lengths: Vec<(&str, u64)> = entries
            .iter()
            .map(|entry| match entry {
                LsEntry::File(stat) => (stat.path.as_str(), stat.body_len),
                LsEntry::Dir { path } => panic!("unexpected dir {path}"),
            })
            .collect();
        assert_eq!(
            lengths,
            vec![
                ("/notes/blob", manifest.size),
                ("/notes/inline", "needle is inline".len() as u64),
            ]
        );
    });
}

#[test]
fn snapshot_pin_retains_file_body_generation_after_delete() {
    block_on(async {
        let manifest = file_manifest("snapshot/blob", 8192);
        let mut module = Memory::new(MEMORY, FILES);

        module
            .execute(
                &mut TestCtx::at(1).with_file_manifest(manifest.clone()),
                &module_msg(publish_file("/blob", &manifest.file_id)),
            )
            .await
            .expect("publish file body");
        module.commit_block().await.unwrap();
        module
            .execute(
                &mut TestCtx::at(2),
                &module_msg(MemoryMsg::Snapshot { name: "pin".into() }),
            )
            .await
            .expect("snapshot");
        module
            .execute(
                &mut TestCtx::at(2),
                &module_msg(MemoryMsg::Delete {
                    path: "/blob".into(),
                }),
            )
            .await
            .expect("delete");
        module.commit_block().await.unwrap();

        assert!(stat_of(&module, "/blob").await.is_none());
        let pinned = read(&module, "/blob", None, Some("pin"))
            .await
            .expect("snapshot pin");
        assert_eq!(
            file_body(&pinned),
            (
                manifest.file_id.as_str(),
                manifest.digest.as_str(),
                manifest.size
            )
        );
    });
}

#[test]
fn find_filters_on_latest_meta_and_discovers_skills() {
    block_on(async {
        let mut module = Memory::new(MEMORY, FILES);
        for msg in [
            publish_meta(
                "/skills/deploy",
                "how to deploy",
                &[("kind", "skill"), ("lang", "rust")],
            ),
            publish_meta("/skills/review", "how to review", &[("kind", "skill")]),
            publish_meta("/skills/readme", "not a skill", &[]),
            publish_meta("/notes/a", "a note", &[("kind", "note")]),
        ] {
            module
                .execute(&mut TestCtx::at(1), &module_msg(msg))
                .await
                .unwrap();
        }
        module.commit_block().await.unwrap();

        // THE discovery query: skills under /skills/ with kind=skill.
        let skills = find(&module, "/skills/", &[("kind", "skill")], 100).await;
        assert_eq!(
            skills.iter().map(|s| s.path.as_str()).collect::<Vec<_>>(),
            ["/skills/deploy", "/skills/review"],
            "sorted, meta-filtered skill discovery"
        );

        // every filter pair must match; an empty filter lists the prefix.
        let rust = find(
            &module,
            "/skills/",
            &[("kind", "skill"), ("lang", "rust")],
            100,
        )
        .await;
        assert_eq!(rust.len(), 1);
        assert_eq!(rust[0].path, "/skills/deploy");
        assert_eq!(
            find(&module, "/skills/", &[], 100).await.len(),
            3,
            "no filter = all files under the prefix"
        );
        assert!(
            find(&module, "/skills/", &[("kind", "SKILL")], 100)
                .await
                .is_empty()
        );
        assert_eq!(find(&module, "/skills/", &[], 2).await.len(), 2, "limit");

        // the filter matches the LATEST meta only: republishing without the
        // kind tag removes the file from discovery.
        module
            .execute(
                &mut TestCtx::at(2),
                &module_msg(publish_meta("/skills/review", "v2", &[])),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let skills = find(&module, "/skills/", &[("kind", "skill")], 100).await;
        assert_eq!(
            skills.iter().map(|s| s.path.as_str()).collect::<Vec<_>>(),
            ["/skills/deploy"]
        );
    });
}

#[test]
fn grep_returns_cited_hits_deterministically() {
    block_on(async {
        let mut module = Memory::new(MEMORY, FILES);
        let ops = [
            publish("/notes/b", "no match here\nquack once\nplain\nquack twice"),
            publish("/notes/a", "first line\nsecond quack line"),
            publish("/other/c", "quack elsewhere"),
        ];
        for msg in ops.clone() {
            module
                .execute(&mut TestCtx::at(1), &module_msg(msg))
                .await
                .unwrap();
        }
        // a superseded generation must NOT be scanned.
        module
            .execute(
                &mut TestCtx::at(1),
                &module_msg(publish("/notes/a", "first line\nsecond quack line\nthird")),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        let hits = grep(&module, "/notes", "quack", 100).await;
        assert_eq!(
            hits,
            vec![
                GrepHit {
                    uri: "duck://memory/notes/a@2#L2".into(),
                    path: "/notes/a".into(),
                    generation: 2,
                    line: 2,
                    text: "second quack line".into(),
                },
                GrepHit {
                    uri: "duck://memory/notes/b@1#L2".into(),
                    path: "/notes/b".into(),
                    generation: 1,
                    line: 2,
                    text: "quack once".into(),
                },
                GrepHit {
                    uri: "duck://memory/notes/b@1#L4".into(),
                    path: "/notes/b".into(),
                    generation: 1,
                    line: 4,
                    text: "quack twice".into(),
                },
            ],
            "path-sorted, 1-indexed, latest-generation-only, cited hits"
        );

        // case-sensitive substring semantics — no regex, no case folding.
        assert!(grep(&module, "/notes", "QUACK", 100).await.is_empty());
        assert_eq!(grep(&module, "/notes", "quack", 2).await.len(), 2, "limit");

        // hit text truncates to 256 BYTES on a char boundary (85 3-byte chars).
        let long_line = "\u{2192}".repeat(90);
        module
            .execute(
                &mut TestCtx::at(2),
                &module_msg(publish("/notes/long", &long_line)),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        let hits = grep(&module, "/notes/long", "\u{2192}", 100).await;
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].text, "\u{2192}".repeat(85));
        assert!(hits[0].text.len() <= MAX_GREP_LINE_BYTES);

        // determinism: a second instance replaying the same ops greps the same.
        let mut twin = Memory::new(MEMORY, FILES);
        for msg in ops {
            twin.execute(&mut TestCtx::at(1), &module_msg(msg))
                .await
                .unwrap();
        }
        twin.execute(
            &mut TestCtx::at(1),
            &module_msg(publish("/notes/a", "first line\nsecond quack line\nthird")),
        )
        .await
        .unwrap();
        twin.commit_block().await.unwrap();
        assert_eq!(
            grep(&twin, "/notes", "quack", 100).await,
            grep(&module, "/notes", "quack", 100).await
        );
    });
}

#[test]
fn watches_fan_out_one_event_per_module_and_unregister_stops_them() {
    block_on(async {
        let mut module = Memory::new(MEMORY, FILES);

        // unknown targets and self-watches are rejected at registration time.
        let err = module
            .execute(
                &mut TestCtx::at(1),
                &module_msg(MemoryMsg::RegisterWatch {
                    prefix: "/".into(),
                    module_id: "ghost".into(),
                }),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        module.abort_block().await.unwrap();
        let err = module
            .execute(
                &mut TestCtx::at(1).knowing(MEMORY),
                &module_msg(MemoryMsg::RegisterWatch {
                    prefix: "/".into(),
                    module_id: MEMORY.into(),
                }),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        module.abort_block().await.unwrap();
        // a watch prefix must be a CANONICAL path: relative, trailing-slash,
        // and dot-segment spellings are all rejected at registration time.
        for prefix in ["skills/", "/skills/", "/skills/./x", ""] {
            let err = module
                .execute(
                    &mut TestCtx::at(1).knowing("agent"),
                    &module_msg(MemoryMsg::RegisterWatch {
                        prefix: prefix.into(),
                        module_id: "agent".into(),
                    }),
                )
                .await
                .expect_err(&format!("{prefix:?} must be rejected"));
            assert!(matches!(err, Error::Module(_)));
            module.abort_block().await.unwrap();
        }

        // agent watches /skills twice over (overlapping prefixes) and bot
        // watches everything: a publish must reach each module exactly ONCE.
        for (prefix, module_id) in [("/skills", "agent"), ("/", "agent"), ("/", "bot")] {
            module
                .execute(
                    &mut TestCtx::at(2).knowing(module_id),
                    &module_msg(MemoryMsg::RegisterWatch {
                        prefix: prefix.into(),
                        module_id: module_id.into(),
                    }),
                )
                .await
                .unwrap();
        }
        module.commit_block().await.unwrap();

        let mut ctx = TestCtx::with_origin(3, Origin::Module("publisher".into()));
        module
            .execute(
                &mut ctx,
                &module_msg(publish_meta(
                    "/skills/new",
                    "skill body",
                    &[("kind", "skill")],
                )),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        assert_eq!(ctx.emitted.len(), 2, "one follow-up per watching module");
        assert_eq!(ctx.emitted[0].target, "agent");
        assert_eq!(ctx.emitted[1].target, "bot");
        let expected = MemoryEvent::Published {
            path: "/skills/new".into(),
            generation: 1,
            meta: [("kind".to_string(), "skill".to_string())].into(),
            author: "publisher".into(),
        };
        for msg in &ctx.emitted {
            assert_eq!(decode_event(&msg.payload).unwrap(), expected);
        }

        // a publish outside agent's narrower prefix still reaches "/" watchers.
        let mut ctx = TestCtx::at(4);
        module
            .execute(&mut ctx, &module_msg(publish("/notes/n", "note")))
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        assert_eq!(
            ctx.emitted
                .iter()
                .map(|m| m.target.as_str())
                .collect::<Vec<_>>(),
            ["agent", "bot"],
            "agent's / watch still matches"
        );

        // unregistering is exact per (prefix, module): agent keeps /skills.
        for msg in [
            MemoryMsg::UnregisterWatch {
                prefix: "/".into(),
                module_id: "agent".into(),
            },
            MemoryMsg::UnregisterWatch {
                prefix: "/".into(),
                module_id: "bot".into(),
            },
            // absent watch: deterministic no-op.
            MemoryMsg::UnregisterWatch {
                prefix: "/never".into(),
                module_id: "bot".into(),
            },
        ] {
            module
                .execute(&mut TestCtx::at(5), &module_msg(msg))
                .await
                .unwrap();
        }
        let mut ctx = TestCtx::at(5);
        module
            .execute(&mut ctx, &module_msg(publish("/skills/other", "x")))
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        assert_eq!(
            ctx.emitted
                .iter()
                .map(|m| m.target.as_str())
                .collect::<Vec<_>>(),
            ["agent"],
            "only the surviving /skills watch fires"
        );
        let mut ctx = TestCtx::at(6);
        module
            .execute(&mut ctx, &module_msg(publish("/notes/quiet", "x")))
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        assert!(ctx.emitted.is_empty());
    });
}

#[test]
fn watch_matching_is_segment_aware_not_a_string_prefix() {
    block_on(async {
        let mut module = Memory::new(MEMORY, FILES);
        module
            .execute(
                &mut TestCtx::at(1).knowing("agent"),
                &module_msg(MemoryMsg::RegisterWatch {
                    prefix: "/a".into(),
                    module_id: "agent".into(),
                }),
            )
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        // "/ab" shares the string prefix but NOT the path segment: no event.
        let mut ctx = TestCtx::at(2);
        module
            .execute(&mut ctx, &module_msg(publish("/ab", "x")))
            .await
            .unwrap();
        assert!(ctx.emitted.is_empty(), "/a must not match /ab");

        // the exact path and true descendants both match.
        let mut ctx = TestCtx::at(2);
        module
            .execute(&mut ctx, &module_msg(publish("/a", "x")))
            .await
            .unwrap();
        assert_eq!(ctx.emitted.len(), 1, "/a matches itself");
        let mut ctx = TestCtx::at(2);
        module
            .execute(&mut ctx, &module_msg(publish("/a/b", "x")))
            .await
            .unwrap();
        assert_eq!(ctx.emitted.len(), 1, "/a matches /a/b");
        module.commit_block().await.unwrap();

        // the root watch matches everything, "/ab" included.
        module
            .execute(
                &mut TestCtx::at(3).knowing("bot"),
                &module_msg(MemoryMsg::RegisterWatch {
                    prefix: "/".into(),
                    module_id: "bot".into(),
                }),
            )
            .await
            .unwrap();
        let mut ctx = TestCtx::at(3);
        module
            .execute(&mut ctx, &module_msg(publish("/ab", "y")))
            .await
            .unwrap();
        module.commit_block().await.unwrap();
        assert_eq!(
            ctx.emitted
                .iter()
                .map(|m| m.target.as_str())
                .collect::<Vec<_>>(),
            ["bot"],
            "the root watch fires; the /a watch still does not"
        );
    });
}

#[test]
fn root_changes_only_after_commit_and_abort_leaves_no_trace() {
    block_on(async {
        let mut module = Memory::new(MEMORY, FILES);
        let root0 = module.root();

        module
            .execute(&mut TestCtx::at(1), &module_msg(publish("/doc", "v1")))
            .await
            .unwrap();
        assert_eq!(
            module.root(),
            root0,
            "staged writes must not move the committed root"
        );
        // queries answer from COMMITTED state only — the NoKV read verbs never
        // observe a staged overlay (unlike the tasks list projection).
        assert!(stat_of(&module, "/doc").await.is_none());

        module.commit_block().await.unwrap();
        let root1 = module.root();
        assert_ne!(root1, root0, "commit moves the root");
        assert!(stat_of(&module, "/doc").await.is_some());

        module
            .execute(&mut TestCtx::at(2), &module_msg(publish("/doc", "v2")))
            .await
            .unwrap();
        module
            .execute(
                &mut TestCtx::at(2),
                &module_msg(MemoryMsg::Delete {
                    path: "/doc".into(),
                }),
            )
            .await
            .unwrap();
        assert_eq!(module.root(), root1, "root remains committed-state only");
        module.abort_block().await.unwrap();
        assert_eq!(module.root(), root1, "abort keeps the root byte-identical");
        assert_eq!(
            inline_body(&read(&module, "/doc", None, None).await.unwrap()),
            "v1"
        );
    });
}

#[test]
fn snapshot_install_round_trips_and_rejects_tampering() {
    block_on(async {
        let mut source = Memory::new(MEMORY, FILES);
        let manifest = file_manifest("snap/blob", 8192);
        // a state exercising every encoded section: files with several
        // generations, a file-backed body, a snapshot that RETAINS a deleted
        // file's generation, and registered watches.
        source
            .execute(&mut TestCtx::at(1), &module_msg(publish("/doc", "v1")))
            .await
            .unwrap();
        source
            .execute(
                &mut TestCtx::at(1).with_file_manifest(manifest.clone()),
                &module_msg(publish_file("/blob", &manifest.file_id)),
            )
            .await
            .unwrap();
        source
            .execute(
                &mut TestCtx::at(1),
                &module_msg(publish_meta("/skills/s", "skill", &[("kind", "skill")])),
            )
            .await
            .unwrap();
        source.commit_block().await.unwrap();
        source
            .execute(
                &mut TestCtx::at(2),
                &module_msg(MemoryMsg::Snapshot { name: "pin".into() }),
            )
            .await
            .unwrap();
        source
            .execute(&mut TestCtx::at(2), &module_msg(publish("/doc", "v2")))
            .await
            .unwrap();
        source
            .execute(
                &mut TestCtx::at(2),
                &module_msg(MemoryMsg::Delete {
                    path: "/skills/s".into(),
                }),
            )
            .await
            .unwrap();
        source
            .execute(
                &mut TestCtx::at(2).knowing("agent"),
                &module_msg(MemoryMsg::RegisterWatch {
                    prefix: "/skills".into(),
                    module_id: "agent".into(),
                }),
            )
            .await
            .unwrap();
        source.commit_block().await.unwrap();

        // the module advertises self-contained snapshot bytes...
        let handle = source.state_sync_handle().expect("state-sync handle");
        let StateSyncHandle::SnapshotBytes(bytes) = handle else {
            panic!("expected SnapshotBytes");
        };
        assert_eq!(bytes, source.snapshot(), "the handle IS the root preimage");

        // ...that install on a joiner only against the matching root.
        let mut target = Memory::new(MEMORY, FILES);
        let mut tampered = bytes.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0x01;
        let err = target.install(&tampered, source.root()).unwrap_err();
        assert!(matches!(err, Error::Module(_)), "tampered bytes must fail");

        target.install(&bytes, source.root()).expect("install");
        assert_eq!(target.root(), source.root());

        // full behavioral equivalence: live reads, the retained pin, and the
        // watch all survived the transfer.
        assert_eq!(
            inline_body(&read(&target, "/doc", None, None).await.unwrap()),
            "v2"
        );
        assert_eq!(
            file_body(&read(&target, "/blob", None, None).await.unwrap()),
            (
                manifest.file_id.as_str(),
                manifest.digest.as_str(),
                manifest.size
            ),
            "the file-backed generation transferred verbatim"
        );
        assert_eq!(
            inline_body(&read(&target, "/skills/s", None, Some("pin")).await.unwrap()),
            "skill",
            "the snapshot-retained record transferred"
        );
        assert!(stat_of(&target, "/skills/s").await.is_none());
        let mut ctx = TestCtx::at(3);
        target
            .execute(&mut ctx, &module_msg(publish("/skills/x", "s")))
            .await
            .unwrap();
        target.commit_block().await.unwrap();
        assert_eq!(ctx.emitted.len(), 1, "the transferred watch still fires");

        // root stability: replaying the same ops on a fresh instance lands on
        // the identical root (canonical encoding, no incidental state).
        assert_ne!(source.root(), StateRoot::ZERO);
    });
}

// ---- hand-encoding helpers mirroring the module's canonical byte layout ----

fn push_u64(out: &mut Vec<u8>, v: u64) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn push_str(out: &mut Vec<u8>, s: &str) {
    push_u64(out, s.len() as u64);
    out.extend_from_slice(s.as_bytes());
}

/// one generation record: empty meta, "system" author, height 1.
fn push_gen(out: &mut Vec<u8>, path: &str, generation: u64, body: &str) {
    push_str(out, path);
    push_u64(out, generation);
    out.push(0); // Body::Inline
    push_str(out, body);
    push_u64(out, 0); // meta entries
    push_str(out, "system");
    push_u64(out, 1); // published_at_height
}

/// one file-backed generation record: empty meta, "system" author, height 1.
fn push_file_gen(
    out: &mut Vec<u8>,
    path: &str,
    generation: u64,
    file_id: &str,
    digest: &str,
    size: u64,
) {
    push_str(out, path);
    push_u64(out, generation);
    out.push(1); // Body::File
    push_str(out, file_id);
    push_str(out, digest);
    push_u64(out, size);
    push_u64(out, 0); // meta entries
    push_str(out, "system");
    push_u64(out, 1); // published_at_height
}

fn sha256_root(bytes: &[u8]) -> StateRoot {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    StateRoot(h.finalize().into())
}

#[test]
fn install_rejects_non_canonical_bytes_even_with_a_colluding_root() {
    // each evil image is presented WITH the root of its own bytes (the
    // colluding-root case): the byte hash passes by construction, so the
    // strict decode has to be the wall that keeps the state out.

    // duplicate (path, generation) keys — a lenient decode would silently
    // collapse them via insert-overwrite.
    let mut dup = Vec::new();
    push_u64(&mut dup, 1); // live count
    push_str(&mut dup, "/a");
    push_u64(&mut dup, 1); // first
    push_u64(&mut dup, 1); // latest
    push_u64(&mut dup, 2); // gens count
    push_gen(&mut dup, "/a", 1, "x");
    push_gen(&mut dup, "/a", 1, "y");
    push_u64(&mut dup, 0); // snapshots
    push_u64(&mut dup, 0); // watches

    // a reordered (descending) live section — same set, non-canonical order.
    let mut desc = Vec::new();
    push_u64(&mut desc, 2);
    push_str(&mut desc, "/b");
    push_u64(&mut desc, 1);
    push_u64(&mut desc, 1);
    push_str(&mut desc, "/a");
    push_u64(&mut desc, 1);
    push_u64(&mut desc, 1);
    push_u64(&mut desc, 2);
    push_gen(&mut desc, "/a", 1, "x");
    push_gen(&mut desc, "/b", 1, "y");
    push_u64(&mut desc, 0);
    push_u64(&mut desc, 0);

    // a live head whose generation records are absent.
    let mut ghost = Vec::new();
    push_u64(&mut ghost, 1);
    push_str(&mut ghost, "/a");
    push_u64(&mut ghost, 1);
    push_u64(&mut ghost, 1);
    push_u64(&mut ghost, 0); // gens
    push_u64(&mut ghost, 0); // snapshots
    push_u64(&mut ghost, 0); // watches

    // a snapshot pin referencing a missing generation record.
    let mut pin = Vec::new();
    push_u64(&mut pin, 0); // live
    push_u64(&mut pin, 0); // gens
    push_u64(&mut pin, 1); // snapshots
    push_str(&mut pin, "s");
    push_u64(&mut pin, 1); // pin count
    push_str(&mut pin, "/a");
    push_u64(&mut pin, 1);
    push_u64(&mut pin, 0); // watches

    // a non-canonical (trailing-slash) watch prefix.
    let mut watch = Vec::new();
    push_u64(&mut watch, 0);
    push_u64(&mut watch, 0);
    push_u64(&mut watch, 0);
    push_u64(&mut watch, 1);
    push_str(&mut watch, "/skills/");
    push_str(&mut watch, "agent");

    // a file body with a digest that is not 64-character lowercase hex.
    let mut bad_file_digest = Vec::new();
    push_u64(&mut bad_file_digest, 1);
    push_str(&mut bad_file_digest, "/blob");
    push_u64(&mut bad_file_digest, 1);
    push_u64(&mut bad_file_digest, 1);
    push_u64(&mut bad_file_digest, 1);
    push_file_gen(
        &mut bad_file_digest,
        "/blob",
        1,
        "blob-id",
        &"A".repeat(64),
        4096,
    );
    push_u64(&mut bad_file_digest, 0);
    push_u64(&mut bad_file_digest, 0);

    // a file body whose canonical bytes are truncated.
    let mut truncated_file_body = Vec::new();
    push_u64(&mut truncated_file_body, 1);
    push_str(&mut truncated_file_body, "/blob");
    push_u64(&mut truncated_file_body, 1);
    push_u64(&mut truncated_file_body, 1);
    push_u64(&mut truncated_file_body, 1);
    push_file_gen(
        &mut truncated_file_body,
        "/blob",
        1,
        "blob-id",
        &"a".repeat(64),
        4096,
    );
    push_u64(&mut truncated_file_body, 0);
    push_u64(&mut truncated_file_body, 0);
    truncated_file_body.pop();

    for (bytes, what) in [
        (dup, "duplicate generation keys"),
        (desc, "descending live paths"),
        (ghost, "live head without records"),
        (pin, "dangling snapshot pin"),
        (watch, "non-canonical watch prefix"),
        (bad_file_digest, "bad file body digest"),
        (truncated_file_body, "truncated file body"),
    ] {
        let colluding_root = sha256_root(&bytes);
        let empty_root = Memory::new(MEMORY, FILES).root();
        let mut target = Memory::new(MEMORY, FILES);
        let err = target
            .install(&bytes, colluding_root)
            .expect_err(&format!("{what} must be rejected"));
        assert!(matches!(err, Error::Module(_)), "{what}");
        assert_eq!(target.root(), empty_root, "{what} must not adopt anything");
    }
}

// a watcher whose execute always fails — proves publish + fan-out are atomic.
struct ExplodingWatcher;

#[async_trait::async_trait(?Send)]
impl Module for ExplodingWatcher {
    fn id(&self) -> ModuleId {
        "boom".into()
    }
    fn root(&self) -> StateRoot {
        StateRoot::ZERO
    }
    async fn execute(&mut self, _ctx: &mut dyn Ctx, _msg: &Msg) -> Result<(), Error> {
        Err(Error::Module("boom".into()))
    }
}

#[test]
fn failed_watch_follow_up_aborts_the_publish_atomically() {
    block_on(async {
        let mut host = Host::genesis(vec![
            Box::new(Memory::new(MEMORY, FILES)),
            Box::new(ExplodingWatcher),
        ])
        .expect("genesis");
        let ctx_at = |height| BlockContext { protocol_version: 0,
            height,
            consensus_time: height,
            origin: Origin::External(b"tester".to_vec()),
        };

        host.submit_at(
            ctx_at(1),
            module_msg(MemoryMsg::RegisterWatch {
                prefix: "/".into(),
                module_id: "boom".into(),
            }),
        )
        .await
        .expect("register watch");
        let root = host.module_root(MEMORY).expect("memory root");
        let app = host.app_hash();

        // the publish stages, the follow-up dispatch fails, the block aborts:
        // publish + notification are one atomic unit (P2).
        host.submit_at(ctx_at(2), module_msg(publish("/doc", "v1")))
            .await
            .expect_err("the exploding watcher must fail the block");
        assert_eq!(
            host.module_root(MEMORY).expect("memory root"),
            root,
            "failed fan-out must leave the memory root unchanged"
        );
        assert_eq!(host.app_hash(), app, "and the app-hash");
        let reply = host
            .query(
                MEMORY,
                &encode_query(&MemoryQuery::Stat {
                    path: "/doc".into(),
                }),
            )
            .await
            .expect("stat");
        assert_eq!(
            decode_reply(&reply).unwrap(),
            MemoryReply::Stat(None),
            "the staged publish must be discarded"
        );
    });
}

#[test]
fn two_instances_replaying_the_same_ops_produce_identical_roots() {
    block_on(async {
        let mut left = Memory::new("left", FILES);
        let mut right = Memory::new("right", FILES);

        let blocks: Vec<Vec<(u64, Origin, MemoryMsg)>> = vec![
            vec![
                (
                    1,
                    Origin::System,
                    publish_meta("/skills/a", "A", &[("kind", "skill")]),
                ),
                (
                    1,
                    Origin::External(vec![7; 32]),
                    publish("/notes/n", "line1\nline2"),
                ),
            ],
            vec![(2, Origin::System, MemoryMsg::Snapshot { name: "s".into() })],
            vec![
                (
                    3,
                    Origin::Module("agent".into()),
                    publish("/skills/a", "A2"),
                ),
                (
                    3,
                    Origin::System,
                    MemoryMsg::Delete {
                        path: "/notes/n".into(),
                    },
                ),
            ],
            vec![(
                4,
                Origin::System,
                MemoryMsg::DropSnapshot { name: "s".into() },
            )],
        ];
        for block in blocks {
            for (height, origin, op) in block {
                left.execute(
                    &mut TestCtx::with_origin(height, origin.clone()),
                    &module_msg(op.clone()),
                )
                .await
                .unwrap();
                right
                    .execute(&mut TestCtx::with_origin(height, origin), &module_msg(op))
                    .await
                    .unwrap();
            }
            left.commit_block().await.unwrap();
            right.commit_block().await.unwrap();
            assert_eq!(
                left.root(),
                right.root(),
                "same ops, same blocks -> byte-identical roots"
            );
        }
        assert_ne!(left.root(), StateRoot::ZERO);
    });
}
