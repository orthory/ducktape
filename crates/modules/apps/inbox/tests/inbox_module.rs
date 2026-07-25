//! write-path consensus rules of the inbox module. the module serves NO
//! queries (the read surface — paged lists, unread counts — is the index
//! guest's job, `src/index.rs`), so committed state is asserted through
//! `Module::root()` and the testkit-gated record read (`Inbox::queue_view`);
//! the qmdb continuity proof lives in `tests/sync_round_trip.rs`.

use futures::executor::block_on;
use host::{BlockContext, Host};
use inbox::{
    Inbox, InboxMsg, MAX_ITEMS_PER_MEMBER, MAX_MEMBERS, Notification, encode_msg,
};
use sdk::{Ctx, Env, Error, Module, ModuleId, Msg, Origin, StateRoot};
use sdk_testkit::{MemStore, TestCtx};

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

fn fresh() -> Inbox {
    Inbox::new(INBOX, Box::new(MemStore::new()))
}

/// one member's committed-or-staged queue via the testkit read.
async fn queue(inbox: &Inbox, member: &str) -> Option<(u64, Vec<Notification>)> {
    inbox.queue_view(member).await.expect("queue view")
}

/// the compact item shape assertions compare: `(seq, kind, body, source,
/// created_at, read)`.
fn item_tuple(n: &Notification) -> (u64, String, String, String, u64, bool) {
    (
        n.seq,
        n.kind.clone(),
        n.body.clone(),
        n.source.clone(),
        n.created_at,
        n.read,
    )
}

fn tuples(items: &[Notification]) -> Vec<(u64, String, String, String, u64, bool)> {
    items.iter().map(item_tuple).collect()
}

fn t(
    seq: u64,
    kind: &str,
    body: &str,
    source: &str,
    created_at: u64,
    read: bool,
) -> (u64, String, String, String, u64, bool) {
    (
        seq,
        kind.into(),
        body.into(),
        source.into(),
        created_at,
        read,
    )
}

#[test]
fn deliver_assigns_per_member_sequence() {
    block_on(async {
        let mut inbox = fresh();

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
        // new items are unread.
        let (next, items) = queue(&inbox, "alice").await.expect("alice exists");
        assert_eq!(next, 3);
        assert_eq!(
            tuples(&items),
            vec![
                t(1, "mention", "hi", "system", 10, false),
                t(2, "reply", "yo", "system", 11, false),
            ]
        );
        let (next, items) = queue(&inbox, "bob").await.expect("bob exists");
        assert_eq!(next, 2);
        assert_eq!(tuples(&items), vec![t(1, "mention", "sup", "system", 12, false)]);
    });
}

#[test]
fn source_is_derived_from_origin() {
    block_on(async {
        let mut inbox = fresh();

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
        // pure-hex module ids; never caller-supplied.
        let (next, items) = queue(&inbox, "m").await.expect("m exists");
        assert_eq!(next, 5);
        assert_eq!(
            tuples(&items),
            vec![
                t(1, "k", "from module", "chat", 1, false),
                t(2, "k", "from external", "ext:deadbeef", 2, false),
                t(3, "k", "from system", "system", 3, false),
                t(4, "k", "from anonymous external", "ext:", 4, false),
            ]
        );
    });
}

#[test]
fn caps_reject_oversized_and_leave_root_unchanged() {
    block_on(async {
        let mut inbox = fresh();
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
            "rejected deliveries never enter the root"
        );
        assert!(
            queue(&inbox, "alice").await.is_none(),
            "nothing was staged for the rejected member"
        );
    });
}

#[test]
fn queue_overflow_drops_oldest_item() {
    block_on(async {
        let mut inbox = fresh();
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
        // kept counting past the drop.
        let (next, items) = queue(&inbox, "alice").await.expect("alice exists");
        assert_eq!(next, cap + 2, "next_seq counted past the drop");
        assert_eq!(items.len(), MAX_ITEMS_PER_MEMBER, "the queue holds the cap");
        assert_eq!(items.first().map(|n| n.seq), Some(2), "seq 1 dropped");
        assert_eq!(items.last().map(|n| n.seq), Some(cap + 1));
    });
}

