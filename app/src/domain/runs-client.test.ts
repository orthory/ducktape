// The runs client mirrors runs-interface: RunsMsg encoding (the requester
// from the block origin, never in a payload; snake_case fields) + RunsReply
// decoding for the PendingRuns / Watches queries.

import { describe, expect, it, vi } from "vitest";

import {
  cancelRun,
  enableJobWorker,
  pendingRuns,
  requestRun,
  unwatchChannel,
  watchChannel,
  watches,
} from "./runs-client";
import type { PendingRun, WatchView } from "./runs-client";
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

describe("runs msgs", () => {
  it("encodes WatchChannel — a unit policy and the Assigned newtype", async () => {
    const transport = stubTransport();

    await watchChannel(transport, {
      channelId: "general",
      policy: "Mention",
      origin: "operator",
    });
    expect(transport.submit).toHaveBeenCalledWith(
      "runs",
      { WatchChannel: { channel_id: "general", policy: "Mention" } },
      "operator",
    );

    await watchChannel(transport, {
      channelId: "general",
      policy: { Assigned: "helper" },
      origin: "operator",
    });
    expect(transport.submit).toHaveBeenCalledWith(
      "runs",
      { WatchChannel: { channel_id: "general", policy: { Assigned: "helper" } } },
      "operator",
    );
  });

  it("encodes UnwatchChannel / RequestRun / CancelRun with the origin", async () => {
    const transport = stubTransport();

    await unwatchChannel(transport, { channelId: "general", origin: "operator" });
    expect(transport.submit).toHaveBeenCalledWith(
      "runs",
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
      "runs",
      { RequestRun: { agent_id: "helper", channel_id: "general", anchor_seq: 12 } },
      "operator",
    );

    await cancelRun(transport, { runId: "run-1", origin: "operator" });
    expect(transport.submit).toHaveBeenCalledWith(
      "runs",
      { CancelRun: { run_id: "run-1" } },
      "operator",
    );
  });

  it("encodes EnableJobWorker with the origin", async () => {
    const transport = stubTransport();
    await enableJobWorker(transport, { enabled: true, origin: "operator" });
    expect(transport.submit).toHaveBeenCalledWith(
      "runs",
      { EnableJobWorker: { enabled: true } },
      "operator",
    );
  });
});

describe("runs queries", () => {
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
    expect(transport.query).toHaveBeenCalledWith("runs", "PendingRuns");
  });

  it("sends the bare string Watches and decodes the watches", async () => {
    const watch: WatchView = { channel_id: "general", policy: "All" };
    const transport = stubTransport({ Watches: [watch] });
    await expect(watches(transport)).resolves.toEqual([watch]);
    expect(transport.query).toHaveBeenCalledWith("runs", "Watches");
  });
});
