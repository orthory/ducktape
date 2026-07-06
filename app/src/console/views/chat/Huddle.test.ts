// The one pure piece of the huddle tile surface: the sweep-affordance staleness
// predicate. Everything else in Huddle.tsx is presentational and is exercised in
// Task 10's live QA.

import { describe, expect, it } from "vitest";

import { STALE_BEACON_MS, isBeaconStale } from "./Huddle";

describe("isBeaconStale", () => {
  const now = 1_000_000;

  it("a fresh beacon is not stale", () => {
    expect(isBeaconStale({ muted: false, cameraOn: true, atMs: now - 1_000 }, 0, now)).toBe(false);
  });

  it("a beacon silent past the window is stale", () => {
    const beacon = { muted: false, cameraOn: false, atMs: now - (STALE_BEACON_MS + 1) };
    expect(isBeaconStale(beacon, 0, now)).toBe(true);
  });

  it("uses the beacon's own timestamp, not the session start, when present", () => {
    // A long-running session, but a recent beacon keeps the member fresh.
    expect(
      isBeaconStale({ muted: true, cameraOn: false, atMs: now - 500 }, now - 60_000, now),
    ).toBe(false);
  });

  it("does NOT flag a never-beaconed member right after we join", () => {
    // Session started 2 s ago, no beacon yet — a peer's first beacon lags ~1 s.
    expect(isBeaconStale(undefined, now - 2_000, now)).toBe(false);
  });

  it("flags a never-beaconed member once our session outlives the window", () => {
    expect(isBeaconStale(undefined, now - (STALE_BEACON_MS + 1), now)).toBe(true);
  });
});
