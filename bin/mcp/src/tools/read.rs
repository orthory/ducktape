//! the read plane: everything an agent could previously only be TOLD, it can
//! now ask for.
//!
//! before this plane existed a run saw exactly what the composer pre-injected
//! into its envelope — the anchored conversation, and a forge item's context if
//! it had one. it could not look up the task it was asked about, read the page
//! it was told to comment on, or open the sibling issue that explains the one it
//! is working. every one of those had to be foreseen in consensus, at compose
//! time, by code that could not know what the agent would want.
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

use agent::CapRequest;
use chat::ChatQuery;
use forge::ForgeQuery;
use pages::PageQuery;
use tasks::TaskQuery;

use super::{Tool, arg_str, opt_u64, schema};
use crate::identity::Run;
use crate::node::{NodeError, Result};

const TARGET_CHAT: &str = "chat";
const TARGET_TASKS: &str = "tasks";
const TARGET_PAGES: &str = "pages";
const TARGET_FORGE: &str = "forge";

/// the newest-messages default: enough to reconstruct a conversation, small
/// enough that a careless call cannot blow the model's context.
const DEFAULT_MESSAGE_LIMIT: u64 = 50;
const MAX_MESSAGE_LIMIT: u64 = 200;

pub(super) fn tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "ducktape_whoami",
            description: "Who you are in Ducktape: your run id, agent id, display name, owner, the \
                          actions you are allowed to take, your resource caps, your workspace \
                          directory, and where your skills are mounted. Call this first if you \
                          are unsure what you are permitted to do — every write tool is gated on \
                          the actions listed here.",
            schema: || schema(&[]),
            handler: whoami,
        },
        Tool {
            name: "ducktape_chat_channels",
            description: "List every chat channel, with its id and name.",
            schema: || schema(&[]),
            handler: chat_channels,
        },
        Tool {
            name: "ducktape_chat_messages",
            description: "Read the most recent messages of a chat channel, oldest first. Use \
                          this to catch up on a conversation you were not anchored in.",
            schema: || {
                schema(&[
                    ("channel_id", "string", true, "The channel to read."),
                    (
                        "limit",
                        "integer",
                        false,
                        "How many of the newest messages to return (default 50, max 200).",
                    ),
                ])
            },
            handler: chat_messages,
        },
        Tool {
            name: "ducktape_tasks",
            description: "List every task with its id, title and status (open, in_progress, \
                          done).",
            schema: || schema(&[]),
            handler: tasks_list,
        },
        Tool {
            name: "ducktape_pages",
            description: "List every page with its id and title.",
            schema: || schema(&[]),
            handler: pages_list,
        },
        Tool {
            name: "ducktape_page",
            description: "Read a whole page as its blocks, in document order. Block ids from \
                          here are what ducktape_page_comment and ducktape_page_check take.",
            schema: || schema(&[("page_id", "string", true, "The page to read.")]),
            handler: page_get,
        },
        Tool {
            name: "ducktape_forge_repos",
            description: "List the forge repos and their current heads.",
            schema: || schema(&[]),
            handler: forge_repos,
        },
        Tool {
            name: "ducktape_forge_items",
            description: "List a forge repo's issues and pull requests. Requires the repo to be \
                          in your forge_read caps.",
            schema: || schema(&[("repo", "string", true, "The forge repo.")]),
            handler: forge_items,
        },
        Tool {
            name: "ducktape_forge_item",
            description: "Read one forge issue or pull request in full — body, branches, \
                          reviews, and the id of its discussion channel (readable with \
                          ducktape_chat_messages). Requires the repo to be in your forge_read \
                          caps.",
            schema: || {
                schema(&[
                    ("repo", "string", true, "The forge repo."),
                    ("number", "integer", true, "The issue or PR number."),
                ])
            },
            handler: forge_item,
        },
        Tool {
            name: "ducktape_forge_pr_diff",
            description: "Read a pull request's exact committed source and target OIDs plus a \
                          bounded unified patch and full diff statistics. The patch is capped at \
                          48 KiB and reports truncation. Fails if the item is not a PR or the \
                          pinned git objects are unavailable locally. Requires the repo to be in \
                          your forge_read caps.",
            schema: || {
                schema(&[
                    ("repo", "string", true, "The forge repo."),
                    ("number", "integer", true, "The pull request number."),
                ])
            },
            handler: forge_pr_diff,
        },
        Tool {
            name: "ducktape_files_ls",
            description: "List a directory in the Ducktape filesystem (duckfs). This is the \
                          shared, replicated filesystem — NOT your local workspace, which you \
                          read with ordinary file tools. Requires the path to be under your \
                          duckfs_read caps.",
            schema: || schema(&[("path", "string", true, "The duckfs directory path.")]),
            handler: files_ls,
        },
        Tool {
            name: "ducktape_files_read",
            description: "Read a file from the Ducktape filesystem (duckfs) as text. Requires \
                          the path to be under your duckfs_read caps.",
            schema: || schema(&[("path", "string", true, "The duckfs file path.")]),
            handler: files_read,
        },
        Tool {
            name: "ducktape_files_grep",
            description: "Search the Ducktape filesystem (duckfs) for matching lines under a \
                          path prefix. Requires the prefix to be under your duckfs_read caps.",
            schema: || {
                schema(&[
                    ("pattern", "string", true, "The pattern to search for."),
                    (
                        "prefix",
                        "string",
                        true,
                        "The duckfs path prefix to search under.",
                    ),
                ])
            },
            handler: files_grep,
        },
    ]
}

