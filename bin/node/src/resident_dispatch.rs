//! the RESIDENT-tier dispatch worker pump.
//!
//! a validator's dispatch worker is fed by the reactor seam: events of
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
//! lane would have carried) and offers each, once, to the SAME off-loop
//! [`dispatch_oracle::DispatchPool`] a validator runs (wired through
//! `oracle_pool::build`): the gate verdict is inline and immediate, the
//! provider CLI runs on a spawned background task, and a pump pass NEVER
//! awaits a provider — a minutes-long run cannot stall the park loop's serve
//! window, boundary follow, or promotion detection. completed results come
//! back over the pool's result lane, re-enter here via
//! [`ResidentDispatch::completed`], and ride the caller's submit-relay lane
//! home.
//!
//! one execution per `(saga_id, attempt)` — the pump's own `Executing` latch
//! keeps a still-pending attempt from being re-offered across ticks (the
//! pool's in-flight dedup prunes at delivery, so it alone cannot carry that
//! guarantee), and the pool's dedup backstops any same-tick redelivery.
//! results re-SEND (never re-run) on a deadline while the attempt stays
//! pending, and entries retire once committed state stops naming them —
//! settled, retried elsewhere, or expired. retirement requires CONFIRMED
//! absence (two consecutive reads): the latch is the exactly-once guarantee,
//! so a single read that momentarily fails to name a live attempt — whatever
//! its source — must not drop it and re-offer the SAME attempt as fresh work
//! (a second child, and a computed-but-unsent result silently lost). real
//! retirements proven by the saga's authoritative per-id view cancel on the
//! first missing subset read; only an unreadable or older view needs a second
//! read. retirement also cancels a non-settled host attempt before its latch
//! is removed; settled work needs no cancellation. a restart re-runs at
//! worst once; the saga module's P5
//! result-singularity collapses the duplicate.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use dispatch_oracle::AttemptControl;
use host::Host;
use host::worker::{WorkOutcome, Worker};
use saga::{SagaMsg, SagaQuery, SagaReply, WorkerRequest};
use sdk::{Event, Msg};

/// how long a relayed result may await its consensus fate before it is
/// re-sent (comfortably above the relay's 10s SUBMIT_HOLD). re-sends are
/// duplicate ops at worst — the saga module's result singularity collapses
/// them deterministically.
const RESULT_RETRY: Duration = Duration::from_secs(15);

/// one attempt's idempotency key — what the worker echoes into its result.
type AttemptKey = (String, u32);

enum AttemptProjection {
    Active,
    Retired,
}

/// where one assigned attempt sits in the execute-then-relay lifecycle.
enum Stage {
    /// offered to the pool; the provider runs (or queues) on a spawned
    /// background task. waiting for the result lane — nothing to send yet,
    /// and the attempt must NOT be offered again.
    Executing,
    /// the follow-up op is computed, needs a (re)send on the relay lane.
    Due(Msg),
    /// relayed, awaiting the validator's Reply (or the re-send deadline).
    InFlight {
        frame: node::FrameId,
        msg: Msg,
        deadline: Instant,
    },
    /// nothing further to send: the fate came back Applied, or the request
    /// was a deliberate skip (foreign spec shape / no follow-up). held until
    /// committed state retires the attempt, so it is never re-offered.
    Settled,
}

/// one tracked attempt: its lifecycle stage plus the retire confirmation —
/// `missed` marks a read that failed to name the attempt, and only a SECOND
/// consecutive miss retires the entry (see the module doc).
struct Entry {
    stage: Stage,
    missed: bool,
}

pub(crate) struct ResidentDispatch {
    /// the off-loop execution pool (`DispatchPool` behind the shared
    /// `Worker` seam): gate inline, spawn the provider, return immediately.
    pool: Box<dyn Worker>,
    /// cloneable control for attempts already handed to the off-loop pool.
    control: AttemptControl,
    /// this node's external submit key — the lease identity queried for.
    me: Vec<u8>,
    /// every attempt this pump has acted on, keyed by `(saga_id, attempt)`;
    /// pruned against committed state on confirmed absence.
    work: HashMap<AttemptKey, Entry>,
}

impl ResidentDispatch {
    pub(crate) fn new(pool: Box<dyn Worker>, control: AttemptControl, me: Vec<u8>) -> Self {
        Self {
            pool,
            control,
            me,
            work: HashMap::new(),
        }
    }

