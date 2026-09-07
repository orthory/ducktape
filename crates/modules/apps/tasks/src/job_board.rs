//! the job board (first-claim kind): a deterministic work board over the
//! module's qmdb store.
//!
//! # what the job board is (and what it is NOT)
//!
//! it is a PRODUCT-level work board for on-platform actors: humans and agent
//! fleets submitting ordinary signed ops over RPC. a submitter posts a job; any
//! worker claims it; **exactly one claim wins by consensus order** -- there is
//! no distributed lock because the total order IS the lock (the same first-claim
//! discipline the agent module uses for run-id turn claims). the claimant does
//! the work off-platform and reports a result; result singularity is enforced
//! in-state. jobs are open-ended work items with a human-visible lifecycle:
//! pending -> processing (claimed, leased) -> finalized (done/failed), plus
//! release, permissionless lease-expiry reclaim, cancel, and prune.
//!
//! the job board is DISTINCT from `saga`. `saga` is the platform's async-RPC
//! ledger for REACTOR effects -- off-consensus work the node's own worker loop
//! performs, addressed by module code. the job board never touches saga or the
//! reactor: workers act by submitting ordinary signed ops, and every "identity"
//! (`submitter`, claim `worker`) is derived from the dispatch [`Origin`] inside
//! this board, never read off the wire.
//!
//! # why a losing claim fails the block
//!
//! every transition guard produces an [`Error::Module`] rejection with a precise
//! message, which fails the submitter's block. that is correct and load-bearing:
//! a claim that lost the race must fail loudly and deterministically on every
//! node -- the rejection IS the product's race-resolution signal.
//!
//! # state
//!
//! one record per job (`j/{job_id}`), the live-job census in `j#`, the
//! per-submitter census in `j@{submitter}` (so no ONE account can fill a
//! shared board -- [`MAX_LIVE_JOBS_PER_SUBMITTER`], the task board's
//! `MAX_OPEN_TASKS_PER_OWNER` shape), and the
//! registered worker set in `w#`. a transition reads ONE record, rewrites it,
//! and stages the result -- no board walk, and a `Prune` stages a delete that
//! drops the key (and its bytes) from the root at commit. the census is a
//! counter because [`MAX_JOBS`] is checked per submit and the store cannot
//! enumerate.
//!
//! transition guards read through the staged overlay, so in-block effects (a
//! first claim) are visible to later ops in the same block. the `Get` query
//! answers from COMMITTED state only, so a read never leaks a staged write that
//! a block abort would take back.

use std::collections::BTreeSet;

use sdk::{Ctx, Error, ModuleId, Msg, Origin, StagedStore};
use sha2::{Digest, Sha256};

use crate::{
    Claim, Job, JobResult, JobStatus, JobsEvent, JobsMsg, JobsQuery, JobsReply, Party, controls,
    encode_job_event, stage_record,
};

/// max bytes of a `job_id` (non-empty).
pub const MAX_JOB_ID: usize = 256;
/// max bytes of a `kind` (non-empty).
pub const MAX_KIND: usize = 64;
/// max bytes of a job `spec`.
pub const MAX_SPEC: usize = 64 * 1024;
/// max bytes of a finalize `payload`.
pub const MAX_PAYLOAD: usize = 64 * 1024;
/// max distinct live job ids on the board.
pub const MAX_JOBS: usize = 65536;
/// max live job records ONE submitter may hold at once, well under
/// [`MAX_JOBS`]: no single account can fill a shared board with its own
/// [`MAX_SPEC`]-sized records (at most 64 MiB of spec bytes per submitter,
/// plus bounded record metadata). [`JobsMsg::Prune`] lets a submitter recede.
/// "live" means the same thing the board census means: the RECORD exists.
/// finalizing or cancelling a job does not free the slot, because the record
/// (and its spec bytes) is still on the board -- only a prune drops it.
pub const MAX_LIVE_JOBS_PER_SUBMITTER: usize = 1024;
/// lower clamp for a claim lease, in views.
pub const MIN_LEASE_VIEWS: u64 = 10;
/// upper clamp for a claim lease, in views.
pub const MAX_LEASE_VIEWS: u64 = 10_000;
/// after this many claims, an expired reclaim fails the job instead of requeuing.
pub const MAX_ATTEMPTS: u64 = 8;
/// the result payload a reclaim writes when it gives up on a job. the index
/// mapper folds the same string, so the two tiers cannot drift.
pub const ATTEMPTS_EXHAUSTED_RESULT: &str = "attempts exhausted";
/// max registered worker modules notified on each successful submit.
pub const MAX_WORKERS: usize = 16;
/// max bytes of a worker module id.
pub const MAX_WORKER_MODULE_ID: usize = 256;

