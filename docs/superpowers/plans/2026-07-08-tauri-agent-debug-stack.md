# tauri-agent debug stack Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the third-party `tauri-plugin-mcp` debug bridge with our own `tauri-plugin-agent`, adopted natively across the Rust plugin, guest instrumentation, driver, fleet, MCP config, and both debug skills.

**Architecture:** Vendor the plugin as a pinned git submodule. The Rust plugin (dev-only) runs an inline loopback TCP debugger and publishes an app-scoped endpoint registry under `${XDG_RUNTIME_DIR|TMPDIR|TMP}/tauri-agent/com.ducktape.app/endpoint.json`. Consumers (the `tauri-agent` CLI and the `tauri-agent-mcp` stdio server) discover the live app by `--app com.ducktape.app`; the fleet isolates parallel worktree apps by giving each its own `XDG_RUNTIME_DIR`.

**Tech Stack:** Tauri v2, Rust, React 19 + Vite 8, Bun, TypeScript 6, git submodule.

## Global Constraints

- App Tauri identifier / discovery id: `com.ducktape.app` (verbatim).
- Windows: `main`, `tray`, `huddle`.
- Debug bridge stays **dev-only**: Rust registration under `#[cfg(all(debug_assertions, desktop))]`; guest install behind `import.meta.env.DEV`; never set `allowReleaseSocket`.
- Submodule pin: `third_party/tauri-agent-plugin` at commit `897f393e9100f88253509068da57ca6dfad345e3` (or newer HEAD at add time — record the actual SHA).
- Do NOT run `cargo fmt --all`; format only touched code. Lint gate is per-crate `cargo clippy -p <crate> --tests --no-deps`.
- Rust crate path from `app/src-tauri/`: submodule is `../../third_party/tauri-agent-plugin`.
- The files crate wasm gate (`cargo check -p files --no-default-features`) is unaffected — this task does not touch `files`.

---

### Task 1: Vendor the plugin as a git submodule

**Files:**
- Create: `.gitmodules` (git-managed), `third_party/tauri-agent-plugin` (submodule tree)

**Interfaces:**
- Produces: the vendored plugin at `third_party/tauri-agent-plugin` with `Cargo.toml` (crate `tauri-plugin-agent`), `bin/tauri-agent.ts`, `bin/tauri-agent-mcp.ts`, `guest-js/index.ts`, `permissions/`.

- [ ] **Step 1: Add the submodule pinned to HEAD**

Run from repo root:
```bash
git submodule add https://github.com/byeongsu-hong/tauri-agent-plugin third_party/tauri-agent-plugin
git -C third_party/tauri-agent-plugin rev-parse HEAD
```
Expected: clones into `third_party/tauri-agent-plugin`; prints a SHA (expect `897f393e9100f88253509068da57ca6dfad345e3` unless upstream advanced).

- [ ] **Step 2: Verify the CLI runs straight off the submodule TS (no build)**

```bash
bun third_party/tauri-agent-plugin/bin/tauri-agent.ts --help
```
Expected: prints the `tauri-agent` usage/commands (`tree`, `find`, `click`, `shot`, ...) with exit 0. If bun reports missing deps, run `bun install --cwd third_party/tauri-agent-plugin` once and retry.

- [ ] **Step 3: Commit**

```bash
git add .gitmodules third_party/tauri-agent-plugin
git commit -m "chore(debug): vendor tauri-agent-plugin as a submodule"
```

---

### Task 2: Swap the Rust plugin

**Files:**
- Modify: `app/src-tauri/Cargo.toml:28`
- Modify: `app/src-tauri/src/main.rs:68-85`
- Modify: `app/src-tauri/tauri.conf.json` (add `plugins.agent`)
- Modify: `app/src-tauri/capabilities/default.json` (add `agent:default`)

**Interfaces:**
- Consumes: the submodule crate from Task 1.
- Produces: a dev build that registers `tauri_plugin_agent` and, at runtime, writes `endpoint.json` for `com.ducktape.app`.

- [ ] **Step 1: Swap the Cargo dependency**

