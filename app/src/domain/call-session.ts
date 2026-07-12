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
import { SAMPLE_RATE, floatToPcm16, nextSpeaking, pcm16ToFloat, rms, voiceErrorOf } from "./voice-session";
import type { VoiceStatus, VoiceError } from "./voice-session";
// The control-frame shapes are generated from the node's `CallClientControl`/
// `CallServerControl` serde enums (`make stream-types`) — a hand-rolled
// mirror here would drift the moment either side adds a variant.
import type { CallClientControl, CallServerControl } from "./stream.gen";

export type { VoiceStatus, VoiceError };

/** The tile grid renders at most this many peers (roster order, plus our own
 *  self preview); a larger huddle still works, its extra tiles just aren't
 *  drawn. This is a UI bound ONLY — the node caps nothing, so any workspace
 *  member whose media reaches us is still decoded regardless of this number. */
export const MAX_VIDEO_PARTICIPANTS = 8;

/** Same-origin worklet module (see public/voice-worklets.js). */
const WORKLET_URL = "/voice-worklets.js";

/** ~10 s at 30 fps: a safety-net keyframe cadence so a peer that joins or loses
 *  sync mid-stream recovers without waiting on an explicit request round-trip. */
const KEYFRAME_INTERVAL = 300;
/** Starting encoder bitrate (kbps); server `rateHint` moves it. */
const START_BITRATE_KBPS = 800;
/** Encoder bitrate envelope (kbps) — the ends of the node's RATE_LADDER_KBPS
 *  [1200, 800, 500, 300]. A `rateHint` outside this is hostile-or-broken, so we
 *  clamp before reconfiguring: a tiny hint would freeze our video, a huge one
 *  would fail the encoder's configure and silently kill the camera. */
const MIN_BITRATE_KBPS = 300;
const MAX_BITRATE_KBPS = 1200;
/** How long an OPEN socket may sit with no inbound frame before the session is
 *  declared failed. The hub emits a mixed frame every 20 ms once it serves the
 *  session, so a silent open socket means the hub can't (media planes not up,
 *  request queued forever) — without this bound the dock shows "connecting…"
 *  indefinitely. Timed from socket open, NOT from start(): getUserMedia's
 *  permission prompt can legitimately hold `start` open for minutes. */
const CONNECT_TIMEOUT_MS = 12_000;
/** RMS that reads as a "full" mic meter (0..1). Ordinary speech sits ~0.05–0.15
 *  (SPEAKING_RMS is 0.02), so this scale gives a lively, reassuring self-check
 *  bar without pinning to the top on every syllable. */
const LEVEL_FULL_RMS = 0.25;

