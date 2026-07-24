//! the job board (first-claim kind): a deterministic in-memory work board.
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
//! # state and integrity
//!
//! the board is a `BTreeMap<String, Job>` with a staging overlay. writes stage
//! during `execute` and publish only at `commit`; the canonical bytes
//! `encode_committed` produces are hashed into the module root. the overlay is a
//! tombstone overlay (`Option<Job>`: `Some` = upsert, `None` = staged delete) so
//! `Prune` can remove a record atomically with the rest of the block. `decode_from`
//! rejects structurally impossible bytes (non-ascending ids) plus the few
//! execute-UNREACHABLE shapes that would wedge a job (e.g. `Processing` without a
//! claim). everything execute-reachable is accepted -- the root comparison, not
//! an invariant sweep, is the integrity check.
//!
//! queries (`Get`/`List`/`Counts`) answer from COMMITTED state only, never the
//! staged overlay; transition guards keep an overlay-aware view internally so
//! in-block effects (a first claim) are visible to later ops in the same block.

use std::collections::{BTreeMap, BTreeSet};

use sdk::codec::{self, Cursor};
use sdk::{Ctx, Error, ModuleId, Msg, Origin};
use sha2::{Digest, Sha256};

use crate::{
    Claim, Job, JobResult, JobStatus, JobsEvent, JobsMsg, JobsQuery, JobsReply,
    encode_job_event,
};

/// max bytes of a `job_id` (non-empty).
pub const MAX_JOB_ID: usize = 256;
/// max bytes of a `kind` (non-empty).
pub const MAX_KIND: usize = 64;
/// max bytes of a job `spec`.
pub const MAX_SPEC: usize = 64 * 1024;
/// max bytes of a finalize `payload`.
pub const MAX_PAYLOAD: usize = 64 * 1024;
/// max distinct live job ids on the board (overlay-aware).
pub const MAX_JOBS: usize = 65536;
/// lower clamp for a claim lease, in views.
pub const MIN_LEASE_VIEWS: u64 = 10;
/// upper clamp for a claim lease, in views.
pub const MAX_LEASE_VIEWS: u64 = 10_000;
/// after this many claims, an expired reclaim fails the job instead of requeuing.
pub const MAX_ATTEMPTS: u64 = 8;
/// hard clamp on a `List` query's `limit`.
pub const MAX_LIST_LIMIT: u64 = 256;
/// max registered worker modules notified on each successful submit.
pub const MAX_WORKERS: usize = 16;
/// max bytes of a worker module id.
pub const MAX_WORKER_MODULE_ID: usize = 256;

#[derive(Default)]
pub(crate) struct JobBoard {
    /// committed board; the root hashes exactly this map.
    jobs: BTreeMap<String, Job>,
    /// committed worker set; every successful submit fans out to these modules.
    workers: BTreeSet<ModuleId>,
    /// staged overlay: `Some` upserts, `None` is a tombstone (staged delete).
    overlay: BTreeMap<String, Option<Job>>,
    /// staged worker overlay: `true` = present, `false` = absent.
    worker_overlay: BTreeMap<ModuleId, bool>,
}

impl JobBoard {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    // ---- overlay-aware reads (execute-internal ONLY) --------------------------
    //
    // transition guards must see in-block effects -- a second claim in the same
    // block has to observe the first claim's staged `Processing` -- so `get`/
    // `require`/`live_count` read through the overlay. queries do NOT: they
    // answer from COMMITTED state only, so a read never leaks a staged write
    // that a block abort would take back.

    /// the live view of a single job, reading through the staged overlay.
    fn get(&self, job_id: &str) -> Option<&Job> {
        match self.overlay.get(job_id) {
            Some(Some(job)) => Some(job),
            Some(None) => None, // tombstoned this block
            None => self.jobs.get(job_id),
        }
    }

    /// clone the live job or reject with a precise not-found message.
    fn require(&self, job_id: &str) -> Result<Job, Error> {
        self.get(job_id)
            .cloned()
            .ok_or_else(|| Error::Module(format!("job not found: {job_id}")))
    }

