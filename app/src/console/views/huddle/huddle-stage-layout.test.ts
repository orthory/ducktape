// Pure layout maths for the huddle video stage: how many gallery columns for a
// given tile count, and which participant the spotlight follows. Kept free of
// React so the decisions are unit-tested directly.

import { describe, expect, it } from "vitest";

import type { HuddleParticipant } from "../../store/huddle-roster";
import { galleryColumns, spotlightKey } from "./huddle-stage-layout";

const p = (key: string, over: Partial<HuddleParticipant> = {}): HuddleParticipant => ({
  key,
  name: key,
  muted: false,
  stale: false,
  isSelf: false,
  speaking: false,
  user: [],
  ...over,
});

describe("galleryColumns", () => {
  it("uses one column for a single tile", () => {
    expect(galleryColumns(1)).toBe(1);
  });
  it("grows roughly with the square root of the tile count", () => {
    expect(galleryColumns(2)).toBe(2);
    expect(galleryColumns(4)).toBe(2);
    expect(galleryColumns(5)).toBe(3);
    expect(galleryColumns(9)).toBe(3);
  });
  it("caps at 4 columns for large huddles", () => {
    expect(galleryColumns(16)).toBe(4);
    expect(galleryColumns(32)).toBe(4);
  });
  it("never returns 0 for an empty stage", () => {
    expect(galleryColumns(0)).toBe(1);
  });
});

describe("spotlightKey", () => {
  const roster = [p("a"), p("b", { speaking: true }), p("c")];

  it("honors an explicit pin when that member is still present", () => {
    expect(spotlightKey(roster, "c")).toBe("c");
  });
  it("ignores a pin for a member who has left", () => {
    expect(spotlightKey(roster, "gone")).toBe("b"); // falls through to the speaker
  });
  it("follows the active speaker when nothing is pinned", () => {
    expect(spotlightKey(roster, null)).toBe("b");
  });
  it("falls back to the first member when no one is speaking", () => {
    expect(spotlightKey([p("a"), p("b")], null)).toBe("a");
  });
  it("is null for an empty roster", () => {
    expect(spotlightKey([], null)).toBeNull();
  });
});
