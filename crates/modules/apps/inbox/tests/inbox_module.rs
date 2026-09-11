//! write-path consensus rules of the inbox module. the module serves NO
//! queries (the read surface — paged lists, unread counts — is the index
//! guest's job, `src/index.rs`), so committed state is asserted through
//! `Module::root()` and the testkit-gated record reads (`Inbox::queue_view`
//! and friends); the qmdb continuity proof lives in `tests/sync_round_trip.rs`.
//!
//! identity is stubbed as a fixed directory behind the ctx's sibling-query
//! seam: two keys on alice's account, one on bob's, a program account, a
//! revoked one, and an account number nobody holds.

use attribution::{Actor, AttributionEvent, Change, ChangeKind, Reason, Source, encode_event};
use futures::executor::block_on;
use identity::{
    AccountView, Control, IdentityQuery, IdentityReply, KeyScheme, KeyView, ProgramStanding,
    decode_query as identity_decode_query, encode_reply as identity_encode_reply,
};
use inbox::{
    AccountNumber, Inbox, InboxAssigned, InboxMsg, MAX_ITEMS_PER_ACCOUNT, Notification,
    decode_assigned, encode_msg,
};
use sdk::{Cause, Env, Error, Hop, ItemRef, Module, Msg, Origin, Root};
use sdk_testkit::{MemStore, TestCtx};

const INBOX: &str = "inbox";
const ATTRIBUTION: &str = "attribution";
const IDENTITY: &str = "identity";

const ALICE: AccountNumber = 7;
const BOB: AccountNumber = 9;
const PROGRAM: AccountNumber = 12;
const REVOKED: AccountNumber = 13;
const GHOST: AccountNumber = 99;

/// alice holds two keys (two devices, one inbox); bob one; a stranger's key
/// is bound to no account.
const ALICE_KEY_1: [u8; 4] = [0xa1, 0xa1, 0xa1, 0xa1];
const ALICE_KEY_2: [u8; 4] = [0xa2, 0xa2, 0xa2, 0xa2];
const BOB_KEY: [u8; 4] = [0xb0, 0xb0, 0xb0, 0xb0];
const STRANGER_KEY: [u8; 4] = [0xc3, 0xc3, 0xc3, 0xc3];

fn view(number: AccountNumber, control: Control, keys: &[[u8; 4]]) -> AccountView {
    AccountView {
        number,
        name: format!("account-{number}"),
        control,
        keys: keys
            .iter()
            .map(|key| KeyView {
                scheme: KeyScheme::Ed25519,
                pubkey: key.to_vec(),
                label: None,
                added_at: 0,
            })
            .collect(),
        avatar: None,
        bio: None,
        updated_at: 0,
    }
}

/// the fixed identity directory the tests resolve against.
fn directory() -> Vec<AccountView> {
    vec![
        view(ALICE, Control::Keys, &[ALICE_KEY_1, ALICE_KEY_2]),
        view(BOB, Control::Keys, &[BOB_KEY]),
        view(
            PROGRAM,
            Control::Program {
                controller: ALICE,
                executor: "agent".into(),
                generation: 0,
                standing: ProgramStanding::Active,
            },
            &[],
        ),
        view(REVOKED, Control::Revoked { controller: ALICE }, &[]),
    ]
}

fn identity_stub(req: &[u8]) -> Result<Vec<u8>, Error> {
    let accounts = directory();
    let found = match identity_decode_query(req).map_err(Error::Module)? {
        IdentityQuery::Get { number } => accounts.into_iter().find(|a| a.number == number),
        IdentityQuery::OfKey { key } => accounts
            .into_iter()
            .find(|a| a.keys.iter().any(|k| k.pubkey == key)),
        other => {
            return Err(Error::Module(format!(
                "unexpected identity query {other:?}"
            )));
        }
    };
    Ok(identity_encode_reply(&IdentityReply::Account(found)))
}

fn ctx_with(origin: Origin, consensus_time: u64, cause: Cause) -> TestCtx {
    TestCtx::with_env(Env {
        height: consensus_time,
        consensus_time,
        origin,
        me: INBOX.into(),
        cause,
    })
    .on_query(IDENTITY, identity_stub)
}

fn ctx(origin: Origin, consensus_time: u64) -> TestCtx {
    ctx_with(origin, consensus_time, Cause::Direct)
}

