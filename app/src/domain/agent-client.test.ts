// The agent client mirrors agent-interface: AgentMsg encoding (ownership from
// the block origin, never in a payload; snake_case fields) + AgentReply
// decoding for the Agents / Agent queries, including the null (absent agent)
// case. The prompt-upload flow itself lives in the store; here we only prove
// the wire shapes and the hex→bytes hash helper. The acting half (watches,
// runs) is runs-client — see runs-client.test.

import { describe, expect, it, vi } from "vitest";

import {
  agent,
  agents,
  hexToBytes,
  pauseAgent,
  registerAgent,
  resumeAgent,
  updateAgent,
} from "./agent-client";
import type { AgentRecord } from "./agent-client";
import type { NodeTransport } from "./transport";

const stubTransport = (reply?: unknown): NodeTransport => ({
  submit: vi.fn().mockResolvedValue({ height: 1, appHash: "aa".repeat(32) }),
  query: vi.fn().mockResolvedValue(reply),
  view: vi.fn(),
  putBlob: vi.fn().mockResolvedValue("ab".repeat(32)),
  getBlob: vi.fn().mockResolvedValue(new Uint8Array()),
  status: vi.fn(),
  telemetry: vi.fn(),
  blocks: vi.fn(),
  onBlock: vi.fn(),
  onTelemetry: vi.fn(),
});

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

describe("agent msgs", () => {
  it("encodes RegisterAgent, passing the origin for owner-gating", async () => {
    const transport = stubTransport();
    const promptHash = hexToBytes("cd".repeat(32));
    await registerAgent(transport, {
      agentId: "helper",
      displayName: "Helper",
      capability: "alpha",
      promptHash,
      allowedActions: ["chat.post"],
      origin: "operator",
    });
    expect(transport.submit).toHaveBeenCalledWith(
      "agent",
      {
        register_agent: {
          agent_id: "helper",
          display_name: "Helper",
          capability: "alpha",
          prompt_hash: promptHash,
          prompt_doc: null,
          allowed_actions: ["chat.post"],
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
          prompt_hash: null,
          prompt_doc: null,
          allowed_actions: null,
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
    prompt_hash: hexToBytes("cd".repeat(32)),
    prompt_doc: null,
    allowed_actions: ["chat.post"],
    status: "active",
    created_at: 1,
    updated_at: 2,
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