In `app/src-tauri/Cargo.toml`, replace:
```toml
tauri-plugin-mcp = { git = "https://github.com/P3GLEG/tauri-plugin-mcp" }
```
with:
```toml
tauri-plugin-agent = { path = "../../third_party/tauri-agent-plugin" }
```

- [ ] **Step 2: Swap the plugin registration**

In `app/src-tauri/src/main.rs`, replace the current block (lines 68-85, the comment through the `#[cfg(...)] { ... }`) with:
```rust
    // dev-only debug bridge (tauri-plugin-agent): registers our agent debugger
    // so the `tauri-agent` CLI / MCP server can drive the real native UI —
    // semantic tree, input, DOM-SVG screenshots, logs — over an app-scoped
    // endpoint registry. Gated to debug + desktop; a release runtime never
    // registers it (and the inline server refuses to bind without the
    // allowReleaseSocket opt-in, which we never set). Inline-server config lives
    // in tauri.conf.json under `plugins.agent`. The endpoint publishes to
    // ${XDG_RUNTIME_DIR|TMPDIR|TMP}/tauri-agent/com.ducktape.app/endpoint.json;
    // set XDG_RUNTIME_DIR per instance to isolate parallel worktree apps.
    #[cfg(all(debug_assertions, desktop))]
    {
        builder = builder.plugin(tauri_plugin_agent::init());
    }
```

- [ ] **Step 3: Enable the inline server in tauri.conf.json**

In `app/src-tauri/tauri.conf.json`, add a top-level `"plugins"` key (a sibling of `"app"` and `"bundle"`). Insert it after the `"app": { ... }` block:
```json
  "plugins": {
    "agent": {
      "inlineServer": {
        "enabled": true,
        "host": "127.0.0.1",
        "port": 0,
        "publishEndpoint": true
      }
    }
  },
```

- [ ] **Step 4: Grant the capability**

In `app/src-tauri/capabilities/default.json`, change the `permissions` array to add `"agent:default"`:
```json
  "permissions": [
    "core:default",
    "core:window:allow-start-dragging",
    "core:window:allow-close",
    "agent:default"
  ]
```

- [ ] **Step 5: Verify it compiles**

```bash
cargo check --manifest-path app/src-tauri/Cargo.toml
```
Expected: builds `tauri-plugin-agent` from the submodule and the app crate; finishes with no errors. (First build is slow — it compiles the new crate.)

- [ ] **Step 6: Lint touched crate**

```bash
cargo clippy --manifest-path app/src-tauri/Cargo.toml --tests --no-deps
```
Expected: no new warnings from the edited file.

- [ ] **Step 7: Commit**

```bash
git add app/src-tauri/Cargo.toml app/src-tauri/Cargo.lock app/src-tauri/src/main.rs app/src-tauri/tauri.conf.json app/src-tauri/capabilities/default.json
git commit -m "feat(debug): register tauri-plugin-agent, drop tauri-plugin-mcp (Rust)"
```

---

### Task 3: Swap the guest instrumentation

**Files:**
- Modify: `app/src/main.tsx:25-32` (the dev-only bridge block)
- Create: `app/src/tauri-agent-plugin.d.ts`
- Modify: `app/vite.config.ts` (add `resolve.alias`)
- Modify: `app/package.json` (remove `tauri-plugin-mcp` devDep)
- Modify: `app/bun.lock` (regenerated by `bun install`)

**Interfaces:**
- Consumes: the submodule `guest-js/index.ts` from Task 1.
- Produces: a dev webview that installs `WebviewAgentInstrumentation` per window; a release bundle that references neither plugin.

- [ ] **Step 1: Add the ambient type shim**

Create `app/src/tauri-agent-plugin.d.ts`:
```ts
// Dev-only guest binding for the vendored tauri-agent-plugin. The runtime
// import is resolved by a Vite alias to the submodule source (see
// vite.config.ts); this ambient declaration is all tsc needs so the app bundle
// never compiles the submodule's TypeScript under our rootDir. Keep in sync
// with the constructor we actually use.
declare module "@byeongsu-hong/tauri-plugin-agent" {
  export class WebviewAgentInstrumentation {
    constructor(options: {
      windowLabel: string;
      state?: Record<string, () => unknown>;
    });
    install(): void;
  }
}
```

