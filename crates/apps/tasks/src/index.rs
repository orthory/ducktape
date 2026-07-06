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
//!
//! from-state rebuild: canonical `TaskQuery::List` enumerates every task with
//! its status and timestamps, so both key spaces re-derive faithfully. what
//! canonical state does NOT carry is per-op provenance — `created_by` rebuilds
//! empty and the two heights collapse to the boundary; the listing itself
//! (id, title, status, created_at, updated_at) is exact.

use indexer::{
    ApplyCtx, Backfill, Derived, Error, ModuleIndexer, OpMeta, RebuildMeta, Result, StateReader,
    ViewReader,
};
use serde::{Deserialize, Serialize};
use crate::{TaskMsg, TaskQuery, TaskReply, TaskStatus, decode_msg, decode_reply, encode_query};

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
/// `{"byStatus": {"status": "open", "after": "...", "limit": 50}}` or
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

/// the two entries one row materializes to — point lookup + status
/// partition. fold and rebuild both write THROUGH this, so the two paths
/// produce byte-identical rows.
fn row_entries(row: &TaskRow) -> Result<[(String, Vec<u8>); 2]> {
    let bytes = encode_row(row)?;
    Ok([
        (task_key(&row.task_id), bytes.clone()),
        (by_status_key(&row.status, &row.task_id), bytes),
    ])
}

fn put_row(out: &mut Derived, row: &TaskRow) -> Result<()> {
    for (key, value) in row_entries(row)? {
        out.put(key, value);
    }
    Ok(())
}

#[async_trait::async_trait(?Send)]
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

    fn supports_rebuild(&self) -> bool {
        true
    }

    /// re-derive both key spaces from canonical `TaskQuery::List`. the
    /// documented degradation: `created_by` is not canonical state (it only
    /// ever existed in `OpMeta`) and rebuilds empty; both heights collapse to
    /// the boundary. timestamps are canonical and survive exactly.
    async fn rebuild_from_state(
        &self,
        state: &dyn StateReader,
        meta: &RebuildMeta,
        out: &mut Backfill<'_>,
    ) -> Result<()> {
        let reply = state.query(&encode_query(&TaskQuery::List)).await?;
        let TaskReply::Tasks(tasks) = decode_reply(&reply).map_err(Error::State)?;
        for task in tasks {
            let row = TaskRow {
                task_id: task.id,
                title: task.title,
                status: task.status,
                created_by: String::new(),
                created_height: meta.height,
                created_at: task.created_at,
                updated_height: meta.height,
                updated_at: task.updated_at,
            };
            for (key, value) in row_entries(&row)? {
                out.put(key, value)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexer::{AppliedOp, BlockOps, IndexStore, OriginTag};
    use crate::encode_msg;

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
                record: None,
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
            view(&store, serde_json::json!({"byStatus": {"status": "open"}}))
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
            view(&store, serde_json::json!({"byStatus": {"status": "open"}}))
        else {
            panic!("wrong reply shape")
        };
        assert_eq!(tasks.len(), 1, "t1 left the open partition");
        assert_eq!(tasks[0].task_id, "t2");

        let TasksViewReply::Tasks { tasks, .. } =
            view(&store, serde_json::json!({"byStatus": {"status": "done"}}))
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
            serde_json::json!({"byStatus": {"status": "open", "limit": 2}}),
        )
        else {
            panic!("wrong reply shape")
        };
        assert_eq!(tasks.len(), 2);
        assert!(has_more);
        let TasksViewReply::Tasks { tasks, .. } = view(
            &store,
            serde_json::json!({"byStatus": {"status": "open", "after": next_after.unwrap()}}),
        ) else {
            panic!("wrong reply shape")
        };
        assert_eq!(tasks.len(), 3);
    }

    /// canonical tasks state standing in for the module's query surface.
    struct CanonicalTasks(Vec<crate::Task>);

    #[async_trait::async_trait(?Send)]
    impl indexer::StateReader for CanonicalTasks {
        async fn query(&self, req: &[u8]) -> indexer::Result<Vec<u8>> {
            assert!(matches!(
                crate::decode_query(req),
                Ok(TaskQuery::List)
            ));
            Ok(crate::encode_reply(&TaskReply::Tasks(
                self.0.clone(),
            )))
        }
    }

    #[tokio::test]
    async fn rebuild_rederives_partitions_with_boundary_provenance() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        // a folded history whose rows will be thrown away by the rebuild.
        apply(
            &store,
            1,
            vec![op(&TaskMsg::CreateTask {
                task_id: "stale".into(),
                title: "gone after rebuild".into(),
            })],
        );

        let state = CanonicalTasks(vec![
            crate::Task {
                id: "t1".into(),
                title: "ship the indexer".into(),
                status: TaskStatus::Done,
                created_at: 1_001,
                updated_at: 1_003,
            },
            crate::Task {
                id: "t2".into(),
                title: "write the spec".into(),
                status: TaskStatus::Open,
                created_at: 1_002,
                updated_at: 1_002,
            },
        ]);
        let written = store
            .rebuild_module("tasks", &state, indexer::RebuildMeta { height: 40, time: 0 })
            .await
            .expect("rebuild");
        assert_eq!(written, 4, "two rows, two entries each");

        // the stale fold row is gone; both partitions match canonical state.
        let TasksViewReply::Task(row) =
            view(&store, serde_json::json!({"task": {"taskId": "stale"}}))
        else {
            panic!("wrong reply shape")
        };
        assert!(row.is_none(), "pre-rebuild rows do not survive");

        let TasksViewReply::Tasks { tasks, .. } =
            view(&store, serde_json::json!({"byStatus": {"status": "open"}}))
        else {
            panic!("wrong reply shape")
        };
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].task_id, "t2");
        // canonical fields survive exactly; provenance is boundary-stamped.
        assert_eq!(tasks[0].created_at, 1_002);
        assert_eq!(tasks[0].created_by, "");
        assert_eq!(tasks[0].created_height, 40);

        let TasksViewReply::Tasks { tasks, .. } =
            view(&store, serde_json::json!({"byStatus": {"status": "done"}}))
        else {
            panic!("wrong reply shape")
        };
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].task_id, "t1");
        assert_eq!(store.applied_height("tasks").unwrap(), 40);
        assert_eq!(store.backfill_height("tasks").unwrap(), Some(40));

        // the fold continues above the boundary.
        apply(
            &store,
            41,
            vec![op(&TaskMsg::UpdateStatus {
                task_id: "t2".into(),
                status: TaskStatus::InProgress,
            })],
        );
        let TasksViewReply::Tasks { tasks, .. } = view(
            &store,
            serde_json::json!({"byStatus": {"status": "in_progress"}}),
        ) else {
            panic!("wrong reply shape")
        };
        assert_eq!(tasks.len(), 1, "rebuilt rows fold forward like originals");
    }
}
