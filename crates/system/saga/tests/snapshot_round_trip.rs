//! snapshot/install round-trip for the saga ledger: committed continuation
//! state covering EVERY status (`Pending`, `Done`, `Failed`, `TimedOut`,
//! `Cancelled`), every origin shape, and every optional field — built through
//! the real ordered-op path — crosses to a fresh module as canonical bytes and
//! re-derives the identical root, with query parity on every saga. the bytes
//! arrive UNTRUSTED (a byzantine peer serves them), so the flip side is
//! exercised too: tampered, truncated, padded, misordered, and
//! bad-discriminant snapshots are rejected and the target module is left
//! byte-identical to before the call.

use futures::executor::block_on;
use saga::SagaModule;
use saga::{
    SagaMsg, SagaOrigin, SagaQuery, SagaReply, SagaStatus, SagaView, decode_reply, encode_msg,
    encode_query,
};
use sdk::{Ctx, Env, Error, Event, Module, Msg, Origin, StateRoot};
use valset::{ValsetReply, encode_reply as valset_encode_reply};

/// a minimal `Ctx`: drives `execute` with a controllable env, resolves a
/// known module for reply_to validation, and serves a canned validator set
/// (the worker half is out of scope — oracle results re-enter as hand-built
/// ops).
struct TestCtx {
    env: Env,
    validators: Vec<Vec<u8>>,
}
impl TestCtx {
    fn new(height: u64, origin: Origin) -> Self {
        Self {
            env: Env {
                protocol_version: 0,
                height,
                consensus_time: height,
                origin,
                me: "saga".into(),
            },
            validators: vec![vec![7u8; 32], vec![9u8; 32]],
        }
    }
}
#[async_trait::async_trait(?Send)]
impl Ctx for TestCtx {
    fn env(&self) -> &Env {
        &self.env
    }
    fn module_root(&self, target: &str) -> Option<StateRoot> {
        (target == "agent").then_some(StateRoot::ZERO)
    }
    async fn query(&self, _t: &str, _r: &[u8]) -> Result<Vec<u8>, Error> {
        Ok(valset_encode_reply(&ValsetReply::Validators(
            self.validators.clone(),
        )))
    }
    fn emit_msg(&mut self, _m: Msg) {}
    fn emit_event(&mut self, _e: Event) {}
}

fn exec(m: &mut SagaModule, height: u64, origin: Origin, op: &SagaMsg) {
    let msg = Msg {
        target: "saga".into(),
        payload: encode_msg(op),
    };
    block_on(m.execute(&mut TestCtx::new(height, origin), &msg)).unwrap();
}

fn get(m: &SagaModule, id: &str) -> Option<SagaView> {
    let reply = block_on(m.query(&encode_query(&SagaQuery::Get { saga_id: id.into() }))).unwrap();
    match decode_reply(&reply).unwrap() {
        SagaReply::Saga(v) => v,
        other => panic!("expected Saga reply, got {other:?}"),
    }
}

fn trigger(id: &str, reply_to: Option<&str>, max_attempts: u32, deadline: Option<u64>) -> SagaMsg {
    SagaMsg::Trigger {
        pinned_assignee: None,
        saga_id: id.into(),
        spec: format!("spec:{id}").into_bytes(),
        reply_to: reply_to.map(String::from),
        reply_payload: format!("corr:{id}").into_bytes(),
        deadline,
        max_attempts,
        lease_views: Some(4),
        capability: None,
    }
}

