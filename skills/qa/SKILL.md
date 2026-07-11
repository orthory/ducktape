---
name: qa
description: Use to drive or test isolated real Ducktape CEF desktop instances managed by tauri-agent-fleet. Fleet builds once, launches private instances, exposes loopback VNC, and drives the direct tauri-agent endpoint. For one already-running app outside Fleet, use tauri-debug.
---

# QA with tauri-agent-fleet

Ducktape delegates generic build caching, process ownership, scheduling, runner
budgets, artifacts, and dashboard serving to `tauri-agent-fleet`. This repository
owns only `.tauri-agent/` and `qa/fleet/`.

## Interactive loop

```bash
FLEET="${FLEET:-app/node_modules/.bin/tauri-agent-fleet}"

"$FLEET" up HEAD
"$FLEET" status --json
"$FLEET" dashboard

# Pick the instance's directories.runtime value from status --json.
export XDG_RUNTIME_DIR=<runtime-directory>
app/scripts/tauri-agent tree --app com.ducktape.app
app/scripts/tauri-agent find --role button --name Create --app com.ducktape.app

"$FLEET" down <instance-id>
```

The opaque instance ID, not the branch, owns HOME, XDG runtime/data, display,
ports, endpoint, VNC token, and exact process groups. Never find or stop desktop
processes with `pkill -f`. Fleet runs Ducktape's cleanup hook after stopping the
desktop so its recorded detached workspace-node group cannot survive teardown.

## Deterministic suites

```bash
"$FLEET" test cef-smoke --jobs 1
```

Suites let the model choose only typed UI actions. `expect`, state, and IPC pass
conditions determine the result. Fleet enforces step, wall-time, token, and
repetition limits and persists actions, usage, semantic frames, console,
network, IPC, screenshot, and replay artifacts outside the model context.

Ducktape is CEF-only on `dev`; use `runtime: cef`. The plugin endpoint is a
debug-build seam and is intentionally absent from release builds.

## Several worktrees or same-artifact instances

```bash
"$FLEET" up dev agent/my-branch
"$FLEET" test cef-smoke cef-smoke --jobs 2
```

The first form builds each selected revision. The second builds the selected
revision/CEF runtime once and launches isolated instances from the same cached
artifact.

## Host prerequisite fallback

Fleet expects `Xvfb` and `x11vnc` on PATH. The existing remote-tauri staging can
be used during host migration:

```bash
export FLEET_VNC_COMMAND="$HOME/.local/opt/remote-tauri/root/usr/bin/x11vnc"
export LD_LIBRARY_PATH="$HOME/.local/opt/remote-tauri/root/usr/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
```
