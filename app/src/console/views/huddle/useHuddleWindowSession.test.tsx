// The window's satellite media controller, exercised against a STUB CallSession
// (the real one needs getUserMedia + a live socket + WebCodecs — unavailable
// headless). We verify the lifecycle wiring: dial the right socket, seed mute,
// push the fan-out set (self excluded), surface peers, proxy camera, end on a
// hard error, and stop on unmount.

import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { CallEvent, CallSession } from "../../../domain/call-session";
import { keyHex } from "../../../domain/chat-client";
import { callSocketUrl } from "../../../domain/transport";
import type { HuddleContext } from "../../store/huddle-window";
import { useHuddleWindowSession } from "./useHuddleWindowSession";

const bytes = (t: string): number[] => Array.from(new TextEncoder().encode(t));

const makeStub = () => {
  const calls = {
    start: [] as string[],
    muted: [] as boolean[],
    camera: [] as boolean[],
    recipients: [] as string[][],
    stopped: 0,
  };
  let cb: (e: CallEvent) => void = () => {};
  const session: CallSession = {
    start: (url) => calls.start.push(url),
    setRecipients: (r) => calls.recipients.push(r),
    setMuted: (m) => calls.muted.push(m),
    setCamera: (on) => calls.camera.push(on),
    setScreenShare: () => {},
    setDevices: () => {},
    bindTile: () => {},
    bindPreview: () => {},
    stop: () => {
      calls.stopped += 1;
    },
  };
  const factory = (c: (e: CallEvent) => void): CallSession => {
    cb = c;
    return session;
  };
  return { calls, fire: (e: CallEvent) => cb(e), factory };
};

const ctx = (): HuddleContext => ({
  channelName: "general",
  channelId: "ch-1",
  nodeUrl: "http://127.0.0.1:4321",
  selfNodeHex: keyHex([9]),
  canEncode: true,
  canDecode: true,
  authorNames: {},
  roster: [
    { user: bytes("alice"), node: [9], joined_at: 0 }, // co-located → self
    { user: bytes("bob"), node: [1], joined_at: 1 },
  ],
  seedMuted: true,
});

describe("useHuddleWindowSession", () => {
  it("dials the channel socket, seeds mute, and pushes the fan-out set (self excluded)", () => {
    const stub = makeStub();
    const c = ctx();
    renderHook(() => useHuddleWindowSession(c, vi.fn(), stub.factory));
    expect(stub.calls.start).toEqual([callSocketUrl(c.nodeUrl, c.channelId)]);
    expect(stub.calls.muted[0]).toBe(true);
    expect(stub.calls.recipients[stub.calls.recipients.length - 1]).toEqual([keyHex([1])]);
  });

  it("surfaces a peer beacon in the projected roster", () => {
    const stub = makeStub();
    const { result } = renderHook(() => useHuddleWindowSession(ctx(), vi.fn(), stub.factory));
    act(() => stub.fire({ kind: "peerBeacon", peer: keyHex([1]), muted: true, cameraOn: false, sharing: false, atMs: 2 }));
    const bob = result.current?.participants.find((p) => !p.isSelf);
    expect(bob?.muted).toBe(true);
  });

  it("proxies mute and camera toggles to the session and local state", () => {
    const stub = makeStub();
    const { result } = renderHook(() => useHuddleWindowSession(ctx(), vi.fn(), stub.factory));
    act(() => result.current?.setCamera(true));
    expect(stub.calls.camera).toContain(true);
    expect(result.current?.cameraOn).toBe(true);
    act(() => result.current?.setMuted(false));
    expect(stub.calls.muted).toContain(false);
    expect(result.current?.muted).toBe(false);
  });

  it("signals the fallback (onMediaEnded) on both a hard error and a replaced session", () => {
    const onError = vi.fn();
    const s1 = makeStub();
    renderHook(() => useHuddleWindowSession(ctx(), onError, s1.factory));
    act(() => s1.fire({ kind: "status", status: "error", error: "connection" }));
    expect(onError).toHaveBeenCalledWith("error");

    const onClosed = vi.fn();
    const s2 = makeStub();
    renderHook(() => useHuddleWindowSession(ctx(), onClosed, s2.factory));
    act(() => s2.fire({ kind: "status", status: "closed" }));
    expect(onClosed).toHaveBeenCalledWith("closed");
  });

  it("re-pushes the fan-out set when the roster changes", () => {
    const stub = makeStub();
    const { rerender } = renderHook(({ c }: { c: HuddleContext }) => useHuddleWindowSession(c, vi.fn(), stub.factory), {
      initialProps: { c: ctx() },
    });
    const grown: HuddleContext = {
      ...ctx(),
      roster: [...ctx().roster, { user: bytes("cara"), node: [2], joined_at: 2 }],
    };
    rerender({ c: grown });
    expect(stub.calls.recipients[stub.calls.recipients.length - 1]).toEqual([keyHex([1]), keyHex([2])]);
  });

  it("stops the session on unmount", () => {
    const stub = makeStub();
    const { unmount } = renderHook(() => useHuddleWindowSession(ctx(), vi.fn(), stub.factory));
    unmount();
    expect(stub.calls.stopped).toBeGreaterThan(0);
  });
});