/// the agent's own committed record, plus the host facts it cannot read off the
/// chain: its run id, workspace, and skill mount.
fn whoami(run: &Run, _args: &Value) -> Result<Value> {
    let record = run.record()?;
    Ok(json!({
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

fn chat_channels(run: &Run, _args: &Value) -> Result<Value> {
    run.node.query(TARGET_CHAT, encode(&ChatQuery::Channels)?)
}

fn chat_messages(run: &Run, args: &Value) -> Result<Value> {
    let limit = opt_u64(args, "limit")
        .unwrap_or(DEFAULT_MESSAGE_LIMIT)
        .min(MAX_MESSAGE_LIMIT);
    let query = ChatQuery::MessagesLatest {
        channel_id: arg_str(args, "channel_id")?,
        limit,
    };
    run.node.query(TARGET_CHAT, encode(&query)?)
}

fn tasks_list(run: &Run, _args: &Value) -> Result<Value> {
    run.node.query(TARGET_TASKS, encode(&TaskQuery::List)?)
}

fn pages_list(run: &Run, _args: &Value) -> Result<Value> {
    run.node.query(TARGET_PAGES, encode(&PageQuery::ListPages)?)
}

fn page_get(run: &Run, args: &Value) -> Result<Value> {
    let query = PageQuery::GetPage {
        page_id: arg_str(args, "page_id")?,
    };
    run.node.query(TARGET_PAGES, encode(&query)?)
}

fn forge_repos(run: &Run, _args: &Value) -> Result<Value> {
    run.node.query(TARGET_FORGE, encode(&ForgeQuery::ListRepos)?)
}

fn forge_items(run: &Run, args: &Value) -> Result<Value> {
    let repo = arg_str(args, "repo")?;
    gate_forge_read(run, &repo)?;
    let query = ForgeQuery::ListItems { repo };
    run.node.query(TARGET_FORGE, encode(&query)?)
}

fn forge_item(run: &Run, args: &Value) -> Result<Value> {
    let repo = arg_str(args, "repo")?;
    let number = opt_u64(args, "number")
        .ok_or_else(|| NodeError::Rejected("this tool needs an integer \"number\"".into()))?;
    gate_forge_read(run, &repo)?;
    let query = ForgeQuery::GetItem { repo, number };
    run.node.query(TARGET_FORGE, encode(&query)?)
}

fn forge_pr_diff(run: &Run, args: &Value) -> Result<Value> {
    let repo = arg_str(args, "repo")?;
    let number = opt_u64(args, "number")
        .ok_or_else(|| NodeError::Rejected("this tool needs an integer \"number\"".into()))?;
    gate_forge_read(run, &repo)?;
    let query = ForgeQuery::PrDiff { repo, number };
    run.node.query(TARGET_FORGE, encode(&query)?)
}

fn files_ls(run: &Run, args: &Value) -> Result<Value> {
    let path = arg_str(args, "path")?;
    gate_duckfs_read(run, &path)?;
    run.node.files("ls", &[("path", path)])
}

/// duckfs reads come back base64 in `b64`. an agent wants TEXT — hand it the
/// decoded body and say plainly when the bytes are not text, rather than
/// handing a model a base64 blob to decode in its head.
fn files_read(run: &Run, args: &Value) -> Result<Value> {
    let path = arg_str(args, "path")?;
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
            "{path:?} is not utf-8 text ({} bytes); this tool reads text files only",
            e.into_bytes().len()
        ))),
    }
}

fn files_grep(run: &Run, args: &Value) -> Result<Value> {
    let prefix = arg_str(args, "prefix")?;
    let pattern = arg_str(args, "pattern")?;
    gate_duckfs_read(run, &prefix)?;
    run.node
        .files("grep", &[("pattern", pattern), ("prefix", prefix)])
}

fn gate_forge_read(run: &Run, repo: &str) -> Result<()> {
    let record = run.record()?;
    run.permits(&record, &CapRequest::ForgeRead(repo))
}

fn gate_duckfs_read(run: &Run, path: &str) -> Result<()> {
    let record = run.record()?;
    run.permits(&record, &CapRequest::DuckfsRead(path))
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
        // model.
        assert_eq!(encode(&ChatQuery::Channels).unwrap(), json!("channels"));
        assert_eq!(
            encode(&ChatQuery::MessagesLatest {
                channel_id: "c".into(),
                limit: 5,
            })
            .unwrap(),
            json!({"messages_latest": {"channel_id": "c", "limit": 5}})
        );
        assert_eq!(
            encode(&ForgeQuery::PrDiff {
                repo: "app".into(),
                number: 8,
            })
            .unwrap(),
            json!({"pr_diff": {"repo": "app", "number": 8}})
        );
        assert_eq!(encode(&TaskQuery::List).unwrap(), json!("list"));
        assert_eq!(encode(&PageQuery::ListPages).unwrap(), json!("list_pages"));
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
    fn the_message_limit_is_defaulted_and_clamped() {
        let clamp = |v: Value| {
            opt_u64(&v, "limit")
                .unwrap_or(DEFAULT_MESSAGE_LIMIT)
                .min(MAX_MESSAGE_LIMIT)
        };
        assert_eq!(clamp(json!({})), DEFAULT_MESSAGE_LIMIT);
        assert_eq!(clamp(json!({"limit": 10})), 10);
        // a model that asks for the whole channel does not get to blow its own
        // context: the cap is ours, not its.
        assert_eq!(clamp(json!({"limit": 10_000})), MAX_MESSAGE_LIMIT);
    }

    #[test]
    fn a_missing_required_argument_names_itself() {
        let err = arg_str(&json!({}), "channel_id").unwrap_err();
        assert!(
            matches!(&err, NodeError::Rejected(m) if m.contains("channel_id")),
            "got {err:?}"
        );
    }
}
