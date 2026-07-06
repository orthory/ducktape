import { describe, expect, it } from "vitest";
import { buildForest, flattenVisible, subtreeIds } from "./page-tree";
import type { PageMeta } from "../../../domain/pages-client";

const pm = (id: string, title: string, parent: string | null): PageMeta => ({ id, title, parent });

describe("page forest", () => {
  it("nests children under parents, sorted by title", () => {
    const forest = buildForest([
      pm("a", "Alpha", null),
      pm("b", "Bravo", "a"),
      pm("c", "Able", "a"),
      pm("d", "Delta", null),
    ]);
    expect(forest.map((n) => n.id)).toEqual(["a", "d"]); // roots by title: Alpha, Delta
    const a = forest[0];
    expect(a.children.map((n) => n.title)).toEqual(["Able", "Bravo"]); // sorted
    expect(a.children[0].depth).toBe(1);
  });
  it("orphans (missing parent) surface at root so nothing is hidden", () => {
    const forest = buildForest([pm("x", "X", "ghost")]);
    expect(forest.map((n) => n.id)).toEqual(["x"]);
  });
  it("flattenVisible hides children under a collapsed node", () => {
    const forest = buildForest([pm("a", "A", null), pm("b", "B", "a")]);
    const rows = flattenVisible(forest, new Set(["a"]));
    expect(rows.map((r) => r.id)).toEqual(["a"]);
    expect(rows[0].hasChildren).toBe(true);
  });
  it("subtreeIds collects a node and all descendants", () => {
    const forest = buildForest([pm("a", "A", null), pm("b", "B", "a"), pm("c", "C", "b"), pm("d", "D", null)]);
    expect([...subtreeIds(forest, "a")].sort()).toEqual(["a", "b", "c"]);
  });
});
