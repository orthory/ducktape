import { describe, expect, it } from "vitest";

import { blocksToInput, parseMessageInput } from "./chat-input";

describe("parseMessageInput", () => {
  it("keeps plain text as a single paragraph", () => {
    expect(parseMessageInput("just words")).toEqual([{ Paragraph: [{ text: "just words", marks: [] }] }]);
  });

  it("marks **bold**, *italic*, and bare URLs inside a paragraph", () => {
    const blocks = parseMessageInput("a **b** and *c* see https://x.test/y");
    expect(blocks).toEqual([
      {
        Paragraph: [
          { text: "a ", marks: [] },
          { text: "b", marks: ["Bold"] },
          { text: " and ", marks: [] },
          { text: "c", marks: ["Italic"] },
          { text: " see ", marks: [] },
          { text: "https://x.test/y", marks: [{ Link: "https://x.test/y" }] },
        ],
      },
    ]);
  });

  it("extracts a fenced code block with its language and splits surrounding text", () => {
    const blocks = parseMessageInput("before\n```ts\nconst a = 1;\n```\nafter");
    expect(blocks).toEqual([
      { Paragraph: [{ text: "before", marks: [] }] },
      { Code: { lang: "ts", text: "const a = 1;" } },
      { Paragraph: [{ text: "after", marks: [] }] },
    ]);
  });

  it("treats a fence with no language as lang null", () => {
    const blocks = parseMessageInput("```\nplain code\n```");
    expect(blocks).toEqual([{ Code: { lang: null, text: "plain code" } }]);
  });

  it("never returns an empty body", () => {
    expect(parseMessageInput("   ")).toEqual([{ Paragraph: [{ text: "", marks: [] }] }]);
  });

  it("leaves spaced asterisks (math / bullets) as plain text", () => {
    expect(parseMessageInput("2 * 3 * 4")).toEqual([{ Paragraph: [{ text: "2 * 3 * 4", marks: [] }] }]);
  });

  it("marks bold text that contains internal spaces", () => {
    expect(parseMessageInput("**bold text** ok")).toEqual([
      {
        Paragraph: [
          { text: "bold text", marks: ["Bold"] },
          { text: " ok", marks: [] },
        ],
      },
    ]);
  });

  it("leaves a fence-only message untouched but marks around it", () => {
    const blocks = parseMessageInput("use *this*:\n```\ncode\n```");
    expect(blocks).toEqual([
      { Paragraph: [{ text: "use ", marks: [] }, { text: "this", marks: ["Italic"] }, { text: ":", marks: [] }] },
      { Code: { lang: null, text: "code" } },
    ]);
  });
});

describe("blocksToInput (edit round-trip)", () => {
  it("round-trips marks and code fences back through the parser", () => {
    const source = "a **b** and *c*\n\n```ts\nconst a = 1;\n```";
    const blocks = parseMessageInput(source);
    // re-rendering to composer text and re-parsing yields the same blocks
    expect(parseMessageInput(blocksToInput(blocks))).toEqual(blocks);
  });

  it("renders code blocks with fences and language", () => {
    expect(blocksToInput([{ Code: { lang: "rs", text: "fn main() {}" } }])).toBe("```rs\nfn main() {}\n```");
  });
});
