//! the RESIDENT-tier dispatch worker pump.
//!
//! a validator's [`DispatchWorker`] is fed by the reactor seam: effects of
//! finalized blocks it EXECUTED. a synced resident never executes blocks — it
//! installs boundary snapshots — so no effect ever reaches it, yet the saga
//! module (now assigning over validators ∪ residents' announced providers)
//! can lease work to its key. left unserved, that lease would stall the
//! attempt until expiry: the dispatch regression the resident-announce design
//! forbids.
//!
//! this pump closes the loop STATE-DRIVEN: each serve-window tick it asks the
//! served boundary's saga module for the pending attempts leased to this node
//! (`SagaQuery::AssignedPending` — the exact [`WorkerRequest`]s the effect
//! lane would have carried), runs the same [`DispatchWorker`] a validator
//! runs, and hands the resulting `OracleResult` op back to the caller for the
//! submit-relay lane. one execution per `(saga_id, attempt)` (an in-memory
//! latch; the saga module's P5 result-singularity makes a post-restart re-run
//! a deterministic no-op), deadline-based re-SEND (never re-run) while the
//! attempt stays pending, and entries retire the moment committed state stops
//! naming them — settled, retried elsewhere, or expired.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use dispatch_oracle::DispatchWorker;
use host::Host;
use reactor::{WorkOutcome, Worker as _};
use saga::{SagaQuery, SagaReply, WorkerRequest};
use sdk::{Effect, Msg};

/// how long a relayed result may await its consensus fate before it is
/// re-sent (comfortably above the relay's 10s SUBMIT_HOLD). re-sends are
/// duplicate ops at worst — the saga module's result singularity collapses
/// them deterministically.
const RESULT_RETRY: Duration = Duration::from_secs(15);

/// one attempt's idempotency key — what the worker echoes into its result.
type AttemptKey = (String, u32);

/// where one executed attempt's follow-up op sits in the relay lifecycle.
enum Stage {
    /// computed, needs a (re)send on the relay lane.
    Due(Msg),
    /// relayed, awaiting the validator's Reply (or the re-send deadline).
    InFlight {
        frame: node::FrameId,
        msg: Msg,
        deadline: Instant,
    },
    /// nothing further to send: the fate came back Applied, or the request
    /// was a deliberate skip (foreign spec shape / no follow-up). held until
    /// committed state retires the attempt, so the worker never re-runs it.
    Settled,
}

pub(crate) struct ResidentDispatch {
    worker: DispatchWorker,
    /// this node's external submit key — the lease identity queried for.
    me: Vec<u8>,
    /// every attempt this pump has acted on, keyed by `(saga_id, attempt)`;
    /// pruned against committed state each tick.
    work: HashMap<AttemptKey, Stage>,
}

impl ResidentDispatch {
    pub(crate) fn new(worker: DispatchWorker, me: Vec<u8>) -> Self {
        Self {
            worker,
            me,
            work: HashMap::new(),
        }
    }

    /// one serve-window tick: read this node's assigned pending attempts from
    /// the served boundary and return the ops due on the relay lane. the
    /// caller relays each and reports `sent` / leaves it due on failure.
    pub(crate) async fn tick(&mut self, host: &Host, now: Instant) -> Vec<(AttemptKey, Msg)> {
        let assigned = assigned_pending(host, &self.me).await;
        self.plan(assigned, now).await
    }

    /// the pump core, separated from the host read so it is unit-testable:
    /// retire entries committed state no longer names, execute newly assigned
    /// attempts ONCE, and surface every due (or re-send-due) op.
    async fn plan(&mut self, assigned: Vec<WorkerRequest>, now: Instant) -> Vec<(AttemptKey, Msg)> {
        // retire: an attempt absent from the committed projection settled,
        // moved on (retry under a new attempt), or expired — its entry (and
        // any un-sent result, now a guaranteed no-op) has nothing left to do.
        let live: std::collections::HashSet<AttemptKey> = assigned
            .iter()
            .map(|r| (r.saga_id.clone(), r.attempt))
            .collect();
        self.work.retain(|key, _| live.contains(key));

        let mut due = Vec::new();
        for request in assigned {
            let key = (request.saga_id.clone(), request.attempt);
            match self.work.get_mut(&key) {
                None => {
                    // first sighting: run the worker exactly once. the worker
                    // itself maps provider failures into an Err OracleResult,
                    // so anything but an op is a deliberate skip.
                    let stage = match self.worker.run(&Effect(saga::encode_worker_request(
                        &request,
                    ))).await {
                        Ok(WorkOutcome::Handled(Some(msg))) => {
                            due.push((key.clone(), msg.clone()));
                            Stage::Due(msg)
                        }
                        Ok(WorkOutcome::Handled(None)) | Ok(WorkOutcome::NotMine) => Stage::Settled,
                        Err(e) => {
                            // unreachable for DispatchWorker (it never errors)
                            // — settle rather than spin a broken worker.
                            eprintln!("[resident dispatch] worker error on {key:?}: {e}");
                            Stage::Settled
                        }
                    };
                    self.work.insert(key, stage);
                }
                Some(Stage::Due(msg)) => {
                    // computed but not successfully relayed yet: offer again.
                    due.push((key.clone(), msg.clone()));
                }
                Some(stage @ Stage::InFlight { .. }) => {
                    // a fate that never arrived stops blocking at the
                    // deadline: demote to due and re-send (never re-run).
                    if let Stage::InFlight { msg, deadline, .. } = stage
                        && now >= *deadline
                    {
                        let msg = msg.clone();
                        due.push((key.clone(), msg.clone()));
                        *stage = Stage::Due(msg);
                    }
                }
                Some(Stage::Settled) => {}
            }
        }
        due
    }