- [ ] **Step 2: Swap the guest bridge in main.tsx**

In `app/src/main.tsx`, replace the block (the comment at line 25 through the `import("tauri-plugin-mcp")...catch` at ~line 32):
```ts
// dev-only: connect the tauri-plugin-mcp guest bindings so the socket helper
// (app/scripts/tauri-debug.mjs) can run JS / inspect the DOM in this webview.
// screenshots work without it; the DOM/JS commands need it. never in release.
if (import.meta.env.DEV) {
  void import("tauri-plugin-mcp")
    .then(({ setupPluginListeners }) => setupPluginListeners())
    .catch(() => {});
}
```
with:
```ts
// dev-only: install the tauri-agent guest instrumentation so the `tauri-agent`
// CLI / MCP server can snapshot the semantic tree, drive input, capture logs,
// and render DOM-SVG screenshots in this webview. One instance per window,
// labelled by the real Tauri window label. Never in release.
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

- [ ] **Step 3: Add the dev Vite alias**

In `app/vite.config.ts`, add a `resolve` block to the config object (sibling of `plugins`, `server`, `test`). Add near the top after the imports:
```ts
import { fileURLToPath } from "node:url";
```
and inside `defineConfig({ ... })`:
```ts
  resolve: {
    alias: {
      // Guest binding resolves to the vendored submodule source. Safe to leave
      // unconditional: the only import is behind import.meta.env.DEV, so a
      // release build dead-code-eliminates it before this alias is reached.
      "@byeongsu-hong/tauri-plugin-agent": fileURLToPath(
        new URL("../third_party/tauri-agent-plugin/guest-js/index.ts", import.meta.url),
      ),
    },
  },
```

- [ ] **Step 4: Remove the old JS dep**

In `app/package.json`, delete the devDependencies line:
```json
    "tauri-plugin-mcp": "^0.1.0",
```
Then regenerate the lockfile:
```bash
cd app && bun install
```
Expected: `bun.lock` updates, `tauri-plugin-mcp` no longer resolved.

- [ ] **Step 5: Verify the production build (typecheck + bundle)**

```bash
cd app && bun run build
```
Expected: `tsc -p tsconfig.build.json` passes (ambient shim satisfies the dynamic import) and `vite build` produces `dist/` with no unresolved-module error. The guest import is DCE'd from the release bundle.

- [ ] **Step 6: Verify the dev alias resolves the real source**

```bash
cd app && DUCKTAPE_TAURI_DEV_PORT=1431 timeout 20 bun run dev >/tmp/vite-alias.log 2>&1 & \
  sleep 8; curl -s "http://localhost:1431/src/main.tsx" | grep -q "tauri-plugin-agent" && echo "dev server served main.tsx"; \
  grep -iE "error|failed to resolve" /tmp/vite-alias.log || echo "no vite resolve errors"; \
  pkill -f "DUCKTAPE_TAURI_DEV_PORT=1431" 2>/dev/null || true
```
Expected: no "Failed to resolve import" for `@byeongsu-hong/tauri-plugin-agent`. (If the barrel `guest-js/index.ts` pulls node-only modules and Vite errors, narrow the alias to the specific instrumentation module and re-run.)

- [ ] **Step 7: Commit**

```bash
git add app/src/main.tsx app/src/tauri-agent-plugin.d.ts app/vite.config.ts app/package.json app/bun.lock
git commit -m "feat(debug): install tauri-agent guest instrumentation, drop tauri-plugin-mcp (JS)"
```

---

### Task 4: Native CLI shim, retire the bespoke driver

**Files:**
- Delete: `app/scripts/tauri-debug.mjs`
- Create: `app/scripts/tauri-agent` (executable bash shim)

**Interfaces:**
- Consumes: `third_party/tauri-agent-plugin/bin/tauri-agent.ts` from Task 1.
- Produces: `app/scripts/tauri-agent <cmd> ...` — the command the skills call.

- [ ] **Step 1: Delete the old driver**

```bash
git rm app/scripts/tauri-debug.mjs
```

- [ ] **Step 2: Write the shim**

Create `app/scripts/tauri-agent`:
```bash
#!/usr/bin/env bash
# Thin path shim over the vendored tauri-agent CLI. NOT a protocol driver — it
# execs `bun <cli>` and defaults --app to our Tauri identifier when the caller
# didn't already pick a target (--app / --from-html / --port / --host).
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cli="$repo_root/third_party/tauri-agent-plugin/bin/tauri-agent.ts"
has_target=0
for a in "$@"; do
  case "$a" in
    --app|--app=*|--from-html|--from-html=*|--port|--port=*|--host|--host=*) has_target=1 ;;
  esac
