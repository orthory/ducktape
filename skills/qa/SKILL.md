---
name: qa
description: Use to DRIVE the fleet's real Ducktape desktop instances for agent QA — one live app per git worktree. The fleet (ops/fleet.sh + the dashboard) brings each worktree's app up headless, VNCs it, and exposes its tauri-plugin-mcp debug socket; this skill drives that socket (screenshot, run JS, assert the DOM, seed data) so an agent can QA a branch while you watch it live in the dashboard. For the single already-running app, use tauri-debug directly.
---

# QA (drive the fleet)

The **fleet** (`ops/fleet.sh` + `ops/fleet-console`, the live noVNC grid) runs the
real Ducktape desktop app for each git worktree, headless, and exposes them all
through one token-multiplexed web port. Each instance already ships the driving
seam this skill needs: `ops/fleet.sh up_one` starts `tauri dev` with
`DUCKTAPE_TAURI_MCP_SOCKET=/tmp/tauri-mcp-<id>.sock` — the same dev-only
[[tauri-debug]] bridge. This skill is the **driving** layer over that: an agent
QAs a worktree's branch through its socket while you watch the tile live.

`<id>` is the branch slugged (`re.sub` of non-alnum → `-`), e.g. `feat/x` → `feat-x`.

## The loop

```bash
ops/fleet.sh up <branch>                       # bring the worktree's app up (headless + VNC + dashboard tile)
export DUCKTAPE_TAURI_MCP_SOCKET=/tmp/tauri-mcp-<id>.sock

node app/scripts/tauri-debug.mjs eval "document.title"                 # assert DOM / drive the UI over the socket
node app/scripts/tauri-debug.mjs eval "[...document.querySelectorAll('button')].find(b=>b.textContent.trim()==='Forge')?.click()"
curl -s "$DUCKTAPE_QA_NODE_URL/v1/submit" -d '<op>'                    # seed node state; the app re-renders

ops/fleet.sh refresh                           # re-emit fleet.json so the dashboard reflects new state
ops/fleet.sh down <branch>                     # when done
```

- **Assertions go over the socket** (`eval` / `query_page`) — display-independent, the
  primary QA signal. Screenshots come from the dashboard tile (noVNC) or
  `import -window root` under the instance's `DISPLAY` (`fleet.sh status` lists it).
- **Feeding the dashboard:** `fleet.sh up` already writes the token→VNC file and starts
  x11vnc; `fleet.sh refresh` regenerates `fleet.json` (served at `/fleet.json`, polled by
  the console). So an agent "feeds" the dashboard just by calling `up` then `refresh` —
  no separate VNC plumbing. `fleet.json`'s "up" gate is exactly `/tmp/tauri-mcp-<id>.sock`
  present + the VNC port open, so a driveable instance is a visible one.

## Several worktrees at once

`ops/fleet.sh up` (no args) brings up every worktree; drive each via its own
`/tmp/tauri-mcp-<id>.sock`. Independent instances are a natural fit for parallel
agents — one per worktree, each socket-scoped.

## Caveat — isolation (read before trusting per-tile data)

There is a **verified isolation bug** in the current fleet bring-up: the app boots in
REMOTE (web-client) mode and, absent `VITE_DUCKTAPE_NODE_URL`, every tile dials the
shared `127.0.0.1:8844` — so tiles show the *same* node's data, not per-worktree
isolated state. UI/DOM QA is still valid; **node-backed data QA is not isolated until
the fix lands.** See `docs/superpowers/specs/2026-07-03-fleet-isolation-finding.md` for
the bug + the verified fix (per-instance node + `VITE_DUCKTAPE_NODE_URL` in
`fleet.sh up_one`).

## Notes

- **Dev only.** The debug socket rides the same dev-only `tauri-plugin-mcp` seam as
  [[tauri-debug]]; a release build opens nothing.
- Bring instances up/down with `ops/fleet.sh` (see `ops/README.md`); this skill never
  manages lifecycle itself — it drives what the fleet already runs. Pair with [[work]]
  for the worktrees.
