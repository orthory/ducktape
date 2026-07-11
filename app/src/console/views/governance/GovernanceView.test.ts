import { describe, expect, it } from "vitest";

import { formatSharePercent, parseShareAllocations } from "./GovernanceView";

describe("parseShareAllocations", () => {
  it("accepts explicit integer rows and rejects duplicate or malformed accounts", () => {
    expect(parseShareAllocations("aabb 60\nccdd 40")).toEqual([
      { accountId: "aabb", shares: 60 },
      { accountId: "ccdd", shares: 40 },
    ]);
    expect(parseShareAllocations("aabb 60\naabb 40")).toBeNull();
    expect(parseShareAllocations("not-hex 10")).toBeNull();
  });

  it("derives display percentages without storing them", () => {
    expect(formatSharePercent(1, 3)).toBe("33.33%");
    expect(formatSharePercent(60, 100)).toBe("60%");
  });
});
