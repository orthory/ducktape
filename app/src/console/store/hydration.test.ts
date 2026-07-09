// The scoped-hydration decisions, proven without React: root diffing names
// exactly the modules a block moved, the module → slice-group map fans out to
// the right re-queries (and nothing else), and the read-your-writes floor
// tracks the highest receipted height so a lagging snapshot never un-renders
// a confirmed write.

import { describe, expect, it } from "vitest";

import type { NodeStatus } from "../../domain/transport";
import { receiptFloor } from "./finalization";
import type { OpLedger } from "./finalization";
import { changedModules, scopeFor } from "./hydration";

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

  it("groups profiles and identity together — authorNames overlays them", () => {
    expect(scopeFor(new Set(["profiles", "identity"]))).toEqual(
      new Set(["people"]),
    );
  });

  it("ignores modules with no console projection", () => {
    expect(scopeFor(new Set(["kv", "blobstore", "tagging", "upgrade"]))).toEqual(
      new Set(),
    );
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

  it("ignores pending, failed, and heightless records", () => {
    expect(
      receiptFloor({
        pending: op("pending", 99),
        failed: op("failed", 42),
        settledWithoutReceipt: op("finalized"),
      }),
    ).toBe(0);
  });
});
