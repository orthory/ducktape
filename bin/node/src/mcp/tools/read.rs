//! the read plane: everything an agent could previously only be TOLD, it can
//! ask for.
//!
//! before this plane existed a run saw exactly what the composer pre-injected
//! into its envelope — the anchored conversation, and a forge item's context if
//! it had one. it could not look up the task it was asked about, read the page
//! it was told to comment on, or open the sibling issue that explains the one it
//! is working. every one of those had to be foreseen in consensus, at compose
//! time, by code that could not know what the agent would want.
//!
//! the plane is four tools. `ducktape_whoami` answers who this run is.
//! `ducktape_actions` lists the catalog: the write operations the runs module
//! owns, straight from consensus, beside the read operations this binary
//! serves. `ducktape_query` runs one read operation by name with the same
//! `operation`/`target`/`input` envelope a write takes, and `ducktape_receipt`
//! reads a write's committed receipt back. reads cross no consensus op, so
//! their table lives here; writes are the module's, so their table does not.
//!
//! queries are built from each module's OWN `*Query` enum rather than
//! hand-written json, so a wire change in `chat` or `forge` breaks this file at
//! COMPILE time instead of at run time in front of a model.
//!
//! caps: `forge_read` and `duckfs_read` gate the two resource families the caps
//! vocabulary actually names. chat / tasks / pages carry no read cap in
//! `ResourceCaps`, so they are ungated here — inventing a gate the registry
//! cannot express would be a permission nobody could grant.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use forge::ForgeQuery;
use pages::PageQuery;
// the ONE duckfs reply filter, shared with the sandboxed run's read lane.
use provider_host::duckfs_cap;
use runs::RunsQuery;
use runs::{CapRequest, ModelQuery};
use tasks::{JobsQuery, TaskQuery, WorkQuery};

use super::{Tool, arg_str, opt_u64, schema};
use crate::mcp::identity::{Run, TARGET_MODEL, TARGET_RUNS};
use crate::mcp::node::{NodeError, Result};

const TARGET_CHAT: &str = "chat";
const TARGET_TASKS: &str = "tasks";
const TARGET_PAGES: &str = "pages";
const TARGET_FORGE: &str = "forge";

/// the read-list default: enough context to be useful, small enough that a
/// careless call cannot blow the model's context.
const DEFAULT_READ_LIMIT: u64 = 50;
const MAX_READ_LIMIT: u64 = 200;

pub(super) fn tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "ducktape_whoami",
            description: "Who you are in Ducktape: your run id, agent id, display name, owner, the \
                          actions you are allowed to take, your resource caps, your workspace \
                          directory, and where your skills are mounted. Call this first if you \
                          are unsure what you are permitted to do — every write is gated on the \
                          actions listed here.",
            schema: || schema(&[]),
            handler: whoami,
        },
        Tool {
            name: "ducktape_actions",
            description: "The operation catalog. Each write operation comes from the runs module \
                          with its target and input schemas, the receipt result it reports, the \
                          grant it requires and the lanes it admits (live via ducktape_action, \
                          final via your final response). Each read operation is one \
                          ducktape_query can run, with its target and input schemas. Pass filter \
                          to keep only names starting with it (e.g. \"pages.\").",
            schema: || schema(&[("filter", "string", false, "A name prefix to narrow the catalog.")]),
            handler: actions,
        },
        Tool {
            name: "ducktape_query",
            description: "Run one read operation from the catalog: operation names it, target \
                          selects the resource it reads (omit when the operation takes none), \
                          input carries its options. Reads of forge repos require the repo in \
                          your forge_read caps; reads of duckfs require the path under your \
                          duckfs_read caps; everything else is ungated.",
            schema: query_schema,
            handler: query,
        },
        Tool {
            name: "ducktape_receipt",
            description: "Read the committed receipt of one write by the receipt_id \
                          ducktape_action returned: its operation, result, target, payload and \
                          status (awaiting the program, claimed, completed with the target's \
                          outcome, or rejected with the reason).",
            schema: || schema(&[("id", "string", true, "The receipt_id ducktape_action returned.")]),
            handler: receipt,
        },
    ]
}

/// one read operation: its catalog entry and the handler behind it. `target`
/// is `None` for an operation that reads no particular resource.
pub(super) struct ReadOperation {
    pub name: &'static str,
    pub description: &'static str,
    pub target: Option<Value>,
    pub input: Value,
    pub handler: fn(&Run, &Value, &Value) -> Result<Value>,
}

impl ReadOperation {
    fn view(&self) -> Value {
        json!({
            "kind": "read",
            "name": self.name,
            "description": self.description,
            "target": self.target,
            "input": self.input,
        })
    }
}

