//! snapshot/install round-trip for the saga tracker: committed continuation
//! state — one saga parked at `Pending`, another advanced to `Done` through the
//! real ordered-op path — crosses to a fresh module as canonical bytes and
//! re-derives the identical root, with query parity on every saga. the bytes
//! arrive UNTRUSTED (a byzantine peer serves them), so the flip side is
//! exercised too: a tampered, truncated, or padded snapshot is rejected and the
//! target module is left byte-identical to before the call.

use futures::executor::block_on;
use saga::SagaModule;
use saga_interface::{
    decode_reply, encode_msg, encode_query, SagaMsg, SagaQuery, SagaReply, SagaStatus, SagaView,
};
use sdk::{Ctx, Effect, Env, Error, Event, Module, Msg, Origin, StateRoot};

/// a minimal `Ctx`: enough to drive `execute`; effects are dropped (the worker
/// half is out of scope — the oracle result re-enters as a hand-built op).
struct NullCtx {
    env: Env,
}
impl NullCtx {
    fn new() -> Self {
        Self { env: Env { height: 0, consensus_time: 0, origin: Origin::System, me: "saga".into() } }
    }
}
#[async_trait::async_trait(?Send)]
impl Ctx for NullCtx {
    fn env(&self) -> &Env { &self.env }
    fn module_root(&self, _t: &str) -> Option<StateRoot> { None }
    async fn query(&self, _t: &str, _r: &[u8]) -> Result<Vec<u8>, Error> { Err(Error::QueryUnsupported) }
    fn emit_msg(&mut self, _m: Msg) {}
    fn emit_event(&mut self, _e: Event) {}
    fn request_effect(&mut self, _eff: Effect) {}
}

fn trigger(id: &str, spec: &[u8]) -> Msg {
    Msg { target: "saga".into(), payload: encode_msg(&SagaMsg::Trigger { saga_id: id.into(), spec: spec.to_vec() }) }
}
fn oracle(id: &str, result: &[u8]) -> Msg {
    Msg { target: "saga".into(), payload: encode_msg(&SagaMsg::OracleResult { saga_id: id.into(), result: result.to_vec() }) }
}
fn get(m: &SagaModule, id: &str) -> Option<SagaView> {
    let reply = block_on(m.query(&encode_query(&SagaQuery::Get { saga_id: id.into() }))).unwrap();
    match decode_reply(&reply).unwrap() { SagaReply::Saga(v) => v }
}

/// a source with one committed `Pending` saga and one committed `Done` saga,
/// built through the real execute path — never by poking internals.
fn source() -> SagaModule {
    let mut m = SagaModule::new("saga");
    let mut ctx = NullCtx::new();
    block_on(m.execute(&mut ctx, &trigger("s-done", b"work"))).unwrap();
    block_on(m.execute(&mut ctx, &trigger("s-pending", b"work"))).unwrap();
    block_on(m.commit_block()).unwrap();
    block_on(m.execute(&mut ctx, &oracle("s-done", b"agreed-answer"))).unwrap();
    block_on(m.commit_block()).unwrap();
    m
}

#[test]
fn installed_snapshot_reconstructs_root_and_reads() {
    let src = source();
    let src_root = src.root();
    assert_ne!(src_root, StateRoot::ZERO, "source must have a real root");
    let snap = src.snapshot();

    // the joiner has UNCOMMITTED staged work of its own: install must drop it —
    // a snapshot describes a block boundary, nothing staged may shadow it.
    let mut dst = SagaModule::new("saga");
    let mut ctx = NullCtx::new();
    block_on(dst.execute(&mut ctx, &trigger("s-staged", b"doomed"))).unwrap();

    dst.install(&snap, src_root).unwrap();

    // THE PROPERTY: identical root — the app-hash linkage a joiner needs.
    assert_eq!(dst.root(), src_root, "installed root must equal the source root");

    // query parity, saga by saga: Pending stayed Pending, Done kept its result.
    assert_eq!(get(&dst, "s-pending"), get(&src, "s-pending"));
    assert_eq!(get(&dst, "s-done"), get(&src, "s-done"));
    assert_eq!(get(&dst, "s-pending").unwrap().status, SagaStatus::Pending);
    let done = get(&dst, "s-done").unwrap();
    assert_eq!(done.status, SagaStatus::Done);
    assert_eq!(done.result, Some(b"agreed-answer".to_vec()));

    // the pre-install staged overlay is gone, not merged.
    assert_eq!(get(&dst, "s-staged"), None, "install must clear the staged overlay");
}

#[test]
fn tampered_snapshot_is_rejected_and_leaves_state_untouched() {
    let src = source();
    let src_root = src.root();
    let snap = src.snapshot();

    // the target already has COMMITTED state of its own, so "untouched" is
    // observable through both root and query.
    let mut dst = SagaModule::new("saga");
    let mut ctx = NullCtx::new();
    block_on(dst.execute(&mut ctx, &trigger("local", b"mine"))).unwrap();
    block_on(dst.commit_block()).unwrap();
    let before_root = dst.root();
    let before_view = get(&dst, "local");

    // flip one byte inside the last saga's result payload: the bytes still
    // DECODE, but the re-derived root cannot match the agreed one.
    let mut forged = snap.clone();
    let last = forged.len() - 1;
    forged[last] ^= 0xff;
    assert!(dst.install(&forged, src_root).is_err(), "a forged payload must be rejected");
    assert_eq!(dst.root(), before_root, "failed install must not move the root");
    assert_eq!(get(&dst, "local"), before_view, "failed install must not touch committed state");

    // honest bytes against the WRONG agreed root are equally rejected.
    assert!(dst.install(&snap, StateRoot::ZERO).is_err(), "a mismatched expected root must be rejected");
    assert_eq!(dst.root(), before_root);
    assert_eq!(get(&dst, "local"), before_view);

    // and the failures left the module fully usable: the honest snapshot under
    // the honest root still lands.
    dst.install(&snap, src_root).unwrap();
    assert_eq!(dst.root(), src_root);
    assert_eq!(get(&dst, "local"), None, "install replaces committed state, never merges");
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
        assert_eq!(dst.root(), empty_root, "rejected prefix ({cut} bytes) must not move the root");
    }

    // trailing bytes after a complete snapshot are equally malformed.
    let mut padded = snap.clone();
    padded.push(0);
    let mut dst = SagaModule::new("saga");
    assert!(dst.install(&padded, src_root).is_err(), "trailing bytes must be rejected");
    assert_eq!(dst.root(), empty_root);

    // a count field claiming more sagas than the bytes carry is caught before
    // anything is built from it.
    let mut inflated = snap.clone();
    inflated[0] = inflated[0].wrapping_add(1); // low byte of the u64-le saga count
    assert!(dst.install(&inflated, src_root).is_err(), "an inflated saga count must be rejected");
    assert_eq!(dst.root(), empty_root);
}
