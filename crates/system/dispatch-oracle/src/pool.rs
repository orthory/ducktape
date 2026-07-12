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

use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use capability_host::ProviderSet;
use futures::future::{BoxFuture, Either, select};
use host::worker::{WorkOutcome, Worker};
use sdk::{Effect, Msg};
use tokio::sync::Semaphore;

use crate::provision::{SharedProvisioner, WorkspaceSpec, assemble_runner_result, bind_workspace};
use crate::{
    AttemptOutput, ExecJob, Gated, ResourceLedger, attempt_output, clean_error, gate,
    oracle_result_with_usage, renew_lease,
};

/// how many provider runs may execute concurrently unless
/// `DUCKTAPE_MAX_CONCURRENT_RUNS` says otherwise.
pub const DEFAULT_MAX_CONCURRENT_RUNS: usize = 4;

fn lease_renew_interval() -> Duration {
    if cfg!(test) {
        Duration::from_millis(25)
    } else {
        Duration::from_secs(10)
    }
}

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
    /// materializes/commits/cleans a per-run workspace — injected by the node
    /// binary, where duckfs-client + the actor lane are reachable. `None`
    /// (the default) keeps the accept-only degrade: the plan is surfaced but
    /// never activated, so the run executes raw-text with no workspace. both
    /// node binaries wire a real provisioner, so this is LIVE on every
    /// production agent run.
    provisioner: Option<SharedProvisioner>,
    /// the host-local load ledger: `gate()` checks `fits()` inline on both
    /// the own-lease and announcement paths; `spawn_exec` reserves a run's
    /// demands before the semaphore acquire, releasing via RAII guard held
    /// to the spawned task's end. `Arc`-wrapped so the spawned task can hold
    /// its own cheap clone alongside `providers`.
    ledger: Arc<ResourceLedger>,
}

