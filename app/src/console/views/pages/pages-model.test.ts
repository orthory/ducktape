import { describe, expect, it } from "vitest";

import type { BlockKind, PageBlock } from "../../../domain/pages-client";
import {
  buildRows,
  continuationKind,
  emptyEnterExits,
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
  kind: "paragraph",
  text: "",
  checked: false,
  children: [],
  ...patch,
});

// the preorder snapshot of: p1 > [a, b(toggle) > [c], d(1.), e(2.)]
const TREE: PageBlock[] = [
  block({ id: "p1", parent: null, kind: "page", text: "Plan", children: ["a", "b", "d", "e"] }),
  block({ id: "a", text: "first" }),
  block({ id: "b", kind: "toggle", text: "details", children: ["c"] }),
  block({ id: "c", parent: "b", text: "inside" }),
  block({ id: "d", kind: "numbered", text: "one" }),
  block({ id: "e", kind: "numbered", text: "two" }),
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
    expect(shortcutFor("# hi")).toEqual({ kind: "heading1", rest: "hi" });
    expect(shortcutFor("### deep")).toEqual({ kind: "heading3", rest: "deep" });
    expect(shortcutFor("- item")).toEqual({ kind: "bulleted", rest: "item" });
    expect(shortcutFor("1. one")).toEqual({ kind: "numbered", rest: "one" });
    expect(shortcutFor("[ ] buy")).toEqual({ kind: "todo", rest: "buy" });
    expect(shortcutFor("[] buy")).toEqual({ kind: "todo", rest: "buy" });
    expect(shortcutFor("> said")).toEqual({ kind: "quote", rest: "said" });
    expect(shortcutFor("--- ")).toEqual({ kind: "divider", rest: "" });
  });

  it("requires the trailing space — a bare prefix stays literal text", () => {
    expect(shortcutFor("#hi")).toBeNull();
    expect(shortcutFor("plain text")).toBeNull();
  });
});

describe("slash menu + list continuation", () => {
  it("filters the catalogue by label or kind", () => {
    expect(filterSlashKinds("head").map((o) => o.kind)).toEqual([
      "heading1",
      "heading2",
      "heading3",
    ]);
    expect(filterSlashKinds("").length).toBeGreaterThan(8);
  });

  it("offers a subpage entry last, without crowding the text kinds", () => {
    expect(filterSlashKinds("pag").map((o) => o.kind)).toContain("page");
    const all = filterSlashKinds("");
    expect(all).toHaveLength(13);
    expect(all[all.length - 1].kind).toBe("page");
  });

  it("continues list kinds on Enter and resets the rest to paragraphs", () => {
    expect(continuationKind("bulleted")).toBe("bulleted");
    expect(continuationKind("todo")).toBe("todo");
    expect(continuationKind("heading1")).toBe("paragraph");
    expect(continuationKind("quote")).toBe("paragraph");
  });
});

describe("emptyEnterExits", () => {
  it("escapes every kind you can get stuck inside", () => {
    const escapable: BlockKind[] = [
      "bulleted",
      "numbered",
      "todo",
      "quote",
      "code",
      "callout",
      "toggle",
    ];
    for (const kind of escapable) expect(emptyEnterExits(kind)).toBe(true);
  });

  it("leaves prose and structural kinds alone", () => {
    const stays: BlockKind[] = [
      "paragraph",
      "heading1",
      "heading2",
      "heading3",
      "divider",
      "page",
    ];
    for (const kind of stays) expect(emptyEnterExits(kind)).toBe(false);
  });

  // the old code inferred this from continuationKind, which only ever agreed
  // for the three list kinds — that is exactly the bug.
  it("covers kinds continuationKind never could", () => {
    for (const kind of ["quote", "code", "callout", "toggle"] as BlockKind[]) {
      expect(continuationKind(kind)).not.toBe(kind);
      expect(emptyEnterExits(kind)).toBe(true);
    }
  });
});
