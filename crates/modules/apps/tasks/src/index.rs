//! tasks' materialized view: by-status listing and point lookups.
//!
//! canonical tasks state serves exactly one read — the full unpaged `List`.
//! this mapper folds applied [`TaskMsg`] ops into status-partitioned rows so
//! the board can page one column at a time.
//!
//! key spaces (inside tasks' per-module index database):
//! - `task/{task_id}`             — the current [`TaskRow`].
//! - `by-status/{status}/{task_id}` — the SAME row, partitioned by status; a
//!   status change moves the row between partitions in one atomic fold.
//!
//! the fold mirrors module semantics exactly: duplicate creates and updates
//! of unknown tasks ERROR in the module, which aborts the block — an applied
//! op is always a clean create or a real transition.
//!
//! this file is the DECISION core — pure functions over [`StateRead`],
//! compiled natively and unit-tested against a plain map. the wasm shell
//! (`src/index_guest.rs`, feature `index-guest`) wires it into the engine.

use index_guest::{Fail, OpRow, StateRead, Writes};
use serde::{Deserialize, Serialize};

use crate::{TaskMsg, TaskStatus, WorkMsg, decode_work_msg};

/// default page size for by-status listing (the cap is the scan clamp).
const DEFAULT_LIST_LIMIT: usize = 50;

/// [`Fail`] code: an applied op's payload did not decode — interface drift,
/// which only a refold can honestly repair.
const FAIL_OP_DECODE: i32 = 2;
/// [`Fail`] code: a stored row did not decode — a damaged read model.
const FAIL_ROW_DECODE: i32 = 3;
/// [`Fail`] code: a view request this mapper does not speak.
const FAIL_BAD_REQUEST: i32 = 4;

/// the stored row of one task.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskRow {
    pub task_id: String,
    pub title: String,
    pub status: TaskStatus,
    /// the origin id that created the task (display-grade).
    pub created_by: String,
    pub created_height: u64,
    pub created_at: u64,
    pub updated_height: u64,
    pub updated_at: u64,
}

/// tasks' view requests, externally tagged:
/// `{"by_status": {"status": "open", "after": "...", "limit": 50}}` or
/// `{"task": {"task_id": "..."}}`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TasksViewQuery {
    ByStatus {
        status: TaskStatus,
        #[serde(default)]
        after: Option<String>,
        #[serde(default)]
        limit: Option<usize>,
    },
    Task {
        task_id: String,
    },
}

/// tasks' view replies.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TasksViewReply {
    Tasks {
        tasks: Vec<TaskRow>,
        has_more: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        next_after: Option<String>,
    },
    Task(Option<TaskRow>),
}

fn task_key(id: &str) -> String {
    format!("task/{id}")
}

/// the status partition segment. an exhaustive match, so a new TaskStatus
/// variant breaks THIS crate at compile time instead of silently mis-filing.
fn status_key(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Open => "open",
        TaskStatus::InProgress => "in-progress",
        TaskStatus::Done => "done",
    }
}

fn by_status_key(status: &TaskStatus, id: &str) -> String {
    format!("by-status/{}/{id}", status_key(status))
}

fn encode_row(row: &TaskRow) -> Result<Vec<u8>, Fail> {
    serde_json::to_vec(row).map_err(|e| Fail::new(FAIL_ROW_DECODE, e.to_string()))
}

fn decode_row(bytes: &[u8]) -> Result<TaskRow, Fail> {
    serde_json::from_slice(bytes).map_err(|e| Fail::new(FAIL_ROW_DECODE, e.to_string()))
}

/// stage the two entries one row materializes to — point lookup + status
/// partition — so every write path produces byte-identical rows.
fn put_row(out: &mut Writes, row: &TaskRow) -> Result<(), Fail> {
    let bytes = encode_row(row)?;
    index_guest::put(out, task_key(&row.task_id), bytes.clone());
    index_guest::put(out, by_status_key(&row.status, &row.task_id), bytes);
    Ok(())
}

/// fold one applied op into derived writes.
pub fn fold_op(op: &OpRow, read: &impl StateRead) -> Result<Writes, Fail> {
    // the "tasks" module hosts a job board too; its ops share this op
    // stream. the task index materializes only task-board ops — a job op
    // is a deterministic skip, exactly as jobs carried no index before.
    let WorkMsg::Task(msg) =
        decode_work_msg(&op.payload).map_err(|e| Fail::new(FAIL_OP_DECODE, e))?
    else {
        return Ok(Writes::new());
    };
    let mut out = Writes::new();
    match msg {
        TaskMsg::CreateTask { task_id, title } => put_row(
            &mut out,
            &TaskRow {
                task_id,
                title,
                status: TaskStatus::Open,
                created_by: op.origin.id.clone().unwrap_or_default(),
                created_height: op.height,
                created_at: op.time,
                updated_height: op.height,
                updated_at: op.time,
            },
        )?,
        TaskMsg::UpdateStatus { task_id, status } => {
            // absent row == the task predates this index; nothing to move.
            let Some(bytes) = read.get(task_key(&task_id).as_bytes()) else {
                return Ok(out);
            };
            let mut row = decode_row(&bytes)?;
            index_guest::delete(&mut out, by_status_key(&row.status, &task_id));
            row.status = status;
            row.updated_height = op.height;
            row.updated_at = op.time;
            put_row(&mut out, &row)?;
        }
    }
    Ok(out)
}

