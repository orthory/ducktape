//! write-path consensus rules of the inbox module. the module serves NO
//! queries (the read surface — paged lists, unread counts — is the index
//! guest's job, `src/index.rs`), so committed state is asserted through
//! `Module::root()` and the testkit-gated record read (`Inbox::queue_view`);
//! the qmdb continuity proof lives in `tests/sync_round_trip.rs`.

use futures::executor::block_on;
use host::{BlockContext, Host};
use inbox::{Inbox, InboxMsg, MAX_ITEMS_PER_MEMBER, MAX_MEMBERS, Notification, encode_msg};
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

/// two distinct submitter keys. a MEMBER is an origin's actor string, so the
/// queue a key owns is exactly `Origin::External(key).actor_string()` — these
/// helpers keep the two in lockstep instead of hand-spelling hex.
const ALICE_KEY: [u8; 4] = [0xa1, 0xa1, 0xa1, 0xa1];
const STRANGER_KEY: [u8; 4] = [0xc3, 0xc3, 0xc3, 0xc3];

fn queue_of(key: [u8; 4]) -> String {
    Origin::External(key.to_vec()).actor_string()
}

fn submitter(key: [u8; 4], consensus_time: u64) -> TestCtx {
    ctx(Origin::External(key.to_vec()), consensus_time)
}

fn fresh() -> Inbox {
    Inbox::new(INBOX, Box::new(MemStore::new()))
}

/// one member's committed-or-staged queue via the testkit read.
async fn queue(inbox: &Inbox, member: &str) -> Option<(u64, Vec<Notification>)> {
    inbox.queue_view(member).await.expect("queue view")
}

/// the compact item shape assertions compare: `(seq, kind, body, source,
/// created_at)`. `read` is not a per-item field any more (see
/// `Inbox::read_watermark_view`) — a test that cares about read status
/// checks the watermark directly.
fn item_tuple(n: &Notification) -> (u64, String, String, String, u64) {
    (
        n.seq,
        n.kind.clone(),
        n.body.clone(),
        n.source.clone(),
        n.created_at,
    )
}

fn tuples(items: &[Notification]) -> Vec<(u64, String, String, String, u64)> {
    items.iter().map(item_tuple).collect()
}

fn t(
    seq: u64,
    kind: &str,
    body: &str,
    source: &str,
    created_at: u64,
) -> (u64, String, String, String, u64) {
    (seq, kind.into(), body.into(), source.into(), created_at)
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
                t(1, "mention", "hi", "system", 10),
                t(2, "reply", "yo", "system", 11),
            ]
        );
        let (next, items) = queue(&inbox, "bob").await.expect("bob exists");
        assert_eq!(next, 2);
        assert_eq!(tuples(&items), vec![t(1, "mention", "sup", "system", 12)]);
    });
}