fn closed(properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

fn no_input() -> Value {
    closed(json!({}), &[])
}

pub(super) fn read_operations() -> Vec<ReadOperation> {
    vec![
        ReadOperation {
            name: "agents.list",
            description: "List registered agents with their status, owner, allowed actions, resource caps, and curated skills.",
            target: None,
            input: bounded_list_schema(),
            handler: agents_list,
        },
        ReadOperation {
            name: "runs.list",
            description: "List in-flight run correlations and this node's recent terminal run observations. Recent runs are a bounded derived cache and can be empty after a snapshot join. Live agent sessions and session keys are deliberately not exposed.",
            target: None,
            input: bounded_list_schema(),
            handler: runs_list,
        },
        ReadOperation {
            name: "chat.channels",
            description: "List every chat channel, with its id and name.",
            target: None,
            input: no_input(),
            handler: chat_channels,
        },
        ReadOperation {
            name: "chat.messages",
            description: "Read the most recent top-level messages of a chat channel, oldest first. Each root carries its thread summary.",
            target: Some(closed(json!({"channel_id": {"type": "string"}}), &["channel_id"])),
            input: closed(
                json!({"limit": {"type": "integer", "description": "How many of the newest roots to return (default 50, max 200)."}}),
                &[],
            ),
            handler: chat_messages,
        },
        ReadOperation {
            name: "tasks.list",
            description: "Read one bounded page of tasks — id, title and status (open, in_progress, done) — in ascending id order. Pass the last id you saw as after to continue.",
            target: None,
            input: tasks_list_schema(),
            handler: tasks_list,
        },
        ReadOperation {
            name: "jobs.get",
            description: "Read a job's specification, execution status, result and bounded discussion, including each comment's authenticated author.",
            target: Some(closed(json!({"job_id": {"type": "string"}}), &["job_id"])),
            input: no_input(),
            handler: job_get,
        },
        ReadOperation {
            name: "pages.list",
            description: "Read one bounded page of page ids and titles. Pass next_after as after to continue.",
            target: None,
            input: page_cursor_schema(),
            handler: pages_list,
        },
        ReadOperation {
            name: "pages.get",
            description: "Read one bounded document-order block page. Pass next_after as after to continue. Block ids here are what pages.comment and pages.set_checked target.",
            target: Some(closed(json!({"page_id": {"type": "string"}}), &["page_id"])),
            input: page_cursor_schema(),
            handler: page_get,
        },
        ReadOperation {
            name: "forge.repos",
            description: "List the forge repos and their current heads.",
            target: None,
            input: no_input(),
            handler: forge_repos,
        },
        ReadOperation {
            name: "forge.items",
            description: "List a forge repo's issues and pull requests. Requires the repo in your forge_read caps.",
            target: Some(closed(json!({"repo": {"type": "string"}}), &["repo"])),
            input: no_input(),
            handler: forge_items,
        },
        ReadOperation {
            name: "forge.item",
            description: "Read one forge issue or pull request in full — body, branches, reviews, and the id of its discussion channel (readable with chat.messages). Requires the repo in your forge_read caps.",
            target: Some(closed(
                json!({"repo": {"type": "string"}, "number": {"type": "integer"}}),
                &["repo", "number"],
            )),
            input: no_input(),
            handler: forge_item,
        },
        ReadOperation {
            name: "forge.pr_diff",
            description: "Read a pull request's exact committed source and target OIDs plus a bounded unified patch and full diff statistics. The patch is capped at 48 KiB and reports truncation; inputs beyond 256 changed files or 8 MiB of aggregate blobs fail instead of returning partial statistics. Fails if the item is not a PR or the pinned git objects are unavailable locally. Requires the repo in your forge_read caps.",
            target: Some(closed(
                json!({"repo": {"type": "string"}, "number": {"type": "integer"}}),
                &["repo", "number"],
            )),
            input: no_input(),
            handler: forge_pr_diff,
        },
        ReadOperation {
            name: "files.ls",
            description: "List a directory in the Ducktape filesystem (duckfs). This is the shared, replicated filesystem — NOT your local workspace, which you read with ordinary file tools. Requires the path under your duckfs_read caps.",
            target: Some(closed(json!({"path": {"type": "string"}}), &["path"])),
            input: no_input(),
            handler: files_ls,
        },
        ReadOperation {
            name: "files.read",
            description: "Read a file from the Ducktape filesystem (duckfs) as text. Requires the path under your duckfs_read caps.",
            target: Some(closed(json!({"path": {"type": "string"}}), &["path"])),
            input: no_input(),
            handler: files_read,
        },
        ReadOperation {
            name: "files.grep",
            description: "Search the Ducktape filesystem (duckfs) for matching lines under a path prefix. Requires the prefix under your duckfs_read caps.",
            target: Some(closed(json!({"prefix": {"type": "string"}}), &["prefix"])),
            input: closed(json!({"pattern": {"type": "string"}}), &["pattern"]),
            handler: files_grep,
        },
        ReadOperation {
            name: "agent.calls",
            description: "List this run's agent.call edges: each pending call and every delivered, failed or cancelled result.",
            target: None,
            input: no_input(),
            handler: agent_calls,
        },
    ]
}

fn find_read(name: &str) -> Option<ReadOperation> {
    read_operations().into_iter().find(|op| op.name == name)
}

fn query_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "operation": {
                "type": "string",
                "description": "A read operation name from ducktape_actions.",
            },
            "target": {
                "type": "object",
                "description": "The resource to read, per the operation's target schema. Omit for operations that take none.",
            },
            "input": {
                "type": "object",
                "description": "The operation's options, per its input schema. Omit when it has none.",
            },
        },
        "required": ["operation"],
        "additionalProperties": false,
    })
}

