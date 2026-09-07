//! the STORE-BACKED cutover-continuity proof for pages: the `pages` guest
//! component over `WasmModule::with_store(QmdbStore)` and the native `Pages`
//! over the same store shape are ROOT-CONTINUOUS — the same op sequence
//! commits the IDENTICAL qmdb merkle root after every block (both roots ARE
//! the store's root, and qmdb's batch canonicalizes mutations by hashed key,
//! so the native logical-key commit order and the wasm hashed-key drain order
//! produce the same op log), the same query replies, and the same resolver
//! sync surface. this cutover changes the executor, not one committed byte, and
//! this proof pins that explicitly, unlike whole-state adapter ports whose root
//! representations differ.

use attribution::AttributionModule;
use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use host::{BlockContext, Host, MemberOutcome, SubmitError};
use pages::{
    BlockKind, NewBlock, PageMsg, PageQuery, Pages, decode_reply, encode_msg, encode_query,
};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};
use sha2::Digest as _;
use statesync::qmdb::{QmdbStore, QmdbSyncReq, encode_qmdb_req};
use wasm_host::WasmModule;

/// GENERATED artifact — built from the `pages` module's guest port by
/// guest-builder (`make wasm-modules`); committed so this proof is self-contained.
const PAGES_WASM: &[u8] = include_bytes!("fixtures/pages.component.wasm");

struct ResolutionParity {
    native: Host,
    wasm: Host,
    height: u64,
}

impl ResolutionParity {
    async fn new(context: &deterministic::Context) -> Self {
        Self {
            native: native_host(context).await,
            wasm: wasm_host_(context).await,
            height: 0,
        }
    }

    async fn submit(&mut self, origin: Origin, msg: Msg) -> Vec<host::DispatchRecord> {
        self.height += 1;
        let context = BlockContext {
            height: self.height,
            consensus_time: 1_000 + self.height,
            origin,
        };
        let native = self
            .native
            .submit_at(
                BlockContext {
                    origin: context.origin.clone(),
                    ..context
                },
                msg.clone(),
            )
            .await
            .unwrap();
        let wasm = self.wasm.submit_at(context, msg).await.unwrap();
        assert_eq!(native.dispatches, wasm.dispatches);
        assert_eq!(self.native.module_roots(), self.wasm.module_roots());
        native.dispatches
    }

    async fn page(&mut self, origin: Origin, msg: PageMsg) -> Vec<host::DispatchRecord> {
        self.submit(origin, op(&msg)).await
    }

    async fn thread(&self, id: &str) -> pages::Thread {
        let query = encode_query(&PageQuery::CommentThread {
            thread_id: id.into(),
        });
        let bytes = self.native.query("pages", &query).await.unwrap();
        assert_eq!(bytes, self.wasm.query("pages", &query).await.unwrap());
        let pages::PageReply::CommentThread(Some(view)) = decode_reply(&bytes).unwrap() else {
            panic!("thread")
        };
        view.thread
    }

    async fn refuse_resolution(&mut self, origin: Origin, thread: &str, resolved: bool) {
        let before = self.thread(thread).await;
        self.height += 1;
        for host in [&mut self.native, &mut self.wasm] {
            let roots = host.module_roots();
            let error = host
                .submit_at(
                    BlockContext {
                        height: self.height,
                        consensus_time: 1_000 + self.height,
                        origin: origin.clone(),
                    },
                    op(&PageMsg::ResolveThread {
                        thread_id: thread.into(),
                        resolved,
                    }),
                )
                .await
                .unwrap_err();
            assert!(
                error.to_string().contains("not the comment author"),
                "{error}"
            );
            assert_eq!(host.module_roots(), roots);
        }
        assert_eq!(self.thread(thread).await, before);
    }

