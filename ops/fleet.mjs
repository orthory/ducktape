import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { basename, dirname, extname, join, relative, resolve, sep } from "node:path";
import { createConnection } from "node:net";

function readJson(path, fallback) {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch {
    return fallback;
  }
}

export function slotFor(path, id) {
  const slots = readJson(path, {});
  if (!(id in slots)) {
    const used = new Set(Object.values(slots));
    let slot = 0;
    while (used.has(slot)) slot += 1;
    slots[id] = slot;
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, JSON.stringify(slots));
  }
  return slots[id];
}

export function freePorts(count) {
  const listeners = Array.from({ length: count }, () =>
    Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } }),
  );
  const ports = listeners.map(({ port }) => port);
  listeners.forEach((listener) => listener.stop());
  return ports;
}

function sh(...args) {
  try {
    return spawnSync(args[0], args.slice(1), {
      encoding: "utf8",
      timeout: 8_000,
    }).stdout.trim();
  } catch {
    return "";
  }
}

function slug(value) {
  return value.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
}

function worktrees(raw) {
  return raw
    .trim()
    .split(/\n\n+/)
    .map((record) => {
      const lines = record.split("\n");
      const path = lines.find((line) => line.startsWith("worktree "))?.slice(9);
      const branch = lines
        .find((line) => line.startsWith("branch "))
        ?.slice(7)
        .replace("refs/heads/", "");
      return { path, branch };
    })
    .filter(({ path, branch }) => path && branch && existsSync(join(path, "app")));
}

function portOpen(port) {
  return new Promise((done) => {
    const socket = createConnection({ host: "127.0.0.1", port });
    let settled = false;
    const finish = (open) => {
      if (settled) return;
      settled = true;
      socket.destroy();
      done(open);
    };
    socket.setTimeout(300);
    socket.once("connect", () => finish(true));
    socket.once("error", () => finish(false));
    socket.once("timeout", () => finish(false));
  });
}

export async function emitFleet(env = process.env) {
  const main = env.MAIN_ROOT;
  const base = env.BASE_BRANCH;
  const state = env.STATE;
  const dist = env.DIST;
  const agentAppId = env.TAURI_AGENT_APP_ID;
  const tsip = env.TSIP;
  const webPort = Number(env.WEB_PORT);
  const displayBase = Number(env.DISP_BASE);
  const vncBase = Number(env.VNC_BASE);
  const slots = readJson(join(state, "slots.json"), {});
  const raw = sh("git", "-C", main, "worktree", "list", "--porcelain");

  const out = await Promise.all(
    worktrees(raw).map(async ({ path, branch }) => {
      const id = slug(branch);
      const commits = sh("git", "-C", path, "log", "-4", "--pretty=%h%x1f%s%x1f%cr")
        .split("\n")
        .filter(Boolean)
        .map((line) => line.split("\x1f"))
        .filter((parts) => parts.length === 3)
        .map(([sha, subject, age]) => ({ sha, subject, age }));
      const dirty = sh("git", "-C", path, "status", "--porcelain")
        .split("\n")
        .filter((line) => line.trim()).length;
      const node = {
        id,
        branch,
        path: relative(main, path) || ".",
        head: {
          sha: sh("git", "-C", path, "rev-parse", "--short", "HEAD"),
          subject: sh("git", "-C", path, "log", "-1", "--pretty=%s"),
        },
        parent: branch === base ? null : base,
        ahead: Number(sh("git", "-C", path, "rev-list", "--count", `${base}..HEAD`) || 0),
        behind: Number(sh("git", "-C", path, "rev-list", "--count", `HEAD..${base}`) || 0),
        activity: { dirty, commits },
        status: "down",
      };

      if (id in slots) {
        const slot = slots[id];
        const vncPort = vncBase + slot;
        const runtimeDir = join(state, id);
        const endpointPath = join(runtimeDir, "tauri-agent", agentAppId, "endpoint.json");
        const endpointReady = existsSync(endpointPath);
        Object.assign(node, {
          slot,
          display: `:${displayBase + slot}`,
          vncPort,
          token: id,
          agent: {
            appId: agentAppId,
            runtimeDir,
            endpointPath,
            endpointReady,
            observe: {
              protocol: "tauri-agent-observe-ndjson",
              cwd: relative(main, path) || ".",
              env: { XDG_RUNTIME_DIR: runtimeDir },
              argv: [
                "app/scripts/tauri-agent",
                "observe",
                "--app",
                agentAppId,
                "--format",
                "ndjson",
              ],
            },
          },
        });
        if (endpointReady && (await portOpen(vncPort))) node.status = "up";
        else if (existsSync(join(state, id, "tauri.log"))) node.status = "building";
      }
      return node;
    }),
  );

  out.sort((a, b) =>
    Number(a.branch !== base) - Number(b.branch !== base) ||
    (a.slot ?? 999) - (b.slot ?? 999) ||
    a.branch.localeCompare(b.branch),
  );
  const doc = {
    generatedAt: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
    host: tsip,
    webPort,
    base,
    worktrees: out,
  };
  mkdirSync(dist, { recursive: true });
  const output = join(dist, "fleet.json");
  writeFileSync(output, `${JSON.stringify(doc, null, 2)}\n`);
  console.log(`fleet.json: ${out.length} worktree(s) -> ${output}`);
  return doc;
}