/// one job record per id.
const RECORD_PREFIX: &[u8] = b"j/";
/// the live-job census (u64 LE) -- what [`MAX_JOBS`] is checked against.
const COUNT_KEY: &[u8] = b"j#";
/// the per-submitter live-job census (u64 LE), one record per submitter --
/// what [`MAX_LIVE_JOBS_PER_SUBMITTER`] is checked against. a zero count drops
/// the key, the same rule [`stage_count`] follows.
/// The key encodes the stored Party: account keys share one counter; old
/// key-owned records retain theirs after admission, just like their authority.
const SUBMITTER_COUNT_PREFIX: &[u8] = b"j@";
/// the registered worker set (a json `BTreeSet<ModuleId>`, at most
/// [`MAX_WORKERS`] entries).
const WORKERS_KEY: &[u8] = b"w#";

fn record_key(job_id: &str) -> Vec<u8> {
    let mut key = RECORD_PREFIX.to_vec();
    key.extend_from_slice(job_id.as_bytes());
    key
}

fn decode_job(bytes: &[u8]) -> Result<Job, Error> {
    sdk::wire::decode(bytes).map_err(|e| Error::Module(format!("job record decode: {e}")))
}

// ---- overlay-aware reads (execute-internal ONLY) ---------------------------
//
// transition guards must see in-block effects -- a second claim in the same
// block has to observe the first claim's staged `Processing` -- so these read
// through the overlay. the `Get` query does NOT.

/// the live view of a single job, reading through the staged overlay.
pub(crate) async fn load(staged: &StagedStore, job_id: &str) -> Result<Option<Job>, Error> {
    let Some(bytes) = staged.get(&record_key(job_id)).await? else {
        return Ok(None);
    };
    decode_job(&bytes).map(Some)
}

/// the live job or a precise not-found rejection.
async fn require(staged: &StagedStore, job_id: &str) -> Result<Job, Error> {
    load(staged, job_id)
        .await?
        .ok_or_else(|| Error::Module(format!("job not found: {job_id}")))
}

/// one census counter, reading through the staged overlay. an absent key is 0.
async fn read_census(staged: &StagedStore, key: &[u8]) -> Result<u64, Error> {
    let Some(bytes) = staged.get(key).await? else {
        return Ok(0);
    };
    let raw: [u8; 8] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| Error::Module("job census record is not a u64".into()))?;
    Ok(u64::from_le_bytes(raw))
}

/// count of distinct live job ids, reading through the staged overlay.
async fn live_count(staged: &StagedStore) -> Result<u64, Error> {
    read_census(staged, COUNT_KEY).await
}

fn submitter_count_key(submitter: &Party) -> Vec<u8> {
    let mut key = SUBMITTER_COUNT_PREFIX.to_vec();
    key.extend_from_slice(&sdk::wire::encode(submitter));
    key
}

/// count of one submitter's live job records, through the staged overlay.
async fn submitter_count(staged: &StagedStore, submitter: &Party) -> Result<u64, Error> {
    read_census(staged, &submitter_count_key(submitter)).await
}

/// stage a submitter's census (see [`SUBMITTER_COUNT_PREFIX`] on the zero case).
fn stage_submitter_count(staged: &mut StagedStore, submitter: &Party, count: u64) {
    let key = submitter_count_key(submitter);
    if count == 0 {
        staged.delete(key);
        return;
    }
    staged.stage(key, count.to_le_bytes().to_vec());
}

/// Stage the census. An empty board drops the counter entirely; source
/// revision records remain so a recreated job cannot reuse an attribution.
fn stage_count(staged: &mut StagedStore, count: u64) {
    if count == 0 {
        staged.delete(COUNT_KEY.to_vec());
        return;
    }
    staged.stage(COUNT_KEY.to_vec(), count.to_le_bytes().to_vec());
}

