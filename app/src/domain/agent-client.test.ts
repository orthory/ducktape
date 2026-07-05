// The agent client mirrors agent-interface: AgentMsg encoding (ownership from
// the block origin, never in a payload; snake_case fields) + AgentReply
// decoding for the Agents / Agent / PendingRuns / Watches queries, including
// the null (absent agent) case. The prompt-upload flow itself lives in the
// store; here we only prove the wire shapes and the hex→bytes hash helper.

import { describe, expect, it, vi } from "vitest";

import {
  agent,
  agents,
  cancelRun,
  hexToBytes,
  pauseAgent,
  registerAgent,
  pendingRuns,
  requestRun,
  resumeAgent,
  unwatchChannel,
  updateAgent,
  watchChannel,
  watches,
} from "./agent-client";
import type { AgentRecord, PendingRun, WatchView } from "./agent-client";
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
        RegisterAgent: {
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
        UpdateAgent: {
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
      { PauseAgent: { agent_id: "helper" } },
      "operator",
    );

    await resumeAgent(transport, { agentId: "helper", origin: "operator" });
    expect(transport.submit).toHaveBeenCalledWith(
      "agent",
      { ResumeAgent: { agent_id: "helper" } },
      "operator",
    );
  });

  it("encodes WatchChannel — a unit policy and the Assigned newtype", async () => {
    const transport = stubTransport();

    await watchChannel(transport, {
      channelId: "general",
      policy: "Mention",
      origin: "operator",
    });
    expect(transport.submit).toHaveBeenCalledWith(
      "agent",
      { WatchChannel: { channel_id: "general", policy: "Mention" } },
      "operator",
    );

    await watchChannel(transport, {
      channelId: "general",
      policy: { Assigned: "helper" },
      origin: "operator",
    });
    expect(transport.submit).toHaveBeenCalledWith(
      "agent",
      { WatchChannel: { channel_id: "general", policy: { Assigned: "helper" } } },
      "operator",
    );
  });

  it("encodes UnwatchChannel / RequestRun / CancelRun with the origin", async () => {
    const transport = stubTransport();

    await unwatchChannel(transport, { channelId: "general", origin: "operator" });
    expect(transport.submit).toHaveBeenCalledWith(
      "agent",
      { UnwatchChannel: { channel_id: "general" } },
      "operator",
    );

    await requestRun(transport, {
      agentId: "helper",
      channelId: "general",
      anchorSeq: 12,
      origin: "operator",
    });
    expect(transport.submit).toHaveBeenCalledWith(
      "agent",
      { RequestRun: { agent_id: "helper", channel_id: "general", anchor_seq: 12 } },
      "operator",
    );

    await cancelRun(transport, { runId: "run-1", origin: "operator" });
    expect(transport.submit).toHaveBeenCalledWith(
      "agent",
      { CancelRun: { run_id: "run-1" } },
      "operator",
    );
  });
});

describe("agent queries", () => {
  const record: AgentRecord = {
    agent_id: "helper",
    owner: { External: [1, 2, 3] },
    display_name: "Helper",
    capability: "alpha",
    prompt_hash: hexToBytes("cd".repeat(32)),
    prompt_doc: null,
    allowed_actions: ["chat.post"],
    status: "Active",
    created_at: 1,
    updated_at: 2,
  };

  it("sends the bare string Agents and decodes the roster", async () => {
    const transport = stubTransport({ Agents: [record] });
    await expect(agents(transport)).resolves.toEqual([record]);
    expect(transport.query).toHaveBeenCalledWith("agent", "Agents");
  });

  it("sends Agent{agent_id} and decodes Agent, including null", async () => {
    const present = stubTransport({ Agent: record });
    await expect(agent(present, "helper")).resolves.toEqual(record);
    expect(present.query).toHaveBeenCalledWith("agent", {
      Agent: { agent_id: "helper" },
    });

    const absent = stubTransport({ Agent: null });
    await expect(agent(absent, "ghost")).resolves.toBeNull();
  });

  it("sends the bare string PendingRuns and decodes the in-flight entries", async () => {
    const view: PendingRun = {
      run_id: "run-1",
      dispatch_id: "ab".repeat(32),
      agent_id: "helper",
      channel_id: "general",
      anchor_seq: 4,
      thread_root: null,
      job_id: null,
      job_claim_height: 0,
      requester: { External: [1] },
      created_at: 1,
    };
    const transport = stubTransport({ PendingRuns: [view] });
    await expect(pendingRuns(transport)).resolves.toEqual([view]);
    expect(transport.query).toHaveBeenCalledWith("agent", "PendingRuns");
  });

  it("sends the bare string Watches and decodes the watches", async () => {
    const watch: WatchView = { channel_id: "general", policy: "All" };
    const transport = stubTransport({ Watches: [watch] });
    await expect(watches(transport)).resolves.toEqual([watch]);
    expect(transport.query).toHaveBeenCalledWith("agent", "Watches");
  });
});
