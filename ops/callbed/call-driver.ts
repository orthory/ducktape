// Drive a real call between two peered ducktape nodes over their /v1/call/ws
// surfaces and self-verify that both AUDIO and VIDEO cross the mesh. No
// mic/webcam needed:
//   - audio: synthesize a PCM tone on one node's session, measure it playing
//     out of the OTHER node's mixed-audio stream.
//   - video: the node treats encoded frames as OPAQUE fragmentable bytes (it
//     never decodes VP8 — that's browser-side WebCodecs), so we send a
//     synthetic multi-fragment "camera frame" and assert it fragments across
//     Service::Video (overlay data plane), crosses to the peer, and reassembles
//     BYTE-EXACT on the far node. This tests the video TRANSPORT (the part the
//     in-process hub tests never route over the real mesh); it does NOT test
//     VP8 encode/decode, which is out of scope for a headless bed.
//
//   bun call-driver.ts <HOST:PORT_A> <HOST:PORT_B>
//   (env SETTLE_MS: warmup after recipients before measuring; default 400)
//
// wire (chat::call_wire — the single definition site; headers BE per D1, PCM payload stays LE):
//   in  0x01|960*i16le PCM,  0x02|flags|ts_ms(4be)|vp8-bytes,  text {type:"recipients",peers}
//   out 0x01|960*i16le mixed PCM (20ms),  0x03|flags|ts_ms(4be)|peer(32)|vp8-bytes,  text control

const [httpA, httpB] = process.argv.slice(2);
if (!httpA || !httpB) {
  console.error("usage: bun call-driver.ts <HOST:PORT_A> <HOST:PORT_B>");
  process.exit(2);
}
const SETTLE_MS = Number(process.env.SETTLE_MS ?? 400);
const N = 960, SR = 48000;
const TAG_AUDIO = 0x01, TAG_VIDEO_CAPTURED = 0x02, TAG_VIDEO_PEER = 0x03;
const FLAG_KEYFRAME = 0x01, PEER_HDR = 38; // 0x03 | flags | ts_ms(4) | peer(32) | data
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

async function statusKey(http: string): Promise<string> {
  const r = await fetch(`http://${http}/v1/status`);
  const j: any = await r.json();
  if (!j.publicKey) throw new Error(`/v1/status from ${http} has no publicKey (embedded daemon, not a validator?)`);
  return j.publicKey;
}

function toneFrame(freq: number, tick: number, amp = 8000): Uint8Array {
  const buf = new Uint8Array(1 + N * 2);
  buf[0] = TAG_AUDIO;
  const dv = new DataView(buf.buffer);
  for (let i = 0; i < N; i++) {
    const t = (tick * N + i) / SR;
    dv.setInt16(1 + i * 2, Math.round(Math.sin(2 * Math.PI * freq * t) * amp), true);
  }
  return buf;
}

function rmsOf(ab: ArrayBuffer): number | null {
  const u = new Uint8Array(ab);
  if (u[0] !== TAG_AUDIO) return null;
  const dv = new DataView(ab, 1);
  const n = (u.byteLength - 1) >> 1;
  let sum = 0;
  for (let i = 0; i < n; i++) { const s = dv.getInt16(i * 2, true); sum += s * s; }
  return Math.sqrt(sum / n);
}

// a synthetic encoded frame: position-dependent fill so any reorder/dup/loss in
// reassembly breaks byte-equality (uniform bytes would hide it).
function synthFrame(len: number, seed: number): Uint8Array {
  const d = new Uint8Array(len);
  for (let i = 0; i < len; i++) d[i] = (i * seed + 3) % 251;
  return d;
}
function capturedVideo(keyframe: boolean, tsMs: number, data: Uint8Array): Uint8Array {
  const buf = new Uint8Array(6 + data.length); // tag | flags | ts_ms(4) | data
  buf[0] = TAG_VIDEO_CAPTURED;
  buf[1] = keyframe ? FLAG_KEYFRAME : 0;
  new DataView(buf.buffer).setUint32(2, tsMs); // big-endian (D1)
  buf.set(data, 6);
  return buf;
}
type PeerVideo = { peer: string; keyframe: boolean; tsMs: number; data: Uint8Array };
function parsePeerVideo(ab: ArrayBuffer): PeerVideo | null {
  const u = new Uint8Array(ab);
  if (u[0] !== TAG_VIDEO_PEER || u.byteLength < PEER_HDR) return null;
  const dv = new DataView(ab);
  const peer = [...u.slice(6, 38)].map((b) => b.toString(16).padStart(2, "0")).join("");
  return { peer, keyframe: (u[1] & FLAG_KEYFRAME) !== 0, tsMs: dv.getUint32(2), data: u.slice(PEER_HDR) };
}
const bytesEqual = (a: Uint8Array, b: Uint8Array) => a.length === b.length && a.every((v, i) => v === b[i]);

async function waitFor<T>(pred: () => T | undefined, ms: number): Promise<T | undefined> {
  const start = Date.now();
  while (Date.now() - start < ms) { const v = pred(); if (v) return v; await sleep(50); }
  return undefined;
}

type Peer = { ws: WebSocket; label: string; key: string; collect: boolean; samples: number[]; videos: PeerVideo[]; texts: string[] };

function connect(http: string, label: string, key: string): Peer {
  const ws = new WebSocket(`ws://${http}/v1/call/ws?channel=general`);
  ws.binaryType = "arraybuffer";
  const p: Peer = { ws, label, key, collect: false, samples: [], videos: [], texts: [] };
  ws.addEventListener("message", (ev: any) => {
    if (typeof ev.data === "string") { p.texts.push(ev.data); return; }
    const ab = ev.data as ArrayBuffer;
    const r = rmsOf(ab);
    if (r !== null) { if (p.collect) p.samples.push(r); return; }
    const pv = parsePeerVideo(ab);
    if (pv) p.videos.push(pv);
  });
  return p;
}

