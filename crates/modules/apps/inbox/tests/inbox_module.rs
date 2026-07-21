use futures::executor::block_on;
use host::{BlockContext, Host};
use inbox::Inbox;
use inbox::{
    InboxMsg, InboxQuery, InboxReply, MAX_BODY_BYTES, MAX_ITEMS_PER_MEMBER, MAX_MEMBERS,
    MAX_QUERY_LIMIT, Notification, decode_reply, encode_msg, encode_query,
};
use sdk::{Ctx, Env, Error, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};
use sdk_testkit::TestCtx;

const INBOX: &str = "inbox";

fn msg(inbox_msg: InboxMsg) -> Msg {
    Msg {
        target: INBOX.into(),
        payload: encode_msg(&inbox_msg),
    }
}

fn deliver(member: &str, kind: &str, body: &str) -> Msg {
    msg(InboxMsg::Deliver {
        member: member.into(),
        kind: kind.into(),
        body: body.into(),
    })
}

fn mark_read(member: &str, up_to_seq: u64) -> Msg {
    msg(InboxMsg::MarkRead {
        member: member.into(),
        up_to_seq,
    })
}

fn clear(member: &str, up_to_seq: u64) -> Msg {
    msg(InboxMsg::Clear {
        member: member.into(),
        up_to_seq,
    })
}

async fn list(inbox: &Inbox, member: &str, from_seq: u64, limit: u64) -> Vec<Notification> {
    match decode_reply(
        &inbox
            .query(&encode_query(&InboxQuery::List {
                member: member.into(),
                from_seq,
                limit,
            }))
            .await
            .expect("query list"),
    )
    .expect("decode reply")
    {
        InboxReply::Items(items) => items,
        other => panic!("expected Items, got {other:?}"),
    }
}

async fn unread(inbox: &Inbox, member: &str) -> u64 {
    match decode_reply(
        &inbox
            .query(&encode_query(&InboxQuery::Unread {
                member: member.into(),
            }))
            .await
            .expect("query unread"),
    )
    .expect("decode reply")
    {
        InboxReply::UnreadCount(count) => count,
        other => panic!("expected UnreadCount, got {other:?}"),
    }
}

async fn host_list(host: &Host, member: &str) -> Vec<Notification> {
    match decode_reply(
        &host
            .query(
                INBOX,
                &encode_query(&InboxQuery::List {
                    member: member.into(),
                    from_seq: 0,
                    limit: MAX_QUERY_LIMIT,
                }),
            )
            .await
            .expect("host query list"),
    )
    .expect("decode reply")
    {
        InboxReply::Items(items) => items,
        other => panic!("expected Items, got {other:?}"),
    }
}

// inbox's execute reads only env (origin + consensus_time); me/height are
// cosmetic, so the shared TestCtx stands in behind two thin constructors.
fn ctx(origin: Origin, consensus_time: u64) -> TestCtx {
    TestCtx::with_env(Env {
        protocol_version: 0,
        height: 0,
        consensus_time,
        origin,
        me: INBOX.into(),
    })
}

fn sys(consensus_time: u64) -> TestCtx {
    ctx(Origin::System, consensus_time)
}

#[test]
fn deliver_assigns_per_member_sequence() {
    block_on(async {
        let mut inbox = Inbox::new(INBOX);

        inbox
            .execute(&mut sys(10), &deliver("alice", "mention", "hi"))
            .await
            .expect("deliver a1");
        inbox
            .execute(&mut sys(11), &deliver("alice", "reply", "yo"))
            .await
            .expect("deliver a2");
        inbox
            .execute(&mut sys(12), &deliver("bob", "mention", "sup"))
            .await
            .expect("deliver b1");
        inbox.commit_block().await.expect("commit");

        let alice = list(&inbox, "alice", 0, MAX_QUERY_LIMIT).await;
        let seqs: Vec<u64> = alice.iter().map(|n| n.seq).collect();
        assert_eq!(seqs, [1, 2], "per-member seq is monotonic from 1");
        assert_eq!(alice[0].member, "alice");
        assert_eq!(alice[0].kind, "mention");
        assert_eq!(alice[0].body, "hi");
        assert_eq!(alice[0].created_at, 10);
        assert!(!alice[0].read, "new items are unread");

        let bob = list(&inbox, "bob", 0, MAX_QUERY_LIMIT).await;
        assert_eq!(bob.len(), 1, "bob's queue is independent");
        assert_eq!(bob[0].seq, 1, "bob's seq starts fresh at 1");
    });
}

