//! the saga tracker — the DETERMINISTIC half of the async engine.
//!
//! a pure state-machine module (in the app-hash) that records async work in
//! flight. it mirrors `directory`'s shape exactly: an in-memory `BTreeMap` with a
//! `pending` overlay staged during a block and merged at the boundary, and a
//! state-based `root()`. what's new is the async control flow it drives:
//!
//! - a `Trigger` op records a `Pending` saga and emits a `WorkerRequest` EFFECT —
//!   a request for the host-owned worker to do the non-deterministic work
//!   (an LLM call, a fetch, a commit). the module does NOT do that work; it only
//!   asks. determinism is preserved: the saga sits at `Pending` on every node.
//! - an `OracleResult` op — submitted by that worker back through the NORMAL op
//!   path once it has a result — advances the saga to `Done`. because it arrives
//!   as an ordered op, every validator applies the identical agreed result. the
//!   worker's non-determinism is laundered through consensus before it touches
//!   state.
//!
//! `root()` folds in the STATUS and RESULT, so a `Pending` saga and a `Done` saga
//! hash differently: the `Trigger` block moves the app-hash even before any result
//! exists, and the `OracleResult` block moves it again. that is what makes the
//! async boundary observable in the authenticated state.

use std::collections::BTreeMap;

use saga_interface::{
    decode_msg, decode_query, encode_reply, encode_worker_request, SagaMsg, SagaQuery, SagaReply,
    SagaStatus, SagaView, WorkerRequest,
};
use sdk::{Ctx, Effect, Error, Module, ModuleId, Msg, StateRoot};
use sha2::{Digest, Sha256};

/// one tracked saga. the id is the map key, so it isn't repeated here.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Saga {
    /// where in the promise-tree we are. bumped on each advance.
    step: u32,
    status: SagaStatus,
    /// the oracle's agreed output, once `Done`.
    result: Option<Vec<u8>>,
}

pub struct SagaModule {
    id: ModuleId,
    /// committed state — what `root()` and the app-hash commit to.
    sagas: BTreeMap<String, Saga>,
    /// sagas created/advanced this block: read ahead of `sagas` (read-your-writes)
    /// but merged in — and reflected in `root()` — only at `commit_block`.
    pending: BTreeMap<String, Saga>,
}

impl SagaModule {
    pub fn new(id: impl Into<ModuleId>) -> Self {
        Self { id: id.into(), sagas: BTreeMap::new(), pending: BTreeMap::new() }
    }

    /// read a saga: a STAGED (this-block) write shadows committed state.
    fn get(&self, saga_id: &str) -> Option<&Saga> {
        self.pending.get(saga_id).or_else(|| self.sagas.get(saga_id))
    }

    /// stage a whole saga for this block without committing.
    fn stage(&mut self, saga_id: String, saga: Saga) {
        self.pending.insert(saga_id, saga);
    }

    /// project a saga to its wire view.
    fn view(saga: &Saga) -> SagaView {
        SagaView { step: saga.step, status: saga.status, result: saga.result.clone() }
    }
}

