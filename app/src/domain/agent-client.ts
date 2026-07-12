// Typed client for the node's `agent` module — the TS mirror of
// `crates/apps/agent-interface`. The agent module is the platform's agent
// REGISTRY and nothing more: a self-contained record book of which agents
// exist — owner, capability tag, curated skills, granted actions, status. The
// acting half of the collaboration loop (watches, runs, cancellation) lives
// in the runs module — see `runs-client`.
//
// This file plays the interface crate's role on the client side. Key contract
// points mirrored here:
//   - ownership is NEVER in a write payload — the module derives an agent's
//     owner from the block's origin, so every write function takes an
//     `origin` and passes it to transport.submit, exactly like chat-client.
//   - an agent's SOUL is its curated skill set, not a stored prompt blob.
//     Each `SkillRef` names a duckfs prefix; `load: "always"` inlines that
//     document into every run's assembled context (the persona), `on_demand`
//     only lists it so the agent can read it from its skill mount when the
//     task calls for it. Consensus pins the prefixes (and optional snapshots);
//     `prompt_hash` and the blob-store prompt path are gone.
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

/** How a curated skill reaches the run. `always` inlines the whole document
 *  into the assembled context every run — that IS the agent's persona;
 *  `on_demand` only indexes it, and the agent reads the body from its skill
 *  mount when the task calls for it. Omitted on the wire ⇒ the node defaults
 *  to `on_demand`. */
export type LoadMode = "always" | "on_demand";

/** One curated skill: a duckfs prefix (the skill's directory, holding its
 *  `SKILL.md`), optionally pinned to a snapshot, plus its load mode. Order is
 *  meaningful — the assembler inlines `always` bodies in curation order. */
export interface SkillRef {
  name: string;
  source_prefix: string;
  /** A snapshot pin; omitted/null means the run's head. */
  source_snapshot?: string | null;
  load: LoadMode;
}

export interface AgentRecord {
  agent_id: string;
  /** The registration origin — gates every mutation of the record. */
  owner: SagaOrigin;
  display_name: string;
  capability: string;
  /** Granted action names, each from `KNOWN_ACTIONS`; sorted and deduped. */
  allowed_actions: string[];
  status: AgentStatus;
  created_at: number;
  updated_at: number;
  /** D3 resource caps — absent on the wire when default-empty. */
  caps?: ResourceCaps;
  /** The curated skill set, in order — absent on the wire when empty. */
  skills?: SkillRef[];
}

const TARGET = "agent";

/** The agent's address, or `null` when there is no honest one to show.
 *
 *  `agent_id` is a DNS label by consensus rule (`validate_agent_id`,
 *  crates/apps/agent/src/lib.rs), and a label IS an RFC 5321 local part verbatim
 *  — so the id is the ident forge attributes every agent commit to
 *  (`bin/noded/src/agent_provision/forge.rs`). `agents` is a RESERVED root label
 *  in duckdns: no account can register the handle and inherit these addresses.
 *
 *  LEGACY agents, registered before that rule, may hold any id — and forge does
 *  NOT address them this way. It derives `<slug>.<hash>@agents.duck` from the
 *  complete id. Reproducing that here means sha256 in a render path; printing
 *  `<id>@agents.duck` anyway would print an address that is simply NOT the
 *  agent's. Show nothing rather than something false. */
export const agentAddress = (agentId: string): string | null =>
  /^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$/.test(agentId) ? `${agentId}@agents.duck` : null;

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

// ── Hex helper ──────────────────────────────────────────

/** A lowercase-hex string → the byte ints the wire carries. Lives here for
 *  historical reasons; identity-client and chat-client re-export it under
 *  their own vocabulary. */
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
    allowedActions: string[];
    /** D3 resource caps; omit to register with the empty default. */
    caps?: ResourceCaps;
    /** The curated skill set, in order; omit to register with none. */
    skills?: SkillRef[];
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
        allowed_actions: params.allowedActions,
        ...(params.caps ? { caps: params.caps } : {}),
        ...(params.skills?.length ? { skills: params.skills } : {}),
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
    allowedActions?: string[] | null;
    /** A provided value REPLACES the whole caps record (send the full caps,
     *  not a patch); null/omitted keeps the current one. */
    caps?: ResourceCaps | null;
    /** A provided list REPLACES the whole curated set (an empty array clears
     *  it); null/omitted keeps the current one. */
    skills?: SkillRef[] | null;
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
        allowed_actions: params.allowedActions ?? null,
        caps: params.caps ?? null,
        skills: params.skills ?? null,
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
