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
use std::time::Duration;

use capability_host::ProviderSet;
use futures::future::BoxFuture;
use reactor::{WorkOutcome, Worker};
use sdk::{Effect, Msg};
use tokio::sync::Semaphore;

use crate::provision::{SharedProvisioner, WorkspaceSpec, assemble_runner_result, bind_workspace};
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
    /// materializes/commits/cleans a per-run duckfs workspace for portable
    /// (v3) runs — injected by the node binary, where duckfs-client + the
    /// actor lane are reachable. `None` (the default) keeps the accept-only
    /// degrade: a v3 plan is surfaced but never activated, so the run executes
    /// like a legacy one (raw-text delivery, no workspace). both node binaries
    /// wire a real provisioner, so this is LIVE on every production agent run.
    provisioner: Option<SharedProvisioner>,
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
            provisioner: None,
        }
    }

    /// wire the node-local blob read path envelope prompts resolve through.
    /// a builder (not a constructor arm) so existing embedders and tests
    /// keep compiling; without it, prompt-pinned envelopes fail loudly.
    pub fn with_resolver(mut self, resolver: crate::BlobResolver) -> Self {
        self.resolver = Some(resolver);
        self
    }

    /// wire the host-side workspace provisioner portable (v3) runs materialize
    /// through — mirrors [`Self::with_resolver`]. without it (or on any run
    /// that carries no v3 plan) the pool takes the legacy branch: the
    /// provider's raw text is delivered verbatim, no workspace exists. the
    /// production binaries always wire one; v3 envelopes are live.
    pub fn with_provisioner(mut self, provisioner: SharedProvisioner) -> Self {
        self.provisioner = Some(provisioner);
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
        let provisioner = self.provisioner.clone();
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
                    Ok(mut prepared) => {
                        // the live-output registry key: the dispatch_id half of
                        // the saga id, exactly what the app's PendingRun rows
                        // expose — set before execute so BOTH the legacy and
                        // provisioned paths carry it into provider.run.
                        prepared.ctx.run_key = Some(run_key_for(&job.saga_id));
                        execute(&job, prepared, provider, provisioner.as_ref())
                            .await
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

/// provision → bind → run → commit → assemble → cleanup, at the dispatch
/// boundary on the spawned task.
///
/// DORMANT unless BOTH a v3 plan AND a wired provisioner are present: without
/// either, this is byte-for-byte today's `provider.run(&input, &ctx).await`
/// with the raw text as the delivered bytes. on the portable path the winning
/// attempt's bytes are the host-assembled `RunnerResult` (prose + receipt).
/// commit runs ONLY on a successful run (a failed/timed-out run yields a saga
/// `Err` with the dir cleaned up and no `output_ref`); cleanup always runs
/// (W5). a commit-mechanism failure degrades to a `no_changes` receipt (R4) —
/// the run's answer is never lost to a receipt-plumbing error.
/// the workspace bracket's host-side bound (#298): provision and commit both
/// block on the daemon actor lane, and a stalled lane must not pin one of
/// the pool's `DUCKTAPE_MAX_CONCURRENT_RUNS` permits until the saga deadline
/// re-leases elsewhere — the step fails, the attempt settles, the lease
/// moves on. the model call itself is bounded separately (X3, in
/// capability-host). tests shrink the window so a hung step is observable
/// without a wall-clock minute.
fn workspace_step_timeout() -> Duration {
    if cfg!(test) {
        Duration::from_millis(250)
    } else {
        Duration::from_secs(60)
    }
}

async fn execute(
    job: &ExecJob,
    prepared: crate::envelope::Prepared,
    provider: &dyn capability_host::Provider,
    provisioner: Option<&SharedProvisioner>,
) -> Result<Vec<u8>, String> {
    let crate::envelope::Prepared {
        input,
        mut ctx,
        workspace,
    } = prepared;
    let Some((plan, prov)) = workspace.zip(provisioner) else {
        // accept-only / legacy: unchanged behavior, raw text bytes.
        return provider.run(&input, &ctx).await.map(String::into_bytes);
    };
    // the REQUESTED sink (Chain when the envelope carried none) — echoed on
    // the assembled RunnerResult below so runs' delivery can route it.
    let sink = plan.sink;
    let spec = WorkspaceSpec {
        run_id: format!("{}:{}", job.saga_id, job.attempt),
        agent_id: ctx.agent_id.clone(),
        // the tagged source (duckfs subtree or forge repo@commit) crosses to
        // the provisioner verbatim — the pool never interprets it.
        source: plan.source,
        // the composer emits SOURCE coords only (D7); the provisioner mints its
        // own writable cwd, so mount_path is advisory-empty here.
        mount_path: String::new(),
        base_tools: plan.base_tools,
        ro_mounts: plan.skills, // C4 skill ro mounts (phase 5)
    };
    // (a)+(b) materialize OUTSIDE storage — bounded: a stalled actor lane
    // fails this attempt instead of pinning a pool permit to the saga
    // deadline. nothing exists yet, so there is nothing to clean up.
    let ws = tokio::time::timeout(workspace_step_timeout(), prov.provision(&spec))
        .await
        .map_err(|_| {
            format!(
                "workspace provision for {} timed out after {:?}",
                spec.run_id,
                workspace_step_timeout()
            )
        })??;
    bind_workspace(ws.as_ref(), &mut ctx); // set workdir_override/env/path_entries
    // run → commit → assemble, unwind-guarded: a panicking provider (or
    // receipt path) must not skip the cleanup below and leak the per-run
    // dir. the panic surfaces as this attempt's Err — the saga settles a
    // failed attempt instead of a silent task death.
    let outcome = futures::FutureExt::catch_unwind(std::panic::AssertUnwindSafe(async {
        match provider.run(&input, &ctx).await {
            Ok(text) => {
                // (d) capture output_ref. a commit-MECHANISM failure (conflict,
                // transport, rejection, a hung actor lane) must never
                // masquerade as a clean tree: the receipt records the error
                // and the status degrades, while the run's answer still
                // delivers (R4 — never lost to receipt plumbing). only
                // `CommitError::Nothing` is a true `no_changes`, and the
                // workspace impl already maps that to Ok.
                let commit = tokio::time::timeout(
                    workspace_step_timeout(),
                    ws.commit(&format!("agent run {}", spec.run_id)),
                )
                .await
                .unwrap_or_else(|_| {
                    Err(format!(
                        "commit timed out after {:?}",
                        workspace_step_timeout()
                    ))
                });
                let (receipt, status) = match commit {
                    Ok(receipt) => (receipt, crate::provision::Status::Ok),
                    Err(e) => {
                        eprintln!("[oracle] commit failed for {}: {e}", spec.run_id);
                        (
                            crate::provision::WorkspaceReceipt::commit_failed(&spec, e),
                            crate::provision::Status::Degraded,
                        )
                    }
                };
                // LIFT the model's task actions into the effects facet: runs
                // applies the host-assembled effects; an empty result lets runs
                // fall back to the response-parsed actions. the sink is the
                // plan's REQUESTED sink (Chain unless the composer named one) —
                // echoed so runs' delivery routes the output without re-deriving
                // intent; the data facet stays host-observed later.
                let effects = crate::provision::effects_from_response_text(&text);
                Ok(assemble_runner_result(
                    &text,
                    &receipt,
                    None,
                    effects,
                    sink,
                    status,
                ))
            }
            Err(e) => Err(e), // failed run: no commit, no output_ref
        }
    }))
    .await;
    ws.cleanup().await; // (e) W5 always — even past a panicking provider
    match outcome {
        Ok(result) => result,
        Err(panic) => Err(format!(
            "provider panicked mid-run: {}",
            panic
                .downcast_ref::<&str>()
                .map(|s| (*s).to_string())
                .or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "non-string panic payload".into())
        )),
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
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::Duration;

    use crate::provision::{ProvisionedWorkspace, WorkspaceReceipt};
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

    // ---- the portable (v3) provisioning bracket -----------------------------

    /// a v3 (portable) run envelope payload — the shape the composer emits for
    /// every run when a files module is wired (the production default). carried
    /// here to exercise the pool's bracket without a live duckfs (the mock
    /// stands in for the checkout engine).
    fn v3_envelope_payload() -> Vec<u8> {
        serde_json::json!({
            "ducktape_run": 3,
            "agent_id": "bot",
            "prompt_hash": null,
            "thread_key": "general#7",
            "instructions": "GENERIC",
            "contract": "CONTRACT",
            "conversation": "CONVERSATION",
            "workspace": {
                "kind": "duckfs",
                "source_prefix": "/shared/agent-workspaces/bot",
                "source_snapshot": "aa".repeat(32)
            },
            "base_tools": [
                {"name":"ducktape-files","version":"1","exposure":"cli"}
            ],
            "skills": [
                {"name":"release","source_prefix":"/shared/skills/release","source_snapshot": "bb".repeat(32)}
            ],
            "result_contract": {"ducktape_runner_result": 1}
        })
        .to_string()
        .into_bytes()
    }

    /// a forge-sourced v3 run envelope — the EXACT byte shapes task 1's
    /// composer emits (task-1 report §"Exact final serde shapes"): tagged
    /// forge workspace, `context`, requested-Pr sink WITHOUT title/body keys.
    fn forge_envelope_payload() -> Vec<u8> {
        serde_json::json!({
            "ducktape_run": 3,
            "agent_id": "bot",
            "prompt_hash": null,
            "thread_key": "forge:app:7#2",
            "instructions": "GENERIC",
            "contract": "CONTRACT",
            "conversation": "CONVERSATION",
            "context": "Forge item context — you are working this item as a session.\nrepo: app\nitem: issue #7 (open)",
            "workspace": {
                "kind": "forge",
                "repo": "app",
                "commit": "d0".repeat(20),
                "branch": "agent/item-7",
                "branch_born": false
            },
            "base_tools": [
                {"name":"ducktape-files","version":"1","exposure":"cli"}
            ],
            "skills": [],
            "result_contract": {
                "ducktape_runner_result": 1,
                "sink": {"mode":"pr","repo":"app","source_branch":"agent/item-7","target_branch":"main"}
            }
        })
        .to_string()
        .into_bytes()
    }

    /// the winning-attempt bytes a portable run assembles: the mock stand-in
    /// for a real duckfs checkout. records that provision/commit/cleanup fired,
    /// binds a deterministic mount + env the provider observes, and (on commit)
    /// mints a fake output_ref — or fails the commit when `fail_commit` is set.
    struct MockProvisioner {
        provisioned: Arc<AtomicBool>,
        committed: Arc<AtomicBool>,
        cleaned: Arc<AtomicBool>,
        fail_commit: Option<String>,
    }

    #[async_trait::async_trait]
    impl crate::provision::WorkspaceProvisioner for MockProvisioner {
        async fn provision(
            &self,
            spec: &WorkspaceSpec,
        ) -> Result<Box<dyn ProvisionedWorkspace>, String> {
            self.provisioned.store(true, Ordering::SeqCst);
            let dir = std::env::temp_dir()
                .join(format!("mock-ws-{}", spec.run_id.replace(':', "_")));
            let mut env = BTreeMap::new();
            env.insert("DUCKTAPE_RUN_WORKSPACE".into(), dir.display().to_string());
            // kind-agnostic: echo whatever source the spec carries, exactly
            // like a real receipt would (duckfs prefix/pin or forge:<repo>).
            let (src, snap) = spec.source.receipt_coords();
            Ok(Box::new(MockWs {
                dir,
                src,
                snap,
                env,
                committed: self.committed.clone(),
                cleaned: self.cleaned.clone(),
                fail_commit: self.fail_commit.clone(),
            }))
        }
    }

    struct MockWs {
        dir: PathBuf,
        src: String,
        snap: Option<String>,
        env: BTreeMap<String, String>,
        committed: Arc<AtomicBool>,
        cleaned: Arc<AtomicBool>,
        /// `Some` makes commit() fail with this error — the commit-mechanism
        /// failure seam.
        fail_commit: Option<String>,
    }

    #[async_trait::async_trait]
    impl ProvisionedWorkspace for MockWs {
        fn workdir(&self) -> PathBuf {
            self.dir.clone()
        }
        fn env(&self) -> BTreeMap<String, String> {
            self.env.clone()
        }
        fn path_entries(&self) -> Vec<PathBuf> {
            Vec::new()
        }
        async fn commit(&self, _message: &str) -> Result<WorkspaceReceipt, String> {
            self.committed.store(true, Ordering::SeqCst);
            if let Some(err) = &self.fail_commit {
                return Err(err.clone());
            }
            Ok(WorkspaceReceipt {
                source_prefix: self.src.clone(),
                source_snapshot: self.snap.clone(),
                output_snapshot: Some("cc".repeat(32)),
                commit_height: Some(9),
                rebased: false,
                no_changes: false,
                commit_error: None,
                branch: None,
                output_commit: None,
            })
        }
        async fn cleanup(&self) {
            self.cleaned.store(true, Ordering::SeqCst);
        }
    }

    fn flags() -> (Arc<AtomicBool>, Arc<AtomicBool>, Arc<AtomicBool>) {
        (
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
        )
    }

    fn pool_with_provisioner(
        providers: Arc<ProviderSet>,
        provisioner: SharedProvisioner,
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
            DispatchPool::with_limit(providers, b"me".to_vec(), spawn, deliver, 4)
                .with_provisioner(provisioner),
            rx,
        )
    }

    #[tokio::test]
    async fn a_v3_run_with_a_provisioner_wired_provisions_binds_commits_and_wraps_the_result() {
        let (providers, probes) = slow_providers(Duration::from_millis(5), false);
        let (provisioned, committed, cleaned) = flags();
        let provisioner: SharedProvisioner = Arc::new(MockProvisioner {
            provisioned: provisioned.clone(),
            committed: committed.clone(),
            cleaned: cleaned.clone(),
            fail_commit: None,
        });
        let (pool, mut rx) = pool_with_provisioner(providers, provisioner);

        let eff = effect_with_payload("s1", 0, Some(b"me"), &v3_envelope_payload());
        pool.run(&eff).await.unwrap();
        let (saga_id, attempt, outcome) = next_result(&mut rx).await;
        assert_eq!((saga_id.as_str(), attempt), ("s1", 0));

        // the delivered bytes are a host-assembled RunnerResult, NOT raw text.
        let bytes = outcome.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["ducktape_runner_result"], 1);
        assert_eq!(v["response_text"], "answer to: GENERIC\n\nCONTRACT\n\nCONVERSATION");
        assert_eq!(v["workspace_receipt"]["output_snapshot"], "cc".repeat(32));
        assert_eq!(v["workspace_receipt"]["commit_height"], 9);
        assert_eq!(v["workspace_receipt"]["source_prefix"], "/shared/agent-workspaces/bot");
        assert_eq!(v["workspace_receipt"]["no_changes"], false);
        assert!(
            !v.as_object().unwrap().contains_key("sink"),
            "a chain-sink run keeps the sink key skip-serialized"
        );

        // the full lifecycle fired, and the provider ran INSIDE the mount.
        assert!(provisioned.load(Ordering::SeqCst), "provision was called");
        assert!(committed.load(Ordering::SeqCst), "commit ran on success");
        assert!(cleaned.load(Ordering::SeqCst), "cleanup always runs (W5)");
        let (_, ctx) = probes.last_run.lock().unwrap().clone().unwrap();
        let expected = std::env::temp_dir().join("mock-ws-s1_0");
        assert_eq!(
            ctx.workdir_override.as_deref(),
            Some(expected.as_path()),
            "the provider observed the bound mount as its cwd"
        );
        assert_eq!(
            ctx.env.get("DUCKTAPE_RUN_WORKSPACE").map(String::as_str),
            Some(expected.display().to_string().as_str()),
            "the run-scoped workspace env was applied"
        );
        assert!(ctx.portable, "a v3 run is portable");
    }

    /// a probe provisioner that captures the [`WorkspaceSpec::ro_mounts`] it is
    /// handed — proving the skills seam composer → WireSkill → PortablePlan.skills
    /// → WorkspaceSpec.ro_mounts (critic #5).
    struct RoMountProbe {
        captured: Arc<Mutex<Vec<crate::provision::RoMount>>>,
    }

    #[async_trait::async_trait]
    impl crate::provision::WorkspaceProvisioner for RoMountProbe {
        async fn provision(
            &self,
            spec: &WorkspaceSpec,
        ) -> Result<Box<dyn ProvisionedWorkspace>, String> {
            *self.captured.lock().unwrap() = spec.ro_mounts.clone();
            Ok(Box::new(ProbeWs))
        }
    }

    struct ProbeWs;

    #[async_trait::async_trait]
    impl ProvisionedWorkspace for ProbeWs {
        fn workdir(&self) -> PathBuf {
            std::env::temp_dir().join("probe-ws")
        }
        fn env(&self) -> BTreeMap<String, String> {
            BTreeMap::new()
        }
        fn path_entries(&self) -> Vec<PathBuf> {
            Vec::new()
        }
        async fn commit(&self, _message: &str) -> Result<WorkspaceReceipt, String> {
            Ok(WorkspaceReceipt {
                source_prefix: String::new(),
                source_snapshot: None,
                output_snapshot: None,
                commit_height: None,
                rebased: false,
                no_changes: true,
                commit_error: None,
                branch: None,
                output_commit: None,
            })
        }
        async fn cleanup(&self) {}
    }

    #[tokio::test]
    async fn a_v3_runs_skills_reach_the_spec_as_ro_mounts() {
        let (providers, _probes) = slow_providers(Duration::from_millis(5), false);
        let captured: Arc<Mutex<Vec<crate::provision::RoMount>>> = Arc::new(Mutex::new(Vec::new()));
        let provisioner: SharedProvisioner = Arc::new(RoMountProbe {
            captured: captured.clone(),
        });
        let (pool, mut rx) = pool_with_provisioner(providers, provisioner);

        let eff = effect_with_payload("s1", 0, Some(b"me"), &v3_envelope_payload());
        pool.run(&eff).await.unwrap();
        let _ = next_result(&mut rx).await;

        let mounts = captured.lock().unwrap().clone();
        assert_eq!(mounts.len(), 1, "the one composed skill became a ro mount");
        assert_eq!(mounts[0].mount_subpath, "release");
        assert_eq!(mounts[0].source_prefix, "/shared/skills/release");
        assert_eq!(
            mounts[0].source_snapshot.as_deref(),
            Some("bb".repeat(32).as_str())
        );
    }

    /// a probe provisioner that captures the WHOLE [`WorkspaceSpec`] — how the
    /// forge tests observe that the tagged source crossed envelope →
    /// PortablePlan → WorkspaceSpec intact.
    struct SpecProbe {
        captured: Arc<Mutex<Option<WorkspaceSpec>>>,
    }

    #[async_trait::async_trait]
    impl crate::provision::WorkspaceProvisioner for SpecProbe {
        async fn provision(
            &self,
            spec: &WorkspaceSpec,
        ) -> Result<Box<dyn ProvisionedWorkspace>, String> {
            *self.captured.lock().unwrap() = Some(spec.clone());
            Ok(Box::new(ProbeWs))
        }
    }

    #[tokio::test]
    async fn a_forge_runs_pinned_source_reaches_the_provisioners_spec() {
        // the forge half of the skills-reach-the-spec pattern: the tagged
        // source survives envelope → PortablePlan → WorkspaceSpec verbatim,
        // so task 3's provisioner sees exactly the committed pin.
        let (providers, _probes) = slow_providers(Duration::from_millis(5), false);
        let captured: Arc<Mutex<Option<WorkspaceSpec>>> = Arc::new(Mutex::new(None));
        let provisioner: SharedProvisioner = Arc::new(SpecProbe {
            captured: captured.clone(),
        });
        let (pool, mut rx) = pool_with_provisioner(providers, provisioner);

        let eff = effect_with_payload("s1", 0, Some(b"me"), &forge_envelope_payload());
        pool.run(&eff).await.unwrap();
        let _ = next_result(&mut rx).await;

        let spec = captured.lock().unwrap().clone().expect("the spec was captured");
        assert_eq!(spec.run_id, "s1:0");
        assert_eq!(
            spec.source,
            crate::workspace_source::WorkspaceSource::Forge {
                repo: "app".into(),
                commit: "d0".repeat(20),
                branch: "agent/item-7".into(),
                branch_born: false,
            }
        );
        assert_eq!(
            spec.mount_path, "",
            "the dead-advisory mount_path is NOT propagated for forge"
        );
    }

    #[tokio::test]
    async fn the_requested_sink_is_echoed_on_the_runner_result() {
        // O1/O2 threading: the plan's requested Pr sink rides the assembled
        // RunnerResult — with title/body as PRESENT empty keys (runs' decode
        // keeps title required; delivery derives the real text, contract §3).
        let (providers, _probes) = slow_providers(Duration::from_millis(5), false);
        let (provisioned, committed, cleaned) = flags();
        let provisioner: SharedProvisioner = Arc::new(MockProvisioner {
            provisioned: provisioned.clone(),
            committed: committed.clone(),
            cleaned: cleaned.clone(),
            fail_commit: None,
        });
        let (pool, mut rx) = pool_with_provisioner(providers, provisioner);

        let eff = effect_with_payload("s1", 0, Some(b"me"), &forge_envelope_payload());
        pool.run(&eff).await.unwrap();
        let (_, _, outcome) = next_result(&mut rx).await;

        let bytes = outcome.unwrap();
        let raw = String::from_utf8(bytes.clone()).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["sink"]["mode"], "pr", "the requested sink is echoed: {v}");
        assert_eq!(v["sink"]["repo"], "app");
        assert_eq!(v["sink"]["source_branch"], "agent/item-7");
        assert_eq!(v["sink"]["target_branch"], "main");
        // the interop obligation: empty title/body are PRESENT keys, never
        // skipped — runs' WireSink keeps title required on decode.
        assert!(
            raw.contains(r#""title":"""#) && raw.contains(r#""body":"""#),
            "empty title/body must be present keys: {raw}"
        );
        // and the forge receipt coords rode along (§5).
        assert_eq!(v["workspace_receipt"]["source_prefix"], "forge:app");
        assert_eq!(v["workspace_receipt"]["source_snapshot"], "d0".repeat(20));
    }

    #[tokio::test]
    async fn a_hung_commit_on_the_forge_path_times_out_into_a_degraded_receipt() {
        // the #298 timeout bracket covers the forge path exactly as duckfs:
        // a stalled commit step fails the capture, the answer still delivers.
        let (providers, _probes) = slow_providers(Duration::from_millis(5), false);
        let cleaned = Arc::new(AtomicBool::new(false));
        let provisioner: SharedProvisioner = Arc::new(HungCommitProvisioner {
            cleaned: cleaned.clone(),
        });
        let (pool, mut rx) = pool_with_provisioner(providers, provisioner);

        let eff = effect_with_payload("s1", 0, Some(b"me"), &forge_envelope_payload());
        pool.run(&eff).await.unwrap();
        let (_, _, outcome) = next_result(&mut rx).await;

        let bytes = outcome.expect("the run's answer still delivers (R4)");
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["status"], "degraded");
        assert!(
            v["workspace_receipt"]["commit_error"]
                .as_str()
                .is_some_and(|e| e.contains("timed out")),
            "the receipt records the timeout: {v}"
        );
        assert!(cleaned.load(Ordering::SeqCst), "cleanup still runs (W5)");
    }

    #[tokio::test]
    async fn a_panicking_provider_on_the_forge_path_still_cleans_up() {
        // the unwind guard covers the forge path exactly as duckfs: no commit,
        // no leaked per-run dir, the attempt settles as a failure.
        let providers = Arc::new(ProviderSet::assemble(
            capability_host::SpecSet::from_specs(vec![spec_toml("alpha")]),
            vec![Box::new(PanicProvider { tag: "alpha".into() })],
        ));
        let (provisioned, committed, cleaned) = flags();
        let provisioner: SharedProvisioner = Arc::new(MockProvisioner {
            provisioned: provisioned.clone(),
            committed: committed.clone(),
            cleaned: cleaned.clone(),
            fail_commit: None,
        });
        let (pool, mut rx) = pool_with_provisioner(providers, provisioner);

        let eff = effect_with_payload("s1", 0, Some(b"me"), &forge_envelope_payload());
        pool.run(&eff).await.unwrap();
        let (_, _, outcome) = next_result(&mut rx).await;

        let err = outcome.expect_err("a panic settles the attempt as a failure");
        assert!(err.contains("panicked"), "got {err}");
        assert!(provisioned.load(Ordering::SeqCst));
        assert!(!committed.load(Ordering::SeqCst), "a panicked run commits NOTHING");
        assert!(cleaned.load(Ordering::SeqCst), "cleanup runs past the panic (W5)");
    }

    #[tokio::test]
    async fn a_failed_v3_run_cleans_up_without_committing_and_delivers_the_error() {
        let (providers, _probes) = slow_providers(Duration::from_millis(5), true);
        let (provisioned, committed, cleaned) = flags();
        let provisioner: SharedProvisioner = Arc::new(MockProvisioner {
            provisioned: provisioned.clone(),
            committed: committed.clone(),
            cleaned: cleaned.clone(),
            fail_commit: None,
        });
        let (pool, mut rx) = pool_with_provisioner(providers, provisioner);

        let eff = effect_with_payload("s1", 0, Some(b"me"), &v3_envelope_payload());
        pool.run(&eff).await.unwrap();
        let (_, _, outcome) = next_result(&mut rx).await;

        let err = outcome.unwrap_err();
        assert!(err.contains("provider exploded"), "the run's error surfaces: {err}");
        assert!(provisioned.load(Ordering::SeqCst), "the mount was materialized");
        assert!(
            !committed.load(Ordering::SeqCst),
            "a failed run commits NOTHING — no output_ref for a discarded attempt"
        );
        assert!(cleaned.load(Ordering::SeqCst), "cleanup still runs on failure (W5)");
    }

    #[tokio::test]
    async fn a_commit_mechanism_failure_degrades_the_receipt_never_fakes_a_clean_tree() {
        // THE silent-data-loss guard: a conflict/transport/rejection during the
        // workspace commit must surface as `commit_error` + a degraded status —
        // never as `no_changes: true` with an Ok status, which would report the
        // agent's lost writes as a clean working copy.
        let (providers, _probes) = slow_providers(Duration::from_millis(5), false);
        let (provisioned, committed, cleaned) = flags();
        let provisioner: SharedProvisioner = Arc::new(MockProvisioner {
            provisioned: provisioned.clone(),
            committed: committed.clone(),
            cleaned: cleaned.clone(),
            fail_commit: Some("commit conflict: head moved".into()),
        });
        let (pool, mut rx) = pool_with_provisioner(providers, provisioner);

        let eff = effect_with_payload("s1", 0, Some(b"me"), &v3_envelope_payload());
        pool.run(&eff).await.unwrap();
        let (_, _, outcome) = next_result(&mut rx).await;

        // the run's answer still delivers (R4) — wrapped, with the failure on
        // the receipt and the status degraded.
        let bytes = outcome.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["ducktape_runner_result"], 1);
        assert_eq!(v["response_text"], "answer to: GENERIC\n\nCONTRACT\n\nCONVERSATION");
        assert_eq!(v["status"], "degraded", "a failed capture degrades the run");
        assert_eq!(
            v["workspace_receipt"]["commit_error"],
            "commit conflict: head moved",
            "the receipt records the real failure for the audit lane (I4)"
        );
        assert_eq!(
            v["workspace_receipt"]["no_changes"], false,
            "a failed capture must NEVER masquerade as a clean tree"
        );
        assert!(v["workspace_receipt"]["output_snapshot"].is_null());
        assert!(cleaned.load(Ordering::SeqCst), "cleanup still runs (W5)");
    }

    /// a workspace whose commit never resolves — the hung-actor-lane probe
    /// for the #298 bracket timeout.
    struct HungCommitProvisioner {
        cleaned: Arc<AtomicBool>,
    }

    #[async_trait::async_trait]
    impl crate::provision::WorkspaceProvisioner for HungCommitProvisioner {
        async fn provision(
            &self,
            _spec: &WorkspaceSpec,
        ) -> Result<Box<dyn ProvisionedWorkspace>, String> {
            Ok(Box::new(HungCommitWs {
                cleaned: self.cleaned.clone(),
            }))
        }
    }

    struct HungCommitWs {
        cleaned: Arc<AtomicBool>,
    }

    #[async_trait::async_trait]
    impl ProvisionedWorkspace for HungCommitWs {
        fn workdir(&self) -> PathBuf {
            std::env::temp_dir()
        }
        fn env(&self) -> BTreeMap<String, String> {
            BTreeMap::new()
        }
        fn path_entries(&self) -> Vec<PathBuf> {
            Vec::new()
        }
        async fn commit(&self, _message: &str) -> Result<WorkspaceReceipt, String> {
            std::future::pending::<()>().await;
            unreachable!("a pending future never resolves")
        }
        async fn cleanup(&self) {
            self.cleaned.store(true, Ordering::SeqCst);
        }
    }

    #[tokio::test]
    async fn a_hung_commit_times_out_into_a_degraded_receipt_not_a_pinned_permit() {
        // the #298 bracket bound: commit blocks on the daemon actor lane, and
        // a stalled lane must not pin a pool permit until the saga deadline.
        // the step times out, the answer still delivers (R4), the receipt
        // records the timeout, cleanup runs.
        let (providers, _probes) = slow_providers(Duration::from_millis(5), false);
        let cleaned = Arc::new(AtomicBool::new(false));
        let provisioner: SharedProvisioner = Arc::new(HungCommitProvisioner {
            cleaned: cleaned.clone(),
        });
        let (pool, mut rx) = pool_with_provisioner(providers, provisioner);

        let eff = effect_with_payload("s1", 0, Some(b"me"), &v3_envelope_payload());
        pool.run(&eff).await.unwrap();
        let (_, _, outcome) = next_result(&mut rx).await;

        let bytes = outcome.expect("the run's answer still delivers (R4)");
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["status"], "degraded", "a timed-out capture degrades the run");
        assert!(
            v["workspace_receipt"]["commit_error"]
                .as_str()
                .is_some_and(|e| e.contains("timed out")),
            "the receipt records the timeout: {v}"
        );
        assert!(cleaned.load(Ordering::SeqCst), "cleanup still runs (W5)");
    }

    /// a provider that panics mid-run — the unwind probe for the cleanup
    /// guard (a leaked per-run dir was #298's second bracket finding).
    struct PanicProvider {
        tag: String,
    }

    #[async_trait::async_trait]
    impl capability_host::Provider for PanicProvider {
        fn capability(&self) -> &str {
            &self.tag
        }
        async fn run(
            &self,
            _prompt: &str,
            _ctx: &capability_host::RunContext,
        ) -> Result<String, String> {
            panic!("provider crashed hard")
        }
    }

    #[tokio::test]
    async fn a_panicking_provider_still_cleans_up_and_fails_the_attempt() {
        let providers = Arc::new(ProviderSet::assemble(
            capability_host::SpecSet::from_specs(vec![spec_toml("alpha")]),
            vec![Box::new(PanicProvider { tag: "alpha".into() })],
        ));
        let (provisioned, committed, cleaned) = flags();
        let provisioner: SharedProvisioner = Arc::new(MockProvisioner {
            provisioned: provisioned.clone(),
            committed: committed.clone(),
            cleaned: cleaned.clone(),
            fail_commit: None,
        });
        let (pool, mut rx) = pool_with_provisioner(providers, provisioner);

        let eff = effect_with_payload("s1", 0, Some(b"me"), &v3_envelope_payload());
        pool.run(&eff).await.unwrap();
        let (_, _, outcome) = next_result(&mut rx).await;

        let err = outcome.expect_err("a panic settles the attempt as a failure");
        assert!(
            err.contains("panicked") && err.contains("provider crashed hard"),
            "the panic payload surfaces in the saga error: {err}"
        );
        assert!(provisioned.load(Ordering::SeqCst), "the mount was materialized");
        assert!(
            !committed.load(Ordering::SeqCst),
            "a panicked run commits NOTHING"
        );
        assert!(
            cleaned.load(Ordering::SeqCst),
            "cleanup runs even past a panicking provider — no leaked per-run dir"
        );
    }

    #[tokio::test]
    async fn a_v3_run_without_a_provisioner_is_dormant_raw_text_no_wrapper() {
        let (providers, probes) = slow_providers(Duration::from_millis(5), false);
        let (pool, mut rx) = pool_with(providers, 4); // NO provisioner

        let eff = effect_with_payload("s1", 0, Some(b"me"), &v3_envelope_payload());
        pool.run(&eff).await.unwrap();
        let (_, _, outcome) = next_result(&mut rx).await;

        // no wrapper: the raw provider text is delivered verbatim.
        assert_eq!(
            outcome.unwrap(),
            b"answer to: GENERIC\n\nCONTRACT\n\nCONVERSATION".to_vec(),
            "an unwired provisioner keeps the accept-only behavior (dormant)"
        );
        let (_, ctx) = probes.last_run.lock().unwrap().clone().unwrap();
        assert!(
            ctx.workdir_override.is_none(),
            "no provisioner => no mount is bound, exactly the accept slice"
        );
        assert!(ctx.portable, "the run is still marked portable at accept");
    }

    #[tokio::test]
    async fn a_v2_run_is_byte_identical_with_or_without_a_provisioner() {
        // the legacy regression guard: every non-portable (v2) run must produce the
        // exact same outcome bytes whether or not a provisioner is wired.
        let expected = b"answer to: GENERIC\n\nCONTRACT\n\nCONVERSATION".to_vec();

        // without a provisioner.
        let (providers, _p) = slow_providers(Duration::from_millis(5), false);
        let (pool, mut rx) = pool_with(providers, 4);
        pool.run(&effect_with_payload("s1", 0, Some(b"me"), &envelope_payload(None)))
            .await
            .unwrap();
        assert_eq!(next_result(&mut rx).await.2.unwrap(), expected);

        // with a provisioner wired: a v2 run carries no plan, so the bracket
        // takes the unchanged branch and the provisioner is never touched.
        let (providers, _p) = slow_providers(Duration::from_millis(5), false);
        let (provisioned, committed, cleaned) = flags();
        let provisioner: SharedProvisioner = Arc::new(MockProvisioner {
            provisioned: provisioned.clone(),
            committed: committed.clone(),
            cleaned: cleaned.clone(),
            fail_commit: None,
        });
        let (pool, mut rx) = pool_with_provisioner(providers, provisioner);
        pool.run(&effect_with_payload("s1", 0, Some(b"me"), &envelope_payload(None)))
            .await
            .unwrap();
        assert_eq!(next_result(&mut rx).await.2.unwrap(), expected);
        assert!(!provisioned.load(Ordering::SeqCst), "a v2 run never provisions");
        assert!(!committed.load(Ordering::SeqCst));
        assert!(!cleaned.load(Ordering::SeqCst));
    }

    /// pin the assembled wire shape against `runs::RunnerResult` field-for-field
    /// (a mirror of the consumer's Deserialize). a rename in EITHER crate must
    /// fail THIS test, never production — the receipt round-trips through
    /// `runs::decode_run_result_v1`.
    #[test]
    fn assembled_runner_result_matches_the_runs_deserialize_contract() {
        // a mirror of runs' faceted Deserialize — a rename in EITHER crate must
        // fail THIS test. facet fields carry serde defaults so the minimal shape
        // still decodes (non-deny_unknown_fields, as runs keeps it).
        #[derive(serde::Deserialize)]
        #[allow(dead_code)]
        struct RunsRunnerResult {
            ducktape_runner_result: u32,
            response_text: String,
            workspace_receipt: RunsWorkspaceReceipt,
            #[serde(default)]
            data: Option<String>,
            #[serde(default)]
            effects: Vec<RunsEffect>,
            #[serde(default)]
            sink: RunsSink,
            #[serde(default)]
            status: RunsStatus,
        }
        #[derive(serde::Deserialize)]
        #[allow(dead_code)]
        struct RunsWorkspaceReceipt {
            source_prefix: String,
            source_snapshot: Option<String>,
            output_snapshot: Option<String>,
            commit_height: Option<u64>,
            rebased: bool,
            no_changes: bool,
            #[serde(default)]
            commit_error: Option<String>,
        }
        #[derive(serde::Deserialize)]
        #[allow(dead_code)]
        struct RunsEffect {
            kind: String,
            #[serde(default)]
            task_id: String,
            #[serde(default)]
            title: String,
            #[serde(default)]
            status: String,
        }
        #[derive(serde::Deserialize, Default, PartialEq, Debug)]
        #[serde(tag = "mode", rename_all = "snake_case")]
        enum RunsSink {
            #[default]
            Chain,
            Pr {
                repo: String,
                source_branch: String,
                target_branch: String,
                title: String,
                body: String,
            },
        }
        #[derive(serde::Deserialize, Default, PartialEq, Debug)]
        #[serde(rename_all = "snake_case")]
        enum RunsStatus {
            #[default]
            Ok,
            Degraded,
            Failed,
        }

        let receipt = WorkspaceReceipt {
            source_prefix: "/shared/agent-workspaces/bot".into(),
            source_snapshot: Some("aa".repeat(32)),
            output_snapshot: Some("cc".repeat(32)),
            commit_height: Some(9),
            rebased: true,
            no_changes: false,
            commit_error: None,
            branch: None,
            output_commit: None,
        };

        use crate::provision::{RunEffect, Sink, Status};

        // (1) the minimal shape (empty facets) still decodes and still yields
        //     response_text via the runs contract.
        let minimal = assemble_runner_result("the answer", &receipt, None, Vec::new(), Sink::Chain, Status::Ok);
        let parsed: RunsRunnerResult = serde_json::from_slice(&minimal)
            .expect("minimal bytes deserialize into the runs contract");
        assert_eq!(parsed.ducktape_runner_result, 1);
        assert_eq!(parsed.response_text, "the answer");
        assert_eq!(parsed.workspace_receipt.output_snapshot, Some("cc".repeat(32)));
        assert!(parsed.effects.is_empty());
        assert_eq!(parsed.sink, RunsSink::Chain);
        assert_eq!(parsed.status, RunsStatus::Ok);
        assert_eq!(parsed.data, None);

        // (2) a fully faceted receipt round-trips field-for-field.
        let full = assemble_runner_result(
            "prose",
            &receipt,
            Some("{\"k\":1}".into()),
            vec![RunEffect {
                kind: "tasks.create".into(),
                task_id: "t1".into(),
                title: "ship".into(),
                status: String::new(),
            }],
            Sink::Pr {
                repo: "app".into(),
                source_branch: "agent/run".into(),
                target_branch: "main".into(),
                title: "PR".into(),
                body: "body".into(),
            },
            Status::Degraded,
        );
        let parsed: RunsRunnerResult = serde_json::from_slice(&full)
            .expect("faceted bytes deserialize into the runs contract");
        assert_eq!(parsed.data.as_deref(), Some("{\"k\":1}"));
        assert_eq!(parsed.effects.len(), 1);
        assert_eq!(parsed.effects[0].kind, "tasks.create");
        assert_eq!(parsed.effects[0].task_id, "t1");
        assert_eq!(parsed.status, RunsStatus::Degraded);
        assert_eq!(
            parsed.sink,
            RunsSink::Pr {
                repo: "app".into(),
                source_branch: "agent/run".into(),
                target_branch: "main".into(),
                title: "PR".into(),
                body: "body".into(),
            }
        );

        // (3) the REQUESTED-sink echo (contract §3): a Pr sink whose
        //     title/body are empty must still serialize them as PRESENT keys
        //     (runs keeps title REQUIRED on decode — the mirror has no serde
        //     default on it), and a forge receipt carrying the additive §5
        //     fields must still decode into runs' CURRENT mirror (unknown
        //     fields tolerated) — the two halves of the flag-day interop.
        let forge_receipt = WorkspaceReceipt {
            source_prefix: "forge:app".into(),
            source_snapshot: Some("d0".repeat(20)),
            output_snapshot: None,
            commit_height: None,
            rebased: false,
            no_changes: false,
            commit_error: None,
            branch: Some("agent/item-7".into()),
            output_commit: Some("e1".repeat(20)),
        };
        let echoed = assemble_runner_result(
            "prose",
            &forge_receipt,
            None,
            Vec::new(),
            Sink::Pr {
                repo: "app".into(),
                source_branch: "agent/item-7".into(),
                target_branch: "main".into(),
                title: String::new(),
                body: String::new(),
            },
            Status::Ok,
        );
        let raw = String::from_utf8(echoed.clone()).unwrap();
        assert!(
            raw.contains(r#""title":"""#) && raw.contains(r#""body":"""#),
            "empty title/body stay PRESENT keys on the wire: {raw}"
        );
        let parsed: RunsRunnerResult = serde_json::from_slice(&echoed)
            .expect("a requested-sink echo deserializes into the runs contract");
        assert_eq!(
            parsed.sink,
            RunsSink::Pr {
                repo: "app".into(),
                source_branch: "agent/item-7".into(),
                target_branch: "main".into(),
                title: String::new(),
                body: String::new(),
            }
        );
        assert_eq!(parsed.workspace_receipt.source_prefix, "forge:app");
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
