import { describe, it, expect } from "vitest";
import { toGraph, isCollapsed } from "../layout";
import type { FleetNode } from "../types";

const n = (over: Partial<FleetNode>): FleetNode => ({
  id: "x",
  branch: "x",
  path: ".",
  head: { sha: "0", subject: "" },
  parent: "dev",
  ahead: 0,
  behind: 0,
  status: "down",
  ...over,
});

describe("isCollapsed", () => {
  it("collapses anything that is not live", () => {
    expect(isCollapsed(n({ status: "up" }))).toBe(false);
    expect(isCollapsed(n({ status: "building" }))).toBe(true);
    expect(isCollapsed(n({ status: "down" }))).toBe(true);
  });
});

describe("toGraph", () => {
  it("lays out horizontally: base on the left, worktrees to the right", () => {
    const nodes = [
      n({ id: "dev", branch: "dev", parent: null }),
      n({ id: "a", branch: "feat/a", status: "up" }),
      n({ id: "b", branch: "feat/b" }),
    ];
    const { rfNodes, rfEdges } = toGraph(nodes, "dev", () => {});

    expect(rfNodes).toHaveLength(3);
    expect(rfNodes.find((x) => x.id === "dev")!.position.x).toBe(0);
    expect(rfNodes.filter((x) => x.position.x > 0)).toHaveLength(2);

    expect(rfEdges).toHaveLength(2);
    expect(rfEdges.every((e) => e.source === "dev")).toBe(true);
    expect(rfEdges.find((e) => e.target === "a")!.animated).toBe(true);
    expect(rfEdges.find((e) => e.target === "b")!.animated).toBe(false);
  });

  it("emits no edges when the base branch is not in the fleet", () => {
    const { rfNodes, rfEdges } = toGraph([n({ id: "a", branch: "feat/a" })], "dev", () => {});
    expect(rfNodes).toHaveLength(1);
    expect(rfEdges).toHaveLength(0);
  });
});
