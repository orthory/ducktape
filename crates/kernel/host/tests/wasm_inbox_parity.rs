//! the STORE-BACKED cutover-continuity proof for inbox: the `inbox` guest
//! component over `WasmModule::with_store(QmdbStore)` and the native `Inbox`
//! over the same store shape are ROOT-CONTINUOUS — the same op sequence
//! commits the IDENTICAL qmdb merkle root after every block (both roots ARE
//! the store's root). the module serves NO queries (its read surface is the
//! index guest's job on the derived tier), so root equality IS the whole
//! equivalence claim.
//!
//! the inbox's primary writer is a SIBLING module's follow-up (`emit_msg`), so
//! the op matrix includes a delivery emitted by a stub producer module — the
//! cross-module path the native inbox tests exercise — asserting the wasm port
//! derives the same Module-origin `source`.
//!
//! `rejections_inner` additionally pins the ACK OWNER GATE inside the compiled
//! component. `env().origin` is the only authorization input that crosses the
//! WIT boundary, so a gate keyed on it is exactly the kind that can be correct
//! natively and inert in the guest: it has to be checked on both sides.

use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use host::{BlockContext, Host, MemberOutcome, SubmitError};
use inbox::{Inbox, InboxMsg, MAX_BODY_BYTES, MAX_KIND_BYTES, MAX_MEMBER_BYTES, encode_msg};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot};
use statesync::qmdb::QmdbStore;
use wasm_host::WasmModule;

/// GENERATED artifact — built from the `inbox` module's guest port by
/// guest-builder (`make wasm-modules`); committed so this proof is self-contained.
const INBOX_WASM: &[u8] = include_bytes!("fixtures/inbox.component.wasm");

/// a fresh qmdb store. `label` doubles as the store id (the deterministic
/// runtime keys storage partitions by id alone).
async fn inbox_store(
    context: &deterministic::Context,
    label: &'static str,
) -> QmdbStore<deterministic::Context> {
    QmdbStore::init(context.child(label), label).await
}

fn wasm_inbox(store: Box<dyn sdk::MerkleStore>) -> WasmModule {
    WasmModule::with_store("inbox", INBOX_WASM, store).expect("load component")
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
            member: queue_of(&key(0xA1)),
            kind: "event".into(),
            body: "produced".into(),
        }));
        Ok(())
    }
}

async fn native_host(context: &deterministic::Context) -> Host {
    let store = inbox_store(context, "native_inbox").await;
    Host::genesis(vec![
        Box::new(Inbox::new("inbox", Box::new(store))),
        Box::new(Producer),
    ])
    .expect("genesis")
}

async fn wasm_host_(context: &deterministic::Context) -> Host {
    let store = inbox_store(context, "wasm_inbox").await;
    Host::genesis(vec![Box::new(wasm_inbox(Box::new(store))), Box::new(Producer)])
        .expect("genesis")
}

/// a 32-byte submitter key (the ordered lane hands modules verified ed25519
/// ids; the parity claim only needs them distinct and non-empty).
fn key(tag: u8) -> Vec<u8> {
    vec![tag; 32]
}

