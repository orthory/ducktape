import { describe, expect, it } from "vitest";
import { applySpanMark, mergeMarks, rangeHasMark, rebaseMarks, rebaseRange, splitMarks } from "./pages-ranges";

describe("Pages UTF-16 ranges", () => {
  it("rebases marks and comment anchors around emoji", () => {
    expect(rebaseRange("a🦆bc", "++a🦆bc", { start: 1, end: 3 })).toEqual({ start: 3, end: 5 });
    expect(rebaseMarks("a🦆bc", "a🦆!bc", [{ start: 1, end: 4, kind: "bold" }]))
      .toEqual([{ start: 1, end: 5, kind: "bold" }]);
  });

  it("toggles, splits, and merges normalized marks", () => {
    const bold = applySpanMark([], { start: 0, end: 6 }, "bold", true);
    const cut = applySpanMark(bold, { start: 2, end: 4 }, "bold", false);
    expect(cut).toEqual([
      { start: 0, end: 2, kind: "bold" },
      { start: 4, end: 6, kind: "bold" },
    ]);
    expect(rangeHasMark(bold, { start: 1, end: 5 }, "bold")).toBe(true);
    const halves = splitMarks(bold, 3);
    expect(mergeMarks(halves.left, halves.right, 3)).toEqual(bold);
  });
});
