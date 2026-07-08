// The pure huddle roster projection shared by the in-app dock and the popped-out
// window: turn the committed roster + ephemeral beacons into per-member rows
// (name, known-muted, removable-when-stale, self) so mute + sweep work in EVERY
// huddle, audio-only included — not just when video tiles happen to be showing.

import { describe, expect, it } from "vitest";

import { keyHex } from "../../domain/chat-client";
import type { HuddleMember } from "../../domain/chat-client";
import { STALE_BEACON_MS, buildParticipants, isBeaconStale } from "./huddle-roster";

const bytes = (text: string): number[] => Array.from(new TextEncoder().encode(text));
const member = (user: string, node: number[], joined = 0): HuddleMember => ({
  user: bytes(user),
  node,
  joined_at: joined,
});

const NOW = 1_000_000;
const selfNode = [9];
const selfHex = keyHex(selfNode);

describe("isBeaconStale", () => {
  it("a fresh beacon is not stale", () => {
    expect(isBeaconStale({ muted: false, cameraOn: true, atMs: NOW - 1_000 }, 0, NOW)).toBe(false);
  });
  it("a beacon silent past the window is stale", () => {
    expect(isBeaconStale({ muted: false, cameraOn: false, atMs: NOW - (STALE_BEACON_MS + 1) }, 0, NOW)).toBe(true);
  });
  it("does NOT flag a never-beaconed member right after we join", () => {
    expect(isBeaconStale(undefined, NOW - 2_000, NOW)).toBe(false);
  });
  it("flags a never-beaconed member once our session outlives the window", () => {
    expect(isBeaconStale(undefined, NOW - (STALE_BEACON_MS + 1), NOW)).toBe(true);
  });
});

describe("buildParticipants", () => {
  const base = {
    peers: {} as Record<string, { muted: boolean; cameraOn: boolean; atMs: number }>,
    selfNodeHex: selfHex,
    authorNames: {} as Record<string, string>,
    selfMuted: false,
    selfSpeaking: false,
    sessionStartMs: NOW - 1_000,
    now: NOW,
  };

  it("flags the self member by node and never marks it stale or removable", () => {
    const rows = buildParticipants({
      ...base,
      roster: [member("me", selfNode)],
      sessionStartMs: NOW - 60_000, // long session, but self is never stale
      selfMuted: true,
    });
    expect(rows).toHaveLength(1);
    expect(rows[0].isSelf).toBe(true);
    expect(rows[0].stale).toBe(false);
    expect(rows[0].muted).toBe(true); // self mute comes from selfMuted, not a beacon
  });

  it("shows a peer as muted only when its beacon says so (not merely absent)", () => {
    const bob = member("bob", [1]);
    const carol = member("carol", [2]);
    const rows = buildParticipants({
      ...base,
      roster: [bob, carol],
      peers: { [keyHex([1])]: { muted: true, cameraOn: false, atMs: NOW } }, // bob known-muted; carol no beacon
    });
    const byKey = Object.fromEntries(rows.map((r) => [r.key, r]));
    expect(byKey[keyHex(bytes("bob"))].muted).toBe(true);
    expect(byKey[keyHex(bytes("carol"))].muted).toBe(false); // no beacon != muted
  });

  it("marks a peer stale (removable) once its beacon goes silent past the window", () => {
    const bob = member("bob", [1]);
    const rows = buildParticipants({
      ...base,
      roster: [bob],
      peers: { [keyHex([1])]: { muted: false, cameraOn: false, atMs: NOW - (STALE_BEACON_MS + 1) } },
    });
    expect(rows[0].stale).toBe(true);
    expect(rows[0].isSelf).toBe(false);
    expect(rows[0].user).toEqual(bytes("bob")); // carried for SweepHuddle
  });

  it("resolves display names via the registry, else the readable fallback", () => {
    const bob = member("bob", [1]);
    const rows = buildParticipants({
      ...base,
      roster: [bob],
      authorNames: { [keyHex(bytes("bob"))]: "Bob!" },
    });
    expect(rows[0].name).toBe("Bob!");
  });

  it("preserves roster order", () => {
    const rows = buildParticipants({
      ...base,
      roster: [member("a", [1]), member("b", [2]), member("c", [3])],
    });
    expect(rows.map((r) => r.name)).toEqual(["a", "b", "c"]);
  });

  it("marks only the self row speaking (peer speaking is not derivable client-side)", () => {
    const rows = buildParticipants({
      ...base,
      roster: [member("me", selfNode), member("bob", [1])],
      selfSpeaking: true,
    });
    const byKey = Object.fromEntries(rows.map((r) => [r.key, r]));
    expect(byKey[keyHex(bytes("me"))].speaking).toBe(true);
    expect(byKey[keyHex(bytes("bob"))].speaking).toBe(false);
  });
});