/// a source holding one committed saga in EVERY status, with every origin
/// shape and every optional field populated somewhere, built through the real
/// execute path — never by poking internals. leases and assignees are live
/// (the ctx serves a two-validator set).
fn source() -> SagaModule {
    let mut m =
        SagaModule::with_assignment("saga", "valset", "capability", saga::LeasePolicy::Open);
    let alice = Origin::External(b"alice".to_vec());

    exec(
        &mut m,
        1,
        alice.clone(),
        &trigger("s-cancelled", None, 1, None),
    );
    exec(
        &mut m,
        1,
        Origin::Module("agent".into()),
        &trigger("s-done", Some("agent"), 1, None),
    );
    exec(
        &mut m,
        1,
        alice.clone(),
        &trigger("s-failed", Some("agent"), 2, Some(50)),
    );
    exec(
        &mut m,
        1,
        Origin::System,
        &trigger("s-pending", None, 3, Some(90)),
    );
    exec(
        &mut m,
        1,
        alice.clone(),
        &trigger("s-timedout", None, 1, Some(2)),
    );
    // a capability-tagged saga: the tag is committed state and must survive
    // the round trip (the ctx serves no capability-registry reply, so the
    // attempt simply assigns nobody — the tag itself is what's under test
    // here).
    exec(
        &mut m,
        1,
        alice.clone(),
        &SagaMsg::Trigger {
            pinned_assignee: None,
            saga_id: "s-tagged".into(),
            spec: b"tagged-spec".to_vec(),
            reply_to: None,
            reply_payload: Vec::new(),
            deadline: None,
            max_attempts: 1,
            lease_views: None,
            capability: Some("alpha".into()),
        },
    );
    block_on(m.commit_block()).unwrap();

    exec(
        &mut m,
        2,
        Origin::External(b"oracle".to_vec()),
        &SagaMsg::OracleResult {
            saga_id: "s-done".into(),
            attempt: 0,
            outcome: Ok(b"agreed-answer".to_vec()),
            usage: None,
        },
    );
    exec(
        &mut m,
        2,
        Origin::External(b"oracle".to_vec()),
        &SagaMsg::OracleResult {
            saga_id: "s-failed".into(),
            attempt: 0,
            outcome: Err("first worker crashed".into()),
            usage: None,
        },
    );
    exec(
        &mut m,
        2,
        alice.clone(),
        &SagaMsg::Cancel {
            saga_id: "s-cancelled".into(),
        },
    );
    block_on(m.commit_block()).unwrap();

    // the second attempt of s-failed fails too -> terminal Failed with a
    // stored error; the crank at view 5 times s-timedout out past deadline 2.
    exec(
        &mut m,
        3,
        Origin::External(b"oracle".to_vec()),
        &SagaMsg::OracleResult {
            saga_id: "s-failed".into(),
            attempt: 1,
            outcome: Err("second worker crashed".into()),
            usage: None,
        },
    );
    block_on(m.commit_block()).unwrap();
    exec(
        &mut m,
        5,
        Origin::External(b"cranker".to_vec()),
        &SagaMsg::Crank {},
    );
    block_on(m.commit_block()).unwrap();
    m
}

#[test]
fn installed_snapshot_reconstructs_root_and_reads_across_every_status() {
    let src = source();
    let src_root = src.root();
    assert_ne!(src_root, StateRoot::ZERO, "source must have a real root");
    let snap = src.snapshot();

    // the source really covers the whole status space (and the field space:
    // assignee/lease from the valset, deadline, result, error, origins).
    let statuses: Vec<SagaStatus> = [
        "s-pending",
        "s-done",
        "s-failed",
        "s-timedout",
        "s-cancelled",
    ]
    .iter()
    .map(|id| get(&src, id).unwrap().status)
    .collect();
    assert_eq!(
        statuses,
        vec![
            SagaStatus::Pending,
            SagaStatus::Done,
            SagaStatus::Failed,
            SagaStatus::TimedOut,
            SagaStatus::Cancelled,
        ]
    );
    let pending = get(&src, "s-pending").unwrap();
    assert_eq!(pending.origin, SagaOrigin::System);
    assert!(
        pending.assignee.is_some(),
        "the valset assigned a lease holder"
    );
    assert!(pending.lease_expires_at.is_some());
    let failed = get(&src, "s-failed").unwrap();
    assert_eq!(failed.attempt, 1, "the failed saga consumed both attempts");
    assert_eq!(failed.error, Some("second worker crashed".to_string()));
    assert_eq!(
        get(&src, "s-done").unwrap().origin,
        SagaOrigin::Module("agent".into())
    );
    assert_eq!(
        get(&src, "s-cancelled").unwrap().origin,
        SagaOrigin::External(b"alice".to_vec())
    );

    // the joiner has UNCOMMITTED staged work of its own: install must drop it —
    // a snapshot describes a block boundary, nothing staged may shadow it.
    let mut dst = SagaModule::new("saga");
    exec(
        &mut dst,
        0,
        Origin::System,
        &trigger("s-staged", None, 1, None),
    );

    dst.install(&snap, src_root).unwrap();

    // THE PROPERTY: identical root — the app-hash linkage a joiner needs.
    assert_eq!(
        dst.root(),
        src_root,
        "installed root must equal the source root"
    );

    // query parity, saga by saga, across every status and every field.
    for id in [
        "s-pending",
        "s-done",
        "s-failed",
        "s-timedout",
        "s-cancelled",
    ] {
        assert_eq!(get(&dst, id), get(&src, id), "query parity for {id}");
    }
    assert_eq!(
        get(&dst, "s-done").unwrap().result,
        Some(b"agreed-answer".to_vec())
    );

    // the pre-install staged overlay is gone, not merged.
    assert_eq!(
        get(&dst, "s-staged"),
        None,
        "install must clear the staged overlay"
    );
}

