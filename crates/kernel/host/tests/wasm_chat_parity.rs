//! the STORE-BACKED cutover-continuity proof for chat: the chat guest
//! component over `WasmModule::with_store(QmdbStore)` and the native `Chat`
//! over the same store shape are ROOT-CONTINUOUS — the same op sequence
//! commits the IDENTICAL qmdb merkle root after every block (both roots ARE
//! the store's root; qmdb's batch canonicalizes mutations by hashed key, so
//! the native logical-key commit order and the wasm hashed-key drain order
//! produce the same op log), including the byte-identical NO-OP blocks the
//! idempotent reaction ops rely on. this cutover changes the executor, not one
//! committed byte. hook fan-out
//! (`emit-msg` follow-ups) and `RegisterHook`'s registry check — a sibling
//! `module-root` read resolved by the runtime's memoized replay — are pinned
//! against a shared sink module.

use chat::{
    Block, Chat, ChatMsg, ChatQuery, ChatReply, PostPolicy, decode_reply, encode_msg, encode_query,
};
use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use host::{BlockContext, Host, MemberOutcome, SubmitError};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};
use sha2::Digest as _;
use statesync::qmdb::{QmdbStore, QmdbSyncReq, encode_qmdb_req};
use tagging::TaggingModule;
use wasm_host::WasmModule;

/// GENERATED artifact — built from the module crate's guest port by
/// guest-builder (`make wasm-modules`); committed so this proof is self-contained.
const CHAT_WASM: &[u8] = include_bytes!("fixtures/chat.component.wasm");

/// a 32-byte submitter key (the ordered lane hands modules verified ed25519
/// ids; the parity claim only needs them distinct and non-empty).
fn key(tag: u8) -> Vec<u8> {
    vec![tag; 32]
}

