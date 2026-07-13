// The agent client mirrors agent-interface: AgentMsg encoding (ownership from
// the block origin, never in a payload; snake_case fields) + AgentReply
// decoding for the Agents / Agent queries, including the null (absent agent)
// case. An agent's soul is its curated skills, so the wire carries SkillRefs
// with their load mode and NO prompt/prompt_hash at all — both asserted here.
// The acting half (watches, runs) is runs-client — see runs-client.test.

import { describe, expect, it, vi } from "vitest";

import {
  agent,
  agentAddress,
  agents,
  hexToBytes,
  KNOWN_ACTIONS,
  pauseAgent,
  registerAgent,
  resumeAgent,
  updateAgent,
} from "./agent-client";
import type { AgentRecord, SkillRef } from "./agent-client";
import { RESERVED_ROOT_LABELS } from "./duckdns-client";
import { makeTransportStub } from "../test/transport-stub";

const stubTransport = (reply?: unknown) =>
  makeTransportStub({ query: vi.fn().mockResolvedValue(reply) });

describe("hexToBytes", () => {
  it("pairs hex chars into 32 byte ints for a digest", () => {
    const digest = "ab".repeat(32);
    const bytes = hexToBytes(digest);
    expect(bytes).toHaveLength(32);
    expect(bytes.every((b) => b === 0xab)).toBe(true);
  });

  it("decodes a mixed digest positionally", () => {
    expect(hexToBytes("00ff10")).toEqual([0, 255, 16]);
  });
});

describe("KNOWN_ACTIONS", () => {
  it("includes the DuckFS text write grant surfaced by the agent module", () => {
    expect(KNOWN_ACTIONS).toContain("duckfs.write_text");
  });
});

describe("agent msgs", () => {
  const persona: SkillRef = {
    name: "persona",
    source_prefix: "/shared/agents/helper/persona",
    load: "always",
  };
  const runbook: SkillRef = {
    name: "runbook",
    source_prefix: "/shared/skills/runbook",
    source_snapshot: "cafe",
    load: "on_demand",
  };

  it("encodes RegisterAgent, passing the origin for owner-gating", async () => {
    const transport = stubTransport();
    await registerAgent(transport, {
      agentId: "helper",
      displayName: "Helper",
      capability: "alpha",
      allowedActions: ["chat.post"],
      origin: "operator",
    });
    // Exact match: no prompt / prompt_hash key is ever sent, and an empty skill
    // set is omitted rather than sent as [].
    expect(transport.submit).toHaveBeenCalledWith(
      "agent",
      {
        register_agent: {
          agent_id: "helper",
          display_name: "Helper",
          capability: "alpha",
          allowed_actions: ["chat.post"],
        },
      },
      "operator",
    );
  });

  it("carries the curated skills — each with its load mode — in order", async () => {
    const transport = stubTransport();
    await registerAgent(transport, {
      agentId: "helper",
      displayName: "Helper",
      capability: "alpha",
      allowedActions: ["chat.post"],
      skills: [persona, runbook],
      origin: "operator",
    });
    expect(transport.submit).toHaveBeenCalledWith(
      "agent",
      {
        register_agent: {
          agent_id: "helper",
          display_name: "Helper",
          capability: "alpha",
          allowed_actions: ["chat.post"],
          skills: [persona, runbook],
        },
      },
      "operator",
    );
  });

  it("encodes UpdateAgent, filling omitted fields with null", async () => {
    const transport = stubTransport();
    await updateAgent(transport, {
      agentId: "helper",
      displayName: "Helper 2",
      origin: "operator",
    });
    expect(transport.submit).toHaveBeenCalledWith(
      "agent",
      {
        update_agent: {
          agent_id: "helper",
          display_name: "Helper 2",
          capability: null,
          allowed_actions: null,
          caps: null,
          skills: null,
        },
      },
      "operator",
    );
  });

  it("UpdateAgent replaces the curated set wholesale — [] clears it", async () => {
    const transport = stubTransport();
    await updateAgent(transport, { agentId: "helper", skills: [], origin: "operator" });
    expect(transport.submit).toHaveBeenCalledWith(
      "agent",
      {
        update_agent: {
          agent_id: "helper",
          display_name: null,
          capability: null,
          allowed_actions: null,
          caps: null,
          skills: [],
        },
      },
      "operator",
    );
  });

  it("encodes PauseAgent / ResumeAgent with the origin", async () => {
    const transport = stubTransport();

    await pauseAgent(transport, { agentId: "helper", origin: "operator" });
    expect(transport.submit).toHaveBeenCalledWith(
      "agent",
      { pause_agent: { agent_id: "helper" } },
      "operator",
    );

    await resumeAgent(transport, { agentId: "helper", origin: "operator" });
    expect(transport.submit).toHaveBeenCalledWith(
      "agent",
      { resume_agent: { agent_id: "helper" } },
      "operator",
    );
  });
});

describe("agent queries", () => {
  const record: AgentRecord = {
    agent_id: "helper",
    owner: { external: [1, 2, 3] },
    display_name: "Helper",
    capability: "alpha",
    allowed_actions: ["chat.post"],
    status: "active",
    created_at: 1,
    updated_at: 2,
    skills: [
      { name: "persona", source_prefix: "/shared/agents/helper/persona", load: "always" },
    ],
  };

  it("sends the bare string Agents and decodes the roster", async () => {
    const transport = stubTransport({ agents: [record] });
    await expect(agents(transport)).resolves.toEqual([record]);
    expect(transport.query).toHaveBeenCalledWith("agent", "agents");
  });

  it("sends Agent{agent_id} and decodes Agent, including null", async () => {
    const present = stubTransport({ agent: record });
    await expect(agent(present, "helper")).resolves.toEqual(record);
    expect(present.query).toHaveBeenCalledWith("agent", {
      agent: { agent_id: "helper" },
    });

    const absent = stubTransport({ agent: null });
    await expect(agent(absent, "ghost")).resolves.toBeNull();
  });
});

// An address the agent cannot be reached at is worse than no address. Consensus
// admits only DNS-label ids, and those ARE the local part verbatim. A LEGACY
// agent (registered before that rule) holds any id, and forge addresses it as
// `<slug>.<hash>@agents.duck` off the complete id — so `<id>@agents.duck` would
// be a plain lie. Show nothing instead.
describe("agent address", () => {
  it("is the id verbatim for every id consensus admits", () => {
    for (const id of ["quackbot", "qa-luna", "a", "9", "a--b", "x".repeat(63)]) {
      expect(agentAddress(id)).toBe(`${id}@agents.duck`);
    }
  });

  it("is absent for a legacy id, never a false address", () => {
    for (const legacy of [
      "qa luna",
      "QA-Luna",
      "quack/bot@example",
      "under_score",
      "dot.ted",
      "-lead",
      "trail-",
      "",
      "x".repeat(64),
    ]) {
      expect(agentAddress(legacy), legacy).toBeNull();
    }
  });

  // the domain is unownable only because `agents` is reserved in duckdns.
  it("lives under the reserved root label", () => {
    expect(RESERVED_ROOT_LABELS.has("agents")).toBe(true);
    expect(agentAddress("quackbot")).toContain("@agents.duck");
  });
});
