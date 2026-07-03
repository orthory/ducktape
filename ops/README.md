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
`dev`, status). Click a tile to open a full-size **interactive** session for that
worktree's app.

## How it works

```
browser ──http/ws :6090 (ONLY exposed port)──▶ one websockify
                                               ├─ --web fleet-console/dist  (UI + fleet.json)
                                               └─ ?token=<worktree> ─▶ 127.0.0.1:<vncPort>
per worktree:  Xvfb :11x → tauri dev (isolated $HOME) → x11vnc 127.0.0.1:591x
```

- **One exposed port** (`:6090`). Every worktree's x11vnc binds `127.0.0.1`; the
  browser reaches them only through websockify's token router.
- **Isolation**: each app runs with its own `$HOME`, so `~/.ducktape` (workspace
  registry) and app-data don't collide. `CARGO_HOME`/`RUSTUP_HOME`/caches are
  pinned to the real home so builds stay warm. Node ports auto-allocate.
- **Port bases** are offset from the single-instance `remote-tauri.sh`
  (`:99/5900/6080`) so both can run at once. Override with `FLEET_DISP_BASE`,
  `FLEET_VITE_BASE`, `FLEET_VNC_BASE`, `FLEET_WEB_PORT`, `FLEET_SCREEN`.
- Reuses the x11vnc / xdotool / noVNC / websockify already staged under
  `~/.local/opt/remote-tauri/` by the `tauri-debug` / remote-tauri setup.

## Notes

- Read-only console: it views and connects; bring instances up/down with the
  script (no lifecycle control in the UI — deferred by design).
- `@xyflow/react` is installed for a later branch-tree view mode; v1 is the grid.
- Design spec: `docs/superpowers/specs/2026-07-03-agent-qa-fleet-dashboard-design.md`.
