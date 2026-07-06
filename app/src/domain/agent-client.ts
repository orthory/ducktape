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
//     `PromptRef` — which module holds the content (memory), where
//     (`"<path>@<generation>"`), how to resolve it (the renderer), and the
//     sha256 pin the resolved content must hash to. `null` keeps the runs
//     module's generic default prompt. A run whose resolved content no longer
//     hashes to the pin fails deterministically.
//
// Everything is a pure function over an injected NodeTransport.

import type { BlockEvent, NodeTransport } from "./transport";
import { replyVariant } from "./wire";

// ── Wire types (AgentReply records + shared enums, verbatim) ─

/** Origin discriminant shared across modules (saga-interface's SagaOrigin): an
 *  external submitter's raw key bytes, a follow-up module, or genesis/system. */
export type SagaOrigin = { external: number[] } | { module: string } | "system";

/** Whether an agent may engage new runs. `tombstoned` is TERMINAL: the record
 *  stays for audit, the id stays reserved, and no mutation (resume included)
 *  is accepted. */
export type AgentStatus = "active" | "paused" | "tombstoned";

/** The v1 prompt renderer: `target` is `"<path>@<generation>"`, resolved at
 *  compose time via the memory module's generation read. */
export const RENDERER_MEMORY_GENERATION = "memory.generation";

/** A consensus-resident reference to an agent's prompt content — the mirror of
 *  the agent crate's `PromptRef`. */
export interface PromptRef {
  /** The module holding the content (v1: "memory"). */
  module: string;
  /** Renderer-specific coordinate — `"<path>@<generation>"` for
   *  memory.generation. */
  target: string;
  /** One of the platform's known renderers (v1: memory.generation). */
  renderer: string;
  /** sha256 of the prompt text — exactly 32 bytes. */
  sha256: number[];
}

export interface AgentRecord {
  agent_id: string;
  /** The registration origin — gates every mutation of the record. */
  owner: SagaOrigin;
  display_name: string;
  capability: string;
  /** Where the agent's prompt lives and what it must hash to; `null` keeps
   *  the runs module's generic default prompt. */
  prompt: PromptRef | null;
  /** Granted action names (shape-validated open-set tags); sorted and deduped. */
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

// ── Prompt pin helpers ───────────────────────────────

/** A 64-char lowercase-hex digest → the 32 byte ints the wire carries as a
 *  PromptRef's `sha256` pin. */
export const hexToBytes = (hex: string): number[] =>
  Array.from({ length: Math.floor(hex.length / 2) }, (_, i) =>
    parseInt(hex.slice(i * 2, i * 2 + 2), 16),
  );

/** Build the memory-generation PromptRef for content published at
 *  `path` / `generation` whose sha256 is `sha256Hex` (64 lowercase hex). */
export const memoryPromptRef = (params: {
  path: string;
  generation: number;
  sha256Hex: string;
}): PromptRef => ({
  module: "memory",
  target: `${params.path}@${params.generation}`,
  renderer: RENDERER_MEMORY_GENERATION,
  sha256: hexToBytes(params.sha256Hex),
});

// ── Msgs (writes — one submit = one block; owner from origin) ──

export const registerAgent = (
  transport: NodeTransport,
  params: {
    agentId: string;
    displayName: string;
    capability: string;
    /** `null` keeps the runs module's generic default prompt. */
    prompt: PromptRef | null;
    allowedActions: string[];
    origin: string;
  },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    {
      register_agent: {
        agent_id: params.agentId,
        display_name: params.displayName,
        capability: params.capability,
        prompt: params.prompt,
        allowed_actions: params.allowedActions,
      },
    },
    params.origin,
  );

/** Owner-gated partial update — an omitted (null) field keeps its value
 *  (clearing a registered prompt means re-registering). */
export const updateAgent = (
  transport: NodeTransport,
  params: {
    agentId: string;
    displayName?: string | null;
    capability?: string | null;
    prompt?: PromptRef | null;
    allowedActions?: string[] | null;
    origin: string;
  },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    {
      update_agent: {
        agent_id: params.agentId,
        display_name: params.displayName ?? null,
        capability: params.capability ?? null,
        prompt: params.prompt ?? null,
        allowed_actions: params.allowedActions ?? null,
      },
    },
    params.origin,
  );

export const pauseAgent = (
  transport: NodeTransport,
  params: { agentId: string; origin: string },
): Promise<BlockEvent> =>
  transport.submit(TARGET, { pause_agent: { agent_id: params.agentId } }, params.origin);

export const resumeAgent = (
  transport: NodeTransport,
  params: { agentId: string; origin: string },
): Promise<BlockEvent> =>
  transport.submit(TARGET, { resume_agent: { agent_id: params.agentId } }, params.origin);

/** Owner-gated and TERMINAL: retire the agent for good. The record stays for
 *  audit, the dispatch recipe is torn down in the same block, and no later
 *  mutation (resume included) is accepted. */
export const tombstoneAgent = (
  transport: NodeTransport,
  params: { agentId: string; origin: string },
): Promise<BlockEvent> =>
  transport.submit(TARGET, { tombstone_agent: { agent_id: params.agentId } }, params.origin);

// ── Queries (reads over committed state) ────────────────

/** Every registered agent. `Agents` is a unit-variant query — the bare
 *  string, like tasks-client's `List`. */
export const agents = (transport: NodeTransport): Promise<AgentRecord[]> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, "agents"))
    .then((reply) => replyVariant<AgentRecord[]>(reply, "agents"));

/** One agent by id, or null when absent. */
export const agent = (
  transport: NodeTransport,
  agentId: string,
): Promise<AgentRecord | null> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, { agent: { agent_id: agentId } }))
    .then((reply) => replyVariant<AgentRecord | null>(reply, "agent"));
