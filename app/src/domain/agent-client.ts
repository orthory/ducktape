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
//     32-byte `prompt_hash` pin — the sha256 of the prompt text. the content
//     itself is content-addressed in the node's blob store (keyed by that
//     digest), so every run composes its prompt from the blob the pin resolves
//     to.
//
// Everything is a pure function over an injected NodeTransport.

import type { BlockEvent, NodeTransport } from "./transport";
import { replyVariant } from "./wire";

// ── Wire types (AgentReply records + shared enums, verbatim) ─

/** Origin discriminant shared across modules (saga-interface's SagaOrigin): an
 *  external submitter's raw key bytes, a follow-up module, or genesis/system. */
export type SagaOrigin = { external: number[] } | { module: string } | "system";

/** Whether an agent may engage new runs. */
export type AgentStatus = "active" | "paused";

/** The D3 resource-capability grant (agent-interface's ResourceCaps). Every
 *  list is canonical sorted+deduped on the node; the wire omits empty lists.
 *  `pages_write` is page-id scoped with the literal `"*"` granting every
 *  page (exact match — no prefixes). */
export interface ResourceCaps {
  forge_read?: string[];
  forge_push?: string[];
  duckfs_read?: string[];
  duckfs_write?: string[];
  tools?: string[];
  secrets?: string[];
  pages_write?: string[];
  subagent_budget?: number;
}

export interface AgentRecord {
  agent_id: string;
  /** The registration origin — gates every mutation of the record. */
  owner: SagaOrigin;
  display_name: string;
  capability: string;
  /** sha256 of the agent's prompt content — exactly 32 bytes. The content is
   *  content-addressed in the node's blob store under this digest. */
  prompt_hash: number[];
  /** Granted action names, each from `KNOWN_ACTIONS`; sorted and deduped. */
  allowed_actions: string[];
  status: AgentStatus;
  created_at: number;
  updated_at: number;
  /** D3 resource caps — absent on the wire when default-empty. */
  caps?: ResourceCaps;
}

const TARGET = "agent";

/** Every action name an agent can be granted (KNOWN_ACTIONS). A RegisterAgent /
 *  UpdateAgent rejects an `allowed_actions` entry outside this set.
 *
 *  Mirrors `agent::KNOWN_ACTIONS`. The node is the authority — an entry missing
 *  here is simply ungrantable from the UI (the checkbox never renders), which is
 *  a silent loss of a permission rather than an error, so the two lists have to
 *  be kept in step by hand. */
export const KNOWN_ACTIONS = [
  "chat.post",
  "chat.post_message",
  "tasks.create",
  "tasks.update_status",
  "pages.comment",
  "pages.set_checked",
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
    allowedActions: string[];
    /** D3 resource caps; omit to register with the empty default. */
    caps?: ResourceCaps;
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
        prompt_hash: params.promptHash,
        allowed_actions: params.allowedActions,
        ...(params.caps ? { caps: params.caps } : {}),
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
    /** A provided value REPLACES the whole caps record (send the full caps,
     *  not a patch); null/omitted keeps the current one. */
    caps?: ResourceCaps | null;
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
        prompt_hash: params.promptHash ?? null,
        allowed_actions: params.allowedActions ?? null,
        caps: params.caps ?? null,
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
