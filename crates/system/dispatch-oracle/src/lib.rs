//! host-side worker for dispatch saga effects.
//!
//! the dispatch module stages a saga whose spec is a self-described
//! [`WorkSpec`]. this crate is the impure host-side counterpart: it resolves
//! the spec's capability tag to a machine-local [`capability_host::Provider`]
//! (whichever executor CLI the operator brought), feeds the payload to it,
//! and submits the raw answer as a saga `OracleResult` op.
//!
//! the payload comes in two shapes (see [`envelope`]): a legacy flat string
//! is fed to the provider VERBATIM, and a run ENVELOPE (marker
//! `ducktape_run`) is assembled host-side — the agent's registered prompt
//! resolved from the node's blob store by its committed content hash, plus
//! the contract and conversation the dispatcher composed. beyond that
//! assembly the crate stays opinion-free: NO prompt text is authored here,
//! NO output shape is parsed here (the dispatch module judges the recipe's
//! output contract in consensus), and NO credentials are touched (BYO CLI
//! auth). foreign leases are skipped: under the strict policy someone else's
//! assignment would be a no-op result, so not spawning is what turns
//! N-nodes-each-paying-for-the-same-call into one call.
//!
//! two workers share ONE gate ([`gate`], so their verdicts can never drift):
//!
//! - [`DispatchWorker`] awaits the provider INLINE — the simple embedding for
//!   tests and in-process reactors.
//! - [`DispatchPool`] (see [`pool`]) hands execution to a spawned background
//!   task and returns immediately — what real hosts run, so a minutes-long
//!   CLI call never stalls the host's event loop.

use capability_host::ProviderSet;
use dispatch::{WorkSpec, decode_work_spec};
use reactor::{WorkOutcome, Worker};
use saga::{SagaMsg, WorkerRequest, decode_worker_request, encode_msg};
use sdk::{Effect, Msg};

mod envelope;
mod pool;
mod provision;
pub use envelope::{BlobResolver, Prepared, RUN_ENVELOPE_VERSION, RUNNER_RESULT_VERSION};
pub use pool::{
    DEFAULT_MAX_CONCURRENT_RUNS, DeliverFn, DispatchPool, SpawnFn, max_concurrent_runs_from_env,
};
pub use provision::{
    BaseTool, PortablePlan, ProvisionedWorkspace, RoMount, RunEffect, SharedProvisioner, Sink, Status,
    WorkspaceProvisioner, WorkspaceReceipt, WorkspaceSpec, assemble_runner_result, bind_workspace,
    effects_from_response_text,
};

/// everything a provider execution needs, extracted by the gate so the
/// expensive run can happen away from the effect-offer callsite (on a spawned
/// task, or inline — the gate does not care).
pub struct ExecJob {
    pub saga_id: String,
    pub attempt: u32,
    pub capability: String,
    /// the fully rendered prompt — the WorkSpec payload, verbatim.
    pub input: String,
}

/// the fast, deterministic half of the worker step: decode routing plus the
/// lease gate. everything here is cheap; only [`Gated::Execute`] carries work
/// that costs real time.
pub enum Gated {
    /// not a dispatch `WorkSpec` effect — offer it to the next worker.
    NotMine,
    /// ours, deliberately not run (foreign lease, or an announcement this
    /// host cannot serve): a claimed skip.
    Skip,
    /// ours, answered without touching a provider: an `Accept` claim for a
    /// servable announcement, or the error `OracleResult` for a lease that
    /// can never execute (non-utf-8 payload, unresolvable capability).
    Immediate(Msg),
    /// our lease, executable: run the provider and submit the result.
    Execute(ExecJob),
}

