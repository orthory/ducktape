//! tasks' read model: task-board columns AND the job board's enumeration —
//! by-status listing, kind-filtered job pages, and the per-status census.
//!
//! canonical tasks state serves dispatch POINT reads (`TaskQuery::Get` and
//! `JobsQuery::Get`, plus the bounded `TaskQuery::List` page); everything a
//! human enumerates folds here.
//!
//! key spaces (inside tasks' per-module index database):
//! - `task/{task_id}`             — the current [`TaskRow`].
//! - `by-status/{status}/{task_id}` — the SAME row, partitioned by status; a
//!   status change moves the row between partitions in one atomic fold.
//! - `job/{job_id}`               — the current [`JobRow`].
//! - `job-status/{status}/{job_id}` — the SAME row, partitioned by status.
//! - `jobcnt/{status}`            — the per-status census counter (u64 BE),
//!   maintained transition-by-transition so `job_counts` is five point
//!   reads, never a board walk.
//!
//! the fold mirrors module semantics exactly: duplicate creates and updates
//! of unknown tasks ERROR in the module, which aborts the block — an applied
//! op is always a clean create or a real transition.
//!
//! this file is the DECISION core — pure functions over [`StateRead`],
//! compiled natively and unit-tested against a plain map. the wasm shell
//! (`src/index_guest.rs`, feature `index-guest`) wires it into the engine.

use index_guest::{Fail, OpRow, OriginKind, OriginTag, StateRead, Writes};
use serde::{Deserialize, Serialize};

use crate::{JobStatus, JobsMsg, TaskMsg, TaskStatus, WorkMsg, decode_work_msg};

/// default page size for by-status listing (the cap is the scan clamp).
const DEFAULT_LIST_LIMIT: usize = 50;
/// max page size for job listing.
const MAX_JOB_LIST_LIMIT: usize = 256;

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

/// the stored row of one job — the read-model mirror of the board's `Job`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobRow {
    pub job_id: String,
    pub kind: String,
    pub spec: String,
    /// rendered submitter: `user:{id}`, `module:{id}`, or `system`.
    pub submitter: String,
    pub status: JobStatus,
    /// total number of successful claims over this job's life.
    pub attempt: u64,
    /// the winning claim, while `Processing`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim: Option<JobClaimRow>,
    /// the reported outcome, once terminal via `Finalize`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<JobResultRow>,
    pub created_height: u64,
    pub created_at: u64,
    pub updated_height: u64,
    pub updated_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobClaimRow {
    /// rendered worker origin.
    pub worker: String,
    pub claimed_at_height: u64,
    pub lease_views: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobResultRow {
    pub ok: bool,
    pub payload: String,
}

/// the per-status census, five counter reads.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobCountsRow {
    pub pending: u64,
    pub processing: u64,
    pub done: u64,
    pub failed: u64,
    pub cancelled: u64,
}

/// tasks' view requests, externally tagged:
/// `{"by_status": {"status": "open", "after": "...", "limit": 50}}` or
/// `{"task": {"task_id": "..."}}`.
#[derive(Debug, Serialize, Deserialize)]
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
    /// job pages: one status partition (or the whole board), optionally
    /// kind-filtered. the filter applies within the scanned page, so a page
    /// may return fewer than `limit` rows while `has_more` still cursors on.
    Jobs {
        #[serde(default)]
        status: Option<JobStatus>,
        #[serde(default)]
        kind_prefix: Option<String>,
        #[serde(default)]
        after: Option<String>,
        #[serde(default)]
        limit: Option<usize>,
    },
    /// the per-status census.
    JobCounts {},
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
    Jobs {
        jobs: Vec<JobRow>,
        has_more: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        next_after: Option<String>,
    },
    JobCounts(JobCountsRow),
}

fn task_key(id: &str) -> String {
    format!("task/{id}")
}

fn job_key(id: &str) -> String {
    format!("job/{id}")
}

