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
//!
//! a joiner rebuilds this module from a peer via [`SagaModule::snapshot`] /
//! [`SagaModule::install`]: the snapshot ships the committed map in the exact
//! canonical encoding `root()` hashes, and install re-derives the root from the
//! decoded temporaries before adopting them — the consensus-agreed root, not the
//! peer, is the trust anchor.

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

/// canonical byte encoding of a committed saga map: u64-le count, then per saga
/// in sorted-id order — u64-le id length + id bytes, u32-le step, one status
/// discriminant byte, one result tag byte (0 absent / 1 present) with a u64-le
/// length prefix when present. this is the exact preimage [`Module::root`]
/// hashes, so a snapshot and the root that must authenticate it cannot drift.
fn encode_committed(sagas: &BTreeMap<String, Saga>) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(sagas.len() as u64).to_le_bytes());
    for (id, s) in sagas {
        out.extend_from_slice(&(id.len() as u64).to_le_bytes());
        out.extend_from_slice(id.as_bytes());
        out.extend_from_slice(&s.step.to_le_bytes());
        out.push(match s.status {
            SagaStatus::Pending => 0,
            SagaStatus::Done => 1,
        });
        match &s.result {
            None => out.push(0),
            Some(r) => {
                out.push(1);
                out.extend_from_slice(&(r.len() as u64).to_le_bytes());
                out.extend_from_slice(r);
            }
        }
    }
    out
}

/// the state-based commitment over a committed saga map — shared by `root()` and
/// `install()` so the verification a snapshot must pass is definitionally the
/// same algorithm the live module answers with.
fn committed_root(sagas: &BTreeMap<String, Saga>) -> StateRoot {
    StateRoot(Sha256::digest(encode_committed(sagas)).into())
}

/// pull `n` bytes off the front of `buf`, checked against the remaining input
/// BEFORE any slicing — a lying length cannot over-read or panic.
fn take<'a>(buf: &mut &'a [u8], n: usize) -> Result<&'a [u8], String> {
    if n > buf.len() {
        return Err("snapshot truncated".into());
    }
    let (head, tail) = buf.split_at(n);
    *buf = tail;
    Ok(head)
}

fn take_u64(buf: &mut &[u8]) -> Result<u64, String> {
    Ok(u64::from_le_bytes(take(buf, 8)?.try_into().expect("8 bytes")))
}

fn take_u32(buf: &mut &[u8]) -> Result<u32, String> {
    Ok(u32::from_le_bytes(take(buf, 4)?.try_into().expect("4 bytes")))
}

/// a length prefix, validated against the remaining input before the caller
/// allocates anything of that size.
fn take_len(buf: &mut &[u8]) -> Result<usize, String> {
    let n = take_u64(buf)?;
    if n > buf.len() as u64 {
        return Err("snapshot length prefix exceeds input".into());
    }
    Ok(n as usize)
}

/// strict decode of an [`encode_committed`] snapshot. the input is UNTRUSTED —
/// it arrives from an arbitrary peer — so every count and length is bounded by
/// the remaining input before allocation, ids must be strictly ascending (one
/// byte encoding per state, and uniqueness for free), unknown discriminants are
/// rejected, and trailing bytes are rejected. never panics on malformed input.
fn decode_committed(mut buf: &[u8]) -> Result<BTreeMap<String, Saga>, String> {
    let count = take_u64(&mut buf)?;
    // every saga costs at least its fixed-width fields, so a count the input
    // cannot possibly hold is rejected before the loop builds anything.
    const MIN_SAGA_BYTES: u64 = 8 + 4 + 1 + 1;
    if count
        .checked_mul(MIN_SAGA_BYTES)
        .map_or(true, |need| need > buf.len() as u64)
    {
        return Err("snapshot saga count exceeds input".into());
    }
    let mut sagas: BTreeMap<String, Saga> = BTreeMap::new();
    for _ in 0..count {
        let id_len = take_len(&mut buf)?;
        let id = std::str::from_utf8(take(&mut buf, id_len)?)
            .map_err(|_| "snapshot saga id is not utf-8".to_string())?
            .to_owned();
        if let Some((last, _)) = sagas.iter().next_back() {
            if last.as_str() >= id.as_str() {
                return Err("snapshot saga ids not strictly ascending".into());
            }
        }
        let step = take_u32(&mut buf)?;
        let status = match take(&mut buf, 1)?[0] {
            0 => SagaStatus::Pending,
            1 => SagaStatus::Done,
            d => return Err(format!("snapshot has unknown status discriminant {d}")),
        };
        let result = match take(&mut buf, 1)?[0] {
            0 => None,
            1 => {
                let len = take_len(&mut buf)?;
                Some(take(&mut buf, len)?.to_vec())
            }
            t => return Err(format!("snapshot has unknown result tag {t}")),
        };
        sagas.insert(id, Saga { step, status, result });
    }
    if !buf.is_empty() {
        return Err("snapshot has trailing bytes".into());
    }
    Ok(sagas)
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

    // ---- state-sync ---------------------------------------------------------
    // hand a joiner the committed continuation state as canonical bytes; the
    // consensus-agreed root — never the serving peer — decides whether they land.

    /// serialize the COMMITTED continuation state (never the staged overlay) into
    /// the canonical encoding `root()` commits to: sorted ids, fixed-width length
    /// prefixes, single-byte enum discriminants. deterministic across nodes.
    pub fn snapshot(&self) -> Vec<u8> {
        encode_committed(&self.sagas)
    }

    /// adopt a peer's snapshot as own committed state — but only after the
    /// decoded temporaries re-derive `expected` via the exact `root()` algorithm,
    /// so a byzantine snapshot cannot land under an agreed root it doesn't match.
    /// all-or-nothing: on any Err this module (and its root) is byte-identical to
    /// before the call. on success the staged overlay is dropped — a snapshot
    /// describes a block boundary, and nothing half-applied may shadow it.
    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        let sagas = decode_committed(bytes).map_err(Error::Module)?;
        if committed_root(&sagas) != expected {
            return Err(Error::Module("snapshot does not match expected root".into()));
        }
        self.sagas = sagas;
        self.pending.clear();
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl Module for SagaModule {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// state-based commitment: sha256 over the canonical committed encoding — a
    /// length-prefixed fold of (id, step, status-discriminant, result) in sorted
    /// order. insertion-order-independent and idempotent — and, crucially,
    /// status-sensitive, so `Pending` and `Done` yield distinct roots. the
    /// preimage IS the snapshot encoding (see [`SagaModule::snapshot`]).
    fn root(&self) -> StateRoot {
        committed_root(&self.sagas)
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