    /// count of distinct live job ids, accounting for staged upserts/tombstones.
    fn live_count(&self) -> usize {
        let mut count = self.jobs.len();
        for (id, entry) in &self.overlay {
            match entry {
                Some(_) if !self.jobs.contains_key(id) => count += 1, // new id
                None if self.jobs.contains_key(id) => count -= 1,     // dropped a committed id
                _ => {}
            }
        }
        count
    }

    fn has_worker(&self, module_id: &str) -> bool {
        match self.worker_overlay.get(module_id) {
            Some(present) => *present,
            None => self.workers.contains(module_id),
        }
    }

    fn worker_count(&self) -> usize {
        let mut count = self.workers.len();
        for (module_id, present) in &self.worker_overlay {
            match (*present, self.workers.contains(module_id)) {
                (true, false) => count += 1,
                (false, true) => count -= 1,
                _ => {}
            }
        }
        count
    }

    fn live_workers(&self) -> Vec<ModuleId> {
        self.worker_overlay
            .keys()
            .chain(self.workers.iter())
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter(|module_id| self.has_worker(module_id))
            .collect()
    }

    // ---- overlay-aware writes ------------------------------------------------

    fn stage_upsert(&mut self, job: Job) {
        self.overlay.insert(job.job_id.clone(), Some(job));
    }

    fn stage_remove(&mut self, job_id: &str) {
        self.overlay.insert(job_id.to_owned(), None);
    }

