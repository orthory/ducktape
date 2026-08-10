//! the `tasks` work module -- ONE consensus module hosting TWO boards:
//!
//! * a **task board** (assigned-list kind): ordered human task lists, and
//! * a **job board** (first-claim kind): a consensus-native work board where
//!   exactly one worker claim wins by consensus order.
//!
//! both are QMDB-BACKED: pure logic over a host-injected [`sdk::MerkleStore`]
//! with the shared [`StagedStore`] overlay in front of it. every record is its
//! OWN store key, so an op touches only the keys it names, `root()` is the
//! store's cached merkle root (never a re-serialization of the whole board),
//! and state-sync rides the store's resolver lane rather than a byte snapshot
//! whose size grew with every job ever submitted.
//!
//! ## the key space
//!
//! | logical key | value |
//! |---|---|
//! | `t/{task_id}` | one [`Task`] record (json) |
//! | `t#` | the task-id enumeration index (a json `BTreeSet<String>`) |
//! | `j/{job_id}` | one [`Job`] record (json) |
//! | `j#` | the live job count (u64 LE) -- what [`MAX_JOBS`] is checked against |
//! | `w#` | the registered worker set (a json `BTreeSet<ModuleId>`) |
//!
//! the store hashes each logical key (`sdk::store_key`) and cannot enumerate,
//! so the task board carries `t#`: [`TaskQuery::List`] is an unpaged read of
//! the WHOLE board and something has to hold the id order. the job board needs
//! no such index -- its only dispatch read is the by-id `Get`, and board
//! enumeration is the index guest's job on the derived tier.
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

// re-export both boards' public caps so external callers keep referring to
// `tasks::MAX_PAYLOAD` / `tasks::MAX_TASK_ID`.
pub use job_board::{
    MAX_ATTEMPTS, MAX_JOB_ID, MAX_JOBS, MAX_KIND, MAX_LEASE_VIEWS, MAX_LIST_LIMIT, MAX_PAYLOAD,
    MAX_SPEC, MAX_WORKER_MODULE_ID, MAX_WORKERS, MIN_LEASE_VIEWS,
};
pub use task_board::MAX_TASK_ID;

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

use sdk::{
    Ctx, Error, MerkleStore, Module, ModuleId, Msg, ResolverSyncTarget, StagedStore, StateRoot,
    StateSyncHandle,
};

/// write-time cap on ONE stored record. the concrete store's codec bounds a
/// stored value at 1 MiB AT DECODE TIME (`statesync::qmdb::store_config`): an
/// oversized value would COMMIT fine and then panic every later read on every
/// validator -- a poison pill. the 4 KiB margin below the codec bound covers
/// the serialized operation's framing (32-byte hashed key, varint length
/// prefix, operation tag), exactly as `kv::MAX_VALUE_LEN` reasons.
///
/// this is the ONE guard the storage swap adds: an op frame may carry up to
/// `node::MAX_FRAME_BYTES` (1 MiB + 16 KiB), so an unbounded `title`/`task_id`
/// was reachable and would now poison the store.
pub const MAX_RECORD_BYTES: usize = (1 << 20) - 4 * 1024;

/// a qmdb-backed work module: the task board and the job board over ONE store.
pub struct Tasks {
    id: ModuleId,
    /// the host-injected authenticated store plus this block's staging overlay
    /// (read-your-writes; folded into `root()` at `commit_block`).
    staged: StagedStore,
}

impl Tasks {
    /// wrap the host-constructed store under module identity `id`. sync -- the
    /// store arrives already opened (or already synced to a verified root).
    pub fn new(id: impl Into<ModuleId>, store: Box<dyn MerkleStore>) -> Self {
        Self {
            id: id.into(),
            staged: StagedStore::new(store),
        }
    }
}

/// refuse a value the store's codec would later panic decoding. `what` names
/// the record in the rejection. an op that writes SEVERAL records checks them
/// all before staging any, so a refused op leaves no overlay entry at all.
///
/// that ordering — CHECK everything, THEN stage everything — is a root
/// invariant, not a style preference. natively this `Tasks` keeps `staged`
/// across every dispatch in a block; the wasm guest rebuilds the module per
/// dispatch and flushes its overlay only on a SUCCESSFUL execute. so a path
/// that stages a write and then returns `Err` leaves residue on one side and
/// none on the other, and the two ports diverge on the root. no path does that
/// today (`task_board::create` checks both records before staging either, and
/// every other transition stages last); keep it that way when adding one.
pub(crate) fn check_record(value: &[u8], what: &str) -> Result<(), Error> {
    if value.len() > MAX_RECORD_BYTES {
        return Err(Error::Module(format!(
            "{what} is {} bytes, over the {MAX_RECORD_BYTES}-byte store record cap",
            value.len()
        )));
    }
    Ok(())
}

/// [`check_record`] then stage — the single-record writer's shape.
pub(crate) fn stage_record(
    staged: &mut StagedStore,
    key: Vec<u8>,
    value: Vec<u8>,
    what: &str,
) -> Result<(), Error> {
    check_record(&value, what)?;
    staged.stage(key, value);
    Ok(())
}

#[async_trait::async_trait(?Send)]
impl Module for Tasks {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// the REAL merkle root over all committed records, cached by the store --
    /// never a re-serialization of the boards.
    fn root(&self) -> StateRoot {
        self.staged.root()
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        self.staged.state_sync_handle()
    }

    /// the network state-sync serve lane: answers the shared qmdb wire requests
    /// (historical proof-carrying op ranges) from committed state. read-only.
    async fn serve_sync(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        self.staged.serve_sync(req).await
    }

    async fn resolver_sync_target(&self) -> Result<ResolverSyncTarget, Error> {
        self.staged.sync_target().await
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        match decode_work_msg(&msg.payload).map_err(Error::Module)? {
            WorkMsg::Task(task_msg) => {
                let consensus_time = ctx.env().consensus_time;
                task_board::execute(&mut self.staged, task_msg, consensus_time).await
            }
            WorkMsg::Job(job_msg) => {
                let id = self.id.clone();
                job_board::execute(&mut self.staged, ctx, job_msg, &id).await
            }
        }
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_work_query(req).map_err(Error::Module)? {
            WorkQuery::Task(TaskQuery::List) => Ok(encode_work_reply(&WorkReply::Task(
                task_board::query_list(&self.staged).await?,
            ))),
            WorkQuery::Job(job_query) => Ok(encode_work_reply(&WorkReply::Job(
                job_board::query(&self.staged, job_query).await?,
            ))),
        }
    }

    /// publish the block's staged writes AND deletes in ONE store batch.
    async fn commit_block(&mut self) -> Result<(), Error> {
        self.staged.commit().await
    }

    /// discard the block's staged writes -- nothing reached the store, so
    /// `root()` is unchanged.
    async fn abort_block(&mut self) -> Result<(), Error> {
        self.staged.abort();
        Ok(())
    }
}
