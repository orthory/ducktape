import { beforeEach, describe, expect, it } from "vitest";

import { loadCollapsed, saveCollapsed } from "./page-collapse";

describe("page collapse persistence", () => {
  beforeEach(() => localStorage.clear());

  it("round-trips a page's collapsed toggles", () => {
    saveCollapsed("p1", new Set(["b1", "b2"]));
    expect([...loadCollapsed("p1")].sort()).toEqual(["b1", "b2"]);
  });

  it("keeps pages apart", () => {
    saveCollapsed("p1", new Set(["b1"]));
    saveCollapsed("p2", new Set(["b9"]));
    expect([...loadCollapsed("p1")]).toEqual(["b1"]);
    expect([...loadCollapsed("p2")]).toEqual(["b9"]);
    expect([...loadCollapsed("unknown")]).toEqual([]);
    expect([...loadCollapsed(null)]).toEqual([]);
  });

  it("forgets a page once nothing is collapsed", () => {
    saveCollapsed("p1", new Set(["b1"]));
    saveCollapsed("p1", new Set());
    expect([...loadCollapsed("p1")]).toEqual([]);
    expect(localStorage.getItem("ducktape.pageBlocksCollapsed")).toBe("{}");
  });

  it("survives corrupt storage instead of taking the editor down with it", () => {
    localStorage.setItem("ducktape.pageBlocksCollapsed", "{not json");
    expect([...loadCollapsed("p1")]).toEqual([]);
    localStorage.setItem("ducktape.pageBlocksCollapsed", '["an array, not a map"]');
    expect([...loadCollapsed("p1")]).toEqual([]);
    localStorage.setItem("ducktape.pageBlocksCollapsed", '{"p1":[1,"b1",null]}');
    expect([...loadCollapsed("p1")]).toEqual(["b1"]);
  });
});
