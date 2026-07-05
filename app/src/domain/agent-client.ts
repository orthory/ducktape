// Typed client for the node's `agent` module — the TS mirror of
// `crates/apps/agent-interface`. The agent module is the collaboration-loop
// orchestrator: it registers agents, watches chat channels, turns engaged
// posts into runs, and validates each run's oracle output before any
// cross-module write.
//
// This file plays the interface crate's role on the client side. Key contract
// points mirrored here:
//   - ownership is NEVER in a write payload — the module derives an agent's
//     owner (and a run's requester) from the block's origin, so every write
//     function takes an `origin` and passes it to transport.submit, exactly
//     like chat-client.
//   - prompt CONTENT lives off-registry: RegisterAgent/UpdateAgent commit only
//     a 32-byte `prompt_hash` (= sha256 of the prompt bytes = the digest
//     `transport.putBlob` returns), and the oracle worker fetches the prompt
//     text from the node's blob store by that hash.
//
// Everything is a pure function over an injected NodeTransport.

import type { BlockEvent, NodeTransport } from "./transport";
import { replyVariant } from "./wire";

// ── Wire types (AgentReply records + shared enums, verbatim) ─

/** Origin discriminant shared across modules (saga-interface's SagaOrigin): an
 *  external submitter's raw key bytes, a follow-up module, or genesis/system. */
export type SagaOrigin = { External: number[] } | { Module: string } | "System";

/** Whether an agent may engage new runs. */
export type AgentStatus = "Active" | "Paused";

/** How a watched channel selects which agents a user post engages. `Assigned`
 *  names exactly one agent; the other three are structural (serde newtype: the
 *  Assigned variant is `{ "Assigned": "<agent_id>" }` on the wire). */
export type TurnPolicy = "Mention" | "All" | { Assigned: string } | "RoundRobin";

export interface AgentRecord {
  agent_id: string;
  /** The registration origin — gates every mutation of the record. */
  owner: SagaOrigin;
  display_name: string;
  capability: string;
  /** sha256 of the agent's prompt content — exactly 32 bytes. */
  prompt_hash: number[];
  /** Granted action names, each from `KNOWN_ACTIONS`; sorted and deduped. */
  allowed_actions: string[];
  status: AgentStatus;
  created_at: number;
  updated_at: number;
}

export interface WatchView {
  channel_id: string;
  policy: TurnPolicy;
}

/** Where a run is in its lifecycle. Only `AwaitingOracle` is non-terminal. */
export type RunStatus =
  | { AwaitingOracle: { saga_id: string } }
  | "Done"
  | { Failed: { reason: string } }
  | "Cancelled";

export interface RunView {
  run_id: string;
  agent_id: string;
  channel_id: string;
  /** The message sequence this run answers. */
  anchor_seq: number;
  /** The anchor's thread root, if it was a thread reply. */
  thread_root: number | null;
  /** Present for jobs-board runs; chat-triggered runs leave this null. */
  job_id: string | null;
  job_claim_height: number;
  /** The run-creating origin — a cancel capability alongside the owner. */
  requester: SagaOrigin;
  status: RunStatus;
  /** sha256 over the pinned transcript window up to `anchor_seq`. */
  context_hash: number[];
  created_at: number;
  updated_at: number;
}

const TARGET = "agent";

/** Query page bound mirrored from the interface crate (MAX_QUERY_LIMIT). */
export const MAX_QUERY_LIMIT = 256;

/** Every action name an agent can be granted (KNOWN_ACTIONS). A RegisterAgent /
 *  UpdateAgent rejects an `allowed_actions` entry outside this set. */
export const KNOWN_ACTIONS = [
  "chat.post",
  "tasks.create",
  "tasks.update_status",
] as const;

// ── Prompt hashing helper ───────────────────────────────

/** A 64-char lowercase-hex digest → the 32 byte ints the wire carries as a
 *  `prompt_hash`. The node's blob store keys by sha256(bytes), so the digest
 *  `transport.putBlob` returns for the prompt text IS the hash to register. */
export const hexToBytes = (hex: string): number[] =>
  Array.from({ length: Math.floor(hex.length / 2) }, (_, i) =>
    parseInt(hex.slice(i * 2, i * 2 + 2), 16),
  );

// ── Msgs (writes — one submit = one block; owner/requester from origin) ──