done
if [ "$has_target" -eq 0 ]; then set -- "$@" --app com.ducktape.app; fi
exec bun "$cli" "$@"
```
Then:
```bash
chmod +x app/scripts/tauri-agent
```

- [ ] **Step 3: Verify with static-HTML mode (no live app needed)**

```bash
printf '%s\n' '<main><button type="button">Forge</button><p role="status">Ready</p></main>' > /tmp/screen.html
app/scripts/tauri-agent tree --from-html /tmp/screen.html
app/scripts/tauri-agent find --role button --name Forge --from-html /tmp/screen.html
```
Expected: `tree` prints a semantic tree containing a `button "Forge"`; `find` returns a match with a `@ref`. (`--from-html` is present, so the shim does not append `--app`.)

- [ ] **Step 4: Commit**

```bash
git add -A app/scripts
git commit -m "feat(debug): replace tauri-debug.mjs with a tauri-agent CLI shim"
```

---

### Task 5: Register the MCP server for Claude Code

**Files:**
- Create: `.mcp.json` (repo root)

**Interfaces:**
- Consumes: `third_party/tauri-agent-plugin/bin/tauri-agent-mcp.ts` from Task 1.
- Produces: a project MCP server named `tauri-agent` exposing `tauri_*` tools.

- [ ] **Step 1: Write the MCP config**

Create `.mcp.json` at the repo root:
```json
{
  "mcpServers": {
    "tauri-agent": {
      "command": "bun",
      "args": ["third_party/tauri-agent-plugin/bin/tauri-agent-mcp.ts"]
    }
  }
}
```

- [ ] **Step 2: Verify the server speaks MCP over stdio**

```bash
printf '%s\n%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"probe","version":"0"}}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
  | timeout 15 bun third_party/tauri-agent-plugin/bin/tauri-agent-mcp.ts 2>/dev/null \
  | grep -q "tauri_tree" && echo "MCP server exposes tauri_* tools"
```
Expected: prints `MCP server exposes tauri_* tools`. (Confirms `initialize` + `tools/list` return the debugger tools.)

- [ ] **Step 3: Commit**

```bash
git add .mcp.json
git commit -m "feat(debug): register tauri-agent MCP server for Claude Code"
```

---

### Task 6: Migrate fleet.sh to endpoint discovery

**Files:**
- Modify: `ops/fleet.sh` (launch env, stale-cleanup, launch guard, dashboard up-gate, teardown)

**Interfaces:**
- Consumes: the per-instance runtime base convention (`XDG_RUNTIME_DIR=$STATE/$id`) and the endpoint path `$STATE/$id/tauri-agent/com.ducktape.app/endpoint.json`.
- Produces: a fleet where each worktree app's registry is isolated under its own state dir, and the dashboard "up" gate reads the endpoint file.

- [ ] **Step 1: Replace the socket variable + launch env**

In `ops/fleet.sh`, in `up_one()`:

Replace the local declaration:
```bash
  local mcp="/tmp/tauri-mcp-$id.sock" home="$STATE/$id/home" wsdir="$STATE/$id"
```
with:
```bash
  local home="$STATE/$id/home" wsdir="$STATE/$id"
  local endpoint="$wsdir/tauri-agent/com.ducktape.app/endpoint.json"
```

The `mkdir -p "$home" "$wsdir"` line already ensures `$wsdir` exists; tighten its perms right after it (XDG runtime dirs are expected user-private):
```bash
  chmod 700 "$wsdir"
```

- [ ] **Step 2: Update the stale-instance cleanup**

Replace:
```bash
  # a dead instance leaves a stale socket file that would block restart; if the
  # VNC (started after the app) is gone, the instance is dead — clear it.
  if [ -S "$mcp" ] && ! port_up "$vnc"; then rm -f "$mcp"; fi
