//! the adapter-port equivalence proof for the inbox cutover: the `inbox` guest
//! component (the NATIVE `inbox` crate compiled to wasm behind `guest-adapter`)
//! and the native `Inbox` module answer the SAME op sequence with the SAME
//! committed state, and their roots move in lockstep (move on commit, hold on
//! no-ops and abort). the module serves NO queries (its read surface is the
//! index guest's job on the derived tier), so the equivalence claim is
//! ROOT-SHAPED: the port persists the native canonical snapshot as one host-KV
//! value (`__state`, with its 32-byte root under `__root`), which makes the
//! wasm root a PURE FUNCTION of the native committed bytes — after every block
//! the wasm root must equal that derivation recomputed from the native host's
//! snapshot. the roots THEMSELVES differ — the host-KV wrapping is a declared
//! state-schema break (revision 2) — and this proof pins that difference so it
//! can never be mistaken for accidental compatibility.
//!
//! the inbox's primary writer is a SIBLING module's follow-up (`emit_msg`), so
//! the op matrix includes a delivery emitted by a stub producer module — the
//! cross-module path the native inbox tests exercise — asserting the wasm port
//! derives the same Module-origin `source`.

use std::collections::BTreeMap;

use host::{BlockContext, FinalizedBlock, Host, MemberOutcome, SubmitError};
use inbox::{Inbox, InboxMsg, MAX_BODY_BYTES, MAX_KIND_BYTES, MAX_MEMBER_BYTES, encode_msg};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};
use sha2::{Digest, Sha256};
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

fn root_of(h: &Host) -> StateRoot {
    h.module_root("inbox").expect("inbox registered")
}

/// the NATIVE host's committed inbox snapshot bytes, captured through the
/// finalized-checkpoint lane (committed state only, never a staged overlay).
fn native_snapshot(h: &Host, height: u64) -> Vec<u8> {
    let snap = h
        .capture_finalized_snapshot(FinalizedBlock {
            height,
            root_hash: h.root_hash(),
        })
        .expect("capture finalized snapshot");
    let module = snap.module("inbox").expect("inbox registered");
    match &module.state_sync {
        StateSyncHandle::SnapshotBytes(bytes) => bytes.clone(),
        other => panic!("inbox must be snapshot-backed: {other:?}"),
    }
}

/// the adapter port's root, recomputed from the native canonical state: the
/// port persists the snapshot under `__state` with its 32-byte root under
/// `__root`, and the wasm root is the host-KV hash over exactly those two
/// pairs. equality against this value proves the wasm committed state is
/// BYTE-IDENTICAL to the native committed state — the old query-matrix
/// equivalence claim, root-shaped.
fn ported_root(native_root: StateRoot, snapshot: &[u8]) -> StateRoot {
    let committed = BTreeMap::from([
        (b"__root".to_vec(), native_root.0.to_vec()),
        (b"__state".to_vec(), snapshot.to_vec()),
    ]);
    StateRoot(Sha256::digest(sdk::hash::encode_pairs(&committed)).into())
}

/// the cross-runtime equivalence at a committed boundary: the wasm root must
/// be the ported derivation of the native committed bytes.
fn assert_state_parity(native: &Host, wasm: &Host, height: u64) {
    let snapshot = native_snapshot(native, height);
    assert_eq!(
        root_of(wasm),
        ported_root(root_of(native), &snapshot),
        "wasm committed state diverged from the native canonical state at block {height}"
    );
}

// ---- hand-encoded expected images ------------------------------------------
//
// the native module's canonical byte layout (the exact root preimage AND the
// snapshot wire): member count, then per member (id, next_seq, item count,
// items ascending by seq), length-prefixed strings and LE u64s throughout.
// spot checks build the EXPECTED committed image and compare it against the
// captured native snapshot — with `assert_state_parity` pinning the wasm side
// to those same bytes, this replaces the old decoded query spot checks.

fn push_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_str(out: &mut Vec<u8>, value: &str) {
    push_u64(out, value.len() as u64);
    out.extend_from_slice(value.as_bytes());
}

