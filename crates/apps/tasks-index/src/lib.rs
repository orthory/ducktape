//! tasks' materialized view: by-status listing and point lookups.
//!
//! canonical tasks state serves exactly one read — the full unpaged `List`.
//! this mapper folds applied [`TaskMsg`] ops into status-partitioned rows so
//! the board can page one column at a time.
//!
//! key spaces:
//! - `task/{task_id}`             — the current [`TaskRow`].
//! - `by-status/{status}/{task_id}` — the SAME row, partitioned by status; a
//!   status change moves the row between partitions in one atomic fold.
//!
//! the fold mirrors module semantics exactly: duplicate creates and updates
//! of unknown tasks ERROR in the module, which aborts the block — an applied
//! op is always a clean create or a real transition.

use indexer::{ApplyCtx, Derived, Error, ModuleIndexer, OpMeta, Result, ViewReader};
use serde::{Deserialize, Serialize};
use tasks_interface::{TaskMsg, TaskStatus, decode_msg};

/// default and max page size for by-status listing.
const DEFAULT_LIST_LIMIT: usize = 50;

/// the stored row of one task.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
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
/// `{"byStatus": {"status": "Open", "after": "...", "limit": 50}}` or
/// `{"task": {"taskId": "..."}}`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TasksViewQuery {
    #[serde(rename_all = "camelCase")]
    ByStatus {
        status: TaskStatus,
        #[serde(default)]
        after: Option<String>,
        #[serde(default)]
        limit: Option<usize>,
    },
    #[serde(rename_all = "camelCase")]
    Task { task_id: String },
}

/// tasks' view replies.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TasksViewReply {
    #[serde(rename_all = "camelCase")]
    Tasks {
        tasks: Vec<TaskRow>,
        has_more: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        next_after: Option<String>,
    },
    Task(Option<TaskRow>),
}

pub struct TasksIndex {
    module: String,
}

impl TasksIndex {
    pub fn new(module: impl Into<String>) -> Self {
        Self {
            module: module.into(),
        }
    }
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

fn encode_row(row: &TaskRow) -> Result<Vec<u8>> {
    serde_json::to_vec(row).map_err(|e| Error::Mapper(e.to_string()))
}

fn put_row(out: &mut Derived, row: &TaskRow) -> Result<()> {
    let bytes = encode_row(row)?;
    out.put(task_key(&row.task_id), bytes.clone());
    out.put(by_status_key(&row.status, &row.task_id), bytes);
    Ok(())
}

impl ModuleIndexer for TasksIndex {
    fn module(&self) -> &str {
        &self.module
    }

    fn index_op(
        &self,
        ctx: &ApplyCtx,
        meta: &OpMeta,
        payload: &[u8],
        out: &mut Derived,
    ) -> Result<()> {
        match decode_msg(payload).map_err(Error::Mapper)? {
            TaskMsg::CreateTask { task_id, title } => put_row(
                out,
                &TaskRow {
                    task_id,
                    title,
                    status: TaskStatus::Open,
                    created_by: meta.origin.id.clone().unwrap_or_default(),
                    created_height: meta.height,
                    created_at: meta.time,
                    updated_height: meta.height,
                    updated_at: meta.time,
                },
            ),
            TaskMsg::UpdateStatus { task_id, status } => {
                // absent row == the task predates this index; nothing to move.
                let Some(bytes) = ctx.get(task_key(&task_id).as_bytes())? else {
                    return Ok(());
                };
                let mut row: TaskRow =
                    serde_json::from_slice(&bytes).map_err(|e| Error::Mapper(e.to_string()))?;
                out.delete(by_status_key(&row.status, &task_id));
                row.status = status;
                row.updated_height = meta.height;
                row.updated_at = meta.time;
                put_row(out, &row)
            }
        }
    }

