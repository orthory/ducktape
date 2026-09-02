//! the task board (assigned-list kind): an ordered, deterministic task list
//! over the module's qmdb store.
//!
//! each task is its OWN record (`t/{task_id}`), so a create or a status change
//! touches one key. the store hashes its keys and cannot enumerate, so the
//! ascending id order [`TaskQuery::List`] answers in lives in ONE enumeration
//! index record (`t#`) -- the same shape `pages` uses for its page index.
//!
//! writes stage during `execute` and publish only at `commit_block`; reads go
//! through the staged overlay, so a later op in the same block sees an earlier
//! one's write. BOTH queries read through it -- DELIBERATELY, and unlike the job
//! board's committed-only `Get`: `automations` probes this board mid-block for a
//! duplicate id before emitting its create, so a task staged earlier in the
//! SAME block must be visible or the probe passes and the create aborts the
//! block. do not "harmonize" the two boards' read visibility.
//!
//! every read is BOUNDED: `Get` is one store read, `List` at most
//! [`MAX_LIST_LIMIT`] plus the index. that bound is the whole point -- the wasm
//! host allows 4096 store reads per dispatch, so an unpaged board walk on a
//! consensus caller's execute path (the agent settle in `runs`, the duplicate
//! probe in `automations`) wedged those callers PERMANENTLY once the board
//! outgrew the budget, with no delete op to shrink it back.

use std::collections::BTreeSet;
use std::ops::Bound;

use sdk::{Error, StagedStore, require_non_empty};

use crate::{Task, TaskMsg, TaskQuery, TaskReply, TaskStatus, check_record, stage_record};

/// max bytes of a `task_id`, matching the job board's [`crate::MAX_JOB_ID`].
///
/// this cap is load-bearing, not cosmetic: every id shares the ONE `t#` index
/// record bounded by [`crate::MAX_RECORD_BYTES`], so an UNCAPPED id let two or
/// three ops (an op frame carries up to `node::MAX_FRAME_BYTES`, 1 MiB + 16 KiB)
/// pack that record to just under the cap and refuse EVERY later create, for
/// every user, forever -- there is no delete op to recover with.
pub const MAX_TASK_ID: usize = 256;

/// max distinct tasks on the board, the peer of the job board's
/// [`crate::MAX_JOBS`]. there is no delete op, so this ceiling is reached ONCE
/// and never receded from — which is exactly why it must refuse by name: past
/// it, a create says "task board full" instead of failing whatever opaque
/// byte/budget limit it would otherwise hit first.
pub const MAX_TASKS: usize = 4096;

/// hard clamp on a `List` page, matching the job board's original constant.
pub const MAX_LIST_LIMIT: u64 = 256;

/// one task record per id.
const RECORD_PREFIX: &[u8] = b"t/";
/// the enumeration index: every live task id, ascending.
///
// ponytail: ONE index record holds every task id, so a create is O(all ids) in
// bytes. that is bounded on both axes -- MAX_TASKS ids, each at most
// MAX_TASK_ID bytes -- and whichever of the two guards trips first (the count
// cap here, or MAX_RECORD_BYTES on the index record at long ids) refuses the
// create loudly. a human task list never gets there; if it must, shard the
// index by id prefix BEFORE the board is used at that scale.
const INDEX_KEY: &[u8] = b"t#";

fn record_key(task_id: &str) -> Vec<u8> {
    let mut key = RECORD_PREFIX.to_vec();
    key.extend_from_slice(task_id.as_bytes());
    key
}

/// read one task through the staged overlay (`None` == absent).
async fn load(staged: &StagedStore, task_id: &str) -> Result<Option<Task>, Error> {
    let Some(bytes) = staged.get(&record_key(task_id)).await? else {
        return Ok(None);
    };
    sdk::wire::decode(&bytes)
        .map(Some)
        .map_err(|e| Error::Module(format!("task record decode: {e}")))
}

/// read the enumeration index through the staged overlay. absent reads as the
/// empty set; `BTreeSet` serializes ASCENDING, so the bytes are canonical and
/// every validator commits the same index record.
async fn load_index(staged: &StagedStore) -> Result<BTreeSet<String>, Error> {
    let Some(bytes) = staged.get(INDEX_KEY).await? else {
        return Ok(BTreeSet::new());
    };
    sdk::wire::decode(&bytes).map_err(|e| Error::Module(format!("task index decode: {e}")))
}

async fn create(
    staged: &mut StagedStore,
    task_id: String,
    title: String,
    consensus_time: u64,
) -> Result<(), Error> {
    sdk::validate_id("task_id", &task_id, MAX_TASK_ID)?;
    require_non_empty("title", &title)?;
    if load(staged, &task_id).await?.is_some() {
        return Err(Error::Module(format!("task already exists: {task_id}")));
    }

    let task = Task {
        id: task_id.clone(),
        title,
        status: TaskStatus::Open,
        created_at: consensus_time,
        updated_at: consensus_time,
    };
    let mut index = load_index(staged).await?;
    if index.len() >= MAX_TASKS {
        return Err(Error::Module(format!(
            "task board full: {MAX_TASKS} live tasks"
        )));
    }
    index.insert(task_id.clone());

    // a create writes TWO records; check both BEFORE staging either, so a
    // refusal leaves the overlay untouched (never an index entry naming a task
    // whose record was refused).
    let record = sdk::wire::encode(&task);
    let index_record = sdk::wire::encode(&index);
    check_record(&record, "task record")?;
    check_record(&index_record, "task index")?;
    staged.stage(record_key(&task_id), record);
    staged.stage(INDEX_KEY.to_vec(), index_record);
    Ok(())
}

