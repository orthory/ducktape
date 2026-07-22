//! the adapter-port equivalence proof for the inbox cutover: the `inbox` guest
//! component (the NATIVE `inbox` crate compiled to wasm behind `guest-adapter`)
//! and the native `Inbox` module answer the SAME op sequence with IDENTICAL
//! query replies, and their roots move in lockstep (move on commit, hold on
//! no-ops and abort). the roots THEMSELVES differ — the port persists the
//! native canonical snapshot as one host-KV value, a declared state-schema
//! break (revision 2) — and this proof pins that difference so it can never be
//! mistaken for accidental compatibility.
//!
//! the inbox's primary writer is a SIBLING module's follow-up (`emit_msg`), so
//! the op matrix includes a delivery emitted by a stub producer module — the
//! cross-module path the native inbox tests exercise — asserting the wasm port
//! derives the same Module-origin `source`.

use host::{BlockContext, Host, MemberOutcome, SubmitError};
use inbox::{
    decode_reply, encode_msg, encode_query, Inbox, InboxMsg, InboxQuery, InboxReply,
    Notification, MAX_BODY_BYTES, MAX_KIND_BYTES, MAX_MEMBER_BYTES, MAX_QUERY_LIMIT,
};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot};
use wasm_host::WasmModule;

/// GENERATED artifact — built from the `inbox` module's guest port by
/// guest-builder (`make wasm-modules`); committed so this proof is self-contained.
const INBOX_WASM: &[u8] = include_bytes!("fixtures/inbox.component.wasm");

fn wasm_inbox() -> WasmModule {
    WasmModule::from_bytes("inbox", INBOX_WASM)
        .expect("load component")
        // the adapter port's host-KV snapshot is revision 2 of the inbox
        // canonical state — the same declaration bin/node makes.
        .with_state_schema_revision(2)
}

/// a stand-in producer module that, on any op, emits an inbox `Deliver`
/// follow-up — the cross-module write path the inbox exists to serve (the
/// native inbox tests deliver through exactly this stub). registered next to
/// BOTH runtimes, so the follow-up reaches the native module and the wasm
/// component through the same drain.
struct Producer;

#[async_trait::async_trait(?Send)]
impl Module for Producer {
    fn id(&self) -> ModuleId {
        "producer".into()
    }

    fn root(&self) -> StateRoot {
        StateRoot::ZERO
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, _msg: &Msg) -> Result<(), Error> {
        ctx.emit_msg(op(&InboxMsg::Deliver {
            member: "alice".into(),
            kind: "event".into(),
            body: "produced".into(),
        }));
        Ok(())
    }
}

fn native_host() -> Host {
    Host::genesis(vec![Box::new(Inbox::new("inbox")), Box::new(Producer)]).expect("genesis")
}

fn wasm_host_() -> Host {
    Host::genesis(vec![Box::new(wasm_inbox()), Box::new(Producer)]).expect("genesis")
}

/// a 32-byte submitter key (the ordered lane hands modules verified ed25519
/// ids; the parity claim only needs them distinct and non-empty).
fn key(tag: u8) -> Vec<u8> {
    vec![tag; 32]
}

fn op(m: &InboxMsg) -> Msg {
    Msg {
        target: "inbox".into(),
        payload: encode_msg(m),
    }
}

fn deliver(member: &str, kind: &str, body: &str) -> Msg {
    op(&InboxMsg::Deliver {
        member: member.into(),
        kind: kind.into(),
        body: body.into(),
    })
}

/// one block's agreed context: both runtimes must see the identical env.
fn block(height: u64, origin: Origin) -> BlockContext {
    BlockContext {
        height,
        consensus_time: 1_000 + height,
        origin,
    }
}

