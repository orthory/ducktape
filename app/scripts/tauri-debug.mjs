#!/usr/bin/env node
/**
 * Self-contained driver for the live Ducktape window. Talks DIRECTLY to the Rust
 * plugin's Unix socket (/tmp/ducktape-tauri-mcp.sock by default, or
 * DUCKTAPE_TAURI_MCP_SOCKET for worktree dev runs) with its newline-delimited JSON
 * protocol — NO tauri-plugin-mcp-server, no MCP layer, no npm dependency beyond
 * node's built-in `net`. The Rust `tauri-plugin-mcp` plugin (src-tauri, dev-only)
 * does the native screenshot/DOM/window work; this is just a thin shell client
 * over its socket.
 *
 *   wire protocol:  send  {command, payload, id}\n   recv  {success, data?, error?, id}\n
 *
 * The per-command payload shapes mirror what tauri-plugin-mcp-server sent (the
 * non-obvious bits: take_screenshot uses save_to_disk/thumbnail; manage_window
 * routes to {operation:setPosition|setSize|focus|center|...} / list_windows /
 * manage_zoom / manage_devtools / manage_webview_state).
 *
 * Requires the app running as Tauri (`bun run tauri dev`) so the socket exists.
 *
 * Usage:
 *   node scripts/tauri-debug.mjs eval "<js>"                # execute_js in the webview
 *   node scripts/tauri-debug.mjs shot [out.png] [label]     # screenshot (focuses window first; --here to skip)
 *   node scripts/tauri-debug.mjs snap [out.png] [label]     # webview-content snapshot (WKWebView.takeSnapshot; off-screen safe)
 *   node scripts/tauri-debug.mjs win <action> [k=v ...]     # manage_window (focus|center|set_size|set_position|list|...)
 *   node scripts/tauri-debug.mjs cmd <command> '<json>'     # any raw socket command + payload
 */
import net from "node:net";
import os from "node:os";
import { writeFileSync, existsSync, statSync, rmSync } from "node:fs";

const SOCKET =
  process.env.DUCKTAPE_TAURI_MCP_SOCKET ||
  (os.platform() === "win32" ? "\\\\.\\pipe\\tmp\\ducktape-tauri-mcp.sock" : "/tmp/ducktape-tauri-mcp.sock");

const USAGE = `tauri-debug — drive the live Ducktape window over its Unix socket (no MCP, no deps)
  node scripts/tauri-debug.mjs eval "<js>"
  node scripts/tauri-debug.mjs shot [out.png] [label]   (--here = don't move the window)
  node scripts/tauri-debug.mjs snap [out.png] [label]   (webview-content snapshot; works off-screen / other Space)
  node scripts/tauri-debug.mjs win <action> [k=v ...]
  node scripts/tauri-debug.mjs cmd <command> '<json-payload>'`;

function connect() {
  const sock = net.createConnection({ path: SOCKET });
  let buf = "";
  const waiters = new Map();
  sock.on("data", (d) => {
    buf += d;
    let i;
    while ((i = buf.indexOf("\n")) >= 0) {
      const line = buf.slice(0, i);
      buf = buf.slice(i + 1);
      if (!line.trim()) continue;
      let m;
      try { m = JSON.parse(line); } catch { continue; }
      const id = m.id && waiters.has(m.id) ? m.id : [...waiters.keys()][0];
      if (id && waiters.has(id)) { const cb = waiters.get(id); waiters.delete(id); cb(m); }
    }
  });
  const ready = new Promise((res, rej) => { sock.once("connect", res); sock.once("error", rej); });
  const send = (command, payload = {}) =>
    new Promise((res, rej) => {
      const id = Date.now().toString() + Math.random().toString(36).slice(2);
      const timeout = setTimeout(() => {
        if (waiters.has(id)) {
          waiters.delete(id);
          rej(new Error("request timed out"));
        }
      }, 30000);
      waiters.set(id, (m) => {
        clearTimeout(timeout);
        return m.success ? res(m.data) : rej(new Error(m.error || "command failed"));
      });
      sock.write(JSON.stringify({ command, payload, id }) + "\n");
    });
  return { sock, ready, send };
}

// the screenshot reply's `data` is usually { data: "<base64 or data:URI>", filePath? }
const pickBase64 = (data) => {
  const s = typeof data === "string" ? data : data && (data.data || data.image || data.base64 || data.thumbnail);
  if (typeof s !== "string") return null;
  return s.startsWith("data:") ? s.split(",")[1] : s; // strip any data:image/...;base64, prefix
};