    async fn resolve(&mut self, origin: Origin, thread: &str, resolved: bool, actor: pages::Party) {
        let relations = self.native.module_root("attribution");
        let dispatches = self
            .page(
                origin,
                PageMsg::ResolveThread {
                    thread_id: thread.into(),
                    resolved,
                },
            )
            .await;
        let dispatch = dispatches
            .iter()
            .find(|dispatch| dispatch.module == "pages")
            .unwrap();
        assert_eq!(
            pages::decode_assigned(&dispatch.assigned).unwrap().actor,
            actor
        );
        let thread = self.thread(thread).await;
        assert_eq!(thread.resolved, resolved);
        assert_eq!(thread.resolved_by, resolved.then_some(actor));
        assert_eq!(self.native.module_root("attribution"), relations);
    }

    async fn identity(&mut self, origin: Origin, msg: identity::IdentityMsg) {
        self.submit(
            origin,
            Msg {
                target: "identity".into(),
                payload: identity::encode_msg(&msg),
            },
        )
        .await;
    }

    async fn found(&mut self, signer: &PrivateKey, name: &str) {
        self.identity(
            signed(signer),
            identity::IdentityMsg::Create {
                name: name.into(),
                scheme: identity::KeyScheme::Ed25519,
            },
        )
        .await;
    }
}

fn signed(key: &PrivateKey) -> Origin {
    Origin::External(key.public_key().as_ref().to_vec())
}

fn thread_comment(thread: &str, target: &str) -> PageMsg {
    PageMsg::AddComment {
        thread_id: thread.into(),
        comment_id: format!("{thread}-comment"),
        target: target.into(),
        text: "Comment".into(),
        mentions: vec![],
        anchor: None,
    }
}

#[test]
fn compiled_thread_resolution_preserves_accounts_and_original_key_authority() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = ResolutionParity::new(&context).await;
        let (alice, bob, sibling, stranger) = (
            PrivateKey::from_seed(1),
            PrivateKey::from_seed(2),
            PrivateKey::from_seed(3),
            PrivateKey::from_seed(4),
        );
        // Both old-key grant routes precede identity admission: opener and
        // page editor. Joining an account must preserve exactly those keys.
        p.page(
            signed(&bob),
            PageMsg::CreatePage {
                page_id: "old-opener-page".into(),
                title: "Old opener".into(),
            },
        )
        .await;
        p.page(
            signed(&alice),
            thread_comment("old-opener", "old-opener-page"),
        )
        .await;
        p.page(
            signed(&alice),
            PageMsg::CreatePage {
                page_id: "old-editor-page".into(),
                title: "Old editor".into(),
            },
        )
        .await;
        p.page(
            signed(&bob),
            thread_comment("old-editor", "old-editor-page"),
        )
        .await;
        p.found(&alice, "Alice").await;
        p.found(&bob, "Bob").await;
        let sibling_key = sibling.public_key().as_ref().to_vec();
        let expires_at = 10_000;
        let preimage = identity::add_key_preimage(
            "parity",
            identity::KeyScheme::Ed25519,
            &sibling_key,
            0,
            1,
            expires_at,
        );
        p.identity(
            signed(&sibling),
            identity::IdentityMsg::AddKey {
                scheme: identity::KeyScheme::Ed25519,
                label: None,
                authorizer: identity::Authorizer {
                    key: alice.public_key().as_ref().to_vec(),
                    account: 1,
                    expires_at,
                    proof: keyscheme::testkit::ed25519_proof(
                        &alice,
                        identity::IDENTITY_ADD_KEY_NS,
                        &preimage,
                    ),
                },
            },
        )
        .await;
        for thread in ["old-opener", "old-editor"] {
            p.refuse_resolution(signed(&sibling), thread, true).await;
            p.resolve(signed(&alice), thread, true, pages::Party::Account(1))
                .await;
            p.refuse_resolution(signed(&sibling), thread, false).await;
        }
        assert_eq!(
            p.thread("old-opener").await.opener,
            pages::Party::Key(alice.public_key().as_ref().to_vec())
        );

        // New account-owned pages admit sibling keys as the same editor;
        // the independent opener can reopen, and a stranger can do neither.
        p.page(
            signed(&alice),
            PageMsg::CreatePage {
                page_id: "account-page".into(),
                title: "Account page".into(),
            },
        )
        .await;
        p.page(
            signed(&bob),
            thread_comment("account-thread", "account-page"),
        )
        .await;
        p.refuse_resolution(signed(&stranger), "account-thread", true)
            .await;
        p.resolve(
            signed(&sibling),
            "account-thread",
            true,
            pages::Party::Account(1),
        )
        .await;
        p.refuse_resolution(signed(&stranger), "account-thread", false)
            .await;
        p.resolve(
            signed(&bob),
            "account-thread",
            false,
            pages::Party::Account(2),
        )
        .await;
        p.identity(
            signed(&alice),
            identity::IdentityMsg::RemoveKey { key: sibling_key },
        )
        .await;
        p.refuse_resolution(signed(&sibling), "account-thread", true)
            .await;
    });
}