```
with:
```bash
  # a dead instance leaves a stale endpoint file that would block restart; if the
  # VNC (started after the app) is gone, the instance is dead — clear it.
  if [ -f "$endpoint" ] && ! port_up "$vnc"; then rm -f "$endpoint"; fi
```

- [ ] **Step 3: Update the launch guard + app env**

Replace the launch guard opener:
```bash
  # the app — isolated HOME, warm build caches, headless WebKit flags
  if ! [ -S "$mcp" ]; then
```
with:
```bash
  # the app — isolated HOME, warm build caches, headless WebKit flags
  if ! [ -f "$endpoint" ]; then
```

In the app launch env, replace:
```bash
      DUCKTAPE_TAURI_DEV_PORT="$vite" DUCKTAPE_TAURI_MCP_SOCKET="$mcp" \
```
with:
```bash
      DUCKTAPE_TAURI_DEV_PORT="$vite" XDG_RUNTIME_DIR="$wsdir" \
```

- [ ] **Step 4: Update the dashboard emitter's "up" gate**

Replace:
```javascript
const socketPath = `/tmp/tauri-mcp-${id}.sock`;
if (existsSync(socketPath) && (await portOpen(vncPort))) node.status = "up";
```
with:
```javascript
const endpointPath = join(state, id, "tauri-agent", "com.ducktape.app", "endpoint.json");
if (existsSync(endpointPath) && (await portOpen(vncPort))) node.status = "up";
```

- [ ] **Step 5: Update teardown**

In the `down)` case, replace:
```bash
      pkill -f "target/debug/ducktape-desktop" 2>/dev/null || true
      pkill -f "tauri-mcp-$id.sock" 2>/dev/null || true
```
with:
```bash
      pkill -f "target/debug/ducktape-desktop" 2>/dev/null || true
```
and replace:
```bash
      rm -f "$TOKENS/$id" "/tmp/tauri-mcp-$id.sock"
```
with:
```bash
      rm -f "$TOKENS/$id" "$STATE/$id/tauri-agent/com.ducktape.app/endpoint.json"
