//! the reactor — the host-owned event loop that closes the async boundary.
//!
//! the host's within-block drain is the DETERMINISTIC re-entry lane: `emit_msg`
//! follow-ups re-dispatch inside ONE block, one app-hash. the reactor is the
//! NON-DETERMINISTIC lane: each block may emit [`Effect`]s that today no one runs,
//! and each effect that a [`Worker`] claims produces a follow-up op submitted as a
//! SEPARATE block — because on a real node it is a separate consensus transaction
//! (the oracle-as-op). that split — deterministic hops stay local and free, only
//! genuine external edges pay a round — is the whole design.
//!
//! this crate is domain-agnostic: it knows `Host`, `Effect`, and `Msg`, nothing
//! about sagas. a [`Worker`] try-decodes an effect it recognizes and, off to the
//! side (non-deterministically, in the real world), computes a result and returns
//! the op that carries it back. modules stay pure; only the worker is impure, and
//! the Worker seam lives HERE on the host side, never inside a module crate — so a
//! module never depends on the reactor.

use std::collections::VecDeque;

use host::Host;
use sdk::{Effect, Event, Msg, StateRoot};

/// outer-loop non-termination guard — the async sibling of the host's
/// `MAX_DISPATCHES`. bounds how many worker rounds one `submit_and_settle` may
/// drive before giving up, so a worker that keeps re-triggering itself can't spin
/// forever.
pub const MAX_WORKER_ROUNDS: u32 = 256;

/// errors from driving the reactor.
#[derive(Debug)]
pub enum Error {
    /// the host rejected a submitted op.
    Host(sdk::Error),
    /// a worker failed to produce its result.
    Worker(String),
    /// the outer worker loop exceeded [`MAX_WORKER_ROUNDS`].
    BudgetExceeded,
}

impl From<sdk::Error> for Error {
    fn from(e: sdk::Error) -> Self {
        Error::Host(e)
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::Host(e) => write!(f, "host error: {e}"),
            Error::Worker(m) => write!(f, "worker error: {m}"),
            Error::BudgetExceeded => write!(f, "worker-round budget exceeded"),
        }
    }
}

impl std::error::Error for Error {}

/// a host-owned, NON-DETERMINISTIC worker behind the effect seam. given an
/// [`Effect`] it recognizes, it does the off-consensus work (an LLM call, a fetch,
/// a commit) and returns the follow-up op that carries the result back through the
/// NORMAL submit path — the oracle-as-transaction. `Ok(None)` means "not my
/// effect": try-decode routing, so the reactor can offer each effect to every
/// worker until one claims it.
///
/// the worker never gets a handle to any module: it CANNOT mutate state directly.
/// its only channel back into the state machine is the `Msg` it returns, which the
/// reactor submits as an ordinary op. that is the oracle pattern enforced by type.
#[async_trait::async_trait(?Send)]
pub trait Worker {
    async fn run(&self, effect: &Effect) -> Result<Option<Msg>, Error>;
}

/// the settled outcome of driving a trigger op to a fixpoint: the final app-hash
/// and every event emitted along the way.
#[derive(Debug)]
pub struct Settled {
    pub app_hash: StateRoot,
    pub events: Vec<Event>,
}

/// the single-process reactor: a [`Host`] plus a set of [`Worker`]s. drives an op
/// to a fixpoint, running workers between blocks. this is the in-process stand-in
/// for the host-owned reactor a real node runs over its finalization stream.
pub struct Reactor {
    host: Host,
    workers: Vec<Box<dyn Worker>>,
}

impl Reactor {
    pub fn new(host: Host, workers: Vec<Box<dyn Worker>>) -> Self {
        Self { host, workers }
    }

    /// borrow the wrapped host (queries, module_root inspection).
    pub fn host(&self) -> &Host {
        &self.host
    }

    pub fn app_hash(&self) -> StateRoot {
        self.host.app_hash()
    }