impl DispatchPool {
    /// the production constructor: concurrency cap from the environment, no
    /// announced capacity (a bare ledger — demandless jobs only) until a
    /// later task wires the operator's real numbers in.
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
            Default::default(),
        )
    }

    /// an explicit concurrency cap (tests; embedders with their own policy)
    /// and announced resource capacity. an empty `capacity` is the direct
    /// (bare) node: only demandless jobs ever fit its ledger.
    pub fn with_limit(
        providers: Arc<ProviderSet>,
        node_key: Vec<u8>,
        spawn: SpawnFn,
        deliver: DeliverFn,
        limit: usize,
        capacity: BTreeMap<String, u64>,
    ) -> Self {
        Self {
            providers,
            node_key,
            spawn,
            deliver,
            semaphore: Arc::new(Semaphore::new(limit.max(1))),
            inflight: Arc::new(Mutex::new(HashSet::new())),
            provisioner: None,
            ledger: Arc::new(ResourceLedger::new(capacity)),
        }
    }

    /// wire the host-side workspace provisioner runs materialize through — a
    /// builder (not a constructor arm) so embedders and tests keep compiling.
    /// without it the pool takes the dormant branch: the provider's raw text is
    /// delivered verbatim, no workspace exists — and, since the soul is
    /// assembled from MATERIALIZED skill mounts, no context document either.
    /// the production binaries always wire one.
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
        let provisioner = self.provisioner.clone();
        let ledger = self.ledger.clone();
        (self.spawn)(Box::pin(async move {
            // reserve BEFORE the semaphore acquire: capacity was already
            // promised at gate time, so a run queued behind the cap still
            // holds its share while it waits. held to the end of THIS task
            // (the whole async block) via the guard's Drop — every exit path
            // (ok, error, panic-unwind) releases. demandless jobs (empty
            // `job.demands`) reserve nothing: `reserve` is a no-op on the
            // legacy path.
            let reservation_key = format!("{}:{}", job.saga_id, job.attempt);
            let _reservation = ledger.reserve(&reservation_key, &job.demands);
            let run = async {
                // over-cap runs queue HERE, on their own task. the heartbeat
                // below already runs while they wait, so a healthy local
                // queue does not lose its lease before execution starts.
                let _permit = semaphore
                    .acquire_owned()
                    .await
                    .expect("run semaphore is never closed");
                // re-resolve by tag inside the task: the gate already proved
                // the capability resolves, and a provider surface is
                // immutable for the process lifetime — this is just how the
                // borrow crosses.
                match providers.resolve(&job.capability) {
                    // the envelope step runs on the spawned task too. its errors
                    // are the run's result: a saga Err, never a silent fallback.
                    Ok(provider) => {
                        match crate::envelope::prepare(&job.input).await {
                            Ok(mut prepared) => {
                                // the live-output registry key: the dispatch_id half of
                                // the saga id, exactly what the app's PendingRun rows
                                // expose — set before execute so BOTH the dormant and
                                // provisioned paths carry it into provider.run.
                                prepared.ctx.run_key = Some(run_key_for(&job.saga_id));
                                // the run's numeric demands ride into RunContext so a
                                // Podman-backed provider can enforce them as container
                                // limits; a Direct backend ignores them.
                                prepared.ctx.limits = job.demands.clone();
                                execute(&job, prepared, provider, provisioner.as_ref())
                                    .await
                                    .map_err(clean_error)
                            }
                            Err(e) => Err(clean_error(e)),
                        }
                    }
                    Err(e) => Err(clean_error(e)),
                }
            };
            let heartbeat_deliver = deliver.clone();
            let heartbeat_saga = job.saga_id.clone();
            let heartbeat_attempt = job.attempt;
            let heartbeat = async move {
                loop {
                    tokio::time::sleep(lease_renew_interval()).await;
                    heartbeat_deliver(renew_lease(&heartbeat_saga, heartbeat_attempt)).await;
                }
            };
            futures::pin_mut!(run, heartbeat);
            let outcome = match select(run, heartbeat).await {
                Either::Left((outcome, _)) => outcome,
                Either::Right(_) => unreachable!("lease heartbeat loop never completes"),
            };
            // error/timeout results are submitted like any other: the saga
            // must see the failure to complete or retry the attempt.
            let (outcome, usage) = match outcome {
                Ok(output) => (Ok(output.bytes), output.usage),
                Err(error) => (Err(error), None),
            };
            deliver(oracle_result_with_usage(
                &job.saga_id,
                job.attempt,
                outcome,
                usage,
            ))
            .await;
            // prune AFTER delivery: a redelivery racing the result's submit
            // is still a skip, not a second child.
            inflight.lock().expect("inflight lock").remove(&key);
        }));
    }
}