/// decode + lease-gate one effect against this host's provider surface.
pub fn gate(providers: &ProviderSet, node_key: &[u8], effect: &Effect) -> Gated {
    let request = match decode_worker_request(&effect.0) {
        Ok(request) => request,
        Err(_) => return Gated::NotMine,
    };
    // the kind-gated decode: foreign spec shapes are NotMine, never a
    // guessed execution.
    let work = match decode_work_spec(&request.spec) {
        Ok(work) => work,
        Err(_) => return Gated::NotMine,
    };
    match &request.assignee {
        // the lease gate, host side: someone else's assignment is a
        // claimed skip — it IS our effect type, but the assignee submits
        // the result.
        Some(assignee) if *assignee != node_key => Gated::Skip,
        Some(_) => gate_own_lease(providers, &request, work),
        // an UNASSIGNED request is an announcement, not a work order:
        // running it would be one execution per capable node. claim it
        // with Accept when this host can actually run the capability;
        // the re-emitted request naming the winner is what executes.
        None => {
            if providers.resolve(&work.capability).is_err() {
                return Gated::Skip;
            }
            Gated::Immediate(accept_op(&request))
        }
    }
}

/// gate an own-lease request down to an [`ExecJob`] — or the inline error
/// result for a lease that can never execute.
fn gate_own_lease(providers: &ProviderSet, request: &WorkerRequest, work: WorkSpec) -> Gated {
    // payload shape first: this verdict must not depend on what happens
    // to be installed on this host.
    let input = match String::from_utf8(work.payload) {
        Ok(input) => input,
        Err(_) => {
            return Gated::Immediate(oracle_result(
                &request.saga_id,
                request.attempt,
                Err(clean_error(
                    "dispatch payload is not utf-8; providers take text".to_string(),
                )),
            ));
        }
    };
    if let Err(e) = providers.resolve(&work.capability) {
        return Gated::Immediate(oracle_result(
            &request.saga_id,
            request.attempt,
            Err(clean_error(e)),
        ));
    }
    Gated::Execute(ExecJob {
        saga_id: request.saga_id.clone(),
        attempt: request.attempt,
        capability: work.capability,
        input,
    })
}

/// Inline worker for dispatch `WorkSpec` saga effects: gate, then await the
/// provider on the caller's own task. real hosts run [`DispatchPool`]
/// instead — an inline await stalls the host loop for the provider's whole
/// runtime.
pub struct DispatchWorker {
    providers: ProviderSet,
    /// this node's external submit key — compared against a request's
    /// `assignee` to decide whether the lease is ours to execute.
    node_key: Vec<u8>,
    /// blob reads for envelope prompt resolution; `None` (the default) fails
    /// prompt-pinned envelopes loudly — see [`envelope::prepare`].
    resolver: Option<BlobResolver>,
}

impl DispatchWorker {
    pub fn new(providers: ProviderSet, node_key: Vec<u8>) -> Self {
        Self {
            providers,
            node_key,
            resolver: None,
        }
    }

    pub fn with_resolver(mut self, resolver: BlobResolver) -> Self {
        self.resolver = Some(resolver);
        self
    }

    async fn answer(&self, capability: &str, payload: &[u8]) -> Result<Vec<u8>, String> {
        // payload shape first: this verdict must not depend on what happens
        // to be installed on this host.
        let input = String::from_utf8(payload.to_vec())
            .map_err(|_| "dispatch payload is not utf-8; providers take text".to_string())?;
        let provider = self.providers.resolve(capability)?;
        // envelope payloads assemble (real prompt, run context); legacy flat
        // strings pass through verbatim with a default context. the inline
        // worker has no provisioner seam, so a v3 plan (if any) is ignored —
        // it never materializes a portable mount; the real portable path is
        // DispatchPool.
        let Prepared { input, ctx, .. } = envelope::prepare(&input, self.resolver.as_ref()).await?;
        let text = provider.run(&input, &ctx).await?;
        Ok(text.into_bytes())
    }
}

#[async_trait::async_trait(?Send)]
impl Worker for DispatchWorker {
    async fn run(&self, effect: &Effect) -> Result<WorkOutcome, reactor::Error> {
        match gate(&self.providers, &self.node_key, effect) {
            Gated::NotMine => Ok(WorkOutcome::NotMine),
            Gated::Skip => Ok(WorkOutcome::Handled(None)),
            Gated::Immediate(msg) => Ok(WorkOutcome::Handled(Some(msg))),
            Gated::Execute(job) => {
                let outcome = self
                    .answer(&job.capability, job.input.as_bytes())
                    .await
                    .map_err(clean_error);
                Ok(WorkOutcome::Handled(Some(oracle_result(
                    &job.saga_id,
                    job.attempt,
                    outcome,
                ))))
            }
        }
    }
}

