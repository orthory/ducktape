//! the tool table: one flat registry of every tool the plane exposes, split
//! read / write because the two halves have genuinely different rules.
//!
//! - READ tools ([`read`]) are ungated except where the caps vocabulary already
//!   names the resource (`forge_read` repos, `duckfs_read` prefixes).
//! - WRITE tools ([`write`]) mirror `agent::KNOWN_ACTIONS` ONE-FOR-ONE. that is
//!   the point: the tool plane grants an agent nothing its registered
//!   `allowed_actions` did not already grant it, and there is exactly one
//!   vocabulary of "what an agent may do" — the one consensus validates a
//!   response's actions against.
//!
//! a tool's `description` is not decoration: it is the entire interface the
//! model has. it says what the tool reads or writes, and — for a write — which
//! action name gates it, so a denied agent can tell its owner precisely which
//! grant to widen.

use serde_json::{Value, json};

use crate::identity::Run;
use crate::node::Result;

#[cfg(test)]
mod librarian;
mod control;
mod read;
mod write;

/// one tool: its MCP declaration and the handler behind it.
pub struct Tool {
    pub name: &'static str,
    pub description: &'static str,
    /// the JSON Schema for `arguments`, built by [`schema`].
    pub schema: fn() -> Value,
    pub handler: fn(&Run, &Value) -> Result<Value>,
}

/// every tool, in the order `tools/list` reports them: `whoami` first (an agent
/// that reads nothing else should still read this), then the rest of the read
/// plane, then the write plane.
pub fn all() -> Vec<Tool> {
    let mut tools = read::tools();
    tools.extend(control::tools());
    tools.extend(write::tools());
    tools
}

pub fn find(name: &str) -> Option<Tool> {
    all().into_iter().find(|t| t.name == name)
}

/// the `tools/list` payload.
pub fn list() -> Value {
    let tools: Vec<Value> = all()
        .iter()
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
                "inputSchema": (t.schema)(),
            })
        })
        .collect();
    json!({"tools": tools})
}

/// a JSON Schema object from `(name, type, required, description)` rows — the
/// whole schema surface this plane needs. no nested objects, no arrays of
/// objects: every tool here takes a flat bag of scalars, and keeping it that
/// way is what lets the table stay a table.
pub fn schema(props: &[(&str, &str, bool, &str)]) -> Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    for (name, ty, req, desc) in props {
        properties.insert(
            (*name).to_string(),
            json!({"type": ty, "description": desc}),
        );
        if *req {
            required.push(Value::String((*name).to_string()));
        }
    }
    json!({
        "type": "object",
        "properties": Value::Object(properties),
        "required": required,
    })
}

/// a required string argument, or a refusal naming it. the model gets the
/// argument's NAME back, not "invalid input" — it can only fix what it can see.
pub fn arg_str(args: &Value, name: &str) -> Result<String> {
    args.get(name)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            crate::node::NodeError::Rejected(format!("this tool needs a string {name:?} argument"))
        })
}

/// an optional integer argument.
pub fn opt_u64(args: &Value, name: &str) -> Option<u64> {
    args.get(name).and_then(Value::as_u64)
}

/// a required boolean argument.
pub fn arg_bool(args: &Value, name: &str) -> Result<bool> {
    args.get(name).and_then(Value::as_bool).ok_or_else(|| {
        crate::node::NodeError::Rejected(format!("this tool needs a boolean {name:?} argument"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_is_uniquely_named_and_declares_a_schema() {
        let tools = all();
        let mut names: Vec<&str> = tools.iter().map(|t| t.name).collect();
        let count = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), count, "tool names must be unique");
        for t in &tools {
            assert!(
                t.name.starts_with("ducktape_"),
                "{} is not namespaced",
                t.name
            );
            assert!(!t.description.is_empty(), "{} has no description", t.name);
            let schema = (t.schema)();
            assert_eq!(schema["type"], "object", "{} schema is not an object", t.name);
        }
    }

    #[test]
    fn every_write_tool_names_a_known_action_in_its_description() {
        // the description is the model's only view of the gate. a write tool
        // whose description does not name its action leaves a denied agent
        // unable to say which grant it needs.
        for t in write::tools() {
            assert!(
                agent::KNOWN_ACTIONS
                    .iter()
                    .any(|a| t.description.contains(a)),
                "write tool {} names no known action in its description",
                t.name
            );
        }
    }

    #[test]
    fn list_reports_every_tool() {
        let listed = list();
        assert_eq!(listed["tools"].as_array().unwrap().len(), all().len());
    }

    #[test]
    fn find_resolves_by_name_and_rejects_an_unknown_one() {
        assert!(find("ducktape_whoami").is_some());
        assert!(find("ducktape_ask_librarian").is_none());
        assert!(find("ducktape_not_a_tool").is_none());
    }

    #[test]
    fn schema_marks_only_the_required_rows() {
        let s = schema(&[
            ("a", "string", true, "required"),
            ("b", "integer", false, "optional"),
        ]);
        assert_eq!(s["required"], json!(["a"]));
        assert_eq!(s["properties"]["b"]["type"], "integer");
    }
}
