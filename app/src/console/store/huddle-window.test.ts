// The pure halves of the huddle pop-out bridge: the display-state projection
// the main window emits, and the command→action mapping it applies. The Tauri
// emit/listen wiring itself needs real windows and is exercised at runtime.

import { describe, expect, it, vi } from "vitest";

import { keyHex } from "../../domain/chat-client";
import type { Channel } from "../../domain/chat-client";
import type { VoiceSlice } from "./state";
import { applyHuddleWindowCmd, buildHuddleWindowState } from "./huddle-window";

const bytes = (text: string): number[] => Array.from(new TextEncoder().encode(text));

const voice = (over: Partial<VoiceSlice> = {}): VoiceSlice => ({
  channelId: "ch-1",
  muted: true,
  status: "live",
  error: null,
  popped: true,
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
  it("is null when not in a huddle", () => {
    expect(buildHuddleWindowState(voice({ channelId: null }), [], {})).toBeNull();
  });

  it("projects the slice plus the channel's roster as display names", () => {
    const alice = bytes("alice");
    const bob = bytes("bob");
    const names = { [keyHex(bob)]: "Bob!" };
    const state = buildHuddleWindowState(
      voice(),
      [channel({ huddle: [{ user: alice, node: [1], joined_at: 0 }, { user: bob, node: [1], joined_at: 1 }] })],
      names,
    );
    expect(state).toEqual({
      channelName: "general",
      status: "live",
      error: null,
      muted: true,
      participants: ["alice", "Bob!"],
    });
  });

  it("falls back to the channel id when the channel is not in the snapshot yet", () => {
    const state = buildHuddleWindowState(voice(), [], {});
    expect(state?.channelName).toBe("ch-1");
    expect(state?.participants).toEqual([]);
  });
});

describe("applyHuddleWindowCmd", () => {
  const actions = () => ({
    setHuddleMuted: vi.fn<(muted: boolean) => void>(),
    leaveHuddle: vi.fn<() => void>(),
    joinHuddle: vi.fn<(channelId: string) => void>(),
  });

  it("maps set-muted and leave onto the store actions", () => {
    const a = actions();
    applyHuddleWindowCmd({ op: "set-muted", muted: false }, a, "ch-1");
    applyHuddleWindowCmd({ op: "leave" }, a, "ch-1");
    expect(a.setHuddleMuted).toHaveBeenCalledWith(false);
    expect(a.leaveHuddle).toHaveBeenCalledOnce();
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
