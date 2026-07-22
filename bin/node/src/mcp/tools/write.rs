//! the write plane: the things an agent may do — now provable, and enforced
//! where it counts.
//!
//! ## every write is one signed action
//!
//! each tool here builds exactly one `agent::AgentAction`, hands it to
//! [`Run::act`], and reports what came back. `act` signs a
//! `RunsMsg::AgentAction` with this run's session key and submits it as an op
//! frame; the runs module then decides, ON EVERY VALIDATOR, whether the agent
//! was allowed to do it.
//!
//! there is deliberately NO permission check in this file. the gate lives in
//! consensus, in `runs`, reusing the same validator the response path uses —
//! one definition of "what an agent may do", checked in one place. a courtesy
//! pre-check here would be a second implementation of that rule, and the two
//! would eventually disagree. when they disagreed, the one that mattered would
//! be the other one.
//!
//! ## what the session key buys
//!
//! a frame's origin is its VERIFIED public key. so an `AgentAction` op is proof
//! that this agent's run made it: consensus can (and does) check that the origin
//! is the session key bound to this run, that the run is still in flight, and
//! that the action fits the agent's committed grant. the write then lands as a
//! MODULE-origin effect, which is what earns it `AuthorRef::Agent` attribution —
//! chat and pages only accept `as_agent` from a module.
//!
//! none of that is available on the frameless lane, where the caller's origin is
//! discarded and the op is re-signed by the node (see `node`'s module doc). that
//! is why this file has no `submit` calls in it at all.
//!
//! ## the action vocabulary is the grant vocabulary
//!
//! the tools map one-for-one onto `agent::KNOWN_ACTIONS`. the tool plane must
//! never become a second, wider set of powers than the one an owner can read off
//! the agent's record and reason about.

use serde_json::{Value, json};

use agent::{AgentAction, MAX_DUCKFS_WRITE_TEXT_BYTES};
use tasks::TaskStatus;

use super::{Tool, arg_bool, arg_str, opt_u64, schema};
use crate::mcp::identity::Run;
use crate::mcp::node::{NodeError, Result};

pub(super) fn tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "ducktape_chat_post",
            description: "Post a message to a chat channel. Use this to report progress or ask a \
                          question while you work — you do not have to save everything for your \
                          final answer. Requires the chat.post_message action.",
            schema: || {
                schema(&[
                    ("channel_id", "string", true, "The channel to post in."),
                    ("text", "string", true, "The message text."),
                    (
                        "thread",
                        "integer",
                        false,
                        "The sequence number of a message to reply to, making this a thread reply.",
                    ),
                ])
            },
            handler: chat_post,
        },
        Tool {
            name: "ducktape_task_create",
            description: "Create a task. Requires the tasks.create action.",
            schema: || schema(&[("title", "string", true, "The task title.")]),
            handler: task_create,
        },
        Tool {
            name: "ducktape_task_status",
            description: "Move a task to open, in_progress, or done. Requires the \
                          tasks.update_status action.",
            schema: || {
                schema(&[
                    ("task_id", "string", true, "The task to move."),
                    ("status", "string", true, "One of: open, in_progress, done."),
                ])
            },
            handler: task_status,
        },
        Tool {
            name: "ducktape_page_comment",
            description: "Comment on a page or on one block of a page. The target is a page id \
                          or a block id from ducktape_page. Requires the pages.comment action \
                          and the owning page in your pages_write caps.",
            schema: || {
                schema(&[
                    (
                        "target",
                        "string",
                        true,
                        "The page id or block id to anchor the comment to.",
                    ),
                    ("text", "string", true, "The comment text."),
                ])
            },
            handler: page_comment,
        },
        Tool {
            name: "ducktape_page_check",
            description: "Tick or untick a todo block on a page. Requires the pages.set_checked \
                          action and the owning page in your pages_write caps.",
            schema: || {
                schema(&[
                    ("block_id", "string", true, "The todo block to flip."),
                    ("checked", "boolean", true, "Whether the todo is done."),
                ])
            },
            handler: page_check,
        },
        Tool {
            name: "ducktape_duckfs_write_text",
            description: "Write one small UTF-8 text file under a granted DuckFS prefix. Requires \
                          the duckfs.write_text action and a duckfs_write prefix containing the \
                          path. The tool fetches the current DuckFS refs head and signs it into \
                          the action as the CAS base.",
            schema: || {
                schema(&[
                    ("path", "string", true, "Absolute DuckFS path to write."),
                    ("text", "string", true, "UTF-8 text content to write."),
                ])
            },
            handler: duckfs_write_text,
        },
    ]
}

fn chat_post(run: &Run, args: &Value) -> Result<Value> {
    run.act(AgentAction::PostMessage {
        channel_id: arg_str(args, "channel_id")?,
        text: arg_str(args, "text")?,
        thread: opt_u64(args, "thread"),
    })
}

fn task_create(run: &Run, args: &Value) -> Result<Value> {
    // the task id is the ONE id the agent supplies: `AgentAction::CreateTask`
    // carries it in the payload (the response path has the model invent it), so
    // unlike the pages/chat ids — which runs derives deterministically in
    // consensus — this one is minted host-side and rides the committed op as
    // plain data. every validator sees the same bytes, so determinism holds.
    let task_id = run.mint("task");
    run.act(AgentAction::CreateTask {
        task_id: task_id.clone(),
        title: arg_str(args, "title")?,
    })?;
    // hand the id back: it is the only thing the agent cannot derive itself, and
    // ducktape_task_status needs it.
    Ok(json!({"task_id": task_id}))
}

