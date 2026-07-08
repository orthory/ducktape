# Node → Logs tab (daemon log viewer + runtime facts)

**Date:** 2026-07-09
**Status:** design approved (scoping via user selection), implementing on `feat/node-logs-tab`.

## Problem

The desktop app spawns each workspace's node as a detached process whose
stdout+stderr are appended to a per-workspace `daemon.log`. Today that log is
surfaced **only on the failure screen** (`NodeFailed`, a one-shot "Open
daemon.log" toggle). During normal operation there is no way to watch the
daemon, and no operator-facing view of the running process's identity (pid,
uptime, which binary). Operators debugging a live node have to leave the app and
`tail -f` a path they have to discover.

## Goal

Add a **Logs** tab to the Node view (`StatusView`) that gives an operator:

1. A **live daemon-log viewer** that follows the tail, with search, level
   highlighting, and a follow/pause control.
2. A **runtime facts** row: PID, uptime, node version, binary path, data dir —
   the operational identity of the running process.

This is inherently **managed-only**: logs and process facts exist only for the
local daemon this app spawned. A remote/unmanaged node shows an honest empty
state.

### Explicitly out of scope (user-deselected)

- Restart control (start+stop stay as they are).
- OS-level "reveal in file manager" and log download/export.
- A copy-visible-tail affordance is kept because it reuses the existing
  clipboard pattern at no real cost.

## Current state (verified)

- `StatusView.tsx` = screen label "Node", tabs `Overview | Connections |
  Permissions` (`TabId`, `TABS`). Managed daemons get Start/Stop in the header.
- `workspace_log_tail(id) -> LogTail { path, tail }` already exists
  (`workspaces.rs`, registered in `main.rs`), reading the last 64 KB of
  `<workspaces_dir>/<id>/daemon.log`. Domain wrapper: `workspaceLogTail(id)` in
  `domain/workspace-client.ts`.
- The pid is written to `<workspace_dir>/node.pid` (`pidfile`,
  `recorded_pid_alive`, `pid_alive` in `workspaces.rs`).
- `resolve_node_bin()` returns the `ducktape-node` binary path.
- The running node's version is already on the frontend as
  `state.status.version`.
- Store exposes `state.workspace` (has `.id`), `state.managed`,
  `state.connected`, `actions.readMetrics()` (the poller pattern to mirror,
  `useLiveMetrics` / `METRICS_POLL_MS`).

## Approach

**Poll the existing tail command** (chosen over true streaming). A poller
re-reads `workspace_log_tail` every ~1.5 s while the Logs tab is mounted and the
node is managed, mirroring `useLiveMetrics`. Rationale: the tail read is already
the app's source of truth for daemon health; polling is the smallest, lowest-risk
change and needs no new streaming/backpressure infra. The store action is shaped
so a streaming source can replace it later without touching the view. Latency
~1.5 s and a 64 KB window are acceptable for an operator glance (documented
limitation, same 64 KB bound `workspace_phase` already uses).

## Components

### Frontend

