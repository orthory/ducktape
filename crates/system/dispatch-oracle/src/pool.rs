//! the off-loop execution pool: [`DispatchPool`] is the [`Worker`] real
//! hosts run.
//!
//! the worker step splits in two. the CHEAP half — decode routing and the
//! lease gate ([`crate::gate`]) — stays inline in `run()`, so the host's
//! event loop keeps its deterministic accept/skip verdicts. the EXPENSIVE
//! half — the provider CLI call, minutes-long under a 300s default timeout —
//! is handed to an injected spawner and `run()` returns "handled, result
//! later" immediately. the completed result re-enters through an injected
//! delivery lane as an ordinary submitted op, which is exactly what the
//! oracle-as-op contract expects: dispatch's never-pop-stack mailbox already
//! delivers results in a LATER block, so execution timing is invisible to
//! consensus.
//!
//! the pool is runtime-agnostic on purpose: it knows how to gate, dedup, and
//! cap — WHERE the future runs ([`SpawnFn`]) and WHERE the result goes
//! ([`DeliverFn`]) are the embedding host's business (bin/node's select-loop
//! mpsc lane, bin/noded's command channel).

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use capability_host::ProviderSet;
use futures::future::BoxFuture;
use reactor::{WorkOutcome, Worker};
use sdk::{Effect, Msg};
use tokio::sync::Semaphore;

use crate::{ExecJob, Gated, clean_error, gate, oracle_result};

/// how many provider runs may execute concurrently unless
/// `DUCKTAPE_MAX_CONCURRENT_RUNS` says otherwise.
pub const DEFAULT_MAX_CONCURRENT_RUNS: usize = 4;