const MIME = {
  ".css": "text/css; charset=utf-8",
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".map": "application/json; charset=utf-8",
  ".svg": "image/svg+xml",
  ".wasm": "application/wasm",
};

function targetFor(tokens, token) {
  if (!/^[a-z0-9-]+$/.test(token) || basename(token) !== token) return null;
  try {
    const line = readFileSync(join(tokens, token), "utf8").trim();
    const match = line.match(/^[^:]+:\s*(127\.0\.0\.1):(\d+)$/);
    return match ? { host: match[1], port: Number(match[2]) } : null;
  } catch {
    return null;
  }
}

export function startWeb(dist, tokens, hostname, port) {
  const root = resolve(dist);
  return Bun.serve({
    hostname,
    port: Number(port),
    async fetch(request, server) {
      const url = new URL(request.url);
      if (url.pathname === "/websockify") {
        const target = targetFor(tokens, url.searchParams.get("token") ?? "");
        if (!target) return new Response("unknown VNC token", { status: 404 });
        const data = { ...target, tcp: null, closed: false };
        return server.upgrade(request, { data })
          ? undefined
          : new Response("WebSocket upgrade required", { status: 400 });
      }
      if (request.method !== "GET" && request.method !== "HEAD") {
        return new Response("method not allowed", { status: 405 });
      }
      let path;
      try {
        path = decodeURIComponent(url.pathname);
      } catch {
        return new Response("bad path", { status: 400 });
      }
      const file = resolve(root, `.${path === "/" ? "/index.html" : path}`);
      if (!file.startsWith(`${root}${sep}`)) return new Response("forbidden", { status: 403 });
      try {
        if (!statSync(file).isFile()) throw new Error("not a file");
      } catch {
        return new Response("not found", { status: 404 });
      }
      const headers = {
        "content-type": MIME[extname(file)] ?? "application/octet-stream",
        "cache-control": file.endsWith("fleet.json") ? "no-store" : "no-cache",
      };
      return new Response(request.method === "HEAD" ? null : Bun.file(file), { headers });
    },
    websocket: {
      idleTimeout: 0,
      perMessageDeflate: false,
      backpressureLimit: 16 * 1024 * 1024,
      closeOnBackpressureLimit: true,
      open(ws) {
        const state = ws.data;
        const tcp = createConnection({ host: state.host, port: state.port });
        state.tcp = tcp;
        tcp.on("data", (data) => {
          const sent = ws.send(data);
          if (sent === -1) tcp.pause();
          else if (sent === 0) tcp.destroy();
        });
        tcp.on("close", () => {
          if (!state.closed) ws.close();
        });
        tcp.on("error", () => {
          if (!state.closed) ws.close(1011, "VNC connection failed");
        });
      },
      message(ws, message) {
        ws.data.tcp.write(Buffer.from(message));
      },
      drain(ws) {
        ws.data.tcp?.resume();
      },
      close(ws) {
        ws.data.closed = true;
        ws.data.tcp?.destroy();
      },
      error(ws) {
        ws.data.closed = true;
        ws.data.tcp?.destroy();
      },
    },
  });
}

async function main() {
  const [command, ...args] = Bun.argv.slice(2);
  if (command === "slot") console.log(slotFor(args[0], args[1]));
  else if (command === "ports") console.log(freePorts(Number(args[0])).join(" "));
  else if (command === "emit") await emitFleet();
  else if (command === "serve") {
    const server = startWeb(args[0], args[1], args[2], args[3]);
    console.log(`fleet web listening on ${server.url}`);
  } else {
    throw new Error("usage: fleet.mjs {slot|ports|emit|serve} ...");
  }
}

if (import.meta.main) main().catch((error) => {
  console.error(error.message);
  process.exit(1);
});