/// the queue a submitter key owns: a member IS that origin's actor string, and
/// only that origin may ack it.
fn queue_of(k: &[u8]) -> String {
    Origin::External(k.to_vec()).actor_string()
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

fn root_of(h: &Host) -> StateRoot {
    h.module_root("inbox").expect("inbox registered")
}

#[test]
fn same_ops_same_state_roots_in_lockstep_and_continuous() {
    deterministic::Runner::default().start(|context| async move {
        same_ops_inner(&context).await;
    });
}

async fn same_ops_inner(context: &deterministic::Context) {
    let mut native = native_host(context).await;
    let mut wasm = wasm_host_(context).await;
    let (alice, bob) = (key(0xA1), key(0xB2));
    let (alice_q, bob_q) = (queue_of(&alice), queue_of(&bob));
    // a queue nobody has been delivered to, owned by a third key — the
    // unknown-member no-op is the OWNER's own empty queue, not a stranger's.
    let (ghost, ghost_q) = (key(0xE7), queue_of(&key(0xE7)));

    // ROOT-CONTINUITY from GENESIS: both roots are the (empty) store's merkle
    // root, identical across the runtimes — and they stay identical after
    // every block (asserted per block below).
    assert_eq!(
        root_of(&native),
        root_of(&wasm),
        "genesis roots must be continuous across the runtimes"
    );

    // every op family, in one deterministic sequence: deliveries from every
    // origin shape (external, system, module follow-up, anonymous external),
    // MarkRead/Clear incl. their idempotent and unknown-member no-op forms,
    // and a post-clear delivery (next_seq never rewinds). `moves` says whether
    // the op changes committed state — root movement must agree on BOTH sides.
    let ops: Vec<(Origin, Msg, bool)> = vec![
        (
            Origin::External(alice.clone()),
            deliver(&alice_q, "mention", "hi"),
            true,
        ),
        (Origin::System, deliver(&alice_q, "reply", "yo"), true),
        (
            Origin::External(bob.clone()),
            deliver(&bob_q, "mention", "sup"),
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
            deliver(&alice_q, "note", "self-note"),
            true,
        ),
        // the ack family is MEMBER-BOUND: every one of these rides the queue
        // owner's own submitter origin, the only origin the gate admits.
        (
            Origin::External(alice.clone()),
            op(&InboxMsg::MarkRead {
                member: alice_q.clone(),
                up_to_seq: 2,
            }),
            true,
        ),
        // idempotent re-ack: same MarkRead again changes nothing.
        (
            Origin::External(alice.clone()),
            op(&InboxMsg::MarkRead {
                member: alice_q.clone(),
                up_to_seq: 2,
            }),
            false,
        ),
        // unknown member: deterministic no-op, never an error — the owner
        // acking their own queue before anything ever landed in it.
        (
            Origin::External(ghost.clone()),
            op(&InboxMsg::MarkRead {
                member: ghost_q.clone(),
                up_to_seq: 99,
            }),
            false,
        ),
        (
            Origin::External(alice.clone()),
            op(&InboxMsg::Clear {
                member: alice_q.clone(),
                up_to_seq: 1,
            }),
            true,
        ),
        (
            Origin::External(ghost.clone()),
            op(&InboxMsg::Clear {
                member: ghost_q.clone(),
                up_to_seq: 99,
            }),
            false,
        ),
        // next_seq survived the clear: this lands as seq 5, not a reused seq.
        (
            Origin::System,
            deliver(&alice_q, "followup", "after clear"),
            true,
        ),
    ];

    let mut final_height = 0;
    for (height, (origin, msg, moves)) in ops.into_iter().enumerate() {
        let height = height as u64 + 1;
        final_height = height;
        let (n_before, w_before) = (root_of(&native), root_of(&wasm));
        native
            .submit_at(block(height, origin.clone()), msg.clone())
            .await
            .expect("native submit");
        wasm.submit_at(block(height, origin), msg)
            .await
            .expect("wasm submit");

        // roots move in LOCKSTEP: a state-changing op moves both commit
        // boundaries, a no-op holds both...
        if moves {
            assert_ne!(root_of(&native), n_before, "native root stuck at {height}");
            assert_ne!(root_of(&wasm), w_before, "wasm root stuck at {height}");
        } else {
            assert_eq!(root_of(&native), n_before, "native root moved at {height}");
            assert_eq!(root_of(&wasm), w_before, "wasm root moved at {height}");
        }
        // THE continuity property: both roots ARE the same store root.
        assert_eq!(
            root_of(&native),
            root_of(&wasm),
            "the two runtimes diverged at {height}"
        );
    }
    let _ = final_height;

    // the read surface is GONE on both runtimes alike: the native module
    // answers QueryUnsupported and the port refuses too (its wit rendering is
    // its own business; the refusal is the parity claim) — and neither
    // refusal moves a root.
    let (n_settled, w_settled) = (root_of(&native), root_of(&wasm));
    let n_err = native
        .query("inbox", b"{}")
        .await
        .expect_err("the native inbox must refuse queries");
    assert!(
        matches!(n_err, Error::QueryUnsupported),
        "native refusal shape: {n_err:?}"
    );
    wasm.query("inbox", b"{}")
        .await
        .expect_err("the wasm inbox must refuse queries");
    assert_eq!(root_of(&native), n_settled, "a refused query moved the native root");
    assert_eq!(root_of(&wasm), w_settled, "a refused query moved the wasm root");
}

#[test]
fn rejections_match_and_leave_no_trace() {
    deterministic::Runner::default().start(|context| async move {
        rejections_inner(&context).await;
    });
}

async fn rejections_inner(context: &deterministic::Context) {
    let mut native = native_host(context).await;
    let mut wasm = wasm_host_(context).await;
    let (alice, stranger) = (key(0xA1), key(0xC3));
    let alice_q = queue_of(&alice);

    for host in [&mut native, &mut wasm] {
        host.submit_at(
            block(1, Origin::External(alice.clone())),
            deliver(&alice_q, "k", "seed"),
        )
        .await
        .expect("seed deliver");
    }
    assert_eq!(root_of(&native), root_of(&wasm));

    // the rejection matrix: every cap-violation family the native module
    // rejects at execute, a malformed payload (the decode seam), and the ACK
    // OWNER GATE — which is the one rule here whose only input crosses the WIT
    // boundary, so it is proven in the compiled component and not just
    // natively. each rejected block must leave BOTH roots byte-identical (the
    // abort path: staged writes discarded, no trace).
    // (who submits, which queue they name, why it is refused). the STRANGER
    // names alice's queue — the reported defect, an unattributed wipe of
    // another member's whole notification history. the other three name the
    // queue called after themselves, so their refusal is about the origin KIND
    // and can never be mistaken for a mismatch.
    let unowned: [(Origin, String, &str); 4] = [
        (
            Origin::External(stranger.clone()),
            alice_q.clone(),
            "only the queue's own member may ack it",
        ),
        (
            Origin::Module("chat".into()),
            "chat".to_string(),
            "a module origin owns no inbox queue",
        ),
        (
            Origin::System,
            "system".to_string(),
            "a system origin owns no inbox queue",
        ),
        (
            Origin::External(Vec::new()),
            "ext:".to_string(),
            "external origin must carry a non-empty submitter id",
        ),
    ];
    let ack_gate: Vec<(Origin, Msg, &str)> = unowned
        .into_iter()
        .flat_map(|(origin, member, needle)| {
            [
                (
                    origin.clone(),
                    op(&InboxMsg::MarkRead {
                        member: member.clone(),
                        up_to_seq: 1,
                    }),
                    needle,
                ),
                (
                    origin,
                    op(&InboxMsg::Clear {
                        member,
                        up_to_seq: 1,
                    }),
                    needle,
                ),
            ]
        })
        .collect();

    let mut rejects: Vec<(Origin, Msg, &str)> = vec![
        (
            Origin::System,
            deliver("", "k", "b"),
            "member must not be empty",
        ),
        (
            Origin::System,
            deliver(&"m".repeat(MAX_MEMBER_BYTES + 1), "k", "b"),
            "member exceeds 256 bytes",
        ),
        (
            Origin::System,
            deliver(&alice_q, &"k".repeat(MAX_KIND_BYTES + 1), "b"),
            "kind exceeds 64 bytes",
        ),
        (
            Origin::System,
            deliver(&alice_q, "k", &"x".repeat(MAX_BODY_BYTES + 1)),
            "body exceeds 16384 bytes",
        ),
        (
            Origin::System,
            Msg {
                target: "inbox".into(),
                payload: b"definitely-not-json".to_vec(),
            },
            "expected value",
        ),
    ];
    rejects.extend(ack_gate);

    for (height, (origin, msg, needle)) in rejects.into_iter().enumerate() {
        let height = height as u64 + 2;
        let (n_before, w_before) = (root_of(&native), root_of(&wasm));

        let n_err = native
            .submit_at(block(height, origin.clone()), msg.clone())
            .await
            .expect_err("native must reject");
        let w_err = wasm
            .submit_at(block(height, origin), msg)
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

        // abort leaves no trace: both roots byte-identical to pre-block and
        // continuous across the runtimes.
        assert_eq!(root_of(&native), n_before, "native root moved on reject");
        assert_eq!(root_of(&wasm), w_before, "wasm root moved on reject");
        assert_eq!(root_of(&native), root_of(&wasm));
    }
}

#[test]
fn multi_dispatch_block_reads_prior_writes_and_isolates_rejections() {
    deterministic::Runner::default().start(|context| async move {
        multi_dispatch_inner(&context).await;
    });
}

async fn multi_dispatch_inner(context: &deterministic::Context) {
    let mut native = native_host(context).await;
    let mut wasm = wasm_host_(context).await;
    let (alice, carol) = (key(0xA1), key(0xC3));
    let (alice_q, bob_q) = (queue_of(&alice), queue_of(&key(0xB2)));

    // ONE block, three ops: the second delivery's seq assignment READS the
    // first op's staged write (next_seq only exists in this block's overlay),
    // and the MarkRead acks an item that only exists staged. on the wasm side
    // that is the outer staged `__state` being reloaded by each later dispatch
    // — the read-your-writes seam the adapter relies on.
    let batch = vec![
        (
            Origin::External(alice.clone()),
            deliver(&alice_q, "k", "first"),
        ),
        (
            Origin::External(alice.clone()),
            deliver(&alice_q, "k", "second"),
        ),
        (
            Origin::External(alice.clone()),
            op(&InboxMsg::MarkRead {
                member: alice_q.clone(),
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
    // the read-your-writes seam decided identically on both runtimes: the
    // second dispatch saw the staged next_seq and the third acked the item
    // that only existed staged — root continuity is the whole claim (the
    // record-level pins live in the module's own tests).
    assert_eq!(root_of(&native), root_of(&wasm));

    // ONE block where the SECOND member rejects: the runtime aborts the staged
    // overlay and replays the accepted member — committed state must equal the
    // accepted subset alone, on both runtimes.
    let (n_before, w_before) = (root_of(&native), root_of(&wasm));
    let batch = vec![
        (Origin::External(alice.clone()), deliver(&bob_q, "k", "ok")),
        (
            Origin::External(carol.clone()),
            deliver(&bob_q, "k", &"x".repeat(MAX_BODY_BYTES + 1)),
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
    // the accepted member landed (roots moved), the rejected one left
    // nothing — identically on both runtimes.
    assert_ne!(root_of(&native), n_before);
    assert_ne!(root_of(&wasm), w_before);
    assert_eq!(root_of(&native), root_of(&wasm));
}