/// the follow-up op that carries one attempt's outcome back through the
/// normal submit path, echoing the request's `(saga_id, attempt)`
/// idempotency key.
fn oracle_result(saga_id: &str, attempt: u32, outcome: Result<Vec<u8>, String>) -> Msg {
    Msg {
        target: "saga".into(),
        payload: encode_msg(&SagaMsg::OracleResult {
            saga_id: saga_id.into(),
            attempt,
            outcome,
        }),
    }
}

/// the claim op for an announcement request — submitted under this node's
/// key; the saga's first-accept-wins rule settles who executes.
fn accept_op(request: &WorkerRequest) -> Msg {
    Msg {
        target: "saga".into(),
        payload: encode_msg(&SagaMsg::Accept {
            saga_id: request.saga_id.clone(),
            attempt: request.attempt,
        }),
    }
}

/// bound an executor's error text well under saga's error cap.
fn clean_error(error: String) -> String {
    const MAX: usize = 2048;
    if error.len() <= MAX {
        return error;
    }
    let mut keep = MAX;
    while keep > 0 && !error.is_char_boundary(keep) {
        keep -= 1;
    }
    let mut out = error;
    out.truncate(keep);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use dispatch::{WORK_SPEC_KIND, WorkSpec, encode_work_spec};
    use saga::encode_worker_request;

    /// a provider surface with one loaded mock spec and NO installed
    /// binaries — enough for every non-live test; no executor is named.
    fn mock_specs_only() -> ProviderSet {
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
        ProviderSet::assemble(capability_host::SpecSet::from_specs(vec![spec]), Vec::new())
    }

    fn effect_for(spec: Vec<u8>, assignee: Option<&[u8]>) -> Effect {
        Effect(encode_worker_request(&WorkerRequest {
            saga_id: "s".into(),
            attempt: 0,
            spec,
            deadline: None,
            assignee: assignee.map(|a| a.to_vec()),
        }))
    }

    fn work_spec() -> Vec<u8> {
        encode_work_spec(&WorkSpec {
            kind: WORK_SPEC_KIND.into(),
            dispatch_id: "d1".into(),
            capability: "alpha".into(),
            payload: b"the entire input".to_vec(),
        })
    }

    #[tokio::test]
    async fn foreign_specs_are_not_mine_and_foreign_leases_are_skipped() {
        let worker = DispatchWorker::new(mock_specs_only(), b"me".to_vec());

        // a foreign spec shape (no kind field) is NotMine, never guessed at.
        let foreign = effect_for(br#"{"run_id":"r","agent_id":"a"}"#.to_vec(), None);
        assert!(matches!(
            worker.run(&foreign).await.unwrap(),
            WorkOutcome::NotMine
        ));

        // someone else's lease: claimed but deliberately not run.
        match worker
            .run(&effect_for(work_spec(), Some(b"peer")))
            .await
            .unwrap()
        {
            WorkOutcome::Handled(None) => {}
            other => panic!("a foreign lease must be a claimed skip, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn an_own_lease_executes_to_a_clean_missing_capability_error() {
        let worker = DispatchWorker::new(mock_specs_only(), b"me".to_vec());
        // the spec is loaded but no binary installed: the spawn path is taken
        // and errors by capability name — proving execution was attempted.
        match worker
            .run(&effect_for(work_spec(), Some(b"me")))
            .await
            .unwrap()
        {
            WorkOutcome::Handled(Some(msg)) => {
                let SagaMsg::OracleResult { outcome, .. } =
                    saga::decode_msg(&msg.payload).unwrap()
                else {
                    panic!("expected an oracle result");
                };
                let err = outcome.unwrap_err();
                assert!(err.contains("\"alpha\" is not provided"), "got: {err}");
            }
            other => panic!("an executable lease must produce an op, got {other:?}"),
        }
    }

    /// a provider whose only job is making resolve() succeed in tests.
    struct StubProvider;
    #[async_trait::async_trait]
    impl capability_host::Provider for StubProvider {
        fn capability(&self) -> &str {
            "alpha"
        }
        async fn run(
            &self,
            _prompt: &str,
            _ctx: &capability_host::RunContext,
        ) -> Result<String, String> {
            Ok("stub answer".into())
        }
    }

    #[tokio::test]
    async fn an_unassigned_request_is_claimed_not_run() {
        // an announcement this host CAN serve: the worker answers with an
        // Accept claim, never an execution — the saga's first-accept-wins
        // rule is what turns N capable nodes into one run.
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
        let providers = ProviderSet::assemble(
            capability_host::SpecSet::from_specs(vec![spec]),
            vec![Box::new(StubProvider)],
        );
        let worker = DispatchWorker::new(providers, b"me".to_vec());
        match worker.run(&effect_for(work_spec(), None)).await.unwrap() {
            WorkOutcome::Handled(Some(msg)) => {
                match saga::decode_msg(&msg.payload).unwrap() {
                    SagaMsg::Accept { saga_id, attempt } => {
                        assert_eq!(saga_id, "s");
                        assert_eq!(attempt, 0);
                    }
                    other => panic!("expected an Accept claim, got {other:?}"),
                }
            }
            other => panic!("a claimable announcement must produce an op, got {other:?}"),
        }

        // an announcement this host CANNOT serve (spec loaded, nothing
        // installed): a quiet skip — never a claim it could not honor.
        let worker = DispatchWorker::new(mock_specs_only(), b"me".to_vec());
        match worker.run(&effect_for(work_spec(), None)).await.unwrap() {
            WorkOutcome::Handled(None) => {}
            other => panic!("an unservable announcement must be a skip, got {other:?}"),
        }
    }

    /// live end-to-end against a REAL locally installed CLI (BYO auth):
    /// the payload goes to the provider VERBATIM and the raw answer comes
    /// back — the whole opinion-free contract. ignored by default; name the
    /// capability tag your host provides:
    /// `DUCKTAPE_LIVE_CAPABILITY=<tag> cargo test -p dispatch-oracle -- --ignored live_run`.
    #[tokio::test]
    #[ignore]
    async fn live_run_feeds_the_payload_verbatim_to_a_local_cli() {
        let capability = std::env::var("DUCKTAPE_LIVE_CAPABILITY")
            .expect("set DUCKTAPE_LIVE_CAPABILITY to a capability this host provides");
        let worker = DispatchWorker::new(
            capability_host::discover().expect("capability specs load"),
            b"live".to_vec(),
        );
        let payload = b"Reply with exactly one word: quack".to_vec();
        let answer = worker
            .answer(&capability, &payload)
            .await
            .expect("the provider answered");
        assert!(
            !answer.is_empty(),
            "the raw answer must be non-empty (got zero bytes)"
        );
    }

    #[tokio::test]
    async fn non_utf8_payloads_error_cleanly() {
        let worker = DispatchWorker::new(mock_specs_only(), b"me".to_vec());
        let spec = encode_work_spec(&WorkSpec {
            kind: WORK_SPEC_KIND.into(),
            dispatch_id: "d1".into(),
            capability: "alpha".into(),
            payload: vec![0xff, 0xfe],
        });
        match worker.run(&effect_for(spec, Some(b"me"))).await.unwrap() {
            WorkOutcome::Handled(Some(msg)) => {
                let SagaMsg::OracleResult { outcome, .. } =
                    saga::decode_msg(&msg.payload).unwrap()
                else {
                    panic!("expected an oracle result");
                };
                assert!(outcome.unwrap_err().contains("not utf-8"));
            }
            other => panic!("expected a submitted error, got {other:?}"),
        }
    }
}