#[test]
fn source_is_derived_from_origin() {
    block_on(async {
        let mut inbox = Inbox::new(INBOX);

        inbox
            .execute(
                &mut ctx(Origin::Module("chat".into()), 1),
                &deliver("m", "k", "from module"),
            )
            .await
            .expect("module deliver");
        inbox
            .execute(
                &mut ctx(Origin::External(vec![0xde, 0xad, 0xbe, 0xef]), 2),
                &deliver("m", "k", "from external"),
            )
            .await
            .expect("external deliver");
        inbox
            .execute(&mut sys(3), &deliver("m", "k", "from system"))
            .await
            .expect("system deliver");
        inbox
            .execute(
                &mut ctx(Origin::External(Vec::new()), 4),
                &deliver("m", "k", "from anonymous external"),
            )
            .await
            .expect("empty-external deliver");
        inbox.commit_block().await.expect("commit");

        let items = list(&inbox, "m", 0, MAX_QUERY_LIMIT).await;
        let sources: Vec<&str> = items.iter().map(|n| n.source.as_str()).collect();
        assert_eq!(
            sources,
            ["chat", "ext:deadbeef", "system", "ext:"],
            "source = module id verbatim / \"ext:\"+hex of external bytes / \
             \"system\" — the ext: prefix domain-separates external keys from \
             pure-hex module ids; never caller-supplied"
        );
    });
}

#[test]
fn caps_reject_oversized_and_leave_root_unchanged() {
    block_on(async {
        let mut inbox = Inbox::new(INBOX);
        let root0 = inbox.root();

        let big_body = "x".repeat(16 * 1024 + 1);
        let err = inbox
            .execute(&mut sys(1), &deliver("alice", "k", &big_body))
            .await
            .expect_err("oversized body must be rejected");
        assert!(matches!(err, Error::Module(ref m) if m.contains("body exceeds")));

        let big_kind = "k".repeat(65);
        inbox
            .execute(&mut sys(1), &deliver("alice", &big_kind, "b"))
            .await
            .expect_err("oversized kind must be rejected");

        inbox
            .execute(&mut sys(1), &deliver("", "k", "b"))
            .await
            .expect_err("empty member must be rejected");

        let big_member = "m".repeat(257);
        inbox
            .execute(&mut sys(1), &deliver(&big_member, "k", "b"))
            .await
            .expect_err("oversized member must be rejected");

        inbox.commit_block().await.expect("commit");
        assert_eq!(
            inbox.root(),
            root0,
            "rejected deliveries never enter the root preimage"
        );
        assert!(
            list(&inbox, "alice", 0, MAX_QUERY_LIMIT).await.is_empty(),
            "nothing was staged"
        );
    });
}

#[test]
fn queue_overflow_drops_oldest_item() {
    block_on(async {
        let mut inbox = Inbox::new(INBOX);
        // one over the per-member cap, all in a single block.
        for i in 0..(MAX_ITEMS_PER_MEMBER as u64 + 1) {
            inbox
                .execute(&mut sys(i), &deliver("alice", "k", "b"))
                .await
                .expect("deliver");
        }
        inbox.commit_block().await.expect("commit");

        assert_eq!(
            unread(&inbox, "alice").await,
            MAX_ITEMS_PER_MEMBER as u64,
            "queue holds exactly the cap"
        );
        // seq 1 (the oldest) was dropped; the window is 2..=MAX_ITEMS_PER_MEMBER+1.
        let first_page = list(&inbox, "alice", 0, 1).await;
        assert_eq!(
            first_page[0].seq, 2,
            "oldest surviving item is seq 2 — seq 1 was dropped deterministically"
        );
        let tail = list(
            &inbox,
            "alice",
            MAX_ITEMS_PER_MEMBER as u64 + 1,
            MAX_QUERY_LIMIT,
        )
        .await;
        assert_eq!(tail.len(), 1);
        assert_eq!(
            tail[0].seq,
            MAX_ITEMS_PER_MEMBER as u64 + 1,
            "the newest item survives"
        );
    });
}

