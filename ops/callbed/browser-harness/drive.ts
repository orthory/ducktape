// L3 driver (connect-only): TWO headless Chromium instances are already running
// (launched separately via the harness runner on fixed debug ports 9333/9334,
// each with fake mic+camera). This driver:
//   * serves the harness page + bundle + worklet from ONE 127.0.0.1 origin
//     (secure context for getUserMedia + AudioWorklet),
//   * over CDP, navigates each browser to the harness pointed at one live
//     callbed node with recipients = [the OTHER node's key],
//   * lets the REAL call client (call-session.ts) capture + WebCodecs-encode +
//     cross the mesh, then reads out of each page:
//       (a) peer-AUDIO energy  — mixed 0x01 playout RMS the real client received,
//       (b) peer-VIDEO frames  — 0x03 the real client's WebCodecs VideoDecoder
//           drew onto the bound canvas (frame count + canvas pixel readback).
//   SUCCESS = at least one direction shows BOTH peer audio energy AND peer video.
//
//   bun drive.ts [nodeA=8080] [nodeB=8081] [channel=general] [dbgA=9333] [dbgB=9334]

const HERE = new URL(".", import.meta.url).pathname;
const WORKLET = `${HERE}../../../app/public/voice-worklets.js`;
const [aPort = "8080", bPort = "8081", channel = "general", dbgA = "9333", dbgB = "9334"] = process.argv.slice(2);
const HARNESS_PORT = 5177;
const SETTLE_MS = Number(process.env.SETTLE_MS ?? 9000);
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const server = Bun.serve({
  port: HARNESS_PORT,
  hostname: "127.0.0.1",
  async fetch(req) {
    const p = new URL(req.url).pathname;
    if (p === "/voice-worklets.js") return new Response(Bun.file(WORKLET), { headers: { "content-type": "text/javascript" } });
    const name = p === "/" ? "index.html" : p.slice(1);
    const file = Bun.file(HERE + name);
    if (!(await file.exists())) return new Response("not found", { status: 404 });
    const type = name.endsWith(".js") ? "text/javascript" : name.endsWith(".html") ? "text/html" : "application/octet-stream";
    return new Response(file, { headers: { "content-type": type } });
  },
});
console.log(`harness server on http://127.0.0.1:${HARNESS_PORT}`);

async function statusKey(port: string): Promise<string> {
  const j: any = await (await fetch(`http://127.0.0.1:${port}/v1/status`)).json();
  if (!j.publicKey) throw new Error(`node :${port} /v1/status has no publicKey`);
  return String(j.publicKey).toLowerCase();
}

class Page {
  ws!: WebSocket;
  idc = 0;
  pending = new Map<number, (v: any) => void>();
  logs: string[] = [];
  constructor(readonly label: string, readonly debugPort: string) {}

  async attach(url: string) {
    let wsUrl = "";
    for (let i = 0; i < 60; i++) {
      try {
        const targets: any[] = await (await fetch(`http://127.0.0.1:${this.debugPort}/json`)).json();
        const page = targets.find((t) => t.type === "page");
        if (page?.webSocketDebuggerUrl) { wsUrl = page.webSocketDebuggerUrl; break; }
      } catch { /* not up yet */ }
      await sleep(200);
    }
    if (!wsUrl) throw new Error(`${this.label}: no page target on :${this.debugPort}`);
    this.ws = new WebSocket(wsUrl);
    await new Promise<void>((r, j) => { this.ws.onopen = () => r(); this.ws.onerror = () => j(new Error("cdp ws error")); });
    this.ws.onmessage = (ev) => {
      const msg = JSON.parse(ev.data as string);
      if (msg.id != null && this.pending.has(msg.id)) { this.pending.get(msg.id)!(msg); this.pending.delete(msg.id); }
      else if (msg.method === "Runtime.consoleAPICalled") {
        const text = (msg.params.args || []).map((a: any) => a.value ?? a.description ?? "").join(" ");
        this.logs.push(text);
      } else if (msg.method === "Runtime.exceptionThrown") {
        this.logs.push("EXC " + JSON.stringify(msg.params.exceptionDetails?.exception?.description ?? msg.params.exceptionDetails));
      }
    };
    await this.send("Page.enable");
    await this.send("Runtime.enable");
    await this.send("Page.navigate", { url });
  }

  send(method: string, params: any = {}): Promise<any> {
    const id = ++this.idc;
    return new Promise((res) => { this.pending.set(id, res); this.ws.send(JSON.stringify({ id, method, params })); });
  }

