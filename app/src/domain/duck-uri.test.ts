import { describe, expect, it } from "vitest";

import { classifyDuckRef } from "./duck-uri";

describe("classifyDuckRef — the module table", () => {
  it("classifies page refs (verbatim legacy grammar)", () => {
    expect(classifyDuckRef("duck://page/pg-1", "Plan", false)).toEqual({
      page: { id: "pg-1", label: "Plan" },
    });
    expect(classifyDuckRef("duck://page/a/b", "x", false)).toBeNull();
    expect(classifyDuckRef("duck://page/", "x", false)).toBeNull();
  });

  it("confines file refs to the attachments root (verbatim legacy rules)", () => {
    expect(
      classifyDuckRef("duck://files/shared/attachments/u1/doc.pdf", "doc.pdf", false),
    ).toEqual({
      file: { path: "/shared/attachments/u1/doc.pdf", name: "doc.pdf", embed: false },
    });
    expect(classifyDuckRef("duck://files/shared/skills/x.md", "x", false)).toBeNull();
    expect(classifyDuckRef("duck://files/shared/attachments/a/b/c", "x", false)).toBeNull();
    expect(classifyDuckRef("duck://files/shared/attachments/../etc/pw", "x", false)).toBeNull();
    expect(classifyDuckRef("duck://files/shared/attachments/u1/a.png", "a.png", true)).toEqual({
      file: { path: "/shared/attachments/u1/a.png", name: "a.png", embed: true },
    });
  });

  it("classifies forge refs: repo, item, discussion anchor", () => {
    expect(classifyDuckRef("duck://forge/ducktape", "", false)).toEqual({
      forge: { repo: "ducktape", number: null },
    });
    expect(classifyDuckRef("duck://forge/ducktape/58", "", false)).toEqual({
      forge: { repo: "ducktape", number: 58 },
    });
    expect(classifyDuckRef("duck://forge/ducktape/58#12", "", false)).toEqual({
      forge: { repo: "ducktape", number: 58, seq: 12 },
    });
    // an anchor needs an item; zero ids are not mintable
    expect(classifyDuckRef("duck://forge/ducktape#12", "", false)).toBeNull();
    expect(classifyDuckRef("duck://forge/ducktape/0", "", false)).toBeNull();
    expect(classifyDuckRef("duck://forge/ducktape/58#0", "", false)).toBeNull();
    // repos cannot carry the item-channel separator
    expect(classifyDuckRef("duck://forge/forge:x:1", "", false)).toBeNull();
  });

  it("classifies channel refs, including colon ids and message anchors", () => {
    expect(classifyDuckRef("duck://channel/general", "", false)).toEqual({
      channel: { id: "general" },
    });
    expect(classifyDuckRef("duck://channel/forge:ducktape:58", "", false)).toEqual({
      channel: { id: "forge:ducktape:58" },
    });
    expect(classifyDuckRef("duck://channel/general#42", "", false)).toEqual({
      channel: { id: "general", seq: 42 },
    });
    expect(classifyDuckRef("duck://channel/general#0", "", false)).toBeNull();
    expect(classifyDuckRef("duck://channel/", "", false)).toBeNull();
  });

  it("leaves unknown modules and gateway hosts unclassified", () => {
    expect(classifyDuckRef("duck://memory/notes/a.md", "", false)).toBeNull();
    expect(classifyDuckRef("duck://team.duck/index.html", "", false)).toBeNull();
    expect(classifyDuckRef("duck://net.duck", "", false)).toBeNull();
  });
});
