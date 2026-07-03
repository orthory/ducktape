#!/usr/bin/env node
/**
 * qa-instance — boot ONE fully isolated Ducktape app instance for a worktree, so
 * several worktrees' apps can run at once without stepping on each other. This is
 * the launcher half of the multi-window worktree QA system; the agent drives each
 * booted instance with app/scripts/tauri-debug.mjs (which already reads the
 * per-instance socket from DUCKTAPE_TAURI_MCP_SOCKET).
 *
 * The whole job is making every shared resource PER-INSTANCE. We allocate one
 * value per collision axis, then RECORD them in a manifest the driver reads back
 * — nothing is hardcoded or re-derived:
 *
 *   axis            how it is isolated                        env the child gets
 *   ─────────────── ──────────────────────────────────────── ─────────────────────────
 *   vite dev port   free port bound :0, pinned strict          DUCKTAPE_TAURI_DEV_PORT
 *   tauri devUrl    --config override -> the vite url          (tauri dev --config)
 *   debug socket    <runRoot>/mcp.sock                         DUCKTAPE_TAURI_MCP_SOCKET
 *   workspace reg   <runRoot>/ducktape (registry.json + nodes) DUCKTAPE_HOME
 *   node binary     shared-target build                        DUCKTAPE_NODE_BIN
 *   X display       private Xvfb (:101+)                        DISPLAY + WebKit flags
 *
 * The desktop app is a multi-WORKSPACE shell: on boot it reads its ~/.ducktape
 * registry and connects the active workspace's node (each workspace already gets
 * its own app-allocated ports + storage), or shows onboarding when there is none.
 * So isolation needs just ONE seam — DUCKTAPE_HOME (added in this branch,
 * workspaces.rs::root) points each instance at a PRIVATE registry. `up` then
 * drives the app's real `workspace_create` command over the debug socket to found
 * a solo workspace, reads its app-allocated http port back from registry.json, and
 * reloads the window onto the live console. The workspace node's port + storage
 * are the app's to allocate; we only isolate the registry root.
 *
 * Run root (short, to stay under the ~108-char unix-socket sun_path limit — NOT
 * the deep .claude/worktrees path):  ${XDG_RUNTIME_DIR:-/tmp}/ducktape-qa/<slug>/
 *
 * Usage:
 *   node app/scripts/qa-instance.mjs up   [<worktree-dir>]   # boot; prints the manifest path
 *   node app/scripts/qa-instance.mjs down [<worktree-dir|slug>] [--last]
 *   node app/scripts/qa-instance.mjs list
 *   node app/scripts/qa-instance.mjs env  [<worktree-dir|slug>]  # print `export`s for the driver
 */
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { spawn, execFileSync } from "node:child_process";
import {
  mkdirSync,
  writeFileSync,
  readFileSync,
  existsSync,
  statSync,
  rmSync,
  readdirSync,
  openSync,
  copyFileSync,
  chmodSync,
} from "node:fs";

// ── run-root layout ─────────────────────────────────────

const QA_HOME = path.join(process.env.XDG_RUNTIME_DIR || os.tmpdir(), "ducktape-qa");
const runRoot = (slug) => path.join(QA_HOME, slug);
const manifestPath = (slug) => path.join(runRoot(slug), "instance.json");