/// the host running attribution's delivery of change `seq` (queue item
/// `item`) here: the source's origin, and the chain the source set.
fn from_attribution(consensus_time: u64, seq: u64, item: u64) -> TestCtx {
    ctx_with(
        Origin::Module(ATTRIBUTION.into()),
        consensus_time,
        Cause::Chain {
            root: Root::Change {
                source: ATTRIBUTION.into(),
                seq,
            },
            hop: Hop::Delivery(ItemRef {
                source: ATTRIBUTION.into(),
                item,
            }),
        },
    )
}

fn submitter(key: [u8; 4], consensus_time: u64) -> TestCtx {
    ctx(Origin::External(key.to_vec()), consensus_time)
}

fn change(seq: u64, recipient: AccountNumber) -> Change {
    Change {
        seq,
        source: Source {
            module: "chat".into(),
            kind: "message".into(),
            object: format!("m{seq}"),
        },
        revision: 1,
        recipient,
        reason: Reason::Mention,
        kind: ChangeKind::Added,
        detail: vec![0xd; 16],
        actor: Actor::Account(BOB),
        cause: Cause::Direct,
        height: seq,
    }
}

fn changed(change: &Change) -> Msg {
    Msg {
        target: INBOX.into(),
        payload: encode_event(&AttributionEvent::Changed(change.clone())),
    }
}

fn admin(msg: InboxMsg) -> Msg {
    Msg {
        target: INBOX.into(),
        payload: encode_msg(&msg),
    }
}

fn mark_read(account: AccountNumber, up_to_seq: u64) -> Msg {
    admin(InboxMsg::MarkRead { account, up_to_seq })
}

fn clear(account: AccountNumber, up_to_seq: u64) -> Msg {
    admin(InboxMsg::Clear { account, up_to_seq })
}

fn fresh() -> Inbox {
    Inbox::new(INBOX, Box::new(MemStore::new()), ATTRIBUTION, IDENTITY)
}

/// deliver `change` as the attribution source would (item = seq, one
/// subscriber) and return the stamp the inbox assigned.
async fn deliver(inbox: &mut Inbox, time: u64, change: &Change) -> Result<InboxAssigned, Error> {
    let mut c = from_attribution(time, change.seq, change.seq);
    inbox.execute(&mut c, &changed(change)).await?;
    let stamp = c.assigned().expect("a delivery stamps");
    Ok(decode_assigned(stamp).expect("stamp decodes"))
}

async fn queue(inbox: &Inbox, account: AccountNumber) -> Option<(u64, Vec<Notification>)> {
    inbox.queue_view(account).await.expect("queue view")
}

/// the compact item shape assertions compare: `(seq, change seq, created_at)`.
fn tuples(items: &[Notification]) -> Vec<(u64, u64, u64)> {
    items
        .iter()
        .map(|n| (n.seq, n.change.seq, n.created_at))
        .collect()
}

#[test]
fn a_human_recipients_change_is_queued_by_reference() {
    block_on(async {
        let mut inbox = fresh();
        let first = change(4, ALICE);
        let second = change(6, ALICE);
        let bobs = change(7, BOB);
        assert_eq!(
            deliver(&mut inbox, 10, &first).await.unwrap(),
            InboxAssigned::Delivered { seq: 1 }
        );
        assert_eq!(
            deliver(&mut inbox, 11, &second).await.unwrap(),
            InboxAssigned::Delivered { seq: 2 }
        );
        assert_eq!(
            deliver(&mut inbox, 12, &bobs).await.unwrap(),
            InboxAssigned::Delivered { seq: 1 }
        );
        inbox.commit_block().await.unwrap();

        // per-account seqs are monotonic from 1 and the inboxes independent;
        // created_at is the block's consensus time; the item is the change
        // by REFERENCE — the detail stays on the canonical record.
        let (next, items) = queue(&inbox, ALICE).await.expect("alice's inbox");
        assert_eq!(next, 3);
        assert_eq!(tuples(&items), vec![(1, 4, 10), (2, 6, 11)]);
        assert_eq!(items[0].account, ALICE);
        assert_eq!(items[0].change, first.reference());
        assert_eq!(items[1].change, second.reference());
        let (next, items) = queue(&inbox, BOB).await.expect("bob's inbox");
        assert_eq!(next, 2);
        assert_eq!(tuples(&items), vec![(1, 7, 12)]);
        assert!(!inbox.is_read(ALICE, 1).await.unwrap());
        assert_eq!(inbox.last_change_view(ALICE).await.unwrap(), 6);
    });
}

