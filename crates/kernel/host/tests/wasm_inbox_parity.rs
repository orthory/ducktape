//! Compiled inbox ingestion and account administration agree with native
//! state roots, stamps, rollback and source authentication.

use attribution::{Actor, AttributionEvent, Change, ChangeKind, Reason, Source, encode_event};
use futures::executor::block_on;
use identity::{
    AccountView, Control, IdentityQuery, IdentityReply, KeyScheme, KeyView, ProgramStanding,
    decode_query as identity_decode_query, encode_reply as identity_encode_reply,
};
use inbox::{AccountNumber, Inbox, InboxAssigned, InboxMsg, decode_assigned, encode_msg};
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

const COMPONENT: &[u8] = include_bytes!("fixtures/inbox.component.wasm");

struct Pair {
    native: Inbox,
    wasm: wasm_host::WasmModule,
}

impl Pair {
    fn new() -> Self {
        Self {
            native: Inbox::new(INBOX, Box::new(MemStore::new()), ATTRIBUTION, IDENTITY),
            wasm: wasm_host::WasmModule::with_store(INBOX, COMPONENT, Box::new(MemStore::new()))
                .unwrap(),
        }
    }

    async fn apply(
        &mut self,
        mut native: TestCtx,
        mut wasm: TestCtx,
        msg: Msg,
    ) -> Result<Option<InboxAssigned>, Error> {
        let a = self.native.execute(&mut native, &msg).await;
        let b = self.wasm.execute(&mut wasm, &msg).await;
        assert_eq!(a, b);
        assert_eq!(native.assigned(), wasm.assigned());
        assert_eq!(native.output(), wasm.output());
        assert_eq!(native.msgs(), wasm.msgs());
        assert_eq!(native.events(), wasm.events());
        a?;
        Ok(native
            .assigned()
            .map(|bytes| decode_assigned(bytes).unwrap()))
    }

    async fn deliver(
        &mut self,
        time: u64,
        change: &Change,
    ) -> Result<Option<InboxAssigned>, Error> {
        self.apply(
            from_attribution(time, change.seq, change.seq),
            from_attribution(time, change.seq, change.seq),
            changed(change),
        )
        .await
    }

    async fn commit(&mut self) {
        self.native.commit_block().await.unwrap();
        self.wasm.commit_block().await.unwrap();
        assert_eq!(self.native.root(), self.wasm.root());
    }
}

#[test]
fn human_queue_is_shared_across_keys_and_preserves_sequence_after_clear() {
    block_on(async {
        let mut pair = Pair::new();
        let mut first = change(4, ALICE);
        first.detail = vec![0xAA; 1 << 20];
        assert_eq!(
            pair.deliver(10, &first).await.unwrap(),
            Some(InboxAssigned::Delivered { seq: 1 })
        );
        assert_eq!(
            pair.deliver(11, &change(6, ALICE)).await.unwrap(),
            Some(InboxAssigned::Delivered { seq: 2 })
        );
        pair.commit().await;
        let before = pair.native.root();
        assert_eq!(
            pair.deliver(12, &change(6, ALICE)).await.unwrap(),
            Some(InboxAssigned::Duplicate)
        );
        pair.commit().await;
        assert_eq!(pair.native.root(), before);
        pair.apply(
            submitter(ALICE_KEY_2, 13),
            submitter(ALICE_KEY_2, 13),
            mark_read(ALICE, u64::MAX),
        )
        .await
        .unwrap();
        pair.commit().await;
        let marked = pair.native.root();
        assert_ne!(marked, before);
        pair.apply(
            submitter(ALICE_KEY_1, 14),
            submitter(ALICE_KEY_1, 14),
            mark_read(ALICE, 1),
        )
        .await
        .unwrap();
        pair.commit().await;
        assert_eq!(pair.native.root(), marked);
        pair.apply(
            submitter(ALICE_KEY_2, 15),
            submitter(ALICE_KEY_2, 15),
            clear(ALICE, u64::MAX),
        )
        .await
        .unwrap();
        pair.commit().await;
        assert_eq!(
            pair.deliver(16, &change(7, ALICE)).await.unwrap(),
            Some(InboxAssigned::Delivered { seq: 3 })
        );
        pair.commit().await;
    });
}

#[test]
fn source_spoofs_and_foreign_admins_cannot_write_or_read_a_human_queue() {
    block_on(async {
        let mut pair = Pair::new();
        pair.deliver(1, &change(1, ALICE)).await.unwrap();
        for origin in [
            Origin::External(BOB_KEY.to_vec()),
            Origin::External(STRANGER_KEY.to_vec()),
            Origin::Program(ALICE),
            Origin::Module("other".into()),
            Origin::System,
        ] {
            assert!(
                pair.apply(
                    ctx(origin.clone(), 2),
                    ctx(origin.clone(), 2),
                    mark_read(ALICE, 9)
                )
                .await
                .is_err()
            );
            assert!(
                pair.apply(
                    ctx(origin.clone(), 2),
                    ctx(origin, 2),
                    changed(&change(2, ALICE))
                )
                .await
                .is_err()
            );
        }
        pair.commit().await;
        assert_eq!(
            pair.deliver(3, &change(2, ALICE)).await.unwrap(),
            Some(InboxAssigned::Delivered { seq: 2 })
        );
        pair.commit().await;
        let before = pair.native.root();
        for recipient in [PROGRAM, REVOKED] {
            assert_eq!(
                pair.deliver(4, &change(3, recipient)).await.unwrap(),
                Some(InboxAssigned::Ignored)
            );
        }
        assert!(pair.deliver(4, &change(3, GHOST)).await.is_err());
        pair.commit().await;
        assert_eq!(pair.native.root(), before);
    });
}

#[test]
fn abort_and_a_later_invalid_change_preserve_operation_boundaries() {
    block_on(async {
        let mut pair = Pair::new();
        let empty = pair.native.root();
        pair.deliver(1, &change(1, ALICE)).await.unwrap();
        assert_eq!(pair.native.root(), empty);
        assert_eq!(pair.wasm.root(), empty);
        pair.native.abort_block().await.unwrap();
        pair.wasm.abort_block().await.unwrap();
        pair.commit().await;
        assert_eq!(pair.native.root(), empty);
        pair.deliver(2, &change(4, ALICE)).await.unwrap();
        assert!(pair.deliver(2, &change(3, ALICE)).await.is_err());
        pair.commit().await;
        assert_eq!(
            pair.deliver(3, &change(5, ALICE)).await.unwrap(),
            Some(InboxAssigned::Delivered { seq: 2 })
        );
        pair.commit().await;
    });
}
