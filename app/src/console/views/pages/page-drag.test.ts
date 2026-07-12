import { describe, expect, it } from "vitest";

import type { PageBlock } from "../../../domain/pages-client";
import { blockSubtree, dropTarget } from "./page-drag";

const blockOf = (patch: Partial<PageBlock> & { id: string }): PageBlock => ({
  parent: "p1",
  page: "p1",
  kind: "paragraph",
  text: "",
  checked: false,
  children: [],
  ...patch,
});

// p1
//  ├ a
//  │  └ a1
//  ├ b
//  └ c
const BLOCKS: PageBlock[] = [
  blockOf({ id: "p1", parent: null, kind: "page", children: ["a", "b", "c"] }),
  blockOf({ id: "a", children: ["a1"] }),
  blockOf({ id: "a1", parent: "a" }),
  blockOf({ id: "b" }),
  blockOf({ id: "c" }),
];

describe("blockSubtree", () => {
  it("collects a block and everything under it", () => {
    expect([...blockSubtree(BLOCKS, "a")].sort()).toEqual(["a", "a1"]);
    expect([...blockSubtree(BLOCKS, "b")]).toEqual(["b"]);
  });

  it("survives a cycle in a torn snapshot instead of hanging", () => {
    const torn = [blockOf({ id: "x", children: ["y"] }), blockOf({ id: "y", children: ["x"] })];
    expect([...blockSubtree(torn, "x")].sort()).toEqual(["x", "y"]);
  });
});

describe("dropTarget", () => {
  it("lands a block after the row it was dropped on", () => {
    expect(dropTarget(BLOCKS, "c", "a", "after")).toEqual({ parent: "p1", after: "a" });
  });

  it("lands a block before the row it was dropped on", () => {
    // before "c" is after "b" — the sibling that precedes it.
    expect(dropTarget(BLOCKS, "a", "c", "before")).toEqual({ parent: "p1", after: "b" });
  });

  it("moves to the very front when dropped before the first sibling", () => {
    expect(dropTarget(BLOCKS, "c", "a", "before")).toEqual({ parent: "p1", after: null });
  });

  it("re-parents into a nested row's parent, at that row's depth", () => {
    expect(dropTarget(BLOCKS, "c", "a1", "after")).toEqual({ parent: "a", after: "a1" });
  });

  // the module rejects a cycle; the indicator must never invite one.
  it("refuses a drop onto itself or into its own subtree", () => {
    expect(dropTarget(BLOCKS, "a", "a", "after")).toBeNull();
    expect(dropTarget(BLOCKS, "a", "a1", "before")).toBeNull();
  });

  it("refuses the drops that are already where the block is — no redundant op", () => {
    // "b" dropped after "a": it already follows "a".
    expect(dropTarget(BLOCKS, "b", "a", "after")).toBeNull();
    // "b" dropped before "c": it already precedes "c".
    expect(dropTarget(BLOCKS, "b", "c", "before")).toBeNull();
  });

  it("refuses a drop on the page root, which is not a row", () => {
    expect(dropTarget(BLOCKS, "a", "p1", "after")).toBeNull();
  });
});