#[test]
fn program_and_revoked_recipients_are_ignored_and_a_missing_one_fails() {
    block_on(async {
        let mut inbox = fresh();
        let before = inbox.root();
        for recipient in [PROGRAM, REVOKED] {
            assert_eq!(
                deliver(&mut inbox, 1, &change(1, recipient)).await.unwrap(),
                InboxAssigned::Ignored,
                "account {recipient} holds no human inbox"
            );
            assert!(queue(&inbox, recipient).await.is_none());
        }
        // the program's controller is NOT notified on its behalf.
        assert!(queue(&inbox, ALICE).await.is_none());

        // an account nobody holds cannot be notified: the delivery fails,
        // which the attribution plane keeps as its receipt.
        let mut c = from_attribution(1, 2, 2);
        let err = inbox
            .execute(&mut c, &changed(&change(2, GHOST)))
            .await
            .unwrap_err();
        assert!(
            format!("{err:?}").contains("does not exist"),
            "the failure names its reason: {err:?}"
        );
        inbox.commit_block().await.unwrap();
        assert_eq!(inbox.root(), before, "nothing was staged");
    });
}

#[test]
fn only_the_attribution_source_delivers() {
    block_on(async {
        let mut inbox = fresh();
        let before = inbox.root();
        let payload = changed(&change(1, ALICE));
        for origin in [
            Origin::Module("chat".into()),
            Origin::External(ALICE_KEY_1.to_vec()),
            Origin::Program(PROGRAM),
            Origin::System,
        ] {
            let mut c = ctx(origin.clone(), 1);
            assert!(
                inbox.execute(&mut c, &payload).await.is_err(),
                "{origin:?} cannot mint a notification"
            );
        }
        // and the source's origin carries deliveries only, never admin ops.
        let mut c = from_attribution(1, 1, 1);
        assert!(inbox.execute(&mut c, &mark_read(ALICE, 1)).await.is_err());
        inbox.commit_block().await.unwrap();
        assert_eq!(inbox.root(), before);
        assert!(queue(&inbox, ALICE).await.is_none());
    });
}

#[test]
fn a_repeated_delivery_is_a_duplicate_and_an_older_one_is_refused() {
    block_on(async {
        let mut inbox = fresh();
        assert_eq!(
            deliver(&mut inbox, 1, &change(5, ALICE)).await.unwrap(),
            InboxAssigned::Delivered { seq: 1 }
        );
        inbox.commit_block().await.unwrap();
        let once = inbox.root();

        // the same change again: stamped, nothing staged, no root movement.
        assert_eq!(
            deliver(&mut inbox, 2, &change(5, ALICE)).await.unwrap(),
            InboxAssigned::Duplicate
        );
        inbox.commit_block().await.unwrap();
        assert_eq!(inbox.root(), once);
        let (next, items) = queue(&inbox, ALICE).await.unwrap();
        assert_eq!((next, items.len()), (2, 1));

        // an older change arriving later violates the source's ordering:
        // an error, never a silent skip.
        assert!(deliver(&mut inbox, 3, &change(3, ALICE)).await.is_err());
        inbox.commit_block().await.unwrap();
        assert_eq!(inbox.root(), once);

        // the duplicate gate is per account: bob's first change is new.
        assert_eq!(
            deliver(&mut inbox, 4, &change(5, BOB)).await.unwrap(),
            InboxAssigned::Delivered { seq: 1 }
        );
    });
}

#[test]
fn every_key_of_an_account_shares_its_one_inbox() {
    block_on(async {
        let mut inbox = fresh();
        for seq in 1..=3 {
            deliver(&mut inbox, seq, &change(seq, ALICE)).await.unwrap();
        }
        inbox.commit_block().await.unwrap();

        // alice's first device marks read; her second device clears — the
        // same inbox, addressed by the account, not by either key.
        inbox
            .execute(&mut submitter(ALICE_KEY_1, 4), &mark_read(ALICE, 2))
            .await
            .expect("device one acks");
        inbox.commit_block().await.unwrap();
        assert_eq!(inbox.read_watermark_view(ALICE).await.unwrap(), 2);
        inbox
            .execute(&mut submitter(ALICE_KEY_2, 5), &clear(ALICE, 1))
            .await
            .expect("device two acks");
        inbox.commit_block().await.unwrap();
        let (next, items) = queue(&inbox, ALICE).await.unwrap();
        assert_eq!(next, 4);
        assert_eq!(
            items.iter().map(|n| n.seq).collect::<Vec<_>>(),
            vec![2, 3],
            "device two's clear removed what device one had read"
        );
        assert!(inbox.is_read(ALICE, 2).await.unwrap());
        assert!(!inbox.is_read(ALICE, 3).await.unwrap());
    });
}