fn task_status(run: &Run, args: &Value) -> Result<Value> {
    let status = task_status_of(&arg_str(args, "status")?)?;
    run.act(AgentAction::UpdateTaskStatus {
        task_id: arg_str(args, "task_id")?,
        // the wire name of a `tasks::TaskStatus` — parsed here purely so a
        // near-miss ("Done") is answered with the three real names instead of
        // burning a consensus round-trip to be told the same thing.
        status: status_wire_name(status).to_string(),
    })
}

fn page_comment(run: &Run, args: &Value) -> Result<Value> {
    run.act(AgentAction::AddPageComment {
        target: arg_str(args, "target")?,
        body: arg_str(args, "text")?,
    })
}

fn page_check(run: &Run, args: &Value) -> Result<Value> {
    run.act(AgentAction::SetPageChecked {
        block: arg_str(args, "block_id")?,
        checked: arg_bool(args, "checked")?,
    })
}

fn duckfs_write_text(run: &Run, args: &Value) -> Result<Value> {
    let path = arg_str(args, "path")?;
    let text = arg_str(args, "text")?;
    if text.len() > MAX_DUCKFS_WRITE_TEXT_BYTES {
        return Err(NodeError::Rejected(format!(
            "text is {} bytes; the cap is {MAX_DUCKFS_WRITE_TEXT_BYTES}",
            text.len()
        )));
    }
    let refs = run.node.files("refs", &[])?;
    let base_snapshot = refs
        .get("head")
        .and_then(Value::as_str)
        .map(str::to_string);
    run.act(AgentAction::DuckfsWriteText {
        path,
        text,
        base_snapshot,
    })
}

fn task_status_of(status: &str) -> Result<TaskStatus> {
    match status {
        "open" => Ok(TaskStatus::Open),
        "in_progress" => Ok(TaskStatus::InProgress),
        "done" => Ok(TaskStatus::Done),
        other => Err(NodeError::Rejected(format!(
            "{other:?} is not a task status; use open, in_progress, or done"
        ))),
    }
}

/// the wire name `tasks::TaskStatus` serializes to — the string
/// `AgentAction::UpdateTaskStatus` carries.
fn status_wire_name(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Open => "open",
        TaskStatus::InProgress => "in_progress",
        TaskStatus::Done => "done",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_status_parses_exactly_the_three_wire_names() {
        assert_eq!(task_status_of("open").unwrap(), TaskStatus::Open);
        assert_eq!(task_status_of("in_progress").unwrap(), TaskStatus::InProgress);
        assert_eq!(task_status_of("done").unwrap(), TaskStatus::Done);
        let err = task_status_of("Done").unwrap_err();
        assert!(
            matches!(&err, NodeError::Rejected(m) if m.contains("open, in_progress, or done")),
            "a near-miss must say what the three names are, got {err:?}"
        );
    }

    #[test]
    fn the_status_wire_names_round_trip_through_the_action() {
        // the action carries the status as a STRING, and tasks decodes it back
        // into its own enum. if these two names ever drift, an agent's
        // "done" silently becomes a rejected op — so pin the round trip.
        for (name, status) in [
            ("open", TaskStatus::Open),
            ("in_progress", TaskStatus::InProgress),
            ("done", TaskStatus::Done),
        ] {
            assert_eq!(status_wire_name(task_status_of(name).unwrap()), name);
            assert_eq!(
                serde_json::to_value(status).unwrap(),
                Value::String(name.into()),
                "the tasks module's own wire name must match"
            );
        }
    }

    #[test]
    fn every_write_tool_maps_to_a_known_action() {
        // the invariant the whole plane rests on: the tools are not a second,
        // wider permission vocabulary than the one an owner grants and consensus
        // enforces. every KNOWN_ACTION that an agent can *invoke* has a tool, and
        // every tool names its action so a denied agent can say what it lacks.
        //
        // chat.post is the exception and belongs to no tool: it authorizes the
        // run's REPLY BLOCKS (its final answer), which the runs module posts —
        // not anything an agent calls mid-run. chat.post_message is the tool-side
        // power, and it is deliberately a different grant.
        let described: Vec<&str> = tools().iter().map(|t| t.description).collect();
        for action in agent::KNOWN_ACTIONS {
            if action == agent::ACTION_CHAT_POST {
                continue;
            }
            assert!(
                described.iter().any(|d| d.contains(action)),
                "no write tool is gated on the {action} action — either the tool is missing or \
                 the plane has drifted from KNOWN_ACTIONS"
            );
        }
    }

    #[test]
    fn chat_post_requires_the_wider_grant_not_the_reply_grant() {
        // the escalation guard, asserted at the tool surface: holding chat.post
        // ("you may answer me") must NOT be what unlocks posting into arbitrary
        // channels. that is chat.post_message, and an owner has to grant it.
        let chat = tools()
            .into_iter()
            .find(|t| t.name == "ducktape_chat_post")
            .expect("the chat tool");
        assert!(chat.description.contains(agent::ACTION_CHAT_POST_MESSAGE));
    }
}