/// the read matrix: every query family, including the empty/zero shapes, a
/// pagination window, and a limit above the clamp.
async fn replies(h: &Host) -> Vec<Vec<u8>> {
    let queries = [
        encode_query(&InboxQuery::List {
            member: "alice".into(),
            from_seq: 0,
            limit: MAX_QUERY_LIMIT,
        }),
        encode_query(&InboxQuery::List {
            member: "alice".into(),
            from_seq: 3,
            limit: 2,
        }),
        encode_query(&InboxQuery::List {
            member: "alice".into(),
            from_seq: 0,
            limit: MAX_QUERY_LIMIT + 1_000,
        }),
        encode_query(&InboxQuery::List {
            member: "bob".into(),
            from_seq: 0,
            limit: MAX_QUERY_LIMIT,
        }),
        encode_query(&InboxQuery::List {
            member: "absent".into(),
            from_seq: 0,
            limit: MAX_QUERY_LIMIT,
        }),
        encode_query(&InboxQuery::Unread {
            member: "alice".into(),
        }),
        encode_query(&InboxQuery::Unread {
            member: "absent".into(),
        }),
    ];
    let mut out = Vec::new();
    for q in &queries {
        out.push(h.query("inbox", q).await.expect("query"));
    }
    out
}

async fn list(h: &Host, member: &str) -> Vec<Notification> {
    let reply = h
        .query(
            "inbox",
            &encode_query(&InboxQuery::List {
                member: member.into(),
                from_seq: 0,
                limit: MAX_QUERY_LIMIT,
            }),
        )
        .await
        .expect("list query");
    match decode_reply(&reply).expect("decode") {
        InboxReply::Items(items) => items,
        other => panic!("expected Items, got {other:?}"),
    }
}

async fn unread(h: &Host, member: &str) -> u64 {
    let reply = h
        .query(
            "inbox",
            &encode_query(&InboxQuery::Unread {
                member: member.into(),
            }),
        )
        .await
        .expect("unread query");
    match decode_reply(&reply).expect("decode") {
        InboxReply::UnreadCount(count) => count,
        other => panic!("expected UnreadCount, got {other:?}"),
    }
}

fn root_of(h: &Host) -> StateRoot {
    h.module_root("inbox").expect("inbox registered")
}

#[test]
fn same_ops_same_replies_roots_in_lockstep_schema_break_pinned() {
    futures::executor::block_on(same_ops_inner());
}

