//! the saga ledger under a real host: the P6 callback-adjacency promise, the
//! two callback-poison pins from design §4, and the strict lease gate driven
//! with explicit block contexts.
//!
//! - **adjacency**: the requester receives its `SagaCallback` as a follow-up
//!   dispatch in the SAME `submit_at` block as the `OracleResult` op — the
//!   terminal transition and the callback commit atomically, and the
//!   requester's staged write is published at the same boundary.
//! - **poison (a)**: a trigger naming an unknown `reply_to` is rejected at
//!   trigger time, before a saga exists that could never terminate cleanly.
//! - **poison (b)**: a requester whose callback arm ERRORS aborts the whole
//!   block — saga included — leaving the saga Pending and every root
//!   byte-identical. this is why requester callback arms must be no-fail by
//!   construction (decode failure = staged no-op + event, never an `Err`).
//! - **strict lease**: with a valset-backed assignment and
//!   `LeasePolicy::Strict`, a finalized result from a non-assignee origin is
//!   a deterministic no-op and the assignee's result lands.

use futures::executor::block_on;
use host::{BlockContext, Host};
use saga::{LeasePolicy, SagaModule};
use saga_interface::{
    SagaCallback, SagaMsg, SagaOutcome, SagaQuery, SagaReply, SagaStatus, SagaView,
    decode_callback, decode_reply, decode_worker_request, encode_callback, encode_msg,
    encode_query,
};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot};
use sha2::{Digest, Sha256};
use valset::Valset;

/// a minimal REQUESTER module: it records every `SagaCallback` it is
/// dispatched, with the same staging discipline as any other module (staged
/// during the block, committed at the boundary, dropped on abort) and a
/// state-based root over the committed callbacks. `poisoned` models the
/// forbidden requester — a callback arm that errors.
struct Recorder {
    id: ModuleId,
    poisoned: bool,
    committed: Vec<SagaCallback>,
    staged: Vec<SagaCallback>,
}
impl Recorder {
    fn new(id: &str, poisoned: bool) -> Self {
        Self {
            id: id.into(),
            poisoned,
            committed: Vec::new(),
            staged: Vec::new(),
        }
    }
}
#[async_trait::async_trait(?Send)]
impl Module for Recorder {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }
    fn root(&self) -> StateRoot {
        let mut h = Sha256::new();
        h.update((self.committed.len() as u64).to_le_bytes());
        for cb in &self.committed {
            let bytes = encode_callback(cb);
            h.update((bytes.len() as u64).to_le_bytes());
            h.update(&bytes);
        }
        StateRoot(h.finalize().into())
    }
    async fn execute(&mut self, _ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        if self.poisoned {
            return Err(Error::Module("recorder callback arm failed".into()));
        }
        let cb = decode_callback(&msg.payload).map_err(Error::Module)?;
        self.staged.push(cb);
        Ok(())
    }
    /// read projection: u64-le committed count, then the LAST committed
    /// callback's canonical bytes (empty when none).
    async fn query(&self, _req: &[u8]) -> Result<Vec<u8>, Error> {
        let mut out = (self.committed.len() as u64).to_le_bytes().to_vec();
        if let Some(last) = self.committed.last() {
            out.extend_from_slice(&encode_callback(last));
        }
        Ok(out)
    }
    async fn commit_block(&mut self) -> Result<(), Error> {
        self.committed.append(&mut self.staged);
        Ok(())
    }
    async fn abort_block(&mut self) -> Result<(), Error> {
        self.staged.clear();
        Ok(())
    }
}

fn at(height: u64, origin: Origin) -> BlockContext {
    BlockContext {
        height,
        consensus_time: height,
        origin,
    }
}

fn trigger(id: &str, reply_to: Option<&str>) -> Msg {
    Msg {
        target: "saga".into(),
        payload: encode_msg(&SagaMsg::Trigger {
            saga_id: id.into(),
            spec: b"work".to_vec(),
            reply_to: reply_to.map(String::from),
            reply_payload: b"corr-1".to_vec(),
            deadline: None,
            max_attempts: 1,
            lease_views: None,
        }),
    }
}