/// the job status partition segment — exhaustive, so a new JobStatus variant
/// breaks THIS crate at compile time instead of silently mis-filing.
fn job_status_key(status: &JobStatus) -> &'static str {
    match status {
        JobStatus::Pending => "pending",
        JobStatus::Processing => "processing",
        JobStatus::Done => "done",
        JobStatus::Failed => "failed",
        JobStatus::Cancelled => "cancelled",
    }
}

fn job_by_status_key(status: &JobStatus, id: &str) -> String {
    format!("job-status/{}/{id}", job_status_key(status))
}

fn job_count_key(status: &JobStatus) -> String {
    format!("jobcnt/{}", job_status_key(status))
}

/// rendered origin for job rows: the submitter/worker identity.
fn render_origin(origin: &OriginTag) -> String {
    let id = origin.id.as_deref().unwrap_or_default();
    match origin.kind {
        OriginKind::Module => format!("module:{id}"),
        OriginKind::External => format!("user:{id}"),
        OriginKind::System => "system".to_string(),
    }
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

/// fold one applied op into derived writes. the "tasks" module hosts both
/// boards on one op stream — each routes to its own fold.
pub fn fold_op(op: &OpRow, read: &impl StateRead) -> Result<Writes, Fail> {
    match decode_work_msg(&op.payload).map_err(|e| Fail::new(FAIL_OP_DECODE, e))? {
        WorkMsg::Task(msg) => fold_task(op, read, msg),
        WorkMsg::Job(msg) => fold_job(op, read, msg),
    }
}

fn fold_task(op: &OpRow, read: &impl StateRead, msg: TaskMsg) -> Result<Writes, Fail> {
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
        TaskMsg::DeleteTask { task_id } => {
            // absent row == the task predates this index; nothing to remove.
            let Some(bytes) = read.get(task_key(&task_id).as_bytes()) else {
                return Ok(out);
            };
            let row = decode_row(&bytes)?;
            index_guest::delete(&mut out, by_status_key(&row.status, &task_id));
            index_guest::delete(&mut out, task_key(&task_id));
        }
    }
    Ok(out)
}

fn decode_job_row(bytes: &[u8]) -> Result<JobRow, Fail> {
    serde_json::from_slice(bytes).map_err(|e| Fail::new(FAIL_ROW_DECODE, e.to_string()))
}

/// stage the two entries one job row materializes to — point lookup +
/// status partition.
fn put_job_row(out: &mut Writes, row: &JobRow) -> Result<(), Fail> {
    let bytes =
        serde_json::to_vec(row).map_err(|e| Fail::new(FAIL_ROW_DECODE, e.to_string()))?;
    index_guest::put(out, job_key(&row.job_id), bytes.clone());
    index_guest::put(out, job_by_status_key(&row.status, &row.job_id), bytes);
    Ok(())
}

fn read_job_count(read: &impl StateRead, status: &JobStatus) -> u64 {
    read.get(job_count_key(status).as_bytes())
        .and_then(|v| <[u8; 8]>::try_from(v.as_slice()).ok())
        .map(u64::from_be_bytes)
        .unwrap_or(0)
}

/// move the census one transition: decrement `from` (if any), increment `to`
/// (if any). saturating on the decrement — a pre-index job's transition can
/// arrive without its submit.
fn move_job_count(
    read: &impl StateRead,
    out: &mut Writes,
    from: Option<&JobStatus>,
    to: Option<&JobStatus>,
) {
    if let Some(from) = from {
        let n = read_job_count(read, from).saturating_sub(1);
        index_guest::put(out, job_count_key(from), n.to_be_bytes().to_vec());
    }
    if let Some(to) = to {
        let n = read_job_count(read, to) + 1;
        index_guest::put(out, job_count_key(to), n.to_be_bytes().to_vec());
    }
}