/// the registered worker set, reading through the staged overlay. `BTreeSet`
/// serializes ASCENDING, so the record bytes are canonical.
async fn load_workers(staged: &StagedStore) -> Result<BTreeSet<ModuleId>, Error> {
    let Some(bytes) = staged.get(WORKERS_KEY).await? else {
        return Ok(BTreeSet::new());
    };
    sdk::wire::decode(&bytes).map_err(|e| Error::Module(format!("worker set decode: {e}")))
}

/// stage the worker set — an EMPTY set drops the key (see [`stage_count`]).
fn stage_workers(staged: &mut StagedStore, workers: &BTreeSet<ModuleId>) -> Result<(), Error> {
    if workers.is_empty() {
        staged.delete(WORKERS_KEY.to_vec());
        return Ok(());
    }
    stage_record(
        staged,
        WORKERS_KEY.to_vec(),
        sdk::wire::encode(workers),
        "worker set",
    )
}

fn stage_job(staged: &mut StagedStore, job: &Job) -> Result<(), Error> {
    stage_record(
        staged,
        record_key(&job.job_id),
        sdk::wire::encode(job),
        "job record",
    )
}

fn worker_module_from_origin(origin: &Origin, module_id: &ModuleId) -> Result<ModuleId, Error> {
    let Origin::Module(worker) = origin else {
        return Err(Error::Module(
            "worker registration requires a module origin".into(),
        ));
    };
    if worker.is_empty() {
        return Err(Error::Module("worker module_id must not be empty".into()));
    }
    if worker.len() > MAX_WORKER_MODULE_ID {
        return Err(Error::Module(format!(
            "worker module_id exceeds {MAX_WORKER_MODULE_ID} bytes"
        )));
    }
    if worker == module_id {
        return Err(Error::Module(
            "the work module cannot register itself as a worker".into(),
        ));
    }
    Ok(worker.clone())
}

async fn register_worker(
    staged: &mut StagedStore,
    origin: &Origin,
    module_id: &ModuleId,
) -> Result<(), Error> {
    let worker = worker_module_from_origin(origin, module_id)?;
    let mut workers = load_workers(staged).await?;
    if workers.contains(&worker) {
        return Ok(());
    }
    if workers.len() >= MAX_WORKERS {
        return Err(Error::Module("worker cap reached".into()));
    }
    workers.insert(worker);
    stage_workers(staged, &workers)
}

async fn unregister_worker(
    staged: &mut StagedStore,
    origin: &Origin,
    module_id: &ModuleId,
) -> Result<(), Error> {
    let worker = worker_module_from_origin(origin, module_id)?;
    let mut workers = load_workers(staged).await?;
    if !workers.remove(&worker) {
        return Ok(());
    }
    stage_workers(staged, &workers)
}

// ---- transitions (each fails the block on any guard violation) -------------

async fn submit(
    staged: &mut StagedStore,
    job_id: String,
    kind: String,
    spec: String,
    actor: &Party,
    height: u64,
) -> Result<JobsEvent, Error> {
    // enforce every size cap HERE, at execute time, with rejection -- so
    // oversized bytes never reach a committed record (the repo's poison-value
    // lesson).
    if job_id.is_empty() {
        return Err(Error::Module("job_id must not be empty".into()));
    }
    if job_id.len() > MAX_JOB_ID {
        return Err(Error::Module(format!("job_id exceeds {MAX_JOB_ID} bytes")));
    }
    if kind.is_empty() {
        return Err(Error::Module("kind must not be empty".into()));
    }
    if kind.len() > MAX_KIND {
        return Err(Error::Module(format!("kind exceeds {MAX_KIND} bytes")));
    }
    if spec.len() > MAX_SPEC {
        return Err(Error::Module(format!("spec exceeds {MAX_SPEC} bytes")));
    }
    if load(staged, &job_id).await?.is_some() {
        return Err(Error::Module(format!("job already exists: {job_id}")));
    }
    let count = live_count(staged).await?;
    if count >= MAX_JOBS as u64 {
        return Err(Error::Module(format!(
            "job board full: {MAX_JOBS} live jobs"
        )));
    }

    let submitter = actor.clone();
    let submitter_live = submitter_count(staged, &submitter).await?;
    if submitter_live >= MAX_LIVE_JOBS_PER_SUBMITTER as u64 {
        return Err(Error::Module(format!(
            "job submitter at cap: {MAX_LIVE_JOBS_PER_SUBMITTER} live jobs"
        )));
    }
    let spec_hash = Sha256::digest(spec.as_bytes()).to_vec();
    stage_job(
        staged,
        &Job {
            job_id: job_id.clone(),
            kind: kind.clone(),
            spec: spec.clone(),
            submitter: submitter.clone(),
            status: JobStatus::Pending,
            attempt: 0,
            claim: None,
            result: None,
            created_at_height: height,
            updated_at_height: height,
        },
    )?;
    stage_count(staged, count + 1);
    stage_submitter_count(staged, &submitter, submitter_live + 1);
    Ok(JobsEvent::Submitted {
        job_id,
        kind,
        submitter,
        spec,
        spec_hash,
    })
}

