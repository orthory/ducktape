//! the typed fs capability (task 16) over the module-injected `sdk::Ctx`: reads
//! ride `Ctx::query` (host-routed committed state), writes ride `emit_msg`
//! (follow-up ops under the emitter's origin). the fake `RouteCtx` here is the
//! harness `TestCtx` grown a REAL query route — it forwards `query` to a live
//! `Files` module over a tempdir, so `FsCap`'s reads round-trip against the same
//! bytes the production op/query path serves.
//!
//! the async story: `FsCap` reads are async and driven with a top-level
//! `block_on`. `Files::query` is synchronous work in an async wrapper (no real
//! await), so `RouteCtx::query` resolves it with a single `now_or_never` poll
//! rather than a nested `block_on` — nesting `block_on` inside `block_on` trips
//! futures' executor re-entry guard (the task-7 lesson).

mod harness;
use harness::{TestCtx, open_files};

use std::collections::{BTreeMap, VecDeque};
use std::future::Future;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use files::objects::object_id;
use files::{
    CHUNK_SIZE, Change, Content, EntryKindWire, Files, FilesMsg, FsCap, Kind, Notify, decode_msg,
    decode_notify, encode_msg, encode_putblob, to_hex,
};
use futures::FutureExt as _;
use sdk::{Ctx, Effect, Env, Error, Event, Module as _, Msg, Origin, StateRoot};

// ---- the fake ctx: harness TestCtx + a real query route ---------------------

/// a deterministic `Ctx` whose `query` forwards to a live `Files` module and
/// whose `emit_msg` collects intents. `env.me` is the CALLING module ("app"), so
/// `FsCap`'s default "files" target is a genuine cross-module query.
struct RouteCtx {
    env: Env,
    files: Files,
    emitted: VecDeque<Msg>,
}

