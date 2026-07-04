import { describe, expect, it } from "vitest";

import type { PageBlock } from "../../../domain/pages-client";
import {
  buildRows,
  continuationKind,
  filterSlashKinds,
  indentTarget,
  moveDownTarget,
  moveUpTarget,
  outdentTarget,
  shortcutFor,
} from "./pages-model";

const block = (patch: Partial<PageBlock> & { id: string }): PageBlock => ({
  parent: "p1",
  page: "p1",
  kind: "Paragraph",
  text: "",
  checked: false,
  children: [],
  ...patch,
});

// the preorder snapshot of: p1 > [a, b(toggle) > [c], d(1.), e(2.)]
const TREE: PageBlock[] = [
  block({ id: "p1", parent: null, kind: "Page", text: "Plan", children: ["a", "b", "d", "e"] }),
  block({ id: "a", text: "first" }),
  block({ id: "b", kind: "Toggle", text: "details", children: ["c"] }),
  block({ id: "c", parent: "b", text: "inside" }),
  block({ id: "d", kind: "Numbered", text: "one" }),
  block({ id: "e", kind: "Numbered", text: "two" }),
];

describe("buildRows", () => {
  it("skips the root, orders preorder, and derives depth from parent links", () => {
    const rows = buildRows(TREE, new Set());
    expect(rows.map((r) => r.block.id)).toEqual(["a", "b", "c", "d", "e"]);
    expect(rows.map((r) => r.depth)).toEqual([0, 0, 1, 0, 0]);
  });

  it("numbers consecutive Numbered siblings as one run", () => {
    const rows = buildRows(TREE, new Set());
    const byId = new Map(rows.map((r) => [r.block.id, r]));
    expect(byId.get("d")?.listIndex).toBe(1);
    expect(byId.get("e")?.listIndex).toBe(2);
    expect(byId.get("a")?.listIndex).toBeUndefined();
  });

  it("hides everything below a collapsed toggle", () => {
    const rows = buildRows(TREE, new Set(["b"]));
    expect(rows.map((r) => r.block.id)).toEqual(["a", "b", "d", "e"]);
  });
});

describe("move targets", () => {
  it("indents under the previous sibling, appended after its last child", () => {
    // b's previous sibling is a (no children yet) -> first child of a.
    expect(indentTarget(TREE, "b")).toEqual({ parent: "a", after: null });
    // d's previous sibling is b, whose last child is c.
    expect(indentTarget(TREE, "d")).toEqual({ parent: "b", after: "c" });
    // a is the first sibling: nothing to adopt it.
    expect(indentTarget(TREE, "a")).toBeNull();
  });

  it("outdents to the grandparent, landing right after the old parent", () => {
    expect(outdentTarget(TREE, "c")).toEqual({ parent: "p1", after: "b" });
    // top-level blocks (parent == root) cannot outdent further.
    expect(outdentTarget(TREE, "a")).toBeNull();
  });

  it("moves among siblings with after-anchors", () => {
    expect(moveUpTarget(TREE, "b")).toEqual({ parent: "p1", after: null });
    expect(moveUpTarget(TREE, "a")).toBeNull();
    expect(moveDownTarget(TREE, "d")).toEqual({ parent: "p1", after: "e" });
    expect(moveDownTarget(TREE, "e")).toBeNull();
  });
});

describe("markdown shortcuts", () => {
  it("maps typed prefixes to kinds and keeps the remainder", () => {
    expect(shortcutFor("# hi")).toEqual({ kind: "Heading1", rest: "hi" });
    expect(shortcutFor("### deep")).toEqual({ kind: "Heading3", rest: "deep" });
    expect(shortcutFor("- item")).toEqual({ kind: "Bulleted", rest: "item" });
    expect(shortcutFor("1. one")).toEqual({ kind: "Numbered", rest: "one" });
    expect(shortcutFor("[ ] buy")).toEqual({ kind: "Todo", rest: "buy" });
    expect(shortcutFor("[] buy")).toEqual({ kind: "Todo", rest: "buy" });
    expect(shortcutFor("> said")).toEqual({ kind: "Quote", rest: "said" });
    expect(shortcutFor("--- ")).toEqual({ kind: "Divider", rest: "" });
  });

  it("requires the trailing space — a bare prefix stays literal text", () => {
    expect(shortcutFor("#hi")).toBeNull();
    expect(shortcutFor("plain text")).toBeNull();
  });
});

describe("slash menu + list continuation", () => {
  it("filters the catalogue by label or kind", () => {
    expect(filterSlashKinds("head").map((o) => o.kind)).toEqual([
      "Heading1",
      "Heading2",
      "Heading3",
    ]);
    expect(filterSlashKinds("").length).toBeGreaterThan(8);
  });

  it("continues list kinds on Enter and resets the rest to paragraphs", () => {
    expect(continuationKind("Bulleted")).toBe("Bulleted");
    expect(continuationKind("Todo")).toBe("Todo");
    expect(continuationKind("Heading1")).toBe("Paragraph");
    expect(continuationKind("Quote")).toBe("Paragraph");
  });
});
