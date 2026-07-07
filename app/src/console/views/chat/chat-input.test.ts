import { describe, expect, it } from "vitest";

import { blocksToInput, parseMessageInput } from "./chat-input";

describe("parseMessageInput", () => {
  it("keeps plain text as a single paragraph", () => {
    expect(parseMessageInput("just words")).toEqual([{ paragraph: [{ text: "just words", marks: [] }] }]);
  });

  it("marks **bold**, *italic*, markdown links, and bare URLs inside a paragraph", () => {
    const blocks = parseMessageInput("a **b** and *c* read [docs](https://x.test/docs) see https://x.test/y");
    expect(blocks).toEqual([
      {
        paragraph: [
          { text: "a ", marks: [] },
          { text: "b", marks: ["bold"] },
          { text: " and ", marks: [] },
          { text: "c", marks: ["italic"] },
          { text: " read ", marks: [] },
          { text: "docs", marks: [{ link: "https://x.test/docs" }] },
          { text: " see ", marks: [] },
          { text: "https://x.test/y", marks: [{ link: "https://x.test/y" }] },
        ],
      },
    ]);
  });

  it("extracts a fenced code block with its language and splits surrounding text", () => {
    const blocks = parseMessageInput("before\n```ts\nconst a = 1;\n```\nafter");
    expect(blocks).toEqual([
      { paragraph: [{ text: "before", marks: [] }] },
      { code: { lang: "ts", text: "const a = 1;" } },
      { paragraph: [{ text: "after", marks: [] }] },
    ]);
  });

  it("treats a fence with no language as lang null", () => {
    const blocks = parseMessageInput("```\nplain code\n```");
    expect(blocks).toEqual([{ code: { lang: null, text: "plain code" } }]);
  });

  it("never returns an empty body", () => {
    expect(parseMessageInput("   ")).toEqual([{ paragraph: [{ text: "", marks: [] }] }]);
  });

  it("leaves spaced asterisks (math / bullets) as plain text", () => {
    expect(parseMessageInput("2 * 3 * 4")).toEqual([{ paragraph: [{ text: "2 * 3 * 4", marks: [] }] }]);
  });

  it("marks bold text that contains internal spaces", () => {
    expect(parseMessageInput("**bold text** ok")).toEqual([
      {
        paragraph: [
          { text: "bold text", marks: ["bold"] },
          { text: " ok", marks: [] },
        ],
      },
    ]);
  });

  it("leaves a fence-only message untouched but marks around it", () => {
    const blocks = parseMessageInput("use *this*:\n```\ncode\n```");
    expect(blocks).toEqual([
      { paragraph: [{ text: "use ", marks: [] }, { text: "this", marks: ["italic"] }, { text: ":", marks: [] }] },
      { code: { lang: null, text: "code" } },
    ]);
  });

  it("turns quote lines into marked quote blocks", () => {
    const blocks = parseMessageInput("> **ship** this\n> with [context](https://x.test/context)");
    expect(blocks).toEqual([
      {
        quote: [
          { text: "ship", marks: ["bold"] },
          { text: " this\nwith ", marks: [] },
          { text: "context", marks: [{ link: "https://x.test/context" }] },
        ],
      },
    ]);
  });

  it("turns divider lines into divider blocks between paragraphs", () => {
    expect(parseMessageInput("before\n---\nafter")).toEqual([
      { paragraph: [{ text: "before", marks: [] }] },
      "divider",
      { paragraph: [{ text: "after", marks: [] }] },
    ]);
  });
});

describe("blocksToInput (edit round-trip)", () => {
  it("round-trips marks and code fences back through the parser", () => {
    const source = "a **b** and *c* plus [docs](https://x.test/docs)\n\n```ts\nconst a = 1;\n```";
    const blocks = parseMessageInput(source);
    // re-rendering to composer text and re-parsing yields the same blocks
    expect(parseMessageInput(blocksToInput(blocks))).toEqual(blocks);
  });

  it("renders code blocks with fences and language", () => {
    expect(blocksToInput([{ code: { lang: "rs", text: "fn main() {}" } }])).toBe("```rs\nfn main() {}\n```");
  });

  it("round-trips quote and divider blocks", () => {
    const blocks = parseMessageInput("intro\n\n> quoted **text**\n\n---\n\noutro");
    expect(parseMessageInput(blocksToInput(blocks))).toEqual(blocks);
  });
});
