import { describe, expect, it } from "vitest";

import { classifyBootError, INCOMPATIBLE_STATE_SCHEMA_MARKER } from "./boot-error";

describe("classifyBootError", () => {
  it("recognizes an incompatible workspace from daemon.log after a generic timeout", () => {
    expect(
      classifyBootError(
        "node did not come up",
        `[node qa] FATAL: ${INCOMPATIBLE_STATE_SCHEMA_MARKER}: legacy/unversioned`,
      ),
    ).toBe("incompatible_workspace");
  });

  it("keeps ordinary startup failures on the retryable surface", () => {
    expect(classifyBootError("address already in use", "FATAL bind failed")).toBe(
      "startup_failure",
    );
  });
});
