//! write-path consensus rules of the inbox module. the module serves NO
//! queries (the read surface — paged lists, unread counts — is the index
//! guest's job, `src/index.rs`), so committed state is asserted through
//! `Module::root()` and the canonical `snapshot()` bytes: the encoding IS the
//! root preimage, so a hand-encoded expected image proves state byte-for-byte.

use futures::executor::block_on;
use host::{BlockContext, Host};
use inbox::{Inbox, InboxMsg, MAX_BODY_BYTES, MAX_ITEMS_PER_MEMBER, MAX_MEMBERS, encode_msg};
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

// inbox's execute reads only env (origin + consensus_time); me/height are
// cosmetic, so the shared TestCtx stands in behind two thin constructors.
fn ctx(origin: Origin, consensus_time: u64) -> TestCtx {
    TestCtx::with_env(Env {
        height: 0,
        consensus_time,
        origin,
        me: INBOX.into(),
    })
}

fn sys(consensus_time: u64) -> TestCtx {
    ctx(Origin::System, consensus_time)
}

// ---- canonical snapshot bytes, hand-encoded ---------------------------------
//
// the module's canonical byte layout is BOTH the `root()` preimage and the
// snapshot wire: member count, then per member (id, next_seq, item count,
// items ascending by seq), length-prefixed strings and LE u64s throughout.
// tests assert committed state by building these bytes and comparing them
// against `snapshot()` (or the root they hash to).

fn push_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_str(out: &mut Vec<u8>, value: &str) {
    push_u64(out, value.len() as u64);
    out.extend_from_slice(value.as_bytes());
}

/// one item as `(seq, kind, body, source, created_at, read)`.
type ItemBytes<'a> = (u64, &'a str, &'a str, &'a str, u64, bool);

fn push_member(out: &mut Vec<u8>, member: &str, next_seq: u64, items: &[ItemBytes]) {
    push_str(out, member);
    push_u64(out, next_seq);
    push_u64(out, items.len() as u64);
    for (seq, kind, body, source, created_at, read) in items {
        push_u64(out, *seq);
        push_str(out, kind);
        push_str(out, body);
        push_str(out, source);
        push_u64(out, *created_at);
        out.push(*read as u8);
    }
}

/// the full canonical image for a committed state (members ascending by id —
/// the caller lists them in that order, as the module's BTreeMap encodes them).
fn snapshot_bytes(members: &[(&str, u64, &[ItemBytes])]) -> Vec<u8> {
    let mut out = Vec::new();
    push_u64(&mut out, members.len() as u64);
    for (member, next_seq, items) in members {
        push_member(&mut out, member, *next_seq, items);
    }
    out
}

/// the root a canonical byte image hashes to: the encoding IS the root
/// preimage.
fn root_of_bytes(bytes: &[u8]) -> StateRoot {
    use sha2::{Digest, Sha256};
    StateRoot(Sha256::digest(bytes).into())
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

        // per-member seqs are monotonic from 1 and the queues independent
        // (bob restarts at 1); created_at is the block's consensus time and
        // new items are unread — all pinned byte-for-byte in the canonical
        // committed image.
        let expected = snapshot_bytes(&[
            (
                "alice",
                3,
                &[
                    (1, "mention", "hi", "system", 10, false),
                    (2, "reply", "yo", "system", 11, false),
                ],
            ),
            ("bob", 2, &[(1, "mention", "sup", "system", 12, false)]),
        ]);
        assert_eq!(inbox.snapshot(), expected);
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

        // source = module id verbatim / "ext:"+hex of external bytes /
        // "system" — the ext: prefix domain-separates external keys from
        // pure-hex module ids; never caller-supplied. all four land in the
        // committed image.
        let expected = snapshot_bytes(&[(
            "m",
            5,
            &[
                (1, "k", "from module", "chat", 1, false),
                (2, "k", "from external", "ext:deadbeef", 2, false),
                (3, "k", "from system", "system", 3, false),
                (4, "k", "from anonymous external", "ext:", 4, false),
            ],
        )]);
        assert_eq!(inbox.snapshot(), expected);
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
        assert_eq!(
            inbox.snapshot(),
            snapshot_bytes(&[]),
            "nothing was staged: committed state is byte-empty"
        );
    });
}