/// the agent's own committed record, plus the host facts it cannot read off the
/// chain: its run id, workspace, and skill mount.
fn whoami(run: &Run, _args: &Value) -> Result<Value> {
    let record = run.record()?;
    Ok(json!({
        "account": record.account,
        "agent_id": record.agent_id,
        "display_name": record.display_name,
        "owner": record.owner,
        "capability": record.capability,
        "status": record.status,
        "allowed_actions": record.allowed_actions,
        "caps": record.caps,
        "skills": record.skills,
        "run_id": run.run_id(),
        "workspace_dir": run.workspace,
        "skills_dir": run.skills,
    }))
}

/// the write catalog as consensus holds it, beside the read table this binary
/// serves. the writes are fetched per call: a module swap that adds an
/// operation shows up here without a tool-binary update.
fn actions(run: &Run, args: &Value) -> Result<Value> {
    let filter = match args.get("filter") {
        None | Some(Value::Null) => None,
        Some(Value::String(filter)) => Some(filter.clone()),
        Some(_) => {
            return Err(NodeError::Rejected(
                "this tool needs a string \"filter\" argument when one is given".into(),
            ));
        }
    };
    let reply = run.node.query(
        TARGET_RUNS,
        encode(&RunsQuery::Catalog {
            filter: filter.clone(),
        })?,
    )?;
    let mut operations: Vec<Value> = reply_array(&reply, "catalog")?
        .into_iter()
        .map(|mut view| {
            view["kind"] = json!("write");
            view
        })
        .collect();
    let keep = |name: &str| filter.as_deref().is_none_or(|prefix| name.starts_with(prefix));
    operations.extend(
        read_operations()
            .iter()
            .filter(|op| keep(op.name))
            .map(ReadOperation::view),
    );
    Ok(json!({"operations": operations}))
}

/// one read operation by name. the envelope is checked for shape only — an
/// object target where the operation takes one, none where it takes none —
/// and each handler reads its own fields by name so a refusal names them.
fn query(run: &Run, args: &Value) -> Result<Value> {
    let name = arg_str(args, "operation")?;
    let Some(operation) = find_read(&name) else {
        return Err(NodeError::Rejected(format!(
            "{name:?} is not a read operation; ducktape_actions lists them"
        )));
    };
    let target = match (args.get("target"), operation.target.is_some()) {
        (None | Some(Value::Null), false) => Value::Null,
        (None | Some(Value::Null), true) => {
            return Err(NodeError::Rejected(format!("{name} requires a target")));
        }
        (Some(_), false) => {
            return Err(NodeError::Rejected(format!("{name} takes no target")));
        }
        (Some(target @ Value::Object(_)), true) => target.clone(),
        (Some(_), true) => {
            return Err(NodeError::Rejected(format!(
                "{name} needs an object \"target\" argument"
            )));
        }
    };
    let input = match args.get("input") {
        None | Some(Value::Null) => json!({}),
        Some(input @ Value::Object(_)) => input.clone(),
        Some(_) => {
            return Err(NodeError::Rejected(format!(
                "{name} needs an object \"input\" argument"
            )));
        }
    };
    (operation.handler)(run, &target, &input)
}

fn receipt(run: &Run, args: &Value) -> Result<Value> {
    run.node.query(
        TARGET_RUNS,
        encode(&RunsQuery::ActionRequest {
            request_id: arg_str(args, "id")?,
        })?,
    )
}

fn agents_list(run: &Run, _target: &Value, input: &Value) -> Result<Value> {
    let limit = list_limit(input)?;
    let reply = run.node.query(
        TARGET_MODEL,
        encode(&runs::RunsQuery::Model {
            query: ModelQuery::Agents,
        })?,
    )?;
    let (agents, total, truncated) = bounded(
        reply_array(
            reply
                .get("model")
                .ok_or_else(|| NodeError::Transport("missing model reply".into()))?,
            "agents",
        )?,
        limit,
    );
    Ok(json!({
        "agents": agents,
        "total": total,
        "truncated": truncated,
    }))
}

fn runs_list(run: &Run, _target: &Value, input: &Value) -> Result<Value> {
    let limit = list_limit(input)?;
    let pending = run
        .node
        .query(TARGET_RUNS, encode(&RunsQuery::PendingRuns)?)?;
    let recent = run
        .node
        .query(TARGET_RUNS, encode(&RunsQuery::RecentRuns)?)?;
    let (pending_runs, pending_total, pending_truncated) =
        bounded(reply_array(&pending, "pending_runs")?, limit);
    let (recent_runs, recent_total, recent_truncated) =
        bounded(reply_array(&recent, "recent_runs")?, limit);
    Ok(json!({
        "pending_runs": pending_runs,
        "pending_total": pending_total,
        "pending_truncated": pending_truncated,
        "recent_runs": recent_runs,
        "recent_total": recent_total,
        "recent_truncated": recent_truncated,
    }))
}

