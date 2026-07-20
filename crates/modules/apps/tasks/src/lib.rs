//! deterministic in-memory task module.
//!
//! the first task slice is intentionally state-based rather than qmdb-backed:
//! the API needs ordered list/query semantics and a small canonical state. the
//! module stages writes during `execute`, publishes them only at
//! `commit_block`, and computes `root()` from committed `BTreeMap` contents.
//! `snapshot`/`install` use the exact canonical byte stream that `root()` hashes
//! so a joiner can verify a peer-provided image before mutating local state.

// the wire surface: this module's shared types, flattened at the crate root.
mod interface;
pub use interface::*;
// the derived-tier materialized view; registered only by serving binaries.
// native-only: `indexer` drags unix-only IO, and the index is node-local
// derived state — the wasm guest (`tasks-wasm`) builds without it.
#[cfg(feature = "native")]
pub mod index;

use std::collections::BTreeMap;

use sdk::codec::{self, Cursor};
use sdk::{Ctx, Error, Module, ModuleId, Msg, StateRoot, require_non_empty};
use sha2::{Digest, Sha256};

pub struct Tasks {
    id: ModuleId,
    tasks: BTreeMap<String, Task>,
    pending: BTreeMap<String, Task>,
}

impl Tasks {
    pub fn new(id: impl Into<ModuleId>) -> Self {
        Self {
            id: id.into(),
            tasks: BTreeMap::new(),
            pending: BTreeMap::new(),
        }
    }

    fn get(&self, task_id: &str) -> Option<&Task> {
        self.pending
            .get(task_id)
            .or_else(|| self.tasks.get(task_id))
    }

    fn list(&self) -> Vec<Task> {
        let mut merged = self.tasks.clone();
        for (id, task) in &self.pending {
            merged.insert(id.clone(), task.clone());
        }
        merged.into_values().collect()
    }

    fn stage_create(
        &mut self,
        task_id: String,
        title: String,
        consensus_time: u64,
    ) -> Result<(), Error> {
        require_non_empty("task_id", &task_id)?;
        require_non_empty("title", &title)?;
        if self.get(&task_id).is_some() {
            return Err(Error::Module(format!("task already exists: {task_id}")));
        }

        self.pending.insert(
            task_id.clone(),
            Task {
                id: task_id,
                title,
                status: TaskStatus::Open,
                created_at: consensus_time,
                updated_at: consensus_time,
            },
        );
        Ok(())
    }

    fn stage_status(
        &mut self,
        task_id: String,
        status: TaskStatus,
        consensus_time: u64,
    ) -> Result<(), Error> {
        require_non_empty("task_id", &task_id)?;
        let mut task = self
            .get(&task_id)
            .cloned()
            .ok_or_else(|| Error::Module(format!("task not found: {task_id}")))?;
        if task.status == status {
            return Ok(());
        }

        task.status = status;
        task.updated_at = consensus_time;
        self.pending.insert(task_id, task);
        Ok(())
    }

    fn root_of(tasks: &BTreeMap<String, Task>) -> StateRoot {
        let mut h = Sha256::new();
        h.update(Self::encode_tasks(tasks));
        StateRoot(h.finalize().into())
    }

    fn encode_tasks(tasks: &BTreeMap<String, Task>) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(tasks.len() as u64).to_le_bytes());
        for task in tasks.values() {
            codec::push_str(&mut out, &task.id);
            codec::push_str(&mut out, &task.title);
            out.push(status_byte(&task.status));
            out.extend_from_slice(&task.created_at.to_le_bytes());
            out.extend_from_slice(&task.updated_at.to_le_bytes());
        }
        out
    }

    pub fn snapshot(&self) -> Vec<u8> {
        Self::encode_tasks(&self.tasks)
    }

    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        let tasks = decode_snapshot(bytes)?;
        if Self::root_of(&tasks) != expected {
            return Err(Error::Module("snapshot root mismatch".into()));
        }
        self.tasks = tasks;
        self.pending.clear();
        Ok(())
    }
}

fn status_byte(status: &TaskStatus) -> u8 {
    match status {
        TaskStatus::Open => 0,
        TaskStatus::InProgress => 1,
        TaskStatus::Done => 2,
    }
}

fn status_from_byte(value: u8) -> Result<TaskStatus, Error> {
    match value {
        0 => Ok(TaskStatus::Open),
        1 => Ok(TaskStatus::InProgress),
        2 => Ok(TaskStatus::Done),
        _ => Err(Error::Module("snapshot has invalid task status".into())),
    }
}

fn decode_snapshot(bytes: &[u8]) -> Result<BTreeMap<String, Task>, Error> {
    let mut c = Cursor::new(bytes);
    let count = c.u64("snapshot task count")?;
    // each task costs at least 33 bytes (two length prefixes, the status byte,
    // two u64 stamps), bounding a forged count before the loop.
    c.bound(count, 33, "snapshot task count")?;

    let mut tasks: BTreeMap<String, Task> = BTreeMap::new();
    for _ in 0..count {
        let id = c.string("snapshot task_id")?;
        let title = c.string("snapshot title")?;
        let status = status_from_byte(c.byte("snapshot status")?)?;
        let created_at = c.u64("snapshot created_at")?;
        let updated_at = c.u64("snapshot updated_at")?;

        require_non_empty("task_id", &id)?;
        require_non_empty("title", &title)?;
        // no updated_at >= created_at check: `stage_status` stamps updated_at
        // with the block's consensus_time unconditionally and NOTHING guarantees
        // cross-block monotonicity, so a legitimately committed state can hold
        // updated_at < created_at. install must accept every execute-reachable
        // state — the root comparison is the integrity check.
        if tasks
            .last_key_value()
            .is_some_and(|(last, _)| last.as_str() >= id.as_str())
        {
            return Err(Error::Module(
                "snapshot task ids not strictly ascending".into(),
            ));
        }

        tasks.insert(
            id.clone(),
            Task {
                id,
                title,
                status,
                created_at,
                updated_at,
            },
        );
    }
    c.finish("tasks snapshot")?;
    Ok(tasks)
}

#[async_trait::async_trait(?Send)]
impl Module for Tasks {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        Self::root_of(&self.tasks)
    }

    /// advertise the snapshot lane: [`Tasks::snapshot`] is the exact preimage
    /// of `root()`, and [`Tasks::install`] verifies before adopting — without
    /// this override, sync orchestration saw `Unsupported` and a joiner could
    /// not rebuild the module at all.
    fn state_sync_handle(&self) -> Result<sdk::StateSyncHandle, Error> {
        Ok(sdk::StateSyncHandle::SnapshotBytes(self.snapshot()))
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            TaskMsg::CreateTask { task_id, title } => {
                self.stage_create(task_id, title, ctx.env().consensus_time)
            }
            TaskMsg::UpdateStatus { task_id, status } => {
                self.stage_status(task_id, status, ctx.env().consensus_time)
            }
        }
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            TaskQuery::List => Ok(encode_reply(&TaskReply::Tasks(self.list()))),
        }
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        for (id, task) in std::mem::take(&mut self.pending) {
            self.tasks.insert(id, task);
        }
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending.clear();
        Ok(())
    }
}
