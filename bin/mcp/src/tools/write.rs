//! the write plane: the same five things an agent could already do — now
//! available WHILE it works, not only in the JSON it returns at the end.
//!
//! ## one vocabulary, not two
//!
//! every tool here maps to exactly one `agent::KNOWN_ACTIONS` name, and is
//! gated on it through `Run::authorize`. that is deliberate and load-bearing:
//! the tool plane must not become a second, wider definition of what an agent
//! may do. an owner grants `chat.post` once, and it governs both the response
//! contract's `actions` and this plane's `ducktape_chat_post`. a tool with no
//! action behind it would be a permission nobody could refuse.
//!
//! ## how it differs from the response path, and why that is written down
//!
//! the runs module validates a response's actions IN CONSENSUS and emits them
//! as module-origin effects, which is how they carry `AuthorRef::Agent`
//! attribution. this plane cannot: it is a host-side process, and only a module
//! may refine an origin into an agent author (chat and pages both reject
//! `as_agent` from an external submitter — by design). so a write from here
//! lands under the agent's OWNER, exactly as if the owner had done it in the
//! app, and the gate that authorized it ran here rather than on-chain.
//!
//! see `identity`'s module doc for the full ceiling and its upgrade path. the
//! short version: this opens no hole that the ambient `/v1/submit` route did
//! not already have open, and under codex's network-less sandbox it is a real
//! boundary.
//!
//! ## ids
//!
//! chat messages, tasks, pages threads and pages comments are all
//! CLIENT-minted. `Run::mint` produces them. a squatted id is refused by the
//! module and the refusal reaches the model verbatim.

use serde_json::{Value, json};

use agent::{
    ACTION_CHAT_POST, ACTION_PAGES_COMMENT, ACTION_PAGES_SET_CHECKED, ACTION_TASKS_CREATE,
    ACTION_TASKS_UPDATE_STATUS, CapRequest,
};
use chat::{Block, ChatMsg};
use pages::{PageMsg, PageQuery};
use tasks::{TaskMsg, TaskStatus};

use super::{Tool, arg_bool, arg_str, opt_str, opt_u64, schema};
use crate::identity::Run;
use crate::node::{NodeError, Result};

const TARGET_CHAT: &str = "chat";
const TARGET_TASKS: &str = "tasks";
const TARGET_PAGES: &str = "pages";

pub(super) fn tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "ducktape_chat_post",
            description: "Post a message to a chat channel. Use this to report progress or ask a \
                          question while you work — you do not have to wait for your final \
                          answer. Requires the chat.post action. The message is attributed to \
                          your owner.",
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
                    (
                        "status",
                        "string",
                        true,
                        "One of: open, in_progress, done.",
                    ),
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
    ]
}

fn chat_post(run: &Run, args: &Value) -> Result<Value> {
    let record = run.record()?;
    let origin = run.authorize(&record, ACTION_CHAT_POST, &[])?;
    let msg = ChatMsg::PostMessage {
        channel_id: arg_str(args, "channel_id")?,
        message_id: run.mint("message"),
        blocks: vec![Block::paragraph(arg_str(args, "text")?)],
        thread: opt_u64(args, "thread"),
        // an external submitter setting `as_agent` is REJECTED by chat: only a
        // module origin may refine itself into an agent author. the post is the
        // owner's, and honestly labelled as such.
        as_agent: None,
    };
    submit(run, TARGET_CHAT, &msg, &origin)
}

fn task_create(run: &Run, args: &Value) -> Result<Value> {
    let record = run.record()?;
    let origin = run.authorize(&record, ACTION_TASKS_CREATE, &[])?;
    let task_id = run.mint("task");
    let msg = TaskMsg::CreateTask {
        task_id: task_id.clone(),
        title: arg_str(args, "title")?,
    };
    submit(run, TARGET_TASKS, &msg, &origin)?;
    // the minted id is the only thing the agent cannot derive itself, and it is
    // what every follow-up (ducktape_task_status) needs — so hand it back
    // rather than making the agent re-list the tasks to find what it just made.
    Ok(json!({"task_id": task_id}))
}

fn task_status(run: &Run, args: &Value) -> Result<Value> {
    let record = run.record()?;
    let origin = run.authorize(&record, ACTION_TASKS_UPDATE_STATUS, &[])?;
    let msg = TaskMsg::UpdateStatus {
        task_id: arg_str(args, "task_id")?,
        status: task_status_of(&arg_str(args, "status")?)?,
    };
    submit(run, TARGET_TASKS, &msg, &origin)
}