#[test]
fn member_cap_rejects_new_member() {
    block_on(async {
        let mut inbox = fresh();
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
        let mut inbox = fresh();
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
        let (_, items) = queue(&inbox, "alice").await.expect("alice exists");
        assert_eq!(
            items.iter().map(|n| (n.seq, n.read)).collect::<Vec<_>>(),
            vec![(1, true), (2, true), (3, false)],
            "only seqs 1,2 are read"
        );
        let root_after_ack = inbox.root();

        // idempotent re-ack: nothing flips, so nothing is staged and the
        // root holds byte-identical.
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
        let (next, items) = queue(&inbox, "alice").await.expect("alice exists");
        assert_eq!(next, 5);
        assert_eq!(
            tuples(&items),
            vec![
                t(3, "k", "b", "system", 1, false),
                t(4, "k", "after clear", "system", 5, false),
            ],
            "seq 1,2 cleared; the new delivery took seq 4 — next_seq did not rewind"
        );
    });
}

#[test]
fn root_moves_only_on_commit_and_abort_leaves_no_trace() {
    block_on(async {
        let mut inbox = fresh();
        let root0 = inbox.root();

        inbox
            .execute(&mut sys(1), &deliver("alice", "k", "b"))
            .await
            .expect("stage deliver");
        assert_eq!(inbox.root(), root0, "staged writes do not move the root");
        assert!(
            queue(&inbox, "alice").await.is_some(),
            "read-your-writes sees the stage"
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
        let (next, items) = queue(&inbox, "alice").await.expect("alice exists");
        assert_eq!((next, items.len()), (2, 1), "the aborted delivery left no trace");
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

/// the committed-state twin the P2 proofs compare against: a fresh inbox
/// replaying the EXPECTED direct delivery — MemStore roots are a function of
/// the record set alone, so root equality proves the follow-up landed with
/// the emitting module as source and the block's consensus time.
async fn expected_root(source: &str, consensus_time: u64) -> StateRoot {
    let mut twin = fresh();
    twin.execute(
        &mut ctx(Origin::Module(source.into()), consensus_time),
        &deliver("alice", "event", "produced"),
    )
    .await
    .expect("twin deliver");
    twin.commit_block().await.expect("twin commit");
    twin.root()
}

#[test]
fn module_follow_up_delivers_atomically_with_source_of_emitter() {
    block_on(async {
        let mut host =
            Host::genesis(vec![Box::new(fresh()), Box::new(Producer)]).expect("genesis");
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

        // root equality against the replayed twin proves the follow-up
        // delivered in THIS block, with the EMITTING module as source (not
        // the external submitter) and the block's consensus time.
        assert_eq!(
            host.module_root(INBOX),
            Some(expected_root("producer", 42).await)
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
        let mut host = Host::genesis(vec![Box::new(fresh()), Box::new(ProducerWithAck)])
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

        // the delivery still committed — and the ghost ack left no trace
        // (the twin never staged a "ghost" member).
        assert_eq!(
            host.module_root(INBOX),
            Some(expected_root("producer-ack", 7).await)
        );
    });
}

// ---- the seq-exhaustion boundary --------------------------------------------

#[test]
fn seq_exhaustion_rejects_deterministically() {
    block_on(async {
        // next_seq == u64::MAX is execute-reachable in principle (2^64 - 2
        // deliveries), so the testkit injector stands the boundary state up...
        let mut inbox = fresh();
        inbox
            .testkit_saturate_seq("alice")
            .await
            .expect("saturate alice");
        inbox.commit_block().await.expect("commit saturation");
        let root_installed = inbox.root();

        // ...and the NEXT delivery to that member has no seq left: reject
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