async fn claim(
    staged: &mut StagedStore,
    job_id: String,
    lease_views: u64,
    actor: &Party,
    height: u64,
) -> Result<(), Error> {
    let mut job = require(staged, &job_id).await?;
    if job.status != JobStatus::Pending {
        // the consensus order already picked the winner; this op lost the
        // race and fails deterministically on every node.
        return Err(Error::Module(format!(
            "job not claimable (status {:?}): {job_id}",
            job.status
        )));
    }
    let worker = actor.clone();
    job.status = JobStatus::Processing;
    job.attempt = job.attempt.saturating_add(1);
    job.claim = Some(Claim {
        worker,
        claimed_at_height: height,
        lease_views: lease_views.clamp(MIN_LEASE_VIEWS, MAX_LEASE_VIEWS),
    });
    job.updated_at_height = height;
    stage_job(staged, &job)
}

async fn finalize(
    staged: &mut StagedStore,
    job_id: String,
    ok: bool,
    payload: String,
    actor: &Party,
    origin: &Origin,
    height: u64,
) -> Result<(), Error> {
    let mut job = require(staged, &job_id).await?;
    // result singularity: a terminal job is not `Processing`, so this guard
    // rejects any second finalize.
    if job.status != JobStatus::Processing {
        return Err(Error::Module(format!(
            "job not in processing (status {:?}): {job_id}",
            job.status
        )));
    }
    let is_claimant = job
        .claim
        .as_ref()
        .is_some_and(|claim| controls(&claim.worker, actor, origin));
    if !is_claimant {
        return Err(Error::Module(format!(
            "only the current claimant may finalize: {job_id}"
        )));
    }
    if payload.len() > MAX_PAYLOAD {
        return Err(Error::Module(format!(
            "payload exceeds {MAX_PAYLOAD} bytes"
        )));
    }
    job.status = if ok {
        JobStatus::Done
    } else {
        JobStatus::Failed
    };
    job.result = Some(JobResult { ok, payload });
    job.updated_at_height = height;
    stage_job(staged, &job) // claim retained for the record
}

async fn release(
    staged: &mut StagedStore,
    job_id: String,
    actor: &Party,
    origin: &Origin,
    height: u64,
) -> Result<(), Error> {
    let mut job = require(staged, &job_id).await?;
    if job.status != JobStatus::Processing {
        return Err(Error::Module(format!(
            "job not in processing (status {:?}): {job_id}",
            job.status
        )));
    }
    let is_claimant = job
        .claim
        .as_ref()
        .is_some_and(|claim| controls(&claim.worker, actor, origin));
    if !is_claimant {
        return Err(Error::Module(format!(
            "only the current claimant may release: {job_id}"
        )));
    }
    job.status = JobStatus::Pending;
    job.claim = None; // attempt count kept
    job.updated_at_height = height;
    stage_job(staged, &job)
}

async fn reclaim(staged: &mut StagedStore, job_id: String, height: u64) -> Result<(), Error> {
    // PERMISSIONLESS (saga's permissionless-crank pattern): any origin may
    // reclaim, because the ONLY thing that authorizes it is a consensus fact
    // -- a deterministic deadline of heights, identical on every node.
    let mut job = require(staged, &job_id).await?;
    if job.status != JobStatus::Processing {
        return Err(Error::Module(format!(
            "reclaim only applies to processing jobs (status {:?}): {job_id}",
            job.status
        )));
    }
    let claim = job
        .claim
        .as_ref()
        .ok_or_else(|| Error::Module(format!("processing job missing claim: {job_id}")))?;
    let deadline = claim.claimed_at_height.saturating_add(claim.lease_views);
    if height <= deadline {
        return Err(Error::Module(format!(
            "lease not expired (height {height} <= deadline {deadline}): {job_id}"
        )));
    }
    if job.attempt >= MAX_ATTEMPTS {
        // give up: fail the job. the claim is retained for the record, the
        // same way a finalize-to-terminal keeps its claim.
        job.status = JobStatus::Failed;
        job.result = Some(JobResult {
            ok: false,
            payload: ATTEMPTS_EXHAUSTED_RESULT.into(),
        });
    } else {
        job.status = JobStatus::Pending;
        job.claim = None; // attempt count kept for the next claim to bump
    }
    job.updated_at_height = height;
    stage_job(staged, &job)
}

