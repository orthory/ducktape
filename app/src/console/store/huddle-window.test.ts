// The pure halves of the huddle pop-out bridge: the display-state projection
// the main window emits, and the command→action mapping it applies. The Tauri
// emit/listen wiring itself needs real windows and is exercised at runtime.

import { describe, expect, it, vi } from "vitest";

import { keyHex } from "../../domain/chat-client";
import type { Channel } from "../../domain/chat-client";
import type { VoiceSlice } from "./state";
import { applyHuddleWindowCmd, buildHuddleWindowState } from "./huddle-window";

const bytes = (text: string): number[] => Array.from(new TextEncoder().encode(text));
const NOW = 1_000_000;

const voice = (over: Partial<VoiceSlice> = {}): VoiceSlice => ({
  channelId: "ch-1",
  muted: true,
  status: "live",
  error: null,
  popped: true,
  cameraOn: false,
  peers: {},
  sessionStartMs: NOW - 1_000,
  speaking: false,
  ...over,
});

const channel = (over: Partial<Channel> = {}): Channel =>
  ({
    id: "ch-1",
    name: "general",
    huddle: [],
    ...over,
  }) as Channel;

describe("buildHuddleWindowState", () => {
  const selfHex = keyHex([9]);

  it("is null when not in a huddle", () => {
    expect(buildHuddleWindowState(voice({ channelId: null }), [], {}, selfHex, NOW)).toBeNull();
  });

  it("projects the slice plus the roster as participant rows (name/muted/stale/self)", () => {
    const alice = bytes("alice");
    const bob = bytes("bob");
    const names = { [keyHex(bob)]: "Bob!" };
    const state = buildHuddleWindowState(
      voice(),
      [
        channel({
          huddle: [
            { user: alice, node: [9], joined_at: 0 }, // co-located with us (node 9) → self
            { user: bob, node: [1], joined_at: 1 },
          ],
        }),
      ],
      names,
      selfHex,
      NOW,
    );
    expect(state?.channelName).toBe("general");
    expect(state?.muted).toBe(true);
    expect(state?.participants).toEqual([
      { key: keyHex(alice), name: "alice", muted: true, stale: false, isSelf: true, speaking: false, user: alice },
      { key: keyHex(bob), name: "Bob!", muted: false, stale: false, isSelf: false, speaking: false, user: bob },
    ]);
  });

  it("falls back to the channel id when the channel is not in the snapshot yet", () => {
    const state = buildHuddleWindowState(voice(), [], {}, selfHex, NOW);
    expect(state?.channelName).toBe("ch-1");
    expect(state?.participants).toEqual([]);
  });
});

describe("applyHuddleWindowCmd", () => {
  const actions = () => ({
    setHuddleMuted: vi.fn<(muted: boolean) => void>(),
    leaveHuddle: vi.fn<() => void>(),
    joinHuddle: vi.fn<(channelId: string) => void>(),
    sweepHuddle: vi.fn<(channelId: string, user: number[]) => void>(),
  });

  it("maps set-muted and leave onto the store actions", () => {
    const a = actions();
    applyHuddleWindowCmd({ op: "set-muted", muted: false }, a, "ch-1");
    applyHuddleWindowCmd({ op: "leave" }, a, "ch-1");
    expect(a.setHuddleMuted).toHaveBeenCalledWith(false);
    expect(a.leaveHuddle).toHaveBeenCalledOnce();
  });

  it("maps sweep onto sweepHuddle for the current channel, and only when one exists", () => {
    const a = actions();
    const bob = Array.from(new TextEncoder().encode("bob"));
    applyHuddleWindowCmd({ op: "sweep", user: bob }, a, "ch-1");
    expect(a.sweepHuddle).toHaveBeenCalledWith("ch-1", bob);
    applyHuddleWindowCmd({ op: "sweep", user: bob }, a, null);
    expect(a.sweepHuddle).toHaveBeenCalledOnce();
  });

  it("retries into the current channel, and only when one exists", () => {
    const a = actions();
    applyHuddleWindowCmd({ op: "retry" }, a, "ch-1");
    expect(a.joinHuddle).toHaveBeenCalledWith("ch-1");
    applyHuddleWindowCmd({ op: "retry" }, a, null);
    expect(a.joinHuddle).toHaveBeenCalledOnce();
  });

  it("treats ready as a no-op for the store", () => {
    const a = actions();
    applyHuddleWindowCmd({ op: "ready" }, a, "ch-1");
    expect(a.setHuddleMuted).not.toHaveBeenCalled();
    expect(a.leaveHuddle).not.toHaveBeenCalled();
    expect(a.joinHuddle).not.toHaveBeenCalled();
  });
});