async fn same_ops_inner() {
    let mut native = native_host();
    let mut wasm = wasm_host_();
    let (alice, bob) = (key(0xA1), key(0xB2));

    // the schema break is visible from genesis: the native root hashes the
    // (empty) canonical member map, the wasm root commits to the (empty)
    // host-KV store.
    // at GENESIS the roots coincide: this module's native encoding of empty
    // state is the same empty canonical map the wasm host store hashes. the
    // declared schema break manifests on the FIRST WRITE (asserted per block
    // below), which is what the revision-2 fence actually guards.
    assert_eq!(
        root_of(&native),
        root_of(&wasm),
        "empty-state roots coincide by construction"
    );

    // every op family, in one deterministic sequence: deliveries from every
    // origin shape (external, system, module follow-up, anonymous external),
    // MarkRead/Clear incl. their idempotent and unknown-member no-op forms,
    // and a post-clear delivery (next_seq never rewinds). `moves` says whether
    // the op changes committed state — root movement must agree on BOTH sides.
    let ops: Vec<(Origin, Msg, bool)> = vec![
        (
            Origin::External(alice.clone()),
            deliver("alice", "mention", "hi"),
            true,
        ),
        (Origin::System, deliver("alice", "reply", "yo"), true),
        (
            Origin::External(bob.clone()),
            deliver("bob", "mention", "sup"),
            true,
        ),
        // the sibling follow-up path: the producer's execute emits the inbox
        // Deliver, so the inbox sees Origin::Module("producer").
        (
            Origin::External(alice.clone()),
            Msg {
                target: "producer".into(),
                payload: Vec::new(),
            },
            true,
        ),
        // an anonymous external self-delivery (inbox accepts any origin).
        (
            Origin::External(Vec::new()),
            deliver("alice", "note", "self-note"),
            true,
        ),
        (
            Origin::System,
            op(&InboxMsg::MarkRead {
                member: "alice".into(),
                up_to_seq: 2,
            }),
            true,
        ),
        // idempotent re-ack: same MarkRead again changes nothing.
        (
            Origin::System,
            op(&InboxMsg::MarkRead {
                member: "alice".into(),
                up_to_seq: 2,
            }),
            false,
        ),
        // unknown member: deterministic no-op, never an error.
        (
            Origin::System,
            op(&InboxMsg::MarkRead {
                member: "ghost".into(),
                up_to_seq: 99,
            }),
            false,
        ),
        (
            Origin::System,
            op(&InboxMsg::Clear {
                member: "alice".into(),
                up_to_seq: 1,
            }),
            true,
        ),
        (
            Origin::System,
            op(&InboxMsg::Clear {
                member: "ghost".into(),
                up_to_seq: 99,
            }),
            false,
        ),
        // next_seq survived the clear: this lands as seq 5, not a reused seq.
        (
            Origin::System,
            deliver("alice", "followup", "after clear"),
            true,
        ),
    ];

    for (height, (origin, msg, moves)) in ops.into_iter().enumerate() {
        let height = height as u64 + 1;
        let (n_before, w_before) = (root_of(&native), root_of(&wasm));
        native
            .submit_at(block(height, origin.clone()), msg.clone())
            .await
            .expect("native submit");
        wasm.submit_at(block(height, origin), msg)
            .await
            .expect("wasm submit");

        // replies identical after every block (the whole read matrix).
        assert_eq!(
            replies(&native).await,
            replies(&wasm).await,
            "replies diverge after block {height}"
        );
        // roots move in LOCKSTEP: a state-changing op moves both commit
        // boundaries, a no-op holds both...
        if moves {
            assert_ne!(root_of(&native), n_before, "native root stuck at {height}");
            assert_ne!(root_of(&wasm), w_before, "wasm root stuck at {height}");
        } else {
            assert_eq!(root_of(&native), n_before, "native root moved at {height}");
            assert_eq!(root_of(&wasm), w_before, "wasm root moved at {height}");
        }
        // ...and the roots themselves always differ (the pinned schema break).
        assert_ne!(root_of(&native), root_of(&wasm));
    }

    // decoded spot check on the wasm side: seq 1 was cleared; seq 2 is read;
    // the producer follow-up carries the EMITTING module as source and the
    // causing block's consensus time; the anonymous external reads "ext:";
    // the post-clear delivery took seq 5 (next_seq never rewinds).
    let alice_items = list(&wasm, "alice").await;
    let seqs: Vec<u64> = alice_items.iter().map(|n| n.seq).collect();
    assert_eq!(seqs, [2, 3, 4, 5], "seq 1 cleared, next_seq preserved");
    assert!(alice_items[0].read, "seq 2 was marked read");
    assert_eq!(alice_items[0].source, "system");
    assert_eq!(alice_items[1].source, "producer", "module-origin source");
    assert_eq!(alice_items[1].created_at, 1_004, "the causing block's time");
    assert!(!alice_items[1].read);
    assert_eq!(alice_items[2].source, "ext:", "anonymous external source");
    assert_eq!(alice_items[3].body, "after clear");
    assert_eq!(unread(&wasm, "alice").await, 3, "seq 3,4,5 remain unread");
    let bob_items = list(&wasm, "bob").await;
    assert_eq!(bob_items.len(), 1);
    assert_eq!(
        bob_items[0].source,
        format!("ext:{}", "b2".repeat(32)),
        "external source is ext: + lowercase hex of the submitter key"
    );

    // queries are read-only on the wasm side too: the root is STABLE across
    // the whole read matrix.
    let settled = root_of(&wasm);
    let _ = replies(&wasm).await;
    assert_eq!(root_of(&wasm), settled, "a query moved the wasm root");
}

#[test]
fn rejections_match_and_leave_no_trace() {
    futures::executor::block_on(rejections_inner());
}

