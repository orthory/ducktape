// W1 store network-scoping: the seat model that feeds the rail. Pure —
// no provider/DOM. Seats are the registry networks (join order) plus the
// badged remote seat when a client connection is live; the active seat is the
// single live node; node control is a per-seat rule.

import { describe, expect, it } from "vitest";

import type { Workspace } from "../../domain/workspace-client";
import {
  activeSeat,
  networksFrom,
  nodeControlForSeat,
  seatColor,
  seatInitial,
} from "../../console/store/networks";

const ws = (over: Partial<Workspace>): Workspace => ({
  id: "a",
  name: "Alpha",
  chainId: "alpha#0001",
  pubkey: "aa",
  founder: true,
  member: true,
  ports: { listen: 1, http: 2, rpc: 3 },
  ...over,
});

describe("networksFrom", () => {
  it("lists local networks in join (registry) order, marking the active one", () => {
    const state = {
      workspaces: [ws({ id: "a", name: "Alpha" }), ws({ id: "b", name: "Beta" })],
      workspace: ws({ id: "b", name: "Beta" }),
      nodeUrl: "http://127.0.0.1:2",
    };
    const seats = networksFrom(state);
    expect(seats.map((s) => s.id)).toEqual(["a", "b"]);
    expect(seats.map((s) => s.active)).toEqual([false, true]);
    expect(seats.every((s) => s.kind === "local")).toBe(true);
  });

  it("appends a badged, active remote seat in client mode (no active workspace)", () => {
    const state = {
      workspaces: [ws({ id: "a" })],
      workspace: null,
      nodeUrl: "http://10.0.0.5:8844",
    };
    const seats = networksFrom(state);
    expect(seats).toHaveLength(2);
    const remote = seats[1];
    expect(remote.kind).toBe("remote");
    expect(remote.active).toBe(true);
    expect(remote.id).toBe("http://10.0.0.5:8844");
    // the local seat is not the live one while the remote connection holds.
    expect(seats[0].active).toBe(false);
  });

  it("has no seats and no active seat before anything connects", () => {
    const state = { workspaces: [], workspace: null, nodeUrl: null };
    expect(networksFrom(state)).toEqual([]);
    expect(activeSeat(state)).toBeNull();
  });
});

describe("activeSeat", () => {
  it("is the live local network", () => {
    const state = {
      workspaces: [ws({ id: "a" }), ws({ id: "b" })],
      workspace: ws({ id: "a" }),
      nodeUrl: "http://127.0.0.1:2",
    };
    expect(activeSeat(state)?.id).toBe("a");
  });
});

describe("nodeControlForSeat (ADR A5, interim form)", () => {
  it("a managed local seat is controllable", () => {
    expect(nodeControlForSeat("local", true)).toBe(true);
  });
  it("an unmanaged local seat is not", () => {
    expect(nodeControlForSeat("local", false)).toBe(false);
  });
  it("a remote seat is never controllable (A6)", () => {
    expect(nodeControlForSeat("remote", true)).toBe(false);
  });
});

describe("chip identity", () => {
  it("seatInitial is the first glyph, uppercased", () => {
    expect(seatInitial("beta net")).toBe("B");
    expect(seatInitial("  ")).toBe("?");
  });

  it("seatColor is deterministic per chain id and theme-invariant hsl", () => {
    const c1 = seatColor({ chainId: "alpha#0001", id: "a" });
    const c2 = seatColor({ chainId: "alpha#0001", id: "different" });
    expect(c1).toMatch(/^hsl\(\d{1,3}, 55%, 45%\)$/);
    // color follows the chain id, not the seat id.
    expect(c1).toBe(c2);
    // a different chain id yields a different hue (with overwhelming odds).
    expect(seatColor({ chainId: "beta#0002", id: "b" })).not.toBe(c1);
  });

  it("a remote seat with no chain id colors from its id", () => {
    expect(seatColor({ chainId: "", id: "http://x" })).toMatch(/^hsl\(/);
  });
});