- **`views/status/LogsTab.tsx`** (new) — the tab body.
  - Managed-only guard: unmanaged/remote → empty state ("Logs are only
    available for the local daemon this app manages").
  - **Runtime facts row** at top — small cells reusing the `StatCard` /
    `CopyValue` look: PID, Uptime, Node version (`state.status.version`), Binary
    path (copyable), Data dir (copyable).
  - **Log viewer**:
    - Monospace scroll region, tail split into lines.
    - **Follow/pause** toggle; while following, auto-scroll to bottom on each
      poll. Scrolling up pauses follow; a **Jump to latest** pill re-enables it.
    - **Search** — case-insensitive substring filter over lines; match count.
    - **Level filter + highlight** — best-effort per-line detection of
      `ERROR | WARN | INFO | DEBUG | TRACE` (RUST_LOG style); color-coded via
      theme tokens; toggle chips to show/hide levels. Lines with no detected
      level are treated as `INFO`-ish "other" and always shown unless filtered.
    - Header: `daemon.log · following`, a **Copy** button (copies the visible
      lines to clipboard via the existing `navigator.clipboard` pattern).
- **`views/status/log-lines.ts`** (new, pure) — line parsing/classification:
  `stripAnsi(line)`, `parseLevel(line)`, `splitLines(tail)`,
  `filterLines(lines, {query, levels})`, `levelCounts(lines)`. Kept pure +
  separately unit-tested (the app's convention, cf. `node-health.ts`).
  - **ANSI stripping (found during live verification):** a real node's tracing
    fmt layer writes ANSI SGR color codes to `daemon.log` (e.g.
    `\x1b[31mERROR\x1b[0m`). Untreated these render as literal garbage AND the
    trailing `m` of a color code fuses to the next word, breaking the `\b` level
    boundary so every colorized line mis-classifies as "other". `splitLines`
    strips SGR (anchored on the ESC byte, so real `[..m` message text survives)
    before classifying and displaying.
- **`StatusView.tsx`** — add `"logs"` to `TabId` and a `["logs","Logs"]` entry
  to `TABS`; render `<LogsTab />` when active.

### Backend (one new command)

- **`workspace_runtime_facts(id) -> RuntimeFacts`** in `workspaces.rs`,
  registered in `main.rs`:
  ```
  RuntimeFacts {
    pid: Option<u32>,        // from node.pid
    alive: Option<bool>,     // kill -0 (None when no pidfile / unknown)
    uptime_secs: Option<u64>,// `ps -o etime=` parsed; None on non-unix/failure
    binary_path: Option<String>, // resolve_node_bin()
    data_dir: String,        // <workspaces_dir>/<id>
    log_path: String,        // <data_dir>/daemon.log
  }
  ```
  - Reuses `pidfile`, `pid_alive`, `resolve_node_bin`, `workspaces_dir`.
  - New pure helper `parse_etime(&str) -> Option<u64>` for `[[dd-]hh:]mm:ss` →
    seconds, unit-tested.
  - Windows: `uptime_secs = None` (graceful).
- **`domain/workspace-client.ts`** — add `RuntimeFacts` interface +
  `workspaceRuntimeFacts(id)` wrapper.

### Store / data flow

- Two thin actions on the ducktape store, keyed on `state.workspace.id`, no-ops
  when `!managed`:
  - `readDaemonLog(): Promise<LogTail | null>` → `ws.workspaceLogTail(id)`.
  - `readRuntimeFacts(): Promise<RuntimeFacts | null>` → new wrapper.
- `LogsTab` owns two pollers (mirroring `useLiveMetrics`: poll on mount, clear
  on unmount, reset on node change): log every ~1.5 s, facts every ~5 s.

## Error handling

- A failed `readDaemonLog` / `readRuntimeFacts` (node stopped mid-view, file
  gone) resolves to `null`; the viewer keeps the last good frame and shows a
  muted "not reachable" hint rather than throwing. Matches the app's
  best-effort log-tail handling in `actions.ts`.
- No pidfile (adopted/remote node we didn't spawn) → facts show "—" for pid /
  uptime; the log viewer still works if the log path exists.

## Testing

- **Vitest** `log-lines.test.ts`: `parseLevel` across INFO/WARN/ERROR/DEBUG/
  TRACE + unmatched; `filterLines` by query and by level set.
- **Vitest** `LogsTab.test.tsx`: managed vs unmanaged empty state; follow/pause
  toggle + jump-to-latest; search filters + match count; level chips hide lines;
  poller lifecycle (mock store actions, à la `StatusView.test.tsx` /
  `onboarding.test.tsx`).
- **Rust** unit test for `parse_etime` (all four field widths) and a
  `workspace_runtime_facts` smoke over a temp dir with a written pidfile.
- **Live**: verify in a real Tauri window via the `tauri-debug` skill — this is
  the one surface where the log is real (start a managed workspace, open Node →
  Logs, confirm the tail follows and facts populate).

## Gates / delivery

- Isolated worktree off `origin/dev` (`feat/node-logs-tab`).
- `cargo clippy -p ducktape-app --tests --no-deps` green (crate the change
  touches; confirm crate name from `src-tauri/Cargo.toml`).
- `make install` (frontend typecheck + build gate) green.
- PR against `dev`; clean-context diff review before any merge decision.
