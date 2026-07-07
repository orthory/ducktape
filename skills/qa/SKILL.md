---
name: qa
description: Use to DRIVE the fleet's real Ducktape desktop instances for agent QA — one live app per git worktree. The fleet (ops/fleet.sh + the dashboard) brings each worktree's app up headless, VNCs it, and exposes its tauri-plugin-agent debug endpoint; this skill drives that endpoint (screenshot, run JS, assert the DOM, seed data) so an agent can QA a branch while you watch it live in the dashboard. For the single already-running app, use tauri-debug directly.
---

# QA (drive the fleet)

The **fleet** (`ops/fleet.sh` + `ops/fleet-console`, the live noVNC grid) runs the
real Ducktape desktop app for each git worktree, headless, and exposes them all
through one token-multiplexed web port. Each instance already ships the driving
seam this skill needs: `ops/fleet.sh up_one` starts `tauri dev` with
`XDG_RUNTIME_DIR=$STATE/<id>` — the same dev-only [[tauri-debug]] bridge
(`tauri-plugin-agent`). Because the runtime base is per-instance, each app's
endpoint registry lands under its own state dir; the app id stays the constant
`com.ducktape.app` across worktrees. This skill is the **driving** layer over
that: an agent QAs a worktree's branch through its endpoint while you watch the
tile live.

`<id>` is the branch slugged (`re.sub` of non-alnum → `-`), e.g. `feat/x` → `feat-x`.

## The loop

```bash
ops/fleet.sh up <branch>                       # bring the worktree's app up (headless + VNC + dashboard tile)
export XDG_RUNTIME_DIR=<fleet-state>/<id>      # the instance's runtime base ($STATE/<id>)

app/scripts/tauri-agent tree --app com.ducktape.app                       # semantic tree / drive the UI
app/scripts/tauri-agent find --role button --name Forge --app com.ducktape.app
app/scripts/tauri-agent click @3 --app com.ducktape.app
app/scripts/tauri-agent eval "document.title" --app com.ducktape.app      # assert DOM
curl -s "$DUCKTAPE_QA_NODE_URL/v1/submit" -d '<op>'                       # seed node state; the app re-renders

ops/fleet.sh refresh                           # re-emit fleet.json so the dashboard reflects new state
ops/fleet.sh down <branch>                     # when done
```

- **Scope every call to the instance** by exporting the same
  `XDG_RUNTIME_DIR=$STATE/<id>` the fleet launched the app with, then
  `--app com.ducktape.app`. The app id is constant; the runtime base is what
  routes a driver call to one tile. (`$STATE` is `ops/fleet.sh`'s state dir; the
  endpoint is `$STATE/<id>/tauri-agent/com.ducktape.app/endpoint.json`.)
- **Assertions go over the endpoint** (`tree` / `find` / `eval`) —
  display-independent, the primary QA signal. Screenshots come from the dashboard
  tile (noVNC), `tauri-agent shot out.svg --app com.ducktape.app` (DOM-SVG,
  WM-free), or `import -window root` under the instance's `DISPLAY`
  (`fleet.sh status` lists it).
- **Feeding the dashboard:** `fleet.sh up` already writes the token→VNC file and
  starts x11vnc; `fleet.sh refresh` regenerates `fleet.json` (served at
  `/fleet.json`, polled by the console). So an agent "feeds" the dashboard just by
  calling `up` then `refresh` — no separate VNC plumbing. `fleet.json`'s "up" gate
  is exactly `$STATE/<id>/tauri-agent/com.ducktape.app/endpoint.json` present + the
  VNC port open, so a driveable instance is a visible one.

## Several worktrees at once

`ops/fleet.sh up` (no args) brings up every worktree; drive each by exporting its
own `XDG_RUNTIME_DIR=$STATE/<id>` + `--app com.ducktape.app`. Independent
instances are a natural fit for parallel agents — one per worktree, each
runtime-base-scoped.

## Isolation — root cause fixed (PR #90 on `dev`)

Tiles used to look un-isolated (all like the shared `8844`) because the headless app
never reached its workspace: a **React StrictMode boot race** in `DucktapeProvider`
dropped `connectActive`. Fixed in **PR #90** (merged to `dev`). To make a tile show a
**live isolated** workspace, the fleet worktree must be on `dev` (for #90) and
`fleet.sh up_one` must seed a workspace + set `DUCKTAPE_NODE_BIN` per instance — see
`docs/superpowers/specs/2026-07-03-fleet-isolation-finding.md`. Until a given tile has
that, its node-backed data is shared, not isolated (DOM/UI QA is valid regardless).

## Notes

- **Dev only.** The debug endpoint rides the same dev-only `tauri-plugin-agent` seam
  as [[tauri-debug]]; a release build registers nothing.
- Bring instances up/down with `ops/fleet.sh` (see `ops/README.md`); this skill never
  manages lifecycle itself — it drives what the fleet already runs. Pair with [[work]]
  for the worktrees.
