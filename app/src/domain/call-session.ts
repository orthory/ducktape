// The browser end of a huddle — everything below the consensus roster, bridged
// to the node's typed `/v1/call/ws` socket (audio + camera video + control on
// one connection). Supersedes the old audio-only `createVoiceSession`.
//
// Audio (unchanged graph, now tagged):
//   capture:  getUserMedia → MediaStreamSource → capture worklet (128-sample
//             quanta → 960-sample / 20 ms frames) → main thread → Float32→Int16
//             LE → `encodeAudioFrame` (0x01 tag) → ws.
//   playback: ws binary → `decodeServerFrame` (0x01) → Int16→Float32 → playback
//             worklet (small ring buffer, underrun = silence) → destination.
// Video (WebCodecs, opt-in via setCamera):
//   encode:   getUserMedia(video) → hidden <video> → requestVideoFrameCallback
//             → VideoFrame → VideoEncoder(vp8) → `encodeCapturedVideo` (0x02) → ws.
//   decode:   ws binary (0x03) → `decodeServerFrame` → per-peer VideoDecoder →
//             draw onto the peer's bound <canvas> tile.
// Control (text json, camelCase tags AND fields — the node's serde enums carry
//   `rename_all` + `rename_all_fields`):
//   out: `{type:"recipients",peers}` on open + roster change; `{type:"beacon",
//        muted,cameraOn}` on every mute/camera toggle; `{type:"keyframeRequest",
//        peer}` when a decoder loses sync.
//   in:  `keyframeRequest` → force the encoder's next frame to be a key;
//        `rateHint` → reconfigure the encoder bitrate; `peerBeacon` → a CallEvent.
//
// Status: `connecting` until the FIRST inbound frame (media or a valid control)
// proves the hub is live — the node emits a mixed audio frame every 20 ms, so
// this lands within a tick even in a solo/muted huddle. A plain-text frame that
// arrives WHILE still connecting is the node's refusal note (no hub, flow busy)
// → `error`; once live, unknown text is ignored (control is the only text a
// healthy session sends). The worklet processors live in
// `public/voice-worklets.js` (same-origin, satisfies the `default-src 'self'`
// CSP) and register `voice-capture` / `voice-playback`.

import {
  encodeAudioFrame,
  encodeCapturedVideo,
  decodeServerFrame,
} from "./call-frames";
import { SAMPLE_RATE, floatToPcm16, pcm16ToFloat } from "./voice-session";
import type { VoiceStatus } from "./voice-session";

export type { VoiceStatus };

/** Tiles beyond this are not rendered — the roster can exceed it, the grid
 *  can't. The node caps concurrent video the same way; this bounds the ui. */
export const MAX_VIDEO_PARTICIPANTS = 8;

/** Same-origin worklet module (see public/voice-worklets.js). */
const WORKLET_URL = "/voice-worklets.js";

/** ~10 s at 30 fps: a safety-net keyframe cadence so a peer that joins or loses
 *  sync mid-stream recovers without waiting on an explicit request round-trip. */
const KEYFRAME_INTERVAL = 300;
/** Starting encoder bitrate (kbps); server `rateHint` moves it. */
const START_BITRATE_KBPS = 800;

export type CallEvent =
  | { kind: "status"; status: VoiceStatus }
  | { kind: "peerBeacon"; peer: string; muted: boolean; cameraOn: boolean; atMs: number };

export interface CallSession {
  /** Open the mic graph and dial the call ws. Idempotent — a second call while
   *  running is ignored (leave then rejoin for a new channel). */
  start(wsUrl: string): void;
  /** Set the fan-out set (peer node hex keys, self excluded). Queued until the
   *  socket is open, then re-sent on every call. */
  setRecipients(hexKeys: string[]): void;
  /** Stop forwarding captured frames (true) without dropping the track; beacons. */
  setMuted(muted: boolean): void;
  /** Enable/disable the camera. Enabling acquires + encodes asynchronously; a
   *  failed acquire leaves the camera off. Beacons on every settled change. */
  setCamera(on: boolean): void;
  /** Bind (or unbind, with null) the canvas a peer's video decodes onto. */
  bindTile(peerHex: string, canvas: HTMLCanvasElement | null): void;
  /** Bind (or unbind, with null) the local camera preview <video>. */
  bindPreview(video: HTMLVideoElement | null): void;
  /** Tear the whole session down: ws, audio graph, camera, every decoder. */
  stop(): void;
}