#[test]
fn strangers_other_accounts_programs_modules_and_the_system_cannot_ack() {
    block_on(async {
        let mut inbox = fresh();
        deliver(&mut inbox, 1, &change(1, ALICE)).await.unwrap();
        inbox.commit_block().await.unwrap();
        let before = inbox.root();

        let refused: Vec<(&str, Origin)> = vec![
            ("an unbound key", Origin::External(STRANGER_KEY.to_vec())),
            ("another account's key", Origin::External(BOB_KEY.to_vec())),
            ("an empty key", Origin::External(Vec::new())),
            ("alice's own program", Origin::Program(PROGRAM)),
            ("a module", Origin::Module("chat".into())),
            ("the system", Origin::System),
        ];
        for (who, origin) in refused {
            for op in [mark_read(ALICE, 1), clear(ALICE, 1)] {
                let mut c = ctx(origin.clone(), 2);
                assert!(
                    inbox.execute(&mut c, &op).await.is_err(),
                    "{who} cannot ack alice's inbox"
                );
            }
        }
        // the gate holds before the lookup: a stranger naming an inbox that
        // holds nothing is refused the same way, never told it is empty.
        let mut c = submitter(STRANGER_KEY, 2);
        assert!(inbox.execute(&mut c, &mark_read(GHOST, 1)).await.is_err());
        inbox.commit_block().await.unwrap();
        assert_eq!(inbox.root(), before, "a refused ack stages nothing");
        assert_eq!(inbox.read_watermark_view(ALICE).await.unwrap(), 0);
    });
}

#[test]
fn mark_read_and_clear_are_idempotent_and_noop_tolerant() {
    block_on(async {
        let mut inbox = fresh();
        // bob's own empty inbox: a no-op, never an error.
        inbox
            .execute(&mut submitter(BOB_KEY, 1), &mark_read(BOB, 5))
            .await
            .expect("empty inbox mark-read is a no-op");
        inbox
            .execute(&mut submitter(BOB_KEY, 1), &clear(BOB, 5))
            .await
            .expect("empty inbox clear is a no-op");
        inbox.commit_block().await.unwrap();
        let empty = inbox.root();
        assert!(queue(&inbox, BOB).await.is_none());

        for seq in 1..=3 {
            deliver(&mut inbox, seq, &change(seq, ALICE)).await.unwrap();
        }
        inbox.commit_block().await.unwrap();
        assert_ne!(inbox.root(), empty);

        inbox
            .execute(&mut submitter(ALICE_KEY_1, 4), &mark_read(ALICE, 2))
            .await
            .unwrap();
        inbox.commit_block().await.unwrap();
        let marked = inbox.root();
        // re-marking the same range, or a lower one, is byte-identical.
        for up_to in [2, 1] {
            inbox
                .execute(&mut submitter(ALICE_KEY_1, 5), &mark_read(ALICE, up_to))
                .await
                .unwrap();
        }
        inbox.commit_block().await.unwrap();
        assert_eq!(inbox.root(), marked);

        inbox
            .execute(&mut submitter(ALICE_KEY_1, 6), &clear(ALICE, 2))
            .await
            .unwrap();
        inbox.commit_block().await.unwrap();
        let cleared = inbox.root();
        let (next, items) = queue(&inbox, ALICE).await.unwrap();
        assert_eq!(next, 4, "next_seq never rewinds");
        assert_eq!(items.iter().map(|n| n.seq).collect::<Vec<_>>(), vec![3]);
        // clearing an already-cleared prefix stages nothing.
        inbox
            .execute(&mut submitter(ALICE_KEY_1, 7), &clear(ALICE, 2))
            .await
            .unwrap();
        inbox.commit_block().await.unwrap();
        assert_eq!(inbox.root(), cleared);
    });
}

