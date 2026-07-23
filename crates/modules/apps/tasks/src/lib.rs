//! the `tasks` work module -- ONE consensus module hosting TWO boards:
//!
//! * a **task board** (assigned-list kind): ordered human task lists, and
//! * a **job board** (first-claim kind): a consensus-native work board where
//!   exactly one worker claim wins by consensus order.
//!
//! both are intentionally state-based rather than qmdb-backed: each needs
//! ordered list/query semantics over a small canonical state. each board stages
//! writes during `execute`, publishes them only at `commit_block`, and the
//! module `root()` hashes the concatenation of the two boards' committed
//! canonical byte streams. `snapshot`/`install` use that exact stream so a
//! joiner can verify a peer-provided image before mutating local state.
//!
//! ops and queries ride ONE wire envelope ([`WorkMsg`]/[`WorkQuery`]): the
//! module's single `execute`/`query` decodes the envelope and routes to the
//! matching board. see `interface` for the wire surface.

// the wire surface: this module's shared types, flattened at the crate root.
mod interface;
pub use interface::*;

// the wasm-guest port: the dispatch shell that adapts this module to the
// ducktape:module world. compiled only by the guest-builder's synthesized
// wasm32 cdylib workspace (feature `guest`), never by the native build.
#[cfg(feature = "guest")]
mod guest;

mod job_board;
mod task_board;

use job_board::JobBoard;
use task_board::TaskBoard;

// re-export the job board's public caps so external callers keep referring to
// `tasks::MAX_PAYLOAD` etc.
pub use job_board::{
    MAX_ATTEMPTS, MAX_JOB_ID, MAX_JOBS, MAX_KIND, MAX_LEASE_VIEWS, MAX_LIST_LIMIT, MAX_PAYLOAD,
    MAX_SPEC, MAX_WORKER_MODULE_ID, MAX_WORKERS, MIN_LEASE_VIEWS,
};

// the derived-tier materialized view over the task board: the PURE decision
// core (fold + view over index_guest::StateRead), compiled everywhere and
// unit-tested natively. the engine shell that runs it inside the module's
// index database is `index_guest` below.
pub mod index;

// the wasm index-mapper shell: wires the pure core into the fluent31 engine.
// compiled only by `guest-builder --index`'s synthesized wasm32 workspace
// (feature `index-guest`), never by the native build.
#[cfg(feature = "index-guest")]
mod index_guest;

use sdk::codec::Cursor;
use sdk::{Ctx, Error, Module, ModuleId, Msg, StateRoot};
use sha2::{Digest, Sha256};

pub struct Tasks {
    id: ModuleId,
    tasks: TaskBoard,
    jobs: JobBoard,
}

impl Tasks {
    pub fn new(id: impl Into<ModuleId>) -> Self {
        Self {
            id: id.into(),
            tasks: TaskBoard::new(),
            jobs: JobBoard::new(),
        }
    }

    /// the canonical committed encoding: the task board's bytes followed by the
    /// job board's bytes. this is the exact `root()` preimage AND the snapshot.
    fn encode_state(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.tasks.encode_committed(&mut out);
        self.jobs.encode_committed(&mut out);
        out
    }

    fn root_of(bytes: &[u8]) -> StateRoot {
        let mut h = Sha256::new();
        h.update(bytes);
        StateRoot(h.finalize().into())
    }

    pub fn snapshot(&self) -> Vec<u8> {
        self.encode_state()
    }

    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        let mut c = Cursor::new(bytes);
        let tasks = TaskBoard::decode_from(&mut c)?;
        let jobs = JobBoard::decode_from(&mut c)?;
        c.finish("work snapshot")?;

        // recompute the combined root over the two boards' committed bytes and
        // reject any image that does not hash to the expected root.
        let mut encoded = Vec::new();
        tasks.encode_committed(&mut encoded);
        jobs.encode_committed(&mut encoded);
        sdk::verify_snapshot_root(Self::root_of(&encoded), expected)?;
        self.tasks = tasks;
        self.jobs = jobs;
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Tasks {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        Self::root_of(&self.encode_state())
    }

    /// advertise the snapshot lane: [`Tasks::snapshot`] is the exact preimage of
    /// `root()`, and [`Tasks::install`] verifies before adopting -- without this
    /// override, sync orchestration saw `Unsupported` and a joiner could not
    /// rebuild the module at all.
    fn snapshot_bytes(&self) -> Option<Vec<u8>> {
        Some(self.snapshot())
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        match decode_work_msg(&msg.payload).map_err(Error::Module)? {
            WorkMsg::Task(task_msg) => self.tasks.execute(task_msg, ctx.env().consensus_time),
            WorkMsg::Job(job_msg) => {
                let id = self.id.clone();
                self.jobs.execute(ctx, job_msg, &id).await
            }
        }
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_work_query(req).map_err(Error::Module)? {
            WorkQuery::Task(TaskQuery::List) => {
                Ok(encode_work_reply(&WorkReply::Task(self.tasks.query_list())))
            }
            WorkQuery::Job(job_query) => Ok(encode_work_reply(&WorkReply::Job(
                self.jobs.query(job_query),
            ))),
        }
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        self.tasks.commit();
        self.jobs.commit();
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.tasks.abort();
        self.jobs.abort();
        Ok(())
    }
}
