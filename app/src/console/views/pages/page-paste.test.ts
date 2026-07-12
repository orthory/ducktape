import { describe, expect, it } from "vitest";

import { MAX_PASTE_BLOCKS, pastePlan } from "./page-paste";

describe("pastePlan", () => {
  it("turns a markdown document into the blocks it looks like", () => {
    const plan = pastePlan(
      ["# Launch", "", "First draft", "- one", "- two", "1. step", "[] ship", "> quoted", "---"].join(
        "\n",
      ),
    );
    expect(plan.dropped).toBe(0);
    expect(plan.blocks).toEqual([
      { kind: "heading1", text: "Launch" },
      { kind: "paragraph", text: "First draft" },
      { kind: "bulleted", text: "one" },
      { kind: "bulleted", text: "two" },
      { kind: "numbered", text: "step" },
      { kind: "todo", text: "ship" },
      { kind: "quote", text: "quoted" },
      { kind: "divider", text: "" },
    ]);
  });

  it("treats blank lines as separators, not as empty blocks", () => {
    expect(pastePlan("a\n\n\n\nb").blocks).toEqual([
      { kind: "paragraph", text: "a" },
      { kind: "paragraph", text: "b" },
    ]);
    expect(pastePlan("\n\n   \n").blocks).toEqual([]);
  });

  it("handles CRLF and trailing whitespace", () => {
    expect(pastePlan("a  \r\nb\r\n").blocks).toEqual([
      { kind: "paragraph", text: "a" },
      { kind: "paragraph", text: "b" },
    ]);
  });

  // every block is one consensus submit, and the finalization ledger holds 512
  // records: an unbounded paste is a burst neither can absorb.
  it("caps the block count and reports what it dropped", () => {
    const plan = pastePlan(Array.from({ length: 90 }, (_, i) => `line ${i}`).join("\n"));
    expect(plan.blocks).toHaveLength(MAX_PASTE_BLOCKS);
    expect(plan.dropped).toBe(90 - MAX_PASTE_BLOCKS);
    expect(plan.blocks[MAX_PASTE_BLOCKS - 1].text).toBe(`line ${MAX_PASTE_BLOCKS - 1}`);
  });
});
