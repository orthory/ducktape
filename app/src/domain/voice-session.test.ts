// The pure, audio-free parts of the voice layer: PCM conversion, the fan-out
// set derivation (self excluded), and the ws url. The audio graph itself needs
// a real browser AudioContext and is exercised at runtime, not here.

import { describe, expect, it } from "vitest";

import { keyBytes, keyHex } from "./chat-client";
import type { HuddleMember } from "./chat-client";
import { callSocketUrl } from "./transport";
import {
  FRAME_SAMPLES,
  SPEAKING_HOLD_MS,
  SPEAKING_RMS,
  floatToPcm16,
  huddleRecipients,
  nextSpeaking,
  pcm16ToFloat,
  rms,
  voiceErrorOf,
} from "./voice-session";

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

describe("rms", () => {
  it("is 0 for silence", () => {
    expect(rms(new Float32Array(960))).toBe(0);
  });
  it("is the amplitude for a full-scale constant signal", () => {
    expect(rms(new Float32Array([1, -1, 1, -1]))).toBeCloseTo(1, 6);
  });
  it("rises with louder input", () => {
    expect(rms(new Float32Array([0.2, -0.2]))).toBeLessThan(rms(new Float32Array([0.8, -0.8])));
  });
});

describe("nextSpeaking (threshold + hold)", () => {
  it("goes active immediately once RMS crosses the threshold", () => {
    const s = nextSpeaking(SPEAKING_RMS + 0.01, 1000, 0);
    expect(s.speaking).toBe(true);
    expect(s.holdUntil).toBe(1000 + SPEAKING_HOLD_MS);
  });

  it("stays silent below the threshold with no active hold", () => {
    expect(nextSpeaking(SPEAKING_RMS - 0.001, 1000, 0)).toEqual({ speaking: false, holdUntil: 0 });
  });

  it("holds 'speaking' through a brief dip below threshold (anti-flicker)", () => {
    const loud = nextSpeaking(SPEAKING_RMS + 0.05, 1000, 0); // holdUntil = 1000 + HOLD
    const dip = nextSpeaking(0, 1000 + SPEAKING_HOLD_MS - 1, loud.holdUntil);
    expect(dip.speaking).toBe(true); // still within the hold window
  });

  it("drops 'speaking' once the hold window elapses", () => {
    const loud = nextSpeaking(SPEAKING_RMS + 0.05, 1000, 0);
    const after = nextSpeaking(0, 1000 + SPEAKING_HOLD_MS + 1, loud.holdUntil);
    expect(after.speaking).toBe(false);
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

describe("voiceErrorOf", () => {
  it("maps permission denials to mic-denied", () => {
    expect(voiceErrorOf("NotAllowedError")).toBe("mic-denied");
    expect(voiceErrorOf("SecurityError")).toBe("mic-denied");
    expect(voiceErrorOf("PermissionDeniedError")).toBe("mic-denied");
  });

  it("maps absent or unusable devices to mic-missing", () => {
    expect(voiceErrorOf("NotFoundError")).toBe("mic-missing");
    expect(voiceErrorOf("DevicesNotFoundError")).toBe("mic-missing");
    expect(voiceErrorOf("OverconstrainedError")).toBe("mic-missing");
    expect(voiceErrorOf("NotReadableError")).toBe("mic-missing");
  });

  it("maps anything else to mic-failed", () => {
    expect(voiceErrorOf("AbortError")).toBe("mic-failed");
    expect(voiceErrorOf("")).toBe("mic-failed");
  });
});
