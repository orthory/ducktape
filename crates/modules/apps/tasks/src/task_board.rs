//! the task board (assigned-list kind): an ordered, deterministic in-memory
//! task list.
//!
//! writes stage during `execute` and publish only at `commit`; the canonical
//! bytes `encode_committed` produces are hashed into the module root. it is a
//! plain shared list -- no claims, no origin-derived identity.

use std::collections::BTreeMap;

use sdk::codec::{self, Cursor};
use sdk::{Error, require_non_empty};

use crate::{Task, TaskMsg, TaskReply, TaskStatus};

#[derive(Default)]
pub(crate) struct TaskBoard {
    tasks: BTreeMap<String, Task>,
    pending: BTreeMap<String, Task>,
}

impl TaskBoard {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    fn get(&self, task_id: &str) -> Option<&Task> {
        self.pending.get(task_id).or_else(|| self.tasks.get(task_id))
    }

    fn list(&self) -> Vec<Task> {
        let mut merged = self.tasks.clone();
        for (id, task) in &self.pending {
            merged.insert(id.clone(), task.clone());
        }
        merged.into_values().collect()
    }

    pub(crate) fn create(
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

    pub(crate) fn update_status(
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

    pub(crate) fn execute(&mut self, msg: TaskMsg, consensus_time: u64) -> Result<(), Error> {
        match msg {
            TaskMsg::CreateTask { task_id, title } => self.create(task_id, title, consensus_time),
            TaskMsg::UpdateStatus { task_id, status } => {
                self.update_status(task_id, status, consensus_time)
            }
        }
    }

    pub(crate) fn query_list(&self) -> TaskReply {
        TaskReply::Tasks(self.list())
    }

    pub(crate) fn commit(&mut self) {
        for (id, task) in std::mem::take(&mut self.pending) {
            self.tasks.insert(id, task);
        }
    }

    pub(crate) fn abort(&mut self) {
        self.pending.clear();
    }

    /// append the canonical committed encoding (count-prefixed, ascending ids).
    pub(crate) fn encode_committed(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&(self.tasks.len() as u64).to_le_bytes());
        for task in self.tasks.values() {
            codec::push_str(out, &task.id);
            codec::push_str(out, &task.title);
            out.push(status_byte(&task.status));
            out.extend_from_slice(&task.created_at.to_le_bytes());
            out.extend_from_slice(&task.updated_at.to_le_bytes());
        }
    }

    /// read this board's portion off a shared cursor. does NOT `finish` the
    /// cursor -- the job board's portion follows in the same snapshot stream.
    pub(crate) fn decode_from(c: &mut Cursor) -> Result<Self, Error> {
        let count = c.u64("snapshot task count")?;
        // each task costs at least 33 bytes (two length prefixes, the status
        // byte, two u64 stamps), bounding a forged count before the loop.
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
            // no updated_at >= created_at check: `update_status` stamps
            // updated_at with the block's consensus_time unconditionally and
            // NOTHING guarantees cross-block monotonicity, so a legitimately
            // committed state can hold updated_at < created_at. install must
            // accept every execute-reachable state -- the root comparison is
            // the integrity check.
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
        Ok(Self {
            tasks,
            pending: BTreeMap::new(),
        })
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