/** Whether this runtime can do video calls (Chromium companion window on Linux;
 *  WebKitGTK lacks WebCodecs). Audio-only huddles work without it. */
export const supportsVideoCalls = (): boolean =>
  typeof VideoEncoder !== "undefined" &&
  typeof VideoDecoder !== "undefined" &&
  typeof (HTMLVideoElement.prototype as { requestVideoFrameCallback?: unknown })
    .requestVideoFrameCallback === "function" &&
  !!navigator.mediaDevices?.getUserMedia;

/** A hub → webview control frame — camelCase tags and fields, mirroring the
 *  node's `CallServerControl` serde attributes. */
interface ServerControl {
  type?: string;
  peer?: string;
  muted?: boolean;
  cameraOn?: boolean;
  maxKbps?: number;
}

/** Create a call session. `onEvent` receives status transitions and peer
 *  beacons; the caller maps status into the ephemeral voice slice (and treats
 *  'closed' as session end). Nothing here touches the DOM until `start`. */
export const createCallSession = (onEvent: (event: CallEvent) => void): CallSession => {
  // ── audio graph ─────────────────────────────────────────
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

  // ── status / refusal ────────────────────────────────────
  let status: VoiceStatus = "connecting";
  // a refusal note or socket error is a FAILURE end: report 'error' once and
  // suppress the browser's follow-up close, so the caller keeps a visible error
  // state instead of having it wiped by 'closed'.
  let failed = false;
  const setStatus = (next: VoiceStatus) => {
    status = next;
    onEvent({ kind: "status", status: next });
  };
  // the first inbound frame that proves the hub is live promotes us out of
  // 'connecting'; anything after error/closed is left alone.
  const markLive = () => {
    if (status === "connecting") setStatus("live");
  };

  // ── camera / encode ─────────────────────────────────────
  let camStream: MediaStream | null = null;
  let camVideo: HTMLVideoElement | null = null; // hidden frame source
  let encoder: VideoEncoder | null = null;
  let previewEl: HTMLVideoElement | null = null;
  let cameraOn = false;
  let forceKeyframe = true; // first frame, and on server keyframeRequest
  let framesSinceKey = 0;
  let bitrateKbps = START_BITRATE_KBPS; // rateHint moves it

  const configureEncoder = () => {
    encoder?.configure({
      codec: "vp8",
      width: 1280,
      height: 720,
      bitrate: bitrateKbps * 1000,
      framerate: 30,
      latencyMode: "realtime",
    });
    forceKeyframe = true;
  };

  const stopCameraGraph = () => {
    if (encoder) {
      try {
        if (encoder.state !== "closed") encoder.close();
      } catch {
        // already closed — nothing to do.
      }
      encoder = null;
    }
    camStream?.getTracks().forEach((t) => t.stop());
    camStream = null;
    if (camVideo) {
      camVideo.srcObject = null;
      camVideo = null;
    }
    if (previewEl) previewEl.srcObject = null;
    framesSinceKey = 0;
    forceKeyframe = true; // the next enable opens with a keyframe
  };

  const startCamera = async () => {
    const media = await navigator.mediaDevices.getUserMedia({
      video: { width: { ideal: 1280 }, height: { ideal: 720 }, frameRate: { ideal: 30 } },
    });
    if (stopped || !cameraOn) {
      media.getTracks().forEach((t) => t.stop());
      return;
    }
    camStream = media;
    encoder = new VideoEncoder({
      output: (chunk) => {
        if (!socket || socket.readyState !== WebSocket.OPEN) return;
        const data = new Uint8Array(chunk.byteLength);
        chunk.copyTo(data);
        socket.send(
          encodeCapturedVideo(chunk.type === "key", Math.round(chunk.timestamp / 1000), data),
        );
      },
      error: () => setCamera(false), // encoder death = camera off, session lives
    });
    configureEncoder();
    const video = document.createElement("video");
    video.muted = true;
    video.playsInline = true;
    video.srcObject = media;
    await video.play();
    if (stopped || !cameraOn) {
      stopCameraGraph();
      return;
    }
    camVideo = video;
    if (previewEl) previewEl.srcObject = media;
    const pump = () => {
      if (!cameraOn || !camVideo || !encoder || encoder.state === "closed") return;
      // rVFC is the portable frame source (Chromium + WebKit) — no
      // MediaStreamTrackProcessor dependency.
      camVideo.requestVideoFrameCallback((_now, meta) => {
        if (cameraOn && encoder && encoder.state === "configured" && encoder.encodeQueueSize < 2) {
          const frame = new VideoFrame(camVideo!, {
            timestamp: Math.round(meta.mediaTime * 1_000_000),
          });
          const key = forceKeyframe || framesSinceKey >= KEYFRAME_INTERVAL;
          if (key) {
            forceKeyframe = false;
            framesSinceKey = 0;
          } else {
            framesSinceKey += 1;
          }
          encoder.encode(frame, { keyFrame: key });
          frame.close();
        }
        pump();
      });
    };
    pump();
  };

  const sendBeacon = () => {
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify({ type: "beacon", muted, cameraOn }));
    }
  };

  const setCamera = (on: boolean): void => {
    if (on === cameraOn) return; // idempotent
    if (on) {
      cameraOn = true;
      startCamera()
        .then(() => {
          if (cameraOn) sendBeacon();
        })
        .catch(() => {
          // acquire/encode setup failed: stay off, surface nothing fatal.
          cameraOn = false;
          stopCameraGraph();
        });
    } else {
      cameraOn = false;
      stopCameraGraph();
      sendBeacon();
    }
  };

  // ── per-peer decode / tiles ─────────────────────────────
  interface PeerPipe {
    decoder: VideoDecoder;
    canvas: HTMLCanvasElement | null;
    awaitingKey: boolean;
    lastRequestMs: number;
  }
  const pipes = new Map<string, PeerPipe>();
  const tileBindings = new Map<string, HTMLCanvasElement>();

  const requestPeerKeyframe = (peerHex: string, pipe: PeerPipe) => {
    const now = Date.now();
    if (now - pipe.lastRequestMs < 1000) return; // ≥1 s, mirroring the hub
    pipe.lastRequestMs = now;
    if (socket?.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify({ type: "keyframeRequest", peer: peerHex }));
    }
  };

  const pipeFor = (peerHex: string): PeerPipe => {
    const existing = pipes.get(peerHex);
    if (existing) return existing;
    const created: PeerPipe = {
      canvas: tileBindings.get(peerHex) ?? null,
      awaitingKey: true,
      lastRequestMs: 0,
      decoder: new VideoDecoder({
        output: (frame) => {
          const canvas = created.canvas;
          if (canvas) {
            canvas.width = frame.displayWidth;
            canvas.height = frame.displayHeight;
            canvas.getContext("2d")?.drawImage(frame, 0, 0);
          }
          frame.close();
        },
        error: () => {
          created.awaitingKey = true;
          requestPeerKeyframe(peerHex, created);
        },
      }),
    };
    created.decoder.configure({ codec: "vp8" });
    pipes.set(peerHex, created);
    return created;
  };

  const onPeerVideo = (peer: string, keyframe: boolean, tsMs: number, data: Uint8Array) => {
    const pipe = pipeFor(peer);
    if (pipe.awaitingKey && !keyframe) {
      requestPeerKeyframe(peer, pipe); // deltas are useless until a sync point
      return;
    }
    pipe.awaitingKey = false;
    pipe.decoder.decode(
      new EncodedVideoChunk({
        type: keyframe ? "key" : "delta",
        timestamp: tsMs * 1000,
        data,
      }),
    );
  };

  // ── control ─────────────────────────────────────────────
  const parseControl = (text: string): ServerControl | null => {
    let parsed: unknown;
    try {
      parsed = JSON.parse(text);
    } catch {
      return null;
    }
    if (!parsed || typeof parsed !== "object") return null;
    const msg = parsed as ServerControl;
    if (msg.type === "keyframeRequest" || msg.type === "peerBeacon" || msg.type === "rateHint") {
      return msg;
    }
    return null;
  };

  const applyControl = (msg: ServerControl) => {
    switch (msg.type) {
      case "keyframeRequest":
        // a peer lost sync with US — the next encoded frame must be a key.
        forceKeyframe = true;
        break;
      case "rateHint":
        if (typeof msg.maxKbps === "number" && msg.maxKbps > 0) {
          bitrateKbps = msg.maxKbps;
          if (encoder && encoder.state === "configured") configureEncoder();
        }
        break;
      case "peerBeacon":
        if (typeof msg.peer === "string") {
          onEvent({
            kind: "peerBeacon",
            peer: msg.peer.toLowerCase(),
            muted: !!msg.muted,
            cameraOn: !!msg.cameraOn,
            atMs: Date.now(),
          });
        }
        break;
    }
  };

  const handleControlText = (text: string) => {
    if (stopped) return;
    const control = parseControl(text);
    if (control) {
      markLive(); // a valid control frame means the hub is relaying — we're live
      applyControl(control);
      return;
    }
    // not a known control frame: the node's refusal note (one plain-text reason,
    // then close). Only a refusal while still connecting; once live, ignore.
    if (status !== "live") {
      failed = true;
      setStatus("error");
    }
  };

  // ── socket ──────────────────────────────────────────────
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
      // NB: no 'live' here — that waits on the first inbound frame so a refusal
      // (which arrives just after open) is still caught while 'connecting'.
      if (pendingRecipients) {
        sendRecipients(pendingRecipients);
        pendingRecipients = null;
      }
    };
    ws.onmessage = (event) => {
      if (typeof event.data === "string") {
        handleControlText(event.data);
        return;
      }
      if (stopped) return;
      const frame = decodeServerFrame(event.data as ArrayBuffer);
      if (!frame) return;
      markLive();
      if (frame.kind === "audio") {
        if (playback) {
          const samples = pcm16ToFloat(frame.pcm);
          playback.port.postMessage(samples, [samples.buffer]);
        }
      } else {
        onPeerVideo(frame.peer, frame.keyframe, frame.tsMs, frame.data);
      }
    };
    ws.onclose = () => {
      if (stopped || failed) return;
      setStatus("closed");
    };
    ws.onerror = () => {
      if (stopped) return;
      failed = true;
      setStatus("error");
    };
  };

  // ── lifecycle ───────────────────────────────────────────
  const start = (wsUrl: string) => {
    if (started) return;
    started = true;
    setStatus("connecting");
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

        // capture: mic → 20 ms Float32 frames on the main thread → tagged ws.
        source = context.createMediaStreamSource(media);
        const cap = new AudioWorkletNode(context, "voice-capture", {
          numberOfInputs: 1,
          numberOfOutputs: 0,
        });
        cap.port.onmessage = (event) => {
          if (muted || !socket || socket.readyState !== WebSocket.OPEN) return;
          socket.send(encodeAudioFrame(floatToPcm16(event.data as Float32Array)));
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
        if (!stopped) setStatus("error");
      });
  };

  const setRecipients = (hexKeys: string[]) => {
    sendRecipients(hexKeys);
  };

  const setMuted = (next: boolean) => {
    muted = next; // the capture callback checks this to stop forwarding
    sendBeacon();
  };

  const bindTile = (peerHex: string, canvas: HTMLCanvasElement | null): void => {
    const key = peerHex.toLowerCase();
    if (canvas) tileBindings.set(key, canvas);
    else tileBindings.delete(key);
    const pipe = pipes.get(key);
    if (pipe) pipe.canvas = canvas;
  };

  const bindPreview = (video: HTMLVideoElement | null): void => {
    previewEl = video;
    if (video && camStream) video.srcObject = camStream;
  };

  const stop = () => {
    if (stopped) return;
    stopped = true;
    cameraOn = false;
    if (socket) {
      // drop handlers first so our own close doesn't fire onEvent('closed').
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

    stopCameraGraph();
    for (const pipe of pipes.values()) {
      try {
        if (pipe.decoder.state !== "closed") pipe.decoder.close();
      } catch {
        // already closed.
      }
    }
    pipes.clear();
    tileBindings.clear();
  };

  return { start, setRecipients, setMuted, setCamera, bindTile, bindPreview, stop };
};