/// serve one materialized-view request.
pub fn serve_view(read: &impl StateRead, req: &[u8]) -> Result<Vec<u8>, Fail> {
    let query: TasksViewQuery =
        serde_json::from_slice(req).map_err(|e| Fail::new(FAIL_BAD_REQUEST, e.to_string()))?;
    let reply = match query {
        TasksViewQuery::ByStatus {
            status,
            after,
            limit,
        } => {
            let prefix = format!("by-status/{}/", status_key(&status));
            let page = read.scan_page(
                prefix.as_bytes(),
                after.as_deref().map(str::as_bytes),
                limit.unwrap_or(DEFAULT_LIST_LIMIT),
            );
            let mut tasks = Vec::with_capacity(page.entries.len());
            for (_key, value) in &page.entries {
                tasks.push(decode_row(value)?);
            }
            TasksViewReply::Tasks {
                tasks,
                has_more: page.has_more,
                next_after: page.next_after,
            }
        }
        TasksViewQuery::Task { task_id } => {
            let row = match read.get(task_key(&task_id).as_bytes()) {
                Some(bytes) => Some(decode_row(&bytes)?),
                None => None,
            };
            TasksViewReply::Task(row)
        }
    };
    serde_json::to_vec(&reply).map_err(|e| Fail::new(FAIL_BAD_REQUEST, e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode_task_msg;
    use index_guest::{OriginTag, apply_to_map};
    use std::collections::BTreeMap;

    fn op(height: u64, msg: &TaskMsg) -> OpRow {
        OpRow {
            height,
            seq: 0,
            time: 1_000 + height,
            origin: OriginTag::external("jess"),
            payload: encode_task_msg(msg),
        }
    }

    fn fold(map: &mut BTreeMap<Vec<u8>, Vec<u8>>, height: u64, msg: &TaskMsg) {
        let writes = fold_op(&op(height, msg), map).expect("fold");
        apply_to_map(map, writes);
    }

    fn view(map: &BTreeMap<Vec<u8>, Vec<u8>>, req: serde_json::Value) -> TasksViewReply {
        let bytes = serve_view(map, &serde_json::to_vec(&req).unwrap()).expect("view");
        serde_json::from_slice(&bytes).expect("reply decodes")
    }

    #[test]
    fn create_lists_open_and_status_moves_partitions() {
        let mut map = BTreeMap::new();
        fold(
            &mut map,
            1,
            &TaskMsg::CreateTask {
                task_id: "t1".into(),
                title: "ship the indexer".into(),
            },
        );
        fold(
            &mut map,
            2,
            &TaskMsg::CreateTask {
                task_id: "t2".into(),
                title: "write the spec".into(),
            },
        );

        let TasksViewReply::Tasks { tasks, .. } =
            view(&map, serde_json::json!({"by_status": {"status": "open"}}))
        else {
            panic!("wrong reply shape")
        };
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].created_by, "jess");

        fold(
            &mut map,
            3,
            &TaskMsg::UpdateStatus {
                task_id: "t1".into(),
                status: TaskStatus::Done,
            },
        );

        let TasksViewReply::Tasks { tasks, .. } =
            view(&map, serde_json::json!({"by_status": {"status": "open"}}))
        else {
            panic!("wrong reply shape")
        };
        assert_eq!(tasks.len(), 1, "t1 left the open partition");
        assert_eq!(tasks[0].task_id, "t2");

        let TasksViewReply::Tasks { tasks, .. } =
            view(&map, serde_json::json!({"by_status": {"status": "done"}}))
        else {
            panic!("wrong reply shape")
        };
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].task_id, "t1");
        assert_eq!(tasks[0].updated_height, 3);
    }

    #[test]
    fn point_lookup_and_pagination() {
        let mut map = BTreeMap::new();
        for i in 0..5 {
            fold(
                &mut map,
                1 + i,
                &TaskMsg::CreateTask {
                    task_id: format!("t{i}"),
                    title: format!("task {i}"),
                },
            );
        }

        let TasksViewReply::Task(Some(row)) =
            view(&map, serde_json::json!({"task": {"task_id": "t3"}}))
        else {
            panic!("t3 exists")
        };
        assert_eq!(row.title, "task 3");

        let TasksViewReply::Tasks {
            tasks,
            has_more,
            next_after,
        } = view(
            &map,
            serde_json::json!({"by_status": {"status": "open", "limit": 2}}),
        )
        else {
            panic!("wrong reply shape")
        };
        assert_eq!(tasks.len(), 2);
        assert!(has_more);
        let TasksViewReply::Tasks { tasks, .. } = view(
            &map,
            serde_json::json!({"by_status": {"status": "open", "after": next_after.unwrap()}}),
        ) else {
            panic!("wrong reply shape")
        };
        assert_eq!(tasks.len(), 3);
    }

    #[test]
    fn job_ops_are_a_deterministic_skip() {
        let map = BTreeMap::new();
        let writes = fold_op(
            &op(
                1,
                // a job-board op rides the same stream; the task index skips it.
                &TaskMsg::CreateTask {
                    task_id: "t".into(),
                    title: "t".into(),
                },
            ),
            &map,
        )
        .expect("fold");
        assert!(!writes.is_empty(), "task ops fold");

        let job = OpRow {
            height: 1,
            seq: 1,
            time: 1_001,
            origin: OriginTag::external("jess"),
            payload: crate::encode_work_msg(&WorkMsg::Job(crate::JobsMsg::Submit {
                job_id: "j1".into(),
                kind: "test".into(),
                spec: "{}".into(),
            })),
        };
        assert_eq!(fold_op(&job, &map).expect("skip"), Writes::new());
    }
}
