// The pure, browser-free helpers of a huddle — everything below the consensus
// roster that a test can exercise without a real AudioContext: PCM ⇄ Float
// conversion, the fan-out set derivation (self excluded), and the shared audio
// constants. The live audio + camera graph moved to `call-session.ts`, which
// bridges these helpers to the node's typed `/v1/call/ws` socket; this module
// is deliberately runtime-free so it is unit-tested directly.

import { keyHex } from "./chat-client";
import type { HuddleMember } from "./chat-client";

/** 48 kHz mono — the daemon's fixed voice format. */
export const SAMPLE_RATE = 48_000;
/** 20 ms at 48 kHz = 960 samples per frame (1920 bytes Int16). */
export const FRAME_SAMPLES = 960;

/** A huddle session's lifecycle, mirrored into the ui's voice slice. */
export type VoiceStatus = "connecting" | "live" | "error" | "closed";

/** Why a session failed — which message the dock shows. `mic-*` come from the
 *  capture graph (getUserMedia/worklet), `connection` from the call ws,
 *  `refused` from a consensus join rejection, and `removed` when the finalized
 *  roster dropped us while the session was live — a sweep by another member, or
 *  another device of this identity leaving (both assigned by the caller). */
export type VoiceError =
  | "mic-denied"
  | "mic-missing"
  | "mic-failed"
  | "connection"
  | "refused"
  | "removed";

// ── Pure helpers (tested) ───────────────────────────────

/** Float32 [-1,1] samples → Int16 little-endian PCM, clamped. A fresh exact-fit
 *  Int16Array (backed by a plain ArrayBuffer) so its `.buffer` is precisely
 *  `2 * length` bytes and goes straight to WebSocket.send. */
export const floatToPcm16 = (input: Float32Array): Int16Array<ArrayBuffer> => {
  const out = new Int16Array(input.length);
  for (let i = 0; i < input.length; i++) {
    const s = Math.max(-1, Math.min(1, input[i]));
    out[i] = s < 0 ? s * 0x8000 : s * 0x7fff;
  }
  return out;
};

/** Int16 PCM → Float32 [-1,1] — the inverse of `floatToPcm16`. */
export const pcm16ToFloat = (input: Int16Array): Float32Array<ArrayBuffer> => {
  const out = new Float32Array(input.length);
  for (let i = 0; i < input.length; i++) {
    const v = input[i];
    out[i] = v < 0 ? v / 0x8000 : v / 0x7fff;
  }
  return out;
};

/** The fan-out set for a huddle: every member's node key as hex, EXCLUDING our
 *  own node key (status.publicKey) and deduplicated — two users huddling from
 *  the same daemon share one node, which must receive each frame once. */
export const huddleRecipients = (
  huddle: HuddleMember[],
  selfNodeHex: string,
): string[] => {
  const self = selfNodeHex.toLowerCase();
  return Array.from(
    new Set(huddle.map((m) => keyHex(m.node)).filter((hex) => hex !== self)),
  );
};

// ── Self active-speaker detection ───────────────────────

/** Root-mean-square amplitude of a mono frame — a cheap "how loud" signal used
 *  to drive the self speaking indicator (and the "you're muted while talking"
 *  banner). Computed on the capture frames the worklet already posts, so it runs
 *  even while muted (mute only stops FORWARDING, not capturing). */
export const rms = (samples: Float32Array): number => {
  if (samples.length === 0) return 0;
  let sum = 0;
  for (let i = 0; i < samples.length; i++) sum += samples[i] * samples[i];
  return Math.sqrt(sum / samples.length);
};

/** Speaking threshold (RMS, ~-34 dBFS) — above this we consider the mic active. */
export const SPEAKING_RMS = 0.02;
/** Keep the indicator on this long after the last supra-threshold frame, so
 *  natural gaps between words don't make it flicker. */
export const SPEAKING_HOLD_MS = 600;

/** Fold one frame's RMS into the speaking state: active immediately on a
 *  supra-threshold frame (and extends the hold), otherwise still "speaking"
 *  until the hold window elapses. Pure so it is unit-tested directly. */
export const nextSpeaking = (
  frameRms: number,
  now: number,
  holdUntil: number,
): { speaking: boolean; holdUntil: number } => {
  const active = frameRms >= SPEAKING_RMS;
  const nextHold = active ? now + SPEAKING_HOLD_MS : holdUntil;
  return { speaking: active || now < holdUntil, holdUntil: nextHold };
};

/** Classify a capture-graph failure by DOMException name. macOS never
 *  re-prompts once mic access is denied, so `mic-denied` must send the user to
 *  System Settings — a generic "failed" would leave them retrying into a wall. */
export const voiceErrorOf = (name: string): VoiceError => {
  switch (name) {
    case "NotAllowedError":
    case "SecurityError":
    case "PermissionDeniedError":
      return "mic-denied";
    case "NotFoundError":
    case "DevicesNotFoundError":
    case "OverconstrainedError":
    case "NotReadableError":
      return "mic-missing";
    default:
      return "mic-failed";
  }
};
