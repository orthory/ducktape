// The pure halves of the huddle pop-out bridge: the context the main window
// pushes (so the popped window can run its own session + roster), and the
// command→action mapping it applies. The Tauri emit/listen wiring itself needs
// real windows and is exercised at runtime.

import { describe, expect, it, vi } from "vitest";

import { keyHex } from "../../domain/chat-client";
import type { Channel } from "../../domain/chat-client";
import type { VideoCapability } from "../../domain/video-capability";
import type { VoiceSlice } from "./state";
import { applyHuddleWindowCmd, buildHuddleContext } from "./huddle-window";

const bytes = (text: string): number[] => Array.from(new TextEncoder().encode(text));
const NOW = 1_000_000;
const CAP: VideoCapability = { canEncode: true, canDecode: true, canScreenShare: true };

const voice = (over: Partial<VoiceSlice> = {}): VoiceSlice => ({
  channelId: "ch-1",
  muted: true,
  status: "live",
  error: null,
  errorNote: null,
  mediaNote: null,
  popped: true,
  cameraOn: false,
  sharing: false,
  peers: {},
  sessionStartMs: NOW - 1_000,
  speaking: false,
  level: 0,
  ...over,
});

const channel = (over: Partial<Channel> = {}): Channel =>
  ({
    id: "ch-1",
    name: "general",
    huddle: [],
    ...over,
  }) as Channel;

describe("buildHuddleContext", () => {
  const selfHex = keyHex([9]);

  it("is null when not in a huddle", () => {
    expect(buildHuddleContext(voice({ channelId: null }), [], {}, "http://n", selfHex, CAP)).toBeNull();
  });

  it("is null when the node url is unresolved", () => {
    expect(buildHuddleContext(voice(), [], {}, null, selfHex, CAP)).toBeNull();
  });

  it("carries the node context + raw roster + caps + seed mute the window needs", () => {
    const alice = bytes("alice");
    const bob = bytes("bob");
    const names = { [keyHex(bob)]: "Bob!" };
    const roster = [
      { user: alice, node: [9], joined_at: 0 },
      { user: bob, node: [1], joined_at: 1 },
    ];
    const ctx = buildHuddleContext(
      voice(),
      [channel({ huddle: roster })],
      names,
      "http://127.0.0.1:4321",
      selfHex,
      CAP,
    );
    expect(ctx?.channelName).toBe("general");
    expect(ctx?.channelId).toBe("ch-1");
    expect(ctx?.nodeUrl).toBe("http://127.0.0.1:4321");
    expect(ctx?.selfNodeHex).toBe(selfHex.toLowerCase());
    expect(ctx?.canEncode).toBe(true);
    expect(ctx?.canDecode).toBe(true);
    expect(ctx?.authorNames).toEqual(names);
    expect(ctx?.roster).toEqual(roster);
    expect(ctx?.seedMuted).toBe(true);
  });

  it("falls back to the channel id when the channel is not in the snapshot yet", () => {
    const ctx = buildHuddleContext(voice(), [], {}, "http://n", selfHex, CAP);
    expect(ctx?.channelName).toBe("ch-1");
    expect(ctx?.roster).toEqual([]);
  });
});

describe("applyHuddleWindowCmd", () => {
  const actions = () => ({
    leaveHuddle: vi.fn<() => void>(),
    sweepHuddle: vi.fn<(channelId: string, user: number[]) => void>(),
    noteHuddleMuted: vi.fn<(muted: boolean) => void>(),
  });

  it("maps leave onto the store action", () => {
    const a = actions();
    applyHuddleWindowCmd({ op: "leave" }, a, "ch-1");
    expect(a.leaveHuddle).toHaveBeenCalledOnce();
  });

  it("records the window's mute so a re-take keeps it", () => {
    const a = actions();
    applyHuddleWindowCmd({ op: "mute", muted: false }, a, "ch-1");
    expect(a.noteHuddleMuted).toHaveBeenCalledWith(false);
  });

  it("maps sweep onto sweepHuddle for the current channel, and only when one exists", () => {
    const a = actions();
    const bob = bytes("bob");
    applyHuddleWindowCmd({ op: "sweep", user: bob }, a, "ch-1");
    expect(a.sweepHuddle).toHaveBeenCalledWith("ch-1", bob);
    applyHuddleWindowCmd({ op: "sweep", user: bob }, a, null);
    expect(a.sweepHuddle).toHaveBeenCalledOnce();
  });

  it("treats ready as a no-op for the store", () => {
    const a = actions();
    applyHuddleWindowCmd({ op: "ready" }, a, "ch-1");
    expect(a.leaveHuddle).not.toHaveBeenCalled();
    expect(a.sweepHuddle).not.toHaveBeenCalled();
  });
});