    fn worker_module_from_origin(
        &self,
        origin: &Origin,
        module_id: &ModuleId,
    ) -> Result<ModuleId, Error> {
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

    fn register_worker(&mut self, origin: &Origin, module_id: &ModuleId) -> Result<(), Error> {
        let worker = self.worker_module_from_origin(origin, module_id)?;
        if self.has_worker(&worker) {
            return Ok(());
        }
        if self.worker_count() >= MAX_WORKERS {
            return Err(Error::Module("worker cap reached".into()));
        }
        self.worker_overlay.insert(worker, true);
        Ok(())
    }

    fn unregister_worker(&mut self, origin: &Origin, module_id: &ModuleId) -> Result<(), Error> {
        let worker = self.worker_module_from_origin(origin, module_id)?;
        if !self.has_worker(&worker) {
            return Ok(());
        }
        self.worker_overlay.insert(worker, false);
        Ok(())
    }

    // ---- transitions (each fails the block on any guard violation) -----------

    fn submit(
        &mut self,
        job_id: String,
        kind: String,
        spec: String,
        origin: &Origin,
        height: u64,
    ) -> Result<JobsEvent, Error> {
        // enforce every size cap HERE, at execute time, with rejection -- so
        // oversized bytes never reach the committed map and never enter the
        // root preimage (the repo's poison-value lesson).
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
        if self.get(&job_id).is_some() {
            return Err(Error::Module(format!("job already exists: {job_id}")));
        }
        if self.live_count() >= MAX_JOBS {
            return Err(Error::Module(format!(
                "job board full: {MAX_JOBS} live jobs"
            )));
        }

        let submitter = actor_from_origin(origin)?;
        let spec_hash = Sha256::digest(spec.as_bytes()).to_vec();
        self.stage_upsert(Job {
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
        });
        Ok(JobsEvent::Submitted {
            job_id,
            kind,
            submitter,
            spec,
            spec_hash,
        })
    }

    fn claim(
        &mut self,
        job_id: String,
        lease_views: u64,
        origin: &Origin,
        height: u64,
    ) -> Result<(), Error> {
        let mut job = self.require(&job_id)?;
        if job.status != JobStatus::Pending {
            // the consensus order already picked the winner; this op lost the
            // race and fails deterministically on every node.
            return Err(Error::Module(format!(
                "job not claimable (status {:?}): {job_id}",
                job.status
            )));
        }
        let worker = actor_from_origin(origin)?;
        job.status = JobStatus::Processing;
        job.attempt = job.attempt.saturating_add(1);
        job.claim = Some(Claim {
            worker,
            claimed_at_height: height,
            lease_views: lease_views.clamp(MIN_LEASE_VIEWS, MAX_LEASE_VIEWS),
        });
        job.updated_at_height = height;
        self.stage_upsert(job);
        Ok(())
    }

    fn finalize(
        &mut self,
        job_id: String,
        ok: bool,
        payload: String,
        origin: &Origin,
        height: u64,
    ) -> Result<(), Error> {
        let mut job = self.require(&job_id)?;
        // result singularity: a terminal job is not `Processing`, so this guard
        // rejects any second finalize.
        if job.status != JobStatus::Processing {
            return Err(Error::Module(format!(
                "job not in processing (status {:?}): {job_id}",
                job.status
            )));
        }
        let worker = actor_from_origin(origin)?;
        if job.claim.as_ref().map(|c| c.worker.as_str()) != Some(worker.as_str()) {
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
        self.stage_upsert(job); // claim retained for the record
        Ok(())
    }

    fn release(&mut self, job_id: String, origin: &Origin, height: u64) -> Result<(), Error> {
        let mut job = self.require(&job_id)?;
        if job.status != JobStatus::Processing {
            return Err(Error::Module(format!(
                "job not in processing (status {:?}): {job_id}",
                job.status
            )));
        }
        let worker = actor_from_origin(origin)?;
        if job.claim.as_ref().map(|c| c.worker.as_str()) != Some(worker.as_str()) {
            return Err(Error::Module(format!(
                "only the current claimant may release: {job_id}"
            )));
        }
        job.status = JobStatus::Pending;
        job.claim = None; // attempt count kept
        job.updated_at_height = height;
        self.stage_upsert(job);
        Ok(())
    }

    fn reclaim(&mut self, job_id: String, height: u64) -> Result<(), Error> {
        // PERMISSIONLESS (saga's permissionless-crank pattern): any origin may
        // reclaim, because the ONLY thing that authorizes it is a consensus fact
        // -- a deterministic deadline of heights, identical on every node.
        let mut job = self.require(&job_id)?;
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
                payload: "attempts exhausted".into(),
            });
        } else {
            job.status = JobStatus::Pending;
            job.claim = None; // attempt count kept for the next claim to bump
        }
        job.updated_at_height = height;
        self.stage_upsert(job);
        Ok(())
    }

    fn cancel(&mut self, job_id: String, origin: &Origin, height: u64) -> Result<(), Error> {
        let mut job = self.require(&job_id)?;
        // once claimed, the worker owns it until finalize/release/lease expiry.
        if job.status != JobStatus::Pending {
            return Err(Error::Module(format!(
                "cancel only applies to pending jobs (status {:?}): {job_id}",
                job.status
            )));
        }
        let actor = actor_from_origin(origin)?;
        if job.submitter != actor {
            return Err(Error::Module(format!(
                "only the submitter may cancel: {job_id}"
            )));
        }
        job.status = JobStatus::Cancelled;
        job.updated_at_height = height;
        self.stage_upsert(job);
        Ok(())
    }

    fn prune(&mut self, job_id: String, origin: &Origin) -> Result<(), Error> {
        let job = self.require(&job_id)?;
        if !job.status.is_terminal() {
            return Err(Error::Module(format!(
                "prune only applies to terminal jobs (status {:?}): {job_id}",
                job.status
            )));
        }
        let actor = actor_from_origin(origin)?;
        if job.submitter != actor {
            return Err(Error::Module(format!(
                "only the submitter may prune: {job_id}"
            )));
        }
        self.stage_remove(&job_id);
        Ok(())
    }

    // ---- dispatch ------------------------------------------------------------

    pub(crate) async fn execute(
        &mut self,
        ctx: &mut dyn Ctx,
        msg: JobsMsg,
        module_id: &ModuleId,
    ) -> Result<(), Error> {
        let env = ctx.env();
        let (origin, height) = (env.origin.clone(), env.height);
        match msg {
            JobsMsg::Submit { job_id, kind, spec } => {
                let event = self.submit(job_id, kind, spec, &origin, height)?;
                for worker in self.live_workers() {
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
            } => self.claim(job_id, lease_views, &origin, height),
            JobsMsg::Finalize {
                job_id,
                ok,
                payload,
            } => self.finalize(job_id, ok, payload, &origin, height),
            JobsMsg::Release { job_id } => self.release(job_id, &origin, height),
            JobsMsg::Reclaim { job_id } => self.reclaim(job_id, height),
            JobsMsg::Cancel { job_id } => self.cancel(job_id, &origin, height),
            JobsMsg::Prune { job_id } => self.prune(job_id, &origin),
            JobsMsg::RegisterWorker {} => self.register_worker(&origin, module_id),
            JobsMsg::UnregisterWorker {} => self.unregister_worker(&origin, module_id),
        }
    }

    /// read projections answer from COMMITTED state only -- never the staged
    /// overlay. a query must not observe a write that a block abort would take
    /// back; transition guards keep their own overlay-aware view internally.
    pub(crate) fn query(&self, q: JobsQuery) -> JobsReply {
        match q {
            JobsQuery::Get { job_id } => JobsReply::Job(self.jobs.get(&job_id).cloned()),
        }
    }

    pub(crate) fn commit(&mut self) {
        for (id, entry) in std::mem::take(&mut self.overlay) {
            match entry {
                Some(job) => {
                    self.jobs.insert(id, job);
                }
                None => {
                    self.jobs.remove(&id);
                }
            }
        }
        for (module_id, present) in std::mem::take(&mut self.worker_overlay) {
            if present {
                self.workers.insert(module_id);
            } else {
                self.workers.remove(&module_id);
            }
        }
    }

    pub(crate) fn abort(&mut self) {
        self.overlay.clear();
        self.worker_overlay.clear();
    }

    // ---- canonical encoding / snapshot ---------------------------------------

    /// append the canonical committed encoding (job map then worker set).
    pub(crate) fn encode_committed(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&(self.jobs.len() as u64).to_le_bytes());
        for job in self.jobs.values() {
            codec::push_str(out, &job.job_id);
            codec::push_str(out, &job.kind);
            codec::push_str(out, &job.spec);
            codec::push_str(out, &job.submitter);
            out.push(status_byte(&job.status));
            out.extend_from_slice(&job.attempt.to_le_bytes());
            match &job.claim {
                None => out.push(0),
                Some(c) => {
                    out.push(1);
                    codec::push_str(out, &c.worker);
                    out.extend_from_slice(&c.claimed_at_height.to_le_bytes());
                    out.extend_from_slice(&c.lease_views.to_le_bytes());
                }
            }
            match &job.result {
                None => out.push(0),
                Some(r) => {
                    out.push(1);
                    out.push(u8::from(r.ok));
                    codec::push_str(out, &r.payload);
                }
            }
            out.extend_from_slice(&job.created_at_height.to_le_bytes());
            out.extend_from_slice(&job.updated_at_height.to_le_bytes());
        }
        out.extend_from_slice(&(self.workers.len() as u64).to_le_bytes());
        for worker in &self.workers {
            codec::push_str(out, worker);
        }
    }

    /// read this board's portion off a shared cursor. does NOT `finish` the
    /// cursor.
    pub(crate) fn decode_from(c: &mut Cursor) -> Result<Self, Error> {
        let count = c.u64("snapshot job count")?;

        let mut jobs: BTreeMap<String, Job> = BTreeMap::new();
        for _ in 0..count {
            let job_id = c.string("snapshot job_id")?;
            let kind = c.string("snapshot kind")?;
            let spec = c.string("snapshot spec")?;
            let submitter = c.string("snapshot submitter")?;
            let status = status_from_byte(c.byte("snapshot status")?)?;
            let attempt = c.u64("snapshot attempt")?;
            let claim = match c.byte("snapshot claim flag")? {
                0 => None,
                1 => Some(Claim {
                    worker: c.string("snapshot claim worker")?,
                    claimed_at_height: c.u64("snapshot claimed_at_height")?,
                    lease_views: c.u64("snapshot lease_views")?,
                }),
                other => {
                    return Err(Error::Module(format!(
                        "snapshot has invalid claim flag: {other}"
                    )));
                }
            };
            let result = match c.byte("snapshot result flag")? {
                0 => None,
                1 => Some(JobResult {
                    ok: c.bool("snapshot result ok")?,
                    payload: c.string("snapshot result payload")?,
                }),
                other => {
                    return Err(Error::Module(format!(
                        "snapshot has invalid result flag: {other}"
                    )));
                }
            };
            let created_at_height = c.u64("snapshot created_at_height")?;
            let updated_at_height = c.u64("snapshot updated_at_height")?;

            // structural check: strictly-ascending ids match `BTreeMap`
            // iteration, so a byte stream that would not re-encode to itself is
            // rejected.
            if jobs
                .last_key_value()
                .is_some_and(|(last, _)| last.as_str() >= job_id.as_str())
            {
                return Err(Error::Module(
                    "snapshot job ids not strictly ascending".into(),
                ));
            }

            // execute-UNREACHABLE shapes are rejected -- these are exactly the
            // invariants every execute path upholds, so refusing them can never
            // refuse an honest validator's snapshot. anything weaker is left to
            // the root comparison in the composer's install. in particular a
            // `Processing` job without a claim would be permanently wedged:
            // finalize/release need worker equality against the claim and
            // reclaim needs its deadline, so no transition could ever repair it.
            match (&status, &claim) {
                (JobStatus::Processing, None) => {
                    return Err(Error::Module(
                        "snapshot has processing job without claim".into(),
                    ));
                }
                (JobStatus::Pending, Some(_)) => {
                    // submit creates without a claim; release/reclaim clear it.
                    return Err(Error::Module("snapshot has pending job with claim".into()));
                }
                _ => {}
            }
            if matches!(status, JobStatus::Done | JobStatus::Failed) && result.is_none() {
                // Done/Failed are only produced by finalize/exhausted-reclaim,
                // and both store a result. (Cancelled legitimately has none.)
                return Err(Error::Module(
                    "snapshot has finalized job without result".into(),
                ));
            }
            jobs.insert(
                job_id.clone(),
                Job {
                    job_id,
                    kind,
                    spec,
                    submitter,
                    status,
                    attempt,
                    claim,
                    result,
                    created_at_height,
                    updated_at_height,
                },
            );
        }
        let worker_count = c.u64("snapshot worker count")?;
        let worker_count = usize::try_from(worker_count)
            .map_err(|_| Error::Module("snapshot worker cap exceeded".into()))?;
        if worker_count > MAX_WORKERS {
            return Err(Error::Module("snapshot worker cap exceeded".into()));
        }
        let mut workers = BTreeSet::new();
        for _ in 0..worker_count {
            let worker = c.string("snapshot worker id")?;
            if worker.is_empty() || worker.len() > MAX_WORKER_MODULE_ID {
                return Err(Error::Module("snapshot worker module id is invalid".into()));
            }
            if workers
                .last()
                .is_some_and(|last: &String| last.as_str() >= worker.as_str())
            {
                return Err(Error::Module(
                    "snapshot worker ids not strictly ascending".into(),
                ));
            }
            workers.insert(worker);
        }
        Ok(Self {
            jobs,
            workers,
            overlay: BTreeMap::new(),
            worker_overlay: BTreeMap::new(),
        })
    }
}

/// derive the acting identity from the dispatch origin -- the ONLY authorship
/// path. an empty external origin (the pre-consensus `Origin::External(vec![])`
/// default) is not an authenticated submitter and is rejected; the string form
/// is the shared [`Origin::actor_string`] convention.
fn actor_from_origin(origin: &Origin) -> Result<String, Error> {
    if matches!(origin, Origin::External(bytes) if bytes.is_empty()) {
        return Err(Error::Module(
            "external origin must carry a non-empty submitter id".into(),
        ));
    }
    Ok(origin.actor_string())
}

fn status_byte(status: &JobStatus) -> u8 {
    match status {
        JobStatus::Pending => 0,
        JobStatus::Processing => 1,
        JobStatus::Done => 2,
        JobStatus::Failed => 3,
        JobStatus::Cancelled => 4,
    }
}

fn status_from_byte(value: u8) -> Result<JobStatus, Error> {
    match value {
        0 => Ok(JobStatus::Pending),
        1 => Ok(JobStatus::Processing),
        2 => Ok(JobStatus::Done),
        3 => Ok(JobStatus::Failed),
        4 => Ok(JobStatus::Cancelled),
        _ => Err(Error::Module("snapshot has invalid job status".into())),
    }
}
