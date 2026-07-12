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
export FLEET_MODEL_PROVIDER=claude   # CLAUDE_MODEL defaults to haiku
"$FLEET" test cef-smoke notification-bell --jobs 1
```

Suites let the model choose only typed UI actions. `expect`, state, and IPC pass
conditions determine the result. Fleet enforces step, wall-time, token, and
repetition limits and persists actions, usage, semantic frames, console,
network, IPC, screenshot, and replay artifacts outside the model context.

Ducktape is CEF-only on `dev`; use `runtime: cef`. The plugin endpoint is a
debug-build seam and is intentionally absent from release builds.

### Action model: local binary execution, not an API key

Fleet ≥ `17b2b40` runs the suite's action-chooser through a LOCAL binary; no
`OPENAI_API_KEY` is needed (that requirement only exists on older fleet pins
whose sole provider was the OpenAI Responses API — `app/package.json` pins the
fleet revision; if `test` demands a key, the pin predates the binary runner).

Two providers, both verified green on the `notification-bell` suite:

- `FLEET_MODEL_PROVIDER=claude` — uses the `claude` CLI and its existing
  Claude Code login, which every dev in this repo already has. `CLAUDE_MODEL`
  defaults to `haiku` — the right tier for choosing one typed UI action per
  step; don't reach for a bigger model unless a suite's objective genuinely
  needs multi-step reasoning.
- `FLEET_MODEL_PROVIDER=codex` (fleet's default) — uses the `codex` CLI
  (`CODEX_MODEL` defaults to `gpt-5.3-codex-spark`). Requires a ChatGPT/Codex
  subscription, which not every dev has — prefer `claude` in shared docs and
  CI recipes.

### Writing pass conditions — what each kind can and cannot see

- **`expect` conditions are evaluated from step 0 and THROW on an absent
  element** (`BRIDGE_UNAVAILABLE` → the whole run dies as
  `infrastructure_failure` before the model acts). Only use `expect` for
  elements that exist at app boot. Never gate on something the objective is
  supposed to create.
- **`ipc` conditions currently see nothing under CEF** — the captured invoke
  ledger is empty (`tauri-agent ipc` returns `[]` even after real commands
  run), so an `ipc: {command, ok}` condition can never pass. Don't use them
  until IPC capture works in the CEF runtime.
- **`state` probes are the reliable post-action assertion.** Register a
  DOM-derived probe in `app/src/main.tsx`'s `WebviewAgentInstrumentation`
  install (`state: { probeName: () => … }`), then assert it. `state.key`
  resolves TOP-LEVEL only (`url` | `title` | `values` | `probes`) — a probe is
  matched by deep-equalling the whole `probes` map:

  ```json
  { "state": { "key": "probes", "equals": { "notifyDropdownOpen": true } } }
  ```

  (Deep-equal over the full map means every registered probe appears in
  `equals` — revisit if the probe set grows.)
- **Budget for the binary runner is much larger than the old API runner**: the
  semantic-tree observation rides through the model each step. `tokens: 30000`
  passes; 1–2k dies at step 0 with `token limit exceeded`. `repetitions` counts
  identical consecutive actions — a correct click that fails a broken pass
  condition burns one repetition per retry, so a too-strict budget converts a
  pass-condition bug into `repeated action limit exceeded`.

Worked example: `.tauri-agent/suites/notification-bell.json` (boot-safe button
`expect` + the `notifyDropdownOpen` probe). Diagnose failures from the run
artifacts under the instance directory: `run.json` (failure + message),
`actions.jsonl` (what the model actually did), `ipc.jsonl`, `failure.png`.

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
