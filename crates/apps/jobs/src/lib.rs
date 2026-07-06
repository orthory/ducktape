//! deterministic in-memory jobs module -- a consensus-native work board.
//!
//! # what jobs is (and what it is NOT)
//!
//! `jobs` is a PRODUCT-level work board for on-platform actors: humans and agent
//! fleets submitting ordinary signed ops over RPC. a submitter posts a job; any
//! worker claims it; **exactly one claim wins by consensus order** -- there is no
//! distributed lock because the total order IS the lock (the same first-claim
//! discipline the agent module uses for run-id turn claims). the claimant does
//! the work off-platform and reports a result; result singularity is enforced
//! in-state. jobs are open-ended work items with a human-visible lifecycle:
//! pending -> processing (claimed, leased) -> finalized (done/failed), plus
//! release, permissionless lease-expiry reclaim, cancel, and prune.
//!
//! `jobs` is DISTINCT from `saga`. `saga` is the platform's async-RPC ledger for
//! REACTOR effects -- off-consensus work the node's own worker loop performs,
//! addressed by module code. `jobs` never touches saga or the reactor: workers
//! act by submitting ordinary signed ops, and every "identity" (`submitter`,
//! claim `worker`) is derived from the dispatch [`Origin`] inside this module,
//! never read off the wire.
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
//! during `execute` and publish only at `commit_block`; `root()` hashes the
//! committed map over a canonical byte encoding. the overlay is a tombstone
//! overlay (`Option<Job>`: `Some` = upsert, `None` = staged delete) so `Prune`
//! can remove a record atomically with the rest of the block. `snapshot`/
//! `install` use the exact `root()` preimage; install verifies the expected root
//! before adopting and rejects structurally impossible bytes (non-ascending ids,
//! trailing bytes) plus the few execute-UNREACHABLE shapes that would wedge a
//! job (e.g. `Processing` without a claim). everything execute-reachable is
//! accepted -- the root comparison, not an invariant sweep, is the integrity
//! check.
//!
//! queries (`Get`/`List`/`Counts`) answer from COMMITTED state only, never the
//! staged overlay; transition guards keep an overlay-aware view internally so
//! in-block effects (a first claim) are visible to later ops in the same block.
//!
//! registered workers are consensus state. every successful `Submit` emits one
//! `JobsEvent::Submitted` follow-up per worker in the submitter's block; that
//! fan-out counts toward the host dispatch budget. THIS SLICE supports one
//! claiming worker for a given kind: if two workers claim the same job in that
//! submit cascade, the second claim rejects against the first staged claim and
//! the whole submit block aborts atomically (P2). `MAX_WORKERS` is reserved for
//! later off-cascade strategies, not multiple in-cascade claimants per kind.
//!
//! # the board is a queue, not an archive
//!
//! terminal jobs stay on the board until their submitter `Prune`s them. this is
//! deliberate: the alternative (auto-retaining every finalized job forever) is
//! unbounded state growth. pruning is the submitter's explicit opt-in to forget.

// the wire surface: this module's shared types, flattened at the crate root.
mod interface;
pub use interface::*;

use std::collections::{BTreeMap, BTreeSet};

use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot};
use sha2::{Digest, Sha256};

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

pub struct Jobs {
    id: ModuleId,
    /// committed board; `root()` hashes exactly this map.
    jobs: BTreeMap<String, Job>,
    /// committed worker set; every successful submit fans out to these modules.
    workers: BTreeSet<ModuleId>,
    /// staged overlay: `Some` upserts, `None` is a tombstone (staged delete).
    overlay: BTreeMap<String, Option<Job>>,
    /// staged worker overlay: `true` = present, `false` = absent.
    worker_overlay: BTreeMap<ModuleId, bool>,
}

impl Jobs {
    pub fn new(id: impl Into<ModuleId>) -> Self {
        Self {
            id: id.into(),
            jobs: BTreeMap::new(),
            workers: BTreeSet::new(),
            overlay: BTreeMap::new(),
            worker_overlay: BTreeMap::new(),
        }
    }