// manage_window action → the actual socket command + payload
const WIN_OP = { focus: "focus", minimize: "minimize", maximize: "maximize", unmaximize: "unmaximize", close: "close", show: "show", hide: "hide", set_position: "setPosition", set_size: "setSize", center: "center", toggle_fullscreen: "toggleFullscreen" };
function manageWindow(send, action, label = "main", extra = {}) {
  if (action === "list") return send("list_windows", {});
  if (action === "set_zoom" || action === "get_zoom") return send("manage_zoom", { action: action === "set_zoom" ? "set" : "get", window_label: label, ...extra });
  if (action === "open_devtools" || action === "close_devtools" || action === "is_devtools_open") {
    const a = action === "open_devtools" ? "open" : action === "close_devtools" ? "close" : "is_open";
    return send("manage_devtools", { action: a, window_label: label });
  }
  if (action === "clear_browsing_data" || action === "set_background_color" || action === "get_bounds" || action === "set_auto_resize") {
    return send("manage_webview_state", { action, window_label: label, ...extra });
  }
  const operation = WIN_OP[action];
  if (!operation) throw new Error("unknown window action: " + action);
  return send("manage_window", { operation, window_label: label, ...extra });
}

async function main() {
  const argv = process.argv.slice(2);
  const flags = new Set(argv.filter((a) => a.startsWith("--")));
  const pos = argv.filter((a) => !a.startsWith("--"));
  const cmd = pos[0];
  if (!cmd || cmd === "help") { console.log(USAGE); return; }

  const { sock, ready, send } = connect();
  try { await ready; } catch (e) {
    console.error(`cannot connect to ${SOCKET} — is \`bun run tauri dev\` running?  (${e.message})`);
    process.exit(1);
  }
  try {
    if (cmd === "eval") {
      console.log(JSON.stringify(await send("execute_js", { code: pos.slice(1).join(" "), window_label: "main" })));
    } else if (cmd === "shot") {
      const out = pos[1] || "/tmp/tauri-shot.png";
      const label = pos[2] || "main";
      if (!flags.has("--here")) {
        await manageWindow(send, "set_position", label, { x: 80, y: 80 }).catch(() => {});
        await manageWindow(send, "focus", label).catch(() => {});
        await new Promise((r) => setTimeout(r, 700));
      }
      const data = await send("take_screenshot", { window_label: label, save_to_disk: false, thumbnail: false, max_width: 1400, quality: 92 });
      const img = pickBase64(data);
      if (img) { const b = Buffer.from(img, "base64"); writeFileSync(out, b); console.log(`saved ${out} (${b.length} bytes)`); }
      else console.log(JSON.stringify(data).slice(0, 400));
    } else if (cmd === "snap") {
      // Capture the webview's rendered content via the dev-only Rust command
      // (WKWebView.takeSnapshot). Unlike `shot` (screen-region capture), this is
      // independent of window position / Space / screen-recording permission, so
      // it works on headless + virtual-display dev boxes. The command writes the
      // PNG to `out` atomically; `execute_js` does not await the invoke's Promise,
      // so we fire it and poll for the file.
      const out = pos[1] || "/tmp/tauri-snap.png";
      const label = pos[2] || "main";
      if (existsSync(out)) rmSync(out);
      const esc = JSON.stringify(out);
      const code = `(() => {
        const inv = (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke)
          || (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke);
        if (!inv) return "no-invoke";
        inv("debug_capture_webview", { path: ${esc} }).then(
          () => {}, (e) => { window.__snapErr = String(e); },
        );
        return "fired";
      })()`;
      const fired = await send("execute_js", { code, window_label: label });
      if (fired && fired.result === "no-invoke")
        throw new Error("invoke() not reachable in the webview (is this a debug build?)");
      // Poll up to 12s for the atomically-written file.
      const deadline = Date.now() + 12000;
      let ok = false;
      while (Date.now() < deadline) {
        if (existsSync(out) && statSync(out).size > 0) { ok = true; break; }
        await new Promise((r) => setTimeout(r, 150));
      }
      if (ok) console.log(`saved ${out} (${statSync(out).size} bytes)`);
      else {
        const err = await send("execute_js", { code: "window.__snapErr || 'timeout (no file written)'", window_label: label }).catch(() => null);
        throw new Error(`snapshot failed: ${err?.result || "timed out"}`);
      }
    } else if (cmd === "win") {
      const extra = {};
      for (const kv of pos.slice(2)) { const [k, v] = kv.split("="); extra[k] = /^-?\d+(\.\d+)?$/.test(v) ? Number(v) : v; }
      console.log(JSON.stringify(await manageWindow(send, pos[1], extra.window_label || "main", extra)));
    } else if (cmd === "cmd") {
      console.log(JSON.stringify(await send(pos[1], pos[2] ? JSON.parse(pos[2]) : {})));
    } else {
      console.error(`unknown command: ${cmd}\n\n${USAGE}`);
      process.exitCode = 2;
    }
  } catch (e) {
    console.error(String(e?.message || e));
    process.exitCode = 1;
  } finally {
    sock.end();
  }
}

main();
