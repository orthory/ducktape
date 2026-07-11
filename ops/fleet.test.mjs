import { afterEach, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { slotFor, startWeb } from "./fleet.mjs";

const cleanups = [];
afterEach(() => {
  while (cleanups.length) cleanups.pop()();
});

test("slots are stable and reuse the lowest free number", () => {
  const dir = mkdtempSync(join(tmpdir(), "ducktape-fleet-"));
  cleanups.push(() => rmSync(dir, { recursive: true, force: true }));
  const path = join(dir, "slots.json");
  expect(slotFor(path, "dev")).toBe(0);
  expect(slotFor(path, "feature")).toBe(1);
  expect(slotFor(path, "dev")).toBe(0);
});

test("udp-port emits plain digits when terminal colors are forced", () => {
  const result = spawnSync("bun", [join(import.meta.dir, "fleet.mjs"), "udp-port"], {
    encoding: "utf8",
    env: { ...process.env, FORCE_COLOR: "1" },
  });
  expect(result.status).toBe(0);
  expect(result.stdout).toMatch(/^\d+\n$/);
});

test("web server serves the console and bridges binary VNC bytes", async () => {
  const dir = mkdtempSync(join(tmpdir(), "ducktape-fleet-"));
  const dist = join(dir, "dist");
  const tokens = join(dir, "tokens");
  mkdirSync(dist);
  mkdirSync(tokens);
  writeFileSync(join(dist, "index.html"), "fleet console");

  const tcp = Bun.listen({
    hostname: "127.0.0.1",
    port: 0,
    socket: { data(socket, data) { socket.write(data); } },
  });
  writeFileSync(join(tokens, "dev"), `dev: 127.0.0.1:${tcp.port}\n`);
  const web = startWeb(dist, tokens, "127.0.0.1", 0);
  cleanups.push(() => {
    web.stop(true);
    tcp.stop(true);
    rmSync(dir, { recursive: true, force: true });
  });

  expect(await (await fetch(new URL("/", web.url))).text()).toBe("fleet console");
  const echoed = await new Promise((resolve, reject) => {
    const ws = new WebSocket(new URL("/websockify?token=dev", web.url).toString().replace("http", "ws"));
    ws.binaryType = "arraybuffer";
    ws.onopen = () => ws.send(Uint8Array.of(0, 1, 2, 255));
    ws.onerror = () => reject(new Error("websocket failed"));
    ws.onmessage = ({ data }) => {
      ws.close();
      resolve([...new Uint8Array(data)]);
    };
  });
  expect(echoed).toEqual([0, 1, 2, 255]);
});