    // ---- overlay-aware reads (execute-internal ONLY) --------------------------
    //
    // transition guards must see in-block effects -- a second claim in the same
    // block has to observe the first claim's staged `Processing` -- so `get`/
    // `require`/`live_count` read through the overlay. queries do NOT: they
    // answer from COMMITTED state only (see [`Module::query`] below), so a read
    // never leaks a staged write that a block abort would take back.

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

    fn worker_module_from_origin(&self, origin: &Origin) -> Result<ModuleId, Error> {
        let Origin::Module(module_id) = origin else {
            return Err(Error::Module(
                "worker registration requires a module origin".into(),
            ));
        };
        if module_id.is_empty() {
            return Err(Error::Module("worker module_id must not be empty".into()));
        }
        if module_id.len() > MAX_WORKER_MODULE_ID {
            return Err(Error::Module(format!(
                "worker module_id exceeds {MAX_WORKER_MODULE_ID} bytes"
            )));
        }
        if module_id == &self.id {
            return Err(Error::Module(
                "jobs cannot register itself as a worker".into(),
            ));
        }
        Ok(module_id.clone())
    }

    fn register_worker(&mut self, origin: &Origin) -> Result<(), Error> {
        let module_id = self.worker_module_from_origin(origin)?;
        if self.has_worker(&module_id) {
            return Ok(());
        }
        if self.worker_count() >= MAX_WORKERS {
            return Err(Error::Module("worker cap reached".into()));
        }
        self.worker_overlay.insert(module_id, true);
        Ok(())
    }

    fn unregister_worker(&mut self, origin: &Origin) -> Result<(), Error> {
        let module_id = self.worker_module_from_origin(origin)?;
        if !self.has_worker(&module_id) {
            return Ok(());
        }
        self.worker_overlay.insert(module_id, false);
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

    // ---- canonical encoding / root / snapshot / install ----------------------

    fn root_of(jobs: &BTreeMap<String, Job>, workers: &BTreeSet<ModuleId>) -> StateRoot {
        let mut h = Sha256::new();
        h.update(Self::encode_state(jobs, workers));
        StateRoot(h.finalize().into())
    }

    fn encode_state(jobs: &BTreeMap<String, Job>, workers: &BTreeSet<ModuleId>) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(jobs.len() as u64).to_le_bytes());
        for job in jobs.values() {
            push_string(&mut out, &job.job_id);
            push_string(&mut out, &job.kind);
            push_string(&mut out, &job.spec);
            push_string(&mut out, &job.submitter);
            out.push(status_byte(&job.status));
            out.extend_from_slice(&job.attempt.to_le_bytes());
            match &job.claim {
                None => out.push(0),
                Some(c) => {
                    out.push(1);
                    push_string(&mut out, &c.worker);
                    out.extend_from_slice(&c.claimed_at_height.to_le_bytes());
                    out.extend_from_slice(&c.lease_views.to_le_bytes());
                }
            }
            match &job.result {
                None => out.push(0),
                Some(r) => {
                    out.push(1);
                    out.push(u8::from(r.ok));
                    push_string(&mut out, &r.payload);
                }
            }
            out.extend_from_slice(&job.created_at_height.to_le_bytes());
            out.extend_from_slice(&job.updated_at_height.to_le_bytes());
        }
        out.extend_from_slice(&(workers.len() as u64).to_le_bytes());
        for worker in workers {
            push_string(&mut out, worker);
        }
        out
    }

    pub fn snapshot(&self) -> Vec<u8> {
        Self::encode_state(&self.jobs, &self.workers)
    }

    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        let (jobs, workers) = decode_snapshot(bytes)?;
        if Self::root_of(&jobs, &workers) != expected {
            return Err(Error::Module("snapshot root mismatch".into()));
        }
        self.jobs = jobs;
        self.workers = workers;
        self.overlay.clear();
        self.worker_overlay.clear();
        Ok(())
    }
}

