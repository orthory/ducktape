import { describe, expect, it } from "vitest";

import type { BlockKind } from "../../../domain/pages-client";
import { headingTopSpace, INDENT, MARKER_HANG, ROW_PAD_Y } from "./pages-style";

describe("pages-style", () => {
  it("hangs the marker exactly as far as the old in-flow gutter was wide", () => {
    // the old row reserved a 20px marker box + an 8px flex gap before the text.
    expect(MARKER_HANG).toBe(28);
  });

  it("keeps one nesting level at 26px", () => {
    expect(INDENT).toBe(26);
  });

  it("gives headings top space, in descending order", () => {
    expect(headingTopSpace("heading1")).toBeGreaterThan(headingTopSpace("heading2"));
    expect(headingTopSpace("heading2")).toBeGreaterThan(headingTopSpace("heading3"));
    expect(headingTopSpace("heading3")).toBeGreaterThan(0);
  });

  it("gives every non-heading kind no extra top space", () => {
    const kinds: BlockKind[] = [
      "paragraph",
      "bulleted",
      "numbered",
      "todo",
      "toggle",
      "quote",
      "code",
      "callout",
      "divider",
      "page",
    ];
    for (const kind of kinds) expect(headingTopSpace(kind)).toBe(0);
  });
});

describe("row padding", () => {
  it("is the offset a hanging marker must add back to sit on its line", () => {
    expect(ROW_PAD_Y).toBe(2.5);
  });
});
