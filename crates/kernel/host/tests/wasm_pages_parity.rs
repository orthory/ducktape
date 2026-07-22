//! the STORE-BACKED cutover-continuity proof for pages: the `pages-wasm`
//! component over `WasmModule::with_store(QmdbStore)` and the native `Pages`
//! over the same store shape are ROOT-CONTINUOUS — the same op sequence
//! commits the IDENTICAL qmdb merkle root after every block (both roots ARE
//! the store's root, and qmdb's batch canonicalizes mutations by hashed key,
//! so the native logical-key commit order and the wasm hashed-key drain order
//! produce the same op log), the same query replies, and the same resolver
//! sync surface. the state schema revision therefore STAYS 1 — this cutover
//! changes the executor, not one committed byte — and this proof pins that
//! explicitly, unlike the whole-state adapter ports whose roots diverge by
//! declared schema break.

use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use host::{BlockContext, Host, MemberOutcome, SubmitError};
use pages::{
    BlockKind, NewBlock, PageMsg, PageQuery, Pages, decode_reply, encode_msg, encode_query,
};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};
use sha2::Digest as _;
use statesync::qmdb::{QmdbStore, QmdbSyncReq, encode_qmdb_req};
use tagging::TaggingModule;
use wasm_host::WasmModule;

/// GENERATED artifact — built from `crates/guests/pages-wasm` by the module
/// build target; committed so this proof is self-contained.
const PAGES_WASM: &[u8] = include_bytes!("fixtures/pages.component.wasm");

/// a 32-byte submitter key (the ordered lane hands modules verified ed25519
/// ids; the parity claim only needs them distinct and non-empty).
fn key(tag: u8) -> Vec<u8> {
    vec![tag; 32]
}

fn op(m: &PageMsg) -> Msg {
    Msg {
        target: "pages".into(),
        payload: encode_msg(m),
    }
}

/// one block's agreed context: both runtimes must see the identical env.
fn block(height: u64, who: &[u8]) -> BlockContext {
    BlockContext {
        height,
        consensus_time: 1_000 + height,
        origin: Origin::External(who.to_vec()),
    }
}

fn nb(id: &str, kind: BlockKind, text: &str) -> NewBlock {
    NewBlock {
        id: id.into(),
        kind,
        text: text.into(),
        marks: vec![],
    }
}

/// a mid-block query probe: `execute` host-routes its payload as a pages
/// query (`Ctx::query` → the wasm runtime's `query_with` replay) and STAGES
/// the reply; the committed root commits to the last reply's bytes. registered
/// in BOTH hosts, so a mid-block staged-over-store read that diverged between
/// the runtimes would diverge the probe roots.
struct QueryProbe {
    staged: Option<Vec<u8>>,
    committed: Vec<u8>,
}

