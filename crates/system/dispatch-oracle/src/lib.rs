//! host-side worker for dispatch saga effects.
//!
//! the dispatch module stages a saga whose spec is a self-described
//! [`WorkSpec`]. this crate is the impure host-side counterpart: it resolves
//! the spec's capability tag to a machine-local [`capability_host::Provider`]
//! (whichever executor CLI the operator brought), feeds the payload to it,
//! and submits the raw answer as a saga `OracleResult` op.
//!
//! the payload is a run ENVELOPE (marker `ducktape_run`, v3-only — legacy
//! flat strings and v2 envelopes fail the run loudly), assembled host-side:
//! the agent's registered prompt resolved from the node's blob store by its
//! committed content hash, plus the contract and conversation the dispatcher
//! composed. beyond that
//! assembly the crate stays opinion-free: NO prompt text is authored here,
//! NO output shape is parsed here (the dispatch module judges the recipe's
//! output contract in consensus), and NO credentials are touched (BYO CLI
//! auth). foreign leases are skipped: under the strict policy someone else's
//! assignment would be a no-op result, so not spawning is what turns
//! N-nodes-each-paying-for-the-same-call into one call.
//!
//! the one worker is [`DispatchPool`]: it gates inline ([`gate`]), hands
//! execution to a spawned background task, and returns immediately — so a
//! minutes-long CLI call never stalls the host's event loop.

use capability_host::{ProviderOutput, ProviderSet};
use dispatch::{WorkSpec, decode_work_spec};
use saga::{SagaMsg, WorkerRequest, decode_worker_request, encode_msg};
use sdk::{Effect, Msg};

mod envelope;
mod pool;
mod provision;
mod workspace_source;
pub use envelope::BlobResolver;
pub use pool::{DeliverFn, DispatchPool, SpawnFn};
pub use provision::{
    ProvisionedWorkspace, RoMount, SharedProvisioner, WorkspaceProvisioner, WorkspaceReceipt,
    WorkspaceSpec,
};
pub use workspace_source::WorkspaceSource;

/// everything a provider execution needs, extracted by the gate so the
/// expensive run can happen away from the effect-offer callsite (on a spawned
/// task, or inline — the gate does not care).
pub(crate) struct ExecJob {
    pub saga_id: String,
    pub attempt: u32,
    pub capability: String,
    /// the fully rendered prompt — the WorkSpec payload, verbatim.
    pub input: String,
}

pub(crate) struct AttemptOutput {
    pub bytes: Vec<u8>,
    pub usage: Option<saga::TokenUsage>,
}

fn attempt_output(output: ProviderOutput, bytes: Vec<u8>) -> AttemptOutput {
    AttemptOutput {
        bytes,
        usage: output.usage.map(|usage| saga::TokenUsage {
            input_tokens: usage.input_tokens,
            cached_input_tokens: usage.cached_input_tokens,
            cache_write_input_tokens: usage.cache_write_input_tokens,
            output_tokens: usage.output_tokens,
            reasoning_output_tokens: usage.reasoning_output_tokens,
        }),
    }
}

/// the fast, deterministic half of the worker step: decode routing plus the
/// lease gate. everything here is cheap; only [`Gated::Execute`] carries work
/// that costs real time.
pub(crate) enum Gated {
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
pub(crate) fn gate(providers: &ProviderSet, node_key: &[u8], effect: &Effect) -> Gated {
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

/// the follow-up op that carries one attempt's outcome back through the
/// normal submit path, echoing the request's `(saga_id, attempt)`
/// idempotency key.
fn oracle_result(saga_id: &str, attempt: u32, outcome: Result<Vec<u8>, String>) -> Msg {
    oracle_result_with_usage(saga_id, attempt, outcome, None)
}

fn oracle_result_with_usage(
    saga_id: &str,
    attempt: u32,
    outcome: Result<Vec<u8>, String>,
    usage: Option<saga::TokenUsage>,
) -> Msg {
    Msg {
        target: "saga".into(),
        payload: encode_msg(&SagaMsg::OracleResult {
            saga_id: saga_id.into(),
            attempt,
            outcome,
            usage,
        }),
    }
}

fn renew_lease(saga_id: &str, attempt: u32) -> Msg {
    Msg {
        target: "saga".into(),
        payload: encode_msg(&SagaMsg::RenewLease {
            saga_id: saga_id.into(),
            attempt,
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
            demands: Default::default(),
        })
    }

    #[test]
    fn foreign_specs_are_not_mine_and_foreign_leases_are_skipped() {
        let providers = mock_specs_only();

        // a foreign spec shape (no kind field) is NotMine, never guessed at.
        let foreign = effect_for(br#"{"run_id":"r","agent_id":"a"}"#.to_vec(), None);
        assert!(matches!(gate(&providers, b"me", &foreign), Gated::NotMine));

        // someone else's lease: claimed but deliberately not run.
        let peer = effect_for(work_spec(), Some(b"peer"));
        assert!(matches!(gate(&providers, b"me", &peer), Gated::Skip));
    }

    #[test]
    fn an_own_lease_with_an_unresolvable_capability_errors_inline() {
        // the spec is loaded but no binary installed: the gate answers with
        // the error result by capability name — never an Execute job.
        let providers = mock_specs_only();
        match gate(&providers, b"me", &effect_for(work_spec(), Some(b"me"))) {
            Gated::Immediate(msg) => {
                let SagaMsg::OracleResult { outcome, .. } = saga::decode_msg(&msg.payload).unwrap()
                else {
                    panic!("expected an oracle result");
                };
                let err = outcome.unwrap_err();
                assert!(err.contains("\"alpha\" is not provided"), "got: {err}");
            }
            _ => panic!("an unresolvable own lease must answer inline"),
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

    #[test]
    fn an_unassigned_request_is_claimed_not_run() {
        // an announcement this host CAN serve: the gate answers with an
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
        match gate(&providers, b"me", &effect_for(work_spec(), None)) {
            Gated::Immediate(msg) => match saga::decode_msg(&msg.payload).unwrap() {
                SagaMsg::Accept { saga_id, attempt } => {
                    assert_eq!(saga_id, "s");
                    assert_eq!(attempt, 0);
                }
                other => panic!("expected an Accept claim, got {other:?}"),
            },
            _ => panic!("a claimable announcement must produce an op"),
        }

        // an announcement this host CANNOT serve (spec loaded, nothing
        // installed): a quiet skip — never a claim it could not honor.
        let providers = mock_specs_only();
        assert!(matches!(
            gate(&providers, b"me", &effect_for(work_spec(), None)),
            Gated::Skip
        ));
    }

    #[test]
    fn non_utf8_payloads_error_cleanly() {
        let providers = mock_specs_only();
        let spec = encode_work_spec(&WorkSpec {
            kind: WORK_SPEC_KIND.into(),
            dispatch_id: "d1".into(),
            capability: "alpha".into(),
            payload: vec![0xff, 0xfe],
            demands: Default::default(),
        });
        match gate(&providers, b"me", &effect_for(spec, Some(b"me"))) {
            Gated::Immediate(msg) => {
                let SagaMsg::OracleResult { outcome, .. } = saga::decode_msg(&msg.payload).unwrap()
                else {
                    panic!("expected an oracle result");
                };
                assert!(outcome.unwrap_err().contains("not utf-8"));
            }
            _ => panic!("expected a submitted error"),
        }
    }
}
