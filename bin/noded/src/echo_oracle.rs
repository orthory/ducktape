//! the embedded daemon's debug echo worker.
//!
//! The real compute plane is a STANDALONE DAEMON now
//! (`ducktape service run compute`), so this binary constructs no provider set,
//! no podman service and no dispatch pool: the reactor seam below carries only
//! the `DUCKTAPE_NODED_ECHO_ORACLE` stand-in the daemon e2e drives, and a
//! release build carries nothing at all.

/// the worker set this daemon offers finalized-block effects to.
///
/// Empty outside the debug echo: an unclaimed effect is still surfaced by the
/// caller's `ModuleNotes`, which is exactly the diagnostic a stuck saga needs
/// ("nothing here executes dispatch work" is the honest answer on a node whose
/// compute daemon is not running).
pub(crate) fn workers() -> Vec<Box<dyn host::worker::Worker>> {
    #[cfg(debug_assertions)]
    if std::env::var_os("DUCKTAPE_NODED_ECHO_ORACLE").is_some() {
        return vec![Box::new(EchoWorker)];
    }
    Vec::new()
}

/// a debug-only stand-in that answers every dispatch WorkSpec inline with a
/// deterministic echo — keeps daemon e2e block arithmetic exact (no spawn,
/// no lane hop) while exercising the full effect->result->delivery path.
#[cfg(debug_assertions)]
struct EchoWorker;

#[cfg(debug_assertions)]
#[async_trait::async_trait(?Send)]
impl host::worker::Worker for EchoWorker {
    async fn run(
        &self,
        event: &sdk::Event,
    ) -> Result<host::worker::WorkOutcome, host::worker::Error> {
        let request = match saga::decode_worker_request(&event.payload) {
            Ok(request) => request,
            Err(_) => return Ok(host::worker::WorkOutcome::NotMine),
        };
        // a dispatch-plane WorkSpec echoes its raw-text lane (the dispatch
        // module judged a Text contract; the agent module normalizes).
        let Ok(work) = dispatch::decode_work_spec(&request.spec) else {
            return Ok(host::worker::WorkOutcome::NotMine);
        };
        Ok(host::worker::WorkOutcome::Handled(Some(sdk::Msg {
            target: "saga".into(),
            payload: saga::encode_msg(&saga::SagaMsg::OracleResult {
                saga_id: request.saga_id,
                attempt: request.attempt,
                // the runs module accepts only ducktape_runner_result wrappers
                // (the flat-payload tolerance is gone) — wrap the echo like a
                // real provider result.
                outcome: Ok(serde_json::json!({
                    "ducktape_runner_result": 1,
                    "response_text": format!("echo: handling dispatch {}", work.dispatch_id),
                    "workspace_receipt": {
                        "source_prefix": "echo",
                        "output_snapshot": null,
                        "commit_height": null,
                        "rebased": false,
                        "no_changes": true,
                    },
                })
                .to_string()
                .into_bytes()),
                usage: None,
            }),
        })))
    }
}