/// derive the acting identity from the dispatch origin -- the ONLY authorship
/// path. an empty external origin (the pre-consensus `Origin::External(vec![])`
/// default) is not an authenticated submitter and is rejected.
///
/// external keys are domain-separated as `ext:` + lowercase hex (the cross-
/// branch module convention) so a future module registered under a pure-hex id
/// can never collide with an external key's hex; module ids pass verbatim and
/// system is the literal "system".
fn actor_from_origin(origin: &Origin) -> Result<String, Error> {
    match origin {
        Origin::External(bytes) if bytes.is_empty() => Err(Error::Module(
            "external origin must carry a non-empty submitter id".into(),
        )),
        Origin::External(bytes) => Ok(format!("ext:{}", hex_lower(bytes))),
        Origin::Module(id) => Ok(id.clone()),
        Origin::System => Ok("system".into()),
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn push_string(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u64).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
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

fn decode_snapshot(bytes: &[u8]) -> Result<(BTreeMap<String, Job>, BTreeSet<ModuleId>), Error> {
    let mut off = 0usize;
    let count = read_u64(bytes, &mut off)?;

    let mut jobs: BTreeMap<String, Job> = BTreeMap::new();
    for _ in 0..count {
        let job_id = read_string(bytes, &mut off)?;
        let kind = read_string(bytes, &mut off)?;
        let spec = read_string(bytes, &mut off)?;
        let submitter = read_string(bytes, &mut off)?;
        let status = status_from_byte(read_u8(bytes, &mut off)?)?;
        let attempt = read_u64(bytes, &mut off)?;
        let claim = match read_u8(bytes, &mut off)? {
            0 => None,
            1 => Some(Claim {
                worker: read_string(bytes, &mut off)?,
                claimed_at_height: read_u64(bytes, &mut off)?,
                lease_views: read_u64(bytes, &mut off)?,
            }),
            other => {
                return Err(Error::Module(format!(
                    "snapshot has invalid claim flag: {other}"
                )));
            }
        };
        let result = match read_u8(bytes, &mut off)? {
            0 => None,
            1 => Some(JobResult {
                ok: read_bool(bytes, &mut off)?,
                payload: read_string(bytes, &mut off)?,
            }),
            other => {
                return Err(Error::Module(format!(
                    "snapshot has invalid result flag: {other}"
                )));
            }
        };
        let created_at_height = read_u64(bytes, &mut off)?;
        let updated_at_height = read_u64(bytes, &mut off)?;

        // structural check: strictly-ascending ids match `BTreeMap` iteration,
        // so a byte stream that would not re-encode to itself is rejected.
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
        // refuse an honest validator's snapshot. anything weaker is left to the
        // root comparison in `install`. in particular a `Processing` job
        // without a claim would be permanently wedged: finalize/release need
        // worker equality against the claim and reclaim needs its deadline, so
        // no transition could ever repair it.
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
            // Done/Failed are only produced by finalize/exhausted-reclaim, and
            // both store a result. (Cancelled legitimately has none.)
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
    let worker_count = read_u64(bytes, &mut off)?;
    let worker_count = usize::try_from(worker_count)
        .map_err(|_| Error::Module("snapshot worker cap exceeded".into()))?;
    if worker_count > MAX_WORKERS {
        return Err(Error::Module("snapshot worker cap exceeded".into()));
    }
    let mut workers = BTreeSet::new();
    for _ in 0..worker_count {
        let worker = read_string(bytes, &mut off)?;
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
    if off != bytes.len() {
        return Err(Error::Module("snapshot has trailing bytes".into()));
    }
    Ok((jobs, workers))
}

fn read_u8(bytes: &[u8], off: &mut usize) -> Result<u8, Error> {
    let end = off
        .checked_add(1)
        .filter(|&end| end <= bytes.len())
        .ok_or_else(|| Error::Module("snapshot truncated".into()))?;
    let value = bytes[*off];
    *off = end;
    Ok(value)
}

fn read_bool(bytes: &[u8], off: &mut usize) -> Result<bool, Error> {
    match read_u8(bytes, off)? {
        0 => Ok(false),
        1 => Ok(true),
        other => Err(Error::Module(format!("snapshot has invalid bool: {other}"))),
    }
}

fn read_u64(bytes: &[u8], off: &mut usize) -> Result<u64, Error> {
    let end = off
        .checked_add(8)
        .filter(|&end| end <= bytes.len())
        .ok_or_else(|| Error::Module("snapshot truncated".into()))?;
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&bytes[*off..end]);
    *off = end;
    Ok(u64::from_le_bytes(buf))
}

fn read_string(bytes: &[u8], off: &mut usize) -> Result<String, Error> {
    let len = read_u64(bytes, off)?;
    let len = usize::try_from(len).map_err(|_| Error::Module("snapshot truncated".into()))?;
    if len > bytes.len() - *off {
        return Err(Error::Module("snapshot truncated".into()));
    }
    let value = std::str::from_utf8(&bytes[*off..*off + len])
        .map_err(|_| Error::Module("snapshot string is not utf-8".into()))?;
    *off += len;
    Ok(value.to_owned())
}

#[async_trait::async_trait(?Send)]
impl Module for Jobs {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        Self::root_of(&self.jobs, &self.workers)
    }

    /// advertise the snapshot lane: [`Jobs::snapshot`] is the exact preimage of
    /// `root()`, and [`Jobs::install`] verifies before adopting -- without this
    /// override a joiner sees `Unsupported` and cannot rebuild the module.
    fn state_sync_handle(&self) -> Result<sdk::StateSyncHandle, Error> {
        Ok(sdk::StateSyncHandle::SnapshotBytes(self.snapshot()))
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let env = ctx.env();
        let (origin, height) = (env.origin.clone(), env.height);
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            JobsMsg::Submit { job_id, kind, spec } => {
                let event = self.submit(job_id, kind, spec, &origin, height)?;
                for worker in self.live_workers() {
                    ctx.emit_msg(Msg {
                        target: worker,
                        payload: encode_event(&event),
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
            JobsMsg::RegisterWorker {} => self.register_worker(&origin),
            JobsMsg::UnregisterWorker {} => self.unregister_worker(&origin),
        }
    }

    /// read projections answer from COMMITTED state only -- never the staged
    /// overlay. a query must not observe a write that a block abort would take
    /// back; transition guards keep their own overlay-aware view internally.
    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            JobsQuery::Get { job_id } => Ok(encode_reply(&JobsReply::Job(
                self.jobs.get(&job_id).cloned(),
            ))),
            JobsQuery::List {
                status,
                kind_prefix,
                limit,
            } => {
                let limit = limit.min(MAX_LIST_LIMIT) as usize;
                let jobs: Vec<Job> = self
                    .jobs
                    .values()
                    .filter(|job| match &status {
                        Some(want) => &job.status == want,
                        None => true,
                    })
                    .filter(|job| job.kind.starts_with(&kind_prefix))
                    .take(limit)
                    .cloned()
                    .collect();
                Ok(encode_reply(&JobsReply::Jobs(jobs)))
            }
            JobsQuery::Counts {} => {
                let mut counts = BoardCounts::default();
                for job in self.jobs.values() {
                    match job.status {
                        JobStatus::Pending => counts.pending += 1,
                        JobStatus::Processing => counts.processing += 1,
                        JobStatus::Done => counts.done += 1,
                        JobStatus::Failed => counts.failed += 1,
                        JobStatus::Cancelled => counts.cancelled += 1,
                    }
                }
                Ok(encode_reply(&JobsReply::Counts(counts)))
            }
        }
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
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
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.overlay.clear();
        self.worker_overlay.clear();
        Ok(())
    }
}