```

- [ ] **Step 6: Verify the script parses**

```bash
bash -n ops/fleet.sh && echo "fleet.sh parses"
grep -n "tauri-mcp\|DUCKTAPE_TAURI_MCP_SOCKET" ops/fleet.sh || echo "no stale socket references remain"
```
Expected: `fleet.sh parses`; and no remaining `tauri-mcp` / `DUCKTAPE_TAURI_MCP_SOCKET` references.

- [ ] **Step 7: Commit**

```bash
git add ops/fleet.sh
git commit -m "feat(fleet): discover apps by endpoint registry, isolate via XDG_RUNTIME_DIR"
```

---

### Task 7: Rewrite the debug skills + fix the doc pointer

**Files:**
- Modify (rewrite): `skills/tauri-debug/SKILL.md`
- Modify (rewrite): `skills/qa/SKILL.md`
- Modify: `docs/superpowers/specs/2026-07-07-tauri-dev-error-handling-design.md` (socket mention)

**Interfaces:**
- Consumes: the CLI shim (Task 4), MCP server (Task 5), fleet convention (Task 6).
- Produces: operator docs that match the new stack.

- [ ] **Step 1: Rewrite `skills/tauri-debug/SKILL.md`**

Rewrite for the single running app. Required content:
- **What's wired** table: Rust dep = `tauri-plugin-agent` (submodule path) in `app/src-tauri/Cargo.toml`; registration in `main.rs` under `cfg(all(debug_assertions, desktop))` (`tauri_plugin_agent::init()`); inline-server config in `tauri.conf.json` `plugins.agent`; capability `agent:default`; guest install in `app/src/main.tsx` under `import.meta.env.DEV`; driver = `app/scripts/tauri-agent` (shim over the vendored CLI); MCP = `.mcp.json` server `tauri-agent`.
- **Run it:** `cd app && bun run tauri dev`, then drive with:
  ```bash
  app/scripts/tauri-agent tree                       # semantic tree (defaults --app com.ducktape.app)
  app/scripts/tauri-agent find --role button --name Forge
  app/scripts/tauri-agent click @3
  app/scripts/tauri-agent eval "document.title"
  app/scripts/tauri-agent shot /tmp/app.svg          # DOM-SVG, WM-free
  app/scripts/tauri-agent logs --clear
  ```
  Or, in Claude Code, the native MCP `tauri_*` tools with `app: "com.ducktape.app"`.
- **Discovery:** endpoint at `${XDG_RUNTIME_DIR|TMPDIR|TMP}/tauri-agent/com.ducktape.app/endpoint.json`; the CLI/MCP resolve it by `--app`. No `/tmp` singleton socket.
- **Headless screenshots:** `shot /tmp/app.svg` (DOM-SVG) works under bare Xvfb with **no window manager** — this replaces the old `import -window root` hack. `--backend native` (PNG) is macOS-only; on Linux use DOM-SVG. `eval`/`tree` confirm state independent of capture.
- **Notes/caveats:** dev-only (Rust `cfg` + guest `import.meta.env.DEV`); daemon unchanged (`ducktape-noded` on `127.0.0.1:8844`, seed via `/v1/submit`); after Rust / `tauri.conf.json` changes `tauri dev` rebuilds and the endpoint file drops and reappears — wait for it before re-driving.
- Keep the existing Xvfb bring-up recipe verbatim (still required for WebKitGTK).

- [ ] **Step 2: Rewrite `skills/qa/SKILL.md`**

Rewrite for the fleet. Required content:
- The fleet brings up one app per worktree with `XDG_RUNTIME_DIR=$STATE/$id`; its endpoint registry lives at `$STATE/$id/tauri-agent/com.ducktape.app/endpoint.json`, which is exactly the dashboard "up" gate.
- Drive a worktree by exporting the matching runtime base, then the CLI shim:
  ```bash
  export XDG_RUNTIME_DIR=<fleet-state>/<id>
  app/scripts/tauri-agent tree --app com.ducktape.app
  app/scripts/tauri-agent find --role button --name Forge --app com.ducktape.app
  app/scripts/tauri-agent eval "document.title" --app com.ducktape.app
  ```
- App id is constant (`com.ducktape.app`); the per-instance `XDG_RUNTIME_DIR` is what scopes a driver call to one tile. Assertions go over `tree`/`find`/`eval` (display-independent). Independent instances → one QA agent per worktree.
- Dev-only: rides the same dev-only seam as `[[tauri-debug]]`; a release build registers nothing.

- [ ] **Step 3: Fix the tauri-dev spec pointer**

In `docs/superpowers/specs/2026-07-07-tauri-dev-error-handling-design.md`, find the `tauri-plugin-mcp` / socket mention (grep it) and update it to reference the `tauri-plugin-agent` endpoint model. Keep the surrounding meaning intact — this is a one-line pointer fix, not a rewrite.

```bash
grep -n "tauri-plugin-mcp\|ducktape-tauri-mcp\|DUCKTAPE_TAURI_MCP_SOCKET" docs/superpowers/specs/2026-07-07-tauri-dev-error-handling-design.md
```

- [ ] **Step 4: Commit**

```bash
git add skills/tauri-debug/SKILL.md skills/qa/SKILL.md docs/superpowers/specs/2026-07-07-tauri-dev-error-handling-design.md
git commit -m "docs(skills): rewrite tauri-debug + qa around tauri-agent CLI/MCP"
```

---

### Task 8: End-to-end live verification (acceptance)

**Files:** none (verification; commit only fixups if needed).

**Interfaces:**
- Consumes: everything above.
- Produces: evidence the live app is drivable through the new stack, headless.

- [ ] **Step 1: Bring up the app headless**

Per the tauri-debug Xvfb recipe (use the standalone node-bin workaround from memory `[[tauri-dev-truncates-node-bin]]` if `bun run sidecar` is skipped):
```bash
Xvfb :99 -screen 0 1400x900x24 -nolisten tcp &
cd app && DISPLAY=:99 WEBKIT_DISABLE_DMABUF_RENDERER=1 WEBKIT_DISABLE_COMPOSITING_MODE=1 \
  LIBGL_ALWAYS_SOFTWARE=1 GDK_BACKEND=x11 dbus-run-session -- bun run tauri dev &