/// A native executor fixture receives real identity/call completions. Calls
/// below enter dispatch as this module and run only through the host queue's
/// authenticated Program origin, never through a synthetic Program submit.
struct ResolutionExecutor;

#[async_trait::async_trait(?Send)]
impl Module for ResolutionExecutor {
    fn id(&self) -> ModuleId {
        "resolution-executor".into()
    }
    fn root(&self) -> StateRoot {
        StateRoot::ZERO
    }
    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        match &ctx.env().origin {
            Origin::Module(source) if source == "identity" => {
                identity::authenticate_event(&ctx.env().origin, "identity", &msg.payload)
                    .map_err(Error::Module)?;
                Ok(())
            }
            Origin::Module(source) if source == "dispatch" => Ok(()),
            _ => Err(Error::Module("unexpected executor input".into())),
        }
    }
}

impl ResolutionParity {
    async fn program_call(&mut self, account: u64, msg: PageMsg) -> host::CallRecord {
        self.submit(
            Origin::Module("resolution-executor".into()),
            Msg {
                target: "dispatch".into(),
                payload: dispatch::encode_msg(&dispatch::DispatchMsg::Call {
                    invocation: format!("resolution-{}", self.height),
                    step: 0,
                    account,
                    target: "pages".into(),
                    payload: encode_msg(&msg),
                }),
            },
        )
        .await;
        self.height += 1;
        let context = BlockContext {
            height: self.height,
            consensus_time: 1_000 + self.height,
            origin: Origin::System,
        };
        let native = self
            .native
            .submit_block(
                BlockContext {
                    origin: Origin::System,
                    ..context
                },
                vec![],
            )
            .await
            .unwrap();
        let wasm = self.wasm.submit_block(context, vec![]).await.unwrap();
        assert_eq!(native.calls, wasm.calls);
        assert_eq!(self.native.module_roots(), self.wasm.module_roots());
        assert_eq!(native.calls.len(), 1);
        let call = native.calls.into_iter().next().unwrap();
        if let host::CallDisposition::Applied = call.disposition {
            let dispatch = call
                .dispatches
                .iter()
                .find(|dispatch| dispatch.module == "pages")
                .unwrap();
            assert_eq!(dispatch.origin, Origin::Program(account));
            assert_eq!(
                pages::decode_assigned(&dispatch.assigned).unwrap().actor,
                pages::Party::Account(account)
            );
        }
        call
    }
}