#[async_trait::async_trait(?Send)]
impl Module for SagaModule {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// state-based commitment: a length-prefixed sha256 over the sorted sagas,
    /// folding in (id, step, status-discriminant, result). order-independent and
    /// idempotent — and, crucially, status-sensitive, so `Pending` and `Done`
    /// yield distinct roots.
    fn root(&self) -> StateRoot {
        let mut h = Sha256::new();
        h.update((self.sagas.len() as u64).to_le_bytes());
        for (id, s) in &self.sagas {
            h.update((id.len() as u64).to_le_bytes());
            h.update(id.as_bytes());
            h.update(s.step.to_le_bytes());
            let status: u8 = match s.status {
                SagaStatus::Pending => 0,
                SagaStatus::Done => 1,
            };
            h.update([status]);
            match &s.result {
                None => h.update([0u8]),
                Some(r) => {
                    h.update([1u8]);
                    h.update((r.len() as u64).to_le_bytes());
                    h.update(r);
                }
            }
        }
        StateRoot(h.finalize().into())
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            SagaMsg::Trigger { saga_id, spec } => {
                // record the saga as pending and ASK the host-owned worker to run
                // the async work. the worker will submit the matching OracleResult
                // op; we do not wait for or apply it here.
                self.stage(
                    saga_id.clone(),
                    Saga { step: 0, status: SagaStatus::Pending, result: None },
                );
                ctx.request_effect(Effect(encode_worker_request(&WorkerRequest {
                    saga_id,
                    spec,
                })));
            }
            SagaMsg::OracleResult { saga_id, result } => {
                match self.get(&saga_id) {
                    // idempotent: a duplicate result (e.g. two nodes both ran the
                    // worker) is a deterministic no-op — the first agreed one wins.
                    Some(s) if s.status == SagaStatus::Done => {}
                    Some(s) => {
                        let step = s.step + 1;
                        self.stage(
                            saga_id,
                            Saga { step, status: SagaStatus::Done, result: Some(result) },
                        );
                    }
                    // a result for an unknown saga is a no-op (never triggered, or
                    // already pruned) — deterministic on every node.
                    None => {}
                }
            }
        }
        Ok(())
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            SagaQuery::Get { saga_id } => {
                Ok(encode_reply(&SagaReply::Saga(self.get(&saga_id).map(Self::view))))
            }
        }
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        for (id, s) in std::mem::take(&mut self.pending) {
            self.sagas.insert(id, s);
        }
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;
    use saga_interface::{encode_msg, encode_query, decode_reply};
    use sdk::{Env, Event, Origin};

    /// a minimal `Ctx` that just captures emitted effects — enough to unit-test
    /// `execute` in isolation (the host provides the real one in integration).
    struct CaptureCtx {
        env: Env,
        effects: Vec<Effect>,
    }
    impl CaptureCtx {
        fn new() -> Self {
            Self {
                env: Env { height: 0, consensus_time: 0, origin: Origin::System, me: "saga".into() },
                effects: Vec::new(),
            }
        }
    }
    #[async_trait::async_trait(?Send)]
    impl Ctx for CaptureCtx {
        fn env(&self) -> &Env { &self.env }
        fn module_root(&self, _t: &str) -> Option<StateRoot> { None }
        async fn query(&self, _t: &str, _r: &[u8]) -> Result<Vec<u8>, Error> { Err(Error::QueryUnsupported) }
        fn emit_msg(&mut self, _m: Msg) {}
        fn emit_event(&mut self, _e: Event) {}
        fn request_effect(&mut self, eff: Effect) { self.effects.push(eff); }
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

    #[test]
    fn trigger_stages_pending_and_emits_one_worker_request() {
        let mut m = SagaModule::new("saga");
        let r0 = m.root();
        let mut ctx = CaptureCtx::new();
        block_on(m.execute(&mut ctx, &trigger("s1", b"hello"))).unwrap();

        // exactly one worker-request effect, carrying the spec.
        assert_eq!(ctx.effects.len(), 1, "trigger emits exactly one WorkerRequest effect");
        let wr = saga_interface::decode_worker_request(&ctx.effects[0].0).unwrap();
        assert_eq!(wr, WorkerRequest { saga_id: "s1".into(), spec: b"hello".to_vec() });

        // read-your-writes shows Pending before commit; root only moves on commit.
        assert_eq!(get(&m, "s1").unwrap().status, SagaStatus::Pending);
        assert_eq!(m.root(), r0, "staged-but-uncommitted work does not move root");
        block_on(m.commit_block()).unwrap();
        assert_ne!(m.root(), r0, "committing the pending saga moves the root");
    }

    #[test]
    fn oracle_result_advances_pending_to_done() {
        let mut m = SagaModule::new("saga");
        let mut ctx = CaptureCtx::new();
        block_on(m.execute(&mut ctx, &trigger("s1", b"hello"))).unwrap();
        block_on(m.commit_block()).unwrap();
        let pending_root = m.root();

        block_on(m.execute(&mut ctx, &oracle("s1", b"olleh"))).unwrap();
        block_on(m.commit_block()).unwrap();

        let v = get(&m, "s1").unwrap();
        assert_eq!(v.status, SagaStatus::Done);
        assert_eq!(v.step, 1, "an advance bumps the step");
        assert_eq!(v.result, Some(b"olleh".to_vec()));
        assert_ne!(m.root(), pending_root, "Pending -> Done moves the root");
    }

    #[test]
    fn oracle_result_is_idempotent() {
        let mut m = SagaModule::new("saga");
        let mut ctx = CaptureCtx::new();
        block_on(m.execute(&mut ctx, &trigger("s1", b"x"))).unwrap();
        block_on(m.commit_block()).unwrap();
        block_on(m.execute(&mut ctx, &oracle("s1", b"first"))).unwrap();
        block_on(m.commit_block()).unwrap();
        let done_root = m.root();

        // a second, different result must NOT overwrite the agreed one.
        block_on(m.execute(&mut ctx, &oracle("s1", b"second"))).unwrap();
        block_on(m.commit_block()).unwrap();
        assert_eq!(get(&m, "s1").unwrap().result, Some(b"first".to_vec()), "first agreed result wins");
        assert_eq!(m.root(), done_root, "a duplicate OracleResult is a no-op — root unchanged");
    }
}