function waitOpen(ws: WebSocket): Promise<void> {
  return new Promise((res, rej) => {
    if (ws.readyState === 1) return res();
    ws.addEventListener("open", () => res(), { once: true });
    ws.addEventListener("error", () => rej(new Error("ws error")), { once: true });
    ws.addEventListener("close", () => rej(new Error("ws closed before open")), { once: true });
  });
}

async function audioPhase(name: string, sender: Peer, receiver: Peer, send: boolean, ms: number, freq: number): Promise<number> {
  receiver.samples = []; receiver.collect = true;
  const ticks = Math.ceil(ms / 20);
  for (let t = 0; t < ticks; t++) {
    if (send && sender.ws.readyState === 1) sender.ws.send(toneFrame(freq, t));
    await sleep(20);
  }
  receiver.collect = false;
  const s = receiver.samples;
  const mean = s.length ? s.reduce((a, b) => a + b, 0) / s.length : 0;
  console.log(`  ${name}: recv=${receiver.label} frames=${s.length} meanRMS=${mean.toFixed(0)}`);
  return mean;
}

// send a multi-fragment keyframe then a delta on `sender`; both must reassemble
// byte-exact on `receiver`, tagged with sender's node key.
async function videoCrosses(sender: Peer, receiver: Peer): Promise<boolean> {
  receiver.videos = [];
  const kf = synthFrame(5000, 1);   // ~4 datagrams on Service::Video
  const delta = synthFrame(5000, 7);
  sender.ws.send(capturedVideo(true, 7, kf));
  const gotKf = await waitFor(() => receiver.videos.find((v) => v.keyframe && v.tsMs === 7), 8000);
  sender.ws.send(capturedVideo(false, 40, delta));
  const gotDelta = await waitFor(() => receiver.videos.find((v) => !v.keyframe && v.tsMs === 40), 8000);
  const kfOk = !!gotKf && gotKf.peer === sender.key && bytesEqual(gotKf.data, kf);
  const dOk = !!gotDelta && gotDelta.peer === sender.key && bytesEqual(gotDelta.data, delta);
  console.log(
    `  video ${sender.label}->${receiver.label}: ` +
    `keyframe=${gotKf ? `${gotKf.data.length}B exact=${bytesEqual(gotKf.data, kf)} peerOk=${gotKf.peer === sender.key}` : "MISSING"}, ` +
    `delta=${gotDelta ? `exact=${bytesEqual(gotDelta.data, delta)}` : "MISSING"}`,
  );
  return kfOk && dOk;
}

async function main() {
  const keyA = await statusKey(httpA), keyB = await statusKey(httpB);
  console.log(`node keys: A=${keyA.slice(0, 12)}… B=${keyB.slice(0, 12)}…`);
  const A = connect(httpA, "A", keyA), B = connect(httpB, "B", keyB);
  await Promise.all([waitOpen(A.ws), waitOpen(B.ws)]);
  console.log("both /v1/call/ws sockets open");

  A.ws.send(JSON.stringify({ type: "recipients", peers: [keyB] }));
  B.ws.send(JSON.stringify({ type: "recipients", peers: [keyA] }));
  await sleep(SETTLE_MS);

  const refusal = [...A.texts, ...B.texts].find((t) => t.toLowerCase().includes("not available"));
  if (refusal) { console.error(`REFUSED by node: ${refusal}`); process.exit(1); }
  if (A.ws.readyState !== 1 || B.ws.readyState !== 1) {
    console.error(`a socket closed early (A=${A.ws.readyState} B=${B.ws.readyState}); texts=${JSON.stringify([...A.texts, ...B.texts])}`);
    process.exit(1);
  }

  console.log("\n== AUDIO ==");
  console.log("-- A -> B --");
  const baseB = await audioPhase("baseline(A silent)", A, B, false, 600, 0);
  const toneB = await audioPhase("tone(A->B 440Hz) ", A, B, true, 1600, 440);
  console.log("-- B -> A --");
  const baseA = await audioPhase("baseline(B silent)", B, A, false, 600, 0);
  const toneA = await audioPhase("tone(B->A 660Hz) ", B, A, true, 1600, 660);

  console.log("\n== VIDEO (fragment/reassemble over Service::Video) ==");
  const vidAB = await videoCrosses(A, B);
  const vidBA = await videoCrosses(B, A);

  const okAudioAB = toneB > 200 && toneB > baseB * 3;
  const okAudioBA = toneA > 200 && toneA > baseA * 3;
  console.log("\n===== VERDICT =====");
  console.log(`A -> B audio crossed real mesh: ${okAudioAB ? "YES ✓" : "NO ✗"}  (base=${baseB.toFixed(0)} tone=${toneB.toFixed(0)})`);
  console.log(`B -> A audio crossed real mesh: ${okAudioBA ? "YES ✓" : "NO ✗"}  (base=${baseA.toFixed(0)} tone=${toneA.toFixed(0)})`);
  console.log(`A -> B video crossed real mesh: ${vidAB ? "YES ✓" : "NO ✗"}  (byte-exact multi-fragment reassembly)`);
  console.log(`B -> A video crossed real mesh: ${vidBA ? "YES ✓" : "NO ✗"}`);
  const texts = [...new Set([...A.texts, ...B.texts])];
  if (texts.length) console.log(`control frames observed (${texts.length}):`, JSON.stringify(texts.slice(0, 4)));
  A.ws.close(); B.ws.close();
  process.exit(okAudioAB && okAudioBA && vidAB && vidBA ? 0 : 1);
}
main().catch((e) => { console.error("driver error:", e); process.exit(3); });