fn oracle(id: &str, attempt: u32, outcome: Result<Vec<u8>, String>) -> Msg {
    Msg {
        target: "saga".into(),
        payload: encode_msg(&SagaMsg::OracleResult {
            saga_id: id.into(),
            attempt,
            outcome,
        }),
    }
}

async fn saga_view(host: &Host, id: &str) -> Option<SagaView> {
    let reply = host
        .query(
            "saga",
            &encode_query(&SagaQuery::Get { saga_id: id.into() }),
        )
        .await
        .unwrap();
    match decode_reply(&reply).unwrap() {
        SagaReply::Saga(v) => v,
    }
}

async fn recorded(host: &Host) -> (u64, Option<SagaCallback>) {
    let bytes = host.query("agent", &[]).await.unwrap();
    let count = u64::from_le_bytes(bytes[..8].try_into().unwrap());
    let last = (bytes.len() > 8).then(|| decode_callback(&bytes[8..]).unwrap());
    (count, last)
}

#[test]
fn the_callback_lands_in_the_same_block_as_the_oracle_result() {
    block_on(async {
        let mut host = Host::genesis(vec![
            Box::new(SagaModule::new("saga")) as Box<dyn Module>,
            Box::new(Recorder::new("agent", false)),
        ])
        .expect("genesis");

        host.submit_at(
            at(1, Origin::External(b"alice".to_vec())),
            trigger("s1", Some("agent")),
        )
        .await
        .expect("trigger");
        assert_eq!(
            saga_view(&host, "s1").await.unwrap().status,
            SagaStatus::Pending
        );
        assert_eq!(recorded(&host).await.0, 0, "no callback before the result");
        let saga_pending = host.module_root("saga").unwrap();
        let agent_before = host.module_root("agent").unwrap();

        // ONE block: the OracleResult op. the saga terminal transition AND the
        // requester's callback commit at this single boundary (P6).
        host.submit_at(
            at(2, Origin::External(b"oracle".to_vec())),
            oracle("s1", 0, Ok(b"answer".to_vec())),
        )
        .await
        .expect("result block");

        let v = saga_view(&host, "s1").await.unwrap();
        assert_eq!(v.status, SagaStatus::Done);
        assert_eq!(v.result, Some(b"answer".to_vec()));
        let (count, last) = recorded(&host).await;
        assert_eq!(
            count, 1,
            "the requester committed its callback in the SAME block"
        );
        assert_eq!(
            last.unwrap(),
            SagaCallback {
                saga_id: "s1".into(),
                payload: b"corr-1".to_vec(),
                outcome: SagaOutcome::Done(b"answer".to_vec()),
            },
            "correlation payload echoed, outcome carried"
        );
        assert_ne!(
            host.module_root("saga").unwrap(),
            saga_pending,
            "the saga root moved"
        );
        assert_ne!(
            host.module_root("agent").unwrap(),
            agent_before,
            "the requester root moved atomically with it"
        );
    });
}

#[test]
fn a_trigger_with_an_unknown_reply_to_is_rejected_up_front() {
    block_on(async {
        // the callback-poison pin, half (a): the callback target is validated
        // against ctx.module_root AT TRIGGER TIME, so a saga that could never
        // terminate cleanly is never created.
        let mut host = Host::genesis(vec![
            Box::new(SagaModule::new("saga")) as Box<dyn Module>,
            Box::new(Recorder::new("agent", false)),
        ])
        .expect("genesis");
        let genesis = host.app_hash();

        let err = host
            .submit_at(
                at(1, Origin::External(b"alice".to_vec())),
                trigger("s1", Some("nope")),
            )
            .await
            .expect_err("unknown reply_to must reject");
        assert!(matches!(err, host::SubmitError::Rejected(Error::Module(_))));
        assert_eq!(saga_view(&host, "s1").await, None, "no saga was created");
        assert_eq!(host.app_hash(), genesis, "the rejected block left no trace");
    });
}

