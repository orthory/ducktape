// L3 browser call client harness — drives the app's REAL call client
// (app/src/domain/call-session.ts createCallSession) inside a headless Chromium
// page with FAKE mic+camera, against the live callbed node this page names.
//
// What is REAL app code here: the ENTIRE media path is call-session.ts —
//   * mic getUserMedia + AudioContext + the same-origin voice-worklets capture
//     graph -> Int16 PCM -> encodeAudioFrame (0x01) -> ws
//   * camera getUserMedia(video) -> requestVideoFrameCallback -> WebCodecs
//     VideoEncoder(vp8) -> encodeCapturedVideo (0x02) -> ws
//   * inbound 0x03 peer video -> per-peer WebCodecs VideoDecoder -> drawImage
//     onto the bound <canvas> tile
//   * inbound 0x01 mixed playout -> decodeServerFrame -> playback worklet
// This harness adds ONLY observation, no reimplementation:
//   * a passive addEventListener("message") tap on the SAME WebSocket the real
//     client opens (coexists with the client's own .onmessage) to measure the
//     mixed-audio RMS and count peer-video frames the real client received.
//     call-session exposes no peer-audio hook, so this is the only way to read
//     what it played without patching app source — the frames measured are the
//     exact bytes the real client decoded/played.
//   * canvas pixel readback of the real VideoDecoder's output.

import { createCallSession } from "../../../app/src/domain/call-session";

interface Tap {
  audioFrames: number;
  audioRmsSum: number;
  audioRmsMax: number;
  audioNonSilent: number; // frames with RMS above the silence floor
  videoFrames: number; // 0x03 peer-video frames received
  videoKeyframes: number;
  videoBytes: number;
  firstAudioMs: number | null;
  firstVideoMs: number | null;
}

const tap: Tap = {
  audioFrames: 0,
  audioRmsSum: 0,
  audioRmsMax: 0,
  audioNonSilent: 0,
  videoFrames: 0,
  videoKeyframes: 0,
  videoBytes: 0,
  firstAudioMs: null,
  firstVideoMs: null,
};
const t0 = Date.now();

// Wrap the global WebSocket so we passively observe every inbound binary frame
// the real client receives on the socket IT opens. We only read; the client's
// own .onmessage still drives decode/playback/draw.
const RealWebSocket = WebSocket;
class TappedWebSocket extends RealWebSocket {
  constructor(url: string | URL, protocols?: string | string[]) {
    super(url, protocols);
    this.binaryType = "arraybuffer";
    this.addEventListener("message", (ev: MessageEvent) => {
      if (typeof ev.data === "string") return;
      const ab = ev.data as ArrayBuffer;
      const u = new Uint8Array(ab);
      if (u.length < 1) return;
      if (u[0] === 0x01 && u.length > 1) {
        const dv = new DataView(ab, 1);
        const n = (u.byteLength - 1) >> 1;
        let sum = 0;
        for (let i = 0; i < n; i++) {
          const s = dv.getInt16(i * 2, true);
          sum += s * s;
        }
        const rms = Math.sqrt(sum / n);
        tap.audioFrames++;
        tap.audioRmsSum += rms;
        if (rms > tap.audioRmsMax) tap.audioRmsMax = rms;
        if (rms > 50) {
          tap.audioNonSilent++;
          if (tap.firstAudioMs === null) tap.firstAudioMs = Date.now() - t0;
        }
      } else if (u[0] === 0x03 && u.byteLength > 38) {
        tap.videoFrames++;
        tap.videoBytes += u.byteLength - 38;
        if (u[1] & 0x01) tap.videoKeyframes++;
        if (tap.firstVideoMs === null) tap.firstVideoMs = Date.now() - t0;
      }
    });
  }
}
(globalThis as unknown as { WebSocket: typeof WebSocket }).WebSocket = TappedWebSocket;

const events: unknown[] = [];
let session: ReturnType<typeof createCallSession> | null = null;

function log(msg: string) {
  // Surfaced to the CDP driver via Runtime.consoleAPICalled.
  console.log("[harness] " + msg);
}

function run() {
  const params = new URLSearchParams(location.search);
  const node = params.get("node") || "8080";
  const peer = (params.get("peer") || "").toLowerCase();
  const channel = params.get("channel") || "general";
  const wsUrl = `ws://127.0.0.1:${node}/v1/call/ws?channel=${channel}`;
  log(`node=${node} peer=${peer.slice(0, 12)}… channel=${channel}`);

  const canvas = document.getElementById("tile") as HTMLCanvasElement;
  const preview = document.getElementById("preview") as HTMLVideoElement;

  session = createCallSession((ev) => {
    events.push(ev);
    log("event " + JSON.stringify(ev));
  });
  // Bind the peer tile BEFORE start so the first inbound keyframe lands on a
  // real canvas and the WebCodecs decoder draws it.
  session.bindTile(peer, canvas);
  session.bindPreview(preview);
  session.start(wsUrl);
  session.setRecipients([peer]);
  // Enable the real camera (WebCodecs VP8 encode) a beat after the socket opens.
  setTimeout(() => {
    log("setCamera(true)");
    session?.setCamera(true);
  }, 700);
}

function snapshotCanvas() {
  const canvas = document.getElementById("tile") as HTMLCanvasElement;
  const out: Record<string, number | string> = { w: canvas.width, h: canvas.height, nonBlank: 0, distinct: 0, sum: 0 };
  try {
    const ctx = canvas.getContext("2d");
    if (ctx && canvas.width > 0 && canvas.height > 0) {
      const img = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
      const seen = new Set<number>();
      let nonBlank = 0;
      let sum = 0;
      // sample every ~1000th pixel so readback is cheap on a 1280x720 tile.
      for (let i = 0; i < img.length; i += 4 * 1009) {
        const r = img[i], g = img[i + 1], b = img[i + 2];
        seen.add((r << 16) | (g << 8) | b);
        if (r || g || b) nonBlank++;
        sum += r + g + b;
      }
      out.nonBlank = nonBlank;
      out.distinct = seen.size;
      out.sum = sum;
    }
  } catch (e) {
    out.err = String(e);
  }
  return out;
}

(window as unknown as { __collect: () => unknown }).__collect = () => ({
  tap,
  events,
  canvas: snapshotCanvas(),
});
(window as unknown as { __run: () => void }).__run = run;

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", run);
} else {
  run();
}