    fn serve_view(&self, reader: &ViewReader, req: &[u8]) -> Result<Vec<u8>> {
        let query: TasksViewQuery =
            serde_json::from_slice(req).map_err(|e| Error::View(e.to_string()))?;
        let reply = match query {
            TasksViewQuery::ByStatus {
                status,
                after,
                limit,
            } => {
                let prefix = format!("by-status/{}/", status_key(&status));
                let page = reader.scan(
                    prefix.as_bytes(),
                    after.as_deref().map(str::as_bytes),
                    limit.unwrap_or(DEFAULT_LIST_LIMIT),
                )?;
                let mut tasks = Vec::with_capacity(page.entries.len());
                for (_key, value) in &page.entries {
                    tasks.push(
                        serde_json::from_slice(value).map_err(|e| Error::Mapper(e.to_string()))?,
                    );
                }
                TasksViewReply::Tasks {
                    tasks,
                    has_more: page.has_more,
                    next_after: page.next_after,
                }
            }
            TasksViewQuery::Task { task_id } => {
                let row = match reader.get(task_key(&task_id).as_bytes())? {
                    Some(bytes) => Some(
                        serde_json::from_slice(&bytes).map_err(|e| Error::Mapper(e.to_string()))?,
                    ),
                    None => None,
                };
                TasksViewReply::Task(row)
            }
        };
        serde_json::to_vec(&reply).map_err(|e| Error::View(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexer::{AppliedOp, BlockOps, IndexStore, OriginTag};
    use tasks_interface::encode_msg;

    fn store(dir: &std::path::Path) -> IndexStore {
        IndexStore::open(dir, &["tasks"])
            .expect("open store")
            .with_indexer(Box::new(TasksIndex::new("tasks")))
    }

    fn op(msg: &TaskMsg) -> AppliedOp {
        AppliedOp {
            module: "tasks".into(),
            origin: OriginTag::external("jess"),
            payload: encode_msg(msg),
        }
    }

    fn apply(store: &IndexStore, height: u64, ops: Vec<AppliedOp>) {
        store
            .apply_block(&BlockOps {
                height,
                time: 1_000 + height,
                ops,
            })
            .expect("apply");
    }

    fn view(store: &IndexStore, req: serde_json::Value) -> TasksViewReply {
        let bytes = store
            .view("tasks", &serde_json::to_vec(&req).unwrap())
            .expect("view");
        serde_json::from_slice(&bytes).expect("reply decodes")
    }

    #[test]
    fn create_lists_open_and_status_moves_partitions() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        apply(
            &store,
            1,
            vec![op(&TaskMsg::CreateTask {
                task_id: "t1".into(),
                title: "ship the indexer".into(),
            })],
        );
        apply(
            &store,
            2,
            vec![op(&TaskMsg::CreateTask {
                task_id: "t2".into(),
                title: "write the spec".into(),
            })],
        );

        let TasksViewReply::Tasks { tasks, .. } =
            view(&store, serde_json::json!({"byStatus": {"status": "Open"}}))
        else {
            panic!("wrong reply shape")
        };
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].created_by, "jess");

        apply(
            &store,
            3,
            vec![op(&TaskMsg::UpdateStatus {
                task_id: "t1".into(),
                status: TaskStatus::Done,
            })],
        );

        let TasksViewReply::Tasks { tasks, .. } =
            view(&store, serde_json::json!({"byStatus": {"status": "Open"}}))
        else {
            panic!("wrong reply shape")
        };
        assert_eq!(tasks.len(), 1, "t1 left the open partition");
        assert_eq!(tasks[0].task_id, "t2");

        let TasksViewReply::Tasks { tasks, .. } =
            view(&store, serde_json::json!({"byStatus": {"status": "Done"}}))
        else {
            panic!("wrong reply shape")
        };
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].task_id, "t1");
        assert_eq!(tasks[0].updated_height, 3);
    }

    #[test]
    fn point_lookup_and_pagination() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        for i in 0..5 {
            apply(
                &store,
                1 + i,
                vec![op(&TaskMsg::CreateTask {
                    task_id: format!("t{i}"),
                    title: format!("task {i}"),
                })],
            );
        }

        let TasksViewReply::Task(Some(row)) =
            view(&store, serde_json::json!({"task": {"taskId": "t3"}}))
        else {
            panic!("t3 exists")
        };
        assert_eq!(row.title, "task 3");

        let TasksViewReply::Tasks {
            tasks,
            has_more,
            next_after,
        } = view(
            &store,
            serde_json::json!({"byStatus": {"status": "Open", "limit": 2}}),
        )
        else {
            panic!("wrong reply shape")
        };
        assert_eq!(tasks.len(), 2);
        assert!(has_more);
        let TasksViewReply::Tasks { tasks, .. } = view(
            &store,
            serde_json::json!({"byStatus": {"status": "Open", "after": next_after.unwrap()}}),
        ) else {
            panic!("wrong reply shape")
        };
        assert_eq!(tasks.len(), 3);
    }
}