#[test]
fn member_cap_rejects_new_member() {
    block_on(async {
        let mut inbox = Inbox::new(INBOX);
        // fill exactly MAX_MEMBERS distinct members in one block.
        for i in 0..MAX_MEMBERS as u64 {
            let member = format!("m{i}");
            inbox
                .execute(&mut sys(0), &deliver(&member, "k", ""))
                .await
                .expect("deliver to fresh member");
        }

        // a NEW member beyond the cap is rejected...
        let err = inbox
            .execute(&mut sys(0), &deliver("overflow", "k", ""))
            .await
            .expect_err("new member beyond cap must be rejected");
        assert!(matches!(err, Error::Module(ref m) if m.contains("member capacity")));

        // ...but delivering to an EXISTING member still works.
        inbox
            .execute(&mut sys(0), &deliver("m0", "k", "again"))
            .await
            .expect("existing member still accepts deliveries");
    });
}

#[test]
fn mark_read_and_clear_are_idempotent_and_noop_tolerant() {
    block_on(async {
        let mut inbox = Inbox::new(INBOX);
        for _ in 0..3 {
            inbox
                .execute(&mut sys(1), &deliver("alice", "k", "b"))
                .await
                .expect("deliver");
        }
        inbox.commit_block().await.expect("commit deliveries");
        assert_eq!(unread(&inbox, "alice").await, 3);

        // MarkRead up to seq 2, twice — idempotent.
        inbox
            .execute(&mut sys(2), &mark_read("alice", 2))
            .await
            .expect("mark read");
        inbox
            .execute(&mut sys(2), &mark_read("alice", 2))
            .await
            .expect("mark read again");
        inbox.commit_block().await.expect("commit mark read");
        assert_eq!(unread(&inbox, "alice").await, 1, "only seq 3 stays unread");

        // no-op tolerance: unknown member / seq must not error and must not
        // move the root.
        let root_before = inbox.root();
        inbox
            .execute(&mut sys(3), &mark_read("nobody", 99))
            .await
            .expect("mark read unknown member is a no-op");
        inbox
            .execute(&mut sys(3), &clear("nobody", 99))
            .await
            .expect("clear unknown member is a no-op");
        inbox.commit_block().await.expect("commit no-ops");
        assert_eq!(
            inbox.root(),
            root_before,
            "no-op acks never change committed state"
        );

        // Clear removes items but never rewinds next_seq: the next delivery
        // gets seq 4, not a reused low seq.
        inbox
            .execute(&mut sys(4), &clear("alice", 2))
            .await
            .expect("clear up to 2");
        inbox
            .execute(
                &mut sys(5),
                &deliver("alice", "k", "after clear"),
            )
            .await
            .expect("deliver after clear");
        inbox.commit_block().await.expect("commit clear+deliver");

        let items = list(&inbox, "alice", 0, MAX_QUERY_LIMIT).await;
        let seqs: Vec<u64> = items.iter().map(|n| n.seq).collect();
        assert_eq!(seqs, [3, 4], "seq 1,2 cleared; next_seq did not rewind");
    });
}

