# Multi-Window Worktree QA — Design

- Date: 2026-07-03
- Status: implemented (see Revision 2)
- Scope: infrastructure to run **several isolated real Tauri app instances at once**, one per worktree, and let a QA agent drive each over the existing `tauri-debug` socket bridge.

> **Revision 2 (2026-07-03).** Between the first draft and merge, PR #76
> (onboarding) landed on `dev` and re-architected the desktop node lifecycle: the
> single `daemon_spawn`-on-`8844` model is retired; the node is now one of the
> user's `~/.ducktape` **workspaces** (`app/src-tauri/src/workspaces.rs`), and the
> app itself allocates each workspace's ports (`:0`) + storage and records them in
> `registry.json`. That **collapses the isolation seam** from the two env vars
> below (§3.3, now inert) to a **single** one: `DUCKTAPE_HOME` on
> `workspaces.rs::root`, pointing each instance at a private registry. The launcher
> then drives the app's real `workspace_create` over the socket and reads the
> app-allocated http port back from `registry.json`. §§2–3.3 below describe the
> original daemon-model reasoning; the collision analysis (§2, "the easy 80% vs the
> hard 20%") still holds — only the specific seam changed. The shipped code follows
> this revision.

## 1. Problem

Today the repo can drive exactly **one** live desktop app (`tauri-debug` skill →
`app/scripts/tauri-debug.mjs` → the dev-only `tauri-plugin-mcp` unix socket). It
is single-tenant by construction: every shared resource is hardcoded or defaults
to one fixed value, so a second `tauri dev` collides with the first.

We want an **agent-based QA system**: for N worktrees (each a task branch under
`.claude/worktrees/`) we boot N fully isolated app instances concurrently, hand
each to an agent that exercises the real native UI + real daemon-backed data, and
tear them down cleanly. The hard part is not "run the app" — it is making every
shared resource *per-instance* so the instances cannot see or corrupt each other.

## 2. The collision surface (the core of "think about socket / vite port")

Running two worktree apps at once collides on five axes. Three are already
env-overridable; two are hardcoded and need a small source change.

| Axis | Set where | Default | Overridable today |
|---|---|---|---|
| Vite dev port | `DUCKTAPE_TAURI_DEV_PORT` env → `app/vite.config.ts` (strictPort when set) | 1420 | ✅ env (Tauri `devUrl` still needs a per-instance config override) |
| Debug socket | `DUCKTAPE_TAURI_MCP_SOCKET` env → `app/src-tauri/src/main.rs` | `/tmp/tauri-mcp.sock` | ✅ env |
| Node binary | `DUCKTAPE_NODE_BIN` env → `app/src-tauri/src/daemon.rs` | sibling of app exe | ✅ env |
| **Daemon HTTP port** | `DEFAULT_LISTEN` const → `app/src/domain/node-bootstrap.ts` (used for adopt-probe, `daemon_spawn`, shutdown) | `127.0.0.1:8844` | ❌ **hardcoded** |
| **Daemon state dir** | `app_data_dir()` from identifier `com.ducktape.app` → `daemon.rs` (`node.toml`, `storage/`, `daemon.log`) | `~/.local/share/com.ducktape.app/node/` — **constant across all worktrees** | ❌ **hardcoded** |

Consequence of the two ❌ rows: even after giving each instance its own vite port
and socket, two apps still (a) both try to bind daemon port 8844 — the second
adopts the first's daemon instead of getting its own, silently sharing state; and
(b) both read/write the *same* `node.toml` + `storage/` — so their consensus
state is physically shared. **Real isolation is impossible without making the
daemon listen address and data dir per-instance.** That is the crux this design
solves; the socket + vite port are the easy 80%.

A sixth axis is headless-only: the **X display**. `import -window root` composites
every window on a display, so N windows on one Xvfb blend together. The obvious
escape — `tauri-debug snap` (`WKWebView.takeSnapshot`) — does **not** work in this
app: its `debug_capture_webview` command is referenced by the driver but never
registered in `main.rs` (only `daemon_spawn` is), so `snap` errors `Command
debug_capture_webview not found`. And `shot` (plugin `take_screenshot`) needs a
window manager Xvfb lacks. Therefore each instance gets its **own Xvfb** (`:101`+):
`import -window root` under that private display grabs exactly one window, no
compositing, no WM — and, as a bonus, QA windows never appear on the user's `:99`
(their live remote-tauri/VNC session). DOM/state assertions go over the socket
(`eval`/`query_page`), which is display-independent and the primary QA signal.

## 3. Design

### 3.1 The instance profile — allocate, then record

Each QA instance is defined by an **instance profile**: a coherent bundle of one
allocated value per collision axis, allocated at launch and **persisted to a
manifest file** the driver/agent reads back. We do not hardcode or hash ports —
we bind `:0`, read the OS-assigned free port, and write it down. This is the
direct answer to "how is the socket file stored and the vite port addressed":

```
run root:   ${XDG_RUNTIME_DIR:-/tmp}/ducktape-qa/<slug>/
  ├── instance.json     # the manifest (below)
  ├── mcp.sock          # per-instance tauri-plugin-mcp socket
  ├── data/             # per-instance daemon state (node.toml, storage/, daemon.log)
  └── tauri-dev.log     # this instance's tauri dev stdout/stderr
```

- **`<slug>`** = the worktree branch with `/`→`+` (same scheme `work` uses for
  `WT_DIR`), lowercased and truncated to keep the socket path well under the
  **~108-char `sun_path` limit** — the reason the run root lives under
  `/tmp`/`$XDG_RUNTIME_DIR` (short) and *not* under the deep
  `.claude/worktrees/<slug>/...` path (which can blow the limit). **ASSUMPTION:**
  slug derived from branch, truncated to 32 chars + a 6-char hash suffix for
  uniqueness.
- **Ports** (vite, daemon-http) are allocated by binding `127.0.0.1:0`, reading
  the port, closing, and immediately handing it to the child. A tiny race window
  exists (TOCTOU between close and child bind); acceptable for a dev QA harness,
  and `strictPort`/bind-failure surfaces it loudly rather than silently.
- **`instance.json`** records everything a driver needs, so nothing re-derives:
  ```json
  {
    "slug": "feat+forge-console-view",
    "worktree": "/home/eddy/dev/ducktape/.claude/worktrees/feat+forge-console-view",
    "vitePort": 1731,
    "viteUrl": "http://localhost:1731",
    "daemonListen": "127.0.0.1:8861",
    "daemonUrl": "http://127.0.0.1:8861",
    "socketPath": "/run/user/1000/ducktape-qa/feat+forge-console-view/mcp.sock",
    "dataDir": "/run/user/1000/ducktape-qa/feat+forge-console-view/data",
    "display": ":99",
    "pids": { "tauri": 12345 }
  }
  ```

### 3.2 Wiring each axis to the profile

| Axis | How the launcher applies the profile value |
|---|---|
| Debug socket | export `DUCKTAPE_TAURI_MCP_SOCKET=<socketPath>` (already honored in `main.rs`) |
| Vite port | export `DUCKTAPE_TAURI_DEV_PORT=<vitePort>` (already honored in `vite.config.ts`) + pass `tauri dev --config` override so `build.devUrl` matches `<viteUrl>` (the headless recipe already uses this `--config` pattern) |
| Node binary | export `DUCKTAPE_NODE_BIN=<abs path to staged ducktape-node>` (already honored) |
| **Daemon HTTP port** | export `VITE_DUCKTAPE_NODE_LISTEN=<daemonListen>` — **needs source change** (§3.3.1) so the webview spawns/adopts on the instance port instead of 8844 |
| **Daemon state dir** | export `DUCKTAPE_NODE_DATA_DIR=<dataDir>` — **needs source change** (§3.3.2) so `daemon_spawn` writes state there instead of `app_data_dir()` |
| Display | spawn a private `Xvfb :<n>` (n≥101), export `DISPLAY=<display>` + the WebKit headless flags from the `tauri-debug` recipe; capture with `import -window root` |

Because `tauri-debug.mjs` **already** reads `DUCKTAPE_TAURI_MCP_SOCKET`, the agent
drives an instance with zero changes to that driver — it just sources the socket
path from the manifest before each call.

### 3.3 Source changes (small, additive, dev-safe)

Two new env seams, mirroring the seams that already exist. Both keep current
behavior exactly when the env var is unset, so nothing changes for a normal run.

#### 3.3.1 `app/src/domain/node-bootstrap.ts` — daemon listen addr

```ts
const DEFAULT_LISTEN =
  import.meta.env.VITE_DUCKTAPE_NODE_LISTEN || "127.0.0.1:8844";
```

`DEFAULT_LISTEN` already feeds adopt-probe, `daemon_spawn`, and the derived
`url` (hence shutdown), so this one line makes the whole daemon lifecycle
per-instance. Mirrors the existing `VITE_DUCKTAPE_NODE_URL` seam right next to it.

#### 3.3.2 `app/src-tauri/src/daemon.rs` — daemon data dir

```rust
let data_dir = std::env::var_os("DUCKTAPE_NODE_DATA_DIR")
    .map(std::path::PathBuf::from)
    .map(Ok)
    .unwrap_or_else(|| app.path().app_data_dir())
    .map_err(|err| format!("no app-data dir: {err}"))?;
```

Everything downstream (`node/`, `node.toml`, `storage/`, `daemon.log`) is derived
from `data_dir`, so this single override isolates all daemon state. Unset → today's
`app_data_dir()` behavior. **ASSUMPTION:** name `DUCKTAPE_NODE_DATA_DIR`; it points
at the dir that currently *is* `app_data_dir()` (the `node/` subdir is still
appended), keeping the on-disk shape identical.

No change is required to the socket (`main.rs`), vite (`vite.config.ts`), or node
binary (`daemon.rs`) seams — they already read env.

### 3.4 The launcher — `app/scripts/qa-instance.mjs`

Dependency-free node (same house style as `tauri-debug.mjs`). Subcommands:

- `up <worktree-dir>` — allocate profile, ensure Xvfb, stage/resolve
  `ducktape-node`, write `instance.json`, spawn `tauri dev` (detached, logged)
  with the profile env + `--config` devUrl override, then **wait for readiness**:
  poll for the `mcp.sock` to appear *and* the daemon URL to answer `/v1/status`.
  Prints the manifest path on success.
- `down <worktree-dir|slug>` — teardown in the `tauri-debug` skill's order: kill
  the `tauri` CLI first (it respawns a crashed app), then the app, then
  `POST <daemonUrl>/v1/shutdown` for the detached node, then remove the run root.
  Leaves the shared Xvfb up (other instances may use it) unless `--last`.
- `list` — read every `instance.json` under the run parent, print a table.

**ASSUMPTION — run model.** Default is **`tauri dev` per worktree** (Model A):
proven (the `tauri-debug` headless recipe already runs exactly this), correct
even when a worktree changes `src-tauri`. Its cost is recompiling the Rust shell
per worktree. Mitigation, applied by default: a **shared `CARGO_TARGET_DIR`**
across worktrees whose `src-tauri` is unchanged — identical Rust fingerprint →
cache hits, near-instant subsequent boots; a worktree that *does* touch
`src-tauri` gets a private target dir to avoid rebuild-thrash. A lighter
"one prebuilt shell, N vite servers" model (Model B) is possible but its runtime
`devUrl` override for a raw dev binary is unverified — deferred, not in the first
cut.

### 3.5 The QA skill — `skills/qa/SKILL.md`

The agent-facing workflow. Given a worktree (or a set of them), the skill:

1. `qa-instance.mjs up <worktree>` → read the returned `instance.json`.
2. `export DUCKTAPE_TAURI_MCP_SOCKET=<socketPath>` and drive the live window with
   the **unchanged** `tauri-debug.mjs` (`eval`, `snap`, `cmd`), and seed daemon
   state via `curl <daemonUrl>/v1/submit` (the app re-renders on each block).
3. Capture evidence per instance — DOM assertions over the socket (`eval`) plus
   `import -window root` screenshots off the instance's private display.
4. `qa-instance.mjs down <worktree>` when finished.

For multiple worktrees, the skill dispatches step 1–4 per instance (parallel
agents are a natural fit, but the first cut documents the single-instance loop and
leaves fan-out to the caller). The skill cross-links `tauri-debug` (single-window
mechanics) and `work` (worktree naming/creation).

## 4. What is explicitly out of scope (YAGNI)

- A standing daemon/service or long-lived scheduler. The launcher is invoked
  per QA session; nothing runs between sessions.
- A results database / web dashboard. Evidence is files under the run root.
- Windows/macOS specifics. The dev box is headless Debian; the design keeps the
  existing cross-platform socket/detach code but only the Linux/Xvfb path is
  exercised.
- A full one-command fan-out orchestrator. The launcher + skill make it trivial
  to add later; it is not built in the first cut.

## 5. Testing / verification

- **Unit**: keep `node-bootstrap.test.ts` green; add a case asserting
  `VITE_DUCKTAPE_NODE_LISTEN` overrides `DEFAULT_LISTEN`. `cargo test -p app` (or
  the app-shell crate) stays green with `DUCKTAPE_NODE_DATA_DIR` unset.
- **Integration (the real proof)**: on the headless box, `qa-instance.mjs up` two
  distinct worktrees, then assert via each instance's socket that (a) both windows
  render, (b) `daemonUrl` differs and each answers `/v1/status`, (c) a `/v1/submit`
  to instance A's daemon does **not** appear in instance B (state isolation), and
  (d) `down` removes both run roots and leaves no daemon on either port.
- Run the repo `make test` gate for the touched Rust/TS.

## 6. Open questions for the user

1. **System shape** (asked, unanswered): launcher+skill (this spec's target),
   full fan-out orchestrator, or source-isolation only? This spec builds
   launcher+skill and makes the orchestrator a cheap follow-on.
2. **Run root location**: `$XDG_RUNTIME_DIR`/`/tmp` (ephemeral, short paths — this
   spec's default) vs. worktree-local `.claude/qa/` (inspectable, survives reboot,
   risks socket-path length).
3. **Env var names**: `VITE_DUCKTAPE_NODE_LISTEN` / `DUCKTAPE_NODE_DATA_DIR` —
   fine, or prefer another convention?
