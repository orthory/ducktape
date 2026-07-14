// The usage client mirrors saga's index wire: `{"usage": {...}}` view request,
// `{"usage": [<UsageRow>…]}` reply, then the app-side executor→account join
// (identity OfNode) and per-account/per-capability grouping.

import { describe, expect, it, vi } from "vitest";

import type { AccountView } from "./identity-client";
import { accountUsage, usageRows, type UsageRow } from "./usage-client";
import { makeTransportStub } from "../test/transport-stub";

const row = (patch: Partial<UsageRow> = {}): UsageRow => ({
  executorHex: "aa11",
  capability: "model-1",
  outcomeOk: true,
  runs: 2,
  totalDurationBlocks: 9,
  inputTokens: 0,
  cachedInputTokens: 0,
  cacheWriteInputTokens: 0,
  outputTokens: 0,
  reasoningOutputTokens: 0,
  ...patch,
});

// account_id [1,2,3] renders as hex "010203".
const account = (patch: Partial<AccountView> = {}): AccountView => ({
  account_id: [1, 2, 3],
  display_name: "jess",
  avatar: null,
  bio: null,
  nonce: 0,
  member_keys: [],
  nodes: [],
  updated_at: 1,
  ...patch,
});

describe("usageRows", () => {
  it("sends the usage view request and decodes the reply variant", async () => {
    const wire = [row()];
    const transport = makeTransportStub({
      view: vi.fn().mockResolvedValue({ usage: wire }),
    });
    await expect(usageRows(transport, { sinceHeight: 100 })).resolves.toEqual(wire);
    expect(transport.view).toHaveBeenCalledWith("saga", {
      usage: { sinceHeight: 100 },
    });
  });

  it("throws on a mismatched reply variant", async () => {
    const transport = makeTransportStub({
      view: vi.fn().mockResolvedValue({ hits: [] }),
    });
    await expect(usageRows(transport)).rejects.toThrow("wanted usage");
  });
});

describe("accountUsage", () => {
  it("groups executors by resolved account with a per-capability breakdown", async () => {
    // two executor nodes bound to the SAME account, plus one unbound node.
    const wire = [
      row({
        executorHex: "aa11",
        capability: "model-1",
        runs: 2,
        totalDurationBlocks: 9,
        inputTokens: 100,
        cachedInputTokens: 60,
        outputTokens: 20,
      }),
      row({
        executorHex: "aa11",
        capability: "model-1",
        outcomeOk: false,
        runs: 1,
        totalDurationBlocks: 4,
      }),
      row({
        executorHex: "bb22",
        capability: "codex",
        runs: 3,
        totalDurationBlocks: 12,
        inputTokens: 50,
        outputTokens: 10,
      }),
      row({ executorHex: "cc33", capability: "model-1", runs: 1, totalDurationBlocks: 2 }),
    ];
    const transport = makeTransportStub({
      view: vi.fn().mockResolvedValue({ usage: wire }),
      query: vi.fn().mockImplementation((_module: string, req: unknown) => {
        const key = (req as { of_node: { node_key: number[] } }).of_node.node_key;
        // aa11 → [0xaa,0x11], bb22 → [0xbb,0x22] both bind to jess's account;
        // cc33 stays unbound.
        const bound = key[0] === 0xaa || key[0] === 0xbb;
        return Promise.resolve({ account: bound ? account() : null });
      }),
    });

    const groups = await accountUsage(transport);
    expect(groups).toHaveLength(2);

    // jess carries both her nodes' rows, runs-desc: 2+1+3 = 6 runs, 1 failed.
    expect(groups[0]).toMatchObject({
      label: "jess",
      accountIdHex: "010203",
      runs: 6,
      failed: 1,
      totalDurationBlocks: 25,
      inputTokens: 150,
      outputTokens: 30,
      cachedInputTokens: 60,
    });
    // runs tie → stable sort keeps first-seen order (model-1 folded first).
    expect(groups[0].byCapability).toEqual([
      {
        capability: "model-1",
        runs: 3,
        failed: 1,
        totalDurationBlocks: 13,
        inputTokens: 100,
        cachedInputTokens: 60,
        cacheWriteInputTokens: 0,
        outputTokens: 20,
        reasoningOutputTokens: 0,
      },
      {
        capability: "codex",
        runs: 3,
        failed: 0,
        totalDurationBlocks: 12,
        inputTokens: 50,
        cachedInputTokens: 0,
        cacheWriteInputTokens: 0,
        outputTokens: 10,
        reasoningOutputTokens: 0,
      },
    ]);

    // the unbound node groups under its own key hex.
    expect(groups[1]).toMatchObject({
      label: "cc33",
      accountIdHex: null,
      runs: 1,
      failed: 0,
      totalDurationBlocks: 2,
    });
  });

  it("returns an empty ledger when the view has no rows", async () => {
    const transport = makeTransportStub({
      view: vi.fn().mockResolvedValue({ usage: [] }),
    });
    await expect(accountUsage(transport)).resolves.toEqual([]);
    // no executors → no identity lookups.
    expect(transport.query).not.toHaveBeenCalled();
  });

  it("treats a failed OfNode lookup as unbound, not a card failure", async () => {
    const transport = makeTransportStub({
      view: vi.fn().mockResolvedValue({ usage: [row()] }),
      query: vi.fn().mockRejectedValue(new Error("identity offline")),
    });
    const groups = await accountUsage(transport);
    expect(groups).toHaveLength(1);
    expect(groups[0].accountIdHex).toBeNull();
    expect(groups[0].label).toBe("aa11");
  });
});