    /// submit `msg` and run to a fixpoint: apply the block, drain its effects
    /// through the workers, submit each worker's follow-up op as its OWN block,
    /// and repeat until a block produces no more work. this IS the host's
    /// `drain-to-fixpoint`, with a worker step wedged between blocks — each worker
    /// result crossing a real block boundary, exactly as it would cross a
    /// consensus round on a real node.
    pub async fn submit_and_settle(&mut self, msg: Msg) -> Result<Settled, Error> {
        let mut queue: VecDeque<Msg> = VecDeque::from([msg]);
        let mut rounds: u32 = 0;
        let mut last = self.host.app_hash();
        let mut events: Vec<Event> = Vec::new();

        while let Some(op) = queue.pop_front() {
            rounds += 1;
            if rounds > MAX_WORKER_ROUNDS {
                return Err(Error::BudgetExceeded);
            }

            // ONE block: the op is applied and committed at the boundary.
            let outcome = self.host.submit(op).await?;
            last = outcome.app_hash;
            events.extend(outcome.events);

            // drain the effect sink the host itself ignores. try-decode routing:
            // the first worker that claims an effect wins.
            for eff in &outcome.effects {
                for w in &self.workers {
                    if let Some(follow) = w.run(eff).await? {
                        queue.push_back(follow);
                        break;
                    }
                }
            }
        }

        Ok(Settled { app_hash: last, events })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;
    use saga::SagaModule;
    use saga_interface::{
        decode_reply, decode_worker_request, encode_msg, encode_query, SagaMsg, SagaQuery,
        SagaReply, SagaStatus, SagaView,
    };

    /// a MOCK oracle standing in for the real (non-deterministic) worker. it
    /// try-decodes a `WorkerRequest`; on anything else it returns `Ok(None)` ("not
    /// my effect"). on a match it computes a stand-in result — reversing the spec
    /// bytes, a pure transform here but MODELING an opaque external computation —
    /// and returns the `OracleResult` op that carries it back through submit,
    /// echoing the request's `(saga_id, attempt)` idempotency key.
    struct MockOracle;

    #[async_trait::async_trait(?Send)]
    impl Worker for MockOracle {
        async fn run(&self, effect: &Effect) -> Result<Option<Msg>, Error> {
            let wr = match decode_worker_request(&effect.0) {
                Ok(wr) => wr,
                Err(_) => return Ok(None),
            };
            let result: Vec<u8> = wr.spec.iter().rev().copied().collect();
            Ok(Some(Msg {
                target: "saga".into(),
                payload: encode_msg(&SagaMsg::OracleResult {
                    saga_id: wr.saga_id,
                    attempt: wr.attempt,
                    outcome: Ok(result),
                }),
            }))
        }
    }

    /// a worker whose FIRST attempt fails: attempt 0 comes back `Err`, every
    /// later attempt succeeds — the retry-through-the-loop fixture.
    struct FlakyOracle;

    #[async_trait::async_trait(?Send)]
    impl Worker for FlakyOracle {
        async fn run(&self, effect: &Effect) -> Result<Option<Msg>, Error> {
            let wr = match decode_worker_request(&effect.0) {
                Ok(wr) => wr,
                Err(_) => return Ok(None),
            };
            let outcome = if wr.attempt == 0 {
                Err("first attempt always fails".to_string())
            } else {
                Ok(wr.spec.iter().rev().copied().collect())
            };
            Ok(Some(Msg {
                target: "saga".into(),
                payload: encode_msg(&SagaMsg::OracleResult {
                    saga_id: wr.saga_id,
                    attempt: wr.attempt,
                    outcome,
                }),
            }))
        }
    }

    fn trigger_with_attempts(id: &str, spec: &[u8], max_attempts: u32) -> Msg {
        Msg {
            target: "saga".into(),
            payload: encode_msg(&SagaMsg::Trigger {
                saga_id: id.into(),
                spec: spec.to_vec(),
                reply_to: None,
                reply_payload: Vec::new(),
                deadline: None,
                max_attempts,
                lease_views: None,
            }),
        }
    }

    fn trigger(id: &str, spec: &[u8]) -> Msg {
        trigger_with_attempts(id, spec, 1)
    }

    async fn get_saga(host: &Host, id: &str) -> Option<SagaView> {
        let reply = host.query("saga", &encode_query(&SagaQuery::Get { saga_id: id.into() })).await.unwrap();
        match decode_reply(&reply).unwrap() { SagaReply::Saga(v) => v }
    }

    #[test]
    fn a_trigger_drives_a_saga_to_done_via_the_mock_oracle() {
        block_on(async {
            let host = Host::genesis(vec![Box::new(SagaModule::new("saga"))]).expect("genesis");
            let mut reactor = Reactor::new(host, vec![Box::new(MockOracle)]);

            let settled = reactor.submit_and_settle(trigger("s1", b"hello")).await.expect("settle");

            // the saga reached Done carrying the AGREED (mock-oracle) result — and
            // the settled app-hash reflects that committed progress.
            let v = get_saga(reactor.host(), "s1").await.expect("saga exists");
            assert_eq!(v.status, SagaStatus::Done, "the saga settled at Done");
            assert_eq!(v.result, Some(b"olleh".to_vec()), "the oracle result is committed");
            assert_eq!(settled.app_hash, reactor.app_hash(), "settled hash == final host app-hash");
        });
    }

    /// the negative control: without the reactor the saga is STUCK at Pending —
    /// the block emits a WorkerRequest effect that nothing drains, and the app-hash
    /// is NOT the settled done-hash. running the reactor then advances it to Done.
    /// this proves the async boundary is real: the OracleResult re-enters as an OP
    /// (via `host.submit` in the loop), it is not applied by the worker directly.
    #[test]
    fn the_worker_is_load_bearing_oracle_re_enters_as_an_op() {
        block_on(async {
            // first, learn the settled done-hash via the full reactor.
            let done_hash = {
                let host = Host::genesis(vec![Box::new(SagaModule::new("saga"))]).expect("genesis");
                let mut reactor = Reactor::new(host, vec![Box::new(MockOracle)]);
                reactor.submit_and_settle(trigger("s1", b"hello")).await.expect("settle").app_hash
            };

            // now a bare host, no reactor: submit the Trigger directly.
            let mut host = Host::genesis(vec![Box::new(SagaModule::new("saga"))]).expect("genesis");
            let genesis = host.app_hash();
            let out = host.submit(trigger("s1", b"hello")).await.expect("submit trigger");

            // the saga is Pending, exactly one WorkerRequest effect went unhandled,
            // and the app-hash is neither genesis nor the settled done-hash.
            assert_eq!(get_saga(&host, "s1").await.unwrap().status, SagaStatus::Pending, "no worker -> stuck at Pending");
            assert_eq!(out.effects.len(), 1, "the block emitted exactly one WorkerRequest effect");
            assert_ne!(out.app_hash, genesis, "creating the pending saga moved the root off genesis");
            assert_ne!(out.app_hash, done_hash, "without the oracle op the state is NOT the done-state");

            // wrap the same host in a reactor and settle from where it is: an empty
            // op is not available, so re-run the effect through the worker manually
            // to prove the ONLY thing missing was the oracle op.
            let mut reactor = Reactor::new(host, vec![Box::new(MockOracle)]);
            let follow = MockOracle.run(&out.effects[0]).await.unwrap().expect("worker claims the effect");
            let settled = reactor.submit_and_settle(follow).await.expect("settle");
            assert_eq!(get_saga(reactor.host(), "s1").await.unwrap().status, SagaStatus::Done, "the oracle op advanced it to Done");
            assert_eq!(settled.app_hash, done_hash, "and it converges on the same settled done-hash");
        });
    }

    /// a failed attempt re-enters the loop as a retry: the `Err` outcome op
    /// makes the saga re-emit a `WorkerRequest` for attempt 1 (a fresh
    /// idempotency key), the worker answers THAT, and the saga settles at
    /// Done on the second attempt — all through ordinary submitted ops.
    #[test]
    fn a_flaky_worker_retries_through_the_loop_and_settles_done() {
        block_on(async {
            let host = Host::genesis(vec![Box::new(SagaModule::new("saga"))]).expect("genesis");
            let mut reactor = Reactor::new(host, vec![Box::new(FlakyOracle)]);

            let settled = reactor
                .submit_and_settle(trigger_with_attempts("s1", b"hello", 2))
                .await
                .expect("settle");

            let v = get_saga(reactor.host(), "s1").await.expect("saga exists");
            assert_eq!(v.status, SagaStatus::Done, "the retry landed");
            assert_eq!(v.attempt, 1, "attempt 0 was consumed by the Err outcome");
            assert_eq!(
                v.result,
                Some(b"olleh".to_vec()),
                "the retried result is committed"
            );
            assert_eq!(settled.app_hash, reactor.app_hash());
        });
    }
}
