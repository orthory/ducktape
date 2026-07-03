---
name: qa
description: Use to QA a change in the real Ducktape desktop app when you need SEVERAL worktrees' apps running at once, or an isolated throwaway instance that can't touch your other running app or its ~/.ducktape workspaces. Boots one fully isolated Tauri instance per worktree (own vite port, debug socket, private workspace registry, display), founds a solo workspace, and drives it via tauri-debug. For driving the ONE app you already have running, use tauri-debug directly.
---

# QA (multi-window worktree QA)

`qa` runs **several real Ducktape desktop apps at once**, one per worktree, each
fully isolated, so an agent can exercise the native UI + real node-backed data of a
task branch without colliding with any other running instance. It is the
multi-window layer over [[tauri-debug]] (which drives a single window) and pairs
with [[work]] (which creates the worktrees under `.claude/worktrees/`).

The isolation is the whole point: a bare second `tauri dev` collides on the vite
port, the `/tmp/tauri-mcp.sock` debug socket, and — critically — the shared
`~/.ducktape` **workspace registry** (its `registry.json` + per-workspace node
storage). `qa` gives every instance its own of each and records them in a manifest.

The desktop app is a multi-**workspace** shell: on boot it reads `~/.ducktape` and
connects the active workspace's node (the app itself allocates that workspace's
ports + storage), or shows onboarding when there is none. So isolation needs just
**one** seam — `DUCKTAPE_HOME` (see `app/src-tauri/src/workspaces.rs::root`) points
each instance at a private registry. `up` then drives the app's real
`workspace_create` command over the debug socket to found a solo workspace and
reads its app-allocated http port back from `registry.json`.

## The launcher — `app/scripts/qa-instance.mjs`

Dependency-free node (node built-ins only), same house as `tauri-debug.mjs`.

```bash
node app/scripts/qa-instance.mjs up   [<worktree-dir>]   # boot; prints the manifest path
node app/scripts/qa-instance.mjs down [<worktree-dir|slug>]
node app/scripts/qa-instance.mjs list
node app/scripts/qa-instance.mjs env  [<worktree-dir|slug>]  # `export`s for the driver
```

`up` allocates a coherent **instance profile** and writes it to
`${XDG_RUNTIME_DIR:-/tmp}/ducktape-qa/<slug>/instance.json`:

| axis | per-instance value | wired via |
|---|---|---|
| vite dev port | free port, pinned strict | `DUCKTAPE_TAURI_DEV_PORT` + tauri `--config` devUrl |
| debug socket | `<runRoot>/mcp.sock` | `DUCKTAPE_TAURI_MCP_SOCKET` |
| workspace registry | `<runRoot>/ducktape` (`registry.json` + `workspaces/<id>/`) | `DUCKTAPE_HOME` |
| node binary | shared-target build | `DUCKTAPE_NODE_BIN` |
| X display | private Xvfb (`:101`+) | `DISPLAY` + WebKit headless flags |

The workspace **node's** http/p2p/rpc ports and storage are the app's to allocate
(binding `:0`, recorded in `registry.json`); isolating `DUCKTAPE_HOME` is enough to
keep two instances' workspaces from ever colliding.

`<slug>` is the branch with `/`→`+` (matching `work`'s worktree dir), length-capped
so the socket path stays under the ~108-char unix `sun_path` limit — that is why the
run root lives under `/tmp`/`$XDG_RUNTIME_DIR`, not the deep worktree path.

`up` boots vite + `tauri dev` (detached, per-instance dbus session), waits for the
window to become driveable, founds workspace `qa`, reloads the window onto the live
console, and **returns only once that workspace's node answers `/v1/status`**. A
failed boot tears its own processes down. The first shell build can take minutes; it
reuses the main checkout's `target/` (`DUCKTAPE_QA_TARGET_DIR` to override) so it is
not rebuilt per worktree.

## Drive an instance (reuse tauri-debug unchanged)

`tauri-debug.mjs` already reads `DUCKTAPE_TAURI_MCP_SOCKET`, so point it at the
instance and drive as usual:

```bash
MAN=$(node app/scripts/qa-instance.mjs up <worktree>)   # -> path to instance.json
eval "$(node app/scripts/qa-instance.mjs env <worktree>)"   # exports SOCKET + NODE_URL + DISPLAY

node app/scripts/tauri-debug.mjs eval "document.title"         # DOM/state assertion (over the socket)
import -window root /tmp/qa-<slug>.png                          # screenshot (DISPLAY is this instance's)
curl -s "$DUCKTAPE_QA_NODE_URL/v1/submit" -d '<op>'           # seed workspace state; app re-renders
```

**Assertions over the socket; screenshots off the private display.** `eval` /
`query_page` read the DOM over the socket and are display-independent — they are the
primary QA signal. For pixels, each instance owns its own Xvfb, so `import -window
root` under that instance's `DISPLAY` (exported by `env`) grabs exactly this window
with no compositing and no window manager. Do **not** use `tauri-debug snap` here —
its `debug_capture_webview` command is not registered in this app; and `shot`
(plugin `take_screenshot`) needs a window manager Xvfb lacks.

## Several worktrees at once

Bring each worktree up, drive it, tear it down. Independent instances are a natural
fit for parallel agents — one agent per worktree, each `env`-scoped to its own
socket and display.

## Teardown

`down` kills in the safe order (tauri CLI first — it respawns a crashed app — then
vite, then `POST /v1/shutdown` to the detached workspace node — the port is its
identity, no pid crosses that boundary — then this instance's Xvfb) and removes the
run root (which holds this instance's `DUCKTAPE_HOME`). It only touches pids recorded
in the manifest — never a global `pkill` — so it cannot disturb another instance.
`list` shows what is up.

## Notes & caveats

- **Frontend vs. Rust shell.** The shared `target/` gives cache hits only when
  `src-tauri` is unchanged across worktrees (the common UI-QA case). A worktree that
  changes Rust should get its own target dir (`DUCKTAPE_QA_TARGET_DIR`) to avoid
  rebuild-thrash.
- **Dev only.** Isolation rides the same dev-only seams as `tauri-debug`; a release
  build opens no socket and honors none of this.
- **Port race.** Ports are allocated by binding `:0` then handing the number to the
  child — a tiny TOCTOU window exists; `strictPort` surfaces a collision loudly
  rather than silently.