impl QueryProbe {
    fn new() -> Self {
        Self {
            staged: None,
            committed: Vec::new(),
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Module for QueryProbe {
    fn id(&self) -> ModuleId {
        "probe".into()
    }
    fn root(&self) -> StateRoot {
        StateRoot(sha2::Sha256::digest(&self.committed).into())
    }
    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        self.staged = Some(ctx.query("pages", &msg.payload).await?);
        Ok(())
    }
    async fn commit_block(&mut self) -> Result<(), Error> {
        if let Some(reply) = self.staged.take() {
            self.committed = reply;
        }
        Ok(())
    }
    async fn abort_block(&mut self) -> Result<(), Error> {
        self.staged = None;
        Ok(())
    }
}

/// a hook-style sink: accepts any payload, holds no state. registered in both
/// hosts so pages' tagging follow-ups have somewhere realistic to land beside
/// the REAL tagging module (see below) without shaping the claim.
async fn native_host(context: &deterministic::Context) -> Host {
    let store = QmdbStore::init(context.child("native_pages"), "pages").await;
    Host::genesis(vec![
        Box::new(Pages::new("pages", Box::new(store)).with_tagging("tagging")),
        // the production tag-report target, kept NATIVE in both hosts for
        // isolation: this proof is about the pages cutover, and an identical
        // native tagging on both sides absorbs the emitted follow-ups
        // identically.
        Box::new(TaggingModule::new("tagging")),
        Box::new(QueryProbe::new()),
    ])
    .expect("genesis")
}

async fn wasm_host_(context: &deterministic::Context) -> Host {
    let store = QmdbStore::init(context.child("wasm_pages"), "pages").await;
    Host::genesis(vec![
        Box::new(
            // NOTE: no `.with_tagging` here — the guest compiles the exact
            // production builder chain (`Pages::new(..).with_tagging`) in.
            WasmModule::with_store("pages", PAGES_WASM, Box::new(store)).expect("load component"),
        ),
        Box::new(TaggingModule::new("tagging")),
        Box::new(QueryProbe::new()),
    ])
    .expect("genesis")
}

/// the read matrix: every query family, including the `None`/absent shapes.
async fn replies(h: &Host) -> Vec<Vec<u8>> {
    let queries = [
        encode_query(&PageQuery::ListPages),
        encode_query(&PageQuery::GetPage {
            page_id: "home".into(),
        }),
        encode_query(&PageQuery::GetPage {
            page_id: "absent".into(),
        }),
        encode_query(&PageQuery::GetBlock {
            block_id: "b1".into(),
        }),
        encode_query(&PageQuery::GetBlock {
            block_id: "b2".into(),
        }),
        encode_query(&PageQuery::GetBlock {
            block_id: "gone".into(),
        }),
        encode_query(&PageQuery::ThreadsForTargets {
            targets: vec!["b1".into(), "home".into()],
        }),
        encode_query(&PageQuery::CommentThread {
            thread_id: "t1".into(),
        }),
        encode_query(&PageQuery::GetComment {
            comment_id: "c1".into(),
        }),
    ];
    let mut out = Vec::new();
    for q in &queries {
        out.push(h.query("pages", q).await.expect("query"));
    }
    out
}

fn roots(h: &Host) -> (StateRoot, StateRoot) {
    (
        h.module_root("pages").expect("pages registered"),
        h.module_root("tagging").expect("tagging registered"),
    )
}

#[test]
fn same_ops_identical_roots_block_by_block() {
    deterministic::Runner::default().start(|context| async move {
        let mut native = native_host(&context).await;
        let mut wasm = wasm_host_(&context).await;
        let (alice, bob) = (key(0xA1), key(0xB2));

        // ROOT CONTINUITY from block zero: both sides commit to the SAME
        // (empty) qmdb store, so unlike the whole-state ports the roots are
        // EQUAL, not a declared schema break.
        assert_eq!(roots(&native), roots(&wasm), "genesis roots diverge");

        // the host's snapshot orchestration sees the wasm tenant exactly as it
        // saw the native module: resolver-backed, never snapshot bytes.
        assert!(native.resolver_backed_ids().contains("pages"));
        assert!(wasm.resolver_backed_ids().contains("pages"));

        // every op family, one block each: tree edits, folder nesting, the
        // comment plane (which also emits the tagging follow-up), and subtree
        // removal. `moves` marks blocks that must change the pages root.
        let ops: Vec<(Vec<u8>, PageMsg)> = vec![
            (
                alice.clone(),
                PageMsg::CreatePage {
                    page_id: "home".into(),
                    title: "Home".into(),
                    parent: None,
                },
            ),
            (
                alice.clone(),
                PageMsg::InsertBlock {
                    parent: "home".into(),
                    after: None,
                    block: nb("b1", BlockKind::Paragraph, "first"),
                },
            ),
            (
                alice.clone(),
                PageMsg::InsertBlock {
                    parent: "b1".into(),
                    after: None,
                    block: nb("b2", BlockKind::Todo, "nested todo"),
                },
            ),
            (
                bob.clone(),
                PageMsg::UpdateText {
                    block_id: "b1".into(),
                    text: "first, edited".into(),
                    marks: None,
                },
            ),
            (
                bob.clone(),
                PageMsg::SetKind {
                    block_id: "b1".into(),
                    kind: BlockKind::Quote,
                },
            ),
            (
                bob.clone(),
                PageMsg::SetChecked {
                    block_id: "b2".into(),
                    checked: true,
                },
            ),
            (
                alice.clone(),
                PageMsg::InsertBlock {
                    parent: "home".into(),
                    after: Some("b1".into()),
                    block: nb("b3", BlockKind::Bulleted, "sibling"),
                },
            ),
            (
                alice.clone(),
                PageMsg::MoveBlock {
                    block_id: "b2".into(),
                    parent: "b3".into(),
                    after: None,
                },
            ),
            (
                alice.clone(),
                PageMsg::CreatePage {
                    page_id: "notes".into(),
                    title: "Notes".into(),
                    parent: Some("home".into()),
                },
            ),
            (
                alice.clone(),
                PageMsg::SetPageParent {
                    page_id: "notes".into(),
                    parent: None,
                },
            ),
            // the comment plane: authorship derives from origin, the staged
            // thread is re-read in the SAME dispatch, and the accepted comment
            // emits the tagging follow-up (dispatched in the same block).
            (
                alice.clone(),
                PageMsg::AddComment {
                    thread_id: "t1".into(),
                    comment_id: "c1".into(),
                    target: "b1".into(),
                    text: "looks good".into(),
                    mentions: vec![],
                    as_agent: None,
                    anchor: None,
                },
            ),
            (
                bob.clone(),
                PageMsg::AddComment {
                    thread_id: "t1".into(),
                    comment_id: "c2".into(),
                    target: "b1".into(),
                    text: "agreed".into(),
                    mentions: vec![],
                    as_agent: None,
                    anchor: None,
                },
            ),
            (
                bob.clone(),
                PageMsg::EditComment {
                    comment_id: "c2".into(),
                    text: "agreed!".into(),
                    mentions: vec![],
                },
            ),
            (
                alice.clone(),
                PageMsg::ResolveThread {
                    thread_id: "t1".into(),
                    resolved: true,
                },
            ),
            (
                bob.clone(),
                PageMsg::DeleteComment {
                    comment_id: "c2".into(),
                },
            ),
            // subtree removal: b3 carries b2 below it — both go, with their
            // comment purge walk.
            (
                alice.clone(),
                PageMsg::RemoveBlock {
                    block_id: "b3".into(),
                },
            ),
            (
                alice.clone(),
                PageMsg::DeletePage {
                    page_id: "notes".into(),
                },
            ),
        ];

        for (height, (who, msg)) in ops.into_iter().enumerate() {
            let height = height as u64 + 1;
            let before = roots(&native);
            native
                .submit_at(block(height, &who), op(&msg))
                .await
                .expect("native submit");
            wasm.submit_at(block(height, &who), op(&msg))
                .await
                .expect("wasm submit");

            // THE claim: identical roots after every block boundary — the wasm
            // module's root() IS the same qmdb merkle root the native module
            // committed.
            assert_eq!(
                roots(&native),
                roots(&wasm),
                "roots diverge after block {height}"
            );
            assert_ne!(
                native.module_root("pages"),
                Some(before.0),
                "pages root stuck at {height}"
            );
            assert_eq!(
                replies(&native).await,
                replies(&wasm).await,
                "replies diverge after block {height}"
            );
        }

        // the resolver sync surface is the SAME store on both sides: identical
        // pinned targets (root + op-log bounds) and identical proof-carrying
        // serve bytes — a joiner cannot tell which executor produced the store.
        let n_target = native
            .resolver_sync_target("pages")
            .await
            .expect("native target");
        let w_target = wasm
            .resolver_sync_target("pages")
            .await
            .expect("wasm target");
        assert_eq!(n_target, w_target, "resolver sync targets diverge");
        let req = encode_qmdb_req(&QmdbSyncReq::Ops {
            op_count: n_target.op_count,
            start_loc: n_target.start,
            max_ops: 64,
            include_pinned: true,
        });
        assert_eq!(
            native
                .serve_sync("pages", &req)
                .await
                .expect("native serve"),
            wasm.serve_sync("pages", &req).await.expect("wasm serve"),
            "sync serve bytes diverge"
        );

        // queries are read-only on the wasm side too.
        let settled = roots(&wasm);
        let _ = replies(&wasm).await;
        assert_eq!(roots(&wasm), settled, "a query moved a root");
    });
}

#[test]
fn revision_stays_one_and_the_sync_handle_matches_native() {
    deterministic::Runner::default().start(|context| async move {
        let native = Pages::new(
            "pages",
            Box::new(QmdbStore::init(context.child("rev_native"), "pages").await),
        )
        .with_tagging("tagging");
        let mut wasm = WasmModule::with_store(
            "pages",
            PAGES_WASM,
            Box::new(QmdbStore::init(context.child("rev_wasm"), "pages").await),
        )
        .expect("load component");

        // the committed encoding is UNCHANGED (same store, same op log, same
        // root — proven above), so the canonical-state revision must stay 1:
        // pre-cutover workspaces reopen without a schema fence.
        assert_eq!(Module::state_schema_revision(&native), 1);
        assert_eq!(Module::state_schema_revision(&wasm), 1);

        // and the declared sync surface is verbatim the native declaration.
        let n_handle = native.state_sync_handle().expect("native handle");
        let w_handle = wasm.state_sync_handle().expect("wasm handle");
        assert_eq!(n_handle, w_handle, "sync handles diverge");
        assert!(
            matches!(w_handle, StateSyncHandle::ResolverBacked { ref backend, .. } if backend == "qmdb"),
            "store-backed tenant must stay resolver-backed: {w_handle:?}"
        );

        // a store-backed tenant has NO byte-snapshot install lane — state
        // arrives by rebuilding the concrete store. fail-closed, not silent.
        let err = wasm
            .install(&[], StateRoot::ZERO)
            .expect_err("store-backed install must refuse");
        assert!(
            err.to_string().contains("injected store"),
            "unexpected refusal: {err}"
        );
    });
}

#[test]
fn rejections_match_and_leave_no_trace() {
    deterministic::Runner::default().start(|context| async move {
        let mut native = native_host(&context).await;
        let mut wasm = wasm_host_(&context).await;
        let alice = key(0xA1);

        for host in [&mut native, &mut wasm] {
            host.submit_at(
                block(1, &alice),
                op(&PageMsg::CreatePage {
                    page_id: "home".into(),
                    title: "Home".into(),
                    parent: None,
                }),
            )
            .await
            .expect("create");
            host.submit_at(
                block(2, &alice),
                op(&PageMsg::InsertBlock {
                    parent: "home".into(),
                    after: None,
                    block: nb("b1", BlockKind::Paragraph, "first"),
                }),
            )
            .await
            .expect("insert");
        }

        // the rejection matrix: distinct refusal families. each rejected block
        // must leave BOTH roots byte-identical (staged writes discarded).
        let rejects: Vec<(PageMsg, &str)> = vec![
            (
                PageMsg::InsertBlock {
                    parent: "ghost".into(),
                    after: None,
                    block: nb("bx", BlockKind::Paragraph, "orphan"),
                },
                "parent block not found",
            ),
            (
                PageMsg::InsertBlock {
                    parent: "home".into(),
                    after: None,
                    block: nb("b1", BlockKind::Paragraph, "duplicate id"),
                },
                "duplicate block id",
            ),
            (
                PageMsg::InsertBlock {
                    parent: "home".into(),
                    after: None,
                    block: nb("p", BlockKind::Page, "sneaky page"),
                },
                "created by CreatePage",
            ),
            (
                PageMsg::MoveBlock {
                    block_id: "home".into(),
                    parent: "b1".into(),
                    after: None,
                },
                "page roots cannot",
            ),
            (
                PageMsg::RemoveBlock {
                    block_id: "home".into(),
                },
                "page roots cannot",
            ),
            (
                PageMsg::EditComment {
                    comment_id: "nope".into(),
                    text: "x".into(),
                    mentions: vec![],
                },
                "comment not found",
            ),
        ];

        for (height, (msg, needle)) in rejects.into_iter().enumerate() {
            let height = height as u64 + 3;
            let before = roots(&native);
            assert_eq!(before, roots(&wasm));

            let n_err = native
                .submit_at(block(height, &alice), op(&msg))
                .await
                .expect_err("native must reject");
            let w_err = wasm
                .submit_at(block(height, &alice), op(&msg))
                .await
                .expect_err("wasm must reject");

            // both reject DETERMINISTICALLY with the native module's reason.
            // the wasm runtime wraps the reason in its wit-error rendering, so
            // the parity claim is containment, not string equality.
            let SubmitError::Rejected(Error::Module(n_msg)) = n_err else {
                panic!("native rejection shape: {n_err:?}");
            };
            let SubmitError::Rejected(Error::Module(w_msg)) = w_err else {
                panic!("wasm rejection shape: {w_err:?}");
            };
            assert!(n_msg.contains(needle), "native reason: {n_msg}");
            assert!(
                w_msg.contains(needle),
                "wasm reason must carry the native reason: {w_msg}"
            );

            // abort invariance: roots byte-identical to pre-block, still equal.
            assert_eq!(roots(&native), before, "native root moved on reject");
            assert_eq!(roots(&wasm), before, "wasm root moved on reject");
            assert_eq!(replies(&native).await, replies(&wasm).await);
        }
    });
}

#[test]
fn multi_dispatch_block_reads_prior_writes_and_mid_block_queries_match() {
    deterministic::Runner::default().start(|context| async move {
        let mut native = native_host(&context).await;
        let mut wasm = wasm_host_(&context).await;
        let alice = key(0xA1);

        // ONE block, four dispatches: the insert reads the page CREATED one
        // dispatch earlier (staged, not committed), the probe host-routes a
        // GetPage query MID-BLOCK (the wasm side answers it staged-over-store
        // through the replay), and the comment reads the staged block. on the
        // wasm side dispatch N+1's reads come from the OUTER staged overlay —
        // the guest's inner pending died with dispatch N — which is exactly
        // the native pending-persists-across-dispatches view.
        let batch = vec![
            (
                Origin::External(alice.clone()),
                op(&PageMsg::CreatePage {
                    page_id: "home".into(),
                    title: "Home".into(),
                    parent: None,
                }),
            ),
            (
                Origin::External(alice.clone()),
                op(&PageMsg::InsertBlock {
                    parent: "home".into(),
                    after: None,
                    block: nb("b1", BlockKind::Paragraph, "first"),
                }),
            ),
            (
                Origin::External(alice.clone()),
                Msg {
                    target: "probe".into(),
                    payload: encode_query(&PageQuery::GetPage {
                        page_id: "home".into(),
                    }),
                },
            ),
            (
                Origin::External(alice.clone()),
                op(&PageMsg::AddComment {
                    thread_id: "t1".into(),
                    comment_id: "c1".into(),
                    target: "b1".into(),
                    text: "mid-block".into(),
                    mentions: vec![],
                    as_agent: None,
                    anchor: None,
                }),
            ),
        ];
        let n_out = native
            .submit_block(block(1, &alice), batch.clone())
            .await
            .expect("native block");
        let w_out = wasm
            .submit_block(block(1, &alice), batch)
            .await
            .expect("wasm block");
        for out in [&n_out, &w_out] {
            assert!(
                out.members
                    .iter()
                    .all(|m| matches!(m, MemberOutcome::Applied { .. })),
                "all members must apply: {:?}",
                out.members
            );
        }
        assert_eq!(roots(&native), roots(&wasm));
        // the probe committed the MID-BLOCK reply it saw: byte-identical
        // staged-over-store reads on both runtimes, and the staged page was
        // genuinely visible (a probe that read `None` would hash differently
        // than one that saw the page — cross-checked below via native).
        assert_eq!(
            native.module_root("probe"),
            wasm.module_root("probe"),
            "mid-block query replies diverge"
        );
        // and the staged page WAS visible mid-block: the committed projection
        // now matches what the probe saw (same query, post-commit).
        let post = native
            .query(
                "pages",
                &encode_query(&PageQuery::GetPage {
                    page_id: "home".into(),
                }),
            )
            .await
            .expect("post-commit query");
        assert_eq!(
            native.module_root("probe"),
            Some(StateRoot(sha2::Sha256::digest(&post).into())),
            "probe must have seen the staged page mid-block"
        );
        assert_eq!(replies(&native).await, replies(&wasm).await);

        // ONE block where the SECOND member rejects: the runtime aborts the
        // staged overlay and replays the accepted member — committed state
        // must equal the accepted subset alone, on both runtimes.
        let before = roots(&native);
        let batch = vec![
            (
                Origin::External(alice.clone()),
                op(&PageMsg::UpdateText {
                    block_id: "b1".into(),
                    text: "accepted".into(),
                    marks: None,
                }),
            ),
            (
                Origin::External(alice.clone()),
                op(&PageMsg::InsertBlock {
                    parent: "home".into(),
                    after: None,
                    block: nb("b1", BlockKind::Paragraph, "duplicate id"),
                }),
            ),
        ];
        let n_out = native
            .submit_block(block(2, &alice), batch.clone())
            .await
            .expect("native block");
        let w_out = wasm
            .submit_block(block(2, &alice), batch)
            .await
            .expect("wasm block");
        for out in [&n_out, &w_out] {
            assert!(matches!(out.members[0], MemberOutcome::Applied { .. }));
            assert!(matches!(out.members[1], MemberOutcome::Rejected { .. }));
        }
        assert_ne!(roots(&native), before, "accepted member must land");
        assert_eq!(roots(&native), roots(&wasm));
        assert_eq!(replies(&native).await, replies(&wasm).await);
        for host in [&native, &wasm] {
            let reply = host
                .query(
                    "pages",
                    &encode_query(&PageQuery::GetBlock {
                        block_id: "b1".into(),
                    }),
                )
                .await
                .expect("query");
            let pages::PageReply::Block(Some(b)) = decode_reply(&reply).expect("decode") else {
                panic!("b1 must exist");
            };
            assert_eq!(b.text, "accepted", "rejected member must leave no trace");
        }
    });
}