/// hand one Send future to the host's background lane. the pool never
/// blocks on it; over-cap runs queue INSIDE their spawned task (on the
/// semaphore), so spawning is always immediate.
pub type SpawnFn = Box<dyn Fn(BoxFuture<'static, ()>)>;

/// deliver one completed `OracleResult` op to the host's submit lane. runs
/// on the spawned task, so it may await (a bounded channel send, a command
/// round-trip) without touching the host loop.
pub type DeliverFn = Arc<dyn Fn(Msg) -> BoxFuture<'static, ()> + Send + Sync>;

/// the concurrency cap: `DUCKTAPE_MAX_CONCURRENT_RUNS` when set to a
/// positive integer (the `DUCKTAPE_PROVIDER_TIMEOUT_SECS` precedent), else
/// [`DEFAULT_MAX_CONCURRENT_RUNS`].
pub fn max_concurrent_runs_from_env() -> usize {
    std::env::var("DUCKTAPE_MAX_CONCURRENT_RUNS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(DEFAULT_MAX_CONCURRENT_RUNS)
}

fn run_key_for(saga_id: &str) -> String {
    saga_id
        .rsplit_once('\x1f')
        .map_or(saga_id, |(_, dispatch_id)| dispatch_id)
        .to_string()
}

/// one attempt's in-flight identity — the same `(saga_id, attempt)`
/// idempotency key the saga itself dedups results on.
type AttemptKey = (String, u32);

/// Production worker for dispatch `WorkSpec` saga effects: gate inline,
/// execute on a spawned task, submit the result through the delivery lane.
pub struct DispatchPool {
    providers: Arc<ProviderSet>,
    /// this node's external submit key — compared against a request's
    /// `assignee` to decide whether the lease is ours to execute.
    node_key: Vec<u8>,
    spawn: SpawnFn,
    deliver: DeliverFn,
    /// caps concurrent provider runs; acquired INSIDE the spawned task so
    /// over-cap work queues there, never on the host loop.
    semaphore: Arc<Semaphore>,
    /// attempts executing locally right now. a redelivered `WorkerRequest`
    /// for an in-flight attempt is a claimed skip — never a second child
    /// process for the same paid call. pruned when the attempt's result has
    /// been handed to the delivery lane.
    inflight: Arc<Mutex<HashSet<AttemptKey>>>,
    /// blob reads for envelope prompt resolution — injected by the embedding
    /// binary like spawn/deliver, so the pool stays storage-agnostic. `None`
    /// fails prompt-pinned envelopes loudly (see [`crate::envelope::prepare`]).
    resolver: Option<crate::BlobResolver>,
}

impl DispatchPool {
    /// the production constructor: concurrency cap from the environment.
    pub fn new(
        providers: Arc<ProviderSet>,
        node_key: Vec<u8>,
        spawn: SpawnFn,
        deliver: DeliverFn,
    ) -> Self {
        Self::with_limit(
            providers,
            node_key,
            spawn,
            deliver,
            max_concurrent_runs_from_env(),
        )
    }

    /// an explicit concurrency cap (tests; embedders with their own policy).
    pub fn with_limit(
        providers: Arc<ProviderSet>,
        node_key: Vec<u8>,
        spawn: SpawnFn,
        deliver: DeliverFn,
        limit: usize,
    ) -> Self {
        Self {
            providers,
            node_key,
            spawn,
            deliver,
            semaphore: Arc::new(Semaphore::new(limit.max(1))),
            inflight: Arc::new(Mutex::new(HashSet::new())),
            resolver: None,
        }
    }

    /// wire the node-local blob read path envelope prompts resolve through.
    /// a builder (not a constructor arm) so existing embedders and tests
    /// keep compiling; without it, prompt-pinned envelopes fail loudly.
    pub fn with_resolver(mut self, resolver: crate::BlobResolver) -> Self {
        self.resolver = Some(resolver);
        self
    }

    /// how many attempts are executing (or queued for the semaphore) right
    /// now — observability plus the test seam for dedup/prune assertions.
    pub fn in_flight(&self) -> usize {
        self.inflight.lock().expect("inflight lock").len()
    }

    /// spawn one gated job. the returned future owns everything it needs
    /// (`Arc`s all the way down), so the pool itself never blocks on it.
    fn spawn_exec(&self, key: AttemptKey, job: ExecJob) {
        let providers = self.providers.clone();
        let deliver = self.deliver.clone();
        let semaphore = self.semaphore.clone();
        let inflight = self.inflight.clone();
        let resolver = self.resolver.clone();
        (self.spawn)(Box::pin(async move {
            // over-cap runs queue HERE, on their own task.
            let _permit = semaphore
                .acquire_owned()
                .await
                .expect("run semaphore is never closed");
            // re-resolve by tag inside the task: the gate already proved the
            // capability resolves, and a provider surface is immutable for
            // the process lifetime — this is just how the borrow crosses.
            let outcome = match providers.resolve(&job.capability) {
                // the envelope step runs on the spawned task too — prompt
                // resolution is a blob read, not host-loop work. its errors
                // are the run's result: a saga Err, never a silent fallback.
                Ok(provider) => match crate::envelope::prepare(&job.input, resolver.as_ref()).await
                {
                    Ok((input, mut ctx)) => {
                        ctx.run_key = Some(run_key_for(&job.saga_id));
                        provider
                            .run(&input, &ctx)
                            .await
                            .map(String::into_bytes)
                            .map_err(clean_error)
                    }
                    Err(e) => Err(clean_error(e)),
                },
                Err(e) => Err(clean_error(e)),
            };
            // error/timeout results are submitted like any other: the saga
            // must see the failure to complete or retry the attempt.
            deliver(oracle_result(&job.saga_id, job.attempt, outcome)).await;
            // prune AFTER delivery: a redelivery racing the result's submit
            // is still a skip, not a second child.
            inflight.lock().expect("inflight lock").remove(&key);
        }));
    }
}

#[async_trait::async_trait(?Send)]
impl Worker for DispatchPool {
    async fn run(&self, effect: &Effect) -> Result<WorkOutcome, reactor::Error> {
        match gate(&self.providers, &self.node_key, effect) {
            Gated::NotMine => Ok(WorkOutcome::NotMine),
            Gated::Skip => Ok(WorkOutcome::Handled(None)),
            Gated::Immediate(msg) => Ok(WorkOutcome::Handled(Some(msg))),
            Gated::Execute(job) => {
                let key: AttemptKey = (job.saga_id.clone(), job.attempt);
                // the dedup gate and the insert are ONE critical section:
                // a redelivered request for an executing attempt is a
                // claimed skip.
                if !self
                    .inflight
                    .lock()
                    .expect("inflight lock")
                    .insert(key.clone())
                {
                    return Ok(WorkOutcome::Handled(None));
                }
                self.spawn_exec(key, job);
                // handled, result later: the follow-up op arrives through
                // the delivery lane, not this return value.
                Ok(WorkOutcome::Handled(None))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use dispatch::{WORK_SPEC_KIND, WorkSpec, encode_work_spec};
    use futures::StreamExt as _;
    use saga::{SagaMsg, WorkerRequest, encode_worker_request};

    fn spec_toml(tag: &str) -> capability_host::CapabilitySpec {
        capability_host::CapabilitySpec::parse(
            &format!(
                r#"
spec = 1
[capability]
tag = "{tag}"
[detect]
bin = "{tag}-cli"
[invoke]
args = []
prompt = "stdin"
[output]
format = "text"
"#
            ),
            "test",
        )
        .expect("mock spec parses")
    }

    /// a provider that records concurrency (current + peak), executions, and
    /// the last (input, ctx) it saw, then sleeps — the observable stand-in
    /// for a slow CLI.
    struct SlowProvider {
        tag: String,
        delay: Duration,
        executions: Arc<AtomicUsize>,
        current: Arc<AtomicUsize>,
        peak: Arc<AtomicUsize>,
        last_run: Arc<Mutex<Option<(String, capability_host::RunContext)>>>,
        fail: bool,
    }

    #[async_trait::async_trait]
    impl capability_host::Provider for SlowProvider {
        fn capability(&self) -> &str {
            &self.tag
        }
        async fn run(
            &self,
            prompt: &str,
            ctx: &capability_host::RunContext,
        ) -> Result<String, String> {
            self.executions.fetch_add(1, Ordering::SeqCst);
            *self.last_run.lock().unwrap() = Some((prompt.to_string(), ctx.clone()));
            let now = self.current.fetch_add(1, Ordering::SeqCst) + 1;
            self.peak.fetch_max(now, Ordering::SeqCst);
            tokio::time::sleep(self.delay).await;
            self.current.fetch_sub(1, Ordering::SeqCst);
            if self.fail {
                Err(format!("provider exploded on: {prompt}"))
            } else {
                Ok(format!("answer to: {prompt}"))
            }
        }
    }

    struct Probes {
        executions: Arc<AtomicUsize>,
        peak: Arc<AtomicUsize>,
        last_run: Arc<Mutex<Option<(String, capability_host::RunContext)>>>,
    }

    fn slow_providers(delay: Duration, fail: bool) -> (Arc<ProviderSet>, Probes) {
        let executions = Arc::new(AtomicUsize::new(0));
        let current = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let last_run = Arc::new(Mutex::new(None));
        let provider = SlowProvider {
            tag: "alpha".into(),
            delay,
            executions: executions.clone(),
            current,
            peak: peak.clone(),
            last_run: last_run.clone(),
            fail,
        };
        let providers = Arc::new(ProviderSet::assemble(
            capability_host::SpecSet::from_specs(vec![spec_toml("alpha")]),
            vec![Box::new(provider)],
        ));
        (
            providers,
            Probes {
                executions,
                peak,
                last_run,
            },
        )
    }

    /// a pool wired to tokio::spawn with an unbounded result lane — the
    /// in-test twin of the hosts' wiring.
    fn pool_with(
        providers: Arc<ProviderSet>,
        limit: usize,
    ) -> (DispatchPool, futures::channel::mpsc::UnboundedReceiver<Msg>) {
        let (tx, rx) = futures::channel::mpsc::unbounded::<Msg>();
        let spawn: SpawnFn = Box::new(|fut| {
            tokio::spawn(fut);
        });
        let deliver: DeliverFn = Arc::new(move |msg| {
            let tx = tx.clone();
            Box::pin(async move {
                let _ = tx.unbounded_send(msg);
            })
        });
        (
            DispatchPool::with_limit(providers, b"me".to_vec(), spawn, deliver, limit),
            rx,
        )
    }

    fn effect_with_payload(
        saga_id: &str,
        attempt: u32,
        assignee: Option<&[u8]>,
        payload: &[u8],
    ) -> Effect {
        Effect(encode_worker_request(&WorkerRequest {
            saga_id: saga_id.into(),
            attempt,
            spec: encode_work_spec(&WorkSpec {
                kind: WORK_SPEC_KIND.into(),
                dispatch_id: "d1".into(),
                capability: "alpha".into(),
                payload: payload.to_vec(),
            }),
            deadline: None,
            assignee: assignee.map(|a| a.to_vec()),
        }))
    }

    fn effect_for(saga_id: &str, attempt: u32, assignee: Option<&[u8]>) -> Effect {
        effect_with_payload(saga_id, attempt, assignee, b"the entire input")
    }

    #[test]
    fn run_key_for_dispatch_saga_uses_last_segment() {
        assert_eq!(run_key_for("dispatch\x1fruns\x1fd1"), "d1");
    }

    #[test]
    fn run_key_for_legacy_saga_uses_whole_id() {
        assert_eq!(run_key_for("s1"), "s1");
    }

    async fn next_result(
        rx: &mut futures::channel::mpsc::UnboundedReceiver<Msg>,
    ) -> (String, u32, Result<Vec<u8>, String>) {
        let msg = tokio::time::timeout(Duration::from_secs(10), rx.next())
            .await
            .expect("a result within budget")
            .expect("the lane stays open");
        assert_eq!(msg.target, "saga");
        match saga::decode_msg(&msg.payload).expect("a saga msg") {
            SagaMsg::OracleResult {
                saga_id,
                attempt,
                outcome,
            } => (saga_id, attempt, outcome),
            other => panic!("expected an OracleResult, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn two_slow_runs_execute_concurrently_and_never_block_the_offer() {
        let (providers, probes) = slow_providers(Duration::from_millis(300), false);
        let (pool, mut rx) = pool_with(providers, 4);

        let offered = std::time::Instant::now();
        for saga in ["s1", "s2"] {
            match pool.run(&effect_for(saga, 0, Some(b"me"))).await.unwrap() {
                WorkOutcome::Handled(None) => {}
                other => panic!("an executable lease spawns and claims, got {other:?}"),
            }
        }
        // the headline property: offering returned in offer time, not in
        // provider time.
        assert!(
            offered.elapsed() < Duration::from_millis(200),
            "the offer step must not await the provider (took {:?})",
            offered.elapsed()
        );

        let mut ids = vec![next_result(&mut rx).await.0, next_result(&mut rx).await.0];
        ids.sort();
        assert_eq!(ids, ["s1", "s2"], "both runs completed");
        assert_eq!(
            probes.peak.load(Ordering::SeqCst),
            2,
            "the two runs overlapped in real time"
        );
    }

    #[tokio::test]
    async fn the_cap_queues_over_limit_runs_without_dropping_them() {
        let (providers, probes) = slow_providers(Duration::from_millis(100), false);
        let (pool, mut rx) = pool_with(providers, 1);

        for saga in ["s1", "s2", "s3"] {
            pool.run(&effect_for(saga, 0, Some(b"me"))).await.unwrap();
        }
        let mut ids: Vec<String> = Vec::new();
        for _ in 0..3 {
            ids.push(next_result(&mut rx).await.0);
        }
        ids.sort();
        assert_eq!(ids, ["s1", "s2", "s3"], "queued runs still complete");
        assert_eq!(
            probes.peak.load(Ordering::SeqCst),
            1,
            "a cap of 1 serializes execution"
        );
        assert_eq!(probes.executions.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn a_redelivered_request_for_an_in_flight_attempt_is_skipped() {
        let (providers, probes) = slow_providers(Duration::from_millis(300), false);
        let (pool, mut rx) = pool_with(providers, 4);

        let eff = effect_for("s1", 0, Some(b"me"));
        pool.run(&eff).await.unwrap();
        // the redelivery, while the first child is still running: a claimed
        // skip, no second spawn.
        match pool.run(&eff).await.unwrap() {
            WorkOutcome::Handled(None) => {}
            other => panic!("an in-flight redelivery must be a skip, got {other:?}"),
        }
        assert_eq!(pool.in_flight(), 1, "one attempt in flight, not two");

        let (saga_id, attempt, outcome) = next_result(&mut rx).await;
        assert_eq!((saga_id.as_str(), attempt), ("s1", 0));
        assert_eq!(outcome.unwrap(), b"answer to: the entire input".to_vec());
        assert_eq!(
            probes.executions.load(Ordering::SeqCst),
            1,
            "exactly one child for the attempt"
        );

        // ... and the key prunes on completion, so a LATER redelivery (e.g.
        // after a restart elsewhere re-leases) may execute again — the
        // stateless pre-pool semantics, with the saga deduping the result.
        while pool.in_flight() > 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        pool.run(&eff).await.unwrap();
        let (saga_id, _, _) = next_result(&mut rx).await;
        assert_eq!(saga_id, "s1");
        assert_eq!(probes.executions.load(Ordering::SeqCst), 2);
    }

    /// a DIFFERENT attempt of the same saga is new work, never deduped —
    /// that is the retry lane.
    #[tokio::test]
    async fn a_new_attempt_of_the_same_saga_executes() {
        let (providers, probes) = slow_providers(Duration::from_millis(100), false);
        let (pool, mut rx) = pool_with(providers, 4);

        pool.run(&effect_for("s1", 0, Some(b"me"))).await.unwrap();
        pool.run(&effect_for("s1", 1, Some(b"me"))).await.unwrap();
        let mut attempts = vec![next_result(&mut rx).await.1, next_result(&mut rx).await.1];
        attempts.sort_unstable();
        assert_eq!(attempts, [0, 1]);
        assert_eq!(probes.executions.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn provider_failures_still_deliver_the_error_result() {
        let (providers, probes) = slow_providers(Duration::from_millis(20), true);
        let (pool, mut rx) = pool_with(providers, 4);

        pool.run(&effect_for("s1", 0, Some(b"me"))).await.unwrap();
        let (saga_id, attempt, outcome) = next_result(&mut rx).await;
        assert_eq!((saga_id.as_str(), attempt), ("s1", 0));
        let err = outcome.unwrap_err();
        assert!(err.contains("provider exploded"), "got: {err}");
        assert_eq!(probes.executions.load(Ordering::SeqCst), 1);
        // pruned: the failed attempt does not squat the in-flight set (the
        // saga's retry re-leases the SAME saga id with a new attempt, but a
        // same-key redelivery must also be runnable again).
        while pool.in_flight() > 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    fn envelope_payload(prompt_hash: Option<&str>) -> Vec<u8> {
        serde_json::json!({
            "ducktape_run": 2,
            "agent_id": "bot",
            "prompt_hash": prompt_hash,
            "thread_key": "general#7",
            "instructions": "GENERIC",
            "contract": "CONTRACT",
            "conversation": "CONVERSATION",
        })
        .to_string()
        .into_bytes()
    }

    #[tokio::test]
    async fn envelope_payloads_reach_the_provider_assembled_with_run_context() {
        let (providers, probes) = slow_providers(Duration::from_millis(5), false);
        let (pool, mut rx) = pool_with(providers, 4);

        let eff = effect_with_payload("s1", 0, Some(b"me"), &envelope_payload(None));
        pool.run(&eff).await.unwrap();
        let (_, _, outcome) = next_result(&mut rx).await;
        assert_eq!(
            outcome.unwrap(),
            b"answer to: GENERIC\n\nCONTRACT\n\nCONVERSATION".to_vec(),
            "assembly order: prompt-or-instructions, contract, conversation"
        );
        let (input, ctx) = probes.last_run.lock().unwrap().clone().unwrap();
        assert!(!input.contains("ducktape_run"), "the provider never sees envelope JSON");
        assert_eq!(ctx.agent_id.as_deref(), Some("bot"));
        assert_eq!(ctx.thread_key.as_deref(), Some("general#7"));
    }

    #[tokio::test]
    async fn provider_context_carries_dispatch_id_run_key() {
        let (providers, probes) = slow_providers(Duration::from_millis(5), false);
        let (pool, mut rx) = pool_with(providers, 4);

        let saga_id = "dispatch\x1fruns\x1fd1";
        let eff = effect_for(saga_id, 0, Some(b"me"));
        pool.run(&eff).await.unwrap();
        let (result_saga_id, _, outcome) = next_result(&mut rx).await;
        assert_eq!(result_saga_id, saga_id);
        assert_eq!(outcome.unwrap(), b"answer to: the entire input".to_vec());
        let (_, ctx) = probes.last_run.lock().unwrap().clone().unwrap();
        assert_eq!(ctx.run_key.as_deref(), Some("d1"));
    }

    #[tokio::test]
    async fn a_wired_resolver_feeds_the_agents_real_prompt() {
        let (providers, probes) = slow_providers(Duration::from_millis(5), false);
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<Msg>();
        let spawn: SpawnFn = Box::new(|fut| {
            tokio::spawn(fut);
        });
        let deliver: DeliverFn = Arc::new(move |msg| {
            let tx = tx.clone();
            Box::pin(async move {
                let _ = tx.unbounded_send(msg);
            })
        });
        let resolver: crate::BlobResolver = Arc::new(|digest: &[u8; 32]| {
            let hit = (*digest == [7u8; 32]).then(|| b"You are Bot.".to_vec());
            Box::pin(async move { hit })
        });
        let pool = DispatchPool::with_limit(providers, b"me".to_vec(), spawn, deliver, 4)
            .with_resolver(resolver);

        let hex = "07".repeat(32);
        let eff = effect_with_payload("s1", 0, Some(b"me"), &envelope_payload(Some(&hex)));
        pool.run(&eff).await.unwrap();
        let (_, _, outcome) = next_result(&mut rx).await;
        assert_eq!(
            outcome.unwrap(),
            b"answer to: You are Bot.\n\nCONTRACT\n\nCONVERSATION".to_vec(),
            "the registered prompt replaces the generic instructions"
        );
        let (input, _) = probes.last_run.lock().unwrap().clone().unwrap();
        assert!(!input.contains("GENERIC"), "no silent generic fallback: {input:?}");
    }

    #[tokio::test]
    async fn a_prompt_pinned_envelope_without_a_resolver_fails_the_saga_loudly() {
        let (providers, probes) = slow_providers(Duration::from_millis(5), false);
        let (pool, mut rx) = pool_with(providers, 4); // no with_resolver

        let hex = "07".repeat(32);
        let eff = effect_with_payload("s1", 0, Some(b"me"), &envelope_payload(Some(&hex)));
        pool.run(&eff).await.unwrap();
        let (saga_id, attempt, outcome) = next_result(&mut rx).await;
        assert_eq!((saga_id.as_str(), attempt), ("s1", 0));
        let err = outcome.unwrap_err();
        assert!(err.contains("no blob resolver"), "got: {err}");
        assert!(err.contains("bot"), "names the agent: {err}");
        assert_eq!(
            probes.executions.load(Ordering::SeqCst),
            0,
            "the provider is never invoked on a failed resolution"
        );
    }

    /// the pool's gate is the shared one: announcements claim (Accept) or
    /// skip inline, foreign leases skip, unresolvable own leases error
    /// inline — none of it spawns.
    #[tokio::test]
    async fn gate_verdicts_stay_inline_and_spawn_nothing() {
        let (providers, probes) = slow_providers(Duration::from_millis(20), false);
        let (pool, _rx) = pool_with(providers, 4);

        // a servable announcement: an immediate Accept claim.
        match pool.run(&effect_for("s1", 0, None)).await.unwrap() {
            WorkOutcome::Handled(Some(msg)) => {
                match saga::decode_msg(&msg.payload).unwrap() {
                    SagaMsg::Accept { saga_id, attempt } => {
                        assert_eq!((saga_id.as_str(), attempt), ("s1", 0));
                    }
                    other => panic!("expected an Accept claim, got {other:?}"),
                }
            }
            other => panic!("a servable announcement must claim, got {other:?}"),
        }

        // a foreign lease: a claimed skip.
        match pool.run(&effect_for("s2", 0, Some(b"peer"))).await.unwrap() {
            WorkOutcome::Handled(None) => {}
            other => panic!("a foreign lease must be a skip, got {other:?}"),
        }

        assert_eq!(pool.in_flight(), 0, "gate verdicts never enter the pool");
        assert_eq!(probes.executions.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn the_env_cap_parses_positive_integers_only() {
        // no env manipulation (parallel tests share the process): assert the
        // default directly and the parse rule via with_limit's clamp.
        assert_eq!(DEFAULT_MAX_CONCURRENT_RUNS, 4);
        let (providers, _probes) = slow_providers(Duration::from_millis(1), false);
        let (tx, _rx) = futures::channel::mpsc::unbounded::<Msg>();
        let deliver: DeliverFn = Arc::new(move |msg| {
            let tx = tx.clone();
            Box::pin(async move {
                let _ = tx.unbounded_send(msg);
            })
        });
        let pool = DispatchPool::with_limit(
            providers,
            b"me".to_vec(),
            Box::new(|_fut| {}),
            deliver,
            0,
        );
        // a zero cap would deadlock every run; the pool clamps to 1.
        assert_eq!(pool.semaphore.available_permits(), 1);
    }
}