fn chat_channels(run: &Run, _target: &Value, _input: &Value) -> Result<Value> {
    run.node.view(TARGET_CHAT, json!({"channels": {}}))
}

fn chat_messages(run: &Run, target: &Value, input: &Value) -> Result<Value> {
    let limit = opt_u64(input, "limit")
        .unwrap_or(DEFAULT_READ_LIMIT)
        .min(MAX_READ_LIMIT);
    let query = json!({"roots": {
        "channel_id": arg_str(target, "channel_id")?,
        "limit": limit,
    }});
    run.node.view(TARGET_CHAT, query)
}

fn tasks_list(run: &Run, _target: &Value, input: &Value) -> Result<Value> {
    // the board's page bound is its own (`tasks::MAX_LIST_LIMIT`, 256) and it
    // clamps whatever arrives; the pages cursor/limit parsing carries the same
    // shape and the same 1..=256 range, so it is reused verbatim.
    let query = WorkQuery::Task(TaskQuery::List {
        limit: u64::from(page_limit(input)?),
        after: page_cursor(input)?,
    });
    run.node.query(TARGET_TASKS, encode(&query)?)
}

fn job_get(run: &Run, target: &Value, _input: &Value) -> Result<Value> {
    let query = WorkQuery::Job(JobsQuery::Get {
        job_id: arg_str(target, "job_id")?,
    });
    run.node.query(TARGET_TASKS, encode(&query)?)
}

fn pages_list(run: &Run, _target: &Value, input: &Value) -> Result<Value> {
    run.node.view(
        TARGET_PAGES,
        json!({"list_pages": {"after": page_cursor(input)?, "limit": page_limit(input)?}}),
    )
}

fn page_get(run: &Run, target: &Value, input: &Value) -> Result<Value> {
    let query = PageQuery::GetPage {
        page_id: arg_str(target, "page_id")?,
        after: page_cursor(input)?,
        limit: page_limit(input)?,
    };
    run.node.query(TARGET_PAGES, encode(&query)?)
}

fn forge_repos(run: &Run, _target: &Value, _input: &Value) -> Result<Value> {
    run.node
        .query(TARGET_FORGE, encode(&ForgeQuery::ListRepos)?)
}

fn forge_items(run: &Run, target: &Value, _input: &Value) -> Result<Value> {
    let repo = arg_str(target, "repo")?;
    gate_forge_read(run, &repo)?;
    let query = ForgeQuery::ListItems { repo };
    run.node.query(TARGET_FORGE, encode(&query)?)
}

fn forge_item(run: &Run, target: &Value, _input: &Value) -> Result<Value> {
    let repo = arg_str(target, "repo")?;
    let number = item_number(target)?;
    gate_forge_read(run, &repo)?;
    let query = ForgeQuery::GetItem { repo, number };
    run.node.query(TARGET_FORGE, encode(&query)?)
}

fn forge_pr_diff(run: &Run, target: &Value, _input: &Value) -> Result<Value> {
    let repo = arg_str(target, "repo")?;
    let number = item_number(target)?;
    gate_forge_read(run, &repo)?;
    let query = ForgeQuery::PrDiff { repo, number };
    run.node.query(TARGET_FORGE, encode(&query)?)
}

fn item_number(target: &Value) -> Result<u64> {
    opt_u64(target, "number").ok_or_else(|| {
        NodeError::Rejected("this operation needs an integer \"number\" in its target".into())
    })
}

fn files_ls(run: &Run, target: &Value, _input: &Value) -> Result<Value> {
    let path = arg_str(target, "path")?;
    gate_duckfs_read(run, &path)?;
    run.node.files("ls", &[("path", path)])
}

/// duckfs reads come back base64 in `b64`. an agent wants TEXT — hand it the
/// decoded body and say plainly when the bytes are not text, rather than
/// handing a model a base64 blob to decode in its head.
fn files_read(run: &Run, target: &Value, _input: &Value) -> Result<Value> {
    let path = arg_str(target, "path")?;
    gate_duckfs_read(run, &path)?;
    let reply = run.node.files("read", &[("path", path.clone())])?;
    let Some(b64) = reply.get("b64").and_then(Value::as_str) else {
        return Ok(reply);
    };
    let bytes = STANDARD
        .decode(b64)
        .map_err(|e| NodeError::Transport(format!("duckfs returned undecodable base64: {e}")))?;
    match String::from_utf8(bytes) {
        Ok(text) => Ok(json!({
            "path": path,
            "text": text,
            "eof": reply.get("eof").cloned().unwrap_or(Value::Null),
        })),
        Err(e) => Err(NodeError::Rejected(format!(
            "{path:?} is not utf-8 text ({} bytes); this operation reads text files only",
            e.into_bytes().len()
        ))),
    }
}