#[test]
fn compiled_thread_resolution_authenticates_queued_program_accounts() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = ResolutionParity::new(&context).await;
        for host in [&mut p.native, &mut p.wasm] {
            host.register(Box::new(ResolutionExecutor));
            host.register(Box::new(dispatch::DispatchModule::new("dispatch", "saga", "identity", Box::new(sdk_testkit::MemStore::new()))));
        }
        let (alice, bob) = (PrivateKey::from_seed(1), PrivateKey::from_seed(2));
        p.found(&alice, "Alice").await;
        p.found(&bob, "Bob").await;
        for account in [3, 4] {
            p.identity(Origin::Module("resolution-executor".into()), identity::IdentityMsg::CreateProgram { name: format!("program-{account}"), controller: 1, request: account }).await;
        }
        p.page(signed(&bob), PageMsg::CreatePage { page_id: "human-page".into(), title: "Human".into() }).await;
        let created = p.program_call(3, PageMsg::CreatePage { page_id: "program-page".into(), title: "Program".into() }).await;
        assert_eq!(created.disposition, host::CallDisposition::Applied);
        p.page(signed(&bob), thread_comment("program-editor", "program-page")).await;
        let opened = p.program_call(3, thread_comment("program-opener", "human-page")).await;
        assert_eq!(opened.disposition, host::CallDisposition::Applied);
        for thread in ["program-editor", "program-opener"] {
            p.refuse_resolution(signed(&alice), thread, true).await;
            p.refuse_resolution(Origin::Module("resolution-executor".into()), thread, true).await;
            let operation = PageMsg::ResolveThread { thread_id: thread.into(), resolved: true };
            let before = p.thread(thread).await;
            let pages_root = p.native.module_root("pages");
            let relation_root = p.native.module_root("attribution");
            let refused = p.program_call(4, operation.clone()).await;
            assert!(matches!(refused.disposition, host::CallDisposition::Rejected { ref reason } if reason.contains("not the comment author")));
            assert_eq!(p.thread(thread).await, before);
            assert_eq!(p.native.module_root("pages"), pages_root);
            assert_eq!(p.native.module_root("attribution"), relation_root);
            let resolved = p.program_call(3, operation).await;
            assert_eq!(resolved.disposition, host::CallDisposition::Applied);
            assert_eq!(p.thread(thread).await.resolved_by, Some(pages::Party::Account(3)));
            assert_eq!(p.native.module_root("attribution"), relation_root);
            let reopened = p.program_call(3, PageMsg::ResolveThread { thread_id: thread.into(), resolved: false }).await;
            assert_eq!(reopened.disposition, host::CallDisposition::Applied);
            assert_eq!(p.thread(thread).await.resolved_by, None);
        }
    });
}

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
/// hosts so pages' attribution follow-ups have somewhere realistic to land beside
/// the REAL attribution module (see below) without shaping the claim.
async fn native_host(context: &deterministic::Context) -> Host {
    let store = QmdbStore::init(context.child("native_pages"), "pages").await;
    Host::genesis(vec![
        Box::new(
            Pages::new("pages", Box::new(store))
                .with_attribution("attribution")
                .with_identity("identity"),
        ),
        // the production tag-report target, kept NATIVE in both hosts for
        // isolation: this proof is about the pages cutover, and an identical
        // native attribution on both sides absorbs the emitted follow-ups
        // identically.
        Box::new(identity::Identity::new(
            "identity",
            Box::new(sdk_testkit::MemStore::new()),
            "parity".into(),
        )),
        Box::new(AttributionModule::new(
            "attribution",
            Box::new(sdk_testkit::MemStore::new()),
        )),
        Box::new(QueryProbe::new()),
    ])
    .expect("genesis")
}

async fn wasm_host_(context: &deterministic::Context) -> Host {
    let store = QmdbStore::init(context.child("wasm_pages"), "pages").await;
    Host::genesis(vec![
        Box::new(
            // NOTE: no `.with_attribution` here — the guest compiles the exact
            // production builder chain (`Pages::new(..).with_attribution`) in.
            WasmModule::with_store("pages", PAGES_WASM, Box::new(store)).expect("load component"),
        ),
        Box::new(identity::Identity::new(
            "identity",
            Box::new(sdk_testkit::MemStore::new()),
            "parity".into(),
        )),
        Box::new(AttributionModule::new(
            "attribution",
            Box::new(sdk_testkit::MemStore::new()),
        )),
        Box::new(QueryProbe::new()),
    ])
    .expect("genesis")
}