#[test]
fn list_pagination_and_unread_count() {
    block_on(async {
        let mut inbox = Inbox::new(INBOX);
        for i in 0..10 {
            inbox
                .execute(&mut sys(i), &deliver("alice", "k", "b"))
                .await
                .expect("deliver");
        }
        inbox.commit_block().await.expect("commit");

        // from_seq is inclusive, ascending.
        let page = list(&inbox, "alice", 4, 3).await;
        let seqs: Vec<u64> = page.iter().map(|n| n.seq).collect();
        assert_eq!(
            seqs,
            [4, 5, 6],
            "page starts at from_seq, ascending, limited"
        );

        // limit is clamped to MAX_QUERY_LIMIT.
        let all = list(&inbox, "alice", 0, MAX_QUERY_LIMIT + 1000).await;
        assert_eq!(all.len(), 10, "all ten items, limit clamped");

        assert_eq!(unread(&inbox, "alice").await, 10);
        inbox
            .execute(&mut sys(11), &mark_read("alice", 7))
            .await
            .expect("mark read");
        inbox.commit_block().await.expect("commit");
        assert_eq!(unread(&inbox, "alice").await, 3, "seq 8,9,10 remain unread");
    });
}

#[test]
fn root_moves_only_on_commit_and_abort_is_byte_identical() {
    block_on(async {
        let mut inbox = Inbox::new(INBOX);
        let root0 = inbox.root();

        inbox
            .execute(&mut sys(1), &deliver("alice", "k", "b"))
            .await
            .expect("stage deliver");
        assert_eq!(inbox.root(), root0, "staged writes do not move the root");
        assert_eq!(
            list(&inbox, "alice", 0, MAX_QUERY_LIMIT).await.len(),
            1,
            "queries read through the staged overlay"
        );

        inbox.commit_block().await.expect("commit");
        let root1 = inbox.root();
        assert_ne!(root1, root0, "commit moves the root");

        inbox
            .execute(&mut sys(2), &deliver("alice", "k", "b2"))
            .await
            .expect("stage another");
        assert_eq!(inbox.root(), root1, "root is committed-state only");
        inbox.abort_block().await.expect("abort");
        assert_eq!(
            inbox.root(),
            root1,
            "abort leaves the root byte-identical to pre-block"
        );
        assert_eq!(
            list(&inbox, "alice", 0, MAX_QUERY_LIMIT).await.len(),
            1,
            "aborted delivery is gone"
        );
    });
}

#[test]
fn snapshot_install_round_trips() {
    block_on(async {
        let mut source = Inbox::new(INBOX);
        source
            .execute(
                &mut ctx(Origin::Module("chat".into()), 5),
                &deliver("alice", "mention", "hi"),
            )
            .await
            .expect("deliver");
        source
            .execute(&mut sys(6), &deliver("alice", "k", "second"))
            .await
            .expect("deliver");
        source
            .execute(&mut sys(7), &deliver("bob", "k", "solo"))
            .await
            .expect("deliver");
        source.commit_block().await.expect("commit");
        // exercise read flags + a clear so the snapshot carries every field.
        source
            .execute(&mut sys(8), &mark_read("alice", 1))
            .await
            .expect("mark read");
        source
            .execute(&mut sys(9), &clear("bob", 1))
            .await
            .expect("clear bob (leaves empty queue, next_seq preserved)");
        source.commit_block().await.expect("commit");

        // the module advertises self-contained snapshot bytes...
        let handle = source.state_sync_handle().expect("handle");
        let bytes = match handle {
            StateSyncHandle::SnapshotBytes(bytes) => bytes,
            other => panic!("expected SnapshotBytes, got {other:?}"),
        };

        // ...that install verbatim against the source root.
        let mut target = Inbox::new(INBOX);
        target.install(&bytes, source.root()).expect("install");
        assert_eq!(target.root(), source.root());
        assert_eq!(
            list(&target, "alice", 0, MAX_QUERY_LIMIT).await,
            list(&source, "alice", 0, MAX_QUERY_LIMIT).await
        );
        assert_eq!(
            list(&target, "bob", 0, MAX_QUERY_LIMIT).await,
            list(&source, "bob", 0, MAX_QUERY_LIMIT).await
        );

        // a wrong expected root is rejected before adopting.
        let mut reject = Inbox::new(INBOX);
        reject
            .install(&bytes, StateRoot::ZERO)
            .expect_err("root mismatch must be rejected");
        assert_eq!(
            reject.root(),
            Inbox::new(INBOX).root(),
            "state untouched on rejected install"
        );

        // next delivery to the cleared member resumes at the preserved next_seq.
        target
            .execute(
                &mut sys(10),
                &deliver("bob", "k", "after clear"),
            )
            .await
            .expect("deliver");
        target.commit_block().await.expect("commit");
        let bob = list(&target, "bob", 0, MAX_QUERY_LIMIT).await;
        assert_eq!(bob[0].seq, 2, "next_seq survived the clear across snapshot");
    });
}