#[test]
fn tampered_snapshot_is_rejected_and_leaves_state_untouched() {
    let src = source();
    let src_root = src.root();
    let snap = src.snapshot();

    // the target already has COMMITTED state of its own, so "untouched" is
    // observable through both root and query.
    let mut dst = SagaModule::new("saga");
    exec(
        &mut dst,
        0,
        Origin::System,
        &trigger("local", None, 1, None),
    );
    block_on(dst.commit_block()).unwrap();
    let before_root = dst.root();
    let before_view = get(&dst, "local");

    // flip one byte inside the last saga's trailing field: the bytes still
    // DECODE, but the re-derived root cannot match the agreed one.
    let mut forged = snap.clone();
    let last = forged.len() - 1;
    forged[last] ^= 0xff;
    assert!(
        dst.install(&forged, src_root).is_err(),
        "a forged payload must be rejected"
    );
    assert_eq!(
        dst.root(),
        before_root,
        "failed install must not move the root"
    );
    assert_eq!(
        get(&dst, "local"),
        before_view,
        "failed install must not touch committed state"
    );

    // honest bytes against the WRONG agreed root are equally rejected: install
    // re-derives the root from the decoded temporaries, it never trusts the peer.
    assert!(
        dst.install(&snap, StateRoot::ZERO).is_err(),
        "a mismatched expected root must be rejected"
    );
    assert_eq!(dst.root(), before_root);
    assert_eq!(get(&dst, "local"), before_view);

    // and the failures left the module fully usable: the honest snapshot under
    // the honest root still lands.
    dst.install(&snap, src_root).unwrap();
    assert_eq!(dst.root(), src_root);
    assert_eq!(
        get(&dst, "local"),
        None,
        "install replaces committed state, never merges"
    );
}

#[test]
fn truncated_or_padded_snapshot_is_rejected() {
    let src = source();
    let src_root = src.root();
    let snap = src.snapshot();
    let empty_root = SagaModule::new("saga").root();

    // EVERY strict prefix must fail — no cut point leaves a decodable snapshot,
    // and none of the failures may move the fresh module's root.
    for cut in 0..snap.len() {
        let mut dst = SagaModule::new("saga");
        assert!(
            dst.install(&snap[..cut], src_root).is_err(),
            "a {cut}-byte prefix of a {}-byte snapshot must be rejected",
            snap.len()
        );
        assert_eq!(
            dst.root(),
            empty_root,
            "rejected prefix ({cut} bytes) must not move the root"
        );
    }

    // trailing bytes after a complete snapshot are equally malformed.
    let mut padded = snap.clone();
    padded.push(0);
    let mut dst = SagaModule::new("saga");
    assert!(
        dst.install(&padded, src_root).is_err(),
        "trailing bytes must be rejected"
    );
    assert_eq!(dst.root(), empty_root);

    // a count field claiming more sagas than the bytes carry is caught before
    // anything is built from it.
    let mut inflated = snap.clone();
    inflated[0] = inflated[0].wrapping_add(1); // low byte of the u64-le saga count
    assert!(
        dst.install(&inflated, src_root).is_err(),
        "an inflated saga count must be rejected"
    );
    assert_eq!(dst.root(), empty_root);
}