/// the read matrix: every kept query family, including the `None`/absent
/// shapes. (the page/thread LISTING reads are index-tier now, so the matrix
/// probes the dispatch surface only.)
async fn replies(h: &Host) -> Vec<Vec<u8>> {
    let queries = [
        encode_query(&PageQuery::GetPage {
            page_id: "home".into(),
            after: None,
            limit: 1,
        }),
        encode_query(&PageQuery::GetPage {
            page_id: "home".into(),
            after: Some("home".into()),
            limit: 1,
        }),
        encode_query(&PageQuery::GetPage {
            page_id: "absent".into(),
            after: None,
            limit: 1,
        }),
        encode_query(&PageQuery::GetPage {
            page_id: "empty".into(),
            after: None,
            limit: 1,
        }),
        encode_query(&PageQuery::GetPage {
            page_id: "empty".into(),
            after: Some("empty".into()),
            limit: 1,
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
        encode_query(&PageQuery::TargetThreadCount {
            target: "b1".into(),
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
        h.module_root("attribution")
            .expect("attribution registered"),
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
        // EQUAL.
        assert_eq!(roots(&native), roots(&wasm), "genesis roots diverge");

        // recovery's disk cohort sees the wasm tenant exactly as it saw the
        // native module: per-block durable on its own qmdb, so a root ahead of
        // the last checkpoint is placeable rather than damage.
        assert!(native.block_durable_ids().contains("pages"));
        assert!(wasm.block_durable_ids().contains("pages"));

        // every op family, one block each: tree edits, page nesting, the
        // comment plane (which also emits the attribution follow-up), and subtree
        // removal. `moves` marks blocks that must change the pages root.
        let ops: Vec<(Vec<u8>, PageMsg)> = vec![
            (
                alice.clone(),
                PageMsg::CreatePage {
                    page_id: "home".into(),
                    title: "Home".into(),
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
                // block ops are gated to the page author (#1650) — bob has no
                // standing on alice's page, so these run as alice.
                alice.clone(),
                PageMsg::UpdateText {
                    block_id: "b1".into(),
                    text: "first, edited".into(),
                    marks: None,
                },
            ),
            (
                alice.clone(),
                PageMsg::SetKind {
                    block_id: "b1".into(),
                    kind: BlockKind::Quote,
                },
            ),
            (
                alice.clone(),
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
                    parent: Some("b3".into()),
                    after: None,
                },
            ),
            // Empty nested pages terminate at their own root even though the
            // containing document has a following sibling (`b3`).
            (
                alice.clone(),
                PageMsg::InsertBlock {
                    parent: "home".into(),
                    after: Some("b1".into()),
                    block: nb("empty", BlockKind::Page, "Empty"),
                },
            ),
            (
                alice.clone(),
                PageMsg::InsertBlock {
                    parent: "home".into(),
                    after: Some("b3".into()),
                    block: nb("notes", BlockKind::Page, "Notes"),
                },
            ),
            (
                alice.clone(),
                PageMsg::MoveBlock {
                    block_id: "notes".into(),
                    parent: None,
                    after: None,
                },
            ),
            // the comment plane: authorship derives from origin, the staged
            // thread is re-read in the SAME dispatch, and the accepted comment
            // emits the attribution follow-up (dispatched in the same block).
            (
                alice.clone(),
                PageMsg::AddComment {
                    thread_id: "t1".into(),
                    comment_id: "c1".into(),
                    target: "b1".into(),
                    text: "looks good".into(),
                    mentions: vec![],
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
                PageMsg::RemoveBlock {
                    block_id: "notes".into(),
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
        .with_attribution("attribution").with_identity("identity");
        let mut wasm = WasmModule::with_store(
            "pages",
            PAGES_WASM,
            Box::new(QmdbStore::init(context.child("rev_wasm"), "pages").await),
        )
        .expect("load component");

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
        let (alice, bob) = (key(0xA1), key(0xB2));

        for host in [&mut native, &mut wasm] {
            host.submit_at(
                block(1, &alice),
                op(&PageMsg::CreatePage {
                    page_id: "home".into(),
                    title: "Home".into(),
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
            // a second block to re-home a thread ONTO, and a thread ALICE
            // opened on the first — the inputs of the opener gate below.
            host.submit_at(
                block(3, &alice),
                op(&PageMsg::InsertBlock {
                    parent: "home".into(),
                    after: Some("b1".into()),
                    block: nb("b2", BlockKind::Paragraph, "second"),
                }),
            )
            .await
            .expect("insert");
            host.submit_at(
                block(4, &alice),
                op(&PageMsg::AddComment {
                    thread_id: "t1".into(),
                    comment_id: "c1".into(),
                    target: "b1".into(),
                    text: "looks good".into(),
                    mentions: vec![],
                    anchor: None,
                }),
            )
            .await
            .expect("comment");
        }

        // the rejection matrix: distinct refusal families. each rejected block
        // must leave BOTH roots byte-identical (staged writes discarded).
        let rejects: Vec<(Vec<u8>, PageMsg, &str)> = vec![
            (
                alice.clone(),
                PageMsg::InsertBlock {
                    parent: "ghost".into(),
                    after: None,
                    block: nb("bx", BlockKind::Paragraph, "orphan"),
                },
                "parent block not found",
            ),
            (
                alice.clone(),
                PageMsg::InsertBlock {
                    parent: "home".into(),
                    after: None,
                    block: nb("b1", BlockKind::Paragraph, "duplicate id"),
                },
                "duplicate block id",
            ),
            (
                alice.clone(),
                PageMsg::SetKind {
                    block_id: "b1".into(),
                    kind: BlockKind::Page,
                },
                "page blocks cannot",
            ),
            (
                alice.clone(),
                PageMsg::MoveBlock {
                    block_id: "home".into(),
                    parent: Some("b1".into()),
                    after: None,
                },
                "inside the moved subtree",
            ),
            (
                alice.clone(),
                PageMsg::MoveBlock {
                    block_id: "b1".into(),
                    parent: None,
                    after: None,
                },
                "only page blocks",
            ),
            (
                alice.clone(),
                PageMsg::EditComment {
                    comment_id: "nope".into(),
                    text: "x".into(),
                    mentions: vec![],
                },
                "comment not found",
            ),
            // THE OPENER GATE, proven in the compiled component and not just
            // natively: it reads `env().origin`, the one authorization input
            // that crosses the WIT boundary, so a gate keyed on it is exactly
            // the kind that can compile, review as correct, and be inert
            // inside the guest.
            //
            // alice opened t1, so bob may not re-home it — and an ungated move
            // was the aiming device for `RemoveBlock`'s comment purge: put a
            // stranger's thread on a throwaway block, remove the block, and
            // their comments are hard-deleted past `DeleteComment`'s own
            // author check.
            (
                bob.clone(),
                PageMsg::MoveCommentThread {
                    thread_id: "t1".into(),
                    target: "b2".into(),
                    anchor: None,
                },
                "not the comment author",
            ),
            // and the pre-consensus empty external origin never passes as a
            // real user here, exactly as on the four sibling comment ops.
            (
                Vec::new(),
                PageMsg::MoveCommentThread {
                    thread_id: "t1".into(),
                    target: "b2".into(),
                    anchor: None,
                },
                "empty origin",
            ),
        ];

        for (height, (who, msg, needle)) in rejects.into_iter().enumerate() {
            let height = height as u64 + 5;
            let before = roots(&native);
            assert_eq!(before, roots(&wasm));

            let n_err = native
                .submit_at(block(height, &who), op(&msg))
                .await
                .expect_err("native must reject");
            let w_err = wasm
                .submit_at(block(height, &who), op(&msg))
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
        // GetPage continuation MID-BLOCK (the wasm side answers it
        // staged-over-store through the replay), and the comment reads the
        // staged block. on the wasm side dispatch N+1's reads come from the
        // OUTER staged overlay — the guest's inner pending died with dispatch
        // N — which is exactly the native pending-persists-across-dispatches
        // view.
        let batch = vec![
            (
                Origin::External(alice.clone()),
                op(&PageMsg::CreatePage {
                    page_id: "home".into(),
                    title: "Home".into(),
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
                        after: Some("home".into()),
                        limit: 1,
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
                    after: Some("home".into()),
                    limit: 1,
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