// ---- P2: a module follow-up delivers atomically in the causing block --------

/// a stand-in producer module that, on any op, emits an inbox `Deliver`
/// follow-up — the cross-module write path the inbox exists to serve.
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
        ctx.emit_msg(deliver("alice", "event", "produced"));
        Ok(())
    }
}

#[test]
fn module_follow_up_delivers_atomically_with_source_of_emitter() {
    block_on(async {
        let mut host =
            Host::genesis(vec![Box::new(Inbox::new(INBOX)), Box::new(Producer)]).expect("genesis");
        let app0 = host.app_hash();

        let out = host
            .submit_at(
                BlockContext { protocol_version: 0,
                    height: 1,
                    consensus_time: 42,
                    origin: Origin::External(b"tester".to_vec()),
                },
                Msg {
                    target: "producer".into(),
                    payload: Vec::new(),
                },
            )
            .await
            .expect("submit producer op");
        assert_ne!(out.app_hash, app0, "the atomic delivery moves the app-hash");

        let items = host_list(&host, "alice").await;
        assert_eq!(items.len(), 1, "the follow-up delivered in the same block");
        assert_eq!(
            items[0].source, "producer",
            "source is the EMITTING module's origin, not the external submitter"
        );
        assert_eq!(
            items[0].created_at, 42,
            "created_at is the block's consensus time"
        );
    });
}

/// a producer that emits a Deliver followed by a no-op MarkRead ack: the ack
/// must not abort the cascade the delivery began.
struct ProducerWithAck;

#[async_trait::async_trait(?Send)]
impl Module for ProducerWithAck {
    fn id(&self) -> ModuleId {
        "producer-ack".into()
    }

    fn root(&self) -> StateRoot {
        StateRoot::ZERO
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, _msg: &Msg) -> Result<(), Error> {
        ctx.emit_msg(deliver("alice", "event", "produced"));
        // a MarkRead against a member/seq that does not yet exist must be a
        // deterministic no-op, never a block-aborting error.
        ctx.emit_msg(mark_read("ghost", 999));
        Ok(())
    }
}

#[test]
fn noop_ack_follow_up_does_not_abort_the_block() {
    block_on(async {
        let mut host = Host::genesis(vec![Box::new(Inbox::new(INBOX)), Box::new(ProducerWithAck)])
            .expect("genesis");

        host.submit_at(
            BlockContext { protocol_version: 0,
                height: 1,
                consensus_time: 7,
                origin: Origin::System,
            },
            Msg {
                target: "producer-ack".into(),
                payload: Vec::new(),
            },
        )
        .await
        .expect("no-op ack must not fail the block");

        let items = host_list(&host, "alice").await;
        assert_eq!(items.len(), 1, "the delivery still committed");
    });
}

// ---- crafted snapshots: decode hardening + the seq-exhaustion boundary ------
//
// these tests hand-encode the module's canonical byte layout (the exact root
// preimage): member count, then per member (id, next_seq, item count, items
// ascending by seq), length-prefixed strings and LE u64s throughout.

fn push_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_str(out: &mut Vec<u8>, value: &str) {
    push_u64(out, value.len() as u64);
    out.extend_from_slice(value.as_bytes());
}

