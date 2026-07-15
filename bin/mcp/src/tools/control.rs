use serde_json::{Value, json};

use super::{Tool, arg_str, schema};
use crate::identity::Run;
use crate::node::{NodeError, Result};

pub(super) fn tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "ducktape_extend_provider_idle",
            description: "Request more silent time for only this live provider invocation. The host returns a bounded synchronous granted or denied result; reuse request_id only with identical seconds.",
            schema: || {
                let mut value = schema(&[
                    (
                        "request_id",
                        "string",
                        true,
                        "An idempotency key for this request.",
                    ),
                    (
                        "requested_secs",
                        "integer",
                        true,
                        "Silent time requested from now; the host applies its own limits and hard cap.",
                    ),
                ]);
                value["additionalProperties"] = Value::Bool(false);
                value
            },
            handler: extend_provider_idle,
        },
        Tool {
            name: "ducktape_delegate",
            description: "Call another registered agent while this run remains live. Agents stay peers; the callee receives caller ∩ callee authority. request_id is idempotent and the root agent's subagent_budget bounds the whole call tree.",
            schema: || {
                json!({
                    "type": "object",
                    "properties": {
                        "request_id": {"type":"string", "description":"Stable idempotency key for this call."},
                        "agent_id": {"type":"string", "description":"Registered peer agent to call."},
                        "instruction": {"type":"string", "description":"Bounded task instruction for the callee."},
                        "skills": {
                            "type":"array",
                            "items":{"type":"string"},
                            "description":"Optional shared-library skills for this task."
                        }
                    },
                    "required":["request_id", "agent_id", "instruction"],
                    "additionalProperties":false
                })
            },
            handler: delegate,
        },
        Tool {
            name: "ducktape_delegations",
            description: "List this caller run's pending peer calls and collect their delivered, failed, or cancelled results.",
            schema: || {
                json!({
                    "type":"object",
                    "properties":{},
                    "additionalProperties":false
                })
            },
            handler: delegations,
        },
    ]
}

fn extend_provider_idle(run: &Run, args: &Value) -> Result<Value> {
    let object = args
        .as_object()
        .ok_or_else(|| NodeError::Rejected("this tool needs an object argument".into()))?;
    if object
        .keys()
        .any(|key| key != "request_id" && key != "requested_secs")
    {
        return Err(NodeError::Rejected(
            "this tool accepts only request_id and requested_secs".into(),
        ));
    }
    let requested_secs = args
        .get("requested_secs")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            NodeError::Rejected("this tool needs an integer \"requested_secs\" argument".into())
        })?;
    run.extend_provider_idle(arg_str(args, "request_id")?, requested_secs)
}

fn delegate(run: &Run, args: &Value) -> Result<Value> {
    let skills = match args.get("skills") {
        None => Vec::new(),
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_string)
                    .ok_or_else(|| NodeError::Rejected("skills must contain only strings".into()))
            })
            .collect::<Result<Vec<_>>>()?,
        Some(_) => {
            return Err(NodeError::Rejected(
                "skills must be an array of strings".into(),
            ));
        }
    };
    run.delegate(
        arg_str(args, "request_id")?,
        agent::DelegationRequest {
            agent_id: arg_str(args, "agent_id")?,
            instruction: arg_str(args, "instruction")?,
            skills,
        },
    )
}

fn delegations(run: &Run, args: &Value) -> Result<Value> {
    if args.as_object().is_none_or(|object| !object.is_empty()) {
        return Err(NodeError::Rejected(
            "this tool takes an empty object".into(),
        ));
    }
    run.delegations()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_accepts_only_request_identity_and_seconds() {
        let schema = (tools()[0].schema)();
        let properties = schema["properties"].as_object().unwrap();
        assert_eq!(properties.len(), 2);
        assert!(properties.contains_key("request_id"));
        assert!(properties.contains_key("requested_secs"));
        assert_eq!(schema["additionalProperties"], false);
    }

    #[test]
    fn delegation_tools_expose_only_run_scoped_inputs() {
        let tools = tools();
        let delegate = tools
            .iter()
            .find(|tool| tool.name == "ducktape_delegate")
            .unwrap();
        let schema = (delegate.schema)();
        assert_eq!(schema["additionalProperties"], false);
        assert!(schema["properties"].get("caller_run_id").is_none());
        assert!(schema["properties"].get("authority").is_none());
        assert_eq!(schema["properties"]["skills"]["type"], "array");
    }

    #[test]
    fn handler_rejects_caller_supplied_identity_or_credentials() {
        for extra in ["run_id", "attempt", "credential", "control_token"] {
            let mut args = serde_json::json!({"request_id":"r", "requested_secs":1});
            args[extra] = Value::String("other".into());
            let error = extend_provider_idle(&Run::from_env(), &args).unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("only request_id and requested_secs")
            );
        }
    }

    #[test]
    fn unavailable_controller_is_a_typed_denial() {
        let reply = extend_provider_idle(
            &Run::from_env(),
            &serde_json::json!({"request_id":"r", "requested_secs":1}),
        )
        .unwrap();
        assert_eq!(reply["status"], "denied");
        assert_eq!(reply["reason"], "unavailable");
    }
}
