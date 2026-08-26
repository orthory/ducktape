//! The run envelope's magic and the headless composer that stamps it.
//!
//! A run's payload is a JSON envelope carrying a `ducktape_run` marker, the
//! instructions, a workspace source, the skill pins, and the strict output
//! contract the runner must answer with. `compute-service` owns the READER —
//! parsing one, assembling the model input, validating a result — and that
//! reader links `provider-host`, which links the microVM sandbox.
//!
//! The schema is not the machinery. A desktop app that submits a durable run
//! needs to compose one payload; it has no business linking a sandbox to do it,
//! and before this crate the only way it could was to spawn `ducktape agent
//! sched` and read a run id off the child's stdout.

/// the fixed value of the `ducktape_run` magic key. Key and value TOGETHER are
/// the envelope's self-identifying token — the digit is part of the magic, like
/// a container magic, never a version to bump.
pub const RUN_ENVELOPE_MARKER: u64 = 1;

/// the fixed value of the `ducktape_runner_result` magic key: what the
/// provisioning wrapper stamps on a result and what an envelope asks for in its
/// `result_contract`. A reader that finds another value is looking at output
/// from something that is not this runner.
pub const RUNNER_RESULT_MARKER: u64 = 1;

/// Compose the ONE payload shape a headless `sched` run carries: a minimal
/// valid envelope with the prompt as its instructions, a fresh per-run duckfs
/// workspace (no pinned snapshot — a headless prompt has no workspace to
/// resume), no skills, no chat contract, and the given credential name.
///
/// Every caller that schedules a headless run goes through here, so the schema
/// lives in exactly one place: a second hand-rolled `serde_json::json!` of this
/// shape somewhere else is a payload that drifts silently until a run fails on
/// a box nobody is watching.
pub fn compose_headless(run_id: &str, prompt: &str, credential: Option<&str>) -> String {
    let mut envelope = serde_json::json!({
        "ducktape_run": RUN_ENVELOPE_MARKER,
        "agent_id": "sched",
        "agent_display_name": "sched",
        "run_id": run_id,
        "instructions": prompt,
        "contract": "",
        "conversation": "",
        "workspace": {
            "kind": "duckfs",
            "source_prefix": "/shared/agent-workspaces/sched",
            "source_snapshot": null,
        },
        "skills": [],
        "library_readable": false,
        "result_contract": { "ducktape_runner_result": RUNNER_RESULT_MARKER },
    });
    if let Some(credential) = credential {
        envelope["credential"] = serde_json::Value::String(credential.to_string());
    }
    serde_json::to_string(&envelope).expect("a headless envelope always serializes")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shape a headless run is admitted by. Every field here is read by
    /// `compute-service`'s accept slice, so a rename is a wire change — this
    /// test is what makes one visible instead of failing a run on a remote box.
    #[test]
    fn a_headless_envelope_carries_the_magic_and_the_result_contract() {
        let composed = compose_headless("run-1", "do the thing", Some("my-cred"));
        let value: serde_json::Value = serde_json::from_str(&composed).unwrap();

        assert_eq!(value["ducktape_run"], RUN_ENVELOPE_MARKER);
        assert_eq!(value["run_id"], "run-1");
        assert_eq!(value["instructions"], "do the thing");
        assert_eq!(value["credential"], "my-cred");
        assert_eq!(
            value["result_contract"]["ducktape_runner_result"],
            RUNNER_RESULT_MARKER
        );
        assert_eq!(value["workspace"]["kind"], "duckfs");
        assert!(value["workspace"]["source_snapshot"].is_null());
        assert_eq!(value["skills"], serde_json::json!([]));
    }

    /// No credential means no KEY, not a null one — the accept slice reads the
    /// field's presence.
    #[test]
    fn a_run_without_a_credential_carries_no_credential_field() {
        let composed = compose_headless("run-2", "prompt", None);
        let value: serde_json::Value = serde_json::from_str(&composed).unwrap();
        assert!(value.get("credential").is_none());
    }
}