fn push_item(out: &mut Vec<u8>, seq: u64, kind: &str, body: &str, source: &str) {
    push_u64(out, seq);
    push_str(out, kind);
    push_str(out, body);
    push_str(out, source);
    push_u64(out, 0); // created_at
    out.push(0); // read = false
}

/// one member with the given counter and items — a minimal crafted snapshot.
fn snapshot_of_member(member: &str, next_seq: u64, items: &[(u64, &str, &str)]) -> Vec<u8> {
    let mut out = Vec::new();
    push_u64(&mut out, 1); // member count
    push_str(&mut out, member);
    push_u64(&mut out, next_seq);
    push_u64(&mut out, items.len() as u64);
    for (seq, kind, body) in items {
        push_item(&mut out, *seq, kind, body, "system");
    }
    out
}

/// the root a valid crafted snapshot must install against: the encoding IS the
/// root preimage.
fn root_of_bytes(bytes: &[u8]) -> StateRoot {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    StateRoot(h.finalize().into())
}

#[test]
fn snapshot_with_zero_next_seq_is_rejected() {
    // next_seq starts at 1 and only increments; 0 is not execute-reachable.
    let bytes = snapshot_of_member("alice", 0, &[]);
    let err = Inbox::new(INBOX)
        .install(&bytes, root_of_bytes(&bytes))
        .expect_err("next_seq == 0 must be rejected");
    assert!(
        matches!(err, Error::Module(ref m) if m.contains("next_seq is zero")),
        "unexpected error: {err:?}"
    );
}

#[test]
fn snapshot_with_over_cap_fields_is_rejected() {
    // execute rejects over-cap fields before staging, so no honest validator
    // can commit them — an image carrying one is corrupt or hostile.
    let big_body = "x".repeat(MAX_BODY_BYTES + 1);
    let bytes = snapshot_of_member("alice", 2, &[(1, "k", big_body.as_str())]);
    let err = Inbox::new(INBOX)
        .install(&bytes, root_of_bytes(&bytes))
        .expect_err("over-cap body must be rejected");
    assert!(
        matches!(err, Error::Module(ref m) if m.contains("body exceeds cap")),
        "unexpected error: {err:?}"
    );

    let big_member = "m".repeat(257);
    let bytes = snapshot_of_member(&big_member, 1, &[]);
    Inbox::new(INBOX)
        .install(&bytes, root_of_bytes(&bytes))
        .expect_err("over-cap member id must be rejected");

    let big_kind = "k".repeat(65);
    let bytes = snapshot_of_member("alice", 2, &[(1, big_kind.as_str(), "b")]);
    Inbox::new(INBOX)
        .install(&bytes, root_of_bytes(&bytes))
        .expect_err("over-cap kind must be rejected");
}

#[test]
fn seq_exhaustion_rejects_deterministically() {
    block_on(async {
        // next_seq == u64::MAX is execute-reachable in principle (2^64 - 2
        // deliveries), so install must ACCEPT it...
        let bytes = snapshot_of_member("alice", u64::MAX, &[(1, "k", "survivor")]);
        let mut inbox = Inbox::new(INBOX);
        inbox
            .install(&bytes, root_of_bytes(&bytes))
            .expect("a maxed-out counter is a valid committed state");
        let root_installed = inbox.root();

        // ...but the NEXT delivery to that member has no seq left: reject
        // deterministically, before any mutation — never panic or wrap.
        let err = inbox
            .execute(
                &mut sys(1),
                &deliver("alice", "k", "one too many"),
            )
            .await
            .expect_err("seq space exhaustion must reject");
        assert!(
            matches!(err, Error::Module(ref m) if m.contains("seq space exhausted")),
            "unexpected error: {err:?}"
        );

        // other members are unaffected, and the rejection left no trace.
        inbox
            .execute(
                &mut sys(1),
                &deliver("bob", "k", "fresh member"),
            )
            .await
            .expect("an unexhausted member still accepts deliveries");
        inbox.abort_block().await.expect("abort");
        assert_eq!(
            inbox.root(),
            root_installed,
            "the rejected delivery staged nothing"
        );
    });
}