#[test]
fn source_is_derived_from_origin() {
    block_on(async {
        let mut inbox = fresh();

        // module and system origins deliver to any member — "m" is neither's
        // own queue, and that is exactly the point.
        inbox
            .execute(
                &mut ctx(Origin::Module("chat".into()), 1),
                &deliver("m", "k", "from module"),
            )
            .await
            .expect("module deliver");
        inbox
            .execute(&mut sys(3), &deliver("m", "k", "from system"))
            .await
            .expect("system deliver");
        inbox.commit_block().await.expect("commit");

        // source = module id verbatim / "system" — never caller-supplied.
        let (next, items) = queue(&inbox, "m").await.expect("m exists");
        assert_eq!(next, 3);
        assert_eq!(
            tuples(&items),
            vec![
                t(1, "k", "from module", "chat", 1),
                t(2, "k", "from system", "system", 3),
            ]
        );

        // an authenticated external origin may deliver only to its OWN
        // queue: self-delivery still derives `source` as "ext:"+hex, never
        // caller-supplied.
        let mut inbox = fresh();
        let ext_key = vec![0xde, 0xad, 0xbe, 0xef];
        let ext_member = Origin::External(ext_key.clone()).actor_string();
        inbox
            .execute(
                &mut ctx(Origin::External(ext_key), 2),
                &deliver(&ext_member, "k", "from external"),
            )
            .await
            .expect("external self-deliver");
        inbox.commit_block().await.expect("commit external");
        let (_, items) = queue(&inbox, &ext_member).await.expect("ext member exists");
        assert_eq!(
            tuples(&items),
            vec![t(1, "k", "from external", "ext:deadbeef", 2)]
        );

        // the pre-consensus anonymous-external default owns no queue at all,
        // so it cannot even self-deliver to "ext:".
        let err = inbox
            .execute(
                &mut ctx(Origin::External(Vec::new()), 4),
                &deliver("ext:", "k", "from anonymous external"),
            )
            .await
            .expect_err("an unauthenticated external must be refused");
        assert!(
            matches!(err, Error::Module(ref m) if m.contains("may only deliver to its own queue"))
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
        // the acks below are alice's own, so the queue is named for her key.
        let alice = queue_of(ALICE_KEY);
        for _ in 0..3 {
            inbox
                .execute(&mut sys(1), &deliver(&alice, "k", "b"))
                .await
                .expect("deliver");
        }
        inbox.commit_block().await.expect("commit deliveries");

        // MarkRead up to seq 2 raises the watermark to 2, so seqs 1 and 2
        // read as read (derived from the watermark) and seq 3 does not.
        inbox
            .execute(&mut submitter(ALICE_KEY, 2), &mark_read(&alice, 2))
            .await
            .expect("mark read");
        inbox.commit_block().await.expect("commit mark read");
        assert_eq!(inbox.read_watermark_view(&alice).await.unwrap(), 2);
        let (_, items) = queue(&inbox, &alice).await.expect("alice exists");
        let mut derived_read = Vec::new();
        for n in &items {
            derived_read.push((n.seq, inbox.is_read(&alice, n.seq).await.unwrap()));
        }
        assert_eq!(
            derived_read,
            vec![(1, true), (2, true), (3, false)],
            "only seqs 1,2 are read"
        );
        let root_after_ack = inbox.root();

        // idempotent re-ack: nothing flips, so nothing is staged and the
        // root holds byte-identical.
        inbox
            .execute(&mut submitter(ALICE_KEY, 2), &mark_read(&alice, 2))
            .await
            .expect("mark read again");
        inbox.commit_block().await.expect("commit re-ack");
        assert_eq!(inbox.root(), root_after_ack, "re-ack is idempotent");

        // no-op tolerance: a submitter acking their OWN queue before anything
        // was ever delivered to it must not error and must not move the root.
        let nobody = queue_of(STRANGER_KEY);
        inbox
            .execute(&mut submitter(STRANGER_KEY, 3), &mark_read(&nobody, 99))
            .await
            .expect("mark read on an empty own queue is a no-op");
        inbox
            .execute(&mut submitter(STRANGER_KEY, 3), &clear(&nobody, 99))
            .await
            .expect("clear on an empty own queue is a no-op");
        inbox.commit_block().await.expect("commit no-ops");
        assert_eq!(
            inbox.root(),
            root_after_ack,
            "no-op acks never change committed state"
        );

        // Clear removes items but never rewinds next_seq: the next delivery
        // gets seq 4, not a reused low seq.
        inbox
            .execute(&mut submitter(ALICE_KEY, 4), &clear(&alice, 2))
            .await
            .expect("clear up to 2");
        inbox
            .execute(&mut sys(5), &deliver(&alice, "k", "after clear"))
            .await
            .expect("deliver after clear");
        inbox.commit_block().await.expect("commit clear+deliver");
        let (next, items) = queue(&inbox, &alice).await.expect("alice exists");
        assert_eq!(next, 5);
        assert_eq!(
            tuples(&items),
            vec![
                t(3, "k", "b", "system", 1),
                t(4, "k", "after clear", "system", 5),
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
        assert_eq!(
            (next, items.len()),
            (2, 1),
            "the aborted delivery left no trace"
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
        let mut host = Host::genesis(vec![Box::new(fresh()), Box::new(Producer)]).expect("genesis");
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
        assert_ne!(
            out.root_hash, app0,
            "the atomic delivery moves the root-hash"
        );

        // root equality against the replayed twin proves the follow-up
        // delivered in THIS block, with the EMITTING module as source (not
        // the external submitter) and the block's consensus time.
        assert_eq!(
            host.module_root(INBOX),
            Some(expected_root("producer", 42).await)
        );
    });
}

/// a producer that emits a Deliver followed by a MarkRead ack — the shape a
/// delivering module would use if it could also ack what it delivered. it
/// cannot: delivering and acking are different principals.
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
        // the module's OWN actor string: even the queue named after the
        // emitter is not the emitter's to ack.
        ctx.emit_msg(mark_read("producer-ack", 999));
        Ok(())
    }
}

#[test]
fn a_module_follow_up_cannot_ack_any_queue() {
    block_on(async {
        let mut host =
            Host::genesis(vec![Box::new(fresh()), Box::new(ProducerWithAck)]).expect("genesis");
        let app0 = host.root_hash();

        // fail CLOSED: the ack is refused, which aborts the cascade its own
        // delivery began. that is the correct trade — nothing in the tree
        // emits an ack as a follow-up, and admitting a module origin would
        // hand every delivering module a lever over the queue it wrote to.
        let err = host
            .submit_at(
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
            .expect_err("a module-origin ack must be refused");
        assert!(
            format!("{err:?}").contains("a module origin owns no inbox queue"),
            "unexpected refusal: {err:?}"
        );
        assert_eq!(host.root_hash(), app0, "the aborted cascade left no trace");
    });
}

// ---- the ack gate: only a queue's own member acks it -------------------------

#[test]
fn only_the_queues_own_member_may_ack_it() {
    block_on(async {
        let mut inbox = fresh();
        let alice = queue_of(ALICE_KEY);
        for _ in 0..3 {
            inbox
                .execute(
                    &mut ctx(Origin::Module("chat".into()), 1),
                    &deliver(&alice, "k", "b"),
                )
                .await
                .expect("a module delivers into alice's queue");
        }
        inbox.commit_block().await.expect("commit deliveries");
        let sealed = inbox.root();

        // a stranger may neither read-mark nor DELETE alice's queue. Clear is
        // permanent — this is the whole defect: an unattributed wipe of another
        // member's notification history.
        for op in [mark_read(&alice, 3), clear(&alice, 3)] {
            let err = inbox
                .execute(&mut submitter(STRANGER_KEY, 2), &op)
                .await
                .expect_err("a stranger must be refused");
            assert!(
                matches!(&err, Error::Module(m) if m.contains("only the queue's own member may ack it")),
                "unexpected refusal: {err:?}"
            );
            inbox.abort_block().await.expect("abort");
        }
        assert_eq!(inbox.root(), sealed, "a refused ack stages nothing");
        let (_, items) = queue(&inbox, &alice).await.expect("alice exists");
        assert_eq!(items.len(), 3, "nothing was cleared");
        assert_eq!(
            inbox.read_watermark_view(&alice).await.unwrap(),
            0,
            "nothing was marked"
        );

        // alice performs both on her own queue.
        inbox
            .execute(&mut submitter(ALICE_KEY, 3), &mark_read(&alice, 3))
            .await
            .expect("the member marks her own queue read");
        inbox
            .execute(&mut submitter(ALICE_KEY, 4), &clear(&alice, 3))
            .await
            .expect("the member clears her own queue");
        inbox.commit_block().await.expect("commit alice's acks");
        // the clear emptied her whole queue: the META record is deleted, not
        // left behind item-less (see `clear_that_empties_a_queue...` below).
        assert!(
            queue(&inbox, &alice).await.is_none(),
            "an emptied queue's meta record is deleted"
        );
    });
}

#[test]
fn only_an_authenticated_external_submitter_owns_a_queue() {
    block_on(async {
        let mut inbox = fresh();
        // every origin that cannot own a queue, against a queue named exactly
        // after it — so the refusal is about the ORIGIN KIND, never a mismatch.
        for (origin, member, refusal) in [
            (
                Origin::Module("chat".into()),
                "chat",
                "a module origin owns no inbox queue",
            ),
            (
                Origin::System,
                "system",
                "a system origin owns no inbox queue",
            ),
            (
                Origin::External(Vec::new()),
                "ext:",
                "external origin must carry a non-empty submitter id",
            ),
        ] {
            for op in [mark_read(member, 1), clear(member, 1)] {
                let err = inbox
                    .execute(&mut ctx(origin.clone(), 1), &op)
                    .await
                    .expect_err("an unownable origin must be refused");
                assert!(
                    matches!(&err, Error::Module(m) if m.contains(refusal)),
                    "{origin:?} must be refused with {refusal}: {err:?}"
                );
                inbox.abort_block().await.expect("abort");
            }
        }
        assert!(
            queue(&inbox, "chat").await.is_none(),
            "no refused ack staged anything"
        );
    });
}

#[test]
fn module_and_system_origins_deliver_to_any_member() {
    block_on(async {
        // a module/system follow-up delivers to a queue it does not own —
        // the module's entire purpose, and unrestricted on purpose: that is
        // the path every cross-module notification rides.
        let mut inbox = fresh();
        let alice = queue_of(ALICE_KEY);
        for (height, origin) in [Origin::Module("chat".into()), Origin::System]
            .into_iter()
            .enumerate()
        {
            inbox
                .execute(&mut ctx(origin, height as u64), &deliver(&alice, "k", "b"))
                .await
                .expect("a module/system origin delivers to any member");
        }
        inbox.commit_block().await.expect("commit");
        let (next, items) = queue(&inbox, &alice).await.expect("alice exists");
        assert_eq!((next, items.len()), (3, 2));
    });
}

#[test]
fn external_origin_delivering_to_another_members_queue_is_refused() {
    block_on(async {
        // the acl table is empty/open at genesis, so an external signed op
        // reaches Deliver directly — it must not be able to mint a
        // fabricated member (the one-way MAX_MEMBERS cap) or flood a real
        // member's queue (evicting their genuine notifications).
        let mut inbox = fresh();
        let alice = queue_of(ALICE_KEY);

        let err = inbox
            .execute(
                &mut submitter(STRANGER_KEY, 1),
                &deliver(&alice, "k", "flood"),
            )
            .await
            .expect_err("a stranger delivering to another member's queue must be refused");
        assert!(
            matches!(&err, Error::Module(m) if m.contains("may only deliver to its own queue")),
            "unexpected refusal: {err:?}"
        );

        // to its OWN queue: accepted.
        inbox
            .execute(
                &mut submitter(STRANGER_KEY, 2),
                &deliver(&queue_of(STRANGER_KEY), "k", "self"),
            )
            .await
            .expect("self-delivery is permitted");

        inbox.commit_block().await.expect("commit");
        assert!(
            queue(&inbox, &alice).await.is_none(),
            "the refused delivery never touched alice's queue"
        );
        let (_, items) = queue(&inbox, &queue_of(STRANGER_KEY))
            .await
            .expect("the stranger's own queue exists");
        assert_eq!(items.len(), 1, "only the self-delivery landed");
    });
}

// ---- the member counter falls -----------------------------------------------

#[test]
fn clear_that_empties_a_queue_frees_its_member_slot() {
    block_on(async {
        let mut inbox = fresh();
        let alice = queue_of(ALICE_KEY);

        // fill the member cap: MAX_MEMBERS-1 module-delivered members, plus
        // alice's own single self-delivered item.
        for i in 0..MAX_MEMBERS as u64 - 1 {
            let member = format!("m{i}");
            inbox
                .execute(&mut sys(0), &deliver(&member, "k", ""))
                .await
                .expect("deliver to fresh member");
        }
        inbox
            .execute(&mut submitter(ALICE_KEY, 0), &deliver(&alice, "k", "hi"))
            .await
            .expect("alice self-delivers");
        inbox.commit_block().await.expect("commit fill");
        assert_eq!(inbox.member_count_view().await.unwrap(), MAX_MEMBERS as u64);

        let err = inbox
            .execute(&mut sys(0), &deliver("overflow", "k", ""))
            .await
            .expect_err("the cap is full");
        assert!(matches!(err, Error::Module(ref m) if m.contains("member capacity")));

        // alice clears her ONLY item: the queue empties, its meta record is
        // deleted, and the member count falls by exactly one.
        inbox
            .execute(&mut submitter(ALICE_KEY, 1), &clear(&alice, 1))
            .await
            .expect("alice clears her own queue");
        inbox.commit_block().await.expect("commit clear");
        assert!(
            queue(&inbox, &alice).await.is_none(),
            "an emptied queue's meta record is deleted"
        );
        assert_eq!(
            inbox.member_count_view().await.unwrap(),
            MAX_MEMBERS as u64 - 1,
            "the member counter fell"
        );

        // the freed slot admits a fresh member again, right at the cap.
        inbox
            .execute(&mut sys(2), &deliver("newcomer", "k", ""))
            .await
            .expect("a fresh member is admitted again at the cap");
        inbox.commit_block().await.expect("commit newcomer");
        assert_eq!(inbox.member_count_view().await.unwrap(), MAX_MEMBERS as u64);

        // the cap is full again (newcomer took the freed slot), so even
        // alice's own re-delivery is rejected until another slot frees up —
        // her old slot is gone, not reserved for her.
        let err = inbox
            .execute(&mut submitter(ALICE_KEY, 3), &deliver(&alice, "k", "again"))
            .await
            .expect_err("the freed slot was already spent on newcomer");
        assert!(matches!(err, Error::Module(ref m) if m.contains("member capacity")));
    });
}

// ---- overflow eviction stays visible -----------------------------------------

#[test]
fn overflow_eviction_is_counted_per_member() {
    block_on(async {
        let mut inbox = fresh();
        let cap = MAX_ITEMS_PER_MEMBER as u64;
        // two over the cap: two drops.
        for i in 0..cap + 2 {
            inbox
                .execute(&mut sys(i), &deliver("alice", "k", "b"))
                .await
                .expect("deliver");
        }
        inbox.commit_block().await.expect("commit");
        assert_eq!(
            inbox.evicted_count("alice").await.unwrap(),
            2,
            "two overflow drops must be counted, not silent"
        );
        assert_eq!(
            inbox.evicted_count("bob").await.unwrap(),
            0,
            "bob never overflowed"
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

// ---- MarkRead is O(1), never O(queue length) --------------------------------

/// a queue at the cap marks read in ONE call: `stage_mark_read` reads and
/// writes the member's META record exactly once, never one distinct read per
/// item — the defect that made a full 4096-item queue unmarkable under the
/// wasm host's 4096-distinct-store-read budget (issue #1742). This is a
/// native test so the wasm host's budget is not itself in play; the property
/// under test is that the fix touches meta only: the watermark lands and
/// every item — including the newest, seq == cap — reads back as read.
#[test]
fn mark_read_on_a_full_queue_costs_one_meta_read_and_write() {
    block_on(async {
        let mut inbox = fresh();
        let alice = queue_of(ALICE_KEY);
        let cap = MAX_ITEMS_PER_MEMBER as u64;
        for i in 0..cap {
            inbox
                .execute(&mut submitter(ALICE_KEY, i), &deliver(&alice, "k", "b"))
                .await
                .expect("deliver");
        }
        inbox.commit_block().await.expect("commit fill");
        let (_, items) = queue(&inbox, &alice).await.expect("alice exists");
        assert_eq!(items.len(), MAX_ITEMS_PER_MEMBER, "the queue holds the cap");

        // ONE MarkRead call covers the whole queue.
        inbox
            .execute(&mut submitter(ALICE_KEY, cap), &mark_read(&alice, cap))
            .await
            .expect("mark read the full queue in one call");
        inbox.commit_block().await.expect("commit mark read");

        assert_eq!(
            inbox.read_watermark_view(&alice).await.unwrap(),
            cap,
            "the watermark landed at the cap"
        );
        for seq in [1, cap / 2, cap] {
            assert!(
                inbox.is_read(&alice, seq).await.unwrap(),
                "seq {seq} reads as read via the watermark"
            );
        }
    });
}
