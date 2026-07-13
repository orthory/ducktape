import { describe, expect, it } from "vitest";

import { blocksToInput, parseMessageInput } from "./chat-input";
import {
  fileRefMarkdown,
  isChannelSeg,
  isFileSeg,
  isForgeSeg,
  isPageSeg,
  pageRefMarkdown,
  parsePageRefs,
  splitDuckRefs,
} from "./duck-ref";

describe("pageRefMarkdown / fileRefMarkdown builders", () => {
  it("build standard markdown link/image syntax", () => {
    expect(pageRefMarkdown("pg-1", "Launch plan")).toBe("[Launch plan](duck://page/pg-1)");
    expect(fileRefMarkdown("/shared/attachments/u/cat.png", "cat.png", true)).toBe(
      "![cat.png](duck://files/shared/attachments/u/cat.png)",
    );
    expect(fileRefMarkdown("/shared/attachments/u/doc.pdf", "doc.pdf", false)).toBe(
      "[doc.pdf](duck://files/shared/attachments/u/doc.pdf)",
    );
  });

  it("sanitizes a label that would break the `[..]` (no `]`/newline/bidi)", () => {
    // `\n` (a control char) is dropped, `]` becomes a space, then collapse.
    expect(pageRefMarkdown("id", "a]b\nc")).toBe("[a bc](duck://page/id)");
    expect(fileRefMarkdown("/shared/attachments/u/f", "a‮b", true)).toContain("[ab]");
  });

  it("neutralizes `*` in a title so the markdown parser can't split the ref", () => {
    // a `*x*` / `**x**` in the label would be parsed as italic/bold on send,
    // splitting the ref across spans — the chip would be lost. Strip `*`.
    expect(pageRefMarkdown("pg-1", "My *great* plan")).toBe("[My great plan](duck://page/pg-1)");
    expect(pageRefMarkdown("pg-2", "a**b**c")).toBe("[a b c](duck://page/pg-2)");
    // and the ref survives the real round-trip now (regression guard).
    const blocks = parseMessageInput(pageRefMarkdown("pg-3", "load *bearing* title"));
    expect(splitDuckRefs(blocksToInput(blocks)).filter(isPageSeg)).toHaveLength(1);
  });
});

describe("splitDuckRefs — the unified tokenizer", () => {
  it("parses a page link", () => {
    const segs = splitDuckRefs("see [Launch plan](duck://page/pg-1) now");
    expect(segs).toEqual([
      { text: "see " },
      { page: { id: "pg-1", label: "Launch plan" } },
      { text: " now" },
    ]);
  });

  it("parses a file link (download) and an image embed", () => {
    const link = splitDuckRefs("[doc.pdf](duck://files/shared/attachments/u/doc.pdf)");
    expect(link).toEqual([
      { file: { path: "/shared/attachments/u/doc.pdf", name: "doc.pdf", embed: false } },
    ]);
    const img = splitDuckRefs("![cat.png](duck://files/shared/attachments/u/cat.png)");
    expect(img).toEqual([
      { file: { path: "/shared/attachments/u/cat.png", name: "cat.png", embed: true } },
    ]);
  });

  it("renders a page and a file ref in the same body", () => {
    const segs = splitDuckRefs(
      "[p](duck://page/x) and ![i](duck://files/shared/attachments/u/i.png)",
    );
    expect(segs.filter(isPageSeg)).toHaveLength(1);
    expect(segs.filter(isFileSeg)).toHaveLength(1);
  });

  it("confines file refs to /shared/attachments — anything else stays literal", () => {
    const home = "[k](duck://files/home/alice/secret.txt)";
    expect(splitDuckRefs(home)).toEqual([{ text: home }]);
    const skills = "[s](duck://files/shared/skills/a/b)";
    expect(splitDuckRefs(skills)).toEqual([{ text: skills }]);
    // right root, wrong depth (must be exactly <dir>/<name>).
    const deep = "[d](duck://files/shared/attachments/a/b/c)";
    expect(splitDuckRefs(deep)).toEqual([{ text: deep }]);
    const shallow = "[d](duck://files/shared/attachments/only)";
    expect(splitDuckRefs(shallow)).toEqual([{ text: shallow }]);
  });

  it("rejects dot-segments in a file path", () => {
    const dots = "[d](duck://files/shared/attachments/../secret)";
    expect(splitDuckRefs(dots)).toEqual([{ text: dots }]);
  });

  it("is lossless — non-refs and malformed refs stay verbatim", () => {
    const source = "hi [not a ref] and [x](https://example.com) and [p](duck://page/ok)";
    const rebuilt = splitDuckRefs(source)
      .map((s) =>
        isPageSeg(s)
          ? pageRefMarkdown(s.page.id, s.page.label)
          : isFileSeg(s)
            ? fileRefMarkdown(s.file.path, s.file.name, s.file.embed)
            : "text" in s
              ? s.text
              : "",
      )
      .join("");
    // the page ref re-emits identically; the rest is untouched literal text.
    expect(rebuilt).toContain("[not a ref]");
    expect(rebuilt).toContain("[x](https://example.com)");
    expect(rebuilt).toContain("[p](duck://page/ok)");
  });

  it("chips forge and channel refs; unknown modules stay literal", () => {
    const segs = splitDuckRefs(
      "fix in [PR](duck://forge/ducktape/58) discussed in [#general](duck://channel/general#42), " +
        "see [notes](duck://memory/notes.md)",
    );
    expect(segs.filter(isForgeSeg)).toEqual([{ forge: { repo: "ducktape", number: 58 } }]);
    expect(segs.filter(isChannelSeg)).toEqual([{ channel: { id: "general", seq: 42 } }]);
    // unknown module: the whole markdown ref stays in the literal run.
    expect(
      segs.some((s) => "text" in s && s.text.includes("[notes](duck://memory/notes.md)")),
    ).toBe(true);
  });

  it("never chips a gateway host — dotted authorities belong to the browser", () => {
    const gw = "[site](duck://team.duck/index.html)";
    expect(splitDuckRefs(gw)).toEqual([{ text: gw }]);
  });

  it("parsePageRefs lists distinct page ids in first-seen order", () => {
    expect(
      parsePageRefs("[a](duck://page/plan) [b](duck://page/spec) [c](duck://page/plan)"),
    ).toEqual(["plan", "spec"]);
  });
});

describe("references survive the message round-trip as plain text", () => {
  // The design hinges on duck:// refs riding as PLAIN markdown text: the chat
  // markdown parser marks only https?:// links, so a duck:// ref comes back out
  // of parse→blocks→input for the renderer's tokenizer (and, later, the agent).
  it("a duck://page ref is not linkified — stays plain, chippable text", () => {
    const ref = "[Plan](duck://page/pg-1)";
    const blocks = parseMessageInput(`see ${ref} here`);
    const para = (blocks[0] as { paragraph: { text: string; marks: unknown[] }[] }).paragraph;
    expect(para.every((s) => s.marks.length === 0)).toBe(true);
    expect(para.map((s) => s.text).join("")).toContain(ref);
    expect(splitDuckRefs(blocksToInput(blocks)).filter(isPageSeg)).toHaveLength(1);
  });

  it("a duck://files image ref survives too", () => {
    const ref = "![c.png](duck://files/shared/attachments/u/c.png)";
    const blocks = parseMessageInput(ref);
    expect(splitDuckRefs(blocksToInput(blocks)).filter(isFileSeg)).toHaveLength(1);
  });
});