export type CallEvent =
  // `note` is the node's own refusal sentence (the one text frame it sends
  // before closing — "no live call hub", "the overlay is not up", …). It says
  // WHY in terms only the node knows, so it is carried to the ui verbatim
  // rather than collapsed into the generic connection-failed copy.
  | { kind: "status"; status: VoiceStatus; error?: VoiceError; note?: string }
  | { kind: "peerBeacon"; peer: string; muted: boolean; cameraOn: boolean; sharing: boolean; atMs: number }
  // Our own mic went above/below the speaking threshold — drives the self
  // speaking ring and the "you're muted while talking" banner. Emitted only on a
  // change (mute keeps capturing, so this fires even while muted).
  | { kind: "selfSpeaking"; speaking: boolean }
  // Our own mic input level, 0..1 (rms scaled), throttled to ~12 Hz. Drives the
  // solo self-check meter so a lone user can SEE the mic responds — emitted even
  // while muted (capture runs regardless), so the check needs no hot-mic moment.
  | { kind: "selfLevel"; level: number }
  // Our own video-lane state SETTLED — the authoritative source for the slice's
  // cameraOn/sharing (fires on toggle, on a failed acquire, on encoder death, and
  // when the browser's own "Stop sharing" ends a screen share).
  | { kind: "selfVideo"; cameraOn: boolean; sharing: boolean }
  // A camera/screen acquire FAILED after the user asked for it — the lane stays
  // off (selfVideo already said so); this carries the WHY so the surface can say
  // something instead of a button that silently snaps back.
  | { kind: "mediaNote"; note: "camera-failed" | "screen-failed" };

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
   *  failed acquire leaves the camera off. Beacons on every settled change.
   *  Turning the camera on while screen-sharing swaps the lane. */
  setCamera(on: boolean): void;
  /** Enable/disable screen share on the SAME video lane (camera XOR screen).
   *  Enabling acquires getDisplayMedia; a denial/cancel leaves it off. Beacons
   *  `sharing` so peers letterbox + label the tile. */
  setScreenShare(on: boolean): void;
  /** Select input/output devices (undefined = system default). Applied live: the
   *  mic swaps into the running capture graph, a live camera re-acquires, and the
   *  speaker routes via setSinkId where supported. Also read at the next acquire. */
  setDevices(prefs: { micId?: string; cameraId?: string; speakerId?: string }): void;
  /** Bind (or unbind, with null) the canvas a peer's video decodes onto. */
  bindTile(peerHex: string, canvas: HTMLCanvasElement | null): void;
  /** Bind (or unbind, with null) the local camera preview <video>. */
  bindPreview(video: HTMLVideoElement | null): void;
  /** Tear the whole session down: ws, audio graph, camera, every decoder. */
  stop(): void;
}
// Runtime video capability now lives in domain/video-capability.ts (a REAL codec
// probe via isConfigSupported — WebKitGTK exposes the WebCodecs API but may not
// register a vp8 encoder, and encode/decode capability can diverge).

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
  // Chosen input/output devices (undefined = system default). Applied at acquire
  // time (start / startVideo) and swapped live by setDevices.
  let micId: string | undefined;
  let cameraId: string | undefined;
  let speakerId: string | undefined;
  // Self active-speaker detection off the capture frames (runs even while muted).
  let speaking = false;
  let speakingHoldUntil = 0;
  // Self mic-level meter (solo self-check): throttle emits and only push when the
  // displayed level actually moves, so a 50 Hz capture doesn't spam React.
  let lastLevelAtMs = 0;
  let lastLevel = -1;
  // recipients requested before the socket was open — flushed on open.
  let pendingRecipients: string[] | null = null;

  // ── status / refusal ────────────────────────────────────
  let status: VoiceStatus = "connecting";
  // a refusal note or socket error is a FAILURE end: report 'error' once and
  // suppress the browser's follow-up close, so the caller keeps a visible error
  // state instead of having it wiped by 'closed'.
  let failed = false;
  // armed when the socket opens; a session still 'connecting' when it fires is
  // stuck against a hub that will never serve it (see CONNECT_TIMEOUT_MS).
  let connectTimer: ReturnType<typeof setTimeout> | null = null;
  const clearConnectTimer = () => {
    if (connectTimer !== null) {
      clearTimeout(connectTimer);
      connectTimer = null;
    }
  };
  const setStatus = (next: VoiceStatus, error?: VoiceError, note?: string) => {
    status = next;
    onEvent({ kind: "status", status: next, error, note });
  };
  // the first inbound frame that proves the hub is live promotes us out of
  // 'connecting'; anything after error/closed is left alone.
  const markLive = () => {
    clearConnectTimer();
    if (status === "connecting") setStatus("live");
  };

  // ── video lane (camera XOR screen) / encode ──────────────
  // One VP8 lane, sourced from EITHER the camera or a screen share — the two are
  // mutually exclusive (a mode-swap), beaconed as `cameraOn` / `sharing`.
  let camStream: MediaStream | null = null;
  let camVideo: HTMLVideoElement | null = null; // hidden frame source
  let encoder: VideoEncoder | null = null;
  let previewEl: HTMLVideoElement | null = null;
  let cameraOn = false;
  let sharing = false; // the lane is a screen share rather than the camera
  // A screen share that displaced a live camera puts it back when it ends (the
  // user's "Stop sharing", a cancelled picker, our own toggle) — sharing a
  // screen mid-video-call must not end your video. Cleared by any explicit
  // camera toggle: the user's own act supersedes the remembered state.
  let resumeCameraAfterShare = false;
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

  const startVideo = async (screen: boolean) => {
    // Guard on the mode WE were asked to start, not "any video" — a swap that
    // flipped the lane to the other mode while our getUserMedia/getDisplayMedia
    // was in flight must make this stale acquire bail (else it overwrites the
    // live stream/encoder, leaking a track and mislabeling the beacon).
    const wanted = () => (screen ? sharing : cameraOn);
    const media = screen
      ? await navigator.mediaDevices.getDisplayMedia({ video: { frameRate: { ideal: 30 } } })
      : await navigator.mediaDevices.getUserMedia({
          video: {
            width: { ideal: 1280 },
            height: { ideal: 720 },
            frameRate: { ideal: 30 },
            // `ideal` (bare string) so an absent camera falls back, not throws.
            ...(cameraId ? { deviceId: cameraId } : {}),
          },
        });
    if (stopped || !wanted()) {
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
      error: () => stopVideoLane(), // encoder death = lane off, session lives
    });
    configureEncoder();
    const video = document.createElement("video");
    video.muted = true;
    video.playsInline = true;
    video.srcObject = media;
    await video.play();
    if (stopped || !wanted()) {
      stopCameraGraph();
      return;
    }
    camVideo = video;
    if (previewEl) previewEl.srcObject = media;
    // A screen share can be ended from the browser's OWN "Stop sharing" UI —
    // mirror that back into our lane state so the beacon + button stay honest.
    if (screen) {
      media.getVideoTracks()[0]?.addEventListener("ended", () => setScreenShare(false));
    }
    const pump = () => {
      if (!wanted() || !camVideo || !encoder || encoder.state === "closed") return;
      // rVFC is the portable frame source (Chromium + WebKit) — no
      // MediaStreamTrackProcessor dependency.
      camVideo.requestVideoFrameCallback((_now, meta) => {
        if (wanted() && encoder && encoder.state === "configured" && encoder.encodeQueueSize < 2) {
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

  /** Force the whole video lane off (encoder death, or a swap failure). */
  const stopVideoLane = (): void => {
    cameraOn = false;
    sharing = false;
    // A force-kill ends any pending share→camera restore too: a stale flag
    // would surprise-enable the webcam at the end of a LATER, unrelated share.
    resumeCameraAfterShare = false;
    stopCameraGraph();
    sendBeacon();
  };

  const sendBeacon = () => {
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(
        JSON.stringify({ type: "beacon", muted, cameraOn, sharing } satisfies CallClientControl),
      );
    }
    // Mirror the settled lane state to the store — authoritative for paths the
    // store can't see (failed acquire, encoder death, native "Stop sharing").
    onEvent({ kind: "selfVideo", cameraOn, sharing });
  };

  const setCamera = (on: boolean): void => {
    // The user's explicit camera choice supersedes any share-displaced state.
    resumeCameraAfterShare = false;
    if (on === cameraOn) return; // idempotent
    if (on) {
      if (sharing) {
        // swap the lane from screen → camera.
        sharing = false;
        stopCameraGraph();
      }
      cameraOn = true;
      startVideo(false)
        .then(() => {
          if (cameraOn) sendBeacon();
        })
        .catch(() => {
          // acquire/encode setup failed: the lane stays off, the session lives —
          // but say so, or the button just snaps back with no explanation.
          cameraOn = false;
          stopCameraGraph();
          sendBeacon();
          onEvent({ kind: "mediaNote", note: "camera-failed" });
        });
    } else {
      cameraOn = false;
      stopCameraGraph();
      sendBeacon();
    }
  };

  /** End-of-share hook: put a camera the share displaced back on. */
  const maybeResumeCamera = (): void => {
    if (!resumeCameraAfterShare || stopped) return;
    resumeCameraAfterShare = false;
    setCamera(true);
  };

  const setScreenShare = (on: boolean): void => {
    if (on === sharing) return; // idempotent
    if (on) {
      if (cameraOn) {
        // swap the lane from camera → screen, and remember to swap back.
        resumeCameraAfterShare = true;
        cameraOn = false;
        stopCameraGraph();
      }
      sharing = true;
      startVideo(true)
        .then(() => {
          if (sharing) sendBeacon();
        })
        .catch(() => {
          // getDisplayMedia denied / cancelled: the share stays off — restore a
          // camera it displaced (cancelling a share must not end your video),
          // and say what happened.
          sharing = false;
          stopCameraGraph();
          sendBeacon();
          onEvent({ kind: "mediaNote", note: "screen-failed" });
          maybeResumeCamera();
        });
    } else {
      sharing = false;
      stopCameraGraph();
      sendBeacon();
      maybeResumeCamera();
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
      socket.send(
        JSON.stringify({ type: "keyframeRequest", peer: peerHex } satisfies CallClientControl),
      );
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
          // A fatal decoder error closes the decoder; decoding into it again
          // would throw uncaught in ws.onmessage on every subsequent frame.
          // Drop the pipe so pipeFor rebuilds a fresh decoder on the next frame
          // (which then hits the awaitingKey gate and requests a keyframe).
          try {
            if (created.decoder.state !== "closed") created.decoder.close();
          } catch {
            // already closed — nothing to do.
          }
          pipes.delete(peerHex);
          requestPeerKeyframe(peerHex, created); // ask the sender for a sync point now
        },
      }),
    };
    created.decoder.configure({ codec: "vp8" });
    pipes.set(peerHex, created);
    return created;
  };

  const onPeerVideo = (peer: string, keyframe: boolean, tsMs: number, data: Uint8Array) => {
    // No decode support on this runtime → ignore peer video rather than throw
    // constructing a VideoDecoder on every frame (a decoder-less WKWebView).
    if (typeof VideoDecoder === "undefined") return;
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
  const parseControl = (text: string): CallServerControl | null => {
    let parsed: unknown;
    try {
      parsed = JSON.parse(text);
    } catch {
      return null;
    }
    if (!parsed || typeof parsed !== "object") return null;
    const msg = parsed as CallServerControl;
    if (msg.type === "keyframeRequest" || msg.type === "peerBeacon" || msg.type === "rateHint") {
      return msg;
    }
    return null;
  };

  const applyControl = (msg: CallServerControl) => {
    switch (msg.type) {
      case "keyframeRequest":
        // a peer lost sync with US — the next encoded frame must be a key.
        forceKeyframe = true;
        break;
      case "rateHint":
        if (typeof msg.maxKbps === "number" && msg.maxKbps > 0) {
          bitrateKbps = Math.min(MAX_BITRATE_KBPS, Math.max(MIN_BITRATE_KBPS, msg.maxKbps));
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
            sharing: !!msg.sharing,
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
    // The reason rides to the ui — it is the only thing that distinguishes "this
    // node runs no call hub" from "the overlay never came up" from a dead socket.
    if (status !== "live") {
      failed = true;
      setStatus("error", "connection", text.trim() || undefined);
    }
  };

  // ── socket ──────────────────────────────────────────────
  const sendRecipients = (peers: string[]) => {
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify({ type: "recipients", peers } satisfies CallClientControl));
    } else {
      pendingRecipients = peers;
    }
  };

  const openSocket = (wsUrl: string) => {
    const ws = new WebSocket(wsUrl);
    ws.binaryType = "arraybuffer";
    socket = ws;
    // A hub that upgrades the socket but never serves the session (media planes
    // down, request parked) would otherwise hold 'connecting' forever.
    connectTimer = setTimeout(() => {
      connectTimer = null;
      if (stopped || status !== "connecting") return;
      failed = true;
      setStatus("error", "connection");
      try {
        ws.close();
      } catch {
        // already closing — the error status above is what matters.
      }
    }, CONNECT_TIMEOUT_MS);
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
      clearConnectTimer();
      if (stopped || failed) return;
      setStatus("closed");
    };
    ws.onerror = () => {
      clearConnectTimer();
      if (stopped) return;
      failed = true;
      setStatus("error", "connection");
    };
  };

  // ── lifecycle ───────────────────────────────────────────
  const audioConstraints = (): MediaTrackConstraints => ({
    echoCancellation: true,
    noiseSuppression: true,
    channelCount: 1,
    // `ideal` (bare string), NOT { exact } — a persisted-but-now-absent device
    // must fall back to the system default, not OverconstrainedError the join.
    ...(micId ? { deviceId: micId } : {}),
  });

  /** Route playout to the chosen speaker — AudioContext.setSinkId is Chromium
   *  only; WebKitGTK / macOS WKWebView lack it, so this is a no-op there and the
   *  speaker picker hides itself (media-devices.canSelectSpeaker). */
  const applySpeaker = (): void => {
    const sink = ctx as (AudioContext & { setSinkId?: (id: string) => Promise<void> }) | null;
    if (speakerId && typeof sink?.setSinkId === "function") void sink.setSinkId(speakerId).catch(() => {});
  };

  /** Re-acquire the mic on a new device, swapping it into the LIVE capture graph
   *  without rebuilding the worklet (playout + encode untouched). */
  const swapMic = async (): Promise<void> => {
    if (!ctx || !capture) return; // not live yet — start() picks up micId at acquire
    try {
      const media = await navigator.mediaDevices.getUserMedia({ audio: audioConstraints() });
      if (stopped) {
        media.getTracks().forEach((t) => t.stop());
        return;
      }
      const next = ctx.createMediaStreamSource(media);
      source?.disconnect();
      stream?.getTracks().forEach((t) => t.stop());
      stream = media;
      source = next;
      source.connect(capture);
    } catch {
      // keep the current mic on a failed acquire.
    }
  };

  const setDevices = (prefs: { micId?: string; cameraId?: string; speakerId?: string }): void => {
    const micChanged = prefs.micId !== micId;
    const cameraChanged = prefs.cameraId !== cameraId;
    micId = prefs.micId;
    cameraId = prefs.cameraId;
    speakerId = prefs.speakerId;
    applySpeaker();
    if (micChanged) void swapMic();
    // Re-acquire the live camera on the new device (a screen share is
    // unaffected). A bad/exact deviceId can OverconstrainedError — turn the lane
    // off cleanly rather than leak an unhandled rejection + a stuck camera flag.
    if (cameraChanged && cameraOn) {
      stopCameraGraph();
      void startVideo(false).catch(() => stopVideoLane());
    }
  };

  const start = (wsUrl: string) => {
    if (started) return;
    started = true;
    setStatus("connecting");
    Promise.resolve()
      .then(() =>
        navigator.mediaDevices.getUserMedia({ audio: audioConstraints() }),
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
          const frame = event.data as Float32Array;
          // Speaking detection first, so it fires even when muted (below).
          const now = Date.now();
          const amplitude = rms(frame);
          const detected = nextSpeaking(amplitude, now, speakingHoldUntil);
          speakingHoldUntil = detected.holdUntil;
          if (detected.speaking !== speaking) {
            speaking = detected.speaking;
            onEvent({ kind: "selfSpeaking", speaking });
          }
          // Meter level: rms scaled so ordinary speech fills a visible chunk.
          // ~12 Hz + a 0.03 dead-band keeps it smooth without flooding React.
          const level = Math.min(1, amplitude / LEVEL_FULL_RMS);
          if (now - lastLevelAtMs > 80 && Math.abs(level - lastLevel) > 0.03) {
            lastLevelAtMs = now;
            lastLevel = level;
            onEvent({ kind: "selfLevel", level });
          }
          if (muted || !socket || socket.readyState !== WebSocket.OPEN) return;
          socket.send(encodeAudioFrame(floatToPcm16(frame)));
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
        applySpeaker(); // route to the chosen speaker if one is set + supported

        openSocket(wsUrl);
      })
      .catch((err: unknown) => {
        // classify the capture-graph failure (getUserMedia / worklet) so the
        // dock can say WHY — mic denied vs missing vs a generic setup failure.
        if (!stopped) setStatus("error", voiceErrorOf(err instanceof DOMException ? err.name : ""));
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
    sharing = false;
    resumeCameraAfterShare = false;
    clearConnectTimer();
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

  return { start, setRecipients, setMuted, setCamera, setScreenShare, setDevices, bindTile, bindPreview, stop };
};