/// duckfs grep's own prefix rule is a raw string prefix (`/shared/team` also
/// matches `/shared/team-secrets/...`), but the cap this operation gates on is
/// segment-boundary (see `ModelRecord::permits`'s doc on `DuckfsRead`). Passing
/// the gate on `prefix` does not make every hit `grep` returns covered by the
/// cap, so each hit's own path is re-checked against the SAME predicate before
/// it reaches the agent — closing the sibling-path leak without narrowing
/// grep's textual-prefix search for callers that rely on it (the raw
/// `/v1/files/grep` route has no cap at all). `prefix` itself is sent to
/// duckfs UNCHANGED, deliberately: widening it to segment form
/// (`/shared/team` -> `/shared/team/`) would close the sibling-scan at the
/// source, but it would also silently empty a legitimate call whose `prefix`
/// names one exact file rather than a directory (grep only matches a file
/// candidate with `child == prefix || child.starts_with(prefix)`, and no
/// file path ends in `/`). `duckfs_cap`'s two filters already re-check every
/// hit and the resume cursor against the cap regardless of what duckfs scanned,
/// so nothing outside the cap can reach the agent either way — a `next` cursor
/// is a resume path, not a hit.
///
/// They live in `provider-host` rather than here because this is not the only
/// gate in front of the raw route any more: a sandboxed run's node tunnel is a
/// cap-checked read lane (`provider-host`'s `read_lane`) that filters the same
/// replies, and the lane and this tool plane must decide identically.
fn files_grep(run: &Run, target: &Value, input: &Value) -> Result<Value> {
    let prefix = arg_str(target, "prefix")?;
    let pattern = arg_str(input, "pattern")?;
    let record = run.record()?;
    run.permits(&record, &CapRequest::DuckfsRead(&prefix))?;
    let mut reply = run
        .node
        .files("grep", &[("pattern", pattern), ("prefix", prefix)])?;
    duckfs_cap::retain_capped_rows(&record, &mut reply, "hits");
    duckfs_cap::scrub_uncapped_cursor(&record, &mut reply, "hits");
    Ok(reply)
}

fn agent_calls(run: &Run, _target: &Value, _input: &Value) -> Result<Value> {
    let run_id = run.run_id().ok_or_else(|| {
        NodeError::Rejected("this server is not bound to a run, so it has no agent calls".into())
    })?;
    run.node.query(
        TARGET_RUNS,
        encode(&RunsQuery::Delegations {
            caller_run_id: run_id.into(),
        })?,
    )
}

fn gate_forge_read(run: &Run, repo: &str) -> Result<()> {
    let record = run.record()?;
    run.permits(&record, &CapRequest::ForgeRead(repo))
}

fn gate_duckfs_read(run: &Run, path: &str) -> Result<()> {
    let record = run.record()?;
    run.permits(&record, &CapRequest::DuckfsRead(path))
}

fn bounded_list_schema() -> Value {
    let mut value = schema(&[(
        "limit",
        "integer",
        false,
        "Maximum rows to return (default 50, minimum 1, maximum 200).",
    )]);
    value["properties"]["limit"]["minimum"] = json!(1);
    value["properties"]["limit"]["maximum"] = json!(MAX_READ_LIMIT);
    value["properties"]["limit"]["default"] = json!(DEFAULT_READ_LIMIT);
    value["additionalProperties"] = Value::Bool(false);
    value
}

/// the task board's page args. same shape and same 1..=256 bound as the pages
/// reader, but the cursor is a task ID (the last one of the previous page), not
/// a `next_after` the reply carries.
fn tasks_list_schema() -> Value {
    let mut value = schema(&[
        (
            "after",
            "string",
            false,
            "Exclusive cursor: the last task id of the previous page.",
        ),
        (
            "limit",
            "integer",
            false,
            "Tasks to return (default and maximum 256).",
        ),
    ]);
    value["properties"]["limit"]["minimum"] = json!(1);
    value["properties"]["limit"]["maximum"] = json!(tasks::MAX_LIST_LIMIT);
    value["properties"]["limit"]["default"] = json!(tasks::MAX_LIST_LIMIT);
    value["additionalProperties"] = Value::Bool(false);
    value
}

fn page_cursor_schema() -> Value {
    let mut value = schema(&[
        (
            "after",
            "string",
            false,
            "Exclusive cursor from the prior page's next_after.",
        ),
        (
            "limit",
            "integer",
            false,
            "Records to return (default and maximum 256).",
        ),
    ]);
    value["properties"]["limit"]["minimum"] = json!(1);
    value["properties"]["limit"]["maximum"] = json!(pages::MAX_PAGE_QUERY_LIMIT);
    value["properties"]["limit"]["default"] = json!(pages::MAX_PAGE_QUERY_LIMIT);
    value["additionalProperties"] = Value::Bool(false);
    value
}

fn page_cursor(args: &Value) -> Result<Option<String>> {
    match args.get("after") {
        None => Ok(None),
        Some(Value::String(cursor)) => Ok(Some(cursor.clone())),
        Some(_) => Err(NodeError::Rejected(
            "this operation needs a string \"after\" argument".into(),
        )),
    }
}

