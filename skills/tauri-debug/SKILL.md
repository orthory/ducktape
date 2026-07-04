---
name: tauri-debug
description: Use when verifying a UI/design change in the real Ducktape desktop window, reproducing a bug in the native window, or when the vite browser preview isn't enough (native WebKitGTK chrome, the LOCAL daemon-managing shell, real daemon-backed data). Drives the live app — screenshot the window, run JS in the webview, inspect/drive the DOM — over a dev-only unix socket. Includes the headless (Xvfb) recipe for a box with no display.
---

# Tauri Debug (drive the live Ducktape app)

The desktop shell ships a **dev-only** debug bridge: `tauri-plugin-mcp`,
registered under `#[cfg(all(debug_assertions, desktop))]` in
`app/src-tauri/src/main.rs`, opens a local unix socket (`/tmp/ducktape-tauri-mcp.sock`,
or `DUCKTAPE_TAURI_MCP_SOCKET`) implementing the native ops — screenshot the
window, run JS in the webview, inspect the DOM, simulate input. Drive it with
the dependency-free helper `app/scripts/tauri-debug.mjs` (newline-delimited JSON
over the socket; node built-ins only).

Use this instead of the vite browser preview when you need the **actual Tauri
window** — WebKitGTK native chrome, the `LOCAL` shell that spawns/adopts its own
`ducktape-noded`, real daemon state over `/v1` — rather than the web build
(which is `isTauri()`-false: no daemon lifecycle, `REMOTE` badge, and the
onboarding/workspace commands throw).

## What's wired (do not re-add)

| Layer | Where |
|---|---|
| Rust plugin | `app/src-tauri/src/main.rs` — `tauri_plugin_mcp::init_with_config(...)` under `cfg(all(debug_assertions, desktop))`; socket from `DUCKTAPE_TAURI_MCP_SOCKET`, default `/tmp/ducktape-tauri-mcp.sock` |
| Rust dep | `app/src-tauri/Cargo.toml` — `tauri-plugin-mcp` (git) |
| Guest JS | `app/src/main.tsx` — `setupPluginListeners()` dynamically imported under `import.meta.env.DEV` (needed for the DOM/JS commands; screenshots work without it) |
| npm dep | `app/package.json` (devDependencies) — `tauri-plugin-mcp` (guest binding only) |
| Driver | `app/scripts/tauri-debug.mjs` — socket-direct CLI |

## Run it

Run the app as Tauri (the plugin is Rust-side — `bun run dev` / vite alone is
NOT enough):

```bash
cd app && bun run tauri dev
```

Then drive the live window (from any shell — no MCP/harness setup):

```bash
node app/scripts/tauri-debug.mjs eval "document.title"                            # run JS in the webview
node app/scripts/tauri-debug.mjs eval "[...document.querySelectorAll('button')].find(b=>b.textContent.trim()==='Forge')?.click()"   # navigate by DOM
node app/scripts/tauri-debug.mjs shot out.png                                     # native screenshot
node app/scripts/tauri-debug.mjs cmd query_page '{"mode":"app_info"}'             # any raw socket command
```

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

To skip the slow `beforeDevCommand` (release `node-bin` build) on a smoke run,
start vite yourself and pass a config override that nulls it:

```bash
DUCKTAPE_TAURI_DEV_PORT=1430 bun run dev &        # vite on :1430 (strict)
# tauri config override: {"build":{"beforeDevCommand":null,"devUrl":"http://localhost:1430"}}
bunx tauri dev --config /tmp/no-before.json --no-dev-server-wait
```

System deps for the headless run (Debian, as root — the box has no `sudo`, use
`su -`): `libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev
libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev build-essential
pkg-config xvfb dbus-x11 imagemagick`.

## Capturing the screen headless — use `import`, not the plugin `shot`

The plugin's `take_screenshot` (helper `shot`) enumerates windows via
`_NET_CLIENT_LIST`, which **fails under Xvfb with no window manager**
(`Failed to get window list`). Capture the framebuffer directly instead:

```bash
DISPLAY=:99 import -window root /tmp/out.png     # imagemagick — reliable under Xvfb
```

Use `eval` / `query_page` to *confirm* state (read the DOM) regardless — they
don't depend on a window manager or screen capture, so they work when a capture
looks wrong.

## Notes & caveats

- **Dev only.** The Rust plugin is gated by `cfg(all(debug_assertions, desktop))`
  and the guest by `import.meta.env.DEV`; a release build never opens the socket.
  Don't move the registration out of those guards.
- **Daemon.** The desktop shell spawns/adopts its own `ducktape-noded` on
  `127.0.0.1:8844` (`daemon_spawn` in `app/src-tauri/src/daemon.rs`). Seed state
  by driving that daemon over `/v1/submit` — the app re-queries and re-renders on
  each finalized block, so a curl write shows up in the live window.
- After Rust / `tauri.conf.json` changes, `tauri dev` rebuilds + restarts; the
  socket drops and reappears — wait for a fresh `/tmp/ducktape-tauri-mcp.sock` before
  re-driving. Frontend edits hot-reload, no restart.
- **Teardown.** Kill the `tauri` CLI before the app (it respawns a crashed app),
  then the app, `Xvfb`, and the detached `ducktape-noded` (or `POST /v1/shutdown`).