    /// a due op left on the relay lane: latch its frame id so the entry waits
    /// for the Reply instead of re-sending every tick.
    pub(crate) fn sent(&mut self, key: &AttemptKey, frame: node::FrameId, now: Instant) {
        if let Some(stage) = self.work.get_mut(key)
            && let Stage::Due(msg) = stage
        {
            *stage = Stage::InFlight {
                frame,
                msg: msg.clone(),
                deadline: now + RESULT_RETRY,
            };
        }
    }

    /// a validator's relay Reply. `Some(key)` when the frame was this pump's
    /// (the caller logs it), `None` otherwise. applied settles the entry;
    /// rejected / refused re-queues the op for the next tick while committed
    /// state still names the attempt.
    pub(crate) fn on_reply(&mut self, frame: &node::FrameId, applied: bool) -> Option<AttemptKey> {
        let key = self.work.iter().find_map(|(key, stage)| match stage {
            Stage::InFlight { frame: f, .. } if f == frame => Some(key.clone()),
            _ => None,
        })?;
        let stage = self.work.get_mut(&key).expect("found above");
        if applied {
            *stage = Stage::Settled;
        } else if let Stage::InFlight { msg, .. } = stage {
            *stage = Stage::Due(msg.clone());
        }
        Some(key)
    }
}