fn page_limit(args: &Value) -> Result<u16> {
    let Some(value) = args.get("limit") else {
        return Ok(pages::MAX_PAGE_QUERY_LIMIT);
    };
    let limit = value.as_u64().ok_or_else(|| {
        NodeError::Rejected("this operation needs an integer \"limit\" argument".into())
    })?;
    if !(1..=u64::from(pages::MAX_PAGE_QUERY_LIMIT)).contains(&limit) {
        return Err(NodeError::Rejected(format!(
            "this operation needs \"limit\" between 1 and {}",
            pages::MAX_PAGE_QUERY_LIMIT
        )));
    }
    Ok(limit as u16)
}

fn list_limit(args: &Value) -> Result<usize> {
    let object = args
        .as_object()
        .ok_or_else(|| NodeError::Rejected("this operation needs an object input".into()))?;
    if object.keys().any(|key| key != "limit") {
        return Err(NodeError::Rejected(
            "this operation accepts only an optional integer \"limit\" argument".into(),
        ));
    }
    let limit = match object.get("limit") {
        None => DEFAULT_READ_LIMIT,
        Some(value) => value.as_u64().ok_or_else(|| {
            NodeError::Rejected("this operation needs an integer \"limit\" argument".into())
        })?,
    };
    if !(1..=MAX_READ_LIMIT).contains(&limit) {
        return Err(NodeError::Rejected(format!(
            "this operation needs \"limit\" between 1 and {MAX_READ_LIMIT}"
        )));
    }
    Ok(limit as usize)
}

fn reply_array(reply: &Value, name: &str) -> Result<Vec<Value>> {
    reply
        .get(name)
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| NodeError::Transport(format!("module returned no {name:?} array: {reply}")))
}

fn bounded(mut rows: Vec<Value>, limit: usize) -> (Vec<Value>, usize, bool) {
    let total = rows.len();
    rows.truncate(limit);
    (rows, total, total > limit)
}

