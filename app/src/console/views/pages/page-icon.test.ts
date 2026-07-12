import { describe, expect, it } from "vitest";

import { composeTitle, splitTitleEmoji } from "./page-icon";

describe("splitTitleEmoji", () => {
  it("lifts a leading emoji out of the title", () => {
    expect(splitTitleEmoji("🦆 Launch plan")).toEqual({ icon: "🦆", title: "Launch plan" });
    expect(splitTitleEmoji("🚀Ship")).toEqual({ icon: "🚀", title: "Ship" });
  });

  it("keeps a composed emoji whole (ZWJ sequences, skin tones, variation selectors)", () => {
    expect(splitTitleEmoji("👩‍💻 Notes").icon).toBe("👩‍💻");
    expect(splitTitleEmoji("👍🏽 Approved").icon).toBe("👍🏽");
    expect(splitTitleEmoji("✅️ Done").icon).toBe("✅️");
  });

  it("finds no icon in an ordinary title", () => {
    expect(splitTitleEmoji("Launch plan")).toEqual({ icon: null, title: "Launch plan" });
    expect(splitTitleEmoji("")).toEqual({ icon: null, title: "" });
    // a digit is Emoji, but not Extended_Pictographic — a numbered title is not
    // an icon.
    expect(splitTitleEmoji("1. First")).toEqual({ icon: null, title: "1. First" });
    // an emoji in the MIDDLE is just text.
    expect(splitTitleEmoji("Ship 🚀")).toEqual({ icon: null, title: "Ship 🚀" });
  });
});

describe("composeTitle", () => {
  it("is the inverse of the split — this is what gets committed", () => {
    for (const raw of ["🦆 Launch plan", "Launch plan", "🦆", "👩‍💻 Notes"]) {
      const { icon, title } = splitTitleEmoji(raw);
      expect(composeTitle(icon, title)).toBe(raw);
    }
  });

  it("drops the separator when there is no title, and the icon when there is none", () => {
    expect(composeTitle("🦆", "")).toBe("🦆");
    expect(composeTitle(null, "Plain")).toBe("Plain");
    expect(composeTitle(null, "")).toBe("");
  });
});