/// re-file a job row under a new status: partition move + census move + row
/// rewrite, one atomic fold.
fn transition_job(
    read: &impl StateRead,
    out: &mut Writes,
    mut row: JobRow,
    to: JobStatus,
    op: &OpRow,
) -> Result<(), Fail> {
    let from = row.status.clone();
    index_guest::delete(out, job_by_status_key(&from, &row.job_id));
    move_job_count(read, out, Some(&from), Some(&to));
    row.status = to;
    row.updated_height = op.height;
    row.updated_at = op.time;
    put_job_row(out, &row)?;
    Ok(())
}

/// fold one applied job-board op. an applied op passed the board's own
/// transition guards, so arms mirror the transition without re-judging it;
/// a row this index never saw (pre-index board) is a deterministic skip.
fn fold_job(op: &OpRow, read: &impl StateRead, msg: JobsMsg) -> Result<Writes, Fail> {
    let mut out = Writes::new();
    let load = |id: &str| -> Result<Option<JobRow>, Fail> {
        match read.get(job_key(id).as_bytes()) {
            Some(bytes) => decode_job_row(&bytes).map(Some),
            None => Ok(None),
        }
    };
    match msg {
        JobsMsg::Submit { job_id, kind, spec } => {
            let row = JobRow {
                job_id,
                kind,
                spec,
                submitter: render_origin(&op.origin),
                status: JobStatus::Pending,
                attempt: 0,
                claim: None,
                result: None,
                created_height: op.height,
                created_at: op.time,
                updated_height: op.height,
                updated_at: op.time,
            };
            move_job_count(read, &mut out, None, Some(&JobStatus::Pending));
            put_job_row(&mut out, &row)?;
        }
        JobsMsg::Claim {
            job_id,
            lease_views,
        } => {
            let Some(mut row) = load(&job_id)? else {
                return Ok(out);
            };
            row.attempt += 1;
            row.claim = Some(JobClaimRow {
                worker: render_origin(&op.origin),
                claimed_at_height: op.height,
                lease_views,
            });
            transition_job(read, &mut out, row, JobStatus::Processing, op)?;
        }
        JobsMsg::Finalize {
            job_id,
            ok,
            payload,
        } => {
            let Some(mut row) = load(&job_id)? else {
                return Ok(out);
            };
            row.result = Some(JobResultRow { ok, payload });
            row.claim = None;
            let to = if ok { JobStatus::Done } else { JobStatus::Failed };
            transition_job(read, &mut out, row, to, op)?;
        }
        JobsMsg::Release { job_id } | JobsMsg::Reclaim { job_id } => {
            let Some(mut row) = load(&job_id)? else {
                return Ok(out);
            };
            row.claim = None;
            transition_job(read, &mut out, row, JobStatus::Pending, op)?;
        }
        JobsMsg::Cancel { job_id } => {
            let Some(row) = load(&job_id)? else {
                return Ok(out);
            };
            transition_job(read, &mut out, row, JobStatus::Cancelled, op)?;
        }
        JobsMsg::Prune { job_id } => {
            let Some(row) = load(&job_id)? else {
                return Ok(out);
            };
            index_guest::delete(&mut out, job_by_status_key(&row.status, &row.job_id));
            index_guest::delete(&mut out, job_key(&row.job_id));
            move_job_count(read, &mut out, Some(&row.status), None);
        }
        // worker registration is dispatch-plane state (who gets submit
        // notifications) — nothing a human enumerates here.
        JobsMsg::RegisterWorker {} | JobsMsg::UnregisterWorker {} => {}
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
        TasksViewQuery::Jobs {
            status,
            kind_prefix,
            after,
            limit,
        } => {
            let prefix = match &status {
                Some(status) => format!("job-status/{}/", job_status_key(status)),
                None => "job/".to_string(),
            };
            let page = read.scan_page(
                prefix.as_bytes(),
                after.as_deref().map(str::as_bytes),
                limit.unwrap_or(DEFAULT_LIST_LIMIT).clamp(1, MAX_JOB_LIST_LIMIT),
            );
            let mut jobs = Vec::with_capacity(page.entries.len());
            for (_key, value) in &page.entries {
                let row = decode_job_row(value)?;
                let keep = kind_prefix
                    .as_ref()
                    .is_none_or(|prefix| row.kind.starts_with(prefix.as_str()));
                if keep {
                    jobs.push(row);
                }
            }
            TasksViewReply::Jobs {
                jobs,
                has_more: page.has_more,
                next_after: page.next_after,
            }
        }
        TasksViewQuery::JobCounts {} => TasksViewReply::JobCounts(JobCountsRow {
            pending: read_job_count(read, &JobStatus::Pending),
            processing: read_job_count(read, &JobStatus::Processing),
            done: read_job_count(read, &JobStatus::Done),
            failed: read_job_count(read, &JobStatus::Failed),
            cancelled: read_job_count(read, &JobStatus::Cancelled),
        }),
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
            assigned: Vec::new(),
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

    // ---- the job board -------------------------------------------------

    fn job_op(height: u64, origin: OriginTag, msg: &JobsMsg) -> OpRow {
        OpRow {
            height,
            seq: 0,
            time: 1_000 + height,
            origin,
            payload: crate::encode_work_msg(&WorkMsg::Job(msg.clone())),
            assigned: Vec::new(),
        }
    }

    fn fold_job_msg(map: &mut BTreeMap<Vec<u8>, Vec<u8>>, height: u64, msg: &JobsMsg) {
        let writes =
            fold_op(&job_op(height, OriginTag::module("runs"), msg), map).expect("fold");
        apply_to_map(map, writes);
    }

    fn counts(map: &BTreeMap<Vec<u8>, Vec<u8>>) -> JobCountsRow {
        let TasksViewReply::JobCounts(counts) = view(map, serde_json::json!({"job_counts": {}}))
        else {
            panic!("wrong reply shape")
        };
        counts
    }

    fn jobs(map: &BTreeMap<Vec<u8>, Vec<u8>>, req: serde_json::Value) -> Vec<JobRow> {
        let TasksViewReply::Jobs { jobs, .. } = view(map, req) else {
            panic!("wrong reply shape")
        };
        jobs
    }

    #[test]
    fn job_lifecycle_moves_partitions_and_census() {
        let mut map = BTreeMap::new();
        fold_job_msg(
            &mut map,
            1,
            &JobsMsg::Submit {
                job_id: "j1".into(),
                kind: "build/wasm".into(),
                spec: "{}".into(),
            },
        );
        fold_job_msg(
            &mut map,
            2,
            &JobsMsg::Submit {
                job_id: "j2".into(),
                kind: "test/e2e".into(),
                spec: "{}".into(),
            },
        );
        assert_eq!(counts(&map).pending, 2);

        let pending = jobs(&map, serde_json::json!({"jobs": {"status": "pending"}}));
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0].submitter, "module:runs");

        // kind filter applies within the page.
        let builds = jobs(
            &map,
            serde_json::json!({"jobs": {"status": "pending", "kind_prefix": "build/"}}),
        );
        assert_eq!(builds.len(), 1);
        assert_eq!(builds[0].job_id, "j1");

        fold_job_msg(
            &mut map,
            3,
            &JobsMsg::Claim {
                job_id: "j1".into(),
                lease_views: 8,
            },
        );
        let counted = counts(&map);
        assert_eq!((counted.pending, counted.processing), (1, 1));
        let processing = jobs(&map, serde_json::json!({"jobs": {"status": "processing"}}));
        assert_eq!(processing[0].attempt, 1);
        assert_eq!(
            processing[0].claim.as_ref().map(|c| c.lease_views),
            Some(8)
        );

        fold_job_msg(
            &mut map,
            4,
            &JobsMsg::Finalize {
                job_id: "j1".into(),
                ok: true,
                payload: "done".into(),
            },
        );
        let counted = counts(&map);
        assert_eq!((counted.processing, counted.done), (0, 1));
        let done = jobs(&map, serde_json::json!({"jobs": {"status": "done"}}));
        assert!(done[0].claim.is_none(), "a finalized job sheds its claim");
        assert_eq!(done[0].result.as_ref().map(|r| r.ok), Some(true));

        fold_job_msg(&mut map, 5, &JobsMsg::Prune { job_id: "j1".into() });
        assert_eq!(counts(&map).done, 0);
        assert!(jobs(&map, serde_json::json!({"jobs": {}})).len() == 1);

        // release requeues; the whole-board page sees it under job/.
        fold_job_msg(
            &mut map,
            6,
            &JobsMsg::Claim {
                job_id: "j2".into(),
                lease_views: 4,
            },
        );
        fold_job_msg(&mut map, 7, &JobsMsg::Release { job_id: "j2".into() });
        let counted = counts(&map);
        assert_eq!((counted.pending, counted.processing), (1, 0));
        let all = jobs(&map, serde_json::json!({"jobs": {}}));
        assert_eq!(all[0].job_id, "j2");
        assert_eq!(all[0].attempt, 1, "the failed claim still counted");
    }

    #[test]
    fn jobs_pages_clamp_the_limit_and_cursor_in_id_order() {
        let mut map = BTreeMap::new();
        for i in 0..300u64 {
            let kind = if i % 2 == 0 { "build/wasm" } else { "test/e2e" };
            fold_job_msg(
                &mut map,
                1 + i,
                &JobsMsg::Submit {
                    job_id: format!("job-{i:03}"),
                    kind: kind.into(),
                    spec: "{}".into(),
                },
            );
        }

        // an over-ask clamps to MAX_JOB_LIST_LIMIT, ascending id order.
        let TasksViewReply::Jobs {
            jobs,
            has_more,
            next_after,
        } = view(&map, serde_json::json!({"jobs": {"limit": 100_000}}))
        else {
            panic!("wrong reply shape")
        };
        assert_eq!(jobs.len(), MAX_JOB_LIST_LIMIT, "limit clamped to 256");
        assert_eq!(jobs.first().unwrap().job_id, "job-000");
        assert_eq!(jobs.last().unwrap().job_id, "job-255");
        assert!(has_more);

        // the cursor resumes exactly past the clamped page.
        let TasksViewReply::Jobs { jobs, has_more, .. } = view(
            &map,
            serde_json::json!({"jobs": {"limit": 100_000, "after": next_after.unwrap()}}),
        ) else {
            panic!("wrong reply shape")
        };
        assert_eq!(jobs.len(), 44, "the remainder of the 300-job board");
        assert_eq!(jobs.first().unwrap().job_id, "job-256");
        assert!(!has_more);

        // limit 0 clamps up to one row — never an empty stuck page.
        let TasksViewReply::Jobs { jobs, .. } =
            view(&map, serde_json::json!({"jobs": {"limit": 0}}))
        else {
            panic!("wrong reply shape")
        };
        assert_eq!(jobs.len(), 1);

        // the kind filter applies WITHIN the scanned page: a page whose only
        // row fails the filter answers empty while has_more still cursors on.
        let TasksViewReply::Jobs { jobs, has_more, .. } = view(
            &map,
            serde_json::json!({"jobs": {"kind_prefix": "test/", "limit": 1}}),
        ) else {
            panic!("wrong reply shape")
        };
        assert!(jobs.is_empty(), "job-000 is build/, filtered from the page");
        assert!(has_more, "the cursor keeps going past the filtered page");
    }

    #[test]
    fn worker_registration_folds_to_nothing() {
        let map = BTreeMap::new();
        let writes = fold_op(
            &job_op(1, OriginTag::module("runs"), &JobsMsg::RegisterWorker {}),
            &map,
        )
        .expect("fold");
        assert_eq!(writes, Writes::new());
    }
}
