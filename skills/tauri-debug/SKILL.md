---
name: tauri-debug
description: Use when verifying a UI/design change in the real Ducktape desktop window, reproducing a bug in the native window, or when the vite browser preview isn't enough (native WebKitGTK chrome, the LOCAL daemon-managing shell, real daemon-backed data). Drives the live app — screenshot the window, run JS in the webview, inspect/drive the DOM, semantic tree + find/click — over a dev-only app-scoped endpoint. Includes the headless (Xvfb) recipe for a box with no display.
---

# Tauri Debug (drive the live Ducktape app)

The desktop shell ships a **dev-only** debug bridge: `tauri-plugin-agent`
(crates.io/npm release `0.0.1`), registered under
`#[cfg(all(debug_assertions, desktop))]` in `app/src-tauri/src/main.rs`. It runs
an inline loopback-TCP debugger and publishes an **app-scoped endpoint registry**
so a driver can screenshot the window, snapshot the semantic tree, find/click by
role+name, run JS in the webview, and read logs. Drive it with the package
`tauri-agent` CLI via the thin shim `app/scripts/tauri-agent` (defaults
`--app com.ducktape.app`), or with the native `tauri_*` MCP tools.

Use this instead of the vite browser preview when you need the **actual Tauri
window** — WebKitGTK native chrome, the `LOCAL` shell that spawns/adopts the
selected workspace's `ducktape-node`, real node state over `/v1` — rather than
the web build
(which is `isTauri()`-false: no daemon lifecycle, `REMOTE` badge, and the
onboarding/workspace commands throw).

## What's wired (do not re-add)

| Layer | Where |
|---|---|
| Rust dep | `app/src-tauri/Cargo.toml` — `tauri-plugin-agent = "0.0.1"` |
| Rust plugin | `app/src-tauri/src/main.rs` — `tauri_plugin_agent::init()` under `cfg(all(debug_assertions, desktop))` |
| Inline server | `app/src-tauri/tauri.conf.json` — `plugins.agent.inlineServer` (`enabled`, `host`, `port:0`, `publishEndpoint`) |
| Capability | `app/src-tauri/capabilities/default.json` — `agent:default` (windows main/tray/huddle) |
| Guest JS | `app/src/main.tsx` — `new WebviewAgentInstrumentation({ windowLabel }).install()` under `import.meta.env.DEV`; loaded from `@byeongsu-hong/tauri-plugin-agent@0.0.1` |
| Driver | `app/scripts/tauri-agent` — thin shim over the package `tauri-agent` binary |
| MCP | `.mcp.json` — server `tauri-agent` via `app/scripts/tauri-agent-mcp`, native `tauri_*` tools |

## Run it

Run the app as Tauri (the plugin is Rust-side — `bun run dev` / vite alone is
NOT enough):

```bash
cd app && bun run tauri dev
```

Then drive the live window (from any shell — no MCP/harness setup):

```bash
app/scripts/tauri-agent attach                                # connect + list windows
app/scripts/tauri-agent tree                                  # compact semantic tree (@refs)
app/scripts/tauri-agent find --role button --name Forge       # find a ref by role+name
app/scripts/tauri-agent click @3                              # drive input by ref
app/scripts/tauri-agent eval "document.title"                 # run JS in the webview
app/scripts/tauri-agent shot /tmp/app.svg                     # DOM-SVG screenshot (headless-safe)
app/scripts/tauri-agent logs                                  # captured console/error logs
```

The shim appends `--app com.ducktape.app` unless you pass `--app`/`--from-html`/
`--port`/`--host`. In Claude Code, the same surface is available as native MCP
tools (`tauri_tree`, `tauri_find`, `tauri_click`, `tauri_shot`, ...) — pass
`{ "app": "com.ducktape.app" }`.

## Discovery (no /tmp singleton socket)

The plugin publishes a TCP endpoint descriptor to
`${XDG_RUNTIME_DIR|TMPDIR|TMP}/tauri-agent/com.ducktape.app/endpoint.json`. The
CLI/MCP find the live app by `--app com.ducktape.app`, resolving the runtime base
from the **same** env. For a single app they inherit the same shell env and
agree automatically; to isolate parallel apps, give each its own
`XDG_RUNTIME_DIR` (this is exactly what the fleet does — see [[qa]]).

## Headless (no display) — the Xvfb recipe

On a box with no display, WebKitGTK still needs an X server. Run under a virtual
framebuffer with the WebKit headless flags (no GPU/DMABUF) and a session bus:

```bash
Xvfb :99 -screen 0 1400x900x24 -nolisten tcp &
cd app && DISPLAY=:99 \
  WEBKIT_DISABLE_DMABUF_RENDERER=1 WEBKIT_DISABLE_COMPOSITING_MODE=1 \
  LIBGL_ALWAYS_SOFTWARE=1 GDK_BACKEND=x11 \
  dbus-run-session -- bun run tauri dev
```

System deps for the headless run (Debian, as root — the box has no `sudo`, use
`su -`): `libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev
libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev build-essential
pkg-config xvfb dbus-x11 imagemagick`.

## Screenshots — DOM-SVG is the headless win

`tauri-agent shot /tmp/app.svg` renders the webview DOM to a deterministic SVG
**in-process** — it needs no window manager, so it works under bare Xvfb (this
replaces the old plugin's `_NET_CLIENT_LIST` capture, which failed with
`Failed to get window list`). `--backend native` (PNG via `NSWindow`) is
**macOS-only**; on Linux stay on DOM SVG. `eval` / `tree` confirm state
independent of any capture. For a raw framebuffer PNG as a visual sanity check
you can still `DISPLAY=:99 import -window root /tmp/fb.png` (imagemagick).

## Notes & caveats

- **Dev only.** The Rust plugin is gated by `cfg(all(debug_assertions, desktop))`
  and the guest by `import.meta.env.DEV`; a release build registers nothing and
  the inline server refuses to bind without `allowReleaseSocket` (never set).
  Don't move the registration out of those guards.
- **Node.** `workspace_select` asks the bounded `NodeControl` actor in
  `app/src-tauri/src/daemon.rs` to spawn/adopt the selected workspace's
  `ducktape-node`; its loopback port comes from `~/.ducktape/registry.json`, not
  a fixed `:8844`. The binary comes from `DUCKTAPE_NODE_BIN` or the sibling next
  to the desktop bin. A stale/old binary shows the honest `REMOTE` + "could not
  reach the node" surface — stage a fresh one with `bun run sidecar`, or build
  `cargo build -p node-bin` and point `DUCKTAPE_NODE_BIN` at that executable.
  Seed state over `/v1/submit`; the app re-renders on each finalized block.
- After Rust / `tauri.conf.json` changes, `tauri dev` rebuilds + restarts; the
  endpoint file drops and reappears — wait for a fresh
  `.../tauri-agent/com.ducktape.app/endpoint.json` before re-driving. Frontend
  edits hot-reload, no restart.
- **Teardown.** Kill the `tauri` CLI before the app (it respawns a crashed app),
  then the app, `Xvfb`, and the detached `ducktape-node` (or `POST /v1/shutdown`).