/// a module's own query enum as the json `/v1/query` carries. the round-trip
/// through `to_value` is what keeps this file honest: the enum, not a string
/// literal here, defines the wire.
fn encode<Q: serde::Serialize>(query: &Q) -> Result<Value> {
    serde_json::to_value(query)
        .map_err(|e| NodeError::Transport(format!("could not encode the query: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queries_encode_to_the_modules_own_wire_shapes() {
        // the guard against this file drifting from the module interfaces: if
        // chat renames a variant, this fails here rather than in front of a
        // model. chat's tools speak the index-tier view wire, so their json
        // literals must DECODE as chat's own view enum.
        serde_json::from_value::<chat::index::ChatViewQuery>(json!({"channels": {}}))
            .expect("the channels view literal is chat's view wire");
        serde_json::from_value::<chat::index::ChatViewQuery>(
            json!({"roots": {"channel_id": "c", "limit": 5}}),
        )
        .expect("the roots view literal is chat's view wire");
        assert_eq!(encode(&ModelQuery::Agents).unwrap(), json!("agents"));
        assert_eq!(
            encode(&RunsQuery::PendingRuns).unwrap(),
            json!("pending_runs")
        );
        assert_eq!(
            encode(&RunsQuery::RecentRuns).unwrap(),
            json!("recent_runs")
        );
        assert_eq!(
            encode(&RunsQuery::Catalog {
                filter: Some("pages.".into())
            })
            .unwrap(),
            json!({"catalog": {"filter": "pages."}})
        );
        assert_eq!(
            encode(&ForgeQuery::PrDiff {
                repo: "app".into(),
                number: 8,
            })
            .unwrap(),
            json!({"pr_diff": {"repo": "app", "number": 8}})
        );
        assert_eq!(
            encode(&WorkQuery::Task(TaskQuery::List {
                limit: 8,
                after: Some("t-3".into()),
            }))
            .unwrap(),
            json!({"task": {"list": {"limit": 8, "after": "t-3"}}})
        );
        serde_json::from_value::<pages::index::PagesViewQuery>(
            json!({"list_pages": {"after": "page-8", "limit": 8}}),
        )
        .expect("the list_pages view literal is pages' view wire");
        assert_eq!(
            encode(&ForgeQuery::GetItem {
                repo: "app".into(),
                number: 7,
            })
            .unwrap(),
            json!({"get_item": {"repo": "app", "number": 7}})
        );
    }

    #[test]
    fn read_operations_are_uniquely_named_and_disjoint_from_the_write_catalog() {
        let ops = read_operations();
        let mut names: Vec<&str> = ops.iter().map(|op| op.name).collect();
        let count = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), count, "read operation names must be unique");
        for op in &ops {
            assert!(!op.description.is_empty(), "{} has no description", op.name);
            assert_eq!(op.input["type"], "object", "{} input is not an object", op.name);
            if let Some(target) = &op.target {
                assert_eq!(target["type"], "object", "{} target is not an object", op.name);
            }
            assert!(
                runs::catalog(None).iter().all(|write| write.name != op.name),
                "{} collides with a write operation",
                op.name
            );
        }
    }

    #[test]
    fn the_query_envelope_is_checked_for_shape_before_any_handler_runs() {
        let run = Run::from_env();
        for (args, needle) in [
            (json!({}), "operation"),
            (json!({"operation": "nope"}), "not a read operation"),
            (json!({"operation": "jobs.get"}), "requires a target"),
            (
                json!({"operation": "chat.channels", "target": {"x": 1}}),
                "takes no target",
            ),
            (
                json!({"operation": "jobs.get", "target": "job-1"}),
                "object \"target\"",
            ),
            (
                json!({"operation": "tasks.list", "input": []}),
                "object \"input\"",
            ),
            (
                json!({"operation": "jobs.get", "target": {}}),
                "job_id",
            ),
        ] {
            let error = query(&run, &args).unwrap_err();
            assert!(
                matches!(&error, NodeError::Rejected(m) if m.contains(needle)),
                "{args} -> {error:?}"
            );
        }
    }

    #[test]
    fn the_message_limit_is_defaulted_and_clamped() {
        let clamp = |v: Value| {
            opt_u64(&v, "limit")
                .unwrap_or(DEFAULT_READ_LIMIT)
                .min(MAX_READ_LIMIT)
        };
        assert_eq!(clamp(json!({})), DEFAULT_READ_LIMIT);
        assert_eq!(clamp(json!({"limit": 10})), 10);
        // a model that asks for the whole channel does not get to blow its own
        // context: the cap is ours, not its.
        assert_eq!(clamp(json!({"limit": 10_000})), MAX_READ_LIMIT);
    }

    #[test]
    fn page_cursor_and_limit_are_bounded() {
        assert_eq!(page_cursor(&json!({})).unwrap(), None);
        assert_eq!(
            page_cursor(&json!({"after": "b8"})).unwrap(),
            Some("b8".into())
        );
        assert!(matches!(
            page_cursor(&json!({"after": 8})),
            Err(NodeError::Rejected(_))
        ));
        assert_eq!(page_limit(&json!({})).unwrap(), pages::MAX_PAGE_QUERY_LIMIT);
        assert_eq!(page_limit(&json!({"limit": 3})).unwrap(), 3);
        assert!(matches!(
            page_limit(&json!({"limit": 0})),
            Err(NodeError::Rejected(_))
        ));
        assert_eq!(page_limit(&json!({"limit": 99})).unwrap(), 99);
        assert!(matches!(
            page_limit(&json!({"limit": 257})),
            Err(NodeError::Rejected(_))
        ));
    }

    #[test]
    fn pages_operations_expose_the_bounded_cursor() {
        for name in ["pages.list", "pages.get"] {
            let op = find_read(name).unwrap();
            assert_eq!(op.input["properties"]["after"]["type"], "string");
            assert_eq!(
                op.input["properties"]["limit"]["maximum"],
                pages::MAX_PAGE_QUERY_LIMIT
            );
            assert_eq!(op.input["additionalProperties"], false);
        }
    }

    #[test]
    fn agent_and_run_inputs_are_exactly_bounded() {
        let expected = json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "integer",
                    "description": "Maximum rows to return (default 50, minimum 1, maximum 200).",
                    "minimum": 1,
                    "maximum": 200,
                    "default": 50,
                }
            },
            "required": [],
            "additionalProperties": false,
        });
        for name in ["agents.list", "runs.list"] {
            let op = find_read(name).unwrap();
            assert_eq!(op.target, None, "{name}");
            assert_eq!(op.input, expected, "{name}");
        }
    }

    #[test]
    fn agent_and_run_limits_reject_bad_arguments_before_querying() {
        let bad = [
            Value::Null,
            json!([]),
            json!("not an object"),
            json!({"other": 1}),
            json!({"limit": "1"}),
            json!({"limit": -1}),
            json!({"limit": 0}),
            json!({"limit": 201}),
        ];
        for input in bad {
            for handler in [
                agents_list as fn(&Run, &Value, &Value) -> Result<Value>,
                runs_list,
            ] {
                assert!(
                    matches!(
                        handler(&Run::from_env(), &Value::Null, &input),
                        Err(NodeError::Rejected(_))
                    ),
                    "accepted {input}"
                );
            }
        }
        assert_eq!(list_limit(&json!({})).unwrap(), 50);
        assert_eq!(list_limit(&json!({"limit": 1})).unwrap(), 1);
        assert_eq!(list_limit(&json!({"limit": 200})).unwrap(), 200);
    }

    #[test]
    fn bounded_rows_preserve_order_and_report_the_full_total() {
        let (rows, total, truncated) = bounded(vec![json!("first"), json!("second")], 1);
        assert_eq!(rows, vec![json!("first")]);
        assert_eq!(total, 2);
        assert!(truncated);
    }

    #[test]
    fn a_missing_required_argument_names_itself() {
        let err = arg_str(&json!({}), "channel_id").unwrap_err();
        assert!(
            matches!(&err, NodeError::Rejected(m) if m.contains("channel_id")),
            "got {err:?}"
        );
    }

    /// a record capped to `duckfs_read = ["/shared/team"]`, shared by the grep
    /// cap tests below.
    fn team_capped_record() -> runs::ModelRecord {
        runs::ModelRecord {
            account: 2,
            agent_id: "bot".into(),
            owner: runs::RunOrigin::External(vec![9; 32]),
            display_name: "BOT".into(),
            capability: "model-1".into(),
            allowed_actions: vec![],
            status: runs::ModelStatus::Active,
            role: runs::ModelRole::General,
            created_at: 0,
            updated_at: 0,
            recipe_hash: vec![],
            caps: runs::ResourceCaps {
                duckfs_read: vec!["/shared/team".into()],
                ..Default::default()
            },
            skills: vec![],
        }
    }

    /// grep's own matcher is a raw string prefix (`/shared/team` also matches
    /// `/shared/team-secrets/...`), but the cap it is gated on is
    /// segment-boundary. A hit from a sibling path that only shares a textual
    /// prefix with the capped one must be dropped before the reply reaches the
    /// agent, while a hit truly under the cap must survive.
    #[test]
    fn grep_hits_outside_the_segment_boundary_cap_are_dropped() {
        let record = team_capped_record();
        let mut reply = json!({
            "hits": [
                {"path": "/shared/team/notes.txt", "line": 1, "text": "ok", "locator": "l1"},
                {"path": "/shared/team-secrets/creds.txt", "line": 1, "text": "aws_secret=x", "locator": "l2"},
            ],
            "next": null,
        });
        duckfs_cap::retain_capped_rows(&record, &mut reply, "hits");
        let paths: Vec<&str> = reply["hits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|h| h["path"].as_str().unwrap())
            .collect();
        assert_eq!(paths, vec!["/shared/team/notes.txt"]);
    }

    /// a `next` cursor naming a path outside the cap (the sibling tree
    /// `/shared/team-secrets/...`, reached because duckfs' grep walk prefixes
    /// on a raw string) must never reach the agent — it is replaced with the
    /// last retained hit's own path so the agent can still resume inside its
    /// cap.
    #[test]
    fn an_uncapped_resume_cursor_is_replaced_by_the_last_retained_hit() {
        let record = team_capped_record();
        let mut reply = json!({
            "hits": [
                {"path": "/shared/team/a.txt", "line": 1, "text": "x", "locator": "l1"},
            ],
            "next": "/shared/team-secrets/creds.txt",
        });
        duckfs_cap::retain_capped_rows(&record, &mut reply, "hits");
        duckfs_cap::scrub_uncapped_cursor(&record, &mut reply, "hits");
        assert_eq!(reply["next"], json!("/shared/team/a.txt"));
    }

    /// same as above but no hit survived the cap filter at all: `next` is
    /// dropped (set to `null`) rather than handed back uncovered.
    #[test]
    fn an_uncapped_resume_cursor_with_no_retained_hits_is_dropped() {
        let record = team_capped_record();
        let mut reply = json!({
            "hits": [
                {"path": "/shared/team-secrets/creds.txt", "line": 1, "text": "aws_secret=x", "locator": "l2"},
            ],
            "next": "/shared/team-secrets/creds.txt",
        });
        duckfs_cap::retain_capped_rows(&record, &mut reply, "hits");
        duckfs_cap::scrub_uncapped_cursor(&record, &mut reply, "hits");
        assert_eq!(reply["hits"].as_array().unwrap().len(), 0);
        assert_eq!(reply["next"], Value::Null);
    }

    /// a resume cursor genuinely inside the cap survives untouched.
    #[test]
    fn a_capped_resume_cursor_survives() {
        let record = team_capped_record();
        let mut reply = json!({
            "hits": [
                {"path": "/shared/team/a.txt", "line": 1, "text": "x", "locator": "l1"},
            ],
            "next": "/shared/team/b.txt",
        });
        duckfs_cap::retain_capped_rows(&record, &mut reply, "hits");
        duckfs_cap::scrub_uncapped_cursor(&record, &mut reply, "hits");
        assert_eq!(reply["next"], json!("/shared/team/b.txt"));
    }

    /// `prefix` is never widened before it reaches duckfs: a cap (and a call)
    /// naming one exact FILE, not a directory, must still see its own hit and
    /// keep a cursor that resumes at that same file — `retain_capped_rows`'s
    /// `p == pre` exact-match arm (mirroring `ModelRecord::permits`) covers
    /// this without any prefix rewriting.
    #[test]
    fn a_cap_naming_one_exact_file_still_sees_its_own_hit_and_cursor() {
        let record = runs::ModelRecord {
            caps: runs::ResourceCaps {
                duckfs_read: vec!["/shared/team/a.txt".into()],
                ..Default::default()
            },
            ..team_capped_record()
        };
        let mut reply = json!({
            "hits": [
                {"path": "/shared/team/a.txt", "line": 1, "text": "x", "locator": "l1"},
            ],
            "next": "/shared/team/a.txt",
        });
        duckfs_cap::retain_capped_rows(&record, &mut reply, "hits");
        duckfs_cap::scrub_uncapped_cursor(&record, &mut reply, "hits");
        assert_eq!(
            reply["hits"].as_array().unwrap().len(),
            1,
            "the exact-file hit must survive"
        );
        assert_eq!(reply["next"], json!("/shared/team/a.txt"));
    }
}