    /// one serve-window tick: read this node's assigned pending attempts from
    /// the served boundary and return the ops due on the relay lane. never
    /// awaits a provider — new assignments are handed to the off-loop pool.
    /// the caller relays each due op and reports `sent` / leaves it due on
    /// failure.
    pub(crate) async fn tick(&mut self, host: &Host, now: Instant) -> Vec<(AttemptKey, Msg)> {
        // an unreadable projection is NOT an empty one: a failed read says
        // nothing about committed state, so the pass does nothing rather
        // than count it toward retirement.
        let Some(assigned) = assigned_pending(host, &self.me).await else {
            return Vec::new();
        };
        let live: HashSet<AttemptKey> = assigned
            .iter()
            .map(|request| (request.saga_id.clone(), request.attempt))
            .collect();
        let missing: Vec<_> = self
            .work
            .keys()
            .filter(|key| !live.contains(*key))
            .cloned()
            .collect();
        let mut retired = HashSet::new();
        let mut active = HashSet::new();
        for key in missing {
            match attempt_projection(host, &self.me, &key).await {
                Some(AttemptProjection::Retired) => {
                    retired.insert(key);
                }
                Some(AttemptProjection::Active) => {
                    active.insert(key);
                }
                None => {}
            }
        }
        self.plan_with_projection(assigned, &retired, &active, now)
            .await
    }

    /// a completed off-loop run arrived on the result lane: queue its op for
    /// the relay. an attempt committed state no longer names (pruned while
    /// the provider ran — lease expired, saga settled elsewhere) is dropped;
    /// its op could only be a deterministic no-op anyway.
    pub(crate) fn completed(&mut self, msg: Msg) {
        let Ok(SagaMsg::OracleResult {
            saga_id, attempt, ..
        }) = saga::decode_msg(&msg.payload)
        else {
            return; // not an oracle result — the pool sends nothing else.
        };
        let key = (saga_id, attempt);
        match self.work.get_mut(&key) {
            Some(Entry {
                stage: stage @ Stage::Executing,
                ..
            }) => *stage = Stage::Due(msg),
            _ => eprintln!(
                "[resident dispatch] dropping a completed run for retired attempt {key:?}"
            ),
        }
    }

    /// the pump core, separated from the host read so it is unit-testable:
    /// retire entries committed state proves obsolete (or an inconclusive
    /// absence confirms over two reads), offer newly assigned attempts to the
    /// pool ONCE, and surface every due (or re-send-due) op.
    #[cfg(test)]
    async fn plan(&mut self, assigned: Vec<WorkerRequest>, now: Instant) -> Vec<(AttemptKey, Msg)> {
        self.plan_with_projection(assigned, &HashSet::new(), &HashSet::new(), now)
            .await
    }

