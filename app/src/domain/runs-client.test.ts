// The runs client mirrors runs-interface: RunsMsg encoding (the requester
// from the block origin, never in a payload; snake_case fields) + RunsReply
// decoding for the PendingRuns / Watches queries.

import { describe, expect, it, vi } from "vitest";

import {
  cancelRun,
  dispatchIdForRun,
  enableJobWorker,
  pendingRuns,
  recentRuns,
  reassignRun,
  requestRun,
  unwatchChannel,
  watchChannel,
  watches,
} from "./runs-client";
import type { PendingRun, RunRecord, WatchView } from "./runs-client";
import { makeTransportStub } from "../test/transport-stub";

const stubTransport = (reply?: unknown) =>
  makeTransportStub({ query: vi.fn().mockResolvedValue(reply) });

describe("runs msgs", () => {
  it("derives the host output-ring key from the stable run id", () => {
    expect(dispatchIdForRun("chat\x1fforge:ducktape:56\x1f7\x1fsummarizer")).toBe(
      "ef0d635e287bb66490c26824198278cf8011f5679de48b0faeaf388843e9e5df",
    );
  });

  it("encodes WatchChannel — a unit policy and the Assigned newtype", async () => {
    const transport = stubTransport();

    await watchChannel(transport, {
      channelId: "general",
      policy: "mention",
      origin: "operator",
    });
    expect(transport.submit).toHaveBeenCalledWith(
      "runs",
      { watch_channel: { channel_id: "general", policy: "mention" } },
      "operator",
    );

    await watchChannel(transport, {
      channelId: "general",
      policy: { assigned: "helper" },
      origin: "operator",
    });
    expect(transport.submit).toHaveBeenCalledWith(
      "runs",
      { watch_channel: { channel_id: "general", policy: { assigned: "helper" } } },
      "operator",
    );
  });

  it("encodes UnwatchChannel / RequestRun / CancelRun with the origin", async () => {
    const transport = stubTransport();

    await unwatchChannel(transport, { channelId: "general", origin: "operator" });
    expect(transport.submit).toHaveBeenCalledWith(
      "runs",
      { unwatch_channel: { channel_id: "general" } },
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
      { request_run: { agent_id: "helper", channel_id: "general", anchor_seq: 12 } },
      "operator",
    );

    await cancelRun(transport, { runId: "run-1", origin: "operator" });
    expect(transport.submit).toHaveBeenCalledWith(
      "runs",
      { cancel_run: { run_id: "run-1" } },
      "operator",
    );

    await reassignRun(transport, { runId: "run-1", attempt: 2, origin: "operator" });
    expect(transport.submit).toHaveBeenCalledWith(
      "runs",
      { reassign_run: { run_id: "run-1", attempt: 2 } },
      "operator",
    );
  });

  it("encodes RequestRun demands only when non-empty — key absent otherwise", async () => {
    const transport = stubTransport();

    await requestRun(transport, {
      agentId: "helper",
      channelId: "general",
      anchorSeq: 12,
      origin: "operator",
      demands: { cores: 4, mem_gb: 8 },
    });
    expect(transport.submit).toHaveBeenCalledWith(
      "runs",
      {
        request_run: {
          agent_id: "helper",
          channel_id: "general",
          anchor_seq: 12,
          demands: { cores: 4, mem_gb: 8 },
        },
      },
      "operator",
    );

    // Empty demands → the field is omitted entirely (never send `{}`: consensus
    // reads a missing key as legacy, but an empty map is not valid wire).
    await requestRun(transport, {
      agentId: "helper",
      channelId: "general",
      anchorSeq: 12,
      origin: "operator",
      demands: {},
    });
    const calls = vi.mocked(transport.submit).mock.calls;
    const msg = calls[calls.length - 1][1] as {
      request_run: Record<string, unknown>;
    };
    expect(msg.request_run).not.toHaveProperty("demands");
    expect(msg).toEqual({
      request_run: { agent_id: "helper", channel_id: "general", anchor_seq: 12 },
    });
  });

  it("encodes EnableJobWorker with the origin", async () => {
    const transport = stubTransport();
    await enableJobWorker(transport, { enabled: true, origin: "operator" });
    expect(transport.submit).toHaveBeenCalledWith(
      "runs",
      { enable_job_worker: { enabled: true } },
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
      requester: { external: [1] },
      created_at: 1,
    };
    const transport = stubTransport({ pending_runs: [view] });
    await expect(pendingRuns(transport)).resolves.toEqual([view]);
    expect(transport.query).toHaveBeenCalledWith("runs", "pending_runs");
  });

  it("sends the bare string Watches and decodes the watches", async () => {
    const watch: WatchView = { channel_id: "general", policy: "all" };
    const transport = stubTransport({ watches: [watch] });
    await expect(watches(transport)).resolves.toEqual([watch]);
    expect(transport.query).toHaveBeenCalledWith("runs", "watches");
  });

  it("sends the bare string RecentRuns and decodes the delivered-runs ring", async () => {
    const record: RunRecord = {
      run_id: "run-1",
      agent_id: "helper",
      channel_id: "forge:app:12",
      anchor_seq: 4,
      outcome: "delivered",
      degraded: false,
      created_at: 2,
      delivered_at: 9,
      executing_node: "ab".repeat(32),
      output_ref: "agent/x@1a2b3c4d5e6f",
      pr_number: 7,
    };
    const failed: RunRecord = {
      ...record,
      run_id: "run-2",
      outcome: "failed",
      executing_node: "unknown",
      output_ref: null,
      pr_number: null,
    };
    const transport = stubTransport({ recent_runs: [record, failed] });
    await expect(recentRuns(transport)).resolves.toEqual([record, failed]);
    expect(transport.query).toHaveBeenCalledWith("runs", "recent_runs");
  });
});
