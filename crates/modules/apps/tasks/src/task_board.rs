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
//! one's write and the unpaged `List` shows this block's staged tasks.

use std::collections::BTreeSet;

use sdk::{Error, StagedStore, require_non_empty};

use crate::{Task, TaskMsg, TaskReply, TaskStatus, check_record, stage_record};

/// max bytes of a `task_id`, matching the job board's [`crate::MAX_JOB_ID`].
///
/// this cap is load-bearing, not cosmetic: every id shares the ONE `t#` index
/// record bounded by [`crate::MAX_RECORD_BYTES`], so an UNCAPPED id let two or
/// three ops (an op frame carries up to `node::MAX_FRAME_BYTES`, 1 MiB + 16 KiB)
/// pack that record to just under the cap and refuse EVERY later create, for
/// every user, forever -- there is no delete op to recover with.
pub const MAX_TASK_ID: usize = 256;

/// one task record per id.
const RECORD_PREFIX: &[u8] = b"t/";
/// the enumeration index: every live task id, ascending.
///
// ponytail: ONE index record holds every task id, so a create is O(all ids) in
// bytes and the board stops where that record hits MAX_RECORD_BYTES (~4k ids at
// the MAX_TASK_ID cap, ~26k at uuid-shaped ids). that stop is PERMANENT, not a
// degradation: there is no delete op, so nothing frees index bytes once they
// are committed. a human task list never gets there; if it must, shard the
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

/// the whole board in ascending id order -- read through the staged overlay,
/// so this block's staged creates and status changes are visible.
pub(crate) async fn query_list(staged: &StagedStore) -> Result<TaskReply, Error> {
    let index = load_index(staged).await?;
    let mut tasks = Vec::with_capacity(index.len());
    for task_id in &index {
        let task = load(staged, task_id).await?.ok_or_else(|| {
            Error::Module(format!("task index names a missing task: {task_id}"))
        })?;
        tasks.push(task);
    }
    Ok(TaskReply::Tasks(tasks))
}
