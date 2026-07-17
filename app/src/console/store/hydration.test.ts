// The scoped-hydration decisions, proven without React: root diffing names
// exactly the modules a block moved, the module → slice-group map fans out to
// the right re-queries (and nothing else), and the read-your-writes floor
// tracks the highest receipted height so a lagging snapshot never un-renders
// a confirmed write.

import { describe, expect, it, vi } from "vitest";

import type { NodeStatus } from "../../domain/transport";
import { makeTransportStub } from "../../test/transport-stub";
import { receiptFloor } from "./finalization";
import type { OpLedger } from "./finalization";
import {
  changedModules,
  fetchCapabilitySlices,
  fetchChatSlices,
  fetchGovernanceSlices,
  fetchPeopleSlices,
  scopeFor,
} from "./hydration";

// ── Fixtures ────────────────────────────────────────────

const statusWith = (roots: Record<string, string>): NodeStatus => ({
  version: "test",
  appHash: "aa",
  height: 7,
  modules: Object.entries(roots).map(([id, root]) => ({ id, root })),
});

// ── Root diffing ────────────────────────────────────────

describe("changedModules", () => {
  it("reads a first sighting as everything changed", () => {
    const next = statusWith({ chat: "c1", valset: "v1" });
    expect(changedModules(null, next)).toEqual(new Set(["chat", "valset"]));
  });

  it("names exactly the modules whose roots moved", () => {
    const prev = statusWith({ chat: "c1", valset: "v1", files: "f1" });
    const next = statusWith({ chat: "c2", valset: "v1", files: "f1" });
    expect(changedModules(prev, next)).toEqual(new Set(["chat"]));
  });

  it("treats a module the previous status never saw as changed", () => {
    const prev = statusWith({ chat: "c1" });
    const next = statusWith({ chat: "c1", pages: "p1" });
    expect(changedModules(prev, next)).toEqual(new Set(["pages"]));
  });

  it("is empty across an idle stride", () => {
    const prev = statusWith({ chat: "c1", valset: "v1" });
    expect(changedModules(prev, statusWith({ chat: "c1", valset: "v1" }))).toEqual(
      new Set(),
    );
  });
});

// ── Scope mapping ───────────────────────────────────────

describe("scopeFor", () => {
  it("maps a module to its slice group", () => {
    expect(scopeFor(new Set(["chat"]))).toEqual(new Set(["chat"]));
  });

  it("folds the dispatch plane into the runs group", () => {
    expect(scopeFor(new Set(["dispatch", "saga"]))).toEqual(new Set(["runs"]));
  });

  it("groups identity and DuckDNS together as account projections", () => {
    expect(scopeFor(new Set(["identity", "duckdns"]))).toEqual(
      new Set(["people"]),
    );
  });

  it("ignores modules with no console projection", () => {
    expect(scopeFor(new Set(["kv", "blobstore", "tagging", "upgrade"]))).toEqual(
      new Set(),
    );
  });
});

// ── The chat slice's focused window ─────────────────────

describe("fetchChatSlices", () => {
  const channel = {
    id: "general",
    name: "general",
    created_at: 1,
    head_seq: 900,
    post_policy: "open",
    hooks: [],
    pinned: [],
  };
  /** Answers `channels`, then routes the message read by variant. */
  const chatNode = (
    reply: (variant: "messages_latest" | "messages_around") => Promise<unknown>,
  ) =>
    vi.fn((_target: string, query: unknown) => {
      if (query === "channels") return Promise.resolve({ channels: [channel] });
      const variant =
        (query as { messages_around?: unknown }).messages_around !== undefined
          ? "messages_around"
          : "messages_latest";
      return reply(variant);
    });

  it("re-pulls the FOCUSED window rather than the tail while one is up", async () => {
    const query = chatNode((variant) =>
      Promise.resolve({ messages: [{ channel_id: "general", seq: variant === "messages_around" ? 12 : 900 }] }),
    );

    const slices = await fetchChatSlices(makeTransportStub({ query }), "general", {
      channelId: "general",
      seq: 12,
    });

    expect(slices.messages.map((m) => m.seq)).toEqual([12]);
  });

  it("falls back to the tail when the node cannot answer the window query", async () => {
    // An old node rejects messages_around outright. Degrade to the tail — the
    // whole hydrate must not fail over one unknown query variant (loadMessageWindow's
    // own catch is what tells the reader why they are back on the newest messages).
    const query = chatNode((variant) =>
      variant === "messages_around"
        ? Promise.reject(new Error("unknown query variant"))
        : Promise.resolve({ messages: [{ channel_id: "general", seq: 900 }] }),
    );

    const slices = await fetchChatSlices(makeTransportStub({ query }), "general", {
      channelId: "general",
      seq: 12,
    });

    expect(slices.messages.map((m) => m.seq)).toEqual([900]);
    expect(slices.activeChannel).toBe("general");
  });

  it("reads the tail for a window focused on a DIFFERENT channel", async () => {
    const query = chatNode((variant) =>
      variant === "messages_around"
        ? Promise.reject(new Error("should not have been asked"))
        : Promise.resolve({ messages: [{ channel_id: "general", seq: 900 }] }),
    );

    const slices = await fetchChatSlices(makeTransportStub({ query }), "general", {
      channelId: "random",
      seq: 12,
    });

    expect(slices.messages.map((m) => m.seq)).toEqual([900]);
  });
});

