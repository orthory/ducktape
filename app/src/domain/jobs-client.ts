// Typed client for the node's `jobs` module — the TS mirror of
// `crates/apps/jobs-interface`. `jobs` is a consensus-native work board: a
// submitter posts a job, any worker claims it, exactly one claim wins by
// consensus order, the claimant processes off-platform and reports a result.
//
// Every identity field (Job.submitter, Claim.worker) is derived by the module
// from the dispatch origin, never carried on the wire — so no write here takes a
// caller-supplied identity. camelCase params in, verbatim serde wire out, pure
// functions over an injected NodeTransport.

import type { BlockEvent, NodeTransport } from "./transport";
import { replyVariant } from "./wire";

// ── Wire types (verbatim serde shapes) ──────────────────

export type JobStatus =
  | "pending"
  | "processing"
  | "done"
  | "failed"
  | "cancelled";

/** Terminal states never transition again (only Prune removes them). */
export const TERMINAL_STATUSES: readonly JobStatus[] = [
  "done",
  "failed",
  "cancelled",
];

export const isTerminal = (status: JobStatus): boolean =>
  TERMINAL_STATUSES.includes(status);

/** The winning claim on a job. `worker` is origin-derived. */
export interface Claim {
  worker: string;
  claimed_at_height: number;
  lease_views: number;
}

/** The claimant's reported outcome, stored once (result singularity). */
export interface JobResult {
  ok: boolean;
  payload: string;
}

export interface Job {
  job_id: string;
  kind: string;
  spec: string;
  /** origin-derived submitter identity, set by the module */
  submitter: string;
  status: JobStatus;
  /** total number of successful claims over this job's life */
  attempt: number;
  claim: Claim | null;
  result: JobResult | null;
  created_at_height: number;
  updated_at_height: number;
}

/** A per-status census of the board. */
export interface BoardCounts {
  pending: number;
  processing: number;
  done: number;
  failed: number;
  cancelled: number;
}

const TARGET = "jobs";

// ── Msgs (writes) ───────────────────────────────────────

/** Post a new job (status Pending, attempt 0). */
export const submitJob = (
  transport: NodeTransport,
  params: { jobId: string; kind: string; spec: string; origin?: string },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    { submit: { job_id: params.jobId, kind: params.kind, spec: params.spec } },
    params.origin,
  );

/** Claim a Pending job; a claim on a non-pending job is rejected. */
export const claimJob = (
  transport: NodeTransport,
  params: { jobId: string; leaseViews: number; origin?: string },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    { claim: { job_id: params.jobId, lease_views: params.leaseViews } },
    params.origin,
  );

/** The current claimant reports a result on a Processing job. */
export const finalizeJob = (
  transport: NodeTransport,
  params: { jobId: string; ok: boolean; payload: string; origin?: string },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    { finalize: { job_id: params.jobId, ok: params.ok, payload: params.payload } },
    params.origin,
  );

/** The current claimant hands a Processing job back to Pending. */
export const releaseJob = (
  transport: NodeTransport,
  params: { jobId: string; origin?: string },
): Promise<BlockEvent> =>
  transport.submit(TARGET, { release: { job_id: params.jobId } }, params.origin);

/** Permissionless requeue of a Processing job whose lease has expired. */
export const reclaimJob = (
  transport: NodeTransport,
  params: { jobId: string; origin?: string },
): Promise<BlockEvent> =>
  transport.submit(TARGET, { reclaim: { job_id: params.jobId } }, params.origin);

/** The submitter cancels a still-Pending job. */
export const cancelJob = (
  transport: NodeTransport,
  params: { jobId: string; origin?: string },
): Promise<BlockEvent> =>
  transport.submit(TARGET, { cancel: { job_id: params.jobId } }, params.origin);

/** The submitter removes a terminal job's record entirely. */
export const pruneJob = (
  transport: NodeTransport,
  params: { jobId: string; origin?: string },
): Promise<BlockEvent> =>
  transport.submit(TARGET, { prune: { job_id: params.jobId } }, params.origin);

// ── Queries (reads over committed state) ────────────────

export const getJob = (
  transport: NodeTransport,
  jobId: string,
): Promise<Job | null> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, { get: { job_id: jobId } }))
    .then((reply) => replyVariant<Job | null>(reply, "job"));

/** Jobs filtered by optional status and a kind prefix, at most `limit`. */
export const listJobs = (
  transport: NodeTransport,
  params: { status?: JobStatus | null; kindPrefix?: string; limit?: number },
): Promise<Job[]> =>
  Promise.resolve()
    .then(() =>
      transport.query(TARGET, {
        list: {
          status: params.status ?? null,
          kind_prefix: params.kindPrefix ?? "",
          limit: params.limit ?? 256,
        },
      }),
    )
    .then((reply) => replyVariant<Job[]>(reply, "jobs"));

export const counts = (transport: NodeTransport): Promise<BoardCounts> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, { counts: {} }))
    .then((reply) => replyVariant<BoardCounts>(reply, "counts"));