impl RouteCtx {
    fn new(files: Files) -> Self {
        Self {
            env: Env {
                protocol_version: 0,
                height: 1,
                consensus_time: 1,
                origin: Origin::Module("app".into()),
                me: "app".into(),
            },
            files,
            emitted: VecDeque::new(),
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Ctx for RouteCtx {
    fn env(&self) -> &Env {
        &self.env
    }

    fn module_root(&self, _target: &str) -> Option<StateRoot> {
        None
    }

    async fn query(&self, _target: &str, req: &[u8]) -> Result<Vec<u8>, Error> {
        // route to the real module. its `query` is a pure computation in an async
        // wrapper (no await point), so ONE poll drives it to completion — this is
        // what keeps the fake route from nesting a `block_on` inside the outer
        // driver's `block_on`.
        self.files
            .query(req)
            .now_or_never()
            .expect("files query resolves synchronously")
    }

    fn emit_msg(&mut self, msg: Msg) {
        self.emitted.push_back(msg);
    }
    fn emit_event(&mut self, _event: Event) {}
    fn request_effect(&mut self, _effect: Effect) {}
}

// ---- seeding drivers (direct module execute + commit_block) -----------------

fn block_on<F: Future>(f: F) -> F::Output {
    futures::executor::block_on(f)
}

fn exec(f: &mut Files, origin: Origin, h: u64, op: FilesMsg) -> TestCtx {
    let mut ctx = TestCtx::new(origin, h);
    block_on(f.execute(
        &mut ctx,
        &Msg {
            target: "files".into(),
            payload: encode_msg(&op),
        },
    ))
    .expect("execute ok");
    ctx
}

fn commit(
    f: &mut Files,
    origin: Origin,
    h: u64,
    base: Option<&str>,
    changes: Vec<Change>,
) -> TestCtx {
    exec(
        f,
        origin,
        h,
        FilesMsg::Commit {
            base_snapshot: base.map(Into::into),
            message: "seed".into(),
            changes,
        },
    )
}

fn putblob(f: &mut Files, h: u64, bytes: &[u8]) {
    block_on(f.execute(
        &mut TestCtx::new(Origin::System, h),
        &Msg {
            target: "files".into(),
            payload: encode_putblob(bytes),
        },
    ))
    .expect("putblob ok");
}

fn commit_block(f: &mut Files) {
    block_on(f.commit_block()).expect("commit_block ok");
}

fn put_inline_change(path: &str, bytes: &[u8]) -> Change {
    Change::Put {
        path: path.into(),
        exec: false,
        meta: BTreeMap::new(),
        content: Content::Inline {
            b64: STANDARD.encode(bytes),
        },
    }
}

fn put_chunks_change(path: &str, size: u64, chunk_hexes: &[String]) -> Change {
    Change::Put {
        path: path.into(),
        exec: false,
        meta: BTreeMap::new(),
        content: Content::Chunks {
            size,
            chunks: chunk_hexes.to_vec(),
        },
    }
}

fn chunk_hex(bytes: &[u8]) -> String {
    to_hex(&object_id(Kind::Chunk, bytes))
}

// ============================================================================
// reads: stat / ls / refs round-trip typed values
// ============================================================================

#[test]
fn stat_ls_refs_round_trip_typed_values() {
    let d = tempfile::tempdir().unwrap();
    let mut f = open_files(&d);
    // seed two files in one committed snapshot.
    commit(
        &mut f,
        Origin::System,
        1,
        None,
        vec![
            put_inline_change("/shared/a.txt", b"alpha"),
            put_inline_change("/shared/b.txt", b"beta!!"),
        ],
    );
    commit_block(&mut f);
    let head = f.committed_head_for_test().expect("head after seed");

    let mut ctx = RouteCtx::new(f);
    let cap = FsCap::new(&mut ctx);

    // stat: a present file → typed EntryInfo; an absent path → None.
    let a = block_on(cap.stat("/shared/a.txt", None))
        .expect("stat ok")
        .expect("a present");
    assert_eq!(a.path, "/shared/a.txt");
    assert_eq!(a.kind, EntryKindWire::File);
    assert_eq!(a.size, 5, "\"alpha\" is 5 bytes");
    assert!(!a.exec);
    assert!(
        block_on(cap.stat("/shared/nope", None))
            .expect("stat ok")
            .is_none(),
        "absent path stats to None"
    );

    // ls: both children in name order, no next (2 < the page limit).
    let (entries, next) = block_on(cap.ls("/shared", None, None, 256)).expect("ls ok");
    assert_eq!(
        entries.iter().map(|e| e.path.as_str()).collect::<Vec<_>>(),
        vec!["/shared/a.txt", "/shared/b.txt"]
    );
    assert_eq!(next, None);

    // refs: head is the committed snapshot hex, one commit → window of 1.
    let r = block_on(cap.refs()).expect("refs ok");
    assert_eq!(r.head, Some(head));
    assert_eq!(r.window_len, 1);
    assert!(r.pins.is_empty());
}

// ============================================================================
// reads: read_all loops Read pages across a multi-page file
// ============================================================================

#[test]
fn read_all_spans_multiple_pages_byte_exact() {
    let d = tempfile::tempdir().unwrap();
    let mut f = open_files(&d);
    // a 2.5-page file (page == MAX_READ_BYTES == CHUNK_SIZE): distinct bytes per
    // region make the reassembled result byte-checkable, and > 1 page forces the
    // read_all loop to take at least three Read calls (1 MiB, 1 MiB, 0.5 MiB).
    let c0 = vec![0xAAu8; CHUNK_SIZE as usize];
    let c1 = vec![0xBBu8; CHUNK_SIZE as usize];
    let tail = vec![0xCCu8; (CHUNK_SIZE / 2) as usize];
    let size = CHUNK_SIZE * 2 + tail.len() as u64;
    putblob(&mut f, 1, &c0);
    putblob(&mut f, 1, &c1);
    putblob(&mut f, 1, &tail);
    commit(
        &mut f,
        Origin::System,
        1,
        None,
        vec![put_chunks_change(
            "/shared/big",
            size,
            &[chunk_hex(&c0), chunk_hex(&c1), chunk_hex(&tail)],
        )],
    );
    commit_block(&mut f);

    let mut expected = Vec::new();
    expected.extend_from_slice(&c0);
    expected.extend_from_slice(&c1);
    expected.extend_from_slice(&tail);

    let mut ctx = RouteCtx::new(f);
    let cap = FsCap::new(&mut ctx);
    let bytes = block_on(cap.read_all("/shared/big", None)).expect("read_all ok");
    assert_eq!(bytes.len() as u64, size, "read_all reassembled every page");
    assert_eq!(bytes, expected, "byte-exact across the page boundaries");
}

#[test]
fn read_all_empty_file_is_empty() {
    let d = tempfile::tempdir().unwrap();
    let mut f = open_files(&d);
    commit(
        &mut f,
        Origin::System,
        1,
        None,
        vec![put_chunks_change("/shared/empty", 0, &[])],
    );
    commit_block(&mut f);

    let mut ctx = RouteCtx::new(f);
    let cap = FsCap::new(&mut ctx);
    // a size-0 file is eof at offset 0 → exactly one Read page, zero bytes.
    let bytes = block_on(cap.read_all("/shared/empty", None)).expect("read_all ok");
    assert!(bytes.is_empty());
}

// ============================================================================
// reads: grep returns hits with correct evidence uris
// ============================================================================

#[test]
fn grep_returns_hits_with_correct_uris() {
    let d = tempfile::tempdir().unwrap();
    let mut f = open_files(&d);
    commit(
        &mut f,
        Origin::System,
        1,
        None,
        vec![
            put_inline_change("/shared/gb/note", b"find the needle here\nplain tail\n"),
            put_inline_change("/shared/gb/plain", b"nothing to see\n"),
        ],
    );
    commit_block(&mut f);
    let head = f.committed_head_for_test().expect("head");

    let mut ctx = RouteCtx::new(f);
    let cap = FsCap::new(&mut ctx);
    let hits = block_on(cap.grep("needle", "/shared/gb", None)).expect("grep ok");
    assert_eq!(hits.len(), 1, "only the note file matches");
    assert_eq!(hits[0].path, "/shared/gb/note");
    assert_eq!(hits[0].line, 1);
    assert_eq!(hits[0].text, "find the needle here");
    assert_eq!(
        hits[0].uri,
        format!("duck://files/shared/gb/note@{head}#L1"),
        "byte-exact evidence uri carries the snapshot hex"
    );
}

// ============================================================================
// writes: intents emit correctly-shaped FilesMsg JSON to target "files"
// ============================================================================

#[test]
fn write_intents_emit_correctly_shaped_msgs() {
    let d = tempfile::tempdir().unwrap();
    let f = open_files(&d);
    let mut ctx = RouteCtx::new(f);
    {
        let mut cap = FsCap::new(&mut ctx);
        cap.commit(
            Some("aa".repeat(32)),
            "an edit",
            vec![put_inline_change("/shared/edited", b"edit")],
        );
        cap.put_inline("/home/app/new.txt", b"hello", "create new");
        cap.pin("bb".repeat(32).as_str(), "release-1");
        cap.watch("/shared", "indexer");
    }

    let emitted: Vec<Msg> = ctx.emitted.iter().cloned().collect();
    assert_eq!(emitted.len(), 4, "one msg per write intent");
    assert!(
        emitted.iter().all(|m| m.target == "files"),
        "every intent targets the fs module"
    );

    // 1: commit passes base + message + changes through verbatim.
    match decode_msg(&emitted[0].payload).expect("decode commit") {
        FilesMsg::Commit {
            base_snapshot,
            message,
            changes,
        } => {
            assert_eq!(base_snapshot, Some("aa".repeat(32)));
            assert_eq!(message, "an edit");
            assert_eq!(changes.len(), 1);
        }
        other => panic!("expected Commit, got {other:?}"),
    }

    // 2: put_inline is create-only sugar — base None + a SINGLE Put(Inline) whose
    // b64 is the base64 of the raw bytes.
    match decode_msg(&emitted[1].payload).expect("decode put_inline") {
        FilesMsg::Commit {
            base_snapshot,
            message,
            changes,
        } => {
            assert_eq!(base_snapshot, None, "put_inline is create-only (base None)");
            assert_eq!(message, "create new");
            assert_eq!(changes.len(), 1, "exactly one Put");
            match &changes[0] {
                Change::Put {
                    path,
                    exec,
                    meta,
                    content,
                } => {
                    assert_eq!(path, "/home/app/new.txt");
                    assert!(!exec);
                    assert!(meta.is_empty());
                    assert_eq!(
                        content,
                        &Content::Inline {
                            b64: STANDARD.encode(b"hello"),
                        }
                    );
                }
                other => panic!("expected a Put change, got {other:?}"),
            }
        }
        other => panic!("expected Commit, got {other:?}"),
    }

    // 3: pin.
    match decode_msg(&emitted[2].payload).expect("decode pin") {
        FilesMsg::Pin { snapshot, name } => {
            assert_eq!(snapshot, "bb".repeat(32));
            assert_eq!(name, "release-1");
        }
        other => panic!("expected Pin, got {other:?}"),
    }

    // 4: watch.
    match decode_msg(&emitted[3].payload).expect("decode watch") {
        FilesMsg::Watch { prefix, module_id } => {
            assert_eq!(prefix, "/shared");
            assert_eq!(module_id, "indexer");
        }
        other => panic!("expected Watch, got {other:?}"),
    }
}

#[test]
fn with_target_overrides_the_emit_target() {
    let d = tempfile::tempdir().unwrap();
    let f = open_files(&d);
    let mut ctx = RouteCtx::new(f);
    {
        let mut cap = FsCap::with_target(&mut ctx, "duckfs");
        cap.pin("cc".repeat(32).as_str(), "p");
    }
    assert_eq!(ctx.emitted.len(), 1);
    assert_eq!(
        ctx.emitted[0].target, "duckfs",
        "with_target retargets both reads and writes"
    );
}

// ============================================================================
// decode_notify: byte-for-byte agreement with the module's emitted shape
// ============================================================================

#[test]
fn decode_notify_round_trips_the_real_emitted_shape() {
    let d = tempfile::tempdir().unwrap();
    let mut f = open_files(&d);
    // register a watch through the REAL Watch op (module-origin, self-targeted),
    // then commit it — so the notification below comes from the production
    // emission path, not a hand-typed payload.
    exec(
        &mut f,
        Origin::Module("indexer".into()),
        1,
        FilesMsg::Watch {
            prefix: "/shared".into(),
            module_id: "indexer".into(),
        },
    );
    commit_block(&mut f);

    // commit under the watched prefix — the module emits one duckfs_notify msg.
    let ctx = commit(
        &mut f,
        Origin::System,
        2,
        None,
        vec![put_inline_change("/shared/doc.txt", b"hi")],
    );
    commit_block(&mut f);
    let head = f.committed_head_for_test().expect("head");

    assert_eq!(ctx.emitted.len(), 1, "exactly one watch hit");
    let payload = &ctx.emitted[0].payload;
    // feed the module's own emitted bytes straight into decode_notify: this proves
    // the two sides agree byte-for-byte.
    let n: Notify = decode_notify(payload).expect("decodes the real notify shape");
    assert_eq!(n.prefix, "/shared");
    assert_eq!(n.path, "/shared/doc.txt");
    assert_eq!(n.snapshot, head);
}

#[test]
fn decode_notify_returns_none_on_foreign_payloads() {
    // a foreign module's op (a FilesMsg commit) carries no duckfs_notify key.
    let commit_op = encode_msg(&FilesMsg::Commit {
        base_snapshot: None,
        message: "not a notify".into(),
        changes: vec![],
    });
    assert!(
        decode_notify(&commit_op).is_none(),
        "a commit op is not a notify"
    );

    // arbitrary well-formed json without the key.
    assert!(
        decode_notify(br#"{"chat":{"body":"hello"}}"#).is_none(),
        "a foreign json object is not a notify"
    );
    assert!(
        decode_notify(br#"{"duckfs_notify":{"prefix":"/x"}}"#).is_none(),
        "a partial notify (missing path/snapshot) is rejected"
    );

    // non-json binary never panics — it decodes to None.
    assert!(decode_notify(&[0xff, 0x00, 0x01, 0x02]).is_none());
    assert!(decode_notify(&[]).is_none());
}
