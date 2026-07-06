// The pure, audio-free parts of the voice layer: PCM conversion, the fan-out
// set derivation (self excluded), and the ws url. The audio graph itself needs
// a real browser AudioContext and is exercised at runtime, not here.

import { describe, expect, it } from "vitest";

import { keyBytes, keyHex } from "./chat-client";
import type { HuddleMember } from "./chat-client";
import { callSocketUrl } from "./transport";
import { FRAME_SAMPLES, floatToPcm16, huddleRecipients, pcm16ToFloat } from "./voice-session";

describe("pcm conversion", () => {
  it("round-trips Float32 → Int16 → Float32 within one quantization step", () => {
    const input = new Float32Array([0, 0.5, -0.5, 1, -1, 0.25, -0.75]);
    const back = pcm16ToFloat(floatToPcm16(input));
    for (let i = 0; i < input.length; i++) {
      expect(back[i]).toBeCloseTo(input[i], 3);
    }
  });

  it("clamps out-of-range samples to the Int16 extremes", () => {
    const pcm = floatToPcm16(new Float32Array([2, -2]));
    expect(pcm[0]).toBe(0x7fff);
    expect(pcm[1]).toBe(-0x8000);
  });

  it("produces an exact-fit buffer for a 20 ms frame (1920 bytes)", () => {
    const pcm = floatToPcm16(new Float32Array(FRAME_SAMPLES));
    expect(pcm.length).toBe(960);
    expect(pcm.buffer.byteLength).toBe(1920);
  });
});

describe("huddleRecipients", () => {
  const member = (node: number[]): HuddleMember => ({ user: [], node, joined_at: 0 });

  it("hex-encodes every peer's node key and excludes our own", () => {
    const selfNode = [1, 2, 3];
    const peerA = [10, 20, 30];
    const peerB = [40, 50, 60];
    const roster = [member(selfNode), member(peerA), member(peerB)];
    expect(huddleRecipients(roster, keyHex(selfNode))).toEqual([keyHex(peerA), keyHex(peerB)]);
  });

  it("matches the self key case-insensitively", () => {
    const selfNode = [0xab, 0xcd];
    expect(huddleRecipients([member(selfNode)], keyHex(selfNode).toUpperCase())).toEqual([]);
  });
});

describe("hex key round-trip", () => {
  it("keyBytes is the inverse of keyHex", () => {
    const bytes = [0, 1, 15, 16, 255, 128, 64];
    expect(keyBytes(keyHex(bytes))).toEqual(bytes);
  });

  it("decodes a 64-char public key into 32 bytes", () => {
    const hex = "ab".repeat(32);
    const bytes = keyBytes(hex);
    expect(bytes).toHaveLength(32);
    expect(bytes.every((b) => b === 0xab)).toBe(true);
  });
});

describe("callSocketUrl", () => {
  it("swaps http→ws, keeps host/port, and appends the channel query", () => {
    expect(callSocketUrl("http://127.0.0.1:8844", "general")).toBe(
      "ws://127.0.0.1:8844/v1/call/ws?channel=general",
    );
  });

  it("swaps https→wss, strips a trailing slash, and encodes the channel", () => {
    expect(callSocketUrl("https://node.example:9000/", "a b")).toBe(
      "wss://node.example:9000/v1/call/ws?channel=a%20b",
    );
  });
});
