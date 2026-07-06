// Typed client for the node's `agent` module — the TS mirror of
// `crates/apps/agent-interface`. The agent module is the platform's agent
// REGISTRY and nothing more: a self-contained record book of which agents
// exist — owner, capability tag, prompt pin, granted actions, status. The
// acting half of the collaboration loop (watches, runs, cancellation) lives
// in the runs module — see `runs-client`.
//
// This file plays the interface crate's role on the client side. Key contract
// points mirrored here:
//   - ownership is NEVER in a write payload — the module derives an agent's
//     owner from the block's origin, so every write function takes an
//     `origin` and passes it to transport.submit, exactly like chat-client.
//   - prompt CONTENT lives off-registry: RegisterAgent/UpdateAgent commit a
//     32-byte `prompt_hash` pin plus an optional `prompt_doc` — a document
//     module doc id whose canonical rendering (block texts joined by blank
//     lines) must hash to the pin. every run composes its prompt in-consensus
//     from that doc.
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

export interface AgentRecord {
  agent_id: string;
  /** The registration origin — gates every mutation of the record. */
  owner: SagaOrigin;
  display_name: string;
  capability: string;
  /** sha256 of the agent's prompt content — exactly 32 bytes. */
  prompt_hash: number[];
  /** The document module doc holding the prompt content, when the prompt is
   *  consensus-resident; its canonical rendering must hash to `prompt_hash`. */
  prompt_doc: string | null;
  /** Granted action names, each from `KNOWN_ACTIONS`; sorted and deduped. */
  allowed_actions: string[];
  status: AgentStatus;
  created_at: number;
  updated_at: number;
}

const TARGET = "agent";

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

// ── Msgs (writes — one submit = one block; owner from origin) ──

export const registerAgent = (
  transport: NodeTransport,
  params: {
    agentId: string;
    displayName: string;
    capability: string;
    /** Exactly 32 bytes — see hexToBytes / the prompt-upload flow. */
    promptHash: number[];
    /** Document module doc id holding the prompt content, when set. */
    promptDoc?: string | null;
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
        prompt_doc: params.promptDoc ?? null,
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
    promptDoc?: string | null;
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
        prompt_doc: params.promptDoc ?? null,
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
