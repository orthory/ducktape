// Typed client for the node's `runs` module — the TS mirror of
// `crates/apps/runs-interface`. The runs module is the collaboration loop's
// actor: it watches chat channels, turns engaged posts into dispatches, and
// validates each run's response before any cross-module write. The agents it
// runs live in the agent registry (`agent-client`); this surface carries only
// the acting half — watches, explicit runs, cancellation, the jobs-worker
// toggle, and the in-flight correlation entries (PendingRuns), pruned when a
// result delivers. Run LIFECYCLE is not runs state — a run is a dispatched
// task whose status and outcome live in the dispatch module under receiver
// "runs" + `dispatch_id`.
//
// Ownership is NEVER in a write payload — the module derives a run's
// requester from the block's origin, so every write function takes an
// `origin` and passes it to transport.submit, exactly like chat-client.
//
// Everything is a pure function over an injected NodeTransport.

import { sha256 } from "@noble/hashes/sha2.js";

import type { SagaOrigin } from "./agent-client";
import type { BlockEvent, NodeTransport } from "./transport";
import { replyVariant } from "./wire";

// ── Wire types (RunsReply records, verbatim) ─────────────

/** How a watched channel selects which agents a user post engages. `Assigned`
 *  names exactly one agent; the other three are structural (serde newtype: the
 *  Assigned variant is `{ "assigned": "<agent_id>" }` on the wire). */
export type TurnPolicy = "mention" | "all" | { assigned: string } | "round_robin";

export interface WatchView {
  channel_id: string;
  policy: TurnPolicy;
}

/** One in-flight run's correlation entry — everything the module keeps while
 *  its dispatch is outstanding. NOT a lifecycle record: the entry prunes when
 *  the result delivers; status and outcome live in the dispatch module under
 *  receiver "runs" + `dispatch_id`. */
export interface PendingRun {
  run_id: string;
  /** hex sha256 of `run_id` — the dispatch-plane id this entry correlates. */
  dispatch_id: string;
  agent_id: string;
  /** Empty for job-backed runs. */
  channel_id: string;
  /** The message sequence this run answers; 0 for job-backed runs. */
  anchor_seq: number;
  /** The anchor's thread root, if it was a thread reply. */
  thread_root: number | null;
  /** Present for jobs-board runs; chat-triggered runs leave this null. */
  job_id: string | null;
  job_claim_height: number;
  /** The run-creating origin — a cancel capability alongside the owner. */
  requester: SagaOrigin;
  created_at: number;
}

/** How a run ended: delivered (possibly degraded) or failed. */
export type RunOutcome = "delivered" | "failed";

/** One terminal run in the node's delivered-runs ring (last 100, newest
 *  first) — derived observability state recorded at delivery, never part of
 *  consensus roots; empty on a snapshot-joined node. */
export interface RunRecord {
  run_id: string;
  agent_id: string;
  /** Empty for job-backed runs. */
  channel_id: string;
  /** The anchor message seq; 0 for job-backed runs. */
  anchor_seq: number;
  outcome: RunOutcome;
  /** The host observed the run as degraded but still delivered it. */
  degraded: boolean;
  /** Consensus counters (creation/delivery block) — only their DIFF is
   *  meaningful as a duration; never render the raw values as clock time. */
  created_at: number;
  delivered_at: number;
  /** Lowercase key hex of the node that executed the run, or "unknown". */
  executing_node: string;
  /** forge `branch@output_commit` or a duckfs snapshot id; null when the run
   *  moved nothing. */
  output_ref: string | null;
  /** The forge PR this run opened or updated, when the PR sink applied. */
  pr_number: number | null;
}

/** Mirror `runs::dispatch_id_for`: output rings are keyed by the lowercase
 * hex SHA-256 of the stable run id, including after the pending row is gone. */
export const dispatchIdForRun = (runId: string): string =>
  Array.from(sha256(new TextEncoder().encode(runId)), (byte) =>
    byte.toString(16).padStart(2, "0"),
  ).join("");

const TARGET = "runs";

// ── Msgs (writes — one submit = one block; requester from origin) ──

export const watchChannel = (
  transport: NodeTransport,
  params: { channelId: string; policy: TurnPolicy; origin: string },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    { watch_channel: { channel_id: params.channelId, policy: params.policy } },
    params.origin,
  );

export const unwatchChannel = (
  transport: NodeTransport,
  params: { channelId: string; origin: string },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    { unwatch_channel: { channel_id: params.channelId } },
    params.origin,
  );

export const enableJobWorker = (
  transport: NodeTransport,
  params: { enabled: boolean; origin: string },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    { enable_job_worker: { enabled: params.enabled } },
    params.origin,
  );

export const requestRun = (
  transport: NodeTransport,
  params: {
    agentId: string;
    channelId: string;
    anchorSeq: number;
    origin: string;
    /** Per-run resource demands (`RequestRun.demands`), dimension → positive
     *  integer. Omitted from the wire when absent or empty — a missing key is
     *  legacy-valid, but consensus rejects an empty map and zero values. */
    demands?: Record<string, number>;
  },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    {
      request_run: {
        agent_id: params.agentId,
        channel_id: params.channelId,
        anchor_seq: params.anchorSeq,
        ...(params.demands && Object.keys(params.demands).length > 0
          ? { demands: params.demands }
          : {}),
      },
    },
    params.origin,
  );

export const cancelRun = (
  transport: NodeTransport,
  params: { runId: string; origin: string },
): Promise<BlockEvent> =>
  transport.submit(TARGET, { cancel_run: { run_id: params.runId } }, params.origin);

export const reassignRun = (
  transport: NodeTransport,
  params: { runId: string; attempt: number; origin: string },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    { reassign_run: { run_id: params.runId, attempt: params.attempt } },
    params.origin,
  );

// ── Queries (reads over committed state) ────────────────

/** Every in-flight correlation entry, ascending by dispatch id. Bounded:
 *  entries prune on delivery and every dispatch has a deadline.
 *  `PendingRuns` is a unit-variant query — the bare string. */
export const pendingRuns = (transport: NodeTransport): Promise<PendingRun[]> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, "pending_runs"))
    .then((reply) => replyVariant<PendingRun[]>(reply, "pending_runs"));

/** Every channel watch. `Watches` is a unit-variant query — the bare string. */
export const watches = (transport: NodeTransport): Promise<WatchView[]> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, "watches"))
    .then((reply) => replyVariant<WatchView[]>(reply, "watches"));

/** The delivered-runs ring, newest first (last 100). `RecentRuns` is a
 *  unit-variant query — the bare string. */
export const recentRuns = (transport: NodeTransport): Promise<RunRecord[]> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, "recent_runs"))
    .then((reply) => replyVariant<RunRecord[]>(reply, "recent_runs"));
