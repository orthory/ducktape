import { describe, expect, it } from "vitest";

import { displayNameForKey, normalizeKey, sameKey, shortKey } from "./names";

const key = "AB".repeat(32);

describe("names", () => {
  it("normalizes keys to trimmed, unprefixed lowercase", () => {
    expect(normalizeKey(` 0x${key} `)).toBe(key.toLowerCase());
    expect(normalizeKey(null)).toBe("");
  });

  it("compares keys by normalized form and never matches empties", () => {
    expect(sameKey(`0x${key}`, key.toLowerCase())).toBe(true);
    expect(sameKey("", "")).toBe(false);
  });

  it("truncates long keys and dashes empties", () => {
    expect(shortKey(key)).toBe(`${key.slice(0, 10)}…${key.slice(-6)}`);
    expect(shortKey("")).toBe("—");
  });

  it("resolves display names exactly or via the normalized key", () => {
    const names = { [key.toLowerCase()]: "Founder Rae" };
    expect(displayNameForKey(key.toLowerCase(), names)).toBe("Founder Rae");
    expect(displayNameForKey(`0x${key}`, names)).toBe("Founder Rae");
    expect(displayNameForKey("c".repeat(64), names)).toBeNull();
  });
});