async fn rejections_inner() {
    let mut native = native_host();
    let mut wasm = wasm_host_();
    let alice = key(0xA1);

    for host in [&mut native, &mut wasm] {
        host.submit_at(
            block(1, Origin::External(alice.clone())),
            deliver("alice", "k", "seed"),
        )
        .await
        .expect("seed deliver");
    }

    // the rejection matrix: every cap-violation family the native module
    // rejects at execute, plus a malformed payload (the decode seam). each
    // rejected block must leave BOTH roots byte-identical (the abort path:
    // staged writes discarded, no trace).
    let rejects: Vec<(Msg, &str)> = vec![
        (deliver("", "k", "b"), "member must not be empty"),
        (
            deliver(&"m".repeat(MAX_MEMBER_BYTES + 1), "k", "b"),
            "member exceeds 256 bytes",
        ),
        (
            deliver("alice", &"k".repeat(MAX_KIND_BYTES + 1), "b"),
            "kind exceeds 64 bytes",
        ),
        (
            deliver("alice", "k", &"x".repeat(MAX_BODY_BYTES + 1)),
            "body exceeds 16384 bytes",
        ),
        (
            Msg {
                target: "inbox".into(),
                payload: b"definitely-not-json".to_vec(),
            },
            "expected value",
        ),
    ];

    for (height, (msg, needle)) in rejects.into_iter().enumerate() {
        let height = height as u64 + 2;
        let (n_before, w_before) = (root_of(&native), root_of(&wasm));

        let n_err = native
            .submit_at(block(height, Origin::System), msg.clone())
            .await
            .expect_err("native must reject");
        let w_err = wasm
            .submit_at(block(height, Origin::System), msg)
            .await
            .expect_err("wasm must reject");

        // both reject DETERMINISTICALLY with the native module's reason. the
        // wasm runtime wraps the reason in its wit-error rendering, so the
        // parity claim is containment, not string equality.
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

        // abort leaves no trace: both roots byte-identical to pre-block.
        assert_eq!(root_of(&native), n_before, "native root moved on reject");
        assert_eq!(root_of(&wasm), w_before, "wasm root moved on reject");
        assert_eq!(replies(&native).await, replies(&wasm).await);
    }
}

#[test]
fn multi_dispatch_block_reads_prior_writes_and_isolates_rejections() {
    futures::executor::block_on(multi_dispatch_inner());
}

async fn multi_dispatch_inner() {
    let mut native = native_host();
    let mut wasm = wasm_host_();
    let (alice, carol) = (key(0xA1), key(0xC3));

    // ONE block, three ops: the second delivery's seq assignment READS the
    // first op's staged write (next_seq only exists in this block's overlay),
    // and the MarkRead acks an item that only exists staged. on the wasm side
    // that is the outer staged `__state` being reloaded by each later dispatch
    // — the read-your-writes seam the adapter relies on.
    let batch = vec![
        (
            Origin::External(alice.clone()),
            deliver("alice", "k", "first"),
        ),
        (
            Origin::External(alice.clone()),
            deliver("alice", "k", "second"),
        ),
        (
            Origin::External(alice.clone()),
            op(&InboxMsg::MarkRead {
                member: "alice".into(),
                up_to_seq: 1,
            }),
        ),
    ];
    let n_out = native
        .submit_block(block(1, Origin::External(alice.clone())), batch.clone())
        .await
        .expect("native block");
    let w_out = wasm
        .submit_block(block(1, Origin::External(alice.clone())), batch)
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
    assert_eq!(replies(&native).await, replies(&wasm).await);
    for host in [&native, &wasm] {
        let items = list(host, "alice").await;
        let seqs: Vec<u64> = items.iter().map(|n| n.seq).collect();
        assert_eq!(seqs, [1, 2], "the second dispatch saw the staged next_seq");
        assert!(items[0].read, "the third dispatch acked the staged item");
        assert!(!items[1].read);
        assert_eq!(unread(host, "alice").await, 1);
    }

    // ONE block where the SECOND member rejects: the runtime aborts the staged
    // overlay and replays the accepted member — committed state must equal the
    // accepted subset alone, on both runtimes.
    let (n_before, w_before) = (root_of(&native), root_of(&wasm));
    let batch = vec![
        (Origin::External(alice.clone()), deliver("bob", "k", "ok")),
        (
            Origin::External(carol.clone()),
            deliver("bob", "k", &"x".repeat(MAX_BODY_BYTES + 1)),
        ),
    ];
    let n_out = native
        .submit_block(block(2, Origin::External(alice.clone())), batch.clone())
        .await
        .expect("native block");
    let w_out = wasm
        .submit_block(block(2, Origin::External(alice)), batch)
        .await
        .expect("wasm block");
    for out in [&n_out, &w_out] {
        assert!(matches!(out.members[0], MemberOutcome::Applied { .. }));
        assert!(matches!(out.members[1], MemberOutcome::Rejected { .. }));
    }
    // the accepted member landed (roots moved), the rejected one left nothing.
    assert_ne!(root_of(&native), n_before);
    assert_ne!(root_of(&wasm), w_before);
    assert_eq!(replies(&native).await, replies(&wasm).await);
    for host in [&native, &wasm] {
        let items = list(host, "bob").await;
        assert_eq!(items.len(), 1, "a rejected member must leave no trace");
        assert_eq!(items[0].body, "ok");
    }
}