/// provision → bind → run → commit → assemble → cleanup, at the dispatch
/// boundary on the spawned task.
///
/// DORMANT without a wired provisioner: the run is plain
/// `provider.run(&input, &ctx).await` with the raw text as the delivered
/// bytes. on the provisioned path the winning attempt's bytes are the
/// host-assembled `RunnerResult` (prose + receipt).
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
) -> Result<AttemptOutput, String> {
    let crate::envelope::Prepared {
        input,
        mut ctx,
        workspace: plan,
    } = prepared;
    let Some(prov) = provisioner else {
        // accept-only / legacy: unchanged behavior, raw text bytes.
        let output = provider.run_with_usage(&input, &ctx).await?;
        let bytes = output.text.as_bytes().to_vec();
        return Ok(attempt_output(output, bytes));
    };
    // the REQUESTED sink (Chain when the envelope carried none) — echoed on
    // the assembled RunnerResult below so runs' delivery can route it.
    let sink = plan.sink;
    let spec = WorkspaceSpec {
        run_id: format!("{}:{}", job.saga_id, job.attempt),
        // the CONSENSUS id, straight from the envelope — the only id that
        // resolves the run back in `runs`. it is deliberately NOT derived from
        // the saga id above: that one exists to key the on-disk workspace dir.
        consensus_run_id: plan.consensus_run_id,
        agent_id: ctx.agent_id.clone(),
        agent_display_name: Some(plan.agent_display_name),
        // the tagged source (duckfs subtree or forge repo@commit) crosses to
        // the provisioner verbatim — the pool never interprets it.
        source: plan.source,
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
        match provider.run_with_usage(&input, &ctx).await {
            Ok(output) => {
                // (d) capture output_ref. a commit-MECHANISM failure (conflict,
                // transport, rejection, a hung actor lane) must never
                // masquerade as a clean tree: the receipt records the error
                // and the status degrades, while the run's answer still
                // delivers (R4 — never lost to receipt plumbing). only
                // `CommitError::Nothing` is a true `no_changes`, and the
                // workspace impl already maps that to Ok.
                let commit = tokio::time::timeout(
                    workspace_step_timeout(),
                    // DuckFS persists this as snapshot audit metadata. Forge
                    // deliberately ignores it and reads the agent-authored
                    // proposal from the workspace instead.
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
                let effects = crate::provision::effects_from_response_text(&output.text);
                let bytes =
                    assemble_runner_result(&output.text, &receipt, None, effects, sink, status);
                Ok(attempt_output(output, bytes))
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
    async fn run(&self, effect: &Effect) -> Result<WorkOutcome, host::worker::Error> {
        match gate(&self.providers, &self.node_key, &self.ledger, effect) {
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
        usage: Option<capability_host::TokenUsage>,
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

        async fn run_with_usage(
            &self,
            prompt: &str,
            ctx: &capability_host::RunContext,
        ) -> Result<capability_host::ProviderOutput, String> {
            self.run(prompt, ctx)
                .await
                .map(|text| capability_host::ProviderOutput {
                    text,
                    usage: self.usage,
                })
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
            usage: None,
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
            DispatchPool::with_limit(
                providers,
                b"me".to_vec(),
                spawn,
                deliver,
                limit,
                Default::default(),
            ),
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
                demands: Default::default(),
            }),
            deadline: None,
            assignee: assignee.map(|a| a.to_vec()),
        }))
    }

    fn effect_for(saga_id: &str, attempt: u32, assignee: Option<&[u8]>) -> Effect {
        effect_with_payload(saga_id, attempt, assignee, &envelope_payload())
    }

    /// what an unprovisioned pool delivers for [`envelope_payload`]: the raw
    /// provider text over the assembled input.
    const PLAIN_ANSWER: &[u8] = b"answer to: GENERIC\n\nCONTRACT\n\nCONVERSATION";

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
        loop {
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
                    ..
                } => return (saga_id, attempt, outcome),
                SagaMsg::RenewLease { .. } => continue,
                other => panic!("expected an OracleResult, got {other:?}"),
            }
        }
    }

    #[tokio::test]
    async fn a_slow_run_renews_its_lease() {
        let (providers, _) = slow_providers(Duration::from_millis(100), false);
        let (pool, mut rx) = pool_with(providers, 1);
        pool.run(&effect_for("s1", 3, Some(b"me"))).await.unwrap();
        let msg = tokio::time::timeout(Duration::from_secs(1), rx.next())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            saga::decode_msg(&msg.payload).unwrap(),
            SagaMsg::RenewLease { saga_id, attempt: 3 } if saga_id == "s1"
        ));
    }

    #[tokio::test]
    async fn provider_usage_rides_the_oracle_result() {
        let executions = Arc::new(AtomicUsize::new(0));
        let provider = SlowProvider {
            tag: "alpha".into(),
            delay: Duration::ZERO,
            executions: executions.clone(),
            current: Arc::new(AtomicUsize::new(0)),
            peak: Arc::new(AtomicUsize::new(0)),
            last_run: Arc::new(Mutex::new(None)),
            fail: false,
            usage: Some(capability_host::TokenUsage {
                input_tokens: 100,
                cached_input_tokens: 60,
                cache_write_input_tokens: 0,
                output_tokens: 20,
                reasoning_output_tokens: 7,
            }),
        };
        let providers = Arc::new(ProviderSet::assemble(
            capability_host::SpecSet::from_specs(vec![spec_toml("alpha")]),
            vec![Box::new(provider)],
        ));
        let (pool, mut rx) = pool_with(providers, 1);
        pool.run(&effect_for("s1", 0, Some(b"me"))).await.unwrap();
        let msg = rx.next().await.unwrap();
        let SagaMsg::OracleResult { usage, .. } = saga::decode_msg(&msg.payload).unwrap() else {
            panic!("expected OracleResult");
        };
        assert_eq!(usage.unwrap().input_tokens, 100);
        assert_eq!(executions.load(Ordering::SeqCst), 1);
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
        assert_eq!(outcome.unwrap(), PLAIN_ANSWER.to_vec());
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

    /// a v3 duckfs run envelope — the shape the composer emits for every run.
    fn envelope_payload() -> Vec<u8> {
        serde_json::json!({
            "ducktape_run": 3,
            "agent_id": "bot",
            "agent_display_name": "BOT",
            "thread_key": "general#7",
            "instructions": "GENERIC",
            "contract": "CONTRACT",
            "conversation": "CONVERSATION",
            "workspace": {
                "kind": "duckfs",
                "source_prefix": "/shared/agent-workspaces/bot",
                "source_snapshot": "aa".repeat(32)
            },
            "skills": [
                {"name":"persona","source_prefix":"/shared/skills/persona","always": true},
                {"name":"release","source_prefix":"/shared/skills/release","source_snapshot": "bb".repeat(32)}
            ],
            "result_contract": {"ducktape_runner_result": 1}
        })
        .to_string()
        .into_bytes()
    }

    #[tokio::test]
    async fn envelope_payloads_reach_the_provider_assembled_with_run_context() {
        let (providers, probes) = slow_providers(Duration::from_millis(5), false);
        let (pool, mut rx) = pool_with(providers, 4);

        let eff = effect_with_payload("s1", 0, Some(b"me"), &envelope_payload());
        pool.run(&eff).await.unwrap();
        let (_, _, outcome) = next_result(&mut rx).await;
        assert_eq!(
            outcome.unwrap(),
            b"answer to: GENERIC\n\nCONTRACT\n\nCONVERSATION".to_vec(),
            "assembly order: prompt-or-instructions, contract, conversation"
        );
        let (input, ctx) = probes.last_run.lock().unwrap().clone().unwrap();
        assert!(
            !input.contains("ducktape_run"),
            "the provider never sees envelope JSON"
        );
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
        assert_eq!(outcome.unwrap(), PLAIN_ANSWER.to_vec());
        let (_, ctx) = probes.last_run.lock().unwrap().clone().unwrap();
        assert_eq!(ctx.run_key.as_deref(), Some("d1"));
    }

    // ---- the portable (v3) provisioning bracket -----------------------------

    /// the same v3 duckfs envelope, named for the provisioning-bracket tests.
    fn v3_envelope_payload() -> Vec<u8> {
        envelope_payload()
    }

    /// a forge-sourced v3 run envelope — the EXACT byte shapes task 1's
    /// composer emits (task-1 report §"Exact final serde shapes"): tagged
    /// forge workspace, `context`, requested-Pr sink WITHOUT title/body keys.
    fn forge_envelope_payload() -> Vec<u8> {
        serde_json::json!({
            "ducktape_run": 3,
            "agent_id": "bot",
            "agent_display_name": "BOT",
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
            let dir =
                std::env::temp_dir().join(format!("mock-ws-{}", spec.run_id.replace(':', "_")));
            let mut env = BTreeMap::new();
            env.insert("DUCKTAPE_RUN_WORKSPACE".into(), dir.display().to_string());
            // kind-agnostic: echo whatever source the spec carries, exactly
            // like a real receipt would (duckfs prefix/pin or forge:<repo>).
            let (src, snap) = spec.source.receipt_coords();
            Ok(Box::new(MockWs {
                dir,
                context_doc: Some("# persona\nYou are Bot.".into()),
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
        /// the assembled soul the real provisioner hands back — the pool must
        /// carry it onto the RunContext, or the agent runs without its persona.
        context_doc: Option<String>,
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
        fn context_doc(&self) -> Option<String> {
            self.context_doc.clone()
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
            DispatchPool::with_limit(
                providers,
                b"me".to_vec(),
                spawn,
                deliver,
                4,
                Default::default(),
            )
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
        assert_eq!(
            v["response_text"],
            "answer to: GENERIC\n\nCONTRACT\n\nCONVERSATION"
        );
        assert_eq!(v["workspace_receipt"]["output_snapshot"], "cc".repeat(32));
        assert_eq!(v["workspace_receipt"]["commit_height"], 9);
        assert_eq!(
            v["workspace_receipt"]["source_prefix"],
            "/shared/agent-workspaces/bot"
        );
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
        // the SOUL crosses provisioner → RunContext. it is assembled from the
        // MATERIALIZED skill mounts (only the provisioner can read them), so
        // this hop is the only way the persona ever reaches the model —
        // capability-host then picks the door (the CLI's auto-load file, or a
        // prepend to the stdin prompt).
        assert_eq!(
            ctx.context_doc.as_deref(),
            Some("# persona\nYou are Bot."),
            "the provisioned context doc must ride into the run"
        );
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

        // both composed skills become ro mounts, IN CURATION ORDER, each
        // carrying its load mode: the provisioner assembles the run's soul from
        // exactly this — inline the `always` bodies, index the rest — so an
        // order or a mode dropped here is a different agent.
        let mounts = captured.lock().unwrap().clone();
        assert_eq!(mounts.len(), 2, "both composed skills became ro mounts");
        assert_eq!(mounts[0].mount_subpath, "persona");
        assert!(mounts[0].always, "the persona skill inlines");
        assert_eq!(mounts[1].mount_subpath, "release");
        assert_eq!(mounts[1].source_prefix, "/shared/skills/release");
        assert!(!mounts[1].always, "an unmarked skill is on-demand");
        assert_eq!(
            mounts[1].source_snapshot.as_deref(),
            Some("bb".repeat(32).as_str())
        );
    }

    /// a probe provisioner that captures the WHOLE [`WorkspaceSpec`] — how the
    /// forge tests observe that the tagged source crossed envelope →
    /// PortablePlan → WorkspaceSpec intact.
    struct SpecProbe {
        captured: Arc<Mutex<Option<WorkspaceSpec>>>,
    }

    struct CommitMessageProbe {
        captured: Arc<Mutex<Option<String>>>,
    }

    #[async_trait::async_trait]
    impl crate::provision::WorkspaceProvisioner for CommitMessageProbe {
        async fn provision(
            &self,
            _spec: &WorkspaceSpec,
        ) -> Result<Box<dyn ProvisionedWorkspace>, String> {
            Ok(Box::new(CommitMessageWs {
                captured: self.captured.clone(),
            }))
        }
    }

    struct CommitMessageWs {
        captured: Arc<Mutex<Option<String>>>,
    }

    #[async_trait::async_trait]
    impl ProvisionedWorkspace for CommitMessageWs {
        fn workdir(&self) -> PathBuf {
            std::env::temp_dir().join("commit-message-probe")
        }
        fn env(&self) -> BTreeMap<String, String> {
            BTreeMap::new()
        }
        fn path_entries(&self) -> Vec<PathBuf> {
            Vec::new()
        }
        async fn commit(&self, message: &str) -> Result<WorkspaceReceipt, String> {
            *self.captured.lock().unwrap() = Some(message.to_string());
            Ok(WorkspaceReceipt {
                source_prefix: "forge:app".into(),
                source_snapshot: Some("d0".repeat(20)),
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
        assert_eq!(spec.agent_id.as_deref(), Some("bot"));
        assert_eq!(spec.agent_display_name.as_deref(), Some("BOT"));
        assert_eq!(
            spec.source,
            crate::workspace_source::WorkspaceSource::Forge {
                repo: "app".into(),
                commit: "d0".repeat(20),
                branch: "agent/item-7".into(),
                branch_born: false,
            }
        );
    }

    #[tokio::test]
    async fn portable_workspace_commit_preserves_the_run_audit_message() {
        let (providers, _probes) = slow_providers(Duration::from_millis(5), false);
        let captured = Arc::new(Mutex::new(None));
        let provisioner: SharedProvisioner = Arc::new(CommitMessageProbe {
            captured: captured.clone(),
        });
        let (pool, mut rx) = pool_with_provisioner(providers, provisioner);
        let saga_id = "dispatch\u{1f}runs\u{1f}private-hash";

        let eff = effect_with_payload(saga_id, 7, Some(b"me"), &v3_envelope_payload());
        pool.run(&eff).await.unwrap();
        let _ = next_result(&mut rx).await;

        let message = captured.lock().unwrap().clone().expect("commit called");
        assert_eq!(message, format!("agent run {saga_id}:7"));
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
        assert!(
            err.contains("provider exploded"),
            "the run's error surfaces: {err}"
        );
        assert!(
            provisioned.load(Ordering::SeqCst),
            "the mount was materialized"
        );
        assert!(
            !committed.load(Ordering::SeqCst),
            "a failed run commits NOTHING — no output_ref for a discarded attempt"
        );
        assert!(
            cleaned.load(Ordering::SeqCst),
            "cleanup still runs on failure (W5)"
        );
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
        assert_eq!(
            v["response_text"],
            "answer to: GENERIC\n\nCONTRACT\n\nCONVERSATION"
        );
        assert_eq!(v["status"], "degraded", "a failed capture degrades the run");
        assert_eq!(
            v["workspace_receipt"]["commit_error"], "commit conflict: head moved",
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
        assert_eq!(
            v["status"], "degraded",
            "a timed-out capture degrades the run"
        );
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
            vec![Box::new(PanicProvider {
                tag: "alpha".into(),
            })],
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
        assert!(
            provisioned.load(Ordering::SeqCst),
            "the mount was materialized"
        );
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
    async fn flat_and_v2_payloads_fail_the_saga_loudly() {
        // FLAG DAY: the flat-string passthrough and the v2 tolerance are gone
        // — both deliver a loud Err result (the saga settles a failed
        // attempt), and the provider is never invoked.
        let (providers, probes) = slow_providers(Duration::from_millis(5), false);
        let (pool, mut rx) = pool_with(providers, 4);

        pool.run(&effect_with_payload(
            "s1",
            0,
            Some(b"me"),
            b"the entire input",
        ))
        .await
        .unwrap();
        let (_, _, outcome) = next_result(&mut rx).await;
        let err = outcome.unwrap_err();
        assert!(err.contains("no ducktape_run envelope marker"), "got {err}");

        let mut v2: serde_json::Value =
            serde_json::from_slice(&envelope_payload()).unwrap();
        v2["ducktape_run"] = serde_json::json!(2);
        pool.run(&effect_with_payload(
            "s2",
            0,
            Some(b"me"),
            v2.to_string().as_bytes(),
        ))
        .await
        .unwrap();
        let (_, _, outcome) = next_result(&mut rx).await;
        let err = outcome.unwrap_err();
        assert!(err.contains("version 2"), "got {err}");
        assert_eq!(
            probes.executions.load(Ordering::SeqCst),
            0,
            "the provider is never invoked on a rejected payload"
        );
    }

    #[tokio::test]
    async fn an_oversized_prose_result_still_fits_the_saga_cap() {
        // THE wedge fix: the host-assembled RunnerResult is the saga Ok
        // payload, and the saga aborts any Ok over saga::MAX_RESULT_BYTES —
        // an uncapped assembly could then never land and the run would wedge
        // until deadline. an oversized prose answer must deliver TRUNCATED
        // (receipt intact) instead.
        let providers = Arc::new(ProviderSet::assemble(
            capability_host::SpecSet::from_specs(vec![spec_toml("alpha")]),
            vec![Box::new(HugeProvider { tag: "alpha".into() })],
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

        let bytes = outcome.expect("the oversized run still delivers Ok");
        assert!(
            bytes.len() <= saga::MAX_RESULT_BYTES,
            "the delivered result must fit the saga cap ({} > {})",
            bytes.len(),
            saga::MAX_RESULT_BYTES
        );
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["ducktape_runner_result"], 1);
        let text = v["response_text"].as_str().unwrap();
        assert!(
            text.ends_with("bytes)]") && text.contains("[output truncated ("),
            "the truncated prose names its original size: …{}",
            &text[text.len().saturating_sub(60)..]
        );
        // the receipt survives truncation — the artifact facet is never lost.
        assert_eq!(v["workspace_receipt"]["output_snapshot"], "cc".repeat(32));
        assert!(cleaned.load(Ordering::SeqCst));
    }

    /// a provider whose answer alone exceeds the saga result cap.
    struct HugeProvider {
        tag: String,
    }

    #[async_trait::async_trait]
    impl capability_host::Provider for HugeProvider {
        fn capability(&self) -> &str {
            &self.tag
        }
        async fn run(
            &self,
            _prompt: &str,
            _ctx: &capability_host::RunContext,
        ) -> Result<String, String> {
            Ok("x".repeat(saga::MAX_RESULT_BYTES + 64 * 1024))
        }
    }

    /// pin the assembled wire shape against `runs::RunnerResult` field-for-field
    /// (a mirror of the consumer's Deserialize). a rename in EITHER crate must
    /// fail THIS test, never production — the receipt round-trips through
    /// `runs::decode_run_result_v1`.
    #[test]
    fn assembled_runner_result_matches_the_runs_deserialize_contract() {
        // a mirror of runs' faceted Deserialize — a rename in EITHER crate must
        // fail THIS test. deny_unknown_fields mirrors runs: an assembled key
        // runs does not know is drift and must fail HERE, not in delivery.
        // facet fields carry serde defaults so the minimal shape still decodes.
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
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
        struct RunsWorkspaceReceipt {
            source_prefix: String,
            output_snapshot: Option<String>,
            commit_height: Option<u64>,
            rebased: bool,
            no_changes: bool,
            #[serde(default)]
            commit_error: Option<String>,
        }
        #[derive(serde::Deserialize)]
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
        let minimal = assemble_runner_result(
            "the answer",
            &receipt,
            None,
            Vec::new(),
            Sink::Chain,
            Status::Ok,
        );
        let parsed: RunsRunnerResult = serde_json::from_slice(&minimal)
            .expect("minimal bytes deserialize into the runs contract");
        assert_eq!(parsed.ducktape_runner_result, 1);
        assert_eq!(parsed.response_text, "the answer");
        assert_eq!(
            parsed.workspace_receipt.output_snapshot,
            Some("cc".repeat(32))
        );
        assert_eq!(
            parsed.workspace_receipt.source_prefix,
            "/shared/agent-workspaces/bot"
        );
        assert_eq!(parsed.workspace_receipt.commit_height, Some(9));
        assert!(parsed.workspace_receipt.rebased);
        assert!(!parsed.workspace_receipt.no_changes);
        assert_eq!(parsed.workspace_receipt.commit_error, None);
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
                ..RunEffect::default()
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
        assert_eq!(parsed.effects[0].title, "ship");
        assert_eq!(parsed.effects[0].status, "");
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
            WorkOutcome::Handled(Some(msg)) => match saga::decode_msg(&msg.payload).unwrap() {
                SagaMsg::Accept { saga_id, attempt } => {
                    assert_eq!((saga_id.as_str(), attempt), ("s1", 0));
                }
                other => panic!("expected an Accept claim, got {other:?}"),
            },
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
            Default::default(),
        );
        // a zero cap would deadlock every run; the pool clamps to 1.
        assert_eq!(pool.semaphore.available_permits(), 1);
    }
}