/// the committed saga projection of this node's leased pending attempts —
/// gracefully empty when the module is absent or the reply unreadable.
async fn assigned_pending(host: &Host, me: &[u8]) -> Vec<WorkerRequest> {
    let Ok(reply) = host
        .query(
            "saga",
            &saga::encode_query(&SagaQuery::AssignedPending {
                assignee: me.to_vec(),
            }),
        )
        .await
    else {
        return Vec::new();
    };
    match saga::decode_reply(&reply) {
        Ok(SagaReply::AssignedPending(requests)) => requests,
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dispatch::{WORK_SPEC_KIND, WorkSpec, encode_work_spec};
    use futures::executor::block_on;

    const ME: &[u8] = b"resident-key";

    /// a provider surface with one loaded mock spec and NO installed binary —
    /// the worker EXECUTES (provider resolve fails, so the result is an Err
    /// OracleResult op) without spawning anything. mirrors dispatch-oracle's
    /// own test fixture.
    fn worker() -> DispatchWorker {
        let spec = capability_host::CapabilitySpec::parse(
            r#"
spec = 1
[capability]
tag = "alpha"
[detect]
bin = "alpha-cli"
[invoke]
args = []
prompt = "stdin"
[output]
format = "text"
"#,
            "test",
        )
        .expect("mock spec parses");
        DispatchWorker::new(
            capability_host::ProviderSet::assemble(
                capability_host::SpecSet::from_specs(vec![spec]),
                Vec::new(),
            ),
            ME.to_vec(),
        )
    }

    fn request(saga_id: &str, attempt: u32) -> WorkerRequest {
        WorkerRequest {
            saga_id: saga_id.into(),
            attempt,
            spec: encode_work_spec(&WorkSpec {
                kind: WORK_SPEC_KIND.into(),
                dispatch_id: "d1".into(),
                capability: "alpha".into(),
                payload: b"the whole input".to_vec(),
            }),
            deadline: None,
            assignee: Some(ME.to_vec()),
        }
    }

    fn frame(byte: u8) -> node::FrameId {
        node::frame_id(&[byte])
    }

    /// `sdk::Msg` derives no PartialEq — project actions to comparable form.
    fn flat(v: &[(AttemptKey, Msg)]) -> Vec<(AttemptKey, String, Vec<u8>)> {
        v.iter()
            .map(|(k, m)| (k.clone(), m.target.clone(), m.payload.clone()))
            .collect()
    }

    #[test]
    fn an_assigned_attempt_executes_once_and_resends_only_after_the_deadline() {
        let mut pump = ResidentDispatch::new(worker(), ME.to_vec());
        let now = Instant::now();
        let key = ("job".to_string(), 0u32);

        // first sighting: exactly one due op, an OracleResult for our key.
        let due = block_on(pump.plan(vec![request("job", 0)], now));
        assert_eq!(due.len(), 1, "one execution, one op");
        assert_eq!(due[0].0, key);
        assert_eq!(due[0].1.target, "saga");
        match saga::decode_msg(&due[0].1.payload).expect("a saga op") {
            saga::SagaMsg::OracleResult {
                saga_id, attempt, ..
            } => {
                assert_eq!((saga_id, attempt), ("job".to_string(), 0));
            }
            other => panic!("expected an OracleResult, got {other:?}"),
        }

        // un-relayed (caller never called sent): offered again, not re-run —
        // byte-identical op.
        let again = block_on(pump.plan(vec![request("job", 0)], now));
        assert_eq!(flat(&again), flat(&due), "still due: the SAME computed op, no re-run");

        // relayed: quiet while the fate is pending...
        pump.sent(&key, frame(1), now);
        let quiet = block_on(pump.plan(vec![request("job", 0)], now));
        assert!(quiet.is_empty(), "in flight: nothing due");
        // ...until the deadline passes, then the SAME op re-sends.
        let resent = block_on(pump.plan(vec![request("job", 0)], now + RESULT_RETRY));
        assert_eq!(flat(&resent), flat(&due), "past the deadline: re-send, never re-run");
    }

    #[test]
    fn an_applied_reply_settles_until_state_retires_the_attempt() {
        let mut pump = ResidentDispatch::new(worker(), ME.to_vec());
        let now = Instant::now();
        let key = ("job".to_string(), 0u32);

        let due = block_on(pump.plan(vec![request("job", 0)], now));
        pump.sent(&key, frame(1), now);

        assert_eq!(pump.on_reply(&frame(9), true), None, "not our frame");
        assert_eq!(pump.on_reply(&frame(1), true), Some(key.clone()));
        let quiet = block_on(pump.plan(vec![request("job", 0)], now + RESULT_RETRY));
        assert!(quiet.is_empty(), "applied: settled, no re-send ever");

        // committed state stops naming the attempt -> the entry retires; a
        // LATER attempt of the same saga is fresh work and executes anew.
        let none = block_on(pump.plan(Vec::new(), now));
        assert!(none.is_empty());
        assert!(pump.work.is_empty(), "retired with committed state");
        let retry = block_on(pump.plan(vec![request("job", 1)], now));
        assert_eq!(retry.len(), 1, "a new attempt is new work");
        assert_eq!(retry[0].0, ("job".to_string(), 1));
        assert_ne!(retry[0].1.payload, due[0].1.payload, "the result echoes the new attempt");
    }

    #[test]
    fn a_rejected_reply_requeues_while_the_attempt_stays_pending() {
        let mut pump = ResidentDispatch::new(worker(), ME.to_vec());
        let now = Instant::now();
        let key = ("job".to_string(), 0u32);

        let due = block_on(pump.plan(vec![request("job", 0)], now));
        pump.sent(&key, frame(1), now);
        assert_eq!(pump.on_reply(&frame(1), false), Some(key));
        let requeued = block_on(pump.plan(vec![request("job", 0)], now));
        assert_eq!(flat(&requeued), flat(&due), "refused: due again immediately");
    }

    #[test]
    fn foreign_spec_shapes_are_skipped_quietly_and_never_rerun() {
        let mut pump = ResidentDispatch::new(worker(), ME.to_vec());
        let now = Instant::now();
        let foreign = WorkerRequest {
            spec: br#"{"run_id":"r","agent_id":"a"}"#.to_vec(),
            ..request("alien", 0)
        };
        assert!(
            block_on(pump.plan(vec![foreign.clone()], now)).is_empty(),
            "a foreign spec produces no op"
        );
        assert!(
            matches!(pump.work.get(&("alien".into(), 0)), Some(Stage::Settled)),
            "and is latched so it is not re-decoded every tick"
        );
        assert!(block_on(pump.plan(vec![foreign], now)).is_empty());
    }
}