/// one item as `(seq, kind, body, source, created_at, read)`.
type ItemBytes<'a> = (u64, &'a str, &'a str, &'a str, u64, bool);

/// the full canonical image for a committed state (members ascending by id).
fn snapshot_bytes(members: &[(&str, u64, &[ItemBytes])]) -> Vec<u8> {
    let mut out = Vec::new();
    push_u64(&mut out, members.len() as u64);
    for (member, next_seq, items) in members {
        push_str(&mut out, member);
        push_u64(&mut out, *next_seq);
        push_u64(&mut out, items.len() as u64);
        for (seq, kind, body, source, created_at, read) in *items {
            push_u64(&mut out, *seq);
            push_str(&mut out, kind);
            push_str(&mut out, body);
            push_str(&mut out, source);
            push_u64(&mut out, *created_at);
            out.push(*read as u8);
        }
    }
    out
}

#[test]
fn same_ops_same_state_roots_in_lockstep_schema_break_pinned() {
    futures::executor::block_on(same_ops_inner());
}

async fn same_ops_inner() {
    let mut native = native_host();
    let mut wasm = wasm_host_();
    let (alice, bob) = (key(0xA1), key(0xB2));

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

        // the equivalence claim after every block: the wasm committed store
        // is exactly {__root, __state} of the native canonical state.
        assert_state_parity(&native, &wasm, height);
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

    // byte-level spot check on the shared final state (parity above pins the
    // wasm side to these same bytes): seq 1 was cleared; seq 2 is read; the
    // producer follow-up carries the EMITTING module as source and the
    // causing block's consensus time; the anonymous external reads "ext:";
    // the post-clear delivery took seq 5 (next_seq never rewinds); bob's
    // source is ext: + lowercase hex of the submitter key.
    let bob_source = format!("ext:{}", "b2".repeat(32));
    let expected = snapshot_bytes(&[
        (
            "alice",
            6,
            &[
                (2, "reply", "yo", "system", 1_002, true),
                (3, "event", "produced", "producer", 1_004, false),
                (4, "note", "self-note", "ext:", 1_005, false),
                (5, "followup", "after clear", "system", 1_011, false),
            ],
        ),
        (
            "bob",
            2,
            &[(1, "mention", "sup", bob_source.as_str(), 1_003, false)],
        ),
    ]);
    assert_eq!(
        native_snapshot(&native, final_height),
        expected,
        "the committed image diverged from the op sequence's expected state"
    );

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
    assert_state_parity(&native, &wasm, 1);

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

        // abort leaves no trace: both roots byte-identical to pre-block, and
        // the wasm store still the ported derivation of the native bytes.
        assert_eq!(root_of(&native), n_before, "native root moved on reject");
        assert_eq!(root_of(&wasm), w_before, "wasm root moved on reject");
        assert_state_parity(&native, &wasm, height);
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
    let alice_source = format!("ext:{}", "a1".repeat(32));

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
    // the committed image pins the staged read-your-writes: the second
    // dispatch saw the staged next_seq (seq 2, not a reused 1) and the third
    // acked the item that only existed staged (seq 1 read) — identically on
    // the wasm side via the ported-root derivation.
    let alice_items: &[ItemBytes] = &[
        (1, "k", "first", alice_source.as_str(), 1_001, true),
        (2, "k", "second", alice_source.as_str(), 1_001, false),
    ];
    assert_eq!(
        native_snapshot(&native, 1),
        snapshot_bytes(&[("alice", 3, alice_items)])
    );
    assert_state_parity(&native, &wasm, 1);

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
    // the accepted member landed (roots moved), the rejected one left nothing:
    // bob holds exactly the accepted "ok" delivery.
    assert_ne!(root_of(&native), n_before);
    assert_ne!(root_of(&wasm), w_before);
    assert_eq!(
        native_snapshot(&native, 2),
        snapshot_bytes(&[
            ("alice", 3, alice_items),
            (
                "bob",
                2,
                &[(1, "k", "ok", alice_source.as_str(), 1_002, false)]
            ),
        ]),
        "a rejected member must leave no trace"
    );
    assert_state_parity(&native, &wasm, 2);
}