    async fn plan_with_projection(
        &mut self,
        assigned: Vec<WorkerRequest>,
        retired: &HashSet<AttemptKey>,
        active: &HashSet<AttemptKey>,
        now: Instant,
    ) -> Vec<(AttemptKey, Msg)> {
        // retire: an attempt absent from the committed projection settled,
        // moved on (retry under a new attempt), or expired — its entry (and
        // any un-sent result, now a guaranteed no-op) has nothing left to do.
        // authoritative terminal/reassigned state retires immediately; an
        // inconclusive absence needs a confirming second read. dropping a
        // live attempt on one flapped read would re-offer the SAME attempt as
        // fresh work (the exactly-once break). retirement cancels a
        // non-settled host attempt before removing its latch; a still-running
        // child's late result is then dropped by `completed`.
        let live: HashSet<AttemptKey> = assigned
            .iter()
            .map(|r| (r.saga_id.clone(), r.attempt))
            .collect();
        let newest = assigned.iter().fold(
            std::collections::HashMap::<String, u32>::new(),
            |mut newest, request| {
                newest
                    .entry(request.saga_id.clone())
                    .and_modify(|attempt| *attempt = (*attempt).max(request.attempt))
                    .or_insert(request.attempt);
                newest
            },
        );
        let control = self.control.clone();
        let me = &self.me;
        self.work.retain(|key, entry| {
            if live.contains(key) || active.contains(key) {
                entry.missed = false;
                return true;
            }
            // A higher committed attempt of the same saga, including one
            // assigned to another node and confirmed by `SagaQuery::Get`, is
            // proof of supersession rather than a flapped projection. Cancel
            // it before any local replacement is offered below. Plain absence
            // still needs a confirming read so a transient flap cannot
            // duplicate a run.
            let superseded = retired.contains(key)
                || newest
                    .get(&key.0)
                    .is_some_and(|attempt| *attempt > key.1);
            if !superseded && !entry.missed {
                entry.missed = true;
                if !matches!(entry.stage, Stage::Settled) {
                    // mid-work absence is the flap signature — worth eyes if
                    // it recurs; a settled entry's absence is plain retirement.
                    eprintln!(
                        "[resident dispatch] attempt {key:?} left the committed \
                         projection mid-work — retiring on the next absent read"
                    );
                }
                return true;
            }
            if !matches!(entry.stage, Stage::Settled) {
                control.cancel(&key.0, key.1, me);
            }
            false
        });

        let mut due = Vec::new();
        for request in assigned {
            let key = (request.saga_id.clone(), request.attempt);
            if let Some(entry) = self.work.get_mut(&key) {
                // named again: whatever absence was seen did not persist.
                entry.missed = false;
            }
            match self.work.get_mut(&key).map(|e| &mut e.stage) {
                None => {
                    // first sighting: offer to the pool exactly once. the
                    // gate runs inline (cheap, deterministic); an executable
                    // lease SPAWNS and returns immediately — the result
                    // arrives later via `completed`.
                    let stage = match self
                        .pool
                        .run(&Event {
                            source: "saga".into(),
                            payload: saga::encode_worker_request(&request),
                        })
                        .await
                    {
                        // an inline verdict WITH an op (unresolvable
                        // capability, non-utf-8 payload): due now.
                        Ok(WorkOutcome::Handled(Some(msg))) => {
                            due.push((key.clone(), msg.clone()));
                            Stage::Due(msg)
                        }
                        // spawned (result later). a deliberate inline skip
                        // maps here too, but an own-lease request never
                        // skips; either way the result lane / retire sweep
                        // decides what happens next, and the attempt is
                        // latched against re-offer.
                        Ok(WorkOutcome::Handled(None)) => Stage::Executing,
                        // not a dispatch WorkSpec: never ours to run.
                        Ok(WorkOutcome::NotMine) => Stage::Settled,
                        Err(e) => {
                            // unreachable for DispatchPool (it never errors)
                            // — settle rather than spin a broken worker.
                            eprintln!("[resident dispatch] worker error on {key:?}: {e}");
                            Stage::Settled
                        }
                    };
                    self.work.insert(
                        key,
                        Entry {
                            stage,
                            missed: false,
                        },
                    );
                }
                Some(Stage::Executing) => {} // provider running off-loop.
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
        if let Some(entry) = self.work.get_mut(key)
            && let Stage::Due(msg) = &entry.stage
        {
            entry.stage = Stage::InFlight {
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
        let key = self.work.iter().find_map(|(key, entry)| match &entry.stage {
            Stage::InFlight { frame: f, .. } if f == frame => Some(key.clone()),
            _ => None,
        })?;
        let entry = self.work.get_mut(&key).expect("found above");
        if applied {
            entry.stage = Stage::Settled;
        } else if let Stage::InFlight { msg, .. } = &entry.stage {
            entry.stage = Stage::Due(msg.clone());
        }
        Some(key)
    }
}

/// the committed saga projection of this node's leased pending attempts.
/// `None` when the module is absent or the reply unreadable — a failed read
/// must never masquerade as an EMPTY projection: emptiness retires entries,
/// and retiring a live attempt re-runs it (see `plan`).
async fn assigned_pending(host: &Host, me: &[u8]) -> Option<Vec<WorkerRequest>> {
    let reply = host
        .query(
            "saga",
            &saga::encode_query(&SagaQuery::AssignedPending {
                assignee: me.to_vec(),
            }),
        )
        .await
        .ok()?;
    match saga::decode_reply(&reply) {
        Ok(SagaReply::AssignedPending(requests)) => Some(requests),
        _ => None,
    }
}

/// Confirm why an assigned attempt disappeared from this resident's subset.
/// `AssignedPending` cannot show a retry that moved to another node, while the
/// per-saga view can: terminal state, a higher attempt, or a changed assignee
/// all prove the old local process must stop on this first read. An unreadable
/// or older view is inconclusive and preserves the two-read flap tolerance.
async fn attempt_projection(
    host: &Host,
    me: &[u8],
    key: &AttemptKey,
) -> Option<AttemptProjection> {
    let reply = host
        .query(
            "saga",
            &saga::encode_query(&SagaQuery::Get {
                saga_id: key.0.clone(),
            }),
        )
        .await
        .ok()?;
    match saga::decode_reply(&reply).ok()? {
        SagaReply::Saga(None) => Some(AttemptProjection::Retired),
        SagaReply::Saga(Some(view))
            if view.status.is_terminal()
                || view.attempt > key.1
                || (view.attempt == key.1 && view.assignee.as_deref() != Some(me)) =>
        {
            Some(AttemptProjection::Retired)
        }
        SagaReply::Saga(Some(view))
            if view.attempt == key.1 && view.assignee.as_deref() == Some(me) =>
        {
            Some(AttemptProjection::Active)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use dispatch::{AdmissionPolicy, WORK_SPEC_KIND, WorkSpec, encode_work_spec};
    use dispatch_oracle::{
        DeliverFn, DispatchPool, ProvisionedWorkspace, SharedProvisioner, SpawnFn,
        WorkspaceProvisioner, WorkspaceReceipt, WorkspaceSpec,
    };
    use futures::StreamExt as _;

    const ME: &[u8] = b"resident-key";

    /// a provider that counts executions and sleeps — the observable
    /// stand-in for a minutes-long CLI (mirrors pool.rs's SlowProvider).
    struct SlowProvider {
        delay: Duration,
        executions: Arc<AtomicUsize>,
        cancellations: Arc<AtomicUsize>,
    }

    struct RunGuard {
        cancellations: Arc<AtomicUsize>,
        completed: bool,
    }

    impl Drop for RunGuard {
        fn drop(&mut self) {
            if !self.completed {
                self.cancellations.fetch_add(1, Ordering::SeqCst);
            }
        }
    }

    #[async_trait::async_trait]
    impl capability_host::Provider for SlowProvider {
        fn capability(&self) -> &str {
            "alpha"
        }
        async fn run(
            &self,
            prompt: &str,
            ctx: &capability_host::RunContext,
        ) -> Result<String, String> {
            self.executions.fetch_add(1, Ordering::SeqCst);
            let mut guard = RunGuard {
                cancellations: self.cancellations.clone(),
                completed: false,
            };
            if let Some(cancellation) = &ctx.cancellation {
                tokio::select! {
                    _ = tokio::time::sleep(self.delay) => {}
                    _ = cancellation.cancelled() => {
                        return Err("mock resident provider cancelled".into());
                    }
                }
            } else {
                tokio::time::sleep(self.delay).await;
            }
            guard.completed = true;
            Ok(format!("answer to: {prompt}"))
        }
    }

    fn spec_toml() -> capability_host::CapabilitySpec {
        capability_host::CapabilitySpec::parse(
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
        .expect("mock spec parses")
    }

    struct TestProvisioner;

    #[async_trait::async_trait]
    impl WorkspaceProvisioner for TestProvisioner {
        async fn provision(
            &self,
            spec: &WorkspaceSpec,
        ) -> Result<Box<dyn ProvisionedWorkspace>, String> {
            Ok(Box::new(TestWorkspace(spec.clone())))
        }
    }

    struct TestWorkspace(WorkspaceSpec);

    #[async_trait::async_trait]
    impl ProvisionedWorkspace for TestWorkspace {
        fn workdir(&self) -> std::path::PathBuf {
            std::env::temp_dir()
        }

        fn env(&self) -> std::collections::BTreeMap<String, String> {
            Default::default()
        }

        fn path_entries(&self) -> Vec<std::path::PathBuf> {
            Vec::new()
        }

        async fn commit(
            &self,
            _audit_message: &str,
            _proposal: Option<&str>,
        ) -> Result<WorkspaceReceipt, String> {
            Ok(WorkspaceReceipt::no_changes(&self.0))
        }

        async fn cleanup(&self) {}
    }

    fn test_provisioner() -> SharedProvisioner {
        Arc::new(TestProvisioner)
    }

    /// a pump wired the way main.rs wires it: a DispatchPool spawning on
    /// tokio with a result lane the test forwards into `completed`. returns
    /// the pump, the lane, and the provider's execution counter. `installed`
    /// false loads the spec with NO provider — the gate answers inline.
    fn pump_with(
        delay: Duration,
        installed: bool,
    ) -> (
        ResidentDispatch,
        futures::channel::mpsc::UnboundedReceiver<Msg>,
        Arc<AtomicUsize>,
    ) {
        pump_with_cancellations(delay, installed, Arc::new(AtomicUsize::new(0)))
    }

    fn pump_with_cancellations(
        delay: Duration,
        installed: bool,
        cancellations: Arc<AtomicUsize>,
    ) -> (
        ResidentDispatch,
        futures::channel::mpsc::UnboundedReceiver<Msg>,
        Arc<AtomicUsize>,
    ) {
        let executions = Arc::new(AtomicUsize::new(0));
        let providers: Vec<Box<dyn capability_host::Provider>> = if installed {
            vec![Box::new(SlowProvider {
                delay,
                executions: executions.clone(),
                cancellations: cancellations.clone(),
            })]
        } else {
            Vec::new()
        };
        let providers = Arc::new(capability_host::ProviderSet::assemble(
            capability_host::SpecSet::from_specs(vec![spec_toml()]),
            providers,
        ));
        let (tx, rx) = futures::channel::mpsc::unbounded::<Msg>();
        let spawn: SpawnFn = Arc::new(|_, fut| {
            tokio::spawn(fut);
        });
        let deliver: DeliverFn = Arc::new(move |msg| {
            let tx = tx.clone();
            Box::pin(async move {
                let _ = tx.unbounded_send(msg);
            })
        });
        let pool = DispatchPool::with_limit(
            providers,
            ME.to_vec(),
            spawn,
            deliver,
            4,
            Default::default(),
            test_provisioner(),
        );
        let control = pool.attempt_control();
        (
            ResidentDispatch::new(Box::new(pool), control, ME.to_vec()),
            rx,
            executions,
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
                // a minimal v3 run envelope: the oracle's prepare() rejects
                // marker-less flat payloads post-flag-day. the test
                // provisioner keeps the production-required workspace path.
                payload: serde_json::json!({
                    "ducktape_run": 3,
                    "agent_id": "bot",
                    "thread_key": null,
                    "instructions": "GENERIC",
                    "contract": "CONTRACT",
                    "conversation": "CONVERSATION",
                    "workspace": {
                        "kind": "duckfs",
                        "source_prefix": "/shared/agent-workspaces/bot",
                        "source_snapshot": null
                    },
                    "skills": [],
                    "result_contract": {"ducktape_runner_result": 1}
                })
                .to_string()
                .into_bytes(),
                demands: Default::default(),
                admission: AdmissionPolicy::Queue,
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

    async fn next_result(rx: &mut futures::channel::mpsc::UnboundedReceiver<Msg>) -> Msg {
        tokio::time::timeout(Duration::from_secs(10), rx.next())
            .await
            .expect("a result within budget")
            .expect("the lane stays open")
    }

    #[tokio::test]
    async fn a_slow_provider_never_blocks_the_pump_pass_and_runs_exactly_once() {
        // a run far longer than the pass budget: the headline property is
        // that plan() returns in gate time, not provider time.
        let (mut pump, mut rx, executions) = pump_with(Duration::from_millis(500), true);
        let now = Instant::now();
        let key = ("job".to_string(), 0u32);

        let offered = Instant::now();
        let due = pump.plan(vec![request("job", 0)], now).await;
        assert!(
            offered.elapsed() < Duration::from_millis(200),
            "the pump pass must not await the provider (took {:?})",
            offered.elapsed()
        );
        assert!(due.is_empty(), "nothing due yet: the run is off-loop");

        // ticks WHILE the provider runs: latched Executing, no re-offer, no
        // second child.
        for _ in 0..3 {
            assert!(
                pump.plan(vec![request("job", 0)], now).await.is_empty(),
                "an executing attempt is never re-offered"
            );
        }

        // the completed result re-enters and becomes the due relay op.
        pump.completed(next_result(&mut rx).await);
        assert_eq!(executions.load(Ordering::SeqCst), 1, "exactly one child");
        let due = pump.plan(vec![request("job", 0)], now).await;
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].0, key);
        assert_eq!(due[0].1.target, "saga");
        match saga::decode_msg(&due[0].1.payload).expect("a saga op") {
            SagaMsg::OracleResult {
                saga_id,
                attempt,
                outcome,
                ..
            } => {
                assert_eq!((saga_id, attempt), ("job".to_string(), 0));
                let result: serde_json::Value = serde_json::from_slice(&outcome.unwrap()).unwrap();
                assert_eq!(
                    result["response_text"],
                    "answer to: GENERIC\n\nCONTRACT\n\nCONVERSATION"
                );
            }
            other => panic!("expected an OracleResult, got {other:?}"),
        }

        // relayed: quiet while the fate is pending, the SAME op re-sends
        // only after the deadline — a re-send, never a re-run.
        pump.sent(&key, frame(1), now);
        assert!(
            pump.plan(vec![request("job", 0)], now).await.is_empty(),
            "in flight: nothing due"
        );
        let resent = pump.plan(vec![request("job", 0)], now + RESULT_RETRY).await;
        assert_eq!(flat(&resent), flat(&due), "past the deadline: re-send");
        assert_eq!(executions.load(Ordering::SeqCst), 1, "still one child");
    }

    #[tokio::test]
    async fn an_applied_reply_settles_until_state_retires_the_attempt() {
        let (mut pump, mut rx, executions) = pump_with(Duration::from_millis(10), true);
        let now = Instant::now();
        let key = ("job".to_string(), 0u32);

        assert!(pump.plan(vec![request("job", 0)], now).await.is_empty());
        pump.completed(next_result(&mut rx).await);
        let due = pump.plan(vec![request("job", 0)], now).await;
        assert_eq!(due.len(), 1);
        pump.sent(&key, frame(1), now);

        assert_eq!(pump.on_reply(&frame(9), true), None, "not our frame");
        assert_eq!(pump.on_reply(&frame(1), true), Some(key.clone()));
        assert!(
            pump.plan(vec![request("job", 0)], now + RESULT_RETRY)
                .await
                .is_empty(),
            "applied: settled, no re-send ever"
        );

        // committed state stops naming the attempt -> the entry retires on
        // the CONFIRMING (second consecutive) absent read; a LATER attempt
        // of the same saga is fresh work and executes anew.
        assert!(pump.plan(Vec::new(), now).await.is_empty());
        assert!(pump.plan(Vec::new(), now).await.is_empty());
        assert!(pump.work.is_empty(), "retired with committed state");
        assert!(pump.plan(vec![request("job", 1)], now).await.is_empty());
        pump.completed(next_result(&mut rx).await);
        let retry = pump.plan(vec![request("job", 1)], now).await;
        assert_eq!(retry.len(), 1, "a new attempt is new work");
        assert_eq!(retry[0].0, ("job".to_string(), 1));
        assert_eq!(executions.load(Ordering::SeqCst), 2, "one child per attempt");
    }

    #[tokio::test]
    async fn a_rejected_reply_requeues_while_the_attempt_stays_pending() {
        let (mut pump, mut rx, _executions) = pump_with(Duration::from_millis(10), true);
        let now = Instant::now();
        let key = ("job".to_string(), 0u32);

        assert!(pump.plan(vec![request("job", 0)], now).await.is_empty());
        pump.completed(next_result(&mut rx).await);
        let due = pump.plan(vec![request("job", 0)], now).await;
        pump.sent(&key, frame(1), now);
        assert_eq!(pump.on_reply(&frame(1), false), Some(key));
        let requeued = pump.plan(vec![request("job", 0)], now).await;
        assert_eq!(
            flat(&requeued),
            flat(&due),
            "refused: due again immediately"
        );
    }

    #[tokio::test]
    async fn a_late_result_for_a_retired_attempt_is_dropped() {
        // The result is already computed when the confirmed absence retires
        // its resident latch. Delivering that queued result afterwards must
        // not resurrect the entry.
        let (mut pump, mut rx, _executions) = pump_with(Duration::from_millis(10), true);
        let now = Instant::now();

        assert!(pump.plan(vec![request("job", 0)], now).await.is_empty());
        assert!(pump.plan(Vec::new(), now).await.is_empty(), "first miss");
        let result = next_result(&mut rx).await;
        assert!(pump.plan(Vec::new(), now).await.is_empty(), "retired");
        assert!(pump.work.is_empty());

        pump.completed(result);
        assert!(pump.work.is_empty(), "a late result does not resurrect");
        assert!(pump.plan(Vec::new(), now).await.is_empty());
    }

    #[tokio::test]
    async fn a_momentary_projection_flap_never_reruns_an_executing_attempt() {
        // one tick's read fails to name a still-pending attempt (a flap —
        // whatever its source), and the provider's result crosses the gap:
        // it is delivered while the entry is absent, so the pool's in-flight
        // dedup is already pruned. without a surviving latch the next tick
        // re-offers the SAME attempt and spawns a second child — the
        // exactly-once break the e2e observed (two runs, one relayed
        // result).
        let (mut pump, mut rx, executions) = pump_with(Duration::from_millis(10), true);
        let now = Instant::now();
        let key = ("job".to_string(), 0u32);

        assert!(pump.plan(vec![request("job", 0)], now).await.is_empty());
        assert!(pump.plan(Vec::new(), now).await.is_empty(), "flap tick");
        // the result lands during the gap (the park loop drains the lane
        // before each tick).
        pump.completed(next_result(&mut rx).await);
        let due = pump.plan(vec![request("job", 0)], now).await;
        assert_eq!(due.len(), 1, "the surviving result is due for relay");
        assert_eq!(due[0].0, key);
        assert_eq!(
            executions.load(Ordering::SeqCst),
            1,
            "a flap survived by a pending attempt must not re-run it"
        );
    }

    #[tokio::test]
    async fn a_momentary_projection_flap_never_drops_a_computed_result() {
        // the flap lands AFTER the provider finished but BEFORE the result
        // was relayed: the Due entry holds the only copy of the computed
        // answer. dropping it re-runs the attempt on re-appearance (two
        // children, and the first answer is silently lost).
        let (mut pump, mut rx, executions) = pump_with(Duration::from_millis(10), true);
        let now = Instant::now();
        let key = ("job".to_string(), 0u32);

        assert!(pump.plan(vec![request("job", 0)], now).await.is_empty());
        pump.completed(next_result(&mut rx).await);
        assert!(pump.plan(Vec::new(), now).await.is_empty(), "flap tick");
        let due = pump.plan(vec![request("job", 0)], now).await;
        assert_eq!(due.len(), 1, "the computed result survives the flap");
        assert_eq!(due[0].0, key);
        assert_eq!(
            executions.load(Ordering::SeqCst),
            1,
            "the surviving result relays; the attempt never re-runs"
        );
    }

    #[tokio::test]
    async fn confirmed_absence_cancels_a_running_attempt_once_on_the_second_read() {
        let cancellations = Arc::new(AtomicUsize::new(0));
        let (mut pump, _rx, executions) =
            pump_with_cancellations(Duration::from_secs(5), true, cancellations.clone());
        let now = Instant::now();

        assert!(pump.plan(vec![request("job", 0)], now).await.is_empty());
        tokio::time::timeout(Duration::from_secs(1), async {
            while executions.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the provider starts");

        assert!(pump.plan(Vec::new(), now).await.is_empty(), "first miss");
        tokio::task::yield_now().await;
        assert_eq!(
            cancellations.load(Ordering::SeqCst),
            0,
            "one missing projection is flap-tolerant"
        );

        assert!(pump.plan(Vec::new(), now).await.is_empty(), "confirmed");
        tokio::time::timeout(Duration::from_secs(1), async {
            while cancellations.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("confirmed absence cancels the provider");
        assert_eq!(cancellations.load(Ordering::SeqCst), 1);
        assert!(pump.work.is_empty(), "the cancelled attempt retires");

        assert!(pump.plan(Vec::new(), now).await.is_empty());
        tokio::task::yield_now().await;
        assert_eq!(
            cancellations.load(Ordering::SeqCst),
            1,
            "retirement cannot cancel the same attempt twice"
        );
    }

    #[tokio::test]
    async fn a_higher_attempt_cancels_its_running_predecessor_on_first_sight() {
        let cancellations = Arc::new(AtomicUsize::new(0));
        let (mut pump, _rx, executions) =
            pump_with_cancellations(Duration::from_secs(5), true, cancellations.clone());
        let now = Instant::now();

        assert!(pump.plan(vec![request("job", 0)], now).await.is_empty());
        tokio::time::timeout(Duration::from_secs(1), async {
            while executions.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the first attempt starts");

        assert!(
            pump.plan(vec![request("job", 1)], now).await.is_empty(),
            "the replacement starts off-loop"
        );
        tokio::time::timeout(Duration::from_secs(1), async {
            while cancellations.load(Ordering::SeqCst) == 0
                || executions.load(Ordering::SeqCst) < 2
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the predecessor is cancelled and the replacement starts");
        assert_eq!(cancellations.load(Ordering::SeqCst), 1);
        assert!(!pump.work.contains_key(&("job".into(), 0)));
        assert!(pump.work.contains_key(&("job".into(), 1)));
    }

    #[tokio::test]
    async fn an_authoritative_remote_retry_cancels_on_the_first_missing_read() {
        let cancellations = Arc::new(AtomicUsize::new(0));
        let (mut pump, _rx, executions) =
            pump_with_cancellations(Duration::from_secs(5), true, cancellations.clone());
        let now = Instant::now();
        let old = ("job".to_string(), 0);

        assert!(pump.plan(vec![request("job", 0)], now).await.is_empty());
        tokio::time::timeout(Duration::from_secs(1), async {
            while executions.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the old local attempt starts");

        assert!(
            pump.plan_with_projection(
                Vec::new(),
                &HashSet::from([old.clone()]),
                &HashSet::new(),
                now,
            )
                .await
                .is_empty()
        );
        tokio::time::timeout(Duration::from_secs(1), async {
            while cancellations.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the committed remote retry cancels immediately");
        assert_eq!(cancellations.load(Ordering::SeqCst), 1);
        assert!(!pump.work.contains_key(&old));
    }

    #[tokio::test]
    async fn authoritative_active_state_resets_projection_misses() {
        let cancellations = Arc::new(AtomicUsize::new(0));
        let (mut pump, _rx, executions) =
            pump_with_cancellations(Duration::from_secs(5), true, cancellations.clone());
        let now = Instant::now();
        let key = ("job".to_string(), 0);
        let active = HashSet::from([key.clone()]);

        assert!(pump.plan(vec![request("job", 0)], now).await.is_empty());
        for _ in 0..3 {
            assert!(
                pump.plan_with_projection(Vec::new(), &HashSet::new(), &active, now)
                    .await
                    .is_empty()
            );
        }
        tokio::task::yield_now().await;
        assert_eq!(cancellations.load(Ordering::SeqCst), 0);
        assert!(pump.work.contains_key(&key));
        assert_eq!(executions.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn inline_gate_verdicts_are_due_without_spawning() {
        // the spec is loaded but NO provider is installed: the gate answers
        // the own-lease request inline with an error OracleResult — due for
        // relay immediately, nothing spawned, no result-lane traffic.
        let (mut pump, _rx, executions) = pump_with(Duration::from_millis(10), false);
        let now = Instant::now();

        let due = pump.plan(vec![request("job", 0)], now).await;
        assert_eq!(due.len(), 1, "the inline error result is due");
        match saga::decode_msg(&due[0].1.payload).expect("a saga op") {
            SagaMsg::OracleResult { outcome, .. } => {
                let err = outcome.unwrap_err();
                assert!(err.contains("\"alpha\" is not provided"), "got: {err}");
            }
            other => panic!("expected an OracleResult, got {other:?}"),
        }
        assert_eq!(executions.load(Ordering::SeqCst), 0, "nothing spawned");
    }

    #[tokio::test]
    async fn foreign_spec_shapes_are_skipped_quietly_and_never_rerun() {
        let (mut pump, _rx, executions) = pump_with(Duration::from_millis(10), true);
        let now = Instant::now();
        let foreign = WorkerRequest {
            spec: br#"{"run_id":"r","agent_id":"a"}"#.to_vec(),
            ..request("alien", 0)
        };
        assert!(
            pump.plan(vec![foreign.clone()], now).await.is_empty(),
            "a foreign spec produces no op"
        );
        assert!(
            matches!(
                pump.work.get(&("alien".into(), 0)).map(|e| &e.stage),
                Some(Stage::Settled)
            ),
            "and is latched so it is not re-decoded every tick"
        );
        assert!(pump.plan(vec![foreign], now).await.is_empty());
        assert_eq!(executions.load(Ordering::SeqCst), 0);
    }
}
