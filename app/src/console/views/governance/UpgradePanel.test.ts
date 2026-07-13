import { describe, expect, it } from "vitest";

import { validateScheduleForm } from "./UpgradePanel";

const base = { currentVersion: 3, currentHeight: 100 };

describe("validateScheduleForm", () => {
  it("accepts a named, higher version at a strictly future height", () => {
    expect(
      validateScheduleForm({ name: "forge-v2", toVersion: 4, activationHeight: 200, ...base }),
    ).toBeNull();
  });

  it("rejects a blank name, a non-increasing version, or a non-future height", () => {
    expect(
      validateScheduleForm({ name: "  ", toVersion: 4, activationHeight: 200, ...base }),
    ).toMatch(/name/i);
    expect(
      validateScheduleForm({ name: "x", toVersion: 3, activationHeight: 200, ...base }),
    ).toMatch(/version/i);
    expect(
      validateScheduleForm({ name: "x", toVersion: 4, activationHeight: 100, ...base }),
    ).toMatch(/height/i);
    expect(
      validateScheduleForm({ name: "x", toVersion: Number("nope"), activationHeight: 200, ...base }),
    ).toMatch(/version/i);
  });
});