```
Expected: app compiles and boots; console webview loads.

- [ ] **Step 2: Confirm the endpoint appears**

```bash
find "${XDG_RUNTIME_DIR:-${TMPDIR:-/tmp}}/tauri-agent" -name endpoint.json -print -exec cat {} \;
```
Expected: `.../tauri-agent/com.ducktape.app/endpoint.json` exists with a `"transport":"tcp"` descriptor (host `127.0.0.1`, a nonzero port).

- [ ] **Step 3: Drive the live window**

```bash
app/scripts/tauri-agent tree
app/scripts/tauri-agent find --role button --name Forge
app/scripts/tauri-agent shot /tmp/live.svg && ls -l /tmp/live.svg
app/scripts/tauri-agent eval "document.title"
```
Expected: `tree` returns the live console's semantic tree; `find` yields a `@ref`; `shot` writes a non-empty SVG **without** a window manager; `eval` returns the title. Drive one navigation via `click @ref` and re-`tree` to confirm the view changed.

- [ ] **Step 4: Confirm the MCP tools list in Claude Code**

Reload MCP servers (or restart the session) so `.mcp.json` is picked up; confirm `tauri-agent` tools appear and a `tauri_tree` call with `{ "app": "com.ducktape.app" }` returns the tree. (If running outside an interactive session, the Task 5 stdio probe already covered the handshake.)

- [ ] **Step 5: One fleet-tile isolation check**

```bash
ops/fleet.sh up <one-branch>
# after it reports "up":
STATE_DIR=$(dirname "$(find /tmp /home -path '*/tauri-agent/com.ducktape.app/endpoint.json' 2>/dev/null | head -1)")
ls "$STATE_DIR/endpoint.json" && echo "tile endpoint isolated under its state dir"
XDG_RUNTIME_DIR="$(cd "$STATE_DIR/../.." && pwd)" app/scripts/tauri-agent tree --app com.ducktape.app | head
```
Expected: the tile's endpoint lives under its own state dir (not `/tmp`), the dashboard shows it "up", and a driver call scoped to that `XDG_RUNTIME_DIR` reaches that instance. Tear down: `ops/fleet.sh down <one-branch>`.

- [ ] **Step 6: Release-safety spot check**

```bash
grep -n "tauri_plugin_agent::init" app/src-tauri/src/main.rs
```
Expected: the only registration is inside the `#[cfg(all(debug_assertions, desktop))]` block — a release binary registers nothing and opens no socket.

- [ ] **Step 7: Commit any fixups**

```bash
git add -A && git commit -m "fix(debug): tauri-agent integration fixups from live verification"   # only if changes were needed
```

---

## Self-Review

**Spec coverage:**
- Vendoring (submodule) → Task 1. ✓
- Rust swap (Cargo/main.rs/tauri.conf/capabilities, keep cfg guard) → Task 2. ✓
- Guest (main.tsx/vite alias/package.json, dev-only) → Task 3. ✓
- Driver retire + native CLI shim → Task 4. ✓
- MCP registration → Task 5. ✓
- fleet.sh (env/up-gate/teardown, XDG_RUNTIME_DIR isolation) → Task 6. ✓
- Skills + doc pointer → Task 7. ✓
- Release-safety invariant → verified in Task 2 (cfg guard preserved) + Task 8 Step 6. ✓
- Verification plan (endpoint appears, CLI drives, MCP tools, fleet tile) → Task 8. ✓

**Placeholder scan:** No TBD/TODO; every code/config step shows exact content; Task 7 skill rewrites specify required content section-by-section (prose docs, not code, so bullet-level content specs are appropriate rather than verbatim full files).

**Type/name consistency:** `com.ducktape.app` used identically everywhere; endpoint path `<runtime>/tauri-agent/com.ducktape.app/endpoint.json` consistent across Rust comment, fleet.sh, dashboard emitter, skills, verification; `WebviewAgentInstrumentation({ windowLabel }).install()` matches the ambient shim in Task 3 Step 1; the shim's target flags (`--app/--from-html/--port/--host`) match the CLI surface used in Tasks 4/7/8.
