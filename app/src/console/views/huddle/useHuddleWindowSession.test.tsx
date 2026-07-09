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
    act(() => stub.fire({ kind: "peerBeacon", peer: keyHex([1]), muted: true, cameraOn: false, atMs: 2 }));
    const bob = result.current?.participants.find((p) => !p.isSelf);
    expect(bob?.muted).toBe(true);
  });

  it("proxies the camera toggle to the session and local state", () => {
    const stub = makeStub();
    const { result } = renderHook(() => useHuddleWindowSession(ctx(), vi.fn(), stub.factory));
    act(() => result.current?.setCamera(true));
    expect(stub.calls.camera).toContain(true);
    expect(result.current?.cameraOn).toBe(true);
  });

  it("ends the session (fallback signal) on a hard error", () => {
    const stub = makeStub();
    const onEnded = vi.fn();
    const { result } = renderHook(() => useHuddleWindowSession(ctx(), onEnded, stub.factory));
    act(() => stub.fire({ kind: "status", status: "error", error: "connection" }));
    expect(onEnded).toHaveBeenCalledWith("error");
    expect(result.current?.status).toBe("error");
  });

  it("stops the session on unmount", () => {
    const stub = makeStub();
    const { unmount } = renderHook(() => useHuddleWindowSession(ctx(), vi.fn(), stub.factory));
    unmount();
    expect(stub.calls.stopped).toBeGreaterThan(0);
  });
});