  async evaluate(expression: string): Promise<any> {
    const r = await this.send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
    if (r.result?.exceptionDetails) return { __error: JSON.stringify(r.result.exceptionDetails) };
    return r.result?.result?.value;
  }
}

async function main() {
  const keyA = await statusKey(aPort);
  const keyB = await statusKey(bPort);
  console.log(`node keys: A(:${aPort})=${keyA.slice(0, 12)}…  B(:${bPort})=${keyB.slice(0, 12)}…`);

  const A = new Page("A", dbgA);
  const B = new Page("B", dbgB);
  const urlA = `http://127.0.0.1:${HARNESS_PORT}/?node=${aPort}&peer=${keyB}&channel=${channel}`;
  const urlB = `http://127.0.0.1:${HARNESS_PORT}/?node=${bPort}&peer=${keyA}&channel=${channel}`;

  await Promise.all([A.attach(urlA), B.attach(urlB)]);
  console.log("both harness pages navigated; settling for real capture/encode/mesh/decode…");
  await sleep(SETTLE_MS);

  const snap1A = await A.evaluate("window.__collect ? window.__collect() : {__error:'no __collect'}");
  const snap1B = await B.evaluate("window.__collect ? window.__collect() : {__error:'no __collect'}");
  await sleep(1200);
  const snap2A = await A.evaluate("window.__collect ? window.__collect() : {}");
  const snap2B = await B.evaluate("window.__collect ? window.__collect() : {}");

  const report = (label: string, s1: any, s2: any) => {
    if (!s1 || s1.__error) { console.log(`\n== page ${label} == COLLECT FAILED: ${JSON.stringify(s1)}`); return null; }
    const t = s1.tap;
    const meanRms = t.audioFrames ? (t.audioRmsSum / t.audioFrames) : 0;
    const c1 = s1.canvas, c2 = s2?.canvas ?? {};
    const canvasChanged = c2 && (c2.sum !== c1.sum || c2.distinct !== c1.distinct);
    console.log(`\n== page ${label} (media RECEIVED from the peer node) ==`);
    console.log(`  AUDIO 0x01 mixed playout: frames=${t.audioFrames} meanRMS=${meanRms.toFixed(0)} maxRMS=${t.audioRmsMax.toFixed(0)} nonSilentFrames=${t.audioNonSilent} firstAtMs=${t.firstAudioMs}`);
    console.log(`  VIDEO 0x03 peer frames (real client received): frames=${t.videoFrames} keyframes=${t.videoKeyframes} bytes=${t.videoBytes} firstAtMs=${t.firstVideoMs}`);
    console.log(`  CANVAS (real WebCodecs VideoDecoder output): ${c1.w}x${c1.h} nonBlankSamples=${c1.nonBlank} distinctColors=${c1.distinct} sum1=${c1.sum} sum2=${c2.sum} changedBetweenSnaps=${canvasChanged}`);
    console.log(`  status events: ${JSON.stringify((s1.events || []).filter((e: any) => e.kind === "status").map((e: any) => e.status))}`);
    console.log(`  peerBeacons: ${(s1.events || []).filter((e: any) => e.kind === "peerBeacon").length}`);
    const audioOk = t.audioNonSilent > 0 && t.audioRmsMax > 200;
    const videoOk = t.videoFrames > 0 && Number(c1.nonBlank) > 0;
    return { audioOk, videoOk };
  };

  const rA = report("A", snap1A, snap2A);
  const rB = report("B", snap1B, snap2B);

  console.log(`\n---- console tails ----`);
  console.log(`A: ${A.logs.slice(-10).join(" | ")}`);
  console.log(`B: ${B.logs.slice(-10).join(" | ")}`);

  const dirOk = (r: any) => r && r.audioOk && r.videoOk;
  const pass = dirOk(rA) || dirOk(rB);
  console.log(`\n===== VERDICT =====`);
  console.log(`page A got BOTH peer audio + peer video: ${dirOk(rA) ? "YES ✓" : "no"}`);
  console.log(`page B got BOTH peer audio + peer video: ${dirOk(rB) ? "YES ✓" : "no"}`);
  console.log(`RESULT: ${pass ? "PASS ✓ — the REAL call client crossed audio+video through the live mesh" : "FAIL ✗"}`);

  server.stop(true);
  process.exit(pass ? 0 : 1);
}

main().catch((e) => { console.error("driver error:", e); server.stop(true); process.exit(3); });