#[test]
fn a_cleared_inbox_keeps_its_numbering_and_its_duplicate_gate() {
    block_on(async {
        let mut inbox = fresh();
        for seq in 1..=2 {
            deliver(&mut inbox, seq, &change(seq, ALICE)).await.unwrap();
        }
        inbox
            .execute(&mut submitter(ALICE_KEY_1, 3), &clear(ALICE, u64::MAX))
            .await
            .unwrap();
        inbox.commit_block().await.unwrap();
        let (next, items) = queue(&inbox, ALICE).await.unwrap();
        assert_eq!(
            (next, items.len()),
            (3, 0),
            "the meta record outlives its items"
        );

        // the last change is still a duplicate; the next continues at seq 3.
        assert_eq!(
            deliver(&mut inbox, 4, &change(2, ALICE)).await.unwrap(),
            InboxAssigned::Duplicate
        );
        assert_eq!(
            deliver(&mut inbox, 4, &change(3, ALICE)).await.unwrap(),
            InboxAssigned::Delivered { seq: 3 }
        );
    });
}

#[test]
fn queue_overflow_drops_oldest_and_counts_the_eviction() {
    block_on(async {
        let mut inbox = fresh();
        let cap = MAX_ITEMS_PER_ACCOUNT as u64;
        for seq in 1..=cap + 1 {
            deliver(&mut inbox, seq, &change(seq, ALICE)).await.unwrap();
        }
        inbox.commit_block().await.unwrap();
        let (next, items) = queue(&inbox, ALICE).await.unwrap();
        assert_eq!(next, cap + 2);
        assert_eq!(items.len(), MAX_ITEMS_PER_ACCOUNT);
        assert_eq!(items.first().map(|n| n.seq), Some(2), "seq 1 was dropped");
        assert_eq!(items.last().map(|n| n.seq), Some(cap + 1));
        assert_eq!(inbox.evicted_count(ALICE).await.unwrap(), 1);
        assert_eq!(inbox.evicted_count(BOB).await.unwrap(), 0);
    });
}

#[test]
fn seq_exhaustion_rejects_deterministically() {
    block_on(async {
        let mut inbox = fresh();
        inbox.testkit_saturate_seq(ALICE).await.unwrap();
        inbox.commit_block().await.unwrap();
        let before = inbox.root();
        assert!(deliver(&mut inbox, 1, &change(1, ALICE)).await.is_err());
        inbox.commit_block().await.unwrap();
        assert_eq!(inbox.root(), before);
    });
}

#[test]
fn mark_read_beyond_the_last_seq_does_not_pre_read_future_deliveries() {
    block_on(async {
        let mut inbox = fresh();
        deliver(&mut inbox, 1, &change(1, ALICE)).await.unwrap();
        inbox
            .execute(&mut submitter(ALICE_KEY_1, 2), &mark_read(ALICE, u64::MAX))
            .await
            .unwrap();
        assert_eq!(inbox.read_watermark_view(ALICE).await.unwrap(), 1);
        deliver(&mut inbox, 3, &change(2, ALICE)).await.unwrap();
        inbox.commit_block().await.unwrap();
        assert!(inbox.is_read(ALICE, 1).await.unwrap());
        assert!(!inbox.is_read(ALICE, 2).await.unwrap());
    });
}

#[test]
fn an_oversized_reference_is_refused_before_staging() {
    block_on(async {
        let mut inbox = fresh();
        let before = inbox.root();
        let mut oversized = change(1, ALICE);
        // the reference carries the reason; a defined reason the store's
        // codec cannot hold is refused whole, and nothing of the delivery
        // (not even the account's meta) is staged.
        oversized.reason = Reason::Defined("r".repeat(sdk::MAX_STORE_VALUE_BYTES));
        assert!(deliver(&mut inbox, 1, &oversized).await.is_err());
        inbox.commit_block().await.unwrap();
        assert_eq!(inbox.root(), before);
        assert!(queue(&inbox, ALICE).await.is_none());
    });
}

#[test]
fn root_moves_only_on_commit_and_abort_leaves_no_trace() {
    block_on(async {
        let mut inbox = fresh();
        let genesis = inbox.root();
        deliver(&mut inbox, 1, &change(1, ALICE)).await.unwrap();
        assert_eq!(
            inbox.root(),
            genesis,
            "staged writes are invisible to root()"
        );
        inbox.abort_block().await.unwrap();
        assert_eq!(inbox.root(), genesis);
        assert!(
            queue(&inbox, ALICE).await.is_none(),
            "abort discards the overlay"
        );

        deliver(&mut inbox, 2, &change(1, ALICE)).await.unwrap();
        inbox.commit_block().await.unwrap();
        assert_ne!(inbox.root(), genesis);
        let (next, items) = queue(&inbox, ALICE).await.unwrap();
        assert_eq!((next, items.len()), (2, 1));
    });
}