fn page_comment(run: &Run, args: &Value) -> Result<Value> {
    let record = run.record()?;
    let target = arg_str(args, "target")?;
    // the cap is PAGE-scoped but the target may be a block, so the owning page
    // must be resolved BEFORE the gate can be applied — the same resolution the
    // runs module's pages lane does, for the same reason. a page root is itself
    // a block that names itself as its page, so one lookup covers both shapes.
    let page = owning_page(run, &target)?;
    let origin = run.authorize(
        &record,
        ACTION_PAGES_COMMENT,
        &[CapRequest::PagesWrite(&page)],
    )?;
    let msg = PageMsg::AddComment {
        thread_id: run.mint("thread"),
        comment_id: run.mint("comment"),
        target,
        text: arg_str(args, "text")?,
        // as with chat: an external origin may not claim an agent author.
        as_agent: None,
    };
    submit(run, TARGET_PAGES, &msg, &origin)
}

fn page_check(run: &Run, args: &Value) -> Result<Value> {
    let record = run.record()?;
    let block_id = arg_str(args, "block_id")?;
    let page = owning_page(run, &block_id)?;
    let origin = run.authorize(
        &record,
        ACTION_PAGES_SET_CHECKED,
        &[CapRequest::PagesWrite(&page)],
    )?;
    let msg = PageMsg::SetChecked {
        block_id,
        checked: arg_bool(args, "checked")?,
    };
    submit(run, TARGET_PAGES, &msg, &origin)
}

/// which page a target (a page id or a block id) belongs to — the page-scoped
/// `pages_write` cap needs the page, and the agent only ever has the block.
fn owning_page(run: &Run, target: &str) -> Result<String> {
    let query = PageQuery::GetBlock {
        block_id: target.to_string(),
    };
    let reply = run.node.query(
        TARGET_PAGES,
        serde_json::to_value(&query)
            .map_err(|e| NodeError::Transport(format!("could not encode the query: {e}")))?,
    )?;
    let block = reply.get("block").unwrap_or(&Value::Null);
    if block.is_null() {
        return Err(NodeError::Rejected(format!(
            "no page or block {target:?} exists"
        )));
    }
    opt_str(block, "page").ok_or_else(|| {
        NodeError::Transport(format!(
            "pages returned a block with no owning page: {block}"
        ))
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

/// the one submit funnel. `origin` can only have come from `Run::authorize`,
/// so there is no path to this function that skipped the gate.
fn submit<M: serde::Serialize>(run: &Run, target: &str, msg: &M, origin: &str) -> Result<Value> {
    let payload = serde_json::to_value(msg)
        .map_err(|e| NodeError::Transport(format!("could not encode the op: {e}")))?;
    run.node.submit(target, payload, origin)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ops_encode_to_the_modules_own_wire_shapes() {
        let post = serde_json::to_value(ChatMsg::PostMessage {
            channel_id: "c".into(),
            message_id: "m".into(),
            blocks: vec![Block::paragraph("hi")],
            thread: None,
            as_agent: None,
        })
        .unwrap();
        assert_eq!(post["post_message"]["channel_id"], "c");
        // the agent NEVER claims agent authorship on an external submit — chat
        // would reject it, and the run would lose a write it thought it made.
        assert!(post["post_message"]["as_agent"].is_null());

        let create = serde_json::to_value(TaskMsg::CreateTask {
            task_id: "t".into(),
            title: "title".into(),
        })
        .unwrap();
        assert_eq!(create, json!({"create_task": {"task_id": "t", "title": "title"}}));
    }

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
    fn every_write_tool_maps_to_a_known_action() {
        // the invariant this whole file rests on: the tool plane is not a
        // second, wider permission vocabulary. every write is gated on a name
        // the agent registry already knows and an owner can already grant.
        let described: Vec<&str> = tools().iter().map(|t| t.description).collect();
        for action in agent::KNOWN_ACTIONS {
            assert!(
                described.iter().any(|d| d.contains(action)),
                "no write tool is gated on the {action} action — either the tool is missing or \
                 the plane has drifted from KNOWN_ACTIONS"
            );
        }
    }
}