async fn update_status(
    staged: &mut StagedStore,
    task_id: String,
    status: TaskStatus,
    consensus_time: u64,
) -> Result<(), Error> {
    sdk::validate_id("task_id", &task_id, MAX_TASK_ID)?;
    let mut task = load(staged, &task_id)
        .await?
        .ok_or_else(|| Error::Module(format!("task not found: {task_id}")))?;
    // an accepted no-op: it stages NOTHING, so the block's root holds.
    if task.status == status {
        return Ok(());
    }

    task.status = status;
    task.updated_at = consensus_time;
    stage_record(
        staged,
        record_key(&task_id),
        sdk::wire::encode(&task),
        "task record",
    )
}

pub(crate) async fn execute(
    staged: &mut StagedStore,
    msg: TaskMsg,
    consensus_time: u64,
) -> Result<(), Error> {
    match msg {
        TaskMsg::CreateTask { task_id, title } => {
            create(staged, task_id, title, consensus_time).await
        }
        TaskMsg::UpdateStatus { task_id, status } => {
            update_status(staged, task_id, status, consensus_time).await
        }
    }
}

/// the board's two reads, both through the staged overlay (this block's staged
/// creates and status changes are visible -- see the module docblock).
pub(crate) async fn query(staged: &StagedStore, q: TaskQuery) -> Result<TaskReply, Error> {
    match q {
        TaskQuery::Get { task_id } => get(staged, task_id).await,
        TaskQuery::List { limit, after } => list(staged, limit, after).await,
    }
}

/// ONE task by id — the point read a consensus caller's execute path uses. an
/// absent id is `None`, never an error: "does this id exist" is the question.
async fn get(staged: &StagedStore, task_id: String) -> Result<TaskReply, Error> {
    Ok(TaskReply::Task(load(staged, &task_id).await?))
}

/// ONE page of the board in ascending id order: at most `limit` tasks (clamped
/// into `1..=MAX_LIST_LIMIT`) whose ids sort strictly after `after`. page by
/// handing the last returned id back as the next `after`; a short page ends the
/// board.
async fn list(staged: &StagedStore, limit: u64, after: Option<String>) -> Result<TaskReply, Error> {
    let limit = limit.clamp(1, MAX_LIST_LIMIT) as usize;
    let index = load_index(staged).await?;
    // the cursor is EXCLUSIVE, so a page that ends on an id resumes past it.
    let start = match after.as_deref() {
        Some(cursor) => Bound::Excluded(cursor),
        None => Bound::Unbounded,
    };
    let mut tasks = Vec::with_capacity(limit);
    for task_id in index.range::<str, _>((start, Bound::Unbounded)).take(limit) {
        let task = load(staged, task_id)
            .await?
            .ok_or_else(|| Error::Module(format!("task index names a missing task: {task_id}")))?;
        tasks.push(task);
    }
    Ok(TaskReply::Tasks(tasks))
}

#[cfg(test)]
mod tests {
    use futures::executor::block_on;
    use sdk_testkit::MemStore;

    use super::*;

    fn staged() -> StagedStore {
        StagedStore::new(Box::new(MemStore::new()))
    }

    /// a board AT the cap refuses the next create BY NAME. the full index is
    /// staged directly rather than built by [`MAX_TASKS`] creates: the guard
    /// reads the index, so seeding it is the same input at one encode's cost.
    #[test]
    fn create_refuses_a_full_board() {
        block_on(async {
            let mut staged = staged();
            let index: BTreeSet<String> = (0..MAX_TASKS).map(|n| format!("t{n:06}")).collect();
            staged.stage(INDEX_KEY.to_vec(), sdk::wire::encode(&index));

            let refused = create(&mut staged, "one-more".into(), "over".into(), 1)
                .await
                .expect_err("a full board refuses");
            assert!(
                refused.to_string().contains("task board full"),
                "the refusal names the cap: {refused}"
            );
            // the refusal staged NOTHING: the would-be record is absent.
            assert!(load(&staged, "one-more").await.unwrap().is_none());
        });
    }

    /// the page walks the whole board in ascending id order, `limit` at a time,
    /// resuming EXCLUSIVELY past the id the caller last saw.
    #[test]
    fn list_pages_the_board_by_cursor() {
        block_on(async {
            let mut staged = staged();
            for id in ["a", "b", "c"] {
                create(&mut staged, id.into(), format!("title {id}"), 1)
                    .await
                    .expect("create");
            }

            let TaskReply::Tasks(first) = list(&staged, 2, None).await.unwrap() else {
                panic!("a list answers a page");
            };
            assert_eq!(
                first.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
                ["a", "b"]
            );

            let cursor = first.last().map(|t| t.id.clone());
            let TaskReply::Tasks(second) = list(&staged, 2, cursor).await.unwrap() else {
                panic!("a list answers a page");
            };
            assert_eq!(
                second.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
                ["c"]
            );

            // a limit over the clamp is clamped, never refused.
            let TaskReply::Tasks(all) = list(&staged, u64::MAX, None).await.unwrap() else {
                panic!("a list answers a page");
            };
            assert_eq!(all.len(), 3);
        });
    }
}