export const registerAgent = (
  transport: NodeTransport,
  params: {
    agentId: string;
    displayName: string;
    capability: string;
    /** Exactly 32 bytes — see hexToBytes / the prompt-upload flow. */
    promptHash: number[];
    allowedActions: string[];
    origin: string;
  },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    {
      RegisterAgent: {
        agent_id: params.agentId,
        display_name: params.displayName,
        capability: params.capability,
        prompt_hash: params.promptHash,
        allowed_actions: params.allowedActions,
      },
    },
    params.origin,
  );

/** Owner-gated partial update — an omitted (null) field keeps its value. */
export const updateAgent = (
  transport: NodeTransport,
  params: {
    agentId: string;
    displayName?: string | null;
    capability?: string | null;
    promptHash?: number[] | null;
    allowedActions?: string[] | null;
    origin: string;
  },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    {
      UpdateAgent: {
        agent_id: params.agentId,
        display_name: params.displayName ?? null,
        capability: params.capability ?? null,
        prompt_hash: params.promptHash ?? null,
        allowed_actions: params.allowedActions ?? null,
      },
    },
    params.origin,
  );

export const pauseAgent = (
  transport: NodeTransport,
  params: { agentId: string; origin: string },
): Promise<BlockEvent> =>
  transport.submit(TARGET, { PauseAgent: { agent_id: params.agentId } }, params.origin);

export const resumeAgent = (
  transport: NodeTransport,
  params: { agentId: string; origin: string },
): Promise<BlockEvent> =>
  transport.submit(TARGET, { ResumeAgent: { agent_id: params.agentId } }, params.origin);

export const watchChannel = (
  transport: NodeTransport,
  params: { channelId: string; policy: TurnPolicy; origin: string },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    { WatchChannel: { channel_id: params.channelId, policy: params.policy } },
    params.origin,
  );

export const unwatchChannel = (
  transport: NodeTransport,
  params: { channelId: string; origin: string },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    { UnwatchChannel: { channel_id: params.channelId } },
    params.origin,
  );

export const enableJobWorker = (
  transport: NodeTransport,
  params: { enabled: boolean; origin: string },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    { EnableJobWorker: { enabled: params.enabled } },
    params.origin,
  );

export const requestRun = (
  transport: NodeTransport,
  params: { agentId: string; channelId: string; anchorSeq: number; origin: string },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    {
      RequestRun: {
        agent_id: params.agentId,
        channel_id: params.channelId,
        anchor_seq: params.anchorSeq,
      },
    },
    params.origin,
  );

export const cancelRun = (
  transport: NodeTransport,
  params: { runId: string; origin: string },
): Promise<BlockEvent> =>
  transport.submit(TARGET, { CancelRun: { run_id: params.runId } }, params.origin);

// ── Queries (reads over committed state) ────────────────

/** Every registered agent. `Agents` is a unit-variant query — the bare
 *  string, like tasks-client's `List`. */
export const agents = (transport: NodeTransport): Promise<AgentRecord[]> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, "Agents"))
    .then((reply) => replyVariant<AgentRecord[]>(reply, "Agents"));

/** One agent by id, or null when absent. */
export const agent = (
  transport: NodeTransport,
  agentId: string,
): Promise<AgentRecord | null> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, { Agent: { agent_id: agentId } }))
    .then((reply) => replyVariant<AgentRecord | null>(reply, "Agent"));

/** Runs ascending by run id, optionally filtered to one channel (null = all);
 *  `limit` is clamped node-side to MAX_QUERY_LIMIT. */
export const runs = (
  transport: NodeTransport,
  params: { channelId: string | null; limit: number },
): Promise<RunView[]> =>
  Promise.resolve()
    .then(() =>
      transport.query(TARGET, {
        Runs: { channel_id: params.channelId, limit: params.limit },
      }),
    )
    .then((reply) => replyVariant<RunView[]>(reply, "Runs"));

/** One run by id, or null when absent. */
export const run = (
  transport: NodeTransport,
  runId: string,
): Promise<RunView | null> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, { Run: { run_id: runId } }))
    .then((reply) => replyVariant<RunView | null>(reply, "Run"));

/** Every channel watch. `Watches` is a unit-variant query — the bare string. */
export const watches = (transport: NodeTransport): Promise<WatchView[]> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, "Watches"))
    .then((reply) => replyVariant<WatchView[]>(reply, "Watches"));