/// the canonical bytes of a single minimal saga (System origin, empty spec /
/// payload, every option absent), with its id — the fixture the
/// discriminant-tampering tests index into. the layout is pinned by the
/// asserted length: count 8, id len 8 + 1, origin 1, reply_to tag 1,
/// reply_payload len 8, spec len 8, capability tag 1, status 1, attempt 4,
/// max_attempts 4, seven option tags at [45..52) (assignee, pinned_assignee,
/// lease_views, lease_expires_at, deadline, result, error), created_at 8,
/// updated_at 8 = 68 bytes.
fn minimal_snapshot(id: &str) -> Vec<u8> {
    let mut m = SagaModule::new("saga");
    exec(
        &mut m,
        0,
        Origin::System,
        &SagaMsg::Trigger {
            pinned_assignee: None,
            saga_id: id.into(),
            spec: Vec::new(),
            reply_to: None,
            reply_payload: Vec::new(),
            deadline: None,
            max_attempts: 1,
            lease_views: None,
            capability: None,
        },
    );
    block_on(m.commit_block()).unwrap();
    let snap = m.snapshot();
    assert_eq!(
        snap.len(),
        68,
        "the minimal-saga layout this test indexes into"
    );
    snap
}

#[test]
fn unknown_discriminants_and_tags_are_rejected() {
    let empty_root = SagaModule::new("saga").root();
    let snap = minimal_snapshot("a");

    // origin discriminant (byte 17), status discriminant (byte 36), and an
    // option tag (byte 45, the assignee) each admit exactly their known
    // values — a state has ONE valid encoding.
    for (index, what) in [(17usize, "origin"), (36, "status"), (45, "option tag")] {
        let mut bad = snap.clone();
        bad[index] = 9;
        let mut dst = SagaModule::new("saga");
        let err = dst.install(&bad, StateRoot::ZERO).unwrap_err();
        assert!(
            matches!(err, Error::Module(_)),
            "unknown {what} must be rejected"
        );
        assert_eq!(
            dst.root(),
            empty_root,
            "rejected {what} must not move the root"
        );
    }
}

#[test]
fn non_ascending_or_duplicate_ids_are_rejected() {
    // craft count=2 streams from two well-formed single-saga bodies: ids out
    // of order ("b" then "a") and duplicated ("a" twice) must both reject —
    // sorted-unique ids are what make the encoding canonical.
    let body_a = minimal_snapshot("a")[8..].to_vec();
    let body_b = minimal_snapshot("b")[8..].to_vec();

    for (first, second, what) in [
        (&body_b, &body_a, "descending ids"),
        (&body_a, &body_a, "duplicate ids"),
    ] {
        let mut bytes = 2u64.to_le_bytes().to_vec();
        bytes.extend_from_slice(first);
        bytes.extend_from_slice(second);
        let mut dst = SagaModule::new("saga");
        let err = dst.install(&bytes, StateRoot::ZERO).unwrap_err();
        assert!(matches!(err, Error::Module(_)), "{what} must be rejected");
        assert_eq!(dst.root(), SagaModule::new("saga").root());
    }

    // the same two bodies in ASCENDING order are a well-formed stream: the
    // rejection above is the ordering check, not an artifact of the crafting.
    let mut bytes = 2u64.to_le_bytes().to_vec();
    bytes.extend_from_slice(&body_a);
    bytes.extend_from_slice(&body_b);
    let mut dst = SagaModule::new("saga");
    let expected = {
        let mut m = SagaModule::new("saga");
        for id in ["a", "b"] {
            exec(
                &mut m,
                0,
                Origin::System,
                &SagaMsg::Trigger {
                    pinned_assignee: None,
                    saga_id: id.into(),
                    spec: Vec::new(),
                    reply_to: None,
                    reply_payload: Vec::new(),
                    deadline: None,
                    max_attempts: 1,
                    lease_views: None,
                    capability: None,
                },
            );
        }
        block_on(m.commit_block()).unwrap();
        m.root()
    };
    dst.install(&bytes, expected).unwrap();
    assert_eq!(dst.root(), expected);
}