fn op(m: &ChatMsg) -> Msg {
    Msg {
        target: "chat".into(),
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

fn post(channel: &str, id: &str, text: &str, thread: Option<u64>) -> ChatMsg {
    ChatMsg::PostMessage {
        channel_id: channel.into(),
        message_id: id.into(),
        blocks: vec![Block::paragraph(text)],
        thread,
        as_agent: None,
    }
}

/// a hook sink: swallows the `ChatEvent` follow-ups a post fans out and
/// commits to the byte-concatenation of everything it received — so a hook
/// notification that diverged (or went missing) between the runtimes diverges
/// the sink roots. staged/committed split keeps the block boundary honest.
struct HookSink {
    staged: Vec<Vec<u8>>,
    committed: Vec<u8>,
}

impl HookSink {
    fn new() -> Self {
        Self {
            staged: Vec::new(),
            committed: Vec::new(),
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Module for HookSink {
    fn id(&self) -> ModuleId {
        "sink".into()
    }
    fn root(&self) -> StateRoot {
        StateRoot(sha2::Sha256::digest(&self.committed).into())
    }
    async fn execute(&mut self, _ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        self.staged.push(msg.payload.clone());
        Ok(())
    }
    async fn commit_block(&mut self) -> Result<(), Error> {
        for payload in self.staged.drain(..) {
            self.committed.extend_from_slice(&payload);
        }
        Ok(())
    }
    async fn abort_block(&mut self) -> Result<(), Error> {
        self.staged.clear();
        Ok(())
    }
}

async fn native_host(context: &deterministic::Context) -> Host {
    let store = QmdbStore::init(context.child("native_chat"), "chat").await;
    Host::genesis(vec![
        Box::new(Chat::new("chat", Box::new(store)).with_tagging("tagging")),
        // the production tag-report target, kept NATIVE in both hosts for
        // isolation: this proof is about the chat cutover, and an identical
        // native tagging on both sides absorbs the emitted follow-ups
        // identically.
        Box::new(TaggingModule::new(
            "tagging",
            Box::new(sdk_testkit::MemStore::new()),
        )),
        Box::new(HookSink::new()),
    ])
    .expect("genesis")
}

async fn wasm_host_(context: &deterministic::Context) -> Host {
    let store = QmdbStore::init(context.child("wasm_chat"), "chat").await;
    Host::genesis(vec![
        Box::new(
            // NOTE: no `.with_tagging` here — the guest compiles the exact
            // production builder chain (`Chat::new(..).with_tagging`) in.
            WasmModule::with_store("chat", CHAT_WASM, Box::new(store)).expect("load component"),
        ),
        Box::new(TaggingModule::new(
            "tagging",
            Box::new(sdk_testkit::MemStore::new()),
        )),
        Box::new(HookSink::new()),
    ])
    .expect("genesis")
}

/// the read matrix — the three kept dispatch queries — over one existing
/// channel (`channel` — a range read against an absent channel REJECTS, and a
/// native rejection and its wit-wrapped rendering are legitimately different
/// strings, so the byte-equal matrix only probes channels both hosts hold)
/// plus the global id lookups. the absent CHANNEL record and the absent
/// message id answer a comparable `None`.
async fn replies(h: &Host, channel: &str, message_id: &str) -> Vec<Vec<u8>> {
    let queries = [
        encode_query(&ChatQuery::Channel {
            channel_id: channel.into(),
        }),
        encode_query(&ChatQuery::Channel {
            channel_id: "absent".into(),
        }),
        encode_query(&ChatQuery::MessagesRange {
            channel_id: channel.into(),
            from_seq: 1,
            limit: 16,
        }),
        encode_query(&ChatQuery::Message {
            message_id: message_id.into(),
        }),
        encode_query(&ChatQuery::Message {
            message_id: "ghost".into(),
        }),
    ];
    let mut out = Vec::new();
    for q in &queries {
        out.push(h.query("chat", q).await.expect("query"));
    }
    out
}

/// chat + tagging + sink: the whole observable state of one host.
fn roots(h: &Host) -> (StateRoot, StateRoot, StateRoot) {
    (
        h.module_root("chat").expect("chat registered"),
        h.module_root("tagging").expect("tagging registered"),
        h.module_root("sink").expect("sink registered"),
    )
}

#[test]
fn same_ops_identical_roots_block_by_block() {
    deterministic::Runner::default().start(|context| async move {
        let mut native = native_host(&context).await;
        let mut wasm = wasm_host_(&context).await;
        let (alice, bob, carol) = (key(0xA1), key(0xB2), key(0xC3));

        // ROOT CONTINUITY from block zero: both sides commit to the SAME
        // (empty) qmdb store — equal roots.
        assert_eq!(roots(&native), roots(&wasm), "genesis roots diverge");
        assert!(native.resolver_backed_ids().contains("chat"));
        assert!(wasm.resolver_backed_ids().contains("chat"));

        // every op family, one block each. `moves` = false marks the
        // idempotent no-op blocks whose op log must stay UNTOUCHED — the
        // native module stages nothing, so the wasm side must commit nothing.
        let ops: Vec<(Vec<u8>, ChatMsg, bool)> = vec![
            (
                alice.clone(),
                ChatMsg::CreateChannel {
                    channel_id: "general".into(),
                    name: "General".into(),
                    post_policy: PostPolicy::Open,
                },
                true,
            ),
            (
                alice.clone(),
                post("general", "m1", "hello world", None),
                true,
            ),
            // a thread reply: bumps the root's summary + the thread index.
            (bob.clone(), post("general", "m2", "hi!", Some(1)), true),
            (
                alice.clone(),
                ChatMsg::EditMessage {
                    channel_id: "general".into(),
                    seq: 1,
                    blocks: vec![Block::paragraph("hello world, edited")],
                    base_rev: Some(0),
                },
                true,
            ),
            (
                bob.clone(),
                ChatMsg::AddReaction {
                    channel_id: "general".into(),
                    seq: 1,
                    emoji: "👍".into(),
                },
                true,
            ),
            // the IDEMPOTENT duplicate: stages nothing on the native side, so
            // the store op log — and the root — must stay byte-identical on
            // the wasm side too (the empty-batch skip parity).
            (
                bob.clone(),
                ChatMsg::AddReaction {
                    channel_id: "general".into(),
                    seq: 1,
                    emoji: "👍".into(),
                },
                false,
            ),
            (
                carol.clone(),
                ChatMsg::AddReaction {
                    channel_id: "general".into(),
                    seq: 1,
                    emoji: "🎉".into(),
                },
                true,
            ),
            // exact remove of the last 🎉 reactor: deletes the reaction record
            // AND rewrites the emoji index (a staged DELETE riding the batch).
            (
                carol.clone(),
                ChatMsg::RemoveReaction {
                    channel_id: "general".into(),
                    seq: 1,
                    emoji: "🎉".into(),
                },
                true,
            ),
            // removing an absent reaction: deterministic no-op block.
            (
                carol.clone(),
                ChatMsg::RemoveReaction {
                    channel_id: "general".into(),
                    seq: 1,
                    emoji: "🎉".into(),
                },
                false,
            ),
            // membership + a members-only channel gate.
            (
                alice.clone(),
                ChatMsg::CreateChannel {
                    channel_id: "private".into(),
                    name: "Private".into(),
                    post_policy: PostPolicy::MembersOnly,
                },
                true,
            ),
            (
                alice.clone(),
                ChatMsg::SetMembership {
                    channel_id: "private".into(),
                    user: bob.clone(),
                    member: true,
                },
                true,
            ),
            (
                bob.clone(),
                post("private", "m3", "members only", None),
                true,
            ),
            // RegisterHook's registry check is `ctx.module_root("sink")` — in
            // the wasm guest that is a SIBLING read resolved by memoized
            // replay before the hook is staged.
            (
                alice.clone(),
                ChatMsg::RegisterHook {
                    channel_id: "general".into(),
                    module_id: "sink".into(),
                },
                true,
            ),
            // this post fans out: a ChatEvent follow-up to the sink (pinned by
            // the sink root) plus the tagging report, all in the same block.
            (
                alice.clone(),
                post("general", "m4", "hook this", None),
                true,
            ),
            (
                alice.clone(),
                ChatMsg::DeleteMessage {
                    channel_id: "general".into(),
                    // m4 is general's THIRD sequence (m3 lives in "private").
                    seq: 3,
                },
                true,
            ),
            (
                bob.clone(),
                ChatMsg::JoinHuddle {
                    channel_id: "general".into(),
                    node: vec![0x11; 32],
                },
                true,
            ),
            // re-joining with the same node key: stages nothing.
            (
                bob.clone(),
                ChatMsg::JoinHuddle {
                    channel_id: "general".into(),
                    node: vec![0x11; 32],
                },
                false,
            ),
            (
                alice.clone(),
                ChatMsg::SweepHuddle {
                    channel_id: "general".into(),
                    user: bob.clone(),
                },
                true,
            ),
            // alice, not bob: hook (un)registration is channel-admin authority
            // and alice owns "general".
            (
                alice.clone(),
                ChatMsg::UnregisterHook {
                    channel_id: "general".into(),
                    module_id: "sink".into(),
                },
                true,
            ),
        ];

        for (height, (who, msg, moves)) in ops.into_iter().enumerate() {
            let height = height as u64 + 1;
            let before = roots(&native);
            native
                .submit_at(block(height, &who), op(&msg))
                .await
                .expect("native submit");
            wasm.submit_at(block(height, &who), op(&msg))
                .await
                .expect("wasm submit");

            // THE claim: identical roots after every block boundary.
            assert_eq!(
                roots(&native),
                roots(&wasm),
                "roots diverge after block {height}"
            );
            let chat_root = native.module_root("chat").expect("chat");
            if moves {
                assert_ne!(chat_root, before.0, "chat root stuck at {height}");
            } else {
                // the idempotent no-op: NOTHING staged, NOTHING committed, so
                // the op log (and the root) is byte-identical to a single
                // application — on both runtimes.
                assert_eq!(
                    chat_root, before.0,
                    "no-op block moved the root at {height}"
                );
            }
            assert_eq!(
                replies(&native, "general", "m1").await,
                replies(&wasm, "general", "m1").await,
                "replies diverge after block {height}"
            );
        }

        // the hook actually fired and matched: the sink saw at least one
        // ChatEvent (its root left the empty-state hash) — identically.
        assert_ne!(
            native.module_root("sink"),
            Some(StateRoot(sha2::Sha256::digest([]).into())),
            "the hook fan-out never reached the sink"
        );

        // identical resolver sync surface: same pinned target, same
        // proof-carrying serve bytes — a joiner cannot tell which executor
        // produced the store.
        let n_target = native
            .resolver_sync_target("chat")
            .await
            .expect("native target");
        let w_target = wasm
            .resolver_sync_target("chat")
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
            native.serve_sync("chat", &req).await.expect("native serve"),
            wasm.serve_sync("chat", &req).await.expect("wasm serve"),
            "sync serve bytes diverge"
        );

        // queries are read-only on the wasm side too.
        let settled = roots(&wasm);
        let _ = replies(&wasm, "general", "m1").await;
        assert_eq!(roots(&wasm), settled, "a query moved a root");
    });
}

#[test]
fn sync_handle_matches_native() {
    deterministic::Runner::default().start(|context| async move {
        let native = Chat::new(
            "chat",
            Box::new(QmdbStore::init(context.child("rev_native"), "chat").await),
        )
        .with_tagging("tagging");
        let wasm = WasmModule::with_store(
            "chat",
            CHAT_WASM,
            Box::new(QmdbStore::init(context.child("rev_wasm"), "chat").await),
        )
        .expect("load component");

        let n_handle = native.state_sync_handle().expect("native handle");
        let w_handle = wasm.state_sync_handle().expect("wasm handle");
        assert_eq!(n_handle, w_handle, "sync handles diverge");
        assert!(
            matches!(w_handle, StateSyncHandle::ResolverBacked { ref backend, .. } if backend == "qmdb"),
            "store-backed tenant must stay resolver-backed: {w_handle:?}"
        );
    });
}

#[test]
fn rejections_match_and_leave_no_trace() {
    deterministic::Runner::default().start(|context| async move {
        let mut native = native_host(&context).await;
        let mut wasm = wasm_host_(&context).await;
        let (alice, carol) = (key(0xA1), key(0xC3));

        for host in [&mut native, &mut wasm] {
            host.submit_at(
                block(1, &alice),
                op(&ChatMsg::CreateChannel {
                    channel_id: "general".into(),
                    name: "General".into(),
                    post_policy: PostPolicy::Open,
                }),
            )
            .await
            .expect("create");
            host.submit_at(
                block(2, &alice),
                op(&ChatMsg::CreateChannel {
                    channel_id: "private".into(),
                    name: "Private".into(),
                    post_policy: PostPolicy::MembersOnly,
                }),
            )
            .await
            .expect("create");
            host.submit_at(block(3, &alice), op(&post("general", "m1", "hello", None)))
                .await
                .expect("post");
        }

        // the rejection matrix: distinct refusal families. each rejected block
        // must leave BOTH roots byte-identical (staged writes discarded).
        let rejects: Vec<(Vec<u8>, ChatMsg, &str)> = vec![
            (
                alice.clone(),
                ChatMsg::CreateChannel {
                    channel_id: "general".into(),
                    name: "Duplicate".into(),
                    post_policy: PostPolicy::Open,
                },
                "already exists",
            ),
            (
                alice.clone(),
                post("general", "m1", "duplicate id", None),
                "already exists",
            ),
            (
                alice.clone(),
                post("ghost", "mx", "no channel", None),
                "unknown channel",
            ),
            // reserved module namespace: an external user may not mint ids
            // containing ':'.
            (
                alice.clone(),
                ChatMsg::CreateChannel {
                    channel_id: "forge:sneaky".into(),
                    name: "Sneak".into(),
                    post_policy: PostPolicy::Open,
                },
                "reserved for modules",
            ),
            // the pre-consensus empty external origin never authenticates.
            (
                Vec::new(),
                post("general", "anon", "anonymous", None),
                "non-empty submitter id",
            ),
            // only the stored author may edit.
            (
                carol.clone(),
                ChatMsg::EditMessage {
                    channel_id: "general".into(),
                    seq: 1,
                    blocks: vec![Block::paragraph("hijack")],
                    base_rev: None,
                },
                "only the author",
            ),
            // members-only gate: carol never joined.
            (
                carol.clone(),
                post("private", "m9", "let me in", None),
                "members-only",
            ),
            // `as_agent` demands a module origin.
            (
                alice.clone(),
                ChatMsg::PostMessage {
                    channel_id: "general".into(),
                    message_id: "agent".into(),
                    blocks: vec![Block::paragraph("i am an agent")],
                    thread: None,
                    as_agent: Some("impostor".into()),
                },
                "module origin",
            ),
            // hooking an unregistered module fails the registry check — the
            // sibling module-root read answers `None` on both runtimes.
            (
                alice.clone(),
                ChatMsg::RegisterHook {
                    channel_id: "general".into(),
                    module_id: "ghost-module".into(),
                },
                "unknown hook module",
            ),
        ];

        for (height, (who, msg, needle)) in rejects.into_iter().enumerate() {
            let height = height as u64 + 4;
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
            assert_eq!(
                replies(&native, "general", "m1").await,
                replies(&wasm, "general", "m1").await
            );
        }
    });
}

#[test]
fn multi_dispatch_block_reads_prior_writes_and_isolates_rejections() {
    deterministic::Runner::default().start(|context| async move {
        let mut native = native_host(&context).await;
        let mut wasm = wasm_host_(&context).await;
        let (alice, carol) = (key(0xA1), key(0xC3));

        // ONE block, three dispatches: the post reads the channel CREATED one
        // dispatch earlier (staged, not committed — its head_seq counter too),
        // and the reaction reads the staged message head. on the wasm side
        // dispatch N+1's reads come from the OUTER staged overlay — the
        // guest's inner pending died with dispatch N — which is exactly the
        // native pending-persists-across-dispatches view.
        let batch = vec![
            (
                Origin::External(alice.clone()),
                op(&ChatMsg::CreateChannel {
                    channel_id: "room".into(),
                    name: "Room".into(),
                    post_policy: PostPolicy::Open,
                }),
            ),
            (
                Origin::External(alice.clone()),
                op(&post("room", "r1", "first in room", None)),
            ),
            (
                Origin::External(alice.clone()),
                op(&ChatMsg::AddReaction {
                    channel_id: "room".into(),
                    seq: 1,
                    emoji: "🚀".into(),
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
        assert_eq!(
            replies(&native, "room", "r1").await,
            replies(&wasm, "room", "r1").await
        );

        // ONE block where the SECOND member rejects: the runtime aborts the
        // staged overlay and replays the accepted member — committed state
        // must equal the accepted subset alone, on both runtimes.
        let before = roots(&native);
        let batch = vec![
            (
                Origin::External(alice.clone()),
                op(&post("room", "r2", "accepted", None)),
            ),
            (
                Origin::External(carol.clone()),
                op(&ChatMsg::EditMessage {
                    channel_id: "room".into(),
                    seq: 1,
                    blocks: vec![Block::paragraph("hijack")],
                    base_rev: None,
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
        assert_eq!(
            replies(&native, "room", "r1").await,
            replies(&wasm, "room", "r1").await
        );
        for host in [&native, &wasm] {
            let reply = host
                .query(
                    "chat",
                    &encode_query(&ChatQuery::Message {
                        message_id: "r2".into(),
                    }),
                )
                .await
                .expect("query");
            let ChatReply::Message(Some(view)) = decode_reply(&reply).expect("decode") else {
                panic!("r2 must exist");
            };
            assert_eq!(view.seq, 2);
            assert!(
                !view.head.deleted && view.head.rev == 0,
                "rejected member must leave no trace"
            );
        }
    });
}