// slug from the worktree branch (matches `work`'s WT_DIR naming: `/`→`+`), lower,
// and length-capped so <runRoot>/mcp.sock stays well under the sun_path limit.
const slugForWorktree = (wt) => {
  const branch = gitBranch(wt) || path.basename(wt);
  const base = branch.replace(/\//g, "+").toLowerCase().replace(/[^a-z0-9+._-]/g, "-");
  return base.length <= 40 ? base : `${base.slice(0, 32)}-${shortHash(base)}`;
};

// tiny non-crypto hash so a truncated slug stays unique
const shortHash = (s) => {
  let h = 0;
  for (let i = 0; i < s.length; i++) h = (Math.imul(31, h) + s.charCodeAt(i)) | 0;
  return (h >>> 0).toString(36).slice(0, 6);
};

// ── git helpers (resolve the worktree + its shared main-checkout target) ──

const git = (wt, args) =>
  execFileSync("git", ["-C", wt, ...args], { encoding: "utf8" }).trim();

const gitBranch = (wt) => {
  try {
    return git(wt, ["branch", "--show-current"]);
  } catch {
    return "";
  }
};

// the MAIN checkout root (parent of the absolute git common dir) — its target/ is
// shared across worktrees so we don't recompile the tauri shell N times.
const mainRoot = (wt) => path.dirname(git(wt, ["rev-parse", "--path-format=absolute", "--git-common-dir"]));

// ── free-port allocation: bind :0, read, close, hand over ─

const allocPort = () =>
  new Promise((res, rej) => {
    const srv = net.createServer();
    srv.on("error", rej);
    srv.listen(0, "127.0.0.1", () => {
      const { port } = srv.address();
      srv.close(() => res(port));
    });
  });

// ── node binary: stage a STABLE copy outside the target dir ──────────────
//
// The app spawns its workspace nodes via DUCKTAPE_NODE_BIN. It must NOT point
// into the shared target dir: the tauri app build's build.rs writes an EMPTY,
// non-executable placeholder over `target/debug/ducktape-node` on every build
// (the "baffling permission-denied" the daemon.rs comment warns about), which
// would clobber a real node binary at that path. So we resolve a real source
// (prefer release — the debug app build never touches it — else debug, else
// build node-bin) and cache one copy under QA_HOME/.bin, immune to the clobber.

const NODE_CACHE = path.join(QA_HOME, ".bin", "ducktape-node");

const stageNodeBin = (wt, targetDir) => {
  if (process.env.DUCKTAPE_NODE_BIN) return process.env.DUCKTAPE_NODE_BIN;
  if (usableExe(NODE_CACHE)) return NODE_CACHE; // already staged — reuse across instances
  const rel = path.join(targetDir, "release", "ducktape-node");
  const dbg = path.join(targetDir, "debug", "ducktape-node");
  let src = usableExe(rel) ? rel : usableExe(dbg) ? dbg : null;
  if (!src) {
    process.stderr.write("· no ducktape-node in the shared target — building it (cargo build -p node-bin)…\n");
    execFileSync("cargo", ["build", "-p", "node-bin"], {
      cwd: mainRoot(wt),
      stdio: "inherit",
      env: { ...process.env, CARGO_TARGET_DIR: targetDir },
    });
    src = usableExe(dbg) ? dbg : usableExe(rel) ? rel : null;
    if (!src) throw new Error(`build produced no usable ducktape-node under ${targetDir}`);
  }
  mkdirSync(path.dirname(NODE_CACHE), { recursive: true });
  copyFileSync(src, NODE_CACHE);
  chmodSync(NODE_CACHE, 0o755);
  process.stderr.write(`· staged node binary → ${NODE_CACHE} (from ${src})\n`);
  return NODE_CACHE;
};

const usableExe = (p) => {
  try {
    const st = statSync(p);
    return st.isFile() && st.size > 0;
  } catch {
    return false;
  }
};

// ── the X display: ONE dedicated Xvfb per instance. A shared display would need
//    a per-window snapshot (`debug_capture_webview` — not registered in this app)
//    or a window manager to capture a single instance; a private display lets
//    `import -window root` grab exactly this instance with no compositing. Bonus:
//    QA windows never land on the user's :99 (their remote-tauri/VNC session). ──

const allocDisplay = () => {
  // first free display number from :101 up (skip :0/:99 that a real/VNC session uses)
  let n = 101;
  while (n < 300 && existsSync(`/tmp/.X11-unix/X${n}`)) n++;
  const display = `:${n}`;
  const xvfb = spawn("Xvfb", [display, "-screen", "0", "1400x900x24", "-nolisten", "tcp"], {
    detached: true,
    stdio: "ignore",
  });
  xvfb.unref();
  return { display, xvfbPid: xvfb.pid };
};

// ── manifest io ─────────────────────────────────────────

const readManifest = (slug) => {
  const p = manifestPath(slug);
  if (!existsSync(p)) return null;
  return JSON.parse(readFileSync(p, "utf8"));
};

const writeManifest = (m) => writeFileSync(manifestPath(m.slug), JSON.stringify(m, null, 2) + "\n");

// resolve an `up`/`down` target (a worktree dir OR a bare slug) to a slug
const targetSlug = (arg) => {
  if (!arg) return slugForWorktree(process.cwd());
  if (existsSync(path.join(arg, ".git")) || existsSync(path.join(arg, "app"))) return slugForWorktree(path.resolve(arg));
  return arg; // treat as a bare slug
};

// ── readiness + the ~/.ducktape registry ────────────────

const wait = (ms) => new Promise((r) => setTimeout(r, ms));

const nodeAnswers = async (url) => {
  try {
    const res = await fetch(`${url}/v1/status`, { signal: AbortSignal.timeout(1500) });
    return res.ok;
  } catch {
    return false;
  }
};

const registryPathOf = (home) => path.join(home, "registry.json");

const readRegistry = (home) => {
  const p = registryPathOf(home);
  if (!existsSync(p)) return null;
  try {
    return JSON.parse(readFileSync(p, "utf8"));
  } catch {
    return null;
  }
};

const logTail = (logFile) =>
  existsSync(logFile) ? readFileSync(logFile, "utf8").split("\n").slice(-25).join("\n") : "(no log)";

// run one ducktape-node onboarding verb; return its last non-empty stdout line
// (the datum — chain-id for `init`, pubkey for `keygen`), matching workspaces.rs.
const runVerb = (nodeBin, args) => {
  const out = execFileSync(nodeBin, args, { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
  return out.trim().split("\n").map((l) => l.trim()).filter(Boolean).pop() || "";
};

// Pre-seed a solo workspace into a PRIVATE registry BEFORE the app boots — the
// deterministic equivalent of workspace_create (workspaces.rs), running the same
// `init` + `keygen` verbs. Driving the app's own command over the socket then
// reloading proved fragile (a reload drops Tauri's IPC injection and the app
// falls back to REMOTE mode). With the workspace already active, the app boots
// straight into it (DucktapeProvider reads activeWorkspace → workspace_select
// spawns the node → live console) — no onboarding, no reload.
const seedWorkspace = async (home, nodeBin, name) => {
  const id = name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "") || "workspace";
  const dir = path.join(home, "workspaces", id);
  mkdirSync(dir, { recursive: true });
  const [listen, http, rpc] = await Promise.all([allocPort(), allocPort(), allocPort()]);
  const at = (p) => `127.0.0.1:${p}`;
  const chainId = runVerb(nodeBin, [
    "init", "--name", name, "--dir", dir,
    "--listen", at(listen), "--advertised", at(listen), "--http", at(http), "--rpc", at(rpc),
  ]);
  const pubkey = runVerb(nodeBin, ["keygen", "--out", path.join(dir, "identity.key")]);
  const registry = {
    version: 1,
    active: id,
    workspaces: [{ id, name, chainId, pubkey, founder: true, member: true, ports: { listen, http, rpc } }],
  };
  writeFileSync(registryPathOf(home), JSON.stringify(registry, null, 2) + "\n");
  return { id, dir, listen, url: `http://${at(http)}` };
};

// Start the workspace node ourselves (detached), so it is up regardless of how
// the app's webview boots. The app then connects to THIS node either way: the
// desktop path's workspace_select adopts an already-listening node (it probes the
// p2p listen port before spawning), and the web-build fallback dials it via
// VITE_DUCKTAPE_NODE_URL. That makes the app's isTauri/IPC boot timing irrelevant
// to isolation — the window always talks to this instance's private node.
const startNode = (nodeBin, dir) => {
  const log = openSync(path.join(dir, "daemon.log"), "a");
  const child = spawn(nodeBin, ["--config", path.join(dir, "node.toml")], {
    cwd: dir, // relative network.toml / identity.key / storage resolve here
    detached: true,
    stdio: ["ignore", log, log],
  });
  child.unref();
  return child.pid;
};

// ── up ──────────────────────────────────────────────────

const up = async (arg) => {
  const wt = path.resolve(arg || process.cwd());
  if (!existsSync(path.join(wt, "app"))) throw new Error(`not a Ducktape worktree (no app/ under ${wt})`);
  const slug = slugForWorktree(wt);
  const root = runRoot(slug);

  const existing = readManifest(slug);
  if (existing?.workspaceHttpUrl && (await nodeAnswers(existing.workspaceHttpUrl))) {
    process.stderr.write(`· instance '${slug}' already up — reusing its manifest\n`);
    console.log(manifestPath(slug));
    return;
  }
  if (existing) await teardown(existing); // stale/dead manifest — clear it before booting

  const ducktapeHome = path.join(root, "ducktape");
  mkdirSync(ducktapeHome, { recursive: true });
  const socketPath = path.join(root, "mcp.sock");
  if (existsSync(socketPath)) rmSync(socketPath); // stale socket from a dead run
  const logFile = path.join(root, "tauri-dev.log");
  const configPath = path.join(root, "tauri-dev.config.json");

  const targetDir = process.env.DUCKTAPE_QA_TARGET_DIR || path.join(mainRoot(wt), "target");
  const appDir = path.join(wt, "app");

  const vitePort = await allocPort();
  const viteUrl = `http://localhost:${vitePort}`;
  const nodeBin = stageNodeBin(wt, targetDir);
  const { display, xvfbPid } = allocDisplay();

  // Seed the solo workspace into the private registry BEFORE boot, then start its
  // node ourselves so it is up independent of the app's webview boot timing.
  process.stderr.write(`· founding workspace 'qa' in ${ducktapeHome}\n`);
  const ws = await seedWorkspace(ducktapeHome, nodeBin, "qa");
  const nodePid = startNode(nodeBin, ws.dir);

  // tauri dev config override: skip the slow release-sidecar beforeDevCommand (we
  // start vite ourselves + set DUCKTAPE_NODE_BIN) and point the window at our vite.
  writeFileSync(configPath, JSON.stringify({ build: { beforeDevCommand: null, devUrl: viteUrl } }));

  const webkit = {
    DISPLAY: display,
    WEBKIT_DISABLE_DMABUF_RENDERER: "1",
    WEBKIT_DISABLE_COMPOSITING_MODE: "1",
    LIBGL_ALWAYS_SOFTWARE: "1",
    GDK_BACKEND: "x11",
  };

  const log = openSync(logFile, "a");

  const vite = spawn("bun", ["run", "dev"], {
    cwd: appDir,
    detached: true,
    stdio: ["ignore", log, log],
    env: {
      ...process.env,
      DUCKTAPE_TAURI_DEV_PORT: String(vitePort),
      // Web-build fallback: if the webview boots before Tauri injects its IPC
      // (isTauri false), the app dials this instead of the default 8844 — so it
      // still lands on THIS instance's node, never a shared/other daemon.
      VITE_DUCKTAPE_NODE_URL: ws.url,
      ...webkit,
    },
  });
  vite.unref();

  // tauri dev under a per-instance dbus session (WebKitGTK needs a session bus).
  // --no-dev-server-wait: we already started vite. Own process group so `down`
  // can signal the whole tree (tauri CLI respawns a crashed app — kill the group).
  // DUCKTAPE_HOME points the registry at this instance's private root.
  const tauri = spawn(
    "dbus-run-session",
    ["--", "bun", "run", "tauri", "dev", "--config", configPath, "--no-dev-server-wait"],
    {
      cwd: appDir,
      detached: true,
      stdio: ["ignore", log, log],
      env: {
        ...process.env,
        DUCKTAPE_TAURI_MCP_SOCKET: socketPath,
        DUCKTAPE_HOME: ducktapeHome,
        DUCKTAPE_NODE_BIN: nodeBin,
        DUCKTAPE_TAURI_DEV_PORT: String(vitePort),
        CARGO_TARGET_DIR: targetDir,
        ...webkit,
      },
    },
  );
  tauri.unref();

  // Record pids up front so an interrupted boot is still cleanable via `down`.
  const manifest = {
    slug,
    worktree: wt,
    branch: gitBranch(wt),
    vitePort,
    viteUrl,
    socketPath,
    ducktapeHome,
    registryPath: registryPathOf(ducktapeHome),
    workspaceId: ws.id,
    workspaceHttpUrl: ws.url,
    display,
    targetDir,
    nodeBin,
    logFile,
    pids: { vite: vite.pid, tauri: tauri.pid, xvfb: xvfbPid, node: nodePid },
  };
  writeManifest(manifest);

  try {
    // Two independent readiness signals: the node's http surface (ours, fast) and
    // the debug socket (the app window — the long pole, gated on the shell build).
    // Wait for BOTH; the socket is what an agent drives, so it must exist.
    process.stderr.write(`· booting '${slug}' (vite ${vitePort}, workspace '${ws.id}')… first shell build can take minutes\n`);
    const upDeadline = Date.now() + 360_000;
    while (Date.now() < upDeadline && !(existsSync(socketPath) && (await nodeAnswers(ws.url)))) await wait(1000);
    if (!(await nodeAnswers(ws.url)))
      throw new Error(`workspace node ${ws.url} never answered.\n--- tauri-dev.log tail ---\n${logTail(logFile)}`);
    if (!existsSync(socketPath))
      throw new Error(`node up but the app's debug socket never appeared (build failed?).\n--- tauri-dev.log tail ---\n${logTail(logFile)}`);

    process.stderr.write(`· '${slug}' is up: window + node '${ws.id}' answering ${ws.url}\n`);
    console.log(manifestPath(slug));
  } catch (err) {
    if (process.env.DUCKTAPE_QA_KEEP) {
      process.stderr.write(`· boot failed — DUCKTAPE_QA_KEEP set, leaving '${slug}' up for inspection (${root})\n`);
    } else {
      process.stderr.write(`· boot failed — tearing down '${slug}'\n`);
      await teardown(manifest);
    }
    throw err;
  }
};

// ── down ────────────────────────────────────────────────

const killGroup = (pid) => {
  if (!pid) return;
  try {
    process.kill(-pid, "SIGTERM"); // detached child leads its own group
  } catch {
    killPid(pid); // group gone but leader may linger
  }
};

const killPid = (pid) => {
  if (!pid) return;
  try {
    process.kill(pid, "SIGTERM");
  } catch {
    /* already gone */
  }
};

// Tear an instance fully down from its manifest — shared by `down` and a failed
// `up`. Order matters (per the tauri-debug skill): tauri CLI first (it respawns a
// crashed app), then vite, then ask the DETACHED workspace node(s) to exit over
// http (no pid crosses that boundary — the port is the node's identity), then the
// private Xvfb, then remove the run root (which holds this instance's DUCKTAPE_HOME).
const teardown = async (m) => {
  killGroup(m.pids?.tauri);
  killGroup(m.pids?.vite);
  await wait(300);
  const urls = new Set(m.workspaceHttpUrl ? [m.workspaceHttpUrl] : []);
  for (const w of (m.ducktapeHome && readRegistry(m.ducktapeHome)?.workspaces) || [])
    urls.add(`http://127.0.0.1:${w.ports.http}`);
  for (const url of urls) {
    try {
      await fetch(`${url}/v1/shutdown`, { method: "POST", signal: AbortSignal.timeout(2000) });
    } catch {
      /* node already down */
    }
  }
  killPid(m.pids?.node); // backup: the node is detached; /v1/shutdown is primary
  killPid(m.pids?.xvfb);
  rmSync(runRoot(m.slug), { recursive: true, force: true });
};

const down = async (arg) => {
  const slug = targetSlug(arg);
  const m = readManifest(slug);
  if (!m) {
    process.stderr.write(`· no instance '${slug}'\n`);
    return;
  }
  await teardown(m);
  process.stderr.write(`· tore down '${slug}' (window, vite, workspace node, Xvfb ${m.display})\n`);
};

// ── list / env ──────────────────────────────────────────

const list = () => {
  if (!existsSync(QA_HOME)) return console.log("(no QA instances)");
  const rows = readdirSync(QA_HOME)
    .map((slug) => readManifest(slug))
    .filter(Boolean);
  if (!rows.length) return console.log("(no QA instances)");
  for (const m of rows)
    console.log(`${m.slug}\tvite ${m.vitePort}\tnode ${m.workspaceHttpUrl || "(booting)"}\tsock ${m.socketPath}`);
};

const printEnv = (arg) => {
  const slug = targetSlug(arg);
  const m = readManifest(slug);
  if (!m) throw new Error(`no running instance '${slug}' — start it with: qa-instance.mjs up`);
  console.log(`export DUCKTAPE_TAURI_MCP_SOCKET=${m.socketPath}`);
  console.log(`export DUCKTAPE_QA_NODE_URL=${m.workspaceHttpUrl || ""}`);
  console.log(`export DISPLAY=${m.display}`);
};

// ── main ────────────────────────────────────────────────

const USAGE = `qa-instance — boot/drive one isolated Ducktape app per worktree
  node app/scripts/qa-instance.mjs up   [<worktree-dir>]
  node app/scripts/qa-instance.mjs down [<worktree-dir|slug>]
  node app/scripts/qa-instance.mjs list
  node app/scripts/qa-instance.mjs env  [<worktree-dir|slug>]`;

const main = async () => {
  const pos = process.argv.slice(2).filter((a) => !a.startsWith("--"));
  const cmd = pos[0];
  switch (cmd) {
    case "up":
      return up(pos[1]);
    case "down":
      return down(pos[1]);
    case "list":
      return list();
    case "env":
      return printEnv(pos[1]);
    default:
      console.log(USAGE);
      if (cmd && cmd !== "help") process.exitCode = 2;
  }
};

main().catch((e) => {
  process.stderr.write(`${e?.message || e}\n`);
  process.exitCode = 1;
});