#[test]
fn a_failing_callback_arm_aborts_the_whole_block_and_the_saga_stays_pending() {
    block_on(async {
        // the callback-poison pin, half (b): the terminal transition and the
        // callback are ONE atomic block. a requester that errors on its
        // callback aborts them both — the saga stays Pending with every root
        // byte-identical, and would stay wedged forever if the requester kept
        // failing. hence the no-fail-callback rule (design §4): requester
        // callback arms must treat bad input as a staged no-op, never an Err.
        let mut host = Host::genesis(vec![
            Box::new(SagaModule::new("saga")) as Box<dyn Module>,
            Box::new(Recorder::new("agent", true)),
        ])
        .expect("genesis");

        host.submit_at(
            at(1, Origin::External(b"alice".to_vec())),
            trigger("s1", Some("agent")),
        )
        .await
        .expect("the trigger itself is fine — agent is registered");
        let pending_hash = host.app_hash();
        let saga_pending = host.module_root("saga").unwrap();

        let err = host
            .submit_at(
                at(2, Origin::External(b"oracle".to_vec())),
                oracle("s1", 0, Ok(b"answer".to_vec())),
            )
            .await
            .expect_err("the poisoned callback aborts the block");
        assert!(matches!(err, host::SubmitError::Rejected(Error::Module(_))));

        // no trace: the saga did NOT advance and no root moved.
        assert_eq!(
            saga_view(&host, "s1").await.unwrap().status,
            SagaStatus::Pending
        );
        assert_eq!(host.module_root("saga").unwrap(), saga_pending);
        assert_eq!(
            host.app_hash(),
            pending_hash,
            "the aborted block left every root untouched"
        );
        assert_eq!(recorded(&host).await.0, 0, "nothing was recorded");
    });
}

#[test]
fn strict_lease_rejects_a_non_assignee_and_accepts_the_assignee() {
    block_on(async {
        // three (genesis-seeded) validators; the saga module assigns each
        // attempt over the valset and enforces the lease strictly.
        let keys = vec![vec![1u8; 32], vec![2u8; 32], vec![3u8; 32]];
        let mut valset = Valset::new("valset");
        for key in &keys {
            valset.insert(key.clone());
        }
        let mut host = Host::genesis(vec![
            Box::new(SagaModule::with_valset(
                "saga",
                "valset",
                LeasePolicy::Strict,
            )) as Box<dyn Module>,
            Box::new(valset),
        ])
        .expect("genesis");

        let outcome = host
            .submit_at(
                at(5, Origin::External(b"requester".to_vec())),
                trigger("s1", None),
            )
            .await
            .expect("trigger");
        assert_eq!(outcome.effects.len(), 1, "one WorkerRequest effect");
        let request = decode_worker_request(&outcome.effects[0].0).unwrap();
        let assignee = request
            .assignee
            .expect("the valset assigned a lease holder");
        assert!(keys.contains(&assignee), "the assignee is a validator");
        assert_eq!(
            saga_view(&host, "s1").await.unwrap().assignee,
            Some(assignee.clone()),
            "the recorded lease matches the advertised one"
        );
        let non_assignee = keys.iter().find(|k| **k != assignee).unwrap().clone();
        let pending_hash = host.app_hash();

        // a finalized result from a NON-assignee is a deterministic no-op —
        // never an error, and no root moves.
        host.submit_at(
            at(6, Origin::External(non_assignee)),
            oracle("s1", 0, Ok(b"intruder".to_vec())),
        )
        .await
        .expect("a foreign result must not abort the block");
        assert_eq!(
            saga_view(&host, "s1").await.unwrap().status,
            SagaStatus::Pending
        );
        assert_eq!(
            host.app_hash(),
            pending_hash,
            "the no-op left the app-hash unchanged"
        );

        // the assignee's result lands.
        host.submit_at(
            at(7, Origin::External(assignee)),
            oracle("s1", 0, Ok(b"legit".to_vec())),
        )
        .await
        .expect("the assignee's result");
        let v = saga_view(&host, "s1").await.unwrap();
        assert_eq!(v.status, SagaStatus::Done);
        assert_eq!(v.result, Some(b"legit".to_vec()));
    });
}
