// The browser audio half of a huddle — everything below the consensus roster.
//
// A session owns one microphone capture graph and one playback graph over a
// single 48 kHz AudioContext, bridged to the node's voice websocket:
//   capture:  getUserMedia → MediaStreamSource → capture worklet (accumulates
//             128-sample render quanta into 960-sample / 20 ms frames) → main
//             thread → Float32→Int16 LE → binary ws frame.
//   playback: binary ws frame → Int16→Float32 → playback worklet (small ring
//             buffer, underrun = silence) → destination.
// A text `recipients` frame (the fan-out set, our own node key excluded) is
// pushed on open and on every roster change. Mute stops FORWARDING captured
// frames but keeps the track live, so unmute is instant. Socket close = session
// end (the server replaces a prior session or refuses with a 503 + one text
// frame). The pure conversion + recipient helpers are exported for tests; the
// audio graph itself needs a real browser and is only touched at runtime.
//
// The worklet processor code lives in `public/voice-worklets.js` (served
// same-origin, so it satisfies the app's `default-src 'self'` CSP — a blob: url
// would need the CSP widened) and registers `voice-capture` / `voice-playback`.

import { keyHex } from "./chat-client";
import type { HuddleMember } from "./chat-client";

/** 48 kHz mono — the daemon's fixed voice format. */
export const SAMPLE_RATE = 48_000;
/** 20 ms at 48 kHz = 960 samples per frame (1920 bytes Int16). */
export const FRAME_SAMPLES = 960;

/** Same-origin worklet module (see public/voice-worklets.js). */
const WORKLET_URL = "/voice-worklets.js";

export type VoiceStatus = "connecting" | "live" | "error" | "closed";

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
 *  own node key (status.publicKey). Case-insensitive on the self key. */
export const huddleRecipients = (
  huddle: HuddleMember[],
  selfNodeHex: string,
): string[] => {
  const self = selfNodeHex.toLowerCase();
  return huddle
    .map((m) => keyHex(m.node))
    .filter((hex) => hex !== self);
};

// ── The session ─────────────────────────────────────────

export interface VoiceSession {
  /** Open the mic graph and dial the voice ws. Idempotent — a second call while
   *  running is ignored (leave then rejoin for a new channel). */
  start(wsUrl: string): void;
  /** Set the fan-out set (peer node hex keys, self excluded). Queued until the
   *  socket is open, then re-sent on every call. */
  setRecipients(hexKeys: string[]): void;
  /** Stop forwarding captured frames (true) without dropping the track. */
  setMuted(muted: boolean): void;
  /** Tear the whole session down: ws, graph, context, mic track. Idempotent. */
  stop(): void;
}

/** Create a voice session. `onStatus` receives lifecycle transitions; the caller
 *  maps them into the ephemeral voice slice (and treats 'closed' as session
 *  end). Nothing here touches the DOM until `start`. */
export const createVoiceSession = (
  onStatus: (status: VoiceStatus) => void,
): VoiceSession => {
  let ctx: AudioContext | null = null;
  let stream: MediaStream | null = null;
  let socket: WebSocket | null = null;
  let source: MediaStreamAudioSourceNode | null = null;
  let capture: AudioWorkletNode | null = null;
  let playback: AudioWorkletNode | null = null;
  let muted = false;
  let started = false;
  let stopped = false;
  // recipients requested before the socket was open — flushed on open.
  let pendingRecipients: string[] | null = null;

  const sendRecipients = (peers: string[]) => {
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify({ type: "recipients", peers }));
    } else {
      pendingRecipients = peers;
    }
  };

  const openSocket = (wsUrl: string) => {
    const ws = new WebSocket(wsUrl);
    ws.binaryType = "arraybuffer";
    socket = ws;
    ws.onopen = () => {
      if (stopped) return;
      onStatus("live");
      if (pendingRecipients) {
        sendRecipients(pendingRecipients);
        pendingRecipients = null;
      }
    };
    ws.onmessage = (event) => {
      // a text frame is only ever the server's refusal note before it closes —
      // the close handler ends the session, so nothing to do here.
      if (typeof event.data === "string") return;
      if (stopped || !playback) return;
      const frame = pcm16ToFloat(new Int16Array(event.data as ArrayBuffer));
      playback.port.postMessage(frame, [frame.buffer]);
    };
    ws.onclose = () => {
      if (stopped) return;
      onStatus("closed");
    };
    ws.onerror = () => {
      if (stopped) return;
      onStatus("error");
    };
  };

  const start = (wsUrl: string) => {
    if (started) return;
    started = true;
    onStatus("connecting");
    Promise.resolve()
      .then(() =>
        navigator.mediaDevices.getUserMedia({
          audio: { echoCancellation: true, noiseSuppression: true, channelCount: 1 },
        }),
      )
      .then(async (media) => {
        if (stopped) {
          media.getTracks().forEach((t) => t.stop());
          return;
        }
        stream = media;
        const context = new AudioContext({ sampleRate: SAMPLE_RATE });
        ctx = context;
        await context.audioWorklet.addModule(WORKLET_URL);
        if (stopped) return;

        // capture: mic → 20 ms Float32 frames on the main thread → binary ws.
        source = context.createMediaStreamSource(media);
        const cap = new AudioWorkletNode(context, "voice-capture", {
          numberOfInputs: 1,
          numberOfOutputs: 0,
        });
        cap.port.onmessage = (event) => {
          if (muted || !socket || socket.readyState !== WebSocket.OPEN) return;
          const pcm = floatToPcm16(event.data as Float32Array);
          socket.send(pcm.buffer);
        };
        source.connect(cap);
        capture = cap;

        // playback: mixed frames from the ws → ring buffer → speakers.
        const play = new AudioWorkletNode(context, "voice-playback", {
          numberOfInputs: 0,
          numberOfOutputs: 1,
          outputChannelCount: [1],
        });
        play.connect(context.destination);
        playback = play;

        openSocket(wsUrl);
      })
      .catch(() => {
        if (!stopped) onStatus("error");
      });
  };

  const setRecipients = (hexKeys: string[]) => {
    sendRecipients(hexKeys);
  };

  const setMuted = (next: boolean) => {
    muted = next;
  };

  const stop = () => {
    if (stopped) return;
    stopped = true;
    if (socket) {
      // drop handlers first so our own close doesn't fire onStatus('closed').
      socket.onopen = null;
      socket.onmessage = null;
      socket.onclose = null;
      socket.onerror = null;
      try {
        socket.close();
      } catch {
        // already closing/closed — nothing to do.
      }
      socket = null;
    }
    try {
      source?.disconnect();
      capture?.disconnect();
      playback?.disconnect();
    } catch {
      // graph already torn down.
    }
    source = capture = playback = null;
    stream?.getTracks().forEach((t) => t.stop());
    stream = null;
    void ctx?.close().catch(() => undefined);
    ctx = null;
  };

  return { start, setRecipients, setMuted, stop };
};
