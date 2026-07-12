# Design: adopt `tauri-plugin-agent` as the Ducktape debug bridge

**Date:** 2026-07-07
**Status:** Approved for planning
**Branch:** `feat/tauri-agent-debug-stack`

## Goal

Replace the third-party `tauri-plugin-mcp` (P3GLEG) debug bridge with our own
[`tauri-agent-plugin`](https://github.com/byeongsu-hong/tauri-agent-plugin)
(crate `tauri-plugin-agent`, npm `@byeongsu-hong/tauri-plugin-agent`), and adopt
it **fully and natively** across the debug stack: the Rust plugin, the guest
instrumentation, the driver, the fleet, both skills, and Claude Code's MCP
config. No dual stack, no bespoke protocol driver left behind.

Chosen shape (decided in brainstorming):

- **Cutover:** replace `tauri-plugin-mcp` outright — it is our own plugin and a
  strict superset, so there is no reason to carry two bridges.
- **Depth:** full native adoption — retire `app/scripts/tauri-debug.mjs`, drive
  the app through the plugin's own `tauri-agent` CLI and `tauri-agent-mcp`
  server, and rewrite both skills around that surface.
- **Dependency shape:** released crates.io/npm packages at `0.0.1`; no vendored
  submodule is needed.

## Why this plugin

`tauri-plugin-agent` is a strict superset of what `tauri-plugin-mcp` gave us:

| | old: `tauri-plugin-mcp` | new: `tauri-plugin-agent` |
|---|---|---|
| Transport | singleton unix socket, NDJSON `{command,payload,id}` | JSON-RPC 2.0, inline loopback TCP (ephemeral port) |
| Discovery | fixed socket path (env override) | app-scoped endpoint registry keyed by Tauri identifier |
| DOM | raw `query_page`/`eval` | semantic tree + `@ref` find/click/inspect, plus `eval` |
| Monitoring | none | console logs, uncaught errors, `unhandledrejection`, fetch metadata, storage, cookies, SPA route |
| Input | click / keys | click, hover, focus, blur, scroll, drag, fill, select, check, press+modifiers |
| Screenshot | native window capture (fails under Xvfb w/o WM) | DOM-rendered SVG (deterministic, WM-free) + native macOS fallback |
| MCP | needs an external server binary | ships stdio MCP server (`tauri-agent-mcp`) |
| Recording | none | action recording/playback |
| Static | none | `--from-html` prototyping (drive a screen with no running app) |
| Release | dev-only `cfg` at registration | dev-only + explicit `allowReleaseSocket` opt-in |

Two wins matter most for *our* stack:

1. **Endpoint registry kills the socket juggling.** The plugin publishes an
   endpoint descriptor to `${XDG_RUNTIME_DIR|TMPDIR|TMP}/tauri-agent/<app-id>/endpoint.json`
   (`app-id` = the Tauri `identifier`, i.e. `com.ducktape.app`). Consumers
   discover the live app by `--app com.ducktape.app`. Per-worktree isolation in
   the fleet becomes a per-instance runtime base (`TMPDIR`), not a
   hand-assigned `DUCKTAPE_TAURI_MCP_SOCKET=/tmp/tauri-mcp-<id>.sock`.
2. **DOM-SVG screenshots work headless without a window manager.** Today the
   fleet/tauri-debug skills fall back to `import -window root` because the old
   plugin's `take_screenshot` needs `_NET_CLIENT_LIST` (absent under bare Xvfb).
   `tauri-agent shot --backend dom` renders the webview to SVG in-process — no
   WM, no framebuffer grab. (Native PNG capture stays macOS-only; on Linux we
   use DOM SVG.)

## Current wiring (what we are replacing)

5 wiring points:

1. `app/src-tauri/Cargo.toml:28` — `tauri-plugin-mcp = { git = ".../P3GLEG/tauri-plugin-mcp" }`
2. `app/src-tauri/src/main.rs:68-85` — `tauri_plugin_mcp::init_with_config(...)`
   under `#[cfg(all(debug_assertions, desktop))]`, socket from
   `DUCKTAPE_TAURI_MCP_SOCKET` (default `/tmp/ducktape-tauri-mcp.sock`)
3. `app/package.json:42` — devDep `tauri-plugin-mcp ^0.1.0`
4. `app/src/main.tsx:25-30` — dev-only `import("tauri-plugin-mcp").setupPluginListeners()`
5. `app/scripts/tauri-debug.mjs` — 182-line NDJSON socket driver (`eval`/`shot`/`cmd`)

4 consumers:

- `skills/tauri-debug/SKILL.md` — single running app
- `skills/qa/SKILL.md` — per-worktree fleet instances
- `ops/fleet.sh` — spawns each worktree app with `DUCKTAPE_TAURI_MCP_SOCKET`;
  the dashboard "up" gate is `existsSync("/tmp/tauri-mcp-<id>.sock")`;
  teardown does `pkill -f tauri-mcp-$id.sock` + `rm -f /tmp/tauri-mcp-$id.sock`
- `docs/superpowers/specs/2026-07-07-tauri-dev-error-handling-design.md` — one
  passing mention

## Architecture of the new integration

### Discovery model (the core mechanism)

```
app (Rust plugin)                         consumer (CLI / MCP)
  inline loopback TCP :<ephemeral>           reads --app com.ducktape.app
  writes  <runtime>/tauri-agent/     <---->  reads   <runtime>/tauri-agent/
          com.ducktape.app/endpoint.json             com.ducktape.app/endpoint.json
  runtime = XDG_RUNTIME_DIR|TMPDIR|TMP       runtime = XDG_RUNTIME_DIR|TMPDIR|TMP
```

Both sides resolve the runtime base from the same env precedence
(`XDG_RUNTIME_DIR` → `TMPDIR` → `TEMP` → `TMP` → OS temp). Isolating the base
per process isolates the registry — the single lever the fleet needs.

**The isolation lever is `XDG_RUNTIME_DIR`, not `TMPDIR`.** `XDG_RUNTIME_DIR` is
first in the precedence chain and is typically already set on a Linux desktop
session (e.g. `/run/user/1000`); a per-instance `TMPDIR` would be silently
overridden by it. So the fleet must set `XDG_RUNTIME_DIR` per instance to move
the registry, and the app and its driver must share the same value.

- **Single running app (tauri-debug):** default runtime base (whatever the
  shell's `XDG_RUNTIME_DIR`/`TMP` resolve to — the app and the CLI inherit the
  same env, so they agree); app id `com.ducktape.app`. One live app, discovered
  by id.
- **Fleet (qa):** each worktree app is launched with
  `XDG_RUNTIME_DIR=$STATE/$id` so its registry lives under
  `$STATE/$id/tauri-agent/com.ducktape.app/`. The QA agent drives it after
  exporting the *same* `XDG_RUNTIME_DIR=$STATE/$id` + `--app com.ducktape.app`.
  The app id is constant across worktrees; the runtime base is what differs.

### Component changes

**Package dependency.** Use the released crates.io/npm packages at `0.0.1`.
- Rust consumes `tauri-plugin-agent = "0.0.1"` from crates.io.
- The CLI/MCP run through the npm package binaries installed under `app/`
  (`tauri-agent` / `tauri-agent-mcp`) via the repo shims.
- The guest binding imports `@byeongsu-hong/tauri-plugin-agent` directly; the
  package carries the compiled JS and types. The import remains dev-only and is
  tree-shaken out of release.

**Rust (`app/src-tauri/`).**
- `Cargo.toml`: remove `tauri-plugin-mcp`; add
  `tauri-plugin-agent = "0.0.1"`.
- `src/main.rs`: replace the `tauri_plugin_mcp::init_with_config(...)` block with
  `builder = builder.plugin(tauri_plugin_agent::init());`, **keeping the exact
  `#[cfg(all(debug_assertions, desktop))]` guard**. Release still registers
  nothing and `allowReleaseSocket` stays off — the current "release opens
  nothing" promise is preserved. Update the surrounding comment.
- `tauri.conf.json`: add
  ```json
  "plugins": { "agent": { "inlineServer": {
    "enabled": true, "host": "127.0.0.1", "port": 0, "publishEndpoint": true
  } } }
  ```
  The block is inert in release because the plugin is not registered there.
- `capabilities/default.json`: add `"agent:default"` to `permissions`. The new
  plugin is command-based (the old one was socket-only and needed no capability).
  The `windows` list already covers `main`, `tray`, `huddle`.

**Guest (`app/`).**
- `src/main.tsx`: replace the dev-only `setupPluginListeners()` block with
  ```ts
  if (import.meta.env.DEV) {
    void (async () => {
      const [{ WebviewAgentInstrumentation }, { getCurrentWindow }] = await Promise.all([
        import("@byeongsu-hong/tauri-plugin-agent"),
        import("@tauri-apps/api/window"),
      ]);
      new WebviewAgentInstrumentation({ windowLabel: getCurrentWindow().label }).install();
    })().catch(() => {});
  }
  ```
  Installs one instrumentation per webview, labelled by the real window label
  (`main`/`tray`/`huddle`), still gated by `import.meta.env.DEV`.
- `vite.config.ts`: no alias is required; resolve the package normally.
- `package.json`: remove the `tauri-plugin-mcp` devDep; add
  `@byeongsu-hong/tauri-plugin-agent = "0.0.1"` as a dev dependency.

**Driver → native CLI.**
- Delete `app/scripts/tauri-debug.mjs`.
- Add a thin **path shim** `app/scripts/tauri-agent` (executable) that execs
  the package `tauri-agent` binary through `bunx --no-install` and defaults
  `--app com.ducktape.app` when no `--app`/`--from-html`/`--port` is given. This
  is a path/ergonomics shim, not a reimplemented protocol — the native CLI does
  all the work. Skills call `app/scripts/tauri-agent tree`,
  `... shot out.svg`, `... find --role button --name Forge`, etc.

**MCP (Claude Code).**
- Add repo-root `.mcp.json`:
  ```json
  { "mcpServers": { "tauri-agent": {
    "command": "app/scripts/tauri-agent-mcp",
    "args": []
  } } }
  ```
  Gives the main agent native `tauri_*` tools (`tauri_tree`, `tauri_find`,
  `tauri_click`, `tauri_shot`, `tauri_logs`, ...) for the single running app;
  each call passes `app: "com.ducktape.app"`. The fleet keeps using the CLI
  (per-instance `TMPDIR`), which is the natural fit for many isolated instances.

**Consumers / docs.**
- `ops/fleet.sh`:
  - launch: replace `DUCKTAPE_TAURI_MCP_SOCKET="$mcp"` with
    `XDG_RUNTIME_DIR="$STATE/$id"` (drop the `/tmp/tauri-mcp-$id.sock`
    variable). Ensure `$STATE/$id` exists and is `0700` before launch (XDG
    runtime dirs are expected to be user-private).
  - dashboard "up" gate: `existsSync("$STATE/$id/tauri-agent/com.ducktape.app/endpoint.json")`.
  - teardown: drop `pkill -f tauri-mcp-$id.sock` and `rm -f /tmp/tauri-mcp-$id.sock`;
    the registry lives under the per-instance state dir and is removed with it.
- `skills/tauri-debug/SKILL.md`: rewrite around the `tauri-agent` CLI + the MCP
  tools for the single app. Document DOM-SVG `shot` as the headless capture (no
  WM hack), native PNG as macOS-only, and the `--app com.ducktape.app`
  discovery. Update the "what's wired" table.
- `skills/qa/SKILL.md`: rewrite around per-worktree `XDG_RUNTIME_DIR=$STATE/$id`
  + `--app com.ducktape.app`; the "up" gate is the endpoint file; assertions go
  through `tauri-agent tree/find/eval`.
- `docs/superpowers/specs/2026-07-07-tauri-dev-error-handling-design.md`: update
  the one socket mention to the endpoint model.

## Release-safety invariant

Release builds must open no debugger surface. Preserved by two independent
guards: (1) the plugin is registered only under
`#[cfg(all(debug_assertions, desktop))]`, so a release binary never calls
`init()`; (2) even if registered, the inline server refuses to bind in a release
build unless `allowReleaseSocket` is set, which we never set. The guest
instrumentation stays behind `import.meta.env.DEV`. `agent:default` in
capabilities is harmless in release because the commands are never invoked (no
guest install, no server).

## Non-goals

- Publishing the plugin to npm/crates.io (future; unblocks versioned deps).
- Wiring the MCP server per-fleet-tile (the CLI covers the fleet cleanly).
- Using `--from-html` static prototyping in CI (available, not adopted here).
- Native (pixel) screenshots on Linux (DOM SVG is the headless path).
- Any change to daemon lifecycle, `/v1` seeding, or the Xvfb bring-up recipe
  beyond the screenshot/discovery deltas above.

## Verification

Headless (Xvfb) bring-up per the tauri-debug recipe, then:

1. `tauri dev` builds with the new dep; the app boots.
2. `<runtime>/tauri-agent/com.ducktape.app/endpoint.json` appears with a TCP
   descriptor.
3. `app/scripts/tauri-agent tree --app com.ducktape.app` returns a semantic tree
   of the live console; `find --role button` + `click @ref` drives navigation;
   `shot out.svg` writes a DOM SVG without a WM.
4. `logs`, `eval`, `wait` behave against the live webview.
5. The `tauri-agent` MCP server starts under `bun` and Claude Code lists its
   `tauri_*` tools; a `tauri_tree` call against `com.ducktape.app` succeeds.
6. One fleet tile: launched with `TMPDIR=$STATE/$id`, its endpoint file lands
   under the per-instance dir, the dashboard shows it "up", and a driver call
   scoped to that `TMPDIR` reaches only that instance.
7. `cargo check -p app` (or the app crate) is green; release build still opens
   no socket (spot-check: no `init()` under `not(debug_assertions)`).

## Blast radius summary

- **Add:** `.mcp.json`, `app/scripts/tauri-agent`, and npm/crates.io package
  dependencies for `tauri-plugin-agent` / `@byeongsu-hong/tauri-plugin-agent`.
- **Modify:** `app/src-tauri/Cargo.toml`, `.../src/main.rs`,
  `.../tauri.conf.json`, `.../capabilities/default.json`, `app/src/main.tsx`,
  `app/vite.config.ts`, `app/package.json` (+ lockfile), `ops/fleet.sh`,
  `skills/tauri-debug/SKILL.md`, `skills/qa/SKILL.md`, the tauri-dev spec.
- **Delete:** `app/scripts/tauri-debug.mjs`.