#[test]
fn queue_overflow_drops_oldest_item() {
    block_on(async {
        let mut inbox = Inbox::new(INBOX);
        let cap = MAX_ITEMS_PER_MEMBER as u64;
        // one over the per-member cap, all in a single block.
        for i in 0..=cap {
            inbox
                .execute(&mut sys(i), &deliver("alice", "k", "b"))
                .await
                .expect("deliver");
        }
        inbox.commit_block().await.expect("commit");

        // seq 1 (the oldest) was dropped deterministically: the committed
        // window is exactly 2..=cap+1 (the queue holds the cap), and next_seq
        // kept counting past the drop. item seq s was delivered at consensus
        // time s-1.
        let survivors: Vec<ItemBytes> = (2..=cap + 1)
            .map(|seq| (seq, "k", "b", "system", seq - 1, false))
            .collect();
        let expected = snapshot_bytes(&[("alice", cap + 2, &survivors)]);
        assert_eq!(inbox.snapshot(), expected);
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

        // MarkRead up to seq 2 flips exactly seqs 1 and 2 in committed state.
        inbox
            .execute(&mut sys(2), &mark_read("alice", 2))
            .await
            .expect("mark read");
        inbox.commit_block().await.expect("commit mark read");
        let read_two = snapshot_bytes(&[(
            "alice",
            4,
            &[
                (1, "k", "b", "system", 1, true),
                (2, "k", "b", "system", 1, true),
                (3, "k", "b", "system", 1, false),
            ],
        )]);
        assert_eq!(inbox.snapshot(), read_two, "only seqs 1,2 are read");
        let root_after_ack = inbox.root();

        // idempotent re-ack: the same MarkRead commits byte-identical state.
        inbox
            .execute(&mut sys(2), &mark_read("alice", 2))
            .await
            .expect("mark read again");
        inbox.commit_block().await.expect("commit re-ack");
        assert_eq!(inbox.root(), root_after_ack, "re-ack is idempotent");

        // no-op tolerance: unknown member / seq must not error and must not
        // move the root.
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
            root_after_ack,
            "no-op acks never change committed state"
        );

        // Clear removes items but never rewinds next_seq: the next delivery
        // gets seq 4, not a reused low seq.
        inbox
            .execute(&mut sys(4), &clear("alice", 2))
            .await
            .expect("clear up to 2");
        inbox
            .execute(&mut sys(5), &deliver("alice", "k", "after clear"))
            .await
            .expect("deliver after clear");
        inbox.commit_block().await.expect("commit clear+deliver");
        let expected = snapshot_bytes(&[(
            "alice",
            5,
            &[
                (3, "k", "b", "system", 1, false),
                (4, "k", "after clear", "system", 5, false),
            ],
        )]);
        assert_eq!(
            inbox.snapshot(),
            expected,
            "seq 1,2 cleared; the new delivery took seq 4 — next_seq did not rewind"
        );
    });
}

#[test]
fn root_moves_only_on_commit_and_abort_is_byte_identical() {
    block_on(async {
        let mut inbox = Inbox::new(INBOX);
        let root0 = inbox.root();
        let empty = inbox.snapshot();

        inbox
            .execute(&mut sys(1), &deliver("alice", "k", "b"))
            .await
            .expect("stage deliver");
        assert_eq!(inbox.root(), root0, "staged writes do not move the root");
        assert_eq!(
            inbox.snapshot(),
            empty,
            "snapshot is committed-state only — the stage is invisible"
        );

        inbox.commit_block().await.expect("commit");
        let root1 = inbox.root();
        assert_ne!(root1, root0, "commit moves the root");
        let committed = inbox.snapshot();

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
            inbox.snapshot(),
            committed,
            "the aborted delivery left no byte behind"
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
            target.snapshot(),
            bytes,
            "install round-trips the canonical bytes verbatim"
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

        // next delivery to the cleared member resumes at the preserved
        // next_seq: the full committed image after it pins alice's carried
        // read flag AND bob's new item landing at seq 2, not a reused seq 1.
        target
            .execute(&mut sys(10), &deliver("bob", "k", "after clear"))
            .await
            .expect("deliver");
        target.commit_block().await.expect("commit");
        let expected = snapshot_bytes(&[
            (
                "alice",
                3,
                &[
                    (1, "mention", "hi", "chat", 5, true),
                    (2, "k", "second", "system", 6, false),
                ],
            ),
            ("bob", 3, &[(2, "k", "after clear", "system", 10, false)]),
        ]);
        assert_eq!(
            target.snapshot(),
            expected,
            "next_seq survived the clear across the snapshot"
        );
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
        let app0 = host.root_hash();

        let out = host
            .submit_at(
                BlockContext {
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
        assert_ne!(out.root_hash, app0, "the atomic delivery moves the root-hash");

        // the committed inbox root IS the hash of the canonical bytes, so
        // equality against this hand-encoded image proves the follow-up
        // delivered in THIS block, with the EMITTING module as source (not
        // the external submitter) and the block's consensus time.
        let expected = snapshot_bytes(&[(
            "alice",
            2,
            &[(1, "event", "produced", "producer", 42, false)],
        )]);
        assert_eq!(host.module_root(INBOX), Some(root_of_bytes(&expected)));
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
            BlockContext {
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

        // the delivery still committed — and the ghost ack left no trace in
        // the committed image (no "ghost" member section).
        let expected = snapshot_bytes(&[(
            "alice",
            2,
            &[(1, "event", "produced", "producer-ack", 7, false)],
        )]);
        assert_eq!(host.module_root(INBOX), Some(root_of_bytes(&expected)));
    });
}

// ---- crafted snapshots: decode hardening + the seq-exhaustion boundary ------
//
// these tests hand-encode the module's canonical byte layout via the same
// helpers the committed-image assertions use.

/// one member with the given counter and items — a minimal crafted snapshot
/// (source "system", created_at 0, unread).
fn snapshot_of_member(member: &str, next_seq: u64, items: &[(u64, &str, &str)]) -> Vec<u8> {
    let items: Vec<ItemBytes> = items
        .iter()
        .map(|(seq, kind, body)| (*seq, *kind, *body, "system", 0, false))
        .collect();
    snapshot_bytes(&[(member, next_seq, &items)])
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
            .execute(&mut sys(1), &deliver("alice", "k", "one too many"))
            .await
            .expect_err("seq space exhaustion must reject");
        assert!(
            matches!(err, Error::Module(ref m) if m.contains("seq space exhausted")),
            "unexpected error: {err:?}"
        );

        // other members are unaffected, and the rejection left no trace.
        inbox
            .execute(&mut sys(1), &deliver("bob", "k", "fresh member"))
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
