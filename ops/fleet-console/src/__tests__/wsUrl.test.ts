import { describe, it, expect } from "vitest";
import { wsUrlFromLocation } from "../wsUrl";

describe("wsUrlFromLocation", () => {
  it("builds a ws:// token URL from a plain http location", () => {
    expect(
      wsUrlFromLocation(
        { protocol: "http:", host: "100.76.154.57:6090" },
        "feat-qa-multiwindow",
      ),
    ).toBe("ws://100.76.154.57:6090/websockify?token=feat-qa-multiwindow");
  });

  it("upgrades to wss:// under https and encodes the token", () => {
    expect(
      wsUrlFromLocation(
        { protocol: "https:", host: "zk.example.ts.net" },
        "feat/odd name",
      ),
    ).toBe("wss://zk.example.ts.net/websockify?token=feat%2Fodd%20name");
  });
});