describe("fetchPeopleSlices", () => {
  it("projects Identity names and optional DuckDNS aliases from two authoritative modules", async () => {
    const query = vi.fn((target: string) => {
      if (target === "identity") {
        return Promise.resolve({
          accounts: [
            {
              account_id: [10],
              display_name: "Rae",
              nonce: 0,
              member_keys: [],
              nodes: [{ node_key: [11], label: "Rae's box" }],
              updated_at: 1,
            },
          ],
        });
      }
      if (target === "duckdns") {
        return Promise.resolve({
          registrations: [{ handle: "rae", account_id: [10] }],
        });
      }
      throw new Error(`unexpected query target ${target}`);
    });
    const slices = await fetchPeopleSlices(makeTransportStub({ query }));

    // Keyed by the ACCOUNT ("0a") *and* by every node it owns ("0b"): a message
    // author is a node key, but a mention mark carries the account id, and both
    // resolve through the same `authorName` map.
    expect(slices.authorNames).toEqual({ "0a": "Rae", "0b": "Rae" });
    expect(slices.nodeUsers).toEqual({
      "0b": { accountId: "0a", name: "Rae", label: "Rae's box" },
    });
    expect(slices.accountHandles).toEqual({ "0a": "rae" });
    expect(query.mock.calls.map(([target]) => target)).toEqual(["identity", "duckdns"]);
  });

  it("names an account with no nodes yet — a mention still resolves", async () => {
    const query = vi.fn((target: string) =>
      target === "identity"
        ? Promise.resolve({
            accounts: [
              {
                account_id: [12],
                display_name: "Nomad",
                nonce: 0,
                member_keys: [],
                nodes: [],
                updated_at: 1,
              },
            ],
          })
        : Promise.resolve({ registrations: [] }),
    );

    const slices = await fetchPeopleSlices(makeTransportStub({ query }));

    expect(slices.authorNames).toEqual({ "0c": "Nomad" });
    expect(slices.nodeUsers).toEqual({});
  });
});

describe("fetchGovernanceSlices", () => {
  it("hydrates proposals and the account-share registry together", async () => {
    const query = vi.fn((_target: string, request: unknown) =>
      Promise.resolve(
        request === "shares"
          ? { shares: { active: true, allocations: [{ account_id: [1], shares: 60 }], total: 60 } }
          : { proposals: [] },
      ),
    );

    await expect(fetchGovernanceSlices(makeTransportStub({ query }))).resolves.toEqual({
      proposals: [],
      governanceShares: {
        active: true,
        allocations: [{ account_id: [1], shares: 60 }],
        total: 60,
      },
    });
    expect(query.mock.calls.map(([, request]) => request)).toEqual(["proposals", "shares"]);
  });
});

describe("fetchCapabilitySlices", () => {
  it("distinguishes a failed registry read from a successful empty registry", async () => {
    const failed = await fetchCapabilitySlices(
      makeTransportStub({ query: vi.fn().mockRejectedValue(new Error("offline")) }),
    );
    expect(failed.capabilities).toEqual([]);
    expect(failed.capabilitiesStatus).toBe("error");

    const empty = await fetchCapabilitySlices(
      makeTransportStub({ query: vi.fn().mockResolvedValue({ all: [] }) }),
    );
    expect(empty.capabilities).toEqual([]);
    expect(empty.capabilitiesStatus).toBe("ready");
  });
});

// ── The read-your-writes floor ──────────────────────────

describe("receiptFloor", () => {
  const op = (
    phase: "pending" | "finalized" | "failed",
    height?: number,
  ): OpLedger[string] => ({ seq: 1, phase, startedAt: 0, height });

  it("is zero on an empty ledger", () => {
    expect(receiptFloor({})).toBe(0);
  });

  it("tracks the highest finalized receipt height", () => {
    expect(
      receiptFloor({
        a: op("finalized", 5),
        b: op("finalized", 9),
        c: op("finalized", 3),
      }),
    ).toBe(9);
  });

  it("includes pending receipts while ignoring failed and heightless records", () => {
    expect(
      receiptFloor({
        pending: op("pending", 99),
        failed: op("failed", 42),
        settledWithoutReceipt: op("finalized"),
      }),
    ).toBe(99);
  });
});
