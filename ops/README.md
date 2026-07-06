# ops/ — fleet QA dashboard

Run the real Ducktape desktop app for **every git worktree** at once, headless,
and watch them live from any device over Tailscale — so you can see how agents
are driving and QAing each branch.

```
ops/
  fleet.sh          fleet manager (userspace, no root)
  fleet-console/    the dashboard (Vite + React + @novnc/novnc, @xyflow/react)
```

## Quick start

```bash
ops/fleet.sh build-console        # one-time: bun install + vite build → dist/
ops/fleet.sh up                   # bring up an instance per worktree (or: up <branch> …)
ops/fleet.sh status               # slots/ports/status + dashboard URL
# open the printed URL on your Mac/phone (must be on the tailnet)
ops/fleet.sh down                 # tear it all down
```

The dashboard shows one live tile per worktree (branch, sha, ahead/behind vs
`dev`, status, and an **agent activity** line — uncommitted-file count + latest
commit). Toggle **Grid** ⇄ **Graph** (the git branch tree via `@xyflow/react`;
`?view=graph` deep-links it). Click a tile to open a full-size **interactive**
session plus that worktree's commit trail.

## How it works

```
browser ──http/ws :6090 (ONLY exposed port)──▶ one websockify
                                               ├─ --web fleet-console/dist  (UI + fleet.json)
                                               └─ ?token=<worktree> ─▶ 127.0.0.1:<vncPort>
per worktree:  Xvfb :11x → tauri dev (isolated $HOME) → x11vnc 127.0.0.1:591x
```

- **One exposed port** (`:6090`). Every worktree's x11vnc binds `127.0.0.1`; the
  browser reaches them only through websockify's token router.
- **Isolation**: each app runs with its own `$HOME` AND `up_one` seeds a solo
  workspace there (active, camelCase `registry.json`) + passes a stable
  `DUCKTAPE_NODE_BIN` (staged outside `target/`, which `tauri dev`'s build.rs
  clobbers to a 0-byte placeholder). The app then boots **LOCAL** on its own
  node — not the shared `127.0.0.1:8844`. **Requires the worktree app to carry
  PR #90** (StrictMode boot fix, on `dev`); worktrees behind `dev` boot REMOTE.
  `CARGO_HOME`/`RUSTUP_HOME`/caches are pinned to the real home so builds stay
  warm.
- **Port bases** are offset from the single-instance `remote-tauri.sh`
  (`:99/5900/6080`) so both can run at once. Override with `FLEET_DISP_BASE`,
  `FLEET_VITE_BASE`, `FLEET_VNC_BASE`, `FLEET_WEB_PORT`, `FLEET_SCREEN`.
- Reuses the x11vnc / xdotool / noVNC / websockify already staged under
  `~/.local/opt/remote-tauri/` by the `tauri-debug` / remote-tauri setup.

## Notes

- Read-only console: it views and connects; bring instances up/down with the
  script (no lifecycle control in the UI — deferred by design).
- Activity source is git/worktree churn (provider-agnostic — works for any
  agent). The `<ActivityFeed>` is pluggable if you want a richer source later.
- Agent QA automation lives in `skills/qa/SKILL.md`; this README is the
  maintained operator reference for the fleet dashboard.
