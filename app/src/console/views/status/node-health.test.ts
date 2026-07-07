import { describe, expect, it } from "vitest";

import type { BlockRecord } from "../../../domain/transport";
import {
  buildPeers,
  commitHealth,
  healthSegments,
  initialsOf,
  nodeLiveness,
  proposalWindow,
} from "./node-health";

// A minimal block: only the fields the derivations read matter here.
const block = (
  height: number,
  proposer: string,
  disposition: BlockRecord["disposition"] = "applied",
): BlockRecord => ({
  height,
  hash: `hash${height}`,
  commitHash: `commit${height}`,
  proposer,
  disposition,
  target: "chat",
  operations: [],
  payload: "",
});

const KEY_A = "aa".repeat(32); // 64-hex validator keys
const KEY_B = "bb".repeat(32);
const KEY_C = "cc".repeat(32);

describe("proposalWindow", () => {
  it("tallies proposals per normalized key with the highest height", () => {
    const w = proposalWindow([
      block(10, KEY_A),
      block(11, KEY_B),
      block(12, KEY_A.toUpperCase()), // casing must not split the tally
      block(13, KEY_A),
    ]);
    expect(w.total).toBe(4);
    expect(w.low).toBe(10);
    expect(w.high).toBe(13);
    expect(w.byProposer.get(KEY_A)).toEqual({ count: 3, lastHeight: 13 });
    expect(w.byProposer.get(KEY_B)).toEqual({ count: 1, lastHeight: 11 });
  });

  it("counts a blank-proposer block toward total but attributes it to no key", () => {
    const w = proposalWindow([block(1, ""), block(2, KEY_A)]);
    expect(w.total).toBe(2);
    expect(w.byProposer.size).toBe(1);
    expect(w.byProposer.get(KEY_A)?.count).toBe(1);
  });

  it("is empty for no blocks", () => {
    const w = proposalWindow([]);
    expect(w).toEqual({ byProposer: new Map(), total: 0, low: null, high: null });
  });
});

describe("buildPeers", () => {
  const base = {
    authorNames: {} as Record<string, string>,
    capabilitiesByNode: new Map<string, string[]>(),
  };

  it("derives validator liveness from the proposal window and residents get none", () => {
    const window = proposalWindow([block(5, KEY_A), block(6, KEY_A), block(7, KEY_B)]);
    const peers = buildPeers({
      ...base,
      members: [KEY_A, KEY_B],
      residents: [KEY_C],
      workspace: null,
      window,
    });

    const a = peers.find((p) => p.keyNorm === KEY_A)!;
    const b = peers.find((p) => p.keyNorm === KEY_B)!;
    const c = peers.find((p) => p.keyNorm === KEY_C)!;

    expect(a.tier).toBe("validator");
    expect(a.activity).toEqual({ count: 2, lastHeight: 6 });
    expect(a.share).toBeCloseTo(2 / 3);
    expect(b.activity).toEqual({ count: 1, lastHeight: 7 });
    // resident: no quorum seat → never proposes → no derived liveness.
    expect(c.tier).toBe("resident");
    expect(c.activity).toBeNull();
    expect(c.share).toBe(0);
  });

  it("orders validators by self, then share desc, then name; residents after", () => {
    const window = proposalWindow([block(1, KEY_B), block(2, KEY_B), block(3, KEY_A)]);
    const peers = buildPeers({
      ...base,
      members: [KEY_A, KEY_B], // A listed first but B leads more
      residents: [KEY_C],
      workspace: { pubkey: KEY_A, founder: true }, // A is self → pinned first
      window,
    });
    expect(peers.map((p) => p.keyNorm)).toEqual([KEY_A, KEY_B, KEY_C]);
    expect(peers[0].isSelf).toBe(true);
    expect(peers[0].isFounder).toBe(true);
    // B is not self, so it is NOT marked founder even though A is genesis.
    expect(peers[1].isFounder).toBe(false);
  });

  it("marks founder only for the local node, never a remote validator", () => {
    const peers = buildPeers({
      ...base,
      members: [KEY_A, KEY_B],
      residents: [],
      workspace: { pubkey: KEY_A, founder: false }, // self is a plain member
      window: proposalWindow([]),
    });
    expect(peers.every((p) => p.isFounder === false)).toBe(true);
  });

  it("resolves display names and capabilities by normalized key", () => {
    const peers = buildPeers({
      authorNames: { [KEY_A]: "genesis-node" },
      capabilitiesByNode: new Map([[KEY_A, ["gpu", "oracle"]]]),
      members: [KEY_A],
      residents: [],
      workspace: null,
      window: proposalWindow([]),
    });
    expect(peers[0].displayName).toBe("genesis-node");
    expect(peers[0].initials).toBe("GE");
    expect(peers[0].capabilities).toEqual(["gpu", "oracle"]);
  });
});

describe("healthSegments", () => {
  it("keeps the last `slots` blocks, oldest-first (newest renders right)", () => {
    const blocks = [block(1, KEY_A), block(2, KEY_A), block(3, KEY_A), block(4, KEY_A)];
    const segs = healthSegments(blocks, 2);
    expect(segs.map((s) => s.height)).toEqual([3, 4]);
  });

  it("returns everything when fewer blocks than slots", () => {
    const segs = healthSegments([block(1, KEY_A)], 10);
    expect(segs).toHaveLength(1);
  });
});

describe("commitHealth", () => {
  it("splits applied vs rejected", () => {
    const segs = healthSegments(
      [block(1, KEY_A), block(2, KEY_A, "rejected"), block(3, KEY_A)],
      10,
    );
    expect(commitHealth(segs)).toEqual({ applied: 2, rejected: 1, total: 3 });
  });

  it("is all-zero for an empty strip", () => {
    expect(commitHealth([])).toEqual({ applied: 0, rejected: 0, total: 0 });
  });
});

describe("nodeLiveness", () => {
  it("is stopped for a disconnected managed node, offline for a remote one", () => {
    expect(nodeLiveness({ connected: false, managed: true, tip: null }).tone).toBe("stopped");
    expect(nodeLiveness({ connected: false, managed: false, tip: 5 }).tone).toBe("offline");
  });

  it("is idle when connected but no tip has arrived, live once it advances", () => {
    expect(nodeLiveness({ connected: true, managed: true, tip: null }).tone).toBe("idle");
    const live = nodeLiveness({ connected: true, managed: false, tip: 1234 });
    expect(live.tone).toBe("live");
    expect(live.detail).toContain("1,234");
  });
});

describe("initialsOf", () => {
  it("takes two words, then two alnum chars, dropping parentheticals", () => {
    expect(initialsOf("eddy hong")).toBe("EH");
    expect(initialsOf("eddy (joined node)")).toBe("ED");
    expect(initialsOf("aa11bb22")).toBe("AA");
    expect(initialsOf("")).toBe("?");
  });
});
