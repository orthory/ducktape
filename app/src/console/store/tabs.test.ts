import { describe, expect, it } from "vitest";
import { addTab, removeTab } from "./state";

describe("doc tabs", () => {
  it("addTab appends unique, preserves order", () => {
    expect(addTab([], "a")).toEqual(["a"]);
    expect(addTab(["a"], "b")).toEqual(["a", "b"]);
    expect(addTab(["a", "b"], "a")).toEqual(["a", "b"]);
  });
  it("removeTab drops the id and picks a neighbor as next active", () => {
    // closing the active middle tab activates the following neighbor.
    expect(removeTab(["a", "b", "c"], "b", "b")).toEqual({ tabs: ["a", "c"], active: "c" });
    // closing the active last tab activates the previous.
    expect(removeTab(["a", "b"], "b", "b")).toEqual({ tabs: ["a"], active: "a" });
    // closing a non-active tab keeps the active one.
    expect(removeTab(["a", "b", "c"], "a", "c")).toEqual({ tabs: ["a", "b"], active: "a" });
    // closing the last remaining tab clears active.
    expect(removeTab(["a"], "a", "a")).toEqual({ tabs: [], active: null });
  });
});