async fn cancel(
    staged: &mut StagedStore,
    job_id: String,
    actor: &Party,
    origin: &Origin,
    height: u64,
) -> Result<(), Error> {
    let mut job = require(staged, &job_id).await?;
    // once claimed, the worker owns it until finalize/release/lease expiry.
    if job.status != JobStatus::Pending {
        return Err(Error::Module(format!(
            "cancel only applies to pending jobs (status {:?}): {job_id}",
            job.status
        )));
    }
    if !controls(&job.submitter, actor, origin) {
        return Err(Error::Module(format!(
            "only the submitter may cancel: {job_id}"
        )));
    }
    job.status = JobStatus::Cancelled;
    job.updated_at_height = height;
    stage_job(staged, &job)
}

async fn prune(
    staged: &mut StagedStore,
    job_id: String,
    actor: &Party,
    origin: &Origin,
) -> Result<(), Error> {
    let job = require(staged, &job_id).await?;
    if !job.status.is_terminal() {
        return Err(Error::Module(format!(
            "prune only applies to terminal jobs (status {:?}): {job_id}",
            job.status
        )));
    }
    if !controls(&job.submitter, actor, origin) {
        return Err(Error::Module(format!(
            "only the submitter may prune: {job_id}"
        )));
    }
    let count = live_count(staged).await?;
    staged.delete(record_key(&job_id));
    stage_count(staged, count.saturating_sub(1));
    // the prune is the ONLY path that frees a board slot -- both censuses
    // recede together, the record's disappearance being what each counts.
    let submitter_live = submitter_count(staged, &job.submitter).await?;
    stage_submitter_count(staged, &job.submitter, submitter_live.saturating_sub(1));
    Ok(())
}

// ---- dispatch --------------------------------------------------------------

pub(crate) async fn execute(
    staged: &mut StagedStore,
    ctx: &mut dyn Ctx,
    msg: JobsMsg,
    actor: &Party,
    module_id: &ModuleId,
) -> Result<(), Error> {
    let env = ctx.env();
    let (origin, height) = (env.origin.clone(), env.height);
    match msg {
        JobsMsg::Submit { job_id, kind, spec } => {
            let workers = load_workers(staged).await?;
            let event = submit(staged, job_id, kind, spec, actor, height).await?;
            for worker in workers {
                ctx.emit_msg(Msg {
                    target: worker,
                    payload: encode_job_event(&event),
                });
            }
            Ok(())
        }
        JobsMsg::Claim {
            job_id,
            lease_views,
        } => claim(staged, job_id, lease_views, actor, height).await,
        JobsMsg::Finalize {
            job_id,
            ok,
            payload,
        } => finalize(staged, job_id, ok, payload, actor, &origin, height).await,
        JobsMsg::Release { job_id } => release(staged, job_id, actor, &origin, height).await,
        JobsMsg::Reclaim { job_id } => reclaim(staged, job_id, height).await,
        JobsMsg::Cancel { job_id } => cancel(staged, job_id, actor, &origin, height).await,
        JobsMsg::Prune { job_id } => prune(staged, job_id, actor, &origin).await,
        JobsMsg::RegisterWorker {} => register_worker(staged, &origin, module_id).await,
        JobsMsg::UnregisterWorker {} => unregister_worker(staged, &origin, module_id).await,
    }
}

/// the read projection answers from COMMITTED state only -- never the staged
/// overlay. a query must not observe a write that a block abort would take
/// back; transition guards keep their own overlay-aware view above.
pub(crate) async fn query(staged: &StagedStore, q: JobsQuery) -> Result<JobsReply, Error> {
    match q {
        JobsQuery::Get { job_id } => {
            let Some(bytes) = staged.get_committed(&record_key(&job_id)).await? else {
                return Ok(JobsReply::Job(None));
            };
            decode_job(&bytes).map(|job| JobsReply::Job(Some(job)))
        }
    }
}
