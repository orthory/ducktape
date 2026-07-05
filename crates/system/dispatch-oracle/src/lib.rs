//! host-side worker for dispatch saga effects.
//!
//! the dispatch module stages a saga whose spec is a self-described
//! [`WorkSpec`]. this crate is the impure host-side counterpart: it resolves
//! the spec's capability tag to a machine-local [`capability_host::Provider`]
//! (whichever executor CLI the operator brought), feeds the payload to it
//! VERBATIM, and submits the raw answer as a saga `OracleResult` op.
//!
//! deliberately opinion-free: NO prompt text is composed here (the payload IS
//! the entire input — the dispatcher composed it), NO output shape is parsed
//! here (the dispatch module judges the recipe's output contract in
//! consensus), and NO credentials are touched (BYO CLI auth). foreign leases
//! are skipped exactly like `agent-oracle`: under the strict policy someone
//! else's assignment would be a no-op result, so not spawning is what turns
//! N-nodes-each-paying-for-the-same-call into one call.

use capability_host::ProviderSet;
use dispatch_interface::decode_work_spec;
use reactor::{WorkOutcome, Worker};
use saga_interface::{SagaMsg, WorkerRequest, decode_worker_request, encode_msg};
use sdk::{Effect, Msg};

/// Production worker for dispatch `WorkSpec` saga effects.
pub struct DispatchWorker {
    providers: ProviderSet,
    /// this node's external submit key — compared against a request's
    /// `assignee` to decide whether the lease is ours to execute.
    node_key: Vec<u8>,
}

impl DispatchWorker {
    pub fn new(providers: ProviderSet, node_key: Vec<u8>) -> Self {
        Self {
            providers,
            node_key,
        }
    }

    async fn answer(&self, capability: &str, payload: &[u8]) -> Result<Vec<u8>, String> {
        // payload shape first: this verdict must not depend on what happens
        // to be installed on this host.
        let input = String::from_utf8(payload.to_vec())
            .map_err(|_| "dispatch payload is not utf-8; providers take text".to_string())?;
        let provider = self.providers.resolve(capability)?;
        let text = provider.run(&input).await?;
        Ok(text.into_bytes())
    }
}

#[async_trait::async_trait(?Send)]
impl Worker for DispatchWorker {
    async fn run(&self, effect: &Effect) -> Result<WorkOutcome, reactor::Error> {
        let request = match decode_worker_request(&effect.0) {
            Ok(request) => request,
            Err(_) => return Ok(WorkOutcome::NotMine),
        };
        // the kind-gated decode: foreign spec shapes (e.g. the legacy agent
        // LlmRequest) are NotMine, never a guessed execution.
        let work = match decode_work_spec(&request.spec) {
            Ok(work) => work,
            Err(_) => return Ok(WorkOutcome::NotMine),
        };
        // the lease gate, host side: someone else's assignment is a claimed
        // skip — it IS our effect type, but the assignee submits the result.
        if let Some(assignee) = &request.assignee {
            if *assignee != self.node_key {
                return Ok(WorkOutcome::Handled(None));
            }
        }
        let outcome = self
            .answer(&work.capability, &work.payload)
            .await
            .map_err(clean_error);
        Ok(WorkOutcome::Handled(Some(oracle_result(&request, outcome))))
    }
}

fn oracle_result(request: &WorkerRequest, outcome: Result<Vec<u8>, String>) -> Msg {
    Msg {
        target: "saga".into(),
        payload: encode_msg(&SagaMsg::OracleResult {
            saga_id: request.saga_id.clone(),
            attempt: request.attempt,
            outcome,
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
    use dispatch_interface::{WORK_SPEC_KIND, WorkSpec, encode_work_spec};
    use saga_interface::encode_worker_request;

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

        // an agent-shaped spec (no kind field) is NotMine — the legacy
        // worker's work, never guessed at.
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
    async fn own_and_open_leases_execute_to_a_clean_missing_capability_error() {
        let worker = DispatchWorker::new(mock_specs_only(), b"me".to_vec());
        // the spec is loaded but no binary installed: the spawn path is taken
        // and errors by capability name — proving execution was attempted.
        for assignee in [Some(b"me".as_slice()), None] {
            match worker
                .run(&effect_for(work_spec(), assignee))
                .await
                .unwrap()
            {
                WorkOutcome::Handled(Some(msg)) => {
                    let SagaMsg::OracleResult { outcome, .. } =
                        saga_interface::decode_msg(&msg.payload).unwrap()
                    else {
                        panic!("expected an oracle result");
                    };
                    let err = outcome.unwrap_err();
                    assert!(err.contains("\"alpha\" is not provided"), "got: {err}");
                }
                other => panic!("an executable lease must produce an op, got {other:?}"),
            }
        }
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
        match worker.run(&effect_for(spec, None)).await.unwrap() {
            WorkOutcome::Handled(Some(msg)) => {
                let SagaMsg::OracleResult { outcome, .. } =
                    saga_interface::decode_msg(&msg.payload).unwrap()
                else {
                    panic!("expected an oracle result");
                };
                assert!(outcome.unwrap_err().contains("not utf-8"));
            }
            other => panic!("expected a submitted error, got {other:?}"),
        }
    }
}
