import { describe, expect, it } from "vitest";

import { highlightLines, langForFilename } from "./highlight";

// The Code tab used to tokenize against github-light ONLY, so every token kept a
// light-theme color and the whole pane went unreadable (~1.1:1) against the dark
// paper. The fix tokenizes against both github themes at once and lets each token
// carry BOTH colors as custom properties, which the `.code-tok` rule in
// global.css resolves per `data-theme`. jsdom has no CSS engine and can't prove
// the contrast — but it CAN prove the precondition the fix rests on: that a dark
// color is emitted at all, and that it actually differs from the light one.
// Revert to a single theme and every assertion below fails.
describe("highlight", () => {
  it("maps a filename to a language, and leaves unknown extensions unhighlighted", () => {
    expect(langForFilename("main.rs")).toBe("rust");
    expect(langForFilename("a.tsx")).toBe("tsx");
    expect(langForFilename("notes.xyz")).toBeNull();
  });

  it("emits a light AND a dark color for every token", async () => {
    const lines = await highlightLines(`const x = "hi";`, "typescript");
    expect(lines).not.toBeNull();
    const tokens = lines!.flat();
    expect(tokens.length).toBeGreaterThan(1); // actually tokenized, not one plain run

    for (const token of tokens) {
      expect(token.style, `token ${JSON.stringify(token.content)}`).toBeDefined();
      expect(token.style!["--shiki-light"]).toMatch(/^#[0-9a-f]{6}$/i);
      expect(token.style!["--shiki-dark"]).toMatch(/^#[0-9a-f]{6}$/i);
    }

    // The two themes must genuinely disagree somewhere, or we'd be shipping one
    // palette under two names and dark mode would still be unreadable.
    expect(tokens.some((t) => t.style!["--shiki-light"] !== t.style!["--shiki-dark"])).toBe(true);
  });

  it("returns null for an unknown language instead of throwing", async () => {
    await expect(highlightLines("hello", "not-a-language")).resolves.toBeNull();
  });
});
